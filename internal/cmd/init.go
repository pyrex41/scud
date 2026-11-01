package cmd

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"github.com/reuben/scud/internal/taskapi"
	"github.com/spf13/cobra"
)

// NewInitCmd creates task storage, never overwrites model settings or tasks.
func NewInitCmd() *cobra.Command {
	command := &cobra.Command{
		Use: "init", Short: "Initialize task storage without overwriting existing tasks or model defaults", Args: cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			cwd, err := os.Getwd()
			if err != nil {
				return err
			}
			if err := os.MkdirAll(filepath.Join(cwd, ".scud", "tasks"), 0755); err != nil {
				return err
			}
			service, err := taskapi.Open(cwd)
			if err != nil {
				return err
			}
			snapshot, err := service.Snapshot(cmd.Context())
			if err != nil {
				return fmt.Errorf("existing task storage was not changed: %w", err)
			}
			if _, err := os.Stat(filepath.Join(cwd, ".scud", "tasks", "tasks.json")); os.IsNotExist(err) {
				snapshot, err = service.Import(cmd.Context(), taskapi.Document{SchemaVersion: 1, Phases: []taskapi.Phase{}}, taskapi.ImportOptions{})
				if err != nil {
					return err
				}
			} else if err != nil {
				return err
			}
			return json.NewEncoder(cmd.OutOrStdout()).Encode(map[string]any{"schema_version": 1, "project": cwd, "revision": snapshot.Revision, "initialized": true})
		},
	}
	command.Flags().Bool("json", false, "JSON is the default output")
	return command
}
