package tdhttp

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/reuben/scud/v2/adapters"
)

func TestAppendAndReplayCarryFenceAndTail(t *testing.T) {
	var appended map[string]any
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get("Authorization") != "Bearer token" {
			t.Fatal("missing bearer")
		}
		switch {
		case r.Method == http.MethodPost && r.URL.Path == "/v1/agent/runs/run-1/events":
			if err := json.NewDecoder(r.Body).Decode(&appended); err != nil {
				t.Fatal(err)
			}
			w.WriteHeader(http.StatusCreated)
			_, _ = w.Write([]byte(`{"tail":1}`))
		case r.Method == http.MethodGet:
			_, _ = w.Write([]byte(`{"tail":1,"events":[{"sequence":1,"type":"core/goal_created","ts":"now","payload":{"kind":"goal_created"}}]}`))
		default:
			t.Fatalf("unexpected %s %s", r.Method, r.URL.Path)
		}
	}))
	defer srv.Close()
	c := &Client{BaseURL: srv.URL, Token: "token", TicketID: "ticket", Attempt: 3, ExecutionID: "exec"}
	if err := c.Append(context.Background(), adapters.Event{RunID: "run-1", Sequence: 1, Type: "core/goal_created", Data: []byte(`{"kind":"goal_created"}`)}); err != nil {
		t.Fatal(err)
	}
	if appended["expected_tail"].(float64) != 0 || appended["attempt"].(float64) != 3 || appended["execution_id"] != "exec" {
		t.Fatalf("bad fenced append: %#v", appended)
	}
	events, err := c.List(context.Background(), "run-1", 0)
	if err != nil {
		t.Fatal(err)
	}
	if len(events) != 1 || events[0].Sequence != 1 {
		t.Fatalf("bad replay: %#v", events)
	}
}
