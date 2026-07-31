package cmd

import (
	"fmt"
	"time"

	"github.com/reuben/scud/internal/config"
	agentexec "github.com/reuben/scud/pkg/executor"
)

func configuredExecutor(cfg *config.Config, kind, providerOverride, modelOverride, workspace string) (agentexec.Runner, error) {
	if kind == "" {
		kind = cfg.Executor.Kind
	}
	switch kind {
	case "", "legacy":
		return agentexec.LegacyRho{}, nil
	case "rho-v1", agentexec.RhoRunV1:
		return configuredRhoV1(cfg, providerOverride, modelOverride, workspace, time.Now()), nil
	default:
		return nil, fmt.Errorf("unknown executor %q (want legacy or rho-v1)", kind)
	}
}

func configuredRhoV1(cfg *config.Config, providerOverride, modelOverride, workspace string, now time.Time) agentexec.RhoV1 {
	ttl := cfg.Executor.GrantTTLSeconds
	if ttl <= 0 {
		ttl = 3600
	}
	providers := uniqueStrings(providerOverride, cfg.Rho.Provider, cfg.Rho.FastProvider, cfg.Rho.SmartProvider)
	models := uniqueStrings(modelOverride, cfg.Rho.Model, cfg.Rho.FastModel, cfg.Rho.SmartModel, cfg.Swarm.Tiers.Fast, cfg.Swarm.Tiers.Standard, cfg.Swarm.Tiers.Smart)
	grantID := cfg.Executor.GrantID
	if grantID == "" {
		grantID = "scud-local"
	}
	networkMode := cfg.Executor.NetworkMode
	if networkMode == "" {
		networkMode = "provider_only"
	}
	return agentexec.RhoV1{
		Command: cfg.Executor.Command,
		Args:    append([]string(nil), cfg.Executor.Args...),
		Grant: agentexec.Grant{
			GrantID:    grantID,
			ExpiresAt:  now.UTC().Add(time.Duration(ttl) * time.Second).Format(time.RFC3339),
			Providers:  providers,
			Models:     models,
			Tools:      append([]string(nil), cfg.Executor.AllowedTools...),
			ReadRoots:  []string{workspace},
			WriteRoots: []string{workspace},
			Network:    agentexec.NetworkGrant{Mode: networkMode},
		},
	}
}

func uniqueStrings(values ...string) []string {
	seen := make(map[string]struct{}, len(values))
	result := make([]string, 0, len(values))
	for _, value := range values {
		if value == "" {
			continue
		}
		if _, ok := seen[value]; ok {
			continue
		}
		seen[value] = struct{}{}
		result = append(result, value)
	}
	return result
}
