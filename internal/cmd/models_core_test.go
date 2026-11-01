package cmd

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/pkg/llm"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"testing"
)

func TestCatalogSetAndCompletePreservesNamespacedID(t *testing.T) {
	const id = "anthropic/claude-catalog-test"
	called := false
	s := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/models":
			fmt.Fprintf(w, `{"data":[{"id":%q}]}`, id)
		case "/chat/completions":
			var req struct {
				Model string `json:"model"`
			}
			if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
				t.Error(err)
			}
			if req.Model != id {
				t.Errorf("model changed: %q", req.Model)
			}
			called = true
			fmt.Fprint(w, `{"choices":[{"message":{"content":"{}"},"finish_reason":"stop"}]}`)
		default:
			t.Errorf("unexpected path %s", r.URL.Path)
			http.NotFound(w, r)
		}
	}))
	defer s.Close()
	t.Setenv("OPENROUTER_BASE_URL", s.URL)
	t.Setenv("OPENROUTER_API_KEY", "fixture")
	d := t.TempDir()
	load := func() (*config.Config, string, error) { cfg, err := config.LoadModelDefaults(d); return cfg, d, err }
	c := newModelsCmd(llm.ListModels, load)
	c.SetArgs([]string{"set", "main", id, "--provider", "openrouter"})
	if err := c.Execute(); err != nil {
		t.Fatal(err)
	}
	cfg, _, err := load()
	if err != nil {
		t.Fatal(err)
	}
	target, err := cfg.ResolveModelTarget("main")
	if err != nil {
		t.Fatal(err)
	}
	p, err := llm.NewTargetProvider(target.Provider)
	if err != nil {
		t.Fatal(err)
	}
	if _, err = p.Complete(context.Background(), &llm.Request{Model: target.Model, Prompt: "fixture"}); err != nil {
		t.Fatal(err)
	}
	if !called {
		t.Fatal("generation not called")
	}
}

func TestModelsCancellationNeverReturnsPartialSuccess(t *testing.T) {
	for _, args := range [][]string{{"list"}, {"set", "main", "match"}} {
		for _, cause := range []error{context.Canceled, context.DeadlineExceeded} {
			t.Run(fmt.Sprintf("%v/%v", args, cause), func(t *testing.T) {
				for _, p := range llm.CatalogProviders() {
					env, _ := llm.CatalogCredentialEnv(p)
					t.Setenv(env, "fixture")
				}
				calls := 0
				d := t.TempDir()
				cat := func(ctx context.Context, p string) ([]llm.CatalogModel, error) {
					calls++
					if calls == 1 {
						return []llm.CatalogModel{{ID: "match", Provider: p}}, nil
					}
					return nil, fmt.Errorf("catalog stopped: %w", cause)
				}
				c := newModelsCmd(cat, func() (*config.Config, string, error) { return &config.Config{}, d, nil })
				var out strings.Builder
				c.SetOut(&out)
				c.SetErr(&out)
				c.SetArgs(args)
				if err := c.Execute(); !errors.Is(err, cause) {
					t.Fatalf("lost cancellation: %v", err)
				}
				if calls != 2 {
					t.Fatalf("continued querying: %d", calls)
				}
				if _, err := os.Stat(d + "/config.toml"); !os.IsNotExist(err) {
					t.Fatalf("config published: %v", err)
				}
			})
		}
	}
}

func TestModelsStopsBetweenProviderQueries(t *testing.T) {
	t.Setenv("OPENAI_API_KEY", "fixture")
	t.Setenv("XAI_API_KEY", "fixture")
	ctx, cancel := context.WithCancel(context.Background())
	calls := 0
	c := newModelsCmd(func(context.Context, string) ([]llm.CatalogModel, error) {
		calls++
		cancel()
		return []llm.CatalogModel{{ID: "match", Provider: "openai"}}, nil
	}, func() (*config.Config, string, error) { return &config.Config{}, t.TempDir(), nil })
	c.SetArgs([]string{"list"})
	if err := c.ExecuteContext(ctx); !errors.Is(err, context.Canceled) {
		t.Fatalf("%v", err)
	}
	if calls != 1 {
		t.Fatalf("queried after cancellation: %d", calls)
	}
}

func TestModelsSetFuzzyAndAmbiguous(t *testing.T) {
	t.Setenv("OPENAI_API_KEY", "fixture")
	t.Setenv("XAI_API_KEY", "")
	t.Setenv("ANTHROPIC_API_KEY", "")
	t.Setenv("OPENROUTER_API_KEY", "")
	d := t.TempDir()
	cfg := config.Default()
	load := modelConfigFunc(func() (*config.Config, string, error) { return cfg, d, nil })
	cat := modelCatalogFunc(func(_ context.Context, p string) ([]llm.CatalogModel, error) {
		if p != "openai" {
			return nil, os.ErrNotExist
		}
		return []llm.CatalogModel{{ID: "gpt-4o", DisplayName: "GPT 4o", Provider: p}, {ID: "gpt-4o-mini", DisplayName: "GPT 4o Mini", Provider: p}}, nil
	})
	c := newModelsCmd(cat, load)
	c.SetArgs([]string{"set", "main", "gpt-4o"})
	var out strings.Builder
	c.SetOut(&out)
	if e := c.Execute(); e != nil {
		t.Fatal(e)
	}
	got, e := config.Load(d)
	if e != nil {
		t.Fatal(e)
	}
	target, e := got.ResolveModelTarget("main")
	if e != nil || target.Model != "gpt-4o" {
		t.Fatalf("%#v %v", target, e)
	}
	c = newModelsCmd(cat, load)
	c.SetArgs([]string{"set", "fast", "gpt 4o"})
	if e = c.Execute(); e == nil || !strings.Contains(e.Error(), "ambiguous") {
		t.Fatalf("expected ambiguity, got %v", e)
	}
}

func TestModelsDefaultsJSONIncludesSetupWhenUnset(t *testing.T) {
	load := modelConfigFunc(func() (*config.Config, string, error) { return &config.Config{}, t.TempDir(), nil })
	c := newModelsCmd(modelCatalogFunc(func(context.Context, string) ([]llm.CatalogModel, error) { return nil, nil }), load)
	c.SetArgs([]string{"defaults", "--json"})
	var out strings.Builder
	c.SetOut(&out)
	if e := c.Execute(); e != nil {
		t.Fatal(e)
	}
	var payload map[string]any
	if e := json.Unmarshal([]byte(out.String()), &payload); e != nil {
		t.Fatal(e)
	}
	if payload["setup"] == nil {
		t.Fatalf("missing setup guidance: %v", payload)
	}
}

func TestModelsRejectBlankQueryAndUnknownProvider(t *testing.T) {
	calls := 0
	cat := modelCatalogFunc(func(context.Context, string) ([]llm.CatalogModel, error) { calls++; return nil, nil })
	load := modelConfigFunc(func() (*config.Config, string, error) { return config.Default(), t.TempDir(), nil })
	for _, args := range [][]string{{"set", "main", "  "}, {"set", "fast", "x", "--provider", "unknown"}} {
		c := newModelsCmd(cat, load)
		c.SetArgs(args)
		if e := c.Execute(); e == nil {
			t.Fatalf("accepted %v", args)
		}
	}
	if calls != 0 {
		t.Fatalf("catalog called %d times on invalid input", calls)
	}
}

func TestModelsListLoadsConfigOnce(t *testing.T) {
	t.Setenv("OPENAI_API_KEY", "fixture")
	t.Setenv("XAI_API_KEY", "")
	t.Setenv("ANTHROPIC_API_KEY", "")
	t.Setenv("OPENROUTER_API_KEY", "")
	loads := 0
	load := modelConfigFunc(func() (*config.Config, string, error) { loads++; return &config.Config{}, t.TempDir(), nil })
	cat := modelCatalogFunc(func(context.Context, string) ([]llm.CatalogModel, error) {
		return []llm.CatalogModel{{ID: "gpt-x", Provider: "openai", DisplayName: "GPT X"}}, nil
	})
	c := newModelsCmd(cat, load)
	c.SetArgs([]string{"list"})
	if e := c.Execute(); e != nil {
		t.Fatal(e)
	}
	if loads != 1 {
		t.Fatalf("config loaded %d times", loads)
	}
}
