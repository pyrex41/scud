package swarm

import (
	"testing"

	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/pkg/model"
)

func TestModelRefPrefersExplicitProvider(t *testing.T) {
	ref := modelRef("custom-deployment", "openai")
	if ref.Provider != "openai" || ref.ID != "custom-deployment" {
		t.Fatalf("modelRef = %#v", ref)
	}
}

func TestModelRefLegacyPrefixFallback(t *testing.T) {
	for modelID, want := range map[string]string{
		"claude-sonnet-4-5": "anthropic",
		"gpt-5.2":           "openai",
		"grok-4.3":          "xai",
	} {
		if got := modelRef(modelID, "").Provider; got != want {
			t.Errorf("modelRef(%q).Provider = %q, want %q", modelID, got, want)
		}
	}
}

func TestResolveProviderUsesTaskTier(t *testing.T) {
	cfg := config.Default()
	cfg.Rho.Provider = "openai"
	cfg.Rho.FastProvider = "xai"
	task := &model.Task{ModelTier: model.TierFast}
	if got := resolveProvider(task, cfg); got != "xai" {
		t.Fatalf("resolveProvider = %q, want xai", got)
	}
	task.ModelTier = model.TierCustom
	if got := resolveProvider(task, cfg); got != "openai" {
		t.Fatalf("custom resolveProvider = %q, want openai", got)
	}
}
