package ui

import (
	"fmt"
	"os"
	"strings"
	"sync"
	"time"
)

// ANSI color codes
const (
	reset   = "\033[0m"
	bold    = "\033[1m"
	dim     = "\033[2m"
	red     = "\033[31m"
	green   = "\033[32m"
	yellow  = "\033[33m"
	blue    = "\033[34m"
	magenta = "\033[35m"
	cyan    = "\033[36m"
)

// Stderr returns os.Stderr for callers that need it.
func Stderr() *os.File { return os.Stderr }

// Semantic helpers
func Blue(s string) string    { return blue + s + reset }
func Green(s string) string   { return green + s + reset }
func Yellow(s string) string  { return yellow + s + reset }
func Cyan(s string) string    { return cyan + s + reset }
func Red(s string) string     { return red + s + reset }
func Magenta(s string) string { return magenta + s + reset }
func Bold(s string) string    { return bold + s + reset }
func Dim(s string) string     { return dim + s + reset }

func BoldBlue(s string) string    { return bold + blue + s + reset }
func BoldGreen(s string) string   { return bold + green + s + reset }
func BoldYellow(s string) string  { return bold + yellow + s + reset }
func BoldCyan(s string) string    { return bold + cyan + s + reset }

// Rule prints a horizontal rule
func Rule(color string) {
	fmt.Fprintf(os.Stderr, "%s%s%s\n", color, strings.Repeat("━", 50), reset)
}

// Header prints a pipeline header
func Header(title, subtitle string) {
	Rule(blue)
	fmt.Fprintf(os.Stderr, "%s %s\n", BoldBlue(title), Cyan(subtitle))
	Rule(blue)
	fmt.Fprintln(os.Stderr)
}

// Phase prints a phase header
func Phase(num int, msg string) {
	fmt.Fprintf(os.Stderr, "%s %s\n", BoldYellow(fmt.Sprintf("Phase %d:", num)), msg)
}

// PhaseSkipped prints a skipped phase
func PhaseSkipped(num int, msg, reason string) {
	fmt.Fprintf(os.Stderr, "%s %s %s\n", BoldYellow(fmt.Sprintf("Phase %d:", num)), msg, Dim(reason))
}

// Info prints an info line
func Info(msg string) {
	fmt.Fprintf(os.Stderr, "  %s %s\n", Cyan("→"), msg)
}

// Success prints a success line
func Success(msg string) {
	fmt.Fprintf(os.Stderr, "  %s %s\n", Green("✓"), msg)
}

// Warn prints a warning line
func Warn(msg string) {
	fmt.Fprintf(os.Stderr, "  %s %s\n", Yellow("⚠"), msg)
}

// Fail prints a failure line
func Fail(msg string) {
	fmt.Fprintf(os.Stderr, "  %s %s\n", Red("✗"), msg)
}

// Complete prints the completion banner
func Complete(msg string) {
	fmt.Fprintln(os.Stderr)
	Rule(green)
	fmt.Fprintf(os.Stderr, "%s\n", BoldGreen("✅ "+msg))
	Rule(green)
}

// Spinner shows an animated spinner with a message.
type Spinner struct {
	msg    string
	done   chan struct{}
	mu     sync.Mutex
	result string
}

var spinFrames = []string{"⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"}

// NewSpinner creates and starts a spinner.
func NewSpinner(msg string) *Spinner {
	s := &Spinner{
		msg:  msg,
		done: make(chan struct{}),
	}
	go s.run()
	return s
}

func (s *Spinner) run() {
	i := 0
	ticker := time.NewTicker(80 * time.Millisecond)
	defer ticker.Stop()
	for {
		select {
		case <-s.done:
			return
		case <-ticker.C:
			frame := spinFrames[i%len(spinFrames)]
			fmt.Fprintf(os.Stderr, "\033[2K\r  %s%s%s %s", cyan, frame, reset, s.msg)
			i++
		}
	}
}

// Stop stops the spinner and shows a result.
func (s *Spinner) Stop(success bool, msg string) {
	close(s.done)
	icon := Green("✓")
	if !success {
		icon = Red("✗")
	}
	fmt.Fprintf(os.Stderr, "\033[2K\r  %s %s\n", icon, msg)
}

// StopWarn stops the spinner with a warning.
func (s *Spinner) StopWarn(msg string) {
	close(s.done)
	fmt.Fprintf(os.Stderr, "\033[2K\r  %s %s\n", Yellow("⚠"), msg)
}

// ProgressBar shows a progress bar with task count.
type ProgressBar struct {
	total     int
	completed int
	failed    int
	width     int
	mu        sync.Mutex
	done      chan struct{}
	label     string
}

// NewProgressBar creates and starts a progress bar.
func NewProgressBar(total int, label string) *ProgressBar {
	pb := &ProgressBar{
		total: total,
		width: 40,
		done:  make(chan struct{}),
		label: label,
	}
	pb.render()
	return pb
}

func (pb *ProgressBar) render() {
	filled := 0
	if pb.total > 0 {
		filled = pb.completed * pb.width / pb.total
	}
	bar := strings.Repeat("█", filled) + strings.Repeat("░", pb.width-filled)
	fmt.Fprintf(os.Stderr, "\033[2K\r  %s [%s%s%s] %d/%d %s",
		Cyan("⠹"), cyan, bar, reset, pb.completed, pb.total, pb.label)
}

// Increment marks one task complete and prints its result.
func (pb *ProgressBar) Increment(name string, success bool) {
	pb.mu.Lock()
	defer pb.mu.Unlock()
	if success {
		pb.completed++
	} else {
		pb.failed++
		pb.completed++
	}
	// Clear bar line, print task result, re-render bar
	fmt.Fprintf(os.Stderr, "\r%s\r", strings.Repeat(" ", 80))
	if success {
		fmt.Fprintf(os.Stderr, "  %s %s\n", Green("✓"), name)
	} else {
		fmt.Fprintf(os.Stderr, "  %s %s\n", Red("✗"), name)
	}
	if pb.completed < pb.total {
		pb.render()
	}
}

// Finish clears the progress bar and prints final summary.
func (pb *ProgressBar) Finish() {
	pb.mu.Lock()
	defer pb.mu.Unlock()
	fmt.Fprintf(os.Stderr, "\r%s\r", strings.Repeat(" ", 80))
	msg := fmt.Sprintf("%d/%d complete", pb.completed, pb.total)
	if pb.failed > 0 {
		msg += fmt.Sprintf(", %s failed", Red(fmt.Sprintf("%d", pb.failed)))
	}
	fmt.Fprintf(os.Stderr, "  %s %s\n", Green("✓"), msg)
}

// TaskLine prints a task summary line (ID + title).
func TaskLine(id, title string) {
	fmt.Fprintf(os.Stderr, "  %s %s  %s\n", Cyan(id), Dim("│"), title)
}

// taskSlot tracks one active or completed task in the MultiProgress display.
type taskSlot struct {
	id      string
	name    string
	done    bool
	success bool
	msg     string
}

// MultiProgress manages stacked per-task spinners above an overall progress bar.
type MultiProgress struct {
	total     int
	completed int
	failed    int
	width     int
	label     string
	slots     []*taskSlot // ordered list of task slots
	slotIndex map[string]int
	mu        sync.Mutex
	done      chan struct{}
	rendered  bool // whether we've output any lines yet
}

// NewMultiProgress creates a multi-progress display with per-task spinners.
func NewMultiProgress(total int, label string) *MultiProgress {
	mp := &MultiProgress{
		total:     total,
		width:     40,
		label:     label,
		slotIndex: make(map[string]int),
		done:      make(chan struct{}),
	}
	go mp.run()
	return mp
}

func (mp *MultiProgress) run() {
	i := 0
	ticker := time.NewTicker(80 * time.Millisecond)
	defer ticker.Stop()
	for {
		select {
		case <-mp.done:
			return
		case <-ticker.C:
			mp.mu.Lock()
			mp.redraw(i)
			mp.mu.Unlock()
			i++
		}
	}
}

// lineCount returns how many lines we last rendered (slots + progress bar).
func (mp *MultiProgress) lineCount() int {
	return len(mp.slots) + 1
}

// redraw redraws all spinner lines + the progress bar. Must be called with mu held.
func (mp *MultiProgress) redraw(frame int) {
	// Move cursor up to overwrite previous output
	if mp.rendered {
		n := mp.lineCount()
		fmt.Fprintf(os.Stderr, "\033[%dA", n)
	}

	// Render each task slot
	for _, s := range mp.slots {
		fmt.Fprint(os.Stderr, "\033[2K") // clear line
		if s.done {
			icon := Green("✓")
			if !s.success {
				icon = Red("✗")
			}
			fmt.Fprintf(os.Stderr, "  %s %s\n", icon, s.msg)
		} else {
			f := spinFrames[frame%len(spinFrames)]
			fmt.Fprintf(os.Stderr, "  %s%s%s %s\n", cyan, f, reset, s.name)
		}
	}

	// Render overall progress bar
	filled := 0
	if mp.total > 0 {
		filled = mp.completed * mp.width / mp.total
	}
	bar := strings.Repeat("█", filled) + strings.Repeat("░", mp.width-filled)
	fmt.Fprintf(os.Stderr, "\033[2K  %s [%s%s%s] %d/%d %s\n",
		Cyan("⠹"), cyan, bar, reset, mp.completed, mp.total, mp.label)

	mp.rendered = true
}

// AddTask registers a new active task spinner.
func (mp *MultiProgress) AddTask(id, name string) {
	mp.mu.Lock()
	defer mp.mu.Unlock()
	slot := &taskSlot{id: id, name: name}
	mp.slotIndex[id] = len(mp.slots)
	mp.slots = append(mp.slots, slot)
}

// CompleteTask marks a task as done and updates the display.
func (mp *MultiProgress) CompleteTask(id string, success bool, msg string) {
	mp.mu.Lock()
	defer mp.mu.Unlock()
	idx, ok := mp.slotIndex[id]
	if !ok {
		return
	}
	mp.slots[idx].done = true
	mp.slots[idx].success = success
	mp.slots[idx].msg = msg
	if success {
		mp.completed++
	} else {
		mp.failed++
		mp.completed++
	}
}

// Finish stops the animation and prints the final summary.
func (mp *MultiProgress) Finish() {
	close(mp.done)
	// Give the ticker goroutine time to exit
	time.Sleep(100 * time.Millisecond)

	mp.mu.Lock()
	defer mp.mu.Unlock()

	// Final redraw to show all completed states
	if mp.rendered {
		n := mp.lineCount()
		fmt.Fprintf(os.Stderr, "\033[%dA", n)
	}
	for _, s := range mp.slots {
		fmt.Fprint(os.Stderr, "\033[2K")
		icon := Green("✓")
		if !s.success {
			icon = Red("✗")
		}
		fmt.Fprintf(os.Stderr, "  %s %s\n", icon, s.msg)
	}

	// Replace progress bar with summary
	fmt.Fprint(os.Stderr, "\033[2K")
	msg := fmt.Sprintf("%d/%d complete", mp.completed, mp.total)
	if mp.failed > 0 {
		msg += fmt.Sprintf(", %s failed", Red(fmt.Sprintf("%d", mp.failed)))
	}
	fmt.Fprintf(os.Stderr, "  %s %s\n", Green("✓"), msg)
}
