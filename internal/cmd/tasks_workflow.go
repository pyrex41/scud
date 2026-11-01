package cmd

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"time"

	"github.com/reuben/scud/internal/inputio"
	"github.com/reuben/scud/internal/taskapi"
	"github.com/reuben/scud/pkg/model"
	"github.com/spf13/cobra"
)

func taskService() (*taskapi.Service, error) {
	cwd, e := os.Getwd()
	if e != nil {
		return nil, e
	}
	return taskapi.Open(cwd)
}
func taskOutput(cmd *cobra.Command, v any) error {
	enc := json.NewEncoder(cmd.OutOrStdout())
	enc.SetIndent("", "  ")
	return enc.Encode(v)
}
func taskJSON(cmd *cobra.Command) { cmd.Flags().Bool("json", false, "Output JSON (the default)") }
func taskDecode[T any](cmd *cobra.Command, path string) (T, error) {
	var r io.Reader = cmd.InOrStdin()
	if path != "" && path != "-" {
		f, e := os.Open(path)
		if e != nil {
			var zero T
			return zero, e
		}
		defer f.Close()
		r = f
	}
	b, err := inputio.ReadAll(cmd.Context(), r, taskapi.MaxJSONBytes)
	if err != nil {
		var zero T
		return zero, err
	}
	return taskapi.Decode[T](bytes.NewReader(b))
}
func NewTaskImportCmd() *cobra.Command {
	var policy, revision string
	c := &cobra.Command{Use: "import [file|-]", Short: "Import a versioned JSON task document", Args: cobra.MaximumNArgs(1), RunE: func(c *cobra.Command, a []string) error {
		path := "-"
		if len(a) > 0 {
			path = a[0]
		}
		d, e := taskDecode[taskapi.Document](c, path)
		if e != nil {
			return e
		}
		s, e := taskService()
		if e != nil {
			return e
		}
		v, e := s.Import(c.Context(), d, taskapi.ImportOptions{Policy: policy, ExpectedRevision: revision})
		if e != nil {
			return e
		}
		return taskOutput(c, v)
	}}
	c.Long = `Import a schema_version: 1 JSON document from a file or stdin (default).
Validate the document before writing. Existing phase tags are refused unless a
policy is explicit: merge appends tasks to matching phase metadata and rejects
duplicate IDs; replace replaces only phases named in the input.
--if-revision is an optimistic revision guard, not a lease or task claim.
Successful imports return the resulting status snapshot as JSON.`
	c.Example = `  scud validate --input proposal.json
  scud import proposal.json
  scud import exported-tasks.json --policy replace
  cat proposal.json | scud import -`
	c.Flags().StringVar(&policy, "policy", "", "Collision policy: merge or replace")
	c.Flags().StringVar(&revision, "if-revision", "", "Only import if the current revision matches")
	taskJSON(c)
	return c
}
func NewTaskListCmd() *cobra.Command {
	var tag string
	c := &cobra.Command{Use: "list", Short: "Export tasks as an importable JSON document", Args: cobra.NoArgs, RunE: func(c *cobra.Command, _ []string) error {
		s, e := taskService()
		if e != nil {
			return e
		}
		v, e := s.Snapshot(c.Context())
		if e != nil {
			return e
		}
		d, e := taskapi.FilterDocument(taskapi.Document{SchemaVersion: 1, Phases: v.Phases}, tag)
		if e != nil {
			return e
		}
		return taskOutput(c, d)
	}}
	c.Long = `Export all phases by default as an importable schema_version: 1 JSON document.
--tag includes the entire named phase and its transitive prerequisite phases,
not just individual prerequisite tasks. Dependency edges are preserved so the
export can validate and import independently. This is not a status snapshot.`
	c.Example = `  scud list > exported-tasks.json
  scud list --tag app > app-with-prerequisites.json`
	c.Flags().StringVar(&tag, "tag", "", "Phase tag (includes whole prerequisite phases)")
	taskJSON(c)
	return c
}
func NewTaskShowCmd() *cobra.Command {
	var tag string
	c := &cobra.Command{Use: "show ID", Short: "Show one task by ID", Args: cobra.ExactArgs(1), RunE: func(c *cobra.Command, a []string) error {
		s, e := taskService()
		if e != nil {
			return e
		}
		t, r, e := s.ReadTask(c.Context(), taskapi.Ref{Tag: tag, ID: a[0]})
		if e != nil {
			return e
		}
		return taskOutput(c, struct {
			Ref  taskapi.Ref `json:"ref"`
			Task *model.Task `json:"task"`
		}{r, t})
	}}
	c.Long = `Read one task as a JSON wrapper with "ref" (tag and id) and "task" fields.
Use phase:id or --tag to qualify the ID; an unqualified ID must be unambiguous.
Reading does not claim, update, or execute the task.`
	c.Example = `  scud show main:1
  scud show 1 --tag main`
	c.Flags().StringVar(&tag, "tag", "", "Phase tag")
	taskJSON(c)
	return c
}
func NewTaskUpdateCmd() *cobra.Command {
	var tag, rev, input, status, title, description, details, strategy, priority, assignedTo string
	var dependencies []string
	var complexity int
	c := &cobra.Command{Use: "update ID", Short: "Update selected task fields", Args: cobra.ExactArgs(1), RunE: func(c *cobra.Command, a []string) error {
		var p taskapi.Patch
		if c.Flags().Changed("input") {
			var e error
			p, e = taskDecode[taskapi.Patch](c, input)
			if e != nil {
				return e
			}
			for _, n := range []string{"status", "title", "description", "details", "test-strategy", "priority", "complexity", "depends-on", "assigned-to"} {
				if c.Flags().Changed(n) {
					return fmt.Errorf("--input cannot be combined with patch flags")
				}
			}
		} else {
			if c.Flags().Changed("status") {
				v := model.TaskStatus(status)
				p.Status = &v
			}
			if c.Flags().Changed("title") {
				p.Title = &title
			}
			if c.Flags().Changed("description") {
				p.Description = &description
			}
			if c.Flags().Changed("details") {
				p.Details = &details
			}
			if c.Flags().Changed("test-strategy") {
				p.TestStrategy = &strategy
			}
			if c.Flags().Changed("priority") {
				v := model.Priority(priority)
				p.Priority = &v
			}
			if c.Flags().Changed("complexity") {
				p.Complexity = &complexity
			}
			if c.Flags().Changed("depends-on") {
				p.Dependencies = &dependencies
			}
			if c.Flags().Changed("assigned-to") {
				p.AssignedTo = &assignedTo
			}
		}
		s, e := taskService()
		if e != nil {
			return e
		}
		v, e := s.Update(c.Context(), taskapi.Ref{Tag: tag, ID: a[0]}, p, rev)
		if e != nil {
			return e
		}
		return taskOutput(c, v)
	}}
	c.Long = `Update a task using patch flags OR --input JSON, never both. Use phase:id
or --tag to qualify an ambiguous ID. Omitted fields remain unchanged.

JSON patch fields: status, title, description, details, test_strategy, priority,
complexity, dependencies, assigned_to. --depends-on replaces the entire dependency
list (it does not append); --depends-on= or JSON dependencies: [] clears it.
An empty assigned_to clears the assignee.

Statuses: pending, in-progress, done, failed, blocked, review, expanded, deferred,
cancelled. Priorities: critical, high, medium, low. Status is user-reported,
not proof of verification. Invalid changes leave the task document untouched.
--if-revision compares the current project revision before writing; it is an
optimistic guard, not a lease or exclusive claim. Returns the updated snapshot.`
	c.Example = `  scud update main:1 --status in-progress
  scud update main:2 --depends-on main:1 --assigned-to alice
  scud update main:2 --depends-on=
  printf '%s\n' '{"test_strategy":"Run tests","dependencies":[]}' | scud update main:2 --input -
  revision=$(scud status | jq -r .revision)
  scud update main:1 --status review --if-revision "$revision"`
	f := c.Flags()
	f.StringVar(&tag, "tag", "", "Phase tag")
	f.StringVar(&rev, "if-revision", "", "Expected project revision (optimistic guard, not a lease)")
	f.StringVar(&input, "input", "", "JSON patch file or - for stdin")
	f.StringVar(&status, "status", "", "pending, in-progress, done, failed, blocked, review, expanded, deferred, cancelled")
	f.StringVar(&title, "title", "", "Title")
	f.StringVar(&description, "description", "", "Description")
	f.StringVar(&details, "details", "", "Details")
	f.StringVar(&strategy, "test-strategy", "", "Test strategy")
	f.StringVar(&priority, "priority", "", "critical, high, medium, low")
	f.IntVar(&complexity, "complexity", 0, "Complexity")
	f.StringSliceVar(&dependencies, "depends-on", nil, "Replace dependencies (comma-separated; use --depends-on= to clear)")
	f.StringVar(&assignedTo, "assigned-to", "", "Assignee (empty clears)")
	taskJSON(c)
	return c
}
func NewTaskStatusCmd() *cobra.Command {
	var watch bool
	var interval time.Duration
	c := &cobra.Command{Use: "status", Short: "Show or watch current task readiness and status", Args: cobra.NoArgs, RunE: func(c *cobra.Command, _ []string) error {
		s, e := taskService()
		if e != nil {
			return e
		}
		last := ""
		for {
			v, e := s.Snapshot(c.Context())
			if e != nil {
				return e
			}
			if v.Revision != last {
				if watch {
					if e = json.NewEncoder(c.OutOrStdout()).Encode(v); e != nil {
						return e
					}
				} else if e = taskOutput(c, v); e != nil {
					return e
				}
				last = v.Revision
			}
			if !watch {
				return nil
			}
			timer := time.NewTimer(taskapi.PollInterval(interval))
			select {
			case <-c.Context().Done():
				timer.Stop()
				return nil
			case <-timer.C:
			}
		}
	}}
	c.Long = `Read a JSON snapshot containing schema_version, revision, phases, summary,
ready references and blocked reasons. Readiness accounts for cross-phase and
inherited dependencies; workflow status is not a verification certificate.

--watch emits an initial snapshot and another on each observed revision change
as JSON lines. This is polling, not an event log or heartbeat, and intermediate
revisions may be missed. --interval is clamped to at least 100ms. Ctrl-C stops
watching; no task is claimed or executed.`
	c.Example = `  scud status
  scud status | jq '{revision, summary, ready, blocked}'
  scud status --watch --interval 500ms`
	c.Flags().BoolVar(&watch, "watch", false, "Poll snapshots and emit observed revision changes as JSON lines")
	c.Flags().DurationVar(&interval, "interval", time.Second, "Poll interval (minimum 100ms)")
	taskJSON(c)
	return c
}
func NewTaskNextCmd() *cobra.Command {
	c := &cobra.Command{Use: "next", Short: "Show the first ready task reference, or null", Args: cobra.NoArgs, RunE: func(c *cobra.Command, _ []string) error {
		s, e := taskService()
		if e != nil {
			return e
		}
		v, e := s.Snapshot(c.Context())
		if e != nil {
			return e
		}
		var r *taskapi.Ref
		if len(v.Ready) > 0 {
			r = &v.Ready[0]
		}
		return taskOutput(c, r)
	}}
	c.Long = `Return only the FIRST ready reference as JSON {"tag":"...","id":"..."},
or null when no task is ready (still exit 0). This is neither a full task nor
the complete ready list: use show for task details or status for all ready refs.
Readiness includes inherited and cross-phase dependencies. No task is claimed,
leased, updated, or executed; concurrent callers can receive the same reference.`
	c.Example = `  scud next
  scud status | jq .ready`
	taskJSON(c)
	return c
}
func NewTaskValidateCmd() *cobra.Command {
	var input string
	c := &cobra.Command{Use: "validate", Short: "Validate current tasks or an input document without writing", Args: cobra.NoArgs, RunE: func(c *cobra.Command, _ []string) error {
		if c.Flags().Changed("input") {
			d, e := taskDecode[taskapi.Document](c, input)
			if e != nil {
				return e
			}
			v, e := taskapi.ValidateDocument(d)
			if e != nil {
				return e
			}
			return taskOutput(c, v)
		}
		s, e := taskService()
		if e != nil {
			return e
		}
		v, e := s.Validate(c.Context())
		if e != nil {
			return e
		}
		return taskOutput(c, v)
	}}
	c.Long = `Validate current project tasks, or a schema_version: 1 JSON document supplied
with --input (file or - for stdin), without writing task state. Input validation
can run before project initialization. Checks include references, cycles, duplicate
IDs and parent relationships. Unknown JSON fields, duplicate keys and trailing
values are rejected. Review a parse proposal here before importing it.`
	c.Example = `  scud validate
  scud validate --input proposal.json
  cat proposal.json | scud validate --input -`
	c.Flags().StringVar(&input, "input", "", "Validate a JSON document without writing (file or -)")
	taskJSON(c)
	return c
}
