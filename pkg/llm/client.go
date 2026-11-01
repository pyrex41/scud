package llm

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/taskapi"
)

// Caller is the interface for dependency injection of LLM calls.
type Caller interface {
	CompleteJSON(ctx context.Context, prompt, system string, fast bool, result any) error
}

// Client wraps provider selection and JSON extraction.
type Client struct {
	selected      Provider
	selectedModel string
	fast          Provider
	smart         Provider
	multiAgent    MultiAgentProvider
	fastModel     string
	smartModel    string
	maxTokens     int
}

// NewTargetClient constructs a client for one explicit provider/model pair.
func NewTargetClient(target config.ModelTarget) (*Client, error) {
	if strings.TrimSpace(target.Provider) == "" || strings.TrimSpace(target.Model) == "" {
		return nil, fmt.Errorf("model target requires provider and model")
	}
	if target.MaxOutputTokens < 0 {
		return nil, fmt.Errorf("model target max_output_tokens cannot be negative")
	}
	if target.MaxOutputTokens == 0 {
		if strings.EqualFold(target.Provider, "anthropic") {
			target.MaxOutputTokens = 8192
		} else {
			target.MaxOutputTokens = 4096
		}
	}
	p, err := NewTargetProvider(target.Provider)
	if err != nil {
		return nil, fmt.Errorf("model target provider %q: %w", target.Provider, err)
	}
	return &Client{selected: p, selectedModel: target.Model, maxTokens: target.MaxOutputTokens}, nil
}

// CompleteTargetJSON sends a JSON request through this client's single selected route.
func (c *Client) CompleteTargetJSON(ctx context.Context, prompt, system string, result any) error {
	if c.selected == nil {
		return fmt.Errorf("client has no explicit model target")
	}
	ctx, cancel := context.WithTimeout(ctx, 2*time.Minute)
	defer cancel()
	resp, err := c.selected.Complete(ctx, &Request{Model: c.selectedModel, Prompt: prompt, SystemPrompt: system, MaxTokens: c.maxTokens, UseMaxCompletionTokens: c.selected.Name() == "openai"})
	if err != nil {
		return fmt.Errorf("llm %s: %w", c.selected.Name(), err)
	}
	if resp.FinishReason == "length" || resp.FinishReason == "max_tokens" {
		return fmt.Errorf("llm %s response was truncated (token limit)", c.selected.Name())
	}
	if resp.FinishReason == "content_filter" || resp.FinishReason == "refusal" {
		return fmt.Errorf("llm %s refused or filtered the response (%s)", c.selected.Name(), resp.FinishReason)
	}
	if resp.FinishReason == "" {
		return fmt.Errorf("llm %s returned an empty or unterminated response", c.selected.Name())
	}
	switch resp.FinishReason {
	case "stop", "end_turn", "stop_sequence":
	default:
		return fmt.Errorf("llm %s returned unsupported termination reason %q", c.selected.Name(), resp.FinishReason)
	}
	if strings.TrimSpace(resp.Text) == "" {
		return fmt.Errorf("llm %s returned empty content", c.selected.Name())
	}
	if err := taskapi.DecodeJSON(strings.NewReader(resp.Text), result); err != nil {
		return fmt.Errorf("parsing JSON from %s: %w", c.selected.Name(), err)
	}
	return nil
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

	// Try to create multi-agent provider (best-effort)
	ma, _ := NewMultiAgentProvider()

	return &Client{
		fast:       fast,
		smart:      smart,
		multiAgent: ma,
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

// CompleteMultiAgent calls the xAI Responses API for multi-agent queries.
// Returns an error if no multi-agent provider is available.
func (c *Client) CompleteMultiAgent(ctx context.Context, req *MultiAgentRequest) (*MultiAgentResponse, error) {
	if c.multiAgent == nil {
		return nil, fmt.Errorf("no multi-agent provider available: set XAI_API_KEY or run `rho auth xai`")
	}
	return c.multiAgent.CompleteMultiAgent(ctx, req)
}

// HasMultiAgent returns true if a multi-agent provider is available.
func (c *Client) HasMultiAgent() bool {
	return c.multiAgent != nil
}
