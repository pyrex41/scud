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
	root.Long = `Wrap optional public sb commands; ordinary task workflows do not require sb.
These commands never run task workers or mark tasks complete. A successful check
is not invariant discharge or proof; report bytes remain unverified. derive,
gen and check may change generated project files. --timeout bounds execution;
--json preserves command stdout/stderr and diagnostics, including failures.`
	root.Example = `  scud sb status --json
  scud sb check --json
  scud sb context`
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
		Long: `Report whether sb is configured, whether its binary is available and the
discharge-report status without running checks. Report presence does not establish
freshness, proof strength or task completion. Use --json for structured output.`,
		Example: `  scud sb status
  scud sb status --json`,
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
			Long: operation.description + `. Invokes the optional sb binary in the project.
Requires sb configuration and an available binary. This operation does not update
task status, run task workers or certify invariant discharge. derive/gen/check
may change generated files. Output is bounded with explicit truncation markers;
--json preserves stdout/stderr and diagnostics even if the command fails.`,
			Example: "  scud sb " + operation.name + "\n  scud sb " + operation.name + " --json --timeout 2m",
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
