package graph

import (
	"fmt"

	"github.com/reuben/scud/v2/core"
)

// Status conversion is intentionally strict. Core has no deferred or waiting
// obligation states: those are reconciliation-layer states and must not be
// silently serialized as succeeded (or otherwise terminal).
func FromCoreStatus(s core.ObligationStatus) (Status, error) {
	switch s {
	case "", core.ObligationPending:
		return Pending, nil
	case core.ObligationReady:
		return Ready, nil
	case core.ObligationRunning:
		return Running, nil
	case core.ObligationSucceeded:
		return Succeeded, nil
	case core.ObligationFailed:
		return Failed, nil
	case core.ObligationBlocked:
		return Blocked, nil
	case core.ObligationCancelled:
		return Cancelled, nil
	default:
		return "", fmt.Errorf("unsupported core obligation status %q", s)
	}
}

func ToCoreStatus(s Status) (core.ObligationStatus, error) {
	switch s.normalized() {
	case Pending:
		return core.ObligationPending, nil
	case Ready:
		return core.ObligationReady, nil
	case Running:
		return core.ObligationRunning, nil
	case Succeeded:
		return core.ObligationSucceeded, nil
	case Failed:
		return core.ObligationFailed, nil
	case Blocked:
		return core.ObligationBlocked, nil
	case Cancelled:
		return core.ObligationCancelled, nil
	case Deferred, Waiting:
		return "", fmt.Errorf("graph status %q is reconciliation-only and has no core representation", s)
	default:
		return "", fmt.Errorf("unsupported graph status %q", s)
	}
}

// FromCore converts core obligations to a validated graph. GoalID and
// capability/policy references remain available to core adapters; graph owns
// only dependency and lifecycle scheduling fields.
func FromCore(obligations []core.Obligation) (Graph, error) {
	nodes := make([]Node, 0, len(obligations))
	for _, o := range obligations {
		status, err := FromCoreStatus(o.Status)
		if err != nil {
			return Graph{}, fmt.Errorf("obligation %q: %w", o.ID, err)
		}
		nodes = append(nodes, Node{ID: ID(o.ID), GoalID: ID(o.GoalID), Description: o.Description, CapabilityRef: o.CapabilityRef, PolicyRef: o.PolicyRef, GrantRef: o.GrantRef, Dependencies: idsFromCore(o.DependsOn), Status: status})
	}
	return Build(nodes)
}

// FromCoreObligations is an explicit alias for FromCore.
func FromCoreObligations(obligations []core.Obligation) (Graph, error) {
	return FromCore(obligations)
}

// ToCore converts a graph to obligations for one goal. It rejects
// reconciliation-only states rather than silently changing their meaning.
func ToCore(g Graph, goalID core.ID) ([]core.Obligation, error) {
	if goalID == "" {
		return nil, fmt.Errorf("goal ID is required")
	}
	if err := g.Validate(); err != nil {
		return nil, err
	}
	out := make([]core.Obligation, 0, len(g.nodes))
	for _, n := range g.Nodes() {
		status, err := ToCoreStatus(n.Status)
		if err != nil {
			return nil, fmt.Errorf("node %q: %w", n.ID, err)
		}
		var deps []core.ID
		if n.Dependencies != nil {
			deps = make([]core.ID, len(n.Dependencies))
			for i, dep := range n.Dependencies {
				deps[i] = core.ID(dep)
			}
		}
		out = append(out, core.Obligation{ID: core.ID(n.ID), GoalID: goalID, Description: n.Description, CapabilityRef: n.CapabilityRef, PolicyRef: n.PolicyRef, GrantRef: n.GrantRef, DependsOn: deps, Status: status})
	}
	return out, nil
}

// ToCoreObligations is an explicit alias for ToCore.
func ToCoreObligations(g Graph, goalID core.ID) ([]core.Obligation, error) {
	return ToCore(g, goalID)
}

// FromCoreEvidence preserves every field that core exposes. Evidence remains
// opaque to graph; reconciliation only checks presence when required.
func FromCoreEvidence(in []core.Evidence) []Evidence {
	if in == nil {
		return nil
	}
	out := make([]Evidence, len(in))
	for i, e := range in {
		out[i] = Evidence{ID: ID(e.ID), Kind: e.Kind, Value: e.Value, URI: e.URI, Hash: e.Hash}
	}
	return out
}

func ToCoreEvidence(in []Evidence) []core.Evidence {
	if in == nil {
		return nil
	}
	out := make([]core.Evidence, len(in))
	for i, e := range in {
		out[i] = core.Evidence{ID: core.ID(e.ID), Kind: e.Kind, Value: e.Value, URI: e.URI, Hash: e.Hash}
	}
	return out
}

// FromCoreState selects one goal's obligations and carries cumulative budget
// usage into a reconciliation snapshot. Core remains authoritative for event
// reduction; graph is a planning/reconciliation view.
func FromCoreState(s core.State, goalID core.ID) (Snapshot, error) {
	if err := s.Validate(); err != nil {
		return Snapshot{}, err
	}
	if goalID == "" {
		return Snapshot{}, fmt.Errorf("goal ID is required")
	}
	if _, ok := s.Goals[goalID]; !ok {
		return Snapshot{}, fmt.Errorf("unknown goal %q", goalID)
	}
	var obligations []core.Obligation
	for _, o := range s.Obligations {
		if o.GoalID == goalID {
			obligations = append(obligations, o)
		}
	}
	g, err := FromCore(obligations)
	if err != nil {
		return Snapshot{}, err
	}
	return Snapshot{Graph: g, Budget: Budget{UsedCost: s.Budget.UsedCost, UsedSteps: s.Budget.UsedSteps}, SeenPlans: map[string]bool{Fingerprint(g): true}}, nil
}

// ToCoreState creates a minimal core state containing one goal and this graph.
// It is intended for adapters/tests; event history and observation history are
// not fabricated by this pure bridge.
func ToCoreState(s Snapshot, goal core.Goal) (core.State, error) {
	if goal.ID == "" {
		return core.State{}, fmt.Errorf("goal ID is required")
	}
	obligations, err := ToCore(s.Graph, goal.ID)
	if err != nil {
		return core.State{}, err
	}
	out := core.NewState()
	out.Goals[goal.ID] = goal
	for _, o := range obligations {
		out.Obligations[o.ID] = o
	}
	out.Budget.UsedCost, out.Budget.UsedSteps = s.Budget.UsedCost, s.Budget.UsedSteps
	if err := out.Validate(); err != nil {
		return core.State{}, err
	}
	return out, nil
}

func idsFromCore(in []core.ID) []ID {
	if in == nil {
		return nil
	}
	out := make([]ID, len(in))
	for i, id := range in {
		out[i] = ID(id)
	}
	return out
}
