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
The next migration slice should expose executor selection and grant construction
through SCUD configuration/CLI, after the Rho CLI command shape is finalized.
