# Review Handoff — Reduced Task Core

Read [REDESIGN](REDESIGN.md), then [V2_SPEC](V2_SPEC.md), then current
[README](../README.md)/[developer guide](../go.md) and code. This handoff
supersedes the earlier execution-heavy v2 review instructions.

## 1. User-directed scope

Scud focuses on PRD -> tasks, ingest/process/read/update task lists, and useful
live status commands for scripts. sb stays optional. Coding-agent execution
is dropped from the public API. Ensemble inference is experimental. Model
selection is main/fast with catalog discovery and defaults set through CLI,
without provider-invocation spelling or manual configuration editing.

Review those goals adversarially. Do not recommend implementing the old role/
tier cascade, execution scheduler or evidence-gated status transition merely
because legacy code remains. Workflow status is user-reported; verification
is separate. The earlier scud `866d0f4` / sb
`6b9dde09b3a98ee5d20d0a6556acceacad4657d8` review is historical context.

## 2. Read-only review rules

Record actual branch/revision and `git status --short`; docs may be uncommitted.
Inspect AGENTS.md if present. Do not modify source, task files, configuration,
credentials or docs; do not commit or launch real coding agents. Temporary
projects/fake provider and sb fixtures are appropriate. No live paid inference
is required. Distinguish runtime reproductions, source inspection and proposed
contracts. No calendar estimates/schedule in the review deliverable.

```sh
go build ./...
go test ./...
go test -race ./internal/taskapi ./internal/mcp ./internal/sbintegration
```

## 3. Reference map / claims audit

Every item is **[VERIFY]** against the actual new tree; no old line numbers are
assumed to apply.

| Surface | Source | Claim to test |
|---|---|---|
| Public API | `cmd/scud/main.go`, `cmd/scud/main_test.go` | Only reduced commands; run/swarm/commit/old utilities not registered |
| Task model/storage | `internal/taskapi/service.go`, strict JSON/lock helpers | Full-field lossless document, strict input, no hidden legacy shadowing |
| Concurrency | Task service + tests | Stable lock, atomic publish, directory sync, revision guard checked under lock; no-op stability |
| Readiness | Task service validator | Qualified refs, inherited parents, external dependencies, cycles and duplicate refs handled before ready |
| Scripts | `internal/cmd/tasks_workflow.go` | JSON stdout, nonzero errors, import/export shape, changed-revision JSONL and cancellation |
| Init | `internal/cmd/init.go` | Repeated init never overwrites tasks/defaults or invents models |
| Models | `internal/cmd/models_core.go`, `internal/config`, `pkg/llm` | Main/fast only in core; authenticated discovery, friendly matching, no ambiguous write or lost default |
| Proposal | `internal/cmd/prd_core.go`, `pkg/generate.Propose` | No persistence, bounded input, no agent/tier rules, task-service-compatible validation |
| Inference | Explicit-target client/provider | No regex salvage, no foreign credential scraping, no hidden coding harness; refusal/truncation rejected |
| Optional sb | `internal/sbintegration`, `internal/cmd/sb_core.go` | Public commands only, bounded/cancellable logs, missing binary explicit, report always unverified |
| Ensemble | `internal/cmd/experimental_core.go` | Direct model calls, bounded fan-out, failure prevents synthesis; no tools/execution |
| MCP | `internal/mcp/server.go` | Five default tools, optional sb only, shared mutation/validation policy and clean stdio |
| Legacy scope | README + source registration | Legacy packages retained honestly, no advertised SCG-v2 codec or strict migrator |

For upstream checks, clone Shen-Backpressure to a temporary directory, record
its revision and compare to the review pin. Source paths of interest there:
`cmd/sb/gates.go`, `derive.go`, `discharge.go`, `context.go`, `loop.go`.
Successful gates can have no report; custom gate manifests can omit default
audit checks. No sb harness adapter table or universal spec-only compile
command should be inferred from README prose.

Model API/CLI documentation must be fetched when investigating current
capabilities. Fixture support is not a live account guarantee. Catalog entries
can exist without supporting the inference mode used by parse.

## 4. Mandatory challenge questions

The first three SME reviews produced ten actionable issues. Fixes now have
regressions; independently replay them rather than treating green unit tests
as proof of correctness:

| Issue | Regression surface |
|---|---|
| OpenRouter model ID stripping | Catalog selection -> persisted target -> submitted model ID is verbatim |
| Multiline TOML corruption/false success | Assignment/table-looking text inside strings stays unchanged; candidate target resolves correctly |
| Non-transitive mixed-ID sorting | Total ordering properties and revision/ready permutations for 2/10/1a and oversized numeric segments |
| Incomplete filtered export | CLI/MCP phase selection includes transitive prerequisite-phase closure and validates alone |
| Launcher exits before descendants | Linux/macOS actual child liveness with inherited and closed stdio, before launcher reaping |
| Blocked input ignores signals | Real CLI SIGTERM with input pipe open/no data, including ensemble input deadline |
| Catalog cancellation loses identity | Preserve context cancellation/deadline and refuse partial success after cancellation |
| MCP EOF retains work | Real stdio EOF during an sb call; HTTP disconnect during active handler |
| MCP duplicate/int precision loss | Actual wire duplicate nested keys rejected; integer 9007199254740993 persists exactly on stdio and HTTP |
| Empty dependency no-op churn | Omitted/null/empty input, clear already empty, unchanged timestamp/revision/file bytes |

MCP must be stateless **2026-07-28**, using official SDK v1.8.0, not a
sessionful server merely labelled JSON-RPC 2.0. Discovery/tool calls work
without initialization. Repeat valid request -> missing/absent/null metadata
to ensure SDK backwards compatibility cannot authorize a later legacy request.
Test matching HTTP version/method/tool-name headers, no session IDs, no legacy
GET/DELETE stream endpoints, cross-origin rejection and independent cancellation.

1. **API shrink:** Are removed commands actually unreachable from root/MCP?
   Is there any hidden worker/commit/role-routing path in the new workflow?
2. **Round trips:** Do JSON import/list/patch preserve all task strings,
   metadata, timestamps and relationships? Test large prose, Unicode, CR/LF,
   raw delimiters, dotted/numeric/UUID IDs, unknown/duplicate JSON keys and
   trailing values. Does legacy refusal prevent accidental state divergence?
3. **Atomicity:** Try malformed replacements, cancelled writes, concurrent
   processes and stale revisions. Could a failed operation partially publish?
   Are post-rename durability failures labelled honestly?
4. **Readiness/status:** Test repeated IDs across phases, dependencies opposite
   phase sort order, parent inheritance/cycles and empty lists. Do all status
   counts include zeroes? Is watch documented as polling snapshots, not a full
   event stream? Does it terminate on cancellation?
5. **Main/fast UX:** With no configured defaults, can a user discover and select
   a model without API syntax/TOML editing? Test friendly name ambiguity,
   duplicate provider IDs, partial catalog failures, pagination, no credentials,
   and concurrent default changes. Is provider identity preserved explicitly?
6. **Proposal safety:** Try empty/huge PRDs, negative count, invalid tag,
   duplicate/missing/cyclic dependencies, refusal/truncation, unknown stop
   reasons and JSON fragments. Can any failure mutate task or source files?
7. **sb honesty:** Reproduce gate success without a report and with an old or
   changed report. Are stdout/stderr retained on failure? Can cancellation
   leave build/test descendants running? Is core CRUD independent of sb?
8. **Experimental isolation:** Are ensembles explicitly unstable, bounded and
   tool-free? Do failed members skip synthesis? Does no experimental feature
   enter the default MCP or become necessary for ordinary task workflows?

## 5. Deliverable

Also check documentation drift: `docs/CLI_REFERENCE.md` must match generated
Cobra help; every registered real command should explain behavior and include
valid examples. Current `.claude`/`.opencode` skill copies must match, and live
project/alias/hook guidance must not invoke retired commands. Replay the two
advice-only skill cases; verify no task mutation/provider calls are executed.
Old research must be marked historical, not silently rewritten as current fact.

- Verdict: implement/use, use with changes, or rethink, scoped to the new core.
- Severity-tagged findings with exact current `file:line`, reproduction and fix.
- Direct answers to all eight questions and a complete [VERIFY] claims audit.
- Clear distinction between tested implementation, documentation-only claims,
  deferred legacy migration/native output adapters, and untested live accounts.
- Remaining work/dependency corrections only—no dates, duration estimates or
  calendar schedule. Do not silently expand the product back into execution.
