package llm

import (
	"context"
	"fmt"
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestAnthropicCatalogHeadersAndData(t *testing.T) {
	s := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get("x-api-key") != "secret" || r.Header.Get("anthropic-version") == "" {
			t.Errorf("missing anthropic auth/version headers")
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"data":[{"id":"claude-test","display_name":"Claude Test"}],"has_more":false}`))
	}))
	defer s.Close()
	t.Setenv("ANTHROPIC_BASE_URL", s.URL)
	t.Setenv("ANTHROPIC_API_KEY", "secret")
	got, e := ListModelsWithClient(context.Background(), "anthropic", s.Client())
	if e != nil || len(got) != 1 || got[0].ID != "claude-test" {
		t.Fatalf("%#v %v", got, e)
	}
}
func TestOpenAICatalogRequiresKey(t *testing.T) {
	t.Setenv("OPENAI_API_KEY", "")
	if _, e := ListModels(context.Background(), "openai"); e == nil {
		t.Fatal("expected auth error")
	}
}

func TestCatalogEndpointOverrideAndAnthropicPagination(t *testing.T) {
	calls := 0
	s := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		calls++
		if r.URL.Path != "/custom/models" {
			t.Errorf("path %s", r.URL.Path)
		}
		if calls == 1 {
			if r.URL.Query().Get("after_id") != "" {
				t.Errorf("unexpected first cursor")
			}
			fmt.Fprint(w, `{"data":[{"id":"a"}],"has_more":true,"last_id":"a"}`)
		} else {
			if r.URL.Query().Get("after_id") != "a" {
				t.Errorf("cursor missing: %s", r.URL.RawQuery)
			}
			fmt.Fprint(w, `{"data":[{"id":"b"}],"has_more":false}`)
		}
	}))
	defer s.Close()
	t.Setenv("ANTHROPIC_BASE_URL", s.URL+"/custom")
	t.Setenv("ANTHROPIC_API_KEY", "test")
	models, e := ListModelsWithClient(context.Background(), "anthropic", s.Client())
	if e != nil || len(models) != 2 || calls != 2 {
		t.Fatalf("models=%#v calls=%d err=%v", models, calls, e)
	}
}

func TestCatalogUnknownProvider(t *testing.T) {
	if _, e := ListModels(context.Background(), "madeup"); e == nil {
		t.Fatal("unknown provider accepted")
	}
}
