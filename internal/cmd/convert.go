package cmd

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/reuben/scud/internal/model"
	"github.com/reuben/scud/internal/scg"
	"github.com/spf13/cobra"
)

func NewConvertCmd() *cobra.Command {
	var format string

	cmd := &cobra.Command{
		Use:   "convert <file>",
		Short: "Convert between JSON and SCG formats",
		Long:  "Reads input file and converts to the specified format. Default: JSON input -> SCG output.",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			inputFile := args[0]
			data, err := os.ReadFile(inputFile)
			if err != nil {
				return fmt.Errorf("reading file: %w", err)
			}
			content := string(data)

			switch format {
			case "json":
				// SCG -> JSON
				phases := scg.ParseMultiPhase(content)
				if len(phases) == 0 {
					// Try single phase
					phase := scg.ParsePhase(content)
					if phase != nil && len(phase.Tasks) > 0 {
						phases = map[string]*model.Phase{phase.Name: phase}
					}
				}
				enc := json.NewEncoder(os.Stdout)
				enc.SetIndent("", "  ")
				return enc.Encode(phases)

			case "scg", "":
				// JSON -> SCG
				// Try as multi-phase map
				var phases map[string]*model.Phase
				if err := json.Unmarshal(data, &phases); err != nil {
					// Try as single phase
					var phase model.Phase
					if err2 := json.Unmarshal(data, &phase); err2 != nil {
						// Try as task array
						var tasks []*model.Task
						if err3 := json.Unmarshal(data, &tasks); err3 != nil {
							return fmt.Errorf("could not parse JSON as phases, phase, or tasks: %w", err)
						}
						phaseName := strings.TrimSuffix(inputFile, ".json")
						phase = model.Phase{Name: phaseName, Tasks: tasks}
					}
					phases = map[string]*model.Phase{phase.Name: &phase}
				}
				fmt.Print(scg.SerializeMultiPhase(phases))
				return nil

			default:
				return fmt.Errorf("unsupported format: %s (use 'json' or 'scg')", format)
			}
		},
	}

	cmd.Flags().StringVar(&format, "format", "", "Output format: json or scg (default: scg)")
	return cmd
}
