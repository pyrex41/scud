// Package graph provides the SCUD v2 planning view: deterministic DAG
// validation, wave construction, graph rewrites, and pure reconciliation.
//
// The v2/core package owns canonical lifecycle state and event reduction.
// This package owns scheduling decisions and temporary reconciliation states
// (deferred and waiting). bridge.go intentionally rejects those temporary
// states when converting back to core, so adapters must make that transition
// explicit instead of silently changing its meaning.
package graph
