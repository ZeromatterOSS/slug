# Rules Integration Phases (9-12)

> **Parent Plan**: [Kuro Bazel-Compatible Build Tool](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> **Key Research**: [rules_cc Native Requirements](../research/2026-01-26-rules-cc-native-requirements.md)

This sub-plan covers integration with the rules_* ecosystem: rules_cc, rules_rust, rules_python, and rules_oci.

---

## Phase 9: rules_cc Integration

### Current Status (2026-02-03)

**Implemented:**
- [x] Native `cc_common` module with `internal_DO_NOT_USE()` method
- [x] `CcCommonInternal` struct with internal functions (stubs)
- [x] `CcToolchainVariables` type
- [x] `CcInfo` and `CcToolchainInfo` provider stubs
- [x] Public API methods: `get_tool_for_action`, `get_execution_requirements`, `action_is_enabled`, `get_memory_inefficient_command_line`, `get_environment_variables`, `empty_variables`
- [x] Internal methods: `create_cc_compile_action`, `get_artifact_name_for_category`, `combine_cc_toolchain_variables`, `actions2ctx_cheat`, `freeze`, etc.
- [x] **`get_link_args` working** - returns proper linker arguments with full buck-out paths
- [x] **`artifact.path` returns full buck-out paths** - critical for Bazel compatibility
- [x] **cc_library compiles and links successfully** - produces both .a and .so files

**Working End-to-End:**
- [x] `cc_common.compile()` - creates compile actions with proper outputs
- [x] `cc_common.link()` / archive - creates static library (.a)
- [x] `cc_common.link()` / shared - creates shared library (.so) with `-shared` flag

**Blocking:**
- [ ] Transitive dependency resolution - rules_cc depends on `protobuf` (via `repo_name = "com_google_protobuf"`) and `platforms`, which aren't being resolved

**Completed (2026-02-04):**
- [x] `repo_name` aliasing in bzlmod - `bazel_dep(..., repo_name = "alias")` now creates cell aliases correctly
  - Aliases are collected from transitive deps' MODULE.bazel via `collect_transitive_repo_aliases()`
  - Registered with cell resolver via `CellsAggregator::new(root_aliases)`
  - Example: `com_google_protobuf -> protobuf`, `com_google_absl -> abseil-cpp`, `io_bazel_stardoc -> stardoc`

**Files:**
- `app/kuro_build_api/src/interpreter/rule_defs/cc_common.rs` - cc_common implementation
- `app/kuro_execute/src/path/artifact_path.rs` - artifact path resolution (full buck-out paths)
- `app/kuro_build_api/src/interpreter/rule_defs/artifact/methods.rs` - artifact `.path` attribute
- `tests/manual_test/BUILD.bazel` - cc_common verification tests

### Key Learnings (2026-02-03)

#### 1. Bazel vs Buck2 Artifact Path Model

**Critical Insight:** Bazel's `File.path` attribute returns the full execution-time path (e.g., `bazel-out/k8-fastbuild/bin/pkg/__target__/file.o`), while Buck2's original `with_full_path` only returned the artifact's relative path (e.g., `_objs/cc_library-compile/hello.o`).

**Why This Matters:** rules_cc stores `object_file.path` as a string in `_NamedLibraryInfo.name`. When `get_link_args` later uses this string in the linker command line, it must be a valid execution-time path. If the path is relative, the linker can't find the file.

**Fix:** Modified `ArtifactPath::with_full_path()` in `artifact_path.rs` to construct the full buck-out path:
```
buck-out/v2/gen/<cell>/<cfg_hash>[/<pkg_path>]/__<target>__/<artifact_path>
```

#### 2. Buck2 Dependency Tracking via Artifacts vs Strings

**Critical Insight:** Buck2 tracks action dependencies by visiting artifacts in command lines (via `CommandLineArgLike::visit_artifacts`). When a string is added to cmd_args, it's used verbatim without dependency tracking. When an artifact is added, Buck2 automatically:
1. Resolves it to the full buck-out path
2. Tracks it as an input dependency

**Implication for Bazel Compatibility:** Bazel rules often pass string paths (from `.path`) through providers, expecting them to work in commands. For Kuro compatibility, either:
- Make `.path` return the full execution path (implemented), OR
- Maintain an artifact registry to resolve strings back to artifacts

#### 3. Action Input Dependencies via `bazel_inputs`

**Problem:** Bazel's `actions.run(inputs=...)` explicitly specifies input dependencies. Buck2's run action infers dependencies from command line artifacts. When rules_cc passes string paths (not artifacts) in the command, Buck2 doesn't see them as dependencies.

**Fix:** Added `bazel_inputs` field to `StarlarkRunActionValues` to explicitly track Bazel-style inputs. These are visited in `visit_artifacts()` to ensure proper dependency ordering.

#### 4. Buck-Out Path Structure

The full buck-out path format is:
```
buck-out/v2/gen/<cell_name>/<cfg_hash>[/<cell_relative_pkg_path>]/__<target_name>__/<artifact_path>
```

Components available from `ConfiguredTargetLabel`:
- `target.pkg().cell_name().as_str()` - cell name
- `target.cfg().output_hash().as_str()` - configuration hash (16 hex chars)
- `target.pkg().cell_relative_path().as_str()` - package path within cell
- `target.name().as_str()` - target name (escaped with `__EQ__` for `=`)

### Overview

Get rules_cc working to compile C and C++ code. For Bazel 9.0+, rules_cc is **almost entirely pure Starlark**. The key insight from research is that Kuro does NOT need to implement native providers or rules—only the native `cc_common` module that the Starlark implementations call into.

### Architecture (Bazel 9.0.0+)

In rules_cc 0.2.16+ with Bazel 9.0+:

- **Providers are pure Starlark**: `CcInfo`, `CcToolchainInfo`, `CcToolchainConfigInfo`, `DebugPackageInfo`
- **Rules are pure Starlark**: `cc_library`, `cc_binary`, `cc_test` in `cc/private/rules_impl/*.bzl`
- **Native builtin required**: The `cc_common` module for low-level action creation

The version switch happens in `extensions.bzl:31`:
```starlark
if _bazel_version_ge("9.0.0-pre.20250911"):
    # Use pure Starlark implementations
```

### Changes Required:

#### 1. Fetch rules_cc from BCR

```python
module(name = "test_cc")
bazel_dep(name = "rules_cc", version = "0.2.16")
```

#### 2. Implement Native `cc_common` Module

Create a Starlark builtin module with public API functions:

| Function | Signature | Description |
|----------|-----------|-------------|
| `get_tool_for_action` | `(*, feature_configuration, action_name) -> str` | Get tool path for action |
| `get_execution_requirements` | `(*, feature_configuration, action_name) -> dict` | Get execution requirements |
| `action_is_enabled` | `(*, feature_configuration, action_name) -> bool` | Check if action enabled |
| `get_memory_inefficient_command_line` | `(*, feature_configuration, action_name, variables) -> list[str]` | Get command line |
| `get_environment_variables` | `(*, feature_configuration, action_name, variables) -> dict` | Get env vars |
| `empty_variables` | `() -> CcToolchainVariables` | Create empty variables |

#### 3. Implement `cc_common.internal_DO_NOT_USE()`

Return a struct with internal functions (accessed via `cc/private/cc_internal.bzl`):

**Critical (needed for basic compilation):**

| Function | Description |
|----------|-------------|
| `create_cc_compile_action` | Creates C++ compile actions |
| `get_artifact_name_for_category` | Gets artifact name (`.o`, `.pic.o`, `.d`) |
| `combine_cc_toolchain_variables` | Combines toolchain variables |
| `actions2ctx_cheat` | Gets rule context from actions object |
| `cc_toolchain_variables` | Creates CcToolchainVariables from dict |
| `freeze` | Freezes list to immutable tuple |

**High Priority (full functionality):**

| Function | Description |
|----------|-------------|
| `create_cc_compile_action_template` | Tree artifact compile template |
| `wrap_link_actions` | Wraps link actions for platform compat |
| `get_link_args` | Gets link arguments from linking context |
| `declare_compile_output_file` | Declares compile output file |
| `declare_other_output_file` | Declares auxiliary outputs (dwo, etc.) |

See [rules_cc Native Requirements](../research/2026-01-26-rules-cc-native-requirements.md) for complete function list.

#### 4. Implement `ctx.fragments.cpp`

C++ configuration fragment with required attributes:

| Attribute/Method | Type | Description |
|------------------|------|-------------|
| `force_pic()` | `() -> bool` | Whether -fPIC is forced |
| `compilation_mode()` | `() -> str` | "opt", "dbg", or "fastbuild" |
| `use_llvm_coverage_map_format()` | `() -> bool` | Coverage format preference |
| `apple_generate_dsym` | `bool` | Whether to generate dSYM files |
| `fdo_instrument()` | `() -> str?` | FDO instrumentation path |

#### 5. NOT Required (Pure Starlark in rules_cc)

These are implemented in rules_cc's Starlark code—Kuro does NOT need native implementations:

- `CcInfo` provider - `cc/private/cc_info.bzl`
- `CcToolchainInfo` provider - `cc/private/rules_impl/cc_toolchain_info.bzl`
- `cc_library` rule - `cc/private/rules_impl/cc_library.bzl`
- `cc_binary` rule - `cc/private/rules_impl/cc_binary.bzl`
- `cc_test` rule - `cc/private/rules_impl/cc_test.bzl`

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

#### Test Migration (Phase 9):

- [ ] ADD `tests/core/cc_common/test_compile.py` for cc_common.compile()
- [ ] ADD `tests/core/cc_common/test_link.py` for cc_common.link()
- [ ] ADD `tests/core/cc_common/test_create_compilation_context.py`
- [ ] ADD `tests/core/rules_cc/test_cc_library.py` for @rules_cc cc_library
- [ ] ADD `tests/core/rules_cc/test_cc_binary.py` for linking

---

## Phase 10: rules_rust Integration

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

## Phase 11: rules_python Integration

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

## Phase 12: rules_oci Integration

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

