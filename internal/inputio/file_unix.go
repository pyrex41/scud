//go:build !windows

package inputio

import (
	"context"
	"errors"
	"fmt"
	"os"
	"time"

	"golang.org/x/sys/unix"
)

// Inherited stdin need not be registered with Go's poller. Close alone then
// cannot interrupt its blocking Read. Use nonblocking reads without spawning a
// reader goroutine; the owned descriptor's original flags are restored on return.
func readFile(ctx context.Context, f *os.File, limit int64) ([]byte, error) {
	raw, err := f.SyscallConn()
	if err != nil {
		return nil, err
	}
	var flags int
	var operationErr error
	if err := raw.Control(func(fd uintptr) {
		flags, operationErr = unix.FcntlInt(fd, unix.F_GETFL, 0)
		if operationErr == nil && flags&unix.O_NONBLOCK == 0 {
			operationErr = unix.SetNonblock(int(fd), true)
		}
	}); err != nil {
		return nil, err
	}
	if operationErr != nil {
		return nil, fmt.Errorf("making input interruptible: %w", operationErr)
	}
	defer func() {
		if flags&unix.O_NONBLOCK == 0 {
			_ = raw.Control(func(fd uintptr) { _ = unix.SetNonblock(int(fd), false) })
		}
	}()
	var b []byte
	buf := make([]byte, 32<<10)
	for {
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		want := min(int64(len(buf)), limit+1-int64(len(b)))
		if want == 0 {
			return b, nil
		}
		var n int
		if err := raw.Control(func(fd uintptr) {
			n, operationErr = unix.Read(int(fd), buf[:int(want)])
		}); err != nil {
			return nil, err
		}
		if n > 0 {
			b = append(b, buf[:n]...)
		}
		if operationErr == nil {
			if n == 0 {
				return b, nil
			}
			continue
		}
		if errors.Is(operationErr, unix.EINTR) {
			continue
		}
		if !errors.Is(operationErr, unix.EAGAIN) && !errors.Is(operationErr, unix.EWOULDBLOCK) {
			return nil, operationErr
		}
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-time.After(10 * time.Millisecond):
		}
	}
}
