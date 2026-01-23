# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with
code in this repository.

## Repository Overview

Kuro is a fast, hermetic, multi-language build system written in Rust.
Kuro is designed to be a fully compatible drop-in replacement for bazel,
built using the internals from Meta's Buck2 project.

This migration is in progress, and work is ongoing.

## Building and Development

**Using Kuro (self-bootstrap):**

```bash
./kuro.py           # Compile and run local kuro binary
```

Follow by normal kuro commands, e.g. `./kuro.py build fbcode//kuro:kuro` to
using local changed kuro binary to build kuro

### Testing

Kuro has extensive test suites located in the `tests/` directory:

**tests/core/** - Core integration tests

- Tests for individual Kuro subsystems and features
- Covers: analysis, audit commands, build system, BXL, configurations, DICE,
  query language, etc.

**tests/e2e/** - End-to-end tests

- Full workflow tests that exercise Kuro as users would
- Tests for: audit, build, BXL scripts, configurations, test command, etc.

**Running tests:**

```bash
# Run all tests in a Python file (e.g., tests/core/analysis/test_cmd_args.py)
kuro test fbcode//kuro/tests/core/analysis:test_cmd_args

# Run a specific test function within a Python file
# (e.g., test_output_artifact_in_relative_to in tests/core/analysis/test_cmd_args.py)
kuro test fbcode//kuro/tests/core/analysis:test_cmd_args -- test_output_artifact_in_relative_to

# Run all tests in a directory
kuro test fbcode//kuro/tests/core/analysis/...
```

## Code Architecture

### Major Components

**app/** - Main Kuro application code

- `kuro` - Main binary entry point
- `kuro_client` - Client-side CLI handling
- `kuro_server` - Server/daemon implementation
- `kuro_server_commands` - Server command implementations
- `kuro_build_api` - Core build system APIs
- `kuro_interpreter` - Starlark interpreter integration
- `kuro_execute` - Action execution framework
- `kuro_query` - Query language implementation
- `kuro_node` - Build graph node representation
- `kuro_artifact` - Artifact handling
- `kuro_bxl` - Buck Extension Language (BXL) support
- `kuro_test` - Test runner framework

**dice/** - Incremental computation engine

- DICE (Deterministic Incremental Computation Engine) powers Kuro's incremental
  builds
- Handles dependency tracking and change detection
- `dice/dice` - Core DICE implementation
- `dice/dice_error` - Error types

**starlark-rust/** - Starlark language implementation

- Kuro uses Starlark (a Python-like language) for build file definitions
- `starlark` - Core language implementation
- `starlark_lsp` - Language Server Protocol support
- `starlark_syntax` - Parser and syntax tree
- `starlark_map` - Optimized map data structure

**prelude/** - Standard build rules

- Contains the same prelude code used internally at Meta
- Default build rules for various languages (C++, Rust, Python, etc.)
- Platform configurations

**gazebo/** - Utility libraries

- `gazebo` - General utilities with `str_pattern_extensions`
- `dupe` - Cheap cloning trait for reference-counted types
- `strong_hash` - Type-safe hashing

**shed/** - Additional utility crates

- `static_interner` - String interning
- `lock_free_hashtable` - Lock-free concurrent data structures
- `provider` - Provider pattern implementations

**remote_execution/** - Remote execution client

- Implements Remote Execution API for distributed builds
- OSS version differs from internal Meta version

**superconsole/** - Terminal UI

- Rich terminal output and progress display

### Key Concepts

**Buck Extension Language (BXL):**

- Allows self-introspection of the build system
- Used for automation tools, LSPs, and compilation databases
- Can inspect and run actions in the build graph

**Multi-language Support:**

- Language-agnostic core with scriptable rule definitions
- Users can implement language support in Starlark
- Support for dependencies across languages

## Coding Conventions

- Follow standard `rustfmt` conventions
- Use `gazebo` utilities, especially `dupe` trait
- Prefer `to_owned` over `.to_string()` for `&str` to `String`
- Use `derivative` library for `PartialEq`/`Hash` when ignoring fields
- Prefer `use crate::foo::bar` over `use super::bar`
- Modules should have either submodules OR types/functions, not both

## Error Handling

Kuro uses a custom error handling system via `kuro_error` instead of `anyhow`.
All error handling in Kuro should follow these patterns:

### Result Type

Always use `kuro_error::Result<T>` instead of `anyhow::Result<T>`:

```rust
fn my_function() -> kuro_error::Result<String> {
    // ...
}
```

### Defining Custom Error Types

Use `#[derive(Debug, kuro_error::Error)]` instead of `thiserror::Error`. Every
error must be tagged with an `ErrorTag`:

```rust
#[derive(Debug, kuro_error::Error)]
#[error("My error message: {field}")]
#[kuro(tag = Input)]  // or other appropriate tag
struct MyError {
    field: String,
}

#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum MyErrors {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Missing required field: {0}")]
    MissingField(String),
}
```

### Error Tags

Common error tags (from `kuro_data::error::ErrorTag`):

- `Input` - User input errors (invalid arguments, malformed build files, etc.)
- `Tier0` - Critical infrastructure failures
- `Environment` - External environment issues (system configuration, external
  services, network/certificates, filesystem)
- Create new/meaningful/distinct error tag whenever possible in
  `app/kuro_data/error.proto`, and if the error is generic, use Input, Tier0,
  and Environment

### Creating Ad-Hoc Errors

Use the `kuro_error!` macro to create errors without defining a type:

```rust
use kuro_error::kuro_error;

if some_condition {
    return Err(kuro_error!(
        kuro_error::ErrorTag::Input,
        "Invalid value: expected {}, got {}",
        expected,
        actual
    ));
}
```

### Internal Errors

For bugs in Kuro code, use `internal_error!` macro:

```rust
use kuro_error::internal_error;

let value = map.get(key).internal_error("Key must exist")?;

// Or:
return Err(internal_error!(
    "Unexpected state: {} should not be empty",
    collection_name
));
```

### Adding Context to Errors

Use `BuckErrorContext` trait for adding context:

```rust
use kuro_error::BuckErrorContext;

// Add context to Results
result.buck_error_context("Failed to process file")?;

// Add context with formatted message
result.with_buck_error_context(|| format!("Failed to process file: {}", path))?;

// For internal errors
value.internal_error("This should never be None")?;
value.with_internal_error(|| format!("Missing key: {}", key))?;
```

### Error Conversion

Kuro's error system provides automatic conversion:

```rust
// From std::io::Error, std::fmt::Error, etc.
std::fs::read_to_string(path)?  // Automatically converts to kuro_error::Error

// From custom error types (if they implement std::error::Error)
my_custom_error?  // Works if error derives kuro_error::Error

// Manual conversion with tags
use kuro_error::conversion::from_any_with_tag;

some_result.map_err(|e| from_any_with_tag(e, ErrorTag::Tier0))?;
```

### Common Patterns

**Function returning Result:**

```rust
fn process_artifact(&self, artifact: &Artifact) -> kuro_error::Result<()> {
    let path = artifact.path()
        .buck_error_context("Failed to get artifact path")?;

    if !path.exists() {
        return Err(kuro_error!(
            kuro_error::ErrorTag::Input,
            "Artifact does not exist: {}",
            path
        ));
    }

    Ok(())
}
```

**Unwrapping with internal_error:**

```rust
// Instead of .unwrap() or .expect()
let value = option_value.internal_error("Value must be set")?;

// For collections
let item = collection.get(index)
    .with_internal_error(|| format!("Missing item at index {}", index))?;
```

### Key Differences from anyhow

1. **No `anyhow!` macro** - Use `kuro_error!` instead
2. **No `.context()`** - Use `.buck_error_context()` instead
3. **Tags required** - All errors must be categorized with an `ErrorTag`
4. **Type is `kuro_error::Result`** - Not `anyhow::Result`
5. **Derive `kuro_error::Error`** - Not `thiserror::Error`

## Internal vs OSS Differences

- Some code uses `@oss-enable` or `@oss-disable` markers
- `is_open_source()` function controls configuration differences
- Internal RE client differs from OSS version
- Internal version has additional Meta-specific integrations (Scribe, etc.)

## Protobuf Handling

Kuro uses Protocol Buffers extensively. On Linux/macOS/Windows, prebuilt
`protoc` binaries are used automatically.
