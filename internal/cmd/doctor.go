package cmd

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"time"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/generate"
	"github.com/spf13/cobra"
)

func NewDoctorCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "doctor",
		Short: "Health check for SCUD project",
		RunE: func(cmd *cobra.Command, args []string) error {
			store, err := getStore()
			if err != nil {
				return err
			}

			issues := 0
			check := func(name string, fn func() error) {
				if err := fn(); err != nil {
					fmt.Printf("  FAIL  %s: %v\n", name, err)
					issues++
				} else {
					fmt.Printf("  OK    %s\n", name)
				}
			}

			fmt.Println("SCUD Doctor")
			fmt.Println()

			// Check .scud/ structure
			check(".scud/ directory", func() error {
				if _, err := os.Stat(store.ScudDir()); err != nil {
					return fmt.Errorf("missing .scud/ directory")
				}
				return nil
			})

			check("tasks file", func() error {
				if _, err := os.Stat(store.TasksFile()); err != nil {
					return fmt.Errorf("missing tasks/tasks.scg")
				}
				return nil
			})

			// Check config
			check("config.toml", func() error {
				_, err := config.Load(store.ScudDir())
				return err
			})

			// Check rho binary
			check("rho in PATH", func() error {
				path, err := exec.LookPath("rho")
				if err != nil {
					return fmt.Errorf("rho not found in PATH")
				}
				fmt.Printf(" (%s)", path)
				return nil
			})

			// Check dependencies
			check("task dependencies", func() error {
				phases, err := store.LoadPhases()
				if err != nil {
					return err
				}
				for tag := range phases {
					result := generate.CheckDeps(phases, tag)
					if !result.OK {
						return fmt.Errorf("issues in tag '%s': %s", tag, generate.FormatCheckResult(result))
					}
				}
				return nil
			})

			// Check for stale in-progress tasks
			check("stale tasks", func() error {
				phases, err := store.LoadPhases()
				if err != nil {
					return err
				}
				stale := 0
				for _, p := range phases {
					for _, t := range p.Tasks {
						if t.Status == "in-progress" && t.UpdatedAt != "" {
							updated, err := time.Parse(time.RFC3339, t.UpdatedAt)
							if err == nil && time.Since(updated) > 24*time.Hour {
								stale++
							}
						}
					}
				}
				if stale > 0 {
					return fmt.Errorf("%d tasks in-progress for >24h", stale)
				}
				return nil
			})

			// Check for orphaned subtasks
			check("orphaned subtasks", func() error {
				phases, err := store.LoadPhases()
				if err != nil {
					return err
				}
				for _, p := range phases {
					taskMap := p.TaskMap()
					for _, t := range p.Tasks {
						if t.ParentID != "" {
							if _, ok := taskMap[t.ParentID]; !ok {
								return fmt.Errorf("task %s references missing parent %s", t.ID, t.ParentID)
							}
						}
					}
				}
				return nil
			})

			// Check guidance directory
			check("guidance directory", func() error {
				guidanceDir := filepath.Join(store.ScudDir(), "guidance")
				if _, err := os.Stat(guidanceDir); err != nil {
					return fmt.Errorf("missing guidance/ directory")
				}
				return nil
			})

			fmt.Println()
			if issues > 0 {
				fmt.Printf("%d issue(s) found.\n", issues)
			} else {
				fmt.Println("All checks passed.")
			}
			return nil
		},
	}
}
