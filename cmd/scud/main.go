package main

import (
	"fmt"
	"os"

	"github.com/reuben/scud/internal/cmd"
	"github.com/spf13/cobra"
)

var version = "dev"

func main() {
	var projectDir string

	root := &cobra.Command{
		Use:     "scud",
		Short:   "DAG-based task management with AI generation and swarm execution",
		Version: version,
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
		cmd.NewListCmd(),
		cmd.NewShowCmd(),
		cmd.NewNextCmd(),
		cmd.NewSetStatusCmd(),
		cmd.NewStatsCmd(),
		cmd.NewWavesCmd(),
		cmd.NewTagsCmd(),
		cmd.NewGenerateCmd(),
		cmd.NewParseCmd(),
		cmd.NewExpandCmd(),
		cmd.NewCheckDepsCmd(),
		cmd.NewSwarmCmd(),
		cmd.NewDoctorCmd(),
		cmd.NewHeavyCmd(),
		cmd.NewCreateCmd(),
		cmd.NewAssignCmd(),
		cmd.NewWhoisCmd(),
		cmd.NewLogCmd(),
		cmd.NewNextBatchCmd(),
		cmd.NewWarmupCmd(),
		cmd.NewCommitCmd(),
		cmd.NewCleanCmd(),
		cmd.NewMermaidCmd(),
		cmd.NewConvertCmd(),
		cmd.NewRestartCmd(),
		cmd.NewAttractorCmd(),
		cmd.NewTranscriptCmd(),
		cmd.NewAnalyzeComplexityCmd(),
		cmd.NewReanalyzeDepsCmd(),
		cmd.NewRunCmd(),
		cmd.NewTestCmd(),
	)

	if err := root.Execute(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
