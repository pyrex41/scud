package config

import "testing"

func TestResolveModelTarget(t *testing.T) {
	c := Default()
	c.Models = map[string]ModelTarget{"primary": {Provider: "openai", Model: "gpt-x", MaxOutputTokens: 900}}
	c.DefaultModel = "primary"
	got, err := c.ResolveModelTarget("")
	if err != nil || got.Provider != "openai" || got.Model != "gpt-x" {
		t.Fatalf("got %#v, %v", got, err)
	}
	if _, err = c.ResolveModelTarget("missing"); err == nil {
		t.Fatal("unknown model target accepted")
	}
	c.Models["broken"] = ModelTarget{Model: "x"}
	if _, err = c.ResolveModelTarget("broken"); err == nil {
		t.Fatal("incomplete target accepted")
	}
}
