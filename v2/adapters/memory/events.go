// Package memory provides deterministic test adapters for v2 contracts. It is
// not a production persistence layer.
package memory

import (
	"context"
	"errors"
	"sort"
	"sync"

	"github.com/reuben/scud/v2/adapters"
)

type EventStore struct {
	mu     sync.RWMutex
	events map[string][]adapters.Event
}

func NewEventStore() *EventStore { return &EventStore{events: make(map[string][]adapters.Event)} }

func (s *EventStore) Append(_ context.Context, event adapters.Event) error {
	if event.RunID == "" || event.Sequence == 0 {
		return errors.New("event run ID and sequence are required")
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	list := s.events[event.RunID]
	if len(list) > 0 && event.Sequence <= list[len(list)-1].Sequence {
		return errors.New("event sequence must increase")
	}
	event.Data = append([]byte(nil), event.Data...)
	s.events[event.RunID] = append(list, event)
	return nil
}

func (s *EventStore) List(_ context.Context, runID string, after uint64) ([]adapters.Event, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	var out []adapters.Event
	for _, event := range s.events[runID] {
		if event.Sequence > after {
			copy := event
			copy.Data = append([]byte(nil), event.Data...)
			out = append(out, copy)
		}
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Sequence < out[j].Sequence })
	return out, nil
}

var _ adapters.EventStore = (*EventStore)(nil)
