package cmd

import (
	"fmt"

	"github.com/reuben/scud/internal/model"
	"github.com/spf13/cobra"
)

func NewMermaidCmd() *cobra.Command {
	var tag string

	cmd := &cobra.Command{
		Use:   "mermaid",
		Short: "Output Mermaid graph of task dependencies",
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

			taskMap := phase.TaskMap()
			tasks := make([]*model.Task, len(phase.Tasks))
			copy(tasks, phase.Tasks)
			model.NaturalSortTasks(tasks)

			fmt.Println("graph TD")
			for _, t := range tasks {
				for _, depID := range t.Dependencies {
					dep, ok := taskMap[depID]
					if !ok {
						continue
					}
					fmt.Printf("  %s[%s] --> %s[%s]\n", t.ID, t.Title, dep.ID, dep.Title)
				}
			}

			// Also show tasks with no dependencies (isolated nodes)
			for _, t := range tasks {
				if len(t.Dependencies) == 0 {
					hasDependents := false
					for _, other := range tasks {
						for _, dep := range other.Dependencies {
							if dep == t.ID {
								hasDependents = true
								break
							}
						}
						if hasDependents {
							break
						}
					}
					if !hasDependents {
						fmt.Printf("  %s[%s]\n", t.ID, t.Title)
					}
				}
			}

			return nil
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	return cmd
}
