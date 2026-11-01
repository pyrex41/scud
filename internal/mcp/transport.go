package mcp

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"sync"

	sdk "github.com/modelcontextprotocol/go-sdk/mcp"
	"github.com/reuben/scud/internal/taskapi"
)

// Serve owns the supplied streams for the duration of the stdio server. The
// official SDK cancels handlers on EOF before waiting for them to finish.
func Serve(ctx context.Context, version, selection, root string, in io.ReadCloser, out io.Writer) error {
	s, err := NewServer(version, selection, root)
	if err != nil {
		return err
	}
	return serveServer(ctx, s, in, out)
}

func serveServer(ctx context.Context, s *sdk.Server, in io.ReadCloser, out io.Writer) error {
	writer := &lockedWriter{Writer: out}
	reader := &strictReader{source: in, scanner: bufio.NewScanner(in), out: writer}
	reader.scanner.Buffer(make([]byte, 64<<10), taskapi.MaxJSONBytes+1)
	return s.Run(ctx, &sdk.IOTransport{Reader: reader, Writer: writer})
}

// strictReader validates each complete raw frame BEFORE SDK envelope/metadata
// decoding. Raw tool arguments then undergo the same strict typed decoder as
// CLI input. SDK convenience typed handlers are deliberately not used.
type strictReader struct {
	source  io.ReadCloser
	scanner *bufio.Scanner
	out     io.Writer
	pending []byte
}

func (r *strictReader) Read(p []byte) (int, error) {
	for len(r.pending) == 0 {
		if !r.scanner.Scan() {
			if err := r.scanner.Err(); err != nil {
				return 0, fmt.Errorf("MCP input frame: %w", err)
			}
			return 0, io.EOF
		}
		raw := r.scanner.Bytes()
		if len(bytes.TrimSpace(raw)) == 0 {
			continue
		}
		if err := validateFrame(raw); err != nil {
			if err := writeFrameError(r.out, err); err != nil {
				return 0, err
			}
			continue
		}
		r.pending = append(append([]byte{}, raw...), '\n')
	}
	n := copy(p, r.pending)
	r.pending = r.pending[n:]
	return n, nil
}

func (r *strictReader) Close() error { return r.source.Close() }

type lockedWriter struct {
	io.Writer
	mu       sync.Mutex
	once     sync.Once
	closeErr error
}

func (w *lockedWriter) Write(b []byte) (int, error) {
	w.mu.Lock()
	defer w.mu.Unlock()
	return w.Writer.Write(b)
}

func (w *lockedWriter) Close() error {
	// Do not wait for the write mutex: closing an owned pipe is what unblocks
	// a writer if the client disappeared without draining its output.
	w.once.Do(func() {
		if closer, ok := w.Writer.(io.Closer); ok {
			w.closeErr = closer.Close()
		}
	})
	return w.closeErr
}

func validateFrame(raw []byte) error {
	fields, err := taskapi.Decode[map[string]json.RawMessage](bytes.NewReader(raw))
	if err != nil {
		return err
	}
	if fields == nil {
		return fmt.Errorf("MCP frame must be an object")
	}
	return nil
}

type frameError struct {
	JSONRPC string `json:"jsonrpc"`
	Error   struct {
		Code    int    `json:"code"`
		Message string `json:"message"`
	} `json:"error"`
}

func writeFrameError(out io.Writer, cause error) error {
	response := frameError{JSONRPC: "2.0"}
	response.Error.Code = -32700
	response.Error.Message = "Invalid JSON-RPC frame: " + cause.Error()
	// No request ID is trusted when the raw frame has duplicate/malformed keys.
	return json.NewEncoder(out).Encode(response)
}

// HTTPHandler serves current stateless Streamable HTTP. Each POST carries all
// protocol metadata, has its own cancellation scope, and creates no session ID.
func HTTPHandler(version, selection, root string) (http.Handler, error) {
	s, err := NewServer(version, selection, root)
	if err != nil {
		return nil, err
	}
	return httpHandler(s), nil
}

func httpHandler(s *sdk.Server) http.Handler {
	handler := sdk.NewStreamableHTTPHandler(func(*http.Request) *sdk.Server { return s }, &sdk.StreamableHTTPOptions{
		Stateless: true, JSONResponse: true, PropagateRequestCancellation: true,
		MaxRequestBodyBytes: taskapi.MaxJSONBytes,
	})
	guard := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Validate Origin even for safe/legacy methods; cross-origin protection
		// alone intentionally skips safe HTTP methods. This local endpoint has
		// no cross-origin/browser trust exceptions.
		if origin := r.Header.Get("Origin"); origin != "" {
			u, err := url.Parse(origin)
			scheme := "http"
			if r.TLS != nil {
				scheme = "https"
			}
			if err != nil || u.User != nil || u.Scheme != scheme || !strings.EqualFold(u.Host, r.Host) || u.Path != "" || u.RawQuery != "" || u.Fragment != "" {
				http.Error(w, "Origin not allowed", http.StatusForbidden)
				return
			}
		}
		if r.URL.Path != "/mcp" {
			http.NotFound(w, r)
			return
		}
		if r.Method == http.MethodPost {
			body := http.MaxBytesReader(w, r.Body, taskapi.MaxJSONBytes)
			data, err := io.ReadAll(body)
			_ = body.Close()
			if err != nil {
				var tooLarge *http.MaxBytesError
				if errors.As(err, &tooLarge) {
					http.Error(w, "MCP frame exceeds size limit", http.StatusRequestEntityTooLarge)
				} else {
					http.Error(w, "Cannot read MCP frame", http.StatusBadRequest)
				}
				return
			}
			if err := validateFrame(data); err != nil {
				w.Header().Set("Content-Type", "application/json")
				w.WriteHeader(http.StatusBadRequest)
				_ = writeFrameError(w, err)
				return
			}
			r.Body = io.NopCloser(bytes.NewReader(data))
			r.ContentLength = int64(len(data))
		}
		handler.ServeHTTP(w, r)
	})
	// Unlike sessions, origin protection is a security boundary, not state.
	// The SDK also keeps its default localhost/DNS-rebinding protection.
	return http.NewCrossOriginProtection().Handler(guard)
}

// ownedInput allows only readers whose shutdown behavior is known. In-memory
// test input cannot block; production stdin is an owned, interruptible file.
func OwnedInput(r io.Reader) (io.ReadCloser, error) {
	if closer, ok := r.(io.ReadCloser); ok {
		return closer, nil
	}
	switch r.(type) {
	case *bytes.Buffer, *bytes.Reader, *strings.Reader:
		return io.NopCloser(r), nil
	default:
		return nil, fmt.Errorf("MCP stdio requires an interruptible owned input stream")
	}
}
