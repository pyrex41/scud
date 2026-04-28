package cmd

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/reuben/scud/internal/storage"
	"github.com/reuben/scud/pkg/model"
	"github.com/spf13/cobra"
)

func NewListCmd() *cobra.Command {
	var tag, status string
	var asJSON bool

	cmd := &cobra.Command{
		Use:   "list",
		Short: "List tasks",
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
				fmt.Printf("No tasks found for tag '%s'\n", tag)
				return nil
			}

			tasks := phase.Tasks
			model.NaturalSortTasks(tasks)

			// Filter by status
			if status != "" {
				filterStatus, valid := model.ParseStatus(status)
				if !valid {
					return fmt.Errorf("invalid status: %s", status)
				}
				var filtered []*model.Task
				for _, t := range tasks {
					if t.Status == filterStatus {
						filtered = append(filtered, t)
					}
				}
				tasks = filtered
			}

			if asJSON {
				enc := json.NewEncoder(os.Stdout)
				enc.SetIndent("", "  ")
				return enc.Encode(tasks)
			}

			if len(tasks) == 0 {
				fmt.Println("No tasks found.")
				return nil
			}

			fmt.Printf("Tasks for '%s' (%d):\n\n", tag, len(tasks))
			for _, t := range tasks {
				prefix := " "
				if t.ParentID != "" {
					prefix = "  "
				}
				fmt.Printf("%s%-6s [%s] %-3d %s  %s\n",
					prefix,
					t.ID,
					model.StatusCode(t.Status),
					t.Complexity,
					model.PriorityCode(t.Priority),
					t.Title,
				)
			}
			return nil
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	cmd.Flags().StringVarP(&status, "status", "s", "", "Filter by status")
	cmd.Flags().BoolVar(&asJSON, "json", false, "Output as JSON")
	return cmd
}

func getStore() (*storage.Storage, error) {
	cwd, _ := os.Getwd()
	root, err := storage.FindRoot(cwd)
	if err != nil {
		return nil, err
	}
	return storage.New(root), nil
}
