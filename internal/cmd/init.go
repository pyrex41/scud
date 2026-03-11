package cmd

import (
	"fmt"
	"os"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/storage"
	"github.com/spf13/cobra"
)

func NewInitCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "init",
		Short: "Initialize a new SCUD project",
		RunE: func(cmd *cobra.Command, args []string) error {
			cwd, _ := os.Getwd()
			store := storage.New(cwd)

			if err := store.Initialize(); err != nil {
				return err
			}

			// Write default config
			cfg := config.Default()
			if err := cfg.Save(store.ScudDir()); err != nil {
				return err
			}

			fmt.Println("Initialized .scud/ project structure")
			fmt.Println("  .scud/config.toml   - configuration")
			fmt.Println("  .scud/tasks/        - task files")
			fmt.Println("  .scud/guidance/     - AI context files")
			return nil
		},
	}
}
