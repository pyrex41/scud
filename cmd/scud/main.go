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
		Use:           "scud",
		Short:         "PRDs to tasks; safe task updates and live status for scripts",
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
