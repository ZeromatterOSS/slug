# Kuro: Bazel-Compatible Build Tool Implementation Plan

## Overview

Kuro is a Bazel 9.0-compatible build tool that leverages Buck2's high-performance Rust internals (DICE incremental computation, starlark-rust interpreter, remote execution architecture) while providing full compatibility with Bazel's BUILD.bazel files, bzlmod module system, and the rules\_\* ecosystem.

Named after the [Costasiella kuroshimae](https://en.wikipedia.org/wiki/Costasiella_kuroshimae) (the "leaf sheep" sea slug), kuro aims to be a small, efficient alternative to Bazel that "eats" the same build files but runs faster.

## Current State Analysis

### Starting Point: Buck2 Fork

- Kuro provides proven, high-performance build infrastructure
- DICE engine delivers 2x performance improvement over traditional build systems
- starlark-rust is a mature Starlark interpreter with type annotation support
- Remote execution architecture is production-ready (Meta scale)
- Modular Rust crates (dice, starlark, gazebo, allocative, superconsole) are reusable
- BXL provides powerful build graph introspection for developer tooling

### Key Gaps to Bridge

| Feature          | Buck                        | Bazel 9.0                | Work Required                                |
| ---------------- | --------------------------- | ------------------------ | -------------------------------------------- |
| Build files      | BUCK                        | BUILD.bazel              | File detection change                        |
| Starlark dialect | `attrs.*`, type annotations | `attr.*`, optional types | Bazel only (`attr.*`), keep type annotations |
| Rule definition  | `impl` param                | `implementation` param   | Bazel only (`implementation`)                |
| Config format    | .buckconfig                 | .bazelrc                 | Overhaul config file parsing                 |
| Dep management   | Cells, no modules           | bzlmod mandatory         | Full bzlmod implementation                   |
| Registry         | None                        | BCR                      | Registry client                              |
| Local isolation  | None (RE-first)             | Sandboxing               | Implement sandboxing                         |
| Rust toolchain   | Nightly required            | -                        | Migrate to stable Rust                       |
| Target patterns  | `//pkg:`                    | `//pkg:all`              | Pattern parsing                              |
| Visibility       | `"PUBLIC"`                  | `"//visibility:public"`  | Syntax change                                |

## Desired End State

After completing this plan, kuro will:

1. **Build with stable Rust** - No nightly compiler required
2. **Parse and execute** standard Bazel 9.0 BUILD.bazel and MODULE.bazel files
3. **Enforce build isolation** via local sandboxing
4. **Fetch dependencies** from the Bazel Central Registry (BCR)
5. **Run rules_cc** to compile C/C++ projects
6. **Run rules_rust** to compile Rust projects
7. **Run rules_python** to run Python projects
8. **Run rules_oci** to build container images
9. **Support query commands** for build graph introspection
10. **Support Linux, Windows, and macOS** platforms
11. **Preserve BXL** for future developer tooling (compile_commands.json, IDE integration)

### Verification Criteria

- [ ] `cargo build --release` works with stable Rust
- [ ] `kuro build //...` works on a project using rules_cc
- [ ] `kuro build //...` works on a project using rules_rust
- [ ] `kuro build //...` works on a project using rules_python
- [ ] `kuro build //...` works on a project using rules_oci
- [ ] `kuro run //:target` executes binaries
- [ ] `kuro query //...` returns dependency information
- [ ] BCR modules are fetched and cached correctly
- [ ] Lockfile (MODULE.bazel.lock) is generated and respected
- [ ] Sandboxed builds catch undeclared dependencies
- [ ] Cross-platform builds work (Linux, Windows, macOS)

## What We're NOT Doing

1. **Buck compatibility** - No support for BUCK files or Buck-specific Starlark
2. **WORKSPACE support** - Removed in Bazel 9.0, not implementing
3. **Android/iOS rules** - Focus on C/C++, Rust, Python first
4. **Java rules** - Lower priority than core languages
5. **Remote execution initially** - Local execution first, RE later
6. **GUI/IDE integration** - CLI only initially
7. **Removing type annotations** - Keep starlark-rust's type support (Bazel is adding this)

## Implementation Approach

We will fork Buck2 and progressively modify it to speak Bazel's dialect. The approach is:

1. **Fork and rebrand** - kuro identity
2. **Starlark compatibility** - Add Bazel APIs while keeping type support
3. **Build file detection** - Switch from BUCK to BUILD.bazel
4. **bzlmod** - Implement module system incrementally
5. **Module extensions** - Support custom dependency resolution
6. **Rule primitives** - Ensure ctx/actions/providers match Bazel API
7. **Rules integration** - Test with actual rules\_\* packages
8. **Stable Rust** - Remove nightly dependencies
9. **Local sandboxing** - Add build isolation
10. **Platform support** - Linux, Windows, macOS
11. **Query commands** - Add bazel-compatible query interface

**Process Note:** Commit changes with a brief message after completing every phase/step.

---

## Sub-Plans

The detailed implementation is split into focused sub-plans:

| Sub-Plan                                                                 | Phases | Description                                                   | Status          |
| ------------------------------------------------------------------------ | ------ | ------------------------------------------------------------- | --------------- |
| [01-foundation.md](./kuro-bazel-subplans/01-foundation.md)               | 1-3    | Fork, rebrand, Starlark dialect, BUILD.bazel detection        | **Complete**    |
| [02-bzlmod.md](./kuro-bazel-subplans/02-bzlmod.md)                       | 4a-5b  | bzlmod module system, BCR integration, resolution, extensions | **In Progress** |
| [03-rule-primitives.md](./kuro-bazel-subplans/03-rule-primitives.md)     | 6      | ctx/actions/providers API alignment                           | Not Started     |
| [04-rules-integration.md](./kuro-bazel-subplans/04-rules-integration.md) | 7-10   | rules_cc, rules_rust, rules_python, rules_oci                 | Not Started     |
| [05-infrastructure.md](./kuro-bazel-subplans/05-infrastructure.md)       | 11-14  | Stable Rust, sandboxing, platform support, query              | Not Started     |

### Related Research Documents

- [bzlmod Resolution Algorithm](../research/2026-01-21-bzlmod-resolution-algorithm.md) - In-depth MVS algorithm documentation
- [Test Infrastructure Mapping](../research/2026-01-22-test-infrastructure-mapping.md) - Test migration strategy

---

## Phase Index

Quick reference to all phases and their locations:

### Foundation (Phases 1-3) - [Sub-plan](./kuro-bazel-subplans/01-foundation.md)

| Phase | Title                                  | Status       |
| ----- | -------------------------------------- | ------------ |
| 1     | Fork and Foundation                    | [x] Complete |
| 2     | Starlark Dialect - Bazel Compatibility | [x] Complete |
| 3     | Build File Recognition                 | [x] Complete |

### bzlmod (Phases 4a-5b) - [Sub-plan](./kuro-bazel-subplans/02-bzlmod.md)

| Phase | Title                            | Status                                        |
| ----- | -------------------------------- | --------------------------------------------- |
| 4a    | bzlmod - Workspace Recognition   | [x] Complete                                  |
| 4b    | bzlmod - Local Dependencies      | [x] Complete                                  |
| 4c    | bzlmod - BCR Integration         | [x] Complete                                  |
| 4d    | bzlmod - Resolution and Lockfile | [x] Complete                                  |
| 5     | Module Extensions                | [ ] Partial (parsing done, execution pending) |
| 5b    | bzlmod Build Integration         | [ ] In Progress                               |

### Rule Primitives (Phase 6) - [Sub-plan](./kuro-bazel-subplans/03-rule-primitives.md)

| Phase | Title                                      | Status          |
| ----- | ------------------------------------------ | --------------- |
| 6     | Rule Primitives and Provider Compatibility | [ ] Not Started |

### Rules Integration (Phases 7-10) - [Sub-plan](./kuro-bazel-subplans/04-rules-integration.md)

| Phase | Title                    | Status          |
| ----- | ------------------------ | --------------- |
| 7     | rules_cc Integration     | [ ] Not Started |
| 8     | rules_rust Integration   | [ ] Not Started |
| 9     | rules_python Integration | [ ] Not Started |
| 10    | rules_oci Integration    | [ ] Not Started |

### Infrastructure (Phases 11-14) - [Sub-plan](./kuro-bazel-subplans/05-infrastructure.md)

| Phase | Title                              | Status          |
| ----- | ---------------------------------- | --------------- |
| 11    | Stable Rust Migration              | [ ] Not Started |
| 12    | Local Build Isolation (Sandboxing) | [ ] Not Started |
| 13    | Platform Support                   | [ ] Not Started |
| 14    | Query Commands                     | [ ] Not Started |

---

## Test Migration Strategy

> **Reference Document**: [`2026-01-22-test-infrastructure-mapping.md`](../research/2026-01-22-test-infrastructure-mapping.md)

### Overview

The Kuro codebase inherits Buck2's extensive pytest-based test infrastructure. As we adopt Bazel semantics, tests must be migrated accordingly:

1. **KEEP+UPDATE** (~34 tests): Buck2 tests covering shared concepts - update syntax/semantics to Bazel
2. **DELETE** (~32 tests): Buck2-specific tests (cells, BUCK files, `attrs.*`) - no Bazel equivalent
3. **ADD** (~123 tests): Bazel concepts not in Buck2 (bzlmod, `attr.*`, providers, sandboxing)
4. **PRESERVE** (~69 tests): Tests covering identical concepts in both systems

### Test Framework Preservation

We preserve the existing pytest infrastructure because:

- Python async tests enable parallel execution
- Golden file infrastructure handles non-determinism
- Sanitization functions are mature and extensible
- Easier to read/write than Bazel's shell-based tests

### Framework Modifications Required

1. **Workspace Setup** (`tests/e2e_util/buck_workspace.py`):
    - Support `MODULE.bazel` as workspace root marker
    - Support `BUILD.bazel` instead of `TARGETS.fixture`
    - Update default config generation

2. **Test Fixtures** (`test_*_data/` directories):
    - Replace `.buckconfig` with `MODULE.bazel`
    - Replace `TARGETS.fixture` with `BUILD.bazel`
    - Update attribute syntax (`attr.*` not `attrs.*`)
    - Update visibility syntax (`//visibility:public`)

3. **Golden Files** (`*.golden`):
    - Update expected output formats for Bazel
    - Add sanitizers for Bazel-specific paths/hashes

### Per-Phase Test Tasks

| Phase      | Test Actions                                                          |
| ---------- | --------------------------------------------------------------------- |
| Phase 2    | Update `attr.*` tests, add `native.*` tests, update rule syntax tests |
| Phase 3    | Update build file detection tests for `BUILD.bazel`                   |
| Phase 4a-d | ADD bzlmod tests, DELETE cell tests                                   |
| Phase 5    | ADD module extension tests                                            |
| Phase 6    | ADD ctx/actions/provider/depset/runfiles tests                        |
| Phase 7-10 | ADD rules\_\* integration tests                                       |
| Phase 12   | ADD sandbox isolation tests                                           |
| Phase 14   | ADD query function tests (deps, rdeps, kind, filter)                  |

### Test Categories to Delete (Buck2-Specific)

- `tests/core/cells/` - Replace with bzlmod workspace tests
- `tests/core/external_cells/` - Replace with bzlmod registry tests
- Tests using `.buckconfig` - Replace with `MODULE.bazel`
- Tests using `attrs.*` API - Replace with `attr.*`
- Tests using `impl` parameter - Replace with `implementation`
- BXL tests - PRESERVE for tooling, but not priority

### Test Categories to Add (Bazel-Specific)

**Critical for Bazel Compatibility:**

- bzlmod parsing and resolution tests
- `attr.*` function tests
- `native.*` module tests
- `ctx.actions.*` API tests
- Provider tests (DefaultInfo, CcInfo, PyInfo, etc.)
- Depset operation tests
- Sandbox isolation tests
- Query function tests
