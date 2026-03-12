package llm

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/reuben/scud/internal/config"
)

// Caller is the interface for dependency injection of LLM calls.
type Caller interface {
	CompleteJSON(ctx context.Context, prompt, system string, fast bool, result any) error
}

// Client wraps provider selection and JSON extraction.
type Client struct {
	fast       Provider
	smart      Provider
	fastModel  string
	smartModel string
	maxTokens  int
}

// NewClient creates a Client from config. It tries to create direct API
// providers first, falling back to rho for each tier.
func NewClient(cfg *config.Config) (*Client, error) {
	fast, err := NewProvider(cfg.LLM.FastProvider)
	if err != nil {
		// Fall back to rho
		fast = &rhoProvider{}
	}

	smart, err := NewProvider(cfg.LLM.SmartProvider)
	if err != nil {
		smart = &rhoProvider{}
	}

	return &Client{
		fast:       fast,
		smart:      smart,
		fastModel:  cfg.LLM.FastModel,
		smartModel: cfg.LLM.SmartModel,
		maxTokens:  cfg.LLM.MaxTokens,
	}, nil
}

// CompleteJSON calls the appropriate provider and extracts/unmarshals JSON
// from the response into result.
func (c *Client) CompleteJSON(ctx context.Context, prompt, system string, fast bool, result any) error {
	provider := c.smart
	model := c.smartModel
	if fast {
		provider = c.fast
		model = c.fastModel
	}

	resp, err := provider.Complete(ctx, &Request{
		Model:        model,
		Prompt:       prompt,
		SystemPrompt: system,
		MaxTokens:    c.maxTokens,
	})
	if err != nil {
		return fmt.Errorf("llm %s: %w", provider.Name(), err)
	}

	jsonStr := ExtractJSON(resp.Text)
	if jsonStr == "" {
		return fmt.Errorf("no JSON found in %s response:\n%s", provider.Name(), truncate(resp.Text, 500))
	}

	if err := json.Unmarshal([]byte(jsonStr), result); err != nil {
		return fmt.Errorf("parsing JSON from %s: %w\nraw: %s", provider.Name(), err, truncate(jsonStr, 500))
	}

	return nil
}
