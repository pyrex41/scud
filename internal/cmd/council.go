package cmd

import (
	"os"

	"github.com/reuben/scud/pkg/heavy"
	"github.com/spf13/cobra"
)

func NewCouncilCmd() *cobra.Command {
	var (
		workerModels []string
		captainModel string
		concurrency  int
		debate       int
		timeout      int
		verbose      bool
		jsonOutput   bool
		queryFile    string
	)

	cmd := &cobra.Command{
		Use:   "council [query...]",
		Short: "Captain + heterogeneous workers (each worker on a different model)",
		Long: `Council runs one captain and N workers, where every worker uses a *different*
model. The captain synthesizes the heterogeneous takes. Useful when you want
genuine model-diversity (e.g. one local gemma, one claude, one grok) rather
than N clones of the same model.

Unlike ` + "`scud heavy`" + ` there is no LLM-driven routing step, so fewer things
can flake. The worker roster is exactly whatever you pass in --workers.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			query, err := resolveQuery(args, queryFile)
			if err != nil {
				return err
			}
			cfg := loadConfigBestEffort()
			cwd, _ := os.Getwd()

			opts := heavy.CouncilOpts{
				Query:        query,
				WorkerModels: workerModels,
				CaptainModel: captainModel,
				Concurrency:  concurrency,
				DebateRounds: debate,
				Verbose:      verbose,
				JSON:         jsonOutput,
				WorkingDir:   cwd,
				TimeoutSecs:  timeout,
			}
			result, err := heavy.RunCouncil(cmd.Context(), cfg, opts)
			if err != nil {
				return err
			}
			return heavy.PrintResult(result, jsonOutput)
		},
	}

	cmd.Flags().StringSliceVar(&workerModels, "workers", nil,
		"Comma-separated list of model ids — one worker per entry "+
			"(e.g. --workers=claude-sonnet,grok-4,llama-cpp://gemma-4-12b)")
	cmd.Flags().StringVar(&captainModel, "captain-model", "", "Model for captain synthesis (default: config synthesis model)")
	cmd.Flags().IntVar(&concurrency, "concurrency", 0, "Max parallel workers (default: all at once)")
	cmd.Flags().IntVar(&debate, "debate", 0, "Number of debate rounds")
	cmd.Flags().IntVar(&timeout, "timeout", 0, "Per-worker timeout in seconds (default 300)")
	cmd.Flags().BoolVarP(&verbose, "verbose", "v", false, "Show worker details on stderr")
	cmd.Flags().BoolVar(&jsonOutput, "json", false, "Structured JSON output")
	cmd.Flags().StringVar(&queryFile, "query-file", "", "Read query from file")

	_ = cmd.MarkFlagRequired("workers")
	return cmd
}
