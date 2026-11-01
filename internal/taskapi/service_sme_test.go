package taskapi

import (
	"context"
	"encoding/json"
	"math/rand"
	"os"
	"reflect"
	"slices"
	"strings"
	"testing"

	"github.com/reuben/scud/pkg/model"
)

func TestEmptyDependencyPatchNoOp(t *testing.T) {
	ctx := context.Background()
	for _, field := range []string{"", `,"dependencies":null`, `,"dependencies":[]`} {
		t.Run(field, func(t *testing.T) {
			s := setup(t)
			input := `{"schema_version":1,"phases":[{"tag":"app","name":"App","id_format":"sequential","tasks":[{"id":"1","title":"one","status":"pending","priority":"medium","updated_at":"old"` + field + `},{"id":"2","title":"two","status":"pending","priority":"medium"}]}]}`
			d, err := Decode[Document](strings.NewReader(input))
			if err != nil {
				t.Fatal(err)
			}
			before, err := s.Import(ctx, d, ImportOptions{})
			if err != nil {
				t.Fatal(err)
			}
			persisted, err := os.ReadFile(s.file())
			if err != nil {
				t.Fatal(err)
			}
			for _, patch := range []string{`{}`, `{"dependencies":null}`, `{"dependencies":[]}`} {
				p, err := Decode[Patch](strings.NewReader(patch))
				if err != nil {
					t.Fatal(err)
				}
				after, err := s.Update(ctx, Ref{Tag: "app", ID: "1"}, p, before.Revision)
				if err != nil {
					t.Fatal(err)
				}
				if !reflect.DeepEqual(after, before) {
					t.Fatalf("no-op %s changed snapshot", patch)
				}
				b, err := os.ReadFile(s.file())
				if err != nil || string(b) != string(persisted) {
					t.Fatalf("no-op wrote file: %v", err)
				}
			}
			deps := []string{"2"}
			after, err := s.Update(ctx, Ref{Tag: "app", ID: "1"}, Patch{Dependencies: &deps}, before.Revision)
			if err != nil {
				t.Fatal(err)
			}
			if after.Revision == before.Revision || after.Phases[0].Tasks[0].UpdatedAt == "old" {
				t.Fatal("real dependency edit did not change revision/timestamp")
			}
		})
	}
}

func TestSnapshotCanonicalPermutation(t *testing.T) {
	ids := []string{"2", "10", "1a", "1", "01", "1.2", "01.10", "000", "0", "99999999999999999999999999", "100000000000000000000000000", "550e8400-e29b-41d4-a716-446655440000", "a.b"}
	rng := rand.New(rand.NewSource(1))
	var want Snapshot
	for trial := range 50 {
		rng.Shuffle(len(ids), func(i, j int) { ids[i], ids[j] = ids[j], ids[i] })
		tasks := make([]*model.Task, 0, len(ids))
		for _, id := range ids {
			x := task(id)
			if trial%2 == 0 {
				x.Dependencies = []string{}
				x.Subtasks = []string{}
			}
			tasks = append(tasks, x)
		}
		d := Document{1, []Phase{{"app", "App", "sequential", tasks}}}
		original, _ := json.Marshal(d)
		got, err := ValidateDocument(d)
		if err != nil {
			t.Fatal(err)
		}
		if trial < 5 {
			s := setup(t)
			imported, err := s.Import(context.Background(), d, ImportOptions{})
			if err != nil {
				t.Fatal(err)
			}
			persisted, err := s.Snapshot(context.Background())
			if err != nil || !reflect.DeepEqual(imported, got) || !reflect.DeepEqual(persisted, got) {
				t.Fatalf("service import/read changed canonical snapshot: %v", err)
			}
		}
		unchanged, _ := json.Marshal(d)
		if string(original) != string(unchanged) {
			t.Fatal("validation mutated caller")
		}
		if trial == 0 {
			want = got
		} else if !reflect.DeepEqual(got, want) {
			t.Fatalf("permutation changed canonical snapshot/revision/ready: %s vs %s", got.Revision, want.Revision)
		}
	}
}

func TestSnapshotPreservesDependencyAndSubtaskSequences(t *testing.T) {
	one, two, parent, childA, childB := task("1"), task("2"), task("3"), task("3.1"), task("3.2")
	parent.Dependencies = []string{"2", "1"}
	parent.Subtasks = []string{"3.2", "3.1"}
	childA.ParentID, childB.ParentID = "3", "3"
	d := Document{1, []Phase{{"app", "App", "sequential", []*model.Task{one, two, parent, childA, childB}}}}
	snap, err := ValidateDocument(d)
	if err != nil {
		t.Fatal(err)
	}
	got := snap.Phases[0].Tasks[2]
	if !slices.Equal(got.Dependencies, parent.Dependencies) || !slices.Equal(got.Subtasks, parent.Subtasks) {
		t.Fatal("normalization reordered collection entries")
	}
}

func TestFilterDocumentWholePhaseClosure(t *testing.T) {
	app, parent, child := task("1"), task("10"), task("10.1")
	app.Dependencies = []string{"base:1"}
	parent.Subtasks = []string{child.ID}
	child.ParentID = parent.ID
	parent.Dependencies = []string{"tools:1"}
	base, extra, tools := task("1"), task("2"), task("1")
	extra.Dependencies = []string{"foundation:1"} // An unreferenced task still requires its own phase closure.
	d := Document{1, []Phase{
		{"unused", "Unused", "sequential", []*model.Task{task("1")}},
		{"tools", "Tools", "sequential", []*model.Task{tools}},
		{"app", "App", "sequential", []*model.Task{child, parent, app}},
		{"foundation", "Foundation", "sequential", []*model.Task{task("1")}},
		{"base", "Base", "sequential", []*model.Task{extra, base}},
	}}
	original, _ := json.Marshal(d)
	out, err := FilterDocument(d, "app")
	if err != nil {
		t.Fatal(err)
	}
	tags := []string{}
	for _, p := range out.Phases {
		tags = append(tags, p.Tag)
	}
	if !slices.Equal(tags, []string{"app", "base", "foundation", "tools"}) {
		t.Fatalf("bad closure: %v", tags)
	}
	if len(out.Phases[0].Tasks) != 3 || len(out.Phases[1].Tasks) != 2 {
		t.Fatal("dropped whole-phase tasks")
	}
	if _, err := ValidateDocument(out); err != nil {
		t.Fatal(err)
	}
	if _, err := setup(t).Import(context.Background(), out, ImportOptions{}); err != nil {
		t.Fatal(err)
	}
	if _, err := FilterDocument(d, "missing"); err == nil {
		t.Fatal("unknown phase accepted")
	}
	all, err := FilterDocument(d, "")
	if err != nil || len(all.Phases) != 5 {
		t.Fatalf("unfiltered export: %v", err)
	}
	unchanged, _ := json.Marshal(d)
	if string(original) != string(unchanged) {
		t.Fatal("export mutated caller")
	}
}
