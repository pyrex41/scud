package mcp

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"reflect"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"

	sdk "github.com/modelcontextprotocol/go-sdk/mcp"
	"github.com/reuben/scud/internal/taskapi"
)

const requestMeta = `"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{"name":"scud-test","version":"1"},"io.modelcontextprotocol/clientCapabilities":{}}`

func rpcRequest(method, params string) string {
	if params != "" {
		params = "," + params
	}
	return `{"jsonrpc":"2.0","id":1,"method":` + strconv.Quote(method) + `,"params":{` + requestMeta + params + `}}`
}

func toolRequest(name, args string) string {
	return rpcRequest("tools/call", `"name":`+strconv.Quote(name)+`,"arguments":`+args)
}

type rpcResponse struct {
	ID     json.RawMessage `json:"id"`
	Result json.RawMessage `json:"result"`
	Error  *struct {
		Code    int    `json:"code"`
		Message string `json:"message"`
	} `json:"error"`
}

func decodeResponse(t *testing.T, raw string) rpcResponse {
	t.Helper()
	var response rpcResponse
	if err := json.Unmarshal([]byte(raw), &response); err != nil {
		t.Fatalf("invalid response %q: %v", raw, err)
	}
	return response
}

func testProject(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	if err := os.Mkdir(filepath.Join(root, ".scud"), 0700); err != nil {
		t.Fatal(err)
	}
	return root
}

type stdioClient struct {
	in     *io.PipeWriter
	reader *bufio.Reader
	out    *io.PipeReader
	done   <-chan error
	once   sync.Once
}

func startStdio(t *testing.T, s *sdk.Server) *stdioClient {
	t.Helper()
	inR, inW := io.Pipe()
	outR, outW := io.Pipe()
	done := make(chan error, 1)
	ctx, cancel := context.WithCancel(t.Context())
	go func() { done <- serveServer(ctx, s, inR, outW) }()
	client := &stdioClient{in: inW, reader: bufio.NewReader(outR), out: outR, done: done}
	t.Cleanup(func() {
		cancel()
		_ = inW.Close()
		_ = outR.Close()
		client.once.Do(func() {
			select {
			case <-done:
			case <-time.After(3 * time.Second):
				t.Error("stdio server leaked after close")
			}
		})
	})
	return client
}

func (c *stdioClient) send(t *testing.T, raw string) {
	t.Helper()
	if _, err := io.WriteString(c.in, raw+"\n"); err != nil {
		t.Fatal(err)
	}
}

func (c *stdioClient) call(t *testing.T, raw string) rpcResponse {
	t.Helper()
	c.send(t, raw)
	line := make(chan string, 1)
	go func() { text, _ := c.reader.ReadString('\n'); line <- text }()
	select {
	case text := <-line:
		return decodeResponse(t, text)
	case <-time.After(5 * time.Second):
		_ = c.out.Close()
		t.Fatal("stdio response timeout")
		return rpcResponse{}
	}
}

func TestSmallDefaultSurface(t *testing.T) {
	names, err := selectedTools("")
	if err != nil || !reflect.DeepEqual(names, []string{"tasks", "import", "update", "status", "parse"}) {
		t.Fatalf("names=%v err=%v", names, err)
	}
	for _, name := range []string{"heavy", "run", "swarm", "commit", "warmup"} {
		if _, err := selectedTools(name); err == nil {
			t.Fatalf("accepted %s", name)
		}
	}
}

func TestStatelessStdioAndRawArguments(t *testing.T) {
	root := testProject(t)
	server, err := NewServer("test", "core", root)
	if err != nil {
		t.Fatal(err)
	}
	client := startStdio(t, server)
	// No initialize or initialized notification precedes these calls.
	response := client.call(t, rpcRequest("server/discover", ""))
	if response.Error != nil || !strings.Contains(string(response.Result), ProtocolVersion) {
		t.Fatalf("discover=%+v", response)
	}
	response = client.call(t, rpcRequest("tools/list", ""))
	if response.Error != nil || !strings.Contains(string(response.Result), `"resultType":"complete"`) {
		t.Fatalf("list=%s error=%+v", response.Result, response.Error)
	}
	for _, raw := range []string{
		`{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}`,
		`{"jsonrpc":"2.0","id":3,"method":"tools/list"}`,
		`{"jsonrpc":"2.0","id":4,"method":"tools/list","params":null}`,
	} {
		missing := client.call(t, raw)
		if missing.Error == nil {
			t.Fatalf("prior request state authorized missing metadata: %s", raw)
		}
	}
	response = client.call(t, toolRequest("scud_import", `{"document":{"schema_version":999,"schema_version":1,"phases":[]}}`))
	if response.Error == nil {
		t.Fatal("raw duplicate key was accepted")
	}
	response = client.call(t, toolRequest("scud_import", `{"document":{"schema_version":1,"phases":[{"tag":"main","name":"Main","id_format":"sequential","tasks":[{"id":"1","title":"Exact integer","complexity":9007199254740993,"status":"pending","priority":"medium"}]}]}}`))
	if response.Error != nil || !strings.Contains(string(response.Result), `"complexity":9007199254740993`) {
		t.Fatalf("large integer changed: %s error=%+v", response.Result, response.Error)
	}
	service, _ := taskapi.Open(root)
	snapshot, err := service.Snapshot(t.Context())
	if err != nil || int64(snapshot.Phases[0].Tasks[0].Complexity) != 9007199254740993 {
		t.Fatalf("persisted wrong number: err=%v", err)
	}
	response = client.call(t, toolRequest("scud_update", `{"id":"1","patch":{"Status":"pending","status":"done"}}`))
	if response.Error == nil {
		t.Fatal("case alias accepted")
	}
	response = client.call(t, toolRequest("scud_status", `{}`))
	if response.Error != nil {
		t.Fatalf("invalid call poisoned stateless next call: %+v", response.Error)
	}
	response = client.call(t, rpcRequest("initialize", ""))
	if response.Error == nil {
		t.Fatal("legacy handshake still enabled")
	}
}

func TestStdioEOFCancelsActiveOperation(t *testing.T) {
	server, _ := NewServer("test", "core", testProject(t))
	started, cancelled := make(chan struct{}), make(chan struct{})
	server.AddTool(&sdk.Tool{Name: "wait", InputSchema: map[string]any{"type": "object"}}, func(ctx context.Context, _ *sdk.CallToolRequest) (*sdk.CallToolResult, error) {
		close(started)
		<-ctx.Done()
		close(cancelled)
		return nil, ctx.Err()
	})
	client := startStdio(t, server)
	client.send(t, toolRequest("wait", `{}`))
	select {
	case <-started:
	case <-time.After(3 * time.Second):
		t.Fatal("handler never started")
	}
	_ = client.in.Close()
	select {
	case <-cancelled:
	case <-time.After(3 * time.Second):
		t.Fatal("stdio EOF did not cancel handler")
	}
	client.once.Do(func() {
		select {
		case err := <-client.done:
			if err != nil && !errors.Is(err, io.EOF) {
				t.Logf("EOF shutdown: %v", err)
			}
		case <-time.After(3 * time.Second):
			t.Fatal("stdio did not finish after cancellation")
		}
	})
}

func post(t *testing.T, url, method, name, body string, mutate func(*http.Request)) (*http.Response, string) {
	t.Helper()
	req, err := http.NewRequestWithContext(t.Context(), "POST", url+"/mcp", strings.NewReader(body))
	if err != nil {
		t.Fatal(err)
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Accept", "application/json, text/event-stream")
	req.Header.Set("MCP-Protocol-Version", ProtocolVersion)
	req.Header.Set("Mcp-Method", method)
	if name != "" {
		req.Header.Set("Mcp-Name", name)
	}
	if mutate != nil {
		mutate(req)
	}
	response, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	b, err := io.ReadAll(response.Body)
	if err != nil {
		t.Fatal(err)
	}
	return response, string(b)
}

func TestHTTPIsStatelessAndValidatesMetadata(t *testing.T) {
	root := testProject(t)
	handler, err := HTTPHandler("test", "core", root)
	if err != nil {
		t.Fatal(err)
	}
	server := httptest.NewServer(handler)
	defer server.Close()
	for _, method := range []string{"server/discover", "tools/list"} {
		response, body := post(t, server.URL, method, "", rpcRequest(method, ""), nil)
		if response.StatusCode != 200 || response.Header.Get("Mcp-Session-Id") != "" || decodeResponse(t, body).Error != nil {
			t.Fatalf("%s: %d %s", method, response.StatusCode, body)
		}
	}
	for _, mutate := range []func(*http.Request){
		func(r *http.Request) { r.Header.Del("Mcp-Method") },
		func(r *http.Request) { r.Header.Set("MCP-Protocol-Version", "2025-11-25") },
		func(r *http.Request) { r.Header.Set("Mcp-Method", "tools/call") },
	} {
		response, _ := post(t, server.URL, "tools/list", "", rpcRequest("tools/list", ""), mutate)
		if response.StatusCode != 400 {
			t.Fatalf("mismatched metadata status=%d", response.StatusCode)
		}
	}
	response, body := post(t, server.URL, "tools/call", "scud_import", toolRequest("scud_import", `{"document":{"schema_version":999,"schema_version":1,"phases":[]}}`), nil)
	if response.StatusCode != 400 || decodeResponse(t, body).Error == nil {
		t.Fatalf("HTTP duplicate accepted: %d %s", response.StatusCode, body)
	}
	largeInteger := `{"document":{"schema_version":1,"phases":[{"tag":"http","name":"HTTP","id_format":"sequential","tasks":[{"id":"1","title":"Exact integer","complexity":9007199254740993,"status":"pending","priority":"medium"}]}]}}`
	response, body = post(t, server.URL, "tools/call", "scud_import", toolRequest("scud_import", largeInteger), nil)
	if response.StatusCode != 200 || !strings.Contains(body, `"complexity":9007199254740993`) {
		t.Fatalf("HTTP integer changed: %d %s", response.StatusCode, body)
	}
	service, _ := taskapi.Open(root)
	state, err := service.Snapshot(t.Context())
	if err != nil || int64(state.Phases[0].Tasks[0].Complexity) != 9007199254740993 {
		t.Fatalf("HTTP persisted wrong integer: %v", err)
	}
	for _, mutate := range []func(*http.Request){
		func(r *http.Request) { r.Header.Del("Mcp-Name") },
		func(r *http.Request) { r.Header.Set("Mcp-Name", "scud_status") },
	} {
		response, _ = post(t, server.URL, "tools/call", "scud_tasks", toolRequest("scud_tasks", `{}`), mutate)
		if response.StatusCode != 400 {
			t.Fatalf("mismatched tool name status=%d", response.StatusCode)
		}
	}
	response, _ = post(t, server.URL, "tools/list", "", strings.ReplaceAll(rpcRequest("tools/list", ""), ProtocolVersion, "2025-11-25"), func(r *http.Request) { r.Header.Set("MCP-Protocol-Version", "2025-11-25") })
	if response.StatusCode != 400 {
		t.Fatalf("legacy protocol accepted: %d", response.StatusCode)
	}
	withoutCapabilities := strings.Replace(rpcRequest("tools/list", ""), `,"io.modelcontextprotocol/clientCapabilities":{}`, "", 1)
	response, _ = post(t, server.URL, "tools/list", "", withoutCapabilities, nil)
	if response.StatusCode != 400 {
		t.Fatalf("request without capabilities accepted: %d", response.StatusCode)
	}
	response, _ = post(t, server.URL, "tools/list", "", rpcRequest("tools/list", ""), func(r *http.Request) { r.Header.Set("Origin", "https://attacker.example") })
	if response.StatusCode != 403 {
		t.Fatalf("cross-origin status=%d", response.StatusCode)
	}
	for _, method := range []string{"GET", "DELETE"} {
		req, _ := http.NewRequestWithContext(t.Context(), method, server.URL+"/mcp", nil)
		response, err := http.DefaultClient.Do(req)
		if err != nil {
			t.Fatal(err)
		}
		_ = response.Body.Close()
		if response.StatusCode != http.StatusMethodNotAllowed {
			t.Fatalf("legacy %s status=%d", method, response.StatusCode)
		}
	}
}

func TestMCPFilteredExportIncludesRequiredPhases(t *testing.T) {
	root := testProject(t)
	server, _ := NewServer("test", "core", root)
	client := startStdio(t, server)
	document := `{"document":{"schema_version":1,"phases":[{"tag":"base","name":"Base","id_format":"sequential","tasks":[{"id":"1","title":"Base","status":"pending","priority":"medium"}]},{"tag":"app","name":"App","id_format":"sequential","tasks":[{"id":"1","title":"App","status":"pending","priority":"medium","dependencies":["base:1"]}]}]}}`
	response := client.call(t, toolRequest("scud_import", document))
	if response.Error != nil {
		t.Fatal(response.Error)
	}
	response = client.call(t, toolRequest("scud_tasks", `{"tag":"app"}`))
	var output struct {
		StructuredContent json.RawMessage `json:"structuredContent"`
	}
	if err := json.Unmarshal(response.Result, &output); err != nil {
		t.Fatal(err)
	}
	d, err := taskapi.Decode[taskapi.Document](strings.NewReader(string(output.StructuredContent)))
	if err != nil || len(d.Phases) != 2 {
		t.Fatalf("incomplete export: %s err=%v", response.Result, err)
	}
	if _, err := taskapi.ValidateDocument(d); err != nil {
		t.Fatalf("filtered MCP export is not standalone: %v", err)
	}
}

func TestHTTPDisconnectCancelsOnlyItsRequest(t *testing.T) {
	server, _ := NewServer("test", "core", testProject(t))
	started, cancelled := make(chan struct{}), make(chan struct{})
	server.AddTool(&sdk.Tool{Name: "wait", InputSchema: map[string]any{"type": "object"}}, func(ctx context.Context, _ *sdk.CallToolRequest) (*sdk.CallToolResult, error) {
		close(started)
		<-ctx.Done()
		close(cancelled)
		return nil, ctx.Err()
	})
	httpServer := httptest.NewServer(httpHandler(server))
	defer httpServer.Close()
	ctx, cancel := context.WithCancel(t.Context())
	defer cancel()
	req, _ := http.NewRequestWithContext(ctx, "POST", httpServer.URL+"/mcp", strings.NewReader(toolRequest("wait", `{}`)))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Accept", "application/json, text/event-stream")
	req.Header.Set("MCP-Protocol-Version", ProtocolVersion)
	req.Header.Set("Mcp-Method", "tools/call")
	req.Header.Set("Mcp-Name", "wait")
	done := make(chan struct{})
	go func() {
		defer close(done)
		response, err := http.DefaultClient.Do(req)
		if err == nil {
			_ = response.Body.Close()
		}
	}()
	select {
	case <-started:
	case <-time.After(3 * time.Second):
		t.Fatal("handler did not start")
	}
	cancel()
	select {
	case <-cancelled:
	case <-time.After(3 * time.Second):
		t.Fatal("HTTP disconnect did not cancel handler")
	}
	select {
	case <-done:
	case <-time.After(3 * time.Second):
		t.Fatal("HTTP client leaked")
	}
	response, body := post(t, httpServer.URL, "tools/list", "", rpcRequest("tools/list", ""), nil)
	if response.StatusCode != 200 || decodeResponse(t, body).Error != nil {
		t.Fatalf("disconnect poisoned next request: %s", body)
	}
}
