package storage

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"syscall"
	"time"

	"github.com/reuben/scud/internal/model"
	"github.com/reuben/scud/internal/scg"
)

type Storage struct {
	root string
}

func New(root string) *Storage {
	return &Storage{root: root}
}

// FindRoot walks up from dir looking for .scud/ directory.
func FindRoot(dir string) (string, error) {
	for {
		if _, err := os.Stat(filepath.Join(dir, ".scud")); err == nil {
			return dir, nil
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			return "", fmt.Errorf("no .scud directory found (run 'scud init')")
		}
		dir = parent
	}
}

func (s *Storage) Root() string { return s.root }

func (s *Storage) ScudDir() string {
	return filepath.Join(s.root, ".scud")
}

func (s *Storage) TasksFile() string {
	return filepath.Join(s.root, ".scud", "tasks", "tasks.scg")
}

func (s *Storage) ConfigFile() string {
	return filepath.Join(s.root, ".scud", "config.toml")
}

func (s *Storage) ActiveTagFile() string {
	return filepath.Join(s.root, ".scud", "active-tag")
}

func (s *Storage) GuidanceDir() string {
	return filepath.Join(s.root, ".scud", "guidance")
}

// Initialize creates the .scud directory structure.
func (s *Storage) Initialize() error {
	dirs := []string{
		filepath.Join(s.root, ".scud"),
		filepath.Join(s.root, ".scud", "tasks"),
		filepath.Join(s.root, ".scud", "guidance"),
		filepath.Join(s.root, ".scud", "archive"),
	}
	for _, d := range dirs {
		if err := os.MkdirAll(d, 0755); err != nil {
			return fmt.Errorf("creating %s: %w", d, err)
		}
	}
	// Create empty tasks file if not exists
	tf := s.TasksFile()
	if _, err := os.Stat(tf); os.IsNotExist(err) {
		if err := os.WriteFile(tf, []byte(""), 0644); err != nil {
			return err
		}
	}
	return nil
}

// LoadPhases reads and parses all phases from the tasks file.
func (s *Storage) LoadPhases() (map[string]*model.Phase, error) {
	f, err := os.OpenFile(s.TasksFile(), os.O_RDONLY|os.O_CREATE, 0644)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	if err := lockFile(f, syscall.LOCK_SH); err != nil {
		return nil, fmt.Errorf("acquiring read lock: %w", err)
	}
	defer syscall.Flock(int(f.Fd()), syscall.LOCK_UN)

	content, err := io.ReadAll(f)
	if err != nil {
		return nil, err
	}
	return scg.ParseMultiPhase(string(content)), nil
}

// SavePhases writes all phases to the tasks file atomically.
func (s *Storage) SavePhases(phases map[string]*model.Phase) error {
	f, err := os.OpenFile(s.TasksFile(), os.O_RDWR|os.O_CREATE, 0644)
	if err != nil {
		return err
	}
	defer f.Close()

	if err := lockFile(f, syscall.LOCK_EX); err != nil {
		return fmt.Errorf("acquiring write lock: %w", err)
	}
	defer syscall.Flock(int(f.Fd()), syscall.LOCK_UN)

	content := scg.SerializeMultiPhase(phases)
	if err := f.Truncate(0); err != nil {
		return err
	}
	if _, err := f.Seek(0, 0); err != nil {
		return err
	}
	_, err = f.WriteString(content)
	return err
}

// UpdatePhase performs an atomic read-modify-write on a single phase.
func (s *Storage) UpdatePhase(tag string, fn func(*model.Phase) error) error {
	f, err := os.OpenFile(s.TasksFile(), os.O_RDWR|os.O_CREATE, 0644)
	if err != nil {
		return err
	}
	defer f.Close()

	if err := lockFile(f, syscall.LOCK_EX); err != nil {
		return fmt.Errorf("acquiring write lock: %w", err)
	}
	defer syscall.Flock(int(f.Fd()), syscall.LOCK_UN)

	content, err := io.ReadAll(f)
	if err != nil {
		return err
	}

	phases := scg.ParseMultiPhase(string(content))
	phase, ok := phases[tag]
	if !ok {
		phase = &model.Phase{Name: tag, IDFormat: "sequential"}
		phases[tag] = phase
	}

	if err := fn(phase); err != nil {
		return err
	}

	output := scg.SerializeMultiPhase(phases)
	if err := f.Truncate(0); err != nil {
		return err
	}
	if _, err := f.Seek(0, 0); err != nil {
		return err
	}
	_, err = f.WriteString(output)
	return err
}

// ActiveTag returns the current active tag.
func (s *Storage) ActiveTag() string {
	data, err := os.ReadFile(s.ActiveTagFile())
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(data))
}

// SetActiveTag sets the current active tag.
func (s *Storage) SetActiveTag(tag string) error {
	return os.WriteFile(s.ActiveTagFile(), []byte(tag+"\n"), 0644)
}

// LoadGuidance concatenates all .md files from the guidance directory.
func (s *Storage) LoadGuidance() string {
	entries, err := os.ReadDir(s.GuidanceDir())
	if err != nil {
		return ""
	}
	var files []string
	for _, e := range entries {
		if !e.IsDir() && strings.HasSuffix(e.Name(), ".md") {
			files = append(files, e.Name())
		}
	}
	sort.Strings(files)

	var b strings.Builder
	for _, f := range files {
		data, err := os.ReadFile(filepath.Join(s.GuidanceDir(), f))
		if err != nil {
			continue
		}
		if b.Len() > 0 {
			b.WriteString("\n\n")
		}
		b.Write(data)
	}
	return b.String()
}

// ResolveTag returns the given tag, or the active tag, or the sole phase, or an error.
// It also persists the resolved tag as the active tag for future commands.
func (s *Storage) ResolveTag(tag string) (string, error) {
	if tag != "" {
		s.SetActiveTag(tag)
		return tag, nil
	}
	if active := s.ActiveTag(); active != "" {
		return active, nil
	}
	// If only one phase exists, use it automatically
	phases, err := s.LoadPhases()
	if err == nil && len(phases) == 1 {
		for name := range phases {
			s.SetActiveTag(name)
			return name, nil
		}
	}
	return "", fmt.Errorf("no tag specified and no active tag set (use -t or 'scud tags <name>')")
}

// lockFile acquires a flock with exponential backoff.
func lockFile(f *os.File, lockType int) error {
	delay := 10 * time.Millisecond
	maxDelay := time.Second
	maxRetries := 10

	for i := 0; i < maxRetries; i++ {
		err := syscall.Flock(int(f.Fd()), lockType|syscall.LOCK_NB)
		if err == nil {
			return nil
		}
		time.Sleep(delay)
		delay *= 2
		if delay > maxDelay {
			delay = maxDelay
		}
	}
	// Final blocking attempt
	return syscall.Flock(int(f.Fd()), lockType)
}
