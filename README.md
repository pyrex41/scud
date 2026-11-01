# SCUD

**PRDs to task lists. Safe updates. Live status for scripts.**

Scud manages dependency-aware task lists. It does not run coding agents or
decide that a task is verified just because someone marked it done. Optional
[Shen-Backpressure](https://github.com/pyrex41/Shen-Backpressure) commands are
available, but ordinary task workflows require neither sb nor model credentials.

## Install

Requires Go **1.27.3** or newer. The module pins the reviewed latest stable
toolchain; Go's automatic toolchain selection can download it when needed.

```sh
go install ./cmd/scud
# Or: go build -o scud ./cmd/scud
```

## Start

```sh
scud init
scud status
```

Task commands output JSON by default. `--json` is accepted for explicit scripted
usage. Errors go to stderr with a nonzero exit status. Use `-C PATH` to work in
another project.

Exit statuses: `1` ordinary error, `3` revision conflict, `124` deadline,
`130` cancellation. Successful reads (including an empty ready list) return 0.

## Models: main and fast

Set your provider's credential in its supported environment variable:
`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `XAI_API_KEY`, or `OPENROUTER_API_KEY`.
Scud does not scrape credentials from other applications.

```sh
scud models list
scud models list --search 'sonnet'
scud models set main 'sonnet'
scud models set fast 'haiku'
scud models defaults
```

These are catalog searches, not hard-coded model recommendations. Available
models depend on the provider/account. Matching is case-insensitive across
provider, display name and ID; ambiguous matches show choices and change
nothing. Narrow a query or add `--provider anthropic` to disambiguate. You do
not need to type provider invocation syntax or edit configuration files.

`main` and `fast` can select the same model. There are no task roles, model
tiers, or automatic complexity-based model selection in the core workflow.
No model is silently selected for a fresh project. Defaults are project-local;
`models set` preserves unrelated configuration. `models list` can run before
initialization; initialize a project before setting its defaults.

```sh
scud models list --json
scud models defaults --json
scud models set main 'a uniquely matching catalog name' --json
```

## PRD → proposal → task list

```sh
scud parse product.md > tasks.json
scud validate --input tasks.json
scud import tasks.json
```

`parse` uses `main`; `--model fast` selects `fast`. A file, `-`, or stdin is
accepted. `--tag NAME` names the proposal phase, and optional `--num N` requests
a target count. Parsing never persists tasks or edits source files; inspect
the proposal before importing. Generation currently uses strict JSON decoding
and graph validation, not a universal native constrained-output guarantee.

## Ingest, read and update

```sh
scud list > exported-tasks.json
scud show main:1
scud next
scud update main:1 --status in-progress
scud update main:1 --status done
scud update main:2 --depends-on main:1 --assigned-to alice
scud update main:2 --depends-on=   # explicitly clear dependencies
```

Unqualified IDs work only when unambiguous; use `--tag NAME` or `NAME:ID`
otherwise. Dependencies use a local ID or `phase:id`. Missing references,
cycles, duplicate IDs and inconsistent parent relationships are errors.
`next` returns only the first ready reference (`{"tag":"main","id":"1"}`), or
`null` when none is ready; it does not claim or run work. Use `status` for all
ready references. `show` returns a JSON wrapper containing `ref` and `task`.

Imports refuse phase collisions by default. `--policy merge` appends new
tasks to matching phase metadata, rejecting duplicate IDs; `--policy replace`
replaces only phases named in the input, not the whole project.

```sh
revision=$(scud status | jq -r .revision)
scud update main:1 --status review --if-revision "$revision"
printf '%s\n' '{"description":"New details","dependencies":[]}' |
  scud update main:2 --input -
```

`import --if-revision HASH` also protects scripted replacement. Omitted patch
fields are unchanged. Revision conflicts and invalid changes leave the task
document untouched. Updates are locked across processes and atomically
published. Workflow status is user-reported, not a verification certificate.
`--if-revision` is an optimistic guard, not a lease or claim. Use JSON patch
input or field flags, never both; `--depends-on` replaces the whole list.

### Import/export document

```json
{
  "schema_version": 1,
  "phases": [{
    "tag": "main",
    "name": "Main",
    "id_format": "sequential",
    "tasks": [{
      "id": "1",
      "title": "Implement the feature",
      "description": "Goal and acceptance criteria",
      "status": "pending",
      "priority": "medium",
      "complexity": 3,
      "dependencies": []
    }]
  }]
}
```

`list` exports this document; `status` returns a richer snapshot. Strings and
all existing task fields, including description/details/test strategy, survive
JSON round trips. Unknown fields, duplicate JSON keys and trailing values are
rejected. IDs/tags may not contain whitespace, controls, `:`, `/`, `\` or `|`.

`list --tag app` includes the whole `app` phase **and its transitive prerequisite
phases**, so the result still validates/imports independently. It never drops
dependency edges to make a partial view look complete. MCP tag-filtered document
reads follow the same rule; single-task and ready-ref queries remain scoped views.

## Live status

```sh
scud status | jq '{revision, summary, ready, blocked}'
scud status --watch --interval 500ms | jq -c '{revision, summary}'
```

`status` reads current persisted state, not cached process state. Watch emits
an initial JSON line and another on each observed revision change; it is a
polling snapshot stream, not an event log or heartbeat. Polling is clamped to
at least 100ms. Ctrl-C stops watching. `blocked` explains dependency-blocked
pending leaf tasks; explicit blocked/review/deferred statuses also appear in
the status counts. Cross-phase and inherited dependencies affect readiness.

## Optional sb

```sh
scud sb status --json
scud sb check --json
scud sb context
scud sb derive --json
scud sb audit
scud sb gen
```

These wrap supported public sb commands. A project marker with no sb binary
is an explicit error when invoking checks; absent sb never blocks task CRUD.
`check` runs gates, `gen` generates guards, and neither is advertised as a
spec-only compiler. Derive/gen/check may change generated project files.
Commands are bounded by `--timeout`; output is bounded with an explicit
truncation marker. JSON output preserves stdout/stderr even on command failure.
On Linux/macOS the invocation owns its private process group until the launcher
is reaped: early successful launcher exit also cleans up remaining descendants.
Processes that deliberately detach into another group are outside that boundary.

Passing gates may produce **no discharge report**. Existing or changed report
bytes remain labelled `unverified`; scud does not infer freshness, rule
coverage, proof strength or a task's completion from them.

## Experimental ensemble

```sh
scud experimental ensemble 'Review this design' --models main,fast
scud experimental ensemble 'Compare approaches' --models main,main --synth main
```

Independent direct-model answers followed by synthesis. No coding harnesses,
filesystem tools or task execution. Failed members prevent synthesis and
produce a nonzero exit with structured diagnostics. This command is explicitly
experimental and is not exposed through MCP.

## MCP

```sh
scud mcp-server
SCUD_TOOLS=full scud mcp-server
scud mcp-server --transport http --listen 127.0.0.1:8080
```

Five default tools: `scud_tasks` (list/show/ready), `scud_import`, `scud_update`,
`scud_status`, `scud_parse`. `full` adds `scud_sb`; an explicit comma-separated
selection such as `SCUD_TOOLS=tasks,status,sb` is supported. Unknown tool names
are errors. No execution, commit or ensemble tools are registered.

This is **stateless MCP 2026-07-28**, using the official Go SDK v1.8.0 (SDK
version and protocol generation are different). No `initialize` handshake,
`notifications/initialized`, persistent HTTP session ID, or legacy GET/SSE
connection is supported. Discovery is `server/discover` and every request
supplies its own protocol version and client capabilities. Missing metadata
is rejected even after a valid earlier request; no capabilities are inferred
from connection history.

For example, this is a complete stdio discovery request, not a handshake:

```json
{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}
```

HTTP uses POST `/mcp`, with `Content-Type: application/json`, `Accept:
application/json, text/event-stream`, and matching `Mcp-Protocol-Version` /
`Mcp-Method` headers. Tool calls also require `Mcp-Name` matching the tool name.
Only loopback IP listeners are allowed; there is no built-in remote auth service.
Cross-origin browser requests are rejected. Normal responses are JSON, not a
resumable SSE stream. Raw frames and typed tool arguments are validated before
SDK normalization, preserving integers and rejecting duplicate members.
Stdio EOF and HTTP request disconnect cancel their in-flight operations;
HTTP shutdown waits briefly for cleanup. The old MCP implementation is removed.

## Compatibility and limits

- The new authoritative store is `.scud/tasks/tasks.json`. A nonempty legacy
  `tasks.scg` is refused with conversion guidance, not silently imported or
  shadowed. Keep an exact backup and explicitly review conversion before
  archiving that file. Automated strict legacy migration is still follow-up
  work; the new CLI does not pretend the tolerant old parser is lossless.
- Old execution/utility command implementations remain in the source tree for
  staged cleanup, but are not registered in the public CLI/MCP. Do not mix old
  binaries/commands writing SCG with the new JSON store.
- Process locking is implemented on Unix and Windows. File content is synced
  before publication; directory metadata is additionally synced on Unix.
  Windows power-loss metadata durability is not promised.
- Catalog and model HTTP behavior is tested with fixtures, not live account
  access. Catalog visibility does not guarantee every listed model supports
  the text-generation request mode.
- Windows/other-platform sb cleanup currently stops the direct child only;
  Linux/macOS additionally supervise its private process group. Windows console
  input is rejected where it cannot be interrupted safely; scripted pipes work.

See the generated [CLI reference](docs/CLI_REFERENCE.md) for exact command help,
[the work specification](docs/V2_SPEC.md), [rationale](docs/REDESIGN.md),
and [developer guide](go.md). The [documentation index](docs/README.md) also
links the current agent skills and distinguishes historical research.
