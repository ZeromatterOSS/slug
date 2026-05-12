# bzlmod Phase 5b: Build Integration

> **Main Plan**: [02-bzlmod.md](./02-bzlmod.md)
> **Parent Phase**: [Phase 5 Overview](./02-bzlmod-phase-5-overview.md)

## Overview

Bridge the gap between bzlmod module resolution and Slug's build system. This phase makes resolved modules available as build targets via `@module_name//:target` syntax.

**Why this phase is critical:** Phases 4a-4d implement the bzlmod parsing, resolution, and fetching infrastructure. However, this infrastructure is currently standalone - resolved modules are not connected to Slug's cell/repository system.

---

## Current State

**What exists:**

- `slug_bzlmod` crate parses MODULE.bazel and resolves dependencies
- BCR client fetches and extracts remote modules to `~/.cache/slug/`
- Local path overrides are parsed and validated
- Resolution produces `ResolvedGraph` with all module metadata

**What's missing:**

- Resolved modules are not registered as Slug cells
- `@module_name` labels don't resolve to fetched module paths
- No integration between bzlmod resolver and `BuckConfigBasedCells`

---

## Future Work: Remove `.buckconfig` Requirement

**Current state:** Pure bzlmod projects still require a `.buckconfig` file with:
- Root cell definition
- Cell aliases to prevent errors from external configs
- `.buckroot` marker file

**Goal:** Projects with `MODULE.bazel` should work without any Buck-specific configuration files.

---

## Slug Cell System Integration Points

| Component                       | File                                                      | Purpose                                            |
| ------------------------------- | --------------------------------------------------------- | -------------------------------------------------- |
| `CellResolver`                  | `app/slug_core/src/cells.rs:211-459`                      | Global registry mapping cell names to paths        |
| `CellsAggregator`               | `app/slug_common/src/legacy_configs/aggregator.rs:45-159` | Collects cell definitions from all sources         |
| `BuckConfigBasedCells`          | `app/slug_common/src/legacy_configs/cells.rs:252-434`     | Parses cell config, already has bzlmod stub        |
| `ExternalCellOrigin`            | `app/slug_core/src/cells/external.rs:22-75`               | Tracks external cell sources (git, bundled, local) |
| `resolve_bzlmod_dependencies()` | `app/slug_common/src/legacy_configs/cells.rs:446-563`     | Existing stub for bzlmod integration               |

---

## Success Criteria

### Automated Verification

- [x] `@bazel_skylib//:defs.bzl` loads successfully after bzlmod resolution
- [x] `@rules_cc//cc:defs.bzl` loads after fetching from BCR - **COMPLETE**
- [x] `@local_module//:target` works with local_path_override (2026-03-05: verified via test_local_path_override.py - 3/3 tests pass)
- [x] Repo aliasing works: `bazel_dep(name="foo", repo_name="bar")` makes `@bar` available
- [x] Transitive repo_name aliases created via `collect_transitive_repo_aliases()`
- [ ] Extension-generated repos accessible via `@repo_name//:target`
- [ ] DICE caches bzlmod resolution (no re-resolution on second build)
- [x] Cell resolver includes all bzlmod modules
- [x] MVS algorithm discovers and fetches ALL transitive dependencies

### Infrastructure Implementation (Complete)

- [x] `ExternalCellOrigin::Bzlmod` variant added (`app/slug_core/src/cells/external.rs`)
- [x] `BzlmodCellSetup` struct with module_name, version, registry_url, source_path
- [x] `resolve_bzlmod_dependencies()` returns external origin for remote modules
- [x] Remote BCR modules marked as external cells via `aggregator.mark_external_cell()`
- [x] `slug_external_cells` bzlmod module with `get_file_ops_delegate` and `copy_to_destination`
- [x] `buck_out_path.rs` handles `Bzlmod` variant in `resolve_external_cell_source`
- [x] External cell expansion copies from cache to project `bazel-external/` directory
- [x] MODULE.bazel dialect supports variable assignments (`enable_top_level_stmt: true`)

### Remaining Infrastructure

- [x] **`@bazel_tools` built-in repository** - See **Phase 5c** (COMPLETE)
- [x] **Version compatibility via `native.bazel_version`** - COMPLETE
- [x] **ProtoInfo built-in provider** - COMPLETE (returns NoneType per Bazel 8+ behavior)
- [x] **`aspect()` built-in** - See **[06-aspects.md](./06-aspects.md)** (Phase 8a COMPLETE)
- [x] **`allow_empty` parameter for attr.label_list()** - COMPLETE
- [x] **`PackageSpecificationInfo` provider** - COMPLETE (added as NoneType in cc_common.rs)

### Manual Verification

**Note**: Use `rules_cc` version **0.2.16** for testing.

- [ ] Create project with `bazel_dep(name = "rules_cc", version = "0.2.16")`
- [ ] Successfully load `@rules_cc//cc:defs.bzl`
- [ ] Build a simple C++ target using `cc_library` and `cc_binary`
- [ ] Verify `native.bazel_version` returns >= "9.0.0"
- [ ] Verify `bazel_features` version checks work correctly
- [ ] Verify cache hit on second build (no network activity)
