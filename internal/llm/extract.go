package llm

import (
	"regexp"
	"strings"
)

var (
	jsonFenceRe = regexp.MustCompile("(?s)```json\\s*\n(.*?)```")
	fenceRe     = regexp.MustCompile("(?s)```\\s*\n(.*?)```")
	jsonArrayRe = regexp.MustCompile(`(?s)\[.*\]`)
	jsonObjRe   = regexp.MustCompile(`(?s)\{.*\}`)
)

// ExtractJSON tries multiple strategies to find JSON in LLM output.
func ExtractJSON(output string) string {
	// Strategy 1: ```json fence
	if m := jsonFenceRe.FindStringSubmatch(output); len(m) > 1 {
		return strings.TrimSpace(m[1])
	}
	// Strategy 2: ``` fence
	if m := fenceRe.FindStringSubmatch(output); len(m) > 1 {
		return strings.TrimSpace(m[1])
	}
	// Strategy 3+4: try object or array based on which appears first
	objIdx := jsonObjRe.FindStringIndex(output)
	arrIdx := jsonArrayRe.FindStringIndex(output)
	if objIdx != nil && arrIdx != nil {
		if objIdx[0] <= arrIdx[0] {
			return jsonObjRe.FindString(output)
		}
		return jsonArrayRe.FindString(output)
	}
	if objIdx != nil {
		return jsonObjRe.FindString(output)
	}
	if arrIdx != nil {
		return jsonArrayRe.FindString(output)
	}
	// Strategy 5: raw output
	trimmed := strings.TrimSpace(output)
	if strings.HasPrefix(trimmed, "[") || strings.HasPrefix(trimmed, "{") {
		return trimmed
	}
	return ""
}
