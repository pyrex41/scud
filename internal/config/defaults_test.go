package config

import (
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
)

func TestSetModelTargetPreservesOtherConfig(t *testing.T) {
	d := t.TempDir()
	p := filepath.Join(d, "config.toml")
	original := "unrelated = \"keep\"\n\n[llm]\nprovider = \"openai\"\n"
	if e := os.WriteFile(p, []byte(original), 0600); e != nil {
		t.Fatal(e)
	}
	if e := SetModelTarget(d, "fast", ModelTarget{Provider: "anthropic", Model: "claude-x"}); e != nil {
		t.Fatal(e)
	}
	b, _ := os.ReadFile(p)
	s := string(b)
	if !strings.Contains(s, "unrelated = \"keep\"") || !strings.Contains(s, "[llm]") || !strings.Contains(s, "model = \"claude-x\"") {
		t.Fatalf("unexpected config: %s", s)
	}
	cfg, e := Load(d)
	if e != nil {
		t.Fatal(e)
	}
	got, e := cfg.ResolveModelTarget("fast")
	if e != nil || got.Provider != "anthropic" || got.Model != "claude-x" {
		t.Fatalf("%#v %v", got, e)
	}
}

func TestConcurrentSetModelTargetPreservesBothAliases(t *testing.T) {
	d := t.TempDir()
	var wg sync.WaitGroup
	errs := make(chan error, 2)
	for _, v := range []struct{ a, p, m string }{{"main", "openai", "m-one"}, {"fast", "anthropic", "f-two"}} {
		wg.Add(1)
		go func(v struct{ a, p, m string }) {
			defer wg.Done()
			errs <- SetModelTarget(d, v.a, ModelTarget{Provider: v.p, Model: v.m})
		}(v)
	}
	wg.Wait()
	close(errs)
	for e := range errs {
		if e != nil {
			t.Fatal(e)
		}
	}
	c, e := LoadModelDefaults(d)
	if e != nil {
		t.Fatal(e)
	}
	m, e := c.ResolveModelTarget("main")
	if e != nil || m.Model != "m-one" {
		t.Fatalf("main %#v %v", m, e)
	}
	f, e := c.ResolveModelTarget("fast")
	if e != nil || f.Model != "f-two" {
		t.Fatalf("fast %#v %v", f, e)
	}
}

func TestSetModelTargetHandlesTOMLSpacingQuotedTablesAndKeepsFields(t *testing.T) {
	d := t.TempDir()
	path := filepath.Join(d, "config.toml")
	source := "main_model\t= \t\"old\" # keep root comment\n[model_targets.\"main\"] # quoted table comment\nprovider=\"xai\" # provider comment\nmodel = \"old\"\nmax_output_tokens = 2718\ncustom = \"keep\"\n[other]\nvalue=42\n"
	if e := os.WriteFile(path, []byte(source), 0600); e != nil {
		t.Fatal(e)
	}
	if e := SetModelTarget(d, "main", ModelTarget{Provider: "openai", Model: "gpt-new"}); e != nil {
		t.Fatal(e)
	}
	data, e := os.ReadFile(path)
	if e != nil {
		t.Fatal(e)
	}
	text := string(data)
	for _, want := range []string{"main_model\t= \"main\" # keep root comment", "[model_targets.\"main\"] # quoted table comment", "provider= \"openai\" # provider comment", "max_output_tokens = 2718", "custom = \"keep\"", "[other]"} {
		if !strings.Contains(text, want) {
			t.Fatalf("lost %q in:\n%s", want, text)
		}
	}
	if e = validateTOML(text, path); e != nil {
		t.Fatalf("candidate invalid TOML: %v", e)
	}
	c, e := LoadModelDefaults(d)
	if e != nil {
		t.Fatal(e)
	}
	got, e := c.ResolveModelTarget("main")
	if e != nil || got.Model != "gpt-new" || got.MaxOutputTokens != 2718 {
		t.Fatalf("target %#v err %v", got, e)
	}
}

func TestSetModelTargetQuotedFastTableAndInvalidSourceAtomic(t *testing.T) {
	d := t.TempDir()
	path := filepath.Join(d, "config.toml")
	source := "fast_model=\"old\"\n[model_targets.'fast'] # literal quoted\nprovider=\"anthropic\"\nmodel=\"old\"\n"
	if e := os.WriteFile(path, []byte(source), 0600); e != nil {
		t.Fatal(e)
	}
	if e := SetModelTarget(d, "fast", ModelTarget{Provider: "anthropic", Model: "claude"}); e != nil {
		t.Fatal(e)
	}
	c, e := LoadModelDefaults(d)
	if e != nil {
		t.Fatal(e)
	}
	got, e := c.ResolveModelTarget("fast")
	if e != nil || got.Model != "claude" {
		t.Fatalf("target %#v err %v", got, e)
	}
	invalid := []byte("main_model = [\n")
	if e = os.WriteFile(path, invalid, 0600); e != nil {
		t.Fatal(e)
	}
	if e = SetModelTarget(d, "main", ModelTarget{Provider: "openai", Model: "gpt"}); e == nil {
		t.Fatal("invalid TOML source accepted")
	}
	after, _ := os.ReadFile(path)
	if string(after) != string(invalid) {
		t.Fatal("invalid source was modified")
	}
}
func TestMainFastLegacyPairingAndUnknown(t *testing.T) {
	c := Default()
	m, e := c.ResolveModelTarget("main")
	if e != nil || m.Model != "grok-4.7" {
		t.Fatalf("main %#v %v", m, e)
	}
	f, e := c.ResolveModelTarget("fast")
	if e != nil || f.Provider != "xai" || f.Model != "grok-build-0.1" {
		t.Fatalf("fast %#v %v", f, e)
	}
	if _, e = c.ResolveModelTarget("typo"); e == nil {
		t.Fatal("unknown target accepted")
	}
	c.LLM.FastProvider = "openai"
	c.LLM.FastModel = ""
	c.Rho.FastProvider = ""
	c.Rho.FastModel = "grok-model"
	if _, e = c.ResolveModelTarget("fast"); e == nil {
		t.Fatal("cross-paired legacy fast model accepted")
	}
	c.LLM.FastProvider = ""
	c.Rho.FastProvider = ""
	c.Rho.FastModel = ""
	if _, e = c.ResolveModelTarget("fast"); e == nil || !strings.Contains(e.Error(), "scud models set fast") {
		t.Fatalf("missing fast target did not provide setup guidance: %v", e)
	}
}

func TestMissingModelConfigHasNoImplicitTargets(t *testing.T) {
	c, e := LoadModelDefaults(t.TempDir())
	if e != nil {
		t.Fatal(e)
	}
	if _, e = c.ResolveModelTarget("main"); e == nil {
		t.Fatal("missing config inherited an implicit legacy target")
	}
}
