package cmd

import (
	"fmt"
	"time"

	"github.com/reuben/scud/internal/model"
	"github.com/spf13/cobra"
)

func NewCreateCmd() *cobra.Command {
	var tag, title, agent, tier, priority string
	var complexity int

	cmd := &cobra.Command{
		Use:   "create",
		Short: "Create a new task",
		RunE: func(cmd *cobra.Command, args []string) error {
			if title == "" {
				return fmt.Errorf("--title is required")
			}

			store, err := getStore()
			if err != nil {
				return err
			}
			tag, err := store.ResolveTag(tag)
			if err != nil {
				return err
			}

			var newID string
			err = store.UpdatePhase(tag, func(p *model.Phase) error {
				newID = p.NextTaskID()
				t := &model.Task{
					ID:         newID,
					Title:      title,
					Status:     model.Pending,
					Complexity: complexity,
					Priority:   model.Medium,
					CreatedAt:  time.Now().UTC().Format(time.RFC3339),
				}

				if priority != "" {
					parsed, ok := model.ParsePriority(priority)
					if !ok {
						return fmt.Errorf("invalid priority: %s (valid: critical, high, medium, low)", priority)
					}
					t.Priority = parsed
				}

				if agent != "" {
					t.AgentType = model.AgentType(agent)
				}
				if tier != "" {
					t.ModelTier = model.ModelTier(tier)
				}

				// Auto-assign agent if not specified
				t.AutoAssignAgent()

				p.Tasks = append(p.Tasks, t)
				return nil
			})
			if err != nil {
				return err
			}

			fmt.Printf("Created task %s: %s\n", newID, title)
			return nil
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	cmd.Flags().StringVar(&title, "title", "", "Task title")
	cmd.Flags().StringVar(&agent, "agent", "", "Agent type (builder, fast-builder, reviewer, tester, planner, researcher, analyzer)")
	cmd.Flags().StringVar(&tier, "tier", "", "Model tier (fast, standard, smart, custom)")
	cmd.Flags().StringVar(&priority, "priority", "", "Priority (critical, high, medium, low)")
	cmd.Flags().IntVar(&complexity, "complexity", 3, "Complexity score")
	return cmd
}
