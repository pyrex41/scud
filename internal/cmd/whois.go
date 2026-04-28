package cmd

import (
	"encoding/json"
	"fmt"
	"os"
	"sort"

	"github.com/reuben/scud/pkg/model"
	"github.com/spf13/cobra"
)

func NewWhoisCmd() *cobra.Command {
	var tag string
	var asJSON bool

	cmd := &cobra.Command{
		Use:   "whois",
		Short: "Group tasks by assignee",
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

			groups := make(map[string][]*model.Task)
			for _, t := range phase.Tasks {
				assignee := t.AssignedTo
				if assignee == "" {
					assignee = "(unassigned)"
				}
				groups[assignee] = append(groups[assignee], t)
			}

			if asJSON {
				enc := json.NewEncoder(os.Stdout)
				enc.SetIndent("", "  ")
				return enc.Encode(groups)
			}

			// Sort assignee names
			var names []string
			for name := range groups {
				names = append(names, name)
			}
			sort.Strings(names)

			for _, name := range names {
				tasks := groups[name]
				model.NaturalSortTasks(tasks)
				fmt.Printf("%s (%d tasks):\n", name, len(tasks))
				for _, t := range tasks {
					fmt.Printf("  %-6s [%s] %s\n", t.ID, model.StatusCode(t.Status), t.Title)
				}
				fmt.Println()
			}
			return nil
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	cmd.Flags().BoolVar(&asJSON, "json", false, "Output as JSON")
	return cmd
}
