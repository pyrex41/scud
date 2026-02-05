# Backpressure Patterns by Language

Backpressure = automated rejection of invalid work. Configure for your stack.

## TypeScript/JavaScript

```json
// package.json scripts
{
  "scripts": {
    "build": "tsc --noEmit",
    "test": "vitest run",
    "lint": "eslint . --max-warnings 0",
    "typecheck": "tsc --noEmit",
    "check": "npm run typecheck && npm run lint && npm run test"
  }
}
```

**AGENTS.md excerpt:**
```markdown
## Validation Commands
- Build: `npm run build`
- Test: `npm run test`
- Lint: `npm run lint`
- Full check: `npm run check`

Run `npm run check` before committing. All must pass.
```

**Key backpressure points:**
- `tsc --noEmit` catches type errors without emitting
- `eslint --max-warnings 0` fails on any warning
- `vitest run` runs tests in CI mode (fails fast)

---

## Rust

```toml
# Cargo.toml
[lints.rust]
unsafe_code = "forbid"

[lints.clippy]
all = "deny"
pedantic = "warn"
```

**AGENTS.md excerpt:**
```markdown
## Validation Commands
- Build: `cargo build`
- Test: `cargo test`
- Lint: `cargo clippy -- -D warnings`
- Format check: `cargo fmt --check`
- Full check: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`

Run full check before committing. Clippy warnings are errors.
```

**Key backpressure points:**
- `cargo clippy -- -D warnings` treats warnings as errors
- `cargo fmt --check` fails if formatting differs
- Rust's type system catches many errors at compile time

---

## Python

```toml
# pyproject.toml
[tool.pytest.ini_options]
addopts = "--strict-markers --tb=short"

[tool.ruff]
select = ["E", "F", "I", "N", "W", "UP", "B", "C4", "SIM"]

[tool.mypy]
strict = true
```

**AGENTS.md excerpt:**
```markdown
## Validation Commands
- Test: `pytest`
- Lint: `ruff check .`
- Type check: `mypy .`
- Format check: `ruff format --check .`
- Full check: `ruff format --check . && ruff check . && mypy . && pytest`

Run full check before committing.
```

**Key backpressure points:**
- `mypy --strict` catches type errors
- `ruff check` fast linting with autofix capability
- `pytest --strict-markers` fails on unknown markers

---

## Go

```yaml
# .golangci.yml
linters:
  enable-all: true
  disable:
    - exhaustruct  # too noisy
linters-settings:
  govet:
    check-shadowing: true
```

**AGENTS.md excerpt:**
```markdown
## Validation Commands
- Build: `go build ./...`
- Test: `go test ./...`
- Lint: `golangci-lint run`
- Full check: `go build ./... && golangci-lint run && go test ./...`

Run full check before committing.
```

**Key backpressure points:**
- Go compiler catches most errors
- `golangci-lint` aggregates multiple linters
- `go test ./...` runs all tests

---

## Ruby

```ruby
# .rubocop.yml
AllCops:
  NewCops: enable
  SuggestExtensions: false

Style/FrozenStringLiteralComment:
  Enabled: true
  EnforcedStyle: always
```

**AGENTS.md excerpt:**
```markdown
## Validation Commands
- Test: `bundle exec rspec`
- Lint: `bundle exec rubocop`
- Type check: `bundle exec srb tc` (if using Sorbet)
- Full check: `bundle exec rubocop && bundle exec rspec`

Run full check before committing.
```

---

## General Principles

1. **Fail fast**: Configure tools to exit on first error
2. **No warnings**: Treat warnings as errors where possible
3. **Type safety**: Use strict type checking modes
4. **Consistent formatting**: Fail on format differences
5. **Comprehensive tests**: High coverage means better backpressure

## AGENTS.md Pattern

Always include a "Validation Commands" section:

```markdown
## Validation Commands
- Build: `<command>`
- Test: `<command>`
- Lint: `<command>`
- Full check: `<combined command>`

Run full check before committing. All must pass.
```

This gives Ralph clear, deterministic feedback.
