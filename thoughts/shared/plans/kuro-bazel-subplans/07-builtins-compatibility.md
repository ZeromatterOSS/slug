# Builtins Compatibility Phase (7)

> **Status: SUPERSEDED.** Use
> [05-builtins-compatibility.md](./05-builtins-compatibility.md) as the
> authoritative builtins plan. This duplicate preserves older history.
>
> **Parent Plan**: [Kuro Bazel-Compatible Build Tool](../2026-01-21-kuro-bazel-compatible-build-tool.md)

This sub-plan covers ensuring Kuro has all Bazel built-in rules, functions, and modules while removing Buck2-specific builtins that conflict with Bazel compatibility.

---

## Overview

Bazel 9.0 completed the "Starlarkification" effort, moving most language-specific rules to external repositories. Kuro must:

1. **Implement all Bazel native rules** - Rules available without `load()`
2. **Implement all Bazel global functions** - Functions available in .bzl files
3. **Implement all Bazel modules** - Top-level modules like `cc_common`, `config`, etc.
4. **Remove Buck2-specific builtins** - Functions that conflict with Bazel semantics

> **CRITICAL ARCHITECTURAL CONSTRAINT — Bazel 9 Starlarkification**
>
> In Bazel 9.0, **ALL language-specific rules are pure Starlark**. They live in
> external repos (`rules_cc`, `rules_java`, `rules_python`, `rules_shell`,
> `protobuf`, `rules_apple`, etc.) and require explicit `load()` statements.
>
> **DO NOT** implement language-specific rules as native Rust builtins.
> Instead, ensure the native *modules* (`cc_common`, `java_common`,
> `platform_common`, etc.) are robust enough for external Starlark rulesets
> to define their own rules via `rule()`.
>
> See the parent plan's [Native → Starlark Migration Architecture](../2026-01-21-kuro-bazel-compatible-build-tool.md#native--starlark-migration-architecture) for details.

---

## Phase 7a: Bazel Native Rules

### Overview

In Bazel 9.0, only **language-agnostic** rules are built-in. The complete
list of true Bazel 9 native rules is below. **Nothing else should be added
as a native rule.**

### True Bazel 9 Native Rules (complete, exhaustive list)

#### General Rules

| Rule | Description | Kuro Status | Location |
|------|-------------|-------------|----------|
| `alias` | Creates alternative name for target | ✓ Implemented | `native_rules.rs` |
| `config_setting` | Matches configuration for `select()` | ✓ Implemented | `native_rules.rs` |
| `filegroup` | Groups files under single label | ✓ Implemented | `native_rules.rs` |
| `genquery` | Runs query language, outputs results | ✓ Implemented | `native_rules.rs` |
| `genrule` | Generic build rule using shell | ✓ Implemented | `native_rules.rs` |
| `starlark_doc_extract` | Extracts docs from .bzl files | ✓ Implemented | `native_rules.rs` |
| `test_suite` | Defines collections of tests | ✓ Implemented | `native_rules.rs` |
| `analysis_test` | Analysis-time test | ✓ Implemented | `native_rules.rs` |
| `exports_files` | Marks files as exported | ✓ Implemented | `native_rules.rs` |
| `package_group` | Defines package set for visibility | ✓ Implemented | `native_rules.rs` |
| `environment_group` | Defines environment groups | ✓ Implemented | `native_rules.rs` (2026-03-11) |

#### Platform & Toolchain Rules

| Rule | Description | Kuro Status | Location |
|------|-------------|-------------|----------|
| `constraint_setting` | Introduces new constraint type | ✓ Implemented | `native_rules.rs` |
| `constraint_value` | Defines value for constraint type | ✓ Implemented | `native_rules.rs` |
| `platform` | Defines platform with constraints | ✓ Implemented | `native_rules.rs` |
| `toolchain` | Declares toolchain type/constraints | ✓ Implemented | `native_rules.rs` |
| `toolchain_type` | Defines new toolchain type | ✓ Implemented | `native_rules.rs` |

### Rules that are NOT native in Bazel 9 (require load() from external repos)

> **DO NOT add these as native Rust rules.** They are defined in Starlark by
> their respective `rules_*` repositories. Kuro's job is to make the native
> *modules* (`cc_common`, `java_common`, etc.) robust enough that these repos
> can define their rules using `rule()`.

| Rules | External Repo | Detection Mechanism |
|-------|---------------|---------------------|
| `cc_library`, `cc_binary`, `cc_test`, `cc_import`, `cc_shared_library`, `cc_toolchain`, `cc_toolchain_suite` | `@rules_cc` | Version string ≥ 9.0.0 |
| `sh_binary`, `sh_test`, `sh_library` | `@rules_shell` | Version string |
| `proto_library`, `cc_proto_library`, `java_proto_library` | `@protobuf` | Version-based |
| `java_library`, `java_binary`, `java_test`, `java_import` | `@rules_java` | Version-based |
| `py_library`, `py_binary`, `py_test` | `@rules_python` | Config flag + feature detection |
| `objc_library`, `objc_import` | `@rules_apple` | Version-based |

#### Buck2-Heritage Native Rules (temporary, to be migrated)

These rules exist as native Rust implementations in Kuro due to Buck2 heritage.
They are **not** native in Bazel 9 but remain for now to avoid breaking existing
functionality. The long-term goal is migrating them to Starlark:

| Rule | Current Status | Migration Target |
|------|----------------|-----------------|
| `cc_library`, `cc_binary`, `cc_test` | Native (Buck2) | `@rules_cc` Starlark |
| `cc_import`, `cc_shared_library` | Native (Buck2) | `@rules_cc` Starlark |
| `cc_toolchain`, `cc_toolchain_suite` | Native (Buck2) | `@rules_cc` Starlark |
| `sh_binary`, `sh_test`, `sh_library` | Native (Buck2) | `@rules_shell` Starlark |
| `cc_libc_top_alias` | Native (Buck2) | Remove or migrate |
| `execution_platform`, `execution_platforms` | Native (Buck2) | MODULE.bazel function |
| `label_flag` | Native (Buck2) | Evaluate if needed |

### Implementation Strategy

**Phase 7a.1: Verify Existing Rules**
- [x] Verify `alias`, `filegroup`, `genrule`, `test_suite`, `sh_binary`, `sh_test` match Bazel API (2026-02: all verified with tests)
- [x] Update attribute names/semantics if different

**Phase 7a.2: Platform Rules (Critical for Toolchains)**
- [x] Implement `constraint_setting` rule (2026-02: implemented in native_rules.rs)
- [x] Implement `constraint_value` rule
- [x] Implement `platform` rule
- [x] Implement `toolchain` rule
- [x] Implement `toolchain_type` rule

**Phase 7a.3: Missing Rules**
- [x] Implement `config_setting` rule (critical for `select()`) (2026-02: implemented with constraint_values + flag_values + values + define_values)
- [x] Implement `genquery` rule (2026-02: implemented in native_rules.rs)
- [x] Implement `sh_library` rule (2026-02: implemented as native rule)
- [x] (Low priority) `starlark_doc_extract` (2026-02: implemented as native rule)

**Phase 7a.4: Buck2-Heritage Rule Migration (future)**
- [ ] Migrate cc_*/sh_* from native Rust to Starlark via rules_cc/rules_shell — detailed in [Plan 27](./27-native-language-rule-removal.md)
- [ ] Remove `execution_platform`/`execution_platforms` as BUILD rules (use MODULE.bazel `register_execution_platforms()`) — detailed in [Plan 27](./27-native-language-rule-removal.md)
- [ ] Remove `cc_libc_top_alias` (Buck2-specific unless Bazel 9 source audit proves otherwise) — detailed in [Plan 27](./27-native-language-rule-removal.md)

### Success Criteria (Phase 7a)

- [x] All true Bazel 9 native rules available in BUILD files without `load()`
- [x] `select()` works with `config_setting`
- [x] Platform/toolchain rules work for rules_cc toolchain resolution
- [x] Bazel BUILD files using native rules parse correctly
- [ ] Buck2-heritage rules migrated to Starlark (future)

---

## Phase 7b: Bazel Global Functions

### Overview

These functions must be available in all .bzl files without any `load()` statement.

### Global Functions for .bzl Files

| Function | Description | Kuro Status | Location |
|----------|-------------|-------------|----------|
| `analysis_test_transition` | Config transition for analysis tests | ✓ Stub | `natives.rs` |
| `aspect` | Defines aspect for dependency propagation | ✓ Implemented | `aspect.rs` (Phase 8) |
| `configuration_field` | References late-bound defaults | ✓ Stub | `configuration_field.rs` |
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
| `exports_files` | Marks files as exported | ✓ Implemented | `native_rules.rs` |
| `glob` | Returns files matching patterns | ✓ Implemented | `natives.rs` |
| `module_name` | Returns module name | ✓ Implemented | `natives.rs` |
| `module_version` | Returns module version | ✓ Implemented | `natives.rs` |
| `package` | Declares package metadata | ✓ Implemented | `package.rs` |
| `package_default_visibility` | Returns default visibility | ✓ Implemented | `natives.rs` |
| `package_group` | Defines package set for visibility | ✓ Implemented | `native_rules.rs` |
| `package_name` | Returns package name | ✓ Implemented | `natives.rs` |
| `package_relative_label` | Converts string to Label | ✓ Implemented | `natives.rs` |
| `repo_name` | Returns canonical repo name | ✓ Implemented | `natives.rs` |
| `repository_name` | Deprecated variant | ✓ Implemented | `natives.rs` |
| `select` | Configurable attributes | ✓ Implemented | |
| `subpackages` | Lists direct subpackages | ✓ Implemented | `path.rs` |

### Implementation Strategy

**Phase 7b.1: Verify Existing Functions**
- [x] Audit all implemented functions match Bazel signatures (2026-02-25)
- [x] Add missing parameters where needed (glob exclude_directories added)

**Phase 7b.2: Missing Functions**
- [x] Implement `package_default_visibility()` (deprecated setter, delegates to set_build_file_default_visibility)
- [x] Implement `package_group()` rule (registers target with visibility attrs)
- [x] Implement `subpackages()` (returns direct subpackage paths)
- [x] Implement `exports_files()` (registers each file as native filegroup target)
- [x] Implement `configuration_field()` (stub in configuration_field.rs)
- [x] Implement `analysis_test_transition()` (stub returning settings dict)

### Success Criteria (Phase 7b)

- [x] All global functions available without load()
- [x] Function signatures match Bazel documentation
- [ ] `package_group` works for visibility specifications (registered, full enforcement not verified)

---

## Phase 7c: Bazel Top-Level Modules

### Overview

These modules must be available as globals in .bzl files. These are the **native
infrastructure** that external Starlark rulesets depend on to define their rules.
Making these robust is the correct way to support language rules — NOT by adding
native rule implementations.

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

> This is the **most critical module** for Bazel compatibility. `rules_cc` defines
> all cc_* rules in Starlark using `cc_common.compile()`, `cc_common.link()`, etc.
> Making this module complete and correct is how Kuro supports C++ builds.

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `compile()` | C++ compilation | ✓ Functional (creates real actions) |
| `create_compilation_context()` | Create CompilationContext | ✓ Implemented |
| `create_compilation_outputs()` | Create CcCompilationOutputs | ✓ Implemented |
| `create_compile_variables()` | Generate compilation vars | ✓ Implemented |
| `create_linker_input()` | Create LinkerInput | ✓ Implemented |
| `create_linking_context()` | Create LinkingContext | ✓ Implemented |
| `create_link_variables()` | Generate linking vars | ✓ Implemented |
| `create_library_to_link()` | Create LibraryToLink | ✓ Implemented |
| `configure_features()` | Create FeatureConfiguration | ✓ Implemented (respects requested/unsupported) |
| `link()` | C++ linking | ✓ Functional (creates real actions) |
| `merge_cc_infos()` | Merge CcInfo providers | ✓ Implemented |
| `is_enabled()` | Check feature enabled | ✓ Implemented |
| `action_is_enabled()` | Check action enabled | ✓ Implemented |
| `get_tool_for_action()` | Get tool path | ✓ Implemented (detects cc/clang/cl) |
| `get_memory_inefficient_command_line()` | Get command line | ✓ Implemented |
| `get_environment_variables()` | Get env vars | ✓ Implemented |
| `get_execution_requirements()` | Get exec requirements | ✓ Implemented |
| `CcToolchainInfo` | Provider | ✓ Implemented |
| `CcInfo` | Provider | ✓ Implemented |

**Status**: ✓ Substantially complete (all methods implemented, tested with rules_cc)

### Module: `config`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `config.bool()` | Boolean build setting | ✓ Implemented |
| `config.int()` | Integer build setting | ✓ Implemented |
| `config.string()` | String build setting | ✓ Implemented |
| `config.string_list()` | String list setting | ✓ Implemented |
| `config.string_set()` | String set setting | ✓ Implemented |
| `config.exec()` | Execution transition | ✓ Implemented |
| `config.target()` | No-op target transition | ✓ Implemented |
| `config.none()` | Remove all configuration | ✓ Implemented |

**Status**: ✓ Complete (all methods implemented in config.rs)

### Module: `platform_common`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `ConstraintSettingInfo` | Provider | ✓ Implemented |
| `ConstraintValueInfo` | Provider | ✓ Implemented |
| `PlatformInfo` | Provider | ✓ Implemented |
| `TemplateVariableInfo` | Provider | ✓ Implemented |
| `ToolchainInfo` | Provider | ✓ Implemented |

**Status**: ✓ Complete (all providers in platform_common.rs)

### Module: `testing`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `analysis_test()` | Creates analysis test | ✓ Implemented |
| `ExecutionInfo` | Provider | ✓ Implemented |
| `TestEnvironment` | Provider (deprecated) | ✓ Implemented |

**Status**: ✓ Complete (analysis_test in cc_common.rs, ExecutionInfo as provider)

### Module: `coverage_common`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `instrumented_files_info()` | Creates InstrumentedFilesInfo | ✓ Stub |

**Status**: ✓ Stub implemented (coverage_common.rs)

### Module: `proto`

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `proto.encode_text()` | Encode proto to text | ✓ Implemented |

**Status**: ✓ Implemented (proto_common.rs, 2026-03-11)

### Module: `java_common`

> This module is what `rules_java` uses to define java_library etc. in Starlark.
> Improving these stubs is how Kuro will support Java builds — NOT by adding
> native `java_library` rules.

| Function | Description | Kuro Status |
|----------|-------------|-------------|
| `compile()` | Java compilation | ✓ Stub |
| `merge()` | Merge JavaInfo | ✓ Stub |
| `JavaRuntimeInfo` | Provider | ✓ Stub (attr) |
| `JavaToolchainInfo` | Provider | ✓ Stub (attr) |
| `JavaInfo` | Provider | ✓ Stub |
| `JavaPluginInfo` | Provider | ✓ Stub |

**Status**: ✓ Stubs implemented (java_common.rs, 2026-03-11)

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
- [x] Implement `config` module fully (2026-02: all methods in config.rs)
- [x] Implement `platform_common` module (2026-02: all providers in platform_common.rs)
- [x] Complete `cc_common` beyond stubs (2026-03: configure_features, link, compile, get_tool_for_action, command line generation)

**Phase 7c.2: Supporting Modules**
- [x] Implement `testing.analysis_test()` (2026-02: in cc_common.rs with late binding)
- [x] Implement `coverage_common` (2026-02: stub in coverage_common.rs)

**Phase 7c.3: Lower Priority**
- [x] Implement `proto` module (proto_common exists; `proto.encode_text()` added 2026-03-11)
- [x] Implement `java_common` module (2026-03-11: stubs in java_common.rs)

### Success Criteria (Phase 7c)

- [x] All modules available as globals in .bzl files
- [x] Module method signatures match Bazel documentation
- [x] rules_cc can use `config.exec()`, platform providers

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

Per 06-prelude-architecture.md, these language-specific directories should be removed:

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
- [ ] Remove `read_config()`, `read_root_config()` (or make them error with migration message) — deferred: still used by prelude
- [x] Remove `oncall()`, `read_oncall()` — never registered as Starlark global, only internal
- [x] Remove `load_symbols()` — already returns error with migration message
- [x] Verify `soft_error()` already errors in OSS — confirmed, validates category prefix

**Phase 7d.2: Deprecation Warnings**
- [ ] Add deprecation warning to `attrs.*` functions (suggest `attr.*`) — deferred: would be noisy
- [ ] Document `ctx.attr` as preferred over `ctx.attrs`

**Phase 7d.3: Prelude Cleanup**
- [ ] Remove unused prelude directories (per 06-prelude-architecture.md)
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
- [x] All true Bazel 9 native rules available without `load()`
- [x] Platform/toolchain rules work
- [ ] Buck2-heritage language rules migrated to Starlark (future)

### Phase 7b (Global Functions)
- [x] All global functions match Bazel signatures
- [ ] `package_group` visibility works (registered, enforcement not fully verified)

### Phase 7c (Modules)
- [x] `config` module fully implemented
- [x] `platform_common` module implemented
- [x] All module methods match Bazel documentation

### Phase 7d (Buck2 Removal)
- [ ] Buck2-specific functions removed/deprecated (partially: load_symbols errors, oncall not registered)
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
