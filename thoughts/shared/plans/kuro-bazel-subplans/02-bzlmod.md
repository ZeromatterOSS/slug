# bzlmod Implementation Plan

> **Parent Plan**: [Kuro Bazel-Compatible Build Tool](../2026-01-21-kuro-bazel-compatible-build-tool.md)

This sub-plan covers the bzlmod module system implementation. Each phase has its own detailed file.

---

## Phase Status

| Phase | Description | Status | Details |
|-------|-------------|--------|---------|
| **4a** | Workspace Recognition | Complete | [Link](./02-bzlmod-phase-4.md#phase-4a-workspace-recognition) |
| **4b** | Local Dependencies | Complete | [Link](./02-bzlmod-phase-4.md#phase-4b-local-dependencies) |
| **4c** | BCR Integration | Complete | [Link](./02-bzlmod-phase-4.md#phase-4c-bcr-integration) |
| **4d** | Resolution & Lockfile | Complete | [Link](./02-bzlmod-phase-4.md#phase-4d-resolution-and-lockfile) |
| **5** | Module Extensions | Complete | [Link](./02-bzlmod-phase-5-overview.md) |
| **5a** | Extension Parsing | Complete | [Link](./02-bzlmod-phase-5-overview.md#phase-5a-extension-parsing) |
| **5b** | Build Integration | Complete | [Link](./02-bzlmod-phase-5b.md) |
| **5c** | Bundle @bazel_tools | Complete | [Link](./02-bzlmod-phase-5c.md) |
| **5d** | DICE Integration | Complete | [Link](./02-bzlmod-phase-5d.md) |
| **5e** | Extension Execution | Complete | [Link](./02-bzlmod-phase-5e.md) |
| **6** | Starlark Migration | Complete (N/A) | [Link](./02-bzlmod-phase-6.md) - Architectural constraint: must stay native |
| **7** | Proto Support | Future | [Link](./02-bzlmod-phase-7.md) |
| **8** | Full subrule() | Future | [Link](./02-bzlmod-phase-8-subrule.md) |
| **9** | External Cell Symlinks | Planned | [Link](./02-bzlmod-phase-9-external-symlinks.md) - Create bazel-external/ symlinks |

**Key Learnings from Completed Phases:**

| Phase | Key Learning |
|-------|--------------|
| 4a | MODULE.bazel parsed via Starlark interpreter; `kuro_bzlmod/src/parser.rs` |
| 4b | `local_path_override()` works; local modules integrated via cell system |
| 4c | Modules fetched to `~/.cache/kuro/`; SRI integrity verification works |
| 4d | MVS algorithm in `resolution.rs`; lockfile format compatible with Bazel 9.0 |
| 5b | BCR modules fetched and registered as cells; cross-cell `load()` works; `@bazel_skylib`, `@rules_cc` load successfully |
| 5c | `@bazel_tools` bundled from Bazel 9.0.0; automatically registered; `http.bzl`, `cache.bzl` load successfully |
| 5d | DICE key in `repository_execution.rs`; actual execution in `repository_executor.rs` with http_archive/git support |
| 5e | **Deferred execution**: Extensions capture RepoSpecs (not execute); repos materialize lazily on first access; module_ctx uses temp dir (deleted after) |
| 6 | **N/A**: Modules must stay native due to global injection architecture (external cells only see base globals) |
| 9 | **Planned**: Create `bazel-external/` symlinks to cache; required for external tools and some code paths that bypass `FileOpsDelegate` |

---

## Manual Testing Protocol

### Test Project Location

A manual test project is maintained at `tests/manual_test/` for validating bzlmod features during development.

### Running Tests

```bash
# From tests/manual_test/:
../../target/release/kuro audit cell          # Check cell resolution
../../target/release/kuro targets root//:     # Parse BUILD files, run tests

# From kuro root:
./target/release/kuro --chdir tests/manual_test audit cell
```

### Current Test Coverage

| Test                      | Command           | Expected Output                                          |
| ------------------------- | ----------------- | -------------------------------------------------------- |
| Cell resolution           | `audit cell`      | Shows root, prelude, bazel_skylib, bazel_tools (bundled) |
| native.bazel_version      | `targets root//:` | Prints "9.0.0"                                           |
| @bazel_skylib loading     | `targets root//:` | dicts.add returns merged dict                            |
| Version comparison        | `targets root//:` | version >= 9.0.0-pre.20250911 is True                    |
| @bazel_tools bundled      | `audit cell`      | bazel_tools registered without .buckconfig entry         |
| @bazel_tools file loads   | `targets root//:` | cache.bzl loaded: True (visibility() function works)     |
| Synthetic extension repos | `targets root//:` | bazel_features_version, bazel_features_globals created   |
| **rules_cc loading**      | `targets root//:` | Test 14c - rules_cc loaded successfully: True            |
| **repository_rule()**     | `targets root//:` | Test 21 - repository_rule type shown                     |
| **@bazel_tools http.bzl** | `targets root//:` | Test 22 - http_archive type: repository_rule             |

### Extending Tests

When implementing new features:

1. **Add bazel_dep** to `tests/manual_test/MODULE.bazel` for new BCR modules
2. **Add load statements** to `tests/manual_test/BUILD.bazel` with print() for validation
3. **Update README.md** with new test documentation
4. **Note**: @bazel_tools is now bundled (Phase 5c) - no shims needed

---

## Implementation Learnings

### What Works (Phase 5b verified)

- BCR modules fetched to `~/.cache/kuro/` and extracted to `bazel-external/`
- Cell resolver includes bzlmod modules alongside .buckconfig cells
- Cross-cell `load()` statements resolve correctly
- `native.bazel_version` returns "9.0.0" (released version for proper comparison)
- Simple @bazel_skylib .bzl files load and execute
- `visibility()` function implemented (no-op stub for now)
- @bazel_tools files using `visibility("public")` can now be loaded (e.g., cache.bzl)
- **Synthetic extension repos** for `bazel_features` work
- **Version comparison works**: `bazel_features` version checks return True for 9.0.0
- **Synthetic cc_compatibility_proxy repo** created for rules_cc

### Current Blockers

- ~~**@bazel_tools http.bzl/git.bzl**: Needs `repository_rule` and `repository_ctx`~~ **RESOLVED** (Phase 5)
- ~~**Repository rule execution**: `repository_rule()` and `repository_ctx` implemented, but rules are not actually invoked~~ **RESOLVED** (Phase 5d) - DICE key + executor implemented
- **Module extensions**: Parsing complete, execution requires DICE integration - see **Phase 5e**
- **rules_cc loading**: **COMPLETE** - rules_cc now loads successfully!
- **cc_library instantiation**: **COMPLETE** (Phase 8g) - `cc_library()` targets register successfully!
  - Analysis fails with provider checking issue (`DefaultInfo in artifact`) - separate from loading/instantiation
  - ~~`aspect()` built-in~~ **RESOLVED** (Phase 8a)
  - ~~`allow_empty` parameter~~ **RESOLVED**
  - ~~`PackageSpecificationInfo` provider~~ **RESOLVED** (added as NoneType)
  - ~~`cfg` parameter on attr.label()~~ **RESOLVED** (accepts string or config.exec())
  - ~~computed defaults (functions)~~ **RESOLVED** (skip coercion for functions)
  - ~~OutputGroupInfo provider~~ **RESOLVED** (changed to NoneType)
  - ~~stub transitions (None cfg)~~ **RESOLVED** (handle None in rule() cfg)
  - ~~`subrules` parameter~~ **RESOLVED** (added to rule())
  - ~~`initializer` parameter~~ **RESOLVED** (added to rule())
  - ~~`allow_rules` parameter~~ **RESOLVED** (added to attr.label/label_list)
  - ~~`values` parameter~~ **RESOLVED** (added to attr.int)
  - ~~`exec_group` built-in~~ **RESOLVED** (added as function returning None)
  - ~~`exec_groups` parameter~~ **RESOLVED** (added to rule())
  - ~~`RunEnvironmentInfo` provider~~ **RESOLVED** (added as callable stub)
  - ~~`outputs` parameter~~ **RESOLVED** (added to rule())
  - ~~`executable`/`test` params~~ **RESOLVED** (added to rule())
  - ~~`testing` module~~ **RESOLVED** (added with TestEnvironment method)

### Resolved Issues

1. ~~**CcToolchainConfigInfoProvider should not exist**~~ Removed from cc_common.rs
2. ~~**DebugPackageInfo, CcSharedLibraryInfo, CcInfo should be None**~~ Changed to `NoneType`
3. ~~**ProtoInfo should be None**~~ Changed to `NoneType` in proto_common.rs

### Architecture Note - Native vs Starlark

The following **must remain in native Rust code**:

1. **None placeholders** (`CcInfo`, `DebugPackageInfo`, `ProtoInfo` as `NoneType`)
   - Code checks `if CcInfo == None` during early loading
   - Prelude injection happens after base globals are established

2. **Action primitives** (functions that create build actions)
   - `cc_common.internal_DO_NOT_USE().create_cc_compile_action`
   - `proto_common.compile()`

3. **Artifact handling** (functions that create/manipulate artifacts)

Everything else should preferably be implemented in Starlark in `prelude/bazel_compat/`.

See [04-prelude-architecture.md](./04-prelude-architecture.md) for detailed architecture explanation.

### Key Version Requirement

- Use `rules_cc` version **0.2.16** for testing (Bazel 9.0 compatible)
- `native.bazel_version` must return "9.0.0" (no suffix) for version comparison
- Version checks like `_bazel_version_ge("9.0.0-pre.20250911")` must return True

---

## Implementation Philosophy: Starlark-First

Following Buck2's core philosophy, Bazel compatibility modules should be implemented in Starlark wherever possible, with only the minimum necessary primitives in native Rust.

### Decision Framework

When implementing a Bazel module, ask:

1. **Does it require build system internals?** (action creation, artifact handling, DICE integration)
   - YES -> Native Rust required
   - NO -> Prefer Starlark

2. **Is it checked before prelude loads?** (e.g., `if CcInfo == None`)
   - YES -> Native placeholder required
   - NO -> Can be Starlark

3. **Is it language/platform specific?**
   - YES -> Strong preference for Starlark
   - NO -> Evaluate case-by-case

### Native vs Starlark Mapping

| Requirement | Implementation |
|-------------|---------------|
| Action primitives (compile, link) | Native |
| Provider placeholders checked early | Native `NoneType` |
| Type constants (platform names) | Starlark |
| Simple provider wrappers | Starlark |
| Configuration structs | Starlark |
| Language-specific utilities | Starlark (in prelude) |
