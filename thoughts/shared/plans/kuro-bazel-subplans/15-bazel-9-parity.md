# Plan 15: Full Bazel 9 Parity

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Per repo-local `AGENTS.md`: kuro targets Bazel 9 exclusively. No
> backwards-compatibility, no migration shims. Break kuro workspaces
> freely; no external users depend on the prototype's current surface.

## Scope

Bring kuro into full compliance with Bazel 9.0.x behaviour across four
subsystems that currently diverge. Each phase is independently
shippable. Each phase's "parity source" cites the exact Bazel source
location that defines the target behaviour.

This plan supersedes two prior workarounds:
- kuro's "using lockfile specs anyway" digest-mismatch fallback
  (masked Bazel-compat bugs elsewhere — remove).
- kuro's `register_bazel_provider_globals` exposing `CcSharedLibraryInfo`,
  `PyInternalStub`, etc. as top-level globals (Bazel 9 removed these —
  remove).

## Current State Analysis

### Known divergences (observed empirically)

1. **Provider globals**: kuro exposes `CcSharedLibraryInfo`,
   `CcSharedLibraryHintInfo`, `PackageSpecificationInfo`,
   `RunEnvironmentInfo`, `py_internal`, `CcInfo`, `PyInfo`, `ProtoInfo`,
   `JavaInfo` at the top level of .bzl files. Bazel 9 removed all of
   these from global scope — they must be loaded via `load()`.
   Location: `app/kuro_build_api/src/interpreter/rule_defs/cc_common.rs:7676`
   and adjacent.

2. **Native `cc_*` / `java_*` / `py_*` rules**: kuro implements
   `NativeRuleKind::CcLibrary`, `NativeRuleKind::CcBinary`,
   `NativeRuleKind::CcTest`, etc. as real native rules. Bazel 9 replaced
   all of these with `EmptyRule` stubs that error out asking the user to
   `load("@rules_cc//cc:defs.bzl", "cc_library")`. Location:
   `app/kuro_analysis/src/analysis/native_rule_analysis.rs:152-170`.

3. **Lockfile v24 digest enforcement**: kuro accepts lockfile entries
   with digest mismatches and logs "using lockfile specs anyway". Bazel
   9 errors on digest mismatch and re-executes the extension. Location:
   `app/kuro_bzlmod/src/lockfile.rs:474-479`.

4. **`@bazel_tools` content gaps**: missing `src/conditions/` was
   patched in commit 2260f5f but there are likely more — kuro's
   `bazel_tools/` was hand-curated rather than copied wholesale from
   upstream. Location: `bazel_tools/` tree (diff against
   `/var/mnt/dev/bazel/src/` and `embedded_tools/`).

### Parity sources (Bazel 9 source of truth)

- Symbol removal pattern: `BaseRuleClasses.java:419-495` — `EmptyRule`
  with optional `bzlLoadLabel` attribute. Failing analysis message:
  ```
  The %s rule has been removed, add the following to your BUILD/bzl file:
      load("%s", "%s")
  ```
- Which rules are EmptyRule in 9:
  - `CcRules.java:48-61`: `cc_toolchain`, `cc_toolchain_suite`,
    `cc_binary`, `cc_shared_library`, `cc_static_library`, `cc_test`,
    `cc_library`, `cc_import`, `fdo_profile`, `fdo_prefetch_hints`,
    `memprof_profile`, `propeller_optimize`
  - `JavaRules.java:42-57`: `java_binary`, `java_library`, `java_import`,
    `java_test`, `java_plugin`, `java_toolchain`, `java_package_configuration`,
    `java_runtime`
  - `ObjcRules.java:42-43`: `objc_import`, `objc_library`
- Bundled `@bazel_tools` content: `/var/mnt/dev/bazel/src/*/BUILD.tools`
  is the authoritative source for each subpackage.

## Phase 1: Remove provider globals from top-level scope

### Overview

Force `load()` for every provider currently exposed as a global.
Matches Bazel 9's "symbol has been removed" pattern but applied at the
Starlark environment construction level rather than per-rule.

### Changes

- `app/kuro_build_api/src/interpreter/rule_defs/cc_common.rs:7676`:
  delete `register_bazel_provider_globals`.
- Removal targets (one per symbol, verify each is gone from top-level):
  - `CcSharedLibraryInfo`, `CcSharedLibraryHintInfo` — kept only inside
    `cc_common.*`
  - `PackageSpecificationInfo` — moved to `@rules_license//rules:providers.bzl`
  - `RunEnvironmentInfo` — removed from global scope
  - `py_internal` — removed, rules_python loads it via
    `@rules_python//python/private:py_internal.bzl`
  - `CcInfo` — must load from `@rules_cc//cc/common:cc_info.bzl`
  - `PyInfo`, `PyRuntimeInfo` — load from `@rules_python//python:providers.bzl`
  - `ProtoInfo` — load from `@rules_proto//proto:defs.bzl` (or its
    successor `@rules_proto//proto:proto_info_provider.bzl`)
  - `JavaInfo`, `JavaPluginInfo` — load from `@rules_java//java:defs.bzl`
  - `DebugPackageInfo` — load from `@rules_cc//cc:debug_package_info.bzl`

- Callers in kuro's own .bzl / BUILD files that referenced these as
  globals: audit `bazel_tools/`, `prelude/`, `examples/*` and add the
  required loads. If a `.bzl` file genuinely needs a Bazel 9 symbol, it
  gets the matching Bazel 9 load path.

### Parity source

Bazel 9 `StarlarkGlobalsImpl.java` (construct `.bzl` environment) and
the per-provider removals in Bazel 9.0 changelog commits. Concretely:
the `StarlarkGlobalsImpl` builder explicitly adds each rules_cc /
rules_python / rules_java symbol through a load hook rather than a
direct global binding.

### Success criteria

- Every symbol in `register_bazel_provider_globals` is unreachable from
  a .bzl file without a `load()`.
- kuro prints the same "use load(...)" hint shape that Bazel 9 does
  when a user-written .bzl references a removed global.
- existing kuro-owned .bzl files (bazel_tools, prelude) pass their own
  load requirements — they must compile against the new strict
  environment.

### Est. effort

2-3 days. Most time in fixing internal .bzl call sites that relied on
the globals.

## Phase 2: Rewrite native `cc_*` / `java_*` / `py_*` rules as EmptyRule

### Overview

Match Bazel 9's deliberate removal of native rule implementations for
cc/java/py. After Bazel 9, those rules exist natively only as error
stubs: their sole purpose is to print "add the load statement for
`@rules_cc//...`" when a user writes `cc_library(...)` without the
load.

### Changes

- `app/kuro_node/src/rule_type.rs`: delete `NativeRuleKind::CcLibrary`,
  `CcBinary`, `CcTest`, `CcImport`, `CcSharedLibrary`, `CcToolchain`,
  `CcToolchainSuite`, `CcLibcTopAlias`. Keep `ToolchainType`,
  `Toolchain`, `ConstraintSetting`, `ConstraintValue`, `Platform`,
  `ExecutionPlatform(s)`, `Alias`, `Filegroup`, `PackageGroup`,
  `TestSuite`, `ConfigSetting`, `Genrule`, `LabelFlag`,
  `StarlarkDocExtract`, `EnvironmentGroup`, `AnalysisTest`,
  `Genquery`, `XcodeConfig` — these remain native in Bazel 9.
- `app/kuro_analysis/src/analysis/native_rule_analysis.rs:152-170`:
  replace the cc_* branches with a new `NativeRuleKind::EmptyRuleStub`
  variant that carries the rule name and expected load label, then
  returns an analysis error matching Bazel's format string.
- `app/kuro_interpreter_for_build/src/interpreter/native_rules.rs`:
  register each removed rule as `EmptyRuleStub` with the load label
  metadata. Remove `create_cc_analysis_result` helpers (they become
  dead code).
- Rules_cc Starlark implementations: ensure kuro can evaluate
  `@rules_cc//cc:defs.bzl`'s Starlark `cc_library`. The extension
  machinery and `register_rule` path need to be exercised. Verify
  against `examples/multi_package` (which already uses cc_library;
  confirm kuro can load rules_cc's Starlark version).

### Parity source

- `BaseRuleClasses.java:419-495` — `EmptyRule` class with optional
  `bzlLoadLabel` and the error template
- `CcRules.java:48-61` — exact list and order
- `JavaRules.java:42-57` — same
- `ObjcRules.java:42-43` — same

### Success criteria

- `cc_library(...)` in a BUILD file without `load("@rules_cc//cc:defs.bzl",
  "cc_library")` produces the exact error message shape from Bazel 9:
  ```
  The cc_library rule has been removed, add the following to your BUILD/bzl file:
      load("@rules_cc//cc:defs.bzl", "cc_library")
  ```
- `cc_library(...)` with the correct load calls into rules_cc's
  Starlark implementation, which produces Bazel-9-compatible providers.
- `examples/multi_package` builds after adding the required `load()`
  statements at the top of its BUILD files.

### Risks

- Rules_cc's Starlark `cc_library` depends on cc_common API surface
  that may have gaps in kuro (toolchain resolution, compile/link
  action creation). Phase 2 cannot land without Phase 4-adjacent work
  on `cc_common`. **Preq-check**: verify `examples/multi_package` can
  build a trivial cc_library via Starlark rules_cc today; if it
  can't, Phase 2 depends on closing that gap first.
- Kuro's native cc_* had Bazel-incompatible behaviour (e.g., a
  `rust_library` output instead of `cc_library` output). Those builds
  will break. Expected and fine.

### Est. effort

1-2 weeks, depending on rules_cc readiness.

## Phase 3: Strict lockfile v24 parsing

### Overview

Remove the "using lockfile specs anyway" fallback
(`app/kuro_bzlmod/src/lockfile.rs:474-479`). On digest mismatch, force
extension re-execution and overwrite the lockfile entry. Matches
Bazel's `--lockfile_mode=update` default.

### Changes

- `app/kuro_bzlmod/src/lockfile.rs:445-509`: modify
  `get_extension_cache` to return `None` on digest mismatch, same as
  empty-specs.
- `app/kuro_bzlmod/src/extension_execution_dice.rs:406-438`: when
  cache returns `None`, execute extension and write updated entry to
  lockfile at end of compute.
- `compute_bzl_transitive_digest`
  (`extension_execution_dice.rs:703-717`) currently stubs to hash
  extension_id only; upgrade to real transitive .bzl hashing matching
  Bazel's `BzlTransitiveDigestUtil.getDigest()`:
  - Load the extension's parent .bzl via DICE
  - BFS the transitive import closure (cycle detection, dedup by path)
  - Read each .bzl file's content
  - Hash in deterministic path order (sort by `Label.toString()`)
  - Emit SRI-formatted `sha256-<base64>`
- `compute_extension_input_hash` already hashes tags correctly; verify
  against a small Bazel-generated lockfile fixture.

### Parity source

- `BzlTransitiveDigestUtil.java` in
  `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/`
- `SingleExtensionEvalFunction.java` for the compute-then-update
  pattern

### Success criteria

- Bazel-generated `MODULE.bazel.lock` (from a fresh Bazel 9 build) is
  read cleanly by kuro. Shared entries hit without digest mismatch.
- User-edited `.bzl` invalidates the right lockfile entries on the
  next kuro build.
- Kuro-generated lockfile entries are re-readable by Bazel 9 on the
  same workspace (round-trip).

### Risks

- Bazel's digest algorithm includes load-target labels as well as file
  content bytes. Missing either leads to silent divergence. Verify
  against a `bazel mod dump_repo_mapping` output before relying on
  digests.
- The llvm-project v24 lockfile uses mixed SRI/bare-base64 for
  different fields. Kuro must emit exactly the same format per field.

### Est. effort

3-5 days.

## Phase 4: Audit `@bazel_tools` content against upstream

### Overview

Replace kuro's `bazel_tools/` tree with content derived verbatim from
Bazel 9's `src/*/BUILD.tools` files. Eliminate divergences discovered
empirically (`src/conditions/` was patched in 2260f5f; others likely
exist).

### Changes

- Script: walk `/var/mnt/dev/bazel/src/` and `embedded_tools/` for
  files named `BUILD.tools` and `BUILD`. For each, check if kuro has
  the corresponding file. If not: copy. If present: diff and
  reconcile.
- High-priority subpackages (most commonly referenced from rules and
  workspaces):
  - `src/conditions/` (already done in 2260f5f)
  - `src/main/protobuf/` (protos)
  - `src/tools/`
  - `tools/build_defs/repo/` (http.bzl, git.bzl, etc.)
  - `tools/cpp/` (cc_toolchain definitions)
  - `tools/python/`
  - `tools/jdk/`
  - `tools/test/`
- Each port updates
  `app/kuro_external_cells_bundled/build.rs` (touch) and rebuilds.

### Parity source

`/var/mnt/dev/bazel/src/` and `/var/mnt/dev/bazel/embedded_tools/`.

### Success criteria

- A clean `find bazel_tools/ -name BUILD` produces the same set of
  files as `find /var/mnt/dev/bazel/src/ -name BUILD.tools` (modulo
  the `.tools` suffix).
- File contents match upstream byte-for-byte (or are explicitly
  annotated with a comment pointing to the upstream file and
  explaining the divergence).
- `kuro cquery @llvm-project//llvm:config` does not error with
  "package @bazel_tools//... does not exist" for any subpackage
  referenced by rules_cc / rules_java / platforms transitive closure.

### Risks

- Some Bazel `BUILD.tools` reference `@bazel_tools//...` targets that
  recursively depend on `@bazel_tools` subpackages not yet in kuro —
  bootstrap problem. Mitigate by porting in dependency order, starting
  from leaf packages.
- Rules in `tools/cpp/BUILD.tools` reference platform-specific
  toolchains (`@local_config_cc//...`). Those already work via kuro's
  extension machinery, but the BUILD.tools may need
  rules_cc-specific load paths that don't exist yet.

### Est. effort

1-2 weeks.

## Phase 5: Audit remaining Starlark API surface

### Overview

After Phases 1-4, run `kuro cquery` / `kuro build` against representative
Bazel workspaces and fix whatever Bazel-9 API gaps surface. This is
open-ended and iterative; budget accordingly.

Known candidate areas:
- `cc_common.*` method signatures (Bazel 9 renamed/removed many)
- `py_common` / `py_runtime_info`
- `coverage_common`
- `testing.*` (especially `testing.TestEnvironment` for
  ExternalRunnerTestInfo)
- `config.*` and `transition.*`
- Module extension API (`module_ctx.*`, `tag_class.*`,
  `extension_metadata.*`, etc.) — Bazel 9 added reproducible-repo
  tracking that kuro doesn't implement

### Deliverable

Running list of API gaps, with per-item parity sources and estimated
effort. Each gap becomes a sub-plan (15.5.1, 15.5.2, ...) when tackled.

## Dependencies and ordering

```
Phase 1 (globals) ─────────────┐
                               ├──► Phase 5 (API audit)
Phase 2 (EmptyRule) ◄───────┐  │
       depends on Phase 4    │  │
                             │  │
Phase 3 (lockfile) ──────────┼──┘
                             │
Phase 4 (bazel_tools) ───────┘
       depends on Phase 2? No — bazel_tools is data, Phase 2 is rule
       definitions; independent.
```

- Phase 1 unblocks Phase 2 (removing globals exposes which rules_cc
  load paths kuro needs to honour).
- Phase 2 depends on rules_cc's Starlark cc_library being evaluable —
  which requires a working `cc_common` API surface. That's partly in
  Phase 5.
- Phase 3 is orthogonal.
- Phase 4 is orthogonal but benefits Phase 2 (rules_cc's load paths
  reach into `@bazel_tools//tools/cpp/...`).

Recommended order: **Phase 3 → Phase 1 → Phase 4 → Phase 2 → Phase 5**.
Phase 3 first because it's self-contained and exposes real lockfile
behaviour. Phase 1 next to force all the load() fixes in kuro's own
.bzl content. Phase 4 before Phase 2 so rules_cc has what it needs.
Phase 2 once rules_cc is viable. Phase 5 is continuous thereafter.

## Open questions

- **rules_cc readiness**: can kuro build a trivial `cc_library` via
  `load("@rules_cc//cc:defs.bzl", "cc_library")` today? Answer
  determines whether Phase 2 is "fix-and-ship" or "blocked on API
  gaps".
- **Lockfile round-trip**: is there a reasonable test fixture (pair of
  small MODULE.bazel files, one that Bazel-generates the lock for, one
  that kuro-regenerates) to validate Phase 3 parity? If not, build
  one as part of Phase 3.
- **Phase 5 budget**: API-surface audits routinely balloon. Time-box
  to 2 weeks and surface remaining items as their own plans.

## Success criteria (plan-level)

- `.bazelversion`-9.0-pinned Bazel workspaces build unchanged under
  kuro.
- Kuro's error messages for removed symbols and rules match Bazel 9's
  exactly (compared mechanically).
- Lockfiles are round-trip-compatible between kuro and Bazel 9.
- `examples/*` in kuro are updated to use Bazel 9 load patterns. No
  workspace in the repo relies on removed globals.
