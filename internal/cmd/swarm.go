package cmd

import (
	"context"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/swarm"
	"github.com/spf13/cobra"
)

func NewSwarmCmd() *cobra.Command {
	var tag string
	var dryRun, noValidate bool

	cmd := &cobra.Command{
		Use:   "swarm",
		Short: "Execute tasks via parallel waves with backpressure",
		RunE: func(cmd *cobra.Command, args []string) error {
			store, err := getStore()
			if err != nil {
				return err
			}
			tag, err := store.ResolveTag(tag)
			if err != nil {
				return err
			}
			cfg, err := config.Load(store.ScudDir())
			if err != nil {
				return err
			}
			return swarm.Run(context.Background(), cfg, store, swarm.RunOpts{
				Tag:        tag,
				DryRun:     dryRun,
				NoValidate: noValidate,
			})
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	cmd.Flags().BoolVar(&dryRun, "dry-run", false, "Show plan without executing")
	cmd.Flags().BoolVar(&noValidate, "no-validate", false, "Skip backpressure validation")
	return cmd
}
