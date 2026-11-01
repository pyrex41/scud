// Package taskapi provides validated, transactional access to the task graph.
package taskapi

import (
	"bytes"
	"cmp"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"reflect"
	"slices"
	"strings"
	"time"
	"unicode"

	"github.com/reuben/scud/internal/storage"
	"github.com/reuben/scud/pkg/model"
)

type Phase struct {
	Tag      string        `json:"tag"`
	Name     string        `json:"name"`
	IDFormat string        `json:"id_format"`
	Tasks    []*model.Task `json:"tasks"`
}
type Document struct {
	SchemaVersion int     `json:"schema_version"`
	Phases        []Phase `json:"phases"`
}
type Ref struct {
	Tag string `json:"tag"`
	ID  string `json:"id"`
}
type Blocked struct {
	Ref     Ref      `json:"ref"`
	Reasons []string `json:"reasons"`
}
type Summary struct {
	Total  int                      `json:"total"`
	Status map[model.TaskStatus]int `json:"status"`
}
type Snapshot struct {
	SchemaVersion int       `json:"schema_version"`
	Revision      string    `json:"revision"`
	Phases        []Phase   `json:"phases"`
	Summary       Summary   `json:"summary"`
	Ready         []Ref     `json:"ready"`
	Blocked       []Blocked `json:"blocked"`
}
type ImportOptions struct{ Policy, ExpectedRevision string }
type Patch struct {
	Status       *model.TaskStatus `json:"status,omitempty"`
	Title        *string           `json:"title,omitempty"`
	Description  *string           `json:"description,omitempty"`
	Details      *string           `json:"details,omitempty"`
	TestStrategy *string           `json:"test_strategy,omitempty"`
	Priority     *model.Priority   `json:"priority,omitempty"`
	Complexity   *int              `json:"complexity,omitempty"`
	Dependencies *[]string         `json:"dependencies,omitempty"`
	AssignedTo   *string           `json:"assigned_to,omitempty"`
}
type Service struct{ root string }

func Open(root string) (*Service, error) {
	root, err := filepath.Abs(root)
	if err != nil {
		return nil, err
	}
	root, err = storage.FindRoot(root)
	if err != nil {
		return nil, err
	}
	return &Service{root}, nil
}

func (s *Service) file() string { return filepath.Join(s.root, ".scud", "tasks", "tasks.json") }
func (s *Service) locked(ctx context.Context, fn func() (Snapshot, error)) (Snapshot, error) {
	if err := ctx.Err(); err != nil {
		return Snapshot{}, err
	}
	if err := os.MkdirAll(filepath.Dir(s.file()), 0755); err != nil {
		return Snapshot{}, err
	}
	f, err := os.OpenFile(s.file()+".taskapi.lock", os.O_CREATE|os.O_RDWR, 0600)
	if err != nil {
		return Snapshot{}, err
	}
	defer f.Close()
	if err = lock(ctx, f); err != nil {
		return Snapshot{}, err
	}
	defer unlock(f)
	if err := ctx.Err(); err != nil {
		return Snapshot{}, err
	}
	return fn()
}
func (s *Service) read() (Document, error) {
	legacyFile, err := os.Open(filepath.Join(s.root, ".scud", "tasks", "tasks.scg"))
	if err != nil && !os.IsNotExist(err) {
		return Document{}, err
	}
	if err == nil {
		legacy, readErr := io.ReadAll(io.LimitReader(legacyFile, MaxJSONBytes+1))
		_ = legacyFile.Close()
		if readErr != nil {
			return Document{}, readErr
		}
		if len(legacy) > MaxJSONBytes || len(bytes.TrimSpace(legacy)) != 0 {
			return Document{}, fmt.Errorf("nonempty legacy .scud/tasks/tasks.scg requires explicit conversion to a version 1 JSON document; archive the legacy file before importing (it is not read or modified)")
		}
	}
	f, err := os.Open(s.file())
	if os.IsNotExist(err) {
		return Document{SchemaVersion: 1, Phases: []Phase{}}, nil
	} else if err != nil {
		return Document{}, err
	}
	defer f.Close()
	var d Document
	if err := DecodeJSON(f, &d); err != nil {
		return Document{}, fmt.Errorf("invalid tasks.json: %w", err)
	}
	return d, nil
}
func (s *Service) write(ctx context.Context, d Document) error {
	b, err := json.MarshalIndent(d, "", "  ")
	if err != nil {
		return err
	}
	if len(b) > MaxJSONBytes {
		return fmt.Errorf("resulting task document exceeds %d bytes; no changes published", MaxJSONBytes)
	}
	f, err := os.CreateTemp(filepath.Dir(s.file()), ".tasks-*")
	if err != nil {
		return err
	}
	name := f.Name()
	defer os.Remove(name)
	if err = f.Chmod(0644); err == nil {
		_, err = f.Write(b)
	}
	if err == nil {
		err = f.Sync()
	}
	e := f.Close()
	if err == nil {
		err = e
	}
	if err != nil {
		return err
	}
	if err = ctx.Err(); err != nil {
		return err
	}
	if err = os.Rename(name, s.file()); err != nil {
		return err
	}
	if err = syncDirectory(filepath.Dir(s.file())); err != nil {
		return fmt.Errorf("task file published but directory sync failed: %w", err)
	}
	return nil
}
func (s *Service) Snapshot(ctx context.Context) (Snapshot, error) {
	return s.locked(ctx, func() (Snapshot, error) {
		d, e := s.read()
		if e != nil {
			return Snapshot{}, e
		}
		return build(d)
	})
}
func (s *Service) Validate(ctx context.Context) (Snapshot, error) { return s.Snapshot(ctx) }

// ValidateDocument validates an independent document without reading or writing a project.
// A subsequent merge or replace is also validated against the current project.
func ValidateDocument(d Document) (Snapshot, error) { return build(d) }

// FilterDocument exports an independently importable document. An empty tag
// includes every phase; otherwise it includes the named phase and the transitive
// closure of whole phases required by dependencies of any included task. Parents
// and subtasks are same-phase and are therefore retained too. Phases and tasks
// are canonically sorted; dependency/subtask sequences retain their input order.
// The input is not modified. Unknown tags and invalid documents return errors.
func FilterDocument(d Document, tag string) (Document, error) {
	snap, err := build(d)
	if err != nil {
		return Document{}, err
	}
	all := Document{SchemaVersion: 1, Phases: snap.Phases}
	if tag == "" {
		return all, nil
	}
	byTag := make(map[string]Phase, len(all.Phases))
	for _, p := range all.Phases {
		byTag[p.Tag] = p
	}
	if _, ok := byTag[tag]; !ok {
		return Document{}, fmt.Errorf("phase not found: %s", tag)
	}
	included := map[string]bool{tag: true}
	queue := []string{tag}
	for len(queue) > 0 {
		current := queue[0]
		queue = queue[1:]
		for _, t := range byTag[current].Tasks {
			for _, dep := range t.Dependencies {
				depTag, _, qualified := strings.Cut(dep, ":")
				if qualified && !included[depTag] {
					included[depTag] = true
					queue = append(queue, depTag)
				}
			}
		}
	}
	out := Document{SchemaVersion: 1, Phases: []Phase{}}
	for _, p := range all.Phases {
		if included[p.Tag] {
			out.Phases = append(out.Phases, p)
		}
	}
	return out, nil
}

func (s *Service) Import(ctx context.Context, in Document, opts ImportOptions) (Snapshot, error) {
	return s.locked(ctx, func() (Snapshot, error) {
		d, e := s.read()
		if e != nil {
			return Snapshot{}, e
		}
		current, e := build(d)
		if e != nil {
			return Snapshot{}, e
		}
		if opts.ExpectedRevision != "" && opts.ExpectedRevision != current.Revision {
			return Snapshot{}, &RevisionConflictError{Expected: opts.ExpectedRevision, Actual: current.Revision}
		}
		if in.SchemaVersion != 1 {
			return Snapshot{}, fmt.Errorf("unsupported schema version %d", in.SchemaVersion)
		}
		if opts.Policy != "" && opts.Policy != "replace" && opts.Policy != "merge" {
			return Snapshot{}, fmt.Errorf("invalid import policy %q", opts.Policy)
		}
		seen := map[string]bool{}
		for _, p := range in.Phases {
			if seen[p.Tag] {
				return Snapshot{}, fmt.Errorf("duplicate phase %q", p.Tag)
			}
			seen[p.Tag] = true
			found := -1
			for i := range d.Phases {
				if d.Phases[i].Tag == p.Tag {
					found = i
					break
				}
			}
			if found < 0 {
				d.Phases = append(d.Phases, p)
			} else if opts.Policy == "replace" {
				d.Phases[found] = p
			} else if opts.Policy == "merge" {
				if d.Phases[found].Name != p.Name || d.Phases[found].IDFormat != p.IDFormat {
					return Snapshot{}, fmt.Errorf("phase metadata mismatch: %s", p.Tag)
				}
				d.Phases[found].Tasks = append(d.Phases[found].Tasks, p.Tasks...)
			} else {
				return Snapshot{}, fmt.Errorf("phase %q already exists", p.Tag)
			}
		}
		_, e = build(d)
		if e != nil {
			return Snapshot{}, e
		}
		if e = s.write(ctx, d); e != nil {
			return Snapshot{}, e
		}
		d, e = s.read()
		if e != nil {
			return Snapshot{}, e
		}
		return build(d)
	})
}
func (s *Service) Update(ctx context.Context, ref Ref, p Patch, expectedRevision string) (Snapshot, error) {
	return s.locked(ctx, func() (Snapshot, error) {
		d, e := s.read()
		if e != nil {
			return Snapshot{}, e
		}
		before, e := build(d)
		if e != nil {
			return Snapshot{}, e
		}
		if expectedRevision != "" && expectedRevision != before.Revision {
			return Snapshot{}, &RevisionConflictError{Expected: expectedRevision, Actual: before.Revision}
		}
		t, _, e := find(d, ref)
		if e != nil {
			return Snapshot{}, e
		}
		original := *t
		if p.Status != nil {
			t.Status = *p.Status
		}
		if p.Title != nil {
			t.Title = *p.Title
		}
		if p.Description != nil {
			t.Description = *p.Description
		}
		if p.Details != nil {
			t.Details = *p.Details
		}
		if p.TestStrategy != nil {
			t.TestStrategy = *p.TestStrategy
		}
		if p.Priority != nil {
			t.Priority = *p.Priority
		}
		if p.Complexity != nil {
			t.Complexity = *p.Complexity
		}
		if p.Dependencies != nil && !slices.Equal(t.Dependencies, *p.Dependencies) {
			t.Dependencies = slices.Clone(*p.Dependencies)
		}
		if p.AssignedTo != nil {
			t.AssignedTo = *p.AssignedTo
		}
		if reflect.DeepEqual(original, *t) {
			return before, nil
		}
		t.UpdatedAt = time.Now().UTC().Format(time.RFC3339Nano)
		_, e = build(d)
		if e != nil {
			return Snapshot{}, e
		}
		if e = s.write(ctx, d); e != nil {
			return Snapshot{}, e
		}
		d, e = s.read()
		if e != nil {
			return Snapshot{}, e
		}
		return build(d)
	})
}
func find(d Document, r Ref) (*model.Task, Ref, error) {
	if strings.Contains(r.ID, ":") {
		parts := strings.SplitN(r.ID, ":", 2)
		if r.Tag != "" && r.Tag != parts[0] {
			return nil, Ref{}, fmt.Errorf("conflicting phase tags")
		}
		r.Tag = parts[0]
		r.ID = parts[1]
	}
	var result *model.Task
	var actual Ref
	for _, p := range d.Phases {
		if r.Tag != "" && p.Tag != r.Tag {
			continue
		}
		for _, t := range p.Tasks {
			if t.ID == r.ID {
				if result != nil {
					return nil, Ref{}, fmt.Errorf("ambiguous task %q", r.ID)
				}
				result = t
				actual = Ref{p.Tag, t.ID}
			}
		}
	}
	if result == nil {
		return nil, Ref{}, fmt.Errorf("task not found: %s", r.ID)
	}
	return result, actual, nil
}
func (s *Service) ReadTask(ctx context.Context, r Ref) (*model.Task, Ref, error) {
	snap, e := s.Snapshot(ctx)
	if e != nil {
		return nil, Ref{}, e
	}
	return find(Document{1, snap.Phases}, r)
}

func build(d Document) (Snapshot, error) {
	if d.SchemaVersion != 1 {
		return Snapshot{}, fmt.Errorf("unsupported schema version")
	}
	if d.Phases == nil {
		d.Phases = []Phase{}
	}
	// Canonicalize equivalent nil/empty collections without mutating callers.
	// Nonempty dependency and subtask sequences are ordered, not sorted sets.
	d.Phases = slices.Clone(d.Phases)
	for i := range d.Phases {
		d.Phases[i].Tasks = slices.Clone(d.Phases[i].Tasks)
		for j, t := range d.Phases[i].Tasks {
			if t == nil {
				continue // The validation below reports invalid tasks.
			}
			copy := *t
			copy.Dependencies = append([]string{}, t.Dependencies...)
			copy.Subtasks = append([]string(nil), t.Subtasks...)
			d.Phases[i].Tasks[j] = &copy
		}
	}
	index := map[string]*model.Task{}
	tags := map[string]bool{}
	for _, p := range d.Phases {
		if !validIdentifier(p.Tag) || strings.TrimSpace(p.Name) == "" || (p.IDFormat != "sequential" && p.IDFormat != "uuid") {
			return Snapshot{}, fmt.Errorf("invalid phase %q", p.Tag)
		}
		if tags[p.Tag] {
			return Snapshot{}, fmt.Errorf("duplicate phase %q", p.Tag)
		}
		tags[p.Tag] = true
		for _, t := range p.Tasks {
			if t == nil || !validIdentifier(t.ID) || strings.TrimSpace(t.Title) == "" {
				return Snapshot{}, fmt.Errorf("invalid task in %s", p.Tag)
			}
			k := p.Tag + ":" + t.ID
			if index[k] != nil {
				return Snapshot{}, fmt.Errorf("duplicate task %s", k)
			}
			if st, ok := model.ParseStatus(string(t.Status)); !ok || st != t.Status {
				return Snapshot{}, fmt.Errorf("invalid status for %s", k)
			}
			if pr, ok := model.ParsePriority(string(t.Priority)); !ok || pr != t.Priority {
				return Snapshot{}, fmt.Errorf("invalid priority for %s", k)
			}
			if t.Complexity < 0 {
				return Snapshot{}, fmt.Errorf("invalid complexity for %s", k)
			}
			index[k] = t
		}
	}
	resolve := func(tag, id string) (string, error) {
		if !strings.Contains(id, ":") {
			id = tag + ":" + id
		}
		if index[id] == nil {
			return "", fmt.Errorf("missing task %s", id)
		}
		return id, nil
	}
	edges := map[string][]string{}
	for k, t := range index {
		tag := strings.SplitN(k, ":", 2)[0]
		depSeen := map[string]bool{}
		for _, dep := range t.Dependencies {
			v, e := resolve(tag, dep)
			if e != nil {
				return Snapshot{}, e
			}
			if v == k {
				return Snapshot{}, fmt.Errorf("self dependency at %s", k)
			}
			if depSeen[v] {
				return Snapshot{}, fmt.Errorf("duplicate dependency %s on %s", v, k)
			}
			depSeen[v] = true
		}
		if t.ParentID != "" && !validIdentifier(t.ParentID) {
			return Snapshot{}, fmt.Errorf("parent must be a same-phase task ID for %s", k)
		}
		parents := map[string]bool{}
		inherited := map[string]bool{}
		cur := t
		for cur != nil {
			for _, dep := range cur.Dependencies {
				v, e := resolve(tag, dep)
				if e != nil {
					return Snapshot{}, e
				}
				if !inherited[v] {
					edges[k] = append(edges[k], v)
					inherited[v] = true
				}
			}
			if cur.ParentID == "" {
				break
			}
			v, e := resolve(tag, cur.ParentID)
			if e != nil {
				return Snapshot{}, e
			}
			if parents[v] || v == k {
				return Snapshot{}, fmt.Errorf("parent cycle at %s", k)
			}
			parents[v] = true
			cur = index[v]
		}
		subSeen := map[string]bool{}
		for _, sub := range t.Subtasks {
			if !validIdentifier(sub) {
				return Snapshot{}, fmt.Errorf("subtask must be a same-phase task ID for %s", k)
			}
			v, e := resolve(tag, sub)
			if e != nil {
				return Snapshot{}, e
			}
			if v == k || subSeen[v] {
				return Snapshot{}, fmt.Errorf("duplicate or self subtask %s on %s", v, k)
			}
			subSeen[v] = true
			if index[v].ParentID != t.ID {
				return Snapshot{}, fmt.Errorf("inconsistent parent for %s", v)
			}
		}
		if t.ParentID != "" {
			p := index[tag+":"+t.ParentID]
			if p == nil || !contains(p.Subtasks, t.ID) {
				return Snapshot{}, fmt.Errorf("inconsistent parent for %s", k)
			}
		}
	}
	state := map[string]int{}
	var visit func(string) error
	visit = func(k string) error {
		if state[k] == 1 {
			return fmt.Errorf("dependency cycle at %s", k)
		}
		if state[k] == 2 {
			return nil
		}
		state[k] = 1
		for _, v := range edges[k] {
			if e := visit(v); e != nil {
				return e
			}
		}
		state[k] = 2
		return nil
	}
	for k := range index {
		if e := visit(k); e != nil {
			return Snapshot{}, e
		}
	}
	slices.SortFunc(d.Phases, func(a, b Phase) int { return cmp.Compare(a.Tag, b.Tag) })
	for i := range d.Phases {
		if d.Phases[i].Tasks == nil {
			d.Phases[i].Tasks = []*model.Task{}
		}
		slices.SortFunc(d.Phases[i].Tasks, func(a, b *model.Task) int {
			n := model.CompareIDs(a.ID, b.ID)
			if n == 0 {
				return cmp.Compare(a.ID, b.ID)
			}
			return n
		})
	}
	b, _ := json.Marshal(d)
	hash := sha256.Sum256(b)
	snap := Snapshot{1, hex.EncodeToString(hash[:]), d.Phases, Summary{Status: map[model.TaskStatus]int{}}, []Ref{}, []Blocked{}}
	for _, status := range []model.TaskStatus{model.Pending, model.InProgress, model.Done, model.Failed, model.Blocked, model.Review, model.Expanded, model.Deferred, model.Cancelled} {
		snap.Summary.Status[status] = 0
	}
	for _, p := range d.Phases {
		for _, t := range p.Tasks {
			snap.Summary.Total++
			snap.Summary.Status[t.Status]++
			if t.Status != model.Pending || len(t.Subtasks) > 0 {
				continue
			}
			r := Ref{p.Tag, t.ID}
			reasons := []string{}
			for _, v := range edges[p.Tag+":"+t.ID] {
				if index[v].Status != model.Done {
					reasons = append(reasons, fmt.Sprintf("dependency %s is %s", v, index[v].Status))
				}
			}
			if len(reasons) == 0 {
				snap.Ready = append(snap.Ready, r)
			} else {
				snap.Blocked = append(snap.Blocked, Blocked{r, reasons})
			}
		}
	}
	return snap, nil
}

// IDs and phase tags cannot contain a colon (the reference separator),
// whitespace, controls, slashes, backslashes or vertical bars.
func validIdentifier(s string) bool {
	if s == "" || s == "." || s == ".." {
		return false
	}
	for _, r := range s {
		if unicode.IsSpace(r) || unicode.IsControl(r) || strings.ContainsRune(":/\\|", r) {
			return false
		}
	}
	return true
}
func contains(a []string, s string) bool {
	for _, v := range a {
		if v == s {
			return true
		}
	}
	return false
}

// PollInterval clamps callers away from busy polling.
func PollInterval(d time.Duration) time.Duration {
	if d < 100*time.Millisecond {
		return 100 * time.Millisecond
	}
	return d
}
