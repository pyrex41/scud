package llm

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestParseGrokCLIXAI(t *testing.T) {
	tests := []struct {
		name        string
		raw         string
		wantOK      bool
		wantToken   string
		wantRefresh string
		wantEmail   string
		wantExpiry  bool
	}{
		{
			name: "happy path",
			raw: `{
				"https://auth.x.ai::b1a00492-073a-47ea-816f-4c329264a828": {
					"key": "ey.access.token",
					"refresh_token": "rt-value",
					"expires_at": "2099-05-23T08:53:43.330932Z",
					"email": "user@example.com"
				}
			}`,
			wantOK:      true,
			wantToken:   "ey.access.token",
			wantRefresh: "rt-value",
			wantEmail:   "user@example.com",
			wantExpiry:  true,
		},
		{
			name:   "empty object",
			raw:    `{}`,
			wantOK: false,
		},
		{
			name:   "malformed json",
			raw:    `not json`,
			wantOK: false,
		},
		{
			name:   "no xai entry",
			raw:    `{"https://auth.example.com::xyz": {"key": "tok"}}`,
			wantOK: false,
		},
		{
			name:   "empty access token rejected",
			raw:    `{"https://auth.x.ai::abc": {"key": ""}}`,
			wantOK: false,
		},
		{
			name:      "no expires_at still works",
			raw:       `{"https://auth.x.ai::abc": {"key": "tok"}}`,
			wantOK:    true,
			wantToken: "tok",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			creds, ok := parseGrokCLIXAI([]byte(tt.raw))
			if ok != tt.wantOK {
				t.Fatalf("ok = %v, want %v", ok, tt.wantOK)
			}
			if !ok {
				return
			}
			if creds.AccessToken != tt.wantToken {
				t.Errorf("AccessToken = %q, want %q", creds.AccessToken, tt.wantToken)
			}
			if creds.RefreshToken != tt.wantRefresh {
				t.Errorf("RefreshToken = %q, want %q", creds.RefreshToken, tt.wantRefresh)
			}
			if creds.AccountLabel != tt.wantEmail {
				t.Errorf("AccountLabel = %q, want %q", creds.AccountLabel, tt.wantEmail)
			}
			if tt.wantExpiry && creds.ExpiresAt == 0 {
				t.Errorf("expected non-zero ExpiresAt")
			}
		})
	}
}

func TestRhoCredentialsRoundTrip(t *testing.T) {
	// Override HOME so UserConfigDir resolves into the tempdir.
	t.Setenv("HOME", t.TempDir())

	creds := xaiCredentials{
		AccessToken:  "tok-abc",
		RefreshToken: "rt-xyz",
		ExpiresAt:    time.Now().Unix() + 3600,
		AccountLabel: "test@example.com",
	}
	if err := saveRhoXAI(creds); err != nil {
		t.Fatalf("saveRhoXAI: %v", err)
	}

	loaded, err := loadRhoXAI()
	if err != nil {
		t.Fatalf("loadRhoXAI: %v", err)
	}
	if loaded == nil {
		t.Fatal("loadRhoXAI returned nil")
	}
	if loaded.AccessToken != creds.AccessToken {
		t.Errorf("AccessToken = %q, want %q", loaded.AccessToken, creds.AccessToken)
	}
	if loaded.RefreshToken != creds.RefreshToken {
		t.Errorf("RefreshToken = %q, want %q", loaded.RefreshToken, creds.RefreshToken)
	}
	if loaded.ExpiresAt != creds.ExpiresAt {
		t.Errorf("ExpiresAt = %d, want %d", loaded.ExpiresAt, creds.ExpiresAt)
	}
}

func TestSaveRhoXAIPreservesOtherProviders(t *testing.T) {
	t.Setenv("HOME", t.TempDir())

	path, err := rhoCredentialsPath()
	if err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	existing := map[string]map[string]any{
		"anthropic": {"access_token": "sk-ant-keep-me"},
	}
	raw, _ := json.Marshal(existing)
	if err := os.WriteFile(path, raw, 0o600); err != nil {
		t.Fatal(err)
	}

	if err := saveRhoXAI(xaiCredentials{AccessToken: "new-xai"}); err != nil {
		t.Fatalf("saveRhoXAI: %v", err)
	}

	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var got map[string]map[string]any
	if err := json.Unmarshal(data, &got); err != nil {
		t.Fatal(err)
	}
	if got["anthropic"]["access_token"] != "sk-ant-keep-me" {
		t.Errorf("anthropic entry was clobbered: %+v", got["anthropic"])
	}
	if got["xai"]["access_token"] != "new-xai" {
		t.Errorf("xai entry not written: %+v", got["xai"])
	}
}

func TestResolveXAITokenEnvWins(t *testing.T) {
	t.Setenv("HOME", t.TempDir())
	t.Setenv("XAI_API_KEY", "env-key-value")

	// Even with a rho creds file present, the env var must win.
	if err := saveRhoXAI(xaiCredentials{AccessToken: "rho-token"}); err != nil {
		t.Fatal(err)
	}

	tok, err := ResolveXAIToken(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if tok != "env-key-value" {
		t.Errorf("got %q, want env-key-value", tok)
	}
}

func TestResolveXAITokenGrokCLIBeforeRho(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("XAI_API_KEY", "")

	// Write a grok-CLI auth.json with a non-expired token.
	grokDir := filepath.Join(home, ".grok")
	if err := os.MkdirAll(grokDir, 0o700); err != nil {
		t.Fatal(err)
	}
	future := time.Now().Add(1 * time.Hour).UTC().Format(time.RFC3339Nano)
	grokRaw := `{"https://auth.x.ai::abc": {"key": "grok-token", "expires_at": "` + future + `"}}`
	if err := os.WriteFile(filepath.Join(grokDir, "auth.json"), []byte(grokRaw), 0o600); err != nil {
		t.Fatal(err)
	}

	// Also write rho credentials. Grok-CLI must win.
	if err := saveRhoXAI(xaiCredentials{AccessToken: "rho-token"}); err != nil {
		t.Fatal(err)
	}

	tok, err := ResolveXAIToken(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if tok != "grok-token" {
		t.Errorf("got %q, want grok-token", tok)
	}
}

func TestResolveXAITokenFallsThroughExpiredGrok(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("XAI_API_KEY", "")

	grokDir := filepath.Join(home, ".grok")
	if err := os.MkdirAll(grokDir, 0o700); err != nil {
		t.Fatal(err)
	}
	past := time.Now().Add(-1 * time.Hour).UTC().Format(time.RFC3339Nano)
	grokRaw := `{"https://auth.x.ai::abc": {"key": "expired-grok", "expires_at": "` + past + `"}}`
	if err := os.WriteFile(filepath.Join(grokDir, "auth.json"), []byte(grokRaw), 0o600); err != nil {
		t.Fatal(err)
	}

	// rho creds, not near expiry — should be picked.
	if err := saveRhoXAI(xaiCredentials{
		AccessToken: "rho-token",
		ExpiresAt:   time.Now().Add(1 * time.Hour).Unix(),
	}); err != nil {
		t.Fatal(err)
	}

	tok, err := ResolveXAIToken(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if tok != "rho-token" {
		t.Errorf("got %q, want rho-token (grok was expired)", tok)
	}
}

func TestResolveXAITokenNoCredentials(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	// Linux runners commonly set XDG_CONFIG_HOME independently of HOME.
	// Pin it too so this test cannot discover the runner user's rho credentials.
	t.Setenv("XDG_CONFIG_HOME", filepath.Join(home, ".config"))
	t.Setenv("XAI_API_KEY", "")

	_, err := ResolveXAIToken(context.Background())
	if err == nil {
		t.Fatal("expected error when no credentials available")
	}
}

func TestHasXAICredentials(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("XDG_CONFIG_HOME", filepath.Join(home, ".config"))
	t.Setenv("XAI_API_KEY", "")
	if HasXAICredentials() {
		t.Error("HasXAICredentials() should be false when nothing is set")
	}

	t.Setenv("XAI_API_KEY", "k")
	if !HasXAICredentials() {
		t.Error("HasXAICredentials() should be true when env var is set")
	}

	t.Setenv("XAI_API_KEY", "")
	if err := saveRhoXAI(xaiCredentials{AccessToken: "tok"}); err != nil {
		t.Fatal(err)
	}
	if !HasXAICredentials() {
		t.Error("HasXAICredentials() should be true when rho creds exist")
	}
}

func TestShouldRefresh(t *testing.T) {
	now := time.Now().Unix()
	tests := []struct {
		name string
		exp  int64
		want bool
	}{
		{"no expiry set", 0, false},
		{"far future", now + 3600, false},
		{"within skew", now + 60, true},
		{"already expired", now - 100, true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := shouldRefresh(xaiCredentials{ExpiresAt: tt.exp})
			if got != tt.want {
				t.Errorf("shouldRefresh(exp=%d) = %v, want %v", tt.exp, got, tt.want)
			}
		})
	}
}

func TestIsExpired(t *testing.T) {
	now := time.Now().Unix()
	if isExpired(xaiCredentials{}) {
		t.Error("unset expiry must not count as expired")
	}
	if isExpired(xaiCredentials{ExpiresAt: now + 3600}) {
		t.Error("future expiry must not count as expired")
	}
	if !isExpired(xaiCredentials{ExpiresAt: now - 1}) {
		t.Error("past expiry must count as expired")
	}
}

func TestRefreshXAITokenRoundTrip(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			t.Errorf("method = %s, want POST", r.Method)
		}
		if r.Header.Get("Content-Type") != "application/x-www-form-urlencoded" {
			t.Errorf("Content-Type = %q, want form-encoded", r.Header.Get("Content-Type"))
		}
		if err := r.ParseForm(); err != nil {
			t.Fatal(err)
		}
		if r.Form.Get("grant_type") != "refresh_token" {
			t.Errorf("grant_type = %q, want refresh_token", r.Form.Get("grant_type"))
		}
		if r.Form.Get("client_id") != xaiOAuthClientID {
			t.Errorf("client_id = %q, want %q", r.Form.Get("client_id"), xaiOAuthClientID)
		}
		if r.Form.Get("refresh_token") != "old-rt" {
			t.Errorf("refresh_token = %q, want old-rt", r.Form.Get("refresh_token"))
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"access_token":"new-at","refresh_token":"new-rt","expires_in":7200}`))
	}))
	defer srv.Close()

	// Swap the endpoint for the duration of the test by stubbing http.DefaultClient
	// would be invasive. Instead, hit the helper via an inline copy that targets
	// the test server. The production helper is exercised in production paths;
	// here we just verify the wire shape.
	creds, err := refreshXAITokenAt(context.Background(), srv.URL, "old-rt")
	if err != nil {
		t.Fatal(err)
	}
	if creds.AccessToken != "new-at" {
		t.Errorf("AccessToken = %q, want new-at", creds.AccessToken)
	}
	if creds.RefreshToken != "new-rt" {
		t.Errorf("RefreshToken = %q, want new-rt", creds.RefreshToken)
	}
	if creds.ExpiresAt == 0 {
		t.Error("ExpiresAt should be set from expires_in")
	}
}
