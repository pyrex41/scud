package config

import (
	"os"
	"path/filepath"
	"strconv"

	"github.com/BurntSushi/toml"
)

type Config struct {
	Rho   RhoConfig   `toml:"rho"`
	Swarm SwarmConfig `toml:"swarm"`
	Heavy HeavyConfig `toml:"heavy"`
	LLM   LLMConfig   `toml:"llm"`
}

type LLMConfig struct {
	Provider           string `toml:"provider"`
	Model              string `toml:"model"`
	SmartProvider      string `toml:"smart_provider"`
	SmartModel         string `toml:"smart_model"`
	FastProvider       string `toml:"fast_provider"`
	FastModel          string `toml:"fast_model"`
	MultiAgentModel    string `toml:"multi_agent_model"`
	MultiAgentEffort   string `toml:"multi_agent_effort"` // "low", "medium", "high", "xhigh"
	MaxTokens          int    `toml:"max_tokens"`
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
	Model       string            `toml:"model"`       // override-all fallback
	Models      HeavyModelsConfig `toml:"models"`      // per-role overrides
	Mode        string            `toml:"mode"`         // "ensemble", "native", "hybrid"
	Concurrency int               `toml:"concurrency"` // default 4
	TimeoutSecs int               `toml:"timeout_secs"` // default 300
	MaxAgents   int               `toml:"max_agents"`   // 0 = no cap
}

type RhoConfig struct {
	Model      string `toml:"model"`
	FastModel  string `toml:"fast_model"`
	SmartModel string `toml:"smart_model"`
}

type SwarmConfig struct {
	RoundSize        int              `toml:"round_size"`
	MaxRalphAttempts int              `toml:"max_ralph_attempts"`
	TaskTimeoutSecs  int              `toml:"task_timeout_secs"`
	Tiers            TierConfig       `toml:"tiers"`
	Backpressure     BackpressureCfg  `toml:"backpressure"`
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
			Model:      "grok-4.3",
			FastModel:  "grok-build-0.1",
			SmartModel: "grok-4.3",
		},
		LLM: LLMConfig{
			Provider:         "xai",
			Model:            "grok-4.20-multi-agent-0309",
			SmartProvider:    "xai",
			SmartModel:       "grok-4.3",
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
				Standard: "grok-4.3",
				Smart:    "grok-4.3",
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

func (c *Config) applyEnv() {
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

// DefaultTOML returns the default config as TOML string.
func DefaultTOML() string {
	return `[rho]
model = "grok-4.3"
fast_model = "grok-build-0.1"
smart_model = "grok-4.3"

[heavy]
# model = ""  # override-all fallback
# mode = ""   # "ensemble" (default), "native", "hybrid"
concurrency = 4
timeout_secs = 300

# Per-role model overrides (cheaper models for bulk work, smart for synthesis)
# [heavy.models]
# routing = "grok-build-0.1"
# agents = "grok-build-0.1"
# synthesis = "grok-4.3"
# debate = "grok-build-0.1"
# native = "grok-4.20-multi-agent-0309"

[swarm]
round_size = 5
max_ralph_attempts = 3
task_timeout_secs = 600

[swarm.tiers]
fast = "grok-build-0.1"
standard = "grok-4.3"
smart = "grok-4.3"

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
	return "grok-4.20-reasoning"
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
