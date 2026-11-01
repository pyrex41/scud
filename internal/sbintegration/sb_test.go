package sbintegration

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
	"time"
)

func fixture(t *testing.T, script string) Client {
	t.Helper()
	if runtime.GOOS == "windows" {
		t.Skip("POSIX executable fixture; native sb uses exec.Command on Windows")
	}
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "sb.toml"), []byte("not parsed by scud"), 0600); err != nil {
		t.Fatal(err)
	}
	bin := filepath.Join(t.TempDir(), "sb")
	if err := os.WriteFile(bin, []byte("#!/bin/sh\n"+script), 0700); err != nil {
		t.Fatal(err)
	}
	return Client{Root: root, Executable: bin}
}

func TestAbsentIsOptional(t *testing.T) {
	c := Client{Root: t.TempDir(), Executable: "/missing/sb"}
	s, err := c.State()
	if err != nil || s.Enabled || s.Available || s.ReportStatus != "absent" {
		t.Fatalf("state=%+v err=%v", s, err)
	}
	if _, err := c.Run(context.Background(), "check"); err == nil {
		t.Fatal("explicit sb check without configuration must fail")
	}
}

func TestGateSuccessWithoutReport(t *testing.T) {
	c := fixture(t, "test \"$1\" = gates || exit 9\nprintf 'gate passed\\n'\n")
	r, err := c.Run(context.Background(), "check")
	if err != nil || !r.Success || r.ExitCode != 0 || r.ReportPresent || r.ReportStatus != "absent" {
		t.Fatalf("result=%+v err=%v", r, err)
	}
}

func TestExistingReportNeverBecomesVerified(t *testing.T) {
	c := fixture(t, "echo compiler-error >&2\nexit 3\n")
	if err := os.Mkdir(filepath.Join(c.Root, ".sb"), 0700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(c.Root, ".sb", "discharge_report.json"), []byte(`{"schema_version":1}`), 0600); err != nil {
		t.Fatal(err)
	}
	r, err := c.Run(context.Background(), "check")
	if err == nil || r.Success || r.ExitCode != 3 || !strings.Contains(r.Stderr, "compiler-error") || r.ReportChanged || r.ReportStatus != "unverified" {
		t.Fatalf("result=%+v err=%v", r, err)
	}
}

func TestNewReportDoesNotEstablishCoverage(t *testing.T) {
	c := fixture(t, "mkdir -p .sb\nprintf '{}' > .sb/discharge_report.json\n")
	r, err := c.Run(context.Background(), "derive")
	if err != nil || !r.ReportChanged || !r.ReportPresent || r.ReportStatus != "unverified" {
		t.Fatalf("result=%+v err=%v", r, err)
	}
}

func TestNoAgentOperations(t *testing.T) {
	c := fixture(t, "exit 0\n")
	for _, operation := range []string{"loop", "run", "gates; rm anything", ""} {
		if _, err := c.Run(context.Background(), operation); err == nil {
			t.Fatalf("accepted %q", operation)
		}
	}
}

func TestCancellationAndOutputBound(t *testing.T) {
	c := fixture(t, "exec sleep 20\n")
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	started := time.Now()
	r, err := c.Run(ctx, "check")
	if err == nil || r.Success || time.Since(started) > 3*time.Second {
		t.Fatalf("result=%+v err=%v", r, err)
	}
	buffer := &boundedBuffer{}
	input := strings.Repeat("x", maxOutput+100)
	n, err := buffer.Write([]byte(input))
	if err != nil || n != len(input) || len(buffer.String()) != maxOutput || !buffer.truncated {
		t.Fatal("output bound incorrect")
	}
	var out strings.Builder
	if err := WriteJSON(&out, Result{Stdout: buffer.String(), Truncated: true}); err != nil || !json.Valid([]byte(out.String())) {
		t.Fatal("bounded output must still serialize as valid JSON")
	}
}

func TestMissingBinaryIsExplicitError(t *testing.T) {
	c := fixture(t, "exit 0\n")
	c.Executable = "/missing/sb"
	if _, err := c.Run(context.Background(), "check"); err == nil || !strings.Contains(err.Error(), "unavailable") {
		t.Fatalf("err=%v", err)
	}
}
