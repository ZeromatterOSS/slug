---
date: 2026-02-05T10:53:29-08:00
researcher: Claude
git_commit: 52f9ff18363a6640b14f9f20ab70fa74c3381b79
branch: main
repository: slug
topic: "Extending Slug with Language-Specific Build File Parsing"
tags: [research, codebase, starlark, dice, bzlmod, module-extensions, cargo, cmake, language-rules, buck2, anonymous-targets, bxl, package-files, gazelle, build-generation, hook-points]
status: complete
last_updated: 2026-02-05
last_updated_by: Claude
last_updated_note: "Added concrete hook points for BUILD file generation (Gazelle-like functionality) and Buck2-specific features"
---

# Research: Extending Slug with Language-Specific Build File Parsing

**Date**: 2026-02-05T10:53:29-08:00
**Researcher**: Claude
**Git Commit**: 52f9ff18363a6640b14f9f20ab70fa74c3381b79
**Branch**: main
**Repository**: slug

## Research Question

Is it viable to extend Slug with the ability to define language-specific build file parsing via Starlark? This would allow:
- Custom build file patterns (e.g., `Cargo.toml`, `go.mod`, `CMakeLists.txt`)
- Analysis-time file reading and tool invocation
- Translation of language-native manifests into BUILD targets
- Potential caching of parsed results in `MODULE.lock`

## Summary

**Viability: Yes, with significant architectural work.**

Slug's architecture provides several foundation pieces that could support language-specific build file parsing, but there are fundamental timing constraints and missing infrastructure that would need to be addressed. The most promising approach is to extend the **module extension system** rather than the BUILD file evaluation system, as module extensions already have file reading capabilities and execute before analysis.

### Key Findings

1. **Existing Infrastructure**: Repository rules and module extensions already support file reading, tool execution, and repository generation - the core operations needed for manifest parsing.

2. **Timing Constraint**: Regular rule analysis cannot read file contents. Dynamic actions can read files but execute AFTER analysis, making them unsuitable for dependency graph construction.

3. **Recommended Approach**: Extend module extensions to generate synthetic BUILD files or repository content from language manifests, similar to how `crate_universe` and `pip.parse` work today.

4. **MODULE.lock Storage**: Already exists and could be extended to cache parsed manifest results for reproducibility and performance.

5. **DICE Integration**: Would require new DICE keys for tracking file dependencies during manifest parsing to enable incremental re-parsing.

## Detailed Findings

### Current Build File Discovery System

**Location**: `app/slug_common/src/buildfiles.rs`, `app/slug_common/src/find_buildfile.rs`

The build file discovery system is configured via buckconfig:

```ini
[buildfile]
name = BUILD.bazel,BUILD
```

**Current limitations**:
- Only exact filename matching (no globs or patterns)
- First-match-wins precedence from the configured list
- No mechanism to register custom parsers per file type
- Configured statically at cell definition time, not dynamically

**Extension potential**: The `BuildfilesKey` DICE key (`buildfiles.rs:74`) provides caching infrastructure. New file patterns could be added by:
1. Extending buckconfig to register `[buildfile] language_files = Cargo.toml,go.mod`
2. Creating a parser registry mapping patterns to Starlark parsing functions

### Repository Context (repository_ctx)

**Location**: `app/slug_interpreter_for_build/src/repository_ctx.rs`

Repository rules provide the most complete file I/O API:

```python
# Available operations
content = repository_ctx.read("Cargo.toml")
repository_ctx.file("BUILD.bazel", generated_content)
result = repository_ctx.execute(["cargo", "metadata", "--format-version=1"])
repository_ctx.download_and_extract(url, output, sha256)
repository_ctx.template("BUILD.bazel", template, substitutions)
```

**Key characteristics**:
- Working directory becomes the repository output (permanent)
- Direct filesystem I/O without DICE dependency tracking
- Invalidation based on attribute hash only, not file content changes
- Executes once per repository, results cached until attributes change

**Limitation for BUILD-time parsing**: Repository rules execute during bzlmod resolution, before analysis. They create entire repositories, not per-package BUILD files. A Cargo workspace would need to become a single repository with generated BUILD files for each crate.

### Module Extensions (module_ctx)

**Location**: `app/slug_interpreter_for_build/src/module_ctx.rs`, `app/slug_interpreter_for_build/src/module_extension.rs`

Module extensions aggregate configuration from all modules and create repositories:

```python
def _cargo_impl(module_ctx):
    for mod in module_ctx.modules:
        for workspace in mod.tags.from_cargo:
            # workspace.manifest is a Label pointing to Cargo.toml
            content = module_ctx.read(workspace.manifest)  # Currently stubbed!
            metadata = parse_cargo_toml(content)

            # Register repository for each crate
            for crate in metadata.crates:
                module_ctx.register_repo(
                    name = crate.name,
                    rule_class = "rust_library",
                    attributes = {"srcs": crate.srcs, "deps": crate.deps}
                )

cargo = module_extension(
    implementation = _cargo_impl,
    tag_classes = {
        "from_cargo": tag_class(attrs = {
            "manifest": attr.label(mandatory = True),
        }),
    },
)
```

**Current status**: The `module_ctx.read()` method exists but is **stubbed** (returns empty string). The infrastructure for path resolution and working directories exists.

**This is the recommended extension point** because:
1. Executes before analysis (can influence dependency graph)
2. Already has file reading API (needs implementation)
3. Can aggregate data from multiple modules
4. Creates repositories that become build targets
5. Results can be cached in MODULE.lock

### Dynamic Actions

**Location**: `app/slug_action_impl/src/context/dynamic_output.rs`, `app/slug_build_api/src/interpreter/rule_defs/artifact/starlark_artifact_value.rs`

Dynamic actions can read artifact contents during execution:

```python
def _impl(actions, config: ArtifactValue, out: OutputArtifact):
    content = config.read_string()
    data = config.read_json()
    # Generate output based on file contents
```

**Critical limitation**: Dynamic actions execute AFTER analysis completes. They can influence what gets built for their outputs, but cannot add new targets to the dependency graph. This makes them unsuitable for manifest-to-BUILD translation where new targets need to be created.

### DICE Integration Points

**Location**: `dice/dice/src/api/key.rs`, `app/slug_common/src/file_ops/dice.rs`

DICE provides incremental computation with automatic dependency tracking:

```rust
// Existing file operation keys
ReadFileKey { cell: CellName, path: CellRelativePathBuf }
ReadDirKey { cell: CellName, path: CellRelativePathBuf }
PathMetadataKey { cell: CellName, path: CellRelativePathBuf }
```

**For language file parsing, new keys would be needed**:

```rust
// Hypothetical new keys
ParsedManifestKey {
    cell: CellName,
    manifest_path: CellRelativePathBuf,
    parser_type: String,  // "cargo", "go", "cmake"
}

GeneratedBuildFileKey {
    cell: CellName,
    package_path: CellRelativePathBuf,
    source_manifests: Vec<CellRelativePathBuf>,
}
```

**File watching**: Slug uses Watchman/EdenFS/Notify for file change detection (`app/slug_file_watcher/`). Changes flow through DICE invalidation. Manifest file changes would automatically trigger re-parsing if properly tracked as DICE dependencies.

### MODULE.lock for Caching

**Location**: `app/slug_bzlmod/src/lockfile.rs`

The lockfile already stores:
- Resolved module versions
- Module extension results (repositories created)
- Content hashes for verification

**Extension for manifest caching**:

```json
{
  "moduleExtensions": {
    "@rules_rust//rust:extensions.bzl%crate": {
      "general": {
        "generatedRepoSpecs": {
          "crate_universe": {
            "ruleClass": "http_archive",
            "attributes": { ... }
          }
        },
        // NEW: Parsed manifest cache
        "parsedManifests": {
          "//third_party/rust:Cargo.toml": {
            "contentHash": "sha256-abc123...",
            "parsedAt": "2026-02-05T10:00:00Z",
            "result": {
              "crates": [
                {"name": "serde", "version": "1.0.193", "deps": ["serde_derive"]}
              ]
            }
          }
        }
      }
    }
  }
}
```

**Benefits**:
- Reproducible builds (same lock = same parse results)
- Fast rebuilds (skip parsing if manifest unchanged)
- Offline capability (parsed data available without re-running parsers)
- Auditable (can diff lockfile to see dependency changes)

## Architecture Options

### Option A: Extend Module Extensions (Recommended)

**Approach**: Implement `module_ctx.read()` and add manifest parsing to language rule extensions.

**Implementation steps**:

1. **Implement module_ctx I/O** (`module_ctx.rs:586-594`):
   ```rust
   fn read(this: &ModuleContext, path: Value, _watch: &str) -> starlark::Result<String> {
       let resolved = this.resolve_path(path.to_str()?)?;
       std::fs::read_to_string(&resolved)
           .map_err(|e| starlark::Error::new_other(e))
   }
   ```

2. **Add DICE dependency tracking**:
   - Record all files read during extension execution
   - Store file hashes in MODULE.lock
   - Invalidate extension results when files change

3. **Create language-specific extensions** (in rules_rust, rules_go, etc.):
   ```python
   # In @rules_rust//rust:extensions.bzl
   def _crate_impl(module_ctx):
       for mod in module_ctx.modules:
           for ws in mod.tags.from_cargo:
               cargo_toml = module_ctx.read(ws.manifest)
               # Parse and generate repos
   ```

4. **Cache results in MODULE.lock**:
   - Store parsed manifest data
   - Include content hashes for change detection
   - Enable `--lockfile-mode=update` to refresh

**Pros**:
- Minimal architectural changes
- Leverages existing bzlmod infrastructure
- Clear ownership (language rules own their parsers)
- Compatible with Bazel's direction

**Cons**:
- Repository granularity (can't generate per-package BUILD)
- Requires manifest at MODULE.bazel evaluation time
- Extension re-execution on any manifest change

### Option B: Custom Build File Parsers

**Approach**: Allow registering Starlark functions as parsers for specific file patterns.

**Implementation**:

1. **Extend buildfile configuration**:
   ```ini
   [buildfile]
   name = BUILD.bazel,BUILD

   [buildfile.parsers]
   Cargo.toml = @rules_rust//rust:cargo_parser.bzl%parse_cargo
   go.mod = @rules_go//go:gomod_parser.bzl%parse_gomod
   ```

2. **Create parser loading infrastructure**:
   ```rust
   // New DICE key
   struct BuildFileParserKey {
       cell: CellName,
       pattern: String,
   }

   // Returns loaded Starlark function
   impl Key for BuildFileParserKey {
       type Value = FrozenValue; // The parser function
   }
   ```

3. **Modify package listing** (`package_listing/interpreter.rs`):
   - When `Cargo.toml` found instead of BUILD file
   - Load and invoke registered parser
   - Parser returns synthetic BUILD file content
   - Continue evaluation as normal

4. **DICE tracking for parsed files**:
   ```rust
   struct ParsedBuildFileKey {
       cell: CellName,
       package: PackageLabel,
       manifest_path: CellRelativePathBuf,
       manifest_hash: String,
   }
   ```

**Pros**:
- Package-level granularity
- Integrates naturally with existing build file system
- Transparent to rule authors (just looks like BUILD files)

**Cons**:
- Significant interpreter changes required
- Potential performance issues (parsing on every analysis)
- Complex DICE invalidation
- Hermeticity concerns (parsers can read arbitrary files)

### Option C: Hybrid Approach

**Approach**: Use module extensions to generate BUILD files into a generated directory, then include that directory as a cell.

**Implementation**:

1. **Module extension generates BUILD files**:
   ```python
   def _cargo_workspace_impl(module_ctx):
       for ws in module_ctx.modules[0].tags.workspace:
           manifest = module_ctx.read(ws.cargo_toml)
           parsed = parse_cargo_manifest(manifest)

           for crate in parsed.crates:
               build_content = generate_rust_library(crate)
               module_ctx.file(
                   "{}/BUILD.bazel".format(crate.path),
                   build_content
               )
   ```

2. **Generated directory becomes a cell**:
   ```ini
   # In .buckconfig
   [cells]
   generated_rust = bazel-external/cargo_workspace
   ```

3. **Main BUILD files reference generated targets**:
   ```python
   # In //src/myapp:BUILD.bazel
   rust_binary(
       name = "myapp",
       deps = ["@generated_rust//serde:serde"],
   )
   ```

**Pros**:
- Uses existing infrastructure
- Generated BUILD files are debuggable
- Clear separation of generation and evaluation
- Can be incrementally adopted

**Cons**:
- Extra indirection (main BUILD references generated)
- Generation must complete before analysis starts
- Changes require re-running extension

## Concrete Hook Points for BUILD File Generation

Based on detailed analysis of the code flow, here are the specific locations where hooks for build file parsing/generation could be inserted:

### Hook Point 1: Content Injection in `parse_file()` (Recommended)

**Location**: `app/slug_interpreter_for_build/src/interpreter/dice_calculation_delegate.rs:182-197`

This is the most surgical insertion point - after file content is read but before parsing:

```rust
async fn parse_file(
    &mut self,
    starlark_path: StarlarkPath<'_>,
) -> slug_error::Result<ParseResult> {
    // Line 187: Read original file content
    let content = DiceFileComputations::read_file(
        self.ctx,
        starlark_path.path().as_ref().as_ref()
    ).await?;

    // >>> HOOK POINT: Augment or replace content <<<
    let content = if let StarlarkPath::BuildFile(build_file) = starlark_path {
        self.generate_or_augment_build_content(build_file, content).await?
    } else {
        content
    };

    // Line 196: Parse (potentially modified) content to AST
    self.configs.parse(starlark_path, content)
}
```

**New method to add**:
```rust
async fn generate_or_augment_build_content(
    &mut self,
    build_file: &BuildFilePath,
    original_content: String,
) -> slug_error::Result<String> {
    let package_path = build_file.package().as_cell_path();

    // Check for generator files in this package
    let listing = self.resolve_package_listing(build_file.package().dupe()).await?;

    // Look for Cargo.toml, go.mod, etc.
    if listing.get_file(PackageRelativePath::new("Cargo.toml")?) {
        let cargo_content = DiceFileComputations::read_file(
            self.ctx,
            &package_path.join(ForwardRelativePath::new("Cargo.toml")?)
        ).await?;

        // Parse and generate targets
        let generated = self.generate_cargo_targets(&cargo_content)?;

        // Prepend to original BUILD content
        Ok(format!("{}\n\n# Original BUILD content:\n{}", generated, original_content))
    } else {
        Ok(original_content)
    }
}
```

**Advantages**:
- Single chokepoint - all BUILD files flow through here
- Content is a simple String - easy to prepend/append/replace
- DICE tracks file reads automatically (Cargo.toml becomes a dependency)
- Error context preserved
- Minimal architectural changes

### Hook Point 2: New DICE Key for Generated Content

**Location**: Create new file `app/slug_interpreter_for_build/src/interpreter/generated_build.rs`

For better caching and separation of concerns, create a dedicated DICE key:

```rust
/// DICE key for generating BUILD file content from language manifests
#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display(fmt = "GeneratedBuildContent({})", _0)]
pub struct GeneratedBuildContentKey(pub PackageLabel);

#[async_trait]
impl Key for GeneratedBuildContentKey {
    type Value = slug_error::Result<Option<GeneratedBuildContent>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellation: &CancellationContext,
    ) -> Self::Value {
        // Get package listing to find generator files
        let listing = ctx.resolve_package_listing(self.0.dupe()).await?;

        // Check for various manifest types
        let generators = find_generator_files(&listing);
        if generators.is_empty() {
            return Ok(None);
        }

        let mut generated_content = String::new();

        for generator in generators {
            match generator {
                GeneratorFile::CargoToml(path) => {
                    let content = ctx.file_ops().read_file(&path).await?;
                    generated_content.push_str(&generate_rust_targets(&content)?);
                }
                GeneratorFile::GoMod(path) => {
                    let content = ctx.file_ops().read_file(&path).await?;
                    generated_content.push_str(&generate_go_targets(&content)?);
                }
                // ... other generators
            }
        }

        Ok(Some(GeneratedBuildContent {
            content: generated_content,
            source_files: generators.iter().map(|g| g.path()).collect(),
        }))
    }
}

pub struct GeneratedBuildContent {
    pub content: String,
    pub source_files: Vec<CellRelativePathBuf>,  // For debugging/auditing
}
```

**Usage in parse_file()**:
```rust
async fn parse_file(&mut self, starlark_path: StarlarkPath<'_>) -> ParseResult {
    let content = DiceFileComputations::read_file(...).await?;

    // Check for generated content
    if let StarlarkPath::BuildFile(build_file) = starlark_path {
        if let Some(generated) = self.ctx
            .compute(&GeneratedBuildContentKey(build_file.package().dupe()))
            .await?
        {
            let content = format!("{}\n{}", generated.content, content);
        }
    }

    self.configs.parse(starlark_path, content)
}
```

**Advantages**:
- Generated content cached separately from BUILD file parsing
- Manifest file changes automatically invalidate via DICE
- Clear separation: generation vs evaluation
- Can be extended with different generator types

### Hook Point 3: Configurable Generator Registry

**Location**: `app/slug_common/src/buildfiles.rs` (extend existing)

Add configuration for which files trigger generation:

```ini
# In .buckconfig
[buildfile]
name = BUILD.bazel,BUILD

[buildfile.generators]
# Format: pattern = @cell//path:generator.bzl%function_name
Cargo.toml = @rules_rust//rust:generators.bzl%generate_from_cargo
go.mod = @rules_go//go:generators.bzl%generate_from_gomod
CMakeLists.txt = @rules_cc//cc:generators.bzl%generate_from_cmake
```

**Implementation**:
```rust
// In buildfiles.rs
pub struct BuildfileGenerators {
    generators: HashMap<FileNameBuf, StarlarkFunctionPath>,
}

impl BuildfileGenerators {
    pub fn from_config(config: &LegacyBuckConfigView) -> Result<Self> {
        let mut generators = HashMap::new();

        if let Some(section) = config.get_section("buildfile.generators") {
            for (pattern, function_path) in section.iter() {
                generators.insert(
                    FileNameBuf::try_from(pattern)?,
                    StarlarkFunctionPath::parse(function_path)?,
                );
            }
        }

        Ok(Self { generators })
    }

    pub fn get_generator(&self, filename: &FileName) -> Option<&StarlarkFunctionPath> {
        self.generators.get(filename)
    }
}
```

**Generator function signature** (in Starlark):
```python
# In @rules_rust//rust:generators.bzl
def generate_from_cargo(ctx):
    """
    Args:
        ctx: GeneratorContext with:
            - ctx.read_file(path) -> string
            - ctx.package_path -> string
            - ctx.package_listing -> PackageListing

    Returns:
        string: Generated BUILD file content
    """
    cargo_toml = ctx.read_file("Cargo.toml")
    parsed = parse_toml(cargo_toml)

    targets = []
    for crate in parsed.get("workspace", {}).get("members", []):
        targets.append(rust_library_target(crate))

    return "\n".join(targets)
```

### Hook Point 4: Target Injection via ModuleInternals

**Location**: `app/slug_interpreter_for_build/src/interpreter/module_internals.rs`

For cases where generating BUILD content isn't enough (need programmatic target creation):

```rust
impl ModuleInternals {
    /// Inject pre-created targets before BUILD evaluation
    pub fn inject_targets(&self, targets: Vec<TargetNode>) -> Result<()> {
        let mut state = self.state.borrow_mut();

        // Ensure we're in recording state
        let recorder = match &mut *state {
            State::BeforeTargets(before) => {
                // Transition to recording with injected targets
                let package = self.create_package(before)?;
                let mut recorder = TargetsRecorder::new();
                for target in targets {
                    recorder.record(target)?;
                }
                *state = State::RecordingTargets(RecordingTargets {
                    package,
                    recorder
                });
                return Ok(());
            }
            State::RecordingTargets(recording) => &mut recording.recorder,
        };

        // Add to existing recorder
        for target in targets {
            recorder.record(target)?;
        }
        Ok(())
    }
}
```

**Usage in configuror.rs** (when creating ModuleInternals):
```rust
pub fn new_extra_context(...) -> Result<(Module, ModuleInternals)> {
    let internals = ModuleInternals::new(...);

    // Check for generated targets
    if let Some(generated) = generated_targets_for_package(package, &package_listing)? {
        internals.inject_targets(generated)?;
    }

    Ok((env, internals))
}
```

### Hook Point 5: Package Listing Extension

**Location**: `app/slug_common/src/package_listing/interpreter.rs:390-407`

Generate BUILD file during directory scanning:

```rust
// In Directory::gather()
let entries = DiceFileComputations::read_dir_ext(ctx, cell_path.as_ref()).await?;

// >>> HOOK: Check for generator files and generate BUILD <<<
let entries = if should_generate_build(buildfile_candidates, &entries) {
    // Find generator file (e.g., Cargo.toml)
    if let Some(generator_file) = find_generator_file(&entries) {
        // Generate BUILD file content
        let generated = generate_build_from_file(ctx, cell_path, &generator_file).await?;

        // Write to a temporary or synthetic location
        // OR: Return modified entries with virtual BUILD file
        add_virtual_build_file(entries, generated)
    } else {
        entries
    }
} else {
    entries
};

let buildfile = find_buildfile(buildfile_candidates, &entries);
```

### Summary: Recommended Architecture

For a Gazelle-like experience, I recommend combining **Hook Points 1, 2, and 3**:

```
┌─────────────────────────────────────────────────────────────────┐
│                     BUILD File Evaluation Flow                   │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. User requests //foo:bar                                      │
│                    │                                             │
│                    ▼                                             │
│  2. PackageListingKey(//foo)                                     │
│     └─ Scans directory, finds BUILD.bazel + Cargo.toml          │
│                    │                                             │
│                    ▼                                             │
│  3. GeneratedBuildContentKey(//foo)  ◄─── NEW DICE KEY          │
│     ├─ Checks generator registry (.buckconfig)                  │
│     ├─ Reads Cargo.toml via DICE (tracks dependency)            │
│     ├─ Invokes @rules_rust//...:generators.bzl%generate_cargo   │
│     └─ Returns generated Starlark content                       │
│                    │                                             │
│                    ▼                                             │
│  4. parse_file() in DiceCalculationDelegate                     │
│     ├─ Reads BUILD.bazel content                                │
│     ├─ Prepends generated content from step 3                   │
│     └─ Parses combined content to AST                           │
│                    │                                             │
│                    ▼                                             │
│  5. Normal BUILD evaluation                                      │
│     └─ Rules execute, targets registered in ModuleInternals     │
│                    │                                             │
│                    ▼                                             │
│  6. EvaluationResult with all targets                           │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Key benefits**:
1. **Incremental**: DICE tracks Cargo.toml as dependency, regenerates only when changed
2. **Configurable**: Generator registry in .buckconfig, different generators per file type
3. **Extensible**: Generators are Starlark functions, users can write custom ones
4. **Debuggable**: Generated content is just Starlark, can be inspected
5. **Composable**: Generated content merges with hand-written BUILD content

## Buck2-Specific Features as Extension Points

Buck2 (Slug's foundation) provides several unique features not found in Bazel that could serve as better entrypoints for language-specific parsing. These deserve special consideration.

### Anonymous Targets (Most Promising)

**Location**: `app/slug_anon_target/`

Anonymous targets are build targets created programmatically during analysis, identified by content hash rather than BUILD file labels. This is **the most promising Buck2 feature** for manifest-based target generation.

**How they work**:

```python
def _my_rule_impl(ctx):
    # Parse manifest during analysis
    parsed = parse_cargo_toml(ctx.attrs.cargo_toml)

    # Dynamically create targets for each crate
    anon_targets = []
    for crate in parsed.crates:
        anon = ctx.actions.anon_target(
            rust_library,
            {
                "name": crate.name,
                "srcs": crate.srcs,
                "deps": crate.deps,
            }
        )
        anon_targets.append(anon)

    # Wait for all anon targets to resolve
    return ctx.actions.anon_targets(anon_targets)
```

**Key characteristics**:
- **Content-addressed**: Same attributes = same target (automatic deduplication)
- **Created during analysis**: Can influence dependency graph (unlike dynamic actions)
- **DICE integrated**: Full caching and incremental computation via `AnonTargetKey`
- **Promise-based**: Results available via `StarlarkPromise` for downstream consumption

**Why this is powerful for manifest parsing**:
1. A "manifest rule" could parse Cargo.toml/go.mod at analysis time
2. Create anonymous targets for each discovered crate/package
3. Targets are deduplicated if multiple rules reference the same manifest
4. DICE handles caching - same manifest = cached anon target results

**Current limitation**: No direct file reading in analysis context. Would need:
- A way to read source files during analysis, OR
- A "pre-analysis" phase that provides parsed data to rules

**Code references**:
- `app/slug_anon_target/src/anon_targets.rs:222-256` - Target creation
- `app/slug_anon_target/src/starlark_defs.rs:257-278` - `ctx.actions.anon_target()` API
- `app/slug_anon_target/src/anon_promises.rs:51-101` - Promise resolution

### BXL (Buck Extension Language)

**Location**: `app/slug_bxl/`

BXL is a top-level scripting system for build introspection. Unlike rules, BXL can access the filesystem and create actions without being triggered by a target.

**File system capabilities** (`ctx.fs`):

```python
def _bxl_impl(ctx):
    # Check if manifest exists
    if ctx.fs.exists("Cargo.toml"):
        # Get source artifact
        manifest = ctx.fs.source("Cargo.toml")

        # List directory contents
        src_files = ctx.fs.list("src/")

        # Create actions based on discovery
        actions = ctx.bxl_actions().actions
        output = actions.declare_output("generated_build.json")
        actions.run(["cargo", "metadata", "--format-version=1"],
                    outputs=[output.as_output()])
```

**BXL advantages**:
- Can inspect filesystem metadata (`exists`, `is_dir`, `list`)
- Can create actions and artifacts
- Can query the build graph (`ctx.uquery()`, `ctx.cquery()`)
- Results are DICE-cached

**BXL limitations for this use case**:
- **Cannot read file contents directly** - No `ctx.fs.read()` method
- Must shell out to tools or use actions to process files
- Runs as separate command, not integrated into normal build flow

**Potential architecture**: BXL as a "pre-build" step:
1. Run `slug bxl //tools:generate_build_files.bxl`
2. BXL discovers manifests, invokes parsers via actions
3. Outputs generated BUILD files or target metadata
4. Normal `slug build` uses generated content

**Code references**:
- `app/slug_bxl/src/bxl/starlark_defs/context/fs.rs:167-362` - Filesystem operations
- `app/slug_bxl/src/bxl/starlark_defs/context/actions.rs:190-212` - Action creation

### PACKAGE Files (Target Injection)

**Location**: `app/slug_interpreter_for_build/src/super_package/`

PACKAGE files set package-level configuration inherited by all BUILD files in subdirectories. This could be extended to inject targets.

**Current capabilities**:

```python
# In PACKAGE file
package(
    visibility = ["PUBLIC"],
)

write_package_value("myapp.config", {"feature_flags": ["foo", "bar"]})
```

**Extension possibility**: Add `inject_targets()` function:

```python
# Hypothetical PACKAGE file
for proto in glob(["**/*.proto"]):
    inject_target(
        name = proto.replace("/", "_").replace(".proto", "_proto"),
        rule = proto_library,
        attrs = {"srcs": [proto]},
    )
```

**How it could work**:
1. PACKAGE evaluation happens before BUILD evaluation
2. PACKAGE has access to `glob()` for file discovery
3. Injected targets stored in `SuperPackage.targets`
4. BUILD evaluation merges injected + declared targets

**Advantages**:
- Hierarchical (parent PACKAGE affects all subdirectories)
- Already integrated with DICE
- File changes would invalidate affected packages

**Challenges**:
- Currently no target injection mechanism
- Would blur PACKAGE (config) vs BUILD (targets) separation
- Need conflict resolution for injected vs declared targets

**Code references**:
- `app/slug_interpreter_for_build/src/super_package/package.rs:105-165` - `package()` function
- `app/slug_interpreter_for_build/src/interpreter/dice_calculation_delegate.rs:441-479` - PACKAGE evaluation

### Deferred Computations (Lazy Parsing)

**Location**: `app/slug_core/src/deferred/`, `app/slug_build_api/src/deferred/`

Deferred computations allow lazy evaluation - computations are registered during analysis but only executed when outputs are needed.

**Current usage**: Actions are deferred - registered during analysis, executed on demand.

**Extension for manifest parsing**:

```rust
// Hypothetical: Deferred manifest parsing
pub struct ManifestParseKey {
    manifest_path: CellRelativePathBuf,
    parser: String,  // "cargo", "go", etc.
}

impl Key for ManifestParseKey {
    type Value = ParsedManifest;

    async fn compute(&self, ctx: &mut DiceComputations) -> Self::Value {
        let content = ctx.file_ops().read_file(&self.manifest_path).await?;
        parse_manifest(&self.parser, &content)
    }
}
```

**In Starlark**:

```python
def _rule_impl(ctx):
    # Returns a "deferred" parsed manifest
    parsed = native.parse_manifest(ctx.attrs.cargo_toml, parser="cargo")

    # Can still create targets based on it
    # The parsing is lazy - only happens if this rule is actually built
```

**Advantages**:
- Parsing only happens if targets are actually needed
- DICE handles caching and invalidation
- Fits existing deferred computation model

**Challenges**:
- Analysis phase expects synchronous results for target creation
- Would need "resolve deferred" step before analysis completes

**Code references**:
- `app/slug_core/src/deferred/key.rs:40-42` - `DeferredHolderKey` enum
- `app/slug_build_api/src/deferred/calculation.rs:83-93` - `lookup_deferred_holder()`

### Subrules (Rule Composition)

**Location**: `app/slug_interpreter_for_build/src/subrule.rs`

Subrules allow decomposing rules into reusable action-generating functions. Could be used for manifest-to-action translation.

**Example pattern**:

```python
# Define a subrule that generates actions from manifest data
_cargo_compile = subrule(
    implementation = _cargo_compile_impl,
    attrs = {
        "crate_info": attr.any(),  # Parsed crate metadata
    },
)

def _cargo_library_impl(ctx):
    # Parse manifest
    parsed = parse_cargo(ctx.attrs.manifest)

    # Use subrule for each crate
    for crate in parsed.crates:
        _cargo_compile(ctx, crate_info=crate)
```

**Advantages**:
- Composable rule logic
- Can be shared across multiple rules
- Clear separation: parsing vs compilation

**Code references**:
- `app/slug_interpreter_for_build/src/subrule.rs` - Subrule implementation
- `thoughts/shared/plans/slug-bazel-subplans/02-bzlmod-phase-8-subrule.md` - Design plans

### Recommended Buck2-Native Architecture

Combining these features, here's a Buck2-native approach:

#### Option D: Anonymous Targets + Package Values

1. **PACKAGE file discovers manifests**:
   ```python
   # PACKAGE
   for manifest in glob(["**/Cargo.toml"]):
       write_package_value(
           "cargo.manifests." + manifest.replace("/", "_"),
           manifest
       )
   ```

2. **Rule reads package values and creates anon targets**:
   ```python
   def _cargo_workspace_impl(ctx):
       manifests = [v for k, v in read_package_values()
                    if k.startswith("cargo.manifests.")]

       for manifest in manifests:
           # Would need file reading here
           parsed = ctx.parse_cargo(manifest)

           for crate in parsed.crates:
               ctx.actions.anon_target(rust_library, {
                   "name": crate.name,
                   "srcs": crate.srcs,
               })
   ```

3. **DICE tracks everything**:
   - Manifest file changes → package value changes → rule re-analysis → new anon targets

#### Option E: BXL Pre-Generation

1. **BXL script discovers and parses**:
   ```python
   # generate_targets.bxl
   def _impl(ctx):
       actions = ctx.bxl_actions().actions

       # Find all Cargo.toml files
       for manifest in find_manifests(ctx):
           # Create parsing action
           parsed = actions.declare_output(manifest + ".parsed.json")
           actions.run(["cargo", "metadata", "--manifest-path", manifest],
                       outputs=[parsed.as_output()])

           # Output for consumption
           ctx.output.ensure(parsed)
   ```

2. **Commit generated content or use as input**:
   - Generated JSON consumed by rules
   - Or generate BUILD files directly

3. **Normal build uses results**:
   ```bash
   slug bxl //tools:generate_targets.bxl  # Pre-step
   slug build //...  # Uses generated content
   ```

## Difficulties and Challenges

### 1. Timing and Phase Separation

**Problem**: Slug strictly separates loading (MODULE.bazel), analysis (BUILD evaluation), and execution (action running). File reading during analysis would blur these boundaries.

**Impact**:
- Adding new targets based on file contents violates the current model
- DICE assumes analysis results depend only on target attributes, not arbitrary files

**Mitigation**:
- Keep file reading in module extension phase (Option A)
- Generate all targets before analysis starts
- Track file dependencies explicitly in lockfile

### 2. Incrementality and DICE Integration

**Problem**: If parsing depends on file contents, DICE needs to track those dependencies for proper invalidation.

**Current gap**: `repository_ctx.read()` and `module_ctx.read()` perform direct filesystem I/O without DICE tracking.

**Required changes**:
```rust
// Add async file reading with DICE tracking
async fn read_with_tracking(
    ctx: &mut DiceComputations<'_>,
    path: &CellPath,
) -> Result<(String, ContentHash)> {
    let content = ctx.file_ops().read_file(path).await?;
    let hash = compute_content_hash(&content);
    Ok((content, hash))
}
```

### 3. Hermeticity Concerns

**Problem**: Language parsers might read files outside the declared manifest, breaking reproducibility.

**Examples**:
- Cargo.toml can include workspace members via globs
- CMakeLists.txt can include() arbitrary files
- go.mod can reference replace directives to local paths

**Mitigation**:
- Sandbox file access to declared paths only
- Require explicit declaration of all input files
- Validate that parsed results are deterministic
- Store all inputs in lockfile for auditing

### 4. Parser Complexity

**Problem**: Implementing parsers for Cargo.toml, go.mod, CMakeLists.txt in Starlark is complex.

**Options**:
a. **Pure Starlark parsers**: Simple but limited (can handle TOML/JSON, struggle with CMake)
b. **Shell out to tools**: `cargo metadata --format-version=1` returns JSON
c. **Native parser functions**: Add `native.parse_toml()`, `native.parse_json()` builtins

**Recommendation**: Combination approach:
- Add `native.parse_toml()` and `native.parse_json()` for common formats
- Allow `module_ctx.execute()` for complex tools like `cargo metadata`
- Parsers in rules_* repositories handle language-specific logic

### 5. Performance

**Problem**: Parsing large manifests or invoking external tools on every build is slow.

**Analysis**:
- Cargo workspace with 100 crates: `cargo metadata` takes ~2-5 seconds
- go.mod with many dependencies: `go list -m -json all` takes ~1-3 seconds
- CMake configuration: can take 10+ seconds

**Mitigation**:
- Cache parsed results in MODULE.lock
- Only re-parse when manifest content hash changes
- Parallelize parsing across independent manifests
- Provide `--lockfile-mode=off` to skip parsing in CI

### 6. Error Handling and Debugging

**Problem**: Parse errors in language files need clear error messages with location information.

**Requirements**:
- Show file path and line number for parse errors
- Distinguish between parse errors and generation errors
- Provide "what would be generated" debugging mode
- Allow overriding generated targets if parser is wrong

### 7. Workspace vs Package Granularity

**Problem**: Some languages have workspace-level manifests (Cargo, Go), others have per-directory (CMake).

**Implications**:
- Cargo workspace: One Cargo.toml generates many rust_library targets
- CMake: Each CMakeLists.txt generates targets for that directory
- Go: One go.mod for the module, but targets per package

**Architecture consideration**: Module extensions naturally handle workspace-level, but per-directory generation needs special handling.

## Code References

### Build File Discovery
- `app/slug_common/src/buildfiles.rs:26` - DEFAULT_BUILDFILES constant
- `app/slug_common/src/buildfiles.rs:74` - BuildfilesKey DICE integration
- `app/slug_common/src/find_buildfile.rs:16-28` - find_buildfile() function

### Repository Context
- `app/slug_interpreter_for_build/src/repository_ctx.rs:1206-1214` - ctx.read() implementation
- `app/slug_interpreter_for_build/src/repository_ctx.rs:1024-1088` - ctx.execute() implementation
- `app/slug_interpreter_for_build/src/repository_ctx.rs:979-1021` - ctx.file() implementation

### Module Extensions
- `app/slug_interpreter_for_build/src/module_ctx.rs:586-594` - module_ctx.read() stub
- `app/slug_interpreter_for_build/src/module_ctx.rs:403-533` - ModuleContext struct
- `app/slug_interpreter_for_build/src/extension_execution.rs:268-276` - register_repo()

### DICE Integration
- `app/slug_common/src/file_ops/dice.rs:285` - ReadFileKey
- `app/slug_bzlmod/src/repository_execution.rs:127-158` - RepositoryRuleExecutionKey
- `dice/dice/src/api/key.rs` - Key trait definition

### Lockfile
- `app/slug_bzlmod/src/lockfile.rs` - MODULE.lock handling

### Hook Points for BUILD Generation
- `app/slug_interpreter_for_build/src/interpreter/dice_calculation_delegate.rs:182-197` - `parse_file()` - content injection point
- `app/slug_interpreter_for_build/src/interpreter/dice_calculation_delegate.rs:571-583` - After package listing, before BUILD evaluation
- `app/slug_interpreter_for_build/src/interpreter/module_internals.rs:155-167` - `record()` - target registration
- `app/slug_interpreter_for_build/src/interpreter/configuror.rs:146-155` - ModuleInternals creation
- `app/slug_common/src/package_listing/interpreter.rs:390-407` - Directory scanning where generator files detected

### Anonymous Targets (Buck2-specific)
- `app/slug_anon_target/src/anon_targets.rs:222-256` - Target creation and hashing
- `app/slug_anon_target/src/starlark_defs.rs:257-278` - `ctx.actions.anon_target()` Starlark API
- `app/slug_anon_target/src/anon_promises.rs:51-101` - Promise resolution after analysis
- `app/slug_anon_target/src/anon_target_node.rs:64-82` - AnonTarget struct definition

### BXL (Buck2-specific)
- `app/slug_bxl/src/bxl/starlark_defs/context/fs.rs:167-362` - Filesystem operations (exists, list, source)
- `app/slug_bxl/src/bxl/starlark_defs/context/actions.rs:190-212` - `ctx.bxl_actions()` for action creation
- `app/slug_bxl/src/bxl/starlark_defs/context/methods.rs:421-544` - BXL context methods

### PACKAGE Files (Buck2-specific)
- `app/slug_interpreter_for_build/src/super_package/package.rs:105-165` - `package()` function
- `app/slug_interpreter_for_build/src/super_package/package_value.rs:227-234` - `write_package_value()`
- `app/slug_interpreter_for_build/src/interpreter/dice_calculation_delegate.rs:441-479` - PACKAGE evaluation

### Deferred Computations (Buck2-specific)
- `app/slug_core/src/deferred/key.rs:40-42` - DeferredHolderKey enum variants
- `app/slug_build_api/src/deferred/calculation.rs:83-93` - lookup_deferred_holder()
- `app/slug_build_api/src/actions/calculation.rs:653-683` - BuildKey DICE implementation

## Historical Context (from thoughts/)

The thoughts directory contains extensive planning for language integration:

- `thoughts/shared/plans/slug-bazel-subplans/07-rules-integration.md` - Plans for rules_rust (crate_universe), rules_python (pip.parse)
- `thoughts/shared/plans/slug-bazel-subplans/02-bzlmod-phase-5e.md` - Module extension execution with deferred repository model
- `thoughts/shared/plans/slug-bazel-subplans/02-bzlmod.md` - Overall bzlmod philosophy

**Key insight**: The current plan follows Bazel's model where language package managers integrate via module extensions (crate_universe, pip.parse, rules_go), not via Gazelle-style BUILD file generation. This research explores extending that model.

## Recommendations

### Short-term (leverage existing infrastructure)

1. **Implement module_ctx.read()** - Enable file reading in module extensions
2. **Add native.parse_toml()** - Support Cargo.toml parsing in Starlark
3. **Extend MODULE.lock** - Cache parsed manifest data with content hashes
4. **Document the pattern** - Show how rules_* can use extensions for manifest parsing

### Medium-term (improve incrementality)

1. **DICE tracking for extension file reads** - Automatic invalidation on file changes
2. **Parallel extension execution** - Parse independent manifests concurrently
3. **Extension result diffing** - Show what changed between lockfile versions

### Medium-term (Buck2-native features)

1. **Add file reading to analysis context** - Enable `ctx.read_source()` for rules to read source files during analysis (with DICE tracking)
2. **Extend anonymous targets for manifest parsing** - Create "manifest rule" pattern that parses files and creates anon targets
3. **Add BXL filesystem content reading** - Implement `ctx.fs.read()` for BXL scripts

### Long-term (new capabilities)

1. **Custom build file parsers** (Option B) - Allow per-pattern Starlark parsers
2. **Generated BUILD caching** - Cache generated BUILD file content
3. **CMake integration** - Explore CMakeLists.txt translation (hardest case)
4. **PACKAGE file target injection** - Allow PACKAGE files to inject targets based on file discovery

## Open Questions

1. **Should generated BUILD files be visible?** Or should they be purely internal to the build system?

2. **How to handle parser failures?** Fall back to empty targets? Fail the build? Allow override?

3. **What about languages without manifests?** (e.g., C/C++ with just source files) - This is Gazelle's domain, not manifest parsing.

4. **Cross-language dependencies?** A Cargo.toml that depends on a C library built by CMake requires coordination.

5. **Remote execution compatibility?** Parsers that shell out to tools need those tools available in the execution environment.
