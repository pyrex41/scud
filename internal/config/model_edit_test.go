package config

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/reuben/scud/internal/filelock"
)

func TestModelEditMultilineNotesAreNotStatements(t *testing.T) {
	for _, note := range []string{
		"\"\"\"\nmain_model = 'do not edit'\n[model_targets.main]\nprovider = 'notes'\nmodel = 'notes'\n\"\"\"",
		"'''\nmain_model = 'do not edit'\n[model_targets.main]\nprovider = 'notes'\nmodel = 'notes'\n'''",
		"\"\"\"escaped \\\"\"\" still a note\nmain_model = 'do not edit'\n[other]\n\"\"\"",
	} {
		t.Run(note, func(t *testing.T) {
			d := t.TempDir()
			p := filepath.Join(d, "config.toml")
			unknown := "[unknown]\nnested = { keep = [1, 2, 3] } # keep this\n"
			source := "notes = " + note + "\n\n[model_targets.main]\nnotes = " + note + "\nprovider = 'old'\nmodel = 'old'\n" + unknown
			if err := os.WriteFile(p, []byte(source), 0600); err != nil {
				t.Fatal(err)
			}
			if err := SetModelTarget(d, "main", ModelTarget{Provider: "openrouter", Model: "anthropic/claude-test"}); err != nil {
				t.Fatal(err)
			}
			data, err := os.ReadFile(p)
			if err != nil {
				t.Fatal(err)
			}
			if strings.Count(string(data), "notes = "+note) != 2 || !strings.Contains(string(data), unknown) {
				t.Fatalf("notes or unknown section changed:\n%s", data)
			}
			cfg, err := LoadModelDefaults(d)
			if err != nil {
				t.Fatal(err)
			}
			target, err := cfg.ResolveModelTarget("main")
			if err != nil || target.Model != "anthropic/claude-test" {
				t.Fatalf("%+v %v", target, err)
			}
		})
	}
}

func TestModelEditUnsupportedLayoutsDoNotWrite(t *testing.T) {
	for _, source := range []string{
		"model_targets = { main = { provider = 'old', model = 'old' } }\n",
		"[model_targets]\nmain = { provider = 'old', model = 'old' }\n",
		"model_targets.main.provider = 'old'\nmodel_targets.main.model = 'old'\n",
		"[model_targets.main]\nprovider = '''old\nprovider = 'note'\n'''\nmodel = 'old'\n",
	} {
		t.Run(source, func(t *testing.T) {
			d := t.TempDir()
			p := filepath.Join(d, "config.toml")
			if err := os.WriteFile(p, []byte(source), 0600); err != nil {
				t.Fatal(err)
			}
			if err := SetModelTarget(d, "main", ModelTarget{Provider: "openai", Model: "new"}); err == nil {
				t.Fatal("unsupported layout accepted")
			}
			data, err := os.ReadFile(p)
			if err != nil || string(data) != source {
				t.Fatalf("file changed: %s %v", data, err)
			}
		})
	}
}

func TestModelEditCancelledDoesNotWrite(t *testing.T) {
	d := t.TempDir()
	p := filepath.Join(d, "config.toml")
	source := "# keep\nmain_model = 'old'\n"
	if err := os.WriteFile(p, []byte(source), 0600); err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if err := SetModelTargetContext(ctx, d, "main", ModelTarget{Provider: "openai", Model: "new"}); !errors.Is(err, context.Canceled) {
		t.Fatalf("%v", err)
	}
	data, _ := os.ReadFile(p)
	if string(data) != source {
		t.Fatal("cancelled write changed file")
	}
}

func TestModelEditSemanticGuardRejectsUnrelatedChange(t *testing.T) {
	before := "notes = 'keep'\n"
	after := "notes = 'lost'\nmain_model = 'main'\n[model_targets.main]\nprovider = 'openai'\nmodel = 'new'\n"
	if err := verifyModelUpdate(before, after, "main", ModelTarget{Provider: "openai", Model: "new"}); err == nil {
		t.Fatal("unrelated change accepted")
	}
}

func TestModelEditLockDeadlineDoesNotWrite(t *testing.T) {
	d := t.TempDir()
	p := filepath.Join(d, "config.toml")
	source := "# keep both aliases\nmain_model = 'old'\nfast_model = 'other'\n"
	if err := os.WriteFile(p, []byte(source), 0600); err != nil {
		t.Fatal(err)
	}
	lock, err := os.OpenFile(filepath.Join(d, "config.toml.lock"), os.O_CREATE|os.O_RDWR, 0600)
	if err != nil {
		t.Fatal(err)
	}
	defer lock.Close()
	if err = filelock.Lock(context.Background(), lock); err != nil {
		t.Fatal(err)
	}
	defer filelock.Unlock(lock)
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Millisecond)
	defer cancel()
	if err = SetModelTargetContext(ctx, d, "main", ModelTarget{Provider: "openai", Model: "new"}); !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("%v", err)
	}
	data, _ := os.ReadFile(p)
	if string(data) != source {
		t.Fatal("lock timeout changed config")
	}
}

func TestModelEditQuotedRootAndTableKeys(t *testing.T) {
	d := t.TempDir()
	p := filepath.Join(d, "config.toml")
	source := "\"main_model\" = 'old' # root comment\n[\"model_targets\".'main']\n'provider' = 'old' # provider comment\n\"model\" = 'old'\n"
	if err := os.WriteFile(p, []byte(source), 0600); err != nil {
		t.Fatal(err)
	}
	if err := SetModelTarget(d, "main", ModelTarget{Provider: "openrouter", Model: "vendor/model"}); err != nil {
		t.Fatal(err)
	}
	data, _ := os.ReadFile(p)
	for _, want := range []string{"# root comment", "# provider comment", "[\"model_targets\".'main']", "vendor/model"} {
		if !strings.Contains(string(data), want) {
			t.Fatalf("lost %s: %s", want, data)
		}
	}
}
