package cmd

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/reuben/scud/pkg/model"
	"github.com/reuben/scud/pkg/wave"
	"github.com/spf13/cobra"
)

func NewNextBatchCmd() *cobra.Command {
	var tag string
	var limit int
	var asJSON bool

	cmd := &cobra.Command{
		Use:   "next-batch",
		Short: "Show all tasks ready to execute (wave 1)",
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

			actionable := phase.ActionableTasks()
			wr := wave.Plan(actionable)

			if len(wr.Waves) == 0 {
				if asJSON {
					fmt.Println("[]")
					return nil
				}
				fmt.Println("No ready tasks.")
				return nil
			}

			wave1 := wr.Waves[0]
			taskIDs := wave1.Tasks
			if limit > 0 && len(taskIDs) > limit {
				taskIDs = taskIDs[:limit]
			}

			if asJSON {
				var tasks []*model.Task
				for _, id := range taskIDs {
					if t := phase.FindTask(id); t != nil {
						tasks = append(tasks, t)
					}
				}
				enc := json.NewEncoder(os.Stdout)
				enc.SetIndent("", "  ")
				return enc.Encode(tasks)
			}

			fmt.Printf("Ready tasks (%d available):\n\n", len(wave1.Tasks))
			for _, id := range taskIDs {
				t := phase.FindTask(id)
				if t == nil {
					continue
				}
				deps := ""
				if len(t.Dependencies) > 0 {
					deps = fmt.Sprintf(" [deps: %s]", strings.Join(t.Dependencies, ", "))
				}
				fmt.Printf("  %-6s [%s] %-3d %s  %s%s\n",
					t.ID,
					model.StatusCode(t.Status),
					t.Complexity,
					model.PriorityCode(t.Priority),
					t.Title,
					deps,
				)
			}
			return nil
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	cmd.Flags().IntVarP(&limit, "limit", "n", 0, "Maximum number of tasks to show")
	cmd.Flags().BoolVar(&asJSON, "json", false, "Output as JSON")
	return cmd
}
