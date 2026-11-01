// Package inputio provides bounded reads of command-owned input.
package inputio

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"math"
	"os"
	"strings"
)

var (
	ErrTooLarge          = errors.New("input exceeds size limit")
	ErrUnsupportedReader = errors.New("input reader must be in-memory or closeable and interruptible")
)

// ReadAll reads at most limit bytes, probing one extra byte to detect overflow.
// It accepts bytes.Buffer, bytes.Reader, strings.Reader, and owned io.ReadCloser
// inputs whose Close promptly interrupts Read (for example os.File and io.Pipe).
// The caller must not concurrently use an owned input. It is closed only on
// cancellation; the caller remains responsible for closing it after success.
// Arbitrary noncloseable readers are rejected: moving Read to a goroutine would
// leak that goroutine when cancellation cannot interrupt the underlying reader.
// Custom ReadClosers must honor the interruptible-Close contract.
// On Unix, owned file reads temporarily use nonblocking mode (restored before
// return); Windows console/character handles are rejected because Close cannot
// interrupt ReadConsole. Ordinary filesystem IO remains subject to kernel and
// filesystem responsiveness on every platform.
func ReadAll(ctx context.Context, r io.Reader, limit int64) ([]byte, error) {
	if limit < 0 || limit == math.MaxInt64 {
		return nil, fmt.Errorf("invalid input limit %d", limit)
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	var stop func() bool
	var closed chan struct{}
	switch r.(type) {
	case *bytes.Buffer, *bytes.Reader, *strings.Reader:
	default:
		closer, ok := r.(io.ReadCloser)
		if !ok {
			return nil, ErrUnsupportedReader
		}
		closed = make(chan struct{})
		stop = context.AfterFunc(ctx, func() {
			defer close(closed)
			_ = closer.Close()
		})
	}
	var b []byte
	var err error
	if f, ok := r.(*os.File); ok {
		b, err = readFile(ctx, f, limit)
	} else {
		b, err = io.ReadAll(io.LimitReader(r, limit+1))
	}
	if stop != nil && !stop() {
		// A callback already started: join it before returning ownership. This
		// prevents a delayed Close racing with the caller's next use of input.
		<-closed
	}
	if ctx.Err() != nil {
		return nil, ctx.Err()
	}
	if int64(len(b)) > limit {
		return nil, fmt.Errorf("%w (%d bytes)", ErrTooLarge, limit)
	}
	return b, err
}
