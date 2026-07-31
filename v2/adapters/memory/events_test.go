package memory

import (
	"context"
	"testing"

	"github.com/reuben/scud/v2/adapters"
)

func TestEventStoreEnforcesMonotonicSequenceAndCopiesData(t *testing.T) {
	store := NewEventStore()
	ctx := context.Background()
	data := []byte("one")
	if err := store.Append(ctx, adapters.Event{RunID: "r", Sequence: 1, Data: data}); err != nil {
		t.Fatal(err)
	}
	data[0] = 'X'
	if err := store.Append(ctx, adapters.Event{RunID: "r", Sequence: 1}); err == nil {
		t.Fatal("expected duplicate sequence error")
	}
	got, err := store.List(ctx, "r", 0)
	if err != nil || len(got) != 1 || string(got[0].Data) != "one" {
		t.Fatalf("unexpected list: %+v, %v", got, err)
	}
}
