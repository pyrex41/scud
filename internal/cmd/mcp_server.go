package cmd

import (
	"context"
	"errors"
	"fmt"
	"net"
	"net/http"
	"os"
	"time"

	mcpserver "github.com/reuben/scud/internal/mcp"
	"github.com/spf13/cobra"
)

func NewMCPServerCmd() *cobra.Command {
	var transport, listen string
	command := &cobra.Command{
		Use:   "mcp-server",
		Short: "Serve stateless MCP 2026-07-28 over stdio or local HTTP",
		Long: `Stateless MCP 2026-07-28: no initialization handshake or sessions.
Each request supplies protocol version and client capabilities in params._meta.
Use io.modelcontextprotocol/protocolVersion: "2026-07-28" and
io.modelcontextprotocol/clientCapabilities: {} for a client with no capabilities.
Discovery uses server/discover, not initialize; metadata is required per request.
Use stdio (default), or --transport http --listen 127.0.0.1:8080 (/mcp).
HTTP is loopback-only; put authenticated remote access behind an explicit proxy.
HTTP POST /mcp also requires Content-Type: application/json, Accept:
application/json, text/event-stream, matching Mcp-Protocol-Version and Mcp-Method
headers, and Mcp-Name for tool calls. No persistent session or legacy GET/SSE.

Control which tools are exposed via the SCUD_TOOLS env var:
  core (default): tasks, import, update, status, parse
  full: core + sb
  Custom: SCUD_TOOLS=tasks,status,sb

No worker execution, commit, or experimental ensemble tools are exposed.`,
		Example: `  scud mcp-server
  SCUD_TOOLS=full scud mcp-server
  SCUD_TOOLS=tasks,status scud mcp-server --transport http --listen 127.0.0.1:8080`,
		RunE: func(cmd *cobra.Command, args []string) error {
			root, err := os.Getwd()
			if err != nil {
				return err
			}
			selection := os.Getenv("SCUD_TOOLS")
			switch transport {
			case "stdio":
				input, err := mcpserver.OwnedInput(cmd.InOrStdin())
				if err != nil {
					return err
				}
				return mcpserver.Serve(cmd.Context(), cmd.Root().Version, selection, root, input, cmd.OutOrStdout())
			case "http":
				host, _, err := net.SplitHostPort(listen)
				if err != nil || net.ParseIP(host) == nil || !net.ParseIP(host).IsLoopback() {
					return fmt.Errorf("--listen must be a loopback IP:port (e.g. 127.0.0.1:8080)")
				}
				handler, err := mcpserver.HTTPHandler(cmd.Root().Version, selection, root)
				if err != nil {
					return err
				}
				listener, err := net.Listen("tcp", listen)
				if err != nil {
					return err
				}
				defer listener.Close()
				server := &http.Server{Handler: handler, ReadHeaderTimeout: 5 * time.Second, ReadTimeout: 30 * time.Second,
					BaseContext: func(net.Listener) context.Context { return cmd.Context() }}
				shutdownDone := make(chan struct{})
				stop := context.AfterFunc(cmd.Context(), func() {
					defer close(shutdownDone)
					ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
					defer cancel()
					if err := server.Shutdown(ctx); err != nil {
						_ = server.Close()
					}
				})
				defer stop()
				fmt.Fprintf(cmd.ErrOrStderr(), "Stateless MCP listening on http://%s/mcp\n", listener.Addr())
				err = server.Serve(listener)
				if cmd.Context().Err() != nil {
					// Await cancellation-aware handlers/child cleanup, not merely
					// listener closure, before main chooses its signal exit status.
					<-shutdownDone
					return cmd.Context().Err()
				}
				if errors.Is(err, http.ErrServerClosed) {
					return nil
				}
				return err
			default:
				return fmt.Errorf("--transport must be stdio or http")
			}
		},
	}
	command.Flags().StringVar(&transport, "transport", "stdio", "Stateless MCP transport: stdio or http")
	command.Flags().StringVar(&listen, "listen", "127.0.0.1:8080", "Loopback HTTP listen address")
	command.Args = cobra.NoArgs
	return command
}
