# Scud Redesign — Small Task API

Status: revised for the user-directed product scope. Normative contract:
[V2_SPEC.md](V2_SPEC.md). Current commands/examples: [README](../README.md).

This describes SCUD **3.x**. Exact command/flag help is generated from the
registered CLI in [CLI_REFERENCE.md](CLI_REFERENCE.md); use that reference
instead of archived research or an older installed binary's help.

## 1. Product decision

**Scud turns PRDs into task lists, manages those lists safely, and gives
scripted workflows live dependency/status information.** It is not the agent
runtime, coding harness, commit manager or orchestration framework.

The user clarified the scope after the initial adversarial review:

- Task ingestion, processing, reading and updates are the center.
- sb remains useful and optional; core workflows work without it.
- Coding-agent execution can be dropped from the public API.
- Ensemble inference remains useful, but is explicitly experimental.
- Models are **main and fast**, discoverable/settable from the CLI without
  exact provider invocation spelling or manual config editing.

These decisions supersede the earlier execution-heavy spec. Do not implement
the old role/profile/tier hierarchy, Runner adapters, repair scheduler or
evidence-gated workflow completion just because older source packages exist.

## 2. Why the smaller boundary

The old surface combined task state, model routing, worker execution, repair,
ensembles and developer utilities. That encouraged duplicated APIs and made
task status difficult to consume independently of a particular harness.

The new surface separates three operations:

1. **Propose:** a model returns an importable task document; no writes occur.
2. **Manage:** a shared task service validates and atomically updates state.
3. **Observe:** scripts read current revisions, status counts, ready tasks and
   blocking reasons, or watch snapshots as JSONL.

sb checks are an explicit side operation, not a precondition for reading or
updating tasks. A user's `done` status is workflow information, not evidence
that an invariant was proved. This keeps external runners and simple scripts
equally useful clients instead of forcing scud to own their execution.

## 3. Model selection without routing machinery

There are two defaults: `main` and `fast`. A call chooses one explicitly;
parsing defaults to main. Both may point to the same model. No complexity
threshold, agent role, profile stack or provider-prefix inference chooses
between them.

`models list` queries authenticated catalogs. `models set main|fast QUERY`
matches display names/IDs case-insensitively, reports ambiguity and persists
the full provider/model pair itself. `models defaults` shows what is set.
Provider authentication remains explicit; no other app's credential files are
scraped. Discovery is not a promise that every listed model supports all APIs.

The first implementation uses strict JSON generation and local graph
validation. Native schema-constrained generation remains useful follow-up,
not a false universal capability claim. Refusal/truncation/malformed output
cannot become a partial import.

## 4. State representation and safety

The review found v1 SCG parsing could discard whitespace, detail lines and
large records. A complete new text grammar is no longer a prerequisite for
the small operational API. Use a versioned, lossless JSON document now, with
the same shape for proposals and import/export.

This intentionally supersedes the draft-v2 counted-text SCG design; no SCG v2
codec is being claimed. Strict legacy migration remains separate work. A
nonempty legacy file is refused rather than silently shadowed or passed
through a tolerant reader under a “lossless” label.

One service provides strict decoding, reference/cycle validation, qualified
lookup, parent-inherited readiness, cross-process locking, atomic publication
and optimistic revision checks. CLI and MCP share it. Status polling observes
fresh persisted snapshots; it is not a replayable event log.

## 5. What remains from the review

The original review against scud `866d0f4` and sb
`6b9dde09b3a98ee5d20d0a6556acceacad4657d8` established important constraints:

- Successful `sb gates` may produce no discharge report; report existence or
  changed bytes do not establish freshness/coverage.
- Full gates include implementation checks, not a spec-only compiler.
- JSON/native output needs local semantic validation; models/endpoints differ.
- External dependencies must be resolved before task readiness is reported.
- Duplicate routing/default layers and model-prefix inference should not return.
- Attractor was registered, not orphaned. Its removal from the public API is
  now a deliberate product-scope decision, not a claim of no users.
- Ensemble modes are preserved as experimental direct inference, without
  retaining a coding harness or three public orchestration commands.

Those constraints still guide the implementation. The removed execution
features mean there is no longer a reason to build an evidence-gated task
completion state machine or optional coding-harness adapters in this scope.

## 6. Work scope and remaining work

Implemented components have acceptance tests, not a calendar commitment:

| Area | Work scope |
|---|---|
| Core task API | Safe import/read/patch, full-field JSON round trips, revisions and readiness |
| Script interface | Small root CLI, clean JSON output, JSONL status watch, useful exit codes |
| PRD proposal | Main/fast selected inference, strict decoding and shared validation, no persistence |
| Model UX | Catalog listing/search, ambiguity-safe CLI defaults, no manual config editing |
| sb | Optional public command wrappers, bounded diagnostics, honest report uncertainty |
| Ensemble | Explicit experimental direct-model fan-out/synthesis; no task execution |
| MCP | Five core tools and one optional sb tool, no execution/commit/ensemble |

Remaining: safe raw-SCG migration, native structured-output adapters, schema
documentation, and staged removal of unregistered legacy code/dependencies.
Legacy utilities are not silently kept as part of the public contract. Restore
a capability only through a specific user need and a small tested interface.
