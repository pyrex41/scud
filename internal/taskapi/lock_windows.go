//go:build windows

package taskapi

import (
	"context"
	"os"

	"github.com/reuben/scud/internal/filelock"
)

func lock(ctx context.Context, f *os.File) error { return filelock.Lock(ctx, f) }
func unlock(f *os.File)                          { _ = filelock.Unlock(f) }
func syncDirectory(path string) error            { return filelock.SyncDir(path) }
