package attractor

import (
	"bufio"
	"fmt"
	"regexp"
	"strings"
)

var (
	// digraph name { ... }
	digraphRe = regexp.MustCompile(`^\s*digraph\s+(\w+)\s*\{`)
	// node [label="..." type="..." key="val"]
	nodeRe = regexp.MustCompile(`^\s*(\w+)\s*\[(.+)\]`)
	// edge: a -> b  or  a -> b [label="..." condition="..."]
	edgeRe = regexp.MustCompile(`^\s*(\w+)\s*->\s*(\w+)\s*(?:\[(.+)\])?`)
	// key="value" inside attribute brackets
	attrRe = regexp.MustCompile(`(\w+)\s*=\s*"([^"]*)"`)
)

// ParseDOT parses a DOT-format string into a PipelineGraph.
func ParseDOT(content string) (*PipelineGraph, error) {
	g := &PipelineGraph{}
	seenNodes := make(map[string]bool)

	scanner := bufio.NewScanner(strings.NewReader(content))
	lineNum := 0
	inGraph := false

	for scanner.Scan() {
		lineNum++
		line := scanner.Text()
		trimmed := strings.TrimSpace(line)

		// Skip blank lines and comments
		if trimmed == "" || strings.HasPrefix(trimmed, "//") {
			continue
		}

		// Opening digraph
		if !inGraph {
			if digraphRe.MatchString(trimmed) {
				inGraph = true
			}
			continue
		}

		// Closing brace
		if trimmed == "}" {
			break
		}

		// Try edge first (since edge lines also contain node IDs)
		if m := edgeRe.FindStringSubmatch(trimmed); m != nil {
			from := m[1]
			to := m[2]
			edge := PipelineEdge{From: from, To: to}

			if m[3] != "" {
				attrs := parseAttrs(m[3])
				edge.Label = attrs["label"]
				edge.Condition = attrs["condition"]
			}

			// Auto-create nodes that haven't been declared
			for _, id := range []string{from, to} {
				if !seenNodes[id] {
					seenNodes[id] = true
					g.Nodes = append(g.Nodes, PipelineNode{
						ID:         id,
						Label:      id,
						Properties: make(map[string]string),
					})
				}
			}

			g.Edges = append(g.Edges, edge)
			continue
		}

		// Try node declaration
		if m := nodeRe.FindStringSubmatch(trimmed); m != nil {
			id := m[1]
			attrs := parseAttrs(m[2])
			node := PipelineNode{
				ID:         id,
				Label:      id,
				Properties: make(map[string]string),
			}
			if v, ok := attrs["label"]; ok {
				node.Label = v
			}
			if v, ok := attrs["type"]; ok {
				node.Type = v
			}
			// All other attrs go into Properties
			for k, v := range attrs {
				if k != "label" && k != "type" {
					node.Properties[k] = v
				}
			}

			if seenNodes[id] {
				// Update existing implicit node
				for i := range g.Nodes {
					if g.Nodes[i].ID == id {
						g.Nodes[i] = node
						break
					}
				}
			} else {
				seenNodes[id] = true
				g.Nodes = append(g.Nodes, node)
			}
			continue
		}
	}

	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("reading DOT input: %w", err)
	}

	if !inGraph {
		return nil, fmt.Errorf("no digraph found in input")
	}

	return g, nil
}

// parseAttrs extracts key="value" pairs from a DOT attribute string.
func parseAttrs(s string) map[string]string {
	attrs := make(map[string]string)
	for _, m := range attrRe.FindAllStringSubmatch(s, -1) {
		attrs[m[1]] = m[2]
	}
	return attrs
}
