package cmd

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/spf13/cobra"
)

func NewStatsCmd() *cobra.Command {
	var tag string
	var asJSON bool

	cmd := &cobra.Command{
		Use:   "stats",
		Short: "Show phase statistics",
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

			s := phase.Stats()

			if asJSON {
				enc := json.NewEncoder(os.Stdout)
				enc.SetIndent("", "  ")
				return enc.Encode(s)
			}

			pct := 0
			if s.Total > 0 {
				pct = s.Done * 100 / s.Total
			}

			fmt.Printf("Phase: %s\n\n", tag)
			fmt.Printf("  Total:       %d\n", s.Total)
			fmt.Printf("  Done:        %d (%d%%)\n", s.Done, pct)
			fmt.Printf("  Pending:     %d\n", s.Pending)
			fmt.Printf("  In Progress: %d\n", s.InProgressN)
			fmt.Printf("  Failed:      %d\n", s.Failed)
			fmt.Printf("  Blocked:     %d\n", s.Blocked)
			fmt.Printf("  Review:      %d\n", s.Review)
			fmt.Printf("  Expanded:    %d\n", s.Expanded)
			fmt.Printf("  Deferred:    %d\n", s.Deferred)
			fmt.Printf("  Cancelled:   %d\n", s.Cancelled)
			fmt.Printf("  Complexity:  %d\n", s.Complexity)
			return nil
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	cmd.Flags().BoolVar(&asJSON, "json", false, "Output as JSON")
	return cmd
}
