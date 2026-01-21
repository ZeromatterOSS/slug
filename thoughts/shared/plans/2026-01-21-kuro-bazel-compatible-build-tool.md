# Kuro: Bazel-Compatible Build Tool Implementation Plan

## Overview

Kuro is a Bazel 9.0-compatible build tool that leverages Buck2's high-performance Rust internals (DICE incremental computation, starlark-rust interpreter, remote execution architecture) while providing full compatibility with Bazel's BUILD.bazel files, bzlmod module system, and the rules_* ecosystem.

Named after the [Costasiella kuroshimae](https://en.wikipedia.org/wiki/Costasiella_kuroshimae) (the "leaf sheep" sea slug), kuro aims to be a small, efficient alternative to Bazel that "eats" the same build files but runs faster.

## Current State Analysis

### Starting Point: Buck2 Fork
- Buck2 provides proven, high-performance build infrastructure
- DICE engine delivers 2x performance improvement over traditional build systems
- starlark-rust is a mature Starlark interpreter with type annotation support
- Remote execution architecture is production-ready (Meta scale)
- Modular Rust crates (dice, starlark, gazebo, allocative, superconsole) are reusable
- BXL provides powerful build graph introspection for developer tooling

### Key Gaps to Bridge
| Feature | Buck2 | Bazel 9.0 | Work Required |
|---------|-------|-----------|---------------|
| Build files | BUCK | BUILD.bazel | File detection change |
| Starlark dialect | `attrs.*`, type annotations | `attr.*`, optional types | API additions (keep types) |
| Rule definition | `impl` param | `implementation` param | Support both, prefer Bazel |
| Dep management | Cells, no modules | bzlmod mandatory | Full bzlmod implementation |
| Registry | None | BCR | Registry client |
| Local isolation | None (RE-first) | Sandboxing | Implement sandboxing |
| Rust toolchain | Nightly required | - | Migrate to stable Rust |
| Target patterns | `//pkg:` | `//pkg:all` | Pattern parsing |
| Visibility | `"PUBLIC"` | `"//visibility:public"` | Syntax change |

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

1. **Buck2 compatibility** - No support for BUCK files or Buck2-specific Starlark
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
7. **Rules integration** - Test with actual rules_* packages
8. **Stable Rust** - Remove nightly dependencies
9. **Local sandboxing** - Add build isolation
10. **Platform support** - Linux, Windows, macOS
11. **Query commands** - Add bazel-compatible query interface

---

## Phase 1: Fork and Foundation

### Overview
Fork Buck2, rebrand to kuro, establish build infrastructure, and verify the base system compiles and runs.

### Changes Required:

#### 1. Repository Setup
**Action**: Fork facebook/buck2 into this repository

```bash
# Clone Buck2 as starting point
git clone --depth 1 https://github.com/facebook/buck2.git kuro-src
# Copy relevant source (excluding .git)
cp -r kuro-src/* /var/mnt/dev/kuro/
rm -rf kuro-src
```

#### 2. Rename Buck2 → Kuro
**Files to modify**: Cargo.toml files, binary names, user-facing strings

- Rename `buck2` binary to `kuro`
- Update all Cargo.toml package names from `buck2_*` to `kuro_*`
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
- [ ] `cargo build --release` succeeds
- [ ] `cargo test` passes (existing Buck2 tests)
- [ ] `./target/release/kuro --version` outputs kuro version
- [ ] `./target/release/kuro --help` shows kuro branding
- [ ] BXL commands still exist (`kuro bxl --help`)

#### Manual Verification:
- [ ] Binary size is reasonable (< 100MB)
- [ ] No references to "buck2" in user-facing output

**Implementation Note**: After completing this phase, pause for confirmation before proceeding.

---

## Phase 2: Starlark Dialect - Bazel Compatibility

### Overview
Modify starlark-rust to support Bazel's Starlark APIs while preserving type annotation support.

### Changes Required:

#### 1. Keep Type Annotation Support
**Important**: Do NOT remove type annotations from starlark-rust.

- Bazel 9.0 has experimental type support (`--experimental_starlark_types`)
- Bazel 10.0 will have full typing
- kuro will be ahead of Bazel here - this is a feature, not a bug
- Type annotations should be **optional** (code without them must work)
- Type errors should be **warnings**, not failures

#### 2. Add Bazel Attribute Module
**File**: Create Bazel-compatible attribute API

Add `attr` module alongside existing `attrs`:

```python
# Both should work:
# Buck2 style (existing):
attrs.string(), attrs.dep(), attrs.list(attrs.dep())

# Bazel style (add):
attr.string(), attr.label(), attr.label_list()
```

Full mapping:
| Bazel `attr.*` | Implementation |
|----------------|----------------|
| `attr.string()` | Maps to `attrs.string()` |
| `attr.int()` | Maps to `attrs.int()` |
| `attr.bool()` | Maps to `attrs.bool()` |
| `attr.label()` | Maps to `attrs.dep()` |
| `attr.label_list()` | Maps to `attrs.list(attrs.dep())` |
| `attr.string_list()` | Maps to `attrs.list(attrs.string())` |
| `attr.string_dict()` | Maps to `attrs.dict(...)` |
| `attr.output()` | Maps to `attrs.output()` |
| `attr.output_list()` | Maps to `attrs.list(attrs.output())` |

#### 3. Rule Definition API Compatibility
**File**: Rule definition handling

Support both parameter names:
```python
# Bazel style (prefer):
my_rule = rule(
    implementation = _impl,
    attrs = {...}
)

# Buck2 style (also works):
my_rule = rule(
    impl = _impl,
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
# Buck2: //pkg: (same meaning)
# Support both, prefer Bazel
```

### Success Criteria:

#### Automated Verification:
- [ ] Parser accepts Bazel-style rule definitions with `implementation`
- [ ] `attr.*` functions are available and work correctly
- [ ] `native.*` functions are available in .bzl context
- [ ] Bazel visibility syntax parses correctly
- [ ] `//pkg:all` pattern works
- [ ] Type annotations still work (optional, warnings only)
- [ ] Unit tests for attribute type mapping pass

#### Manual Verification:
- [ ] Sample .bzl file with Bazel syntax loads without errors
- [ ] Sample .bzl file with Buck2 syntax still loads (backwards compat during transition)
- [ ] Type-annotated .bzl file works with annotations as optional

---

## Phase 3: Build File Recognition

### Overview
Change kuro to recognize BUILD.bazel and BUILD files instead of BUCK files.

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
Remove any Buck2-specific build file handling that doesn't apply to Bazel.

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
- [ ] `kuro build //...` finds BUILD.bazel files
- [ ] `kuro build //...` ignores BUCK files
- [ ] Workspace root detected by MODULE.bazel presence
- [ ] Package boundaries correctly identified
- [ ] BUILD.bazel takes precedence over BUILD

#### Manual Verification:
- [ ] Create test directory with BUILD.bazel, verify it's found
- [ ] Create test directory with both BUILD and BUILD.bazel, verify BUILD.bazel used

---

## Phase 4a: bzlmod - Workspace Recognition

### Overview
Parse MODULE.bazel as workspace root marker and implement basic parsing.

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
- [ ] MODULE.bazel parses without errors
- [ ] `module()` directive extracts name, version, compatibility_level
- [ ] `bazel_dep()` directives are collected
- [ ] Workspace root correctly identified by MODULE.bazel
- [ ] Missing MODULE.bazel gives clear error

#### Manual Verification:
- [ ] Create project with MODULE.bazel, verify kuro recognizes it
- [ ] Invalid MODULE.bazel syntax gives helpful error message

---

## Phase 4b: bzlmod - Local Dependencies

### Overview
Implement local module loading via `local_path_override()`.

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
- [ ] `local_path_override()` parses correctly
- [ ] Local module's MODULE.bazel is found and parsed
- [ ] Local module's BUILD.bazel files are found
- [ ] Can build targets from local modules: `@local_module//:target`

#### Manual Verification:
- [ ] Create two-module project with local override
- [ ] Build target that depends on local module
- [ ] Modify local module, verify rebuild happens

---

## Phase 4c: bzlmod - BCR Integration

### Overview
Implement Bazel Central Registry client for fetching remote modules.

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
- [ ] BCR metadata fetched successfully
- [ ] Source archives downloaded and extracted
- [ ] Integrity verification works (fails on mismatch)
- [ ] Git repositories cloned correctly
- [ ] Cache prevents re-downloads
- [ ] Custom registry URL works (`--registry=URL`)

#### Manual Verification:
- [ ] Add `bazel_dep(name = "bazel_skylib", version = "1.5.0")`, verify fetched
- [ ] Offline build works after initial fetch
- [ ] Network failure gives clear error message

---

## Phase 4d: bzlmod - Resolution and Lockfile

### Overview
Implement Minimal Version Selection (MVS) algorithm and lockfile generation.

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
- [ ] MVS correctly resolves diamond dependencies
- [ ] Compatibility level conflicts are detected
- [ ] MODULE.bazel.lock is generated in correct format
- [ ] Subsequent builds use lockfile (no network if unchanged)
- [ ] Lockfile updates when MODULE.bazel changes
- [ ] `--lockfile_mode=error` fails if lockfile would change

#### Manual Verification:
- [ ] Add dependency with transitive deps, verify correct versions selected
- [ ] Modify MODULE.bazel, verify lockfile updates
- [ ] Offline build works with valid lockfile
- [ ] Commit lockfile, verify teammate gets same versions

---

## Phase 5: Module Extensions

### Overview
Implement module extensions which allow custom dependency resolution logic.

### Changes Required:

#### 1. Extension Definition Parsing
**File**: `kuro_bzlmod/src/extensions.rs`

```python
# In extensions.bzl
my_ext = module_extension(
    implementation = _my_ext_impl,
    tag_classes = {
        "install": tag_class(attrs = {"name": attr.string()}),
    },
)

def _my_ext_impl(module_ctx):
    for mod in module_ctx.modules:
        for tag in mod.tags.install:
            # Create repositories based on tags
            pass
```

#### 2. Extension Usage Parsing
**File**: MODULE.bazel parsing

```python
maven = use_extension("@rules_jvm_external//:extensions.bzl", "maven")
maven.install(
    artifacts = ["com.google.guava:guava:31.1-jre"],
)
use_repo(maven, "maven")
```

#### 3. Extension Execution
```rust
fn evaluate_extensions(
    resolved_modules: &ResolvedGraph,
    extensions: &[ExtensionUsage],
) -> Result<HashMap<String, Repository>> {
    for ext in extensions {
        // 1. Collect all tags from all modules using this extension
        let all_tags = collect_tags(resolved_modules, ext);

        // 2. Execute the implementation function with module_ctx
        let repos = execute_extension(ext, all_tags)?;

        // 3. Register generated repositories
        for repo in repos {
            register_repository(repo)?;
        }
    }
}
```

#### 4. module_ctx Object
```python
class module_ctx:
    modules: list[module]  # All modules using this extension
    os: struct             # OS info
    # File I/O methods
    def read(path): ...
    def download(url): ...
    def extract(archive): ...
```

#### 5. Update Lockfile with Extension Data
Record extension results in MODULE.bazel.lock for caching.

### Success Criteria:

#### Automated Verification:
- [ ] `use_extension()` parses correctly
- [ ] Extension tags collected from all using modules
- [ ] Extension implementation executes
- [ ] Generated repositories are accessible
- [ ] Extension results cached in lockfile

#### Manual Verification:
- [ ] Simple extension creating a filegroup works
- [ ] rules_python's `pip.parse()` extension works (stretch goal)

---

## Phase 6: Rule Primitives and Provider Compatibility

### Overview
Ensure kuro's rule execution API matches Bazel's ctx, actions, and provider interfaces.

### Changes Required:

#### 1. AnalysisContext (ctx) API
Bazel's ctx object:
```python
ctx.label              # Target's label
ctx.attr               # Resolved attributes
ctx.file               # Single file attribute access
ctx.files              # File list attribute access
ctx.executable         # Executable attribute access
ctx.outputs            # Declared outputs
ctx.actions            # Action factory
ctx.build_file_path    # BUILD file path
ctx.workspace_name     # Workspace name
ctx.bin_dir            # Output bin directory
ctx.genfiles_dir       # Generated files directory
```

#### 2. Actions API
```python
ctx.actions.run(
    executable = ...,
    arguments = [...],
    inputs = [...],
    outputs = [...],
    mnemonic = "Compile",
    progress_message = "Compiling %{label}",
    env = {...},
    execution_requirements = {...},
)

ctx.actions.run_shell(command = "...", ...)
ctx.actions.write(output = ..., content = "...")
ctx.actions.declare_file(name)
ctx.actions.declare_directory(name)
ctx.actions.args()
ctx.actions.symlink(output, target_file)
ctx.actions.expand_template(template, output, substitutions)
```

#### 3. Args Builder
```python
args = ctx.actions.args()
args.add("--flag", value)
args.add_all(files, format_each="--input=%s")
args.add_joined(items, join_with=",")
args.use_param_file("@%s")
args.set_param_file_format("multiline")
```

#### 4. Built-in Providers
```python
DefaultInfo(files, runfiles, executable, default_runfiles, data_runfiles)
RunInfo(...)
OutputGroupInfo(**groups)
InstrumentedFilesInfo(...)
CcInfo(compilation_context, linking_context)
PyInfo(transitive_sources, imports)
```

#### 5. Depset Implementation
```python
depset(
    direct = [...],
    transitive = [...],
    order = "postorder",  # or "preorder", "topological", "default"
)
```

#### 6. Runfiles
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

---

## Phase 7: rules_cc Integration

### Overview
Get rules_cc working to compile C and C++ code.

### Changes Required:

#### 1. Fetch rules_cc from BCR
```python
module(name = "test_cc")
bazel_dep(name = "rules_cc", version = "0.0.9")
```

#### 2. CcInfo Provider
```python
CcInfo(
    compilation_context = CompilationContext(
        headers = depset(...),
        includes = depset(...),
        quote_includes = depset(...),
        system_includes = depset(...),
        defines = depset(...),
        local_defines = depset(...),
    ),
    linking_context = LinkingContext(
        linker_inputs = depset(...),
    ),
)
```

#### 3. C++ Toolchain Support
- Detect system compiler (gcc, clang, msvc)
- Configure include paths, library paths
- Support cross-compilation

#### 4. Test with Real Project
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
- [ ] `kuro build //:main` compiles and links successfully
- [ ] Header dependencies tracked correctly
- [ ] Incremental builds work
- [ ] `kuro test //:mylib_test` runs tests

#### Manual Verification:
- [ ] Build a non-trivial C++ project
- [ ] Verify compile_commands.json generation (via BXL)

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

Common unstable features Buck2 may use:
- `box_patterns`
- `never_type` (`!`)
- `try_blocks`
- `associated_type_defaults`
- `generic_const_exprs`
- `specialization`
- Nightly-only APIs in std

#### 2. Categorize by Difficulty

Create a tracking list:

| Feature | Usage Count | Stable Alternative | Difficulty |
|---------|-------------|-------------------|------------|
| `feature_name` | N files | Alternative approach | Easy/Medium/Hard |

#### 3. Replace with Stable Alternatives

**Common replacements:**

| Nightly Feature | Stable Alternative |
|-----------------|-------------------|
| `box_patterns` | Match on `&**boxed` or use methods |
| `never_type` (!) | Use `std::convert::Infallible` |
| `try_blocks` | Use closures returning Result |
| `let_chains` | Nested if-let (stable in Rust 1.76+) |
| `associated_type_defaults` | Explicit type parameters |

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

---

## Testing Strategy

### Unit Tests
- Starlark parser tests for Bazel dialect
- MVS resolution algorithm tests
- Sandbox isolation tests
- Provider and depset tests

### Integration Tests
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

- Buck2 repository: https://github.com/facebook/buck2
- Bazel documentation: https://bazel.build/
- Bazel Central Registry: https://registry.bazel.build/
- rules_cc: https://github.com/bazelbuild/rules_cc
- rules_rust: https://github.com/bazelbuild/rules_rust
- rules_python: https://github.com/bazelbuild/rules_python
- rules_oci: https://github.com/bazel-contrib/rules_oci
- Starlark specification: https://github.com/bazelbuild/starlark
- bzlmod documentation: https://bazel.build/external/module
- Costasiella kuroshimae (mascot): https://en.wikipedia.org/wiki/Costasiella_kuroshimae
