package config

import (
	"fmt"
	"reflect"
	"strings"

	"github.com/BurntSushi/toml"
)

// tomlStatementLines excludes continuation lines of strings, arrays and inline
// tables. Only statement starts may be interpreted as assignments or headers.
// The input has already been validated by the TOML parser.
func tomlStatementLines(text string) []bool {
	lines := []bool{true}
	var quote byte
	multi, comment := false, false
	depth := 0
	for i := 0; i < len(text); i++ {
		c := text[i]
		if c == '\n' {
			comment = false
			lines = append(lines, quote == 0 && depth == 0)
			continue
		}
		if comment {
			continue
		}
		if quote != 0 {
			if quote == '"' && c == '\\' {
				// Skip escaped quotes (including escaped triple delimiters), but
				// still visit newlines to retain the line index.
				if i+1 < len(text) && text[i+1] != '\n' {
					i++
				}
				continue
			}
			if c == quote {
				if !multi {
					quote = 0
					continue
				}
				if strings.HasPrefix(text[i:], strings.Repeat(string(quote), 3)) {
					// Four/five quotes include one/two literal quotes followed
					// by the closing delimiter.
					for i+1 < len(text) && text[i+1] == quote {
						i++
					}
					quote, multi = 0, false
				}
			}
			continue
		}
		switch c {
		case '#':
			comment = true
		case '"', '\'':
			quote = c
			multi = strings.HasPrefix(text[i:], strings.Repeat(string(c), 3))
			if multi {
				i += 2
			}
		case '[', '{':
			depth++
		case ']', '}':
			depth--
		}
	}
	return lines
}

// Validate the semantic delta, including unknown fields, before publication.
// Layouts that cannot be edited faithfully (e.g. inline target tables) fail
// closed rather than silently rewriting or dropping unrelated configuration.
func verifyModelUpdate(before, after, alias string, target ModelTarget) error {
	var oldDoc, newDoc map[string]any
	if _, err := toml.Decode(before, &oldDoc); err != nil {
		return err
	}
	if _, err := toml.Decode(after, &newDoc); err != nil {
		return err
	}
	if oldDoc == nil {
		oldDoc = map[string]any{}
	}
	oldDoc[alias+"_model"] = alias
	tables, ok := oldDoc["model_targets"].(map[string]any)
	if oldDoc["model_targets"] != nil && !ok {
		return fmt.Errorf("unsupported model_targets layout; config unchanged")
	}
	if tables == nil {
		tables = map[string]any{}
		oldDoc["model_targets"] = tables
	}
	entry, ok := tables[alias].(map[string]any)
	if tables[alias] != nil && !ok {
		return fmt.Errorf("unsupported model target layout; config unchanged")
	}
	if entry == nil {
		entry = map[string]any{}
		tables[alias] = entry
	}
	entry["provider"], entry["model"] = target.Provider, target.Model
	if !reflect.DeepEqual(oldDoc, newDoc) {
		return fmt.Errorf("unsupported config layout: update would change unrelated values; config unchanged")
	}
	var cfg Config
	if _, err := toml.Decode(after, &cfg); err != nil {
		return err
	}
	resolved, err := cfg.ResolveModelTarget(alias)
	if err != nil || resolved.Provider != target.Provider || resolved.Model != target.Model {
		return fmt.Errorf("config update does not resolve requested %s target; config unchanged", alias)
	}
	return nil
}
