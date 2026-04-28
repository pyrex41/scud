package cmd

import (
	"context"
	"fmt"
	"os"
	"strings"

	"github.com/reuben/scud/pkg/attractor"
	"github.com/spf13/cobra"
)

// NewAttractorCmd returns the top-level attractor command with its subcommands.
func NewAttractorCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "attractor",
		Short: "Pipeline engine for multi-step AI workflows",
	}

	cmd.AddCommand(
		newAttractorRunCmd(),
		newAttractorValidateCmd(),
		newAttractorImportCmd(),
		newAttractorExportCmd(),
	)

	return cmd
}

func newAttractorRunCmd() *cobra.Command {
	var resume string
	var headless, simulated bool
	var model, provider string

	cmd := &cobra.Command{
		Use:   "run <dotfile>",
		Short: "Execute a pipeline from a DOT file",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			content, err := os.ReadFile(args[0])
			if err != nil {
				return fmt.Errorf("reading %s: %w", args[0], err)
			}

			graph, err := attractor.ParseDOT(string(content))
			if err != nil {
				return fmt.Errorf("parsing DOT: %w", err)
			}

			if issues := attractor.Validate(graph); len(issues) > 0 {
				for _, issue := range issues {
					fmt.Fprintf(os.Stderr, "warning: %s\n", issue)
				}
			}

			opts := attractor.RunnerOpts{
				WorkDir:    ".",
				Model:      model,
				Provider:   provider,
				Headless:   headless,
				Simulated:  simulated,
				ResumePath: resume,
			}

			runner := attractor.NewRunner(graph, opts)

			// Resume from checkpoint if specified
			if resume != "" {
				cp, err := attractor.LoadCheckpoint(resume)
				if err != nil {
					return fmt.Errorf("loading checkpoint: %w", err)
				}
				runner.Checkpoint = cp
			}

			runner.Checkpoint.GraphFile = args[0]
			return runner.Run(context.Background())
		},
	}

	cmd.Flags().StringVar(&resume, "resume", "", "checkpoint file to resume from")
	cmd.Flags().BoolVar(&headless, "headless", false, "run without interactive prompts")
	cmd.Flags().BoolVar(&simulated, "simulated", false, "simulate execution (dry run)")
	cmd.Flags().StringVar(&model, "model", "", "AI model to use")
	cmd.Flags().StringVar(&provider, "provider", "", "AI provider to use")

	return cmd
}

func newAttractorValidateCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "validate <dotfile>",
		Short: "Validate a pipeline DOT file",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			content, err := os.ReadFile(args[0])
			if err != nil {
				return fmt.Errorf("reading %s: %w", args[0], err)
			}

			graph, err := attractor.ParseDOT(string(content))
			if err != nil {
				return fmt.Errorf("parsing DOT: %w", err)
			}

			issues := attractor.Validate(graph)
			if len(issues) == 0 {
				fmt.Println("pipeline is valid")
				return nil
			}

			for _, issue := range issues {
				fmt.Fprintf(os.Stderr, "  - %s\n", issue)
			}
			return fmt.Errorf("found %d issue(s)", len(issues))
		},
	}
}

func newAttractorImportCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "import <dotfile>",
		Short: "Import a DOT file and output SCG format",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			content, err := os.ReadFile(args[0])
			if err != nil {
				return fmt.Errorf("reading %s: %w", args[0], err)
			}

			graph, err := attractor.ParseDOT(string(content))
			if err != nil {
				return fmt.Errorf("parsing DOT: %w", err)
			}

			// Output as a simple SCG-like text representation
			var b strings.Builder
			b.WriteString("# Imported from: " + args[0] + "\n\n")
			b.WriteString("@nodes\n")
			b.WriteString("# id | label | type\n")
			for _, n := range graph.Nodes {
				b.WriteString(fmt.Sprintf("%s | %s | %s\n", n.ID, n.Label, n.Type))
			}
			if len(graph.Edges) > 0 {
				b.WriteString("\n@edges\n")
				b.WriteString("# from -> to\n")
				for _, e := range graph.Edges {
					line := fmt.Sprintf("%s -> %s", e.From, e.To)
					if e.Condition != "" {
						line += fmt.Sprintf(" | %s", e.Condition)
					}
					b.WriteString(line + "\n")
				}
			}
			fmt.Print(b.String())
			return nil
		},
	}
}

func newAttractorExportCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "export <dotfile>",
		Short: "Read a pipeline DOT file and re-export as canonical DOT",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			content, err := os.ReadFile(args[0])
			if err != nil {
				return fmt.Errorf("reading %s: %w", args[0], err)
			}

			graph, err := attractor.ParseDOT(string(content))
			if err != nil {
				return fmt.Errorf("parsing DOT: %w", err)
			}

			// Output as canonical DOT
			var b strings.Builder
			b.WriteString("digraph pipeline {\n")
			for _, n := range graph.Nodes {
				b.WriteString(fmt.Sprintf("    %s [label=%q", n.ID, n.Label))
				if n.Type != "" {
					b.WriteString(fmt.Sprintf(" type=%q", n.Type))
				}
				for k, v := range n.Properties {
					b.WriteString(fmt.Sprintf(" %s=%q", k, v))
				}
				b.WriteString("]\n")
			}
			for _, e := range graph.Edges {
				b.WriteString(fmt.Sprintf("    %s -> %s", e.From, e.To))
				if e.Label != "" || e.Condition != "" {
					b.WriteString(" [")
					var attrs []string
					if e.Label != "" {
						attrs = append(attrs, fmt.Sprintf("label=%q", e.Label))
					}
					if e.Condition != "" {
						attrs = append(attrs, fmt.Sprintf("condition=%q", e.Condition))
					}
					b.WriteString(strings.Join(attrs, " "))
					b.WriteString("]")
				}
				b.WriteString("\n")
			}
			b.WriteString("}\n")
			fmt.Print(b.String())
			return nil
		},
	}
}
