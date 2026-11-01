//go:build linux || darwin

package main

import (
	"bufio"
	"bytes"
	"context"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"syscall"
	"testing"
	"time"
)

func TestMCPStdioHelper(t *testing.T) {
	if os.Getenv("SCUD_MCP_STDIO_TEST_HELPER") != "1" {
		return
	}
	os.Args = []string{"scud", "-C", os.Getenv("SCUD_MCP_STDIO_TEST_ROOT"), "mcp-server"}
	main()
	os.Exit(0)
}

func TestMCPStdioSIGTERMInterruptsIdleOpenInput(t *testing.T) {
	root := t.TempDir()
	if err := os.Mkdir(filepath.Join(root, ".scud"), 0700); err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(t.Context(), 10*time.Second)
	defer cancel()
	binary, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	cmd := exec.CommandContext(ctx, binary, "-test.run=^TestMCPStdioHelper$")
	cmd.Env = append(os.Environ(), "SCUD_MCP_STDIO_TEST_HELPER=1", "SCUD_MCP_STDIO_TEST_ROOT="+root)
	input, err := cmd.StdinPipe()
	if err != nil {
		t.Fatal(err)
	}
	defer input.Close()
	output, err := cmd.StdoutPipe()
	if err != nil {
		t.Fatal(err)
	}
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	if err := cmd.Start(); err != nil {
		t.Fatal(err)
	}
	_, _ = input.Write([]byte(`{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}` + "\n"))
	if _, err := bufio.NewReader(output).ReadString('\n'); err != nil {
		_ = cmd.Wait()
		t.Fatalf("MCP did not start: %v %s", err, stderr.String())
	}
	if err := cmd.Process.Signal(syscall.SIGTERM); err != nil {
		t.Fatal(err)
	}
	err = cmd.Wait()
	exit, ok := errors.AsType[*exec.ExitError](err)
	if ctx.Err() != nil || !ok || exit.ExitCode() != 130 {
		t.Fatalf("MCP blocked on idle stdin: err=%v timeout=%v stderr=%s", err, ctx.Err(), stderr.String())
	}
}
