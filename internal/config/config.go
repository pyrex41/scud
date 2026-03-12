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
}

type HeavyConfig struct {
	Model       string `toml:"model"`       // "" = use rho.smart_model
	Concurrency int    `toml:"concurrency"` // default 4
	TimeoutSecs int    `toml:"timeout_secs"` // default 300
	MaxAgents   int    `toml:"max_agents"`   // 0 = no cap
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
			Model:      "grok-4.20-reasoning",
			FastModel:  "grok-code-fast-1",
			SmartModel: "grok-4.20-reasoning",
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
				Fast:     "grok-code-fast-1",
				Standard: "grok-4.20-reasoning",
				Smart:    "grok-4.20-reasoning",
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
model = "grok-4.20-reasoning"
fast_model = "grok-code-fast-1"
smart_model = "grok-4.20-reasoning"

[swarm]
round_size = 5
max_ralph_attempts = 3
task_timeout_secs = 600

[swarm.tiers]
fast = "grok-code-fast-1"
standard = "grok-4.20-reasoning"
smart = "grok-4.20-reasoning"

[swarm.backpressure]
commands = []
stop_on_failure = true
timeout_secs = 300
`
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
