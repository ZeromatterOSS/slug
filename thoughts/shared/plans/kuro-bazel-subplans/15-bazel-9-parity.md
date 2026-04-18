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

### 15.5.1 CC toolchain `TemplateVariableInfo` propagation (PARTIAL)

**Status:** Minimal unblock landed in commit `fe3639f` (2026-04-17).
Seeded `STACK_FRAME_UNLIMITED=""` in `ctx.var` static entries so
`@llvm-project//llvm:llvm` analysis can evaluate rules_cc 0.2.17's
`_expand_make_variables_for_copts` without failing on
`$(STACK_FRAME_UNLIMITED) not defined`.

**Remaining work (real plumbing):**

Bazel's `ctx.var` is built by aggregating `TemplateVariableInfo`
providers from the resolved toolchains declared via
`rule(toolchains=[...])`. In Bazel, rules_cc's `cc_toolchain` rule
returns a `TemplateVariableInfo` populated from
`cc_toolchain._additional_make_variables |
cc_helper.get_toolchain_global_make_variables(cc_toolchain) |
cc_helper.get_cc_flags_make_variable(...)` (see
`@rules_cc//cc/private/rules_impl/cc_toolchain.bzl:135`).

Kuro's `cc_toolchain` is a native stub
(`app/kuro_analysis/src/analysis/native_rule_analysis.rs:166` —
`NativeRuleKind::CcToolchain => create_minimal_analysis_result(target)`)
that returns only `DefaultInfo`. Defaults that Bazel seeds via
`cc_toolchain_provider_helper.bzl::_additional_make_variables` are
therefore not published, and `ctx.var` falls back to kuro's hardcoded
builtin list in `app/kuro_build_api/src/interpreter/rule_defs/context.rs`
(two active sites: the `ctx.var` attribute and
`expand_make_variables`).

Target:
1. Make `ctx.var` (attribute + `expand_make_variables`) gather
   `TemplateVariableInfo` from `resolved_toolchains_for_ctx`
   (already computed in `env.rs:859-950`) and merge the variables
   dict into the returned `Dict`.
2. Either (a) implement the native `cc_toolchain` rule analysis to
   return a real `TemplateVariableInfo`, or (b) let rules_cc's
   Starlark `cc_toolchain` run (requires promoting the native stub
   to `EmptyRule` per Phase 2 and providing full `cc_common` surface).

**Parity sources:**
- `@rules_cc//cc/private/rules_impl/cc_toolchain.bzl:135`
  (`template_vars = cc_toolchain._additional_make_variables | ...`)
- `@rules_cc//cc/private/rules_impl/cc_toolchain_provider_helper.bzl:65-75`
  (`_additional_make_variables` — seeds `STACK_FRAME_UNLIMITED=""`)
- `@rules_cc//cc/common/cc_helper.bzl:583` (`_lookup_var` — order:
  `additional_vars` first, then `ctx.var`)
- Bazel CcToolchain impl: `src/main/java/com/google/devtools/build/lib/rules/cpp/`

**Est. effort:** 2-3 days for (1) + (2a); 1-2 weeks for (2b).

### 15.5.2 Extension-repo action-path resolution (LANDED)

**Status:** Fully landed in commits `bfe28b4` and `325e06a`
(2026-04-17). Two gaps:

1. **Path bridging**: `BuckOutPathResolver::resolve_external_cell_source`
   in `app/kuro_core/src/fs/buck_out_path.rs:346-348` places action
   command lines for `ExternalCellOrigin::ExtensionRepo` cells at
   `buck-out/v2/external_cells/extension_repo/{canonical_name}/...`,
   but materialization writes content to
   `bazel-external/{canonical_name}/`. No symlink was being created.
   Bzlmod cells already did this in
   `app/kuro_common/src/legacy_configs/cells.rs:822`. Parity fix:
   `app/kuro_external_cells/src/extension_repo.rs::ensure_buck_out_extension_repo_symlink`.

2. **Symlink-aware directory listing**: `ExtensionRepoFileOpsDelegate::read_dir`
   classified entries via `DirEntry::file_type()`, which does not
   follow symlinks. `repository_ctx.symlink(src_dir, dst_dir)` (used
   by `rules_cc`'s `llvm_configure` to overlay whole subtrees) left
   entries typed `FileType::Symlink`. `gather_package_listing_impl`
   in `app/kuro_common/src/package_listing/interpreter.rs:414` only
   recurses into `FileType::Directory`, so `glob(["lib/Support/*.c"])`
   returned an empty list. Fix: stat each entry (`tokio::fs::metadata`)
   to classify by the resolved target's type, falling back to the
   symlink's own metadata only if the target is broken.

**Related latent bug (not yet fixed):** `IoFileOpsDelegate` in
`app/kuro_common/src/io/fs.rs:149` has the same behaviour — symlinks
in a regular cell's source tree classify as `Symlink` and don't
recurse. No real workspace hits it yet (normal repos don't symlink
whole subdirs), but Bazel's glob follows symlinks by default, so
kuro diverges. File as its own sub-plan if a workspace surfaces it.

### 15.5.3 Implicit DefaultInfo from `attr.output` declarations (LANDED)

**Status:** Landed in commit `54b3be8` (2026-04-18).

Bazel convention: a rule with `attr.output` / `attr.output_list`
attributes that does not return DefaultInfo gets an implicit
DefaultInfo whose `default_outputs` are the declared artifacts. e.g.
bazel_skylib's `expand_template.bzl`:

```python
def _expand_template_impl(ctx):
    ctx.actions.expand_template(
        template = ctx.file.template,
        output = ctx.outputs.out,
        substitutions = ctx.attr.substitutions,
    )
```

No return statement. Under kuro's previous behaviour,
`ProviderCollection::try_from_value_subtarget` auto-injected an
**empty** DefaultInfo, so `kuro build :abi_breaking_h_gen` reported
"does not have any outputs", and cc_library targets that consumed the
output via `hdrs = [":abi_breaking_h_gen"]` saw nothing to depend on.

**Fix:**
- Thread `output_attr_names()` through the `RuleSpec` trait (extracted
  from `FrozenStarlarkRuleCallable`'s existing `output_attr_names`
  field).
- New `AnalysisContext::collect_implicit_default_outputs` method
  materialises the `CtxOutputs` wrapper if the impl never accessed
  `ctx.outputs`, then calls `get_attr(name)` for each declared output
  attr. Must run before `take_state` because declaration goes through
  the live `AnalysisRegistry`.
- New `DefaultInfo::with_default_outputs` constructor.
- `maybe_inject_implicit_default_info` in `run_analysis` appends the
  implicit DefaultInfo to the rule's return value unless the user
  already returned one (user-supplied DefaultInfo wins even if its
  `default_outputs` is empty — explicit > implicit).

**Parity source:** `@bazel_skylib//rules:expand_template.bzl` and
Bazel's `RuleClass.computeImplicitOutputs` (implicit from attr.output).

### 15.5.4 `attr.output` generated-file path layout divergence (LANDED)

**Status:** Landed option A in commit `4c1923a` (2026-04-18).

After the DefaultInfo auto-inject lands, `kuro build @llvm-project//llvm:abi_breaking_h_gen`
succeeds and produces the header at:

```
buck-out/v2/gen/llvm-project/9b5202f249973417/llvm/__abi_breaking_h_gen__/include/llvm/Config/abi-breaking.h
```

but `@llvm-project//llvm:Support` compile commands include
`-Ibuck-out/v2/gen/llvm-project/9b5202f249973417/external/llvm-project/llvm/include`
and fail with `fatal error: llvm/Config/abi-breaking.h: No such file or directory`.

Two path deltas between kuro output and the cc_library include path:

1. **Extra `__<target>__/` segment.** kuro's `BaseDeferredKey::make_hashed_path`
   in `app/kuro_core/src/deferred/base_deferred_key.rs:148` always appends
   `__<escaped_target_name>__/` between `<pkg>/` and the output path.
   Bazel's `attr.output`-declared files go at `bazel-bin/<pkg>/<out_path>`
   with no target wrapper.
2. **`external/<canonical>/` prefix missing.** For external-cell
   targets, Bazel's bin-dir layout is
   `bazel-bin/external/<canonical_name>/<pkg>/<out_path>`. kuro's buck-out
   layout is `buck-out/v2/gen/<cell_name>/<cfg>/<pkg>/...`, without an
   `external/` prefix.

Fixing either half in isolation is insufficient — cc_library's include
search path is Bazel-shaped (`<bin_dir>/external/<canonical>/<pkg>/<include_dir>`)
so both transformations need to apply for declared-output `hdrs`.

**Options to evaluate:**
- (A) Make `attr.output`/`attr.output_list` declarations use a
  Bazel-style path resolver that omits `__<target>__/` and prefixes
  `external/<canonical>/` for external-cell targets. Would require
  distinguishing "Bazel-style attr.output" from Buck2-style
  `declare_output` without collision risk when two targets in the same
  package declare outputs with the same name.
- (B) After action execution, symlink from the Bazel-layout path to
  the actual kuro buck-out path. Cheap, but introduces two truth
  sources and a clean/rebuild coherence concern.
- (C) Change cc_library's include-dir computation to point at the
  kuro-layout path (`<bin_dir>/<cell>/<pkg>/__<target>__/<include_dir>`).
  Breaks if the same cc_library consumes multiple generated hdrs
  from different targets sharing an include dir.

Option (A) is closest to Bazel semantics but the most invasive. (B) is
a pragmatic unblock; a follow-up can migrate to (A) once the rest of
Plan 15 shakes out.

**Parity sources:**
- `src/main/java/com/google/devtools/build/lib/actions/ArtifactFactory.java`
  — declared-file path computation (bazel-bin + package path)
- `src/main/java/com/google/devtools/build/lib/rules/cpp/CcCompilationContext.java`
  — include-dir aggregation for cc_library deps

**Landed implementation (option A):**

- `BuckOutPathKind::BazelOutput` variant in `app/kuro_core/src/fs/buck_out_path.rs`.
- `BaseDeferredKey::make_hashed_path` in
  `app/kuro_core/src/deferred/base_deferred_key.rs` handles the new
  variant with a dedicated assembly that omits the `__<target>__/`
  segment and prefixes `external/<cell>/` for non-root cells.
- `ArtifactPath::with_full_path` in
  `app/kuro_execute/src/path/artifact_path.rs` mirrors the new layout
  in command-line path strings so compile actions can find the
  materialized files.
- `AspectDeferredKey::make_hashed_path` falls through to the existing
  Configuration layout (aspects don't declare `attr.output` outputs
  currently; revisit if that changes).
- `CtxOutputs::declare_file` in
  `app/kuro_build_api/src/interpreter/rule_defs/context.rs` passes the
  new kind for `attr.output` / `attr.output_list` declarations.

Other output-declaring callers (`ctx.actions.declare_output`,
`ctx.actions.write`, etc.) keep the default Configuration layout —
intermediate outputs that don't need to match Bazel's bin-dir shape.

### 15.5.5 Shell-quoting for string-define values (LANDED)

**Status:** Landed in commit `53e04f4` (2026-04-18). Root cause was
not shell-escaping but Starlark raw-string lexer divergence from
Python/Bazel spec.

Python's r-string spec: inside `r"..."`, a backslash followed by a
matching quote does not end the string, but the backslash **remains**
in the result. `r"\""` is the two-char string `\"`.

Kuro's lexer (`starlark-rust/starlark_syntax/src/lexer.rs:393-403`)
dropped the backslash for that case. So
`r'LLVM_VERSION_STRING=\"23.0.0git\"'` produced
`LLVM_VERSION_STRING="23.0.0git"` in Starlark memory (with literal
quotes) instead of `LLVM_VERSION_STRING=\"23.0.0git\"` (with literal
backslashes). rules_cc's `_tokenize` in cc_helper.bzl then stripped
the literal quotes during shell-style tokenisation, yielding
`LLVM_VERSION_STRING=23.0.0git` (no quotes at all) — a malformed
numeric token during `#define PACKAGE_VERSION LLVM_VERSION_STRING`
expansion.

Fix: preserve the backslash in the raw-string escape path. Tests
updated in `lexer_tests::test_string_lit` and the f_string golden.

**Parity source:** Python Language Reference §2.4.1.1 "String and
Bytes literals". Bazel's Starlark follows Python raw-string
semantics.

### 15.5.6 `strip_include_prefix` for cc_library (LANDED)

**Status:** Landed in commit `0defacf` (2026-04-18).

cc_library's `strip_include_prefix = "include"` was ignored in both
paths:

- Native stub (`app/kuro_analysis/src/analysis/native_rule_analysis.rs`)
  didn't look at the attribute.
- `cc_common.compile` handler derived the include dir from `srcs`
  (broken for hdrs-only libraries) and used the wrong path formula
  (`external/<cell>/<strip_prefix>` instead of
  `external/<cell>/<pkg>/<strip_prefix>`).

Both fixed. The `cc_common.compile` path also appends the derived
include dir to the returned `CcCompilationContext.includes` so
dependents pick it up through normal provider propagation, not only
via the in-session `register_external_include_dir` global.

**Parity source:** `src/main/java/com/google/devtools/build/lib/rules/cpp/CcCompilationHelper.java`
— `stripIncludePrefix` + `CcCompilationContext.headerInfo.headers`.

### 15.5.7 `Label("//:...")` in repo rule inserts spurious `_main/` (LANDED)

**Status:** Landed in commit `ac28913` (2026-04-18). Fixed cell-name
extraction from bazel-external directory names. Module-extension
canonical layouts of the form `{owner}+{extension}+{repo_name}` (e.g.
`_main+llvm_repos_extension+llvm-raw`) were being stripped at the
first `+`, yielding `_main` instead of the apparent `llvm-raw`.

Three cases now handled in
`app/kuro_interpreter_for_build/src/interpreter/natives.rs::extract_cell_and_package_from_filename`:
- 0 `+` segments: plain name (e.g. `llvm-project`)
- 1 segment: bzlmod module cell `{name}+{version}` → first segment
- 2+ segments: canonical extension repo → last segment

Verified via `rm -rf bazel-external && kuro build @llvm-project//llvm:Support`:
llvm_configure re-materializes correctly; its `Label("//:...")` now
resolves to the correct cell.

`Label("//:llvm/CMakeLists.txt")` evaluated inside
`llvm_configure._llvm_configure_impl` (running in the
`@llvm-project` extension repo) resolves to:

```
bazel-external/llvm-project/_main/llvm/CMakeLists.txt
```

The extra `_main/` segment is the bzlmod canonical name of the root
workspace; it shouldn't appear in the resolved path. `_main` is the
root-module canonical name in bzlmod — so the Label is being stamped
with the wrong "current repo" when evaluated inside a repo rule.
Fixing this likely requires the repo_ctx machinery to supply its own
repo as the Label's implicit repo instead of falling back to `_main`.

**Location to investigate:**
`app/kuro_interpreter_for_build/src/repository_ctx.rs::resolve_label_to_path`
(handles string-label path arguments) and its Label-object sibling.
The Label's repo attribute at construction time is what matters —
check how `Label(...)` inside a repo rule's Starlark sets the repo
context.

**Impact:** Blocks any `kuro clean` + rebuild scenario for
llvm-project. Does NOT block the already-materialized scenario
(session state observed earlier had a working repo on disk from a
prior successful materialisation).

**Parity source:** Bazel 9's
`src/main/java/com/google/devtools/build/lib/bazel/repository/starlark/StarlarkRepositoryModule.java`
— how Label() resolves inside `repository_ctx` callbacks.

### 15.5.8 Transitive hdrs not threaded into cc compile action inputs (LANDED)

**Status:** Landed in commit `5f64d82` (2026-04-18).

`cc_common.compile` invoked `actions.run()` with only (args, outputs,
category, identifier, progress) — no `inputs=` kwarg. Generated hdrs
referenced via `-I` flags (e.g., `abi-breaking.h` from
`expand_template`) were not declared as action inputs, so kuro's
scheduler didn't build them before the consuming compile ran. The
`-I` path pointed at a location that didn't exist yet.

Fix: collect hdr artifacts from `public_hdrs` / `private_hdrs` /
`textual_hdrs` and from each dep `CcCompilationContext.headers`
depset. Pass the list as `inputs=` to both PIC and non-PIC
`actions.run()` calls. `inputs` is handled by `actions.run`'s
`bazel_inputs` pathway (tracks as dependency, runs dep actions first).

Effect on `@llvm-project//llvm:Support`: command count jumps from 43
(just the compile actions that used to fail) to 83 (dep actions +
compile actions) — generated hdrs materialize before their consumers
compile.

### 15.5.9 rules_cc `cc_common.compile` is Starlark, not kuro's native (IN PROGRESS)

**Status:** Partially landed. rules_cc's Starlark `cc_common.compile`
now produces virtual-include symlinks at correct paths and Support's
compile command receives the right `-I<virtual-includes-dir>` flag,
but the symlink action fails to run before the consuming compile.

**Path chain**: `@rules_cc//cc/common:cc_common.bzl` →
`@cc_compatibility_proxy//:symbols.bzl` →
`@rules_cc//cc/private:cc_common.bzl` → Starlark `_compile` →
`compile.bzl::compile` → `cc_compilation_helper.bzl::_compute_public_headers`
→ `actions.declare_shareable_artifact` + `actions.symlink`.

Earlier session's claim "never reaches `cc_common.compile`" was wrong.
Placing `fail("KURO_TRACE ...")` verifies _cc_library_impl enters
compile() and returns a compilation_context with a populated
`virtual_include_path` and two headers (the virtual symlink + the
source). `declare_shareable_artifact` is called with
`external/llvm-project/third-party/siphash/_virtual_includes/siphash/siphash/SipHash.h`.

**Landed this session:**

1. **`BuckOutPathKind::Shareable` variant**
   (`app/kuro_core/src/fs/buck_out_path.rs`) — resolves to
   `buck-out/v2/gen/<cell>/<cfg_hash>/<filename>` with no package,
   `__<target>__/`, or duplicated `external/<cell>/` prefix. The
   `filename` passed to `declare_shareable_artifact` is already
   bin-dir-relative (rules_cc joins its own `external/<cell>/<pkg>/...`).
   Handled in `BaseDeferredKey::make_hashed_path`
   (`app/kuro_core/src/deferred/base_deferred_key.rs`) and mirrored in
   `ArtifactPath::with_full_path`
   (`app/kuro_execute/src/path/artifact_path.rs`). Aspect and
   anon-target matches fall back to Configuration semantics; BXL key
   matching widened to treat Shareable as Configuration-like.

2. **`declare_shareable_artifact` uses `Shareable`**
   (`app/kuro_action_impl/src/context/unsorted.rs:229, 257`). Removed
   a duplicate definition later in the same starlark_module that was
   shadowing the Shareable-using version with Configuration semantics.

3. **`ctx.bin_dir.path` returns real per-target bin_dir**
   (`app/kuro_build_api/src/interpreter/rule_defs/cc_common.rs`:
   `CtxCheatWithActions` now stores `cfg_hash`; `actions2ctx_cheat`
   populates it from the owner's `ConfiguredTargetLabel.cfg().output_hash()`).
   Prior stub returned `bazel-out/k8-fastbuild/bin`, so
   `paths.join(bin_dir, virtual_include_dir)` in rules_cc produced a
   nonexistent path.

4. **Removed `_virtual_includes` filter in
   `create_cc_compile_action`** (`cc_common.rs:1669-1671`). Was
   actively rejecting the virtual-includes `-I` flag from the compile
   command.

5. **Threaded transitive headers into `create_cc_compile_action`
   inputs** (`cc_common.rs` near line 1849). Previously only the
   kuro-native `fn compile` (line ~3108, added in 5f64d82) threaded
   `cc_compilation_context.headers` into `actions.run(inputs=...)`;
   the Starlark rules_cc path goes through `create_cc_compile_action`
   which still passed no inputs.

**Remaining blocker (15.5.10 candidate):** The symlink action is
registered via `copy_file_impl` (`app/kuro_action_impl/src/context/copy.rs:76`)
and the compile receives the virtual_header in its `inputs=` kwarg
(visible in `collected_bazel_inputs` / `StarlarkRunActionValues.bazel_inputs`),
yet kuro runs the compile without waiting for the symlink. Empty
virtual-includes directory is created, then the compile fails with
`fatal error: siphash/SipHash.h: No such file or directory`.

Hypothesis: `run.rs::run` adds the artifact to `artifacts.inputs`
(line 754) AND stores it in `StarlarkRunActionValues.bazel_inputs`
for `visit_artifacts` (line 478 of actions/impls/run.rs). The
`visit_artifacts` path should establish a DICE edge, but something
in the path from bazel_input → artifact_group → action-dependency is
not producing a must-materialize edge. Worth comparing against how
the kuro-native `fn compile`'s 5f64d82 threading actually works for
abi-breaking.h — same mechanism, works there.

**Parity source:** `@rules_cc//cc/private/compile:compile.bzl` +
`@rules_cc//cc/private/compile:cc_compilation_helper.bzl::_compute_public_headers`
— how Bazel's Starlark implementation generates virtual-includes
symlinks and propagates the virtual_include_dir through CcInfo.

`@llvm-project//llvm:Support` compiles 25 files, then fails on
`config.h`'s `PACKAGE_VERSION` expansion:

```
<command-line>: error: too many decimal points in number
external/llvm-project/llvm/include/llvm/Config/config.h:280:25: note:
    in expansion of macro 'LLVM_VERSION_STRING'
  280 | #define PACKAGE_VERSION LLVM_VERSION_STRING
```

BUILD declares `defines = [r'LLVM_VERSION_STRING=\"{}\"'.format(PACKAGE_VERSION)]`
— the escaped-double-quote is intended to produce a string literal
define. Bazel emits `-DLLVM_VERSION_STRING="23.0.0git"` with literal
double quotes. kuro emits `-DLLVM_VERSION_STRING=23.0.0git` (quotes
stripped), so preprocessor expansion turns `23.0.0git` into a malformed
numeric token.

Root cause: raw-string lexer in starlark-rust drops the backslash in
`\"` / `\'` escapes (see 15.5.5 notes).

### 15.5.10 Symlink action deps not scheduled before consuming compile (OPEN)

**Status:** Open. Blocks `@llvm-project//llvm:Support` even after
15.5.9's infrastructure landed. Artifacts flow correctly; scheduling
does not.

**Observed state after 15.5.9:**
- `declare_shareable_artifact` produces a `BuildArtifactPath` with
  `BuckOutPathKind::Shareable`. Its `.path` attribute resolves to
  `buck-out/v2/gen/<cell>/<cfg_hash>/<filename>` as intended.
- `actions.symlink(output=virtual_header, target_file=src)` registers
  a `CopyMode::Symlink` action via
  `copy_file_impl` (`app/kuro_action_impl/src/context/copy.rs:76`).
- Support's `create_cc_compile_action` gets the virtual_header in the
  merged `cc_compilation_context.headers` depset, iterates it into
  `compile_inputs`, and passes it as `inputs=` to `actions.run`.
- `run.rs::run` inserts the artifact into `artifacts.inputs` (line
  754) AND stores it in `StarlarkRunActionValues.bazel_inputs` (line
  873) so `visit_artifacts` sees it (actions/impls/run.rs:478).
- Compile command at build time contains the correct
  `-Ibuck-out/v2/gen/<cell>/<cfg_hash>/external/<cell>/<pkg>/_virtual_includes/<name>`
  include flag.

**Symptom:** kuro executes the compile immediately without
materializing the symlink first. Filesystem shows the parent dir
scaffolding (`.../_virtual_includes/siphash/siphash/`) but no
`SipHash.h` symlink inside. Compile fails with
`fatal error: siphash/SipHash.h: No such file or directory`.

**Working comparison point:** `5f64d82` threaded identical inputs=
logic into kuro's **native** `fn compile` (cc_common.rs:3311) and
solved the abi-breaking.h case. The mechanism works there. So the
gap is in kuro's `create_cc_compile_action` + Starlark `actions.run`
plumbing, not in the inputs= machinery itself.

**Investigation angles:**
- Compare action-inputs wiring between kuro's native `fn compile`
  (working, 5f64d82) and rules_cc's Starlark path that ends in
  `_cc_internal.create_cc_compile_action` → kuro's native
  `create_cc_compile_action` (broken, this section).
- Verify `visit_artifacts` in `actions/impls/run.rs:478` actually
  fires for `bazel_inputs` entries and that the ArtifactGroup it
  produces participates in DICE dep-edge construction.
- Maybe `collected_bazel_inputs` are consumed twice — once as
  `artifacts.inputs.insert(ArtifactGroup::Artifact(…))` (run.rs:754)
  and once as `StarlarkRunActionValues.bazel_inputs` — but the former
  path doesn't make it into the action's ExecutionDeps, and the
  latter is only a `visit_artifacts` hook that might be bypassed.
- Check `register_action`'s handling of `artifacts.inputs` vs
  `artifacts.declared_outputs` in
  `app/kuro_build_api/src/actions/registry.rs:213`.

**Diagnostic that confirmed scope:**
- `tracing::error!` traces on `declare_shareable_artifact`,
  `copy_file_impl`, and `create_cc_compile_action` verified the
  symlink action IS registered and the virtual_header IS in the
  compile's compile_inputs list with the Shareable-layout path.
- `find buck-out/v2/gen/…/_virtual_includes/siphash/siphash/` shows
  empty dir — scaffolding created, but the symlink action never ran.

**Parity source:** Buck2's `app/buck2_build_api/src/actions/registry.rs`
— how registered actions get scheduled when an output artifact
appears in another action's inputs via `visit_artifacts`.

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
