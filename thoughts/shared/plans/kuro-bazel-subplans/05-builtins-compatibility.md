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
| `config_setting` | Matches configuration for `select()` | ✓ Implemented (native, with ConfigurationInfo, flag_values support; 2026-02-26) | `native_rules.rs`, `native_rule_analysis.rs`, `calculation.rs` |
| `label_flag` | Label-typed build setting | ✓ Implemented (native) | `native_rules.rs`, `native_rule_analysis.rs` |
| `filegroup` | Groups files under single label | ✓ Exists (native + bazel_tools Starlark impl) | `native_rules.rs`, `bazel_tools/tools/build_rules/filegroup.bzl` |
| `genquery` | Runs query language, outputs results | ✓ Stub (creates empty output via GenruleAction "touch $@"; 2026-02-25) | `native_rules.rs`, `native_rule_analysis.rs` |
| `genrule` | Generic build rule using shell | ✓ Implemented (native, with GenruleAction; cmd_bash preferred on Unix; $(location :file) works for source files; 2026-02-25) | `native_rules.rs`, `native_rule_analysis.rs`, `genrule_action.rs` |
| `starlark_doc_extract` | Extracts docs from .bzl files | ✓ Stub (empty output; hasattr(native, "starlark_doc_extract") returns True for rules_python IS_BAZEL_7_OR_HIGHER; 2026-03-11) | `native_rules.rs`, `native_rule_analysis.rs` |
| `test_suite` | Defines collections of tests | ✓ Implemented (native, TESTS_ATTRIBUTE, expansion works) | `native_rules.rs`, `native_rule_analysis.rs` |
| `cc_import` | Imports prebuilt C/C++ libraries | ✓ Implemented (native, static_library/shared_library/hdrs/alwayslink; 2026-03-11) | `native_rules.rs`, `native_rule_analysis.rs` |
| `cc_toolchain` | Legacy C++ toolchain definition | ✓ Implemented (native, registers target; 2026-03-11) | `natives.rs`, `native_rules.rs` |
| `cc_toolchain_suite` | Legacy C++ toolchain suite | ✓ Implemented (native, registers target; 2026-03-11) | `natives.rs`, `native_rules.rs` |

#### Platform & Toolchain Rules

| Rule | Description | Kuro Status | Location |
|------|-------------|-------------|----------|
| `constraint_setting` | Introduces new constraint type | ✓ Implemented (native) | `native_rules.rs`, `native_rule_analysis.rs` |
| `constraint_value` | Defines value for constraint type | ✓ Implemented (native, with ConstraintValueInfo) | `native_rules.rs`, `native_rule_analysis.rs`, `platform_common.rs` |
| `platform` | Defines platform with constraints | ✓ Implemented (native, PlatformInfo with merged constraints) | `native_rules.rs`, `native_rule_analysis.rs` |
| `toolchain` | Declares toolchain type/constraints | ✓ Implemented (native, toolchain_type + toolchain deps) | `native_rules.rs`, `native_rule_analysis.rs` |
| `toolchain_type` | Defines new toolchain type | ✓ Implemented (native, minimal stub) | `native_rules.rs`, `native_rule_analysis.rs` |

#### Shell Rules

| Rule | Description | Kuro Status | Location |
|------|-------------|-------------|----------|
| `sh_binary` | Executable shell script | ✓ Implemented (native, DefaultInfo.executable set to first src) | `native_rules.rs`, `native_rule_analysis.rs` |
| `sh_library` | Library of shell scripts | ✓ Implemented (native, srcs as DefaultInfo outputs) | `native_rules.rs`, `native_rule_analysis.rs` |
| `sh_test` | Test written as shell script | ✓ Implemented (native, sh_binary + ExternalRunnerTestInfo) | `native_rules.rs`, `native_rule_analysis.rs` |

### Implementation Strategy

**Phase 7a.1: Verify Existing Rules**
- [x] Verify `alias`, `filegroup`, `genrule`, `test_suite`, `sh_binary`, `sh_test` match Bazel API (2026-02-25)
- [x] Update attribute names/semantics if different (done: test_suite uses TESTS_ATTRIBUTE label list for node.tests() expansion)

**Phase 7a.2: Platform Rules (Critical for Toolchains)**
- [x] Implement `constraint_setting` rule
- [x] Implement `constraint_value` rule
- [x] Implement `platform` rule (produces PlatformInfo with merged constraints from constraint_values deps)
- [x] Implement `toolchain` rule (creates native TargetNode with toolchain_type + toolchain deps)
- [x] Implement `toolchain_type` rule (minimal stub, create_minimal_analysis_result)

**Phase 7a.3: Missing Rules**
- [x] Implement `config_setting` rule (critical for `select()`)
- [x] Implement `genquery` rule (2026-02-25: stub creates empty output file via GenruleAction "touch $@"; NativeRuleKind::Genquery, analyze_genquery in native_rule_analysis.rs)
- [x] Implement `sh_library` rule (native rule returning srcs as DefaultInfo outputs)
- [x] Implement `sh_binary` rule (native rule with DefaultInfo.executable set to first src)
- [x] Implement `sh_test` rule (like sh_binary + ExternalRunnerTestInfo, `kuro test` works)
- [x] `starlark_doc_extract` (2026-03-11: stub creates empty output; hasattr detection works)

### Success Criteria (Phase 7a)

- [x] All native rules available in BUILD files without `load()`
- [x] `select()` works with `config_setting` (fixed: filegroup srcs now accepts select(), analyze_filegroup uses configured attrs to resolve selectors)
- [x] `flag_values` attribute supported in `config_setting` (2026-02-26: string-based storage with graceful fallback to no-match for missing flag targets; DICE-async lookup in calculation.rs)
- [x] Platform/toolchain rules work for rules_cc toolchain resolution (via ToolchainsStub; rules_cc works end-to-end)
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
| `configuration_field` | References late-bound defaults | ✓ Implemented (resolves known fragment+name pairs to labels) | `configuration_field.rs`, `attrs_global.rs` |
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
| `existing_rule` | Retrieves rule instance | ✓ Implemented | `path.rs` (direct BUILD global + `natives.rs`) |
| `existing_rules` | Returns all rules in package | ✓ Implemented | `path.rs` (direct BUILD global + `natives.rs`) |
| `exports_files` | Marks files as exported | ✓ Implemented (2026-02-25) | `native_rules.rs` |
| `glob` | Returns files matching patterns | ✓ Implemented | `path.rs` |
| `module_name` | Returns module name | ✓ Implemented | `natives.rs` |
| `module_version` | Returns module version | ✓ Implemented | `natives.rs` |
| `package` | Declares package metadata | ✓ Implemented | `package.rs` |
| `package_default_visibility` | Returns default visibility | ✓ Implemented | `package.rs` |
| `package_group` | Defines package set for visibility | ✓ Registered (visibility enforcement unverified) | `native_rules.rs` |
| `package_name` | Returns package name | ✓ Implemented | `path.rs` |
| `package_relative_label` | Converts string to Label | ✓ Implemented (2026-02-25) | `path.rs` (direct BUILD global) |
| `repo_name` | Returns canonical repo name | ✓ Implemented (2026-02-25) | `path.rs` (direct BUILD global) |
| `repository_name` | Deprecated variant | ✓ Implemented | `path.rs` |
| `select` | Configurable attributes | ✓ Implemented | |
| `subpackages` | Lists direct subpackages | ✓ Implemented (2026-02-25) | `path.rs` |

### Implementation Strategy

**Phase 7b.1: Verify Existing Functions**
- [x] Audit all implemented functions match Bazel signatures (2026-02-25)
- [x] Add missing parameters where needed (glob: added exclude_directories param; 2026-02-25)

**Phase 7b.2: Missing Functions**
- [x] Implement `package_default_visibility()` (deprecated setter, delegates to set_build_file_default_visibility)
- [x] Implement `package_group()` rule (registers filegroup target with visibility attrs)
- [x] Implement `subpackages()` (returns direct subpackage paths from package listing)
- [x] Implement `exports_files()` (registers each file as a native filegroup target in native_rules.rs)
- [x] Implement `configuration_field()` (stub in configuration_field.rs, resolves to known labels)
- [x] Implement `analysis_test_transition()` (stub in register_bzl_module_globals)

### Success Criteria (Phase 7b)

- [x] All global functions available without load() (2026-02-25: audited and verified)
- [x] Function signatures match Bazel signatures (2026-02-25: glob exclude_directories added, all others verified)
- [ ] `package_group` works for visibility specifications (registered but full visibility checking not yet verified)

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
| `compile()` | C++ compilation | ✓ Implemented (creates real compile actions; 2026-03-10) |
| `create_compilation_context()` | Create CompilationContext | ✓ Implemented (headers, includes, defines; 2026-03-11) |
| `create_compilation_outputs()` | Create CcCompilationOutputs | ✓ Implemented (stores objects/pic_objects; 2026-03-11) |
| `create_compile_variables()` | Generate compilation vars | ✓ Implemented (stores source/output/flags/includes/defines in dict; 2026-03-11) |
| `create_linker_input()` | Create LinkerInput | ✓ Implemented (stores user_link_flags, additional_inputs; 2026-03-11) |
| `create_linking_context()` | Create LinkingContext | ✓ Implemented (wraps linker_inputs depset; 2026-03-11) |
| `create_link_variables()` | Generate linking vars | ✓ Implemented (stores user_link_flags/search_dirs/dynamic flag in dict; 2026-03-11) |
| `create_library_to_link()` | Create LibraryToLink | ✓ Implemented (stores static/pic/dynamic libraries + objects/pic_objects + alwayslink; 2026-03-11) |
| `configure_features()` | Create FeatureConfiguration | ✓ Implemented (stores requested/unsupported features, 30+ default features; 2026-03-11) |
| `link()` | C++ linking | ✓ Implemented (creates real link actions, supports executable/dynamic_library/static_library; 2026-03-11) |
| `merge_cc_infos()` | Merge CcInfo providers | ✓ Implemented (properly merges headers/includes/defines depsets from all CcInfos; 2026-03-11) |
| `is_enabled()` | Check feature enabled | ✓ Implemented (consults FeatureConfiguration's enabled set; 2026-03-11) |
| `action_is_enabled()` | Check action enabled | ✓ Implemented (consults FeatureConfiguration for action-specific features; 2026-03-11) |
| `get_tool_for_action()` | Get tool path | ✓ Implemented (platform-aware MSVC/GCC/Clang) |
| `get_memory_inefficient_command_line()` | Get command line | ✓ Implemented (generates real compiler/linker command lines) |
| `get_environment_variables()` | Get env vars | ✓ Implemented (returns MSVC INCLUDE/LIB on Windows; 2026-03-11) |
| `get_execution_requirements()` | Get exec requirements | Stub |
| `CcToolchainInfo` | Provider | ✓ Implemented |

**Status**: Nearly complete — all critical functions implemented. Only `get_execution_requirements` returns empty dict (acceptable default). `create_linking_context_from_compilation_outputs` now properly populates linker_inputs depset. `get_memory_inefficient_command_line` now emits compilation-mode-based flags: `-O2 -DNDEBUG` for opt, `-g -O0` for dbg, plus MSVC equivalents and linker strip flags. (2026-03-11)

### Module: `config`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `config.bool()` | Boolean build setting | ✓ Implemented |
| `config.int()` | Integer build setting | ✓ Implemented |
| `config.string()` | String build setting | ✓ Implemented |
| `config.string_list()` | String list setting | ✓ Implemented |
| `config.string_set()` | String set setting | ✓ Implemented (2026-03-11) |
| `config.exec()` | Execution transition | ✓ Implemented |
| `config.target()` | No-op target transition | ✓ Implemented |
| `config.none()` | Remove all configuration | ✓ Implemented |

**Status**: Fully implemented (config.rs)

### Module: `platform_common`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `ConstraintSettingInfo` | Provider | ✓ Implemented (ProviderCallableLike) |
| `ConstraintValueInfo` | Provider | ✓ Implemented (ProviderCallableLike + ProviderLike instance) |
| `PlatformInfo` | Provider | ✓ Implemented (callable, kwargs→dict, ProviderCallableLike) |
| `TemplateVariableInfo` | Provider | ✓ Implemented (callable, creates instances) |
| `ToolchainInfo` | Provider | ✓ Implemented (callable, kwargs→dict, ProviderCallableLike) |

**Status**: Partially implemented - ConstraintValueInfo, ConstraintSettingInfo, TemplateVariableInfo work

### Module: `testing`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `analysis_test()` | Creates analysis test | ✓ Implemented (FrozenAnalysisTestCallable; 2026-02-25) |
| `ExecutionInfo` | Provider | ✓ Implemented (callable provider, requirements kwarg; 2026-02-25) |
| `TestEnvironment` | Provider (deprecated) | ✓ Implemented |

**Status**: Mostly implemented

### Module: `coverage_common`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `instrumented_files_info()` | Creates InstrumentedFilesInfo | ✓ Implemented (stub, accepts all params, returns InstrumentedFilesInfoInstance) |

**Status**: Implemented (stub)

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
- [x] Implement `config` module fully (2026-02-25: all methods done in config.rs)
- [x] Implement `platform_common` module (ConstraintSettingInfo, ConstraintValueInfo, PlatformInfo, ToolchainInfo, TemplateVariableInfo all work)
- [x] Complete `cc_common` beyond stubs (rules_cc builds work: cc_library, cc_binary, cc_test, cc_proto_library)

**Phase 7c.2: Supporting Modules**
- [x] Implement `testing.analysis_test()` (2026-02-25: AnalysisTestCallable in cc_common.rs, ANALYSIS_TEST_REGISTER late binding, analyze_analysis_test in native_rule_analysis.rs)
- [x] Implement `coverage_common` (already fully implemented: CoverageCommonModule, instrumented_files_info(), InstrumentedFilesInfo provider registered in more.rs)

**Phase 7c.3: Lower Priority**
- [ ] Implement `proto` module
- [ ] Implement `java_common` module

### Success Criteria (Phase 7c)

- [x] All modules available as globals in .bzl files (2026-02-25: cc_common, config, platform_common, testing, coverage_common all available)
- [x] Module method signatures match Bazel documentation (2026-02-25: verified against Bazel docs)
- [x] rules_cc can use `config.exec()`, platform providers (2026-02-25: rules_cc works end-to-end)

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
- [x] Remove `read_config()`, `read_root_config()` (changed to error with Bazel migration message; 2026-02-24)
- [x] Remove `oncall()`, `read_oncall()` (removed from register_module_natives; 2026-02-24)
- [x] Remove `load_symbols()` (changed to error with migration message; 2026-02-24)
- [x] Verify `soft_error()` already errors in OSS (confirmed: handle_soft_error returns Err when is_open_source=true)

**Phase 7d.2: Deprecation Warnings**
- [x] Add deprecation warning to `attrs.*` functions (suggest `attr.*`) (2026-02-25: tracing::warn! via OnceLock in attrs_global.rs; fires at most once per daemon start)
- [x] Also updated bazel_tools/tools/build_rules/filegroup.bzl to use Bazel-compatible `attr.label_list(allow_files=True)` instead of `attrs.list(attrs.source())`
- [ ] Document `ctx.attr` as preferred over `ctx.attrs`

**Phase 7d.3: Prelude Cleanup**
- [x] Simplify `prelude/native.bzl`: removed 11 language-specific load()s and ~350 lines of Meta-internal macro stubs (android, apple, cxx, erlang, python, rust, kotlin); reduced from 576→40 lines (2026-02-25)
- [x] Simplify `prelude/rules.bzl`: removed `APPLE_PLATFORMS_KEY` injection that added unused `_apple_platforms` attr to every rule (2026-02-25)
- [x] Remove language-specific dirs from `prelude/user/all.bzl` (android/user, cxx/user, xcode) - done 2026-02-25
- [x] Remove `rules.bzl` load from `prelude/native.bzl` (2026-02-26): Buck2 language-specific rules (android, apple, cxx, erlang, etc.) are no longer loaded at startup. `native` struct is now `__kuro_builtins__ + user_rules` only. All 356 build targets and 5 tests pass.
- [x] Remove unused prelude directories: android/, apple/, cxx/, erlang/, go_bootstrap/, go/, haskell/, java/, kotlin/, python/, python_bootstrap/, rust/, csharp/, ocaml/, julia/, js/, lua/, aosp/, linking/ - DONE 2026-02-26; 732 files removed, ~124k lines. Simplified rules_impl.bzl to core-only. All 358 targets + 5 tests pass.
- [x] Keep core infrastructure: `prelude.bzl`, `native.bzl`, `rules.bzl`, `rules_impl.bzl` (simplified to core rules)
- [x] Keep BXL support files: `prelude/bxl/`

### Success Criteria (Phase 7d)

- [x] `read_config()`, `read_root_config()` error with clear migration message (2026-02-24)
- [x] `oncall()`, `read_oncall()`, `load_symbols()` removed (2026-02-24)
- [x] `soft_error()` already errors in OSS (confirmed)
- [x] `attrs.*` emits deprecation warning (2026-02-25)
- [x] Prelude reduced to core + extensions (2026-02-26: 15+ language dirs removed, rules_impl.bzl simplified)
- [x] No Buck2-specific functions pollute Bazel-style BUILD files (2026-02-26: native.bzl only exposes __kuro_builtins__ which contains Bazel-compatible rules)

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
- [x] All Bazel native rules available without `load()` (2026-02-25: alias, config_setting, filegroup, genrule, platform, constraint_setting/value, sh_binary/library/test, test_suite, toolchain/type, label_flag all work)
- [x] Platform/toolchain rules work (2026-02-25: constraint_setting, constraint_value, platform, config_setting all work; rules_cc toolchain works end-to-end)
- [x] genrule improvements: cmd_bash preferred on Unix, $(location :file) works for source files in srcs (2026-02-25)
- [x] sh_test uses bash interpreter so scripts don't need +x bit (2026-02-25)

### Phase 7b (Global Functions)
- [x] All global functions match Bazel signatures (2026-02-25: audited against Bazel docs)
- [x] repo_name(), existing_rule(), existing_rules(), package_relative_label() available as direct BUILD globals (2026-02-25)
- [ ] `package_group` visibility works (registered but full visibility enforcement not yet verified)

### Phase 7c (Modules)
- [x] `config` module fully implemented (config.rs)
- [x] `platform_common` module implemented (ConstraintSettingInfo, ConstraintValueInfo, PlatformInfo, ToolchainInfo, TemplateVariableInfo)
- [x] All module methods match Bazel documentation (verified: cc_common, config, platform_common, testing, coverage_common)

### Phase 7d (Buck2 Removal)
- [x] Buck2-specific functions removed/deprecated (read_config/read_root_config errors; oncall/load_symbols removed; attrs.* deprecated)
- [x] Prelude cleaned up (2026-02-26: 15+ language dirs removed, rules_impl.bzl simplified to core rules only)
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
