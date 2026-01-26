# Kuro Manual Test Project

This directory contains a manual test project for validating bzlmod features during development. It serves as both a test fixture and documentation of current capabilities.

## Usage

### Running Tests

From this directory:

```bash
# Check cell resolution
../../target/release/kuro audit cell

# Parse BUILD files (verify loads work)
../../target/release/kuro targets root//:

# Build a target (when rules are implemented)
../../target/release/kuro build //:test_target
```

Or from the kuro root:

```bash
./target/release/kuro --chdir tests/manual_test audit cell
```

### What This Tests

1. **bzlmod cell resolution** - MODULE.bazel parsed, deps fetched from BCR
2. **Cross-cell loading** - `@bazel_skylib//lib:dicts.bzl` loads from BCR module
3. **native.bazel_version** - Returns "9.0.0-kuro" for compatibility
4. **bazel_tools bundling** - @bazel_tools auto-registered for bzlmod projects (Phase 5c)

## Current Status

| Feature | Status | Notes |
|---------|--------|-------|
| `@bazel_skylib` loading | Working | Simple .bzl files load correctly |
| `native.bazel_version` | Working | Returns "9.0.0-kuro" |
| `@rules_cc` loading | Blocked | Needs CcInfo provider (Phase 6) |
| `@bazel_tools` bundled | Working | Auto-registered for bzlmod projects |
| `@bazel_tools` cache.bzl | Working | visibility() function implemented |
| `@bazel_tools` http.bzl | Blocked | Needs repository_rule (Phase 5) |

## Directory Structure

```
manual_test/
├── MODULE.bazel          # Root module with bazel_deps
├── BUILD.bazel           # Test loads and prints
├── .buckconfig           # Cell configuration (minimal)
├── .buckroot             # Workspace marker
├── prelude/              # Minimal prelude stub
│   ├── BUILD.bazel
│   └── prelude.bzl
└── bazel-external/       # Auto-populated external modules
    ├── bazel_skylib/     # BCR module (auto-fetched)
    └── bazel_tools/      # Legacy shim (no longer needed)
```

## Extending This Test

When implementing new features, extend this test project:

1. **New bzlmod features**: Add `bazel_dep()` entries to MODULE.bazel
2. **New loads**: Add `load()` statements to BUILD.bazel
3. **Verify loads**: Run `kuro targets root//:` and check print output

## Learnings from Testing

### What Works

- BCR modules are fetched to `~/.cache/kuro/` and copied to `bazel-external/`
- Cell resolver includes bzlmod modules alongside .buckconfig cells
- Cross-cell `load()` statements resolve correctly
- `native.bazel_version` is accessible as "9.0.0-kuro"
- `@bazel_tools` is auto-registered as a bundled cell for bzlmod projects

### Current Blockers

1. **rules_cc loading** - Fails with "Variable `CcInfo` not found" because native providers aren't exposed (Phase 6)
2. **@bazel_tools http.bzl** - Requires `repository_rule` Starlark global (Phase 5 - repository rules)
3. **@bazel_tools toolchain_utils.bzl** - Loads `@rules_cc` which needs CcInfo provider (Phase 6)
4. **Module extensions** - Parsing works, execution not implemented (Phase 5)

### What Now Works

- `visibility()` function - no-op stub allows bazel_tools files with `visibility("public")` to load
- `@bazel_tools//tools/build_defs/repo:cache.bzl` - loads successfully
- `@bazel_tools//tools/build_defs/repo:utils.bzl` - loads successfully (used by cache.bzl)

### Testing Protocol

1. **Before changes**: Run `kuro audit cell` to verify baseline
2. **After changes**: Run `kuro targets root//:` to verify loads
3. **Check output**: Look for print statements in BUILD.bazel
4. **Verify errors**: Error messages should be clear and actionable
