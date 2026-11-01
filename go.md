# SCUD — Developer Guide

## Current product boundary

The root CLI in `cmd/scud/main.go` registers task workflows, PRD proposals,
model-default management, optional sb commands, experimental model ensembles,
and MCP. It no longer registers coding-agent execution, swarm, commit,
attractor, or the large utility-command surface. Legacy implementations remain
in the tree during staged cleanup; their existence is not a public API promise.

## Build and test

The module and CI use **Go 1.27.3**, the latest stable release checked against
the official Go download index. No experimental flags are enabled. CI reads
the toolchain from `go.mod`; maintainers should update that pin for patch releases.

```sh
go build ./...
go test ./...
go test -race ./internal/taskapi ./internal/mcp ./internal/sbintegration
```

Modern language/library usage is focused on real boundaries:

- Stable `encoding/json/v2` + `jsontext` reject malformed/duplicate input;
  existing JSON encoding remains v1 intentionally to preserve wire/revision shape.
- `taskapi.Decode[T]` and `llm.RequestJSON[T]` provide typed results, with no
  partial value returned on failure. Narrow non-generic ports stay mockable.
- `slices.SortFunc`/`cmp.Compare` replace untyped sorting in the core graph.
- `sync.WaitGroup.Go` owns ensemble goroutine registration/completion.
- `errors.AsType` exposes typed revision conflicts and dedicated script exits.
- `new(value)` and `t.Context()` simplify typed patch/test fixtures.

Do not introduce generic repositories, generic-method wrappers, experimental
flags or broad legacy rewrites just to exercise features. The abstraction
should remove duplication or an unsafe/untyped boundary.

No live model/sb installation is needed for the fixture suites. Tests use
temporary projects, fake catalogs/HTTP responses and fake sb executables.
Root-command tests restore working directories and must not run in parallel.

## Code map

| Area | Responsibility |
|---|---|
| `cmd/scud/main.go` | Small public surface, `-C`, cancellation and stderr errors |
| `internal/taskapi` | Versioned JSON document, strict decode, validation, readiness, revisions, locking/atomic publication |
| `internal/cmd/tasks_workflow.go` | Import/list/show/update/next/status/validate; JSON output and polling JSONL |
| `internal/cmd/prd_core.go`, `pkg/generate.Propose` | Read-only PRD proposal using a selected main/fast model; shared graph validation |
| `internal/config` | Explicit provider/model identity and CLI-managed main/fast settings; legacy config compatibility |
| `pkg/llm` | Authenticated model catalogs, explicit-target inference and strict response handling |
| `internal/cmd/models_core.go` | Friendly catalog search, ambiguity errors, persisted defaults |
| `internal/sbintegration` | Public sb subprocess wrapper, bounded logs, optional unverified report digest |
| `internal/cmd/experimental_core.go` | Bounded direct-model fan-out and synthesis; no worker harness |
| `internal/mcp` | Five tools + opt-in sb; official SDK, stateless 2026-07-28 stdio/HTTP, raw JSON bindings |
| `internal/inputio` | Bounded cancellation-aware owned input; no abandoned reader goroutines |

The reduced task service is the authority for new CLI/MCP mutations. Existing
`internal/storage`/SCG code is not the new write path. Never bypass the service
with tolerant parse-and-save helpers, even if a legacy command still does so.

## Contracts to protect

- `Document`: `schema_version=1`, phases with tag/name/id format and full tasks.
- `Snapshot`: schema version, content revision, phases, counts, ready refs and
  dependency-blocking reasons. Status counts include zeroes for known states.
- IDs are phase-local; unqualified lookup rejects ambiguity. Qualified refs
  and inherited parent dependencies are resolved before declaring readiness.
- Strict JSON rejects duplicates/unknown fields/trailing values. Validate the
  entire candidate graph before publishing imports or patches.
- Lock a stable sidecar, not an inode replaced by rename. Sync temp content,
  rename under lock, then sync its directory on Unix. Windows uses native
  byte-range locking; directory-fsync durability is not promised there. A post-publication sync failure
  must not be described as “nothing changed.”
- No-op patches preserve timestamps/revisions; imports preserve supplied
  metadata. `--if-revision` is checked inside the mutation lock.
- CLI machine output goes to Cobra writers; no UI/log banners on stdout.
- MCP uses low-level official SDK `Server.AddTool`, not generic `AddTool` map
  normalization. Validate the raw frame before SDK decode, then raw arguments
  into concrete types; preserve integer lexemes and reject duplicates. Test
  actual stdio/HTTP, not only handlers constructed with already-decoded maps.
- Require current protocol/capability metadata on **every** request, including
  after valid prior requests. Disable SDK legacy handshake/fallback behavior.
  HTTP uses Stateless + JSONResponse + PropagateRequestCancellation; stdio
  EOF must cancel active handlers. No `Mcp-Session-Id` or legacy GET/SSE API.
- Main/fast defaults store provider and canonical model ID together. Never
  infer a provider from an ID or reintroduce task-tier routing.
- sb output is orthogonal to workflow status. Report presence/change is not
  current-run provenance or coverage. Do not parse sb.toml/Shen to fill gaps.

## Follow-up scope

Strict raw-SCG migration, schema-generated API documentation, removal of
unregistered legacy packages/dependencies, and native constrained-output
capability adapters remain explicit follow-up work. The current version does
not implement the superseded execution/evidence scheduler described by the
earlier redesign drafts. See [V2_SPEC](docs/V2_SPEC.md) for scope and acceptance,
not a calendar schedule.
