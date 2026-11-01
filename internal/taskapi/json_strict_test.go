package taskapi

import (
	"strings"
	"testing"
)

func TestDecodeJSONStrictV2(t *testing.T) {
	type document struct {
		Title  string `json:"title"`
		Nested struct {
			Value string `json:"value"`
		} `json:"nested"`
	}
	for name, data := range map[string]string{
		"invalid utf8":      "{\"title\":\"\xff\"}",
		"duplicate":         `{"title":"a","title":"b"}`,
		"escaped duplicate": `{"title":"a","\u0074itle":"b"}`,
		"case alias":        `{"title":"a","Title":"b"}`,
		"unknown":           `{"unknown":1}`,
		"case mismatch":     `{"Title":"a"}`,
		"nested unknown":    `{"nested":{"VALUE":"a"}}`,
		"trailing":          `{} {}`,
	} {
		t.Run(name, func(t *testing.T) {
			var v document
			if err := DecodeJSON(strings.NewReader(data), &v); err == nil {
				t.Fatal("accepted invalid input")
			}
		})
	}
	large := strings.Repeat("x", 100000)
	var v document
	if err := DecodeJSON(strings.NewReader(`{"title":"`+large+`"}`), &v); err != nil || v.Title != large {
		t.Fatalf("large string: %v", err)
	}
	var generic map[string]any
	if err := DecodeJSON(strings.NewReader(`{"n":42,"nested":{"x":1.5}}`), &generic); err != nil {
		t.Fatal(err)
	}
	if _, ok := generic["n"].(float64); !ok {
		t.Fatalf("number type changed: %T", generic["n"])
	}
	if err := DecodeJSON(strings.NewReader(`{"nested":{"Σ":1,"ς":2}}`), &generic); err == nil {
		t.Fatal("Unicode case-folded aliases in a generic map accepted")
	}
	if err := DecodeJSON(strings.NewReader(`{"title":"`+strings.Repeat("x", maxJSONBytes)+`"}`), &v); err == nil {
		t.Fatal("oversized input accepted")
	}
}
