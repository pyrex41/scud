package rho

import (
	"context"
	"testing"
	"time"
)

func TestExtractJSON(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  string
	}{
		{
			name:  "json code fence",
			input: "Here is the result:\n```json\n{\"key\": \"value\"}\n```\nDone.",
			want:  `{"key": "value"}`,
		},
		{
			name:  "plain code fence",
			input: "Result:\n```\n[1, 2, 3]\n```",
			want:  "[1, 2, 3]",
		},
		{
			name:  "bare json array",
			input: "The output is [1, 2, 3] here",
			want:  "[1, 2, 3]",
		},
		{
			name:  "bare json object",
			input: `The output is {"foo": "bar"} here`,
			want:  `{"foo": "bar"}`,
		},
		{
			name:  "raw json starting with bracket",
			input: `[{"id": 1}]`,
			want:  `[{"id": 1}]`,
		},
		{
			name:  "raw json starting with brace",
			input: `{"id": 1}`,
			want:  `{"id": 1}`,
		},
		{
			name:  "no json at all",
			input: "just some regular text",
			want:  "",
		},
		{
			name:  "json fence takes priority over bare",
			input: "```json\n{\"fenced\": true}\n```\n{\"bare\": true}",
			want:  `{"fenced": true}`,
		},
		{
			name:  "multiline json in fence",
			input: "```json\n{\n  \"key\": \"value\",\n  \"num\": 42\n}\n```",
			want:  "{\n  \"key\": \"value\",\n  \"num\": 42\n}",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := extractJSON(tt.input)
			if got != tt.want {
				t.Errorf("extractJSON() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestTruncate(t *testing.T) {
	tests := []struct {
		input string
		n     int
		want  string
	}{
		{"hello", 10, "hello"},
		{"hello world", 5, "hello..."},
		{"", 5, ""},
		{"abc", 3, "abc"},
		{"abcd", 3, "abc..."},
	}
	for _, tt := range tests {
		got := truncate(tt.input, tt.n)
		if got != tt.want {
			t.Errorf("truncate(%q, %d) = %q, want %q", tt.input, tt.n, got, tt.want)
		}
	}
}

func TestAdaptiveTimeoutHeartbeat(t *testing.T) {
	ctx, at := NewAdaptiveTimeout(
		context.Background(),
		100*time.Millisecond, // base timeout
		200*time.Millisecond, // idle timeout
		2*time.Second,        // max timeout
	)

	// Should start alive
	select {
	case <-ctx.Done():
		t.Fatal("context should not be done yet")
	default:
	}

	// Heartbeat should extend
	if !at.Heartbeat() {
		t.Error("Heartbeat() should return true within max timeout")
	}
	if !at.Extended() {
		t.Error("Extended() should be true after heartbeat")
	}

	// Wait for idle timeout without heartbeat
	time.Sleep(300 * time.Millisecond)
	select {
	case <-ctx.Done():
		// expected
	default:
		t.Error("context should be done after idle timeout")
	}
}

func TestAdaptiveTimeoutMaxCap(t *testing.T) {
	ctx, at := NewAdaptiveTimeout(
		context.Background(),
		50*time.Millisecond,  // base
		500*time.Millisecond, // idle
		150*time.Millisecond, // max - short cap
	)

	// Keep heartbeating past the max
	for i := 0; i < 5; i++ {
		at.Heartbeat()
		time.Sleep(30 * time.Millisecond)
	}

	// After max timeout, context should be cancelled
	select {
	case <-ctx.Done():
		// expected - max timeout reached
	case <-time.After(500 * time.Millisecond):
		t.Error("context should have been cancelled by max timeout")
	}
}

func TestAdaptiveTimeoutIdleDuration(t *testing.T) {
	_, at := NewAdaptiveTimeout(
		context.Background(),
		5*time.Second,
		5*time.Second,
		10*time.Second,
	)
	defer at.cancel()

	time.Sleep(50 * time.Millisecond)
	idle := at.IdleDuration()
	if idle < 50*time.Millisecond {
		t.Errorf("IdleDuration() = %v, expected >= 50ms", idle)
	}

	at.Heartbeat()
	idle = at.IdleDuration()
	if idle > 10*time.Millisecond {
		t.Errorf("IdleDuration() after heartbeat = %v, expected near zero", idle)
	}
}
