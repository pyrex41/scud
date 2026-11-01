package taskapi

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"

	"github.com/reuben/scud/pkg/model"
)

func setup(t *testing.T) *Service {
	t.Helper()
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, ".scud", "tasks"), 0755); err != nil {
		t.Fatal(err)
	}
	s, e := Open(root)
	if e != nil {
		t.Fatal(e)
	}
	return s
}
func task(id string) *model.Task {
	return &model.Task{ID: id, Title: id, Status: model.Pending, Priority: model.Medium, Complexity: 1, ModelOverride: "model", CreatedAt: "2026-01-01", Details: "prose\n\nmore"}
}
func TestWorkflow(t *testing.T) {
	ctx := context.Background()
	s := setup(t)
	a, b := task("1"), task("1")
	b.Dependencies = []string{"a:1"}
	d := Document{1, []Phase{{"a", "Phase A", "sequential", []*model.Task{a}}, {"b", "Phase B", "sequential", []*model.Task{b}}}}
	snap, e := s.Import(ctx, d, ImportOptions{})
	if e != nil {
		t.Fatal(e)
	}
	if len(snap.Ready) != 1 || len(snap.Blocked) != 1 {
		t.Fatalf("bad readiness: %+v", snap)
	}
	if _, _, e = s.ReadTask(ctx, Ref{ID: "1"}); e == nil {
		t.Fatal("ambiguous id accepted")
	}
	persisted, e := s.Snapshot(ctx)
	if e != nil {
		t.Fatal(e)
	}
	if snap.Revision != persisted.Revision {
		t.Fatal("unstable revision")
	}
	encoded, _ := json.Marshal(Document{1, persisted.Phases})
	var round Document
	if e = json.Unmarshal(encoded, &round); e != nil {
		t.Fatal(e)
	}
	other := setup(t)
	if _, e = other.Import(ctx, round, ImportOptions{}); e != nil {
		t.Fatal(e)
	}
	read, _, e := other.ReadTask(ctx, Ref{Tag: "a", ID: "1"})
	if e != nil || read.ModelOverride != "model" || read.Details != a.Details || read.CreatedAt != a.CreatedAt {
		t.Fatalf("lossy round trip: %+v %v", read, e)
	}
	done := model.Done
	if _, e = s.Update(ctx, Ref{Tag: "a", ID: "1"}, Patch{Status: &done}, "wrong"); e == nil {
		t.Fatal("stale revision accepted")
	}
	snap, e = s.Update(ctx, Ref{Tag: "a", ID: "1"}, Patch{Status: &done}, snap.Revision)
	if e != nil {
		t.Fatal(e)
	}
	if len(snap.Ready) != 1 || snap.Ready[0].Tag != "b" {
		t.Fatalf("cross phase dependency not released: %+v", snap.Ready)
	}
}
func TestInvalidImportIsAtomic(t *testing.T) {
	ctx := context.Background()
	s := setup(t)
	good := Document{1, []Phase{{"a", "A", "sequential", []*model.Task{task("1")}}}}
	before, e := s.Import(ctx, good, ImportOptions{})
	if e != nil {
		t.Fatal(e)
	}
	for _, deps := range [][]string{{"missing"}, {"2"}} {
		x := task("2")
		x.Dependencies = deps
		bad := Document{1, []Phase{{"b", "B", "sequential", []*model.Task{x}}}}
		if _, e = s.Import(ctx, bad, ImportOptions{}); e == nil {
			t.Fatal("invalid dependency accepted")
		}
		after, e := s.Snapshot(ctx)
		if e != nil || after.Revision != before.Revision {
			t.Fatalf("partial write: %v", e)
		}
	}
	dup := Document{1, []Phase{{"b", "B", "sequential", []*model.Task{task("2"), task("2")}}}}
	if _, e = s.Import(ctx, dup, ImportOptions{}); e == nil {
		t.Fatal("duplicate accepted")
	}
}
func TestConcurrentUpdates(t *testing.T) {
	ctx := context.Background()
	s := setup(t)
	var tasks []*model.Task
	for i := 0; i < 16; i++ {
		tasks = append(tasks, task(string(rune('a'+i))))
	}
	_, e := s.Import(ctx, Document{1, []Phase{{"a", "A", "sequential", tasks}}}, ImportOptions{})
	if e != nil {
		t.Fatal(e)
	}
	var wg sync.WaitGroup
	errs := make(chan error, 16)
	for _, tt := range tasks {
		wg.Add(1)
		go func(id string) {
			defer wg.Done()
			v := model.Done
			_, e := s.Update(ctx, Ref{Tag: "a", ID: id}, Patch{Status: &v}, "")
			errs <- e
		}(tt.ID)
	}
	wg.Wait()
	close(errs)
	for e := range errs {
		if e != nil {
			t.Fatal(e)
		}
	}
	snap, e := s.Snapshot(ctx)
	if e != nil || snap.Summary.Status[model.Done] != 16 {
		t.Fatalf("lost update: %+v %v", snap.Summary, e)
	}
}
func TestLegacyRejected(t *testing.T) {
	s := setup(t)
	if e := os.WriteFile(filepath.Join(s.root, ".scud/tasks/tasks.scg"), []byte("legacy"), 0644); e != nil {
		t.Fatal(e)
	}
	if _, e := s.Snapshot(context.Background()); e == nil {
		t.Fatal("legacy silently shadowed")
	}
}

func TestStrictJSON(t *testing.T) {
	for _, input := range []string{`{"schema_version":1,"schema_version":1,"phases":[]}`, `{"schema_version":1,"phases":[{"tag":"a","Tag":"b"}]}`, `{"title":"x","title":"y"}`, `{} {}`} {
		var v any
		if e := DecodeJSON(strings.NewReader(input), &v); e == nil {
			t.Fatalf("accepted %s", input)
		}
	}
}

func TestPatchAndRevisionGuards(t *testing.T) {
	ctx := context.Background()
	s := setup(t)
	one, two := task("1"), task("2")
	two.UpdatedAt = "old"
	d := Document{1, []Phase{{"a", "A", "sequential", []*model.Task{one, two}}}}
	initial, e := s.Import(ctx, d, ImportOptions{})
	if e != nil {
		t.Fatal(e)
	}
	noOp, e := s.Update(ctx, Ref{Tag: "a", ID: "2"}, Patch{}, initial.Revision)
	if e != nil || noOp.Revision != initial.Revision {
		t.Fatalf("no-op changed revision: %v", e)
	}
	got, _, e := s.ReadTask(ctx, Ref{Tag: "a", ID: "2"})
	if e != nil || got.UpdatedAt != "old" {
		t.Fatalf("no-op timestamp: %+v %v", got, e)
	}
	deps := []string{"1"}
	assignee := "someone"
	changed, e := s.Update(ctx, Ref{Tag: "a", ID: "2"}, Patch{Dependencies: &deps, AssignedTo: &assignee}, initial.Revision)
	if e != nil {
		t.Fatal(e)
	}
	if len(changed.Blocked) != 1 {
		t.Fatalf("not blocked: %+v", changed.Blocked)
	}
	got, _, _ = s.ReadTask(ctx, Ref{Tag: "a", ID: "2"})
	if got.UpdatedAt == "old" || got.AssignedTo != assignee {
		t.Fatalf("patch not applied: %+v", got)
	}
	stamp := got.UpdatedAt
	same, e := s.Update(ctx, Ref{Tag: "a", ID: "2"}, Patch{AssignedTo: &assignee}, changed.Revision)
	if e != nil || same.Revision != changed.Revision {
		t.Fatalf("identical patch changed revision: %v", e)
	}
	got, _, e = s.ReadTask(ctx, Ref{Tag: "a", ID: "2"})
	if e != nil || got.UpdatedAt != stamp {
		t.Fatal("identical patch changed timestamp")
	}
	if _, e = s.Import(ctx, d, ImportOptions{Policy: "replace", ExpectedRevision: initial.Revision}); e == nil {
		t.Fatal("stale import accepted")
	}
	empty := []string{}
	changed, e = s.Update(ctx, Ref{Tag: "a", ID: "2"}, Patch{Dependencies: &empty}, changed.Revision)
	if e != nil || len(changed.Ready) != 2 {
		t.Fatalf("clear dependencies: %v %+v", e, changed.Ready)
	}
	canceled, cancel := context.WithCancel(ctx)
	cancel()
	if _, e = s.Import(canceled, d, ImportOptions{Policy: "replace"}); e == nil {
		t.Fatal("canceled import accepted")
	}
	after, e := s.Snapshot(ctx)
	if e != nil || after.Revision != changed.Revision {
		t.Fatal("canceled import changed data")
	}
}

func TestGraphStrictnessAndDeterminism(t *testing.T) {
	base := func() *model.Task { return task("1") }
	for _, bad := range []string{"bad tag", "bad\ttag", "bad\\tag", "bad/tag", "bad:tag"} {
		if _, e := ValidateDocument(Document{1, []Phase{{bad, "Name", "sequential", []*model.Task{base()}}}}); e == nil {
			t.Fatalf("invalid tag %q", bad)
		}
	}
	for _, deps := range [][]string{{"1"}, {"2", "a:2"}} {
		a, b := base(), task("2")
		a.Dependencies = deps
		if _, e := ValidateDocument(Document{1, []Phase{{"a", "A", "sequential", []*model.Task{a, b}}}}); e == nil {
			t.Fatalf("accepted dependencies %v", deps)
		}
	}
	a, b := base(), task("2")
	a.Subtasks = []string{"2", "2"}
	b.ParentID = "1"
	if _, e := ValidateDocument(Document{1, []Phase{{"a", "A", "sequential", []*model.Task{a, b}}}}); e == nil {
		t.Fatal("duplicate subtask accepted")
	}
	// Parent depends on its child: the inherited edge makes the child depend on itself.
	a.Subtasks = []string{"2"}
	a.Dependencies = []string{"2"}
	if _, e := ValidateDocument(Document{1, []Phase{{"a", "A", "sequential", []*model.Task{a, b}}}}); e == nil {
		t.Fatal("inherited cycle accepted")
	}
	x, y := task("1"), task("01")
	first, e := ValidateDocument(Document{1, []Phase{{"a", "A", "sequential", []*model.Task{x, y}}}})
	if e != nil {
		t.Fatal(e)
	}
	second, e := ValidateDocument(Document{1, []Phase{{"a", "A", "sequential", []*model.Task{y, x}}}})
	if e != nil || first.Revision != second.Revision {
		t.Fatal("natural sort tie unstable")
	}
	empty1, _ := ValidateDocument(Document{SchemaVersion: 1})
	empty2, _ := ValidateDocument(Document{SchemaVersion: 1, Phases: []Phase{}})
	if empty1.Revision != empty2.Revision {
		t.Fatal("empty revision unstable")
	}
}

func TestImportRejectsCorruptExisting(t *testing.T) {
	s := setup(t)
	bad := Document{1, []Phase{{"a", "A", "sequential", []*model.Task{task("1"), task("1")}}}}
	data, _ := json.Marshal(bad)
	if e := os.WriteFile(s.file(), data, 0644); e != nil {
		t.Fatal(e)
	}
	good := Document{1, []Phase{{"a", "A", "sequential", []*model.Task{task("1")}}}}
	if _, e := s.Import(context.Background(), good, ImportOptions{Policy: "replace"}); e == nil {
		t.Fatal("silently repaired corrupt project")
	}
	after, e := os.ReadFile(s.file())
	if e != nil || string(after) != string(data) {
		t.Fatal("corrupt project changed")
	}
}
