package config

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/BurntSushi/toml"
	"github.com/reuben/scud/internal/filelock"
)

type Config struct {
	DefaultModel string                 `toml:"default_model"`
	MainModel    string                 `toml:"main_model"`
	FastModelRef string                 `toml:"fast_model"`
	ModelTargets map[string]ModelTarget `toml:"model_targets"`
	Models       map[string]ModelTarget `toml:"models"`
	Rho          RhoConfig              `toml:"rho"`
	Executor     ExecutorConfig         `toml:"executor"`
	Swarm        SwarmConfig            `toml:"swarm"`
	Heavy        HeavyConfig            `toml:"heavy"`
	LLM          LLMConfig              `toml:"llm"`
}

// ModelTarget is an explicit provider/model pair used by the simple generation path.
type ModelTarget struct {
	Provider        string `toml:"provider"`
	Model           string `toml:"model"`
	MaxOutputTokens int    `toml:"max_output_tokens"`
}

// ResolveModelTarget returns an explicitly named target, or the configured default.
// Legacy configurations resolve as a single deterministic route without fallbacks.
func (c *Config) ResolveModelTarget(name string) (ModelTarget, error) {
	if name == "" || name == "main" {
		name = "main"
	}
	if name == "fast" {
		ref := c.FastModelRef
		if ref != "" {
			if t, ok := c.ModelTargets[ref]; ok {
				return validateTarget(ref, t)
			}
			if t, ok := c.Models[ref]; ok {
				return validateTarget(ref, t)
			}
			return ModelTarget{}, fmt.Errorf("unknown fast model target %q", ref)
		}
		return legacyFast(c)
	}
	if name == "main" {
		ref := c.MainModel
		if ref == "" {
			ref = c.DefaultModel
		}
		if ref != "" {
			if t, ok := c.ModelTargets[ref]; ok {
				return validateTarget(ref, t)
			}
			if t, ok := c.Models[ref]; ok {
				return validateTarget(ref, t)
			}
			return ModelTarget{}, fmt.Errorf("unknown main model target %q", ref)
		}
		return legacyMain(c)
	}
	if name != "" {
		if t, ok := c.Models[name]; ok {
			if t.Provider == "" || t.Model == "" {
				return ModelTarget{}, fmt.Errorf("model target %q requires provider and model", name)
			}
			return t, nil
		}
		return ModelTarget{}, fmt.Errorf("unknown model target %q; use main, fast, or configure a named target", name)
	}
	return ModelTarget{}, fmt.Errorf("no model target configured")
}
func validateTarget(name string, t ModelTarget) (ModelTarget, error) {
	if t.Provider == "" || t.Model == "" {
		return ModelTarget{}, fmt.Errorf("model target %q requires provider and model", name)
	}
	return t, nil
}
func legacyMain(c *Config) (ModelTarget, error) {
	provider, model := c.LLM.SmartProvider, c.LLM.SmartModel
	if provider != "" || model != "" {
		if provider == "" || model == "" {
			return ModelTarget{}, fmt.Errorf("legacy smart model needs both provider and model; set main with `scud models set main ...`")
		}
		return ModelTarget{Provider: provider, Model: model, MaxOutputTokens: c.LLM.MaxTokens}, nil
	}
	provider, model = c.Rho.SmartProvider, c.Rho.SmartModel
	if provider != "" || model != "" {
		if provider == "" || model == "" {
			return ModelTarget{}, fmt.Errorf("legacy rho smart model needs both provider and model; set main with `scud models set main ...`")
		}
		return ModelTarget{Provider: provider, Model: model, MaxOutputTokens: c.LLM.MaxTokens}, nil
	}
	provider, model = c.LLM.Provider, c.LLM.Model
	if provider == "" && model == "" {
		provider, model = c.Rho.Provider, c.Rho.Model
	}
	if provider == "" || model == "" {
		return ModelTarget{}, fmt.Errorf("no main model configured; run `scud models set main ...`")
	}
	return ModelTarget{Provider: provider, Model: model, MaxOutputTokens: c.LLM.MaxTokens}, nil
}
func legacyFast(c *Config) (ModelTarget, error) {
	provider, model := c.LLM.FastProvider, c.LLM.FastModel
	if provider != "" || model != "" {
		if provider == "" || model == "" {
			return ModelTarget{}, fmt.Errorf("legacy llm fast model needs both provider and model; set fast with `scud models set fast ...`")
		}
		return ModelTarget{Provider: provider, Model: model, MaxOutputTokens: c.LLM.MaxTokens}, nil
	}
	provider, model = c.Rho.FastProvider, c.Rho.FastModel
	if provider == "" && model == "" {
		return ModelTarget{}, fmt.Errorf("no fast model configured; run `scud models set fast ...`")
	}
	if provider == "" || model == "" {
		return ModelTarget{}, fmt.Errorf("legacy fast model needs both provider and model; set fast with `scud models set fast ...`")
	}
	return ModelTarget{Provider: provider, Model: model, MaxOutputTokens: c.LLM.MaxTokens}, nil
}

type LLMConfig struct {
	Provider         string `toml:"provider"`
	Model            string `toml:"model"`
	SmartProvider    string `toml:"smart_provider"`
	SmartModel       string `toml:"smart_model"`
	FastProvider     string `toml:"fast_provider"`
	FastModel        string `toml:"fast_model"`
	MultiAgentModel  string `toml:"multi_agent_model"`
	MultiAgentEffort string `toml:"multi_agent_effort"` // "low", "medium", "high", "xhigh"
	MaxTokens        int    `toml:"max_tokens"`
}

// HeavyModelsConfig holds per-role model overrides for the heavy ensemble.
type HeavyModelsConfig struct {
	Routing   string `toml:"routing"`   // Captain routing step
	Agents    string `toml:"agents"`    // parallel rho agent execution
	Synthesis string `toml:"synthesis"` // Captain synthesis step
	Debate    string `toml:"debate"`    // critique/resynthesis rounds
	Native    string `toml:"native"`    // xAI multi-agent model
}

type HeavyConfig struct {
	Model       string            `toml:"model"`        // override-all fallback
	Models      HeavyModelsConfig `toml:"models"`       // per-role overrides
	Mode        string            `toml:"mode"`         // "ensemble", "native", "hybrid"
	Concurrency int               `toml:"concurrency"`  // default 4
	TimeoutSecs int               `toml:"timeout_secs"` // default 300
	MaxAgents   int               `toml:"max_agents"`   // 0 = no cap
}

type RhoConfig struct {
	Provider      string `toml:"provider"`
	Model         string `toml:"model"`
	FastProvider  string `toml:"fast_provider"`
	FastModel     string `toml:"fast_model"`
	SmartProvider string `toml:"smart_provider"`
	SmartModel    string `toml:"smart_model"`
}

// ExecutorConfig selects the agent harness used by run and swarm. "legacy"
// preserves the historical rho-cli invocation; "rho-v1" enables rho.run/v1.
type ExecutorConfig struct {
	Kind            string   `toml:"kind"`
	Command         string   `toml:"command"`
	Args            []string `toml:"args"`
	GrantID         string   `toml:"grant_id"`
	GrantTTLSeconds int      `toml:"grant_ttl_seconds"`
	AllowedTools    []string `toml:"allowed_tools"`
	NetworkMode     string   `toml:"network_mode"`
}

type SwarmConfig struct {
	RoundSize        int             `toml:"round_size"`
	MaxRalphAttempts int             `toml:"max_ralph_attempts"`
	TaskTimeoutSecs  int             `toml:"task_timeout_secs"`
	Tiers            TierConfig      `toml:"tiers"`
	Backpressure     BackpressureCfg `toml:"backpressure"`
}

type TierConfig struct {
	Fast     string `toml:"fast"`
	Standard string `toml:"standard"`
	Smart    string `toml:"smart"`
}

type BackpressureCfg struct {
	Commands      []string `toml:"commands"`
	StopOnFailure bool     `toml:"stop_on_failure"`
	TimeoutSecs   int      `toml:"timeout_secs"`
}

func Default() *Config {
	return &Config{
		Rho: RhoConfig{
			Provider:      "xai",
			Model:         "grok-4.7",
			FastProvider:  "xai",
			FastModel:     "grok-build-0.1",
			SmartProvider: "xai",
			SmartModel:    "grok-4.7",
		},
		Executor: ExecutorConfig{
			Kind:            "legacy",
			Command:         "rho-cli",
			GrantID:         "scud-local",
			GrantTTLSeconds: 3600,
			NetworkMode:     "provider_only",
		},
		LLM: LLMConfig{
			Provider:         "xai",
			Model:            "grok-4.20-multi-agent-0309",
			SmartProvider:    "xai",
			SmartModel:       "grok-4.7",
			FastProvider:     "xai",
			FastModel:        "grok-build-0.1",
			MultiAgentModel:  "grok-4.20-multi-agent-0309",
			MultiAgentEffort: "low",
			MaxTokens:        4096,
		},
		Heavy: HeavyConfig{
			Concurrency: 4,
			TimeoutSecs: 300,
		},
		Swarm: SwarmConfig{
			RoundSize:        5,
			MaxRalphAttempts: 3,
			TaskTimeoutSecs:  600,
			Tiers: TierConfig{
				Fast:     "grok-build-0.1",
				Standard: "grok-4.7",
				Smart:    "grok-4.7",
			},
			Backpressure: BackpressureCfg{
				StopOnFailure: true,
				TimeoutSecs:   300,
			},
		},
	}
}

func Load(scudDir string) (*Config, error) {
	cfg := Default()
	path := filepath.Join(scudDir, "config.toml")
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			cfg.applyEnv()
			return cfg, nil
		}
		return nil, err
	}
	if _, err := toml.Decode(string(data), cfg); err != nil {
		return nil, err
	}
	cfg.applyEnv()
	return cfg, nil
}

// LoadModelDefaults loads only explicitly configured defaults; a missing file is empty,
// rather than inheriting legacy baked-in provider/model choices.
func LoadModelDefaults(scudDir string) (*Config, error) {
	path := filepath.Join(scudDir, "config.toml")
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		return &Config{}, nil
	}
	if err != nil {
		return nil, err
	}
	cfg := &Config{}
	if _, err = toml.Decode(string(data), cfg); err != nil {
		return nil, fmt.Errorf("parsing %s: %w", path, err)
	}
	return cfg, nil
}

func (c *Config) applyEnv() {
	if v := os.Getenv("SCUD_EXECUTOR"); v != "" {
		c.Executor.Kind = v
	}
	if v := os.Getenv("SCUD_RHO_COMMAND"); v != "" {
		c.Executor.Command = v
	}
	if v := os.Getenv("SCUD_RHO_PROVIDER"); v != "" {
		c.Rho.Provider = v
	}
	if v := os.Getenv("SCUD_RHO_FAST_PROVIDER"); v != "" {
		c.Rho.FastProvider = v
	}
	if v := os.Getenv("SCUD_RHO_SMART_PROVIDER"); v != "" {
		c.Rho.SmartProvider = v
	}
	if v := os.Getenv("SCUD_MODEL"); v != "" {
		c.Rho.Model = v
	}
	if v := os.Getenv("SCUD_FAST_MODEL"); v != "" {
		c.Rho.FastModel = v
	}
	if v := os.Getenv("SCUD_SMART_MODEL"); v != "" {
		c.Rho.SmartModel = v
	}
	if v := os.Getenv("SCUD_HEAVY_MODEL"); v != "" {
		c.Heavy.Model = v
	}
	if v := os.Getenv("SCUD_HEAVY_MODE"); v != "" {
		c.Heavy.Mode = v
	}
	if v := os.Getenv("SCUD_HEAVY_MODEL_ROUTING"); v != "" {
		c.Heavy.Models.Routing = v
	}
	if v := os.Getenv("SCUD_HEAVY_MODEL_AGENTS"); v != "" {
		c.Heavy.Models.Agents = v
	}
	if v := os.Getenv("SCUD_HEAVY_MODEL_SYNTHESIS"); v != "" {
		c.Heavy.Models.Synthesis = v
	}
	if v := os.Getenv("SCUD_HEAVY_MODEL_DEBATE"); v != "" {
		c.Heavy.Models.Debate = v
	}
	if v := os.Getenv("SCUD_HEAVY_MODEL_NATIVE"); v != "" {
		c.Heavy.Models.Native = v
	}
	if v := os.Getenv("SCUD_HEAVY_CONCURRENCY"); v != "" {
		if n, err := strconv.Atoi(v); err == nil && n > 0 {
			c.Heavy.Concurrency = n
		}
	}
	if v := os.Getenv("SCUD_ROUND_SIZE"); v != "" {
		if n, err := strconv.Atoi(v); err == nil && n > 0 {
			c.Swarm.RoundSize = n
		}
	}

	// LLM overrides
	if v := os.Getenv("SCUD_PROVIDER"); v != "" {
		c.LLM.Provider = v
	}
	if v := os.Getenv("SCUD_SMART_PROVIDER"); v != "" {
		c.LLM.SmartProvider = v
	}
	if v := os.Getenv("SCUD_SMART_MODEL"); v != "" {
		c.LLM.SmartModel = v
	}
	if v := os.Getenv("SCUD_FAST_PROVIDER"); v != "" {
		c.LLM.FastProvider = v
	}
	if v := os.Getenv("SCUD_FAST_MODEL"); v != "" {
		c.LLM.FastModel = v
	}
	if v := os.Getenv("SCUD_MAX_TOKENS"); v != "" {
		if n, err := strconv.Atoi(v); err == nil && n > 0 {
			c.LLM.MaxTokens = n
		}
	}
	if v := os.Getenv("SCUD_MULTI_AGENT_MODEL"); v != "" {
		c.LLM.MultiAgentModel = v
	}
	if v := os.Getenv("SCUD_MULTI_AGENT_EFFORT"); v != "" {
		c.LLM.MultiAgentEffort = v
	}
}

// Save writes the config to config.toml.
func (c *Config) Save(scudDir string) error {
	path := filepath.Join(scudDir, "config.toml")
	f, err := os.Create(path)
	if err != nil {
		return err
	}
	defer f.Close()
	return toml.NewEncoder(f).Encode(c)
}

// SetModelTarget persists one main/fast default while leaving all other config text intact.
func SetModelTarget(scudDir, alias string, target ModelTarget) error {
	return SetModelTargetContext(context.Background(), scudDir, alias, target)
}

// SetModelTargetContext also cancels while waiting for a concurrent config writer.
func SetModelTargetContext(ctx context.Context, scudDir, alias string, target ModelTarget) error {
	if alias != "main" && alias != "fast" {
		return fmt.Errorf("model default must be main or fast")
	}
	if target.Provider == "" || target.Model == "" {
		return fmt.Errorf("provider and model are required")
	}
	if err := ctx.Err(); err != nil {
		return err
	}
	if err := os.MkdirAll(scudDir, 0700); err != nil {
		return err
	}
	lock, err := os.OpenFile(filepath.Join(scudDir, "config.toml.lock"), os.O_CREATE|os.O_RDWR, 0600)
	if err != nil {
		return err
	}
	defer lock.Close()
	if err = filelock.Lock(ctx, lock); err != nil {
		return err
	}
	defer filelock.Unlock(lock)
	path := filepath.Join(scudDir, "config.toml")
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		data = nil
	} else if err != nil {
		return err
	}
	text := string(data)
	if err := validateTOML(text, path); err != nil {
		return err
	}
	text, err = putTargetTable(text, alias, target)
	if err != nil {
		return err
	}
	rootKey := "main_model"
	if alias == "fast" {
		rootKey = "fast_model"
	}
	text, err = putTopLevelKey(text, rootKey, alias)
	if err != nil {
		return err
	}
	if err = validateTOML(text, "updated config"); err != nil {
		return fmt.Errorf("refusing invalid config update: %w", err)
	}
	if err = verifyModelUpdate(string(data), text, alias, target); err != nil {
		return err
	}
	tmp, err := os.CreateTemp(scudDir, "config-*.toml")
	if err != nil {
		return err
	}
	name := tmp.Name()
	defer os.Remove(name)
	if _, err = tmp.WriteString(text); err != nil {
		tmp.Close()
		return err
	}
	if err = tmp.Sync(); err != nil {
		tmp.Close()
		return err
	}
	if err = tmp.Close(); err != nil {
		return err
	}
	if err := ctx.Err(); err != nil {
		return err
	}
	if err = os.Rename(name, path); err != nil {
		return err
	}
	if err := filelock.SyncDir(scudDir); err != nil {
		return fmt.Errorf("model defaults published but directory sync failed: %w", err)
	}
	return nil
}

func validateTOML(text, path string) error {
	var doc map[string]any
	if _, err := toml.Decode(text, &doc); err != nil {
		return fmt.Errorf("parsing %s: %w", path, err)
	}
	return nil
}

// assignmentParts returns a normalized key, the value/comment boundary, and the equals position.
func assignmentParts(line string) (key string, eq, comment int, ok bool) {
	eq, comment = -1, -1
	quote := rune(0)
	escaped := false
	for i, r := range line {
		if quote != 0 {
			if escaped {
				escaped = false
				continue
			}
			if r == '\\' && quote == '"' {
				escaped = true
				continue
			}
			if r == quote {
				quote = 0
			}
			continue
		}
		if r == '"' || r == '\'' {
			quote = r
			continue
		}
		if r == '#' {
			comment = i
			break
		}
		if r == '=' && eq < 0 {
			eq = i
		}
	}
	if comment < 0 {
		comment = len(line)
	}
	if eq < 0 || eq >= comment {
		return "", eq, comment, false
	}
	raw := strings.TrimSpace(line[:eq])
	if raw == "" {
		return "", eq, comment, false
	}
	if raw[0] == '"' {
		if v, e := strconv.Unquote(raw); e == nil {
			return v, eq, comment, true
		}
	}
	if raw[0] == '\'' && raw[len(raw)-1] == '\'' {
		return raw[1 : len(raw)-1], eq, comment, true
	}
	return raw, eq, comment, true
}

func lineHeader(line string) ([]string, bool) {
	s := strings.TrimSpace(stripTOMLComment(line))
	if len(s) >= 4 && strings.HasPrefix(s, "[[") && strings.HasSuffix(s, "]]") {
		return []string{"\x00array"}, true
	}
	if len(s) < 2 || s[0] != '[' || s[len(s)-1] != ']' || strings.HasPrefix(s, "[[") {
		return nil, false
	}
	inside := strings.TrimSpace(s[1 : len(s)-1])
	var parts []string
	start := 0
	quote := rune(0)
	escaped := false
	for i, r := range inside {
		if quote != 0 {
			if escaped {
				escaped = false
				continue
			}
			if r == '\\' && quote == '"' {
				escaped = true
				continue
			}
			if r == quote {
				quote = 0
			}
			continue
		}
		if r == '"' || r == '\'' {
			quote = r
			continue
		}
		if r == '.' {
			parts = append(parts, strings.TrimSpace(inside[start:i]))
			start = i + 1
		}
	}
	parts = append(parts, strings.TrimSpace(inside[start:]))
	for i, p := range parts {
		if p == "" {
			return nil, false
		}
		if p[0] == '"' {
			v, e := strconv.Unquote(p)
			if e != nil {
				return nil, false
			}
			parts[i] = v
		} else if p[0] == '\'' && p[len(p)-1] == '\'' {
			parts[i] = p[1 : len(p)-1]
		}
	}
	return parts, true
}
func stripTOMLComment(s string) string {
	quote := rune(0)
	escaped := false
	for i, r := range s {
		if quote != 0 {
			if escaped {
				escaped = false
				continue
			}
			if r == '\\' && quote == '"' {
				escaped = true
				continue
			}
			if r == quote {
				quote = 0
			}
			continue
		}
		if r == '"' || r == '\'' {
			quote = r
			continue
		}
		if r == '#' {
			return s[:i]
		}
	}
	return s
}
func putTargetTable(text, alias string, target ModelTarget) (string, error) {
	lines := strings.Split(text, "\n")
	visible := tomlStatementLines(text)
	table := []string{"model_targets", alias}
	start, end := -1, len(lines)
	for i, l := range lines {
		if !visible[i] {
			continue
		}
		if h, ok := lineHeader(l); ok {
			if equalPath(h, table) {
				start = i
			} else if start >= 0 && i > start {
				end = i
				break
			}
		}
	}
	if start < 0 {
		if text != "" && !strings.HasSuffix(text, "\n") {
			lines = append(lines, "")
		}
		lines = append(lines, "[model_targets."+alias+"]", "provider = "+strconv.Quote(target.Provider), "model = "+strconv.Quote(target.Model))
		return strings.Join(lines, "\n"), nil
	}
	for _, kv := range []struct{ k, v string }{{"provider", target.Provider}, {"model", target.Model}} {
		found := false
		for i := start + 1; i < end; i++ {
			if !visible[i] {
				continue
			}
			if _, ok := lineHeader(lines[i]); ok {
				continue
			}
			k, eq, comment, ok := assignmentParts(lines[i])
			if ok && k == kv.k {
				suffix := ""
				if comment >= 0 && comment < len(lines[i]) {
					suffix = " " + strings.TrimSpace(lines[i][comment:])
				}
				lines[i] = lines[i][:eq+1] + " " + strconv.Quote(kv.v) + suffix
				found = true
				break
			}
		}
		if !found {
			lines = append(lines[:end], append([]string{kv.k + " = " + strconv.Quote(kv.v)}, lines[end:]...)...)
			end++
			visible = tomlStatementLines(strings.Join(lines, "\n"))
		}
	}
	return strings.Join(lines, "\n"), nil
}
func equalPath(a, b []string) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}
func putTopLevelKey(text, key, value string) (string, error) {
	lines := strings.Split(text, "\n")
	visible := tomlStatementLines(text)
	insert := len(lines)
	for i, l := range lines {
		if !visible[i] {
			continue
		}
		if _, ok := lineHeader(l); ok {
			insert = i
			break
		}
		k, eq, comment, ok := assignmentParts(l)
		if ok && k == key {
			suffix := ""
			if comment >= 0 && comment < len(l) {
				suffix = " " + strings.TrimSpace(l[comment:])
			}
			lines[i] = l[:eq+1] + " " + strconv.Quote(value) + suffix
			return strings.Join(lines, "\n"), nil
		}
	}
	lines = append(lines[:insert], append([]string{key + " = " + strconv.Quote(value)}, lines[insert:]...)...)
	return strings.Join(lines, "\n"), nil
}

// DefaultTOML returns the default config as TOML string.
func DefaultTOML() string {
	return `[rho]
provider = "xai"
model = "grok-4.7"
fast_provider = "xai"
fast_model = "grok-build-0.1"
smart_provider = "xai"
smart_model = "grok-4.7"

[executor]
# "legacy" preserves the original rho-cli flags; "rho-v1" uses rho.run/v1 JSONL.
kind = "legacy"
command = "rho-cli"
grant_id = "scud-local"
grant_ttl_seconds = 3600
network_mode = "provider_only"
# allowed_tools = ["read", "edit", "bash"]
# args = ["run", "--request-file", "-", "--events", "jsonl"]

[heavy]
# model = ""  # override-all fallback
# mode = ""   # "ensemble" (default), "native", "hybrid"
concurrency = 4
timeout_secs = 300

# Per-role model overrides (cheaper models for bulk work, smart for synthesis)
# [heavy.models]
# routing = "grok-build-0.1"
# agents = "grok-build-0.1"
# synthesis = "grok-4.7"
# debate = "grok-build-0.1"
# native = "grok-4.20-multi-agent-0309"

[swarm]
round_size = 5
max_ralph_attempts = 3
task_timeout_secs = 600

[swarm.tiers]
fast = "grok-build-0.1"
standard = "grok-4.7"
smart = "grok-4.7"

[swarm.backpressure]
commands = []
stop_on_failure = true
timeout_secs = 300
`
}

// HeavyModel resolves the model for a given heavy ensemble role.
// Priority: Heavy.Models.<role> > Heavy.Model > Rho.SmartModel > default.
// For "native": Heavy.Models.Native > LLM.MultiAgentModel > default.
func (c *Config) HeavyModel(role string) string {
	var perRole string
	switch role {
	case "routing":
		perRole = c.Heavy.Models.Routing
	case "agents":
		perRole = c.Heavy.Models.Agents
	case "synthesis":
		perRole = c.Heavy.Models.Synthesis
	case "debate":
		perRole = c.Heavy.Models.Debate
	case "native":
		perRole = c.Heavy.Models.Native
		if perRole != "" {
			return perRole
		}
		if c.LLM.MultiAgentModel != "" {
			return c.LLM.MultiAgentModel
		}
		return "grok-4.20-multi-agent-0309"
	}
	if perRole != "" {
		return perRole
	}
	if c.Heavy.Model != "" {
		return c.Heavy.Model
	}
	if c.Rho.SmartModel != "" {
		return c.Rho.SmartModel
	}
	return "grok-4.7"
}

// ModelForTier resolves a model tier to an actual model name.
func (c *Config) ModelForTier(tier string) string {
	switch tier {
	case "fast":
		return c.Swarm.Tiers.Fast
	case "smart":
		return c.Swarm.Tiers.Smart
	case "standard":
		return c.Swarm.Tiers.Standard
	default:
		return c.Swarm.Tiers.Standard
	}
}

// ProviderForTier resolves the provider paired with ModelForTier. Empty legacy
// configurations fall back to Rho.Provider and then model-prefix inference in
// the executor adapter.
func (c *Config) ProviderForTier(tier string) string {
	switch tier {
	case "fast":
		if c.Rho.FastProvider != "" {
			return c.Rho.FastProvider
		}
	case "smart":
		if c.Rho.SmartProvider != "" {
			return c.Rho.SmartProvider
		}
	}
	return c.Rho.Provider
}
