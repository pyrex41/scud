package cmd

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/reuben/scud/pkg/model"
	"github.com/spf13/cobra"
)

func NewShowCmd() *cobra.Command {
	var tag string
	var asJSON bool

	cmd := &cobra.Command{
		Use:   "show <id>",
		Short: "Show task details",
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
			phases, err := store.LoadPhases()
			if err != nil {
				return err
			}
			phase, ok := phases[tag]
			if !ok {
				return fmt.Errorf("tag '%s' not found", tag)
			}
			t := phase.FindTask(args[0])
			if t == nil {
				return fmt.Errorf("task '%s' not found in tag '%s'", args[0], tag)
			}

			if asJSON {
				enc := json.NewEncoder(os.Stdout)
				enc.SetIndent("", "  ")
				return enc.Encode(t)
			}

			fmt.Printf("ID:          %s\n", t.ID)
			fmt.Printf("Title:       %s\n", t.Title)
			fmt.Printf("Status:      %s\n", t.Status)
			fmt.Printf("Priority:    %s\n", t.Priority)
			fmt.Printf("Complexity:  %d\n", t.Complexity)
			if t.AgentType != "" {
				fmt.Printf("Agent:       %s\n", t.AgentType)
			}
			if t.ModelTier != "" {
				fmt.Printf("Model Tier:  %s\n", t.ModelTier)
			}
			if len(t.Dependencies) > 0 {
				fmt.Printf("Deps:        %s\n", strings.Join(t.Dependencies, ", "))
			}
			if t.ParentID != "" {
				fmt.Printf("Parent:      %s\n", t.ParentID)
			}
			if len(t.Subtasks) > 0 {
				fmt.Printf("Subtasks:    %s\n", strings.Join(t.Subtasks, ", "))
			}
			if t.AssignedTo != "" {
				fmt.Printf("Assigned:    %s\n", t.AssignedTo)
			}
			if t.Description != "" {
				fmt.Printf("\nDescription:\n%s\n", indent(t.Description))
			}
			if t.Details != "" {
				fmt.Printf("\nDetails:\n%s\n", indent(t.Details))
			}
			if t.TestStrategy != "" {
				fmt.Printf("\nTest Strategy:\n%s\n", indent(t.TestStrategy))
			}
			return nil
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	cmd.Flags().BoolVar(&asJSON, "json", false, "Output as JSON")
	return cmd
}

func indent(s string) string {
	lines := strings.Split(s, "\n")
	for i, l := range lines {
		lines[i] = "  " + l
	}
	return strings.Join(lines, "\n")
}

func NewNextCmd() *cobra.Command {
	var tag string

	cmd := &cobra.Command{
		Use:   "next",
		Short: "Show next ready task",
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
			phase, ok := phases[tag]
			if !ok {
				return fmt.Errorf("tag '%s' not found", tag)
			}
			t := phase.FindNextTask(phases)
			if t == nil {
				fmt.Println("No ready tasks.")
				stats := phase.Stats()
				fmt.Printf("  Pending=%d, InProgress=%d, Blocked=%d, Done=%d\n",
					stats.Pending, stats.InProgressN, stats.Blocked, stats.Done)
				return nil
			}
			fmt.Printf("Next task: %s\n", t.ID)
			fmt.Printf("  Title:      %s\n", t.Title)
			fmt.Printf("  Priority:   %s\n", t.Priority)
			fmt.Printf("  Complexity: %d\n", t.Complexity)
			if len(t.Dependencies) > 0 {
				fmt.Printf("  Deps:       %s\n", strings.Join(t.Dependencies, ", "))
			}
			fmt.Printf("\nClaim with: scud set-status %s in-progress -t %s\n", t.ID, tag)
			return nil
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	return cmd
}

func NewSetStatusCmd() *cobra.Command {
	var tag, from, to string

	cmd := &cobra.Command{
		Use:   "set-status <id> <status>",
		Short: "Update task status",
		Args:  cobra.RangeArgs(1, 2),
		RunE: func(cmd *cobra.Command, args []string) error {
			store, err := getStore()
			if err != nil {
				return err
			}
			tag, err := store.ResolveTag(tag)
			if err != nil {
				return err
			}

			// Bulk mode: --from X --to Y
			if from != "" && to != "" {
				fromStatus, ok := model.ParseStatus(from)
				if !ok {
					return fmt.Errorf("invalid --from status: %s", from)
				}
				toStatus, ok := model.ParseStatus(to)
				if !ok {
					return fmt.Errorf("invalid --to status: %s", to)
				}
				count := 0
				err := store.UpdatePhase(tag, func(p *model.Phase) error {
					for _, t := range p.Tasks {
						if t.Status == fromStatus {
							t.Status = toStatus
							t.SetUpdatedNow()
							count++
						}
					}
					return nil
				})
				if err != nil {
					return err
				}
				fmt.Printf("Updated %d tasks from %s to %s\n", count, from, to)
				return nil
			}

			if len(args) != 2 {
				return fmt.Errorf("usage: scud set-status <id> <status>")
			}

			id := args[0]
			newStatus, ok := model.ParseStatus(args[1])
			if !ok {
				return fmt.Errorf("invalid status: %s\nValid: pending, in-progress, done, failed, blocked, review, expanded, deferred, cancelled", args[1])
			}

			return store.UpdatePhase(tag, func(p *model.Phase) error {
				t := p.FindTask(id)
				if t == nil {
					return fmt.Errorf("task '%s' not found in tag '%s'", id, tag)
				}
				t.Status = newStatus
				t.SetUpdatedNow()
				fmt.Printf("%s: %s -> %s\n", id, t.Title, newStatus)
				return nil
			})
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	cmd.Flags().StringVar(&from, "from", "", "Bulk: source status")
	cmd.Flags().StringVar(&to, "to", "", "Bulk: target status")
	return cmd
}
