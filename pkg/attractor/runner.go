package attractor

import (
	"context"
	"fmt"
	"path/filepath"
)

// RunnerOpts configures a pipeline Runner.
type RunnerOpts struct {
	WorkDir    string
	Model      string
	Provider   string
	Headless   bool
	Simulated  bool
	ResumePath string // checkpoint file to resume from
}

// Runner executes a pipeline graph node by node.
type Runner struct {
	Graph      *PipelineGraph
	Context    *PipelineContext
	Checkpoint *Checkpoint
	WorkDir    string
	Model      string
	Provider   string
	Headless   bool
	Simulated  bool
}

// NewRunner creates a Runner from a graph and options.
func NewRunner(graph *PipelineGraph, opts RunnerOpts) *Runner {
	pctx := NewContext()
	if opts.WorkDir != "" {
		pctx.Set("workdir", opts.WorkDir)
	}
	if opts.Model != "" {
		pctx.Set("model", opts.Model)
	}
	if opts.Provider != "" {
		pctx.Set("provider", opts.Provider)
	}

	r := &Runner{
		Graph:   graph,
		Context: pctx,
		Checkpoint: &Checkpoint{
			Context: make(map[string]string),
		},
		WorkDir:   opts.WorkDir,
		Model:     opts.Model,
		Provider:  opts.Provider,
		Headless:  opts.Headless,
		Simulated: opts.Simulated,
	}

	return r
}

// Run executes the pipeline until an exit node is reached or an error occurs.
func (r *Runner) Run(ctx context.Context) error {
	// Determine starting node
	var currentID string
	completedSet := make(map[string]bool)

	if r.Checkpoint.CurrentNode != "" {
		// Resume from checkpoint
		currentID = r.Checkpoint.CurrentNode
		for _, id := range r.Checkpoint.Completed {
			completedSet[id] = true
		}
		r.Context.Load(r.Checkpoint.Context)
	} else {
		start := r.Graph.StartNode()
		if start == nil {
			return fmt.Errorf("no start node in graph")
		}
		currentID = start.ID
	}

	for {
		if err := ctx.Err(); err != nil {
			return fmt.Errorf("context cancelled: %w", err)
		}

		node := r.Graph.FindNode(currentID)
		if node == nil {
			return fmt.Errorf("node %q not found in graph", currentID)
		}

		// Skip already-completed nodes (from checkpoint resume)
		if !completedSet[currentID] {
			// Execute node
			handler := NewHandler(node.Type)
			outcome, err := handler.Handle(ctx, node, r.Context)
			if err != nil {
				return fmt.Errorf("executing node %q: %w", currentID, err)
			}

			// Record outcome
			r.Checkpoint.Outcomes = append(r.Checkpoint.Outcomes, *outcome)
			completedSet[currentID] = true
			r.Checkpoint.Completed = append(r.Checkpoint.Completed, currentID)

			if !r.Headless {
				fmt.Printf("[%s] %s: %s\n", node.Type, node.Label, outcome.Status)
			}

			// If this is an exit node, we're done
			if node.Type == "exit" {
				r.saveCheckpoint(currentID)
				return nil
			}
		}

		// Select next node
		nextID, err := r.selectNext(currentID)
		if err != nil {
			return err
		}

		// Save checkpoint
		r.saveCheckpoint(nextID)

		currentID = nextID
	}
}

// selectNext picks the next node by evaluating edge conditions.
func (r *Runner) selectNext(currentID string) (string, error) {
	edges := r.Graph.Successors(currentID)
	if len(edges) == 0 {
		return "", fmt.Errorf("no outgoing edges from node %q", currentID)
	}

	// First pass: edges with conditions
	for _, e := range edges {
		if e.Condition != "" && EvalCondition(e.Condition, r.Context) {
			return e.To, nil
		}
	}

	// Second pass: first edge without a condition (default path)
	for _, e := range edges {
		if e.Condition == "" {
			return e.To, nil
		}
	}

	// Fallback: first edge regardless
	return edges[0].To, nil
}

func (r *Runner) saveCheckpoint(nextNodeID string) {
	r.Checkpoint.CurrentNode = nextNodeID
	r.Checkpoint.Context = r.Context.All()

	if r.WorkDir != "" {
		path := filepath.Join(r.WorkDir, "checkpoint.json")
		_ = SaveCheckpoint(r.Checkpoint, path) // best-effort
	}
}
