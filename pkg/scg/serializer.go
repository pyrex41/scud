package scg

import (
	"fmt"
	"sort"
	"strings"
	"time"

	"github.com/reuben/scud/pkg/model"
)

// SerializePhase serializes a single phase to SCG format.
func SerializePhase(p *model.Phase) string {
	var b strings.Builder

	b.WriteString("# SCUD Graph v1\n")
	b.WriteString(fmt.Sprintf("# Phase: %s\n\n", p.Name))

	// @meta
	b.WriteString("@meta {\n")
	b.WriteString(fmt.Sprintf("  name %s\n", p.Name))
	if p.IDFormat != "" && p.IDFormat != "sequential" {
		b.WriteString(fmt.Sprintf("  id_format %s\n", p.IDFormat))
	}
	b.WriteString(fmt.Sprintf("  updated %s\n", time.Now().UTC().Format(time.RFC3339)))
	b.WriteString("}\n\n")

	// Sort tasks by natural order
	tasks := make([]*model.Task, len(p.Tasks))
	copy(tasks, p.Tasks)
	model.NaturalSortTasks(tasks)

	// @nodes
	b.WriteString("@nodes\n")
	b.WriteString("# id | title | status | complexity | priority\n")
	for _, t := range tasks {
		b.WriteString(fmt.Sprintf("%s | %s | %s | %d | %s\n",
			t.ID,
			EscapeField(t.Title),
			model.StatusCode(t.Status),
			t.Complexity,
			model.PriorityCode(t.Priority),
		))
	}

	// @edges - only write if there are dependencies
	hasEdges := false
	for _, t := range tasks {
		if len(t.Dependencies) > 0 {
			hasEdges = true
			break
		}
	}
	if hasEdges {
		b.WriteString("\n@edges\n")
		b.WriteString("# dependent -> dependency\n")
		for _, t := range tasks {
			deps := make([]string, len(t.Dependencies))
			copy(deps, t.Dependencies)
			model.NaturalSort(deps)
			for _, dep := range deps {
				b.WriteString(fmt.Sprintf("%s -> %s\n", t.ID, dep))
			}
		}
	}

	// @parents
	hasParents := false
	for _, t := range tasks {
		if len(t.Subtasks) > 0 {
			hasParents = true
			break
		}
	}
	if hasParents {
		b.WriteString("\n@parents\n")
		b.WriteString("# parent: child1, child2\n")
		for _, t := range tasks {
			if len(t.Subtasks) > 0 {
				subs := make([]string, len(t.Subtasks))
				copy(subs, t.Subtasks)
				model.NaturalSort(subs)
				b.WriteString(fmt.Sprintf("%s: %s\n", t.ID, strings.Join(subs, ", ")))
			}
		}
	}

	// @assignments
	hasAssignments := false
	for _, t := range tasks {
		if t.AssignedTo != "" {
			hasAssignments = true
			break
		}
	}
	if hasAssignments {
		b.WriteString("\n@assignments\n")
		b.WriteString("# id | assigned_to\n")
		for _, t := range tasks {
			if t.AssignedTo != "" {
				b.WriteString(fmt.Sprintf("%s | %s\n", t.ID, t.AssignedTo))
			}
		}
	}

	// @agents
	hasAgents := false
	for _, t := range tasks {
		if t.AgentType != "" {
			hasAgents = true
			break
		}
	}
	if hasAgents {
		b.WriteString("\n@agents\n")
		b.WriteString("# id | agent_type | model_tier\n")
		for _, t := range tasks {
			if t.AgentType != "" {
				tier := t.ModelTier
				if tier == "" {
					tier = model.TierStandard
				}
				b.WriteString(fmt.Sprintf("%s | %s | %s\n", t.ID, t.AgentType, tier))
			}
		}
	}

	// @details
	hasDetails := false
	for _, t := range tasks {
		if t.Description != "" || t.Details != "" || t.TestStrategy != "" {
			hasDetails = true
			break
		}
	}
	if hasDetails {
		b.WriteString("\n@details\n")
		for _, t := range tasks {
			writeDetail(&b, t.ID, "description", t.Description)
			writeDetail(&b, t.ID, "details", t.Details)
			writeDetail(&b, t.ID, "test_strategy", t.TestStrategy)
		}
	}

	return b.String()
}

func writeDetail(b *strings.Builder, id, field, value string) {
	if value == "" {
		return
	}
	b.WriteString(fmt.Sprintf("%s | %s |\n", id, field))
	for _, line := range strings.Split(value, "\n") {
		b.WriteString("  ")
		b.WriteString(line)
		b.WriteByte('\n')
	}
}

// SerializeMultiPhase serializes multiple phases to a single SCG string.
func SerializeMultiPhase(phases map[string]*model.Phase) string {
	if len(phases) == 0 {
		return ""
	}

	// Sort phase names for deterministic output
	names := make([]string, 0, len(phases))
	for name := range phases {
		names = append(names, name)
	}
	sort.Strings(names)

	var parts []string
	for _, name := range names {
		parts = append(parts, SerializePhase(phases[name]))
	}
	return strings.Join(parts, "\n---\n\n")
}
