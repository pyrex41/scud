package cmd

import (
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/spf13/cobra"
)

func NewLogCmd() *cobra.Command {
	var tag string

	cmd := &cobra.Command{
		Use:   "log <id> <message>",
		Short: "Append a log entry for a task",
		Args:  cobra.ExactArgs(2),
		RunE: func(cmd *cobra.Command, args []string) error {
			id := args[0]
			message := args[1]

			store, err := getStore()
			if err != nil {
				return err
			}
			tag, err := store.ResolveTag(tag)
			if err != nil {
				return err
			}

			logDir := filepath.Join(store.ScudDir(), "logs", tag)
			if err := os.MkdirAll(logDir, 0755); err != nil {
				return fmt.Errorf("creating log directory: %w", err)
			}

			logFile := filepath.Join(logDir, id+".log")
			f, err := os.OpenFile(logFile, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
			if err != nil {
				return fmt.Errorf("opening log file: %w", err)
			}
			defer f.Close()

			timestamp := time.Now().UTC().Format(time.RFC3339)
			entry := fmt.Sprintf("[%s] %s\n", timestamp, message)
			if _, err := f.WriteString(entry); err != nil {
				return fmt.Errorf("writing log entry: %w", err)
			}

			fmt.Printf("Logged to %s: %s\n", id, message)
			return nil
		},
	}

	cmd.Flags().StringVarP(&tag, "tag", "t", "", "Phase tag")
	return cmd
}
