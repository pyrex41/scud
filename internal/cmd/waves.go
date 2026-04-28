package cmd

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/reuben/scud/pkg/wave"
	"github.com/spf13/cobra"
)

func NewWavesCmd() *cobra.Command {
	var tag string
	var asJSON bool

	cmd := &cobra.Command{
		Use:   "waves",
		Short: "Show execution waves",
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

			if asJSON {
				enc := json.NewEncoder(os.Stdout)
				enc.SetIndent("", "  ")
				return enc.Encode(wr)
			}

			if len(wr.Waves) == 0 {
				fmt.Println("No actionable tasks to plan.")
				return nil
			}

			fmt.Printf("Execution plan for '%s':\n\n", tag)
			for _, w := range wr.Waves {
				fmt.Printf("Wave %d (%d tasks):\n", w.Number, len(w.Tasks))
				for _, id := range w.Tasks {
					if t := phase.FindTask(id); t != nil {
						agent := string(t.AgentType)
						if agent == "" {
							agent = "builder"
						}
						fmt.Printf("  %s: %s [%s, complexity=%d]\n",
							id, t.Title, agent, t.Complexity)
					}
				}
				fmt.Println()
			}

			if len(wr.CircularDeps) > 0 {
				fmt.Printf("Circular dependencies: %v\n", wr.CircularDeps)
			}
			return nil
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	cmd.Flags().BoolVar(&asJSON, "json", false, "Output as JSON")
	return cmd
}
