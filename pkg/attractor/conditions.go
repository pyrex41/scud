package attractor

import "strings"

// EvalCondition evaluates a simple "key=value" condition expression against
// the pipeline context. An empty expression always evaluates to true.
func EvalCondition(expr string, ctx *PipelineContext) bool {
	expr = strings.TrimSpace(expr)
	if expr == "" {
		return true
	}
	parts := strings.SplitN(expr, "=", 2)
	if len(parts) != 2 {
		return false
	}
	key := strings.TrimSpace(parts[0])
	want := strings.TrimSpace(parts[1])
	return ctx.Get(key) == want
}
