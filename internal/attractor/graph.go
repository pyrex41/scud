package attractor

// FindNode returns the node with the given ID, or nil.
func (g *PipelineGraph) FindNode(id string) *PipelineNode {
	for i := range g.Nodes {
		if g.Nodes[i].ID == id {
			return &g.Nodes[i]
		}
	}
	return nil
}

// Successors returns all edges originating from nodeID.
func (g *PipelineGraph) Successors(nodeID string) []PipelineEdge {
	var out []PipelineEdge
	for _, e := range g.Edges {
		if e.From == nodeID {
			out = append(out, e)
		}
	}
	return out
}

// Predecessors returns all edges pointing to nodeID.
func (g *PipelineGraph) Predecessors(nodeID string) []PipelineEdge {
	var out []PipelineEdge
	for _, e := range g.Edges {
		if e.To == nodeID {
			out = append(out, e)
		}
	}
	return out
}

// StartNode returns the first node with type "start", or nil.
func (g *PipelineGraph) StartNode() *PipelineNode {
	for i := range g.Nodes {
		if g.Nodes[i].Type == "start" {
			return &g.Nodes[i]
		}
	}
	return nil
}

// ExitNodes returns all nodes with type "exit".
func (g *PipelineGraph) ExitNodes() []PipelineNode {
	var out []PipelineNode
	for _, n := range g.Nodes {
		if n.Type == "exit" {
			out = append(out, n)
		}
	}
	return out
}
