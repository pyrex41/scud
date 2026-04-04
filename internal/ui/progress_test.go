package ui

import (
	"testing"
)

func TestColorFunctions(t *testing.T) {
	tests := []struct {
		name string
		fn   func(string) string
		want string
	}{
		{"Blue", Blue, blue + "test" + reset},
		{"Green", Green, green + "test" + reset},
		{"Yellow", Yellow, yellow + "test" + reset},
		{"Cyan", Cyan, cyan + "test" + reset},
		{"Red", Red, red + "test" + reset},
		{"Magenta", Magenta, magenta + "test" + reset},
		{"Bold", Bold, bold + "test" + reset},
		{"Dim", Dim, dim + "test" + reset},
		{"BoldBlue", BoldBlue, bold + blue + "test" + reset},
		{"BoldGreen", BoldGreen, bold + green + "test" + reset},
		{"BoldYellow", BoldYellow, bold + yellow + "test" + reset},
		{"BoldCyan", BoldCyan, bold + cyan + "test" + reset},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := tt.fn("test")
			if got != tt.want {
				t.Errorf("%s(%q) = %q, want %q", tt.name, "test", got, tt.want)
			}
		})
	}
}

func TestProgressBarFillCalculation(t *testing.T) {
	tests := []struct {
		name      string
		total     int
		completed int
		wantFill  int // expected filled chars out of width=40
	}{
		{"empty", 10, 0, 0},
		{"half", 10, 5, 20},
		{"full", 10, 10, 40},
		{"quarter", 8, 2, 10},
		{"zero total", 0, 0, 0},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			pb := &ProgressBar{total: tt.total, completed: tt.completed, width: 40}
			filled := 0
			if pb.total > 0 {
				filled = pb.completed * pb.width / pb.total
			}
			if filled != tt.wantFill {
				t.Errorf("filled = %d, want %d", filled, tt.wantFill)
			}
		})
	}
}

func TestSpinFramesLength(t *testing.T) {
	if len(spinFrames) == 0 {
		t.Error("spinFrames should not be empty")
	}
	if len(spinFrames) != 10 {
		t.Errorf("spinFrames has %d frames, expected 10", len(spinFrames))
	}
}

func TestMultiProgressSlotTracking(t *testing.T) {
	mp := &MultiProgress{
		total:     3,
		width:     40,
		label:     "test",
		slotIndex: make(map[string]int),
		done:      make(chan struct{}),
	}

	mp.AddTask("1", "Task one")
	mp.AddTask("2", "Task two")
	mp.AddTask("3", "Task three")

	if len(mp.slots) != 3 {
		t.Errorf("slots = %d, want 3", len(mp.slots))
	}
	if mp.slotIndex["2"] != 1 {
		t.Errorf("slotIndex[2] = %d, want 1", mp.slotIndex["2"])
	}

	mp.CompleteTask("1", true, "done")
	mp.CompleteTask("2", false, "failed")

	if mp.completed != 2 {
		t.Errorf("completed = %d, want 2", mp.completed)
	}
	if mp.failed != 1 {
		t.Errorf("failed = %d, want 1", mp.failed)
	}
	if !mp.slots[0].success {
		t.Error("slot 0 should be success")
	}
	if mp.slots[1].success {
		t.Error("slot 1 should not be success")
	}

	// Complete nonexistent task should be no-op
	mp.CompleteTask("nonexistent", true, "ok")
	if mp.completed != 2 {
		t.Errorf("completed after no-op = %d, want 2", mp.completed)
	}
}
