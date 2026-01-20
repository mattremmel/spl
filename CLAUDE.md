# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

SPL (Simple Programming Language) is a Rust compiler for a custom programming language. Library crate, Rust 2024 edition.

## Build & Test

```bash
cargo test                     # Run all tests
cargo clippy --all-targets -- -D warnings  # Lint (treat warnings as errors)
cargo +nightly fmt             # Format (requires nightly)
```

## Architecture

*This is a new project. Update this section as components are implemented.*
