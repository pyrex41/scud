// Package core contains the provider-independent domain kernel for SCUD v2.
//
// The package deliberately has no I/O, clocks, persistence, provider SDKs, or
// application dependencies. A run is represented by State and advanced by
// applying Events with Reduce. Replay applies the same events from an empty
// state and therefore produces the same result. Reconcile computes a
// deterministic next Decision from a state; it does not perform that decision.
//
// Values returned by constructors and reducers own their slices and maps. The
// public structs are intentionally small and can be serialized by an adapter,
// but callers should treat values as immutable and use the returned value from
// each operation rather than changing a State in place.
package core
