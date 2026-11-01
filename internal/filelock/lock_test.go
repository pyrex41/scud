package filelock

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestContentionRespectsContext(t *testing.T) {
	path := filepath.Join(t.TempDir(), "lock")
	first, err := os.OpenFile(path, os.O_CREATE|os.O_RDWR, 0600)
	if err != nil {
		t.Fatal(err)
	}
	defer first.Close()
	second, err := os.OpenFile(path, os.O_RDWR, 0600)
	if err != nil {
		t.Fatal(err)
	}
	defer second.Close()
	if err := Lock(t.Context(), first); err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(t.Context(), 50*time.Millisecond)
	defer cancel()
	if err := Lock(ctx, second); !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("contended lock err=%v", err)
	}
	if err := Unlock(first); err != nil {
		t.Fatal(err)
	}
	if err := Lock(t.Context(), second); err != nil {
		t.Fatal(err)
	}
	if err := Unlock(second); err != nil {
		t.Fatal(err)
	}
}
