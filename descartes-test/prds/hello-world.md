# Hello World CLI App

## Overview
Build a simple Rust CLI application that greets users by name and supports a few options.

## Requirements

### 1. Project Setup
- Initialize a new Rust project with `cargo init`
- Add `clap` as a dependency for argument parsing

### 2. Basic Greeting
- Accept a `--name` flag (default: "World")
- Print `Hello, <name>!` to stdout

### 3. Greeting Style Options
- Add a `--shout` flag that uppercases the greeting
- Add a `--count` flag (default: 1) that repeats the greeting N times

### 4. Unit Tests
- Test default greeting outputs "Hello, World!"
- Test custom name greeting
- Test shout mode uppercases output
- Test count repeats the greeting

### 5. README
- Add a README.md with usage examples
