package model

import (
	"strings"
	"testing"
)

func TestNaturalSort(t *testing.T) {
	ids := []string{"10", "1.10", "2", "1.2", "1.1", "1"}
	NaturalSort(ids)
	want := "1,1.1,1.2,1.10,2,10"
	got := strings.Join(ids, ",")
	if got != want {
		t.Errorf("NaturalSort = %s, want %s", got, want)
	}
}

func TestCompareIDs(t *testing.T) {
	tests := []struct {
		a, b string
		want int
	}{
		{"1", "2", -1},
		{"2", "1", 1},
		{"1", "1", 0},
		{"1", "1.1", -1},
		{"1.1", "1.2", -1},
		{"1.2", "1.10", -1},
		{"10", "2", 1},
		{"abc", "def", -1},
		{"1", "abc", -1}, // numeric < alpha
	}

	for _, tt := range tests {
		got := CompareIDs(tt.a, tt.b)
		if (tt.want < 0 && got >= 0) || (tt.want > 0 && got <= 0) || (tt.want == 0 && got != 0) {
			t.Errorf("CompareIDs(%q, %q) = %d, want sign %d", tt.a, tt.b, got, tt.want)
		}
	}
}
