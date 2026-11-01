package cmd

import (
	"fmt"
	"os"
	"time"

	"github.com/reuben/scud/internal/sbintegration"
	"github.com/spf13/cobra"
)

// NewSBCmd exposes optional verification independently of task workflow state.
func NewSBCmd() *cobra.Command {
	root := &cobra.Command{Use: "sb", Short: "Optional sb verification and context (never executes task workers)"}
	var jsonOutput bool
	var timeout time.Duration
	root.PersistentFlags().BoolVar(&jsonOutput, "json", false, "Write complete JSON to stdout")
	root.PersistentFlags().DurationVar(&timeout, "timeout", 5*time.Minute, "Maximum sb command duration")
	client := func() (sbintegration.Client, error) {
		cwd, err := os.Getwd()
		if err != nil {
			return sbintegration.Client{}, err
		}
		project, err := sbintegration.FindRoot(cwd)
		return sbintegration.Client{Root: project, Timeout: timeout}, err
	}
	root.AddCommand(&cobra.Command{
		Use: "status", Short: "Report sb configuration/availability without running checks", Args: cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			c, err := client()
			if err != nil {
				return err
			}
			state, err := c.State()
			if err != nil {
				return err
			}
			if jsonOutput {
				return sbintegration.WriteJSON(cmd.OutOrStdout(), state)
			}
			_, err = fmt.Fprintf(cmd.OutOrStdout(), "sb configured: %t\nbinary available: %t\ndischarge report: %s\n", state.Enabled, state.Available, state.ReportStatus)
			return err
		},
	})
	for _, operation := range []struct{ name, description string }{
		{"check", "Run sb gates; command success is not invariant discharge"},
		{"context", "Read sb context as supplementary diagnostics"},
		{"derive", "Run sb derive explicitly"},
		{"audit", "Render sb's audit report"},
		{"gen", "Run sb guard generation (not a spec-only validation guarantee)"},
	} {
		root.AddCommand(&cobra.Command{
			Use: operation.name, Short: operation.description, Args: cobra.NoArgs,
			RunE: func(cmd *cobra.Command, _ []string) error {
				if timeout <= 0 {
					return fmt.Errorf("--timeout must be positive")
				}
				c, err := client()
				if err != nil {
					return err
				}
				r, runErr := c.Run(cmd.Context(), cmd.Name())
				if jsonOutput {
					if err := sbintegration.WriteJSON(cmd.OutOrStdout(), r); err != nil {
						return err
					}
				} else {
					fmt.Fprint(cmd.OutOrStdout(), r.Stdout)
					fmt.Fprint(cmd.ErrOrStderr(), r.Stderr)
				}
				return runErr
			},
		})
	}
	return root
}
