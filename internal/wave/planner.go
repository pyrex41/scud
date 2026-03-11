package wave

import "github.com/reuben/scud/internal/model"

// Wave represents a group of tasks that can execute in parallel.
type Wave struct {
	Number int
	Tasks  []string
}

// WaveResult contains the computed waves and any circular dependencies.
type WaveResult struct {
	Waves        []Wave
	CircularDeps []string
}

// Plan computes execution waves using Kahn's algorithm.
// Only dependencies present in the provided task set are considered.
func Plan(tasks []*model.Task) WaveResult {
	if len(tasks) == 0 {
		return WaveResult{}
	}

	// Build task set for quick lookup
	taskSet := make(map[string]bool, len(tasks))
	for _, t := range tasks {
		taskSet[t.ID] = true
	}

	// Build in-degree map (only counting deps within task set)
	inDegree := make(map[string]int, len(tasks))
	// dependents[dep] = list of tasks that depend on dep
	dependents := make(map[string][]string)

	for _, t := range tasks {
		if _, ok := inDegree[t.ID]; !ok {
			inDegree[t.ID] = 0
		}
		for _, dep := range t.Dependencies {
			if taskSet[dep] {
				inDegree[t.ID]++
				dependents[dep] = append(dependents[dep], t.ID)
			}
		}
	}

	var result WaveResult
	waveNum := 1
	remaining := len(tasks)

	for remaining > 0 {
		// Find all tasks with in-degree 0
		var ready []string
		for _, t := range tasks {
			if deg, ok := inDegree[t.ID]; ok && deg == 0 {
				ready = append(ready, t.ID)
			}
		}

		if len(ready) == 0 {
			// Circular dependency - collect remaining
			for id, deg := range inDegree {
				if deg > 0 {
					result.CircularDeps = append(result.CircularDeps, id)
				}
			}
			model.NaturalSort(result.CircularDeps)
			break
		}

		model.NaturalSort(ready)
		result.Waves = append(result.Waves, Wave{
			Number: waveNum,
			Tasks:  ready,
		})
		waveNum++

		// Remove ready tasks and decrement dependents
		for _, id := range ready {
			delete(inDegree, id)
			remaining--
			for _, dep := range dependents[id] {
				if _, ok := inDegree[dep]; ok {
					inDegree[dep]--
				}
			}
		}
	}

	return result
}
