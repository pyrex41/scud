package inputio

import (
	"context"
	"io"
	"os"

	"golang.org/x/sys/windows"
)

// os.File's Windows implementation cancels pending pipe IO on Close. Console
// ReadConsole is synchronous and cannot be interrupted this way, so reject it
// rather than pretending to provide cancellation or leaking a reader goroutine.
func readFile(_ context.Context, f *os.File, limit int64) ([]byte, error) {
	raw, err := f.SyscallConn()
	if err != nil {
		return nil, err
	}
	var kind uint32
	var operationErr error
	if err := raw.Control(func(fd uintptr) { kind, operationErr = windows.GetFileType(windows.Handle(fd)) }); err != nil {
		return nil, err
	}
	if operationErr != nil {
		return nil, operationErr
	}
	if kind != windows.FILE_TYPE_DISK && kind != windows.FILE_TYPE_PIPE {
		return nil, ErrUnsupportedReader
	}
	return io.ReadAll(io.LimitReader(f, limit+1))
}
