package mcp

import (
	"fmt"
	"testing"

	"github.com/mark3labs/mcp-go/mcp"
)

func TestContainsTool(t *testing.T) {
	tests := []struct {
		tier string
		name string
		want bool
	}{
		{"next,show,heavy", "heavy", true},
		{"next,show,heavy", "list", false},
		{"next, show, heavy", "show", true},
		{"list", "list", true},
		{"", "", true}, // edge case: empty matches empty
	}

	for _, tt := range tests {
		got := containsTool(tt.tier, tt.name)
		if got != tt.want {
			t.Errorf("containsTool(%q, %q) = %v, want %v", tt.tier, tt.name, got, tt.want)
		}
	}
}

func TestTextResultTruncation(t *testing.T) {
	short := "hello"
	result := textResult(short)
	if result.IsError {
		t.Error("short text should not be error")
	}
	if len(result.Content) != 1 {
		t.Fatalf("expected 1 content, got %d", len(result.Content))
	}

	// Test truncation at maxResponseLen
	long := make([]byte, maxResponseLen+100)
	for i := range long {
		long[i] = 'x'
	}
	result = textResult(string(long))
	tc, ok := result.Content[0].(mcp.TextContent)
	if !ok {
		t.Fatal("expected TextContent type")
	}
	if len(tc.Text) > maxResponseLen+50 { // allow for "... (truncated)" suffix
		t.Errorf("text length = %d, expected <= %d", len(tc.Text), maxResponseLen+50)
	}
}

func TestErrResult(t *testing.T) {
	result := errResult(fmt.Errorf("test error"))
	if !result.IsError {
		t.Error("expected IsError = true")
	}
}

func TestToolDefinitions(t *testing.T) {
	tools := []struct {
		name string
		fn   func() interface{ GetName() string }
	}{
		// Just verify tool constructors don't panic
	}
	_ = tools

	// Verify core tools can be constructed
	_ = toolWarmup()
	_ = toolNext()
	_ = toolShow()
	_ = toolSetStatus()
	_ = toolStats()
	_ = toolCommit()
	_ = toolList()
	_ = toolHeavy()
	_ = toolCreate()
}
