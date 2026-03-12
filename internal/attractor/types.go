package attractor

// PipelineGraph represents a parsed pipeline.
type PipelineGraph struct {
	Nodes []PipelineNode
	Edges []PipelineEdge
}

// PipelineNode represents a single step in a pipeline.
type PipelineNode struct {
	ID         string            `json:"id"`
	Label      string            `json:"label"`
	Type       string            `json:"type"` // start, codergen, tool, wait.human, parallel, fan_in, conditional, manager, exit
	Properties map[string]string `json:"properties,omitempty"`
}

// PipelineEdge represents a directed connection between two nodes.
type PipelineEdge struct {
	From      string `json:"from"`
	To        string `json:"to"`
	Label     string `json:"label,omitempty"`
	Condition string `json:"condition,omitempty"` // key=value expression
}

// Outcome records the result of executing a single node.
type Outcome struct {
	NodeID string `json:"node_id"`
	Status string `json:"status"` // success, failure, skipped
	Output string `json:"output,omitempty"`
	Error  string `json:"error,omitempty"`
}

// Checkpoint captures runner state so a pipeline can be resumed.
type Checkpoint struct {
	GraphFile   string            `json:"graph_file"`
	CurrentNode string            `json:"current_node"`
	Completed   []string          `json:"completed"`
	Outcomes    []Outcome         `json:"outcomes"`
	Context     map[string]string `json:"context"`
}
