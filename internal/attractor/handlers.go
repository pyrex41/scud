package attractor

import (
	"bufio"
	"context"
	"fmt"
	"os"
	"os/exec"
	"strings"

	"github.com/reuben/scud/internal/rho"
)

// Handler executes the logic for a specific node type.
type Handler interface {
	Handle(ctx context.Context, node *PipelineNode, pctx *PipelineContext) (*Outcome, error)
}

// NewHandler returns the appropriate handler for a node type.
func NewHandler(nodeType string) Handler {
	switch nodeType {
	case "start":
		return &startHandler{}
	case "exit":
		return &exitHandler{}
	case "codergen":
		return &codergenHandler{}
	case "tool":
		return &toolHandler{}
	case "wait.human":
		return &humanHandler{}
	case "parallel":
		return &parallelHandler{}
	case "fan_in":
		return &fanInHandler{}
	case "conditional":
		return &conditionalHandler{}
	case "manager":
		return &managerHandler{}
	default:
		return &startHandler{} // fallback: pass-through
	}
}

// startHandler just marks the pipeline as started.
type startHandler struct{}

func (h *startHandler) Handle(_ context.Context, node *PipelineNode, pctx *PipelineContext) (*Outcome, error) {
	pctx.Set("status", "success")
	return &Outcome{NodeID: node.ID, Status: "success"}, nil
}

// exitHandler marks the pipeline as finished.
type exitHandler struct{}

func (h *exitHandler) Handle(_ context.Context, node *PipelineNode, _ *PipelineContext) (*Outcome, error) {
	return &Outcome{NodeID: node.ID, Status: "success"}, nil
}

// codergenHandler runs rho-cli with a prompt from node properties.
type codergenHandler struct{}

func (h *codergenHandler) Handle(ctx context.Context, node *PipelineNode, pctx *PipelineContext) (*Outcome, error) {
	prompt := node.Properties["prompt"]
	if prompt == "" {
		prompt = node.Label
	}

	model := pctx.Get("model")
	workDir := pctx.Get("workdir")

	result, err := rho.Run(ctx, rho.Options{
		Prompt:     prompt,
		Model:      model,
		WorkingDir: workDir,
	})
	if err != nil {
		return &Outcome{NodeID: node.ID, Status: "failure", Error: err.Error()}, nil
	}

	pctx.Set("status", "success")
	pctx.Set("output", result.Stdout)

	if result.ExitCode != 0 {
		pctx.Set("status", "failure")
		return &Outcome{
			NodeID: node.ID,
			Status: "failure",
			Output: result.Stdout,
			Error:  result.Stderr,
		}, nil
	}

	return &Outcome{NodeID: node.ID, Status: "success", Output: result.Stdout}, nil
}

// toolHandler runs a shell command from node properties.
type toolHandler struct{}

func (h *toolHandler) Handle(ctx context.Context, node *PipelineNode, pctx *PipelineContext) (*Outcome, error) {
	command := node.Properties["command"]
	if command == "" {
		return &Outcome{NodeID: node.ID, Status: "failure", Error: "no command property"}, nil
	}

	cmd := exec.CommandContext(ctx, "sh", "-c", command)
	if workDir := pctx.Get("workdir"); workDir != "" {
		cmd.Dir = workDir
	}

	var stdout, stderr strings.Builder
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr

	err := cmd.Run()
	if err != nil {
		pctx.Set("status", "failure")
		return &Outcome{
			NodeID: node.ID,
			Status: "failure",
			Output: stdout.String(),
			Error:  stderr.String(),
		}, nil
	}

	pctx.Set("status", "success")
	pctx.Set("output", stdout.String())
	return &Outcome{NodeID: node.ID, Status: "success", Output: stdout.String()}, nil
}

// humanHandler waits for user input on stdin.
type humanHandler struct{}

func (h *humanHandler) Handle(ctx context.Context, node *PipelineNode, pctx *PipelineContext) (*Outcome, error) {
	prompt := node.Properties["prompt"]
	if prompt == "" {
		prompt = node.Label
	}

	fmt.Fprintf(os.Stdout, "\n[HUMAN INPUT REQUIRED] %s\n> ", prompt)

	scanner := bufio.NewScanner(os.Stdin)
	if scanner.Scan() {
		input := scanner.Text()
		pctx.Set("human_input", input)
		pctx.Set("status", "success")
		return &Outcome{NodeID: node.ID, Status: "success", Output: input}, nil
	}

	if err := scanner.Err(); err != nil {
		return &Outcome{NodeID: node.ID, Status: "failure", Error: err.Error()}, nil
	}
	return &Outcome{NodeID: node.ID, Status: "failure", Error: "no input received"}, nil
}

// parallelHandler is a placeholder that marks a node for parallel execution.
type parallelHandler struct{}

func (h *parallelHandler) Handle(_ context.Context, node *PipelineNode, pctx *PipelineContext) (*Outcome, error) {
	pctx.Set("status", "success")
	return &Outcome{NodeID: node.ID, Status: "success"}, nil
}

// fanInHandler is a placeholder that waits for all predecessors.
type fanInHandler struct{}

func (h *fanInHandler) Handle(_ context.Context, node *PipelineNode, pctx *PipelineContext) (*Outcome, error) {
	pctx.Set("status", "success")
	return &Outcome{NodeID: node.ID, Status: "success"}, nil
}

// conditionalHandler evaluates a condition and routes accordingly.
type conditionalHandler struct{}

func (h *conditionalHandler) Handle(_ context.Context, node *PipelineNode, pctx *PipelineContext) (*Outcome, error) {
	condition := node.Properties["condition"]
	if condition == "" {
		pctx.Set("status", "success")
		return &Outcome{NodeID: node.ID, Status: "success"}, nil
	}

	if EvalCondition(condition, pctx) {
		pctx.Set("result", "true")
		pctx.Set("status", "success")
	} else {
		pctx.Set("result", "false")
		pctx.Set("status", "success")
	}
	return &Outcome{NodeID: node.ID, Status: "success"}, nil
}

// managerHandler orchestrates a sub-pipeline (placeholder).
type managerHandler struct{}

func (h *managerHandler) Handle(_ context.Context, node *PipelineNode, pctx *PipelineContext) (*Outcome, error) {
	pctx.Set("status", "success")
	return &Outcome{NodeID: node.ID, Status: "success"}, nil
}
