//go:build linux || darwin

package mcp

import (
	"errors"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"testing"
	"time"
)

// This exercises the complete SDK EOF -> tool context -> sb subprocess path,
// not merely a synthetic handler observing context cancellation.
func TestStdioDisconnectStopsRealSBInvocation(t *testing.T) {
	root := testProject(t)
	if err := os.WriteFile(filepath.Join(root, "sb.toml"), []byte("# fixture"), 0600); err != nil {
		t.Fatal(err)
	}
	bin := t.TempDir()
	if err := os.WriteFile(filepath.Join(bin, "sb"), []byte("#!/bin/sh\necho $$ > child.pid\nexec /bin/sleep 30\n"), 0700); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", bin+string(os.PathListSeparator)+os.Getenv("PATH"))
	server, _ := NewServer("test", "full", root)
	client := startStdio(t, server)
	client.send(t, toolRequest("scud_sb", `{"operation":"check"}`))
	var pid int
	deadline := time.Now().Add(3 * time.Second)
	for pid == 0 && time.Now().Before(deadline) {
		if b, err := os.ReadFile(filepath.Join(root, "child.pid")); err == nil {
			pid, _ = strconv.Atoi(strings.TrimSpace(string(b)))
		}
		if pid == 0 {
			time.Sleep(5 * time.Millisecond)
		}
	}
	if pid == 0 {
		t.Fatal("sb invocation did not start")
	}
	_ = client.in.Close()
	client.once.Do(func() {
		select {
		case <-client.done:
		case <-time.After(3 * time.Second):
			t.Fatal("MCP retained disconnected sb invocation")
		}
	})
	if err := syscall.Kill(pid, 0); !errors.Is(err, syscall.ESRCH) {
		t.Fatalf("owned sb launcher still exists after server exit (pid=%d): %v", pid, err)
	}
}
