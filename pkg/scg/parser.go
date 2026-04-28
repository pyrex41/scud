package scg

import (
	"bufio"
	"fmt"
	"strings"
	"time"

	"github.com/reuben/scud/pkg/model"
)

type section int

const (
	sectionNone section = iota
	sectionMeta
	sectionNodes
	sectionEdges
	sectionParents
	sectionAssignments
	sectionAgents
	sectionDetails
	sectionSkip // sections we ignore
)

// ParsePhase parses a single SCG phase from text.
func ParsePhase(content string) *model.Phase {
	p := &model.Phase{
		IDFormat: "sequential",
	}
	taskMap := make(map[string]*model.Task)
	var currentSection section
	var detailTaskID, detailField string
	var detailBuf strings.Builder

	flushDetail := func() {
		if detailTaskID != "" && detailField != "" {
			if t, ok := taskMap[detailTaskID]; ok {
				val := strings.TrimRight(detailBuf.String(), "\n")
				switch detailField {
				case "description":
					t.Description = val
				case "details":
					t.Details = val
				case "test_strategy":
					t.TestStrategy = val
				}
			}
			detailTaskID = ""
			detailField = ""
			detailBuf.Reset()
		}
	}

	scanner := bufio.NewScanner(strings.NewReader(content))
	for scanner.Scan() {
		line := scanner.Text()

		// Section headers
		trimmed := strings.TrimSpace(line)
		if trimmed == "" || strings.HasPrefix(trimmed, "#") {
			// Check for phase name in comment
			if strings.HasPrefix(trimmed, "# Phase:") {
				p.Name = strings.TrimSpace(strings.TrimPrefix(trimmed, "# Phase:"))
			}
			continue
		}

		// Section transitions
		if strings.HasPrefix(trimmed, "@") {
			flushDetail()
			switch {
			case strings.HasPrefix(trimmed, "@meta"):
				currentSection = sectionMeta
			case trimmed == "@nodes":
				currentSection = sectionNodes
			case trimmed == "@edges":
				currentSection = sectionEdges
			case trimmed == "@parents":
				currentSection = sectionParents
			case trimmed == "@assignments":
				currentSection = sectionAssignments
			case trimmed == "@agents":
				currentSection = sectionAgents
			case trimmed == "@details":
				currentSection = sectionDetails
			default:
				currentSection = sectionSkip
			}
			continue
		}

		// Closing brace for @meta
		if currentSection == sectionMeta && trimmed == "}" {
			currentSection = sectionNone
			continue
		}

		switch currentSection {
		case sectionMeta:
			parseMeta(trimmed, p)
		case sectionNodes:
			if t := parseNode(trimmed); t != nil {
				p.Tasks = append(p.Tasks, t)
				taskMap[t.ID] = t
			}
		case sectionEdges:
			parseEdge(trimmed, taskMap)
		case sectionParents:
			parseParent(trimmed, taskMap)
		case sectionAssignments:
			parseAssignment(trimmed, taskMap)
		case sectionAgents:
			parseAgent(trimmed, taskMap)
		case sectionDetails:
			parseDetailLine(line, &detailTaskID, &detailField, &detailBuf, taskMap, flushDetail)
		}
	}
	flushDetail()
	return p
}

func parseMeta(line string, p *model.Phase) {
	line = strings.TrimSpace(line)
	line = strings.TrimSuffix(line, "{")
	line = strings.TrimSpace(line)
	parts := strings.SplitN(line, " ", 2)
	if len(parts) < 2 {
		return
	}
	key := strings.TrimSpace(parts[0])
	val := strings.TrimSpace(parts[1])
	switch key {
	case "name":
		p.Name = val
	case "id_format":
		p.IDFormat = val
	}
}

func parseNode(line string) *model.Task {
	parts := SplitByPipe(line)
	if len(parts) < 5 {
		return nil
	}
	t := &model.Task{
		ID:        strings.TrimSpace(parts[0]),
		Title:     UnescapeField(strings.TrimSpace(parts[1])),
		Status:    model.StatusFromCode(strings.TrimSpace(parts[2])),
		Priority:  model.Medium,
		CreatedAt: time.Now().UTC().Format(time.RFC3339),
	}
	if c := strings.TrimSpace(parts[3]); c != "" {
		fmt.Sscanf(c, "%d", &t.Complexity)
	}
	if len(parts) >= 5 {
		t.Priority = model.PriorityFromCode(strings.TrimSpace(parts[4]))
	}
	return t
}

func parseEdge(line string, taskMap map[string]*model.Task) {
	// Format: dependent -> dependency
	// Skip behavioral edges (~~, >>, !=)
	if !strings.Contains(line, "->") {
		return
	}
	parts := strings.SplitN(line, "->", 2)
	if len(parts) != 2 {
		return
	}
	dependent := strings.TrimSpace(parts[0])
	dependency := strings.TrimSpace(parts[1])
	// Strip optional label/condition suffix
	if idx := strings.Index(dependency, "|"); idx >= 0 {
		dependency = strings.TrimSpace(dependency[:idx])
	}
	if t, ok := taskMap[dependent]; ok {
		t.Dependencies = append(t.Dependencies, dependency)
	}
}

func parseParent(line string, taskMap map[string]*model.Task) {
	// Format: parent: child1, child2
	parts := strings.SplitN(line, ":", 2)
	if len(parts) != 2 {
		return
	}
	parentID := strings.TrimSpace(parts[0])
	children := strings.Split(parts[1], ",")
	parent, ok := taskMap[parentID]
	if !ok {
		return
	}
	for _, c := range children {
		childID := strings.TrimSpace(c)
		if childID == "" {
			continue
		}
		parent.Subtasks = append(parent.Subtasks, childID)
		if child, ok := taskMap[childID]; ok {
			child.ParentID = parentID
		}
	}
}

func parseAssignment(line string, taskMap map[string]*model.Task) {
	parts := SplitByPipe(line)
	if len(parts) < 2 {
		return
	}
	id := strings.TrimSpace(parts[0])
	assignee := strings.TrimSpace(parts[1])
	if t, ok := taskMap[id]; ok {
		t.AssignedTo = assignee
	}
}

func parseAgent(line string, taskMap map[string]*model.Task) {
	parts := SplitByPipe(line)
	if len(parts) < 2 {
		return
	}
	id := strings.TrimSpace(parts[0])
	agentType := strings.TrimSpace(parts[1])
	if t, ok := taskMap[id]; ok {
		t.AgentType = model.AgentType(agentType)
		if len(parts) >= 3 {
			t.ModelTier = model.ModelTier(strings.TrimSpace(parts[2]))
		}
	}
}

func parseDetailLine(line string, taskID, field *string, buf *strings.Builder, taskMap map[string]*model.Task, flush func()) {
	trimmed := strings.TrimSpace(line)

	// Check if this is a new detail header: "id | field |"
	if !strings.HasPrefix(line, "  ") && strings.Contains(trimmed, "|") {
		parts := SplitByPipe(trimmed)
		if len(parts) >= 2 {
			id := strings.TrimSpace(parts[0])
			f := strings.TrimSpace(parts[1])
			if _, ok := taskMap[id]; ok && (f == "description" || f == "details" || f == "test_strategy") {
				flush()
				*taskID = id
				*field = f
				return
			}
		}
	}

	// Continuation line (indented with 2 spaces)
	if *taskID != "" && strings.HasPrefix(line, "  ") {
		if buf.Len() > 0 {
			buf.WriteByte('\n')
		}
		buf.WriteString(strings.TrimPrefix(line, "  "))
	}
}

// ParseMultiPhase parses a multi-phase SCG file (phases separated by \n---\n).
func ParseMultiPhase(content string) map[string]*model.Phase {
	phases := make(map[string]*model.Phase)
	if strings.TrimSpace(content) == "" {
		return phases
	}

	// Split on --- separator
	sections := splitPhases(content)
	for _, sec := range sections {
		sec = strings.TrimSpace(sec)
		if sec == "" {
			continue
		}
		p := ParsePhase(sec)
		if p.Name != "" {
			phases[p.Name] = p
		}
	}
	return phases
}

func splitPhases(content string) []string {
	var sections []string
	var current strings.Builder
	scanner := bufio.NewScanner(strings.NewReader(content))
	for scanner.Scan() {
		line := scanner.Text()
		if strings.TrimSpace(line) == "---" {
			sections = append(sections, current.String())
			current.Reset()
		} else {
			current.WriteString(line)
			current.WriteByte('\n')
		}
	}
	if current.Len() > 0 {
		sections = append(sections, current.String())
	}
	return sections
}
