//go:build !windows && !linux && !darwin

package sbintegration

import "os/exec"

// Other platforms currently provide only os/exec's direct-child cancellation.
// Descendant ownership requires a non-reaping exit observation implementation.
func runProcess(command *exec.Cmd) error { return command.Run() }
