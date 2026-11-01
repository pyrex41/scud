package llm

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"testing"
	"time"
)

type catalogRoundTrip func(*http.Request) (*http.Response, error)

func (f catalogRoundTrip) RoundTrip(r *http.Request) (*http.Response, error) { return f(r) }

type catalogFailBody struct{ err error }

func (b catalogFailBody) Read([]byte) (int, error) { return 0, b.err }
func (b catalogFailBody) Close() error             { return nil }

func TestCatalogErrorsPreserveIdentityWithoutLeaking(t *testing.T) {
	t.Setenv("OPENAI_API_KEY", "secret-token")
	t.Setenv("OPENAI_BASE_URL", "https://user:secret-token@example.invalid/v1")
	for _, cause := range []error{context.Canceled, context.DeadlineExceeded, io.ErrUnexpectedEOF} {
		for _, body := range []bool{false, true} {
			t.Run(fmt.Sprintf("%v/body=%v", cause, body), func(t *testing.T) {
				unsafe := &url.Error{Op: "Get", URL: "https://secret-token@example.invalid", Err: fmt.Errorf("secret-token: %w", cause)}
				client := &http.Client{Transport: catalogRoundTrip(func(r *http.Request) (*http.Response, error) {
					if body {
						return &http.Response{StatusCode: 200, Body: catalogFailBody{unsafe}, Header: make(http.Header)}, nil
					}
					return nil, unsafe
				})}
				_, err := ListModelCatalog(context.Background(), "openai", client)
				if !errors.Is(err, cause) {
					t.Fatalf("lost cause: %v", err)
				}
				if strings.Contains(err.Error(), "secret-token") || strings.Contains(err.Error(), "example.invalid") {
					t.Fatalf("leaked sensitive transport text: %v", err)
				}
			})
		}
	}
}

func TestCatalogBodyCancellationUsesContextError(t *testing.T) {
	t.Setenv("OPENAI_API_KEY", "fixture")
	ctx, cancel := context.WithCancel(context.Background())
	client := &http.Client{Transport: catalogRoundTrip(func(r *http.Request) (*http.Response, error) {
		cancel()
		return &http.Response{StatusCode: 200, Body: catalogFailBody{io.ErrUnexpectedEOF}, Header: make(http.Header)}, nil
	})}
	_, err := ListModelCatalog(ctx, "openai", client)
	if !errors.Is(err, context.Canceled) {
		t.Fatalf("%v", err)
	}
}

func TestCatalogExpiredContextDoesNotQuery(t *testing.T) {
	for _, deadline := range []bool{false, true} {
		var ctx context.Context
		var cancel context.CancelFunc
		want := context.Canceled
		if deadline {
			ctx, cancel = context.WithDeadline(context.Background(), time.Now().Add(-time.Second))
			want = context.DeadlineExceeded
		} else {
			ctx, cancel = context.WithCancel(context.Background())
			cancel()
		}
		client := &http.Client{Transport: catalogRoundTrip(func(*http.Request) (*http.Response, error) { t.Error("queried after cancellation"); return nil, io.EOF })}
		_, err := ListModelCatalog(ctx, "openai", client)
		cancel()
		if !errors.Is(err, want) {
			t.Fatalf("%v", err)
		}
	}
}
