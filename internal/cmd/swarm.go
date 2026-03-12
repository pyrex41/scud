package cmd

import (
	"context"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/swarm"
	"github.com/spf13/cobra"
)

func NewSwarmCmd() *cobra.Command {
	var tag string
	var dryRun, noValidate, noRepair, allTags bool
	var roundSize int

	cmd := &cobra.Command{
		Use:   "swarm",
		Short: "Execute tasks via parallel waves with backpressure",
		RunE: func(cmd *cobra.Command, args []string) error {
			store, err := getStore()
			if err != nil {
				return err
			}
			cfg, err := config.Load(store.ScudDir())
			if err != nil {
				return err
			}

			opts := swarm.RunOpts{
				DryRun:     dryRun,
				NoValidate: noValidate,
				NoRepair:   noRepair,
				AllTags:    allTags,
				RoundSize:  roundSize,
			}

			if allTags {
				return swarm.Run(context.Background(), cfg, store, opts)
			}

			resolved, err := store.ResolveTag(tag)
			if err != nil {
				return err
			}
			opts.Tag = resolved
			return swarm.Run(context.Background(), cfg, store, opts)
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	cmd.Flags().BoolVar(&dryRun, "dry-run", false, "Show plan without executing")
	cmd.Flags().BoolVar(&noValidate, "no-validate", false, "Skip backpressure validation")
	cmd.Flags().BoolVar(&noRepair, "no-repair", false, "Skip ralph recovery loop on failure")
	cmd.Flags().BoolVar(&allTags, "all-tags", false, "Iterate all phases/tags in order")
	cmd.Flags().IntVarP(&roundSize, "round-size", "n", 0, "Round size override (0 = use config)")
	return cmd
}
