// Package graph contains deterministic, provider-neutral graph operations for
// SCUD v2. It intentionally owns no persistence or execution concerns.
package graph

import (
	"fmt"
	"sort"
	"strings"
)

// ID identifies a node in a graph.
type ID string

// Status is the lifecycle state used by the reconciliation layer. A zero
// status is treated as Pending.
type Status string

const (
	Pending   Status = "pending"
	Ready     Status = "ready"
	Running   Status = "running"
	Succeeded Status = "succeeded"
	Failed    Status = "failed"
	Blocked   Status = "blocked"
	Cancelled Status = "cancelled"
	Deferred  Status = "deferred"
	Waiting   Status = "waiting"
)

func (s Status) normalized() Status {
	if s == "" {
		return Pending
	}
	return s
}

func (s Status) candidate() bool {
	s = s.normalized()
	return s == Pending || s == Ready || s == Deferred
}

func (s Status) terminal() bool {
	s = s.normalized()
	return s == Succeeded || s == Failed || s == Blocked || s == Cancelled
}

func (s Status) valid() bool {
	s = s.normalized()
	switch s {
	case Pending, Ready, Running, Succeeded, Failed, Blocked, Cancelled, Deferred, Waiting:
		return true
	default:
		return false
	}
}

// Evidence is a small provider-neutral proof attached to a node. Value can be
// a summary, digest, or opaque reference; graph does not interpret it.
type Evidence struct {
	ID    ID
	Kind  string
	Value string
	URI   string
	Hash  string
}

// Node is a graph vertex. Dependencies are directed edges from this node to
// its prerequisites. Cost is reserved when the node is admitted.
type Node struct {
	ID               ID
	GoalID           ID
	Description      string
	CapabilityRef    string
	PolicyRef        string
	GrantRef         string
	Dependencies     []ID
	Status           Status
	Cost             uint64
	Priority         int
	Progress         float64
	MinProgress      float64
	EvidenceRequired bool
	Evidence         []Evidence
}

// Graph is an immutable-by-convention graph value. All mutating operations
// return a copy, which makes reconciliation snapshots safe to retain.
type Graph struct {
	nodes map[ID]Node
}

// New constructs a graph from nodes. For validation errors (including
// duplicate IDs), use Build; New is convenient for literals in tests and
// callers that validate immediately afterwards.
func New(nodes ...Node) Graph {
	g := Graph{nodes: make(map[ID]Node, len(nodes))}
	for _, n := range nodes {
		n = cloneNode(n)
		if n.Status == "" {
			n.Status = Pending
		}
		g.nodes[n.ID] = n
	}
	return g
}

// NewGraph is an explicit alias for New.
func NewGraph(nodes ...Node) Graph { return New(nodes...) }

// Build validates and constructs a graph atomically.
func Build(nodes []Node) (Graph, error) {
	seen := make(map[ID]struct{}, len(nodes))
	for _, n := range nodes {
		if n.ID == "" {
			return Graph{}, fmt.Errorf("node id is required")
		}
		if _, ok := seen[n.ID]; ok {
			return Graph{}, fmt.Errorf("duplicate node %q", n.ID)
		}
		seen[n.ID] = struct{}{}
	}
	g := New(nodes...)
	if err := g.Validate(); err != nil {
		return Graph{}, err
	}
	return g, nil
}

// Nodes returns all nodes in stable ID order.
func (g Graph) Nodes() []Node {
	ids := g.ids()
	out := make([]Node, 0, len(ids))
	for _, id := range ids {
		out = append(out, cloneNode(g.nodes[id]))
	}
	return out
}

// Node returns a copy of the requested node.
func (g Graph) Node(id ID) (Node, bool) {
	n, ok := g.nodes[id]
	if !ok {
		return Node{}, false
	}
	return cloneNode(n), true
}

// IDs returns all node IDs in stable order.
func (g Graph) IDs() []ID { return g.ids() }

func (g Graph) ids() []ID {
	ids := make([]ID, 0, len(g.nodes))
	for id := range g.nodes {
		ids = append(ids, id)
	}
	sort.Slice(ids, func(i, j int) bool { return ids[i] < ids[j] })
	return ids
}

// Report describes every structural graph error, rather than stopping at the
// first one. Missing dependencies and cycles are sorted deterministically.
type Report struct {
	EmptyIDs              []ID
	InvalidStatuses       []StatusIssue
	MissingDependencies   []MissingDependency
	DuplicateDependencies []DuplicateDependency
	Cycles                [][]ID
}

type StatusIssue struct {
	Node   ID
	Status Status
}

type MissingDependency struct {
	Node       ID
	Dependency ID
}

type DuplicateDependency struct {
	Node       ID
	Dependency ID
}

func (r Report) Valid() bool {
	return len(r.EmptyIDs) == 0 && len(r.InvalidStatuses) == 0 && len(r.MissingDependencies) == 0 && len(r.DuplicateDependencies) == 0 && len(r.Cycles) == 0
}

// Err converts a report to a compact, useful validation error.
func (r Report) Err() error {
	if r.Valid() {
		return nil
	}
	parts := make([]string, 0, 4)
	if len(r.EmptyIDs) > 0 {
		parts = append(parts, fmt.Sprintf("empty node IDs: %v", r.EmptyIDs))
	}
	if len(r.InvalidStatuses) > 0 {
		parts = append(parts, fmt.Sprintf("invalid statuses: %v", r.InvalidStatuses))
	}
	if len(r.MissingDependencies) > 0 {
		parts = append(parts, fmt.Sprintf("missing dependencies: %v", r.MissingDependencies))
	}
	if len(r.DuplicateDependencies) > 0 {
		parts = append(parts, fmt.Sprintf("duplicate dependencies: %v", r.DuplicateDependencies))
	}
	if len(r.Cycles) > 0 {
		parts = append(parts, fmt.Sprintf("cycles: %v", r.Cycles))
	}
	return fmt.Errorf("invalid graph: %s", strings.Join(parts, "; "))
}

// Check returns a complete structural report.
func (g Graph) Check() Report {
	r := Report{}
	for _, id := range g.ids() {
		if id == "" {
			r.EmptyIDs = append(r.EmptyIDs, id)
		}
	}
	for _, id := range g.ids() {
		n := g.nodes[id]
		if !n.Status.valid() {
			r.InvalidStatuses = append(r.InvalidStatuses, StatusIssue{Node: id, Status: n.Status})
		}
		seen := map[ID]bool{}
		for _, dep := range n.Dependencies {
			if seen[dep] {
				r.DuplicateDependencies = append(r.DuplicateDependencies, DuplicateDependency{Node: id, Dependency: dep})
			}
			if _, ok := g.nodes[dep]; !ok && !seen[dep] {
				r.MissingDependencies = append(r.MissingDependencies, MissingDependency{Node: id, Dependency: dep})
			}
			seen[dep] = true
		}
	}
	sort.Slice(r.MissingDependencies, func(i, j int) bool {
		if r.MissingDependencies[i].Node == r.MissingDependencies[j].Node {
			return r.MissingDependencies[i].Dependency < r.MissingDependencies[j].Dependency
		}
		return r.MissingDependencies[i].Node < r.MissingDependencies[j].Node
	})
	sort.Slice(r.DuplicateDependencies, func(i, j int) bool {
		if r.DuplicateDependencies[i].Node == r.DuplicateDependencies[j].Node {
			return r.DuplicateDependencies[i].Dependency < r.DuplicateDependencies[j].Dependency
		}
		return r.DuplicateDependencies[i].Node < r.DuplicateDependencies[j].Node
	})
	r.Cycles = findCycles(g)
	return r
}

// Cycles returns a deterministic copy of all detected cycle paths.
func (g Graph) Cycles() [][]ID {
	r := g.Check()
	cycles := make([][]ID, len(r.Cycles))
	for i, cycle := range r.Cycles {
		cycles[i] = append([]ID(nil), cycle...)
	}
	return cycles
}

// MissingDependencies returns all missing edge references in stable order.
func (g Graph) MissingDependencies() []MissingDependency {
	return append([]MissingDependency(nil), g.Check().MissingDependencies...)
}

// Validate checks IDs, references, duplicate edges, and acyclicity.
func (g Graph) Validate() error { return g.Check().Err() }

// Validate is a package-level convenience for callers with a graph value.
func Validate(g Graph) error { return g.Validate() }

// CycleError is returned when wave planning encounters a cycle.
type CycleError struct{ Cycles [][]ID }

func (e CycleError) Error() string { return fmt.Sprintf("graph contains cycle(s): %v", e.Cycles) }

func findCycles(g Graph) [][]ID {
	marks := map[ID]uint8{}
	stack := []ID{}
	cycles := [][]ID{}
	seen := map[string]bool{}
	var visit func(ID)
	visit = func(id ID) {
		marks[id] = 1
		stack = append(stack, id)
		for _, dep := range sortedDeps(g.nodes[id].Dependencies) {
			if _, ok := g.nodes[dep]; !ok {
				continue
			}
			switch marks[dep] {
			case 0:
				visit(dep)
			case 1:
				start := 0
				for i, x := range stack {
					if x == dep {
						start = i
						break
					}
				}
				cycle := append([]ID(nil), stack[start:]...)
				cycle = append(cycle, dep)
				key := cycleKey(cycle)
				if !seen[key] {
					seen[key] = true
					cycles = append(cycles, cycle)
				}
			}
		}
		stack = stack[:len(stack)-1]
		marks[id] = 2
	}
	for _, id := range g.ids() {
		if marks[id] == 0 {
			visit(id)
		}
	}
	sort.Slice(cycles, func(i, j int) bool { return cycleKey(cycles[i]) < cycleKey(cycles[j]) })
	return cycles
}

func cycleKey(c []ID) string {
	parts := make([]string, len(c))
	for i, id := range c {
		parts[i] = string(id)
	}
	return strings.Join(parts, "\x00")
}

func sortedDeps(in []ID) []ID {
	out := append([]ID(nil), in...)
	sort.Slice(out, func(i, j int) bool { return out[i] < out[j] })
	return out
}

// Ready returns candidates whose dependencies are succeeded (or listed in
// completed). The result is always lexicographically sorted.
func (g Graph) Ready(completed map[ID]bool) []ID {
	done := map[ID]bool{}
	for id, yes := range completed {
		if yes {
			done[id] = true
		}
	}
	for _, n := range g.Nodes() {
		if n.Status.normalized() == Succeeded {
			done[n.ID] = true
		}
	}
	ready := []ID{}
	for _, n := range g.Nodes() {
		if !n.Status.candidate() {
			continue
		}
		ok := true
		for _, dep := range n.Dependencies {
			if !done[dep] {
				ok = false
				break
			}
		}
		if ok {
			ready = append(ready, n.ID)
		}
	}
	return ready
}

// ReadySet is an alias for Ready.
func (g Graph) ReadySet(completed map[ID]bool) []ID { return g.Ready(completed) }

// Waves computes deterministic parallel waves. Nodes blocked by a running or
// terminal-unsuccessful prerequisite are omitted; structural errors are
// returned. A cycle returns a CycleError and no partial plan.
func (g Graph) Waves(completed map[ID]bool) ([][]ID, error) {
	if err := g.Validate(); err != nil {
		if len(g.Check().Cycles) > 0 {
			return nil, CycleError{Cycles: g.Check().Cycles}
		}
		return nil, err
	}
	done := map[ID]bool{}
	for id, yes := range completed {
		if yes {
			done[id] = true
		}
	}
	for _, n := range g.Nodes() {
		if n.Status.normalized() == Succeeded {
			done[n.ID] = true
		}
	}
	remaining := map[ID]bool{}
	for _, n := range g.Nodes() {
		if n.Status.candidate() && !done[n.ID] {
			remaining[n.ID] = true
		}
	}
	var waves [][]ID
	for len(remaining) > 0 {
		ready := make([]ID, 0)
		for _, id := range g.ids() {
			if !remaining[id] {
				continue
			}
			n := g.nodes[id]
			ok := true
			for _, dep := range n.Dependencies {
				if remaining[dep] || !done[dep] {
					ok = false
					break
				}
			}
			if ok {
				ready = append(ready, id)
			}
		}
		if len(ready) == 0 {
			// A non-candidate prerequisite can legitimately leave nodes out of
			// the plan; only report cycles among still-plannable candidates.
			cycleNodes := make([]Node, 0, len(remaining))
			for id := range remaining {
				cycleNodes = append(cycleNodes, g.nodes[id])
			}
			if cg, err := Build(cycleNodes); err == nil {
				if c := findCycles(cg); len(c) > 0 {
					return nil, CycleError{Cycles: c}
				}
			}
			break
		}
		waves = append(waves, ready)
		for _, id := range ready {
			delete(remaining, id)
			done[id] = true
		}
	}
	return waves, nil
}

// Plan is an alias for Waves.
func (g Graph) Plan(completed map[ID]bool) ([][]ID, error) { return g.Waves(completed) }

// TopologicalOrder flattens Waves while retaining deterministic wave order.
func (g Graph) TopologicalOrder(completed map[ID]bool) ([]ID, error) {
	waves, err := g.Waves(completed)
	if err != nil {
		return nil, err
	}
	var ids []ID
	for _, wave := range waves {
		ids = append(ids, wave...)
	}
	return ids, nil
}

// Rewrite describes an atomic graph rewrite. AddNodes and RemoveIDs are
// applied first, followed by dependency replacements and edge additions/removals.
type Rewrite struct {
	AddNodes            []Node
	RemoveIDs           []ID
	ReplaceDependencies map[ID][]ID
	AddDependencies     map[ID][]ID
	RemoveDependencies  map[ID][]ID
}

// Rewrite applies and validates a graph rewrite without mutating the source.
func (g Graph) Rewrite(r Rewrite) (Graph, error) {
	next := cloneGraph(g)
	for _, id := range r.RemoveIDs {
		if _, ok := next.nodes[id]; !ok {
			return Graph{}, fmt.Errorf("cannot remove unknown node %q", id)
		}
		delete(next.nodes, id)
	}
	for _, n := range r.AddNodes {
		if n.ID == "" {
			return Graph{}, fmt.Errorf("added node id is required")
		}
		if _, ok := next.nodes[n.ID]; ok {
			return Graph{}, fmt.Errorf("node %q already exists", n.ID)
		}
		n.Status = n.Status.normalized()
		next.nodes[n.ID] = cloneNode(n)
	}
	for id, deps := range r.ReplaceDependencies {
		n, ok := next.nodes[id]
		if !ok {
			return Graph{}, fmt.Errorf("cannot rewrite unknown node %q", id)
		}
		n.Dependencies = append([]ID(nil), deps...)
		next.nodes[id] = n
	}
	for id, deps := range r.AddDependencies {
		n, ok := next.nodes[id]
		if !ok {
			return Graph{}, fmt.Errorf("cannot add dependency to unknown node %q", id)
		}
		seen := map[ID]bool{}
		for _, d := range n.Dependencies {
			seen[d] = true
		}
		for _, d := range deps {
			if !seen[d] {
				n.Dependencies = append(n.Dependencies, d)
				seen[d] = true
			}
		}
		next.nodes[id] = n
	}
	for id, deps := range r.RemoveDependencies {
		n, ok := next.nodes[id]
		if !ok {
			return Graph{}, fmt.Errorf("cannot remove dependency from unknown node %q", id)
		}
		remove := map[ID]bool{}
		for _, d := range deps {
			remove[d] = true
		}
		kept := n.Dependencies[:0]
		for _, d := range n.Dependencies {
			if !remove[d] {
				kept = append(kept, d)
			}
		}
		n.Dependencies = append([]ID(nil), kept...)
		next.nodes[id] = n
	}
	if err := next.Validate(); err != nil {
		return Graph{}, err
	}
	return next, nil
}

// ApplyRewrite is a descriptive alias for Rewrite.
func (g Graph) ApplyRewrite(r Rewrite) (Graph, error) { return g.Rewrite(r) }

// AddNode returns a graph with one node added.
func (g Graph) AddNode(n Node) (Graph, error) { return g.Rewrite(Rewrite{AddNodes: []Node{n}}) }

// RemoveNode removes a node. Dependents must be rewritten explicitly.
func (g Graph) RemoveNode(id ID) (Graph, error) { return g.Rewrite(Rewrite{RemoveIDs: []ID{id}}) }

// SetDependencies replaces one node's dependency list.
func (g Graph) SetDependencies(id ID, deps []ID) (Graph, error) {
	return g.Rewrite(Rewrite{ReplaceDependencies: map[ID][]ID{id: deps}})
}

// AddDependency adds one edge if it is not already present.
func (g Graph) AddDependency(id, dependency ID) (Graph, error) {
	return g.Rewrite(Rewrite{AddDependencies: map[ID][]ID{id: {dependency}}})
}

// RemoveDependency removes one edge.
func (g Graph) RemoveDependency(id, dependency ID) (Graph, error) {
	return g.Rewrite(Rewrite{RemoveDependencies: map[ID][]ID{id: {dependency}}})
}

func cloneGraph(g Graph) Graph {
	n := Graph{nodes: make(map[ID]Node, len(g.nodes))}
	for id, node := range g.nodes {
		n.nodes[id] = cloneNode(node)
	}
	return n
}

func cloneNode(n Node) Node {
	n.Dependencies = append([]ID(nil), n.Dependencies...)
	n.Evidence = append([]Evidence(nil), n.Evidence...)
	if n.Status == "" {
		n.Status = Pending
	}
	return n
}
