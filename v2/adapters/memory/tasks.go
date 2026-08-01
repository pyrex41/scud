package memory

import (
	"context"
	"errors"
	"sort"
	"sync"

	"github.com/reuben/scud/v2/adapters"
)

// TaskDirectory is a tiny test implementation of the td seam. Production
// adapters can map these calls to SCG, SQLite, or a remote task service.
type TaskDirectory struct {
	mu    sync.RWMutex
	tasks map[string]adapters.Task
}

func NewTaskDirectory(tasks []adapters.Task) *TaskDirectory {
	byID := make(map[string]adapters.Task, len(tasks))
	for _, task := range tasks {
		task.Dependencies = append([]string(nil), task.Dependencies...)
		byID[task.ID] = task
	}
	return &TaskDirectory{tasks: byID}
}

func (d *TaskDirectory) Get(_ context.Context, id string) (adapters.Task, error) {
	d.mu.RLock()
	defer d.mu.RUnlock()
	task, ok := d.tasks[id]
	if !ok {
		return adapters.Task{}, errors.New("task not found")
	}
	task.Dependencies = append([]string(nil), task.Dependencies...)
	return task, nil
}

func (d *TaskDirectory) SetStatus(_ context.Context, id, status string) error {
	d.mu.Lock()
	defer d.mu.Unlock()
	task, ok := d.tasks[id]
	if !ok {
		return errors.New("task not found")
	}
	task.Status = status
	d.tasks[id] = task
	return nil
}

func (d *TaskDirectory) Ready(_ context.Context, _ string) ([]adapters.Task, error) {
	d.mu.RLock()
	defer d.mu.RUnlock()
	var ready []adapters.Task
	for _, task := range d.tasks {
		if task.Status != "pending" {
			continue
		}
		ok := true
		for _, dependency := range task.Dependencies {
			dep, exists := d.tasks[dependency]
			if !exists || dep.Status != "done" {
				ok = false
				break
			}
		}
		if ok {
			task.Dependencies = append([]string(nil), task.Dependencies...)
			ready = append(ready, task)
		}
	}
	sort.Slice(ready, func(i, j int) bool { return ready[i].ID < ready[j].ID })
	return ready, nil
}

var _ adapters.TaskDirectory = (*TaskDirectory)(nil)
