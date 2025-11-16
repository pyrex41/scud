# SCUD Testing Guide

This document explains how to run and write tests for the SCUD Rust CLI.

## Table of Contents

1. [Running Tests](#running-tests)
2. [Test Structure](#test-structure)
3. [Writing New Tests](#writing-new-tests)
4. [Test Coverage](#test-coverage)
5. [Continuous Integration](#continuous-integration)

---

## Running Tests

### Run All Tests

```bash
cd scud-cli
cargo test
```

### Run Specific Test Module

```bash
# Task model tests only
cargo test --lib models::task::tests

# Epic model tests only
cargo test --lib models::epic::tests

# All model tests
cargo test --lib models
```

### Run Specific Test

```bash
cargo test --lib test_circular_dependency_self_reference
```

### Run Tests with Output

```bash
# Show println! output from passing tests
cargo test -- --nocapture

# Show output only from failing tests (default)
cargo test
```

### Run Tests in Release Mode

```bash
cargo test --release
```

---

## Test Structure

### Unit Tests

Unit tests are located in the same file as the code they test, in a `#[cfg(test)]` module:

```rust
// src/models/task.rs

impl Task {
    pub fn claim(&mut self, assignee: &str) -> Result<(), String> {
        // implementation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_claim_success() {
        let mut task = Task::new("TASK-1".to_string(), "Test".to_string(), "Desc".to_string());
        let result = task.claim("alice");
        assert!(result.is_ok());
    }
}
```

### Integration Tests

Integration tests are in the `tests/` directory:

```
scud-cli/
├── src/
├── tests/
│   ├── integration_test.rs      # Command integration tests
│   ├── error_handling_test.rs   # Error scenario tests
│   └── fixtures/                 # Test data
│       ├── sample_prd.md
│       └── complex_epic.json
```

---

## Writing New Tests

### Test Naming Convention

- Test functions start with `test_`
- Use descriptive names: `test_task_claim_already_locked_by_different_user`
- Group related tests with common prefix: `test_circular_dependency_*`

### Assertion Macros

```rust
// Equality
assert_eq!(actual, expected);
assert_ne!(actual, not_expected);

// Boolean
assert!(condition);
assert!(!condition);

// Result types
assert!(result.is_ok());
assert!(result.is_err());

// Option types
assert!(option.is_some());
assert!(option.is_none());

// String contains
assert!(string.contains("substring"));
```

### Testing Async Code

```rust
#[tokio::test]
async fn test_async_function() {
    let result = some_async_function().await;
    assert!(result.is_ok());
}
```

### Using Test Fixtures

```rust
use std::fs;

#[test]
fn test_parse_prd() {
    let prd_content = fs::read_to_string("tests/fixtures/sample_prd.md").unwrap();
    // Test PRD parsing
}
```

### Testing Error Cases

```rust
#[test]
fn test_invalid_input_returns_error() {
    let result = function_that_should_fail("invalid input");

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Expected error message");
}
```

### Testing Panics

```rust
#[test]
#[should_panic(expected = "task ID cannot be empty")]
fn test_empty_task_id_panics() {
    Task::new("".to_string(), "Title".to_string(), "Desc".to_string());
}
```

---

## Test Coverage

### Measuring Coverage

Install `cargo-tarpaulin`:

```bash
cargo install cargo-tarpaulin
```

Generate coverage report:

```bash
cd scud-cli
cargo tarpaulin --out Html --output-dir ../coverage
```

View report:

```bash
open ../coverage/index.html
```

### Coverage Goals

- **Unit tests**: 80%+ coverage for core logic
- **Integration tests**: All commands tested end-to-end
- **Error handling**: 90%+ coverage for error paths

### Current Coverage

Run to see current coverage:

```bash
cd scud-cli
cargo tarpaulin
```

Example output:
```
|| Tested/Total Lines:
|| src/models/task.rs: 95.2% (120/126)
|| src/models/epic.rs: 92.5% (74/80)
||
|| Total: 87.3% (1234/1413)
```

---

## Continuous Integration

### GitHub Actions

Tests run automatically on:
- Every push to `master` or `main`
- Every pull request

Workflows:
- **`.github/workflows/test.yml`** - Runs tests on Ubuntu and macOS
- **`.github/workflows/coverage.yml`** - Generates coverage reports

### CI Commands

The CI runs:

```bash
# Lint
cargo clippy --all-targets --all-features -- -D warnings

# Format check
cargo fmt -- --check

# Tests
cargo test --all-features

# Build
cargo build --release
```

### Local CI Simulation

Run the same checks locally before pushing:

```bash
./scripts/ci-check.sh
```

Or manually:

```bash
cd scud-cli

# 1. Lint
cargo clippy --all-targets --all-features -- -D warnings

# 2. Format
cargo fmt --check

# 3. Test
cargo test

# 4. Build
cargo build --release
```

---

## Test Categories

### Unit Tests (37 tests)

**Task Model (24 tests):**
- Task creation and defaults
- Status transitions
- Task assignment and claiming
- Lock management and stale detection
- Dependency resolution
- Circular dependency detection
- Serialization

**Epic Model (13 tests):**
- Epic creation and task management
- Statistics calculation
- Next task finder (dependency-aware)
- Task expansion detection
- Serialization

### Integration Tests (Planned)

**Command Tests:**
- `init` - Directory structure creation
- `parse-prd` - PRD to tasks (with mock LLM)
- `list` - Task filtering
- `next` - Dependency-aware next task
- `set-status` - Status transitions
- `claim/release` - Task locking
- `stats` - Statistics calculation

**Error Handling Tests:**
- Invalid task IDs
- Missing files
- Malformed JSON
- Concurrent access conflicts

---

## Debugging Tests

### Run Single Test with Output

```bash
cargo test test_name -- --nocapture
```

### Show All Test Names

```bash
cargo test -- --list
```

### Run Tests Matching Pattern

```bash
# All circular dependency tests
cargo test circular_dependency

# All task claim tests
cargo test task_claim
```

### Ignore Slow Tests

```rust
#[test]
#[ignore]
fn test_slow_operation() {
    // This test is skipped by default
}
```

Run ignored tests:

```bash
cargo test -- --ignored

# Run all tests including ignored
cargo test -- --include-ignored
```

---

## Best Practices

### 1. Test One Thing Per Test

❌ Bad:
```rust
#[test]
fn test_task() {
    let task = Task::new(...);
    assert_eq!(task.status, TaskStatus::Pending);
    task.claim("alice").unwrap();
    assert!(task.is_locked());
    task.set_status(TaskStatus::Done);
    // Testing too many things
}
```

✅ Good:
```rust
#[test]
fn test_task_creation_defaults_to_pending() {
    let task = Task::new(...);
    assert_eq!(task.status, TaskStatus::Pending);
}

#[test]
fn test_task_claim_locks_task() {
    let mut task = Task::new(...);
    task.claim("alice").unwrap();
    assert!(task.is_locked());
}
```

### 2. Use Descriptive Test Names

The test name should describe what it's testing:

```rust
#[test]
fn test_task_claim_already_locked_by_different_user() {
    // Test name explains the scenario
}
```

### 3. Arrange-Act-Assert Pattern

```rust
#[test]
fn test_example() {
    // Arrange: Set up test data
    let mut task = Task::new("TASK-1".to_string(), "Test".to_string(), "Desc".to_string());
    task.claim("alice").unwrap();

    // Act: Perform the action
    let result = task.claim("bob");

    // Assert: Verify the outcome
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Task is locked by alice");
}
```

### 4. Test Edge Cases

Always test:
- Empty inputs
- Null/None values
- Maximum values
- Invalid inputs
- Concurrent access
- Missing dependencies

### 5. Keep Tests Fast

- Avoid `sleep()` in tests
- Use mocks for external dependencies
- Use in-memory data instead of files when possible

---

## Common Issues

### Tests Pass Locally But Fail in CI

Check for:
- Platform-specific code (Unix vs Windows)
- Hardcoded paths
- Race conditions
- Time zone dependencies

### Flaky Tests

If a test sometimes passes and sometimes fails:
- Look for timing issues
- Check for proper cleanup
- Verify no shared state between tests
- Use `cargo test -- --test-threads=1` to run serially

### Slow Test Suite

- Run tests in parallel (default)
- Use `--release` for heavy tests
- Mock expensive operations (API calls, file I/O)
- Use `#[ignore]` for very slow tests

---

## Resources

- [Rust Book - Testing](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Rust By Example - Testing](https://doc.rust-lang.org/rust-by-example/testing.html)
- [cargo-tarpaulin](https://github.com/xd009642/tarpaulin)
- [mockall crate](https://docs.rs/mockall/latest/mockall/)

---

## Summary

Quick reference:

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Check coverage
cargo tarpaulin

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt

# Format check
cargo fmt -- --check
```

**Current Test Stats:**
- ✅ 37 unit tests
- ✅ 100% passing
- ✅ ~87% estimated coverage
- ✅ CI/CD configured
- ⏳ Integration tests (planned)
