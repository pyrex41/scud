package taskapi

import (
	"bytes"
	"encoding/json/jsontext"
	jsonv2 "encoding/json/v2"
	"fmt"
	"io"
	"strings"
	"unicode"
)

// MaxJSONBytes bounds both decoded inputs and persisted output, so successful
// writes remain readable by the same service.
const MaxJSONBytes = 32 << 20
const maxJSONBytes = MaxJSONBytes

// Decode returns a typed value only after the entire input passes validation.
// DecodeJSON is retained for provider/transport interfaces using destinations.
func Decode[T any](r io.Reader) (T, error) {
	var value T
	if err := DecodeJSON(r, &value); err != nil {
		var zero T
		return zero, err
	}
	return value, nil
}

// DecodeJSON accepts one JSON value of at most 32 MiB. Unknown struct fields,
// invalid UTF-8, duplicate names, and case-folded object-name aliases are errors.
func DecodeJSON(r io.Reader, dst any) error {
	b, err := io.ReadAll(io.LimitReader(r, maxJSONBytes+1))
	if err != nil {
		return err
	}
	if len(b) > maxJSONBytes {
		return fmt.Errorf("JSON input exceeds %d bytes", maxJSONBytes)
	}

	// V2 already rejects duplicate names, but unlike the old decoder it allows
	// case-folded names in maps. Retain the stricter existing contract even for
	// generic destinations, without coercing any numbers during this scan.
	dec := jsontext.NewDecoder(bytes.NewReader(b))
	var objects []map[string]struct{}
	for {
		depth := dec.StackDepth()
		var isName bool
		if depth > 0 {
			kind, n := dec.StackIndex(depth)
			isName = kind == jsontext.KindBeginObject && n%2 == 0
		}
		tok, e := dec.ReadToken()
		if e == io.EOF {
			break
		}
		if e != nil {
			return e
		}
		if isName && tok.Kind() == jsontext.KindString {
			name := tok.String()
			key := foldJSONName(name)
			if _, ok := objects[len(objects)-1][key]; ok {
				return fmt.Errorf("duplicate JSON key %q", name)
			}
			objects[len(objects)-1][key] = struct{}{}
		}
		switch tok.Kind() {
		case jsontext.KindBeginObject:
			objects = append(objects, map[string]struct{}{})
		case jsontext.KindBeginArray:
			objects = append(objects, nil)
		case jsontext.KindEndObject, jsontext.KindEndArray:
			objects = objects[:len(objects)-1]
		}
	}
	return jsonv2.Unmarshal(b, dst, jsonv2.RejectUnknownMembers(true))
}

// foldJSONName uses the same Unicode simple-fold equivalence as EqualFold.
func foldJSONName(s string) string {
	var b strings.Builder
	for _, r := range s {
		min := r
		for f := unicode.SimpleFold(r); f != r; f = unicode.SimpleFold(f) {
			if f < min {
				min = f
			}
		}
		b.WriteRune(min)
	}
	return b.String()
}
