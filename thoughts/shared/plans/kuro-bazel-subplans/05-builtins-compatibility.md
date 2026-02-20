# Builtins Compatibility Phase (7)

> **Parent Plan**: [Kuro Bazel-Compatible Build Tool](../2026-01-21-kuro-bazel-compatible-build-tool.md)

This sub-plan covers ensuring Kuro has all Bazel built-in rules, functions, and modules while removing Buck2-specific builtins that conflict with Bazel compatibility.

---

## Overview

Bazel 9.0 completed the "Starlarkification" effort, moving most language-specific rules to external repositories. Kuro must:

1. **Implement all Bazel native rules** - Rules available without `load()`
2. **Implement all Bazel global functions** - Functions available in .bzl files
3. **Implement all Bazel modules** - Top-level modules like `cc_common`, `config`, etc.
4. **Remove Buck2-specific builtins** - Functions that conflict with Bazel semantics

---

## Phase 7a: Bazel Native Rules

### Overview

In Bazel 9.0, only **language-agnostic** rules are built-in. Language-specific rules (cc_*, java_*, py_*, proto_*) require `load()` from external repositories.

### Native Rules to Implement

#### General Rules

| Rule | Description | Kuro Status | Location |
|------|-------------|-------------|----------|
| `alias` | Creates alternative name for target | ✓ Implemented (native + prelude) | `native_rules.rs`, `prelude/alias.bzl` |
| `config_setting` | Matches configuration for `select()` | ✓ Implemented (native, with ConfigurationInfo) | `native_rules.rs`, `native_rule_analysis.rs` |
| `label_flag` | Label-typed build setting | ✓ Implemented (native) | `native_rules.rs`, `native_rule_analysis.rs` |
| `filegroup` | Groups files under single label | ✓ Exists | `prelude/filegroup.bzl` |
| `genquery` | Runs query language, outputs results | Not implemented | TBD |
| `genrule` | Generic build rule using shell | ✓ Implemented (native, with GenruleAction) | `native_rules.rs`, `native_rule_analysis.rs`, `genrule_action.rs` |
| `starlark_doc_extract` | Extracts docs from .bzl files | Not implemented | Low priority |
| `test_suite` | Defines collections of tests | ✓ Exists | `prelude/test_suite.bzl` |

#### Platform & Toolchain Rules

| Rule | Description | Kuro Status | Location |
|------|-------------|-------------|----------|
| `constraint_setting` | Introduces new constraint type | ✓ Implemented (native) | `native_rules.rs`, `native_rule_analysis.rs` |
| `constraint_value` | Defines value for constraint type | ✓ Implemented (native, with ConstraintValueInfo) | `native_rules.rs`, `native_rule_analysis.rs`, `platform_common.rs` |
| `platform` | Defines platform with constraints | Needs implementation | TBD |
| `toolchain` | Declares toolchain type/constraints | Needs implementation | TBD |
| `toolchain_type` | Defines new toolchain type | Needs implementation | TBD |

#### Shell Rules

| Rule | Description | Kuro Status | Location |
|------|-------------|-------------|----------|
| `sh_binary` | Executable shell script | ✓ Exists | `prelude/sh_binary.bzl` |
| `sh_library` | Library of shell scripts | Needs verification | TBD |
| `sh_test` | Test written as shell script | ✓ Exists | `prelude/sh_test.bzl` |

### Implementation Strategy

**Phase 7a.1: Verify Existing Rules**
- [ ] Verify `alias`, `filegroup`, `genrule`, `test_suite`, `sh_binary`, `sh_test` match Bazel API
- [ ] Update attribute names/semantics if different

**Phase 7a.2: Platform Rules (Critical for Toolchains)**
- [x] Implement `constraint_setting` rule
- [x] Implement `constraint_value` rule
- [x] Implement `platform` rule (produces PlatformInfo with merged constraints from constraint_values deps)
- [x] Implement `toolchain` rule (creates native TargetNode with toolchain_type + toolchain deps)
- [x] Implement `toolchain_type` rule (minimal stub, create_minimal_analysis_result)

**Phase 7a.3: Missing Rules**
- [x] Implement `config_setting` rule (critical for `select()`)
- [ ] Implement `genquery` rule
- [x] Implement `sh_library` rule (native rule returning srcs as DefaultInfo outputs)
- [x] Implement `sh_binary` rule (native rule with DefaultInfo.executable set to first src)
- [x] Implement `sh_test` rule (like sh_binary + ExternalRunnerTestInfo, `kuro test` works)
- [ ] (Low priority) `starlark_doc_extract`

### Success Criteria (Phase 7a)

- [x] All native rules available in BUILD files without `load()`
- [x] `select()` works with `config_setting` (fixed: filegroup srcs now accepts select(), analyze_filegroup uses configured attrs to resolve selectors)
- [ ] Platform/toolchain rules work for rules_cc toolchain resolution
- [x] Bazel BUILD files using native rules parse correctly

---

## Phase 7b: Bazel Global Functions

### Overview

These functions must be available in all .bzl files without any `load()` statement.

### Global Functions for .bzl Files

| Function | Description | Kuro Status | Location |
|----------|-------------|-------------|----------|
| `analysis_test_transition` | Config transition for analysis tests | Not implemented | TBD |
| `aspect` | Defines aspect for dependency propagation | ✓ Implemented | `aspect.rs` (Phase 8) |
| `configuration_field` | References late-bound defaults | Not implemented | TBD |
| `depset` | Creates dependency set | ✓ Implemented | `transitive_set/globals.rs` |
| `exec_group` | Establishes execution group | ✓ Stub | `rule.rs` |
| `exec_transition` | Defines exec transition (internal) | Not implemented | Low priority |
| `macro` | Defines symbolic macro | Not implemented | Future |
| `materializer_rule` | Creates materializer rule | Not implemented | Low priority |
| `module_extension` | Creates module extension | ✓ Implemented | `bzlmod/` |
| `provider` | Defines provider type | ✓ Implemented | `provider.rs` |
| `repository_rule` | Creates repository rule | ✓ Implemented | `repository_rule.rs` |
| `rule` | Creates new rule callable | ✓ Implemented | `rule.rs` |
| `select` | Configurable attributes | ✓ Implemented | Built-in |
| `subrule` | Constructs subrule instance | ✓ Stub | `subrule.rs` |
| `tag_class` | Creates tag class for extensions | ✓ Implemented | `bzlmod/` |
| `visibility` | Sets load visibility | ✓ Stub | `visibility.rs` |

### Global Functions for BUILD Files

| Function | Description | Kuro Status | Location |
|----------|-------------|-------------|----------|
| `depset` | Creates depset | ✓ Available | |
| `existing_rule` | Retrieves rule instance | ✓ Implemented | `natives.rs` |
| `existing_rules` | Returns all rules in package | ✓ Implemented | `natives.rs` |
| `exports_files` | Marks files as exported | Needs verification | |
| `glob` | Returns files matching patterns | ✓ Implemented | `natives.rs` |
| `module_name` | Returns module name | ✓ Implemented | `natives.rs` |
| `module_version` | Returns module version | ✓ Implemented | `natives.rs` |
| `package` | Declares package metadata | ✓ Implemented | `package.rs` |
| `package_default_visibility` | Returns default visibility | Needs implementation | |
| `package_group` | Defines package set for visibility | Needs implementation | |
| `package_name` | Returns package name | ✓ Implemented | `natives.rs` |
| `package_relative_label` | Converts string to Label | ✓ Implemented | `natives.rs` |
| `repo_name` | Returns canonical repo name | ✓ Implemented | `natives.rs` |
| `repository_name` | Deprecated variant | ✓ Implemented | `natives.rs` |
| `select` | Configurable attributes | ✓ Implemented | |
| `subpackages` | Lists direct subpackages | Not implemented | |

### Implementation Strategy

**Phase 7b.1: Verify Existing Functions**
- [ ] Audit all implemented functions match Bazel signatures
- [ ] Add missing parameters where needed

**Phase 7b.2: Missing Functions**
- [ ] Implement `package_default_visibility()`
- [ ] Implement `package_group()` rule
- [x] Implement `subpackages()` (returns direct subpackage paths from package listing)
- [ ] Implement `exports_files()` (verify or implement)
- [ ] Implement `configuration_field()`
- [ ] Implement `analysis_test_transition()`

### Success Criteria (Phase 7b)

- [ ] All global functions available without load()
- [ ] Function signatures match Bazel documentation
- [ ] `package_group` works for visibility specifications

---

## Phase 7c: Bazel Top-Level Modules

### Overview

These modules must be available as globals in .bzl files.

### Module: `attr`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `attr.bool()` | Boolean attribute | ✓ Implemented |
| `attr.int()` | Integer attribute | ✓ Implemented |
| `attr.int_list()` | List of integers | ✓ Implemented |
| `attr.label()` | Single dependency | ✓ Implemented |
| `attr.label_list()` | List of dependencies | ✓ Implemented |
| `attr.label_keyed_string_dict()` | Label → String mapping | ✓ Implemented |
| `attr.output()` | Single output file | ✓ Implemented |
| `attr.output_list()` | List of output files | ✓ Implemented |
| `attr.string()` | String attribute | ✓ Implemented |
| `attr.string_dict()` | String → String mapping | ✓ Implemented |
| `attr.string_keyed_label_dict()` | String → Label mapping | ✓ Implemented |
| `attr.string_list()` | List of strings | ✓ Implemented |
| `attr.string_list_dict()` | String → String list mapping | ✓ Implemented |

**Status**: ✓ Complete

### Module: `cc_common`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `compile()` | C++ compilation | Stub |
| `create_compilation_context()` | Create CompilationContext | Stub |
| `create_compilation_outputs()` | Create CcCompilationOutputs | Stub |
| `create_compile_variables()` | Generate compilation vars | Stub |
| `create_linker_input()` | Create LinkerInput | Stub |
| `create_linking_context()` | Create LinkingContext | Stub |
| `create_link_variables()` | Generate linking vars | Stub |
| `create_library_to_link()` | Create LibraryToLink | Stub |
| `configure_features()` | Create FeatureConfiguration | Stub |
| `link()` | C++ linking | Stub |
| `merge_cc_infos()` | Merge CcInfo providers | Stub |
| `is_enabled()` | Check feature enabled | Stub |
| `action_is_enabled()` | Check action enabled | Stub |
| `get_tool_for_action()` | Get tool path | Stub |
| `get_memory_inefficient_command_line()` | Get command line | Stub |
| `get_environment_variables()` | Get env vars | Stub |
| `get_execution_requirements()` | Get exec requirements | Stub |
| `CcToolchainInfo` | Provider | ✓ Implemented |

**Status**: Partially implemented (stubs for rules_cc loading, full implementation needed for compilation)

### Module: `config`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `config.bool()` | Boolean build setting | Not implemented |
| `config.int()` | Integer build setting | Not implemented |
| `config.string()` | String build setting | Not implemented |
| `config.string_list()` | String list setting | Not implemented |
| `config.string_set()` | String set setting | Not implemented |
| `config.exec()` | Execution transition | ✓ Stub |
| `config.target()` | No-op target transition | Not implemented |
| `config.none()` | Remove all configuration | Not implemented |

**Status**: Mostly not implemented (needed for toolchain resolution)

### Module: `platform_common`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `ConstraintSettingInfo` | Provider | ✓ Implemented (ProviderCallableLike) |
| `ConstraintValueInfo` | Provider | ✓ Implemented (ProviderCallableLike + ProviderLike instance) |
| `PlatformInfo` | Provider | Stub (not callable yet) |
| `TemplateVariableInfo` | Provider | ✓ Implemented (callable, creates instances) |
| `ToolchainInfo` | Provider | Stub (not callable yet) |

**Status**: Partially implemented - ConstraintValueInfo, ConstraintSettingInfo, TemplateVariableInfo work

### Module: `testing`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `analysis_test()` | Creates analysis test | Not implemented |
| `ExecutionInfo` | Provider | Not implemented |
| `TestEnvironment` | Provider (deprecated) | ✓ Implemented |

**Status**: Partially implemented

### Module: `coverage_common`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `instrumented_files_info()` | Creates InstrumentedFilesInfo | Not implemented |

**Status**: Not implemented

### Module: `proto`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `proto.encode_text()` | Encode proto to text | Not implemented |

**Status**: Not implemented (low priority)

### Module: `java_common`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `compile()` | Java compilation | Not implemented |
| `merge()` | Merge JavaInfo | Not implemented |
| `JavaRuntimeInfo` | Provider | Not implemented |
| `JavaToolchainInfo` | Provider | Not implemented |

**Status**: Not implemented (deferred - Java not in initial scope)

### Module: `apple_common`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `apple_toolchain()` | Apple toolchain utilities | ✓ Stub |
| `platform` | Platform constants | ✓ Stub |
| `platform_type` | Platform types | ✓ Stub |
| `XcodeVersionConfig` | Provider | ✓ Stub |

**Status**: Stubs implemented (sufficient for rules_cc loading)

### Implementation Strategy

**Phase 7c.1: Critical Modules (blocks rules_cc)**
- [ ] Implement `config` module fully
- [ ] Implement `platform_common` module
- [ ] Complete `cc_common` beyond stubs

**Phase 7c.2: Supporting Modules**
- [ ] Implement `testing.analysis_test()`
- [ ] Implement `coverage_common`

**Phase 7c.3: Lower Priority**
- [ ] Implement `proto` module
- [ ] Implement `java_common` module

### Success Criteria (Phase 7c)

- [ ] All modules available as globals in .bzl files
- [ ] Module method signatures match Bazel documentation
- [ ] rules_cc can use `config.exec()`, platform providers

---

## Phase 7d: Buck2-Specific Removal

### Overview

Remove or deprecate Buck2-specific functions that conflict with Bazel semantics or add confusion.

### Immediate Removal (High Priority)

These functions have no Bazel equivalent and should be removed:

| Function | Location | Reason | Action |
|----------|----------|--------|--------|
| `read_config()` | `functions/read_config.rs` | Buck2-specific `.buckconfig` reading | Remove or error |
| `read_root_config()` | `functions/read_config.rs` | Buck2-specific | Remove or error |
| `oncall()` | `natives.rs` | Buck2-specific metadata | Remove |
| `read_oncall()` | `natives.rs` | Buck2-specific metadata | Remove |
| `load_symbols()` | `functions/load_symbols.rs` | Discouraged, rarely used | Remove |
| `soft_error()` | `functions/soft_error.rs` | Buck2-specific error reporting | Remove (already errors in OSS) |

### Gradual Deprecation (Medium Priority)

These should emit warnings but continue to work:

| Item | Location | Reason | Action |
|------|----------|--------|--------|
| `attrs.*` namespace | `attrs/attrs_global.rs` | Buck2 style, prefer `attr.*` | Deprecation warning |
| `ctx.attrs` | `context.rs` | Buck2 uses `attrs`, Bazel uses `attr` | Keep both, prefer `ctx.attr` |
| `host_info().kuro` | `functions/host_info.rs` | Buck2 detection field | Rename to `host_info().buck2` or remove |

### Keep as Extensions (No Conflict)

These are useful and don't conflict with Bazel:

| Item | Location | Reason |
|------|----------|--------|
| `plugins.*` namespace | `plugins.rs` | Useful Kuro-specific extension |
| `bxl.*` namespace | `kuro_bxl/` | Separate feature, no conflict |
| `read_package_value()` | `package_value.rs` | Useful for PACKAGE files |
| `write_package_value()` | `package_value.rs` | Useful for PACKAGE files |
| `warning()` | `functions/warning.rs` | Convenience function |
| `sha1()`, `sha256()` | `functions/` | Generic utilities |
| `dedupe()` | `functions/` | Generic utility |
| `regex()` | `functions/` | Generic utility |

### Buck2 Prelude Directories to Remove

Per 04-prelude-architecture.md, these language-specific directories should be removed:

| Directory | Contents | Bazel Replacement |
|-----------|----------|-------------------|
| `prelude/android/` | Android rules | rules_android |
| `prelude/apple/` | Apple/iOS rules | rules_apple |
| `prelude/cxx/` | Buck2 C++ rules | rules_cc |
| `prelude/erlang/` | Erlang rules | N/A |
| `prelude/go_bootstrap/` | Go rules | rules_go |
| `prelude/haskell/` | Haskell rules | rules_haskell |
| `prelude/java/` | Java rules | rules_java |
| `prelude/kotlin/` | Kotlin rules | rules_kotlin |
| `prelude/python/` | Buck2 Python rules | rules_python |
| `prelude/rust/` | Buck2 Rust rules | rules_rust |
| `prelude/csharp/` | C# rules | N/A |
| `prelude/ocaml/` | OCaml rules | N/A |
| `prelude/julia/` | Julia rules | N/A |
| `prelude/js/` | JavaScript rules | rules_js |
| `prelude/lua/` | Lua rules | N/A |

### Implementation Strategy

**Phase 7d.1: Immediate Removals**
- [ ] Remove `read_config()`, `read_root_config()` (or make them error with migration message)
- [ ] Remove `oncall()`, `read_oncall()`
- [ ] Remove `load_symbols()`
- [ ] Verify `soft_error()` already errors in OSS

**Phase 7d.2: Deprecation Warnings**
- [ ] Add deprecation warning to `attrs.*` functions (suggest `attr.*`)
- [ ] Document `ctx.attr` as preferred over `ctx.attrs`

**Phase 7d.3: Prelude Cleanup**
- [ ] Remove unused prelude directories (per 04-prelude-architecture.md)
- [ ] Keep core infrastructure: `prelude.bzl`, `native.bzl`, `rules.bzl`, etc.
- [ ] Keep BXL support files: `prelude/bxl/`

### Success Criteria (Phase 7d)

- [ ] `read_config()` removed or errors with clear migration message
- [ ] `attrs.*` emits deprecation warning
- [ ] Prelude reduced to core + extensions
- [ ] No Buck2-specific functions pollute Bazel-style BUILD files

---

## Dependencies

### This Phase Depends On

- Phase 2 (Starlark dialect) - `attr.*` implemented
- Phase 4 (bzlmod) - Module system working
- Phase 6 (Rule primitives) - `ctx.*`, `actions.*` implemented

### This Phase Blocks

- Phase 9+ (Rules integration) - Need platform/toolchain rules
- Full rules_cc functionality - Need `config.*`, `platform_common`

---

## Success Criteria Summary

### Phase 7a (Native Rules)
- [ ] All Bazel native rules available without `load()`
- [ ] Platform/toolchain rules work

### Phase 7b (Global Functions)
- [ ] All global functions match Bazel signatures
- [ ] `package_group` visibility works

### Phase 7c (Modules)
- [ ] `config` module fully implemented
- [ ] `platform_common` module implemented
- [ ] All module methods match Bazel documentation

### Phase 7d (Buck2 Removal)
- [ ] Buck2-specific functions removed/deprecated
- [ ] Prelude cleaned up
- [ ] Clear migration path documented

---

## Testing Strategy

### Unit Tests

**`tests/core/builtins/`**
- `test_native_rules.py` - All native rules available
- `test_global_functions.py` - All globals work
- `test_modules.py` - Module methods work
- `test_buck2_removal.py` - Removed functions error appropriately

### Integration Tests

**`tests/e2e/bazel_compat/`**
- `test_platform_toolchain.py` - Platform/toolchain rules
- `test_config_setting.py` - `select()` with config_setting
- `test_package_group.py` - Visibility with package_group

---

## References

### Bazel Documentation
- [Global functions for .bzl files](https://bazel.build/rules/lib/globals/bzl)
- [Global functions for BUILD files](https://bazel.build/rules/lib/globals/build)
- [native module](https://bazel.build/rules/lib/native)
- [Top-level modules](https://bazel.build/rules/lib/toplevel)
- [Built-in rules](https://bazel.build/reference/be/overview)

### Bazel Source
- `src/main/java/com/google/devtools/build/lib/packages/` - Rule definitions
- `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/` - Starlark APIs
