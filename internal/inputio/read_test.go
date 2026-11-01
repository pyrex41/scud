package inputio

import (
	"bytes"
	"context"
	"errors"
	"io"
	"os"
	"strings"
	"testing"
	"time"
)

func TestReadAllBounds(t *testing.T) {
	for _, r := range []io.Reader{strings.NewReader("abc"), bytes.NewReader([]byte("abc")), bytes.NewBufferString("abc")} {
		b, err := ReadAll(context.Background(), r, 3)
		if err != nil || string(b) != "abc" {
			t.Fatalf("read = %q, %v", b, err)
		}
	}
	if _, err := ReadAll(context.Background(), strings.NewReader("abcd"), 3); !errors.Is(err, ErrTooLarge) {
		t.Fatalf("overflow = %v", err)
	}
	if _, err := ReadAll(context.Background(), io.LimitReader(strings.NewReader("abc"), 3), 3); !errors.Is(err, ErrUnsupportedReader) {
		t.Fatalf("unsupported = %v", err)
	}
}

func TestReadAllCancelsOwnedPipes(t *testing.T) {
	for _, file := range []bool{false, true} {
		t.Run(map[bool]string{false: "io.Pipe", true: "os.Pipe"}[file], func(t *testing.T) {
			var r io.ReadCloser
			var w io.WriteCloser
			if file {
				var err error
				r, w, err = os.Pipe()
				if err != nil {
					t.Fatal(err)
				}
			} else {
				r, w = io.Pipe()
			}
			defer r.Close()
			defer w.Close()
			ctx, cancel := context.WithTimeout(context.Background(), 20*time.Millisecond)
			defer cancel()
			start := time.Now()
			if _, err := ReadAll(ctx, r, 100); !errors.Is(err, context.DeadlineExceeded) {
				t.Fatalf("cancel = %v", err)
			}
			if time.Since(start) > time.Second {
				t.Fatal("read did not unblock")
			}
		})
	}
}

type trackedInput struct {
	*bytes.Reader
	closed bool
}

func (r *trackedInput) Close() error { r.closed = true; return nil }

func TestSuccessfulReadDisarmsClose(t *testing.T) {
	for range 100 {
		ctx, cancel := context.WithCancel(context.Background())
		r := &trackedInput{Reader: bytes.NewReader([]byte("abc"))}
		if _, err := ReadAll(ctx, r, 3); err != nil {
			t.Fatal(err)
		}
		cancel()
		if r.closed {
			t.Fatal("successful input was closed")
		}
	}
}
