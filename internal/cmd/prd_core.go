package cmd

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/inputio"
	"github.com/reuben/scud/internal/taskapi"
	"github.com/reuben/scud/pkg/generate"
	"github.com/reuben/scud/pkg/llm"
	"github.com/reuben/scud/pkg/model"
	"github.com/spf13/cobra"
	"os"
	"strings"
)

// NewPRDCmd creates a read-only PRD proposal command. Inject caller for tests.
func NewPRDCmd(callers ...generate.TargetCaller) *cobra.Command {
	var modelName, tag string
	var num int
	c := &cobra.Command{Use: "parse [FILE]", Short: "Propose tasks from a PRD (no changes written)", Args: cobra.MaximumNArgs(1), RunE: func(cmd *cobra.Command, args []string) error {
		input := cmd.InOrStdin()
		if num < 0 {
			return fmt.Errorf("--num cannot be negative")
		}
		modelName = strings.ToLower(strings.TrimSpace(modelName))
		if modelName != "main" && modelName != "fast" {
			return fmt.Errorf("--model must be main or fast")
		}
		phaseTag := tag
		if phaseTag == "" {
			phaseTag = "main"
		}
		if _, e := taskapi.ValidateDocument(taskapi.Document{SchemaVersion: 1, Phases: []taskapi.Phase{{Tag: phaseTag, Name: phaseTag, IDFormat: "sequential", Tasks: []*model.Task{}}}}); e != nil {
			return fmt.Errorf("invalid --tag %q: %w", phaseTag, e)
		}
		var content []byte
		var err error
		if len(args) == 0 || args[0] == "-" {
			content, err = inputio.ReadAll(cmd.Context(), input, 4<<20)
		} else {
			var f *os.File
			f, err = os.Open(args[0])
			if err == nil {
				content, err = inputio.ReadAll(cmd.Context(), f, 4<<20)
				closeErr := f.Close()
				if err == nil {
					err = closeErr
				}
			}
		}
		if errors.Is(err, inputio.ErrTooLarge) {
			return fmt.Errorf("PRD exceeds 4 MiB input limit: %w", err)
		}
		if err != nil {
			return fmt.Errorf("reading PRD: %w", err)
		}
		if len(content) > 4<<20 {
			return fmt.Errorf("PRD exceeds 4 MiB input limit")
		}
		if len(content) == 0 || len(bytes.TrimSpace(content)) == 0 {
			return fmt.Errorf("PRD content is empty")
		}
		var caller generate.TargetCaller
		if len(callers) > 0 {
			caller = callers[0]
		} else {
			store, e := getStore()
			if e != nil {
				return e
			}
			cfg, e := config.LoadModelDefaults(store.ScudDir())
			if e != nil {
				return e
			}
			if modelName == "" {
				modelName = "main"
			}
			target, e := cfg.ResolveModelTarget(modelName)
			if e != nil {
				return e
			}
			cl, e := llm.NewTargetClient(target)
			if e != nil {
				return e
			}
			caller = cl
		}
		p, err := generate.Propose(cmd.Context(), caller, string(content), generate.ProposalOptions{Tag: tag, NumTasks: num})
		if err != nil {
			return err
		}
		enc := json.NewEncoder(cmd.OutOrStdout())
		enc.SetIndent("", "  ")
		return enc.Encode(p)
	}}
	c.Flags().StringVar(&modelName, "model", "main", "Model default: main or fast")
	c.Flags().StringVar(&tag, "tag", "main", "Phase tag")
	c.Flags().IntVar(&num, "num", 0, "Requested task count (optional)")
	c.Flags().Bool("json", true, "Emit proposal JSON")
	return c
}
