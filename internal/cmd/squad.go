package cmd

import (
	"os"

	"github.com/reuben/scud/internal/heavy"
	"github.com/spf13/cobra"
)

func NewSquadCmd() *cobra.Command {
	var (
		workers      int
		workerModel  string
		captainModel string
		concurrency  int
		debate       int
		timeout      int
		verbose      bool
		jsonOutput   bool
		queryFile    string
	)

	cmd := &cobra.Command{
		Use:   "squad [query...]",
		Short: "Captain + N clone workers (one shared model, typically local)",
		Long: `Squad runs one smart captain and N identical worker clones, all using the
same --worker-model. The captain synthesizes their independent takes. Unlike
` + "`scud heavy`" + ` there is no LLM-driven routing step, so fewer things can flake.

Designed for pairing a capable captain (e.g. claude-sonnet) with a swarm of a
cheap or local model (e.g. llama-cpp://gemma-4-12b-it). rho's llama.cpp
lifecycle is shared across the squad — all N workers hit the same server.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			query, err := resolveQuery(args, queryFile)
			if err != nil {
				return err
			}
			cfg := loadConfigBestEffort()
			cwd, _ := os.Getwd()

			opts := heavy.SquadOpts{
				Query:        query,
				Workers:      workers,
				WorkerModel:  workerModel,
				CaptainModel: captainModel,
				Concurrency:  concurrency,
				DebateRounds: debate,
				Verbose:      verbose,
				JSON:         jsonOutput,
				WorkingDir:   cwd,
				TimeoutSecs:  timeout,
			}
			result, err := heavy.RunSquad(cmd.Context(), cfg, opts)
			if err != nil {
				return err
			}
			return heavy.PrintResult(result, jsonOutput)
		},
	}

	cmd.Flags().IntVar(&workers, "workers", 4, "Number of worker clones")
	cmd.Flags().StringVar(&workerModel, "worker-model", "", "Model for every worker (required, e.g. llama-cpp://gemma-4-12b)")
	cmd.Flags().StringVar(&captainModel, "captain-model", "", "Model for captain synthesis (default: config synthesis model)")
	cmd.Flags().IntVar(&concurrency, "concurrency", 0, "Max parallel workers (default: all at once)")
	cmd.Flags().IntVar(&debate, "debate", 0, "Number of debate rounds")
	cmd.Flags().IntVar(&timeout, "timeout", 0, "Per-worker timeout in seconds (default 300)")
	cmd.Flags().BoolVarP(&verbose, "verbose", "v", false, "Show worker details on stderr")
	cmd.Flags().BoolVar(&jsonOutput, "json", false, "Structured JSON output")
	cmd.Flags().StringVar(&queryFile, "query-file", "", "Read query from file")

	_ = cmd.MarkFlagRequired("worker-model")
	return cmd
}
