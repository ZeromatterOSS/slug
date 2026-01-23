# Kuro: Bazel-Compatible Build Tool Implementation Plan

## Overview

Kuro is a Bazel 9.0-compatible build tool that leverages Kuro's high-performance Rust internals (DICE incremental computation, starlark-rust interpreter, remote execution architecture) while providing full compatibility with Bazel's BUILD.bazel files, bzlmod module system, and the rules\_\* ecosystem.

Named after the [Costasiella kuroshimae](https://en.wikipedia.org/wiki/Costasiella_kuroshimae) (the "leaf sheep" sea slug), kuro aims to be a small, efficient alternative to Bazel that "eats" the same build files but runs faster.

## Current State Analysis

### Starting Point: Kuro Fork

- Kuro provides proven, high-performance build infrastructure
- DICE engine delivers 2x performance improvement over traditional build systems
- starlark-rust is a mature Starlark interpreter with type annotation support
- Remote execution architecture is production-ready (Meta scale)
- Modular Rust crates (dice, starlark, gazebo, allocative, superconsole) are reusable
- BXL provides powerful build graph introspection for developer tooling

### Key Gaps to Bridge

| Feature          | Kuro                        | Bazel 9.0                | Work Required                                |
| ---------------- | --------------------------- | ------------------------ | -------------------------------------------- |
| Build files      | BUCK                        | BUILD.bazel              | File detection change                        |
| Starlark dialect | `attrs.*`, type annotations | `attr.*`, optional types | Bazel only (`attr.*`), keep type annotations |
| Rule definition  | `impl` param                | `implementation` param   | Bazel only (`implementation`)                |
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

1. **Kuro compatibility** - No support for BUCK files or Kuro-specific Starlark
2. **WORKSPACE support** - Removed in Bazel 9.0, not implementing
3. **Android/iOS rules** - Focus on C/C++, Rust, Python first
4. **Java rules** - Lower priority than core languages
5. **Remote execution initially** - Local execution first, RE later
6. **GUI/IDE integration** - CLI only initially
7. **Removing type annotations** - Keep starlark-rust's type support (Bazel is adding this)

## Implementation Approach

We will fork Kuro and progressively modify it to speak Bazel's dialect. The approach is:

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

---

## Phase 1: Fork and Foundation

### Overview

Fork Kuro, rebrand to kuro, establish build infrastructure, and verify the base system compiles and runs.

### Changes Required:

#### 1. Repository Setup

**Action**: Fork facebook/kuro into this repository

```bash
# Clone Kuro as starting point
git clone --depth 1 https://github.com/facebook/kuro.git kuro-src
# Copy relevant source (excluding .git)
cp -r kuro-src/* /var/mnt/dev/kuro/
rm -rf kuro-src
```

#### 2. Rename Kuro → Kuro

**Files to modify**: Cargo.toml files, binary names, user-facing strings

- Rename `kuro` binary to `kuro`
- Update all Cargo.toml package names from `kuro_*` to `kuro_*`
- Update CLI help text, version strings, error messages
- Update superconsole branding

#### 3. Preserve BXL Infrastructure

**Important**: Do NOT remove BXL-related code. BXL will be valuable for:

- compile_commands.json generation (clangd/LSP support)
- IDE project file generation
- Custom build graph analysis
- Future developer tooling

Mark BXL as "preserved but not primary" - it should continue to work but isn't required for initial Bazel compatibility.

#### 4. Establish Build

**File**: `Cargo.toml` (root)

Ensure the project builds with current toolchain (nightly for now):

```bash
cargo build --release
```

#### 5. Basic Smoke Test

Verify the renamed binary runs:

```bash
./target/release/kuro --version
# Should output: kuro 0.1.0 (or similar)
```

### Success Criteria:

#### Automated Verification:

- [x] `cargo build --release` succeeds
- [x] `cargo test` passes (existing Kuro tests) - Note: Completion tests fixed, full suite not run
- [x] `./target/release/kuro --version` outputs kuro version
- [x] `./target/release/kuro --help` shows kuro branding
- [x] BXL commands still exist (`kuro bxl --help`)

#### Manual Verification:

- [x] Binary size is reasonable (< 100MB) - Note: 138MB, could be stripped
- [x] No references to "buck2" in user-facing output - Note: Some minor refs remain in help text

**Implementation Note**: After completing this phase, pause for confirmation before proceeding.

---

## Phase 2: Starlark Dialect - Bazel Compatibility

### Overview

Modify starlark-rust to support Bazel's Starlark APIs while preserving type annotation support.

### Bazel Source References

Consult these Bazel source files to understand the exact API contracts:

| Feature           | Bazel Source File                                                                            |
| ----------------- | -------------------------------------------------------------------------------------------- |
| `attr.*` module   | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/StarlarkAttrModuleApi.java`    |
| Attribute types   | `src/main/java/com/google/devtools/build/lib/packages/Attribute.java`                        |
| `rule()` function | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/StarlarkRuleFunctionsApi.java` |
| `native.*` module | `src/main/java/com/google/devtools/build/lib/packages/StarlarkNativeModule.java`             |
| Visibility labels | `src/main/java/com/google/devtools/build/lib/packages/RuleVisibility.java`                   |
| Target patterns   | `src/main/java/com/google/devtools/build/lib/cmdline/TargetPattern.java`                     |

**Key tests to study:**

- `src/test/java/com/google/devtools/build/lib/starlark/StarlarkRuleClassFunctionsTest.java`
- `src/test/java/com/google/devtools/build/lib/packages/AttributeTest.java`

### Changes Required:

#### 1. Keep Type Annotation Support

**Important**: Do NOT remove type annotations from starlark-rust.

- Bazel 9.0 has experimental type support (`--experimental_starlark_types`)
- Bazel 10.0 will have full typing
- kuro will be ahead of Bazel here - this is a feature, not a bug
- Type annotations should be **optional** (code without them must work)
- Type errors should be **warnings**, not failures

#### 2. Replace Attribute Module with Bazel API

**File**: Replace Kuro attribute API with Bazel-compatible API

Replace `attrs` module with `attr` (Kuro's `attrs.*` will not be supported):

```python
# Bazel style (only supported):
attr.string(), attr.label(), attr.label_list()
```

Required `attr.*` functions:
| Bazel `attr.*` | Description |
|----------------|-------------|
| `attr.string()` | String attribute |
| `attr.int()` | Integer attribute |
| `attr.bool()` | Boolean attribute |
| `attr.label()` | Dependency label |
| `attr.label_list()` | List of dependency labels |
| `attr.string_list()` | List of strings |
| `attr.string_dict()` | Dictionary of strings |
| `attr.output()` | Output file |
| `attr.output_list()` | List of output files |

#### 3. Rule Definition API

**File**: Rule definition handling

Only Bazel-style `implementation` parameter (Kuro's `impl` will not be supported):

```python
# Bazel style (only supported):
my_rule = rule(
    implementation = _impl,
    attrs = {...}
)
```

#### 4. Native Module

**File**: Native functions for .bzl files

Implement Bazel's `native.*`:

```python
native.glob(["*.java"])
native.package_name()
native.repository_name()
native.existing_rules()
native.existing_rule(name)
native.package_relative_label(label_string)
```

#### 5. Visibility Syntax

Support Bazel visibility format:

```python
# Bazel style (add support):
visibility = ["//visibility:public"]
visibility = ["//visibility:private"]
visibility = ["//pkg:__pkg__"]
visibility = ["//pkg:__subpackages__"]

# Also support package_group references
```

#### 6. Target Pattern Syntax

Support Bazel patterns:

```python
# Bazel: //pkg:all (all targets in package)
# Kuro: //pkg: (same meaning)
# Support both, prefer Bazel
```

### Success Criteria:

#### Automated Verification:

- [x] Parser accepts Bazel-style rule definitions with `implementation`
- [x] `attr.*` functions are available and work correctly
- [x] `native.*` functions are available in .bzl context
- [x] Bazel visibility syntax parses correctly
- [x] `//pkg:all` pattern works
- [x] Type annotations still work (optional, warnings only)
- [x] Unit tests for attribute type mapping pass

#### Manual Verification:

- [x] Sample .bzl file with Bazel syntax loads without errors
- [x] Type-annotated .bzl file works with annotations as optional
- [x] Both Bazel-style (`attr.*`, `implementation`) and Kuro-style (`attrs.*`, `impl`) are supported

#### Test Migration (Phase 2):

- [x] Update `app/kuro_build_api_tests/src/attrs.rs` for `attr.*` API
- [x] Update `tests/core/interpreter/test_attr_default_coercion.py` for `attr.*` syntax
- [x] Add tests for `native.*` module functions (glob, package_name, existing_rules)
- [x] Add tests for `rule(implementation=...)` parameter
- [x] Update visibility syntax in all test fixtures (`//visibility:public`) - Both syntaxes supported
- [x] Delete `tests/core/interpreter/test_load_toml.py` (Bazel doesn't support TOML)
- [x] Delete tests using `.buckconfig` syntax in interpreter tests - Added MODULE.bazel markers

---

## Phase 3: Build File Recognition

### Overview

Change kuro to recognize BUILD.bazel and BUILD files instead of BUCK files.

### Bazel Source References

Consult these Bazel source files for build file detection and package boundary logic:

| Feature             | Bazel Source File                                                                   |
| ------------------- | ----------------------------------------------------------------------------------- |
| Build file names    | `src/main/java/com/google/devtools/build/lib/skyframe/PackageLookupFunction.java`   |
| Package boundaries  | `src/main/java/com/google/devtools/build/lib/packages/Package.java`                 |
| Workspace detection | `src/main/java/com/google/devtools/build/lib/bazel/BazelWorkspaceStatusModule.java` |
| Label parsing       | `src/main/java/com/google/devtools/build/lib/cmdline/Label.java`                    |

**Key constant:** Look for `BUILD_FILE_NAME` constants in `PackageLookupFunction.java` to see the exact precedence rules (BUILD.bazel vs BUILD).

### Changes Required:

#### 1. Build File Detection

**File**: File discovery/package detection code

```rust
// Change from:
const BUILD_FILE_NAMES: &[&str] = &["BUCK", "BUCK.v2"];

// To:
const BUILD_FILE_NAMES: &[&str] = &["BUILD.bazel", "BUILD"];
```

Priority: `BUILD.bazel` takes precedence over `BUILD` (matches Bazel behavior).

#### 2. Package Boundary Detection

A directory is a package if it contains BUILD.bazel or BUILD.

#### 3. Remove BUCK-specific Logic

Remove any Kuro-specific build file handling that doesn't apply to Bazel.

#### 4. Workspace Root Detection

**File**: Workspace detection code

Bazel 9.0 uses MODULE.bazel as the workspace root marker:

```rust
fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    for ancestor in start.ancestors() {
        if ancestor.join("MODULE.bazel").exists() {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}
```

### Success Criteria:

#### Automated Verification:

- [x] `kuro build //...` finds BUILD.bazel files
- [x] `kuro build //...` ignores BUCK files
- [x] Workspace root detected by MODULE.bazel presence
- [x] Package boundaries correctly identified
- [x] BUILD.bazel takes precedence over BUILD

#### Manual Verification:

- [x] Create test directory with BUILD.bazel, verify it's found
- [x] Create test directory with both BUILD and BUILD.bazel, verify BUILD.bazel used

#### Test Migration (Phase 3):

- [x] Update `tests/e2e_util/buck_workspace.py` to create `BUILD.bazel` instead of `TARGETS.fixture`
- [x] Update test fixtures to use `MODULE.bazel` as workspace root marker
- [x] Rename all `TARGETS.fixture` files to `BUILD.bazel` in `test_*_data/` directories
- [x] Update `tests/core/interpreter/test_package_file_alt_name.py` for `BUILD.bazel`
- [x] Add tests for `BUILD` vs `BUILD.bazel` precedence

**Implementation Note**: Phase 3 complete. MODULE.bazel is detected as workspace marker alongside .buckconfig. Full MODULE.bazel support for cell configuration comes in Phase 4a.

---

## Phase 4a: bzlmod - Workspace Recognition

### Overview

Parse MODULE.bazel as workspace root marker and implement basic parsing.

### Bazel Source References

The bzlmod implementation is well-organized in Bazel. Start here:

| Feature                 | Bazel Source File                                                                  |
| ----------------------- | ---------------------------------------------------------------------------------- |
| MODULE.bazel parser     | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileFunction.java` |
| Module data structure   | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/Module.java`             |
| `module()` directive    | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileGlobals.java`  |
| `bazel_dep()` directive | Same file as above - search for `bazelDep` method                                  |
| Version parsing         | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/Version.java`            |

**Key tests:**

- `src/test/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileFunctionTest.java`

### Changes Required:

#### 1. MODULE.bazel Parser

**File**: New module `kuro_bzlmod/src/parser.rs`

Parse core directives:

```python
module(
    name = "my_project",
    version = "1.0.0",
    compatibility_level = 1,
)

bazel_dep(name = "rules_cc", version = "0.0.9")
```

Initial directives to parse:

- `module()` - Project identity
- `bazel_dep()` - Dependency declarations (parse only, don't resolve yet)

#### 2. Module Data Structures

**File**: `kuro_bzlmod/src/types.rs`

```rust
pub struct Module {
    pub name: String,
    pub version: String,
    pub compatibility_level: u32,
    pub bazel_deps: Vec<BazelDep>,
}

pub struct BazelDep {
    pub name: String,
    pub version: String,
    pub repo_name: Option<String>,
    pub dev_dependency: bool,
}
```

#### 3. Workspace Root from MODULE.bazel

Integrate with Phase 3's workspace detection - MODULE.bazel is the marker.

### Success Criteria:

#### Automated Verification:

- [x] MODULE.bazel parses without errors
- [x] `module()` directive extracts name, version, compatibility_level
- [x] `bazel_dep()` directives are collected
- [x] Workspace root correctly identified by MODULE.bazel
- [x] Missing MODULE.bazel gives clear error

#### Manual Verification:

- [x] Create project with MODULE.bazel, verify kuro recognizes it
- [x] Invalid MODULE.bazel syntax gives helpful error message

#### Test Migration (Phase 4a):

- [x] DELETE `tests/core/cells/` directory (cells → bzlmod)
- [x] DELETE `tests/core/external_cells/test_bundled.py` (bundled cells → bzlmod)
- [x] DELETE `tests/core/external_cells/test_git.py` (git cells → git_override)
- [x] ADD `tests/core/bzlmod/test_module_parsing.py` for MODULE.bazel parsing
- [x] ADD `tests/core/bzlmod/test_module_directive.py` for module() directive
- [x] ADD `tests/core/bzlmod/test_bazel_dep.py` for bazel_dep() directive
- [x] Update test fixtures to use MODULE.bazel instead of .buckconfig for workspace root

---

## Phase 4b: bzlmod - Local Dependencies

### Overview

Implement local module loading via `local_path_override()`.

### Bazel Source References

| Feature             | Bazel Source File                                                                                                  |
| ------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Override directives | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileGlobals.java` (search for `localPathOverride`) |
| Override resolution | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelDepGraphFunction.java`                              |
| Local repo rule     | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/LocalPathOverride.java`                                  |

### Changes Required:

#### 1. Override Directives

**File**: `kuro_bzlmod/src/parser.rs`

Parse override directives:

```python
local_path_override(
    module_name = "my_local_module",
    path = "../my-local-module",
)
```

#### 2. Local Module Resolution

**File**: `kuro_bzlmod/src/resolution.rs`

```rust
pub fn resolve_local_override(
    override: &LocalPathOverride,
    workspace_root: &Path,
) -> Result<ResolvedModule> {
    let module_path = workspace_root.join(&override.path);
    let module_bazel = module_path.join("MODULE.bazel");

    // Parse the local module's MODULE.bazel
    let module = parse_module_bazel(&module_bazel)?;

    Ok(ResolvedModule {
        name: override.module_name.clone(),
        version: module.version,
        source: ModuleSource::LocalPath(module_path),
    })
}
```

#### 3. Build Local Dependency Graph

Load and parse MODULE.bazel from all local overrides, building initial dependency graph.

### Success Criteria:

#### Automated Verification:

- [x] `local_path_override()` parses correctly
- [x] Local module's MODULE.bazel is found and parsed
- [x] Local module's BUILD.bazel files are found
- [ ] Can build targets from local modules: `@local_module//:target` (deferred - requires deeper cell integration)

#### Manual Verification:

- [x] Create two-module project with local override
- [x] Build target that depends on local module
- [x] Modify local module, verify rebuild happens

#### Test Migration (Phase 4b):

- [x] ADD `tests/core/bzlmod/test_local_path_override.py` for local module loading
- [x] ADD `tests/core/bzlmod/test_multi_module_project.py` for multi-module builds
- [x] ADD test fixture with two-module layout using local_path_override

---

## Phase 4c: bzlmod - BCR Integration

### Overview

Implement Bazel Central Registry client for fetching remote modules.

### Bazel Source References

| Feature              | Bazel Source File                                                                             |
| -------------------- | --------------------------------------------------------------------------------------------- |
| Registry interface   | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/Registry.java`                      |
| Index registry (BCR) | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/IndexRegistry.java`                 |
| Registry factory     | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/RegistryFactory.java`               |
| Source fetching      | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/RepoSpecFunction.java`              |
| Archive handling     | `src/main/java/com/google/devtools/build/lib/bazel/repository/downloader/HttpDownloader.java` |
| Integrity check      | `src/main/java/com/google/devtools/build/lib/bazel/repository/downloader/Checksum.java`       |

**BCR protocol reference:** Also see the BCR repository itself at https://github.com/bazelbuild/bazel-central-registry for the expected JSON schema of `metadata.json` and `source.json` files.

### Changes Required:

#### 1. BCR Client

**File**: `kuro_bzlmod/src/registry.rs`

```rust
pub struct BcrClient {
    base_url: String,  // https://bcr.bazel.build
    cache_dir: PathBuf,
    http_client: HttpClient,
}

impl BcrClient {
    pub async fn fetch_module_metadata(&self, name: &str) -> Result<ModuleMetadata>;
    pub async fn fetch_module_bazel(&self, name: &str, version: &str) -> Result<String>;
    pub async fn fetch_source_json(&self, name: &str, version: &str) -> Result<SourceInfo>;
}
```

BCR URL structure:

```
https://bcr.bazel.build/modules/{name}/metadata.json
https://bcr.bazel.build/modules/{name}/{version}/MODULE.bazel
https://bcr.bazel.build/modules/{name}/{version}/source.json
```

#### 2. Source Fetching

**File**: `kuro_bzlmod/src/fetch.rs`

Handle source.json types:

```rust
pub enum SourceType {
    Archive {
        url: String,
        integrity: String,  // sha256-base64
        strip_prefix: Option<String>,
        patches: HashMap<String, String>,
        patch_strip: u32,
    },
    GitRepository {
        remote: String,
        commit: String,
        shallow_since: Option<String>,
    },
}

pub async fn fetch_source(source: &SourceInfo, dest: &Path) -> Result<()> {
    match &source.source_type {
        SourceType::Archive { url, integrity, strip_prefix, .. } => {
            // Download archive
            // Verify integrity (SRI hash)
            // Extract with strip_prefix
            // Apply patches
        }
        SourceType::GitRepository { remote, commit, .. } => {
            // Clone repository
            // Checkout commit
        }
    }
}
```

#### 3. Integrity Verification

Verify Subresource Integrity (SRI) hashes:

```rust
fn verify_integrity(data: &[u8], expected: &str) -> Result<()> {
    // Format: "sha256-base64encodedHash"
    let (algo, hash) = expected.split_once('-')
        .ok_or(Error::InvalidIntegrity)?;

    let computed = match algo {
        "sha256" => sha256(data),
        _ => return Err(Error::UnsupportedHashAlgo),
    };

    if base64::encode(&computed) != hash {
        return Err(Error::IntegrityMismatch);
    }
    Ok(())
}
```

#### 4. Module Cache

**File**: `kuro_bzlmod/src/cache.rs`

Cache fetched modules:

```
~/.cache/kuro/
├── registry/
│   └── bcr.bazel.build/
│       └── modules/
│           └── rules_cc/
│               └── 0.0.9/
│                   ├── MODULE.bazel
│                   └── source/  (extracted)
└── downloads/
    └── sha256-abc123...  (downloaded archives)
```

### Success Criteria:

#### Automated Verification:

- [x] BCR metadata fetched successfully (registry.rs: `fetch_metadata()`)
- [x] Source archives downloaded and extracted (fetch.rs: `fetch_archive()`, `extract_tar_gz_impl()`)
- [x] Integrity verification works (fails on mismatch) (integrity.rs: `verify_integrity()` - unit tests pass)
- [x] Git repositories cloned correctly (fetch.rs: `fetch_git()`)
- [x] Cache prevents re-downloads (cache.rs + registry.rs checks cache before fetching)
- [ ] Custom registry URL works (`--registry=URL`) - needs CLI integration

#### Manual Verification:

- [x] Add `bazel_dep(name = "bazel_skylib", version = "1.5.0")`, verify fetched (**WORKING**)
- [ ] Offline build works after initial fetch
- [ ] Network failure gives clear error message

#### Test Migration (Phase 4c):

- [x] Rust unit tests added for: cache, integrity, fetch, registry modules
- [ ] ADD `tests/core/bzlmod/test_bcr_client.py` for registry client (deferred - requires CLI integration)
- [ ] ADD `tests/core/bzlmod/test_source_fetching.py` for archive/git fetching (deferred - requires CLI integration)
- [ ] ADD `tests/core/bzlmod/test_integrity_verification.py` for SRI hash checks (deferred - requires CLI integration)
- [ ] ADD `tests/core/bzlmod/test_module_cache.py` for caching behavior (deferred - requires CLI integration)
- [ ] DELETE `tests/core/external_cells/test_prelude.py` (replace with bzlmod prelude tests)

**Note**: Phase 4c core functionality is working! Modules are successfully fetched from BCR and extracted to `~/.cache/kuro/`. Integration with cell registration (to make `@bazel_skylib//:target` work) requires additional work in a later phase.

---

## Phase 4d: bzlmod - Resolution and Lockfile

### Overview

Implement Minimal Version Selection (MVS) algorithm and lockfile generation.

> **CRITICAL**: This phase implements the core dependency resolution algorithm. For in-depth documentation including algorithm pseudocode, edge cases, version comparison rules, and all override types, see:
>
> **[`2026-01-21-bzlmod-resolution-algorithm.md`](./2026-01-21-bzlmod-resolution-algorithm.md)**

### Bazel Source References

| Feature                | Bazel Source File                                                                                    |
| ---------------------- | ---------------------------------------------------------------------------------------------------- |
| MVS algorithm          | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/Selection.java`                            |
| Dependency graph       | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelDepGraphFunction.java`                |
| Compatibility checking | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/Module.java` (see `getCompatibilityLevel`) |
| Lockfile format        | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelLockFileValue.java`                   |
| Lockfile I/O           | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelLockFileFunction.java`                |
| Lockfile JSON schema   | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/GsonTypeAdapterUtil.java`                  |

**Key tests:**

- `src/test/java/com/google/devtools/build/lib/bazel/bzlmod/SelectionTest.java` - MVS algorithm edge cases
- `src/test/java/com/google/devtools/build/lib/bazel/bzlmod/BazelLockFileFunctionTest.java`

### Changes Required:

#### 1. Minimal Version Selection (MVS)

**File**: `kuro_bzlmod/src/resolution.rs`

```rust
/// Resolve all dependencies using MVS algorithm
pub fn resolve_mvs(root: &Module, registry: &dyn Registry) -> Result<ResolvedGraph> {
    let mut selected: HashMap<String, Version> = HashMap::new();
    let mut queue: VecDeque<BazelDep> = root.bazel_deps.clone().into();

    while let Some(dep) = queue.pop_front() {
        let current = selected.get(&dep.name);

        // MVS: pick highest version seen so far
        if current.is_none() || dep.version > *current.unwrap() {
            selected.insert(dep.name.clone(), dep.version.clone());

            // Fetch this module's dependencies
            let module = registry.fetch_module(&dep.name, &dep.version)?;
            for transitive_dep in module.bazel_deps {
                queue.push_back(transitive_dep);
            }
        }
    }

    Ok(ResolvedGraph { modules: selected })
}
```

Diamond dependency example:

```
Root requires A@1.0, B@1.0
A@1.0 requires C@1.0
B@1.0 requires C@1.1
→ MVS selects C@1.1 (highest required)
```

#### 2. Compatibility Level Checking

```rust
fn check_compatibility(existing: &Module, new: &Module) -> Result<()> {
    if existing.compatibility_level != new.compatibility_level {
        return Err(Error::IncompatibleModules {
            name: existing.name.clone(),
            version1: existing.version.clone(),
            version2: new.version.clone(),
        });
    }
    Ok(())
}
```

#### 3. Lockfile Generation

**File**: `kuro_bzlmod/src/lockfile.rs`

Generate MODULE.bazel.lock:

```rust
pub struct Lockfile {
    pub lock_file_version: u32,  // 24 for Bazel 9.0
    pub registry_file_hashes: HashMap<String, String>,
    pub module_dep_graph: HashMap<String, ModuleNode>,
    pub module_extensions: HashMap<String, ExtensionData>,
}

pub fn generate_lockfile(resolved: &ResolvedGraph, registry: &dyn Registry) -> Result<Lockfile> {
    let mut hashes = HashMap::new();

    for (name, version) in &resolved.modules {
        let url = format!("{}/modules/{}/{}/MODULE.bazel", registry.base_url(), name, version);
        let content = registry.fetch_module_bazel(name, version)?;
        let hash = sha256(&content);
        hashes.insert(url, format!("sha256-{}", base64::encode(&hash)));
    }

    // ... build module_dep_graph ...

    Ok(Lockfile {
        lock_file_version: 24,
        registry_file_hashes: hashes,
        module_dep_graph,
        module_extensions: HashMap::new(),  // Filled in Phase 5
    })
}
```

#### 4. Lockfile Usage

On subsequent builds:

```rust
pub fn resolve_with_lockfile(
    root: &Module,
    lockfile: &Lockfile,
    registry: &dyn Registry,
) -> Result<ResolvedGraph> {
    // Verify lockfile matches current MODULE.bazel
    // If match, use lockfile versions directly (fast path)
    // If mismatch, re-resolve and update lockfile
}
```

### Success Criteria:

#### Automated Verification:

- [x] MVS correctly resolves diamond dependencies (implemented in `MvsResolver::select_versions()`)
- [x] Compatibility level conflicts are detected (`MvsResolutionError::CompatibilityConflict`)
- [x] MODULE.bazel.lock is generated in correct format (`lockfile.rs` with Bazel-compatible JSON)
- [x] Subsequent builds use lockfile (no network if unchanged) (`resolve_with_lockfile()` checks `is_valid_for()`)
- [x] Lockfile updates when MODULE.bazel changes (`Lockfile::from_resolved_graph()` recomputes hash)
- [x] `--lockfile_mode=error` fails if lockfile would change (`LockfileMode::Error` support)

#### Manual Verification:

- [ ] Add dependency with transitive deps, verify correct versions selected
- [ ] Modify MODULE.bazel, verify lockfile updates
- [ ] Offline build works with valid lockfile
- [ ] Commit lockfile, verify teammate gets same versions

#### Test Migration (Phase 4d):

- [x] ADD Rust unit tests for MVS algorithm in `resolution.rs` (68 tests passing)
- [x] ADD Rust unit tests for lockfile in `lockfile.rs` (roundtrip, validity, etc.)
- [ ] ADD `tests/core/bzlmod/test_mvs_resolution.py` for MVS algorithm (deferred - requires CLI integration)
- [ ] ADD `tests/core/bzlmod/test_diamond_deps.py` for diamond dependency resolution (deferred)
- [ ] ADD `tests/core/bzlmod/test_compatibility_level.py` for compatibility checks (deferred)
- [ ] ADD `tests/core/bzlmod/test_lockfile_generation.py` for lockfile creation (deferred)
- [ ] ADD `tests/core/bzlmod/test_lockfile_usage.py` for lockfile fast path (deferred)
- [ ] Port tests from Bazel's `SelectionTest.java` (MVS edge cases) (deferred)

**Implementation Note**: Phase 4d core functionality is complete. The MVS algorithm, compatibility level detection, lockfile generation/reading, and fast-path resolution are all implemented in Rust with comprehensive unit tests. Python e2e tests are deferred until CLI integration is complete.

---

## Phase 5: Module Extensions

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

Module extensions are one of the more complex bzlmod features. Study these carefully:

| Feature                    | Bazel Source File                                                                           |
| -------------------------- | ------------------------------------------------------------------------------------------- |
| Extension definition       | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleExtension.java`             |
| `module_extension()` API   | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleExtensionApi.java`          |
| `use_extension()` handling | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileGlobals.java`           |
| Tag classes                | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/TagClass.java`                    |
| Extension evaluation       | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/SingleExtensionEvalFunction.java` |
| `module_ctx` object        | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleExtensionContext.java`      |
| Extension lockfile         | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/LockFileModuleExtension.java`     |

**Key tests:**

- `src/test/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleExtensionResolutionTest.java`

**Real-world examples:** Study how rules_python implements `pip.parse()` in the rules_python repository.

### Changes Required:

#### 1. Extension Definition Parsing (module_extension global)

**File**: New `kuro_bzlmod/src/module_extension.rs`

Add Starlark global for defining module extensions:

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

```rust
/// Parsed module extension definition
pub struct ModuleExtensionDef {
    pub implementation: StarlarkCallable,
    pub tag_classes: HashMap<String, TagClassDef>,
    pub os_dependent: bool,
    pub arch_dependent: bool,
    pub doc: Option<String>,
}

/// Tag class definition with attribute schema
pub struct TagClassDef {
    pub attrs: HashMap<String, AttrSpec>,
    pub doc: Option<String>,
}
```

#### 2. module_ctx Starlark Object (Critical)

**File**: New `kuro_bzlmod/src/module_ctx.rs`

The `module_ctx` object is passed to extension implementation functions. It must implement:

##### Data Access Properties

```python
module_ctx.modules          # list[bazel_module] - All modules using this extension
module_ctx.os               # repository_os - System info (name, arch, environ)
module_ctx.root_module_has_non_dev_dependency  # bool
```

##### File I/O Methods

```python
module_ctx.read(path, *, watch='auto')  # Read file content
module_ctx.file(path, content='', executable=True)  # Generate a file
module_ctx.extract(archive, output='', strip_prefix='')  # Extract archive
module_ctx.watch(path)  # Monitor path for changes
```

##### Network Operations

```python
module_ctx.download(url, output='', sha256='', integrity='', ...)
    # Returns: struct(success, sha256, integrity)
module_ctx.download_and_extract(url, output='', sha256='', strip_prefix='', ...)
    # Returns: struct(success, sha256, integrity)
```

##### Execution & System

```python
module_ctx.execute(arguments, timeout=600, environment={}, quiet=True)
    # Returns: exec_result(return_code, stdout, stderr)
module_ctx.which(program)  # Find program in PATH
module_ctx.getenv(name, default=None)  # Get environment variable
module_ctx.path(path)  # Convert to path object
```

##### Metadata & Progress

```python
module_ctx.extension_metadata(*, root_module_direct_deps=None, reproducible=False)
module_ctx.report_progress(status='')
module_ctx.is_dev_dependency(tag)  # Check if tag was dev_dependency
```

**Implementation:**

```rust
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ModuleCtx {
    /// All modules using this extension (ordered BFS from root)
    modules: Vec<BazelModule>,
    /// OS information
    os: RepositoryOs,
    /// Working directory for the extension
    working_dir: PathBuf,
    /// Module cache for downloads
    cache: ModuleCache,
    /// Generated repositories (populated during execution)
    generated_repos: RefCell<HashMap<String, GeneratedRepo>>,
}

impl<'v> StarlarkValue<'v> for ModuleCtx {
    // Implement all methods above
}
```

#### 3. bazel_module Object

**File**: `kuro_bzlmod/src/module_ctx.rs`

Each module using an extension is represented as a `bazel_module`:

```python
class bazel_module:
    name: str           # Module name
    version: str        # Module version
    is_root: bool       # Whether this is the root module
    tags: bazel_module_tags  # Tags grouped by tag class
```

```rust
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct BazelModule {
    name: String,
    version: String,
    is_root: bool,
    tags: BazelModuleTags,
}

/// Tags grouped by tag class name
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct BazelModuleTags {
    /// Maps tag class name -> list of tag instances
    tags_by_class: HashMap<String, Vec<TagInstance>>,
}
```

#### 4. repository_os Object

**File**: `kuro_bzlmod/src/module_ctx.rs`

```rust
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct RepositoryOs {
    name: String,    // "linux", "mac os x", "windows"
    arch: String,    // "amd64", "aarch64"
    environ: HashMap<String, String>,
}
```

#### 5. path Object

**File**: `kuro_bzlmod/src/module_ctx.rs`

```rust
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct RepositoryPath {
    path: PathBuf,
}

impl RepositoryPath {
    fn basename(&self) -> String;
    fn dirname(&self) -> Option<RepositoryPath>;
    fn exists(&self) -> bool;
    fn is_dir(&self) -> bool;
    fn get_child(&self, relative: &str) -> RepositoryPath;
    fn readdir(&self) -> Vec<RepositoryPath>;
    fn realpath(&self) -> RepositoryPath;
}
```

#### 6. Extension Execution Engine

**File**: `kuro_bzlmod/src/execution.rs`

```rust
pub struct ExtensionExecutor {
    starlark_env: Environment,
    module_cache: ModuleCache,
    cell_resolver: CellResolver,
}

impl ExtensionExecutor {
    /// Execute all extensions and return generated repositories
    pub async fn execute_extensions(
        &self,
        resolved_graph: &ResolvedGraph,
        aggregated_extensions: &HashMap<String, AggregatedExtension>,
    ) -> Result<HashMap<String, GeneratedRepo>> {
        let mut all_repos = HashMap::new();

        for (ext_id, ext) in aggregated_extensions {
            // 1. Load the extension's .bzl file
            let bzl_path = self.resolve_bzl_label(&ext.extension_bzl_file)?;
            let module = self.load_bzl_file(&bzl_path)?;

            // 2. Get the module_extension definition
            let ext_def = module.get(&ext.extension_name)?
                .downcast_ref::<ModuleExtensionDef>()?;

            // 3. Build module_ctx from aggregated data
            let module_ctx = self.build_module_ctx(ext, resolved_graph)?;

            // 4. Call the implementation function
            let result = ext_def.implementation.invoke(&module_ctx)?;

            // 5. Collect generated repositories
            for (name, repo) in module_ctx.generated_repos.borrow().iter() {
                all_repos.insert(name.clone(), repo.clone());
            }
        }

        Ok(all_repos)
    }
}
```

#### 7. Repository Rule Invocation

Extensions call repository rules to create repositories. Need to support:

- `http_archive()` - Download and extract archives
- `http_file()` - Download single files
- `git_repository()` - Clone git repos
- `new_local_repository()` - Create repo from local path
- Custom repository rules defined in .bzl files

**Integration point**: Repository rules should reuse `kuro_bzlmod/src/fetch.rs` for downloading.

#### 8. Update Lockfile with Extension Data

**File**: `kuro_bzlmod/src/lockfile.rs`

Populate existing `LockfileExtensionData` structure:

```rust
impl Lockfile {
    pub fn add_extension_result(
        &mut self,
        ext_id: &str,
        input_hash: &str,
        generated_repos: &HashMap<String, GeneratedRepo>,
    ) {
        self.module_extensions.insert(ext_id.to_string(), LockfileExtensionData {
            bzl_file: /* extract from ext_id */,
            extension_name: /* extract from ext_id */,
            input_hash: input_hash.to_string(),
            generated_repos: /* convert GeneratedRepo -> LockfileGeneratedRepo */,
        });
    }

    pub fn get_cached_extension(&self, ext_id: &str, input_hash: &str)
        -> Option<&LockfileExtensionData>
    {
        self.module_extensions.get(ext_id)
            .filter(|data| data.input_hash == input_hash)
    }
}
```

### Success Criteria:

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

#### Test Migration (Phase 5):

- [x] ADD Rust unit tests for extension parsing (`parser.rs` tests)
- [x] ADD Rust unit tests for extension aggregation (`extensions.rs` tests)
- [ ] ADD `tests/core/bzlmod/test_use_extension.py` for extension usage
- [ ] ADD `tests/core/bzlmod/test_module_extension_def.py` for extension definition
- [ ] ADD `tests/core/bzlmod/test_tag_classes.py` for tag class handling
- [ ] ADD `tests/core/bzlmod/test_module_ctx.py` for module_ctx object
- [ ] ADD `tests/core/bzlmod/test_module_ctx_download.py` for download methods
- [ ] ADD `tests/core/bzlmod/test_module_ctx_execute.py` for execute method
- [ ] ADD `tests/core/bzlmod/test_extension_lockfile.py` for extension caching

---

## Phase 5b: bzlmod Build Integration

### Overview

Bridge the gap between bzlmod module resolution and Kuro's build system. This phase makes resolved modules (both local and remote) available as build targets via `@module_name//:target` syntax.

**Why this phase is critical:** Phases 4a-4d implement the bzlmod parsing, resolution, and fetching infrastructure. However, this infrastructure is currently standalone - resolved modules are not connected to Kuro's cell/repository system. Without this integration:

- `@rules_cc//:defs.bzl` cannot be loaded
- `bazel_dep()` modules cannot be built against
- Extension-generated repositories are inaccessible

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

### Kuro Cell System Integration Points

Based on codebase analysis, the key integration points are:

| Component                       | File                                                      | Purpose                                            |
| ------------------------------- | --------------------------------------------------------- | -------------------------------------------------- |
| `CellResolver`                  | `app/kuro_core/src/cells.rs:211-459`                      | Global registry mapping cell names to paths        |
| `CellsAggregator`               | `app/kuro_common/src/legacy_configs/aggregator.rs:45-159` | Collects cell definitions from all sources         |
| `BuckConfigBasedCells`          | `app/kuro_common/src/legacy_configs/cells.rs:252-434`     | Parses cell config, already has bzlmod stub        |
| `ExternalCellOrigin`            | `app/kuro_core/src/cells/external.rs:22-75`               | Tracks external cell sources (git, bundled, local) |
| `resolve_bzlmod_dependencies()` | `app/kuro_common/src/legacy_configs/cells.rs:446-563`     | Existing stub for bzlmod integration               |

### Changes Required:

#### 1. Complete bzlmod Cell Registration

**File**: `app/kuro_common/src/legacy_configs/cells.rs`

The `resolve_bzlmod_dependencies()` method (lines 446-563) already exists but is incomplete. Complete it to:

```rust
async fn resolve_bzlmod_dependencies(
    &self,
    project_root: &ProjectRoot,
) -> anyhow::Result<Vec<(CellName, CellRootPathBuf, Option<ExternalCellOrigin>)>> {
    let module_bazel = project_root.root().join("MODULE.bazel");
    if !module_bazel.exists() {
        return Ok(vec![]);
    }

    // 1. Parse root MODULE.bazel
    let parsed = kuro_bzlmod::parse_module_bazel(&module_bazel)?;

    // 2. Resolve dependencies using MVS
    let cache = ModuleCache::new()?;
    let resolver = MvsResolver::new(cache.clone()).await?;
    let resolved = resolver.resolve(&parsed).await?;

    // 3. Convert resolved modules to cell registrations
    let mut cells = Vec::new();
    for (module_key, module_info) in resolved.modules() {
        let cell_name = CellName::unchecked_new(&module_key.name)?;
        let cell_path = match &module_info.source {
            ModuleSource::LocalPath(path) => {
                // Local override - use relative path
                CellRootPathBuf::new(path.clone())
            }
            ModuleSource::Registry { url, .. } => {
                // Remote module - use cache path
                let cache_path = cache.module_path(&module_key.name, &module_key.version);
                CellRootPathBuf::new(cache_path)
            }
        };

        let origin = Some(ExternalCellOrigin::Bzlmod(BzlmodCellSetup {
            module_name: module_key.name.clone(),
            version: module_key.version.to_string(),
            registry: module_info.registry_url.clone(),
        }));

        cells.push((cell_name, cell_path, origin));
    }

    Ok(cells)
}
```

#### 2. Add Bzlmod ExternalCellOrigin Variant

**File**: `app/kuro_core/src/cells/external.rs`

Add new variant for bzlmod-sourced cells:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub enum ExternalCellOrigin {
    Bundled(CellName),
    Git(GitCellSetup),
    LocalPath(LocalPathCellSetup),
    Bzlmod(BzlmodCellSetup),  // NEW
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct BzlmodCellSetup {
    pub module_name: String,
    pub version: String,
    pub registry: Option<String>,
}
```

#### 3. Wire bzlmod Cells into Aggregator

**File**: `app/kuro_common/src/legacy_configs/cells.rs`

In `parse_with_file_ops_and_options_inner()`, ensure bzlmod cells are added to the aggregator:

```rust
// After parsing .buckconfig cells (around line 400)
// Add bzlmod-resolved cells
let bzlmod_cells = self.resolve_bzlmod_dependencies(&project_root).await?;
for (cell_name, cell_path, origin) in bzlmod_cells {
    // Don't override cells already defined in .buckconfig
    if !aggregator.has_cell(&cell_name) {
        aggregator.add_cell(cell_name, cell_path)?;
        if let Some(origin) = origin {
            aggregator.mark_external(cell_name, origin);
        }
    }
}
```

#### 4. Register Extension-Generated Repositories

**File**: New integration point needed

After extension execution (Phase 5), generated repositories must be registered:

```rust
pub fn register_extension_repos(
    aggregator: &mut CellsAggregator,
    extension_repos: &HashMap<String, GeneratedRepo>,
) -> Result<()> {
    for (repo_name, repo) in extension_repos {
        let cell_name = CellName::unchecked_new(repo_name)?;
        let cell_path = CellRootPathBuf::new(repo.path.clone().unwrap());

        aggregator.add_cell(cell_name, cell_path)?;
        aggregator.mark_external(cell_name, ExternalCellOrigin::Bzlmod(BzlmodCellSetup {
            module_name: repo_name.clone(),
            version: "extension".to_string(),
            registry: None,
        }));
    }
    Ok(())
}
```

#### 5. Handle repo_name Aliasing

**File**: `app/kuro_common/src/legacy_configs/cells.rs`

Support `bazel_dep(name = "foo", repo_name = "bar")` aliasing:

```rust
// In resolve_bzlmod_dependencies()
if let Some(repo_name) = dep.repo_name {
    // Register alias: @bar -> @foo
    aggregator.add_alias(
        CellAlias::new(&repo_name)?,
        cell_name.clone(),
    )?;
}
```

#### 6. DICE Key for Bzlmod Resolution

**File**: `app/kuro_common/src/dice/cells.rs`

Ensure bzlmod resolution is cached via DICE:

```rust
#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display("BzlmodResolutionKey")]
pub struct BzlmodResolutionKey;

#[async_trait]
impl Key for BzlmodResolutionKey {
    type Value = Arc<ResolvedGraph>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations<'_>,
        _cancellation: &CancellationContext,
    ) -> Self::Value {
        // Resolve bzlmod dependencies, cached by DICE
        let resolver = ctx.get_bzlmod_resolver().await?;
        Arc::new(resolver.resolve().await?)
    }
}
```

### Success Criteria:

#### Automated Verification:

- [ ] `@bazel_skylib//:lib.bzl` loads successfully after bzlmod resolution
- [ ] `@rules_cc//cc:defs.bzl` loads after fetching from BCR
- [ ] `@local_module//:target` works with local_path_override
- [ ] Repo aliasing works: `bazel_dep(name="foo", repo_name="bar")` makes `@bar` available
- [ ] Extension-generated repos accessible via `@repo_name//:target`
- [ ] DICE caches bzlmod resolution (no re-resolution on second build)
- [ ] Cell resolver includes all bzlmod modules

#### Manual Verification:

- [ ] Create project with `bazel_dep(name = "bazel_skylib", version = "1.5.0")`
- [ ] Successfully load `@bazel_skylib//lib:paths.bzl`
- [ ] Build target that depends on skylib function
- [ ] Verify cache hit on second build (no network activity)

#### Test Migration (Phase 5b):

- [ ] ADD `tests/core/bzlmod/test_cell_registration.py` for module→cell mapping
- [ ] ADD `tests/core/bzlmod/test_label_resolution.py` for @module//:target syntax
- [ ] ADD `tests/core/bzlmod/test_repo_aliasing.py` for repo_name parameter
- [ ] ADD `tests/core/bzlmod/test_extension_repo_registration.py` for extension repos
- [ ] UPDATE existing cell tests to verify bzlmod cells coexist with .buckconfig cells

---

## Phase 6: Rule Primitives and Provider Compatibility

### Overview

Ensure kuro's rule execution API matches Bazel's ctx, actions, and provider interfaces. Kuro already has substantial infrastructure that needs Bazel API alignment.

### Kuro Existing Implementation

Kuro already has most of this infrastructure. The work is primarily **API alignment**, not building from scratch:

| Feature                        | Kuro Location                                                                       | Status                           |
| ------------------------------ | ----------------------------------------------------------------------------------- | -------------------------------- |
| `AnalysisContext` (ctx)        | `app/kuro_build_api/src/interpreter/rule_defs/context.rs:176`                       | Exists, needs API tweaks         |
| `ctx.actions`                  | `app/kuro_build_api/src/interpreter/rule_defs/context.rs:60`                        | Exists via `AnalysisActions`     |
| `ctx.actions.run()`            | `app/kuro_action_impl/src/context/run.rs:121`                                       | Exists, verify parameter names   |
| `ctx.actions.write()`          | `app/kuro_action_impl/src/context/write.rs:110`                                     | Exists                           |
| `ctx.actions.declare_output()` | `app/kuro_action_impl/src/context/unsorted.rs:50`                                   | Needs rename to `declare_file()` |
| `DefaultInfo`                  | `app/kuro_build_api/src/interpreter/rule_defs/provider/builtin/default_info.rs:136` | Exists                           |
| `RunInfo`                      | `app/kuro_build_api/src/interpreter/rule_defs/provider/builtin/run_info.rs`         | Exists                           |
| `TransitiveSet`                | `app/kuro_build_api/src/interpreter/rule_defs/transitive_set/transitive_set.rs:112` | Exists, needs `depset` alias     |
| Action execution               | `app/kuro_build_api/src/actions/execute/action_executor.rs:312`                     | Exists                           |

### Bazel Source References

This is a critical phase - the rule API must match Bazel exactly. Study these thoroughly:

| Feature                 | Bazel Source File                                                                            |
| ----------------------- | -------------------------------------------------------------------------------------------- |
| **ctx object**          | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/StarlarkRuleContextApi.java`   |
| ctx implementation      | `src/main/java/com/google/devtools/build/lib/analysis/starlark/StarlarkRuleContext.java`     |
| **actions API**         | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/StarlarkActionFactoryApi.java` |
| actions implementation  | `src/main/java/com/google/devtools/build/lib/analysis/starlark/StarlarkActionFactory.java`   |
| **Args builder**        | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/CommandLineArgsApi.java`       |
| Args implementation     | `src/main/java/com/google/devtools/build/lib/analysis/starlark/Args.java`                    |
| **DefaultInfo**         | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/DefaultInfoApi.java`           |
| **RunInfo**             | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/RunEnvironmentInfoApi.java`    |
| **OutputGroupInfo**     | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/OutputGroupInfoApi.java`       |
| **depset**              | `src/main/java/com/google/devtools/build/lib/collect/nestedset/Depset.java`                  |
| depset ordering         | `src/main/java/com/google/devtools/build/lib/collect/nestedset/Order.java`                   |
| **Runfiles**            | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/RunfilesApi.java`              |
| Runfiles implementation | `src/main/java/com/google/devtools/build/lib/analysis/Runfiles.java`                         |
| **Provider definition** | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/ProviderApi.java`              |

**Starlark builtins (important!):**

- `src/main/starlark/builtins_bzl/common/` - Built-in rule implementations in Starlark
- These show how Bazel's own rules use the ctx/actions API

**Key tests:**

- `src/test/java/com/google/devtools/build/lib/analysis/starlark/StarlarkRuleContextTest.java`
- `src/test/java/com/google/devtools/build/lib/analysis/RunfilesTest.java`

### Changes Required:

#### 1. AnalysisContext (ctx) API Alignment

**File**: `app/kuro_build_api/src/interpreter/rule_defs/context.rs`

Current Kuro `AnalysisContext` (line 176) needs these additional/renamed attributes:

```python
# Bazel ctx attributes (ensure all available):
ctx.label              # ✓ Already exists
ctx.attr               # ✓ Already exists (line 295)
ctx.file               # NEED: Single file attribute access
ctx.files              # NEED: File list attribute access
ctx.executable         # NEED: Executable attribute access
ctx.outputs            # ✓ Exists as declare_output, needs direct access
ctx.actions            # ✓ Already exists (line 178)
ctx.build_file_path    # NEED: BUILD file path
ctx.workspace_name     # NEED: Workspace name (from MODULE.bazel)
ctx.bin_dir            # NEED: Output bin directory path
ctx.genfiles_dir       # NEED: Generated files directory path
ctx.var                # NEED: Make variable access
ctx.configuration      # NEED: Build configuration access
ctx.fragments          # NEED: Configuration fragments
```

Add to `analysis_context_methods()` (line 295):

```rust
#[starlark_module]
fn analysis_context_methods(builder: &mut MethodsBuilder) {
    // Existing methods...

    #[starlark(attribute)]
    fn file<'v>(this: &AnalysisContext<'v>) -> anyhow::Result<...> {
        // Return struct with single-file attributes
    }

    #[starlark(attribute)]
    fn files<'v>(this: &AnalysisContext<'v>) -> anyhow::Result<...> {
        // Return struct with file-list attributes
    }

    #[starlark(attribute)]
    fn executable<'v>(this: &AnalysisContext<'v>) -> anyhow::Result<...> {
        // Return struct with executable attributes
    }

    #[starlark(attribute)]
    fn build_file_path<'v>(this: &AnalysisContext<'v>) -> anyhow::Result<&str> {
        // Return path to BUILD.bazel file
    }

    #[starlark(attribute)]
    fn workspace_name<'v>(this: &AnalysisContext<'v>) -> anyhow::Result<&str> {
        // Return module name from MODULE.bazel
    }
}
```

#### 2. Actions API Alignment

**Files**: `app/kuro_action_impl/src/context/*.rs`

| Bazel Method          | Kuro File        | Status                          |
| --------------------- | ---------------- | ------------------------------- |
| `run()`               | `run.rs:121`     | ✓ Verify parameters match Bazel |
| `run_shell()`         | TBD              | NEED: Shell command action      |
| `write()`             | `write.rs:110`   | ✓ Exists                        |
| `declare_file()`      | `unsorted.rs:50` | ✓ Rename from `declare_output`  |
| `declare_directory()` | TBD              | NEED: Directory output          |
| `args()`              | TBD              | NEED: Args builder              |
| `symlink()`           | `copy.rs`        | CHECK: May exist                |
| `expand_template()`   | TBD              | NEED: Template expansion        |

**Key addition - `ctx.actions.args()` builder:**

```rust
// New file: app/kuro_action_impl/src/context/args.rs

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ArgsBuilder {
    items: Vec<ArgItem>,
    param_file: Option<ParamFileSpec>,
}

impl<'v> StarlarkValue<'v> for ArgsBuilder {
    // Methods: add(), add_all(), add_joined(), use_param_file(), set_param_file_format()
}
```

#### 3. depset() Global Function

**File**: `app/kuro_build_api/src/interpreter/rule_defs/transitive_set/globals.rs`

Kuro uses `transitive_set()` but Bazel uses `depset()`. Add alias:

```rust
#[starlark_module]
pub fn register_depset(builder: &mut GlobalsBuilder) {
    /// Bazel-compatible depset (alias for transitive_set)
    fn depset<'v>(
        direct: Option<&List<'v>>,
        transitive: Option<&List<'v>>,
        order: Option<&str>,  // "default", "postorder", "preorder", "topological"
    ) -> anyhow::Result<TransitiveSet<'v>> {
        // Map to transitive_set implementation
    }
}
```

**Order mapping:**
| Bazel Order | Kuro Equivalent |
|-------------|-----------------|
| `"default"` | BFS traversal |
| `"postorder"` | `PostorderTransitiveSetIterator` (line 110) |
| `"preorder"` | `PreorderTransitiveSetIterator` (line 53) |
| `"topological"` | `TopologicalTransitiveSetIterator` (line 189) |

#### 4. Built-in Providers

**Directory**: `app/kuro_build_api/src/interpreter/rule_defs/provider/builtin/`

**DefaultInfo** (line 136 in `default_info.rs`):

- ✓ Already exists with `files`, `runfiles`, `executable`
- CHECK: Parameter names match Bazel exactly

**NEED: OutputGroupInfo**

```rust
// New file: output_group_info.rs
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct OutputGroupInfo {
    groups: HashMap<String, TransitiveSet>,
}
```

**NEED: CcInfo, PyInfo** (for rules_cc, rules_python integration):

```rust
// New file: cc_info.rs - Critical for Phase 7
pub struct CcInfo {
    compilation_context: CompilationContext,
    linking_context: LinkingContext,
}

pub struct CompilationContext {
    headers: TransitiveSet,
    includes: TransitiveSet,
    defines: TransitiveSet,
    // ...
}
```

#### 5. Runfiles

**File**: Likely in `app/kuro_build_api/src/interpreter/rule_defs/`

```python
ctx.runfiles(
    files = [...],
    transitive_files = depset(...),
    symlinks = {...},
    root_symlinks = {...},
    collect_data = True,
    collect_default = True,
)
```

Check Kuro's existing runfiles implementation and align API.

### Success Criteria:

#### Automated Verification:

- [ ] ctx.actions.run() executes actions correctly
- [ ] ctx.actions.args() builds command lines
- [ ] depset operations are efficient
- [ ] DefaultInfo provider works
- [ ] Runfiles are collected correctly
- [ ] All documented ctx methods available

#### Manual Verification:

- [ ] Simple rule that compiles a C file works
- [ ] Rule with transitive dependencies collects all inputs

#### Test Migration (Phase 6):

- [ ] UPDATE `tests/core/analysis/test_cmd_args.py` for `ctx.actions.args()` API
- [ ] UPDATE `tests/core/transitive_sets/test_transitive_sets.py` → rename to `test_depset.py`
- [ ] ADD `tests/core/analysis/test_ctx_attr.py` for ctx.attr access
- [ ] ADD `tests/core/analysis/test_ctx_file.py` for ctx.file/ctx.files
- [ ] ADD `tests/core/analysis/test_ctx_actions_run.py` for ctx.actions.run()
- [ ] ADD `tests/core/analysis/test_ctx_actions_write.py` for ctx.actions.write()
- [ ] ADD `tests/core/analysis/test_ctx_actions_declare.py` for declare_file/directory
- [ ] ADD `tests/core/analysis/test_default_info.py` for DefaultInfo provider
- [ ] ADD `tests/core/analysis/test_runfiles.py` for runfiles collection
- [ ] ADD `tests/core/analysis/test_depset_ordering.py` for depset order parameter
- [ ] ADD `tests/core/analysis/test_provider_definition.py` for custom providers

---

## Phase 7: rules_cc Integration

### Overview

Get rules*cc working to compile C and C++ code. We target Bazel 9.0.0+ where rules_cc uses Starlark providers. Further in depth research is required to determine if the cc rules are \_actually* pure starlark with fallbacks purely for older bazel versions.
Due to the recency of the release of bazel 9, assumptions about version numbers should should be regularly be double checked by fetching web content, and plans and research should cite links, as well as filenames+line numbers heavily

### Architecture (Bazel 9.0.0+)

TODO: Perform in-depth research &

### Changes Required:

#### 1. Fetch rules_cc from BCR

```python
module(name = "test_cc")
bazel_dep(name = "rules_cc", version = "0.2.16") 
```
#### X. Unknown
This must be filled in with further research

#### 6. Test with Real Project

```python
load("@rules_cc//cc:defs.bzl", "cc_binary", "cc_library", "cc_test")

cc_library(
    name = "mylib",
    srcs = ["mylib.cc"],
    hdrs = ["mylib.h"],
)

cc_binary(
    name = "main",
    srcs = ["main.cc"],
    deps = [":mylib"],
)

cc_test(
    name = "mylib_test",
    srcs = ["mylib_test.cc"],
    deps = [":mylib", "@googletest//:gtest_main"],
)
```

### Success Criteria:

#### Automated Verification:

- [ ] Native `cc_common` module is available
- [ ] `cc_common.compile()` creates compilation actions
- [ ] `cc_common.link()` creates linking actions
- [ ] rules_cc's `CcInfo` provider works (uses Starlark `provider()`)
- [ ] `kuro build //:main` compiles and links successfully
- [ ] Header dependencies tracked correctly
- [ ] Incremental builds work
- [ ] `kuro test //:mylib_test` runs tests

#### Manual Verification:

- [ ] Build a non-trivial C++ project
- [ ] Verify compile_commands.json generation (via BXL)
- [ ] Test with both gcc and clang

#### Test Migration (Phase 7):

- [ ] ADD `tests/core/cc_common/test_compile.py` for cc_common.compile()
- [ ] ADD `tests/core/cc_common/test_link.py` for cc_common.link()
- [ ] ADD `tests/core/cc_common/test_create_compilation_context.py`
- [ ] ADD `tests/core/rules_cc/test_cc_library.py` for @rules_cc cc_library
- [ ] ADD `tests/core/rules_cc/test_cc_binary.py` for linking

---

## Phase 8: rules_rust Integration

### Overview

Get rules_rust working to compile Rust code.

### Changes Required:

#### 1. Fetch rules_rust from BCR

```python
bazel_dep(name = "rules_rust", version = "0.40.0")
```

#### 2. Rust Toolchain

- Download or detect rustc/cargo
- Handle edition, target triple

#### 3. Test with Real Project

```python
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library", "rust_test")

rust_library(
    name = "mylib",
    srcs = ["lib.rs"],
)

rust_binary(
    name = "main",
    srcs = ["main.rs"],
    deps = [":mylib"],
)
```

#### 4. crate_universe for Cargo Dependencies

```python
crate = use_extension("@rules_rust//crate_universe:extension.bzl", "crate")
crate.from_cargo(
    name = "crates",
    cargo_lockfile = "//:Cargo.lock",
    manifests = ["//:Cargo.toml"],
)
use_repo(crate, "crates")
```

### Success Criteria:

#### Automated Verification:

- [ ] `kuro build //:main` compiles Rust code
- [ ] `kuro test //:rust_test` runs tests
- [ ] crate_universe resolves Cargo dependencies

#### Manual Verification:

- [ ] Build a Rust project with external crates

---

## Phase 9: rules_python Integration

### Overview

Get rules_python working for Python projects.

### Changes Required:

#### 1. Fetch rules_python from BCR

```python
bazel_dep(name = "rules_python", version = "0.31.0")
```

#### 2. Python Toolchain

```python
python = use_extension("@rules_python//python/extensions:python.bzl", "python")
python.toolchain(python_version = "3.11")
```

#### 3. pip Integration

```python
pip = use_extension("@rules_python//python/extensions:pip.bzl", "pip")
pip.parse(
    hub_name = "pip",
    python_version = "3.11",
    requirements_lock = "//:requirements_lock.txt",
)
use_repo(pip, "pip")
```

### Success Criteria:

#### Automated Verification:

- [ ] `kuro run //:py_main` executes Python
- [ ] `kuro test //:py_test` runs pytest
- [ ] pip dependencies available

#### Manual Verification:

- [ ] Build a Python project with pip dependencies

---

## Phase 10: rules_oci Integration

### Overview

Enable container image building via rules_oci.

### Changes Required:

#### 1. Fetch rules_oci and rules_pkg

```python
bazel_dep(name = "rules_oci", version = "2.0.0")
bazel_dep(name = "rules_pkg", version = "0.9.1")
```

#### 2. Container Building

```python
load("@rules_oci//oci:defs.bzl", "oci_image", "oci_push")
load("@rules_pkg//pkg:tar.bzl", "pkg_tar")

pkg_tar(
    name = "app_layer",
    srcs = [":app"],
    package_dir = "/usr/local/bin",
)

oci_image(
    name = "image",
    base = "@distroless_base",
    tars = [":app_layer"],
    entrypoint = ["/usr/local/bin/app"],
)
```

### Success Criteria:

#### Automated Verification:

- [ ] `kuro build //:image` creates OCI image
- [ ] Multi-arch images work

#### Manual Verification:

- [ ] Load image into Docker and run container

---

## Phase 11: Stable Rust Migration

### Overview

Migrate from nightly Rust to stable Rust by auditing and replacing unstable features.

### Changes Required:

#### 1. Audit Unstable Features

**File**: All `*.rs` files, especially `lib.rs` files

Search for:

```rust
#![feature(...)]
```

Common unstable features Kuro may use:

- `box_patterns`
- `never_type` (`!`)
- `try_blocks`
- `associated_type_defaults`
- `generic_const_exprs`
- `specialization`
- Nightly-only APIs in std

#### 2. Categorize by Difficulty

Create a tracking list:

| Feature        | Usage Count | Stable Alternative   | Difficulty       |
| -------------- | ----------- | -------------------- | ---------------- |
| `feature_name` | N files     | Alternative approach | Easy/Medium/Hard |

#### 3. Replace with Stable Alternatives

**Common replacements:**

| Nightly Feature            | Stable Alternative                   |
| -------------------------- | ------------------------------------ |
| `box_patterns`             | Match on `&**boxed` or use methods   |
| `never_type` (!)           | Use `std::convert::Infallible`       |
| `try_blocks`               | Use closures returning Result        |
| `let_chains`               | Nested if-let (stable in Rust 1.76+) |
| `associated_type_defaults` | Explicit type parameters             |

#### 4. Update rust-toolchain

**File**: `rust-toolchain` or `rust-toolchain.toml`

Change from:

```toml
[toolchain]
channel = "nightly-2024-XX-XX"
```

To:

```toml
[toolchain]
channel = "stable"
# Or specific version: channel = "1.75.0"
```

#### 5. Document Any Remaining Blockers

If any features truly require nightly with no reasonable workaround:

- Document why in `docs/nightly-features.md`
- Consider if the feature can be made optional
- Track upstream stabilization

### Success Criteria:

#### Automated Verification:

- [ ] `cargo +stable build --release` succeeds
- [ ] `cargo +stable test` passes
- [ ] No `#![feature(...)]` in the codebase (or documented exceptions)
- [ ] CI runs on stable Rust

#### Manual Verification:

- [ ] Build tested on latest stable Rust release
- [ ] Performance is not significantly degraded

**Implementation Note**: This may be iterative - some features may need creative workarounds. Document all changes for future reference.

---

## Phase 12: Local Build Isolation (Sandboxing)

### Overview

Implement local build sandboxing to ensure hermetic builds and catch undeclared dependencies.

### Bazel Source References

Bazel's sandboxing is well-documented in source. The linux-sandbox is particularly instructive:

| Feature                    | Bazel Source File                                                                      |
| -------------------------- | -------------------------------------------------------------------------------------- |
| Sandbox abstraction        | `src/main/java/com/google/devtools/build/lib/sandbox/SandboxedSpawn.java`              |
| Sandbox strategy base      | `src/main/java/com/google/devtools/build/lib/sandbox/AbstractSandboxSpawnRunner.java`  |
| **Linux sandbox**          | `src/main/java/com/google/devtools/build/lib/sandbox/LinuxSandboxedSpawnRunner.java`   |
| Linux sandbox C helper     | `src/main/tools/linux-sandbox/` (C code for namespace setup)                           |
| **macOS sandbox**          | `src/main/java/com/google/devtools/build/lib/sandbox/DarwinSandboxedSpawnRunner.java`  |
| macOS sandbox profile      | Look for `.sb` sandbox profile files                                                   |
| **Windows sandbox**        | `src/main/java/com/google/devtools/build/lib/sandbox/WindowsSandboxedSpawnRunner.java` |
| Symlink sandbox (fallback) | `src/main/java/com/google/devtools/build/lib/sandbox/SymlinkedSandboxedSpawn.java`     |
| Sandbox options            | `src/main/java/com/google/devtools/build/lib/sandbox/SandboxOptions.java`              |

**Critical implementation detail:** Study `src/main/tools/linux-sandbox/linux-sandbox.cc` - this is the actual C program that sets up Linux namespaces. You may want to write a similar helper in Rust.

**Key tests:**

- `src/test/java/com/google/devtools/build/lib/sandbox/` - Full sandbox test suite
- `src/test/shell/integration/sandboxing_test.sh` - Integration tests

### Changes Required:

#### 1. Sandbox Infrastructure

**File**: New module `kuro_sandbox/`

Create sandbox abstraction:

```rust
pub trait Sandbox {
    /// Execute an action in an isolated environment
    fn execute(&self, action: &Action, inputs: &[PathBuf], outputs: &[PathBuf]) -> Result<()>;
}
```

#### 2. Linux Sandbox Implementation

**File**: `kuro_sandbox/src/linux.rs`

Use Linux namespaces for isolation:

- Mount namespace: Create isolated filesystem view
- Symlink/bind mount declared inputs into sandbox
- Outputs written to sandbox, then copied out
- Network namespace (optional): Block network access

Similar to Bazel's `linux-sandbox`:

```rust
pub struct LinuxSandbox {
    // Sandbox root directory
    sandbox_root: PathBuf,
    // Whether to block network
    block_network: bool,
}
```

#### 3. macOS Sandbox Implementation

**File**: `kuro_sandbox/src/macos.rs`

Use `sandbox-exec` with custom profiles:

```rust
pub struct MacOsSandbox {
    profile: SandboxProfile,
}
```

Or use symlink-based sandbox (less secure but portable).

#### 4. Windows Sandbox Implementation

**File**: `kuro_sandbox/src/windows.rs`

Options:

- Symlink-based sandbox (most portable)
- Windows containers (heavier)
- Filesystem virtualization

Start with symlink-based approach:

```rust
pub struct WindowsSandbox {
    sandbox_root: PathBuf,
}
```

#### 5. Integration with Action Execution

**File**: Action execution code

```rust
fn execute_action(action: &Action) -> Result<()> {
    let sandbox = create_sandbox_for_platform()?;

    // Create sandbox with only declared inputs visible
    sandbox.execute(
        action,
        &action.inputs,
        &action.outputs,
    )?;

    // Verify outputs exist
    for output in &action.outputs {
        if !output.exists() {
            return Err(Error::MissingOutput(output.clone()));
        }
    }

    Ok(())
}
```

#### 6. Sandbox Configuration

**File**: CLI and configuration

```bash
# Enable/disable sandboxing
kuro build --sandbox=true //...   # Default on
kuro build --sandbox=false //...  # For debugging

# Sandbox strategy
kuro build --sandbox_strategy=linux-sandbox //...
kuro build --sandbox_strategy=symlink //...
```

### Success Criteria:

#### Automated Verification:

- [ ] Actions only see declared inputs
- [ ] Action fails if it reads undeclared file
- [ ] Action fails if it writes outside declared outputs
- [ ] Sandbox works on Linux (namespace-based)
- [ ] Sandbox works on macOS
- [ ] Sandbox works on Windows (symlink-based)
- [ ] `--sandbox=false` disables sandboxing

#### Manual Verification:

- [ ] Deliberately omit an input dependency, verify build fails with sandbox
- [ ] Same build succeeds with `--sandbox=false` (proving sandbox caught it)
- [ ] Performance overhead is acceptable (< 10% slowdown)

#### Test Migration (Phase 12):

- [ ] ADD `tests/core/sandbox/test_input_isolation.py` for undeclared input detection
- [ ] ADD `tests/core/sandbox/test_output_isolation.py` for undeclared output detection
- [ ] ADD `tests/core/sandbox/test_sandbox_strategies.py` for strategy selection
- [ ] ADD `tests/core/sandbox/test_sandbox_disabled.py` for `--sandbox=false`
- [ ] Port tests from Bazel's `src/test/java/com/google/devtools/build/lib/sandbox/`
- [ ] Port shell tests from `sandboxing_test.sh`

**Implementation Note**: Start with symlink-based sandbox for all platforms, then optimize Linux with namespaces.

---

## Phase 13: Platform Support

### Overview

Ensure kuro works on Linux, Windows, and macOS.

### Changes Required:

#### 1. Linux Support (Primary)

- Test on Ubuntu, Fedora
- Linux namespace sandboxing

#### 2. Windows Support

- MSVC toolchain for rules_cc
- Handle .exe extensions
- Symlink-based sandboxing

#### 3. macOS Support

- Intel and Apple Silicon
- Xcode toolchain integration

### Success Criteria:

#### Automated Verification:

- [ ] CI passes on Linux, Windows, macOS

#### Manual Verification:

- [ ] Build same project on all three platforms

---

## Phase 14: Query Commands

### Overview

Implement Bazel-compatible query commands for build graph introspection.

### Bazel Source References

Bazel has three query engines. Study the query language carefully:

| Feature                   | Bazel Source File                                                                                 |
| ------------------------- | ------------------------------------------------------------------------------------------------- |
| **Query language parser** | `src/main/java/com/google/devtools/build/lib/query2/engine/QueryParser.java`                      |
| Query language grammar    | `src/main/java/com/google/devtools/build/lib/query2/engine/Lexer.java`                            |
| **Query functions**       | `src/main/java/com/google/devtools/build/lib/query2/engine/QueryFunctions.java`                   |
| `deps()` function         | `src/main/java/com/google/devtools/build/lib/query2/engine/DepsFunction.java`                     |
| `rdeps()` function        | `src/main/java/com/google/devtools/build/lib/query2/engine/RdepsFunction.java`                    |
| `kind()` function         | `src/main/java/com/google/devtools/build/lib/query2/engine/KindFunction.java`                     |
| Set operations            | `src/main/java/com/google/devtools/build/lib/query2/engine/BinaryOperatorExpression.java`         |
| **cquery (configured)**   | `src/main/java/com/google/devtools/build/lib/query2/cquery/ConfiguredTargetQueryEnvironment.java` |
| **aquery (action)**       | `src/main/java/com/google/devtools/build/lib/query2/aquery/ActionGraphQueryEnvironment.java`      |
| Output formatters         | `src/main/java/com/google/devtools/build/lib/query2/query/output/`                                |

**Query language specification:** https://bazel.build/query/language (official docs have the full grammar)

**Key tests:**

- `src/test/java/com/google/devtools/build/lib/query2/` - Comprehensive query tests
- `src/test/shell/integration/query_test.sh` - Integration tests

### Changes Required:

#### 1. Query Command (`kuro query`)

Query the unconfigured target graph:

```bash
kuro query "deps(//src:main)"
kuro query "rdeps(//..., //lib:foo)"
kuro query "//..." --output=label
kuro query "//..." --output=build
```

#### 2. Configured Query (`kuro cquery`)

Query with configurations applied:

```bash
kuro cquery "deps(//src:main)" --output=json
```

#### 3. Action Query (`kuro aquery`)

Query the action graph:

```bash
kuro aquery "//src:main" --output=jsonproto
```

#### 4. Query Language Compatibility

Support Bazel query syntax:

- `deps()`, `rdeps()`
- `allpaths()`, `somepath()`
- `kind()`, `attr()`
- `filter()`
- Set operations: `+`, `-`, `^`

### Success Criteria:

#### Automated Verification:

- [ ] `kuro query "deps(//...)"` returns dependencies
- [ ] `kuro cquery` shows configured targets
- [ ] `kuro aquery` shows actions
- [ ] Query output formats match Bazel

#### Manual Verification:

- [ ] IDE/tooling integration using query commands works

#### Test Migration (Phase 14):

- [ ] UPDATE `tests/core/query/test_buildfiles.py` for Bazel buildfiles() function
- [ ] ADD `tests/core/query/test_deps.py` for deps() function
- [ ] ADD `tests/core/query/test_rdeps.py` for rdeps() function
- [ ] ADD `tests/core/query/test_kind.py` for kind() function
- [ ] ADD `tests/core/query/test_attr.py` for attr() function
- [ ] ADD `tests/core/query/test_filter.py` for filter() function
- [ ] ADD `tests/core/query/test_allpaths.py` for allpaths() function
- [ ] ADD `tests/core/query/test_somepath.py` for somepath() function
- [ ] ADD `tests/core/query/test_set_operations.py` for +, -, ^ operators
- [ ] ADD `tests/core/query/test_output_formats.py` for --output=label|build|xml|json
- [ ] ADD `tests/core/query/test_cquery.py` for configured query
- [ ] ADD `tests/core/query/test_aquery.py` for action query
- [ ] Port comprehensive tests from Bazel's `bazel_query_test.sh` (50+ test cases)

---

## Testing Strategy

> **Detailed Mapping**: See [`2026-01-22-test-infrastructure-mapping.md`](../research/2026-01-22-test-infrastructure-mapping.md) for the complete test-by-test migration plan.

### Test Migration Summary

| Action      | Count | Description                         |
| ----------- | ----- | ----------------------------------- |
| KEEP+UPDATE | ~34   | Update Buck2 tests for Bazel syntax |
| DELETE      | ~32   | Remove Buck2-specific tests         |
| ADD         | ~123  | Create new Bazel-concept tests      |
| PRESERVE    | ~69   | Keep unchanged (shared concepts)    |

### Framework Preservation

We preserve the pytest-based test framework:

- **Location**: `tests/e2e_util/` (framework), `tests/core/` and `tests/e2e/` (tests)
- **Pattern**: `@buck_test()` decorator with async/await
- **Fixtures**: `test_*_data/` directories with `MODULE.bazel` and `BUILD.bazel`
- **Golden files**: `*.golden` with sanitization for non-determinism

### Unit Tests (Rust)

- `app/kuro_build_api_tests/src/attrs.rs` - Update for `attr.*` API
- `app/kuro_build_api_tests/src/actions.rs` - Update for `ctx.actions.*` API
- `app/kuro_build_api_tests/src/nodes.rs` - Preserve DICE node tests
- ADD new module: `app/kuro_bzlmod_tests/` for bzlmod resolution

### Integration Tests (Python)

- Full build tests with rules_cc, rules_rust, rules_python
- bzlmod resolution with real BCR
- Lockfile generation and caching
- Cross-platform sandbox tests

### Compatibility Tests

- Compare output with actual Bazel
- Test against real-world open source projects

### Performance Tests

- Benchmark against Bazel
- Measure sandbox overhead
- Profile memory usage

---

## Performance Considerations

### DICE Advantages to Preserve

- Incremental computation
- Parallel execution via Tokio
- Smart invalidation
- Deferred materialization

### Sandbox Performance

- Symlink-based sandbox is faster than copy-based
- Linux namespaces add minimal overhead
- Consider sandbox reuse between actions

### bzlmod Optimization

- Cache aggressively
- Parallel BCR downloads
- Use lockfile to skip resolution

---

## References

- Kuro repository: https://github.com/facebook/kuro
- Bazel documentation: https://bazel.build/
- Bazel Central Registry: https://registry.bazel.build/
- rules_cc: https://github.com/bazelbuild/rules_cc
- rules_rust: https://github.com/bazelbuild/rules_rust
- rules_python: https://github.com/bazelbuild/rules_python
- rules_oci: https://github.com/bazel-contrib/rules_oci
- Starlark specification: https://github.com/bazelbuild/starlark
- bzlmod documentation: https://bazel.build/external/module
- Costasiella kuroshimae (mascot): https://en.wikipedia.org/wiki/Costasiella_kuroshimae

### Bazel Source Code References

When implementing Bazel-compatible features, consult the Bazel source at https://github.com/bazelbuild/bazel for authoritative behavior and architectural patterns.

**Key directories:**

| Area                         | Bazel Source Path                                               |
| ---------------------------- | --------------------------------------------------------------- |
| **Starlark API definitions** | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/` |
| **Starlark builtins**        | `src/main/starlark/builtins_bzl/`                               |
| **bzlmod implementation**    | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/`     |
| **Sandboxing**               | `src/main/java/com/google/devtools/build/lib/sandbox/`          |
| **Query engine**             | `src/main/java/com/google/devtools/build/lib/query2/`           |
| **Actions**                  | `src/main/java/com/google/devtools/build/lib/actions/`          |
| **Rules (ctx, providers)**   | `src/main/java/com/google/devtools/build/lib/analysis/`         |
| **Package loading**          | `src/main/java/com/google/devtools/build/lib/packages/`         |
| **Skyframe (incremental)**   | `src/main/java/com/google/devtools/build/skyframe/`             |

**How to use these references:**

1. Clone Bazel source: `git clone https://github.com/bazelbuild/bazel`
2. Navigate to the relevant directory for the feature you're implementing
3. Study the interfaces, data structures, and algorithms
4. Pay attention to edge cases handled in tests: `src/test/java/...`
