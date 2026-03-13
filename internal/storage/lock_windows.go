//go:build windows

package storage

import (
	"os"
	"time"

	"golang.org/x/sys/windows"
)

const (
	lockfileExclusiveLock = 0x00000002
	lockfileFailImmediately = 0x00000001
)

func lockShared(f *os.File) error {
	return lockWithRetry(f, 0)
}

func lockExclusive(f *os.File) error {
	return lockWithRetry(f, lockfileExclusiveLock)
}

func unlock(f *os.File) {
	ol := new(windows.Overlapped)
	windows.UnlockFileEx(windows.Handle(f.Fd()), 0, 1, 0, ol)
}

func lockWithRetry(f *os.File, flags uint32) error {
	delay := 10 * time.Millisecond
	maxDelay := time.Second
	maxRetries := 10

	ol := new(windows.Overlapped)
	for i := 0; i < maxRetries; i++ {
		err := windows.LockFileEx(
			windows.Handle(f.Fd()),
			flags|lockfileFailImmediately,
			0, 1, 0, ol,
		)
		if err == nil {
			return nil
		}
		time.Sleep(delay)
		delay *= 2
		if delay > maxDelay {
			delay = maxDelay
		}
	}
	// Final blocking attempt (no FAIL_IMMEDIATELY)
	return windows.LockFileEx(windows.Handle(f.Fd()), flags, 0, 1, 0, ol)
}
