package llm

import (
	"bytes"
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"testing"
)

func TestOpenAICompatProvider(t *testing.T) {
	want := map[string]string{"result": "hello"}
	wantJSON, _ := json.Marshal(want)

	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get("Authorization") != "Bearer test-key" {
			t.Errorf("unexpected auth header: %s", r.Header.Get("Authorization"))
		}
		if r.Header.Get("Content-Type") != "application/json" {
			t.Errorf("unexpected content type: %s", r.Header.Get("Content-Type"))
		}

		resp := ChatResponse{
			Choices: []ChatChoice{
				{Message: ChatMessage{Role: "assistant", Content: string(wantJSON)}},
			},
		}
		json.NewEncoder(w).Encode(resp)
	}))
	defer srv.Close()

	p := &openAICompatProvider{
		name:   "test-openai",
		apiKey: "test-key",
		url:    srv.URL,
	}

	if p.Name() != "test-openai" {
		t.Errorf("Name() = %q, want test-openai", p.Name())
	}

	resp, err := p.Complete(context.Background(), &Request{
		Prompt:       "test prompt",
		SystemPrompt: "be helpful",
		MaxTokens:    100,
	})
	if err != nil {
		t.Fatal(err)
	}

	var got map[string]string
	if err := json.Unmarshal([]byte(resp.Text), &got); err != nil {
		t.Fatalf("response not valid JSON: %v\ntext: %s", err, resp.Text)
	}
	if got["result"] != "hello" {
		t.Errorf("result = %q, want hello", got["result"])
	}
}

func TestAnthropicProvider(t *testing.T) {
	want := []map[string]string{{"key": "val"}}
	wantJSON, _ := json.Marshal(want)

	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get("x-api-key") != "ant-key" {
			t.Errorf("unexpected x-api-key: %s", r.Header.Get("x-api-key"))
		}
		if r.Header.Get("anthropic-version") != "2023-06-01" {
			t.Errorf("unexpected anthropic-version: %s", r.Header.Get("anthropic-version"))
		}

		resp := AnthropicResponse{
			Content: []AnthropicContent{
				{Type: "text", Text: string(wantJSON)},
			},
		}
		json.NewEncoder(w).Encode(resp)
	}))
	defer srv.Close()

	// Test with a testable anthropic provider that allows URL override
	p := &anthropicTestProvider{apiKey: "ant-key", url: srv.URL}

	resp, err := p.Complete(context.Background(), &Request{
		Prompt:       "test",
		SystemPrompt: "system",
		MaxTokens:    50,
	})
	if err != nil {
		t.Fatal(err)
	}

	var got []map[string]string
	if err := json.Unmarshal([]byte(resp.Text), &got); err != nil {
		t.Fatalf("response not valid JSON: %v\ntext: %s", err, resp.Text)
	}
	if len(got) != 1 || got[0]["key"] != "val" {
		t.Errorf("got %v, want [{key:val}]", got)
	}
}

func TestExtractJSON(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  string
	}{
		{
			name:  "json fenced",
			input: "Here is the result:\n```json\n{\"a\":1}\n```\n",
			want:  `{"a":1}`,
		},
		{
			name:  "generic fence",
			input: "Result:\n```\n[1,2,3]\n```\nDone.",
			want:  `[1,2,3]`,
		},
		{
			name:  "raw JSON object",
			input: `Some text {"key":"value"} more text`,
			want:  `{"key":"value"}`,
		},
		{
			name:  "raw JSON array",
			input: `Results: [1, 2, 3] end`,
			want:  `[1, 2, 3]`,
		},
		{
			name:  "mixed text with JSON",
			input: "I computed the answer.\n{\"score\":42}\nHope that helps!",
			want:  `{"score":42}`,
		},
		{
			name:  "no JSON",
			input: "Just some plain text without any brackets",
			want:  "",
		},
		{
			name:  "raw starts with bracket",
			input: `[{"id":"1"}]`,
			want:  `[{"id":"1"}]`,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := ExtractJSON(tt.input)
			if got != tt.want {
				t.Errorf("ExtractJSON() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestNewProviderUnknown(t *testing.T) {
	_, err := NewProvider("nonexistent-provider")
	if err == nil {
		t.Fatal("expected error for unknown provider")
	}
}

func TestNewProviderMissingKeys(t *testing.T) {
	envVars := map[string]string{
		"xai":        "XAI_API_KEY",
		"openai":     "OPENAI_API_KEY",
		"openrouter": "OPENROUTER_API_KEY",
		"anthropic":  "ANTHROPIC_API_KEY",
	}
	for name, envVar := range envVars {
		old := os.Getenv(envVar)
		os.Unsetenv(envVar)
		_, err := NewProvider(name)
		if old != "" {
			os.Setenv(envVar, old)
		}
		if err == nil {
			t.Errorf("NewProvider(%q) should fail without API key", name)
		}
	}
}

func TestNewProviderRho(t *testing.T) {
	p, err := NewProvider("rho")
	if err != nil {
		t.Fatalf("NewProvider(rho) error: %v", err)
	}
	if p.Name() != "rho" {
		t.Errorf("Name() = %q, want rho", p.Name())
	}
}

func TestCompleteJSON(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		resp := ChatResponse{
			Choices: []ChatChoice{
				{Message: ChatMessage{Role: "assistant", Content: `{"count":5}`}},
			},
		}
		json.NewEncoder(w).Encode(resp)
	}))
	defer srv.Close()

	client := &Client{
		fast:      &openAICompatProvider{name: "test", apiKey: "k", url: srv.URL},
		smart:     &openAICompatProvider{name: "test", apiKey: "k", url: srv.URL},
		maxTokens: 100,
	}

	var result struct {
		Count int `json:"count"`
	}
	err := client.CompleteJSON(context.Background(), "test", "system", true, &result)
	if err != nil {
		t.Fatal(err)
	}
	if result.Count != 5 {
		t.Errorf("count = %d, want 5", result.Count)
	}
}

// anthropicTestProvider is a test helper that allows overriding the API URL.
type anthropicTestProvider struct {
	apiKey string
	url    string
}

func (p *anthropicTestProvider) Name() string { return "anthropic" }

func (p *anthropicTestProvider) Complete(ctx context.Context, req *Request) (*Response, error) {
	antReq := AnthropicRequest{
		Model:     req.Model,
		MaxTokens: req.MaxTokens,
		System:    req.SystemPrompt,
		Messages: []AnthropicMessage{
			{Role: "user", Content: req.Prompt},
		},
	}

	body, _ := json.Marshal(antReq)

	httpReq, _ := http.NewRequestWithContext(ctx, http.MethodPost, p.url, bytes.NewReader(body))
	httpReq.Header.Set("Content-Type", "application/json")
	httpReq.Header.Set("x-api-key", p.apiKey)
	httpReq.Header.Set("anthropic-version", "2023-06-01")

	resp, err := http.DefaultClient.Do(httpReq)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	var antResp AnthropicResponse
	if err := json.NewDecoder(resp.Body).Decode(&antResp); err != nil {
		return nil, err
	}

	var texts []string
	for _, c := range antResp.Content {
		if c.Type == "text" {
			texts = append(texts, c.Text)
		}
	}

	return &Response{Text: strings.Join(texts, "\n")}, nil
}
