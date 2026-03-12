package cmd

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/reuben/scud/internal/llm"
	"github.com/reuben/scud/internal/ui"
	"github.com/spf13/cobra"
)

// MultiAgentResult is the JSON output for the multiagent command.
type MultiAgentResult struct {
	Query        string `json:"query"`
	Model        string `json:"model"`
	Effort       string `json:"effort"`
	Text         string `json:"text"`
	InputTokens  int    `json:"input_tokens"`
	OutputTokens int    `json:"output_tokens"`
	TotalTokens  int    `json:"total_tokens"`
	DurationSecs float64 `json:"duration_secs"`
}

func NewMultiAgentCmd() *cobra.Command {
	var (
		model      string
		effort     string
		tools      []string
		jsonOutput bool
		verbose    bool
		queryFile  string
	)

	cmd := &cobra.Command{
		Use:   "multiagent [query...]",
		Short: "Query the xAI native multi-agent model",
		Long: `Calls the xAI Responses API with the grok-4.20-multi-agent-beta-0309 model.
This model internally orchestrates 4-16 sub-agents to produce a high-quality answer.

Effort levels control agent count:
  low/medium  = 4 agents (faster, cheaper)
  high/xhigh  = 16 agents (more thorough)

Server-side tools (optional):
  web_search, x_search, code_execution, collections_search`,
		RunE: func(cmd *cobra.Command, args []string) error {
			query, err := resolveQuery(args, queryFile)
			if err != nil {
				return err
			}

			cfg := loadConfigBestEffort()

			if model == "" {
				model = cfg.LLM.MultiAgentModel
			}
			if effort == "" {
				effort = cfg.LLM.MultiAgentEffort
				if effort == "" {
					effort = "low"
				}
			}

			provider, err := llm.NewMultiAgentProvider()
			if err != nil {
				return fmt.Errorf("multi-agent provider: %w", err)
			}

			if verbose {
				agentCount := "4"
				if effort == "high" || effort == "xhigh" {
					agentCount = "16"
				}
				ui.Header("Multi-Agent", fmt.Sprintf("(model: %s, effort: %s, agents: %s)", model, effort, agentCount))
				if len(tools) > 0 {
					fmt.Fprintf(os.Stderr, "  Tools: %s\n", strings.Join(tools, ", "))
				}
			}

			spin := ui.NewSpinner("Running multi-agent query...")
			start := time.Now()

			ctx := context.Background()
			resp, err := provider.CompleteMultiAgent(ctx, &llm.MultiAgentRequest{
				Model:  model,
				Prompt: query,
				Effort: effort,
				Tools:  tools,
			})
			elapsed := time.Since(start)

			if err != nil {
				spin.Stop(false, fmt.Sprintf("Failed: %v", err))
				return err
			}
			spin.Stop(true, fmt.Sprintf("Complete (%.1fs, %d tokens)", elapsed.Seconds(), resp.TotalTokens))

			if jsonOutput {
				enc := json.NewEncoder(os.Stdout)
				enc.SetIndent("", "  ")
				return enc.Encode(MultiAgentResult{
					Query:        query,
					Model:        model,
					Effort:       effort,
					Text:         resp.Text,
					InputTokens:  resp.InputTokens,
					OutputTokens: resp.OutputTokens,
					TotalTokens:  resp.TotalTokens,
					DurationSecs: elapsed.Seconds(),
				})
			}

			fmt.Print(resp.Text)
			if !strings.HasSuffix(resp.Text, "\n") {
				fmt.Println()
			}
			return nil
		},
	}

	cmd.Flags().StringVar(&model, "model", "", "Model override (default: grok-4.20-multi-agent-beta-0309)")
	cmd.Flags().StringVar(&effort, "effort", "", "Agent effort: low/medium (4 agents), high/xhigh (16 agents)")
	cmd.Flags().StringSliceVar(&tools, "tools", nil, "Server-side tools: web_search, x_search, code_execution")
	cmd.Flags().BoolVar(&jsonOutput, "json", false, "Structured JSON output")
	cmd.Flags().BoolVarP(&verbose, "verbose", "v", false, "Show details on stderr")
	cmd.Flags().StringVar(&queryFile, "query-file", "", "Read query from file")

	return cmd
}
