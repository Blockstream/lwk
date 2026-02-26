# AGENTS.md - Guidelines for Coding Agents

This file contains guidelines for agentic coding agents working in the LWK (Liquid Wallet Kit) repository.

## Instruction Priority and Conflict Resolution

When multiple instructions apply, follow this priority order:
1. Safety, security, and data integrity
2. Correctness and testability
3. Repository conventions (style, structure, commit format)
4. Performance and speed of execution

If instructions conflict, follow the higher-priority rule and document the tradeoff briefly in your final message.

## Project Overview

LWK is a Rust workspace containing libraries for Liquid wallets. It consists of multiple crates:
- `lwk_wollet` - Watch-only wallets based on CT descriptors
- `lwk_signer` - Signing operations
- `lwk_jade` / `lwk_ledger` - Hardware wallet integrations
- `lwk_bindings` - UniFFI bindings for Python/Kotlin/Swift/C#
- `lwk_common` - Shared utilities
- `lwk_cli` - Command line interface
- `lwk_wasm` - WebAssembly bindings

## Build Commands

```bash
# Build the entire workspace
cargo build

# Build specific crate
cargo build -p lwk_wollet

# Cross-compile for WASM
cargo check --target wasm32-unknown-unknown -p lwk_wollet
```

## Test Commands


```bash

# Run all tests, including integration tests that spawn executables and Docker containers
cargo test

# Run all unit tests, much faster; every unit test should run in less than a second
cargo test --lib

# Run tests for specific package
cargo test -p lwk_wollet

# Run a single test
cargo test -p lwk_wollet test_name_here

# Run bindings tests
cargo test -p lwk_bindings --features foreign_bindings

# Build tests without running
cargo test --no-run
```

## Lint Commands

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy --all-features --all-targets -- -D warnings

# Security audit
cargo audit --deny yanked

# Generate documentation
cargo doc --no-deps -p lwk_wollet --all-features
RUSTDOCFLAGS="-D warnings --cfg docsrs" cargo +nightly doc --all-features --no-deps
```

## Just Commands

The project uses `just` for complex workflows:

```bash
just --list                           # List all recipes
just build-bindings-lib              # Build liblwk.so
just python-test-bindings            # Test Python bindings
just swift                           # Build Swift framework (macOS only)
just android                         # Build Android libs
just kotlin                          # Generate Kotlin bindings
just mdbook                          # Build documentation
```

## Code Style Guidelines

### Rust Version
- Rust toolchain: **1.85.0** (specified in `rust-toolchain.toml`)
- Edition: **2021**

### Imports
- Group imports: std lib → external crates → workspace crates → local modules
- Use `use crate::` for local module imports
- Re-export commonly used types at crate root (see `lwk_wollet/src/lib.rs`)

### Error Handling
- Use `thiserror` for error enums with `#[derive(thiserror::Error, Debug)]`
- Use `#[error(transparent)]` for wrapping other errors
- Use `#[from]` for automatic conversion
- Return `Result<T, Error>` from public functions
- Deny `unwrap()` in non-test code: `#![cfg_attr(not(test), deny(clippy::unwrap_used))]`

### Naming Conventions
- Types: `PascalCase` (e.g., `WolletDescriptor`, `TxBuilder`)
- Functions/Methods: `snake_case` (e.g., `full_scan_with_electrum_client`)
- Constants: `SCREAMING_SNAKE_CASE` (e.g., `LIQUID_SOCKET`)
- Feature flags: `snake_case` (e.g., `foreign_bindings`, `test_emulator`)

### Documentation
- Require docs on public items: `#![warn(missing_docs)]`
- Use `//!` for module-level documentation
- Document all public functions with examples where appropriate
- Mark feature-gated items: `#[cfg_attr(docsrs, feature(doc_cfg))]`

### Type Patterns
- Use `pub type` aliases for common types (e.g., `pub type BlindingPublicKey = elements::secp256k1_zkp::PublicKey`)
- Prefer `&str` over `&String` in function parameters
- Use builder patterns for complex construction (e.g., `TxBuilder`, `WolletBuilder`)

### Commit Messages
Format: `context: <description>`
- Context: crate/directory name or `ci`/`fix`/`feat`/`docs`/`refactor`
- Breaking changes: append `!` after context
- Title: max 50 chars, imperative mood, no period
- Body: blank line after title, explain "why", use bullet points

### Feature Flags
- Default features should be minimal but functional
- Group related features logically
- Use `#[cfg(feature = "...")]` for conditional compilation
- Document feature requirements in crate README

### Testing
- Unit tests in `#[cfg(test)]` modules
- Integration tests in `tests/` directory
- Use `lwk_test_util` for shared test utilities
- E2E tests require Docker environment (Jade emulator, etc.)
- Possible panics, unchecked index array access, unwraps, are allowed in testing code.
- Do not suggest safety improvements (e.g., bounds checks, error handling) for these in tests unless they cause compile/run failures.

## Additional Resources
- CI Configuration: `.gitlab-ci.yml` (GitLab) and `.github/workflows/` (GitHub)
- Nix development: `nix develop .` for reproducible environment
