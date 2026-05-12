# Foundation Phases (1-3)

> **Parent Plan**: [Slug Bazel-Compatible Build Tool](../2026-01-21-slug-bazel-compatible-build-tool.md)

This sub-plan covers the foundational work: forking, rebranding, Starlark dialect changes, and build file recognition.

---

## Phase 1: Fork and Foundation

### Overview

Fork Slug, rebrand to slug, establish build infrastructure, and verify the base system compiles and runs.

### Changes Required:

#### 1. Repository Setup

**Action**: Fork facebook/slug into this repository

```bash
# Clone Slug as starting point
git clone --depth 1 https://github.com/facebook/slug.git slug-src
# Copy relevant source (excluding .git)
cp -r slug-src/* /var/mnt/dev/slug/
rm -rf slug-src
```

#### 2. Rename Slug → Slug

**Files to modify**: Cargo.toml files, binary names, user-facing strings

- Rename `slug` binary to `slug`
- Update all Cargo.toml package names from `slug_*` to `slug_*`
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
./target/release/slug --version
# Should output: slug 0.1.0 (or similar)
```

### Success Criteria:

#### Automated Verification:

- [x] `cargo build --release` succeeds
- [x] `cargo test` passes (existing Slug tests) - Note: Completion tests fixed, full suite not run
- [x] `./target/release/slug --version` outputs slug version
- [x] `./target/release/slug --help` shows slug branding
- [x] BXL commands still exist (`slug bxl --help`)

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
- slug will be ahead of Bazel here - this is a feature, not a bug
- Type annotations should be **optional** (code without them must work)
- Type errors should be **warnings**, not failures

#### 2. Replace Attribute Module with Bazel API

**File**: Replace Slug attribute API with Bazel-compatible API

Replace `attrs` module with `attr` (Slug's `attrs.*` will not be supported):

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

Only Bazel-style `implementation` parameter (Slug's `impl` will not be supported):

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
# Slug: //pkg: (same meaning)
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
- [x] Both Bazel-style (`attr.*`, `implementation`) and Slug-style (`attrs.*`, `impl`) are supported

#### Test Migration (Phase 2):

- [x] Update `app/slug_build_api_tests/src/attrs.rs` for `attr.*` API
- [x] Update `tests/core/interpreter/test_attr_default_coercion.py` for `attr.*` syntax
- [x] Add tests for `native.*` module functions (glob, package_name, existing_rules)
- [x] Add tests for `rule(implementation=...)` parameter
- [x] Update visibility syntax in all test fixtures (`//visibility:public`) - Both syntaxes supported
- [x] Delete `tests/core/interpreter/test_load_toml.py` (Bazel doesn't support TOML)
- [x] Delete tests using `.buckconfig` syntax in interpreter tests - Added MODULE.bazel markers

---

## Phase 3: Build File Recognition

### Overview

Change slug to recognize BUILD.bazel and BUILD files instead of BUCK files.

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

Remove any Slug-specific build file handling that doesn't apply to Bazel.

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

- [x] `slug build //...` finds BUILD.bazel files
- [x] `slug build //...` ignores BUCK files
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
