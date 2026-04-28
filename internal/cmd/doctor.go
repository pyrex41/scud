package cmd

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"time"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/pkg/generate"
	"github.com/spf13/cobra"
)

func NewDoctorCmd() *cobra.Command {
	var asJSON bool

	cmd := &cobra.Command{
		Use:   "doctor",
		Short: "Health check for SCUD project",
		RunE: func(cmd *cobra.Command, args []string) error {
			store, err := getStore()
			if err != nil {
				return err
			}

			type checkResult struct {
				Name   string `json:"name"`
				OK     bool   `json:"ok"`
				Detail string `json:"detail,omitempty"`
			}

			var results []checkResult
			issues := 0

			check := func(name string, fn func() (string, error)) {
				detail, err := fn()
				if err != nil {
					results = append(results, checkResult{Name: name, OK: false, Detail: err.Error()})
					issues++
					if !asJSON {
						fmt.Printf("  FAIL  %s: %v\n", name, err)
					}
				} else {
					results = append(results, checkResult{Name: name, OK: true, Detail: detail})
					if !asJSON {
						msg := fmt.Sprintf("  OK    %s", name)
						if detail != "" {
							msg += fmt.Sprintf(" (%s)", detail)
						}
						fmt.Println(msg)
					}
				}
			}

			if !asJSON {
				fmt.Println("SCUD Doctor")
				fmt.Println()
			}

			check(".scud/ directory", func() (string, error) {
				if _, err := os.Stat(store.ScudDir()); err != nil {
					return "", fmt.Errorf("missing .scud/ directory")
				}
				return "", nil
			})

			check("tasks file", func() (string, error) {
				if _, err := os.Stat(store.TasksFile()); err != nil {
					return "", fmt.Errorf("missing tasks/tasks.scg")
				}
				return "", nil
			})

			check("config.toml", func() (string, error) {
				_, err := config.Load(store.ScudDir())
				return "", err
			})

			check("rho in PATH", func() (string, error) {
				path, err := exec.LookPath("rho")
				if err != nil {
					return "", fmt.Errorf("rho not found in PATH")
				}
				return path, nil
			})

			check("task dependencies", func() (string, error) {
				phases, err := store.LoadPhases()
				if err != nil {
					return "", err
				}
				for tag := range phases {
					result := generate.CheckDeps(phases, tag)
					if !result.OK {
						return "", fmt.Errorf("issues in tag '%s': %s", tag, generate.FormatCheckResult(result))
					}
				}
				return "", nil
			})

			check("stale tasks", func() (string, error) {
				phases, err := store.LoadPhases()
				if err != nil {
					return "", err
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
					return "", fmt.Errorf("%d tasks in-progress for >24h", stale)
				}
				return "", nil
			})

			check("orphaned subtasks", func() (string, error) {
				phases, err := store.LoadPhases()
				if err != nil {
					return "", err
				}
				for _, p := range phases {
					taskMap := p.TaskMap()
					for _, t := range p.Tasks {
						if t.ParentID != "" {
							if _, ok := taskMap[t.ParentID]; !ok {
								return "", fmt.Errorf("task %s references missing parent %s", t.ID, t.ParentID)
							}
						}
					}
				}
				return "", nil
			})

			check("guidance directory", func() (string, error) {
				guidanceDir := filepath.Join(store.ScudDir(), "guidance")
				if _, err := os.Stat(guidanceDir); err != nil {
					return "", fmt.Errorf("missing guidance/ directory")
				}
				return "", nil
			})

			if asJSON {
				type doctorOutput struct {
					Issues  int           `json:"issues"`
					Results []checkResult `json:"results"`
				}
				enc := json.NewEncoder(os.Stdout)
				enc.SetIndent("", "  ")
				return enc.Encode(doctorOutput{Issues: issues, Results: results})
			}

			fmt.Println()
			if issues > 0 {
				fmt.Printf("%d issue(s) found.\n", issues)
			} else {
				fmt.Println("All checks passed.")
			}
			return nil
		},
	}

	cmd.Flags().BoolVar(&asJSON, "json", false, "Output as JSON")
	return cmd
}
