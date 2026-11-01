package cmd

import (
	"reflect"
	"testing"
	"time"

	"github.com/reuben/scud/internal/config"
	agentexec "github.com/reuben/scud/pkg/executor"
	"github.com/spf13/cobra"
)

func TestConfiguredExecutorDefaultsToLegacy(t *testing.T) {
	runner, err := configuredExecutor(config.Default(), "", "", "", t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if _, ok := runner.(agentexec.LegacyRho); !ok {
		t.Fatalf("runner = %T, want executor.LegacyRho", runner)
	}
}

func TestConfiguredRhoV1BuildsBoundedWorkspaceGrant(t *testing.T) {
	cfg := config.Default()
	cfg.Executor.AllowedTools = []string{"read", "edit"}
	workspace := t.TempDir()
	now := time.Date(2026, 7, 31, 12, 0, 0, 0, time.UTC)
	runner := configuredRhoV1(cfg, "anthropic", "claude-sonnet-4-5", workspace, now)

	if runner.Grant.ExpiresAt != "2026-07-31T13:00:00Z" {
		t.Fatalf("expiry = %q", runner.Grant.ExpiresAt)
	}
	if !reflect.DeepEqual(runner.Grant.ReadRoots, []string{workspace}) || !reflect.DeepEqual(runner.Grant.WriteRoots, []string{workspace}) {
		t.Fatalf("unexpected roots: %#v", runner.Grant)
	}
	if runner.Grant.Providers[0] != "anthropic" || runner.Grant.Models[0] != "claude-sonnet-4-5" {
		t.Fatalf("explicit provider/model not first in grant: %#v", runner.Grant)
	}
	if !reflect.DeepEqual(runner.Grant.Tools, []string{"read", "edit"}) {
		t.Fatalf("tools = %#v", runner.Grant.Tools)
	}
}

func TestConfiguredExecutorRejectsUnknownKind(t *testing.T) {
	if _, err := configuredExecutor(config.Default(), "mystery", "", "", t.TempDir()); err == nil {
		t.Fatal("expected unknown executor error")
	}
}

func TestRunAndSwarmExposeExecutorFlags(t *testing.T) {
	for name, command := range map[string]*cobra.Command{"run": NewRunCmd(), "swarm": NewSwarmCmd()} {
		for _, flag := range []string{"executor", "provider"} {
			if command.Flags().Lookup(flag) == nil {
				t.Errorf("%s missing --%s", name, flag)
			}
		}
	}
}
