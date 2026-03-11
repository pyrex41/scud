package scg

import "strings"

// EscapeField escapes pipe, backslash, and newline characters for SCG pipe-delimited fields.
func EscapeField(s string) string {
	s = strings.ReplaceAll(s, `\`, `\\`)
	s = strings.ReplaceAll(s, "|", `\|`)
	s = strings.ReplaceAll(s, "\n", `\n`)
	return s
}

// UnescapeField reverses the escaping done by EscapeField.
func UnescapeField(s string) string {
	var b strings.Builder
	b.Grow(len(s))
	for i := 0; i < len(s); i++ {
		if s[i] == '\\' && i+1 < len(s) {
			switch s[i+1] {
			case '\\':
				b.WriteByte('\\')
				i++
			case '|':
				b.WriteByte('|')
				i++
			case 'n':
				b.WriteByte('\n')
				i++
			default:
				b.WriteByte('\\')
			}
		} else {
			b.WriteByte(s[i])
		}
	}
	return b.String()
}

// SplitByPipe splits a string by unescaped pipe characters, trimming each part.
func SplitByPipe(s string) []string {
	var parts []string
	var current strings.Builder
	for i := 0; i < len(s); i++ {
		if s[i] == '\\' && i+1 < len(s) {
			current.WriteByte(s[i])
			current.WriteByte(s[i+1])
			i++
		} else if s[i] == '|' {
			parts = append(parts, strings.TrimSpace(current.String()))
			current.Reset()
		} else {
			current.WriteByte(s[i])
		}
	}
	parts = append(parts, strings.TrimSpace(current.String()))
	return parts
}
