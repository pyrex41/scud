package memory

import (
	"context"
	"testing"

	"github.com/reuben/scud/v2/adapters"
)

func TestTaskDirectoryReadyHonorsDependencies(t *testing.T) {
	directory := NewTaskDirectory([]adapters.Task{{ID: "a", Status: "done"}, {ID: "b", Status: "pending", Dependencies: []string{"a"}}, {ID: "c", Status: "pending", Dependencies: []string{"missing"}}})
	ready, err := directory.Ready(context.Background(), "")
	if err != nil || len(ready) != 1 || ready[0].ID != "b" {
		t.Fatalf("unexpected ready tasks: %+v, %v", ready, err)
	}
}
