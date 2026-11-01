package cmd

import (
	"bytes"
	"context"
	"errors"
	"io"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"
	"time"

	"github.com/reuben/scud/internal/taskapi"
	"github.com/spf13/cobra"
)

func TestTaskListFilteredExportImportsStandalone(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, ".scud", "tasks"), 0755); err != nil {
		t.Fatal(err)
	}
	t.Chdir(root)
	input := `{"schema_version":1,"phases":[
	{"tag":"app","name":"App","id_format":"sequential","tasks":[{"id":"1","title":"App","status":"pending","priority":"medium","dependencies":["base:1"]}]},
	{"tag":"base","name":"Base","id_format":"sequential","tasks":[{"id":"1","title":"Base","status":"pending","priority":"medium"},{"id":"2","title":"Extra","status":"pending","priority":"medium","dependencies":["tools:1"]}]},
	{"tag":"tools","name":"Tools","id_format":"sequential","tasks":[{"id":"1","title":"Tools","status":"pending","priority":"medium"}]},
	{"tag":"unused","name":"Unused","id_format":"sequential","tasks":[]}]}`
	importCmd := NewTaskImportCmd()
	importCmd.SetIn(strings.NewReader(input))
	importCmd.SetOut(io.Discard)
	importCmd.SetArgs([]string{"-"})
	if err := importCmd.Execute(); err != nil {
		t.Fatal(err)
	}
	for _, tag := range []string{"", "app"} {
		var out bytes.Buffer
		c := NewTaskListCmd()
		c.SetOut(&out)
		c.SetArgs([]string{"--tag", tag})
		if err := c.Execute(); err != nil {
			t.Fatal(err)
		}
		d, err := taskapi.Decode[taskapi.Document](bytes.NewReader(out.Bytes()))
		if err != nil {
			t.Fatal(err)
		}
		tags := []string{}
		for _, p := range d.Phases {
			tags = append(tags, p.Tag)
		}
		want := []string{"app", "base", "tools"}
		if tag == "" {
			want = append(want, "unused")
		}
		if !slices.Equal(tags, want) {
			t.Fatalf("export tags %v, want %v", tags, want)
		}
		if _, err := taskapi.ValidateDocument(d); err != nil {
			t.Fatal(err)
		}
		other := t.TempDir()
		if err := os.MkdirAll(filepath.Join(other, ".scud", "tasks"), 0755); err != nil {
			t.Fatal(err)
		}
		s, err := taskapi.Open(other)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := s.Import(context.Background(), d, taskapi.ImportOptions{}); err != nil {
			t.Fatal(err)
		}
	}
}

func TestTaskInputCancellation(t *testing.T) {
	for _, makeCmd := range []func() *cobra.Command{NewTaskImportCmd, NewTaskValidateCmd, NewTaskUpdateCmd} {
		c := makeCmd()
		t.Run(c.Name(), func(t *testing.T) {
			r, w := io.Pipe()
			defer r.Close()
			defer w.Close()
			ctx, cancel := context.WithCancel(context.Background())
			defer cancel()
			c.SetContext(ctx)
			c.SetIn(r)
			c.SetOut(io.Discard)
			c.SetErr(io.Discard)
			c.SilenceErrors, c.SilenceUsage = true, true
			switch c.Name() {
			case "import":
				c.SetArgs([]string{"-"})
			case "validate":
				c.SetArgs([]string{"--input", "-"})
			case "update":
				c.SetArgs([]string{"1", "--input", "-"})
			}
			done := make(chan error, 1)
			go func() { done <- c.Execute() }()
			// Receiving an initial partial write confirms decoding has started and
			// is now waiting for more stdin rather than a complete JSON value.
			if _, err := w.Write([]byte("{")); err != nil {
				t.Fatal(err)
			}
			cancel()
			select {
			case err := <-done:
				if !errors.Is(err, context.Canceled) {
					t.Fatalf("got %v, want cancellation", err)
				}
			case <-time.After(2 * time.Second):
				t.Fatal("blocked input ignored cancellation")
			}
		})
	}
}
