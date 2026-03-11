package main

import (
	"fmt"
	"os"

	"github.com/reuben/scud/internal/cmd"
	"github.com/spf13/cobra"
)

var version = "dev"

func main() {
	root := &cobra.Command{
		Use:     "scud",
		Short:   "DAG-based task management with AI generation and swarm execution",
		Version: version,
	}

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
	)

	if err := root.Execute(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
