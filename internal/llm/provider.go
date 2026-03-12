package llm

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"strings"
)

// Request is a unified LLM request.
type Request struct {
	Model        string
	SystemPrompt string
	Prompt       string
	MaxTokens    int
}

// Response is a unified LLM response.
type Response struct {
	Text string
}

// MultiAgentRequest extends Request with Responses API options.
type MultiAgentRequest struct {
	Model  string
	Prompt string
	System string
	Effort string   // "low", "medium", "high", "xhigh"
	Tools  []string // "web_search", "x_search", "code_execution"
}

// MultiAgentResponse includes the response text and token usage.
type MultiAgentResponse struct {
	Text         string
	InputTokens  int
	OutputTokens int
	TotalTokens  int
}

// MultiAgentProvider can call the xAI Responses API for multi-agent queries.
type MultiAgentProvider interface {
	Name() string
	CompleteMultiAgent(ctx context.Context, req *MultiAgentRequest) (*MultiAgentResponse, error)
}

// Provider is the interface for LLM provider implementations.
type Provider interface {
	Name() string
	Complete(ctx context.Context, req *Request) (*Response, error)
}

// NewProvider creates a provider by name. Returns an error if the provider
// is unknown or if the required API key is not set.
func NewProvider(name string) (Provider, error) {
	switch strings.ToLower(name) {
	case "xai":
		key := os.Getenv("XAI_API_KEY")
		if key == "" {
			return nil, fmt.Errorf("XAI_API_KEY not set")
		}
		return &openAICompatProvider{
			name:   "xai",
			apiKey: key,
			url:    "https://api.x.ai/v1/chat/completions",
		}, nil
	case "openai":
		key := os.Getenv("OPENAI_API_KEY")
		if key == "" {
			return nil, fmt.Errorf("OPENAI_API_KEY not set")
		}
		return &openAICompatProvider{
			name:   "openai",
			apiKey: key,
			url:    "https://api.openai.com/v1/chat/completions",
		}, nil
	case "openrouter":
		key := os.Getenv("OPENROUTER_API_KEY")
		if key == "" {
			return nil, fmt.Errorf("OPENROUTER_API_KEY not set")
		}
		return &openAICompatProvider{
			name:   "openrouter",
			apiKey: key,
			url:    "https://openrouter.ai/api/v1/chat/completions",
		}, nil
	case "anthropic":
		key := os.Getenv("ANTHROPIC_API_KEY")
		if key == "" {
			return nil, fmt.Errorf("ANTHROPIC_API_KEY not set")
		}
		return &anthropicProvider{apiKey: key}, nil
	case "xai-responses":
		key := os.Getenv("XAI_API_KEY")
		if key == "" {
			return nil, fmt.Errorf("XAI_API_KEY not set")
		}
		return &xaiResponsesProvider{apiKey: key}, nil
	case "rho":
		return &rhoProvider{}, nil
	default:
		return nil, fmt.Errorf("unknown provider: %s", name)
	}
}

// NewMultiAgentProvider creates a MultiAgentProvider. Currently only xAI is supported.
func NewMultiAgentProvider() (MultiAgentProvider, error) {
	key := os.Getenv("XAI_API_KEY")
	if key == "" {
		return nil, fmt.Errorf("XAI_API_KEY not set")
	}
	return &xaiResponsesProvider{apiKey: key}, nil
}

// openAICompatProvider works with any OpenAI-compatible API (xAI, OpenAI, OpenRouter).
type openAICompatProvider struct {
	name   string
	apiKey string
	url    string
}

func (p *openAICompatProvider) Name() string { return p.name }

func (p *openAICompatProvider) Complete(ctx context.Context, req *Request) (*Response, error) {
	var msgs []ChatMessage
	if req.SystemPrompt != "" {
		msgs = append(msgs, ChatMessage{Role: "system", Content: req.SystemPrompt})
	}
	msgs = append(msgs, ChatMessage{Role: "user", Content: req.Prompt})

	chatReq := ChatRequest{
		Model:     req.Model,
		Messages:  msgs,
		MaxTokens: req.MaxTokens,
	}

	body, err := json.Marshal(chatReq)
	if err != nil {
		return nil, fmt.Errorf("marshaling request: %w", err)
	}

	httpReq, err := http.NewRequestWithContext(ctx, http.MethodPost, p.url, bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("creating request: %w", err)
	}
	httpReq.Header.Set("Content-Type", "application/json")
	httpReq.Header.Set("Authorization", "Bearer "+p.apiKey)

	resp, err := http.DefaultClient.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("%s request failed: %w", p.name, err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("reading %s response: %w", p.name, err)
	}

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("%s returned status %d: %s", p.name, resp.StatusCode, truncate(string(respBody), 500))
	}

	var chatResp ChatResponse
	if err := json.Unmarshal(respBody, &chatResp); err != nil {
		return nil, fmt.Errorf("parsing %s response: %w", p.name, err)
	}
	if len(chatResp.Choices) == 0 {
		return nil, fmt.Errorf("%s returned no choices", p.name)
	}

	return &Response{Text: chatResp.Choices[0].Message.Content}, nil
}

// anthropicProvider calls the Anthropic messages API.
type anthropicProvider struct {
	apiKey string
}

func (p *anthropicProvider) Name() string { return "anthropic" }

func (p *anthropicProvider) Complete(ctx context.Context, req *Request) (*Response, error) {
	antReq := AnthropicRequest{
		Model:     req.Model,
		MaxTokens: req.MaxTokens,
		System:    req.SystemPrompt,
		Messages: []AnthropicMessage{
			{Role: "user", Content: req.Prompt},
		},
	}

	body, err := json.Marshal(antReq)
	if err != nil {
		return nil, fmt.Errorf("marshaling request: %w", err)
	}

	httpReq, err := http.NewRequestWithContext(ctx, http.MethodPost, "https://api.anthropic.com/v1/messages", bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("creating request: %w", err)
	}
	httpReq.Header.Set("Content-Type", "application/json")
	httpReq.Header.Set("x-api-key", p.apiKey)
	httpReq.Header.Set("anthropic-version", "2023-06-01")

	resp, err := http.DefaultClient.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("anthropic request failed: %w", err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("reading anthropic response: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("anthropic returned status %d: %s", resp.StatusCode, truncate(string(respBody), 500))
	}

	var antResp AnthropicResponse
	if err := json.Unmarshal(respBody, &antResp); err != nil {
		return nil, fmt.Errorf("parsing anthropic response: %w", err)
	}

	var texts []string
	for _, c := range antResp.Content {
		if c.Type == "text" {
			texts = append(texts, c.Text)
		}
	}
	if len(texts) == 0 {
		return nil, fmt.Errorf("anthropic returned no text content")
	}

	return &Response{Text: strings.Join(texts, "\n")}, nil
}

// rhoProvider falls back to the rho-cli subprocess.
type rhoProvider struct{}

func (p *rhoProvider) Name() string { return "rho" }

func (p *rhoProvider) Complete(ctx context.Context, req *Request) (*Response, error) {
	rhoPath, err := exec.LookPath("rho-cli")
	if err != nil {
		return nil, fmt.Errorf("rho-cli not found in PATH: %w", err)
	}

	var args []string
	if req.Model != "" {
		args = append(args, "--model", req.Model)
	}
	if req.SystemPrompt != "" {
		args = append(args, "--system-append", req.SystemPrompt)
	}
	args = append(args, req.Prompt)

	cmd := exec.CommandContext(ctx, rhoPath, args...)
	var stdout, stderr strings.Builder
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr

	if err := cmd.Run(); err != nil {
		if ctx.Err() != nil {
			return nil, fmt.Errorf("rho timed out: %w", ctx.Err())
		}
		if cmd.ProcessState != nil && cmd.ProcessState.ExitCode() != 0 {
			return nil, fmt.Errorf("rho exited with code %d: %s", cmd.ProcessState.ExitCode(), stderr.String())
		}
	}

	return &Response{Text: stdout.String()}, nil
}

// xaiResponsesProvider calls the xAI Responses API for multi-agent queries.
type xaiResponsesProvider struct {
	apiKey string
}

func (p *xaiResponsesProvider) Name() string { return "xai-responses" }

// Complete implements Provider for the Responses API (single-turn).
func (p *xaiResponsesProvider) Complete(ctx context.Context, req *Request) (*Response, error) {
	maReq := &MultiAgentRequest{
		Model:  req.Model,
		Prompt: req.Prompt,
		System: req.SystemPrompt,
		Effort: "low",
	}
	maResp, err := p.CompleteMultiAgent(ctx, maReq)
	if err != nil {
		return nil, err
	}
	return &Response{Text: maResp.Text}, nil
}

// CompleteMultiAgent calls the xAI Responses API with multi-agent support.
func (p *xaiResponsesProvider) CompleteMultiAgent(ctx context.Context, req *MultiAgentRequest) (*MultiAgentResponse, error) {
	var input []ResponsesMessage
	if req.System != "" {
		input = append(input, ResponsesMessage{Role: "developer", Content: req.System})
	}
	input = append(input, ResponsesMessage{Role: "user", Content: req.Prompt})

	apiReq := ResponsesRequest{
		Model: req.Model,
		Input: input,
	}

	if req.Effort != "" {
		apiReq.Reasoning = &ResponsesReasoning{Effort: req.Effort}
	}

	for _, tool := range req.Tools {
		apiReq.Tools = append(apiReq.Tools, ResponsesTool{Type: tool})
	}

	body, err := json.Marshal(apiReq)
	if err != nil {
		return nil, fmt.Errorf("marshaling request: %w", err)
	}

	httpReq, err := http.NewRequestWithContext(ctx, http.MethodPost, "https://api.x.ai/v1/responses", bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("creating request: %w", err)
	}
	httpReq.Header.Set("Content-Type", "application/json")
	httpReq.Header.Set("Authorization", "Bearer "+p.apiKey)

	resp, err := http.DefaultClient.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("xai-responses request failed: %w", err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("reading xai-responses response: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("xai-responses returned status %d: %s", resp.StatusCode, truncate(string(respBody), 500))
	}

	var apiResp ResponsesResponse
	if err := json.Unmarshal(respBody, &apiResp); err != nil {
		return nil, fmt.Errorf("parsing xai-responses response: %w", err)
	}

	if apiResp.Error != nil {
		return nil, fmt.Errorf("xai-responses error: %s", apiResp.Error.Message)
	}

	var texts []string
	for _, out := range apiResp.Output {
		for _, c := range out.Content {
			if c.Type == "output_text" {
				texts = append(texts, c.Text)
			}
		}
	}
	if len(texts) == 0 {
		return nil, fmt.Errorf("xai-responses returned no text content")
	}

	return &MultiAgentResponse{
		Text:         strings.Join(texts, "\n"),
		InputTokens:  apiResp.Usage.InputTokens,
		OutputTokens: apiResp.Usage.OutputTokens,
		TotalTokens:  apiResp.Usage.TotalTokens,
	}, nil
}

func truncate(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n] + "..."
}
