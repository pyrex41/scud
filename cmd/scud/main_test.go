package main

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"strings"
	"testing"

	"github.com/reuben/scud/internal/taskapi"
	"github.com/reuben/scud/pkg/model"
)

// The root's -C changes the process cwd, so these tests must not run in parallel.
func runRoot(t *testing.T, dir, input string, args ...string) (string, string, error) {
	t.Helper()
	cwd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	defer func() {
		if err := os.Chdir(cwd); err != nil {
			t.Errorf("restore cwd: %v", err)
		}
	}()
	root := newRoot()
	var out, stderr bytes.Buffer
	root.SetIn(strings.NewReader(input))
	root.SetOut(&out)
	root.SetErr(&stderr)
	root.SetArgs(append([]string{"-C", dir}, args...))
	err = root.Execute()
	return out.String(), stderr.String(), err
}
func decodeRoot[T any](t *testing.T, data string) T {
	t.Helper()
	var v T
	if err := json.Unmarshal([]byte(data), &v); err != nil {
		t.Fatalf("invalid JSON %q: %v", data, err)
	}
	return v
}
func mustRoot(t *testing.T, dir, input string, args ...string) string {
	t.Helper()
	out, stderr, err := runRoot(t, dir, input, args...)
	if err != nil {
		t.Fatalf("%v: %v; stdout=%q stderr=%q", args, err, out, stderr)
	}
	if stderr != "" {
		t.Fatalf("%v wrote stderr: %q", args, stderr)
	}
	return out
}

func TestPublicRootCommands(t *testing.T) {
	root := newRoot()
	var got []string
	for _, c := range root.Commands() {
		got = append(got, c.Name())
	}
	sort.Strings(got)
	want := []string{"experimental", "import", "init", "list", "mcp-server", "models", "next", "parse", "sb", "show", "status", "update", "validate"}
	sort.Strings(want)
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("public commands: got %v want %v", got, want)
	}
	dir := t.TempDir()
	for _, name := range []string{"run", "swarm", "commit", "roles", "tier"} {
		out, _, err := runRoot(t, dir, "", name)
		if err == nil || out != "" {
			t.Errorf("removed command %q: output %q error %v", name, out, err)
		}
	}
	for _, args := range [][]string{{"--help"}, {"help"}, {"completion", "--help"}, {"--version"}} {
		if out, _, err := runRoot(t, dir, "", args...); err != nil || out == "" {
			t.Errorf("%v: output %q error %v", args, out, err)
		}
	}
}

func TestRootTaskWorkflow(t *testing.T) {
	t.Setenv("PATH", t.TempDir())
	dir := t.TempDir()
	other := t.TempDir()
	initOut := mustRoot(t, dir, "", "init", "--json")
	decodeRoot[map[string]any](t, initOut)
	defaults := decodeRoot[map[string]json.RawMessage](t, mustRoot(t, dir, "", "models", "defaults", "--json"))
	for _, name := range []string{"main", "fast"} {
		if string(defaults[name]) != "null" {
			t.Fatalf("unexpected guessed default %s: %s", name, defaults[name])
		}
	}
	help := mustRoot(t, dir, "", "models", "--help")
	if !strings.Contains(help, "set") {
		t.Fatal("model help does not identify the set command")
	}
	doc := taskapi.Document{SchemaVersion: 1, Phases: []taskapi.Phase{
		{Tag: "one", Name: "First phase", IDFormat: "sequential", Tasks: []*model.Task{{ID: "1", Title: "First", Status: model.Pending, Priority: model.Medium, Complexity: 1, Details: "preserve prose\nnext line"}}},
		{Tag: "two", Name: "Second phase", IDFormat: "sequential", Tasks: []*model.Task{{ID: "1", Title: "Second", Status: model.Pending, Priority: model.Medium, Complexity: 1, Dependencies: []string{"one:1"}}}},
	}}
	data, err := json.Marshal(doc)
	if err != nil {
		t.Fatal(err)
	}
	imported := decodeRoot[taskapi.Snapshot](t, mustRoot(t, dir, string(data), "import", "--json"))
	if len(imported.Ready) != 1 || imported.Ready[0].Tag != "one" || len(imported.Blocked) != 1 {
		t.Fatalf("cross-phase readiness: %+v", imported)
	}
	config := []byte("# user configuration, do not overwrite\n")
	configPath := filepath.Join(dir, ".scud", "config.toml")
	if err = os.WriteFile(configPath, config, 0644); err != nil {
		t.Fatal(err)
	}
	tasksPath := filepath.Join(dir, ".scud", "tasks", "tasks.json")
	before, err := os.ReadFile(tasksPath)
	if err != nil {
		t.Fatal(err)
	}
	mustRoot(t, dir, "", "init")
	after, err := os.ReadFile(tasksPath)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatal("init changed existing tasks")
	}
	cfg, err := os.ReadFile(configPath)
	if err != nil || !bytes.Equal(cfg, config) {
		t.Fatal("init changed existing configuration")
	}
	export := mustRoot(t, dir, "", "list", "--json")
	round := decodeRoot[taskapi.Document](t, export)
	if len(round.Phases) != 2 || round.Phases[0].Tasks[0].Details != "preserve prose\nnext line" {
		t.Fatal("list lost document fields")
	}
	mustRoot(t, other, "", "init")
	copied := decodeRoot[taskapi.Snapshot](t, mustRoot(t, other, export, "import", "--json"))
	if copied.Revision != imported.Revision {
		t.Fatal("export/import revision changed")
	}
	if out, _, e := runRoot(t, dir, "", "update", "1", "--status", "done", "--tag", "one", "--if-revision", "stale", "--json"); e == nil || out != "" {
		t.Fatalf("stale update: stdout %q error %v", out, e)
	}
	changed := decodeRoot[taskapi.Snapshot](t, mustRoot(t, dir, "", "update", "1", "--tag", "one", "--status", "done", "--if-revision", imported.Revision, "--json"))
	if len(changed.Ready) != 1 || changed.Ready[0].Tag != "two" {
		t.Fatalf("dependency not released: %+v", changed.Ready)
	}
	changed = decodeRoot[taskapi.Snapshot](t, mustRoot(t, dir, "", "update", "1", "--tag", "two", "--depends-on=", "--if-revision", changed.Revision, "--json"))
	if len(changed.Phases[1].Tasks[0].Dependencies) != 0 {
		t.Fatal("dependency clear did not apply")
	}
	changed = decodeRoot[taskapi.Snapshot](t, mustRoot(t, dir, "", "update", "1", "--tag", "two", "--depends-on", "one:1", "--if-revision", changed.Revision, "--json"))
	if !reflect.DeepEqual(changed.Phases[1].Tasks[0].Dependencies, []string{"one:1"}) {
		t.Fatal("dependency update did not apply")
	}
	status := decodeRoot[taskapi.Snapshot](t, mustRoot(t, dir, "", "status", "--json"))
	if status.Revision != changed.Revision || len(status.Ready) != 1 {
		t.Fatal("status not current")
	}
	next := decodeRoot[taskapi.Ref](t, mustRoot(t, dir, "", "next", "--json"))
	if next.Tag != "two" || next.ID != "1" {
		t.Fatalf("next: %+v", next)
	}
	if out, _, e := runRoot(t, dir, `{"schema_version":1,"schema_version":1}`, "validate", "--input", "-", "--json"); e == nil || out != "" {
		t.Fatalf("invalid validate: stdout=%q error=%v", out, e)
	}
	sb := decodeRoot[map[string]any](t, mustRoot(t, dir, "", "sb", "status", "--json"))
	if sb["enabled"] != false || sb["available"] != false {
		t.Fatalf("sb unexpectedly available: %+v", sb)
	}
	out, _, e := runRoot(t, dir, "", "sb", "check", "--json")
	if e == nil {
		t.Fatal("sb check succeeded without sb")
	}
	check := decodeRoot[map[string]any](t, out)
	if check["success"] != false {
		t.Fatalf("unexpected sb check result: %+v", check)
	}
}
