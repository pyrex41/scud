package llm

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"
)

// xAI OAuth constants — these match the Grok-CLI public client that rho's
// loopback / device-code flows are registered against.
const (
	xaiOAuthClientID  = "b1a00492-073a-47ea-816f-4c329264a828"
	xaiTokenURL       = "https://auth.x.ai/oauth2/token"
	xaiRefreshSkew    = 120 * time.Second
	xaiRefreshTimeout = 30 * time.Second
)

// xaiCredentials matches the per-provider shape rho writes into its unified
// credentials file under the "xai" key. Field names mirror rho's JSON.
type xaiCredentials struct {
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token,omitempty"`
	ExpiresAt    int64  `json:"expires_at,omitempty"` // unix epoch seconds
	AccountLabel string `json:"account_label,omitempty"`
}

// rhoCredentialsFile is the on-disk layout of ~/.config/rho/credentials.json
// (or the platform equivalent). Other providers (e.g. "anthropic") may also
// be present; we only touch the "xai" key.
type rhoCredentialsFile map[string]xaiCredentials

// grokCLIEntry matches a single entry inside ~/.grok/auth.json keyed by
// "<issuer>::<client_id>". We never write to this file.
type grokCLIEntry struct {
	Key          string `json:"key"`
	RefreshToken string `json:"refresh_token"`
	ExpiresAt    string `json:"expires_at"` // RFC 3339
	Email        string `json:"email,omitempty"`
}

// Serialize concurrent refreshes within this process. There's still a cross-
// process race with rho itself; the loser will get a 4xx and re-read.
var xaiRefreshMu sync.Mutex

// HasXAICredentials reports whether *some* credential source is available
// without doing any network I/O. Used by provider constructors to decide
// whether to register the xAI provider at all.
func HasXAICredentials() bool {
	if strings.TrimSpace(os.Getenv("XAI_API_KEY")) != "" {
		return true
	}
	if _, ok := loadGrokCLIXAI(); ok {
		return true
	}
	if creds, err := loadRhoXAI(); err == nil && creds != nil && creds.AccessToken != "" {
		return true
	}
	return false
}

// ResolveXAIToken returns a usable bearer token for xAI, refreshing if the
// stored token is within xaiRefreshSkew of expiry.
//
// Resolution order matches rho:
//  1. XAI_API_KEY env var (caller-supplied static key)
//  2. ~/.grok/auth.json (written by xAI's official grok CLI — read-only)
//  3. rho's unified credentials file (refreshed in place)
func ResolveXAIToken(ctx context.Context) (string, error) {
	if env := strings.TrimSpace(os.Getenv("XAI_API_KEY")); env != "" {
		return env, nil
	}

	if creds, ok := loadGrokCLIXAI(); ok && !isExpired(creds) {
		return creds.AccessToken, nil
	}
	// If the grok-CLI token is expired we deliberately fall through; we don't
	// write back to ~/.grok/auth.json so there's nowhere to persist a refresh.

	creds, err := loadRhoXAI()
	if err != nil {
		return "", err
	}
	if creds == nil {
		return "", errors.New("no xAI credentials found: set XAI_API_KEY, or run `rho auth xai` to sign in with your SuperGrok / X Premium subscription")
	}

	if shouldRefresh(*creds) && creds.RefreshToken != "" {
		xaiRefreshMu.Lock()
		defer xaiRefreshMu.Unlock()

		// Another goroutine in this process may have already refreshed.
		if fresh, err := loadRhoXAI(); err == nil && fresh != nil && !shouldRefresh(*fresh) {
			return fresh.AccessToken, nil
		}

		refreshed, refreshErr := refreshXAIToken(ctx, creds.RefreshToken)
		if refreshErr == nil {
			// xAI may not return a new refresh_token on every rotation; preserve.
			if refreshed.RefreshToken == "" {
				refreshed.RefreshToken = creds.RefreshToken
			}
			if refreshed.AccountLabel == "" {
				refreshed.AccountLabel = creds.AccountLabel
			}
			_ = saveRhoXAI(refreshed) // best-effort persist; in-memory token still valid
			return refreshed.AccessToken, nil
		}
		// Refresh failed (network, revoked refresh token, etc). Fall through
		// to the stored token — it may still work for a bit, and the worst
		// case (a 401 at request time) gives the user a clearer signal than
		// blocking the call entirely.
	}

	return creds.AccessToken, nil
}

// --- grok-CLI reader ---

func grokCLIPath() string {
	home, err := os.UserHomeDir()
	if err != nil {
		return ""
	}
	return filepath.Join(home, ".grok", "auth.json")
}

func loadGrokCLIXAI() (xaiCredentials, bool) {
	path := grokCLIPath()
	if path == "" {
		return xaiCredentials{}, false
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return xaiCredentials{}, false
	}
	return parseGrokCLIXAI(data)
}

func parseGrokCLIXAI(data []byte) (xaiCredentials, bool) {
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		return xaiCredentials{}, false
	}
	// We don't pin the client_id — if xAI rotates the Grok-CLI client, the
	// "https://auth.x.ai::" prefix still resolves the entry.
	for key, value := range raw {
		if !strings.HasPrefix(key, "https://auth.x.ai::") {
			continue
		}
		var entry grokCLIEntry
		if err := json.Unmarshal(value, &entry); err != nil {
			continue
		}
		if entry.Key == "" {
			continue
		}
		creds := xaiCredentials{
			AccessToken:  entry.Key,
			RefreshToken: entry.RefreshToken,
			AccountLabel: entry.Email,
		}
		if entry.ExpiresAt != "" {
			if t, err := time.Parse(time.RFC3339Nano, entry.ExpiresAt); err == nil {
				creds.ExpiresAt = t.Unix()
			} else if t, err := time.Parse(time.RFC3339, entry.ExpiresAt); err == nil {
				creds.ExpiresAt = t.Unix()
			}
		}
		return creds, true
	}
	return xaiCredentials{}, false
}

// --- rho credentials reader/writer ---

// rhoCredentialsPath mirrors rho's `dirs::config_dir()/rho/credentials.json`.
// Go's os.UserConfigDir() returns the same per-platform paths as the Rust
// `dirs` crate (Linux: ~/.config, macOS: ~/Library/Application Support).
func rhoCredentialsPath() (string, error) {
	dir, err := os.UserConfigDir()
	if err != nil {
		return "", fmt.Errorf("locating user config dir: %w", err)
	}
	return filepath.Join(dir, "rho", "credentials.json"), nil
}

func loadRhoXAI() (*xaiCredentials, error) {
	path, err := rhoCredentialsPath()
	if err != nil {
		return nil, err
	}
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("reading rho credentials: %w", err)
	}
	var file rhoCredentialsFile
	if err := json.Unmarshal(data, &file); err != nil {
		return nil, fmt.Errorf("parsing rho credentials: %w", err)
	}
	entry, ok := file["xai"]
	if !ok || entry.AccessToken == "" {
		return nil, nil
	}
	return &entry, nil
}

func saveRhoXAI(creds xaiCredentials) error {
	path, err := rhoCredentialsPath()
	if err != nil {
		return err
	}
	// Read-merge-write so we preserve other providers' entries (e.g. anthropic).
	file := rhoCredentialsFile{}
	if data, err := os.ReadFile(path); err == nil {
		_ = json.Unmarshal(data, &file)
	}
	file["xai"] = creds

	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return fmt.Errorf("creating rho config dir: %w", err)
	}
	out, err := json.MarshalIndent(file, "", "  ")
	if err != nil {
		return fmt.Errorf("serializing rho credentials: %w", err)
	}
	if err := os.WriteFile(path, out, 0o600); err != nil {
		return fmt.Errorf("writing rho credentials: %w", err)
	}
	return nil
}

// --- refresh ---

func refreshXAIToken(ctx context.Context, refreshToken string) (xaiCredentials, error) {
	return refreshXAITokenAt(ctx, xaiTokenURL, refreshToken)
}

// refreshXAITokenAt is the testable variant — it accepts an explicit token
// endpoint so tests can point at an httptest.Server.
func refreshXAITokenAt(ctx context.Context, tokenURL, refreshToken string) (xaiCredentials, error) {
	refreshCtx, cancel := context.WithTimeout(ctx, xaiRefreshTimeout)
	defer cancel()

	form := url.Values{}
	form.Set("grant_type", "refresh_token")
	form.Set("refresh_token", refreshToken)
	form.Set("client_id", xaiOAuthClientID)

	req, err := http.NewRequestWithContext(refreshCtx, http.MethodPost, tokenURL, strings.NewReader(form.Encode()))
	if err != nil {
		return xaiCredentials{}, fmt.Errorf("building refresh request: %w", err)
	}
	req.Header.Set("Accept", "application/json")
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return xaiCredentials{}, fmt.Errorf("xAI token refresh: %w", err)
	}
	defer resp.Body.Close()

	body, _ := io.ReadAll(resp.Body)
	if resp.StatusCode != http.StatusOK {
		return xaiCredentials{}, fmt.Errorf("xAI token refresh returned %d: %s", resp.StatusCode, truncate(string(body), 300))
	}

	var parsed struct {
		AccessToken  string `json:"access_token"`
		RefreshToken string `json:"refresh_token"`
		ExpiresIn    int64  `json:"expires_in"`
	}
	if err := json.Unmarshal(body, &parsed); err != nil {
		return xaiCredentials{}, fmt.Errorf("parsing refresh response: %w", err)
	}
	if parsed.AccessToken == "" {
		return xaiCredentials{}, errors.New("xAI refresh response missing access_token")
	}
	creds := xaiCredentials{
		AccessToken:  parsed.AccessToken,
		RefreshToken: parsed.RefreshToken,
	}
	if parsed.ExpiresIn > 0 {
		creds.ExpiresAt = time.Now().Unix() + parsed.ExpiresIn
	}
	return creds, nil
}

// --- expiry helpers ---

func isExpired(creds xaiCredentials) bool {
	if creds.ExpiresAt == 0 {
		return false
	}
	return time.Now().Unix() >= creds.ExpiresAt
}

func shouldRefresh(creds xaiCredentials) bool {
	if creds.ExpiresAt == 0 {
		return false
	}
	return time.Now().Unix()+int64(xaiRefreshSkew.Seconds()) >= creds.ExpiresAt
}
