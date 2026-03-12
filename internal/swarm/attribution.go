package swarm

import (
	"context"
	"fmt"
	"os/exec"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"time"
)

// Attribution represents a failure attributed to a specific task.
type Attribution struct {
	TaskID     string
	File       string
	Line       int
	Confidence string // "high", "medium", "low"
	Reason     string
}

var (
	fileLineRe = regexp.MustCompile(`([^\s:]+):(\d+)`)
	taskIDRe   = regexp.MustCompile(`\[([^\]]+)\]`)
)

// AttributeFailure analyzes validation failure output to identify which tasks caused it.
// It parses stderr for file:line patterns, runs git blame to find [TASK-ID] commit prefixes,
// and matches against the given waveTasks.
func AttributeFailure(ctx context.Context, vr ValidationResult, workDir string, waveTasks []string) []Attribution {
	waveSet := make(map[string]bool, len(waveTasks))
	for _, id := range waveTasks {
		waveSet[id] = true
	}

	// Collect file:line patterns from all failed command stderr/stdout
	type fileLine struct {
		file string
		line int
	}
	var locations []fileLine
	seen := make(map[string]bool)

	for _, cr := range vr.Results {
		if cr.Passed {
			continue
		}
		for _, output := range []string{cr.Stderr, cr.Stdout} {
			matches := fileLineRe.FindAllStringSubmatch(output, -1)
			for _, m := range matches {
				lineNum, err := strconv.Atoi(m[2])
				if err != nil {
					continue
				}
				key := fmt.Sprintf("%s:%d", m[1], lineNum)
				if !seen[key] {
					seen[key] = true
					locations = append(locations, fileLine{file: m[1], line: lineNum})
				}
			}
		}
	}

	if len(locations) == 0 {
		return nil
	}

	// For each file:line, run git blame to find task IDs
	taskHits := make(map[string][]string) // taskID -> list of reasons
	for _, loc := range locations {
		blameCtx, cancel := context.WithTimeout(ctx, 5*time.Second)
		lineRange := fmt.Sprintf("%d,%d", loc.line, loc.line)
		cmd := exec.CommandContext(blameCtx, "git", "blame", "-L", lineRange, "--", loc.file)
		cmd.Dir = workDir
		out, err := cmd.Output()
		cancel()
		if err != nil {
			continue
		}

		blameOutput := string(out)
		idMatches := taskIDRe.FindAllStringSubmatch(blameOutput, -1)
		for _, m := range idMatches {
			taskID := m[1]
			if waveSet[taskID] {
				reason := fmt.Sprintf("%s:%d", loc.file, loc.line)
				taskHits[taskID] = append(taskHits[taskID], reason)
			}
		}
	}

	if len(taskHits) == 0 {
		return nil
	}

	// Build attributions with confidence scoring
	totalMatched := len(taskHits)
	var attributions []Attribution
	for taskID, reasons := range taskHits {
		confidence := "low"
		if totalMatched == 1 {
			confidence = "high"
		} else if totalMatched <= 3 {
			confidence = "medium"
		}

		attributions = append(attributions, Attribution{
			TaskID:     taskID,
			File:       reasons[0],
			Line:       0, // already encoded in File as file:line
			Confidence: confidence,
			Reason:     fmt.Sprintf("git blame matched %d error location(s): %s", len(reasons), strings.Join(reasons, ", ")),
		})
	}

	// Sort by confidence: high > medium > low
	confidenceOrder := map[string]int{"high": 0, "medium": 1, "low": 2}
	sort.Slice(attributions, func(i, j int) bool {
		return confidenceOrder[attributions[i].Confidence] < confidenceOrder[attributions[j].Confidence]
	})

	return attributions
}
