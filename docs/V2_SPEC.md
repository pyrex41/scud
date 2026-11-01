# scud — Reduced Task Workflow Specification and Work Scope

Status: scope revised after the initial review and subsequent user direction.
Current product line: **SCUD 3.x**. This filename is retained for links to the
original design work; it does not indicate the released product version.
This document supersedes the earlier execution-heavy v2 proposal. It describes
the new core and explicitly separates implemented contracts from follow-up
work. No calendar schedule or duration estimates are part of this plan.

## 1. Scope

Scud provides:

1. PRD text -> validated, importable task proposal.
2. Ingest/read/update task lists safely and consistently.
3. Fresh status/counts/readiness for scripted workflows, including JSONL watch.
4. Optional sb commands that work independently of task state.
5. Simple main/fast model discovery and CLI-managed defaults.
6. Explicitly experimental ensemble inference.

Non-goals: coding-agent execution, swarm/repair scheduling, committing source,
role/profile/tier routing, automatic task-to-model assignment, Shen parsing,
proof claims from file presence, or a second task-state database.

Workflow status is user-reported. Neither `done` nor a passing command is an
invariant discharge claim. Verification metadata must remain orthogonal if
added later, with independently validated provenance/coverage contracts.

## 2. Public surface

| Command | Contract |
|---|---|
| `init` | Initialize empty task storage; never overwrite existing tasks/config |
| `parse [FILE|-]` | Propose JSON from PRD; main by default, optional `--model fast`, `--tag`, `--num`; no persistence |
| `import [FILE|-]` | Strict document ingestion; refuse phase collisions unless explicit merge/replace |
| `list [--tag TAG]` | Export an importable document |
| `show ID [--tag TAG]` | Read one task; ambiguity is an error |
| `update ID` | Patch explicitly supplied fields, optionally guarded by revision |
| `next [--tag TAG]` | Return the first ready `{tag,id}` reference, or JSON `null`; never claim or execute work |
| `status [--watch]` | Fresh snapshot; watch is initial + changed-revision JSONL |
| `validate [--input FILE|-]` | Validate project or independent candidate document without task mutation |
| `models list/defaults/set` | Discover models and choose main/fast without hand-editing config |
| `sb status/check/context/derive/audit/gen` | Optional bounded public sb wrappers |
| `experimental ensemble` | Direct-model fan-out and synthesis; no coding tools |
| `mcp-server` | Stateless MCP 2026-07-28 over stdio or loopback HTTP |

Task/init/parse/ensemble commands emit JSON by default. Status watch uses one
complete JSON object per line. Model commands support human output and `--json`.
sb supports human streams and complete structured `--json` results. No banners,
progress text or credential material contaminate machine stdout. Errors go
to stderr and return nonzero. Invalid input must not publish partial state.
The root supports `-C PATH`, help/version and context cancellation.

Execution/commit/attractor/utility commands are no longer registered; retained
legacy source is not a compatibility promise. Experimental APIs are excluded
from core MCP. Unknown custom MCP tool names fail explicitly.

## 3. Task document and service

The authoritative store is `.scud/tasks/tasks.json`, with document shape:

```json
{
  "schema_version": 1,
  "phases": [{
    "tag": "main",
    "name": "Main",
    "id_format": "sequential",
    "tasks": [{
      "id": "1", "title": "Goal", "description": "Acceptance criteria",
      "status": "pending", "priority": "medium", "complexity": 3,
      "dependencies": []
    }]
  }]
}
```

Tasks retain all `pkg/model.Task` fields, including distinct description,
details and test strategy, assignments, timestamps, agent/tier/model metadata
and parent/subtask relationships. Retained routing metadata is inert in the
core workflow. Do not remove it during import/export or interpret it as an
execution instruction. Phase tag, display name and ID format remain distinct.

Validation:

- Schema version exactly 1; strict unknown-field, duplicate-key and trailing
  value rejection. Independent validation and mutation share one validator.
- Nonempty phase/task IDs; no whitespace, controls, `:`, `/`, `\`, `|`, or
  standalone `.`/`..`. Numeric, dotted and UUID IDs are supported.
- Unique phase IDs and phase-local task IDs; nonempty names/titles; supported
  phase ID format (`sequential`/`uuid`), full status/priority values and
  nonnegative complexity. No mandatory Fibonacci sizing or routing.
- Dependency refs resolve locally or as `phase:id`; no missing/self/duplicate
  dependencies or effective dependency cycles. Parent links are same-phase,
  reciprocal with subtasks, acyclic, and supply inherited dependencies.
- Unqualified lookup must have exactly one match. Supplying a phase never
  silently picks a task from another phase.

`internal/taskapi` is the shared transactional boundary. All CLI/MCP writes
use it: lock a stable sidecar across processes, check expected revision inside
the lock, validate existing and candidate documents, serialize losslessly,
sync a temp file, rename, and sync the directory. Cancellation before
publication aborts the write. Post-publication directory-sync errors explicitly
say publication already occurred. Locking uses native Unix/Windows mechanisms;
directory metadata sync is Unix-only, with no equivalent Windows power-loss
durability promise. Bound decoded and serialized documents to 32 MiB; a merge
that would make storage unreadable must fail before publication.

Import policies are phase-scoped. Default refuses any colliding phase. Merge
requires matching phase metadata and appends only nonduplicate tasks. Replace
replaces input phases and preserves unrelated phases, then validates the entire
graph. Invalid replacements cannot strand references. No-op patches preserve
timestamps and revision; actual patches update `UpdatedAt`. Imports preserve
supplied timestamps. Never stamp time during read/serialization.

Omitted/null/empty dependency collections are equivalent for no-op detection;
clearing an already-empty dependency list must not create a timestamp/revision.
Natural ID ordering must be total and transitive for numeric, nonnumeric,
leading-zero and arbitrarily large numeric segments, not conditional pairwise
numeric/lexical comparison. Validate normalization across input permutations.
Filtered document export includes the transitive whole-phase prerequisite
closure, so it remains a standalone valid document; do not discard edges.

Patch fields: status, title, description, details, test_strategy, priority,
complexity, dependencies and assigned_to. Unspecified fields are unchanged;
empty strings/arrays explicitly clear supported fields. `--input` patch JSON
cannot be mixed with field flags. Both import and update accept `--if-revision`.

## 4. Status and scripted workflow semantics

Snapshot fields: `schema_version`, `revision`, `phases`, `summary`, `ready`,
`blocked`. Revision is a deterministic digest of the normalized document;
stable sorting uses byte tie-breaks for numerically equal IDs. Counts expose
every supported workflow status including zeroes.

Only pending leaf tasks whose effective dependencies are done are ready.
Resolve cross-phase and inherited dependencies first; never discard an unmet
dependency merely because a planner's input is phase-local. Pending leaf tasks
with unmet dependencies have explicit blocker reasons. Other task states are
visible in counts/the document, not secretly eligible. `next` is a read, not
a lease or claim; scripts needing concurrency use explicit guarded updates.

`status --watch --interval D` emits the initial snapshot and subsequent
observed revision changes. Minimum interval is 100ms. It is a polling snapshot
stream, not a complete mutation history; intermediate revisions may be missed.
Each read obtains fresh persisted state; cancellation ends the watch promptly.
No tasks is an empty successful result, not an inference/execution request.

## 5. Models and PRD proposals

### 5.1 Main and fast, managed through the CLI

Exactly two public defaults: main and fast. Main is the normal parse default;
fast is chosen explicitly. They may point to the same provider/model. There
is no role/profile/tier/default cascade or complexity-based selection.

```sh
scud models list
scud models list --search 'sonnet'
scud models set main 'sonnet'
scud models set fast 'haiku'
scud models defaults --json
```

List queries authenticated provider catalogs, not guessed static availability.
Human display names are used where supplied; matching covers provider/name/ID
case-insensitively. Setting requires a unique match; ambiguity shows choices
and writes nothing. Exact canonical IDs and optional `--provider` remain
available but must not be required just to know invocation spelling.
Catalog failures/partial discovery must be visible, not turn incomplete data
into a silent ambiguous-default choice. Paginated catalogs are followed or
explicitly reported incomplete. No catalog means an actionable error.

Provider and canonical ID are stored together by `models set`, preserving
unrelated config. Setting one default must not erase the other. Missing fresh
configuration means unset defaults, not a hidden vendor choice. Legacy pairs
can be migrated only when explicit and coherent; unknown/partial routes fail
actionably. Credentials come from documented environment variables; no foreign
credential scraping, provider-prefix guessing or fallback coding harness.

Explicit model IDs, including vendor-qualified OpenRouter IDs, reach inference
verbatim. Configuration editing must understand multiline TOML strings and
verify the candidate's requested target plus unrelated semantic values before
publication; syntax-only validation is insufficient. Unsupported layouts fail
without writing, never falsely report successful selection. Discovery propagates
cancellation/deadline identity immediately, even after earlier partial successes.

Supported environment credentials: OPENAI_API_KEY, XAI_API_KEY,
ANTHROPIC_API_KEY, OPENROUTER_API_KEY. Optional provider BASE_URL environment
overrides are shared by discovery and generation for compatible endpoints.
Authentication still requires setup; model discovery is not a credential broker.

### 5.2 Proposal acceptance

`parse` reads bounded nonempty PRD text, selects main or fast, and requests
goal/acceptance-sized tasks. Count is optional guidance, never a fixed default
of ten; no agent assignment from complexity. The response is a complete
proposal, not a task mutation. The importable output uses the document in §3.

Reject API errors, refusal/filtering, token-limit truncation, empty/unknown
termination, malformed/unknown/duplicate JSON data, missing fields, invalid
dependencies and cycles. Validate the proposal with the task service contract
before emitting it. Never salvage by regex or persist partial responses in
the new path. Provider-side native constrained-output adapters remain follow-up
work; strict JSON/graph checks are implemented, not schema-guaranteed decoding.
Capability differs by model/endpoint, even for “OpenAI-compatible” providers.

## 6. Optional sb

Detect `sb.toml` and resolve the sb executable. Status reads availability without
executing checks. Explicit operations map to public commands: check -> gates,
context -> context, derive -> derive, audit -> audit-report, gen -> gen.
Do not parse sb.toml, Shen source or private sb state to infer semantics.

Commands have bounded runtime and output, preserve stdout/stderr/exit status,
and return structured failure diagnostics when `--json` is selected. Linux/macOS
own the invocation's private process group until its launcher is reaped;
cancellation and early successful launcher exit clean up remaining descendants,
including children with inherited or closed output descriptors. Detached groups
are outside this ownership boundary. Windows/other platforms currently stop
only the direct child; no stronger subprocess-tree guarantee is advertised there.
No shell interpolation or agent loop is exposed. Generated project files may
change only through explicitly invoked upstream operations.

Report presence/digest/change can be observed, but report status remains
`unverified`. Passing gates may produce no report; changed bytes establish
neither current-run provenance nor claim coverage. No automatic task status
change, invariant-discharge claim, history certification or spec-first compiler
claim is made. A marker with a missing binary errors on explicit operations;
core task operations still work without sb.

## 7. Ensemble and MCP

Experimental ensemble takes explicit main/fast members (repetition allowed)
and a synthesis default. Direct model calls only; no filesystem tools, coding
workers or task mutation. Bound member count, concurrency and total runtime.
Member failure preserves diagnostics, skips synthesis and returns nonzero.
The output marks itself experimental. No ensemble MCP tool is registered.

Overall ensemble timeout includes input collection. PRD/import/patch input
reads honor cancellation while blocked, closing owned interruptible streams
or using safe nonblocking file reads. Do not abandon an uninterruptible reader
goroutine. Unknown nonclosable blocking readers are rejected. Windows console
input that cannot be interrupted safely is explicitly unsupported.

Default MCP tools: scud_tasks, scud_import, scud_update, scud_status, scud_parse.
The read tool supports document/single-task/ready selection instead of separate
redundant utility tools. `SCUD_TOOLS=full` adds scud_sb; custom explicit subsets
are accepted, unknown names rejected. Handlers use the same services and return
structured data/errors over stdio without stdout banners.

### Stateless protocol and transport contract

Use MCP **2026-07-28**, not the old sessionful protocol. The official Go SDK
**v1.8.0** implements this revision; SDK major version is not protocol version.
Only this protocol revision is supported. No initialize/initialized handshake,
HTTP session ID, legacy GET/SSE session stream or session-derived capabilities.

Every request independently requires `params._meta` keys
`io.modelcontextprotocol/protocolVersion` and
`io.modelcontextprotocol/clientCapabilities` (object, empty allowed). Validate
literal per-request metadata before dispatch; do not call SDK convenience
accessors that can fall back to earlier request/initialization state. Missing
metadata after a valid prior call must still fail. Support `server/discover`
and `resultType: "complete"`; reject legacy handshake methods.

Low-level SDK tool registration retains `json.RawMessage`. Strictly validate
the raw whole frame before SDK envelope decoding, then decode raw arguments
into concrete types. Do not round-trip ingestion through `map[string]any` or
float64. Duplicate members, casing aliases and out-of-range typed integers
must fail without mutation; accepted large integers retain their exact value.

Stdio EOF/read failure cancels in-flight work before shutdown completes.
Streamable HTTP is opt-in via `--transport http --listen LOOPBACK_IP:PORT`,
POST `/mcp`, with Stateless, JSONResponse and PropagateRequestCancellation.
Require matching protocol/method/name headers as defined by the protocol,
reject cross-origin browser requests, and keep localhost/DNS-rebinding guards.
No remote-auth service is implied by a loopback listener. Client disconnect
cancels only its own request; later requests work independently. Server signal
shutdown cancels handlers and waits for bounded cleanup before returning.

References: [protocol](https://modelcontextprotocol.io/specification/2026-07-28),
[official SDK](https://github.com/modelcontextprotocol/go-sdk/releases/tag/v1.8.0).

## 8. Compatibility and follow-up work plan

Toolchain: Go 1.27.3 or newer, pinned in `go.mod` and reused by CI. Stable
json/v2 decoding, typed generic JSON helpers, generic sorting, WaitGroup.Go,
typed error inspection and context-aware tests are used where they simplify
the core. No experimental flags or generic abstraction layers are required.

The original review baselines were scud `866d0f4` and sb
`6b9dde09b3a98ee5d20d0a6556acceacad4657d8`. Source changed since that review;
they are not a claim about the new runtime. Tests, not old README assertions,
establish the implemented behavior.

Legacy SCG is not silently parsed or shadowed. A nonempty legacy task file
blocks the new service with explicit migration guidance. Exact backups and a
strict raw-file conversion diagnostic pass are required before a future
migrator can advertise losslessness. Known v1 risks include lost title spaces,
blank/comment/section-like detail lines, terminal newlines and scanner overflow.
Ordinary escaping is not universally broken. No SCG v2 codec ships here.

| Work area | Acceptance / remaining scope |
|---|---|
| Task service + CLI | Implemented shared validation, guarded atomic writes, lossless JSON round trips, dependency readiness and JSONL watch; keep regression/race tests |
| Main/fast UX | Implemented catalog/search/set/defaults; fixture-test authentication, paging, ambiguity, config preservation/concurrency and friendly errors |
| PRD proposal | Implemented explicit-target inference and strict proposal validation; add native capability adapters separately rather than claim universal schema support |
| sb | Implemented optional wrapper/diagnostics with no proof claims; future discharge projection requires a versioned public status/identity/provenance contract |
| Experimental ensemble | Implemented direct inference; stabilization is optional and must not expand core API |
| MCP | Implemented reduced shared surface; maintain parity tests with CLI |
| Legacy migration | Follow-up: raw SCG corpus, explicit loss diagnosis, exact backup, conflict-safe publication and usable conversion command |
| Cleanup | Follow-up: remove unregistered legacy command/packages and unused dependencies after call-site/compatibility audit; do not revive their APIs |
| Schema/docs | Follow-up: publish machine JSON Schema and compatibility policy; current versioned Go types/strict decoder define acceptance |

There is no dependency on reintroducing execution, repair loops, role selection,
evidence-gated completion or the superseded W0–W5 scheduler plan. Restore any
such feature only through an explicit new scope decision.
