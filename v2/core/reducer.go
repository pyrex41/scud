package core

import (
	"fmt"
	"sort"
)

// Reduce applies one event without mutating the input state. Sequence zero is
// accepted for manually constructed events; non-zero sequences must be the
// next revision.
func Reduce(state State, event Event) (State, error) {
	if !event.Kind.valid() {
		return state, fmt.Errorf("invalid event kind %q", event.Kind)
	}
	if event.Sequence != 0 && event.Sequence != state.Revision+1 {
		return state, fmt.Errorf("event sequence %d does not follow revision %d", event.Sequence, state.Revision)
	}
	n := state.Clone()
	if n.Goals == nil {
		n.Goals = map[ID]Goal{}
	}
	if n.Obligations == nil {
		n.Obligations = map[ID]Obligation{}
	}
	if n.Observations == nil {
		n.Observations = map[ID]Observation{}
	}
	if event.ID != "" {
		for _, prior := range n.Events {
			if prior.ID == event.ID {
				return state, fmt.Errorf("duplicate event id %q", event.ID)
			}
		}
	}

	switch event.Kind {
	case EventGoalCreated:
		g := event.Goal
		if err := ValidateID(g.ID); err != nil {
			return state, err
		}
		if g.Status == "" {
			g.Status = GoalPending
		}
		if !g.Status.valid() {
			return state, fmt.Errorf("invalid goal status %q", g.Status)
		}
		if _, exists := n.Goals[g.ID]; exists {
			return state, fmt.Errorf("goal %q already exists", g.ID)
		}
		n.Goals[g.ID] = g
	case EventObligationAdded:
		o := event.Obligation
		if err := ValidateID(o.ID); err != nil {
			return state, err
		}
		if _, exists := n.Goals[o.GoalID]; !exists {
			return state, fmt.Errorf("unknown goal %q", o.GoalID)
		}
		if o.Status == "" {
			o.Status = ObligationPending
		}
		if !o.Status.valid() {
			return state, fmt.Errorf("invalid obligation status %q", o.Status)
		}
		if _, exists := n.Obligations[o.ID]; exists {
			return state, fmt.Errorf("obligation %q already exists", o.ID)
		}
		o.DependsOn = append([]ID(nil), o.DependsOn...)
		for _, dep := range o.DependsOn {
			d, exists := n.Obligations[dep]
			if !exists || d.GoalID != o.GoalID {
				return state, fmt.Errorf("obligation %q: unknown or cross-goal dependency %q", o.ID, dep)
			}
		}
		n.Obligations[o.ID] = o
		if err := validateAcyclic(n.Obligations); err != nil {
			return state, err
		}
	case EventObservationAdded:
		ob := event.Observation
		if err := ValidateID(ob.ID); err != nil {
			return state, err
		}
		if _, exists := n.Observations[ob.ID]; exists {
			return state, fmt.Errorf("observation %q already exists", ob.ID)
		}
		target, ok := n.Obligations[ob.ObligationID]
		if !ok || target.GoalID != ob.GoalID {
			return state, fmt.Errorf("observation targets unknown obligation %q", ob.ObligationID)
		}
		if !ob.Outcome.valid() {
			return state, fmt.Errorf("invalid observation outcome %q", ob.Outcome)
		}
		ob.Evidence = append([]Evidence(nil), ob.Evidence...)
		n.Observations[ob.ID] = ob
		switch ob.Outcome {
		case OutcomeSuccess:
			target.Status = ObligationSucceeded
		case OutcomeFailure:
			target.Status = ObligationFailed
		}
		n.Obligations[target.ID] = target
		refreshGoal(&n, target.GoalID)
	case EventDecisionIssued:
		d := event.Decision
		if err := validateDecision(n, d); err != nil {
			return state, err
		}
		if d.ID != "" {
			for _, prior := range n.Decisions {
				if prior.ID == d.ID {
					return state, fmt.Errorf("duplicate decision id %q", d.ID)
				}
			}
		}
		n.Decisions = append(n.Decisions, d)
		if d.ObligationID != "" {
			o := n.Obligations[d.ObligationID]
			switch d.Kind {
			case DecisionExecute:
				o.Status = ObligationRunning
			case DecisionCancel:
				o.Status = ObligationCancelled
			case DecisionBlock:
				o.Status = ObligationBlocked
			case DecisionSucceed:
				o.Status = ObligationSucceeded
			case DecisionFail:
				o.Status = ObligationFailed
			}
			n.Obligations[o.ID] = o
			refreshGoal(&n, o.GoalID)
		} else if g, ok := n.Goals[d.GoalID]; ok {
			switch d.Kind {
			case DecisionSucceed:
				g.Status = GoalSucceeded
			case DecisionFail:
				g.Status = GoalFailed
			case DecisionBlock:
				g.Status = GoalBlocked
			case DecisionCancel:
				g.Status = GoalCancelled
			case DecisionExhaust:
				g.Status = GoalExhausted
			}
			n.Goals[g.ID] = g
		}
	case EventBudgetConsumed:
		if event.Budget.Steps == 0 && event.Budget.Cost == 0 {
			return state, fmt.Errorf("budget delta is empty")
		}
		if ^uint64(0)-n.Budget.UsedSteps < event.Budget.Steps || ^uint64(0)-n.Budget.UsedCost < event.Budget.Cost {
			return state, fmt.Errorf("budget overflow")
		}
		n.Budget.UsedSteps += event.Budget.Steps
		n.Budget.UsedCost += event.Budget.Cost
		if n.Budget.MaxSteps != 0 && n.Budget.UsedSteps > n.Budget.MaxSteps || n.Budget.MaxCost != 0 && n.Budget.UsedCost > n.Budget.MaxCost {
			return state, fmt.Errorf("budget exceeded")
		}
	}

	n.Revision++
	e := cloneEvent(event)
	e.Sequence = n.Revision
	n.Events = append(n.Events, e)
	return n, nil
}

// Append assigns the next sequence number and applies event.
func (s State) Append(event Event) (State, error) {
	event.Sequence = s.Revision + 1
	return Reduce(s, event)
}

// Replay deterministically applies an event stream to an empty state. Zero
// sequence numbers are assigned their stream position.
func Replay(events []Event) (State, error) {
	s := NewState()
	for i, e := range events {
		if e.Sequence == 0 {
			e.Sequence = uint64(i + 1)
		}
		var err error
		s, err = Reduce(s, e)
		if err != nil {
			return State{}, fmt.Errorf("event %d: %w", i, err)
		}
	}
	return s, nil
}

func validateDecision(s State, d Decision) error {
	if !d.Kind.valid() {
		return fmt.Errorf("invalid decision kind %q", d.Kind)
	}
	g, ok := s.Goals[d.GoalID]
	if !ok {
		return fmt.Errorf("decision references unknown goal %q", d.GoalID)
	}
	if g.Status.terminal() && d.Kind == DecisionExecute {
		return fmt.Errorf("goal %q is terminal", d.GoalID)
	}
	if d.ObligationID != "" {
		o, ok := s.Obligations[d.ObligationID]
		if !ok || o.GoalID != d.GoalID {
			return fmt.Errorf("decision references invalid obligation %q", d.ObligationID)
		}
		if o.Status.terminal() && d.Kind == DecisionExecute {
			return fmt.Errorf("obligation %q is terminal", d.ObligationID)
		}
	}
	return nil
}

func refreshGoal(s *State, goalID ID) {
	g, ok := s.Goals[goalID]
	if !ok || g.Status.terminal() {
		return
	}
	count, succeeded := 0, 0
	for _, o := range s.Obligations {
		if o.GoalID != goalID {
			continue
		}
		count++
		if o.Status == ObligationSucceeded {
			succeeded++
		}
		if o.Status == ObligationFailed {
			g.Status = GoalFailed
			s.Goals[goalID] = g
			return
		}
		if o.Status == ObligationBlocked {
			g.Status = GoalBlocked
			s.Goals[goalID] = g
			return
		}
	}
	if count > 0 && succeeded == count {
		g.Status = GoalSucceeded
	} else if count > 0 {
		g.Status = GoalRunning
	}
	s.Goals[goalID] = g
}

// Reconcile computes the next deterministic proposal. It considers goals and
// obligations by lexicographic ID, so map iteration order cannot affect it.
func Reconcile(s State) (Decision, error) {
	if err := s.Validate(); err != nil {
		return Decision{}, err
	}
	goals := make([]ID, 0, len(s.Goals))
	for id := range s.Goals {
		goals = append(goals, id)
	}
	sort.Slice(goals, func(i, j int) bool { return goals[i] < goals[j] })
	for _, gid := range goals {
		g := s.Goals[gid]
		if g.Status.terminal() {
			continue
		}
		if s.Budget.MaxSteps != 0 && s.Budget.UsedSteps >= s.Budget.MaxSteps || s.Budget.MaxCost != 0 && s.Budget.UsedCost >= s.Budget.MaxCost {
			return Decision{ID: ID(fmt.Sprintf("decision:%d:%s", s.Revision+1, gid)), GoalID: gid, Kind: DecisionExhaust, Reason: "budget exhausted"}, nil
		}
		var nodes []Obligation
		for _, o := range s.Obligations {
			if o.GoalID == gid {
				nodes = append(nodes, o)
			}
		}
		sort.Slice(nodes, func(i, j int) bool { return nodes[i].ID < nodes[j].ID })
		if len(nodes) == 0 {
			return Decision{ID: ID(fmt.Sprintf("decision:%d:%s", s.Revision+1, gid)), GoalID: gid, Kind: DecisionWait, Reason: "goal has no obligations"}, nil
		}
		for _, o := range nodes {
			status := o.Status
			if status == "" {
				status = ObligationPending
			}
			if status == ObligationFailed {
				return Decision{ID: ID(fmt.Sprintf("decision:%d:%s", s.Revision+1, gid)), GoalID: gid, ObligationID: o.ID, Kind: DecisionFail, Reason: "obligation failed"}, nil
			}
			if status == ObligationBlocked {
				return Decision{ID: ID(fmt.Sprintf("decision:%d:%s", s.Revision+1, gid)), GoalID: gid, ObligationID: o.ID, Kind: DecisionBlock, Reason: "obligation blocked"}, nil
			}
			if status == ObligationCancelled {
				return Decision{ID: ID(fmt.Sprintf("decision:%d:%s", s.Revision+1, gid)), GoalID: gid, ObligationID: o.ID, Kind: DecisionBlock, Reason: "obligation cancelled"}, nil
			}
		}
		for _, o := range nodes {
			status := o.Status
			if status == "" {
				status = ObligationPending
			}
			if status != ObligationPending && status != ObligationReady {
				continue
			}
			for _, dep := range o.DependsOn {
				if s.Obligations[dep].Status == ObligationFailed || s.Obligations[dep].Status == ObligationBlocked || s.Obligations[dep].Status == ObligationCancelled {
					return Decision{ID: ID(fmt.Sprintf("decision:%d:%s", s.Revision+1, gid)), GoalID: gid, ObligationID: o.ID, Kind: DecisionBlock, Reason: "dependency cannot complete"}, nil
				}
			}
		}
		for _, o := range nodes {
			status := o.Status
			if status == "" {
				status = ObligationPending
			}
			if status != ObligationPending && status != ObligationReady {
				continue
			}
			ready := true
			for _, dep := range o.DependsOn {
				if s.Obligations[dep].Status != ObligationSucceeded {
					ready = false
					break
				}
			}
			if ready {
				return Decision{ID: ID(fmt.Sprintf("decision:%d:%s:%s", s.Revision+1, gid, o.ID)), GoalID: gid, ObligationID: o.ID, Kind: DecisionExecute, Reason: "dependencies satisfied"}, nil
			}
		}
		allDone := true
		for _, o := range nodes {
			status := o.Status
			if status == "" {
				status = ObligationPending
			}
			if !status.terminal() {
				allDone = false
				break
			}
		}
		if allDone {
			return Decision{ID: ID(fmt.Sprintf("decision:%d:%s", s.Revision+1, gid)), GoalID: gid, Kind: DecisionSucceed, Reason: "all obligations terminal"}, nil
		}
		return Decision{ID: ID(fmt.Sprintf("decision:%d:%s", s.Revision+1, gid)), GoalID: gid, Kind: DecisionWait, Reason: "awaiting observations"}, nil
	}
	return Decision{}, fmt.Errorf("no active goals")
}
