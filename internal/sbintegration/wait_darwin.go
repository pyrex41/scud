package sbintegration

import (
	"errors"
	"runtime"
	"syscall"
	"unsafe"

	"golang.org/x/sys/unix"
)

func killGroup(pid int) error {
	err := unix.Kill(-pid, unix.SIGKILL)
	if errors.Is(err, unix.EPERM) {
		// Darwin returns EPERM, not ESRCH, when the only group members
		// are zombies. Keep real permission failures as supervisor errors.
		members, inspectErr := unix.SysctlKinfoProcSlice("kern.proc.pgrp", pid)
		if inspectErr == nil {
			for _, member := range members {
				if member.Proc.P_stat != 5 /* SZOMB */ {
					return err
				}
			}
			return unix.ESRCH
		}
	}
	return err
}

func observeExit(pid int) error {
	// Darwin's waitid(2) is not wrapped by x/sys/unix. siginfo_t is 104
	// bytes on supported 64-bit Darwin; an oversized, aligned buffer avoids
	// depending on its field layout (we only need the blocking exit event).
	var info [32]uint64
	for {
		_, _, errno := syscall.Syscall6(syscall.SYS_WAITID, 1 /* P_PID */, uintptr(pid), uintptr(unsafe.Pointer(&info[0])), unix.WEXITED|unix.WNOWAIT, 0, 0)
		runtime.KeepAlive(&info)
		if errno == syscall.EINTR {
			continue
		}
		if errno != 0 {
			return errno
		}
		return nil
	}
}
