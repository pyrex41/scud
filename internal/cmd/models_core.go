package cmd

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"github.com/reuben/scud/internal/config"
	"github.com/reuben/scud/pkg/llm"
	"github.com/spf13/cobra"
	"os"
	"sort"
	"strings"
	"time"
)

type modelCatalogFunc func(context.Context, string) ([]llm.CatalogModel, error)
type modelConfigFunc func() (*config.Config, string, error)

// NewModelsCmd creates model discovery/default commands with production dependencies.
func NewModelsCmd() *cobra.Command {
	return newModelsCmd(llm.ListModels, func() (*config.Config, string, error) {
		s, e := getStore()
		if e != nil {
			return nil, "", e
		}
		c, e := config.LoadModelDefaults(s.ScudDir())
		return c, s.ScudDir(), e
	})
}

// Explicit typed ports keep tests injectable without an any/type-switch API.
func newModelsCmd(catalog modelCatalogFunc, load modelConfigFunc) *cobra.Command {
	root := &cobra.Command{Use: "models", Short: "Discover and choose main and fast models", RunE: func(c *cobra.Command, _ []string) error {
		cfg, _, e := load()
		if e != nil {
			return e
		}
		for _, a := range []string{"main", "fast"} {
			t, e := cfg.ResolveModelTarget(a)
			if e != nil {
				fmt.Fprintf(c.OutOrStdout(), "%s: not set (%v)\n", a, e)
			} else {
				fmt.Fprintf(c.OutOrStdout(), "%s: %s — %s\n", a, t.Provider, t.Model)
			}
		}
		return nil
	}}
	root.Long = `Discover live provider catalogs and configure only the project-local main
and fast defaults. With no subcommand, show both defaults as text. No model or
provider is guessed for a new project, and credentials are not read from apps.
Discovery uses OPENAI_API_KEY, XAI_API_KEY, ANTHROPIC_API_KEY and OPENROUTER_API_KEY.
Use list to discover models, set to choose a unique match, defaults to inspect.
Catalog availability does not guarantee text-generation compatibility.`
	root.Example = `  scud models
  scud models list --json
  scud models defaults --json`
	listProvider, search := "", ""
	var asJSON bool
	list := &cobra.Command{Use: "list", Short: "List models available from authenticated providers", Args: cobra.NoArgs, RunE: func(c *cobra.Command, _ []string) error {
		ctx, cancel := context.WithTimeout(c.Context(), 20*time.Second)
		defer cancel()
		providers := []string{"openai", "xai", "anthropic", "openrouter"}
		warnings := []string{}
		if listProvider != "" {
			if _, ok := llm.CatalogCredentialEnv(listProvider); !ok {
				return fmt.Errorf("unknown provider %q (supported: %s)", listProvider, strings.Join(llm.CatalogProviders(), ", "))
			}
			providers = []string{listProvider}
		} else {
			providers = providers[:0]
			for _, p := range llm.CatalogProviders() {
				env, _ := llm.CatalogCredentialEnv(p)
				if os.Getenv(env) != "" {
					providers = append(providers, p)
				} else {
					warnings = append(warnings, p+": skipped ("+env+" not set)")
				}
			}
		}
		rows := []llm.CatalogModel{}
		for _, p := range providers {
			if e := ctx.Err(); e != nil {
				return e
			}
			ms, e := catalog(ctx, p)
			if err := catalogCancellation(ctx, e); err != nil {
				return err
			}
			if e != nil {
				warnings = append(warnings, p+": "+e.Error())
				continue
			}
			rows = append(rows, ms...)
		}
		if e := ctx.Err(); e != nil {
			return e
		}
		if len(rows) == 0 {
			return fmt.Errorf("no provider catalogs available: %s", strings.Join(warnings, "; "))
		}
		if search != "" {
			filtered := rows[:0]
			for _, m := range rows {
				hay := strings.ToLower(m.Provider + " " + m.ID + " " + m.DisplayName)
				ok := true
				for _, w := range strings.Fields(strings.ToLower(search)) {
					if !strings.Contains(hay, w) {
						ok = false
						break
					}
				}
				if ok {
					filtered = append(filtered, m)
				}
			}
			rows = filtered
		}
		sort.Slice(rows, func(i, j int) bool {
			if rows[i].Provider != rows[j].Provider {
				return rows[i].Provider < rows[j].Provider
			}
			return rows[i].ID < rows[j].ID
		})
		cfgForMark, _, _ := load()
		if asJSON {
			enc := json.NewEncoder(c.OutOrStdout())
			selected := map[string][]string{}
			if cfg := cfgForMark; cfg != nil {
				for _, a := range []string{"main", "fast"} {
					if t, e := cfg.ResolveModelTarget(a); e == nil {
						key := t.Provider + "\x00" + t.Model
						selected[key] = append(selected[key], a)
					}
				}
			}
			type row struct {
				llm.CatalogModel
				Defaults []string `json:"defaults,omitempty"`
			}
			out := make([]row, 0, len(rows))
			for _, m := range rows {
				out = append(out, row{m, selected[m.Provider+"\x00"+m.ID]})
			}
			return enc.Encode(struct {
				Models   []row    `json:"models"`
				Warnings []string `json:"warnings,omitempty"`
			}{out, warnings})
		}
		for _, m := range rows {
			mark := ""
			if cfg := cfgForMark; cfg != nil {
				for _, a := range []string{"main", "fast"} {
					if t, e := cfg.ResolveModelTarget(a); e == nil && t.Provider == m.Provider && t.Model == m.ID {
						mark += " " + a
					}
				}
			}
			fmt.Fprintf(c.OutOrStdout(), "%-12s %-30s %-6s %s\n", m.Provider, m.DisplayName, strings.TrimSpace(mark), m.ID)
		}
		for _, w := range warnings {
			fmt.Fprintf(c.ErrOrStderr(), "warning: %s\n", w)
		}
		return nil
	}}
	list.Long = `Query live catalogs for providers whose API-key environment variables are set,
or restrict discovery with --provider openai|xai|anthropic|openrouter. Missing
credentials are skipped; provider failures are reported as warnings if another
catalog succeeds. No available catalog is an error. Discovery needs no project.
--search matches every case-insensitive word against provider, model ID and
display name. Text is the default; --json returns models and optional warnings.
Configured main/fast selections are marked when project settings are available.`
	list.Example = `  scud models list
  scud models list --provider anthropic --search sonnet --json`
	list.Flags().StringVar(&listProvider, "provider", "", "Only query this provider")
	list.Flags().StringVar(&search, "search", "", "Case-insensitive search words")
	list.Flags().BoolVar(&asJSON, "json", false, "Emit JSON")
	defaults := &cobra.Command{Use: "defaults", Short: "Show the configured main and fast model defaults", Args: cobra.NoArgs, RunE: func(c *cobra.Command, _ []string) error {
		cfg, _, e := load()
		if e != nil {
			return e
		}
		out := map[string]any{}
		for _, a := range []string{"main", "fast"} {
			t, e := cfg.ResolveModelTarget(a)
			if e == nil {
				out[a] = map[string]string{"provider": t.Provider, "model": t.Model}
			} else {
				out[a] = nil
			}
		}
		if out["main"] == nil || out["fast"] == nil {
			out["setup"] = "Run `scud models list`, then `scud models set main QUERY` and/or `scud models set fast QUERY`. Set provider API-key environment variables to enable catalog discovery."
		}
		if asJSON {
			return json.NewEncoder(c.OutOrStdout()).Encode(out)
		}
		for _, a := range []string{"main", "fast"} {
			t, e := cfg.ResolveModelTarget(a)
			if e != nil {
				fmt.Fprintf(c.OutOrStdout(), "%s: not set\n", a)
			} else {
				fmt.Fprintf(c.OutOrStdout(), "%s: %s — %s\n", a, t.Provider, t.Model)
			}
		}
		return nil
	}}
	defaults.Long = `Show the configured main and fast defaults without calling provider APIs.
Unset defaults remain unset: --json represents them as null and includes setup
guidance. No model target is guessed. Run inside an initialized project.`
	defaults.Example = `  scud models defaults
  scud models defaults --json`
	defaults.Flags().BoolVar(&asJSON, "json", false, "Emit JSON")
	set := &cobra.Command{Use: "set main|fast QUERY...", Short: "Set a default by matching a live model catalog", Args: cobra.MinimumNArgs(2), RunE: func(c *cobra.Command, args []string) error {
		alias := strings.ToLower(args[0])
		if alias != "main" && alias != "fast" {
			return fmt.Errorf("default must be main or fast")
		}
		query := strings.Join(args[1:], " ")
		if strings.TrimSpace(query) == "" {
			return fmt.Errorf("model query cannot be empty")
		}
		warnings := []string{}
		ctx, cancel := context.WithTimeout(c.Context(), 20*time.Second)
		defer cancel()
		providers := []string{"openai", "xai", "anthropic", "openrouter"}
		if listProvider != "" {
			if _, ok := llm.CatalogCredentialEnv(listProvider); !ok {
				return fmt.Errorf("unknown provider %q (supported: %s)", listProvider, strings.Join(llm.CatalogProviders(), ", "))
			}
			providers = []string{listProvider}
		} else {
			providers = providers[:0]
			for _, p := range llm.CatalogProviders() {
				env, _ := llm.CatalogCredentialEnv(p)
				if os.Getenv(env) != "" {
					providers = append(providers, p)
				} else {
					warnings = append(warnings, p+": skipped ("+env+" not set)")
				}
			}
		}
		var matches []llm.CatalogModel
		var failures []string
		for _, p := range providers {
			if e := ctx.Err(); e != nil {
				return e
			}
			ms, e := catalog(ctx, p)
			if err := catalogCancellation(ctx, e); err != nil {
				return err
			}
			if e != nil {
				failures = append(failures, p+": "+e.Error())
				continue
			}
			for _, m := range ms {
				if matchWords(query, m) {
					matches = append(matches, m)
				}
			}
		}
		if e := ctx.Err(); e != nil {
			return e
		}
		if len(matches) == 0 {
			return fmt.Errorf("no model matched %q (catalog errors: %s; skipped providers: %s)", query, strings.Join(failures, "; "), strings.Join(warnings, "; "))
		}
		if len(failures) > 0 {
			return fmt.Errorf("some authenticated provider catalogs failed; refusing an incomplete match: %s (retry or specify --provider)", strings.Join(failures, "; "))
		}
		exact := matches[:0]
		for _, m := range matches {
			if strings.EqualFold(strings.TrimSpace(query), m.ID) {
				exact = append(exact, m)
			}
		}
		if len(exact) > 0 {
			matches = exact
		}
		if len(matches) > 1 {
			var choices []string
			for _, m := range matches {
				choices = append(choices, fmt.Sprintf("%s: %s (%s)", m.Provider, m.DisplayName, m.ID))
			}
			sort.Strings(choices)
			return fmt.Errorf("model query %q is ambiguous; choose one: %s", query, strings.Join(choices, "; "))
		}
		cfg, dir, e := load()
		if e != nil {
			return e
		}
		_ = cfg
		if e = config.SetModelTargetContext(ctx, dir, alias, config.ModelTarget{Provider: matches[0].Provider, Model: matches[0].ID}); e != nil {
			return e
		}
		if asJSON {
			return json.NewEncoder(c.OutOrStdout()).Encode(struct {
				Model    llm.CatalogModel `json:"model"`
				Warnings []string         `json:"warnings,omitempty"`
			}{matches[0], warnings})
		}
		for _, w := range warnings {
			fmt.Fprintf(c.ErrOrStderr(), "warning: %s\n", w)
		}
		fmt.Fprintf(c.OutOrStdout(), "%s default set to %s (%s)\n", alias, matches[0].DisplayName, matches[0].ID)
		return nil
	}}
	set.Long = `Set only main or fast by searching live authenticated provider catalogs.
QUERY words match case-insensitively across provider, model ID and display name;
an exact model ID is preferred. Ambiguous queries show choices and write nothing.
Narrow the query or use --provider; targets are never guessed. If an authenticated
catalog fails, refuse an incomplete match (retry or restrict the provider).
Initialize the project first. Saves the resolved provider/model pair while
preserving unrelated settings. main and fast may use the same model.
Credentials come only from the supported provider API-key environment variables.`
	set.Example = `  scud models list --provider anthropic --search sonnet
  scud models set main sonnet --provider anthropic
  scud models set fast haiku --provider anthropic --json`
	set.Flags().StringVar(&listProvider, "provider", "", "Limit lookup to a provider")
	set.Flags().BoolVar(&asJSON, "json", false, "Emit JSON")
	root.AddCommand(list, defaults, set)
	return root
}
func catalogCancellation(ctx context.Context, err error) error {
	if e := ctx.Err(); e != nil {
		return e
	}
	if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
		return err
	}
	return nil
}
func matchWords(q string, m llm.CatalogModel) bool {
	hay := strings.ToLower(m.Provider + " " + m.ID + " " + m.DisplayName)
	for _, w := range strings.Fields(strings.ToLower(q)) {
		if !strings.Contains(hay, w) {
			return false
		}
	}
	return true
}
