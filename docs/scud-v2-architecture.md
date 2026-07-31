# SCUD v2 migration boundary

This document records the first migration seam for v2. It is deliberately
additive: the current CLI and graph implementation remain the default until a
consumer opts into the v2 packages.

## Shape

The v2 core owns task orchestration, graph decisions, and lifecycle state. It
does not import a model provider, `rho-cli`, Shen, SQLite, SCG, or the `td`
command. Those concerns are represented by interfaces in
`v2/adapters`:

| concern | v2 seam | concrete integration |
| --- | --- | --- |
| bounded agent run | `adapters.Runner` | `v2/adapters/rho` (`rho.run/v1`) |
| authorization | `adapters.Policy` | `v2/adapters/shen` |
| append-only progress | `adapters.EventStore` | application-owned store; `adapters/memory` for tests |
| task directory (td) | `adapters.TaskDirectory` | SCG/SQLite/remote adapter; `adapters/memory` for tests |

`RunRequest.Model` is an opaque string. A provider-aware adapter may interpret
`provider/model`; the scheduler must only pass it through. Event `Data` is
opaque bytes for the same reason. This keeps provider credentials and policy
syntax out of graph packages and makes contract tests deterministic.

## DAG projection and ownership

The v2 event stream is the source of truth for goal and obligation lifecycle.
`v2/core.State` is a replayable projection of that stream; it is not a second
database and must never be updated by mutating fields in place. `v2/graph`
derives a deterministic DAG view and next decision from a projected state. It
does not own persistence, dispatch, credentials, or policy decisions.

Ownership is intentionally one-way:

1. An adapter (or CLI import) appends a domain event.
2. `core.Reduce` validates and applies it, producing a new state.
3. `graph`/`core.Reconcile` reads that state and proposes the next decision.
4. An executor/policy adapter performs or rejects the proposal and emits the
   resulting observation event.

Legacy SCG files and `td` databases are input/output projections during the
migration, never competing sources of truth. Their adapters must preserve
event order and IDs when importing, and must not silently write back status
changes outside the event append path. This lets v1 and v2 run side by side
while making ownership explicit.

## rho.run/v1 adapter

`v2/adapters/rho` is a thin translation layer over the existing
`pkg/executor` protocol implementation. It validates the adapter's model
syntax, translates requests and limits, and copies events before delivering
them to a v2 sink. The adapter is optional; callers can provide any
`adapters.Runner` implementation. Existing `pkg/executor.LegacyRho` remains
available for compatibility.

The JSONL fixtures under `v2/adapters/rho/testdata` are conformance examples:
`completed.jsonl` covers deltas and a terminal result, while
`invalid_after_terminal.jsonl` ensures consumers reject post-terminal events.
They can be replayed through `rho.Consume` without spawning a process.

## Shen policy adapter

`v2/adapters/shen` depends only on a local `Evaluator` interface, not a Shen SDK
version. A future SDK integration implements that interface and can be swapped
without changing v2 core. Policy failures are returned as errors; a successful
decision is explicit (`Allowed` plus a reason and optional constraints).

## Event and task stores

`EventStore.Append` is append-only and requires strictly increasing sequence
numbers per run. `List` accepts a cursor (`after`) so consumers can resume
without replaying the whole stream. `TaskDirectory` exposes only the task
projection needed by scheduling; richer legacy fields stay inside its adapter.
The in-memory implementations are test fixtures, not durable storage.

## Legacy package inventory

The following packages should be retained during the migration:

- `pkg/model`, `pkg/scg`, `pkg/wave`: stable graph/model APIs and SCG
  compatibility. New v2 code should consume a `TaskDirectory` projection
  rather than import these packages directly.
- `pkg/executor`: keep `Runner`, `LegacyRho`, and the rho.run/v1 protocol
  validator while downstream callers move to `v2/adapters`.
- `internal/db`, `internal/storage`, `internal/config`: retain for the v1 CLI
  and implement v2 adapters on top only after persistence semantics are tested.

The following should move behind adapters, then be deprecated once no v1
entrypoint imports them:

- `internal/rho`: legacy process invocation; use `v2/adapters/rho` for new
  execution and leave this package as a compatibility shim.
- direct `internal/db` event/session calls: expose them through an
  `EventStore` adapter so core code cannot depend on SQLite details.
- direct task-file reads/writes in command packages: expose them through a
  `TaskDirectory` adapter.

No package is deleted by this scaffold. A later release can add deprecation
markers and an integrated `scud v2` alias once the graph implementation and
adapter backends are ready; `scudv2` remains the low-risk standalone entrypoint
during migration.

## Migration sequence

1. Add adapter-backed entrypoints and replay the fixtures in CI.
2. Implement durable EventStore and TaskDirectory adapters, preserving v1
   locking and event ordering behavior.
3. Run v2 behind the explicit `scudv2` command/config switch; compare
   projections and decisions with the v1 scheduler.
4. Migrate callers package-by-package, then deprecate direct legacy imports.

## Opt-in CLI

`cmd/scudv2` is intentionally standalone so the existing `scud` command and
its legacy state files are unaffected. It accepts either a JSON object with an
`events` array (and optional `goal`/`budget`) or a bare JSON event array:

```sh
scudv2 validate goal.json
scudv2 plan goal.json
scudv2 replay goal.json
```

`validate` replays and checks invariants, `plan` prints the deterministic next
decision, and `replay` prints the projected state. The command performs no
agent invocation, provider lookup, policy evaluation, or persistence write.
