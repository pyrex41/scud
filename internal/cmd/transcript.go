package cmd

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/reuben/scud/internal/db"
	"github.com/reuben/scud/internal/storage"
	"github.com/reuben/scud/internal/transcript"
	"github.com/spf13/cobra"
)

// NewTranscriptCmd creates the "transcript" command group.
func NewTranscriptCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:     "transcript",
		Aliases: []string{"tx"},
		Short:   "Manage session transcripts",
	}

	cmd.AddCommand(
		newTranscriptListCmd(),
		newTranscriptViewCmd(),
		newTranscriptSearchCmd(),
		newTranscriptStatsCmd(),
		newTranscriptImportCmd(),
	)
	return cmd
}

func openDB() (*db.DB, error) {
	wd, err := os.Getwd()
	if err != nil {
		return nil, err
	}
	root, err := storage.FindRoot(wd)
	if err != nil {
		return nil, err
	}
	return db.Open(filepath.Join(root, ".scud"))
}

func newTranscriptListCmd() *cobra.Command {
	var limit int

	cmd := &cobra.Command{
		Use:   "list",
		Short: "List recent sessions",
		RunE: func(cmd *cobra.Command, args []string) error {
			database, err := openDB()
			if err != nil {
				return err
			}
			defer database.Close()

			sessions, err := database.ListSessions(limit)
			if err != nil {
				return err
			}
			if len(sessions) == 0 {
				fmt.Println("No sessions found.")
				return nil
			}

			for _, s := range sessions {
				status := s.Status
				ended := ""
				if s.EndedAt != "" {
					ended = fmt.Sprintf(" -> %s", s.EndedAt)
				}
				model := ""
				if s.Model != "" {
					model = fmt.Sprintf(" [%s]", s.Model)
				}
				tag := ""
				if s.Tag != "" {
					tag = fmt.Sprintf(" (%s)", s.Tag)
				}
				fmt.Printf("%-8s %-8s %s%s%s%s\n", s.ID[:minLen(len(s.ID), 8)], status, s.StartedAt, ended, model, tag)
			}
			return nil
		},
	}

	cmd.Flags().IntVarP(&limit, "limit", "n", 20, "Maximum sessions to show")
	return cmd
}

func newTranscriptViewCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "view <session-id>",
		Short: "View transcript for a session",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			database, err := openDB()
			if err != nil {
				return err
			}
			defer database.Close()

			sessionID := args[0]
			session, err := database.GetSession(sessionID)
			if err != nil {
				return err
			}

			fmt.Printf("Session: %s\n", session.ID)
			fmt.Printf("Model:   %s\n", session.Model)
			fmt.Printf("Status:  %s\n", session.Status)
			fmt.Printf("Started: %s\n", session.StartedAt)
			if session.EndedAt != "" {
				fmt.Printf("Ended:   %s\n", session.EndedAt)
			}
			fmt.Println(strings.Repeat("-", 60))

			msgs, err := database.GetTranscript(sessionID)
			if err != nil {
				return err
			}
			for _, m := range msgs {
				fmt.Printf("\n[%s] %s:\n%s\n", m.Timestamp, m.Role, m.Content)
			}
			return nil
		},
	}
	return cmd
}

func newTranscriptSearchCmd() *cobra.Command {
	var limit int

	cmd := &cobra.Command{
		Use:   "search <query>",
		Short: "Search transcript messages",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			database, err := openDB()
			if err != nil {
				return err
			}
			defer database.Close()

			msgs, err := database.SearchMessages(args[0], limit)
			if err != nil {
				return err
			}
			if len(msgs) == 0 {
				fmt.Println("No matches found.")
				return nil
			}

			for _, m := range msgs {
				content := m.Content
				if len(content) > 200 {
					content = content[:200] + "..."
				}
				fmt.Printf("[%s] session=%s role=%s\n  %s\n\n",
					m.Timestamp, m.SessionID[:minLen(len(m.SessionID), 8)], m.Role, content)
			}
			return nil
		},
	}

	cmd.Flags().IntVarP(&limit, "limit", "n", 50, "Maximum results")
	return cmd
}

func newTranscriptStatsCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "stats",
		Short: "Show transcript database statistics",
		RunE: func(cmd *cobra.Command, args []string) error {
			database, err := openDB()
			if err != nil {
				return err
			}
			defer database.Close()

			sessions, err := database.ListSessions(0)
			if err != nil {
				return err
			}

			var totalMsgs int
			for _, s := range sessions {
				msgs, err := database.GetTranscript(s.ID)
				if err != nil {
					continue
				}
				totalMsgs += len(msgs)
			}

			fmt.Printf("Sessions: %d\n", len(sessions))
			fmt.Printf("Messages: %d\n", totalMsgs)
			return nil
		},
	}
	return cmd
}

func newTranscriptImportCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "import [dir]",
		Short: "Import JSONL transcripts from a directory",
		Args:  cobra.MaximumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			database, err := openDB()
			if err != nil {
				return err
			}
			defer database.Close()

			dir := ""
			if len(args) > 0 {
				dir = args[0]
			} else {
				// Default: look for Claude Code transcripts in home dir
				home, err := os.UserHomeDir()
				if err != nil {
					return err
				}
				dir = filepath.Join(home, ".claude", "transcripts")
			}

			if _, err := os.Stat(dir); os.IsNotExist(err) {
				return fmt.Errorf("directory not found: %s", dir)
			}

			w := transcript.NewWatcher(database, dir)
			return w.ImportPath(dir)
		},
	}
	return cmd
}

func minLen(a, b int) int {
	if a < b {
		return a
	}
	return b
}
