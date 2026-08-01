// Package tdhttp connects SCUD v2 durability to td's fenced agent-run API.
package tdhttp

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"sync"

	"github.com/reuben/scud/v2/adapters"
)

type Client struct {
	BaseURL     string
	Token       string
	TicketID    string
	Attempt     uint64
	ExecutionID string
	HTTP        *http.Client
	mu          sync.Mutex
	tails       map[string]uint64
}

type Run struct {
	ID, GoalID, ConfigDigest string
}

func (c *Client) CreateRun(ctx context.Context, run Run) error {
	body := map[string]any{"id": run.ID, "ticket_id": c.TicketID, "goal_id": run.GoalID,
		"config_digest": run.ConfigDigest, "attempt": c.Attempt,
		"execution_id": c.ExecutionID, "idempotency_key": "scud:create:" + run.ID}
	return c.call(ctx, http.MethodPost, "/v1/agent/runs", body, nil)
}

// Authorize signs the exact unsigned rho.run/v1 request under td's live
// Shen-authorized claim. It matches executor.RhoV1.Authorize.
func (c *Client) Authorize(ctx context.Context, runID string, unsigned []byte) (string, string, error) {
	digest := fmt.Sprintf("%x", sha256.Sum256(unsigned))
	body := map[string]any{"request_sha256": digest, "unsigned_request": string(unsigned), "attempt": c.Attempt,
		"execution_id": c.ExecutionID, "idempotency_key": "scud:grant:" + runID + ":" + digest}
	var response struct {
		Allowed      bool   `json:"allowed"`
		Witness      string `json:"witness"`
		IssuerPubkey string `json:"issuer_pubkey"`
	}
	if err := c.call(ctx, http.MethodPost, "/v1/agent/runs/"+url.PathEscape(runID)+"/grants", body, &response); err != nil {
		return "", "", err
	}
	if !response.Allowed || response.Witness == "" || response.IssuerPubkey == "" {
		return "", "", fmt.Errorf("td denied execution grant")
	}
	return response.Witness, response.IssuerPubkey, nil
}

func (c *Client) Append(ctx context.Context, event adapters.Event) error {
	c.mu.Lock()
	defer c.mu.Unlock()
	if c.tails == nil {
		c.tails = map[string]uint64{}
	}
	tail := c.tails[event.RunID]
	if tail == 0 && event.Sequence > 1 {
		tail = event.Sequence - 1
	}
	body := map[string]any{"expected_tail": tail, "attempt": c.Attempt,
		"execution_id": c.ExecutionID, "idempotency_key": fmt.Sprintf("scud:%s:%d", event.RunID, event.Sequence),
		"event": map[string]any{"sequence": event.Sequence, "event_id": fmt.Sprintf("%s:%d", event.RunID, event.Sequence),
			"type": event.Type, "ts": event.Time, "payload": json.RawMessage(event.Data),
			"idempotency_key": fmt.Sprintf("scud:%s:%d", event.RunID, event.Sequence)}}
	if err := c.call(ctx, http.MethodPost, "/v1/agent/runs/"+url.PathEscape(event.RunID)+"/events", body, nil); err != nil {
		return err
	}
	c.tails[event.RunID] = event.Sequence
	return nil
}

func (c *Client) List(ctx context.Context, runID string, after uint64) ([]adapters.Event, error) {
	var response struct {
		Events []struct {
			Sequence uint64 `json:"sequence"`
			Type, TS string
			Payload  json.RawMessage
		} `json:"events"`
		Tail uint64 `json:"tail"`
	}
	path := "/v1/agent/runs/" + url.PathEscape(runID) + "/events?after=" + strconv.FormatUint(after, 10)
	if err := c.call(ctx, http.MethodGet, path, nil, &response); err != nil {
		return nil, err
	}
	c.mu.Lock()
	if c.tails == nil {
		c.tails = map[string]uint64{}
	}
	c.tails[runID] = response.Tail
	c.mu.Unlock()
	out := make([]adapters.Event, len(response.Events))
	for i, event := range response.Events {
		out[i] = adapters.Event{RunID: runID, Sequence: event.Sequence, Time: event.TS, Type: event.Type, Data: append([]byte(nil), event.Payload...)}
	}
	return out, nil
}

func (c *Client) call(ctx context.Context, method, path string, body any, out any) error {
	var reader io.Reader
	if body != nil {
		encoded, err := json.Marshal(body)
		if err != nil {
			return err
		}
		reader = bytes.NewReader(encoded)
	}
	req, err := http.NewRequestWithContext(ctx, method, strings.TrimRight(c.BaseURL, "/")+path, reader)
	if err != nil {
		return err
	}
	req.Header.Set("Authorization", "Bearer "+c.Token)
	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	httpClient := c.HTTP
	if httpClient == nil {
		httpClient = http.DefaultClient
	}
	resp, err := httpClient.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	data, _ := io.ReadAll(io.LimitReader(resp.Body, 1<<20))
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return fmt.Errorf("td %s %s returned %d: %s", method, path, resp.StatusCode, strings.TrimSpace(string(data)))
	}
	if out != nil && len(data) > 0 {
		return json.Unmarshal(data, out)
	}
	return nil
}

var _ adapters.EventStore = (*Client)(nil)
