package llm

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"strings"
)

// Request is a unified LLM request.
type Request struct {
	Model                  string
	SystemPrompt           string
	Prompt                 string
	MaxTokens              int
	UseMaxCompletionTokens bool
}

// Response is a unified LLM response.
type Response struct {
	Text         string
	FinishReason string
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
		if !HasXAICredentials() {
			return nil, fmt.Errorf("no xAI credentials: set XAI_API_KEY or run `rho auth xai`")
		}
		return &openAICompatProvider{
			name:              "xai",
			legacyModelPrefix: true,
			keyFn:             ResolveXAIToken,
			url:               strings.TrimRight(envOr("XAI_BASE_URL", "https://api.x.ai/v1"), "/") + "/chat/completions",
		}, nil
	case "openai":
		key := os.Getenv("OPENAI_API_KEY")
		if key == "" {
			return nil, fmt.Errorf("OPENAI_API_KEY not set")
		}
		return &openAICompatProvider{
			name:              "openai",
			legacyModelPrefix: true,
			apiKey:            key,
			url:               strings.TrimRight(envOr("OPENAI_BASE_URL", "https://api.openai.com/v1"), "/") + "/chat/completions",
		}, nil
	case "openrouter":
		key := os.Getenv("OPENROUTER_API_KEY")
		if key == "" {
			return nil, fmt.Errorf("OPENROUTER_API_KEY not set")
		}
		return &openAICompatProvider{
			name:              "openrouter",
			legacyModelPrefix: true,
			apiKey:            key,
			url:               strings.TrimRight(envOr("OPENROUTER_BASE_URL", "https://openrouter.ai/api/v1"), "/") + "/chat/completions",
		}, nil
	case "anthropic":
		key := os.Getenv("ANTHROPIC_API_KEY")
		if key == "" {
			return nil, fmt.Errorf("ANTHROPIC_API_KEY not set")
		}
		return &anthropicProvider{apiKey: key, legacyModelPrefix: true}, nil
	case "xai-responses":
		if !HasXAICredentials() {
			return nil, fmt.Errorf("no xAI credentials: set XAI_API_KEY or run `rho auth xai`")
		}
		return &xaiResponsesProvider{keyFn: ResolveXAIToken}, nil
	case "rho":
		return &rhoProvider{}, nil
	default:
		return nil, fmt.Errorf("unknown provider: %s", name)
	}
}

// NewTargetProvider creates a provider for the new explicit-target path. It uses
// environment credentials only; unlike legacy NewProvider it never reads rho auth files.
func NewTargetProvider(name string) (Provider, error) {
	switch strings.ToLower(name) {
	case "openai", "xai", "openrouter":
		keyName := map[string]string{"openai": "OPENAI_API_KEY", "xai": "XAI_API_KEY", "openrouter": "OPENROUTER_API_KEY"}[strings.ToLower(name)]
		key := os.Getenv(keyName)
		if key == "" {
			return nil, fmt.Errorf("%s is required for %s; set it in the environment", keyName, name)
		}
		baseKey := map[string]string{"openai": "OPENAI_BASE_URL", "xai": "XAI_BASE_URL", "openrouter": "OPENROUTER_BASE_URL"}[strings.ToLower(name)]
		fallback := map[string]string{"openai": "https://api.openai.com/v1", "xai": "https://api.x.ai/v1", "openrouter": "https://openrouter.ai/api/v1"}[strings.ToLower(name)]
		return &openAICompatProvider{name: strings.ToLower(name), apiKey: key, url: strings.TrimRight(envOr(baseKey, fallback), "/") + "/chat/completions"}, nil
	case "anthropic":
		key := os.Getenv("ANTHROPIC_API_KEY")
		if key == "" {
			return nil, fmt.Errorf("ANTHROPIC_API_KEY is required for anthropic; set it in the environment")
		}
		return &anthropicProvider{apiKey: key, url: strings.TrimRight(envOr("ANTHROPIC_BASE_URL", "https://api.anthropic.com/v1"), "/") + "/messages"}, nil
	default:
		return nil, fmt.Errorf("unsupported explicit model provider %q", name)
	}
}

func envOr(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}

// CatalogModel is an available model returned by a provider's authenticated catalog.
type CatalogModel struct {
	ID          string `json:"id"`
	DisplayName string `json:"display_name"`
	Provider    string `json:"provider"`
}

type CatalogPage struct {
	Models     []CatalogModel
	Truncated  bool
	NextCursor string
}

// CatalogCredentialEnv returns the environment variable used for each supported catalog.
func CatalogCredentialEnv(provider string) (string, bool) {
	m := map[string]string{"openai": "OPENAI_API_KEY", "xai": "XAI_API_KEY", "anthropic": "ANTHROPIC_API_KEY", "openrouter": "OPENROUTER_API_KEY"}
	v, ok := m[strings.ToLower(provider)]
	return v, ok
}
func CatalogProviders() []string { return []string{"openai", "xai", "anthropic", "openrouter"} }

// ListModels fetches the provider's live model catalog using the same credentials and base URL as generation.
func ListModels(ctx context.Context, provider string) ([]CatalogModel, error) {
	p, err := ListModelCatalog(ctx, provider, http.DefaultClient)
	if err == nil && p.Truncated {
		return p.Models, fmt.Errorf("%s model catalog pagination limit reached (next cursor %q)", provider, p.NextCursor)
	}
	return p.Models, err
}
func ListModelsWithClient(ctx context.Context, provider string, client *http.Client) ([]CatalogModel, error) {
	p, err := ListModelCatalog(ctx, provider, client)
	if err == nil && p.Truncated {
		return p.Models, fmt.Errorf("%s model catalog pagination limit reached (next cursor %q)", provider, p.NextCursor)
	}
	return p.Models, err
}
func ListModelCatalog(ctx context.Context, provider string, client *http.Client) (CatalogPage, error) {
	if err := ctx.Err(); err != nil {
		return CatalogPage{}, err
	}
	var base, key string
	switch strings.ToLower(provider) {
	case "openai":
		base = envOr("OPENAI_BASE_URL", "https://api.openai.com/v1")
		key = os.Getenv("OPENAI_API_KEY")
	case "xai":
		base = envOr("XAI_BASE_URL", "https://api.x.ai/v1")
		key = os.Getenv("XAI_API_KEY")
	case "openrouter":
		base = envOr("OPENROUTER_BASE_URL", "https://openrouter.ai/api/v1")
		key = os.Getenv("OPENROUTER_API_KEY")
	case "anthropic":
		base = envOr("ANTHROPIC_BASE_URL", "https://api.anthropic.com/v1")
		key = os.Getenv("ANTHROPIC_API_KEY")
	default:
		return CatalogPage{}, fmt.Errorf("unsupported catalog provider %q", provider)
	}
	if key == "" {
		return CatalogPage{}, fmt.Errorf("%s credentials not configured", provider)
	}
	page := CatalogPage{}
	for n := 0; n < 10; n++ {
		if err := ctx.Err(); err != nil {
			return CatalogPage{}, err
		}
		endpoint := strings.TrimRight(base, "/") + "/models"
		params := url.Values{}
		params.Set("limit", "100")
		if page.NextCursor != "" {
			param := "after"
			if strings.EqualFold(provider, "anthropic") {
				param = "after_id"
			}
			params.Set(param, page.NextCursor)
		}
		req, err := http.NewRequestWithContext(ctx, http.MethodGet, endpoint+"?"+params.Encode(), nil)
		if err != nil {
			return CatalogPage{}, fmt.Errorf("%s model catalog: invalid endpoint configuration", provider)
		}
		if strings.EqualFold(provider, "anthropic") {
			req.Header.Set("x-api-key", key)
			req.Header.Set("anthropic-version", "2023-06-01")
		} else {
			req.Header.Set("Authorization", "Bearer "+key)
		}
		resp, err := client.Do(req)
		if err != nil {
			return CatalogPage{}, catalogIOError(ctx, provider, "request failed", err)
		}
		if err := ctx.Err(); err != nil {
			resp.Body.Close()
			return CatalogPage{}, err
		}
		if resp.StatusCode < 200 || resp.StatusCode >= 300 {
			resp.Body.Close()
			return CatalogPage{}, fmt.Errorf("%s model catalog returned status %d", provider, resp.StatusCode)
		}
		var payload struct {
			Data []struct {
				ID          string `json:"id"`
				DisplayName string `json:"display_name"`
			} `json:"data"`
			HasMore    bool   `json:"has_more"`
			NextCursor string `json:"next_cursor"`
			LastID     string `json:"last_id"`
		}
		body, readErr := io.ReadAll(io.LimitReader(resp.Body, (4<<20)+1))
		resp.Body.Close()
		if err := ctx.Err(); err != nil {
			return CatalogPage{}, err
		}
		if readErr != nil {
			return CatalogPage{}, catalogIOError(ctx, provider, "response read failed", readErr)
		}
		if len(body) > 4<<20 {
			return CatalogPage{}, fmt.Errorf("%s model catalog response is unreadable or exceeds 4 MiB", provider)
		}
		if err = json.Unmarshal(body, &payload); err != nil {
			return CatalogPage{}, fmt.Errorf("decoding %s model catalog: invalid JSON", provider)
		}
		for _, m := range payload.Data {
			name := m.DisplayName
			if name == "" {
				name = m.ID
			}
			page.Models = append(page.Models, CatalogModel{ID: m.ID, DisplayName: name, Provider: provider})
		}
		more := payload.HasMore
		cursor := payload.NextCursor
		if cursor == "" {
			cursor = payload.LastID
		}
		if cursor == "" && more && len(payload.Data) > 0 {
			cursor = payload.Data[len(payload.Data)-1].ID
		}
		if !more || cursor == "" {
			page.Truncated = more
			page.NextCursor = cursor
			return page, nil
		}
		page.NextCursor = cursor
	}
	page.Truncated = true
	return page, nil
}

// Keep the cause available to errors.Is without exposing URLs, credentials, or
// arbitrary transport/body error text in the printable error.
type catalogError struct {
	message string
	cause   error
}

func (e *catalogError) Error() string { return e.message }
func (e *catalogError) Unwrap() error { return e.cause }
func catalogIOError(ctx context.Context, provider, message string, cause error) error {
	if err := ctx.Err(); err != nil {
		cause = err
	}
	return &catalogError{message: provider + " model catalog " + message, cause: cause}
}

// NewMultiAgentProvider creates a MultiAgentProvider. Currently only xAI is supported.
func NewMultiAgentProvider() (MultiAgentProvider, error) {
	if !HasXAICredentials() {
		return nil, fmt.Errorf("no xAI credentials: set XAI_API_KEY or run `rho auth xai`")
	}
	return &xaiResponsesProvider{keyFn: ResolveXAIToken}, nil
}

// openAICompatProvider works with any OpenAI-compatible API (xAI, OpenAI, OpenRouter).
// Either apiKey or keyFn must be set. keyFn takes precedence and is called per-request
// so providers backed by OAuth (xAI subscription auth) can refresh transparently.
type openAICompatProvider struct {
	legacyModelPrefix bool
	name              string
	apiKey            string
	keyFn             func(ctx context.Context) (string, error)
	url               string
}

func (p *openAICompatProvider) Name() string { return p.name }

func (p *openAICompatProvider) resolveKey(ctx context.Context) (string, error) {
	if p.keyFn != nil {
		return p.keyFn(ctx)
	}
	return p.apiKey, nil
}

func (p *openAICompatProvider) Complete(ctx context.Context, req *Request) (*Response, error) {
	var msgs []ChatMessage
	if req.SystemPrompt != "" {
		msgs = append(msgs, ChatMessage{Role: "system", Content: req.SystemPrompt})
	}
	msgs = append(msgs, ChatMessage{Role: "user", Content: req.Prompt})

	// Only legacy callers use synthetic provider prefixes. Explicit catalog IDs
	// (including namespaced OpenRouter IDs) are opaque and must remain verbatim.
	model := req.Model
	if idx := strings.Index(model, "/"); p.legacyModelPrefix && idx >= 0 {
		model = model[idx+1:]
	}

	chatReq := ChatRequest{
		Model:          model,
		Messages:       msgs,
		ResponseFormat: &ResponseFormat{Type: "json_object"},
	}
	if req.UseMaxCompletionTokens {
		chatReq.MaxCompletionTokens = req.MaxTokens
	} else {
		chatReq.MaxTokens = req.MaxTokens
	}

	body, err := json.Marshal(chatReq)
	if err != nil {
		return nil, fmt.Errorf("marshaling request: %w", err)
	}

	key, err := p.resolveKey(ctx)
	if err != nil {
		return nil, fmt.Errorf("resolving %s api key: %w", p.name, err)
	}

	httpReq, err := http.NewRequestWithContext(ctx, http.MethodPost, p.url, bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("creating request: %w", err)
	}
	httpReq.Header.Set("Content-Type", "application/json")
	httpReq.Header.Set("Authorization", "Bearer "+key)

	resp, err := http.DefaultClient.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("%s request failed: %w", p.name, err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(io.LimitReader(resp.Body, (8<<20)+1))
	if err != nil {
		return nil, fmt.Errorf("reading %s response: %w", p.name, err)
	}
	if len(respBody) > 8<<20 {
		return nil, fmt.Errorf("%s response exceeds 8 MiB limit", p.name)
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

	reason := chatResp.Choices[0].FinishReason
	if chatResp.Choices[0].Message.Refusal != "" {
		reason = "refusal"
	}
	return &Response{Text: chatResp.Choices[0].Message.Content, FinishReason: reason}, nil
}

// anthropicProvider calls the Anthropic messages API.
type anthropicProvider struct {
	legacyModelPrefix bool
	apiKey            string
	url               string
}

func (p *anthropicProvider) Name() string { return "anthropic" }

func (p *anthropicProvider) Complete(ctx context.Context, req *Request) (*Response, error) {
	// Strip provider prefix (e.g. "anthropic/claude-..." -> "claude-...")
	model := req.Model
	if idx := strings.Index(model, "/"); p.legacyModelPrefix && idx >= 0 {
		model = model[idx+1:]
	}

	antReq := AnthropicRequest{
		Model:     model,
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

	url := p.url
	if url == "" {
		url = "https://api.anthropic.com/v1/messages"
	}
	httpReq, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(body))
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

	respBody, err := io.ReadAll(io.LimitReader(resp.Body, (8<<20)+1))
	if err != nil {
		return nil, fmt.Errorf("reading anthropic response: %w", err)
	}
	if len(respBody) > 8<<20 {
		return nil, fmt.Errorf("anthropic response exceeds 8 MiB limit")
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
		if c.Type == "refusal" {
			return &Response{Text: c.Text, FinishReason: "refusal"}, nil
		}
		if c.Type == "text" {
			texts = append(texts, c.Text)
		}
	}
	if len(texts) == 0 {
		return nil, fmt.Errorf("anthropic returned no text content")
	}

	return &Response{Text: strings.Join(texts, "\n"), FinishReason: antResp.StopReason}, nil
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
// Either apiKey or keyFn must be set; keyFn takes precedence.
type xaiResponsesProvider struct {
	apiKey string
	keyFn  func(ctx context.Context) (string, error)
}

func (p *xaiResponsesProvider) Name() string { return "xai-responses" }

func (p *xaiResponsesProvider) resolveKey(ctx context.Context) (string, error) {
	if p.keyFn != nil {
		return p.keyFn(ctx)
	}
	return p.apiKey, nil
}

// Complete implements Provider for the Responses API (single-turn).
func (p *xaiResponsesProvider) Complete(ctx context.Context, req *Request) (*Response, error) {
	// Strip provider prefix
	model := req.Model
	if idx := strings.Index(model, "/"); idx >= 0 {
		model = model[idx+1:]
	}
	maReq := &MultiAgentRequest{
		Model:  model,
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

	key, err := p.resolveKey(ctx)
	if err != nil {
		return nil, fmt.Errorf("resolving xai-responses api key: %w", err)
	}

	httpReq, err := http.NewRequestWithContext(ctx, http.MethodPost, "https://api.x.ai/v1/responses", bytes.NewReader(body))
	if err != nil {
		return nil, fmt.Errorf("creating request: %w", err)
	}
	httpReq.Header.Set("Content-Type", "application/json")
	httpReq.Header.Set("Authorization", "Bearer "+key)

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
