package cmd

import (
	"context"
	"fmt"
	"os"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/pkg/generate"
	"github.com/reuben/scud/pkg/llm"
	"github.com/spf13/cobra"
)

func NewGenerateCmd() *cobra.Command {
	var tag string
	var numTasks int
	var noExpand, noCheckDeps, pipeline bool

	cmd := &cobra.Command{
		Use:   "generate <file>",
		Short: "Generate tasks from PRD (full pipeline)",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			store, err := getStore()
			if err != nil {
				return err
			}

			cfg, err := config.Load(store.ScudDir())
			if err != nil {
				return err
			}

			if pipeline {
				client, err := llm.NewClient(cfg)
				if err != nil {
					return fmt.Errorf("creating LLM client: %w", err)
				}
				dot, err := generate.GeneratePipeline(context.Background(), client, args[0])
				if err != nil {
					return err
				}
				fmt.Println(dot)
				return nil
			}

			tag, err := store.ResolveTag(tag)
			if err != nil {
				return err
			}

			return generate.Generate(context.Background(), cfg, store, args[0], tag, numTasks, generate.GenerateOpts{
				NoExpand:    noExpand,
				NoCheckDeps: noCheckDeps,
				PRDFile:     args[0],
			})
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag (required)")
	cmd.Flags().IntVarP(&numTasks, "num", "n", 10, "Target number of tasks")
	cmd.Flags().BoolVar(&noExpand, "no-expand", false, "Skip task expansion")
	cmd.Flags().BoolVar(&noCheckDeps, "no-check-deps", false, "Skip dependency check")
	cmd.Flags().BoolVar(&pipeline, "pipeline", false, "Generate attractor pipeline DOT instead of tasks")
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
	var prdFile string
	var fix bool

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

			// Structural check
			result := generate.CheckDeps(phases, tag)
			fmt.Println(generate.FormatCheckResult(result))

			// LLM-powered PRD validation
			if prdFile != "" {
				prdContent, err := os.ReadFile(prdFile)
				if err != nil {
					return fmt.Errorf("reading PRD: %w", err)
				}

				cfg, err := config.Load(store.ScudDir())
				if err != nil {
					return err
				}
				client, err := llm.NewClient(cfg)
				if err != nil {
					return fmt.Errorf("creating LLM client: %w", err)
				}

				fmt.Println("\n--- PRD Coverage Analysis ---")
				coverage, err := generate.CheckDepsWithPRD(context.Background(), client, store, tag, string(prdContent))
				if err != nil {
					return fmt.Errorf("PRD validation: %w", err)
				}
				fmt.Println(generate.FormatCoverageResult(coverage))

				if fix && coverage.CoverageScore < 100 {
					fmt.Println("\n--- Applying Fixes ---")
					if err := generate.FixPRDIssues(context.Background(), client, store, tag, string(prdContent), coverage); err != nil {
						return fmt.Errorf("fixing PRD issues: %w", err)
					}
					fmt.Println("Fixes applied successfully.")
				}
			}

			return nil
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	cmd.Flags().StringVar(&prdFile, "prd", "", "PRD file for LLM-powered coverage validation")
	cmd.Flags().BoolVar(&fix, "fix", false, "Auto-fix PRD coverage issues (requires --prd)")
	return cmd
}
