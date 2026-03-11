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
| Target patterns  | `//pkg:`                    | `//pkg:all`              | Pattern parsing                              |
| Visibility       | `"PUBLIC"`                  | `"//visibility:public"`  | Syntax change                                |

## Desired End State

After completing this plan, kuro will:

1. **Parse and execute** standard Bazel 9.0 BUILD.bazel and MODULE.bazel files
2. **Enforce build isolation** via local sandboxing
3. **Fetch dependencies** from the Bazel Central Registry (BCR)
4. **Run rules_cc** to compile C/C++ projects
5. **Run rules_rust** to compile Rust projects
6. **Run rules_python** to run Python projects
7. **Run rules_oci** to build container images
8. **Support query commands** for build graph introspection
9. **Support Linux, Windows, and macOS** platforms
10. **Preserve BXL** for future developer tooling (compile_commands.json, IDE integration)

### Version Compatibility Requirements

**Critical**: Kuro must report itself as Bazel 9.0+ for compatibility with modern rules:

1. **`native.bazel_version`** must return a version string >= "9.0.0" (e.g., "9.0.0" or "9.0.0-kuro")
2. **Version checks from `bazel_features`** must work correctly:
    - `_bazel_version_ge("9.0.0-pre.1231")` must return `True`
    - Version comparison functions (`ge`, `gt`, `lt`) must work with semver strings
3. **Abort on incompatible version**: If Kuro is somehow configured to report < 9.0.0, it should abort with a clear error
4. **Test with rules_cc 0.2.16** - This is the minimum version that works properly with Bazel 9.0

The `bazel_features` module (https://github.com/bazel-contrib/bazel_features) provides feature detection that many rules depend on. Kuro must satisfy these version checks to be compatible with the modern rules\_\* ecosystem.

### Depset ↔ TransitiveSet Bridge Strategy

Kuro must support Bazel’s `depset` API **and** preserve Buck2’s `transitive_set` advantages for Kuro‑specific rules and internal optimizations. These types are not identical, so we need an explicit bridge with clearly defined semantics and performance constraints.

#### Design Goals

1. **Bazel compatibility at the API boundary**: rules\_\*, prelude shims, and Bazel semantics should consume and return `depset`.
2. **Buck2 internals can still use `transitive_set`** where projections, reductions, and typed definitions provide value.
3. **Explicit conversion** (not implicit coercion) to avoid silent semantic loss.
4. **Cache conversions** to avoid repeated materialization in hot analysis paths.

#### Conversion API (Proposed)

Provide explicit native helpers (Starlark visible) for controlled conversion:

```python
# Native bridge helpers (names subject to bikeshed)
native.transitive_set_from_depset(d, order = "default")
native.depset_from_transitive_set(t, order = "default")
```

**Semantics:**

- `depset -> transitive_set`:
    - Use a single built‑in `transitive_set` definition (e.g. `BazelDepsetTset`) with one field (`items`).
    - Preserve transitive structure when possible by wiring children as tset children.
    - Ordering is **best‑effort**; store the `order` string for traversal hints.
- `transitive_set -> depset`:
    - Materialize via traversal (default preorder) and build a depset from values.
    - Projections/reductions are not preserved; document this loss explicitly.

#### Performance & Architectural Implications

- **Conversion cost** is proportional to the transitive closure size if materialized. This can be expensive if done per target in analysis.
- **Caching is required**:
    - Cache on `FrozenTransitiveSet` for `depset` projection per order.
    - Cache on `depset` objects for `transitive_set` projection per order.
- **Hot‑path safety**: only convert at API boundaries, avoid repeated conversions in tight loops.
- **Ordering fidelity**: Bazel `depset` order semantics do not perfectly match `transitive_set` traversal. The bridge must specify “best‑effort” mapping and document behavior.
- **No implicit coercion**: attribute coercion should not silently convert between these types.

#### Success Criteria (Bridge)

- Explicit conversion APIs exist and are documented.
- Conversion round‑trip is deterministic for basic cases:
    - `depset -> transitive_set -> depset` preserves element sets.
    - `transitive_set -> depset -> transitive_set` preserves element sets.
- Conversion is cached and does not regress analysis time in representative rule graphs.

### Verification Criteria

- [x] `kuro build //...` works on a project using rules_cc
- [x] `kuro build //...` works on a project using rules_rust
- [x] `kuro build //...` works on a project using rules_python
- [x] `kuro build //...` works on a project using rules_oci
- [x] `kuro run //:target` executes binaries (verified: hello_rust_bin prints "Hello, Kuro!")
- [x] `kuro query //...` returns dependency information
- [x] BCR modules are fetched and cached correctly
- [x] Lockfile (MODULE.bazel.lock) is generated and respected
- [x] Sandboxed builds catch undeclared dependencies (buck-out input isolation implemented; 2026-02-23)
- [x] Cross-platform builds work (Linux, Windows, macOS) — Windows verified with MSVC (2026-03-10)

## What We're NOT Doing

1. **Buck compatibility** - No support for BUCK files or Buck-specific Starlark
2. **WORKSPACE support** - Removed in Bazel 9.0, not implementing
3. **Android/iOS rules** - Focus on C/C++, Rust, Python first
4. **Java rules** - Lower priority than core languages
5. **Remote execution initially** - Local execution first, RE later
6. **GUI/IDE integration** - CLI only initially
7. **Removing type annotations** - Keep starlark-rust's type support (Bazel is adding this)
8. **Native language rule implementations** - In Bazel 9.0, language rules (cc*\*, py*_, proto\__) are pure Starlark in their respective rules\_\* repos. Kuro does NOT implement `native.py_library`, `native.proto_library`, etc. See [Native → Starlark Migration Architecture](#native--starlark-migration-architecture).

## Native → Starlark Migration Architecture

A critical Bazel 9.0 design principle: **all language rules are pure Starlark**. The native (C++/Java) implementations that existed in Bazel's core have been completely removed. Each rules\_\* repo independently detects the Bazel version and switches to its own Starlark implementations. Kuro must understand and support each repo's switching mechanism.

### Migration Pattern Summary

| Rules Repo              | Detection Mechanism             | Switch Point                                                | Starlark Path                                                   |
| ----------------------- | ------------------------------- | ----------------------------------------------------------- | --------------------------------------------------------------- |
| **rules_cc 0.2.16**     | Version string comparison       | `_bazel_version_ge("9.0.0-pre.20250911")`                   | `@cc_compatibility_proxy` synthetic repo selects Starlark impls |
| **rules_python 1.8.0+** | Config flag + feature detection | `enable_pystar` + `hasattr(native, "starlark_doc_extract")` | `@rules_python_internal` config repo enables Starlark impls     |
| **protobuf 33.4+**      | Feature detection               | `hasattr(native, "proto_library")` → False                  | Rules in `@protobuf//bazel/*.bzl` (self-contained)              |
| **rules_rust 0.40.0**   | Pure Starlark always            | N/A (no native fallback)                                    | Rules in `@rules_rust//rust/`                                   |

### rules_cc: Version-Based Proxy Repository

rules_cc 0.2.16 uses `@bazel_features` to compare `native.bazel_version` against `"9.0.0-pre.20250911"`:

```starlark
# cc/extensions.bzl:31
if _bazel_version_ge("9.0.0-pre.20250911"):
    # Bazel 9.0+: Load Starlark implementations from cc/private/rules_impl/
    # CcInfo, cc_common, etc. are all pure Starlark
else:
    # Pre-9.0: Delegate to native.cc_library, native.cc_binary, etc.
```

**Kuro approach**: Generate synthetic `@cc_compatibility_proxy` repo matching the Bazel 9.0+ path. Only the `cc_common` module needs native implementation.

### rules_python: Config-Based Switching

rules_python uses a two-stage mechanism:

1. **Repository rule** (`internal_config_repo.bzl`): Checks `hasattr(native, "starlark_doc_extract")` (Bazel 7+ feature) AND `RULES_PYTHON_ENABLE_PYSTAR` env var (default `"1"`). Generates `@rules_python_internal//:rules_python_config.bzl`.

2. **Rule files** (`py_library.bzl`, `py_binary.bzl`, `py_test.bzl`):

```starlark
load("@rules_python_internal//:rules_python_config.bzl", "config")
_py_library_impl = _starlark_py_library if config.enable_pystar else native.py_library
```

**Kuro approach**: Generate synthetic `@rules_python_internal` with `enable_pystar = True`. Provide `py_internal` stubs for the Starlark implementations. Do NOT implement `native.py_library` (removed in Bazel 9.0).

### protobuf: Feature Detection

protobuf 33.4+ checks whether native proto rules exist:

```starlark
# bazel/proto_library.bzl
if not hasattr(native, "proto_library"):
    _proto_library(**kwattrs)  # Starlark implementation
else:
    native.proto_library(**kwattrs)  # Native (Bazel 6-7 only)
```

**Note**: protobuf 27.0 (older BCR version) uses `proto_library = native.proto_library` directly — this is incompatible with Bazel 9.0. Must use protobuf 33.4+.

**Kuro approach**: Ensure `hasattr(native, "proto_library")` returns False (don't register native proto rules). protobuf's Starlark implementation handles everything. Need `ProtoInfo` provider and `proto_common` module stubs.

### Implications for Kuro

1. **Do NOT implement native language rules** (`native.py_library`, `native.proto_library`, etc.) — these are removed in Bazel 9.0. At most, stub them as `= None` on the `native` module to avoid crashes when code checks `hasattr(native, "py_library")`, but never provide a real implementation.
2. **DO implement native modules** (`cc_common`, `proto_common`) that Starlark implementations call into
3. **DO provide synthetic repos** that configure each rules\_\* repo to use its Starlark path
4. **DO provide provider stubs** (`PyInfo`, `ProtoInfo`) that Starlark implementations reference
5. **Version matters**: Use recent rules\_\* versions that support Bazel 9.0 (rules_cc 0.2.16, rules_python 1.8.0+, protobuf 33.4+)

## BXL Preservation Strategy

BXL (Buck Extension Language) is a powerful build graph introspection system inherited from Buck2. It will be preserved as a **Kuro-specific extension feature** that enhances Bazel-compatible builds.

### Why Preserve BXL

1. **IDE Integration** - Generates `compile_commands.json` for clangd/LSP support
2. **Build Analysis** - Custom queries and analysis beyond standard Bazel query
3. **Developer Tooling** - Project file generation (VSCode, Visual Studio, etc.)
4. **No Bazel Conflict** - BXL uses separate `.bxl` files and `kuro bxl` command

### BXL vs Bazel Aspects

| Feature        | BXL                              | Bazel Aspects                       |
| -------------- | -------------------------------- | ----------------------------------- |
| **Invocation** | External (`kuro bxl`)            | Internal (during analysis)          |
| **Purpose**    | User-facing automation           | Rule-internal computation           |
| **Execution**  | After build/analysis             | During analysis phase               |
| **Use Case**   | IDE integration, custom analysis | Provider propagation, cross-cutting |

BXL and aspects solve different problems - BXL is for external introspection, aspects are for internal computation. Both are needed.

### Preservation Approach

1. **Keep `kuro bxl` command** - Separate from Bazel build commands
2. **Keep `.bxl` file extension** - No conflict with `.bzl` files
3. **Keep `prelude/bxl/` directory** - BXL support files
4. **Document as extension** - Position as "Kuro Extension Language"
5. **Update examples** - Ensure BXL examples work with Bazel-style rules (rules_cc, etc.)

### Key BXL Features

- `ctx.uquery()`, `ctx.cquery()`, `ctx.aquery()` - Query operations
- `ctx.analysis(targets)` - Run analysis and access providers
- `ctx.build(artifacts)` - Build and materialize artifacts
- `ctx.bxl_actions()` - Create custom actions
- `ctx.output.print()`, `ctx.output.ensure()` - Output handling

See `docs/bxl/` for full documentation.

## Implementation Approach

We will fork Buck2 and progressively modify it to speak Bazel's dialect. The approach is:

1. **Fork and rebrand** - kuro identity
2. **Starlark compatibility** - Add Bazel APIs while keeping type support
3. **Build file detection** - Switch from BUCK to BUILD.bazel
4. **bzlmod** - Implement module system incrementally
5. **Module extensions** - Support custom dependency resolution
6. **Rule primitives** - Ensure ctx/actions/providers match Bazel API
7. **Rules integration** - Test with actual rules\_\* packages
8. **Local sandboxing** - Add build isolation
9. **Platform support** - Linux, Windows, macOS
10. **Query commands** - Add bazel-compatible query interface

**Process Notes:**

- Commit changes with a brief message after completing every phase/step.
- **IMPORTANT:** When adding stubbed functions during migration (functions that exist for API compatibility but don't yet implement full functionality), mark them with a `TODO` comment explaining what needs to be implemented. Use the format `// TODO(component): Description of what needs to be implemented.`

---

## Sub-Plans

The detailed implementation is split into focused sub-plans:

| Sub-Plan                                                                           | Phases | Description                                                   | Status          |
| ---------------------------------------------------------------------------------- | ------ | ------------------------------------------------------------- | --------------- |
| [01-foundation.md](./kuro-bazel-subplans/01-foundation.md)                         | 1-3    | Fork, rebrand, Starlark dialect, BUILD.bazel detection        | **Complete**    |
| [02-bzlmod.md](./kuro-bazel-subplans/02-bzlmod.md)                                 | 4a-5c  | bzlmod module system, BCR integration, resolution, extensions | **Complete**    |
| [03-rule-primitives.md](./kuro-bazel-subplans/03-rule-primitives.md)               | 6a,6c  | ctx/actions/providers API alignment + repository_ctx          | **Complete** (Tier 1-3 done, Tier 4 stubs adequate for rules_*) |
| [04-prelude-architecture.md](./kuro-bazel-subplans/04-prelude-architecture.md)     | 6b     | Prelude preservation, Bazel shim migration, cleanup           | **Complete** (6b.1-6b.3 done, 6b.4 partial)  |
| [05-builtins-compatibility.md](./kuro-bazel-subplans/05-builtins-compatibility.md) | 7a-7d  | Bazel native rules, global functions, modules, Buck2 removal  | **Complete** (all native rules, globals, modules done; documentation items remain) |
| [06-aspects.md](./kuro-bazel-subplans/06-aspects.md)                               | 8a-8d  | Bazel aspects implementation (blocks rules_cc)                | **Complete**    |
| [09-unified-execution-architecture.md](./kuro-bazel-subplans/09-unified-execution-architecture.md) | 9a-9f  | Lockfile compat, unified DICE execution, .buckconfig removal  | **Complete**    |
| [07-rules-integration.md](./kuro-bazel-subplans/07-rules-integration.md)           | 10-14  | rules_cc, rules_rust, rules_python, protobuf, rules_oci       | **Complete**    |
| [08-infrastructure.md](./kuro-bazel-subplans/08-infrastructure.md)                 | 16-18  | Sandboxing, platform support, query                           | **Functional**  |

### Related Research Documents

- [bzlmod Resolution Algorithm](../research/2026-01-21-bzlmod-resolution-algorithm.md) - In-depth MVS algorithm documentation
- [Test Infrastructure Mapping](../research/2026-01-22-test-infrastructure-mapping.md) - Test migration strategy
- [BXL vs AXL Comparison](../research/bxl-vs-axl-comparison.md) - Compare Buck2's BXL with Aspect's AXL for build introspection
- [rules_cc Native Requirements](../research/2026-01-26-rules-cc-native-requirements.md) - What Kuro must provide for rules_cc (Bazel 9.0+)
- [Sync Extension Executor Architecture Analysis](../research/2026-02-18-sync-extension-executor-architecture-analysis.md) - Comparison of Kuro's sync executor vs Bazel/Buck2 approaches
- Native → Starlark Migration: Each rules\_\* repo's version detection is documented inline in [07-rules-integration.md](./kuro-bazel-subplans/07-rules-integration.md#bazel-90-native--starlark-migration-architecture)

---

## Phase Index

Quick reference to all phases and their locations:

### Foundation (Phases 1-3) - [Sub-plan](./kuro-bazel-subplans/01-foundation.md)

| Phase | Title                                  | Status       |
| ----- | -------------------------------------- | ------------ |
| 1     | Fork and Foundation                    | [x] Complete |
| 2     | Starlark Dialect - Bazel Compatibility | [x] Complete |
| 3     | Build File Recognition                 | [x] Complete |

### bzlmod (Phases 4a-5c) - [Sub-plan](./kuro-bazel-subplans/02-bzlmod.md)

| Phase | Title                            | Status                                        |
| ----- | -------------------------------- | --------------------------------------------- |
| 4a    | bzlmod - Workspace Recognition   | [x] Complete                                  |
| 4b    | bzlmod - Local Dependencies      | [x] Complete                                  |
| 4c    | bzlmod - BCR Integration         | [x] Complete                                  |
| 4d    | bzlmod - Resolution and Lockfile | [x] Complete                                  |
| 5     | Module Extensions                | [x] Complete (see 02-bzlmod.md phases 5-5e)   |
| 5b    | bzlmod Build Integration         | [x] Complete                                  |
| 5c    | Bundle @bazel_tools Repository   | [x] Bundled (file loading blocked by APIs)    |

### Rule Primitives (Phases 6a, 6c) - [Sub-plan](./kuro-bazel-subplans/03-rule-primitives.md)

| Phase | Title                                      | Status          |
| ----- | ------------------------------------------ | --------------- |
| 6a    | `ctx` and `ctx.actions` Completeness       | [~] Partial — Tier 1 complete; Tier 2: all Done (param files, sibling, info_file/version_file); Tier 3-4 remain |
| 6c    | `repository_ctx` Implementation            | [x] Done — full implementation in `repository_ctx.rs`; all 5 attrs + 18 methods; Starlark repo rule execution via late binding in `starlark_repo_rule_executor.rs` |

### Prelude Architecture (Phase 6b) - [Sub-plan](./kuro-bazel-subplans/04-prelude-architecture.md)

| Phase | Title                                            | Status          |
| ----- | ------------------------------------------------ | --------------- | --- |
| 6b.1  | Preserve Buck2 Prelude Loading Mechanism         | [x] Already working (prelude.bzl → native.bzl → __kuro_builtins__ flow preserved) |
| 6b.2  | Migrate Bazel Shims from Native Rust to Starlark | [x] Revised: Must stay native for external cell access (2026-01-28 discovery) |
| 6b.3  | Remove Unused Buck2 Prelude Code                 | [x] Done via Phase 7d (15+ language dirs removed, 732 files, ~124k lines; 2026-02-26) |
| 6b.4  | Simplify Native Module Registration              | [~] Partial (native modules work but could be further reduced) |

### Builtins Compatibility (Phases 7a-7d) - [Sub-plan](./kuro-bazel-subplans/05-builtins-compatibility.md)

| Phase | Title                   | Status          |
| ----- | ----------------------- | --------------- |
| 7a    | Bazel Native Rules      | [~] Partial (constraint_setting/value, config_setting, platform, toolchain_type, cc_libc_top_alias, genquery stub done; genrule cmd/cmd_bash accept select(); 2026-02-25; genrule cmd_ps/cmd_bat Windows shells; config_setting values={} supports compilation_mode/cpu/crosstool_top/compiler keys; config_setting define_values={} properly stored and matched; 2026-03-11) |
| 7b    | Bazel Global Functions  | [~] Partial (audit done, glob exclude_directories added, missing functions implemented; package_group visibility resolution working with cross-package deps; existing_rules()/existing_rule() return "kind" field; repository_name() returns "@" for root cell; module_name()/module_version() added; 2026-03-11) |
| 7c    | Bazel Top-Level Modules | [~] Partial (config module done, platform_common done, testing.analysis_test() done, coverage_common done; cc_common compile passes flags, compilation context preserves all include types, linking_context extracts objects; Label.workspace_root returns "external/<repo>" for external repos, Label.repo_name added; 2026-03-11) |
| 7d    | Buck2-Specific Removal  | [~] Partial (read_config/read_root_config error with message; oncall/read_oncall/load_symbols removed; soft_error already errors; 2026-02-24; native.bzl 576→40 lines, rules.bzl APPLE_PLATFORMS_KEY removed, user/all.bzl Android/CXX/Xcode removed; 2026-02-25) |

### Aspects (Phases 8a-8d) - [Sub-plan](./kuro-bazel-subplans/06-aspects.md)

| Phase | Title                              | Status          |
| ----- | ---------------------------------- | --------------- |
| 8a    | Stub aspect() Function             | [x] Complete    |
| 8b    | Aspect Context and Basic Execution | [x] Complete    |
| 8c    | Shadow Graph Propagation           | [x] Complete    |
| 8d    | Advanced Features                  | [x] Complete (aspect attr resolution, cc_proto_library) |

### Unified Execution Architecture (Phases 9a-9f) - [Sub-plan](./kuro-bazel-subplans/09-unified-execution-architecture.md)

| Phase | Title                              | Status          |
| ----- | ---------------------------------- | --------------- |
| 9a    | Lockfile Format Compatibility      | [x] Complete (Bazel 9.0 format, version 26) |
| 9b    | Pre-Computed Canonical Names       | [x] Complete (pre_compute_extension_repo_cells) |
| 9c    | DICE-Only Extension Execution      | [x] Complete (sync executor removed) |
| 9d    | .buckconfig Elimination for Cells  | [x] Complete (cells from MODULE.bazel only) |
| 9e    | Configuration Migration (.bazelrc) | [x] Complete (bazelrc parser + injection; 2026-02-25) |
| 9f    | Cleanup and Unification            | [x] Complete (dead code removed) |

### Rules Integration (Phases 10-14) - [Sub-plan](./kuro-bazel-subplans/07-rules-integration.md)

| Phase | Title                    | Status                                                                                  |
| ----- | ------------------------ | --------------------------------------------------------------------------------------- |
| 10    | rules_cc Integration     | [x] In Progress (cc_library, cc_binary, cc_test build+test work; linkstatic=True and False; RPATH fix)                             |
| 11    | rules_rust Integration   | [x] Complete (rules_rust 0.40.0, rust_library + rust_binary)                            |
| 12    | rules_python Integration | [x] Complete (rules_python 1.8.0, enable_pystar=True, py_library + py_binary + py_test) |
| 13    | protobuf Integration     | [x] Complete (proto_library + cc_proto_library build end-to-end, 313 commands) |
| 14    | rules_oci Integration    | [x] Complete (rules_pkg/pkg_tar + oci_image build end-to-end 2026-02-19)                |
| 15    | bazel_skylib Integration | [x] Complete (bazel_skylib 1.5.0: copy_file, write_file, selects.config_setting_group work; 2026-02-26) |

### Infrastructure (Phases 16-18) - [Sub-plan](./kuro-bazel-subplans/08-infrastructure.md)

| Phase | Title                              | Status          |
| ----- | ---------------------------------- | --------------- |
| 16    | Local Build Isolation (Sandboxing) | [x] Functional (Linux: user+mount namespaces, root read-only, output dirs writable, --nosandbox flag; 2026-02-20) |
| 17    | Platform Support                   | [x] Functional (Linux+Windows+macOS: @local_config_platform//:host auto-generated with host OS/CPU; CC toolchain config platform-aware; MSVC auto-detection; CcToolchainInfoStub per-platform; --copt/--cxxopt/--linkopt/--strip/--features flags; execution_requirements; PlatformFragment/JavaFragment/AppleFragment stubs; 30+ common Bazel CLI flags accepted; package_group visibility resolution; 2026-03-11) |
| 18    | Query Commands + Test Runner       | [x] Functional (deps, rdeps, allpaths, somepath, kind, attr, filter, buildfiles, tests; --output=label/json/build/graph; kuro test //... runs 4 tests) |

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
| Phase 7-13 | ADD rules\_\* integration tests                                       |
| Phase 15   | ADD sandbox isolation tests                                           |
| Phase 17   | ADD query function tests (deps, rdeps, kind, filter)                  |

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

---

## Key Learnings

This section documents important lessons learned during development that are broadly applicable.

### Build System Debugging

#### Debug vs Release Binary Mismatch (2026-02-04)

**Problem:** Debug logging (eprintln!, tracing::warn!, file writes) wasn't appearing during development.

**Root Cause:** The `kuro` symlink at project root points to `target/debug/kuro`, but development was using `cargo build --release` which outputs to `target/release/kuro`.

**Solution:** Always match the build type to the symlink target:

- Use `cargo build -p kuro` (debug) when using the `./kuro` symlink
- Or update the symlink to point to release if using `cargo build --release`

**Detection Method:** Added `panic!("DEBUG: message")` to verify code path was being reached - this confirmed the release binary was running, not the debug binary being built.

#### Bazel Artifact Path Model

**Critical Insight:** Bazel's `File.path` returns full execution-time paths (e.g., `bazel-out/k8-fastbuild/bin/pkg/__target__/file.o`), while Buck2's original implementation returned relative paths. This breaks rules that store paths as strings for later command-line use.

**Fix:** Modified `artifact.path` to construct full buck-out paths: `buck-out/v2/gen/<cell>/<cfg_hash>[/<pkg>]/__<target>__/<artifact_path>`

#### Buck2 Dependency Tracking

Buck2 tracks action dependencies by visiting artifacts in command lines via `CommandLineArgLike::visit_artifacts`. When string paths are used (common in Bazel rules), Buck2 doesn't see them as dependencies. Added `bazel_inputs` field to explicitly track Bazel-style inputs.

### bzlmod Implementation

#### repo_name Aliasing

The `bazel_dep(..., repo_name="alias")` pattern creates cell aliases that allow `@alias//...` to resolve to the actual module. Implementation involves:

1. `collect_transitive_repo_aliases()` extracts aliases from transitive deps' MODULE.bazel files
2. Aliases returned in `BzlmodResolutionResult.aliases`
3. Merged into `root_aliases` HashMap
4. Passed to `CellsAggregator::new()` for cell resolver registration

Example aliases created: `com_google_protobuf -> protobuf`, `com_google_absl -> abseil-cpp`

---

## Test Suite TODO (as of 2026-03-03)

Current status: **~980 pass, ~160 skip, 0 fail** in `tests/core/` (updated 2026-03-11). All test categories (analysis, build, run, test, bzlmod, docs, help, log, completion, interpreter, transitive_sets, validation) pass with 0 failures. 1158 tests collected.

### CI Infrastructure (2026-03-05)

- **`test.py` accepts `--kuro` arg** (was failing with argparse error in CI)
- **Python integration tests added to CI** via `run_python_tests()` in `test.py`
- **`requirements-test.txt` created** (pytest>=9.0.0, pytest-asyncio>=1.0.0)
- **`run_test_py/action.yml` updated** to install pytest deps before running
- **Test collection fixed** (0 errors, 933 tests collected):
  - `tests/e2e/` excluded from collection (requires Meta-internal workspace)
  - Template files excluded (`test_bxl_template.py`, `test_bxl_check_dependencies_template.py`)
  - `tests/manual_test/` excluded
- **`Attr.md` golden file updated** to match current `attr.*` documentation
- **121 tracked `.pyc` files removed** from git + `.gitignore` updated
  (were causing `check_no_changes` CI failure after pytest runs)

### Fixed (2026-03-03)

- **Source location tests + `test_local_incompatible`** (6 tests in `test_error_categorization.py`) - Fixed by:
  1. Updating test assertions to use `kuro_*` crate paths instead of `buck2_*`
  2. Updating `KURO` vs `BUCK2` source area
  3. For `test_local_incompatible`: changed `get_command_executor` to return `Err(IncompatibleExecutorPreferences)` instead of `None`, and updated `category_key` assertion to `"IncompatibleExecutorPreferences"` (Input tag is "hidden" so type name is used)
  4. Fixed `kuro_client` Windows build error (unix-only symlink calls in `run.rs`)
  5. Added `.exe` extension support for kuro binary path detection in conftest.py

### Potentially Fixable Skips

These SKIP_TESTS entries could be fixed with code changes:

1. ~~**`test_what_materialized_*`** (3 tests in `test_log/`) - "Materializations not tracked for local execution". Would need to implement materialization event tracking for local builds (currently only tracked for RE).~~ **FIXED** (2026-03-05): Added `MaterializationStart`/`MaterializationEnd` span events in `local.rs` after `declare_existing`, using `calc_output_count_and_bytes()` for stats.

2. ~~**`test_attr_default_coercion.py`** (in collect_ignore) - kuro doesn't validate label defaults at rule definition time. Could add validation in `AttrType::Label` coercion for default values.~~ **FIXED**: Added `strict_label_parsing` mode to `BuildAttrCoercionContext` - bare names without `:` or `//` now fail at bzl evaluation time. Moved from `collect_ignore` to active tests.

4. ~~**`test_unbound_artifact`** and **`test_unbound_artifact_inside_tset`** in `test_unbound_artifact.py` - "Unbound artifact build hangs daemon - deadlock in kuro". When `out.as_output()` AND plain `out` appear in the same cmd_args, the plain `out` (unbound DeclaredArtifact) was treated as a declared output by Bazel compat code, but after freeze it became a StarlarkArtifact treated as INPUT during execution → circular dependency → deadlock.~~ **FIXED** (2026-03-06): In `visit_declared_artifact` in `run.rs`, before treating an unbound artifact as a declared output, check if it's already in `declared_outputs` (from an explicit `.as_output()` call). If yes, produce `ArtifactErrors::UnboundArtifact` error ("Artifact must be bound by now") instead of creating a build-time deadlock. The existing valid Bazel compat code (unbound artifact NOT already in declared_outputs) is preserved.

3. ~~**`test_critical_path_test_entries`** in `test_critical_path.py` - "TestListing/TestExecution critical path entries not tracked". `KuroTestRunner::execute_test_from_spec` only performed `TestStage::Testing`, never `TestStage::Listing`.~~ **FIXED** (2026-03-05): Added listing stage in `kuro_test_runner/runner.rs` — runs command with `--list` first, parses test case names from stdout, reports LISTING_SUCCESS/LISTING_FAILED result, then runs testing stage with discovered test cases. Also fixed suite format to use full label (`{cell}//{package}:{target}`).

### Investigate Further

- `test_build_file_race` - "Build fails unexpectedly in kuro (file locking behavior differs)". Worth investigating if this is a real correctness issue.
- ~~**`test_noop`** in `test_paranoid.py` - Only needed `execution_platforms` data dir. Created minimal buck project at `tests/core/build/test_paranoid_data/execution_platforms/` with `.buckconfig`, `.buckroot`, `TARGETS.fixture`. Removed file from `collect_ignore`; added RE/paranoid-specific tests to `SKIP_TESTS`.~~ **FIXED**: `test_noop` now passes; `test_paranoid_enable_disable` skipped (uses `buck.debug("paranoid")` - Buck2-specific).
- `test_paranoid_enable_disable` - Requires `buck.debug("paranoid", ...)` command - Buck2-specific conservative RE caching. Not fixable without RE/paranoid mode implementation.

### Already Investigated - Not Fixable Without Major Work

- RE-dependent tests (~80): hybrid executor, cache uploads, dep files remote
- Eden/cgroup tests (~20): require EdenFS or Linux cgroups
- Buck2 modifier syntax (~40): `?modifier` target syntax not part of Bazel
- Meta-internal tests: manifold HTTP, BUCK2_TEST_* env vars, native.constraint rule

### New Tests Added (2026-03-06)

**Aspect tests** (`tests/core/analysis/test_aspects.py`, 5 tests):
- `test_aspect_basic_propagation` - Aspect propagates through deps via shadow graph
- `test_aspect_transitive_propagation` - Transitive propagation through 3-level dep chain
- `test_aspect_provider_access` - Aspects can access providers from target
- `test_aspect_required_providers_filter` - `required_providers` skips non-matching targets
- `test_aspect_ctx_rule_kind` - `ctx.rule.kind` returns the rule type name

**bzlmod test** (`tests/core/bzlmod/test_module_parsing.py`):
- Converted `test_module_bazel_syntax_error` from @skip/pass to real test with test data in `test_module_parsing_invalid_data/`
