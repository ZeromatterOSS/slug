# bzlmod Active Phases

> **Parent Plan**: [Kuro Bazel-Compatible Build Tool](../2026-01-21-kuro-bazel-compatible-build-tool.md)
> **Completed Work**: [02-bzlmod-completed.md](./02-bzlmod-completed.md)
> **Future Work**: [02-bzlmod-future.md](./02-bzlmod-future.md)

This sub-plan covers the bzlmod module system. This file contains active work in progress.

---

## Quick Reference: Completed Phases

| Phase | Description | Key Learnings | Details |
|-------|-------------|---------------|---------|
| **4a** | Workspace Recognition | MODULE.bazel parsed via Starlark interpreter; `kuro_bzlmod/src/parser.rs` | [Link](./02-bzlmod-completed.md#phase-4a-bzlmod---workspace-recognition-) |
| **4b** | Local Dependencies | `local_path_override()` works; local modules integrated via cell system | [Link](./02-bzlmod-completed.md#phase-4b-bzlmod---local-dependencies-) |
| **4c** | BCR Integration | Modules fetched to `~/.cache/kuro/`; SRI integrity verification works | [Link](./02-bzlmod-completed.md#phase-4c-bzlmod---bcr-integration-) |
| **4d** | Resolution & Lockfile | MVS algorithm in `resolution.rs`; lockfile format compatible with Bazel 9.0 | [Link](./02-bzlmod-completed.md#phase-4d-bzlmod---resolution-and-lockfile-) |
| **6** | Starlark Migration | **BLOCKED**: Modules must stay native due to global injection architecture | [Link](./02-bzlmod-completed.md#phase-6-migrate-stubs-to-starlark-) |

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

- **@bazel_tools http.bzl/git.bzl**: Needs `repository_rule` and `repository_ctx` (Phase 5)
- **Module extensions**: Parsing complete, synthetic repo workaround implemented, full execution not implemented
- **rules_cc loading**: **COMPLETE** - rules_cc now loads successfully!
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

1. ~~**CcToolchainConfigInfoProvider should not exist**~~ ✅ Removed from cc_common.rs
2. ~~**DebugPackageInfo, CcSharedLibraryInfo, CcInfo should be None**~~ ✅ Changed to `NoneType`
3. ~~**ProtoInfo should be None**~~ ✅ Changed to `NoneType` in proto_common.rs

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

See [06-prelude-architecture.md](./06-prelude-architecture.md) for detailed architecture explanation.

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
   - YES → Native Rust required
   - NO → Prefer Starlark

2. **Is it checked before prelude loads?** (e.g., `if CcInfo == None`)
   - YES → Native placeholder required
   - NO → Can be Starlark

3. **Is it language/platform specific?**
   - YES → Strong preference for Starlark
   - NO → Evaluate case-by-case

### Native vs Starlark Mapping

| Requirement | Implementation |
|-------------|---------------|
| Action primitives (compile, link) | Native |
| Provider placeholders checked early | Native `NoneType` |
| Type constants (platform names) | Starlark |
| Simple provider wrappers | Starlark |
| Configuration structs | Starlark |
| Language-specific utilities | Starlark (in prelude) |

---

## Phase 5: Module Extensions (In Progress)

### Overview

Implement module extensions which allow custom dependency resolution logic. This phase is split into two parts:

- **Phase 5a (Complete)**: Parse `use_extension()` and collect tags from MODULE.bazel files
- **Phase 5b (Remaining)**: Execute extensions and generate repositories

### Current Implementation Status

**Completed (Phase 5a):**

- `use_extension()` parsing in `kuro_bzlmod/src/globals.rs:614-644`
- `ExtensionProxy` Starlark value for capturing tag method calls (`globals.rs:97-146`)
- `ExtensionTagInvoker` for recording tag invocations (`globals.rs:148-193`)
- `use_repo()` for importing generated repositories (`globals.rs:657-688`)
- Extension data types: `ExtensionUsage`, `ExtensionTag`, `TagValue`, `UseRepo` (`types.rs:317-479`)
- Extension aggregation: `AggregatedExtension`, `aggregate_extensions()` (`extensions.rs:74-155`)
- Placeholder types for execution: `ExtensionResult`, `GeneratedRepo`, `ModuleInfo` (`extensions.rs:158-221`)

**Remaining (Phase 5b):**

- `.bzl` file loading for extensions
- `module_extension()` Starlark global
- `module_ctx` Starlark object
- Repository rule invocation
- Extension execution engine
- Lockfile integration for caching

### Bazel Source References

| Feature                    | Bazel Source File                                                                           |
| -------------------------- | ------------------------------------------------------------------------------------------- |
| Extension definition       | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleExtension.java`             |
| `module_extension()` API   | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleExtensionApi.java`          |
| `use_extension()` handling | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileGlobals.java`           |
| Tag classes                | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/TagClass.java`                    |
| Extension evaluation       | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/SingleExtensionEvalFunction.java` |
| `module_ctx` object        | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleExtensionContext.java`      |
| Extension lockfile         | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/LockFileModuleExtension.java`     |

**Key tests:** `src/test/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleExtensionResolutionTest.java`

**Real-world examples:** Study how rules_python implements `pip.parse()` in the rules_python repository.

### Changes Required

#### 1. Extension Definition Parsing (module_extension global)

**File**: New `kuro_bzlmod/src/module_extension.rs`

```python
# In extensions.bzl
my_ext = module_extension(
    implementation = _my_ext_impl,
    tag_classes = {
        "install": tag_class(attrs = {"name": attr.string()}),
    },
    os_dependent = False,
    arch_dependent = False,
)
```

#### 2. module_ctx Starlark Object (Critical)

**File**: New `kuro_bzlmod/src/module_ctx.rs`

**Data Access Properties:**
```python
module_ctx.modules          # list[bazel_module] - All modules using this extension
module_ctx.os               # repository_os - System info (name, arch, environ)
module_ctx.root_module_has_non_dev_dependency  # bool
```

**File I/O Methods:**
```python
module_ctx.read(path, *, watch='auto')
module_ctx.file(path, content='', executable=True)
module_ctx.extract(archive, output='', strip_prefix='')
module_ctx.watch(path)
```

**Network Operations:**
```python
module_ctx.download(url, output='', sha256='', integrity='', ...)
module_ctx.download_and_extract(url, output='', sha256='', strip_prefix='', ...)
```

**Execution & System:**
```python
module_ctx.execute(arguments, timeout=600, environment={}, quiet=True)
module_ctx.which(program)
module_ctx.getenv(name, default=None)
module_ctx.path(path)
```

#### 3. Extension Execution Engine

**File**: `kuro_bzlmod/src/execution.rs`

Execute all extensions and return generated repositories:
1. Load the extension's .bzl file
2. Get the module_extension definition
3. Build module_ctx from aggregated data
4. Call the implementation function
5. Collect generated repositories

#### 4. Repository Rule Invocation

Extensions call repository rules to create repositories:
- `http_archive()` - Download and extract archives
- `http_file()` - Download single files
- `git_repository()` - Clone git repos
- `new_local_repository()` - Create repo from local path
- Custom repository rules defined in .bzl files

### Success Criteria

#### Automated Verification:

- [x] `use_extension()` parses correctly
- [x] Extension tags collected from all using modules
- [ ] `module_extension()` global available in .bzl files
- [ ] `tag_class()` global available with attrs parameter
- [ ] `module_ctx.modules` returns correct bazel_module list
- [ ] `module_ctx.download()` fetches files with integrity verification
- [ ] `module_ctx.execute()` runs commands and returns output
- [ ] Extension implementation function executes successfully
- [ ] Generated repositories are accessible via @repo_name
- [ ] Extension results cached in lockfile
- [ ] Lockfile cache hit skips re-execution

#### Manual Verification:

- [ ] Simple extension creating a filegroup works
- [ ] Extension that downloads a file works
- [ ] Extension that executes a command works
- [ ] rules_python's `pip.parse()` extension works (stretch goal)

---

## Phase 5b: bzlmod Build Integration (In Progress)

### Overview

Bridge the gap between bzlmod module resolution and Kuro's build system. This phase makes resolved modules available as build targets via `@module_name//:target` syntax.

**Why this phase is critical:** Phases 4a-4d implement the bzlmod parsing, resolution, and fetching infrastructure. However, this infrastructure is currently standalone - resolved modules are not connected to Kuro's cell/repository system.

### Current State

**What exists:**

- `kuro_bzlmod` crate parses MODULE.bazel and resolves dependencies
- BCR client fetches and extracts remote modules to `~/.cache/kuro/`
- Local path overrides are parsed and validated
- Resolution produces `ResolvedGraph` with all module metadata

**What's missing:**

- Resolved modules are not registered as Kuro cells
- `@module_name` labels don't resolve to fetched module paths
- No integration between bzlmod resolver and `BuckConfigBasedCells`

### Future Work: Remove `.buckconfig` Requirement

**Current state:** Pure bzlmod projects still require a `.buckconfig` file with:
- Root cell definition
- Cell aliases to prevent errors from external configs
- `.buckroot` marker file

**Goal:** Projects with `MODULE.bazel` should work without any Buck-specific configuration files.

### Kuro Cell System Integration Points

| Component                       | File                                                      | Purpose                                            |
| ------------------------------- | --------------------------------------------------------- | -------------------------------------------------- |
| `CellResolver`                  | `app/kuro_core/src/cells.rs:211-459`                      | Global registry mapping cell names to paths        |
| `CellsAggregator`               | `app/kuro_common/src/legacy_configs/aggregator.rs:45-159` | Collects cell definitions from all sources         |
| `BuckConfigBasedCells`          | `app/kuro_common/src/legacy_configs/cells.rs:252-434`     | Parses cell config, already has bzlmod stub        |
| `ExternalCellOrigin`            | `app/kuro_core/src/cells/external.rs:22-75`               | Tracks external cell sources (git, bundled, local) |
| `resolve_bzlmod_dependencies()` | `app/kuro_common/src/legacy_configs/cells.rs:446-563`     | Existing stub for bzlmod integration               |

### Success Criteria

#### Automated Verification:

- [x] `@bazel_skylib//:defs.bzl` loads successfully after bzlmod resolution
- [x] `@rules_cc//cc:defs.bzl` loads after fetching from BCR - **COMPLETE**
- [ ] `@local_module//:target` works with local_path_override
- [x] Repo aliasing works: `bazel_dep(name="foo", repo_name="bar")` makes `@bar` available
- [x] Transitive repo_name aliases created via `collect_transitive_repo_aliases()`
- [ ] Extension-generated repos accessible via `@repo_name//:target`
- [ ] DICE caches bzlmod resolution (no re-resolution on second build)
- [x] Cell resolver includes all bzlmod modules
- [x] MVS algorithm discovers and fetches ALL transitive dependencies

#### Infrastructure Implementation (Complete):

- [x] `ExternalCellOrigin::Bzlmod` variant added (`app/kuro_core/src/cells/external.rs`)
- [x] `BzlmodCellSetup` struct with module_name, version, registry_url, source_path
- [x] `resolve_bzlmod_dependencies()` returns external origin for remote modules
- [x] Remote BCR modules marked as external cells via `aggregator.mark_external_cell()`
- [x] `kuro_external_cells` bzlmod module with `get_file_ops_delegate` and `copy_to_destination`
- [x] `buck_out_path.rs` handles `Bzlmod` variant in `resolve_external_cell_source`
- [x] External cell expansion copies from cache to project `bazel-external/` directory
- [x] MODULE.bazel dialect supports variable assignments (`enable_top_level_stmt: true`)

#### Remaining Infrastructure:

- [x] **`@bazel_tools` built-in repository** - See **Phase 5c** (COMPLETE)
- [x] **Version compatibility via `native.bazel_version`** - COMPLETE
- [x] **ProtoInfo built-in provider** - COMPLETE (returns NoneType per Bazel 8+ behavior)
- [x] **`aspect()` built-in** - See **[08-aspects.md](./08-aspects.md)** (Phase 8a COMPLETE)
- [x] **`allow_empty` parameter for attr.label_list()** - COMPLETE
- [x] **`PackageSpecificationInfo` provider** - COMPLETE (added as NoneType in cc_common.rs)

#### Manual Verification:

**Note**: Use `rules_cc` version **0.2.16** for testing.

- [ ] Create project with `bazel_dep(name = "rules_cc", version = "0.2.16")`
- [ ] Successfully load `@rules_cc//cc:defs.bzl`
- [ ] Build a simple C++ target using `cc_library` and `cc_binary`
- [ ] Verify `native.bazel_version` returns >= "9.0.0"
- [ ] Verify `bazel_features` version checks work correctly
- [ ] Verify cache hit on second build (no network activity)

---

## Phase 5c: Bundle @bazel_tools Repository (In Progress)

### Overview

Bundle the `@bazel_tools` repository from Bazel's source and make it automatically available to all bzlmod projects.

**Why this phase is critical:** Many BCR modules load from `@bazel_tools`:
- `rules_cc` loads `@bazel_tools//tools/cpp:toolchain_utils.bzl`
- Module extensions use `@bazel_tools//tools/build_defs/repo:http.bzl` for `http_archive`
- `bazel_features` uses `@bazel_tools` for version detection

### Source

Copy the `tools/` directory from Bazel repository HEAD:
- **Repository**: https://github.com/bazelbuild/bazel
- **Directory**: `tools/`
- **Destination**: `bazel_tools/` in Kuro source tree

### Key Directories to Include

| Directory                      | Purpose                                         | Priority |
| ------------------------------ | ----------------------------------------------- | -------- |
| `tools/build_defs/repo/`       | Repository rules (http_archive, git_repository) | Critical |
| `tools/cpp/`                   | C++ toolchain utilities                         | Critical |
| `tools/build_defs/build_info/` | Build info utilities                            | Medium   |
| `tools/osx/`                   | macOS toolchain                                 | Medium   |
| `tools/sh/`                    | Shell utilities                                 | Low      |

### Directory Structure

```
kuro/
├── bazel_tools/              # Copied from Bazel via scripts/sync_bazel_tools.sh
│   ├── tools/
│   │   ├── build_defs/repo/  # http.bzl, git.bzl, etc.
│   │   ├── cpp/              # toolchain_utils.bzl, etc.
│   │   └── ...
│   ├── MODULE.bazel
│   └── .buckconfig
├── prelude/
├── scripts/
│   └── sync_bazel_tools.sh
└── app/kuro_external_cells_bundled/
```

### Success Criteria

#### Automated Verification:

- [x] `bazel_tools/` directory exists with tools from Bazel 9.0.0
- [x] `kuro_external_cells_bundled` builds successfully with bazel_tools (3 tests passing)
- [x] `@bazel_tools` cell automatically registered for bzlmod projects
- [x] `load("@bazel_tools//tools/build_defs/repo:cache.bzl", ...)` succeeds
- [ ] `load("@bazel_tools//tools/cpp:toolchain_utils.bzl", ...)` succeeds
    - **Blocker**: File found but loads `@rules_cc` which isn't available in bazel_tools context
- [ ] `load("@bazel_tools//tools/build_defs/repo:http.bzl", ...)` succeeds
    - **Blocker**: Requires `repository_rule` Starlark global (Phase 5)

#### Manual Verification:

- [x] Create bzlmod project without explicit bazel_tools configuration
- [x] Verify `@bazel_tools` is available via `kuro audit cell`
- [ ] Load a .bzl file from rules_cc that depends on @bazel_tools
- [x] Build binary size increase is reasonable (~2MB for bazel_tools)

### Future Work: Bazel-Specific Starlark APIs

| API                           | Used In                       | Purpose                    | Status      |
| ----------------------------- | ----------------------------- | -------------------------- | ----------- |
| `visibility("public")`        | `cache.bzl`, `http.bzl`, etc. | Package visibility control | Implemented |
| `repository_rule`             | `http.bzl`, `git.bzl`         | Repository rule definition | Phase 5     |
| `repository_ctx` methods      | `http.bzl`, `git.bzl`         | Repository rule context    | Phase 5     |
| Module-level `config_setting` | Various BUILD files           | Configuration transitions  | Future      |

### Future: Visibility Enforcement (Research Task)

The current `visibility()` implementation is a no-op stub. Before implementing enforcement, research is needed:

**Research Questions:**
1. How does Bazel's `visibility()` interact with `load()` statements?
2. What happens when loading a `visibility("private")` file from another package?
3. How do package specifications like `"//foo:__subpackages__"` work?
4. Does visibility apply at file level or symbol level?

**References:**
- Bazel source: `src/main/java/com/google/devtools/build/lib/packages/BzlVisibility.java`
- Bazel docs: https://bazel.build/rules/lib/globals/bzl#visibility
- Test cases: `BzlVisibilityTest.java`
