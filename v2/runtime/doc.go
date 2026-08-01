// Package runtime coordinates the provider-independent SCUD v2 kernel with
// adapter seams. It owns no provider SDK, persistence implementation, prompt
// format, or command-line behavior.
//
// Runtime persists core events as opaque JSON payloads in an adapters.EventStore
// and replays only those canonical events. Runner progress is copied to the
// same store as non-canonical envelopes and never affects core state. Step is
// deterministic up to adapter responses: it selects the next ready obligation,
// authorizes it, reserves budget, runs it, and records an observation.
package runtime
