package executor

import (
	"strings"
	"testing"
)

func TestConsumeRhoV1CompletedStream(t *testing.T) {
	stream := strings.Join([]string{
		`{"protocol":"rho.run/v1","run_id":"run-1","seq":1,"time":"2026-07-31T20:00:00Z","type":"run.started","data":{"provider":"anthropic"}}`,
		`{"protocol":"rho.run/v1","run_id":"run-1","seq":3,"time":"2026-07-31T20:00:01Z","type":"message.delta","data":{"text":"Done."}}`,
		`{"protocol":"rho.run/v1","run_id":"run-1","seq":4,"time":"2026-07-31T20:00:02Z","type":"run.completed","data":{"status":"succeeded","stop_reason":"complete","usage":{"input_tokens":12,"output_tokens":3},"artifacts":[]}}`,
	}, "\n")

	var types []string
	result, err := ConsumeRhoV1(strings.NewReader(stream), "run-1", func(event Event) {
		types = append(types, event.Type)
	})
	if err != nil {
		t.Fatal(err)
	}
	if result.Text != "Done." || result.Outcome != "completed" {
		t.Fatalf("unexpected result: %#v", result)
	}
	if result.Usage.InputTokens != 12 || result.Usage.OutputTokens != 3 {
		t.Fatalf("unexpected usage: %#v", result.Usage)
	}
	if got := strings.Join(types, ","); got != "run.started,message.delta,run.completed" {
		t.Fatalf("event types = %q", got)
	}
}

func TestConsumeRhoV1PreservesFailure(t *testing.T) {
	stream := `{"protocol":"rho.run/v1","run_id":"run-2","seq":1,"time":"2026-07-31T20:00:00Z","type":"run.failed","data":{"code":"rate_limited","message":"try later","retryable":true,"retry_after_ms":500}}`
	result, err := ConsumeRhoV1(strings.NewReader(stream), "run-2", nil)
	if err != nil {
		t.Fatal(err)
	}
	if result.Outcome != "failed" || result.Failure == nil || !result.Failure.Retryable {
		t.Fatalf("unexpected failure result: %#v", result)
	}
}

func TestConsumeRhoV1RejectsInvalidStreams(t *testing.T) {
	tests := map[string]string{
		"wrong protocol":   `{"protocol":"rho.run/v2","run_id":"run-1","seq":1,"time":"now","type":"run.cancelled","data":{"reason":"stop"}}`,
		"wrong run":        `{"protocol":"rho.run/v1","run_id":"other","seq":1,"time":"now","type":"run.cancelled","data":{"reason":"stop"}}`,
		"missing terminal": `{"protocol":"rho.run/v1","run_id":"run-1","seq":1,"time":"now","type":"run.started","data":{}}`,
		"post terminal": strings.Join([]string{
			`{"protocol":"rho.run/v1","run_id":"run-1","seq":1,"time":"now","type":"run.cancelled","data":{"reason":"stop"}}`,
			`{"protocol":"rho.run/v1","run_id":"run-1","seq":2,"time":"now","type":"message.delta","data":{"text":"late"}}`,
		}, "\n"),
		"non monotonic": strings.Join([]string{
			`{"protocol":"rho.run/v1","run_id":"run-1","seq":2,"time":"now","type":"run.started","data":{}}`,
			`{"protocol":"rho.run/v1","run_id":"run-1","seq":2,"time":"now","type":"run.cancelled","data":{"reason":"stop"}}`,
		}, "\n"),
	}
	for name, stream := range tests {
		t.Run(name, func(t *testing.T) {
			if _, err := ConsumeRhoV1(strings.NewReader(stream), "run-1", nil); err == nil {
				t.Fatal("expected invalid stream to fail")
			}
		})
	}
}

func TestRhoV1RequestRequiresProvider(t *testing.T) {
	_, err := (RhoV1{}).Run(t.Context(), Request{RunID: "run-1", Model: ModelRef{ID: "custom"}}, nil)
	if err == nil {
		t.Fatal("expected missing provider to fail before process invocation")
	}
}
