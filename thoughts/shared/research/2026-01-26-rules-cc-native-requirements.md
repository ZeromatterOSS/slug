# rules_cc Native Function Requirements (Bazel 9.0+)

**Date:** 2026-01-26
**rules_cc version:** 0.2.16+ (BCR)
**Target Bazel version:** 9.0.0+

## Overview

In Bazel 9.0+, rules_cc is **almost entirely pure Starlark**. The `extensions.bzl` file (line 31) switches behavior based on version:

```starlark
if _bazel_version_ge("9.0.0-pre.20250911"):
    # Use pure Starlark implementations
else:
    # Use native.cc_library, native.cc_binary, etc.
```

For Bazel 9.0+:
- **Providers are pure Starlark**: `CcInfo`, `CcToolchainInfo`, `CcToolchainConfigInfo`, `DebugPackageInfo` are all defined in Starlark
- **Rules are pure Starlark**: `cc_library`, `cc_binary`, `cc_test` are implemented in `cc/private/rules_impl/*.bzl`
- **Native builtins still required**: The native `cc_common` module is needed for low-level action creation

## What Kuro Must Provide

### 1. Native `cc_common` Builtin Module

The Starlark implementations in rules_cc load the native `cc_common` from `cc/private/rules_impl/native.bzl`:

```starlark
# Line 33 - No load() statement, this accesses a Starlark builtin
native_cc_common = cc_common
```

#### Public API Functions on `cc_common`

| Function | Signature | Description |
|----------|-----------|-------------|
| `get_tool_for_action` | `(*, feature_configuration, action_name) -> str` | Get tool path for an action |
| `get_execution_requirements` | `(*, feature_configuration, action_name) -> dict` | Get execution requirements |
| `action_is_enabled` | `(*, feature_configuration, action_name) -> bool` | Check if action is enabled |
| `get_memory_inefficient_command_line` | `(*, feature_configuration, action_name, variables) -> list[str]` | Get command line for action |
| `get_environment_variables` | `(*, feature_configuration, action_name, variables) -> dict` | Get env vars for action |
| `empty_variables` | `() -> CcToolchainVariables` | Create empty variables object |
| `do_not_use_tools_cpp_compiler_present` | property -> bool | Check if compiler is configured |
| `legacy_cc_flags_make_variable_do_not_use` | `(*, cc_toolchain) -> str` | Get legacy CC_FLAGS |
| `check_experimental_cc_shared_library` | `() -> bool` | Check shared library support |
| `incompatible_disable_objc_library_transition` | `() -> bool` | Check objc transition flag |
| `add_go_exec_groups_to_binary_rules` | `() -> bool` | Check Go exec groups flag |
| `implementation_deps_allowed_by_allowlist` | `(*, ctx) -> bool` | Check impl deps allowlist |
| `create_compile_action` | See below | Create compile action (allowlisted) |

#### Internal API: `cc_common.internal_DO_NOT_USE()`

From `cc/private/cc_internal.bzl`:
```starlark
cc_internal = cc_common.internal_DO_NOT_USE() if hasattr(cc_common, "internal_DO_NOT_USE") else struct()
```

Must return a struct with these functions:

**Critical Priority (needed for basic compilation):**

| Function | Description |
|----------|-------------|
| `create_cc_compile_action` | Creates C++ compile actions |
| `get_artifact_name_for_category` | Gets artifact name (`.o`, `.pic.o`, `.d`, etc.) |
| `combine_cc_toolchain_variables` | Combines toolchain variables |
| `actions2ctx_cheat` | Gets rule context from actions object |
| `cc_toolchain_variables` | Creates CcToolchainVariables from dict |
| `freeze` | Freezes list to immutable tuple |

**High Priority (needed for full functionality):**

| Function | Description |
|----------|-------------|
| `create_cc_compile_action_template` | Creates tree artifact compile template |
| `wrap_link_actions` | Wraps link actions for platform compat |
| `get_link_args` | Gets link arguments from linking context |
| `declare_compile_output_file` | Declares compile output file |
| `declare_other_output_file` | Declares auxiliary outputs (dwo, etc.) |
| `is_tree_artifact` | Checks if artifact is tree artifact |
| `compute_output_name_prefix_dir` | Computes output prefix directory |
| `intern_string_sequence_variable_value` | Interns string sequences for efficiency |
| `per_file_copts` | Gets per-file compile options |
| `check_private_api` | Checks API access (allowlist enforcement) |
| `create_header_info` | Creates HeaderInfo struct |
| `create_header_info_with_deps` | Creates HeaderInfo with dependencies |

**Lower Priority (advanced features):**

| Function | Description |
|----------|-------------|
| `create_lto_backend_action` | Creates LTO backend action |
| `create_lto_backend_action_template` | Creates LTO action template |
| `dynamic_library_soname` | Gets soname for dynamic library |
| `dynamic_library_symlink` | Creates dynamic library symlink |
| `dynamic_library_symlink2` | Creates dynamic library symlink (v2) |
| `exec_os` | Gets execution OS name |
| `get_artifact_name_extension_for_category` | Gets extension for category |
| `intern_seq` | Interns a sequence |
| `check_toplevel` | Checks toplevel context |
| `collect_per_file_lto_backend_opts` | Collects LTO options |

### 2. C++ Configuration Fragment

From `cc/private/toolchain_config/configure_features.bzl`:
```starlark
cpp_configuration = ctx.fragments.cpp
```

Required `ctx.fragments.cpp` attributes/methods:

| Attribute/Method | Type | Description |
|------------------|------|-------------|
| `force_pic()` | `() -> bool` | Returns whether -fPIC is forced |
| `compilation_mode()` | `() -> str` | Returns "opt", "dbg", or "fastbuild" |
| `use_llvm_coverage_map_format()` | `() -> bool` | Returns coverage format preference |
| `apple_generate_dsym` | `bool` | Whether to generate dSYM files |
| `objc_generate_linkmap` | `bool` | Whether to generate linkmaps (Obj-C) |
| `objc_should_strip_binary` | `bool` | Whether to strip Obj-C binaries |
| `fdo_instrument()` | `() -> str?` | FDO instrumentation path or None |
| `cs_fdo_instrument()` | `() -> str?` | CS-FDO instrumentation path or None |
| `_dont_enable_host_nonhost` | `bool` | Internal flag |
| `_fdo_prefetch_hints_label` | `Label?` | FDO prefetch hints label |

### 3. Feature Configuration Object

Returned by the Starlark `configure_features()`, the `feature_configuration` object needs:

| Method | Signature | Description |
|--------|-----------|-------------|
| `is_enabled` | `(feature_name: str) -> bool` | Check if feature is enabled |

The feature configuration is created by `cc_toolchain._toolchain_features.configure_features()` which is also pure Starlark.

## What Kuro Does NOT Need to Provide (Bazel 9.0+)

### Native Providers

For Bazel 9.0+, these providers are **pure Starlark** in rules_cc:

- `CcInfo` - Defined in `cc/private/cc_info.bzl`
- `CcToolchainInfo` - Defined in `cc/private/rules_impl/cc_toolchain_info.bzl`
- `CcToolchainConfigInfo` - Defined in `cc/private/toolchain_config/cc_toolchain_config_info.bzl`
- `DebugPackageInfo` - Defined in `cc/private/debug_package_info.bzl`
- `CcSharedLibraryInfo` - Defined in `cc/private/rules_impl/cc_shared_library.bzl`

Kuro does NOT need native implementations of these providers.

### Native Rules

For Bazel 9.0+, these rules are **pure Starlark** in rules_cc:

- `cc_library` - `cc/private/rules_impl/cc_library.bzl`
- `cc_binary` - `cc/private/rules_impl/cc_binary.bzl`
- `cc_test` - `cc/private/rules_impl/cc_test.bzl`
- `cc_import` - `cc/private/rules_impl/cc_import.bzl`
- `cc_shared_library` - `cc/private/rules_impl/cc_shared_library.bzl`
- `cc_toolchain` - `cc/private/rules_impl/cc_toolchain.bzl`

## Implementation Strategy

### Phase 1: Core cc_common Module
1. Create native `cc_common` Starlark builtin module
2. Implement `internal_DO_NOT_USE()` returning internal struct
3. Implement critical internal functions:
   - `actions2ctx_cheat`
   - `create_cc_compile_action`
   - `get_artifact_name_for_category`
   - `combine_cc_toolchain_variables`
   - `cc_toolchain_variables`
   - `freeze`

### Phase 2: C++ Configuration
1. Implement `ctx.fragments.cpp` fragment
2. Implement required attributes/methods

### Phase 3: Public cc_common Functions
1. Implement `get_tool_for_action`
2. Implement `get_execution_requirements`
3. Implement `get_memory_inefficient_command_line`
4. Implement `get_environment_variables`
5. Implement `empty_variables`
6. Implement remaining public functions

### Phase 4: Complete Internal API
1. Implement remaining internal functions
2. Implement LTO support functions
3. Implement symlink functions

## Testing Strategy

1. **Parse test**: Load `@rules_cc//cc:defs.bzl` without errors
2. **Rule declaration test**: Declare `cc_library` target
3. **Toolchain test**: Configure C++ toolchain
4. **Compile test**: Build simple C file
5. **Link test**: Build `cc_binary` with deps
6. **Header test**: Verify header dependency tracking
7. **Incremental test**: Verify incremental compilation works

## Key Source Files (rules_cc 0.2.16)

### Entry points
- `cc/defs.bzl` - Main public exports
- `cc/extensions.bzl` - Module extension and version switching

### Starlark implementations (Bazel 9.0+)
- `cc/common/cc_common.bzl` - Re-exports cc_common
- `cc/private/cc_common.bzl` - Main cc_common struct definition
- `cc/private/cc_info.bzl` - CcInfo provider (pure Starlark)
- `cc/private/rules_impl/cc_library.bzl` - cc_library rule
- `cc/private/rules_impl/cc_toolchain_info.bzl` - CcToolchainInfo provider

### Native access
- `cc/private/rules_impl/native.bzl` - Native cc_common access
- `cc/private/cc_internal.bzl` - Internal API access

### Compilation
- `cc/private/compile/compile.bzl` - cc_common.compile()
- `cc/private/compile/compile_build_variables.bzl` - Variable handling

### Linking
- `cc/private/link/link.bzl` - cc_common.link()
- `cc/private/link/cc_linking_helper.bzl` - Link action creation
