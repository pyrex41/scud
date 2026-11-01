package sbintegration

import "golang.org/x/sys/unix"

func killGroup(pid int) error { return unix.Kill(-pid, unix.SIGKILL) }

func observeExit(pid int) error {
	for {
		var info unix.Siginfo
		err := unix.Waitid(unix.P_PID, pid, &info, unix.WEXITED|unix.WNOWAIT, nil)
		if err != unix.EINTR {
			return err
		}
	}
}
