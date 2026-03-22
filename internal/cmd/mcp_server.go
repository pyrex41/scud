package cmd

import (
	mcpserver "github.com/reuben/scud/internal/mcp"
	"github.com/spf13/cobra"
)

func NewMCPServerCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "mcp-server",
		Short: "Start MCP stdio server for tool integration",
		Long: `Starts a Model Context Protocol (MCP) server over stdio,
exposing scud commands as tools for AI agents (Claude Code, Cowork, etc).

Control which tools are exposed via the SCUD_TOOLS env var:
  core (default): warmup, next, show, set-status, stats, commit
  full: core + list, heavy, create
  Custom: SCUD_TOOLS=next,show,heavy`,
		RunE: func(cmd *cobra.Command, args []string) error {
			return mcpserver.Serve(cmd.Root().Version)
		},
	}
}
