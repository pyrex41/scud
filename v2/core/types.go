package core

import "fmt"

// ID identifies a domain object. IDs are compared lexicographically when the
// kernel needs a stable ordering.
type ID string

// GoalStatus is the lifecycle state of a goal.
type GoalStatus string

const (
	GoalPending   GoalStatus = "pending"
	GoalRunning   GoalStatus = "running"
	GoalSucceeded GoalStatus = "succeeded"
	GoalFailed    GoalStatus = "failed"
	GoalBlocked   GoalStatus = "blocked"
	GoalCancelled GoalStatus = "cancelled"
	GoalExhausted GoalStatus = "exhausted"
)

func (s GoalStatus) valid() bool {
	switch s {
	case GoalPending, GoalRunning, GoalSucceeded, GoalFailed, GoalBlocked, GoalCancelled, GoalExhausted:
		return true
	default:
		return false
	}
}

func (s GoalStatus) terminal() bool {
	return s == GoalSucceeded || s == GoalFailed || s == GoalBlocked || s == GoalCancelled || s == GoalExhausted
}

// IsTerminal reports whether a goal can no longer be advanced.
func (s GoalStatus) IsTerminal() bool { return s.terminal() }

// ObligationStatus is the lifecycle state of one DAG node.
type ObligationStatus string

const (
	ObligationPending   ObligationStatus = "pending"
	ObligationReady     ObligationStatus = "ready"
	ObligationRunning   ObligationStatus = "running"
	ObligationSucceeded ObligationStatus = "succeeded"
	ObligationFailed    ObligationStatus = "failed"
	ObligationBlocked   ObligationStatus = "blocked"
	ObligationCancelled ObligationStatus = "cancelled"
)

func (s ObligationStatus) valid() bool {
	switch s {
	case ObligationPending, ObligationReady, ObligationRunning, ObligationSucceeded, ObligationFailed, ObligationBlocked, ObligationCancelled:
		return true
	default:
		return false
	}
}

func (s ObligationStatus) terminal() bool {
	return s == ObligationSucceeded || s == ObligationFailed || s == ObligationBlocked || s == ObligationCancelled
}

// IsTerminal reports whether an obligation can no longer be advanced.
func (s ObligationStatus) IsTerminal() bool { return s.terminal() }

// Goal describes the unit being reconciled. Status is normally maintained by
// the reducer; a zero status is interpreted as pending by NewState.
type Goal struct {
	ID          ID         `json:"id"`
	Title       string     `json:"title,omitempty"`
	Description string     `json:"description,omitempty"`
	Status      GoalStatus `json:"status"`
}

// Obligation is a node in a goal's dependency DAG. Dependencies must belong to
// the same goal and are completed before this node can run.
type Obligation struct {
	ID            ID               `json:"id"`
	GoalID        ID               `json:"goal_id"`
	Description   string           `json:"description,omitempty"`
	DependsOn     []ID             `json:"depends_on,omitempty"`
	CapabilityRef string           `json:"capability_ref,omitempty"`
	PolicyRef     string           `json:"policy_ref,omitempty"`
	GrantRef      string           `json:"grant_ref,omitempty"`
	Status        ObligationStatus `json:"status"`
}

// Observation is an external, provider-neutral report about an obligation.
// The kernel consumes observations but does not interpret their Evidence
// beyond retaining it for replay and auditing.
type Observation struct {
	ID           ID         `json:"id"`
	GoalID       ID         `json:"goal_id"`
	ObligationID ID         `json:"obligation_id"`
	Outcome      Outcome    `json:"outcome"`
	Detail       string     `json:"detail,omitempty"`
	Evidence     []Evidence `json:"evidence,omitempty"`
}

// Outcome is the result reported by an observation.
type Outcome string

const (
	OutcomeUnknown Outcome = "unknown"
	OutcomeSuccess Outcome = "success"
	OutcomeFailure Outcome = "failure"
)

func (o Outcome) valid() bool {
	return o == OutcomeUnknown || o == OutcomeSuccess || o == OutcomeFailure
}

// Evidence is opaque, provider-neutral support for an observation. URI and
// Hash are optional and have no operational meaning to this package.
type Evidence struct {
	ID    ID     `json:"id,omitempty"`
	Kind  string `json:"kind"`
	Value string `json:"value,omitempty"`
	URI   string `json:"uri,omitempty"`
	Hash  string `json:"hash,omitempty"`
}

// Budget limits work and records usage. A zero maximum means unlimited. Used
// values are monotonically increased by BudgetConsumed events.
type Budget struct {
	MaxSteps  uint64 `json:"max_steps,omitempty"`
	MaxCost   uint64 `json:"max_cost,omitempty"`
	UsedSteps uint64 `json:"used_steps,omitempty"`
	UsedCost  uint64 `json:"used_cost,omitempty"`
}

// DecisionKind is an instruction for an adapter/executor. Decisions are
// proposals only; applying a DecisionIssued event records the proposal and,
// for execution/cancellation, updates the corresponding lifecycle state.
type DecisionKind string

const (
	DecisionExecute DecisionKind = "execute"
	DecisionWait    DecisionKind = "wait"
	DecisionSucceed DecisionKind = "succeed"
	DecisionFail    DecisionKind = "fail"
	DecisionBlock   DecisionKind = "block"
	DecisionCancel  DecisionKind = "cancel"
	DecisionExhaust DecisionKind = "exhaust"
)

func (k DecisionKind) valid() bool {
	switch k {
	case DecisionExecute, DecisionWait, DecisionSucceed, DecisionFail, DecisionBlock, DecisionCancel, DecisionExhaust:
		return true
	default:
		return false
	}
}

// Decision is a deterministic proposal produced by Reconcile.
type Decision struct {
	ID           ID           `json:"id"`
	GoalID       ID           `json:"goal_id"`
	ObligationID ID           `json:"obligation_id,omitempty"`
	Kind         DecisionKind `json:"kind"`
	Reason       string       `json:"reason,omitempty"`
	Cost         uint64       `json:"cost,omitempty"`
}

// EventKind identifies the union payload in Event.
type EventKind string

const (
	EventGoalCreated      EventKind = "goal_created"
	EventObligationAdded  EventKind = "obligation_added"
	EventDecisionIssued   EventKind = "decision_issued"
	EventObservationAdded EventKind = "observation_added"
	EventBudgetConsumed   EventKind = "budget_consumed"
)

func (k EventKind) valid() bool {
	switch k {
	case EventGoalCreated, EventObligationAdded, EventDecisionIssued, EventObservationAdded, EventBudgetConsumed:
		return true
	default:
		return false
	}
}

// Event is an append-only state transition. Exactly one payload is used,
// according to Kind. Sequence is assigned by Append and is checked by Reduce;
// zero is accepted when reducing a standalone event.
type Event struct {
	Sequence    uint64      `json:"sequence"`
	ID          ID          `json:"id,omitempty"`
	Kind        EventKind   `json:"kind"`
	Data        []byte      `json:"data,omitempty"`
	Goal        Goal        `json:"goal,omitempty"`
	Obligation  Obligation  `json:"obligation,omitempty"`
	Decision    Decision    `json:"decision,omitempty"`
	Observation Observation `json:"observation,omitempty"`
	Budget      BudgetDelta `json:"budget,omitempty"`
}

// BudgetDelta records usage added by an event.
type BudgetDelta struct {
	Steps uint64 `json:"steps,omitempty"`
	Cost  uint64 `json:"cost,omitempty"`
}

// State is a snapshot of the reducer. Maps and slices are owned by the value
// returned from NewState, Reduce, and Replay; callers should not mutate them.
type State struct {
	Goals        map[ID]Goal        `json:"goals"`
	Obligations  map[ID]Obligation  `json:"obligations"`
	Observations map[ID]Observation `json:"observations"`
	Decisions    []Decision         `json:"decisions"`
	Events       []Event            `json:"events"`
	Budget       Budget             `json:"budget"`
	Revision     uint64             `json:"revision"`
}

// NewState returns an empty, ready-to-use state.
func NewState() State {
	return State{Goals: map[ID]Goal{}, Obligations: map[ID]Obligation{}, Observations: map[ID]Observation{}}
}

// NewStateWithBudget returns an empty state with the supplied immutable limits.
func NewStateWithBudget(b Budget) State {
	s := NewState()
	s.Budget.MaxSteps, s.Budget.MaxCost = b.MaxSteps, b.MaxCost
	return s
}

// Clone returns a deep value copy suitable for callers that need to retain a
// snapshot while advancing another state.
func (s State) Clone() State {
	o := NewState()
	for k, v := range s.Goals {
		o.Goals[k] = v
	}
	for k, v := range s.Obligations {
		v.DependsOn = append([]ID(nil), v.DependsOn...)
		o.Obligations[k] = v
	}
	for k, v := range s.Observations {
		v.Evidence = append([]Evidence(nil), v.Evidence...)
		o.Observations[k] = v
	}
	o.Decisions = append([]Decision(nil), s.Decisions...)
	for _, e := range s.Events {
		o.Events = append(o.Events, cloneEvent(e))
	}
	o.Budget, o.Revision = s.Budget, s.Revision
	return o
}

func cloneEvent(e Event) Event {
	e.Data = append([]byte(nil), e.Data...)
	e.Obligation.DependsOn = append([]ID(nil), e.Obligation.DependsOn...)
	e.Observation.Evidence = append([]Evidence(nil), e.Observation.Evidence...)
	return e
}

// ValidateID provides a small shared check for adapters creating contracts.
func ValidateID(id ID) error {
	if id == "" {
		return fmt.Errorf("id is required")
	}
	return nil
}
