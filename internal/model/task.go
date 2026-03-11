package model

import (
	"fmt"
	"time"
)

type TaskStatus string

const (
	Pending    TaskStatus = "pending"
	InProgress TaskStatus = "in-progress"
	Done       TaskStatus = "done"
	Failed     TaskStatus = "failed"
	Blocked    TaskStatus = "blocked"
	Review     TaskStatus = "review"
	Expanded   TaskStatus = "expanded"
	Deferred   TaskStatus = "deferred"
	Cancelled  TaskStatus = "cancelled"
)

var statusToCode = map[TaskStatus]string{
	Pending: "P", InProgress: "I", Done: "D", Failed: "!",
	Blocked: "B", Review: "R", Expanded: "X", Deferred: "F", Cancelled: "C",
}

var codeToStatus = map[string]TaskStatus{
	"P": Pending, "I": InProgress, "D": Done, "!": Failed,
	"B": Blocked, "R": Review, "X": Expanded, "F": Deferred, "C": Cancelled,
}

func StatusCode(s TaskStatus) string {
	if c, ok := statusToCode[s]; ok {
		return c
	}
	return "P"
}

func StatusFromCode(c string) TaskStatus {
	if s, ok := codeToStatus[c]; ok {
		return s
	}
	return Pending
}

func ParseStatus(s string) (TaskStatus, bool) {
	switch TaskStatus(s) {
	case Pending, InProgress, Done, Failed, Blocked, Review, Expanded, Deferred, Cancelled:
		return TaskStatus(s), true
	}
	if st, ok := codeToStatus[s]; ok {
		return st, true
	}
	return Pending, false
}

type Priority string

const (
	Critical Priority = "critical"
	High     Priority = "high"
	Medium   Priority = "medium"
	Low      Priority = "low"
)

var priorityToCode = map[Priority]string{
	Critical: "C", High: "H", Medium: "M", Low: "L",
}

var codeToPriority = map[string]Priority{
	"C": Critical, "H": High, "M": Medium, "L": Low,
}

func PriorityCode(p Priority) string {
	if c, ok := priorityToCode[p]; ok {
		return c
	}
	return "M"
}

func PriorityFromCode(c string) Priority {
	if p, ok := codeToPriority[c]; ok {
		return p
	}
	return Medium
}

type AgentType string

const (
	AgentBuilder    AgentType = "builder"
	AgentFastBuilder AgentType = "fast-builder"
	AgentReviewer   AgentType = "reviewer"
	AgentTester     AgentType = "tester"
	AgentPlanner    AgentType = "planner"
	AgentResearcher AgentType = "researcher"
	AgentAnalyzer   AgentType = "analyzer"
)

type ModelTier string

const (
	TierFast     ModelTier = "fast"
	TierStandard ModelTier = "standard"
	TierSmart    ModelTier = "smart"
	TierCustom   ModelTier = "custom"
)

type Task struct {
	ID            string     `json:"id"`
	Title         string     `json:"title"`
	Description   string     `json:"description"`
	Details       string     `json:"details,omitempty"`
	TestStrategy  string     `json:"test_strategy,omitempty"`
	Status        TaskStatus `json:"status"`
	Complexity    int        `json:"complexity"`
	Priority      Priority   `json:"priority"`
	Dependencies  []string   `json:"dependencies"`
	ParentID      string     `json:"parent_id,omitempty"`
	Subtasks      []string   `json:"subtasks,omitempty"`
	AgentType     AgentType  `json:"agent_type,omitempty"`
	ModelTier     ModelTier  `json:"model_tier,omitempty"`
	ModelOverride string     `json:"model_override,omitempty"`
	AssignedTo    string     `json:"assigned_to,omitempty"`
	CreatedAt     string     `json:"created_at,omitempty"`
	UpdatedAt     string     `json:"updated_at,omitempty"`
}

// IsReady returns true if all dependencies are in the done set.
func (t *Task) IsReady(doneSet map[string]bool) bool {
	if t.Status != Pending {
		return false
	}
	for _, dep := range t.Dependencies {
		if !doneSet[dep] {
			return false
		}
	}
	return true
}

// GetEffectiveDeps walks the ParentID chain and collects all dependencies.
func (t *Task) GetEffectiveDeps(allTasks map[string]*Task) []string {
	seen := make(map[string]bool)
	var deps []string
	for _, d := range t.Dependencies {
		if !seen[d] {
			seen[d] = true
			deps = append(deps, d)
		}
	}
	// Walk parent chain
	current := t.ParentID
	for current != "" {
		if parent, ok := allTasks[current]; ok {
			for _, d := range parent.Dependencies {
				if !seen[d] {
					seen[d] = true
					deps = append(deps, d)
				}
			}
			current = parent.ParentID
		} else {
			break
		}
	}
	return deps
}

// WouldCreateCycle checks if adding newDep as a dependency would create a cycle.
func (t *Task) WouldCreateCycle(newDep string, allTasks map[string]*Task) bool {
	// DFS from newDep to see if we can reach t.ID
	visited := make(map[string]bool)
	var dfs func(id string) bool
	dfs = func(id string) bool {
		if id == t.ID {
			return true
		}
		if visited[id] {
			return false
		}
		visited[id] = true
		if task, ok := allTasks[id]; ok {
			for _, dep := range task.Dependencies {
				if dfs(dep) {
					return true
				}
			}
		}
		return false
	}
	return dfs(newDep)
}

// NeedsExpansion returns true if complexity >= 5, not already expanded, and not a subtask.
func (t *Task) NeedsExpansion() bool {
	return t.Complexity >= 5 && t.Status != Expanded && len(t.Subtasks) == 0 && t.ParentID == ""
}

// RecommendedSubtasks returns the suggested number of subtasks based on complexity.
func (t *Task) RecommendedSubtasks() int {
	switch {
	case t.Complexity >= 13:
		return 3
	case t.Complexity >= 5:
		return 2
	default:
		return 0
	}
}

// AutoAssignAgent sets agent type and model tier based on complexity.
func (t *Task) AutoAssignAgent() {
	if t.AgentType == "" {
		switch {
		case t.Complexity <= 2:
			t.AgentType = AgentFastBuilder
		default:
			t.AgentType = AgentBuilder
		}
	}
	if t.ModelTier == "" {
		switch {
		case t.Complexity <= 2:
			t.ModelTier = TierFast
		case t.Complexity <= 5:
			t.ModelTier = TierStandard
		default:
			t.ModelTier = TierSmart
		}
	}
}

// SetUpdatedNow sets UpdatedAt to current time.
func (t *Task) SetUpdatedNow() {
	t.UpdatedAt = time.Now().UTC().Format(time.RFC3339)
}

type Phase struct {
	Name     string
	Tasks    []*Task
	IDFormat string // "sequential" or "uuid"
}

// TaskMap returns tasks indexed by ID.
func (p *Phase) TaskMap() map[string]*Task {
	m := make(map[string]*Task, len(p.Tasks))
	for _, t := range p.Tasks {
		m[t.ID] = t
	}
	return m
}

// FindTask returns the task with the given ID, or nil.
func (p *Phase) FindTask(id string) *Task {
	for _, t := range p.Tasks {
		if t.ID == id {
			return t
		}
	}
	return nil
}

// ActionableTasks returns pending tasks that can be worked on:
// skip expanded parents, include subtasks of expanded parents.
func (p *Phase) ActionableTasks() []*Task {
	var result []*Task
	expandedParents := make(map[string]bool)
	for _, t := range p.Tasks {
		if t.Status == Expanded || len(t.Subtasks) > 0 {
			expandedParents[t.ID] = true
		}
	}
	for _, t := range p.Tasks {
		if t.Status != Pending {
			continue
		}
		// Skip expanded parents
		if expandedParents[t.ID] {
			continue
		}
		// Include subtasks only if parent is expanded
		if t.ParentID != "" && !expandedParents[t.ParentID] {
			continue
		}
		result = append(result, t)
	}
	return result
}

// FindNextTask finds the next ready task considering cross-phase deps.
func (p *Phase) FindNextTask(allPhases map[string]*Phase) *Task {
	doneSet := make(map[string]bool)
	// Collect all done tasks across phases
	for tag, phase := range allPhases {
		for _, t := range phase.Tasks {
			if t.Status == Done {
				doneSet[t.ID] = true
				doneSet[fmt.Sprintf("%s:%s", tag, t.ID)] = true
			}
		}
	}
	// Also mark expanded parents with all subtasks done as done
	for _, t := range p.Tasks {
		if (t.Status == Expanded || len(t.Subtasks) > 0) && allSubtasksDone(t, p) {
			doneSet[t.ID] = true
			doneSet[fmt.Sprintf("%s:%s", p.Name, t.ID)] = true
		}
	}

	taskMap := make(map[string]*Task)
	for _, ph := range allPhases {
		for _, t := range ph.Tasks {
			taskMap[t.ID] = t
			taskMap[fmt.Sprintf("%s:%s", ph.Name, t.ID)] = t
		}
	}

	actionable := p.ActionableTasks()
	// Sort for determinism
	NaturalSortTasks(actionable)
	for _, t := range actionable {
		effectiveDeps := t.GetEffectiveDeps(taskMap)
		allMet := true
		for _, dep := range effectiveDeps {
			if !doneSet[dep] {
				allMet = false
				break
			}
		}
		if allMet {
			return t
		}
	}
	return nil
}

func allSubtasksDone(t *Task, p *Phase) bool {
	if len(t.Subtasks) == 0 {
		return false
	}
	for _, subID := range t.Subtasks {
		sub := p.FindTask(subID)
		if sub == nil || sub.Status != Done {
			return false
		}
	}
	return true
}

type PhaseStats struct {
	Total       int
	Pending     int
	InProgressN int
	Done        int
	Failed      int
	Blocked     int
	Review      int
	Expanded    int
	Deferred    int
	Cancelled   int
	Complexity  int
}

// Stats computes phase statistics. Subtasks excluded from total.
// Expanded parents with all subs done count as done.
func (p *Phase) Stats() PhaseStats {
	var s PhaseStats
	for _, t := range p.Tasks {
		if t.ParentID != "" {
			continue // exclude subtasks from total
		}
		s.Total++
		status := t.Status
		// If expanded and all subtasks done, count as done
		if (status == Expanded || len(t.Subtasks) > 0) && allSubtasksDone(t, p) {
			status = Done
		} else if status == Expanded {
			s.Expanded++
			continue // don't count complexity for expanded
		}
		s.Complexity += t.Complexity
		switch status {
		case Pending:
			s.Pending++
		case InProgress:
			s.InProgressN++
		case Done:
			s.Done++
		case Failed:
			s.Failed++
		case Blocked:
			s.Blocked++
		case Review:
			s.Review++
		case Deferred:
			s.Deferred++
		case Cancelled:
			s.Cancelled++
		}
	}
	return s
}

// NextTaskID returns the next sequential ID for this phase.
func (p *Phase) NextTaskID() string {
	max := 0
	for _, t := range p.Tasks {
		if n := parseLeadingInt(t.ID); n > max {
			max = n
		}
	}
	return fmt.Sprintf("%d", max+1)
}
