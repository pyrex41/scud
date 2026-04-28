package attractor

import "sync"

// PipelineContext is a thread-safe key-value store used to pass data between
// pipeline nodes.
type PipelineContext struct {
	mu   sync.RWMutex
	data map[string]string
}

// NewContext returns an empty PipelineContext.
func NewContext() *PipelineContext {
	return &PipelineContext{data: make(map[string]string)}
}

// Set stores a value.
func (c *PipelineContext) Set(key, value string) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.data[key] = value
}

// Get retrieves a value (empty string if missing).
func (c *PipelineContext) Get(key string) string {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return c.data[key]
}

// All returns a snapshot of all key-value pairs.
func (c *PipelineContext) All() map[string]string {
	c.mu.RLock()
	defer c.mu.RUnlock()
	out := make(map[string]string, len(c.data))
	for k, v := range c.data {
		out[k] = v
	}
	return out
}

// Load bulk-sets values from the provided map.
func (c *PipelineContext) Load(data map[string]string) {
	c.mu.Lock()
	defer c.mu.Unlock()
	for k, v := range data {
		c.data[k] = v
	}
}
