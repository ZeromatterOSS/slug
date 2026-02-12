# Rules Integration Phases (9-13)

> **Parent Plan**: [Kuro Bazel-Compatible Build Tool](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> **Key Research**: [rules_cc Native Requirements](../research/2026-01-26-rules-cc-native-requirements.md)

This sub-plan covers integration with the rules_* ecosystem: rules_cc, rules_rust, rules_python, protobuf, and rules_oci.

## Bazel 9.0 Native → Starlark Migration Architecture

**Critical context for ALL rules_* phases**: In Bazel 9.0, all language rules are pure Starlark. The native (C++/Java) implementations have been removed from Bazel's core. Each rules_* repo has a different mechanism for detecting Bazel 9.0+ and switching to Starlark implementations. Kuro intercepts these mechanisms via synthetic repositories.

### Pattern: Each rules_* repo checks Bazel version differently

| Rules Repo | Detection Mechanism | What Kuro Must Provide |
|---|---|---|
| **rules_cc 0.2.16** | `_bazel_version_ge("9.0.0-pre.20250911")` via `@bazel_features` | `cc_common` native module |
| **rules_python 1.8.0+** | `enable_pystar` config flag + `hasattr(native, "starlark_doc_extract")` | `py_internal` stubs, `PyInfo`/`PyRuntimeInfo` globals |
| **protobuf 33.4+** | `hasattr(native, "proto_library")` → must return False | `ProtoInfo` provider, `proto_common` module stubs |
| **rules_rust 0.40.0** | Pure Starlark always (no native fallback) | Detect system `rustc`/`cargo` |

### Anti-pattern: Do NOT implement native language rules

Kuro must NOT implement `native.py_library`, `native.py_binary`, `native.py_test`, `native.proto_library`, etc. These were removed from Bazel 9.0. They may be stubbed as `= None` on the `native` module to prevent crashes when code does `hasattr(native, "py_library")` or similar checks, but there must never be a real implementation behind them.

Instead, Kuro should:

1. **Generate synthetic repos** that configure each rules_* repo to use its Starlark path
2. **Provide native modules** (`cc_common`, `proto_common`) that Starlark implementations call
3. **Provide provider globals** (`PyInfo`, `ProtoInfo`) as Starlark builtins
4. **Stub internal APIs** (`py_internal`, etc.) that Starlark implementations depend on

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
- [x] **`kuro run` with cc_binary** - Bazel-compatible `DefaultInfo(executable=...)` support
  - Added `executable` field to `DefaultInfoGen` struct
  - `kuro run` now checks `DefaultInfo.executable` as fallback when no `RunInfo` is present
  - Modified `build/outputs.rs` and `build/result_report.rs` to support this

**Completed (2026-02-05):**
- [x] **Bazel default target pattern support** - `@repo//path` now resolves to `@repo//path:path`
  - Added `ParsedPattern::parse_infer_target()` in `kuro_core/src/pattern/pattern.rs`
  - Modified `BuildAttrCoercionContext` to use Bazel-compatible parsing for label coercion
  - This fixes deps like `"@rules_cc//cc/runfiles"` which should mean `@rules_cc//cc/runfiles:runfiles`
  - Files: `app/kuro_core/src/pattern/pattern.rs`, `app/kuro_interpreter_for_build/src/attrs/coerce/ctx.rs`
- [x] **Starlark filegroup rule for bazel_tools** - `@bazel_tools//tools/test:collect_cc_coverage` now works
  - Created `bazel_tools/tools/build_rules/filegroup.bzl` with Starlark filegroup rule implementation
  - Updated `bazel_tools/tools/test/BUILD` to load and use the Starlark filegroup
  - Added `visibility = ["PUBLIC"]` to make targets accessible to all packages
  - Added `RuleType::Native(NativeRuleKind)` infrastructure for future native rule support
  - Files: `bazel_tools/tools/build_rules/filegroup.bzl`, `bazel_tools/tools/test/BUILD`, `app/kuro_node/src/rule_type.rs`

**Completed (2026-02-05):**
- [x] **Native constraint_setting/constraint_value rules** - Now properly register targets
  - Added `RuleType::Native(NativeRuleKind::ConstraintSetting/ConstraintValue)` to rule_type.rs
  - Implemented in `native_rules.rs` with proper visibility support
  - Analysis implemented in `native_rule_analysis.rs`
- [x] **package(default_visibility=...) support** - BUILD files can now set default visibility
  - Added `build_file_default_visibility` field to `ModuleInternals`
  - `package()` function now sets default visibility in BUILD file context
  - Updated `parse_visibility()` to recognize Bazel-style `//visibility:public`
- [x] **Native alias() rule** - Aliases now properly forward to their actual targets
  - Added `NativeRuleKind::Alias` to rule_type.rs
  - Implemented in `native_rules.rs` with proper target registration
  - Analysis forwards providers from the actual target

**Completed (2026-02-05) - cc_test support:**
- [x] **ctx.exec_groups** - Stub implementation that returns `ExecGroupsDict`
  - `ExecGroupToolchains.at()` returns None → forces legacy cc_test path
  - Files: `context.rs` (ExecGroupsDict/ExecGroupInfo/ExecGroupToolchains), `rule.rs` (ExecGroupValue)
- [x] **File.dirname attribute** - `artifact.dirname` returns directory part of path
  - File: `artifact/methods.rs`
- [x] **platform_common.ConstraintValueInfo** - Proper provider with ProviderCallableLike
  - `ConstraintValueInfoProvider` implements `ProviderCallableLike` trait
  - `ConstraintValueInfoInstance` implements `ProviderLike` trait
  - Native constraint_value targets now include ConstraintValueInfo in provider collections
  - Files: `platform_common.rs`, `native_rule_analysis.rs`
- [x] **ctx.target_platform_has_constraint()** - Always returns false (no platform constraints yet)
  - File: `context.rs`
- **cc_test builds and runs** with both `linkstatic=True` and `linkstatic=False`
- `kuro test //:hello_test` and `kuro test //:hello_test_dynamic` both pass

**Completed (2026-02-05) - Dynamic linking & test runner:**
- [x] **Test runner integration** - `kuro test` works with cc_test rules end-to-end
  - Auto-injects `ExternalRunnerTestInfo` for rules with `test=True`
  - `Rule.is_test` field threaded through StarlarkRuleCallable → freeze → Rule → TargetNode
  - Factory: `create_external_runner_test_info_for_bazel_test()` in `external_runner_test_info.rs`
- [x] **Dynamic linking support in cc_common.get_link_args** - Full linkstatic=False support
  - Process `libraries_to_link` based on `.type` field from rules_cc providers
  - `dynamic_library` → `-l<name>`, `versioned_dynamic_library` → `-l:<name>`
  - `object_file_group` → iterate `.object_files`, `static_library`/`object_file` → full path
  - `library_search_directories` → `-L<dir>` flags with depset iteration
  - `$ORIGIN`-relative RUNPATH for shared library resolution at runtime
  - Normalize rules_cc relative paths (strip `../` prefix) for proper $ORIGIN computation
  - Deduplicate rpath entries

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

#### 5. DefaultInfo.executable vs RunInfo

**Problem:** Bazel binaries (cc_binary, etc.) use `DefaultInfo(executable=binary)` to mark targets as runnable. Buck2/Kuro uses a separate `RunInfo` provider. When rules_cc's cc_binary returns DefaultInfo with executable, `kuro run` failed because it only looked for RunInfo.

**Fix:** Modified Kuro to support Bazel's pattern:
1. Added `executable` field to `DefaultInfoGen` struct (stored as list with 0-1 elements)
2. `build/outputs.rs`: When no RunInfo exists, check `DefaultInfo.executable` and add the artifact as a Run output
3. `build/result_report.rs`: When building run_args, fallback to `DefaultInfo.executable` if no RunInfo

**Files:**
- `app/kuro_build_api/src/interpreter/rule_defs/provider/builtin/default_info.rs` - Store executable
- `app/kuro_build_api/src/build/outputs.rs` - Include executable as run output
- `app/kuro_server_commands/src/build/result_report.rs` - Build run_args from executable

### Overview

Get rules_cc working to compile C and C++ code. For Bazel 9.0+, rules_cc is **almost entirely pure Starlark**. The key insight from research is that Kuro does NOT need to implement native providers or rules—only the native `cc_common` module that the Starlark implementations call into.

### Migration Architecture (Bazel 9.0.0+)

In rules_cc 0.2.16+ with Bazel 9.0+:

- **Providers are pure Starlark**: `CcInfo`, `CcToolchainInfo`, `CcToolchainConfigInfo`, `DebugPackageInfo`
- **Rules are pure Starlark**: `cc_library`, `cc_binary`, `cc_test` in `cc/private/rules_impl/*.bzl`
- **Native builtin required**: The `cc_common` module for low-level action creation

#### Version Check Details

The version switch happens in `cc/extensions.bzl:31` inside `_compatibility_proxy_repo_impl()`:
```starlark
if _bazel_version_ge("9.0.0-pre.20250911"):
    # Bazel 9.0+: Load Starlark implementations from cc/private/rules_impl/
    # proxy.bzl: cc_binary = @rules_cc//cc/private/rules_impl:cc_binary.bzl
    # symbols.bzl: CcInfo = @rules_cc//cc/private:cc_info.bzl, cc_common = @rules_cc//cc/private:cc_common.bzl
else:
    # Pre-9.0: Delegate to native rules
    # proxy.bzl: cc_binary = native.cc_binary
    # symbols.bzl: NativeCcInfo = CcInfo (from native globals)
```

The `_bazel_version_ge()` function comes from `@bazel_features//private:util.bzl`. Kuro reports `"9.0.0"` (no prerelease suffix), which compares greater than `"9.0.0-pre.20250911"` in semver (released > prerelease).

**Kuro approach**: Generate synthetic `@cc_compatibility_proxy` repo matching the Bazel 9.0+ path (lines 61-109 of extensions.bzl). Implemented in `synthetic_repos.rs:386-478`.

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

- [x] Native `cc_common` module is available
- [x] `cc_common.compile()` creates compilation actions
- [x] `cc_common.link()` creates linking actions
- [x] rules_cc's `CcInfo` provider works (uses Starlark `provider()`)
- [x] `kuro build //:main` compiles and links successfully
- [x] `kuro run //:hello_bin` executes cc_binary (via DefaultInfo.executable)
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

Get rules_python working for Python projects. **Critical**: rules_python 1.8.0+ uses a config-driven switch (`enable_pystar`) to choose between native rules and Starlark implementations. For Bazel 9.0 compatibility, Kuro MUST use the Starlark path (`enable_pystar = True`), NOT implement `native.py_library`.

### Migration Architecture

#### How rules_python Detects Bazel Version

Unlike rules_cc which compares version strings, rules_python uses **feature detection**:

```starlark
# internal_config_repo.bzl - Repository rule (NOT executed by Kuro - we generate synthetic repo)
def _internal_config_repo_impl(rctx):
    pystar_requested = _bool_from_environ(rctx, "RULES_PYTHON_ENABLE_PYSTAR", "1")  # default: enabled
    if pystar_requested and hasattr(native, "starlark_doc_extract"):  # Bazel 7+ feature
        enable_pystar = True
    else:
        enable_pystar = False
    rctx.file("rules_python_config.bzl", "config = struct(enable_pystar = {})".format(enable_pystar))
```

#### Rule File Switching Pattern

Each py_*.bzl file uses a ternary:

```starlark
# py_library.bzl:23
load("@rules_python_internal//:rules_python_config.bzl", "config")
load("//python/private/common:py_library_macro_bazel.bzl", _starlark_py_library = "py_library")

_py_library_impl = _starlark_py_library if config.enable_pystar else native.py_library
```

Same pattern in `py_binary.bzl` and `py_test.bzl`.

#### Why NOT `enable_pystar = False`

Setting `enable_pystar = False` forces fallback to `native.py_library`, which:
- Does NOT exist in Bazel 9.0 (removed from core)
- Would require Kuro to implement native Python rule analysis
- Is the WRONG architectural direction for Bazel 9.0 compatibility

#### Why `enable_pystar = True`

Setting `enable_pystar = True` uses rules_python's own Starlark implementations, which:
- Are the only path in Bazel 9.0
- Are well-tested (default since rules_python 0.40.0)
- Require `py_internal` stubs (Bazel-internal API)
- Require `hasattr(native, "starlark_doc_extract")` to return True (via `IS_BAZEL_7_OR_HIGHER` check in `util.bzl:87`)

### Changes Required:

#### 1. Fetch rules_python from BCR

```python
bazel_dep(name = "rules_python", version = "1.8.0")
```

Version 1.8.0+ has pystar as default and the native rule fallback removed.

#### 2. Update Synthetic `@rules_python_internal` Repository

Change the current synthetic repo to generate `enable_pystar = True`:

```python
# @rules_python_internal//:rules_python_config.bzl
config = struct(
  enable_pystar = True,
)
```

**File**: `app/kuro_bzlmod/src/synthetic_repos.rs` - `generate_rules_python_internal_config_repo()`

#### 3. Implement `py_internal` Stubs

The Starlark implementations load `@rules_python_internal//:py_internal.bzl` which re-exports Bazel's `py_internal` global. Kuro must provide this as a stub.

Generate `@rules_python_internal//:py_internal.bzl`:
```python
# Stub py_internal for Kuro compatibility
# The Starlark implementations use this for advanced features;
# basic py_library/py_binary/py_test work with minimal stubs.
py_internal_impl = struct(
    # Add stubs as needed based on what rules_python actually calls
)
```

#### 4. Ensure `native.starlark_doc_extract` Exists

rules_python's `util.bzl:87` checks `IS_BAZEL_7_OR_HIGHER = hasattr(native, "starlark_doc_extract")`. Either:
- Add `starlark_doc_extract` as a stub on the `native` module, OR
- Ensure the check doesn't matter (since we generate the config synthetically)

Note: Since Kuro generates `@rules_python_internal` synthetically (bypassing the repo rule), the `hasattr` check only matters in `util.bzl` where `IS_BAZEL_7_OR_HIGHER` is used for other purposes (like `py_runtime.bzl:21`).

#### 5. Provide `PyInfo` and `PyRuntimeInfo` Globals

Already implemented in `py_common.rs`. These are needed by `python/private/reexports.bzl`:
```python
BuiltinPyInfo = PyInfo           # native global
BuiltinPyRuntimeInfo = PyRuntimeInfo  # native global
```

#### 6. Python Toolchain Detection

For basic functionality, detect system Python:
```python
python = use_extension("@rules_python//python/extensions:python.bzl", "python")
python.toolchain(python_version = "3.11")
```

The `pythons_hub` synthetic repo already handles this.

#### 7. Test with Real Project

```python
load("@rules_python//python:defs.bzl", "py_binary", "py_library", "py_test")

py_library(name = "hello_py_lib", srcs = ["hello_py.py"])
py_binary(name = "hello_py_bin", srcs = ["hello_py_main.py"], deps = [":hello_py_lib"])
py_test(name = "hello_py_test", srcs = ["hello_py_test.py"], deps = [":hello_py_lib"])
```

#### 8. pip Integration (Later)

```python
pip = use_extension("@rules_python//python/extensions:pip.bzl", "pip")
pip.parse(
    hub_name = "pip",
    python_version = "3.11",
    requirements_lock = "//:requirements_lock.txt",
)
use_repo(pip, "pip")
```

### Implementation Strategy (Iterative)

1. Switch synthetic repo to `enable_pystar = True`
2. Attempt build, fix errors one-by-one (same approach as rules_cc)
3. Stub `py_internal` functions as rules_python's Starlark code exercises them
4. Most basic py_library/py_binary/py_test should work without complex `py_internal` features
5. Advanced features (coverage, C extensions via `PyWrapCcHelper`) can be stubbed

### Success Criteria:

#### Automated Verification:

- [ ] `kuro build //:hello_py_lib` analyzes py_library via Starlark rules
- [ ] `kuro build //:hello_py_bin` creates executable Python binary
- [ ] `kuro run //:hello_py_bin` executes Python
- [ ] `kuro test //:hello_py_test` runs test
- [ ] `enable_pystar = True` path works (NOT native.py_library fallback)

#### Manual Verification:

- [ ] Build a Python project with pip dependencies
- [ ] Verify Python toolchain detection works

---

## Phase 12: protobuf Integration

### Current Status (2026-02-12)

**Implemented:**
- [x] `proto_library` builds end-to-end (283 commands including protoc_minimal build from source)
- [x] `config_setting` with `values` attribute now evaluates against known Bazel flag defaults
  - `strict_public_imports` defaults to "off" → config_setting matches → `--allowed_public_imports` not added
  - `strict_proto_deps` defaults to "off" → config_setting matches → `--direct_dependencies` not added
  - Fix in: `native_rule_analysis.rs` (check_values_match_defaults) and `native_rules.rs` (store values dict)
- [x] protobuf + abseil-cpp C++ code builds via rules_cc (all 283 commands succeed)
- [x] `hasattr(native, "proto_library")` returns False → Starlark proto rules used

**Blocking:**
- [ ] `cc_proto_library` requires aspect attribute resolution (Phase 8d Advanced Features)
  - The `cc_proto_aspect` has `_aspect_cc_proto_toolchain` attribute with `configuration_field()` default
  - Current aspect ctx.attr creates empty struct (Phase 8b placeholder)
  - Need: resolve `configuration_field(fragment="proto", name="proto_toolchain_for_cc")` → label → dependency
  - Alternative: Enable `INCOMPATIBLE_ENABLE_PROTO_TOOLCHAIN_RESOLUTION = True` + implement `ctx.toolchains`

### Overview

Get Protocol Buffer compilation working. For Bazel 9.0, protobuf rules are pure Starlark in the `protobuf` BCR module (not `rules_proto`, which is archived).

### Migration Architecture

#### History

- **Bazel 6-7**: `proto_library`, `cc_proto_library`, etc. were native rules in Bazel core
- **Bazel 8 (Dec 2024)**: Proto rules removed from core, `--incompatible_autoload_externally` provided compat
- **Bazel 9 (Jan 2026)**: Autoloading removed, explicit `load()` from `@protobuf//bazel/*.bzl` required
- **Jan 14, 2026**: `rules_proto` repository archived (use `@protobuf` instead)

#### How protobuf Detects Bazel Version

protobuf uses **feature detection** (not version string comparison):

```starlark
# protobuf/bazel/proto_library.bzl
def proto_library(**kwattrs):
    if not hasattr(native, "proto_library"):
        # Bazel 8+: Use Starlark implementation
        _proto_library(**kwattrs)
    else:
        # Bazel 6-7: Use native implementation to avoid ProtoInfo mismatches
        native.proto_library(**kwattrs)
```

Since Kuro targets Bazel 9.0 and does NOT register `native.proto_library`, `hasattr(native, "proto_library")` returns False → Starlark path is used automatically.

#### Version Requirements

- **protobuf 27.0** (in BCR cache): Uses `proto_library = native.proto_library` directly — **INCOMPATIBLE with Bazel 9.0**
- **protobuf 33.4+**: Full Starlark implementations, required for Bazel 9.0
- The BCR has protobuf up to 34.0-rc1

### Changes Required:

#### 1. Fetch protobuf from BCR

```python
bazel_dep(name = "protobuf", version = "33.5")
```

Note: protobuf has 18 direct dependencies including rules_cc, rules_python, rules_java, abseil-cpp, zlib, re2. This is a large dependency tree.

#### 2. Ensure `native.proto_library` Is Not a Real Implementation

Kuro must NOT register `proto_library` as a functioning native rule. It may be stubbed as `= None` on the `native` module if needed to avoid crashes, but `hasattr(native, "proto_library")` should return False (or the value should be None), so protobuf's Starlark path is triggered.

#### 3. Provide `ProtoInfo` Provider

protobuf's Starlark code references `ProtoInfo` (re-exported from `bazel/common/proto_info.bzl`). In newer versions this is defined in Starlark; in transitional versions it may reference the native `ProtoInfo`. Kuro may need a `ProtoInfo` provider stub similar to `PyInfo`.

#### 4. Provide `proto_common` Module

protobuf's Starlark implementations use `proto_common` for:
- `proto_common.compile()` - Invoke protoc
- `proto_common.ProtoLangToolchainInfo` - Toolchain provider

Kuro needs native stubs for this module, similar to `cc_common`.

#### 5. Handle protobuf's Large Dependency Tree

protobuf depends on: rules_cc, rules_python, rules_java, rules_kotlin, rules_rust, rules_ruby, abseil-cpp, zlib, jsoncpp, re2, bazel_skylib, rules_pkg, rules_shell, rules_license, apple_support, platforms, bazel_features.

Many of these may need synthetic repos or stub handling.

#### 6. Test with Real Project

```python
load("@protobuf//bazel:proto_library.bzl", "proto_library")
load("@protobuf//bazel:cc_proto_library.bzl", "cc_proto_library")
load("@protobuf//bazel:py_proto_library.bzl", "py_proto_library")

proto_library(
    name = "hello_proto",
    srcs = ["hello.proto"],
)

cc_proto_library(
    name = "hello_cc_proto",
    deps = [":hello_proto"],
)
```

### Success Criteria:

#### Automated Verification:

- [x] `kuro build //:hello_proto` compiles .proto files (283 commands, builds protoc_minimal from source)
- [ ] `kuro build //:hello_cc_proto` generates C++ code from protos (blocked by aspect attribute resolution - Phase 8d)
- [ ] `kuro build //:hello_py_proto` generates Python code from protos
- [x] `hasattr(native, "proto_library")` returns False

#### Manual Verification:

- [ ] Build a project with proto dependencies
- [ ] Verify protoc is found/downloaded correctly

---

## Phase 13: rules_oci Integration

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

