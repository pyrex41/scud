# SCUD executor boundary

`pkg/executor` separates SCUD's task/wave scheduler from the harness that
performs one agent run. `swarm.RunOpts.Executor` accepts any implementation of
`Runner`; leaving it nil preserves the existing `rho-cli` behavior through
`LegacyRho`.

`RhoV1` is the opt-in adapter for the versioned `rho.run/v1` protocol. It sends
one JSON request on stdin and consumes JSONL events, validating protocol, run
identity, monotonic sequence numbers, terminal payloads, and the exactly-one
terminal invariant. Unknown event types remain visible to the callback so that
new producer events do not require a SCUD release.

```go
runner := executor.RhoV1{
    Grant: executor.Grant{
        GrantID: "local-scud",
        ExpiresAt: "2030-01-01T00:00:00Z",
        Providers: []string{"anthropic", "openai", "xai"},
        Models: []string{"*"},
        ReadRoots: []string{workspace},
        WriteRoots: []string{workspace},
        Network: executor.NetworkGrant{Mode: "provider_only"},
    },
}

err := swarm.Run(ctx, cfg, store, swarm.RunOpts{Executor: runner})
```

The adapter defaults to:

```text
rho-cli run --request-file - --events jsonl
```

Use `RhoV1.Command` and `RhoV1.Args` to target another conforming executable.
SCUD selects the adapter through `.scud/config.toml`:

```toml
[rho]
provider = "anthropic"
model = "claude-sonnet-4-5"
fast_provider = "xai"
fast_model = "grok-build-0.1"
smart_provider = "openai"
smart_model = "gpt-5.2"

[executor]
kind = "rho-v1" # default remains "legacy"
command = "rho-cli"
grant_id = "scud-local"
grant_ttl_seconds = 3600
allowed_tools = ["read", "edit", "bash"]
network_mode = "provider_only"
```

`scud run` and `scud swarm` accept `--executor legacy|rho-v1` and
`--provider anthropic|openai|xai` overrides. `scud run --model` pairs with the
explicit provider. Environment equivalents are `SCUD_EXECUTOR`,
`SCUD_RHO_COMMAND`, `SCUD_RHO_PROVIDER`, `SCUD_RHO_FAST_PROVIDER`, and
`SCUD_RHO_SMART_PROVIDER`.

The local grant is deliberately rooted to the active project workspace and has
a finite lifetime. A future control-plane integration should replace it with a
signed externally authorized grant rather than expanding this local config.
