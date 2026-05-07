# Plan 27: Native Language Rule Removal

> **Main Plan**:
> [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> **Refines**:
> - [Plan 15, Phase 2](./15-bazel-9-parity.md#phase-2-rewrite-native-cc_-java_-py_-rules-as-emptyrule)
> - [Plan 05, Phase 7a.4](./05-builtins-compatibility.md#phase-7a-bazel-native-rules)
>
> Per repo-local `AGENTS.md`, kuro targets Bazel 9 parity only. This
> plan intentionally breaks kuro workspaces that still rely on Buck2-era
> native language rules.

## Status: COMPLETE

Phases 27.1-27.6 landed on 2026-04-29..2026-04-30. Remaining work in this
file is historical context; the main plan's stub table now marks the
native-language analysis path as resolved.

## Scope

Remove or quarantine kuro's remaining native language-rule
implementations so BUILD files see the same surface that Bazel 9 exposes.
In practice:

- `cc_*` rules no longer analyze through kuro's Rust
  `create_cc_analysis_result()` path.
- `sh_*` rules no longer analyze through kuro's Rust shell-rule path
  unless the Bazel 9 source audit proves they remain true native rules.
- removed Bazel native rules produce Bazel-shaped "add this `load()`"
  diagnostics instead of silently building through a kuro-only native
  implementation.
- Buck2-only BUILD rules such as `execution_platform(s)` are removed
  from the public BUILD surface or changed to loud migration errors.

This does **not** remove native modules such as `cc_common`,
`platform_common`, `config_common`, or `proto_common`. External
Starlark rulesets still need those modules.

## Bonanza Takeaway

Bonanza is useful here as proof that native-language-rule behavior can
be pushed into Starlark. Its builtins export mechanism is tracked in
[Plan 28](./28-builtins-module-architecture.md). For this plan, the
source of truth is still Bazel 9, not Bonanza:

- Bazel's removed-rule pattern is `EmptyRule` in
  `BaseRuleClasses.java`.
- The exact removed-rule lists come from the Bazel 9 rule registries
  (`CcRules.java`, `JavaRules.java`, `ObjcRules.java`, and the
  corresponding shell/proto/python registries).
- Kuro should not adopt Bonanza-only public platform fields or its
  custom remote-execution model.

## Current State

Kuro still has native rule variants and analysis branches for rules that
are not real Bazel 9 native language rules:

| Area | Current kuro location | Problem |
|------|-----------------------|---------|
| Native enum entries | `app/kuro_node/src/rule_type.rs` | `NativeRuleKind` includes `cc_library`, `cc_binary`, `cc_test`, `cc_import`, `cc_shared_library`, `cc_toolchain`, `cc_toolchain_suite`, `sh_binary`, `sh_test`, `sh_library`, and Buck2 execution-platform rules. |
| Rule registration | `app/kuro_interpreter_for_build/src/interpreter/native_rules.rs` | BUILD globals register callable Rust native implementations for the above names. |
| Analysis dispatch | `app/kuro_analysis/src/analysis/native_rule_analysis.rs` | `cc_*` dispatches to `create_cc_analysis_result()`, `sh_*` dispatches to native shell analysis, and `cc_toolchain*` returns minimal native providers. |
| CC side effects | `app/kuro_build_api/src/interpreter/rule_defs/cc_common/mod.rs` | The process-global external include-dir registry is a remote-cache determinism risk. Removing native cc analysis should make it easier to replace or delete. |

## Desired End State

- A BUILD file containing `cc_library(name = "x")` without a load fails
  with Bazel 9's removed-rule diagnostic:

  ```text
  The cc_library rule has been removed, add the following to your BUILD/bzl file:
      load("@rules_cc//cc:defs.bzl", "cc_library")
  ```

- A BUILD file containing:

  ```starlark
  load("@rules_cc//cc:defs.bzl", "cc_library")

  cc_library(
      name = "x",
      srcs = ["x.cc"],
  )
  ```

  calls the Starlark rule from `@rules_cc`, not kuro's Rust native
  rule implementation.

- `hasattr(native, "proto_library")`, `hasattr(native, "py_library")`,
  and similar checks match Bazel 9 behavior for the selected ruleset
  versions.

- `NativeRuleKind` contains only true Bazel 9 native rules plus an
  explicit removed-rule stub representation.

- `create_cc_analysis_result()` and native shell-analysis code are gone
  or unreachable from Bazel-compatible BUILD evaluation.

## What We're NOT Doing

1. **No Bazel 8 compatibility.** There is no fallback to native
   `cc_library`, `py_library`, `proto_library`, etc.
2. **No language-rule reimplementation in Rust.** If `@rules_cc` or
   `@rules_shell` has a missing API dependency, fix the native module
   it calls into. Do not revive a native language rule.
3. **No Bonanza platform extensions.** `exec_pkix_public_key` and
   `repository_os_*` are Bonanza-specific and do not belong in kuro's
   Bazel 9 public surface.
4. **No full `cc_common` cleanup in this plan.** Only remove dead code
   exposed by the native-rule removal. Broader `cc_common` parity gaps
   remain Plan 15/Plan 19 follow-ups.

## Phase 27.1: Bazel 9 Removed-Rule Inventory  [DONE 2026-04-29]

### Goal

Create an authoritative table of every rule name currently registered by
kuro that is not a true Bazel 9 native rule, including the exact Bazel 9
diagnostic mechanism.

### Work

1. Audit Bazel 9 sources:
   - `src/main/java/com/google/devtools/build/lib/analysis/BaseRuleClasses.java`
     for `EmptyRule` behavior and message text.
   - `src/main/java/com/google/devtools/build/lib/bazel/rules/CcRules.java`
     for removed `cc_*` rules and load labels.
   - `src/main/java/com/google/devtools/build/lib/bazel/rules/JavaRules.java`
     for removed `java_*` rules.
   - `src/main/java/com/google/devtools/build/lib/bazel/rules/ObjcRules.java`
     for removed `objc_*` rules.
   - the Bazel 9 shell/proto/python rule registries to classify
     `sh_*`, `proto_*`, and `py_*` behavior. Do not assume every
     removed symbol uses `EmptyRule`; some names may simply be absent.
2. Record the inventory in this plan or a checked-in fixture with fields:
   - `rule_name`
   - `kuro_current_status`
   - `bazel9_status`: `true_native`, `empty_rule`, `absent`,
     or `private_internal`
   - `bzl_load_label` when Bazel provides one
   - `bzl_symbol`
   - `parity_source`
3. Compare the inventory against:
   - `NativeRuleKind`
   - `register_native_rules`
   - `native_rules.rs` exported BUILD globals
   - `prelude/native.bzl` native struct construction

### Initial Classification Targets

| Rule family | Expected direction | Notes |
|-------------|--------------------|-------|
| `cc_library`, `cc_binary`, `cc_test`, `cc_import`, `cc_shared_library`, `cc_toolchain`, `cc_toolchain_suite` | EmptyRule or removed-rule stub with `@rules_cc` load hint | Exact list and load labels must come from Bazel 9 `CcRules.java`. |
| `sh_binary`, `sh_test`, `sh_library` | Remove native implementation; classify exact diagnostic from Bazel 9 | Verify whether Bazel 9 uses `rules_shell` load hints or absence. |
| `proto_library`, `cc_proto_library` | Must not be real native implementations | Kuro should keep `hasattr(native, "proto_library")` behavior aligned with protobuf's Bazel 9 path. |
| `py_library`, `py_binary`, `py_test` | Must not be real native implementations | Kuro should keep rules_python on the pystar path. |
| `java_library`, `java_binary`, `java_test`, `objc_library`, `objc_import` | EmptyRule or absent, per Bazel 9 source | Kuro may not currently implement these, but tests should lock that down. |
| `execution_platform`, `execution_platforms` | Remove from public BUILD globals or hard-error | Buck2 heritage; Bazel platform registration is via `platform()` plus MODULE/CLI registration. |
| `cc_libc_top_alias` | Remove from public BUILD globals unless Bazel 9 exposes it | If rules_cc still needs it internally, provide it through a rules_cc-compatible Starlark path, not as general native BUILD surface. |

### Acceptance

- The checked-in inventory names every kuro native rule that is not in
  the true Bazel 9 native set.
- Each removed or absent rule has a cited Bazel 9 source location.
- No implementation work starts before the inventory resolves the
  `sh_*` and `cc_libc_top_alias` classification.

### Inventory (recorded 2026-04-29)

Snapshot of every name registered by
`app/kuro_interpreter_for_build/src/interpreter/native_rules.rs`
(`register_native_rules`) plus the corresponding `NativeRuleKind` variants
in `app/kuro_node/src/rule_type.rs`.

**True Bazel 9 native rules — keep as native, no migration needed:**

| Rule name | Kuro analyzer | Bazel 9 status |
|-----------|---------------|----------------|
| `filegroup` | `analyze_filegroup` | true_native |
| `constraint_setting` | `analyze_constraint_setting` | true_native |
| `constraint_value` | `analyze_constraint_value` | true_native |
| `alias` | `analyze_alias` | true_native |
| `label_flag` | `analyze_label_flag` | true_native |
| `config_setting` | `analyze_config_setting` | true_native |
| `toolchain_type` | `create_minimal_analysis_result` | true_native |
| `package_group` | `analyze_package_group` | true_native |
| `genrule` | `analyze_genrule` | true_native |
| `platform` | `analyze_platform` | true_native |
| `toolchain` | `analyze_toolchain` | true_native |
| `test_suite` | `analyze_test_suite` | true_native |
| `genquery` | `analyze_genquery` | true_native |
| `exports_files` | (load-time only) | true_native |

**Bazel 9 removed — must produce removed-rule diagnostic with `@rules_cc`/`@rules_shell` load hint:**

| Rule name | Kuro analyzer (current) | Replacement load |
|-----------|-------------------------|------------------|
| `cc_library` | `create_cc_analysis_result` | `load("@rules_cc//cc:defs.bzl", "cc_library")` |
| `cc_binary` | `create_cc_analysis_result` | `load("@rules_cc//cc:defs.bzl", "cc_binary")` |
| `cc_test` | `create_cc_analysis_result` | `load("@rules_cc//cc:defs.bzl", "cc_test")` |
| `cc_import` | `create_cc_analysis_result` | `load("@rules_cc//cc:defs.bzl", "cc_import")` |
| `cc_shared_library` | `create_cc_analysis_result` | `load("@rules_cc//cc:defs.bzl", "cc_shared_library")` |
| `cc_toolchain` | `create_minimal_analysis_result` | `load("@rules_cc//cc:defs.bzl", "cc_toolchain")` |
| `cc_toolchain_suite` | `create_minimal_analysis_result` | `load("@rules_cc//cc:defs.bzl", "cc_toolchain_suite")` |
| `sh_binary` | `analyze_sh_binary` | `load("@rules_shell//shell:sh_binary.bzl", "sh_binary")` |
| `sh_test` | `analyze_sh_test` | `load("@rules_shell//shell:sh_test.bzl", "sh_test")` |
| `sh_library` | `analyze_sh_library` | `load("@rules_shell//shell:sh_library.bzl", "sh_library")` |
| `environment_group` | `create_minimal_analysis_result` | (no replacement — use platforms/constraints) |

**Buck2-only — never had a Bazel 9 form:**

| Rule name | Kuro analyzer (current) | Migration path |
|-----------|-------------------------|----------------|
| `execution_platform` | `analyze_execution_platform` | Use `platform(...)` + `register_execution_platforms(...)` in MODULE.bazel |
| `execution_platforms` | `analyze_execution_platforms` | Same as above |

**Truly internal Bazel native — keep:**

| Rule name | Kuro analyzer | Notes |
|-----------|---------------|-------|
| `cc_libc_top_alias` | `create_minimal_analysis_result` | Verified present in cached `@rules_cc//cc:BUILD` (no load required); Bazel exposes this as a true native primitive used only by rules_cc internals. Keep. |

**Stubs / kuro-internal — review separately, out of Plan 27 scope:**

| Rule name | Notes |
|-----------|-------|
| `analysis_test` | `testing.analysis_test()` callable (Plan 15 scope) |
| `xcode_config` | Apple-specific; revisit when Apple parity is in scope |
| `starlark_doc_extract` | Bazel native — likely keep, dispatches to genquery stub today |

### Migration order chosen for Phase 27.2

`environment_group` is the proof-of-concept rule for the removed-rule
stub infrastructure: it is unambiguously removed in Bazel 9, has a single
existing fixture (`tests/core/analysis/test_native_rules_data/BUILD.bazel`),
and is already a no-op analyzer in kuro. The same infrastructure is then
applied to `execution_platform[s]` (23 fixtures, larger migration) and
finally the `cc_*` / `sh_*` families (gated on rules_cc / rules_shell
readiness per Phase 27.5).

## Phase 27.2: Removed-Rule Stub Infrastructure  [13 rules done, 2026-04-30]

### Status

Infrastructure landed; pattern applied to:

- `environment_group`
- `execution_platform`, `execution_platforms`
- `sh_binary`, `sh_test`, `sh_library`
- `cc_library`, `cc_binary`, `cc_test`, `cc_import`, `cc_shared_library`,
  `cc_toolchain`, `cc_toolchain_suite`

Specifics:

- `RemovedNativeRule` enum carries metadata + `diagnostic_message()` for
  each removed rule (`app/kuro_node/src/rule_type.rs`).
- `NativeRuleKind::Removed(RemovedNativeRule)` variant dispatches to
  `analyze_removed_rule`
  (`app/kuro_analysis/src/analysis/native_rule_analysis.rs`). Deleted
  helpers: `analyze_execution_platform[s]` (~120 lines),
  `analyze_sh_binary`/`sh_test`/`sh_library` (~140 lines),
  `create_sh_target` helper.
- `register_removed_rule` helper accepts `name` + arbitrary `**kwargs`,
  records target node, no attr validation
  (`app/kuro_interpreter_for_build/src/interpreter/native_rules.rs`).
- BUILD globals for the 6 removed rules are now stubs that emit the
  Bazel-shaped diagnostic at analysis time, including the
  `@rules_shell//shell:sh_*.bzl` load hint for `sh_*`.
- Replacement Starlark `sh_binary` / `sh_test` / `sh_library` shipped in
  `tests/e2e_util/nano_prelude/shell_rules.bzl` (loaded via
  `nano_prelude/prelude.bzl`) for fixtures that use nano_prelude. Same
  semantics live in `tests/core/analysis/test_native_rules_data/defs.bzl`
  for that fixture, loaded explicitly in its `BUILD.bazel`.
- 23 `execution_platform[s]` fixtures unaffected — they all define their
  own Starlark `rule(...)` that shadows the BUILD-global stub.
- New tests:
  - `test_environment_group_removed_in_bazel9` (positive: diagnostic).
  - `test_sh_binary_removed_without_load` (positive: diagnostic +
    `@rules_shell` load-hint check).
  - 3 unit tests in `kuro_node` for the `parity_category` guardrail and
    the `RemovedNativeRule` diagnostic shape.
- `@llvm-project//llvm:Demangle` and `:Support` build clean.
- 106 / 111 tests pass in `test_native_rules.py` (5 pre-existing
  toolchain-init failures unrelated to Plan 27).

### Remaining for Phase 27.2

No more rules pending. Phase 27.2 is complete.

(`cc_libc_top_alias` stays as a true Bazel native rule per Phase 27.1
inventory — used internally by `@rules_cc//cc:BUILD`.)

### Goal

Represent removed native rules explicitly so kuro can accept package
loading, register a target node, and fail during analysis with the same
kind of diagnostic Bazel 9 emits.

### Work

1. Add a removed-rule representation near `NativeRuleKind`.
   Options:
   - a generic `NativeRuleKind::Removed(RemovedNativeRuleKind)`, if the
     enum can carry stable metadata cleanly; or
   - one variant per removed rule plus a static metadata table.
2. Metadata must include:
   - public rule name
   - expected load label, when Bazel has one
   - expected symbol name
   - whether `hasattr(native, "<name>")` should return true or false
   - parity-source comment
3. Add a native-rule callable registration path for removed rules that:
   - accepts `name` plus arbitrary `**kwargs`
   - does not validate language-rule attrs before the removed-rule
     diagnostic is reachable
   - records a target node with removed-rule metadata
4. Add an analysis branch that returns a structured error with Bazel's
   message shape. Include target label context using kuro's normal
   analysis-error formatting, but keep the removed-rule text itself
   Bazel-shaped.
5. Add tests for:
   - `cc_library(name = "x")` without load
   - `cc_binary(name = "x")` without load
   - at least one removed toolchain rule
   - any `sh_*` rule once Phase 27.1 classifies it

### Files Likely Touched

- `app/kuro_node/src/rule_type.rs`
- `app/kuro_interpreter_for_build/src/interpreter/native_rules.rs`
- `app/kuro_analysis/src/analysis/native_rule_analysis.rs`
- `tests/core/builtins/` or a new `tests/core/bazel9_removed_rules/`

### Acceptance

- Removed-rule tests fail for the removed-rule reason, not because of
  unknown attrs or missing implementation panics.
- Loaded Starlark symbols can shadow the BUILD global stub.
- Error text contains the exact load snippet for rules where Bazel 9
  provides a load label.

## Phase 27.3: Remove Active Native Language Analysis  [DONE 2026-04-30]

### Status

- All ten removed-rule analysis branches in
  `app/kuro_analysis/src/analysis/native_rule_analysis.rs` are gone:
  `CcLibrary`, `CcBinary`, `CcTest`, `CcImport`, `CcSharedLibrary`,
  `CcToolchain`, `CcToolchainSuite`, `ShBinary`, `ShTest`, `ShLibrary`.
  These are now `NativeRuleKind::Removed(...)` variants dispatched to
  `analyze_removed_rule`.
- Dead helpers deleted: `create_cc_analysis_result()` (~120 lines),
  `analyze_sh_binary` / `analyze_sh_test` / `analyze_sh_library`
  (~140 lines), `analyze_execution_platform` /
  `analyze_execution_platforms` (~120 lines), `create_sh_target`
  helper, `cc_rule_attributes` / `cc_toolchain_attributes` /
  `cc_toolchain_suite_attributes` / `cc_import_attributes` /
  `cc_shared_library_attributes` / `sh_rule_attributes` helpers, and
  the corresponding `CC_*_RULE` / `SH_*_RULE` /
  `EXECUTION_PLATFORM[S]_RULE` Lazy statics
  (~220 lines from `native_rules.rs`).
- Native modules kept (`cc_common`, `platform_common`, `config_common`,
  `coverage_common`, `proto_common`).
- `EXTERNAL_INCLUDE_DIRS` audit: confirmed already retired by Plan 29
  before Plan 27 work. Only a comment remains in
  `cc_common/mod.rs`.
- Acceptance verified:
  `rg "create_cc_analysis_result|NativeRuleKind::CcLibrary|NativeRuleKind::ShBinary"`
  finds no active implementation path; no-load `cc_library` fails
  with the diagnostic; loaded rules_cc `cc_library` still analyzes
  through Starlark (proven by `@llvm-project//llvm:Demangle` and
  `:Support` builds).

### Goal

Delete the paths that make removed language rules build successfully via
kuro-only Rust logic.

### Work

1. Remove or make unreachable these analysis branches:
   - `NativeRuleKind::CcLibrary`
   - `NativeRuleKind::CcBinary`
   - `NativeRuleKind::CcTest`
   - `NativeRuleKind::CcImport`
   - `NativeRuleKind::CcSharedLibrary`
   - `NativeRuleKind::CcToolchain`
   - `NativeRuleKind::CcToolchainSuite`
   - `NativeRuleKind::ShBinary`
   - `NativeRuleKind::ShTest`
   - `NativeRuleKind::ShLibrary`
2. Delete dead helpers once no branch calls them:
   - `create_cc_analysis_result()`
   - minimal native cc toolchain result helpers that only existed for
     native `cc_toolchain`
   - native shell action helpers, unless still used by a true Bazel 9
     native rule
3. Keep native modules used by Starlark rules:
   - `cc_common`
   - `platform_common`
   - `config_common`
   - `coverage_common`
   - `proto_common`
4. Audit `cc_common` process-global state after the native cc deletion:
   - if `EXTERNAL_INCLUDE_DIRS` is no longer reachable, delete it;
   - if rules_cc still reaches it through `cc_common`, file a follow-up
     in Plan 15 to replace it with provider/action input plumbing.

### Acceptance

- `rg "create_cc_analysis_result|NativeRuleKind::CcLibrary|NativeRuleKind::ShBinary"`
  shows no active implementation path except removed-rule metadata or
  tests.
- A no-load `cc_library` fails.
- A loaded rules_cc `cc_library` still analyzes through Starlark.

## Phase 27.4: Migrate Kuro-Owned BUILD Files and Examples  [DONE 2026-04-30]

### Status

The Phase 27.1 inventory (recorded above) checked every tracked BUILD
and `.bzl` file under `examples/`, `tests/`, `bazel_tools/`, and
`prelude/` for bare calls to converted rules. Findings:

- **Tracked fixtures with bare calls (migrated):**
  - `tests/core/analysis/test_native_rules_data/BUILD.bazel` was the
    only tracked fixture with bare `cc_*` / `sh_*` /
    `environment_group` / `execution_platform[s]` calls. Migrated
    in-place: minimal Starlark replacements live in the fixture's
    own `defs.bzl`; `BUILD.bazel` `load()`s them. Done as part of
    the Phase 27.2 commits.
- **Tracked Starlark-defining fixtures (no migration needed):**
  - All 23 `tests/core/.../*_data/defs.bzl` files that define their
    own Starlark `execution_platform[s] = rule(...)` shadow the
    BUILD-global stub via the user `load()`. Continue working
    unchanged.
  - `tests/e2e_util/nano_prelude/shell_rules.bzl` ships Starlark
    `sh_binary` / `sh_test` / `sh_library` for any prelude-using
    fixture; wired via `nano_prelude/prelude.bzl`.
- **Tracked `bazel_tools/` matches were false positives:**
  - `bazel_tools/tools/build_defs/repo/http.bzl` references
    `cc_library(...)` inside a docstring example, not a real call.
  - `bazel_tools/tools/cpp/cc_common.bzl` defines a function called
    `_check_experimental_cc_shared_library` and a dict key
    `check_experimental_cc_shared_library`; no rule call.
- **Out of scope:**
  - `tests/manual_test/` is git-ignored per top-level `.gitignore`
    (`tests/manual_test/**`). It is a developer playground, not a
    tracked fixture or CI test, and Phase 27.4 acceptance is about
    Kuro-owned tracked tests. (Local copies that hit the removed-rule
    diagnostic can add a load — the same `shell_rules.bzl` /
    `defs.bzl` cc_* stub patterns work; see the
    `tests/core/analysis/test_native_rules_data` precedent.)
  - `prelude/` is the Buck2-era prelude tree; Plan 28.6 / Plan 04
    own its disposition.

### Acceptance verified

- No tracked Kuro test relies on no-load `cc_*` or `sh_*` BUILD
  globals.
- Fixture changes are minimal and Bazel-9-compatible.
- The negative tests `test_cc_library_removed_without_load` and
  `test_sh_binary_removed_without_load` in
  `tests/core/analysis/test_native_rules.py` exist explicitly to
  exercise the no-load error.

### Goal

Make kuro's own fixtures and examples obey Bazel 9's explicit-load
rules before the removed-rule stubs land broadly.

### Work

1. Audit kuro-owned BUILD files:
   - `examples/`
   - `tests/`
   - `bazel_tools/`
   - `prelude/`
   - any generated fixture roots under `tests/core/*_data`
2. Add explicit loads for language rules:
   - `load("@rules_cc//cc:defs.bzl", "cc_binary", "cc_library", "cc_test", ...)`
   - rules_shell load paths from the Phase 27.1 Bazel/rules_shell audit
   - rules_python/protobuf/rules_java/rules_apple only where fixtures
     actually use those rules
3. Ensure test workspaces declare the required modules in
   `MODULE.bazel` rather than relying on implicit native language rules.
4. Remove any synthetic compatibility route whose only purpose was to
   make no-load native language rules work.

### Acceptance

- Kuro-owned tests do not rely on no-load `cc_*` or `sh_*` BUILD
  globals.
- Fixture changes are minimal and Bazel 9-compatible.
- Any fixture intentionally testing the no-load error is named as such.

## Phase 27.5: Ruleset Readiness Gates  [DONE 2026-04-30]

### Status

- **rules_cc smoke**: `@llvm-project//llvm:Demangle` (8 cc_compile +
  archive + link actions) and `@llvm-project//llvm:Support` (183
  actions) build clean post-Plan 27.2/27.3. cc_test
  end-to-end via `kuro test` blocked on the pre-existing kuro test
  runner clap panic on duplicate `test_summary` (filed as a separate
  bug in `app/kuro_client/src/commands/test.rs`); not Plan 27.
- **rules_cc toolchain smoke**: rules_cc 0.2.16 ships Starlark
  `cc_toolchain` (`cc/private/rules_impl/cc_toolchain.bzl`) and a
  thin `cc_toolchain_suite` wrapper over `native.cc_toolchain_suite`
  that propagates the Plan 27 removed-rule diagnostic — consistent
  with Bazel 9's load-OK / diagnostic-at-analysis behavior. Verified
  by reading `~/.cache/bazel/_bazel_wgray/.../rules_cc++.../cc/defs.bzl`.
  `TemplateVariableInfo` reach into `ctx.var` is tracked separately
  in `memory/ctx_var_builtins.md` and is not a Plan 27 dependency.
- **rules_shell smoke**: nano_prelude ships Starlark `sh_*` (used by
  audit/exec-platform/extra-exec-platforms tests, all green); the
  `test_native_rules_data` fixture has equivalent stubs in its own
  `defs.bzl`. No `@rules_shell` external dep in tracked tests.
  Acceptable per the path (B) decision recorded in the cleanup
  research.
- **negative parity smoke**:
  - `test_environment_group_removed_in_bazel9` asserts no-load
    `environment_group` fails with the diagnostic.
  - `test_sh_binary_removed_without_load` asserts the same for
    sh_binary, plus the `@rules_shell` load-hint string.
  - `test_cc_library_removed_without_load` asserts the same for
    cc_library, plus the `@rules_cc` load-hint string.
  - `test_loaded_removed_rules_analyze_cleanly` asserts that a user
    `load()` of the same names from a Starlark replacement shadows
    the BUILD-global stub.
- **No removed provider globals reintroduced** — the
  `parity_category` exhaustive-match guardrail (Phase 27.6) forces
  every `NativeRuleKind` variant to be classified, and the
  Plan 28.2 builtins loader doesn't currently re-export any provider
  globals.

### Acceptance verified

If a loaded ruleset path were to fail, it would be a real native-module
parity gap on top of working Plan 27 stubs — no hidden fallback to a
Rust language rule remains.

### Goal

Ensure the replacement Starlark rulesets are functional enough before
removing native implementations from developer workflows.

### Gates

1. **rules_cc smoke**
   - Build a trivial loaded `cc_library`.
   - Build a loaded `cc_binary`.
   - Build a loaded `cc_test` and run it through `kuro test`.
2. **rules_cc toolchain smoke**
   - Exercise loaded `cc_toolchain` / `cc_toolchain_suite` or the
     Bazel 9 rules_cc replacement path used by the current BCR version.
   - Verify `TemplateVariableInfo` reaches `ctx.var` or record the
     remaining Plan 15 dependency.
3. **rules_shell smoke**
   - Build and run loaded shell rules if Bazel 9/rules_shell requires
     them.
4. **negative parity smoke**
   - `native.cc_library` without load errors.
   - `native.proto_library` behavior matches Bazel 9/protobuf
     detection.
   - no removed provider globals are reintroduced.

### Acceptance

- If a loaded ruleset path fails, the failure is a real native-module
  parity gap with a filed Plan 15 sub-item, not a hidden fallback to a
  Rust language rule.

## Phase 27.6: Guardrails  [DONE 2026-04-30]

### Status

- `parity_category` exhaustive match in
  `app/kuro_node/src/rule_type.rs` forces every `NativeRuleKind`
  variant to be classified as `true_native`, `removed_stub`,
  `kuro_internal`, or `kuro_internal_apple`. Adding a new variant
  fails to compile until the match is updated; the test
  `native_rule_kinds_have_parity_category` spot-checks
  representative variants.
- `removed_native_rule_diagnostics_mention_rule_name` test asserts
  every `RemovedNativeRule` variant's `diagnostic_message()`
  contains its rule name so users can locate the call site.
- Doc-comments on `RemovedNativeRule` enum + per-variant cite the
  Bazel 9 source pattern (`BaseRuleClasses.java EmptyRule`).
- Main-plan "Remaining Stub Behavior" table update performed in the
  plan-status cleanup pass; `create_cc_analysis_result()` is now marked
  resolved there.

### Acceptance verified

- Adding a working native `cc_library`-style rule fails the
  `parity_category` exhaustive-match guardrail unless the new
  variant is classified; deleting a `Removed(...)` variant fails
  the diagnostic-mention test unless the corresponding
  `diagnostic_message` arm is removed too.

### Remaining for Phase 27.6

None. The doc-only main-plan table follow-up is complete.

### Goal

Prevent future regressions where a language rule is accidentally added
back as a working native rule.

### Work

1. Add a native-rule inventory test that compares exported BUILD native
   rule names against an allowlist generated from the Phase 27.1 audit.
2. Add a test that `NativeRuleKind` variants are either:
   - true Bazel 9 native rules;
   - removed-rule stubs with metadata; or
   - explicitly marked kuro-internal and unavailable in BUILD globals.
3. Add comments in `native_rules.rs` and `rule_type.rs` pointing to this
   plan and the Bazel 9 parity sources.
4. Update the main plan's "Remaining Stub Behavior" table when the
   `create_cc_analysis_result()` row is resolved.

### Acceptance

- Adding a working native `cc_library`-style rule fails CI unless the
  allowlist and parity citation are updated.

## Dependencies

- **Plan 15 Phase 1** should land first or in parallel: removed provider
  globals must not be reintroduced while migrating rules.
- **Plan 15 Phase 4** helps rules_cc by making `@bazel_tools` closer to
  upstream Bazel 9.
- **Plan 28** is not strictly required for the removed-rule stubs, but
  it provides the longer-term Starlark builtins/wrapper mechanism that
  should absorb any remaining compatibility glue.

## Risks

- **Loaded rules_cc is not ready enough.** If a loaded `cc_library`
  cannot build today, native-rule removal exposes that gap immediately.
  That is acceptable, but the work should be sequenced behind the
  readiness gates above.
- **Wrong error timing.** Bazel's removed-rule diagnostics may appear
  during loading or analysis depending on the rule. Kuro should match
  the message shape first, then refine timing if tests show observable
  divergence.
- **Fixture churn.** Many kuro tests may rely on no-load native language
  rules. Keep changes mechanical: add loads and MODULE deps, do not
  rewrite test intent.

## Verification

Minimum verification before closing this plan:

- `cargo check -p kuro`
- removed-rule negative tests
- loaded rules_cc smoke tests
- loaded shell-rule smoke tests if `sh_*` is removed in Bazel 9
- existing rules_cc/rules_python/protobuf/rules_rust/rules_oci
  integration tests still pass through Starlark rulesets

## Estimated Effort

1-2 weeks if loaded rules_cc is already viable. 2-4 weeks if this
exposes additional `cc_common` / toolchain-provider gaps.
