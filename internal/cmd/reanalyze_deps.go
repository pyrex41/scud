package cmd

import (
	"context"
	"fmt"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/generate"
	"github.com/reuben/scud/internal/llm"
	"github.com/spf13/cobra"
)

func NewReanalyzeDepsCmd() *cobra.Command {
	var tag string
	var apply bool

	cmd := &cobra.Command{
		Use:   "reanalyze-deps",
		Short: "Cross-phase dependency analysis using LLM",
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

			client, err := llm.NewClient(cfg)
			if err != nil {
				return fmt.Errorf("creating LLM client: %w", err)
			}

			suggestions, err := generate.ReanalyzeDeps(context.Background(), client, store, tag)
			if err != nil {
				return err
			}

			if len(suggestions) == 0 {
				fmt.Println("No dependency changes suggested.")
				return nil
			}

			for _, s := range suggestions {
				fmt.Printf("Task %s:\n", s.TaskID)
				if len(s.AddDeps) > 0 {
					fmt.Printf("  Add deps:    %v\n", s.AddDeps)
				}
				if len(s.RemoveDeps) > 0 {
					fmt.Printf("  Remove deps: %v\n", s.RemoveDeps)
				}
				fmt.Printf("  Reason: %s\n\n", s.Reason)
			}

			if apply {
				if err := generate.ApplyDepSuggestions(store, tag, suggestions); err != nil {
					return fmt.Errorf("applying suggestions: %w", err)
				}
				fmt.Println("Applied all dependency suggestions.")
			}

			return nil
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	cmd.Flags().BoolVar(&apply, "apply", false, "Apply suggested dependency changes")
	return cmd
}
