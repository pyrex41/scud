package model

import (
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

// CompareIDs compares two task IDs using natural sort.
// Split on ".", compare segments numerically where possible.
// "1" < "1.1" < "1.2" < "1.10" < "2" < "10"
func CompareIDs(a, b string) int {
	partsA := strings.Split(a, ".")
	partsB := strings.Split(b, ".")

	for i := 0; i < len(partsA) && i < len(partsB); i++ {
		na, errA := strconv.Atoi(partsA[i])
		nb, errB := strconv.Atoi(partsB[i])

		if errA == nil && errB == nil {
			if na != nb {
				if na < nb {
					return -1
				}
				return 1
			}
		} else {
			// Lexicographic comparison
			if partsA[i] != partsB[i] {
				if partsA[i] < partsB[i] {
					return -1
				}
				return 1
			}
		}
	}

	// Shorter ID comes first
	if len(partsA) != len(partsB) {
		if len(partsA) < len(partsB) {
			return -1
		}
		return 1
	}
	return 0
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
