//go:build !windows

package storage

import (
	"os"
	"syscall"
	"time"
)

func lockShared(f *os.File) error {
	return lockWithRetry(f, syscall.LOCK_SH)
}

func lockExclusive(f *os.File) error {
	return lockWithRetry(f, syscall.LOCK_EX)
}

func unlock(f *os.File) {
	syscall.Flock(int(f.Fd()), syscall.LOCK_UN)
}

func lockWithRetry(f *os.File, lockType int) error {
	delay := 10 * time.Millisecond
	maxDelay := time.Second
	maxRetries := 10

	for i := 0; i < maxRetries; i++ {
		err := syscall.Flock(int(f.Fd()), lockType|syscall.LOCK_NB)
		if err == nil {
			return nil
		}
		time.Sleep(delay)
		delay *= 2
		if delay > maxDelay {
			delay = maxDelay
		}
	}
	// Final blocking attempt
	return syscall.Flock(int(f.Fd()), lockType)
}
