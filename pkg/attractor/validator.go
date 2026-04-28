package attractor

import "fmt"

// Validate checks a PipelineGraph for structural issues and returns a list of
// human-readable problems. An empty slice means the graph is valid.
func Validate(g *PipelineGraph) []string {
	var issues []string

	// Build node ID set
	nodeIDs := make(map[string]bool, len(g.Nodes))
	for _, n := range g.Nodes {
		nodeIDs[n.ID] = true
	}

	// Exactly one start node
	startCount := 0
	for _, n := range g.Nodes {
		if n.Type == "start" {
			startCount++
		}
	}
	switch {
	case startCount == 0:
		issues = append(issues, "no start node found (need exactly one node with type=\"start\")")
	case startCount > 1:
		issues = append(issues, fmt.Sprintf("found %d start nodes (need exactly one)", startCount))
	}

	// At least one exit node
	exitCount := 0
	for _, n := range g.Nodes {
		if n.Type == "exit" {
			exitCount++
		}
	}
	if exitCount == 0 {
		issues = append(issues, "no exit node found (need at least one node with type=\"exit\")")
	}

	// All edge endpoints must reference valid nodes
	for _, e := range g.Edges {
		if !nodeIDs[e.From] {
			issues = append(issues, fmt.Sprintf("edge references unknown source node %q", e.From))
		}
		if !nodeIDs[e.To] {
			issues = append(issues, fmt.Sprintf("edge references unknown target node %q", e.To))
		}
	}

	// Orphan detection: every non-start node should be reachable (have an incoming edge)
	// and every non-exit node should have at least one outgoing edge.
	hasIncoming := make(map[string]bool)
	hasOutgoing := make(map[string]bool)
	for _, e := range g.Edges {
		hasIncoming[e.To] = true
		hasOutgoing[e.From] = true
	}
	for _, n := range g.Nodes {
		if n.Type != "start" && !hasIncoming[n.ID] {
			issues = append(issues, fmt.Sprintf("node %q has no incoming edges (orphan)", n.ID))
		}
		if n.Type != "exit" && !hasOutgoing[n.ID] {
			issues = append(issues, fmt.Sprintf("node %q has no outgoing edges (dead end)", n.ID))
		}
	}

	return issues
}
