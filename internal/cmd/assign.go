package cmd

import (
	"fmt"

	"github.com/reuben/scud/internal/model"
	"github.com/spf13/cobra"
)

func NewAssignCmd() *cobra.Command {
	var tag string

	cmd := &cobra.Command{
		Use:   "assign <id> <person>",
		Short: "Assign a task to a person",
		Args:  cobra.ExactArgs(2),
		RunE: func(cmd *cobra.Command, args []string) error {
			id := args[0]
			person := args[1]

			store, err := getStore()
			if err != nil {
				return err
			}
			tag, err := store.ResolveTag(tag)
			if err != nil {
				return err
			}

			return store.UpdatePhase(tag, func(p *model.Phase) error {
				t := p.FindTask(id)
				if t == nil {
					return fmt.Errorf("task '%s' not found in tag '%s'", id, tag)
				}
				t.AssignedTo = person
				t.SetUpdatedNow()
				fmt.Printf("%s: assigned to %s\n", id, person)
				return nil
			})
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	return cmd
}
