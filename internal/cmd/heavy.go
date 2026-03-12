package cmd

import (
	"fmt"
	"os"
	"strings"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/heavy"
	"github.com/reuben/scud/internal/storage"
	"github.com/spf13/cobra"
)

func NewHeavyCmd() *cobra.Command {
	var (
		model        string
		debate       int
		concurrency  int
		timeout      int
		verbose      bool
		jsonOutput   bool
		queryFile    string
		native       bool
		nativeEffort string
		nativeTools  []string
	)

	cmd := &cobra.Command{
		Use:   "heavy [query...]",
		Short: "Multi-agent reasoning ensemble (16 agents)",
		Long: `Heavy mode runs a 16-agent reasoning ensemble: Captain routes the query
to relevant specialists, agents analyze in parallel, and Captain synthesizes
a unified answer. Optional debate rounds improve quality through critique cycles.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			// Resolve query
			query, err := resolveQuery(args, queryFile)
			if err != nil {
				return err
			}

			// Load config best-effort (heavy works without .scud directory)
			cfg := loadConfigBestEffort()

			// Resolve working directory
			cwd, _ := os.Getwd()

			opts := heavy.RunOpts{
				Query:        query,
				Model:        model,
				Concurrency:  concurrency,
				DebateRounds: debate,
				Verbose:      verbose,
				JSON:         jsonOutput,
				WorkingDir:   cwd,
				TimeoutSecs:  timeout,
				Native:       native,
				NativeEffort: nativeEffort,
				NativeTools:  nativeTools,
			}

			result, err := heavy.Run(cmd.Context(), cfg, opts)
			if err != nil {
				return err
			}

			return heavy.PrintResult(result, jsonOutput)
		},
	}

	cmd.Flags().StringVar(&model, "model", "", "Model override for all agents")
	cmd.Flags().IntVar(&debate, "debate", 0, "Number of debate rounds")
	cmd.Flags().IntVar(&concurrency, "concurrency", 0, "Max parallel agents (default 4)")
	cmd.Flags().IntVar(&timeout, "timeout", 0, "Per-agent timeout in seconds (default 300)")
	cmd.Flags().BoolVarP(&verbose, "verbose", "v", false, "Show agent details on stderr")
	cmd.Flags().BoolVar(&jsonOutput, "json", false, "Structured JSON output")
	cmd.Flags().StringVar(&queryFile, "query-file", "", "Read query from file")
	cmd.Flags().BoolVar(&native, "native", false, "Use xAI native multi-agent model instead of rho ensemble")
	cmd.Flags().StringVar(&nativeEffort, "effort", "", "Native agent effort: low/medium (4 agents), high/xhigh (16 agents)")
	cmd.Flags().StringSliceVar(&nativeTools, "tools", nil, "Native server-side tools: web_search, x_search, code_execution")

	return cmd
}

func resolveQuery(args []string, queryFile string) (string, error) {
	if queryFile != "" {
		data, err := os.ReadFile(queryFile)
		if err != nil {
			return "", fmt.Errorf("reading query file: %w", err)
		}
		return strings.TrimSpace(string(data)), nil
	}
	if len(args) == 0 {
		return "", fmt.Errorf("query required: provide as arguments or use --query-file")
	}
	return strings.Join(args, " "), nil
}

func loadConfigBestEffort() *config.Config {
	cwd, err := os.Getwd()
	if err != nil {
		return config.Default()
	}
	root, err := storage.FindRoot(cwd)
	if err != nil {
		return config.Default()
	}
	store := storage.New(root)
	cfg, err := config.Load(store.ScudDir())
	if err != nil {
		return config.Default()
	}
	return cfg
}
