package generate

import (
	"context"
	"fmt"

	"github.com/reuben/scud/internal/llm"
	"github.com/reuben/scud/internal/model"
)

// ComplexityAnalysis holds the result of LLM-powered complexity analysis.
type ComplexityAnalysis struct {
	Complexity int    `json:"complexity"`
	Reasoning  string `json:"reasoning"`
}

// AnalyzeComplexity uses an LLM to analyze the complexity of a task.
func AnalyzeComplexity(ctx context.Context, caller llm.Caller, task *model.Task) (*ComplexityAnalysis, error) {
	prompt := AnalyzeComplexityPrompt(task.Title, task.Description, task.Details)

	var result ComplexityAnalysis
	if err := caller.CompleteJSON(ctx, prompt, "", true, &result); err != nil {
		return nil, fmt.Errorf("llm analyze-complexity: %w", err)
	}

	return &result, nil
}
