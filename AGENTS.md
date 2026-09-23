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
* Main crates:
  - `lwk_wollet` - Watch-only wallets based on CT descriptors
  - `lwk_signer` - Signing operations
  - `lwk_common` - Shared utilities
* Specific actions:
  - `lwk_registry` - Asset registry integration
  - `lwk_payment_instructions` - Parser for addresses, invoices and other payment instructions
  - `lwk_prices` - Exchange rates between currencies
* Cross chain swaps:
  - `lwk_boltz` - Boltz integration
  - `lwk_anyswap` - Anyswap integration. Work in progress.
* Hardware wallets integration:
  - `lwk_jade` - Jade integration.
  - `lwk_ledger` - Ledger integration. Work in progress.
  - `lwk_hwi` - Hardware wallet interface. Currently unused.
* Build bindings for other languages:
  - `lwk_bindings` - UniFFI bindings for Python/Kotlin/Swift/C#
  - `lwk_wasm` - WebAssembly bindings
* CLI:
  - `lwk_tiny_jrpc` - Tiny JSON-RPC server
  - `lwk_rpc_model` - Data model for the RPC server
  - `lwk_app` - RPC server app
  - `lwk_cli` - Command line interface
* Experimental:
  - `lwk_simplicity` - Tools for working with the Simplicity language. Highly experimental and NOT production-ready.
* Test utilities:
  - `lwk_containers` - Docker containers for the test environment
  - `lwk_test_util` - Shared test utilities
  - `amp2_mock` - AMP2 mock server for testing

If you are doing changes in a crate, you must read the crate README.md.

## Build Commands

```bash
# Build the entire workspace
cargo -q build

# Build specific crate
cargo -q build -p lwk_wollet

# Cross-compile for WASM
cargo -q check --target wasm32-unknown-unknown -p lwk_wollet
```

## Test Commands

```bash
# Run all tests, including integration tests that spawn executables and Docker containers
cargo -q test

# Run all unit tests, much faster; every unit test should run in less than a second
cargo -q test --lib

# Run tests for specific package
cargo -q test -p lwk_wollet

# Run a single test
cargo -q test -p lwk_wollet test_name_here

# Run bindings tests
cargo -q test -p lwk_bindings --features foreign_bindings,test_env

# Build tests without running
cargo -q test --no-run
```

## Lint Commands

```bash
# Format code
cargo -q fmt

# Check formatting
cargo -q fmt --check

# Run clippy
cargo -q clippy --all-features --all-targets -- -D warnings

# Security audit (yanked dependencies are temporarily allowed until boltz-rust is updated)
cargo audit

# Generate documentation
cargo -q doc --no-deps -p lwk_wollet --all-features
RUSTDOCFLAGS="-D warnings --cfg docsrs" cargo +nightly -q doc --all-features --no-deps
```

### YAML Validation
Use `yq` to validate YAML files:
`yq '.' .gitlab-ci.yml > /dev/null && echo "ok"`
or python:
`python3 -c 'import yaml; yaml.safe_load(open(".gitlab-ci.yml")); print("ok")'`

## Just Commands

The project uses `just` for complex workflows, some examples include:

```bash
just --list                           # List all recipes
just build-bindings-lib              # Build liblwk.so
just python-test-bindings            # Test Python bindings
just swift                           # Build Swift framework (macOS only)
just android                         # Build Android libs
just kotlin                          # Generate Kotlin bindings
just mdbook                          # Build documentation
```

Add a `just` recipe when adding a complex flow.

## Code Style Guidelines

### Rust Version
- Rust toolchain: **1.91.0** (specified in `rust-toolchain.toml`)
- Edition: **2021**

### Dependencies
- Remove unused dependencies
- Move dependencies used only in tests under `[dev-dependencies]`
- Use workspace dependencies if already available there
- For workspace dependencies, if there is a single field, prefer `crate.workspace = true` over `crate = { workspace = true}`; if there are multiple fields use `crate = { workspace = true, features = ["..."] }`

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
- Use builder patterns for complex construction (e.g., `TxBuilder`, `WolletBuilder`) and for constructor with large/unknown number of parameters

### Commit Messages
Format: `context: <description>`
- Context: crate/directory name or `ci`/`fix`/`feat`/`docs`/`refactor`
- For crates, do not include `lwk_` for example `lwk_wollet:` use just `wollet:`
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
