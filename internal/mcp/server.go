package mcp

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"strings"
	"time"

	"github.com/mark3labs/mcp-go/mcp"
	"github.com/mark3labs/mcp-go/server"
	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/storage"
	"github.com/reuben/scud/pkg/heavy"
	"github.com/reuben/scud/pkg/model"
)

const maxResponseLen = 4096

// Serve starts an MCP stdio server exposing scud tools.
func Serve(version string) error {
	s := server.NewMCPServer("scud", version)
	registerTools(s)
	return server.ServeStdio(s)
}

func registerTools(s *server.MCPServer) {
	tier := os.Getenv("SCUD_TOOLS")
	if tier == "" {
		tier = "core"
	}

	// Core tools — always registered
	s.AddTool(toolWarmup(), handleWarmup)
	s.AddTool(toolNext(), handleNext)
	s.AddTool(toolShow(), handleShow)
	s.AddTool(toolSetStatus(), handleSetStatus)
	s.AddTool(toolStats(), handleStats)
	s.AddTool(toolCommit(), handleCommit)

	if tier == "full" || tier == "all" || containsTool(tier, "list") {
		s.AddTool(toolList(), handleList)
	}
	if tier == "full" || tier == "all" || containsTool(tier, "heavy") {
		s.AddTool(toolHeavy(), handleHeavy)
	}
	if tier == "full" || tier == "all" || containsTool(tier, "create") {
		s.AddTool(toolCreate(), handleCreate)
	}

	// Custom comma-separated list — add individual tools
	if tier != "core" && tier != "full" && tier != "all" {
		for _, name := range strings.Split(tier, ",") {
			switch strings.TrimSpace(name) {
			case "warmup", "next", "show", "set-status", "stats", "commit":
				// already registered as core
			case "list":
				s.AddTool(toolList(), handleList)
			case "heavy":
				s.AddTool(toolHeavy(), handleHeavy)
			case "create":
				s.AddTool(toolCreate(), handleCreate)
			}
		}
	}
}

func containsTool(tier, name string) bool {
	for _, t := range strings.Split(tier, ",") {
		if strings.TrimSpace(t) == name {
			return true
		}
	}
	return false
}

// --- Tool Definitions ---

func toolWarmup() mcp.Tool {
	return mcp.NewTool("scud_warmup",
		mcp.WithDescription("Session orientation: show project status, recent git history, and next task"),
	)
}

func toolNext() mcp.Tool {
	return mcp.NewTool("scud_next",
		mcp.WithDescription("Find the next available task based on DAG dependencies"),
	)
}

func toolShow() mcp.Tool {
	return mcp.NewTool("scud_show",
		mcp.WithDescription("Show details for a specific task"),
		mcp.WithString("id", mcp.Required(), mcp.Description("Task ID, e.g. '1' or '1.2'")),
	)
}

func toolSetStatus() mcp.Tool {
	return mcp.NewTool("scud_set_status",
		mcp.WithDescription("Update the status of a task"),
		mcp.WithString("id", mcp.Required(), mcp.Description("Task ID")),
		mcp.WithString("status", mcp.Required(), mcp.Description("New status: pending, in-progress, done, blocked, failed")),
	)
}

func toolStats() mcp.Tool {
	return mcp.NewTool("scud_stats",
		mcp.WithDescription("Show task completion statistics"),
	)
}

func toolCommit() mcp.Tool {
	return mcp.NewTool("scud_commit",
		mcp.WithDescription("Task-aware git commit with auto-prefixed task ID"),
		mcp.WithString("message", mcp.Description("Commit message (optional, uses task title if empty)")),
	)
}

func toolList() mcp.Tool {
	return mcp.NewTool("scud_list",
		mcp.WithDescription("List all tasks with status"),
		mcp.WithString("status", mcp.Description("Filter by status: pending, in-progress, done")),
	)
}

func toolHeavy() mcp.Tool {
	return mcp.NewTool("scud_heavy",
		mcp.WithDescription("Run multi-agent reasoning ensemble"),
		mcp.WithString("query", mcp.Required(), mcp.Description("The question or task to analyze")),
		mcp.WithString("mode", mcp.Description("ensemble (default), native, hybrid")),
		mcp.WithString("model_agents", mcp.Description("Model for agent execution (e.g. grok-4.1-fast)")),
		mcp.WithString("model_synthesis", mcp.Description("Model for synthesis (e.g. grok-4.20-reasoning)")),
	)
}

func toolCreate() mcp.Tool {
	return mcp.NewTool("scud_create",
		mcp.WithDescription("Create a new task"),
		mcp.WithString("title", mcp.Required(), mcp.Description("Task title")),
	)
}

// --- Handlers ---

func getStoreAndConfig() (*storage.Storage, *config.Config, error) {
	cwd, err := os.Getwd()
	if err != nil {
		return nil, nil, fmt.Errorf("getwd: %w", err)
	}
	root, err := storage.FindRoot(cwd)
	if err != nil {
		return nil, nil, fmt.Errorf("no .scud directory found (run 'scud init' first)")
	}
	store := storage.New(root)
	cfg, err := config.Load(store.ScudDir())
	if err != nil {
		cfg = config.Default()
	}
	return store, cfg, nil
}

func textResult(text string) *mcp.CallToolResult {
	if len(text) > maxResponseLen {
		text = text[:maxResponseLen] + "\n... (truncated)"
	}
	return &mcp.CallToolResult{
		Content: []mcp.Content{mcp.NewTextContent(text)},
	}
}

func errResult(err error) *mcp.CallToolResult {
	return &mcp.CallToolResult{
		Content: []mcp.Content{mcp.NewTextContent(err.Error())},
		IsError: true,
	}
}

func handleWarmup(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {
	store, _, err := getStoreAndConfig()
	if err != nil {
		return errResult(err), nil
	}
	phases, err := store.LoadPhases()
	if err != nil {
		return errResult(err), nil
	}

	var sb strings.Builder
	for name, phase := range phases {
		total := len(phase.Tasks)
		done := 0
		inProgress := 0
		for _, t := range phase.Tasks {
			switch t.Status {
			case model.Done:
				done++
			case model.InProgress:
				inProgress++
			}
		}
		sb.WriteString(fmt.Sprintf("Phase: %s | %d tasks (%d done, %d in-progress, %d remaining)\n",
			name, total, done, inProgress, total-done-inProgress))
	}

	// Find next task
	for _, phase := range phases {
		if t := phase.FindNextTask(phases); t != nil {
			sb.WriteString(fmt.Sprintf("\nNext task: %s - %s\n", t.ID, t.Title))
			break
		}
	}

	return textResult(sb.String()), nil
}

func handleNext(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {
	store, _, err := getStoreAndConfig()
	if err != nil {
		return errResult(err), nil
	}
	phases, err := store.LoadPhases()
	if err != nil {
		return errResult(err), nil
	}
	for _, phase := range phases {
		if t := phase.FindNextTask(phases); t != nil {
			return textResult(fmt.Sprintf("Next task: %s\nTitle: %s\nStatus: %s\nPriority: %s\nComplexity: %d\n\n%s",
				t.ID, t.Title, t.Status, t.Priority, t.Complexity, t.Description)), nil
		}
	}
	return textResult("No tasks available. All tasks are done or blocked."), nil
}

func handleShow(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {
	args := req.GetArguments()
	id, _ := args["id"].(string)
	if id == "" {
		return errResult(fmt.Errorf("id is required")), nil
	}

	store, _, err := getStoreAndConfig()
	if err != nil {
		return errResult(err), nil
	}
	phases, err := store.LoadPhases()
	if err != nil {
		return errResult(err), nil
	}
	for _, phase := range phases {
		for _, t := range phase.Tasks {
			if t.ID == id {
				var sb strings.Builder
				sb.WriteString(fmt.Sprintf("ID: %s\nTitle: %s\nStatus: %s\nPriority: %s\nComplexity: %d\n",
					t.ID, t.Title, t.Status, t.Priority, t.Complexity))
				if len(t.Dependencies) > 0 {
					sb.WriteString(fmt.Sprintf("Dependencies: %s\n", strings.Join(t.Dependencies, ", ")))
				}
				if t.Description != "" {
					sb.WriteString(fmt.Sprintf("\nDescription:\n%s\n", t.Description))
				}
				if t.Details != "" {
					sb.WriteString(fmt.Sprintf("\nDetails:\n%s\n", t.Details))
				}
				return textResult(sb.String()), nil
			}
		}
	}
	return errResult(fmt.Errorf("task %q not found", id)), nil
}

func handleSetStatus(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {
	args := req.GetArguments()
	id, _ := args["id"].(string)
	status, _ := args["status"].(string)
	if id == "" || status == "" {
		return errResult(fmt.Errorf("id and status are required")), nil
	}

	store, _, err := getStoreAndConfig()
	if err != nil {
		return errResult(err), nil
	}

	err = store.UpdatePhase("", func(phase *model.Phase) error {
		for _, t := range phase.Tasks {
			if t.ID == id {
				t.Status = model.TaskStatus(status)
				return nil
			}
		}
		return fmt.Errorf("task %q not found", id)
	})
	if err != nil {
		return errResult(err), nil
	}
	return textResult(fmt.Sprintf("Task %s status set to %s", id, status)), nil
}

func handleStats(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {
	store, _, err := getStoreAndConfig()
	if err != nil {
		return errResult(err), nil
	}
	phases, err := store.LoadPhases()
	if err != nil {
		return errResult(err), nil
	}

	var sb strings.Builder
	totalAll, doneAll := 0, 0
	for name, phase := range phases {
		stats := phase.Stats()
		totalAll += stats.Total
		doneAll += stats.Done
		sb.WriteString(fmt.Sprintf("%s: %d/%d done (%.0f%%)\n",
			name, stats.Done, stats.Total,
			float64(stats.Done)/float64(max(stats.Total, 1))*100))
	}
	if len(phases) > 1 {
		sb.WriteString(fmt.Sprintf("\nOverall: %d/%d done (%.0f%%)\n",
			doneAll, totalAll,
			float64(doneAll)/float64(max(totalAll, 1))*100))
	}
	return textResult(sb.String()), nil
}

func handleCommit(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {
	args := req.GetArguments()
	message, _ := args["message"].(string)

	cmdArgs := []string{"commit"}
	if message != "" {
		cmdArgs = append(cmdArgs, "-m", message)
	}
	out, err := exec.CommandContext(ctx, "scud", cmdArgs...).CombinedOutput()
	if err != nil {
		return errResult(fmt.Errorf("%s: %s", err, string(out))), nil
	}
	return textResult(string(out)), nil
}

func handleList(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {
	args := req.GetArguments()
	statusFilter, _ := args["status"].(string)

	store, _, err := getStoreAndConfig()
	if err != nil {
		return errResult(err), nil
	}
	phases, err := store.LoadPhases()
	if err != nil {
		return errResult(err), nil
	}

	var sb strings.Builder
	for _, phase := range phases {
		for _, t := range phase.Tasks {
			if statusFilter != "" && string(t.Status) != statusFilter {
				continue
			}
			sb.WriteString(fmt.Sprintf("%-8s %-12s %s\n", t.ID, t.Status, t.Title))
		}
	}
	if sb.Len() == 0 {
		return textResult("No tasks found."), nil
	}
	return textResult(sb.String()), nil
}

func handleHeavy(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {
	args := req.GetArguments()
	query, _ := args["query"].(string)
	if query == "" {
		return errResult(fmt.Errorf("query is required")), nil
	}

	mode, _ := args["mode"].(string)
	modelAgents, _ := args["model_agents"].(string)
	modelSynthesis, _ := args["model_synthesis"].(string)

	_, cfg, _ := getStoreAndConfig()
	if cfg == nil {
		cfg = config.Default()
	}

	cwd, _ := os.Getwd()
	opts := heavy.RunOpts{
		Query:          query,
		Mode:           mode,
		ModelAgents:    modelAgents,
		ModelSynthesis: modelSynthesis,
		WorkingDir:     cwd,
		TimeoutSecs:    300,
	}

	result, err := heavy.Run(ctx, cfg, opts)
	if err != nil {
		return errResult(err), nil
	}
	return textResult(result.Synthesis), nil
}

func handleCreate(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {
	args := req.GetArguments()
	title, _ := args["title"].(string)
	if title == "" {
		return errResult(fmt.Errorf("title is required")), nil
	}

	store, _, err := getStoreAndConfig()
	if err != nil {
		return errResult(err), nil
	}

	var newID string
	err = store.UpdatePhase("", func(phase *model.Phase) error {
		newID = phase.NextTaskID()
		t := &model.Task{
			ID:        newID,
			Title:     title,
			Status:    model.Pending,
			Priority:  model.Medium,
			CreatedAt: time.Now().UTC().Format(time.RFC3339),
		}
		phase.Tasks = append(phase.Tasks, t)
		return nil
	})
	if err != nil {
		return errResult(err), nil
	}
	return textResult(fmt.Sprintf("Created task %s: %s", newID, title)), nil
}
