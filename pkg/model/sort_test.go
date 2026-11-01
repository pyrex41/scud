package model

import (
	"math/rand"
	"slices"
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

func TestCompareIDsTotalOrder(t *testing.T) {
	ids := []string{"", "0", "00", "01", "1", "1.0", "01.00", "1.2", "01.10", "2", "10", "1a", "abc", "a.2", "a.10", ".", "+2", "-2", "99999999999999999999", "100000000000000000000", "00099999999999999999999", "550e8400-e29b-41d4-a716-446655440000", "1..2"}
	for _, a := range ids {
		for _, b := range ids {
			ab := CompareIDs(a, b)
			if ab != -CompareIDs(b, a) || (ab == 0) != (a == b) {
				t.Fatalf("not antisymmetric/total: %q %q: %d", a, b, ab)
			}
			for _, c := range ids {
				if ab < 0 && CompareIDs(b, c) < 0 && CompareIDs(a, c) >= 0 {
					t.Fatalf("not transitive: %q < %q < %q", a, b, c)
				}
			}
		}
	}
	want := slices.Clone(ids)
	NaturalSort(want)
	rng := rand.New(rand.NewSource(42))
	for range 100 {
		got := slices.Clone(ids)
		rng.Shuffle(len(got), func(i, j int) { got[i], got[j] = got[j], got[i] })
		NaturalSort(got)
		if !slices.Equal(got, want) {
			t.Fatalf("permutation-dependent order: %v != %v", got, want)
		}
	}
	for _, pair := range [][2]string{{"2", "10"}, {"10", "1a"}, {"1.2", "01.10"}, {"1", "01.0"}, {"99999999999999999999", "100000000000000000000"}, {"01", "1"}} {
		if CompareIDs(pair[0], pair[1]) >= 0 {
			t.Errorf("expected %q < %q", pair[0], pair[1])
		}
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
