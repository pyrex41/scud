package cmd

import (
	"fmt"
	"os"
	"os/exec"

	"github.com/reuben/scud/internal/storage"
	"github.com/reuben/scud/pkg/model"
	"github.com/spf13/cobra"
)

func NewCommitCmd() *cobra.Command {
	var message string
	var addAll bool

	cmd := &cobra.Command{
		Use:   "commit",
		Short: "Task-aware git commit — prefixes message with [TASK-ID]",
		RunE: func(cmd *cobra.Command, args []string) error {
			// Try to find an in-progress task for the commit prefix
			var taskID, taskTitle string

			cwd, _ := os.Getwd()
			root, err := storage.FindRoot(cwd)
			if err == nil {
				store := storage.New(root)
				tag := store.ActiveTag()
				if tag != "" {
					phases, err := store.LoadPhases()
					if err == nil {
						if phase, ok := phases[tag]; ok {
							for _, t := range phase.Tasks {
								if t.Status == model.InProgress {
									taskID = t.ID
									taskTitle = t.Title
									break
								}
							}
						}
					}
				}
			}

			// Build commit message
			commitMsg := message
			if commitMsg == "" && taskTitle != "" {
				commitMsg = taskTitle
			}
			if commitMsg == "" {
				return fmt.Errorf("no commit message provided and no in-progress task found")
			}
			if taskID != "" {
				commitMsg = fmt.Sprintf("[%s] %s", taskID, commitMsg)
			}

			// Build git command
			gitArgs := []string{"commit"}
			if addAll {
				gitArgs = append(gitArgs, "-a")
			}
			gitArgs = append(gitArgs, "-m", commitMsg)

			gitCmd := exec.Command("git", gitArgs...)
			gitCmd.Stdout = os.Stdout
			gitCmd.Stderr = os.Stderr
			gitCmd.Stdin = os.Stdin

			if err := gitCmd.Run(); err != nil {
				return fmt.Errorf("git commit failed: %w", err)
			}
			return nil
		},
	}

	cmd.Flags().StringVarP(&message, "message", "m", "", "Commit message")
	cmd.Flags().BoolVarP(&addAll, "all", "a", false, "Stage all modified files (-a)")
	return cmd
}
