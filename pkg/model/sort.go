package model

import (
	"cmp"
	"sort"
	"strconv"
	"strings"
)

// NaturalSortTasks sorts tasks by their IDs using natural sort order.
func NaturalSortTasks(tasks []*Task) {
	sort.Slice(tasks, func(i, j int) bool {
		return CompareIDs(tasks[i].ID, tasks[j].ID) < 0
	})
}

// NaturalSort sorts string IDs in natural order.
func NaturalSort(ids []string) {
	sort.Slice(ids, func(i, j int) bool {
		return CompareIDs(ids[i], ids[j]) < 0
	})
}

// CompareIDs defines a total natural order on IDs split into dot-separated
// hierarchical segments. Nonempty ASCII-digit segments precede all other
// segments and compare as unsigned integers of arbitrary length; other segments
// compare bytewise. Equal segments defer to later segments, then fewer segments
// sort first. Only when the entire hierarchy is equal numerically does the
// original byte spelling break ties (including leading zeros). Thus
// "1" < "1.1" < "1.2" < "1.10" < "2" < "10".
func CompareIDs(a, b string) int {
	partsA := strings.Split(a, ".")
	partsB := strings.Split(b, ".")

	for i := 0; i < len(partsA) && i < len(partsB); i++ {
		x, y := partsA[i], partsB[i]
		numeric := func(s string) bool {
			if s == "" {
				return false
			}
			for j := range len(s) {
				if s[j] < '0' || s[j] > '9' {
					return false
				}
			}
			return true
		}
		nx, ny := numeric(x), numeric(y)
		if nx != ny {
			if nx {
				return -1
			}
			return 1
		}
		if nx {
			x, y = strings.TrimLeft(x, "0"), strings.TrimLeft(y, "0")
			if n := cmp.Compare(len(x), len(y)); n != 0 {
				return n
			}
		}
		if n := strings.Compare(x, y); n != 0 {
			return n
		}
	}

	// Shorter ID comes first
	if len(partsA) != len(partsB) {
		if len(partsA) < len(partsB) {
			return -1
		}
		return 1
	}
	return strings.Compare(a, b)
}

func parseLeadingInt(s string) int {
	// Extract the leading numeric part before any dot
	parts := strings.SplitN(s, ".", 2)
	n, err := strconv.Atoi(parts[0])
	if err != nil {
		return 0
	}
	return n
}
