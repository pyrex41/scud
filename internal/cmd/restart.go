package cmd

import (
	"fmt"

	"github.com/reuben/scud/pkg/model"
	"github.com/spf13/cobra"
)

func NewRestartCmd() *cobra.Command {
	var tag string

	cmd := &cobra.Command{
		Use:   "restart <id>",
		Short: "Reset task status to pending",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			id := args[0]

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
				oldStatus := t.Status
				t.Status = model.Pending
				t.SetUpdatedNow()
				fmt.Printf("%s: %s -> pending (was %s)\n", id, t.Title, oldStatus)
				return nil
			})
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	return cmd
}
