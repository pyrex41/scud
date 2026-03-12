package cmd

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"strings"

	"github.com/reuben/scud/internal/storage"
	"github.com/spf13/cobra"
)

func NewWarmupCmd() *cobra.Command {
	var asJSON bool

	cmd := &cobra.Command{
		Use:   "warmup",
		Short: "Session orientation — show git history, active tag, stats, next task",
		RunE: func(cmd *cobra.Command, args []string) error {
			if asJSON {
				return warmupJSON()
			}
			return warmupText()
		},
	}

	cmd.Flags().BoolVar(&asJSON, "json", false, "Output as JSON")
	return cmd
}

func warmupText() error {
	// Git info
	fmt.Println("=== Recent Git History ===")
	gitLog := exec.Command("git", "log", "--oneline", "-5")
	gitLog.Stdout = os.Stdout
	gitLog.Stderr = os.Stderr
	if err := gitLog.Run(); err != nil {
		fmt.Println("  (no git history available)")
	}
	fmt.Println()

	cwd, _ := os.Getwd()
	root, err := storage.FindRoot(cwd)
	if err != nil {
		fmt.Println("No .scud directory found. Run 'scud init' to get started.")
		return nil
	}
	store := storage.New(root)

	activeTag := store.ActiveTag()
	fmt.Printf("=== Active Tag: %s ===\n", activeTag)
	if activeTag == "" {
		fmt.Println("  (no active tag set)")
		return nil
	}

	phases, err := store.LoadPhases()
	if err != nil {
		return err
	}
	phase, ok := phases[activeTag]
	if !ok {
		fmt.Printf("  Tag '%s' not found in tasks file.\n", activeTag)
		return nil
	}

	s := phase.Stats()
	pct := 0
	if s.Total > 0 {
		pct = s.Done * 100 / s.Total
	}
	fmt.Printf("\n=== Stats ===\n")
	fmt.Printf("  Total: %d  Done: %d (%d%%)  Pending: %d  In Progress: %d\n",
		s.Total, s.Done, pct, s.Pending, s.InProgressN)
	if s.Failed > 0 {
		fmt.Printf("  Failed: %d\n", s.Failed)
	}
	if s.Blocked > 0 {
		fmt.Printf("  Blocked: %d\n", s.Blocked)
	}

	t := phase.FindNextTask(phases)
	if t != nil {
		fmt.Printf("\n=== Next Task ===\n")
		fmt.Printf("  %s: %s\n", t.ID, t.Title)
		fmt.Printf("  Priority: %s  Complexity: %d\n", t.Priority, t.Complexity)
		if len(t.Dependencies) > 0 {
			fmt.Printf("  Deps: %s\n", strings.Join(t.Dependencies, ", "))
		}
		fmt.Printf("\n  Claim with: scud set-status %s in-progress\n", t.ID)
	} else {
		fmt.Printf("\n  No ready tasks.\n")
	}

	return nil
}

type warmupOutput struct {
	ActiveTag string          `json:"active_tag"`
	Stats     *warmupStats    `json:"stats,omitempty"`
	NextTask  *warmupNextTask `json:"next_task,omitempty"`
}

type warmupStats struct {
	Total      int `json:"total"`
	Done       int `json:"done"`
	Pending    int `json:"pending"`
	InProgress int `json:"in_progress"`
	Failed     int `json:"failed"`
	Blocked    int `json:"blocked"`
}

type warmupNextTask struct {
	ID           string   `json:"id"`
	Title        string   `json:"title"`
	Priority     string   `json:"priority"`
	Complexity   int      `json:"complexity"`
	Dependencies []string `json:"dependencies,omitempty"`
}

func warmupJSON() error {
	cwd, _ := os.Getwd()
	root, err := storage.FindRoot(cwd)
	if err != nil {
		return json.NewEncoder(os.Stdout).Encode(warmupOutput{})
	}
	store := storage.New(root)

	out := warmupOutput{ActiveTag: store.ActiveTag()}
	if out.ActiveTag == "" {
		return json.NewEncoder(os.Stdout).Encode(out)
	}

	phases, err := store.LoadPhases()
	if err != nil {
		return err
	}
	phase, ok := phases[out.ActiveTag]
	if !ok {
		return json.NewEncoder(os.Stdout).Encode(out)
	}

	s := phase.Stats()
	out.Stats = &warmupStats{
		Total:      s.Total,
		Done:       s.Done,
		Pending:    s.Pending,
		InProgress: s.InProgressN,
		Failed:     s.Failed,
		Blocked:    s.Blocked,
	}

	t := phase.FindNextTask(phases)
	if t != nil {
		out.NextTask = &warmupNextTask{
			ID:           t.ID,
			Title:        t.Title,
			Priority:     string(t.Priority),
			Complexity:   t.Complexity,
			Dependencies: t.Dependencies,
		}
	}

	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	return enc.Encode(out)
}
