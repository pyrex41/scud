package graph

import (
	"reflect"
	"testing"
)

func TestWavesAreDeterministic(t *testing.T) {
	g, err := Build([]Node{
		{ID: "z", Dependencies: []ID{"b", "a"}},
		{ID: "b"}, {ID: "a"}, {ID: "m", Dependencies: []ID{"z"}},
	})
	if err != nil {
		t.Fatal(err)
	}
	want := [][]ID{{"a", "b"}, {"z"}, {"m"}}
	for i := 0; i < 20; i++ {
		got, err := g.Waves(nil)
		if err != nil {
			t.Fatal(err)
		}
		if !reflect.DeepEqual(got, want) {
			t.Fatalf("iteration %d: waves = %#v, want %#v", i, got, want)
		}
	}
	if got := g.Ready(nil); !reflect.DeepEqual(got, []ID{"a", "b"}) {
		t.Fatalf("ready = %v", got)
	}
}

func TestValidationReportsMissingAndCycles(t *testing.T) {
	g := New(
		Node{ID: "a", Dependencies: []ID{"missing", "missing"}},
		Node{ID: "b", Dependencies: []ID{"c"}},
		Node{ID: "c", Dependencies: []ID{"b"}},
	)
	r := g.Check()
	if r.Valid() || len(r.MissingDependencies) != 1 || len(r.DuplicateDependencies) != 1 || len(r.Cycles) != 1 {
		t.Fatalf("unexpected report: %#v", r)
	}
	if _, ok := g.Waves(nil); ok == nil {
		t.Fatal("expected validation error")
	}
}

func TestRewriteIsAtomicAndGuardsCycles(t *testing.T) {
	g, err := Build([]Node{{ID: "a"}, {ID: "b", Dependencies: []ID{"a"}}})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := g.AddDependency("a", "b"); err == nil {
		t.Fatal("expected cycle error")
	}
	if got := g.Ready(nil); !reflect.DeepEqual(got, []ID{"a"}) {
		t.Fatalf("source graph mutated after rejected rewrite: %v", got)
	}
	next, err := g.Rewrite(Rewrite{
		AddNodes:           []Node{{ID: "c"}},
		AddDependencies:    map[ID][]ID{"c": {"b"}},
		RemoveDependencies: map[ID][]ID{"b": {"a"}},
	})
	if err != nil {
		t.Fatal(err)
	}
	if got := next.Ready(nil); !reflect.DeepEqual(got, []ID{"a", "b"}) {
		t.Fatalf("rewritten ready = %v", got)
	}
}

func TestRewriteUpdatesNodeMetadata(t *testing.T) {
	g, err := Build([]Node{{ID: "a", Description: "old"}})
	if err != nil {
		t.Fatal(err)
	}
	n, _ := g.Node("a")
	n.Description, n.Status = "new", Running
	next, err := g.UpdateNode(n)
	if err != nil {
		t.Fatal(err)
	}
	got, _ := next.Node("a")
	if got.Description != "new" || got.Status != Running {
		t.Fatalf("updated node = %+v", got)
	}
}

func TestCompletedSetAndTerminalStatuses(t *testing.T) {
	g, err := Build([]Node{{ID: "a", Status: Succeeded}, {ID: "b", Dependencies: []ID{"a"}}, {ID: "c", Status: Blocked}})
	if err != nil {
		t.Fatal(err)
	}
	waves, err := g.Waves(nil)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(waves, [][]ID{{"b"}}) {
		t.Fatalf("waves = %v", waves)
	}
	if got := g.Ready(map[ID]bool{"a": true}); !reflect.DeepEqual(got, []ID{"b"}) {
		t.Fatalf("ready = %v", got)
	}
}
