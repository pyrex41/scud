package generate

import (
	"bytes"
	"context"
	"encoding/json"
	"github.com/reuben/scud/internal/taskapi"
	"strings"
	"testing"
)

type proposalCaller string

func (p proposalCaller) CompleteTargetJSON(_ context.Context, prompt, _ string, out any) error {
	if !strings.Contains(prompt, "observable verification") {
		panic("prompt missing goal/verification guidance")
	}
	return json.Unmarshal([]byte(p), out)
}
func TestProposeRequiresCaller(t *testing.T) {
	if _, err := Propose(context.Background(), nil, "PRD", ProposalOptions{}); err == nil {
		t.Fatal("expected missing caller error")
	}
}

const validProposalJSON = `{"tasks":[{"title":"One","description":"Verify one","priority":"medium","complexity":3,"dependencies":[]},{"title":"Two","description":"Verify two","priority":"high","complexity":2,"dependencies":["1"]}]}`

func TestProposeUsesSharedDocumentValidation(t *testing.T) {
	p, e := Propose(context.Background(), proposalCaller(validProposalJSON), "Requirements", ProposalOptions{})
	if e != nil {
		t.Fatal(e)
	}
	if p.SchemaVersion != 1 || p.Phases[0].Tag != "main" {
		t.Fatalf("unexpected proposal %#v", p)
	}
	b, err := json.Marshal(p)
	if err != nil {
		t.Fatal(err)
	}
	var doc taskapi.Document
	if err = taskapi.DecodeJSON(bytes.NewReader(b), &doc); err != nil {
		t.Fatal(err)
	}
	if _, err = taskapi.ValidateDocument(doc); err != nil {
		t.Fatalf("proposal JSON does not roundtrip through taskapi: %v", err)
	}
	if _, e = Propose(context.Background(), proposalCaller(strings.Replace(validProposalJSON, `"dependencies":["1"]`, `"dependencies":["1","1"]`, 1)), "Requirements", ProposalOptions{}); e == nil {
		t.Fatal("duplicate dependency accepted")
	}
	cycle := strings.Replace(validProposalJSON, `"dependencies":[]`, `"dependencies":["2"]`, 1)
	if _, e = Propose(context.Background(), proposalCaller(cycle), "Requirements", ProposalOptions{}); e == nil {
		t.Fatal("cycle accepted")
	}
}
func TestProposeRejectsInvalidInputBeforeCalling(t *testing.T) {
	caller := proposalCaller(validProposalJSON)
	for _, tc := range []struct {
		content string
		opts    ProposalOptions
	}{{"  ", ProposalOptions{}}, {"x", ProposalOptions{NumTasks: -1}}, {"x", ProposalOptions{Tag: "unsafe/tag"}}, {strings.Repeat("x", (4<<20)+1), ProposalOptions{}}} {
		if _, e := Propose(context.Background(), caller, tc.content, tc.opts); e == nil {
			t.Fatalf("accepted invalid input: %#v", tc)
		}
	}
}
