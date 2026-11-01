package taskapi

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/reuben/scud/pkg/model"
)

func TestOversizedMergeDoesNotPublishUnreadableState(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, ".scud"), 0700); err != nil {
		t.Fatal(err)
	}
	service, err := Open(root)
	if err != nil {
		t.Fatal(err)
	}
	description := strings.Repeat("x", 17<<20)
	makeDocument := func(tag string) Document {
		return Document{SchemaVersion: 1, Phases: []Phase{{Tag: tag, Name: tag, IDFormat: "sequential", Tasks: []*model.Task{{ID: "1", Title: "Large task", Description: description, Status: model.Pending, Priority: model.Medium}}}}}
	}
	before, err := service.Import(t.Context(), makeDocument("a"), ImportOptions{})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := service.Import(t.Context(), makeDocument("b"), ImportOptions{}); err == nil {
		t.Fatal("oversized combined document was accepted")
	}
	after, err := service.Snapshot(t.Context())
	if err != nil || before.Revision != after.Revision || len(after.Phases) != 1 {
		t.Fatalf("failed merge changed or broke storage: err=%v phases=%d", err, len(after.Phases))
	}
}

func TestDecodeReturnsZeroOnFailure(t *testing.T) {
	value, err := Decode[struct {
		Title string `json:"title"`
	}](strings.NewReader(`{"title":"partial","unknown":true}`))
	if err == nil || value.Title != "" {
		t.Fatalf("partial decode escaped: %+v err=%v", value, err)
	}
}

func TestTypedRevisionConflict(t *testing.T) {
	root := t.TempDir()
	if err := os.Mkdir(filepath.Join(root, ".scud"), 0700); err != nil {
		t.Fatal(err)
	}
	service, err := Open(root)
	if err != nil {
		t.Fatal(err)
	}
	_, err = service.Update(t.Context(), Ref{Tag: "main", ID: "1"}, Patch{Status: new(model.Done)}, "stale")
	if conflict, ok := errors.AsType[*RevisionConflictError](err); !ok || conflict.Expected != "stale" {
		t.Fatalf("expected typed revision error, got %v", err)
	}
}
