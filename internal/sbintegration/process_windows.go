//go:build windows

package sbintegration

import "os/exec"

// Windows currently guarantees cancellation of the direct launcher only.
// A race-free descendant guarantee needs a suspended launch assigned to a Job
// Object before resumption; os/exec does not expose the primary thread handle.
// Assigning a Job after Start would let a fast launcher escape before assignment.
func runProcess(command *exec.Cmd) error { return command.Run() }
