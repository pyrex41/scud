package cmd

import (
	"context"
	"fmt"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/pkg/generate"
	"github.com/reuben/scud/pkg/llm"
	"github.com/spf13/cobra"
)

func NewAnalyzeComplexityCmd() *cobra.Command {
	var tag string
	var apply bool

	cmd := &cobra.Command{
		Use:   "analyze-complexity <id>",
		Short: "Analyze task complexity using LLM",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			taskID := args[0]

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

			phases, err := store.LoadPhases()
			if err != nil {
				return err
			}
			phase, ok := phases[tag]
			if !ok {
				return fmt.Errorf("tag '%s' not found", tag)
			}
			task := phase.FindTask(taskID)
			if task == nil {
				return fmt.Errorf("task '%s' not found in tag '%s'", taskID, tag)
			}

			client, err := llm.NewClient(cfg)
			if err != nil {
				return fmt.Errorf("creating LLM client: %w", err)
			}

			result, err := generate.AnalyzeComplexity(context.Background(), client, task)
			if err != nil {
				return err
			}

			fmt.Printf("Task %s: %s\n", task.ID, task.Title)
			fmt.Printf("Current complexity: %d\n", task.Complexity)
			fmt.Printf("Suggested complexity: %d\n", result.Complexity)
			fmt.Printf("Reasoning: %s\n", result.Reasoning)

			if apply {
				task.Complexity = result.Complexity
				task.AutoAssignAgent()
				task.SetUpdatedNow()
				if err := store.SavePhases(phases); err != nil {
					return fmt.Errorf("saving: %w", err)
				}
				fmt.Printf("\nApplied complexity %d to task %s\n", result.Complexity, task.ID)
			}

			return nil
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	cmd.Flags().BoolVar(&apply, "apply", false, "Apply the suggested complexity")
	return cmd
}
