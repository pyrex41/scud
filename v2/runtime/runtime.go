package runtime

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"sort"
	"strings"
	"sync"

	"github.com/reuben/scud/v2/adapters"
	"github.com/reuben/scud/v2/core"
	"github.com/reuben/scud/v2/graph"
)

var (
	ErrInvalidConfig = errors.New("invalid runtime config")
	ErrNoWork        = errors.New("no runnable work")
	ErrStoreRequired = errors.New("event store is required")
)

// Config describes one provider-blind run. Model, prompts, working directory,
// and context are passed through to Runner without interpretation. Costs are
// opaque scheduling estimates used only for deterministic budget accounting.
type Config struct {
	RunID            string
	Goal             core.Goal
	Obligations      []core.Obligation
	Budget           core.Budget
	Costs            map[core.ID]uint64
	EvidenceRequired map[core.ID]bool
	Model            string
	WorkingDir       string
	SystemPrompt     string
	Limits           adapters.Limits
	Context          map[string]any
	Backpressure     graph.Policy
	Runner           adapters.Runner
	Policy           adapters.Policy
	Store            adapters.EventStore
}

// Runtime is a serialized coordinator. Step is safe for concurrent callers;
// adapter execution occurs while holding the run lock so two callers cannot
// admit the same obligation.
type Runtime struct {
	mu     sync.Mutex
	cfg    Config
	state  core.State
	stream uint64
	ready  bool
}

// New validates configuration and creates an uninitialized runtime. Call
// Replay (or Step, which calls it lazily) before inspecting State.
func New(cfg Config) (*Runtime, error) {
	if cfg.RunID == "" || cfg.Goal.ID == "" {
		return nil, fmt.Errorf("%w: run ID and goal ID are required", ErrInvalidConfig)
	}
	if cfg.Runner == nil {
		return nil, fmt.Errorf("%w: runner is required", ErrInvalidConfig)
	}
	if cfg.Store == nil {
		return nil, ErrStoreRequired
	}
	cfg.Obligations = append([]core.Obligation(nil), cfg.Obligations...)
	cfg.Costs = cloneCosts(cfg.Costs)
	cfg.EvidenceRequired = cloneBools(cfg.EvidenceRequired)
	cfg.Context = cloneContext(cfg.Context)
	return &Runtime{cfg: cfg, state: core.NewStateWithBudget(cfg.Budget)}, nil
}

// State returns an independent state snapshot.
func (r *Runtime) State() core.State { r.mu.Lock(); defer r.mu.Unlock(); return r.state.Clone() }

// Replay loads the canonical core event stream. If the store is empty, the
// configured goal and DAG are initialized as canonical events.
func (r *Runtime) Replay(ctx context.Context) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	return r.replayLocked(ctx)
}

func (r *Runtime) replayLocked(ctx context.Context) error {
	events, err := r.cfg.Store.List(ctx, r.cfg.RunID, 0)
	if err != nil {
		return err
	}
	var canonical []core.Event
	for _, e := range events {
		if e.Sequence > r.stream {
			r.stream = e.Sequence
		}
		if !strings.HasPrefix(e.Type, "core/") {
			continue
		}
		var ce core.Event
		if err := json.Unmarshal(e.Data, &ce); err != nil {
			return fmt.Errorf("decode canonical event %d: %w", e.Sequence, err)
		}
		canonical = append(canonical, ce)
	}
	if len(canonical) == 0 {
		r.state = core.NewStateWithBudget(r.cfg.Budget)
		if err := r.initializeLocked(ctx); err != nil {
			return err
		}
	} else {
		s, err := core.Replay(canonical)
		if err != nil {
			return err
		}
		// Limits are run configuration, while usage is canonical event state.
		// Reapply limits after replay so a resumed run cannot silently become
		// unbounded merely because limits are not repeated in every event.
		s.Budget.MaxSteps, s.Budget.MaxCost = r.cfg.Budget.MaxSteps, r.cfg.Budget.MaxCost
		if err := s.Validate(); err != nil {
			return err
		}
		r.state = s
	}
	r.ready = true
	return nil
}

func (r *Runtime) initializeLocked(ctx context.Context) error {
	// Validate the complete plan before writing its first event; otherwise a
	// malformed plan could leave a durable, partially initialized stream.
	nodes := make([]graph.Node, 0, len(r.cfg.Obligations))
	for _, o := range r.cfg.Obligations {
		if o.GoalID != r.cfg.Goal.ID {
			return fmt.Errorf("%w: obligation %q references goal %q", ErrInvalidConfig, o.ID, o.GoalID)
		}
		nodes = append(nodes, graph.Node{ID: graph.ID(o.ID), Dependencies: ids(o.DependsOn), Status: graphStatus(o.Status)})
	}
	if _, err := graph.Build(nodes); err != nil {
		return fmt.Errorf("%w: %v", ErrInvalidConfig, err)
	}
	if _, err := r.appendCoreLocked(ctx, core.Event{ID: core.ID("goal:" + string(r.cfg.Goal.ID)), Kind: core.EventGoalCreated, Goal: r.cfg.Goal}); err != nil {
		return err
	}
	byID := make(map[core.ID]core.Obligation, len(r.cfg.Obligations))
	for _, o := range r.cfg.Obligations {
		if _, exists := byID[o.ID]; exists {
			return fmt.Errorf("%w: duplicate obligation %q", ErrInvalidConfig, o.ID)
		}
		byID[o.ID] = o
	}
	ids := make([]core.ID, 0, len(byID))
	for id := range byID {
		ids = append(ids, id)
	}
	sort.Slice(ids, func(i, j int) bool { return ids[i] < ids[j] })
	added := map[core.ID]bool{}
	for len(added) < len(ids) {
		progress := false
		for _, id := range ids {
			if added[id] {
				continue
			}
			o := byID[id]
			depsReady := true
			for _, dep := range o.DependsOn {
				if !added[dep] {
					depsReady = false
					break
				}
			}
			if !depsReady {
				continue
			}
			if _, err := r.appendCoreLocked(ctx, core.Event{ID: core.ID("obligation:" + string(o.ID)), Kind: core.EventObligationAdded, Obligation: o}); err != nil {
				return err
			}
			added[id], progress = true, true
		}
		if !progress {
			return fmt.Errorf("%w: invalid obligation DAG", ErrInvalidConfig)
		}
	}
	return nil
}

// Step performs at most one bounded runner execution. A wait, block, or
// terminal decision is returned without invoking Runner.
func (r *Runtime) Step(ctx context.Context) (core.Decision, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if !r.ready {
		if err := r.replayLocked(ctx); err != nil {
			return core.Decision{}, err
		}
	}
	decision, err := core.Reconcile(r.state)
	if err != nil {
		return core.Decision{}, err
	}
	if decision.Kind != core.DecisionExecute {
		if decision.Kind != core.DecisionWait {
			if _, err := r.appendCoreLocked(ctx, core.Event{ID: decision.ID, Kind: core.EventDecisionIssued, Decision: decision}); err != nil {
				return core.Decision{}, err
			}
		}
		return decision, nil
	}
	o := r.state.Obligations[decision.ObligationID]
	if !r.allowedByGraph(o) {
		return core.Decision{ID: decision.ID, GoalID: decision.GoalID, ObligationID: decision.ObligationID, Kind: core.DecisionWait, Reason: "backpressure deferred execution"}, nil
	}
	if r.cfg.Policy != nil {
		input := adapters.PolicyInput{RunID: r.cfg.RunID, Action: string(core.DecisionExecute), Resource: string(o.ID), Attributes: map[string]string{"goal_id": string(o.GoalID), "capability_ref": o.CapabilityRef, "policy_ref": o.PolicyRef, "grant_ref": o.GrantRef}}
		allowed, err := r.cfg.Policy.Authorize(ctx, input)
		if err != nil {
			return core.Decision{}, err
		}
		if !allowed.Allowed {
			blocked := core.Decision{ID: decision.ID + ":blocked", GoalID: decision.GoalID, ObligationID: decision.ObligationID, Kind: core.DecisionBlock, Reason: defaultReason(allowed.Reason, "authorization denied")}
			if _, err := r.appendCoreLocked(ctx, core.Event{ID: blocked.ID, Kind: core.EventDecisionIssued, Decision: blocked}); err != nil {
				return core.Decision{}, err
			}
			return blocked, nil
		}
	}
	cost := r.cfg.Costs[o.ID]
	if !r.budgetAllows(cost) {
		exhaust := core.Decision{ID: decision.ID + ":exhausted", GoalID: decision.GoalID, ObligationID: decision.ObligationID, Kind: core.DecisionExhaust, Reason: "budget exhausted"}
		// Exhaustion is canonical state, not a transient scheduler answer. Record
		// it so Run terminates and replay cannot silently retry over the limit.
		exhaust.ObligationID = ""
		if _, err := r.appendCoreLocked(ctx, core.Event{ID: exhaust.ID, Kind: core.EventDecisionIssued, Decision: exhaust}); err != nil {
			return core.Decision{}, err
		}
		return exhaust, nil
	}
	decision.Cost = cost
	if _, err := r.appendCoreLocked(ctx, core.Event{ID: decision.ID, Kind: core.EventDecisionIssued, Decision: decision}); err != nil {
		return core.Decision{}, err
	}
	if _, err := r.appendCoreLocked(ctx, core.Event{ID: decision.ID + ":budget", Kind: core.EventBudgetConsumed, Budget: core.BudgetDelta{Steps: 1, Cost: cost}}); err != nil {
		return core.Decision{}, err
	}
	runCtx, cancel := context.WithCancel(ctx)
	defer cancel()
	var progressErr error
	result, runErr := r.cfg.Runner.Run(runCtx, adapters.RunRequest{RunID: r.cfg.RunID, Prompt: o.Description, SystemPrompt: r.cfg.SystemPrompt, Model: r.cfg.Model, WorkingDir: r.cfg.WorkingDir, Limits: r.cfg.Limits, Context: cloneContext(r.cfg.Context)}, func(e adapters.Event) {
		if progressErr != nil {
			return
		}
		if err := r.appendProgressLocked(ctx, e); err != nil {
			progressErr = err
			cancel()
		}
	})
	if progressErr != nil {
		return core.Decision{}, fmt.Errorf("persist runner progress: %w", progressErr)
	}
	observation := observationFromResult(o, result, runErr)
	if observation.Outcome == core.OutcomeSuccess && r.cfg.EvidenceRequired[o.ID] && len(observation.Evidence) == 0 {
		observation.Outcome = core.OutcomeFailure
		observation.Detail = "completion requires evidence"
	}
	if _, err := r.appendCoreLocked(ctx, core.Event{ID: observation.ID, Kind: core.EventObservationAdded, Observation: observation}); err != nil {
		return core.Decision{}, err
	}
	return decision, nil
}

// Run advances until the selected goal is terminal or reconciliation waits
// for an external observation. A wait is not an error and can be resumed by
// calling Run or Step after new events are appended.
func (r *Runtime) Run(ctx context.Context) error {
	for {
		if err := ctx.Err(); err != nil {
			return err
		}
		d, err := r.Step(ctx)
		if err != nil {
			return err
		}
		if d.Kind == core.DecisionWait || r.Terminal() {
			return nil
		}
	}
}

// Terminal reports whether the configured goal is terminal.
func (r *Runtime) Terminal() bool {
	r.mu.Lock()
	defer r.mu.Unlock()
	g, ok := r.state.Goals[r.cfg.Goal.ID]
	return ok && g.Status.IsTerminal()
}

func (r *Runtime) appendCoreLocked(ctx context.Context, event core.Event) (core.Event, error) {
	n, err := r.state.Append(event)
	if err != nil {
		return core.Event{}, err
	}
	canonical := n.Events[len(n.Events)-1]
	b, err := json.Marshal(canonical)
	if err != nil {
		return core.Event{}, err
	}
	r.stream++
	if err := r.cfg.Store.Append(ctx, adapters.Event{RunID: r.cfg.RunID, Sequence: r.stream, Type: "core/" + string(canonical.Kind), Data: b}); err != nil {
		r.stream--
		return core.Event{}, err
	}
	r.state = n
	return canonical, nil
}

func (r *Runtime) appendProgressLocked(ctx context.Context, event adapters.Event) error {
	r.stream++
	e := adapters.Event{RunID: r.cfg.RunID, Sequence: r.stream, Time: event.Time, Type: "progress/" + event.Type, Data: append([]byte(nil), event.Data...)}
	if err := r.cfg.Store.Append(ctx, e); err != nil {
		r.stream--
		return err
	}
	return nil
}

func (r *Runtime) budgetAllows(cost uint64) bool {
	b := r.state.Budget
	if b.MaxSteps != 0 && b.UsedSteps >= b.MaxSteps {
		return false
	}
	return b.MaxCost == 0 || cost <= b.MaxCost-b.UsedCost
}

func (r *Runtime) allowedByGraph(o core.Obligation) bool {
	nodes := make([]graph.Node, 0, len(r.state.Obligations))
	for _, x := range r.state.Obligations {
		nodes = append(nodes, graph.Node{ID: graph.ID(x.ID), Dependencies: ids(x.DependsOn), Status: graphStatus(x.Status), Cost: r.cfg.Costs[x.ID], EvidenceRequired: r.cfg.EvidenceRequired[x.ID]})
	}
	g, err := graph.Build(nodes)
	if err != nil {
		return false
	}
	s := graph.NewSnapshot(g)
	res, err := graph.Reconcile(s, graph.Request{Action: graph.Admit, ID: graph.ID(o.ID)}, r.cfg.Backpressure)
	return err == nil && res.Decision.Action == graph.Admit
}

func observationFromResult(o core.Obligation, result adapters.RunResult, runErr error) core.Observation {
	outcome := core.OutcomeSuccess
	detail := result.Text
	if runErr != nil {
		outcome, detail = core.OutcomeFailure, runErr.Error()
	}
	if result.Failure != nil {
		outcome, detail = core.OutcomeFailure, result.Failure.Message
	}
	if result.Outcome != "" && !strings.EqualFold(result.Outcome, "success") && !strings.EqualFold(result.Outcome, "completed") {
		outcome = core.OutcomeFailure
	}
	obs := core.Observation{ID: core.ID("observation:" + string(o.ID) + ":" + string(outcome)), GoalID: o.GoalID, ObligationID: o.ID, Outcome: outcome, Detail: detail}
	if result.Text != "" {
		obs.Evidence = []core.Evidence{{Kind: "runner_output", Value: result.Text}}
	}
	return obs
}

func graphStatus(s core.ObligationStatus) graph.Status {
	switch s {
	case core.ObligationRunning:
		return graph.Running
	case core.ObligationSucceeded:
		return graph.Succeeded
	case core.ObligationFailed:
		return graph.Failed
	case core.ObligationBlocked:
		return graph.Blocked
	case core.ObligationCancelled:
		return graph.Cancelled
	case core.ObligationReady:
		return graph.Ready
	default:
		return graph.Pending
	}
}
func ids(in []core.ID) []graph.ID {
	out := make([]graph.ID, len(in))
	for i, id := range in {
		out[i] = graph.ID(id)
	}
	return out
}
func cloneCosts(in map[core.ID]uint64) map[core.ID]uint64 {
	out := map[core.ID]uint64{}
	for k, v := range in {
		out[k] = v
	}
	return out
}
func cloneBools(in map[core.ID]bool) map[core.ID]bool {
	out := map[core.ID]bool{}
	for k, v := range in {
		out[k] = v
	}
	return out
}
func cloneContext(in map[string]any) map[string]any {
	out := map[string]any{}
	for k, v := range in {
		out[k] = v
	}
	return out
}
func defaultReason(given, fallback string) string {
	if given != "" {
		return given
	}
	return fallback
}
