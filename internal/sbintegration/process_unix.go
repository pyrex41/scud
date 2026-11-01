//go:build linux || darwin

package sbintegration

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"sync"
	"syscall"
)

// runProcess owns a private process group until its launcher has exited. Observe
// exit without reaping first: even an empty group cannot have its ID recycled
// while its original leader remains our unreaped child. Never signal a saved
// numeric PID after Cmd.Wait has released that ownership.
func runProcess(command *exec.Cmd) error {
	command.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
	var mu sync.Mutex
	cleaned := false
	var cleanupErr error
	cleanup := func() error {
		mu.Lock()
		defer mu.Unlock()
		if cleaned {
			return os.ErrProcessDone
		}
		cleaned = true
		if command.Process == nil {
			return os.ErrProcessDone
		}
		err := killGroup(command.Process.Pid)
		if errors.Is(err, syscall.ESRCH) {
			return os.ErrProcessDone
		}
		cleanupErr = err
		return err
	}
	command.Cancel = cleanup
	if err := command.Start(); err != nil {
		return err
	}
	observeErr := observeExit(command.Process.Pid)
	if observeErr != nil {
		observeErr = fmt.Errorf("observing sb launcher exit: %w", observeErr)
	}
	if !errors.Is(observeErr, syscall.ECHILD) {
		// This also handles successful launchers with children that close all
		// stdio: neither context cancellation nor WaitDelay detects those.
		// If observation itself was denied/unsupported, terminate the owned
		// invocation before Wait rather than disabling cancellation and hanging.
		// No code here has reaped this private child, and ECHILD is handled
		// separately because it would invalidate that ownership assumption.
		_ = cleanup()
	} else {
		// Ownership is no longer provable if observing the child failed. Seal
		// cancellation rather than ever risk signaling a recycled group ID.
		mu.Lock()
		cleaned = true
		mu.Unlock()
	}
	waitErr := command.Wait()
	if cleanupErr != nil {
		cleanupErr = fmt.Errorf("cleaning sb process group: %w", cleanupErr)
	}
	return errors.Join(waitErr, observeErr, cleanupErr)
}
