package main

import (
	"bytes"
	"encoding/json"
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
)

// Keep live agent instructions under test as well as generated --help. Research
// and release notes are immutable/historical evidence, not current workflows.
func activeGuidance(t *testing.T) []string {
	t.Helper()
	root := filepath.Join("..", "..")
	paths := []string{"README.md", "CLAUDE.md", "go.md", "docs/README.md", "docs/V2_SPEC.md", "docs/REDESIGN.md", "docs/REVIEW_HANDOFF.md", "docs/CLI_REFERENCE.md", "pkg/executor/README.md", ".opencode/scud-commands.md", ".opencode/skills/scud-sm.md", ".opencode/hook/session-start.md"}
	for _, dir := range []string{".claude/skills/scud", ".claude/skills/scud-tasks", ".opencode/skills/scud", ".opencode/skills/scud-tasks", ".claude/commands/scud", ".opencode/command"} {
		err := filepath.WalkDir(filepath.Join(root, dir), func(path string, entry fs.DirEntry, err error) error {
			if err != nil {
				return err
			}
			if entry.IsDir() && entry.Name() == "evals" {
				return filepath.SkipDir
			}
			if !entry.IsDir() && strings.HasSuffix(path, ".md") {
				relative, err := filepath.Rel(root, path)
				if err != nil {
					return err
				}
				paths = append(paths, relative)
			}
			return nil
		})
		if err != nil {
			t.Fatal(err)
		}
	}
	return paths
}

func TestLiveGuidanceUsesRegisteredCommands(t *testing.T) {
	root := helpRoot()
	invocations := regexp.MustCompile("\\bscud[ \\t]+([a-z][a-z0-9-]*)")
	inline := regexp.MustCompile("`([^`]+)`")
	flags := regexp.MustCompile(`--[a-z][a-z0-9-]*`)
	for _, relative := range activeGuidance(t) {
		body, err := os.ReadFile(filepath.Join("..", "..", relative))
		if err != nil {
			t.Fatal(err)
		}
		language, inFence := "", false
		for _, line := range strings.Split(string(body), "\n") {
			trimmed := strings.TrimSpace(line)
			if strings.HasPrefix(trimmed, "```") {
				if inFence {
					language, inFence = "", false
				} else {
					language, inFence = strings.TrimPrefix(trimmed, "```"), true
				}
				continue
			}
			var snippets []string
			if inFence {
				if language == "sh" || language == "bash" || language == "shell" {
					snippets = append(snippets, line)
				}
			} else {
				for _, code := range inline.FindAllStringSubmatch(line, -1) {
					snippets = append(snippets, code[1])
				}
			}
			for _, snippet := range snippets {
				for _, match := range invocations.FindAllStringSubmatchIndex(snippet, -1) {
					name := snippet[match[2]:match[3]]
					command, _, err := root.Find([]string{name})
					if err != nil || command == root {
						t.Errorf("%s recommends unregistered command scud %s", relative, name)
						continue
					}
					call := snippet[match[0]+len("scud "):]
					for _, delimiter := range []string{"|", ">", ";", "&&", "#"} {
						call = strings.SplitN(call, delimiter, 2)[0]
					}
					selected, _, err := root.Find(strings.Fields(call))
					if err != nil {
						t.Errorf("%s has invalid command example %q: %v", relative, call, err)
						continue
					}
					for _, flag := range flags.FindAllString(call, -1) {
						key := strings.TrimPrefix(flag, "--")
						selected.InitDefaultHelpFlag()
						if selected.Flags().Lookup(key) == nil && selected.InheritedFlags().Lookup(key) == nil {
							t.Errorf("%s recommends unknown flag %s for %s", relative, flag, selected.CommandPath())
						}
					}
				}
			}
		}
	}
}

func TestSkillCopiesStayInSync(t *testing.T) {
	for _, skill := range []struct{ folder, name string }{{"scud", "scud-guide"}, {"scud-tasks", "scud-tasks"}} {
		canonical, err := os.ReadFile(filepath.Join("..", "..", ".claude", "skills", skill.folder, "SKILL.md"))
		if err != nil {
			t.Fatal(err)
		}
		mirror, err := os.ReadFile(filepath.Join("..", "..", ".opencode", "skills", skill.folder, "SKILL.md"))
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Equal(canonical, mirror) {
			t.Errorf("%s skill copies drifted", skill.folder)
		}
		text := string(canonical)
		if !strings.HasPrefix(text, "---\nname: "+skill.name+"\n") || !strings.Contains(text, "\ndescription: ") {
			t.Errorf("%s missing canonical frontmatter", skill.folder)
		}
		if len(strings.Split(text, "\n")) >= 500 {
			t.Errorf("%s skill should link references rather than exceed 500 lines", skill.folder)
		}
		for _, contract := range []string{"2026-07-28", "revision", "null", "main", "fast"} {
			if skill.folder == "scud-tasks" && (contract == "main" || contract == "fast") {
				continue
			}
			if !strings.Contains(text, contract) {
				t.Errorf("%s skill omits %s contract", skill.folder, contract)
			}
		}
	}
}

func TestSessionHooksDoNotMutateTaskStatus(t *testing.T) {
	body, err := os.ReadFile(filepath.Join("..", "..", ".claude", "settings.local.json"))
	if err != nil {
		t.Fatal(err)
	}
	var config struct {
		Hooks map[string][]struct {
			Hooks []struct {
				Command string `json:"command"`
			} `json:"hooks"`
		} `json:"hooks"`
	}
	if err := json.Unmarshal(body, &config); err != nil {
		t.Fatal(err)
	}
	mutation := regexp.MustCompile(`\bscud\s+(?:set-status|update|import|parse|run|swarm|commit)\b`)
	for event, definitions := range config.Hooks {
		for _, definition := range definitions {
			for _, hook := range definition.Hooks {
				if mutation.MatchString(hook.Command) {
					t.Errorf("%s hook contains automatic task mutation: %s", event, hook.Command)
				}
			}
		}
	}
}

func TestResearchIsExplicitlyHistorical(t *testing.T) {
	files, err := filepath.Glob(filepath.Join("..", "..", "thoughts", "shared", "research", "*.md"))
	if err != nil {
		t.Fatal(err)
	}
	for _, file := range files {
		body, err := os.ReadFile(file)
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Contains(body, []byte("**Historical research — pre-v3.**")) {
			t.Errorf("%s lacks its historical-scope banner", file)
		}
	}
}
