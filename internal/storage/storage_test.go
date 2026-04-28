package storage

import (
	"os"
	"path/filepath"
	"sync"
	"testing"

	"github.com/reuben/scud/pkg/model"
)

func TestInitializeAndLoad(t *testing.T) {
	dir := t.TempDir()
	store := New(dir)

	if err := store.Initialize(); err != nil {
		t.Fatal(err)
	}

	// Check files exist
	if _, err := os.Stat(store.TasksFile()); err != nil {
		t.Errorf("tasks file not created: %v", err)
	}

	phases, err := store.LoadPhases()
	if err != nil {
		t.Fatal(err)
	}
	if len(phases) != 0 {
		t.Errorf("expected 0 phases, got %d", len(phases))
	}
}

func TestUpdatePhase(t *testing.T) {
	dir := t.TempDir()
	store := New(dir)
	store.Initialize()

	// Create a phase with tasks
	err := store.UpdatePhase("v1", func(p *model.Phase) error {
		p.Tasks = append(p.Tasks, &model.Task{
			ID:         "1",
			Title:      "First task",
			Status:     model.Pending,
			Complexity: 3,
			Priority:   model.High,
		})
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}

	// Read back
	phases, err := store.LoadPhases()
	if err != nil {
		t.Fatal(err)
	}
	p, ok := phases["v1"]
	if !ok {
		t.Fatal("phase v1 not found")
	}
	if len(p.Tasks) != 1 {
		t.Fatalf("tasks = %d, want 1", len(p.Tasks))
	}
	if p.Tasks[0].Title != "First task" {
		t.Errorf("title = %q, want 'First task'", p.Tasks[0].Title)
	}
}

func TestConcurrentUpdatePhase(t *testing.T) {
	dir := t.TempDir()
	store := New(dir)
	store.Initialize()

	// Create initial phase
	store.UpdatePhase("test", func(p *model.Phase) error {
		for i := 0; i < 10; i++ {
			p.Tasks = append(p.Tasks, &model.Task{
				ID:       string(rune('a' + i)),
				Title:    "task",
				Status:   model.Pending,
				Priority: model.Medium,
			})
		}
		return nil
	})

	// Concurrent updates - each marks one task done
	var wg sync.WaitGroup
	for i := 0; i < 10; i++ {
		wg.Add(1)
		go func(idx int) {
			defer wg.Done()
			id := string(rune('a' + idx))
			store.UpdatePhase("test", func(p *model.Phase) error {
				if t := p.FindTask(id); t != nil {
					t.Status = model.Done
				}
				return nil
			})
		}(i)
	}
	wg.Wait()

	// Verify all tasks are done
	phases, _ := store.LoadPhases()
	p := phases["test"]
	for _, task := range p.Tasks {
		if task.Status != model.Done {
			t.Errorf("task %s status = %s, want done", task.ID, task.Status)
		}
	}
}

func TestActiveTag(t *testing.T) {
	dir := t.TempDir()
	store := New(dir)
	store.Initialize()

	if tag := store.ActiveTag(); tag != "" {
		t.Errorf("initial tag = %q, want empty", tag)
	}

	store.SetActiveTag("v1")
	if tag := store.ActiveTag(); tag != "v1" {
		t.Errorf("tag = %q, want v1", tag)
	}
}

func TestResolveTagPersistsExplicit(t *testing.T) {
	dir := t.TempDir()
	store := New(dir)
	store.Initialize()

	// Passing -t should persist as active tag
	tag, err := store.ResolveTag("v2")
	if err != nil {
		t.Fatal(err)
	}
	if tag != "v2" {
		t.Errorf("resolved = %q, want v2", tag)
	}
	if active := store.ActiveTag(); active != "v2" {
		t.Errorf("active = %q, want v2 (should be persisted)", active)
	}
}

func TestResolveTagSinglePhase(t *testing.T) {
	dir := t.TempDir()
	store := New(dir)
	store.Initialize()

	// Create a single phase
	store.UpdatePhase("only-one", func(p *model.Phase) error {
		p.Tasks = append(p.Tasks, &model.Task{
			ID:     "1",
			Title:  "task",
			Status: model.Pending,
		})
		return nil
	})

	// No active tag, no -t flag — should auto-select the sole phase
	tag, err := store.ResolveTag("")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if tag != "only-one" {
		t.Errorf("resolved = %q, want only-one", tag)
	}
	if active := store.ActiveTag(); active != "only-one" {
		t.Errorf("active = %q, want only-one (should be persisted)", active)
	}
}

func TestResolveTagMultiplePhasesNoActive(t *testing.T) {
	dir := t.TempDir()
	store := New(dir)
	store.Initialize()

	// Create two phases
	store.UpdatePhase("a", func(p *model.Phase) error {
		p.Tasks = append(p.Tasks, &model.Task{ID: "1", Title: "t", Status: model.Pending})
		return nil
	})
	store.UpdatePhase("b", func(p *model.Phase) error {
		p.Tasks = append(p.Tasks, &model.Task{ID: "1", Title: "t", Status: model.Pending})
		return nil
	})

	// No active tag, no -t flag, multiple phases — should error
	_, err := store.ResolveTag("")
	if err == nil {
		t.Fatal("expected error with multiple phases and no active tag")
	}
}

func TestLoadGuidance(t *testing.T) {
	dir := t.TempDir()
	store := New(dir)
	store.Initialize()

	// Write guidance files
	guidanceDir := filepath.Join(dir, ".scud", "guidance")
	os.WriteFile(filepath.Join(guidanceDir, "01-context.md"), []byte("Project context here"), 0644)
	os.WriteFile(filepath.Join(guidanceDir, "02-rules.md"), []byte("Rule: do X"), 0644)

	guidance := store.LoadGuidance()
	if guidance == "" {
		t.Error("guidance is empty")
	}
	if !contains(guidance, "Project context here") || !contains(guidance, "Rule: do X") {
		t.Errorf("guidance missing content: %q", guidance)
	}
}

func TestFindRoot(t *testing.T) {
	dir := t.TempDir()
	// Create .scud in root
	os.MkdirAll(filepath.Join(dir, ".scud"), 0755)
	// Create nested directory
	nested := filepath.Join(dir, "src", "pkg")
	os.MkdirAll(nested, 0755)

	root, err := FindRoot(nested)
	if err != nil {
		t.Fatal(err)
	}
	if root != dir {
		t.Errorf("root = %q, want %q", root, dir)
	}
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(s) > 0 && containsStr(s, substr))
}

func containsStr(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
