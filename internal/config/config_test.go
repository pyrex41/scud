package config

import (
	"os"
	"path/filepath"
	"testing"
)

func TestDefaultValues(t *testing.T) {
	cfg := Default()
	if cfg.Rho.Model == "" {
		t.Error("default Rho.Model is empty")
	}
	if cfg.Rho.Provider != "xai" {
		t.Errorf("default Rho.Provider = %q, want xai", cfg.Rho.Provider)
	}
	if cfg.Executor.Kind != "legacy" {
		t.Errorf("default Executor.Kind = %q, want legacy", cfg.Executor.Kind)
	}
	if cfg.LLM.Provider != "xai" {
		t.Errorf("LLM.Provider = %q, want %q", cfg.LLM.Provider, "xai")
	}
	if cfg.LLM.MaxTokens != 4096 {
		t.Errorf("LLM.MaxTokens = %d, want 4096", cfg.LLM.MaxTokens)
	}
	if cfg.Swarm.RoundSize != 5 {
		t.Errorf("Swarm.RoundSize = %d, want 5", cfg.Swarm.RoundSize)
	}
	if cfg.Heavy.Concurrency != 4 {
		t.Errorf("Heavy.Concurrency = %d, want 4", cfg.Heavy.Concurrency)
	}
	if cfg.Heavy.TimeoutSecs != 300 {
		t.Errorf("Heavy.TimeoutSecs = %d, want 300", cfg.Heavy.TimeoutSecs)
	}
	if cfg.Swarm.Backpressure.StopOnFailure != true {
		t.Error("Swarm.Backpressure.StopOnFailure should default to true")
	}
}

func TestLoadMissingFile(t *testing.T) {
	dir := t.TempDir()
	cfg, err := Load(dir)
	if err != nil {
		t.Fatalf("Load with missing file: %v", err)
	}
	if cfg.LLM.Provider != "xai" {
		t.Errorf("expected defaults when file missing, got Provider=%q", cfg.LLM.Provider)
	}
}

func TestSaveAndLoad(t *testing.T) {
	dir := t.TempDir()
	cfg := Default()
	cfg.LLM.Provider = "anthropic"
	cfg.Swarm.RoundSize = 10

	if err := cfg.Save(dir); err != nil {
		t.Fatalf("Save: %v", err)
	}

	// Verify file exists
	if _, err := os.Stat(filepath.Join(dir, "config.toml")); err != nil {
		t.Fatalf("config.toml not created: %v", err)
	}

	loaded, err := Load(dir)
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	if loaded.LLM.Provider != "anthropic" {
		t.Errorf("Provider = %q, want %q", loaded.LLM.Provider, "anthropic")
	}
	if loaded.Swarm.RoundSize != 10 {
		t.Errorf("RoundSize = %d, want 10", loaded.Swarm.RoundSize)
	}
}

func TestEnvOverrides(t *testing.T) {
	tests := []struct {
		name  string
		env   string
		value string
		check func(*Config) string
		want  string
	}{
		{
			name:  "SCUD_EXECUTOR overrides Executor.Kind",
			env:   "SCUD_EXECUTOR",
			value: "rho-v1",
			check: func(c *Config) string { return c.Executor.Kind },
			want:  "rho-v1",
		},
		{
			name:  "SCUD_RHO_PROVIDER overrides Rho.Provider",
			env:   "SCUD_RHO_PROVIDER",
			value: "anthropic",
			check: func(c *Config) string { return c.Rho.Provider },
			want:  "anthropic",
		},
		{
			name:  "SCUD_MODEL overrides Rho.Model",
			env:   "SCUD_MODEL",
			value: "test-model",
			check: func(c *Config) string { return c.Rho.Model },
			want:  "test-model",
		},
		{
			name:  "SCUD_PROVIDER overrides LLM.Provider",
			env:   "SCUD_PROVIDER",
			value: "openai",
			check: func(c *Config) string { return c.LLM.Provider },
			want:  "openai",
		},
		{
			name:  "SCUD_HEAVY_MODEL overrides Heavy.Model",
			env:   "SCUD_HEAVY_MODEL",
			value: "heavy-test",
			check: func(c *Config) string { return c.Heavy.Model },
			want:  "heavy-test",
		},
		{
			name:  "SCUD_HEAVY_MODE overrides Heavy.Mode",
			env:   "SCUD_HEAVY_MODE",
			value: "hybrid",
			check: func(c *Config) string { return c.Heavy.Mode },
			want:  "hybrid",
		},
		{
			name:  "SCUD_HEAVY_MODEL_AGENTS overrides Heavy.Models.Agents",
			env:   "SCUD_HEAVY_MODEL_AGENTS",
			value: "fast-agent",
			check: func(c *Config) string { return c.Heavy.Models.Agents },
			want:  "fast-agent",
		},
		{
			name:  "SCUD_HEAVY_MODEL_SYNTHESIS overrides Heavy.Models.Synthesis",
			env:   "SCUD_HEAVY_MODEL_SYNTHESIS",
			value: "smart-synth",
			check: func(c *Config) string { return c.Heavy.Models.Synthesis },
			want:  "smart-synth",
		},
		{
			name:  "SCUD_MAX_TOKENS overrides LLM.MaxTokens",
			env:   "SCUD_MAX_TOKENS",
			value: "8192",
			check: func(c *Config) string {
				if c.LLM.MaxTokens == 8192 {
					return "8192"
				}
				return ""
			},
			want: "8192",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Setenv(tt.env, tt.value)
			dir := t.TempDir()
			cfg, err := Load(dir)
			if err != nil {
				t.Fatalf("Load: %v", err)
			}
			got := tt.check(cfg)
			if got != tt.want {
				t.Errorf("got %q, want %q", got, tt.want)
			}
		})
	}
}

func TestEnvOverrideInvalidInt(t *testing.T) {
	t.Setenv("SCUD_ROUND_SIZE", "not-a-number")
	dir := t.TempDir()
	cfg, err := Load(dir)
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	// Should keep default when env is invalid
	if cfg.Swarm.RoundSize != 5 {
		t.Errorf("RoundSize = %d, want 5 (default)", cfg.Swarm.RoundSize)
	}
}

func TestHeavyModel(t *testing.T) {
	tests := []struct {
		name string
		cfg  Config
		role string
		want string
	}{
		{
			name: "per-role routing override",
			cfg:  Config{Heavy: HeavyConfig{Models: HeavyModelsConfig{Routing: "route-model"}}},
			role: "routing",
			want: "route-model",
		},
		{
			name: "fallback to Heavy.Model",
			cfg:  Config{Heavy: HeavyConfig{Model: "heavy-fallback"}},
			role: "agents",
			want: "heavy-fallback",
		},
		{
			name: "fallback to Rho.SmartModel",
			cfg:  Config{Rho: RhoConfig{SmartModel: "rho-smart"}},
			role: "synthesis",
			want: "rho-smart",
		},
		{
			name: "ultimate default",
			cfg:  Config{},
			role: "debate",
			want: "grok-4.20-reasoning",
		},
		{
			name: "native per-role",
			cfg:  Config{Heavy: HeavyConfig{Models: HeavyModelsConfig{Native: "native-model"}}},
			role: "native",
			want: "native-model",
		},
		{
			name: "native fallback to MultiAgentModel",
			cfg:  Config{LLM: LLMConfig{MultiAgentModel: "multi-agent"}},
			role: "native",
			want: "multi-agent",
		},
		{
			name: "native ultimate default",
			cfg:  Config{},
			role: "native",
			want: "grok-4.20-multi-agent-0309",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := tt.cfg.HeavyModel(tt.role)
			if got != tt.want {
				t.Errorf("HeavyModel(%q) = %q, want %q", tt.role, got, tt.want)
			}
		})
	}
}

func TestModelForTier(t *testing.T) {
	cfg := Default()
	tests := []struct {
		tier string
		want string
	}{
		{"fast", cfg.Swarm.Tiers.Fast},
		{"standard", cfg.Swarm.Tiers.Standard},
		{"smart", cfg.Swarm.Tiers.Smart},
		{"unknown", cfg.Swarm.Tiers.Standard}, // defaults to standard
	}
	for _, tt := range tests {
		t.Run(tt.tier, func(t *testing.T) {
			got := cfg.ModelForTier(tt.tier)
			if got != tt.want {
				t.Errorf("ModelForTier(%q) = %q, want %q", tt.tier, got, tt.want)
			}
		})
	}
}

func TestProviderForTier(t *testing.T) {
	cfg := Default()
	cfg.Rho.Provider = "openai"
	cfg.Rho.FastProvider = "xai"
	cfg.Rho.SmartProvider = "anthropic"
	for tier, want := range map[string]string{
		"fast": "xai", "standard": "openai", "smart": "anthropic", "unknown": "openai",
	} {
		if got := cfg.ProviderForTier(tier); got != want {
			t.Errorf("ProviderForTier(%q) = %q, want %q", tier, got, want)
		}
	}
}

func TestLoadMalformedFile(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "config.toml")
	if err := os.WriteFile(path, []byte("not valid [[ toml {{"), 0644); err != nil {
		t.Fatal(err)
	}
	_, err := Load(dir)
	if err == nil {
		t.Error("expected error for malformed TOML")
	}
}
