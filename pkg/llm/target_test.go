package llm

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/reuben/scud/internal/config"
)

type responseProvider struct{ response *Response }

func (p responseProvider) Name() string { return "fixture" }
func (p responseProvider) Complete(context.Context, *Request) (*Response, error) {
	return p.response, nil
}

func TestNewTargetProviderRequiresEnvironmentCredential(t *testing.T) {
	t.Setenv("XAI_API_KEY", "")
	_, e := NewTargetProvider("xai")
	if e == nil || !strings.Contains(e.Error(), "XAI_API_KEY") {
		t.Fatalf("expected actionable env credential error, got %v", e)
	}
}

func TestTargetJSONRejectsUnsafeTerminationAndDuplicateKeys(t *testing.T) {
	responses := []*Response{{Text: `{"ok":true}`, FinishReason: "length"}, {Text: `{"ok":true}`, FinishReason: "max_tokens"}, {Text: `{"ok":true}`, FinishReason: "content_filter"}, {Text: `{"ok":true}`, FinishReason: "refusal"}, {Text: `{"ok":true}`, FinishReason: ""}, {Text: `{"ok":true}`, FinishReason: "unknown"}, {Text: `{"ok":true,"ok":false}`, FinishReason: "stop"}}
	for _, r := range responses {
		c := &Client{selected: responseProvider{r}}
		var out struct {
			OK bool `json:"ok"`
		}
		if e := c.CompleteTargetJSON(context.Background(), "", "", &out); e == nil {
			t.Errorf("unsafe response accepted: %#v", r)
		}
	}
	c := &Client{selected: responseProvider{&Response{Text: `{"ok":true}`, FinishReason: "stop"}}}
	var out struct {
		OK bool `json:"ok"`
	}
	if e := c.CompleteTargetJSON(context.Background(), "", "", &out); e != nil || !out.OK {
		t.Fatalf("valid JSON rejected: %v", e)
	}
}

func TestTargetClientUsesOpenAIMaxCompletionTokens(t *testing.T) {
	t.Setenv("OPENAI_API_KEY", "fixture")
	var got map[string]any
	s := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if e := json.NewDecoder(r.Body).Decode(&got); e != nil {
			t.Error(e)
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"choices":[{"message":{"content":"{\"ok\":true}"},"finish_reason":"stop"}]}`))
	}))
	defer s.Close()
	t.Setenv("OPENAI_BASE_URL", s.URL+"/v1")
	c, e := NewTargetClient(config.ModelTarget{Provider: "openai", Model: "reasoning-model"})
	if e != nil {
		t.Fatal(e)
	}
	var out struct {
		OK bool `json:"ok"`
	}
	if e = c.CompleteTargetJSON(context.Background(), "p", "s", &out); e != nil {
		t.Fatal(e)
	}
	if got["max_completion_tokens"] != float64(4096) {
		t.Fatalf("modern OpenAI budget missing: %#v", got)
	}
	if _, ok := got["max_tokens"]; ok {
		t.Fatalf("legacy max_tokens sent: %#v", got)
	}
}

func TestAnthropicTargetGetsValidDefaultBudget(t *testing.T) {
	t.Setenv("ANTHROPIC_API_KEY", "fixture")
	var got map[string]any
	s := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewDecoder(r.Body).Decode(&got)
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"content":[{"type":"text","text":"{\"ok\":true}"}],"stop_reason":"end_turn"}`))
	}))
	defer s.Close()
	t.Setenv("ANTHROPIC_BASE_URL", s.URL+"/v1")
	c, e := NewTargetClient(config.ModelTarget{Provider: "anthropic", Model: "claude-test", MaxOutputTokens: 0})
	if e != nil {
		t.Fatal(e)
	}
	var out struct {
		OK bool `json:"ok"`
	}
	if e = c.CompleteTargetJSON(context.Background(), "p", "s", &out); e != nil {
		t.Fatal(e)
	}
	if got["max_tokens"] != float64(8192) {
		t.Fatalf("Anthropic max_tokens invalid: %#v", got)
	}
}

func TestNewTargetClientRejectsInvalidRouteAndBudget(t *testing.T) {
	for _, target := range []config.ModelTarget{{Provider: "openai", Model: ""}, {Provider: "openai", Model: "x", MaxOutputTokens: -1}} {
		if _, e := NewTargetClient(target); e == nil {
			t.Fatalf("accepted %#v", target)
		}
	}
	t.Setenv("OPENAI_API_KEY", "")
	if _, e := NewTargetClient(config.ModelTarget{Provider: "openai", Model: "x"}); e == nil || !strings.Contains(e.Error(), "OPENAI_API_KEY") {
		t.Fatalf("expected actionable credential error: %v", e)
	}
}

type deadlineProvider struct{ hasDeadline *bool }

func (p deadlineProvider) Name() string { return "fixture" }
func (p deadlineProvider) Complete(ctx context.Context, _ *Request) (*Response, error) {
	_, ok := ctx.Deadline()
	*p.hasDeadline = ok
	return &Response{Text: `{"ok":true}`, FinishReason: "stop"}, nil
}
func TestTargetCallHasDeadline(t *testing.T) {
	has := false
	c := &Client{selected: deadlineProvider{&has}}
	var out struct {
		OK bool `json:"ok"`
	}
	if e := c.CompleteTargetJSON(context.Background(), "", "", &out); e != nil {
		t.Fatal(e)
	}
	if !has {
		t.Fatal("target provider request has no deadline")
	}
}

func TestTargetProviderBoundsHTTPResponse(t *testing.T) {
	t.Setenv("OPENAI_API_KEY", "fixture")
	s := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { _, _ = w.Write([]byte(strings.Repeat("x", (8<<20)+1))) }))
	defer s.Close()
	t.Setenv("OPENAI_BASE_URL", s.URL+"/v1")
	c, e := NewTargetClient(config.ModelTarget{Provider: "openai", Model: "test"})
	if e != nil {
		t.Fatal(e)
	}
	var out any
	e = c.CompleteTargetJSON(context.Background(), "", "", &out)
	if e == nil || !strings.Contains(e.Error(), "8 MiB") {
		t.Fatalf("expected bounded response error, got %v", e)
	}
}
