//go:build linux || darwin

package sbintegration

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"testing"
	"time"
)

func TestEarlyExitCleansDescendants(t *testing.T) {
	for _, closed := range []bool{false, true} {
		t.Run(map[bool]string{false: "inherited-streams", true: "closed-streams"}[closed], func(t *testing.T) {
			redirect := ""
			if closed {
				redirect = " </dev/null >/dev/null 2>&1"
			}
			c := fixture(t, "sleep 30"+redirect+" &\necho $! > child.pid\nprintf launcher-output\nprintf launcher-diagnostic >&2\nexit 0\n")
			pid := 0
			defer func() {
				if pid == 0 {
					b, _ := os.ReadFile(filepath.Join(c.Root, "child.pid"))
					pid, _ = strconv.Atoi(strings.TrimSpace(string(b)))
				}
				if pid > 0 && childRunning(pid) {
					_ = syscall.Kill(pid, syscall.SIGKILL)
				}
			}()
			ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
			defer cancel()
			r, err := c.Run(ctx, "check")
			if err != nil || !r.Success || r.ExitCode != 0 || r.Stdout != "launcher-output" || r.Stderr != "launcher-diagnostic" {
				t.Fatalf("result=%+v err=%v", r, err)
			}
			b, err := os.ReadFile(filepath.Join(c.Root, "child.pid"))
			if err != nil {
				t.Fatal(err)
			}
			pid, err = strconv.Atoi(strings.TrimSpace(string(b)))
			if err != nil || pid <= 0 {
				t.Fatalf("child PID %q: %v", b, err)
			}
			deadline := time.Now().Add(time.Second)
			for childRunning(pid) && time.Now().Before(deadline) {
				time.Sleep(10 * time.Millisecond)
			}
			if childRunning(pid) {
				t.Fatalf("sleeping descendant %d survived launcher exit", pid)
			}
		})
	}
}

func childRunning(pid int) bool {
	if err := syscall.Kill(pid, 0); err == syscall.ESRCH {
		return false
	}
	// Orphans may remain zombies until the platform's init reaps them. A
	// zombie cannot execute or retain descriptors and is not a live leak.
	b, err := exec.Command("ps", "-o", "stat=", "-p", strconv.Itoa(pid)).Output()
	return err == nil && strings.TrimSpace(string(b)) != "" && !strings.HasPrefix(strings.TrimSpace(string(b)), "Z")
}

func TestCancelDescendantGroup(t *testing.T) {
	c := fixture(t, "sleep 20 &\necho $! > child.pid\nwait\n")
	defer func() {
		b, _ := os.ReadFile(filepath.Join(c.Root, "child.pid"))
		pid, _ := strconv.Atoi(strings.TrimSpace(string(b)))
		if pid > 0 && childRunning(pid) {
			_ = syscall.Kill(pid, syscall.SIGKILL)
		}
	}()
	ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
	defer cancel()
	started := time.Now()
	r, err := c.Run(ctx, "check")
	if err == nil || r.Success || time.Since(started) > 2*time.Second {
		t.Fatalf("command group did not cancel promptly: %+v err=%v", r, err)
	}
	b, err := os.ReadFile(filepath.Join(c.Root, "child.pid"))
	if err != nil {
		t.Fatal(err)
	}
	pid, err := strconv.Atoi(strings.TrimSpace(string(b)))
	if err != nil || pid <= 0 {
		t.Fatalf("child PID %q: %v", b, err)
	}
	deadline := time.Now().Add(time.Second)
	for childRunning(pid) && time.Now().Before(deadline) {
		time.Sleep(10 * time.Millisecond)
	}
	if childRunning(pid) {
		t.Fatalf("canceled descendant %d still running", pid)
	}
}

func TestReportDirectoryCannotEscapeProject(t *testing.T) {
	c := fixture(t, "exit 0\n")
	if err := os.Symlink(t.TempDir(), filepath.Join(c.Root, ".sb")); err != nil {
		t.Fatal(err)
	}
	if _, err := c.State(); err == nil {
		t.Fatal("symlinked sb state must not be read")
	}
}
