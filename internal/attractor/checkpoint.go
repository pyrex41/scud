package attractor

import (
	"encoding/json"
	"fmt"
	"os"
)

// SaveCheckpoint writes the checkpoint to a JSON file.
func SaveCheckpoint(cp *Checkpoint, path string) error {
	data, err := json.MarshalIndent(cp, "", "  ")
	if err != nil {
		return fmt.Errorf("marshaling checkpoint: %w", err)
	}
	if err := os.WriteFile(path, data, 0644); err != nil {
		return fmt.Errorf("writing checkpoint to %s: %w", path, err)
	}
	return nil
}

// LoadCheckpoint reads a checkpoint from a JSON file.
func LoadCheckpoint(path string) (*Checkpoint, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("reading checkpoint from %s: %w", path, err)
	}
	var cp Checkpoint
	if err := json.Unmarshal(data, &cp); err != nil {
		return nil, fmt.Errorf("unmarshaling checkpoint: %w", err)
	}
	return &cp, nil
}
