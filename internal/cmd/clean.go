package cmd

import (
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/reuben/scud/internal/scg"
	"github.com/spf13/cobra"
)

func NewCleanCmd() *cobra.Command {
	var removePhase bool

	cmd := &cobra.Command{
		Use:   "clean <tag>",
		Short: "Archive a phase",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			tag := args[0]

			store, err := getStore()
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

			// Archive to .scud/archive/
			archiveDir := filepath.Join(store.ScudDir(), "archive")
			if err := os.MkdirAll(archiveDir, 0755); err != nil {
				return fmt.Errorf("creating archive directory: %w", err)
			}

			timestamp := time.Now().UTC().Format("20060102-150405")
			archiveFile := filepath.Join(archiveDir, fmt.Sprintf("%s-%s.scg", tag, timestamp))

			content := scg.SerializePhase(phase)
			if err := os.WriteFile(archiveFile, []byte(content), 0644); err != nil {
				return fmt.Errorf("writing archive: %w", err)
			}

			fmt.Printf("Archived '%s' to %s\n", tag, archiveFile)

			if removePhase {
				delete(phases, tag)
				if err := store.SavePhases(phases); err != nil {
					return fmt.Errorf("saving after delete: %w", err)
				}
				fmt.Printf("Removed '%s' from tasks file.\n", tag)
			}

			return nil
		},
	}

	cmd.Flags().BoolVar(&removePhase, "delete", false, "Remove the phase from tasks file after archiving")
	return cmd
}
