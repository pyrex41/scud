package cmd

import (
	"context"
	"fmt"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/generate"
	"github.com/spf13/cobra"
)

func NewGenerateCmd() *cobra.Command {
	var tag string
	var numTasks int
	var noExpand, noCheckDeps bool

	cmd := &cobra.Command{
		Use:   "generate <file>",
		Short: "Generate tasks from PRD (full pipeline)",
		Args:  cobra.ExactArgs(1),
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
			return generate.Generate(context.Background(), cfg, store, args[0], tag, numTasks, generate.GenerateOpts{
				NoExpand:    noExpand,
				NoCheckDeps: noCheckDeps,
			})
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag (required)")
	cmd.Flags().IntVarP(&numTasks, "num", "n", 10, "Target number of tasks")
	cmd.Flags().BoolVar(&noExpand, "no-expand", false, "Skip task expansion")
	cmd.Flags().BoolVar(&noCheckDeps, "no-check-deps", false, "Skip dependency check")
	return cmd
}

func NewParseCmd() *cobra.Command {
	var tag string
	var numTasks int

	cmd := &cobra.Command{
		Use:   "parse <file>",
		Short: "Parse PRD into tasks",
		Args:  cobra.ExactArgs(1),
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
			return generate.ParsePRD(context.Background(), cfg, store, args[0], tag, numTasks)
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag (required)")
	cmd.Flags().IntVarP(&numTasks, "num", "n", 10, "Target number of tasks")
	return cmd
}

func NewExpandCmd() *cobra.Command {
	var tag, taskID string

	cmd := &cobra.Command{
		Use:   "expand",
		Short: "Expand complex tasks into subtasks",
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
			return generate.Expand(context.Background(), cfg, store, tag, taskID)
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	cmd.Flags().StringVarP(&taskID, "id", "i", "", "Specific task ID to expand")
	return cmd
}

func NewCheckDepsCmd() *cobra.Command {
	var tag string

	cmd := &cobra.Command{
		Use:   "check-deps",
		Short: "Validate task dependencies",
		RunE: func(cmd *cobra.Command, args []string) error {
			store, err := getStore()
			if err != nil {
				return err
			}
			tag, err := store.ResolveTag(tag)
			if err != nil {
				return err
			}
			phases, err := store.LoadPhases()
			if err != nil {
				return err
			}
			result := generate.CheckDeps(phases, tag)
			fmt.Println(generate.FormatCheckResult(result))
			return nil
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	return cmd
}
