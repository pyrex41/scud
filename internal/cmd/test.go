package cmd

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"github.com/reuben/scud/internal/ui"
	"github.com/spf13/cobra"
)

func NewTestCmd() *cobra.Command {
	var tag, command string

	cmd := &cobra.Command{
		Use:   "test",
		Short: "Run project tests",
		RunE: func(cmd *cobra.Command, args []string) error {
			cwd, _ := os.Getwd()

			commands := detectTestCommands(cwd)
			if command != "" {
				commands = []string{command}
			}

			if len(commands) == 0 {
				return fmt.Errorf("no test command detected; use --command to specify one")
			}

			allPassed := true
			for _, c := range commands {
				spin := ui.NewSpinner(fmt.Sprintf("Running: %s", c))
				start := time.Now()

				cmdExec := exec.Command("sh", "-c", c)
				cmdExec.Dir = cwd

				var stdout, stderr strings.Builder
				cmdExec.Stdout = &stdout
				cmdExec.Stderr = &stderr

				err := cmdExec.Run()
				duration := time.Since(start)

				if err != nil {
					spin.Stop(false, fmt.Sprintf("%s (%.1fs)", c, duration.Seconds()))
					allPassed = false
					// Show output
					if stderr.Len() > 0 {
						fmt.Fprintln(os.Stderr, stderr.String())
					}
					if stdout.Len() > 0 {
						fmt.Println(stdout.String())
					}
				} else {
					spin.Stop(true, fmt.Sprintf("%s (%.1fs)", c, duration.Seconds()))
				}
			}

			if !allPassed {
				return fmt.Errorf("some tests failed")
			}

			ui.Success("All tests passed")
			return nil
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag (unused, for consistency)")
	cmd.Flags().StringVar(&command, "command", "", "Test command to run (overrides auto-detect)")
	return cmd
}

func detectTestCommands(dir string) []string {
	if fileExistsAt(filepath.Join(dir, "go.mod")) {
		return []string{"go test ./..."}
	}
	if fileExistsAt(filepath.Join(dir, "Cargo.toml")) {
		return []string{"cargo test"}
	}
	if fileExistsAt(filepath.Join(dir, "package.json")) {
		return detectNPMTestCommands(dir)
	}
	if fileExistsAt(filepath.Join(dir, "pyproject.toml")) || fileExistsAt(filepath.Join(dir, "setup.py")) {
		return []string{"pytest"}
	}
	if fileExistsAt(filepath.Join(dir, "Makefile")) {
		return []string{"make test"}
	}
	return nil
}

func detectNPMTestCommands(dir string) []string {
	data, err := os.ReadFile(filepath.Join(dir, "package.json"))
	if err != nil {
		return nil
	}
	content := string(data)
	if strings.Contains(content, `"test"`) {
		return []string{"npm test"}
	}
	return nil
}

func fileExistsAt(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}
