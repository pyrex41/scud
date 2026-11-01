package taskapi

import (
	"testing"

	"github.com/reuben/scud/pkg/model"
)

func TestEmptyCountsHaveStableKeys(t *testing.T) {
	snapshot, err := ValidateDocument(Document{SchemaVersion: 1})
	if err != nil {
		t.Fatal(err)
	}
	if len(snapshot.Summary.Status) != 9 || snapshot.Summary.Total != 0 || len(snapshot.Ready) != 0 {
		t.Fatalf("unexpected empty status: %+v", snapshot)
	}
	for _, status := range []model.TaskStatus{model.Pending, model.InProgress, model.Done, model.Failed, model.Blocked, model.Review, model.Expanded, model.Deferred, model.Cancelled} {
		count, exists := snapshot.Summary.Status[status]
		if !exists || count != 0 {
			t.Fatalf("missing stable zero for %s", status)
		}
	}
}
