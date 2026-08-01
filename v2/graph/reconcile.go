package graph

import (
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"sort"
	"strings"
)

// Action is a provider-neutral reconciliation request/decision.
type Action string

const (
	Admit    Action = "admit"
	Defer    Action = "defer"
	Ask      Action = "ask"
	Cancel   Action = "cancel"
	Replan   Action = "replan"
	Complete Action = "complete"
	Block    Action = "block"

	ActionAdmit    = Admit
	ActionDefer    = Defer
	ActionAsk      = Ask
	ActionCancel   = Cancel
	ActionReplan   = Replan
	ActionComplete = Complete
	ActionBlock    = Block
)

// Policy controls pure backpressure and feedback-loop guards. A zero limit
// means unlimited, except MaxActive where zero means unlimited as well.
type Policy struct {
	MaxActive                  int
	MaxCost                    uint64
	MaxSteps                   uint64
	MaxReplans                 int
	RequireProgressForComplete bool
}

// Budget records cumulative reservations. Admission consumes cost and steps;
// cancellation does not refund them, preserving a conservative budget.
type Budget struct {
	UsedCost  uint64
	UsedSteps uint64
}

// Snapshot is the complete input/output of reconciliation. It contains no
// handles to providers and can be copied or replayed safely.
type Snapshot struct {
	Graph     Graph
	Budget    Budget
	Replans   int
	SeenPlans map[string]bool
	Questions []string
}

// State is an alias useful to callers that model reconciliation as a state
// machine.
type State = Snapshot

// NewSnapshot returns a normalized snapshot and records the initial graph
// fingerprint, preventing a no-op replan from creating a feedback loop.
func NewSnapshot(g Graph) Snapshot {
	s := Snapshot{Graph: cloneGraph(g), SeenPlans: map[string]bool{}}
	s.SeenPlans[Fingerprint(g)] = true
	return s
}

// NewState is an alias for NewSnapshot.
func NewState(g Graph) Snapshot { return NewSnapshot(g) }

// Clone makes all nested graph and slice data independent.
func (s Snapshot) Clone() Snapshot {
	o := Snapshot{
		Graph: cloneGraph(s.Graph), Budget: s.Budget, Replans: s.Replans,
		SeenPlans: map[string]bool{}, Questions: append([]string(nil), s.Questions...),
	}
	for p, yes := range s.SeenPlans {
		o.SeenPlans[p] = yes
	}
	return o
}

// Request asks reconciliation to perform one guarded lifecycle transition.
// ID is required for every action except Replan of an entire graph.
type Request struct {
	Action   Action
	ID       ID
	Reason   string
	Progress float64
	Evidence []Evidence
	Question string
	Rewrite  *Rewrite
}

// Event is an alias for Request for event-oriented adapters.
type Event = Request

// Decision is the deterministic result for an adapter/executor.
type Decision struct {
	Action Action
	ID     ID
	Reason string
}

// Result carries both the new immutable snapshot and the proposal.
type Result struct {
	State    Snapshot
	Decision Decision
}

var (
	ErrUnknownNode   = errors.New("unknown node")
	ErrInvalidAction = errors.New("invalid action")
	ErrFeedbackLoop  = errors.New("replan feedback loop guarded")
	ErrReplanLimit   = errors.New("replan limit reached")
)

// Reconcile applies one request without mutating the input snapshot. Budget
// exhaustion and incomplete evidence are represented as defer/ask decisions,
// not provider-specific errors.
func Reconcile(s Snapshot, req Request, p Policy) (Result, error) {
	if err := s.Graph.Validate(); err != nil {
		return Result{}, err
	}
	n := s.Clone()
	if n.SeenPlans == nil {
		n.SeenPlans = map[string]bool{Fingerprint(n.Graph): true}
	}
	if req.Action == "" {
		return Result{}, ErrInvalidAction
	}
	if req.Action != Replan {
		if req.ID == "" {
			return Result{}, fmt.Errorf("%w: empty ID", ErrUnknownNode)
		}
		if _, ok := n.Graph.Node(req.ID); !ok {
			return Result{}, fmt.Errorf("%w %q", ErrUnknownNode, req.ID)
		}
	}
	decision := Decision{Action: req.Action, ID: req.ID, Reason: req.Reason}

	switch req.Action {
	case Admit:
		node, _ := n.Graph.Node(req.ID)
		if node.Status == Running {
			return Result{State: n, Decision: decision}, nil
		}
		if node.Status.terminal() {
			return Result{}, fmt.Errorf("cannot admit terminal node %q", req.ID)
		}
		// An explicit admission resolves a prior clarification request.
		if node.Status == Waiting {
			node.Status = Pending
			setNode(&n, node)
		}
		if !contains(n.Graph.Ready(nil), req.ID) {
			node.Status = Deferred
			setNode(&n, node)
			decision.Action, decision.Reason = Defer, "dependencies are not complete"
			return Result{State: n, Decision: decision}, nil
		}
		active, _ := activeUsage(n.Graph)
		if p.MaxActive > 0 && active >= p.MaxActive {
			node.Status = Deferred
			setNode(&n, node)
			decision.Action, decision.Reason = Defer, "active-task budget exhausted"
			return Result{State: n, Decision: decision}, nil
		}
		if p.MaxCost > 0 && (n.Budget.UsedCost > p.MaxCost || node.Cost > p.MaxCost-n.Budget.UsedCost) {
			node.Status = Deferred
			setNode(&n, node)
			decision.Action, decision.Reason = Defer, "cost budget exhausted"
			return Result{State: n, Decision: decision}, nil
		}
		if p.MaxSteps > 0 && n.Budget.UsedSteps >= p.MaxSteps {
			node.Status = Deferred
			setNode(&n, node)
			decision.Action, decision.Reason = Defer, "step budget exhausted"
			return Result{State: n, Decision: decision}, nil
		}
		node.Status = Running
		n.Budget.UsedCost += node.Cost
		n.Budget.UsedSteps++
		setNode(&n, node)
		return Result{State: n, Decision: decision}, nil

	case Defer:
		node, _ := n.Graph.Node(req.ID)
		if node.Status.terminal() {
			return Result{}, fmt.Errorf("cannot defer terminal node %q", req.ID)
		}
		node.Status = Deferred
		setNode(&n, node)
		decision.Reason = defaultReason(req.Reason, "deferred by policy")
		return Result{State: n, Decision: decision}, nil

	case Ask:
		node, _ := n.Graph.Node(req.ID)
		if node.Status.terminal() {
			return Result{}, fmt.Errorf("cannot ask about terminal node %q", req.ID)
		}
		node.Status = Waiting
		setNode(&n, node)
		if req.Question != "" {
			n.Questions = append(n.Questions, req.Question)
		}
		decision.Reason = defaultReason(req.Reason, req.Question)
		if decision.Reason == "" {
			decision.Reason = "clarification required"
		}
		return Result{State: n, Decision: decision}, nil

	case Cancel:
		node, _ := n.Graph.Node(req.ID)
		if node.Status == Succeeded {
			return Result{}, fmt.Errorf("cannot cancel succeeded node %q", req.ID)
		}
		node.Status = Cancelled
		setNode(&n, node)
		decision.Reason = defaultReason(req.Reason, "cancelled")
		return Result{State: n, Decision: decision}, nil

	case Block:
		node, _ := n.Graph.Node(req.ID)
		if node.Status == Succeeded || node.Status == Cancelled {
			return Result{}, fmt.Errorf("cannot block terminal node %q", req.ID)
		}
		node.Status = Blocked
		setNode(&n, node)
		decision.Reason = defaultReason(req.Reason, "blocked")
		return Result{State: n, Decision: decision}, nil

	case Complete:
		node, _ := n.Graph.Node(req.ID)
		if node.Status != Running {
			return Result{}, fmt.Errorf("cannot complete node %q in status %q", req.ID, node.Status)
		}
		progress := req.Progress
		if progress < node.Progress {
			progress = node.Progress
		}
		minimum := node.MinProgress
		if minimum == 0 {
			minimum = 1
		}
		if (p.RequireProgressForComplete || node.MinProgress > 0) && progress < minimum {
			decision.Action, decision.Reason = Ask, "completion requires additional progress"
			return Result{State: n, Decision: decision}, nil
		}
		if node.EvidenceRequired && len(req.Evidence) == 0 && len(node.Evidence) == 0 {
			decision.Action, decision.Reason = Ask, "completion requires evidence"
			return Result{State: n, Decision: decision}, nil
		}
		node.Progress = progress
		node.Evidence = append(node.Evidence, req.Evidence...)
		node.Status = Succeeded
		setNode(&n, node)
		return Result{State: n, Decision: decision}, nil

	case Replan:
		if req.Rewrite == nil {
			return Result{}, fmt.Errorf("replan requires a rewrite")
		}
		if p.MaxReplans > 0 && n.Replans >= p.MaxReplans {
			return guardedReplanBlock(n, req, decision, ErrReplanLimit)
		}
		g, err := n.Graph.Rewrite(*req.Rewrite)
		if err != nil {
			return Result{}, err
		}
		fingerprint := Fingerprint(g)
		if n.SeenPlans[fingerprint] {
			return guardedReplanBlock(n, req, decision, ErrFeedbackLoop)
		}
		n.Graph = g
		n.Replans++
		n.SeenPlans[fingerprint] = true
		decision.Reason = defaultReason(req.Reason, "graph replanned")
		return Result{State: n, Decision: decision}, nil

	default:
		return Result{}, fmt.Errorf("%w %q", ErrInvalidAction, req.Action)
	}
}

// Apply is an alias for Reconcile.
func Apply(s Snapshot, req Request, p Policy) (Result, error) {
	return Reconcile(s, req, p)
}

func guardedReplanBlock(s Snapshot, req Request, d Decision, cause error) (Result, error) {
	if req.ID != "" {
		if node, ok := s.Graph.Node(req.ID); ok && !node.Status.terminal() {
			node.Status = Blocked
			setNode(&s, node)
			d.Action, d.Reason = Block, cause.Error()
			return Result{State: s, Decision: d}, nil
		}
	}
	return Result{State: s, Decision: Decision{Action: Block, ID: req.ID, Reason: cause.Error()}}, nil
}

func activeUsage(g Graph) (int, uint64) {
	count := 0
	var cost uint64
	for _, n := range g.Nodes() {
		if n.Status == Running {
			count++
			cost += n.Cost
		}
	}
	return count, cost
}

func setNode(s *Snapshot, n Node) {
	// Graph is deliberately encapsulated; replacing through Rewrite preserves
	// validation and copy semantics.
	g, err := s.Graph.Rewrite(Rewrite{ReplaceDependencies: map[ID][]ID{n.ID: n.Dependencies}})
	if err != nil {
		return
	}
	// Rewrite only replaces edges, so overlay lifecycle fields from n.
	clone := cloneGraph(g)
	clone.nodes[n.ID] = cloneNode(n)
	s.Graph = clone
}

func contains(ids []ID, id ID) bool {
	for _, x := range ids {
		if x == id {
			return true
		}
	}
	return false
}

func defaultReason(got, fallback string) string {
	if got != "" {
		return got
	}
	return fallback
}

// Fingerprint returns a stable SHA-256 identity for graph structure. Lifecycle
// status is intentionally omitted so status-only feedback cannot evade the
// replan loop guard.
func Fingerprint(g Graph) string {
	var b strings.Builder
	for _, n := range g.Nodes() {
		b.WriteString(string(n.ID))
		b.WriteByte('|')
		b.WriteString(string(n.GoalID))
		b.WriteByte('|')
		b.WriteString(n.Description)
		b.WriteByte('|')
		b.WriteString(n.CapabilityRef)
		b.WriteByte('|')
		b.WriteString(n.PolicyRef)
		b.WriteByte('|')
		b.WriteString(n.GrantRef)
		b.WriteByte('|')
		for _, d := range sortedDeps(n.Dependencies) {
			b.WriteString(string(d))
			b.WriteByte(',')
		}
		b.WriteByte(';')
	}
	h := sha256.Sum256([]byte(b.String()))
	return hex.EncodeToString(h[:])
}

// Active returns running node IDs in stable order.
func (s Snapshot) Active() []ID {
	var ids []ID
	for _, n := range s.Graph.Nodes() {
		if n.Status == Running {
			ids = append(ids, n.ID)
		}
	}
	sort.Slice(ids, func(i, j int) bool { return ids[i] < ids[j] })
	return ids
}
