package cmd

import (
	"fmt"
	"sort"

	"github.com/spf13/cobra"
)

func NewTagsCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "tags [name]",
		Short: "List or set active tag",
		RunE: func(cmd *cobra.Command, args []string) error {
			store, err := getStore()
			if err != nil {
				return err
			}

			if len(args) > 0 {
				if err := store.SetActiveTag(args[0]); err != nil {
					return err
				}
				fmt.Printf("Active tag set to '%s'\n", args[0])
				return nil
			}

			// List all tags
			phases, err := store.LoadPhases()
			if err != nil {
				return err
			}
			active := store.ActiveTag()

			var names []string
			for name := range phases {
				names = append(names, name)
			}
			sort.Strings(names)

			if len(names) == 0 {
				fmt.Println("No tags found.")
				return nil
			}

			for _, name := range names {
				marker := " "
				if name == active {
					marker = "*"
				}
				p := phases[name]
				s := p.Stats()
				fmt.Printf(" %s %-20s %d tasks (%d done, %d pending)\n",
					marker, name, s.Total, s.Done, s.Pending)
			}
			return nil
		},
	}

	return cmd
}
