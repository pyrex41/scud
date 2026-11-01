package main

import (
	"context"
	"errors"
	"fmt"
	"os"
	"os/signal"
	"syscall"

	"github.com/reuben/scud/internal/cmd"
	"github.com/reuben/scud/internal/taskapi"
	"github.com/spf13/cobra"
)

var version = "dev"

func main() {
	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()
	if err := newRoot().ExecuteContext(ctx); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(exitCode(err))
	}
}

func exitCode(err error) int {
	if _, ok := errors.AsType[*taskapi.RevisionConflictError](err); ok {
		return 3
	}
	if errors.Is(err, context.Canceled) {
		return 130
	}
	if errors.Is(err, context.DeadlineExceeded) {
		return 124
	}
	return 1
}

func newRoot() *cobra.Command {
	var projectDir string

	root := &cobra.Command{
		Use:   "scud",
		Short: "PRDs to tasks; safe task updates and live status for scripts",
		Long: `Scud v3 manages dependency-aware tasks; it does not run coding agents.

Start with init, choose main/fast with models, then parse a PRD into a proposal.
Review and validate the proposal before import. Use next and show to inspect work,
update to report progress, and status --watch for polling snapshots.

Task commands output JSON by default; models and sb offer --json explicitly.
next returns only the first ready reference or null; it never claims work.
Ordinary task reads and updates need neither model credentials nor optional sb.

Exit codes: 0 success (including no ready task), 1 error, 3 revision conflict,
124 deadline exceeded, 130 cancellation. Errors are written to stderr.

See README.md for the workflow and docs/CLI_REFERENCE.md for all command help.`,
		Example: `  scud init
  scud models list
  scud parse product.md > proposal.json
  scud validate --input proposal.json
  scud import proposal.json
  scud next
  scud show main:1
  scud update main:1 --status in-progress
  scud status --watch`,
		Version:       version,
		SilenceUsage:  true,
		SilenceErrors: true,
		PersistentPreRunE: func(c *cobra.Command, args []string) error {
			if projectDir != "" {
				if err := os.Chdir(projectDir); err != nil {
					return fmt.Errorf("changing to project directory %q: %w", projectDir, err)
				}
			}
			return nil
		},
	}

	root.PersistentFlags().StringVarP(&projectDir, "project", "C", "", "Change to directory before running")

	root.AddCommand(
		cmd.NewInitCmd(),
		cmd.NewPRDCmd(),
		cmd.NewTaskImportCmd(),
		cmd.NewTaskListCmd(),
		cmd.NewTaskShowCmd(),
		cmd.NewTaskUpdateCmd(),
		cmd.NewTaskNextCmd(),
		cmd.NewTaskStatusCmd(),
		cmd.NewTaskValidateCmd(),
		cmd.NewModelsCmd(),
		cmd.NewSBCmd(),
		cmd.NewExperimentalCmd(),
		cmd.NewMCPServerCmd(),
	)
	return root
}
