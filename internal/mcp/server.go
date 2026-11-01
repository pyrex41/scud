// Package mcp exposes the task API using stateless MCP 2026-07-28. Tool
// arguments remain raw JSON until strict typed decoding; no float64/map roundtrip.
package mcp

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"path/filepath"
	"reflect"
	"strings"
	"time"

	"github.com/modelcontextprotocol/go-sdk/jsonrpc"
	sdk "github.com/modelcontextprotocol/go-sdk/mcp"
	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/internal/sbintegration"
	"github.com/reuben/scud/internal/storage"
	"github.com/reuben/scud/internal/taskapi"
	"github.com/reuben/scud/pkg/generate"
	"github.com/reuben/scud/pkg/llm"
)

const ProtocolVersion = "2026-07-28"

type argumentSchema struct {
	Type        string   `json:"type"`
	Description string   `json:"description,omitempty"`
	Enum        []string `json:"enum,omitempty"`
}

type objectSchema struct {
	Type                 string                    `json:"type"`
	Properties           map[string]argumentSchema `json:"properties"`
	Required             []string                  `json:"required,omitempty"`
	AdditionalProperties bool                      `json:"additionalProperties"`
}

type definition struct {
	tool    *sdk.Tool
	handler sdk.ToolHandler
}

// NewServer supports only the current stateless protocol: server/discover and
// tools calls work immediately, with version/capabilities supplied per request.
func NewServer(version, selection, root string) (*sdk.Server, error) {
	names, err := selectedTools(selection)
	if err != nil {
		return nil, err
	}
	s := sdk.NewServer(&sdk.Implementation{Name: "scud", Version: version}, &sdk.ServerOptions{
		SupportedProtocolVersions: []string{ProtocolVersion},
	})
	// Do not leave a second, legacy handshake API enabled by the SDK's
	// backwards-compatibility machinery.
	s.AddReceivingMiddleware(func(next sdk.MethodHandler) sdk.MethodHandler {
		return func(ctx context.Context, method string, request sdk.Request) (sdk.Result, error) {
			if method == "initialize" || method == "notifications/initialized" {
				return nil, &jsonrpc.Error{Code: jsonrpc.CodeMethodNotFound, Message: "Use stateless MCP 2026-07-28 per-request metadata; initialization is not supported"}
			}
			if !strings.HasPrefix(method, "notifications/") {
				// The SDK retains legacy fallback state for other consumers.
				// Inspect THIS request's literal metadata, never ProtocolVersion()
				// or ClientCapabilities(), which can fall back to that state.
				params := request.GetParams()
				if params == nil || (reflect.ValueOf(params).Kind() == reflect.Pointer && reflect.ValueOf(params).IsNil()) {
					return nil, &jsonrpc.Error{Code: jsonrpc.CodeInvalidParams, Message: "Every request requires params._meta with protocol version and client capabilities"}
				}
				meta := params.GetMeta()
				version, versionOK := meta["io.modelcontextprotocol/protocolVersion"].(string)
				capabilities, capabilitiesOK := meta["io.modelcontextprotocol/clientCapabilities"].(map[string]any)
				if !versionOK || !capabilitiesOK || capabilities == nil {
					return nil, &jsonrpc.Error{Code: jsonrpc.CodeInvalidParams, Message: "Every request requires its own protocolVersion and object clientCapabilities metadata"}
				}
				if version != ProtocolVersion {
					data, _ := json.Marshal(struct {
						Supported []string `json:"supported"`
						Requested string   `json:"requested"`
					}{[]string{ProtocolVersion}, version})
					return nil, &jsonrpc.Error{Code: -32022, Message: "Unsupported protocol version", Data: data}
				}
			}
			return next(ctx, method, request)
		}
	})
	all := definitions(root)
	for _, name := range names {
		d := all[name]
		// The low-level method intentionally bypasses the SDK's map/default
		// normalization: the typed binder below is the argument validator.
		s.AddTool(d.tool, d.handler)
	}
	return s, nil
}

func selectedTools(selection string) ([]string, error) {
	core := []string{"tasks", "import", "update", "status", "parse"}
	if selection == "" || selection == "core" {
		return core, nil
	}
	if selection == "full" {
		return append(core, "sb"), nil
	}
	known := map[string]bool{"tasks": true, "import": true, "update": true, "status": true, "parse": true, "sb": true}
	names, seen := []string{}, map[string]bool{}
	for _, raw := range strings.Split(selection, ",") {
		name := strings.TrimPrefix(strings.TrimSpace(raw), "scud_")
		if !known[name] {
			return nil, fmt.Errorf("unknown SCUD_TOOLS entry %q; use core, full, or tasks,import,update,status,parse,sb", raw)
		}
		if !seen[name] {
			seen[name] = true
			names = append(names, name)
		}
	}
	return names, nil
}

type taskQuery struct {
	Tag   string `json:"tag,omitempty"`
	ID    string `json:"id,omitempty"`
	Ready bool   `json:"ready,omitempty"`
}
type importArgs struct {
	Document   taskapi.Document `json:"document"`
	Policy     string           `json:"policy,omitempty"`
	IfRevision string           `json:"if_revision,omitempty"`
}
type updateArgs struct {
	ID         string        `json:"id"`
	Tag        string        `json:"tag,omitempty"`
	Patch      taskapi.Patch `json:"patch"`
	IfRevision string        `json:"if_revision,omitempty"`
}
type parseArgs struct {
	Content string `json:"content"`
	Model   string `json:"model,omitempty"`
	Tag     string `json:"tag,omitempty"`
}
type sbArgs struct {
	Operation string `json:"operation"`
}

func definitions(root string) map[string]definition {
	str, obj := argumentSchema{Type: "string"}, argumentSchema{Type: "object"}
	service := func() (*taskapi.Service, error) { return taskapi.Open(root) }
	return map[string]definition{
		"tasks": bind("scud_tasks", "Read the task document, one task (id), or ready refs (ready=true). A tag-filtered document includes prerequisite phases so it remains importable.", true, false,
			map[string]argumentSchema{"tag": str, "id": str, "ready": {Type: "boolean"}}, nil,
			func(ctx context.Context, args taskQuery) (any, error) {
				if args.ID != "" && args.Ready {
					return nil, fmt.Errorf("id and ready are mutually exclusive")
				}
				s, err := service()
				if err != nil {
					return nil, err
				}
				if args.ID != "" {
					t, ref, err := s.ReadTask(ctx, taskapi.Ref{Tag: args.Tag, ID: args.ID})
					return struct {
						Ref  taskapi.Ref `json:"ref"`
						Task any         `json:"task"`
					}{ref, t}, err
				}
				snap, err := s.Snapshot(ctx)
				if err != nil {
					return nil, err
				}
				if args.Ready {
					refs := []taskapi.Ref{}
					for _, ref := range snap.Ready {
						if args.Tag == "" || ref.Tag == args.Tag {
							refs = append(refs, ref)
						}
					}
					return struct {
						Revision string        `json:"revision"`
						Ready    []taskapi.Ref `json:"ready"`
					}{snap.Revision, refs}, nil
				}
				return taskapi.FilterDocument(taskapi.Document{SchemaVersion: 1, Phases: snap.Phases}, args.Tag)
			}),
		"status": bind("scud_status", "Fresh task counts, revision, ready refs and dependency blockers. Workflow status is user-reported, not verification.", true, false,
			map[string]argumentSchema{}, nil, func(ctx context.Context, _ struct{}) (any, error) {
				s, err := service()
				if err != nil {
					return nil, err
				}
				return s.Snapshot(ctx)
			}),
		"import": bind("scud_import", "Atomically ingest an importable document; colliding phases require explicit merge or replace. Optional revision guard prevents lost updates.", false, false,
			map[string]argumentSchema{"document": obj, "policy": {Type: "string", Enum: []string{"merge", "replace"}}, "if_revision": str}, []string{"document"},
			func(ctx context.Context, args importArgs) (any, error) {
				s, err := service()
				if err != nil {
					return nil, err
				}
				return s.Import(ctx, args.Document, taskapi.ImportOptions{Policy: args.Policy, ExpectedRevision: args.IfRevision})
			}),
		"update": bind("scud_update", "Patch one task. Unspecified fields stay unchanged; status changes do not run workers or imply verification.", false, false,
			map[string]argumentSchema{"id": str, "tag": str, "patch": obj, "if_revision": str}, []string{"id", "patch"},
			func(ctx context.Context, args updateArgs) (any, error) {
				if strings.TrimSpace(args.ID) == "" {
					return nil, fmt.Errorf("a nonempty task id is required")
				}
				s, err := service()
				if err != nil {
					return nil, err
				}
				return s.Update(ctx, taskapi.Ref{Tag: args.Tag, ID: args.ID}, args.Patch, args.IfRevision)
			}),
		"parse": bind("scud_parse", "Propose an importable task document from PRD text using main or fast. Calls a model, never persists tasks or edits source.", true, true,
			map[string]argumentSchema{"content": str, "model": {Type: "string", Enum: []string{"main", "fast"}}, "tag": str}, []string{"content"},
			func(ctx context.Context, args parseArgs) (any, error) {
				name := args.Model
				if name == "" {
					name = "main"
				}
				project, err := storage.FindRoot(root)
				if err != nil {
					return nil, err
				}
				cfg, err := config.LoadModelDefaults(filepath.Join(project, ".scud"))
				if err != nil {
					return nil, err
				}
				target, err := cfg.ResolveModelTarget(name)
				if err != nil {
					return nil, err
				}
				caller, err := llm.NewTargetClient(target)
				if err != nil {
					return nil, err
				}
				return generate.Propose(ctx, caller, args.Content, generate.ProposalOptions{Tag: args.Tag})
			}),
		"sb": bind("scud_sb", "Optional public sb commands. check/gen/derive may edit generated files. Results never modify task status; report presence is not proof.", false, true,
			map[string]argumentSchema{"operation": {Type: "string", Enum: []string{"status", "check", "context", "derive", "audit", "gen"}}}, []string{"operation"},
			func(ctx context.Context, args sbArgs) (any, error) {
				project, err := sbintegration.FindRoot(root)
				if err != nil {
					return nil, err
				}
				client := sbintegration.Client{Root: project, Timeout: 5 * time.Minute}
				if args.Operation == "status" {
					return client.State()
				}
				r, err := client.Run(ctx, args.Operation)
				if err != nil {
					b, _ := json.Marshal(r)
					return nil, fmt.Errorf("%w\n%s", err, b)
				}
				return r, nil
			}),
	}
}

// bind retains raw lexemes until strict decoding into T. Schema checks also
// reject null/wrong primitive argument types rather than treating them as absent.
func bind[T any](name, description string, readOnly, openWorld bool, properties map[string]argumentSchema, required []string, run func(context.Context, T) (any, error)) definition {
	schema := objectSchema{Type: "object", Properties: properties, Required: required}
	return definition{
		tool: &sdk.Tool{Name: name, Description: description, InputSchema: schema,
			Annotations: &sdk.ToolAnnotations{ReadOnlyHint: readOnly, OpenWorldHint: new(openWorld), DestructiveHint: new(!readOnly)}},
		handler: func(ctx context.Context, req *sdk.CallToolRequest) (*sdk.CallToolResult, error) {
			raw := req.Params.Arguments
			if len(raw) == 0 {
				raw = json.RawMessage(`{}`)
			}
			args, err := decodeArguments[T](raw, schema)
			if err != nil {
				return nil, &jsonrpc.Error{Code: jsonrpc.CodeInvalidParams, Message: err.Error()}
			}
			value, err := run(ctx, args)
			if err != nil {
				return &sdk.CallToolResult{IsError: true, Content: []sdk.Content{&sdk.TextContent{Text: err.Error()}}}, nil
			}
			b, err := json.Marshal(value)
			if err != nil {
				return nil, err
			}
			return &sdk.CallToolResult{Content: []sdk.Content{&sdk.TextContent{Text: string(b)}}, StructuredContent: value}, nil
		},
	}
}

func decodeArguments[T any](raw json.RawMessage, schema objectSchema) (T, error) {
	var zero T
	fields, err := taskapi.Decode[map[string]json.RawMessage](bytes.NewReader(raw))
	if err != nil {
		return zero, err
	}
	if fields == nil {
		return zero, fmt.Errorf("arguments must be an object")
	}
	for _, key := range schema.Required {
		if _, exists := fields[key]; !exists {
			return zero, fmt.Errorf("%s is required", key)
		}
	}
	for key, value := range fields {
		property, exists := schema.Properties[key]
		if !exists {
			return zero, fmt.Errorf("unknown argument %q", key)
		}
		v := bytes.TrimSpace(value)
		valid := len(v) != 0
		if valid {
			switch property.Type {
			case "string":
				valid = v[0] == '"'
			case "object":
				valid = v[0] == '{'
			case "boolean":
				valid = bytes.Equal(v, []byte("true")) || bytes.Equal(v, []byte("false"))
			}
		}
		if !valid {
			return zero, fmt.Errorf("%s must be a %s", key, property.Type)
		}
		if len(property.Enum) != 0 {
			var text string
			_ = json.Unmarshal(value, &text)
			match := false
			for _, allowed := range property.Enum {
				if text == allowed {
					match = true
					break
				}
			}
			if !match {
				return zero, fmt.Errorf("%s must be one of %s", key, strings.Join(property.Enum, ", "))
			}
		}
	}
	return taskapi.Decode[T](bytes.NewReader(raw))
}
