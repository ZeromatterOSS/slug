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

## Latest Slice 2026-05-10: `ctx.attr` Source-File Targets

The Plan 56 NativeShim depset-hashability slice cleared the
`rules_rust+0.69.0/rust/private/rustc.bzl:1374 deps = depset(deps)` blocker.
The next bounded SDK smoke advanced to:

```text
rules_rust+0.69.0/rust/private/rust.bzl:220
compile_data_targets = depset(ctx.attr.compile_data)
error: cannot add an item of type 'File' to a depset of 'Target'
```

This is a systemic Bazel label-attribute boundary, not a rules_rust special
case. Bazel 9 docs define `attr.label_list` as a dependency attribute whose
`ctx.attr` value is a list of `Target`s, and define `allow_files` as allowing
`File` targets. Bazel 9 `ctx.files` is the projection
`[f for t in ctx.attr.<ATTR> for f in t.files]`, while `ctx.file` is the
single-file projection for `allow_single_file` label attrs. Therefore Kuro must
not place raw `File` values in `ctx.attr` for Bazel `attr.label` /
`attr.label_list` source-file entries. It should expose target-like source-file
values with `DefaultInfo.files`, while preserving raw file values for Buck-style
`attrs.source()` attributes.

Parity references:

- Bazel 9.0 `attr.label_list`: `ctx.attr` is a list of `Target`s and
  `allow_files` allows `File` targets.
- Bazel 9.0 `ctx.files`: a projection from `ctx.attr.<ATTR>` targets to files.
- Bazel 9.0 `ctx.file`: the single-file projection for `allow_single_file`.
- Bazel 9.0 label syntax: file target names are package-relative paths and
  target-name punctuation includes characters such as `(` and `)`.

Follow-up smoke after this source-file target change advanced past
`rules_rust+0.69.0/rust/private/rust.bzl:220` and failed while creating a
source-file target label for:

```text
src/output_tests/expected/into_bytes_enum.repr(C).expected.rs
```

Kuro's target-name validator still used an older, narrower punctuation set and
rejected the parentheses. The systemic fix is to align `TargetName` validation
with Bazel 9's target-name punctuation set so source file labels and rule target
names share the same label grammar.

## Latest Slice 2026-05-09: cc_common LinkerInput depset eligibility

The rules_cc `link_extra_lib` blocker from `/tmp/plan15-cc-list-hash-1.log`
was another instance of cc_common value objects carrying Bazel-immutable data
through Kuro live Starlark list values:

```text
contexts_to_merge.append(cc_common.create_linking_context(linker_inputs = depset([linker_input])))
error: depset elements must not be mutable values
```

Kuro still validates direct depset elements with `Value::get_hashed()` and
still rejects raw mutable lists/dicts. The narrow fix is on cc_common value
objects: `CompilationOutputs`, `LibraryToLink`, and `LinkerInput` hash their
cc_common-owned list/tuple fields by content, so those provider values are
depset-eligible without making arbitrary mutable lists depset-eligible.
`_cc_internal.freeze` continues to return Starlark lists for list/tuple inputs
because rules_cc relies on Bazel's list type and `list + list` behavior.

Do not revive the unsafe frozen-heap copying experiment. It attempted to force
live heap values through Starlark `Freezer` from analysis-time code and caused
`assertion failed: !self.is_forward()` panics. The parity-compatible direction
is explicit immutable/hashable cc_common value representation, not globally
relaxing depset validation or copying live values into a frozen heap.

Focused coverage added/updated in
`app/kuro_build_api_tests/src/interpreter/rule_defs/cc_common.rs`:

- `_cc_internal.freeze` preserves list type and `+` behavior for list, tuple,
  depset-derived values, and `CompilationOutputs`.
- `cc_common.create_linker_input` accepts a `Label` owner, library depset,
  nested user link flag lists, and additional-input depset, then allows
  `depset([linker_input])` and `create_linking_context(linker_inputs = ...)`.
- Existing depset tests continue to reject raw mutable list/dict elements.

## Previous Frontier 2026-05-09: rules_cc `tuple + list` in C++ link action

The Plan 44/BazelOutput path slice cleared the glibc `select_file` blocker.
`ctx.actions.declare_file` and `declare_directory` now use Bazel-shaped
generated output paths without the `__<target>__/` segment, including external
repo package paths. Focused verification:

- `pytest -q tests/core/analysis/test_ctx_actions.py::test_actions_declare_file_bazel_path_shape tests/core/analysis/test_ctx_actions.py::test_actions_declare_file_external_bazel_path_shape tests/core/analysis/test_ctx_actions.py::test_actions_declare_directory_bazel_path_shape tests/core/analysis/test_ctx_actions.py::test_actions_declare_directory_external_bazel_path_shape`
- `pytest -q tests/core/analysis/test_cmd_args.py::test_args_add_all_map_each_sequence_returns tests/core/analysis/test_cmd_args.py::test_args_add_joined_map_each_sequence_returns tests/core/analysis/test_cmd_args.py::test_args_depset_add_all_transforms tests/core/analysis/test_cmd_args.py::test_args_depset_add_joined_transforms`
- `cargo test -p kuro_build_api_tests --lib interpreter::rule_defs::cmd_args::tests::map_each_sequence_returns_expand_as_items`
- `cargo build -p kuro`
- `git diff --check`

Fresh bounded smoke from `/var/mnt/dev/zeromatter`:

```sh
bash -o pipefail -c 'timeout 220s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep '\''kurod\[zeromatter\].*plan44-bazel-output-path-1'\'' \
    -- \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan44-bazel-output-path-1 \
      build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan44-bazel-output-path-1.log'
```

Outcome: Kuro exited status `3` after 158s
(`memory_smoke_summary elapsed_s=158 peak_rss_kib=802136 final_rss_kib=644308`).
The previous `bazel_skylib+1.9.0/rules/select_file.bzl:36` failure for
`llvm+0.7.0//runtimes/glibc:libc.s` did not recur. The run advanced through
glibc shared library selection and failed later in rules_cc dynamic linking:

```text
bazel-external/rules_cc+0.2.17/cc/private/link/cpp_link_action.bzl:127
object_files = object_files + additional_object_files
error: Operation `+` not supported for types `tuple` and `list`
```

The concrete next blocker is systemic Bazel-compatible sequence `+` behavior
for Starlark tuples/lists in rules_cc, not a rules_cc target-name workaround.
There was also a side warning from `@rules_rs//rs:extensions.bzl%crate` about
missing root `Cargo.toml` metadata for `diplomat`; the terminal build failure
is the rules_cc tuple/list operation above.

## Previous Frontier 2026-05-09: glibc `select_file` cannot find `libc.s`

The rules_cc `cmd_args.add_all(map_each=...)` tuple-output blocker is cleared.
Kuro now expands list and tuple values returned by `map_each` for
`cmd_args.add_all`/`add_joined`, preserves scalar values as single command-line
items, skips `None`, and keeps the deferred depset streaming path gated on the
no-transform case.

Focused verification:

- `cargo fmt -- app/kuro_build_api/src/interpreter/rule_defs/cmd_args/typ.rs app/kuro_build_api_tests/src/interpreter/rule_defs/cmd_args/tests.rs`
- `cargo test -p kuro_build_api_tests map_each_sequence_returns_expand_as_items -- --nocapture`
- `pytest -q tests/core/analysis/test_cmd_args.py -k map_each`
- `cargo build -p kuro`
- `git diff --check`

Fresh bounded smoke from `/var/mnt/dev/zeromatter`:

```sh
bash -o pipefail -c 'timeout 220s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep '\''kurod\[zeromatter\].*plan15-map-each-seq-1'\'' \
    -- \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan15-map-each-seq-1 \
      build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan15-map-each-seq-1.log'
```

Outcome: Kuro exited status `3` after 190s
(`memory_smoke_summary elapsed_s=190 peak_rss_kib=891896 final_rss_kib=668632`).
The previous
`rules_cc+0.2.17/cc/private/rules_impl/cc_static_library.bzl:174`
`tuple (repr: ())` failure did not recur. The run advanced through the C++
toolchain/glibc path and failed in
`bazel_skylib+1.9.0/rules/select_file.bzl:36` while analyzing
`llvm+0.7.0//runtimes/glibc:libc.s`:

```text
fail("Can not find specified file in [%s]" % files_str)
error: fail: Can not find specified file in [
  buck-out/v2/gen/llvm+0.7.0/.../__generate_glibc_stubs__/build/c.s,
  buck-out/v2/gen/llvm+0.7.0/.../__generate_glibc_stubs__/build/dl.s,
  ...
  buck-out/v2/gen/llvm+0.7.0/.../__generate_glibc_stubs__/build/all.map]
```

The concrete next blocker is systemic file/path selection parity for generated
outputs consumed by `bazel_skylib` `select_file`, not a target-name workaround.
Investigate how Bazel represents the requested `libc.s` file from
`generate_glibc_stubs` relative to the listed generated outputs and fix Kuro's
artifact/path matching or output-group/default-output behavior accordingly.

## Previous Frontier 2026-05-09: rules_cc `cmd_args.add_all(map_each=...)` tuple output

The `with_cfg` provider-key blocker is cleared. A focused provider collection
regression now covers native provider keys:

- `cargo fmt -- app/kuro_build_api_tests/src/interpreter/rule_defs/provider/collection.rs`
- `cargo test -p kuro_build_api_tests provider_collection_contains_native_provider_keys -- --nocapture`
- `cargo test -p kuro_build_api_tests provider_collection_contains_methods_and_in_operator -- --nocapture`
- `cargo test -p kuro_build_api_tests test_schema_provider_missing_fields_are_absent -- --nocapture`
- `pytest -q tests/core/configurations/test_configuration_dep_uquery_correctness.py`
- `pytest -q tests/core/configurations/transition/test_attr.py`
- `cargo build -p kuro`
- `git diff --check`

Additional note: `pytest -q
tests/core/configurations/transition/test_select_in_transition_attr.py` ran 3/4
passing; the only failure was the expected-failure regex still looking for
`old: root//:iphone#...` while Kuro now prints `old: //:iphone#...`.

Fresh bounded smoke from `/var/mnt/dev/zeromatter`:

```sh
bash -o pipefail -c 'timeout 220s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep '\''kurod\[zeromatter\].*plan15-provider-callable-1'\'' \
    -- \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan15-provider-callable-1 \
      build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan15-provider-callable-1.log'
```

Outcome: Kuro exited status `3` after 184s
(`memory_smoke_summary elapsed_s=184 peak_rss_kib=902404 final_rss_kib=710988`).
The previous
`with_cfg/private/transitioning_alias.bzl:55 if provider in target` /
`AnalysisTestResultInfo ... got function` failure did not recur. The run
advanced through the glibc `with_cfg` alias and failed later under
`rules_cc+0.2.17/cc/private/rules_impl/cc_static_library.bzl:174`:

```text
args = actions.args().add_all(linker_inputs, map_each = map_each)
error: Expected `CellPath | CellRoot | File | Label | OutputArtifact |
ProjectRoot | ResolvedStringWithMacros | TaggedCommandLine | TargetLabel |
TransitiveSetArgsProjection | WriteJsonCliArgs | cmd_args | str | list |
RunInfo`, but got `tuple (repr: ())`
```

The concrete next blocker is systemic `cmd_args.add_all(map_each=...)` return
parity. In this rules_cc path `_linkopts_map_each(linker_input)` returns
`linker_input.user_link_flags`, which can be an empty tuple. Next owner should
teach Kuro's `cmd_args.add_all` map_each handling to accept Bazel-compatible
sequence outputs, especially tuples, instead of special-casing this target.

## Previous Frontier 2026-05-09: C++ toolchain `with_cfg` provider lookup

The Rust toolchain type label canonicalization blocker is cleared. Kuro now
normalizes Bzlmod module-version canonical repo names while indexing
`ctx.toolchains`, so the provider resolved under
`@@rules_rust//rust:toolchain_type` is found when rules_rust looks up
`@@rules_rust+0.69.0//rust:toolchain_type`.

Focused verification:

- `cargo fmt -- app/kuro_build_api/src/interpreter/rule_defs/context.rs`
- `cargo test -p kuro_build_api toolchain_type_lookup --lib`
- `cargo test -p kuro_analysis test_normalize_constraint_label --lib`
- `cargo build -p kuro`
- `git diff --check`

Fresh bounded smoke from `/var/mnt/dev/zeromatter`:

```sh
bash -o pipefail -c 'timeout 220s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep '\''kurod\[zeromatter\].*plan15-toolchain-label-canon-1'\'' \
    -- \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan15-toolchain-label-canon-1 \
      build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan15-toolchain-label-canon-1.log'
```

Outcome: Kuro exited status `3` after 164s
(`memory_smoke_summary elapsed_s=164 peak_rss_kib=785084 final_rss_kib=605488`).
The prior `Toolchain type '@@rules_rust+0.69.0//rust:toolchain_type' was not
resolved` error did not recur. The run advanced into the C++ toolchain path:

```text
Failed to analyze mandatory toolchain impl
'llvm+toolchain+llvm_toolchains//:linux_x86_64_cc_toolchain'
for toolchain type '@@bazel_tools//tools/cpp:toolchain_type'
```

The concrete next blocker is a provider collection indexing error under
`with_cfg.bzl+0.12.0/with_cfg/private/transitioning_alias.bzl:51` while
analyzing the glibc C++ toolchain dependency chain:

```text
ctx.attr.exports[0]
provider collection operation [] parameter type must be a provider type but not
and instance of provider (for example, `RunInfo` or user defined provider type),
got `int`
```

Next owner should fix provider collection `[]`/dependency indexing parity for
transitioning aliases and provider forwarding in the `with_cfg`/glibc toolchain
path. Avoid target-name special cases and preserve depset validation.

Previous frontier, now cleared:

```sh
bash -o pipefail -c 'timeout 180s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep '\''kurod\[zeromatter\].*plan54-configured-gather-probe-1'\'' \
    -- \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan54-configured-gather-probe-1 \
      build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan54-configured-gather-probe-1.log'
```

Outcome: Kuro exited status `3` after 158s
(`memory_smoke_summary elapsed_s=158 peak_rss_kib=653752 final_rss_kib=578572`).
The run reached `zeromatter//sdk:sdk_contents` analysis deps, completed configured
node/gather-deps for the SDK aggregation chain, reached
`rules_rust//ffi/rs:empty_allocator_libraries`, completed toolchain resolution,
and then failed in `ctx.toolchains`:

```text
Toolchain type '@@rules_rust+0.69.0//rust:toolchain_type' was not resolved.
Ensure the toolchain is registered via register_toolchains() and the rule declares it in toolchains=[...]
```

The instrumentation shows Kuro resolved/analyzed a provider for
`@@rules_rust//rust:toolchain_type`, while Starlark later looked up
`@@rules_rust+0.69.0//rust:toolchain_type` from
`Label("//rust:toolchain_type")` in
`rules_rust+0.69.0/rust/private/utils.bzl`. This was fixed by normalizing
Bzlmod module-version repo names in `ctx.toolchains` lookup; do not reintroduce
target-name special cases or weaken depset validation.

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

Detailed execution plan: [Plan 27: Native Language Rule
Removal](./27-native-language-rule-removal.md). Plan 27 expands this
phase to cover kuro's current `sh_*`, `execution_platform(s)`, and
`cc_libc_top_alias` remnants, and requires a Bazel 9 source audit
before deciding whether each symbol is an `EmptyRule`, absent, or
private/internal.

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

### Findings (2026-05-05) — `MODULE.bazel.lock` round-trip divergence

Comparing kuro's emitted lockfile against a fresh
`bazel 9.1.0 mod graph` run on `examples/multi_package` revealed
**three** different key-format conventions in play and
**over-locking** of internal extensions:

| Source | `moduleExtensions` key format | Example |
|---|---|---|
| Fresh bazel 9.1.0 | `@@<canonical>+//pkg:file%name` | `@@rules_python+//python/uv:uv.bzl%uv` |
| Stale committed kuro lock | `//pkg:file%name` (no `@`) | `//host:extension.bzl%host_platform` |
| Kuro (current) | `@<apparent>//pkg:file%name` | `@platforms//host:extension.bzl%host_platform` |

Source: `app/kuro_bzlmod/src/extensions.rs:209-226`
`canonical_extension_id()` rewrites `//pkg:file.bzl` →
`@<module>//pkg:file.bzl` for owner/consumer dedupe; that key then
flows through `extension_execution_dice.rs:776`
`lockfile.set_extension_cache(extension_id, …)` straight into the
lockfile.

**Over-locking:** kuro emits 12 entries for `multi_package`'s lock;
fresh bazel emits 4. The 8 kuro-only entries are extensions that bazel
treats as internal/version-bound and intentionally does not lock,
because their output is fully determined by the bazel/kuro version
(the implementation is bundled with the tool, not user-specified):

- `@platforms//host:extension.bzl%host_platform`
- `@rules_cc//cc:extensions.bzl%cc_configure_extension`
- `@rules_cc//cc:extensions.bzl%compatibility_proxy`
- `@bazel_features//private:extensions.bzl%version_extension`
- `@rules_java//java:extensions.bzl%toolchains`
- (plus a handful of similar transitively-invoked internal extensions)

Round-trip success criterion 3
(*"Kuro-generated lockfile entries are re-readable by Bazel 9 on the
same workspace"*) is currently broken on both axes.

### Changes

### Changes

- `app/kuro_bzlmod/src/lockfile.rs:445-509`: modify
  `get_extension_cache` to return `None` on digest mismatch, same as
  empty-specs.
- `app/kuro_bzlmod/src/extension_execution_dice.rs:406-438`: when
  cache returns `None`, execute extension and write updated entry to
  lockfile at end of compute.
- **Lockfile key format**: write keys as
  `@@<canonical>+//pkg:file.bzl%name` (canonical repo with `@@` prefix
  and `+` version-or-empty suffix), not the current
  `@<apparent>//pkg:file.bzl%name`. Affects both
  `set_extension_cache()` (write) and `get_extension_cache()` (read).
  Internal aggregation can keep its `@<apparent>` form; convert at the
  lockfile boundary only.
- **Skip internal/version-bound extensions** when writing the
  lockfile: maintain a small allow-skip set
  (`KURO_INTERNAL_EXTENSIONS`) keyed by canonical extension id. Initial
  members: `host_platform`, `cc_configure_extension`,
  `compatibility_proxy`, `version_extension`,
  `rules_java//java:extensions.bzl%toolchains`. These extensions ship
  their implementation with kuro; locking them is noise that
  guarantees bazel-round-trip mismatches.

  **Side-fix (2026-05-05):** the lockfile-key format change exposed a
  latent bug in `pending_repo_cells.rs:185` where the *raw*
  `usage.extension_bzl_file` (e.g. `//java:extensions.bzl`) was passed
  to `extract_owning_module` instead of the canonicalized `ext_id`
  built 12 lines above. With a bare `//` path the helper falls through
  to "no module prefix → `_main`", mis-attributing every transitive
  module's relative `use_extension(…)` to the root module. Fixed in
  the same commit; symlink targets now match bazel's
  `<owning_module>+<ext>+<repo>` exactly. The same fix also taught
  `extract_owning_module` to strip the bazel-canonical trailing `+` on
  the module segment (`@@<repo>+//…` → `<repo>`), since
  `pending_repo_cells.rs:330` re-derives canonical names from lockfile
  keys and would otherwise emit `<repo>++<ext>+<repo>` once the keys
  use the new format.

  **Blocker (2026-05-05):** a naive skip-list breaks
  `examples/multi_package` because kuro's spoke-materialization path
  reads back the lockfile entries for these very extensions to seed
  cell registration (`pending_repo_cells.rs::seed_from_lockfile`).
  Removing the entries causes `File not found:
  multi_package//bazel-external/_main+host_platform+host_platform`
  and similar at BUILD-evaluation time. Decoupling spoke registration
  from the lockfile is a prerequisite — likely a small refactor of the
  startup-time seed path so it sources its inputs from in-memory
  aggregation results rather than re-reading the persisted lockfile.
  The format-fix portion of this phase is independent and can land
  first; over-locking removal lands once spoke seeding is decoupled.
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

### 15.5.2 CC toolchain analysis dependency cycle (PARTIAL 2026-05-09)

Plan 51's 2026-05-09 `ctx.toolchains` await instrumentation narrowed the
zeromatter `//sdk:sdk_contents` low-RSS stall to C++ toolchain analysis, not
memory growth.

Evidence from `/tmp/plan51-toolchain-await-1-memory.log`:

- The stuck displayed frontier remains
  `rules_rust+0.69.0//ffi/rs:empty_allocator_libraries (...) -- running
  analysis [evaluate_rule]`.
- `empty_allocator_libraries` waits while preparing `ctx.toolchains` provider
  values. Its target-configuration edge waits on
  `rules_rust+rust+rust_linux_x86_64__x86_64-unknown-linux-gnu__stable_tools//:rust_toolchain`;
  that Rust toolchain then waits on
  `llvm+toolchain+llvm_toolchains//:linux_x86_64_cc_toolchain`.
- `analysis_key_start` appears for
  `llvm+toolchain+llvm_toolchains//:linux_x86_64_cc_toolchain` in both
  configurations, but neither key reaches `analysis_deps_ready` or
  `analysis_evaluate_rule_phase`.
- Several support/header targets that are likely dependencies of the LLVM
  `cc_toolchain` also wait on the same C++ toolchain provider, including
  `bazel_tools//tools/cpp:malloc`,
  `bazel_tools//tools/cpp:link_extra_lib`,
  `rules_cc+0.2.17//:link_extra_lib`,
  `glibc_headers_x86_64-linux-gnu.2.28//:gnu_libc_headers`, and
  `linux_kernel_headers_x86.4.19.325//:kernel_headers`.

Working hypothesis:

Kuro's eager `ctx.toolchains` provider construction plus minimal native
`cc_toolchain` handling creates a cycle:
`cc_toolchain` analysis needs deps such as C++ header/support `cc_library`
targets; those targets prepare `ctx.toolchains`; provider construction asks
DICE for the same configured `cc_toolchain` analysis result.

2026-05-09 refresh from `/tmp/plan68-label-tool-2.log`: after Plan 36's
`repository_ctx.execute([Label(...)])`/`use_repo_rule` dynamic-cell fix,
zeromatter `//sdk:sdk_contents` advanced past the previous `toml2json`
ENOENT and stale `crates__clap-4.5.60//:clap` zero-target failures. The
bounded smoke timed out at the same `rules_rust//ffi/rs:empty_allocator_libraries`
analysis wait, with the target stuck after `toolchain_resolution_start` and
before later rule implementation phases. Continue this blocker under the
toolchain-analysis cycle investigation rather than extension spoke
materialization.

Next work:

1. Compare Bazel's `cc_toolchain` analysis dependency shape for this LLVM
   toolchain. In particular, determine whether support deps are analyzed before
   the C++ toolchain provider is available, or whether native C++ toolchain
   analysis treats them specially.
2. Implement the Bazel-parity behavior in Kuro's C++ toolchain path. Likely
   candidates are real native `cc_toolchain` provider construction, lazy
   `ctx.toolchains` provider realization, or a cycle-safe treatment for C++
   toolchain implementation/support deps.
3. Re-run the Plan 51 zeromatter smoke after the C++ toolchain cycle is fixed.

2026-05-09 Plan 15 slice:

- Added active-analysis-key tracking and used it while constructing
  `ctx.toolchains` provider values. If the requested toolchain type is
  Bazel's C++ toolchain type and the selected implementation target is already
  in analysis, Kuro now returns a cycle-safe minimal C++ toolchain provider for
  that `ctx.toolchains` access instead of awaiting the same configured
  `cc_toolchain` key. Normal non-cyclic C++ toolchain provider construction
  still analyzes the selected toolchain implementation through DICE.
- Added the minimal provider surface needed by rules_cc support deps inside
  the cycle: `ToolchainInfo(cc=..., cc_provider_in_toolchain=True)` plus a
  direct `CcToolchainInfo` provider exposing empty file depsets, basic tool
  path strings, empty `CcInfo`, empty feature data, and C++ fragment defaults.
- Bounded zeromatter smoke:

  ```sh
  timeout 180s env KURO_MEMORY_CHECKPOINTS=1 \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan15-cc-toolchain-cycle-1 \
      build //sdk:sdk_contents \
    2>&1 | tee /tmp/plan15-cc-toolchain-cycle-1-memory.log
  ```

  The run no longer stalls at the previous low-RSS
  `llvm+toolchain+llvm_toolchains//:linux_x86_64_cc_toolchain` await
  frontier. The log records `active_cc_toolchain_synthetic` for
  `bazel_tools//tools/cpp:link_extra_lib` and
  `rules_cc+0.2.17//:link_extra_lib` in both observed configurations, and the
  build advances into rules_cc analysis.
- Plan 54's 2026-05-09 cc provider immutability slice fixed that remaining
  `LinkerInput` depset element failure. A small follow-on in this Plan 15
  area also aligned the synthetic `CcToolchainInfo` cycle-breaker callback
  method names with rules_cc's keyword-only API
  (`feature_configuration`, not `_feature_configuration`) for
  `needs_pic_for_dynamic_libraries`, `static_runtime_lib`, and
  `dynamic_runtime_lib`.
- Latest bounded zeromatter smoke:

  ```sh
  timeout 180s env KURO_MEMORY_CHECKPOINTS=1 \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan54-cc-provider-immutability-3 \
      build //sdk:sdk_contents \
    2>&1 | tee /tmp/plan54-cc-provider-immutability-3-memory.log
  ```

  The build no longer fails at `rules_cc+0.2.17//:link_extra_lib` with mutable
  depset elements and no longer fails on the synthetic C++ toolchain method
  signature. The later rules_cc `implementation_deps` gate has also been
  addressed in the 2026-05-09 Plan 54 follow-up: Bazel 9 defaults
  `--experimental_cc_implementation_deps` to true, and Kuro now preserves,
  parses, propagates, and exposes the flag through
  `ctx.fragments.cpp.experimental_cc_implementation_deps()`.

  Latest bounded smoke:

  ```sh
  timeout 180s env KURO_MEMORY_CHECKPOINTS=1 \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan54-cc-implementation-deps-1 \
      build //sdk:sdk_contents \
    2>&1 | tee /tmp/plan54-cc-implementation-deps-1.log
  ```

  That run advanced past the old `requires
  --experimental_cc_implementation_deps` failure and reached
  `zeromatter//sdk:sdk_contents` analysis. The next observed frontier is no
  longer C++ implementation deps: rules_rs's `@rules_rs//rs:extensions.bzl%crate`
  extension failed to execute `toml2json` from
  `rules_rs+override/rs/private/toml2json.bzl:6` with `No such file or
  directory`, then the bounded client timed out in the daemon wait loop.

  2026-05-09 Label execute follow-up:

  ```sh
  set -o pipefail
  timeout 180s env KURO_MEMORY_CHECKPOINTS=1 \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan55-label-execute-1 \
      build //sdk:sdk_contents \
    2>&1 | tee /tmp/plan55-label-execute-1.log
  ```

  The previous rules_rs `toml2json.bzl:6` `ctx.execute([Label(...), ...])`
  `No such file or directory` failure no longer appears. The run still
  timed out after reaching `zeromatter//sdk:sdk_contents` analysis; the
  latest visible frontier is the same low-activity daemon wait pattern
  after `aspect_tools_telemetry+telemetry+aspect_tools_telemetry_report`
  stubs and later package/interpreter work. Continue from the timeout
  rather than reopening the C++ implementation-deps or toml2json slices.

  2026-05-09 `repository_ctx.workspace_root` follow-up:

  ```sh
  set -o pipefail
  timeout 90s env KURO_MEMORY_CHECKPOINTS=1 \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan55-workspace-root-2 \
      build //sdk:sdk_contents \
    2>&1 | tee /tmp/plan55-workspace-root-2.log
  ```

  Kuro now passes the invocation workspace root to repository rules for
  `repository_ctx.workspace_root`, matching Bazel's
  `StarlarkRepositoryContext.getWorkspaceRoot()` behavior while keeping
  relative `repository_ctx.path("...")` resolution anchored in the generated
  repository directory. The old `toml2json` failure remains gone. The run
  reached later extension/repository-rule work and then failed with a daemon
  event-bus broken pipe after earlier rules_kotlin provider-field and Gazelle
  repo-rule failures. The next narrow parity blocker is provider field
  presence: rules_kotlin expects `hasattr(provider_instance,
  "strip_prefix_template")` to be false when that optional provider field was
  not supplied, but Kuro currently exposes it as present with value `None`.

  2026-05-09 provider-field presence follow-up:

  Bazel 9 parity was checked against
  `src/main/java/com/google/devtools/build/lib/packages/StarlarkInfoWithSchema.java`
  and `src/main/java/net/starlark/java/eval/Starlark.java`, plus a focused
  Bazel 9.1.0 repro. Missing optional schema provider fields are absent from
  `dir`, make `hasattr` false, and make `getattr(x, name, default)` return the
  fallback; explicitly supplied `None` remains present. Kuro now tracks provider
  field presence separately from the stored field value in
  `app/kuro_build_api/src/interpreter/rule_defs/provider/user.rs`, treats
  Bazel-style list/doc schema fields as optional in
  `app/kuro_build_api/src/interpreter/rule_defs/provider/callable.rs`, and has
  focused tests in
  `app/kuro_build_api_tests/src/interpreter/rule_defs/provider/tests.rs`.

  Verification passed:

  ```sh
  cargo fmt
  cargo test -p kuro_build_api_tests creates_providers -- --nocapture
  cargo test -p kuro_build_api_tests test_schema_provider_missing_fields_are_absent -- --nocapture
  cargo test -p kuro_build_api_tests test_runtime_constructor_error_on_missing_required -- --nocapture
  cargo test -p kuro_build_api_tests interpreter::rule_defs::provider::tests -- --nocapture
  cargo check -p kuro_build_api
  cargo build -p kuro
  git diff --check
  ```

  A broader provider-filter test still has the pre-existing unrelated
  `interpreter::rule_defs::provider::builtin::validation_spec::test_attributes`
  artifact-path golden mismatch.

  Bounded zeromatter smoke:

  ```sh
  set -o pipefail
  LOG=/tmp/plan56-provider-presence-1.log
  ISOLATION=plan56-provider-presence-1
  timeout 120s env KURO_MEMORY_CHECKPOINTS=1 \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir "$ISOLATION" \
      build //sdk:sdk_contents \
    2>&1 | tee "$LOG"
  ```

  The smoke timed out with exit 124, but the prior rules_kotlin
  `strip_prefix_template` provider-field failure is gone. The next visible
  frontier is waiting on
  `crates__github.com_ZeroMatter_diplomat.git_99406ff1//runtime` package file
  tree loading, with Gazelle `go_repository` cache/stub failures and non-host
  JDK download timeouts as concurrent later noise.

2026-05-09 follow-up slice:

- Investigated the apparent
  `crates__github.com_ZeroMatter_diplomat.git_99406ff1//runtime` package-file
  tree stall. The package itself was missing from a stubbed extension repo, but
  the actual stall was Kuro's missing-directory diagnostic path:
  `extended_ignore_error` scanned every registered cell looking for same-path
  suggestions. Metadata probes on extension cells call `get_file_ops_delegate`,
  which can lazily materialize unrelated repos, explaining the concurrent
  Gazelle and JDK fetch noise. Bazel 9 package loading diagnostics do not fetch
  unrelated repositories to generate "did you mean another cell?" suggestions.
- Fixed the diagnostic path in `app/kuro_common/src/file_ops/error.rs` by
  skipping external cells during cross-cell missing-path suggestion probes.
  Focused coverage:

  ```sh
  cargo test -p kuro_common missing_path_suggestion_probe_skips_external_cells -- --nocapture
  ```

- Rebuilt and reran a targeted repro:

  ```sh
  cd /var/mnt/dev/zeromatter
  timeout 30s /var/mnt/dev/kuro/target/debug/kuro \
    --isolation-dir plan57-diplomat-targeted-after \
    build @crates__github.com_ZeroMatter_diplomat.git_99406ff1//runtime:all
  ```

  It now fails quickly with the real missing-package error instead of waiting
  while materializing unrelated repos.
- Bounded zeromatter smoke:

  ```sh
  set -o pipefail
  LOG=/tmp/plan57-missing-dir-suggestion-1.log
  ISOLATION=plan57-missing-dir-suggestion-1
  timeout 120s env KURO_MEMORY_CHECKPOINTS=1 \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir "$ISOLATION" \
      build //sdk:sdk_contents \
    2>&1 | tee "$LOG"
  ```

  The smoke advanced past the prior diplomat package-file-tree wait and reached
  `zeromatter//sdk:sdk_contents` analysis. It failed later at
  `platforms+1.1.0//:BUILD` because `module_version()` is missing as a
  BUILD-file global:

  ```text
  error: Variable `module_version` not found
   --> bazel-external/platforms+1.1.0/BUILD:21:10
  ```

2026-05-09 BUILD module metadata globals follow-up:

- Bazel 9 parity source: `StarlarkNativeModule.BINDINGS_FOR_BUILD_FILES`
  adds every non-rule native-module method directly to BUILD-file globals, and
  `StarlarkGlobalsImpl.getFixedBuildFileToplevelsSharedWithNative()` returns
  that binding map. The `module_name()` / `module_version()` implementations
  in `StarlarkNativeModule` read `TargetDefinitionContext`'s associated module
  name/version. Kuro already exposed `native.module_name()` and
  `native.module_version()`, but omitted the direct BUILD global forms.
- Fixed the systemic gap in
  `app/kuro_interpreter_for_build/src/interpreter/functions/path.rs` by adding
  direct BUILD globals `module_name()` and `module_version()` backed by the same
  cell/module metadata used by `native.*`.
- Focused coverage in `tests/core/analysis/test_build_globals.py` now verifies:
  direct root BUILD globals (`root@1.2.3`), matching native-module values, and
  direct globals while evaluating a local external bzlmod module repo via
  `@dep`.
- Verification:

  ```sh
  cargo fmt
  cargo check -p kuro_interpreter_for_build
  cargo build -p kuro
  pytest tests/core/analysis/test_build_globals.py::test_module_metadata_direct_globals \
    tests/core/analysis/test_build_globals.py::test_module_metadata_native_globals \
    tests/core/analysis/test_build_globals.py::test_external_module_metadata_direct_globals -q
  git diff --check
  ```

- Bounded zeromatter smoke:

  ```sh
  cd /var/mnt/dev/zeromatter
  timeout 180s env KURO_MEMORY_CHECKPOINTS=1 \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan58-module-build-globals-1 \
      build //sdk:sdk_contents \
    2>&1 | tee /tmp/plan58-module-build-globals-1.log
  ```

  The smoke failed quickly with exit 3, but the previous
  `platforms+1.1.0//:BUILD` `module_version()` missing-symbol error is gone.
  The log shows `platforms+1.1.0` targets reaching analysis. The next observed
  blocker is a missing package in another generated crate repo:

  ```text
  package `crates__github.com_Aleph-Alpha_ts-rs.git_a6bbbd18//ts-rs:` does not exist
  dir `crates__github.com_Aleph-Alpha_ts-rs.git_a6bbbd18//ts-rs` does not exist.
  ```

2026-05-09 symbolic macro inherited-attrs follow-up:

- Plan 55 fixed Kuro's missing symbolic macro `inherit_attrs = <rule>` handling
  for omitted inherited attrs. The immediate ZeroMatter failure was
  `copy_to_resource_directory(...): Missing parameter target_triple`, where
  `target_triple` came from an inherited `attr.string()` on the backing rule.
  Kuro now stores `inherit_attrs`, injects omitted inherited rule attrs into
  the macro implementation call, and preserves `StarlarkAttribute` implicit
  default metadata so omitted inherited `attr.string()` becomes `None` for the
  macro implementation while normal rule attr coercion still sees the existing
  coerced default.
- Focused verification passed:

  ```sh
  cargo fmt -- app/kuro_interpreter_for_build/src/rule.rs \
    app/kuro_interpreter_for_build/src/macro_callable.rs \
    app/kuro_interpreter_for_build/src/interpreter/natives.rs
  cargo check -p kuro_interpreter_for_build
  cargo test -p kuro_build_api_tests map_each_sequence_returns_expand_as_items -- --nocapture
  cargo build -p kuro
  pytest -q tests/core/analysis/test_symbolic_macros.py::test_symbolic_macro_inherited_rule_attr_default
  git diff --check
  ```
- Bounded ZeroMatter smoke:

  ```sh
  timeout 260s env KURO_MEMORY_CHECKPOINTS=1 \
    /var/mnt/dev/kuro/scripts/memory_smoke.sh \
      --interval 5 \
      --include-pgrep 'kurod\[zeromatter\].*plan55-symbolic-macro-inherit-1' \
      -- /var/mnt/dev/kuro/target/debug/kuro \
        --isolation-dir plan55-symbolic-macro-inherit-1 \
        build //sdk:sdk_contents \
    2>&1 | tee /tmp/plan55-symbolic-macro-inherit-1.log
  ```

  The old `copy_to_resource_directory` / missing `target_triple` load-time
  error is gone. The smoke timed out with exit `124` after reaching analysis;
  sampled total RSS peaked around `864 MiB`. Latest visible frontier returned
  to the C++ toolchain-analysis wait shape:

  ```text
  Waiting on bazel_tools//tools/cpp:malloc (...#8d4033f8c19b9f73) -- running analysis [evaluate_rule], and 9 other actions
  ```

  Continue under the Plan 15 C++ toolchain-analysis/cycle investigation rather
  than reopening the symbolic-macro inherited-attrs blocker.

### 15.5.1.1 Repository-rule path/delete parity for generated git crate repos (2026-05-09)

- Follow-up on the missing
  `crates__github.com_Aleph-Alpha_ts-rs.git_a6bbbd18//ts-rs` package from the
  previous smoke.
- Compared Bazel 9.1.0 behavior with focused repository-rule repros:
  `repository_ctx.path(".")` stringifies to the absolute external repository
  path; `ctx.delete(ctx.path("."))` returns `False` when the root is absent,
  deletes the repository root and returns `True` when it is present; and
  `ctx.execute(..., working_directory = str(ctx.path(".")))` creates a missing
  working directory before launching the command.
- Kuro parity changes:
  - `RepositoryPath` display now uses the normalized absolute path.
  - `repository_ctx.execute` creates missing working directories and treats
    `environment = {"KEY": None}` as an unset request.
  - `repository_ctx.delete` returns a bool and normalizes paths before removal,
    avoiding `remove_dir_all("repo/.")` `EINVAL`.
  - Extension repo lazy materialization now discards prior stub markers when a
    valid RepoSpec is present, so a shared `bazel-external/` stub from an earlier
    failed run does not mask the real repo rule.
  - Repo-rule diagnostic summaries are wider so failures include the actionable
    Starlark frame and filesystem/command error.
- Verification:

  ```sh
  cargo fmt
  cargo test -p kuro_interpreter_for_build repository_ctx::tests::test_ --lib
  cargo test -p kuro_external_cells extension_repo::tests::stub_marker_detection_accepts_plain_and_hashed_stubs --lib
  cargo check -p kuro_interpreter_for_build
  cargo build -p kuro
  ```

- Bounded zeromatter smoke:

  ```sh
  cd /var/mnt/dev/zeromatter
  timeout 240s env KURO_MEMORY_CHECKPOINTS=1 \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan64-repoctx-delete-root-2 \
      build //sdk:sdk_contents \
    2>&1 | tee /tmp/plan64-repoctx-delete-root-2.log
  ```

  The smoke still failed, but advanced past the previous missing `ts-rs`
  package: the generated git crate repo materialized and
  `ts-rs/BUILD.bazel` exists. The next observed blocker is canonical load-label
  parity for an apparent repo name in that generated BUILD file:

  ```text
  Error loading `load` of `@crates__ts-rs-12.0.1//:crate.bzl`
  The `load` ... of `crates__ts-rs-12.0.1//crate.bzl` should use the canonical name
  `rules_rs+crate+crates__ts-rs-12.0.1//crate.bzl`
  ```

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

### 15.5.10 Symlink action deps not scheduled before consuming compile (LANDED 2026-04-18)

**Status:** Landed. Two root causes, each fixed independently.

**Investigation summary.** Tracing in `RunAction::visit_artifacts`
(`app/kuro_action_impl/src/actions/impls/run.rs:478`) showed that
`bazel_inputs.len=402` on the SipHash.cpp compile, with the
virtual_header artifact `<build artifact external/.../siphash/SipHash.h
bound to llvm-project//third-party/siphash:siphash>` present and
successfully cast to `CommandLineArgLike`. DICE scheduling was fine —
the symlink `CopyAction::execute` ran before the compile (confirmed
via tracing on copy.rs). So the failure wasn't at scheduling.

**Root cause #1: bazel_inputs missing from `CommandExecutionRequest`.**
`RunAction` has two visitor passes:
- `RunAction::visit_artifacts` (used by `Action::inputs()` for DICE
  scheduling) — correctly iterates `bazel_inputs`.
- `expand_command_line_and_worker` (used by `prepare` to build the
  `CommandExecutionRequest`'s `inputs` list, which `materialize_inputs`
  reads) — visits only exe/args/env. `bazel_inputs` were silently
  dropped here.

Consequence: the local executor's `materialize_inputs` never knew it
had to materialize the virtual_header. The materializer had the
artifact in `Declared` (not `Materialized`) state, so the compile ran
against an empty `_virtual_includes/siphash/siphash/` directory.

Worse: the materializer's `declare` handler unconditionally
dispatches a `clean_path` future
(`app/kuro_execute_impl/src/materializers/deferred/command_processor.rs:927`)
that deletes whatever's already at the declared path. So even though
`CopyAction::execute` had a sidechannel workaround that materialized
the symlink on disk immediately, the materializer wiped it out
seconds later, before the compile ran.

**Fix #1** (`app/kuro_action_impl/src/actions/impls/run.rs`): in
`expand_command_line_and_worker`, after visiting `values.args`, also
iterate `self.starlark_values.bazel_inputs` and call
`visit_artifacts` on each via `ValueAsCommandLineLike`. Now both
visitor passes see the same artifacts; `materialize_inputs` properly
materializes them before the consuming action runs.

**Root cause #2: `resolve_configuration_hash_path` clobbers Shareable
and BazelOutput path kinds.**
`materialize_inputs`
(`app/kuro_execute_impl/src/executors/local.rs:1579`) calls
`artifact.resolve_configuration_hash_path(artifact_fs)` to get the
path to materialize. That chains through
`BuckOutPathResolver::resolve_gen_configuration_hash_path`, which
hardcoded `BuckOutPathKind::Configuration` — regardless of the
artifact's actual kind. For a Shareable artifact whose real on-disk
layout is `buck-out/v2/gen/<cell>/<cfg_hash>/<path>`, this returned
the Configuration layout
`buck-out/v2/gen/<cell>/<cfg_hash>/<pkg>/__<target>__/<path>`.

The materializer had recorded the artifact at the Shareable path (via
`declare_copy` → `resolve_build` which honours Shareable). So
`materialize_many` received a path the materializer didn't recognize
and silently did nothing.

**Fix #2** (`app/kuro_core/src/fs/buck_out_path.rs`):
`resolve_gen_configuration_hash_path` now preserves the artifact's
real `path_resolution_method` for non-content-based kinds, only
redirecting `ContentHash` to `Configuration` (its actual purpose per
the existing doc comment — "except for content-based artifacts"). The
behaviour change is a strict subset of what the function should have
been doing all along.

**Verification:** After both fixes, `SipHash.h` materializes in
`buck-out/v2/gen/llvm-project/<hash>/external/llvm-project/third-party/siphash/_virtual_includes/siphash/siphash/SipHash.h`
as a symlink to the upstream source. All 25+ `@llvm-project//llvm:Support`
sources compile (including `SipHash.cpp` and `SipHash.pic.o` files
land in `buck-out`). Build now fails at the *link* step with "Failed
to spawn a process" for `external/rules_cc/cc/private/toolchain/link_dynamic_library.sh`
— a separate materialization-of-executables issue for 15.5.11.

**Collateral cleanup:** The sidechannel on-disk-symlink workaround in
`CopyAction::execute` (copy.rs:215-274 before this commit) was added
in 15.5.9 to paper over this exact problem. It never worked because
the materializer deleted the file before the consumer ran. Removed as
part of this fix since the proper materialization path now handles
it.

**Parity source:** Bazel's Skyframe dependency tracking works on the
assumption that every action input is materialized before the action
runs. Buck2 implements this via `materialize_inputs` keyed off the
action's `CommandExecutionRequest` inputs. The bug was a Bazel/kuro
translation gap: Bazel rules pass inputs via an explicit `inputs=`
kwarg on `ctx.actions.run`, but kuro's `RunAction` treated that kwarg
as metadata-for-DICE only, not as "this needs to be on disk".

### 15.5.11 Executable spawn fails for tools in bzlmod external cells (LANDED 2026-04-19)

**Status:** Landed. Two stacked bugs; both fixed.

**Investigation.** Link failed with "Spawning executable
`external/rules_cc/cc/private/toolchain/link_dynamic_library.sh`
failed: Failed to spawn a process". Kuro already had machinery
(`ensure_external_symlink` in
`app/kuro_core/src/cells.rs`) to create `<project_root>/external/<cell>`
symlinks pointing at `bazel-external/<cell>+<version>`, and did emit
the correct symlink target for rules_cc (0.2.17). But the symlink
visible at `<project_root>/external/rules_cc` was
`../bazel-external/rules_cc+0.2.11` — left over from a prior Bazel
run. Bazel had pinned a different version. Because the old symlink's
`symlink_metadata()` returned `Ok`, kuro's creator bailed out and
never replaced it.

**Bug #1:** `ensure_external_symlink` only created a symlink when one
was absent. It didn't validate the target, so dangling or stale
symlinks left behind by Bazel survived. **Fix
(`app/kuro_core/src/cells.rs`):** if the entry is a symlink and
`readlink` doesn't match the desired target, remove and recreate.
Non-symlink entries (real files/dirs) are still left alone so we
never clobber user content.

Once the spawn worked, the wrapper script (which *is* the correct
target for bundled rules_cc when interface libraries are in play)
rejected its arguments with exit 13: "Interface library builder (-o)
not found". The script expects a 5-arg prefix
(`<yes|no> <iface_builder> <dyn_lib> <iface_lib> <real_linker>`)
before the actual linker flags. kuro's `get_link_args` produces only
the real linker flags (`-shared -o …`). rules_cc decides whether to
wrap based on
`is_dynamic_library(link_type) and is_enabled("supports_interface_shared_libraries") and not is_enabled("has_configured_linker_path")`.

**Bug #2:** kuro's default `FeatureConfiguration` enabled
`supports_interface_shared_libraries` on *all* platforms
(`cc_common.rs:390`). Bazel's `unix_cc_toolchain_config.bzl` never
enables that feature on Linux/macOS — interface libraries are a
Windows (MSVC) concept. The mis-enablement made rules_cc pick the
wrapper script path; the wrapper then saw raw linker flags and
rejected them. **Fix (`cc_common.rs`):** only enable
`supports_interface_shared_libraries` on Windows.

**Verification:** `@llvm-project//llvm:Support` now **builds
successfully**. `libSupport.a` (35 MB) and `libSupport.so` (13 MB)
both produced end-to-end.

**Parity source:** `unix_cc_toolchain_config.bzl` in Bazel's own
`tools/cpp/` — it does not include `supports_interface_shared_libraries`
in the Linux/macOS feature set. Match that.

### 15.5.12 gentbl_rule (`[DefaultInfo()]` + self-referencing outputs) (LANDED 2026-04-19)

**Status:** Landed. Unblocks `@llvm-project//llvm:llvm-tblgen` (and
anything else depending on tablegen-generated `.inc` headers).

**Investigation.** After `:Support` built, `:llvm-tblgen` failed with
`fatal error: llvm/CodeGen/GenVT.inc: No such file or directory`.
GenVT.inc is produced by a `gentbl_rule` (a user-defined rule in
`llvm-project-overlay/mlir/tblgen.bzl`) that runs `llvm-min-tblgen` on
`ValueTypes.td`. The rule's impl:

```python
ctx.actions.run(outputs=[ctx.outputs.out], inputs=trans_srcs,
                executable=ctx.executable.tblgen, ...)
return [DefaultInfo()]    # <-- empty DefaultInfo()
```

**Bug #1: empty DefaultInfo skipped auto-inject.**
`maybe_inject_implicit_default_info` in `kuro_analysis/src/analysis/env.rs`
short-circuited whenever the impl returned *any* DefaultInfo, even an
empty `DefaultInfo()`. Bazel's contract is that an empty `DefaultInfo()`
still auto-populates `files=` with the rule's predeclared outputs (cf.
Bazel's `DefaultInfo` docs: "If files is not specified, the default output
for the target becomes the list of all predeclared outputs."). In kuro's
behaviour the gentbl's output never made it into the filegroup, never
into the consuming cc_library's textual_hdrs, so GenVT.inc was simply
absent. **Fix:** when an existing DefaultInfo has empty `default_outputs`
and the rule has `attr.output` declarations, replace it with a populated
one. Non-empty author-supplied outputs are left alone.

**Bug #2: action's own output ended up in its own input set → DICE
deadlock.**
With fix #1 in place, DICE correctly scheduled the `TdGenerate` action
— but the action's `inputs()` contained three artifacts, one of which
was GenVT.inc itself (its own output). The gentbl_rule impl does
`args.add("-o", ctx.outputs.out)`, and when that args object was
rendered, `StarlarkDeclaredArtifact::visit_artifacts` routed the
output through `visit_declared_artifact` whose *default* trait impl
forwards to `visit_input`
(`kuro_build_api/src/interpreter/rule_defs/cmd_args/traits.rs:58-67`).
The output artifact was bound to *this* action, so the action's
`inputs()` set contained its own `BuildArtifact` → DICE waited
forever for `ensure_artifact_group_staged(GenVT.inc)` to complete,
which required the `TdGenerate` action, which was waiting on itself.

**Fix:** in `RunAction::inputs()` and in `prepare()`, filter out any
`ArtifactGroup::Artifact(build)` whose `BuildArtifact` is one of the
action's own outputs. Source artifacts and transitive-set projections
are left untouched. The filter addresses the symptom cleanly — a
broader fix (distinguishing declared-output-as-arg from
declared-artifact-as-input in `visit_declared_artifact`) would require
plumbing the action's output list into cmd_args visitors, which does
not have it today. `prepare()` also needs the same filter because
`visitor.inputs()` is computed from a second visitor traversal, and
`ctx.artifact_values(ag)` panics if the artifact wasn't in
`ensured_inputs`.

**Verification:** `vt_gen_filegroup___gen_vt_33394888_genrule` builds
(GenVT.inc materialized at
`buck-out/v2/gen/llvm-project/<hash>/external/llvm-project/llvm/include/llvm/CodeGen/GenVT.inc`).
`@llvm-project//llvm:llvm-tblgen` builds end-to-end (322 commands).
`@llvm-project//llvm:Support` still builds.

**Parity source:** Bazel's
`com.google.devtools.build.lib.starlarkbuildapi.DefaultInfoApi` —
"If files is not specified, the default output for the target becomes
the list of all predeclared outputs." plus
`com.google.devtools.build.lib.actions.Action.getInputs()` — an
action's inputs never include its own declared outputs by construction;
Bazel builds them as two disjoint sets.

### 15.5.13 `attr.output_list` filenames not resolvable as labels (LANDED 2026-04-19)

**Status:** Landed. Partial progress on `@llvm-project//llvm:llc` —
label-resolution phase is past; a separate `strip_include_prefix`
issue (15.5.14) now blocks downstream.

**Investigation.** Building `:llc` triggered multi-output tablegen
rules like

```python
gentbl_cc_library(
    name = "XCoreCommonTableGen",
    tbl_outs = [
        (["-gen-register-info"], [
            "lib/Target/XCore/XCoreGenRegisterInfo.inc",
            "lib/Target/XCore/XCoreGenRegisterInfoEnums.inc",
            …
            "lib/Target/XCore/XCoreGenRegisterInfoTargetDesc.inc",
        ]),
        …
    ],
)
```

`gentbl_filegroup` expands this into a `gentbl_rule` with `out =
outs[0]` and `additional_outputs = outs[1:]`, then a `native.filegroup`
whose `srcs = [<all out files>]`. The filegroup's by-filename srcs
failed label resolution with "Unknown target
`lib/Target/XCore/XCoreGenRegisterInfoTargetDesc.inc`" — the file is a
declared output of the gentbl_rule, but kuro never registered it in
the package's output-file → target map, so the label couldn't resolve.

**Root cause.** Two collaborating bugs in kuro:
1. `attr.output_list` didn't set `is_output = true` on the attribute
   (`kuro_interpreter_for_build/src/attrs/attrs_global.rs::output_list`).
   Without that, `FrozenStarlarkRuleCallable::output_attr_names()`
   didn't include it, so target coercion never iterated the list's
   filenames.
2. Even with the attribute flagged, the registration loop in
   `kuro_interpreter_for_build/src/rule.rs::call` only handled
   `CoercedAttr::String` (single-output). `attr.output_list` coerces
   to `CoercedAttr::List`, so the list's filenames were still
   ignored.

**Fix.** (a) `output_list` now sets `is_output = true` on the emitted
`StarlarkAttribute`. (b) The registration loop handles both
`CoercedAttr::String` (single filename) and `CoercedAttr::List` (a
list of filename strings), calling `register_output_file` for each.

**Verification.** `:llc` now reaches analysis without
"Unknown target" for tablegen-produced .inc filenames.
`:Support` and `:llvm-tblgen` still build.

**Parity source:** Bazel's
`com.google.devtools.build.lib.packages.PackageBuilder` registers
every predeclared output (from `attr.output` / `attr.output_list`)
in the package's output-file table at loading time, so labels for
those files resolve to the declaring target. Match that for both
attribute variants.

### 15.5.14 `artifact.root.path` + duplicate `actions.symlink` registration (LANDED 2026-04-19)

**Status:** Landed. `@llvm-project//llvm:llc` now builds end-to-end
(2684 commands, 408 MB binary).

Two bugs, each independently blocking `:llc` after 15.5.13:

**(a) `artifact.root.path` returned wrong prefix for Bazel parity.**
rules_cc's `cc_compilation_helper.bzl::_repo_relative_path`
(called from `_compute_public_headers`) does

```python
relative_path = paths.relativize(artifact.path, artifact.root.path)
```

and expects `relative_path` to begin with the target's package dir
(e.g. `llvm/lib/Target/XCore/XCoreGenAsmWriter.inc` for a file
declared in `llvm-project//llvm`), which downstream checks against
`strip_include_prefix = "llvm/lib/Target/XCore"`.

kuro computed `root.path` as `full_path - short_path` (suffix-strip).
`short_path` for BuildArtifacts omits the package component, so the
stripped root *included* the package (`buck-out/…/external/llvm-project/llvm`)
and `relativize` gave back just the filename. `strip_include_prefix`
never matched. XCoreCommonTableGen's cc_library failed with
"header '…' is not under the specified strip prefix".

**Fix.** Added `with_root_path` on `ArtifactPath` and the
`StarlarkArtifactLike` trait (implemented for StarlarkArtifact /
StarlarkDeclaredArtifact / StarlarkOutputArtifact /
StarlarkPromiseArtifact). For BuildArtifacts it returns
`buck-out/v2/gen/<cell>/<cfg>` (root cell) or
`buck-out/v2/gen/<cell>/<cfg>/external/<cell>` (non-root cell),
matching Bazel's bin_dir layout. Source artifacts return empty
string (Bazel's convention). `artifact.root.path` now uses this
instead of the suffix-strip heuristic.

**(b) Duplicate `actions.symlink` registration with `hdrs=textual_hdrs`.**
`gentbl_cc_library` with `strip_include_prefix` sets
`hdrs = [":filegroup"]` *and* `textual_hdrs = [":filegroup"]` — same
filegroup in both. rules_cc's `_compute_public_headers` then runs
twice (once for hdrs, once for textual_hdrs) and each run calls

```python
virtual_header = actions.declare_shareable_artifact(<same path>)
actions.symlink(output=virtual_header, target_file=original_header)
```

kuro's `declare_shareable_artifact` already dedupes via
`path_to_artifact`, so both runs get the same artifact. But each
`actions.symlink` then called `register_action` which called
`OutputArtifact::bind(key)` — the second call panicked with
"Attempted to bind an artifact which was already bound".

**Fix.** `ActionsRegistry::register` is now idempotent: if every
output in the set is already bound to a single existing action key,
return that key and skip re-registering. If outputs are bound to
different actions (genuine conflict), fall through so `bind()` errors
normally. Added `OutputArtifact::existing_action_key()` helper.

**Verification:** `@llvm-project//llvm:llc` builds — 2684 commands,
408 MB binary. `:Support`, `:llvm-tblgen`, `:BinaryFormat` still
build.

**Parity source:** Bazel's `StarlarkAction` / `SpawnAction`
registration is idempotent in the same way — registering a spawn
for an artifact already owned by an equivalent action is a no-op,
not an error. Kuro matches the no-op semantics for the common case
without trying to verify "equivalent" (which would need a structural
comparison of the action).

### 15.5.15 `genrule(output_to_bindir=1)` type error (LANDED 2026-04-20)

**Status:** Landed. Trivial — unblocks loading of
`@llvm-project//clang/BUILD.bazel`.

**Bug.** `@llvm-project//clang/BUILD.bazel:1777` declares
`genrule(..., output_to_bindir = 1, ...)`. Bazel accepts int (0/1)
or bool there. Kuro's native `genrule` typed the parameter as `&str`
with default `""`, so evaluation errored with
`Type of parameter 'output_to_bindir' doesn't match, expected 'str', actual 'int (repr: 1)'`.

**Fix.** Relax the parameter type to `Value<'v>` and ignore its
content. Kuro already ignored the attribute's semantics; this is
purely about accepting the upstream call sites.

### 15.5.16 Anonymous `rule(cfg=dict(...))` transitions not findable (LANDED 2026-04-20)

**Status:** Landed. Unblocks clang analysis past the rules_python
py_binary transition.

**Fix.** Short-circuit anonymous-transition lookup in three places:
- `TransitionCalculation::apply_transition` (top of impl) — return
  `TransitionApplied::Single(cfg)` for `MagicObject(name =
  "_anonymous_transition")` before the inner `fetch_transition`
  call that would otherwise fail.
- `do_apply_transition` (DICE key compute path) — same guard.
- `TransitionAttrsKey::compute` — return `Ok(None)` so the
  `resolve_transition_attrs` pass doesn't trip on missing attrs.

Rationale: kuro does not yet execute Starlark config transitions;
honouring the anonymous `rule(cfg=dict(...))` form as identity
matches what we already do for `config.target()` and other no-op
transitions.

### 15.5.17 bazel_tools `python_bootstrap_template.txt` self-cycle (LANDED 2026-04-20)

**Status:** Landed. Unblocks clang past the next configured-target
cycle.

**Fix.** `bazel_tools//tools/python:BUILD` declared a filegroup
`name = "python_bootstrap_template.txt"` whose `srcs` list contained
the source file of the same name. Kuro treats `:python_bootstrap_template.txt`
as a label reference, which matches the filegroup itself, so DICE
sees a self-dependency cycle. Replaced with `exports_files()` — the
source file already exists in the package and referencing
`:python_bootstrap_template.txt` resolves directly to the file when
the source is publicly exported.

### 15.5.18 rules_python `pythons_hub` stub missing `versions.bzl` (LANDED 2026-04-20)

**Status:** Landed. Unblocks clang past the rules_python
`config_settings` evaluation.

**Fix.** rules_python's module extension generates `pythons_hub` with
a `versions.bzl` exposing `DEFAULT_PYTHON_VERSION`, `MINOR_MAPPING`,
`PYTHON_VERSIONS`. Kuro doesn't yet run the extension; its stub
`extension_repo.rs::materialize_stub_repo` now emits a default
`versions.bzl` covering Python 3.8–3.13 minor versions so
`construct_config_settings()` in
`rules_python//python/config_settings/BUILD.bazel` evaluates
without crashing on empty `MINOR_MAPPING`.

### 15.5.19 cross-package output-file label resolution (LANDED 2026-04-20)

**Status:** Landed. Unblocks clang past the LLVM frontend `.inc`
lookups.

**Observed.** `clang/BUILD.bazel` has
`cc_library(name="sema", srcs = [...] + ["//llvm:include/llvm/Frontend/OpenACC/ACC.inc"])`.
kuro's per-module `output_file_registry` tracks predeclared outputs
for labels resolved *within the same package*. A cross-package label
like `//llvm:include/.../ACC.inc` survives coercion as a full label
and hits DICE target lookup, which raised MissingTargetError because
the package has no target named `include/.../ACC.inc` (the file is a
predeclared output of an `acc_gen_impl__…_genrule` target).

**Fix.** Two pieces:
1. `SourceAttrType::coerce_item` — when the bare source label has no
   `:`, try `ctx.output_file_target()` before falling through to
   `coerce_path`. Added an `output_file_target` method to the
   `AttrCoercionContext` trait with a default that returns `None`;
   `BuildAttrCoercionContext` implements it using its existing
   `output_file_registry`. This handles intra-package file-name
   references in `srcs`.
2. `EvaluationResult::resolve_target` — on `MissingTargetError` for
   a slashed file-like name (`.inc`/`.h`/`.h.inc`/`.cpp`/`.def`),
   linear-scan the package's targets checking each node's
   `CoercedAttr::String` / `CoercedAttr::List` attribute values for a
   match, and redirect to the declaring target. Handles the
   cross-package case (label was already parsed and parked; DICE
   lookup falls back to this check).

The registry is per-module (per `BuildAttrCoercionContext`) so a
cross-package DICE lookup needs the linear scan; maintaining a
persistent reverse index on every `EvaluationResult` would cost
memory per package for the common case that all target lookups hit
by name.

### 15.5.20 `py_internal` stub attributes return callables (LANDED 2026-04-20)

**Status:** Landed. Unblocks clang past rules_python's py_library
analysis.

**Fix.** rules_python ≥ 1.9 calls `py_internal.<method>(args)` from
inside rule implementations (e.g.
`py_internal.get_label_repo_runfiles_path(ctx.label)`). Kuro's
`PyInternalStub::get_attr` returned `Value::new_none()` for all known
attribute names, so the subsequent `.call()` errored with "Operation
`call()` not supported on type `NoneType`".

Added a `PyInternalStubCall` Starlark value that accepts any call
arguments and returns an empty string. `get_attr` now routes the
method-like attributes through it; non-method attributes still return
`None`. Matches our "stub that never errors" philosophy for
Bazel-specific internal APIs.

### 15.5.21 rules_python py_binary Python toolchain (LANDED 2026-04-20)

**Status:** Landed. `clang:bundle_resources` (py_binary) now
**builds successfully** end-to-end; py_library + py_binary analysis
proceed past the `py3_runtime is missing` fail; resolution surfaces a
real `ToolchainInfo` with `py3_runtime` when rules_python is in the
module graph. `clang:clang` build advances past analysis of all
rules_python-backed targets and now fails on a separate
`$(WORKSPACE_ROOT)` Make-variable gap in `cc_library` (see
`ctx_var_builtins.md` — Make-variable expansion, unrelated to
toolchain resolution).

**Root causes.** Three distinct bugs stacked on top of each other:

1. **Mandatory flag ignored.** kuro's `rule(toolchains=[...])`
   extractor reduced every entry to `Vec<String>` and `env.rs` built
   the default exec group's `RequiredToolchainType` list with
   `mandatory: true` hardcoded. rules_python passes
   `ruleb.ToolchainType(TOOLCHAIN_TYPE, mandatory = False)` for
   optional types (`exec_tools_toolchain_type`, cc toolchain, target
   py toolchain on py_library) — kuro promoted those to mandatory,
   so a single missing registration poisoned the entire resolution.

2. **Empty `ResolvedToolchains` swallowed optional misses.** Even
   when resolution returned partial results, `ctx.toolchains[T]` for
   an unregistered optional `T` hit the "map non-empty but entry
   absent" arm in `context.rs::at()` and raised
   `"Toolchain type '...' was not resolved"`. Bazel returns the
   entry (None) for declared-but-unresolved optional types; kuro now
   matches.

3. **No host Python toolchain registered.** LLVM's MODULE.bazel
   depends on rules_python (for small py_binary helpers) but never
   calls `register_toolchains` or configures `python.toolchain`.
   Bazel historically auto-generated `@local_config_python`; kuro
   now does the same.

**Landed changes.**

- `app/kuro_node/src/rule.rs`, `app/kuro_interpreter_for_build/src/rule.rs`,
  `app/kuro_analysis/src/analysis/env.rs`:
  `Rule.toolchain_types: Vec<String>` → `Vec<(String, bool)>`
  (label, mandatory). The Starlark `rule()` builder extracts
  `.mandatory` from `ToolchainTypeRequirement` / `config_common.toolchain_type`;
  `env.rs` threads the flag into `RequiredToolchainType.mandatory`.
  Raw string/Label entries default to mandatory=true (Bazel default).

- `app/kuro_build_api/src/interpreter/rule_defs/context.rs`
  `ResolvedToolchains::at()` rewritten as a three-way match:
  resolved → return `ToolchainInfo`; declared-but-unresolved → return
  `None`; not declared → raise. Matches Bazel's
  `ctx.toolchains[type]` semantics.

- `app/kuro_external_cells_bundled/build.rs` generates a new
  `local_config_python` bundled cell with:
  - `py_runtime(interpreter_path = <detected host python3>, python_version = "PY3")`
  - `py_runtime_pair(py3_runtime = :py3_runtime)`
  - `toolchain(name = host_toolchain, toolchain_type = "@rules_python//python:toolchain_type", toolchain = :py_runtime_pair)`
  - Stub `stub_toolchain_info` rule (returns empty `ToolchainInfo`)
    plus `toolchain(name = host_launcher_maker_toolchain, toolchain_type = "@bazel_tools//tools/launcher:launcher_maker_toolchain_type")`
    for `bazel_9_or_later` py_binary's mandatory launcher toolchain.
    `host_python3` probed in build.rs (`/usr/bin/python3` →
    `/usr/local/bin/python3` → `/opt/homebrew/bin/python3`).

- `app/kuro_external_cells_bundled/src/lib.rs` adds
  `LOCAL_CONFIG_PYTHON` to the bundled-cell set.

- `app/kuro_common/src/legacy_configs/cells.rs`
  (parse/cell-resolution path) auto-registers the
  `local_config_python` cell alongside `local_config_platform` for
  bzlmod workspaces. After collecting `registered_toolchains` from
  all modules, if `rules_python` is in the module graph and no
  existing `local_config_python` registration is present, prepends
  `@local_config_python//:host_toolchain` and
  `@local_config_python//:host_launcher_maker_toolchain` at lowest
  priority. Explicit user registrations override (they appear earlier
  in `all_toolchains` and resolve first).

- `bazel_tools/tools/launcher/BUILD` adds the `launcher_toolchain_type`
  and `launcher_maker_toolchain_type` targets (previously only in
  `BUILD.tools`/`BUILD.bootstrap`). kuro's `toolchain()` rule requires
  the toolchain_type to be a resolvable dep target.

- `app/kuro_build_api/src/interpreter/rule_defs/cc_common.rs`
  `PyInternalStub` gains the rules_python 1.9 method names it was
  missing: `declare_constant_metadata_file`,
  `expand_location_and_make_variables`, `is_singleton_depset`,
  `make_runfiles_respect_legacy_external_runfiles`,
  `merge_runfiles_with_generated_inits_empty_files_supplier`.
  `PyInternalStubCall` special-cases two paths beyond the
  empty-string default:
  - `merge_runfiles_with_generated_inits_empty_files_supplier` and
    `make_runfiles_respect_legacy_external_runfiles` pass through
    the `runfiles=` kwarg (so `RunfilesBuilder.add()` accepts the
    return value).
  - `declare_constant_metadata_file(ctx=, name=, root=)` dispatches
    to `ctx.actions.declare_file(name)` (return is used as an
    action output; callers read `.path`).

**Parity source.**
- Bazel toolchain resolution algorithm:
  https://bazel.build/extending/toolchains#toolchain-resolution
- rules_python toolchain declarations:
  `bazel-external/rules_python+1.9.0/python/private/py_library.bzl:295`
  (py_library's toolchains list with mandatory=False on both target
  and exec_tools types); `py_executable.bzl:1848-1851` (py_binary
  adds cc + launcher_maker toolchains, launcher_maker is mandatory
  when `bazel_9_or_later`).
- `platform_common.ToolchainInfo` provider shape:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/analysis/platform/ToolchainInfo.java`.

**Additional fixes that landed in the same changeset to unblock
`clang:bundle_resources` end-to-end** (scope-adjacent, not strictly
toolchain resolution):

1. **`DefaultInfo.files_to_run.executable` fallback.**
   `app/kuro_build_api/src/interpreter/rule_defs/provider/builtin/default_info.rs`
   `files_to_run` attribute getter now falls back to the single
   `default_output` when `DefaultInfo.executable` is empty.
   Matches Bazel's "source file with exec bit is implicitly its own
   executable" semantics. Unblocks rules_python's
   `_build_data_writer` (alias → `build_data_writer.sh` source file)
   when used as `ctx.actions.run(executable=...)`.

2. **py_internal method name/positional handling.**
   `app/kuro_build_api/src/interpreter/rule_defs/cc_common.rs`:
   `PyInternalStub.has_attr` / `get_attr` / `dir_attr` gain the
   rules_python 1.9 methods `declare_constant_metadata_file`,
   `expand_location_and_make_variables`, `is_singleton_depset`,
   `make_runfiles_respect_legacy_external_runfiles`,
   `merge_runfiles_with_generated_inits_empty_files_supplier`.
   `PyInternalStubCall.invoke` handles both keyword and positional
   call shapes for the runfiles-passthrough methods (rules_python
   calls `make_runfiles_respect_legacy_external_runfiles(ctx, rf)`
   positionally but `merge_runfiles_with_generated_inits_empty_files_supplier(ctx=, runfiles=)`
   by keyword). `declare_constant_metadata_file` dispatches to
   `ctx.actions.declare_file(name)` so callers that read `.path`
   on the result get a real File instead of an empty string.

**Remaining analysis gaps (separate tickets).**

1. **Exec-tools toolchain.** rules_python declares
   `exec_tools_toolchain_type` as mandatory=False. We resolve it to
   None today (no registration), which matches Bazel behaviour for
   workspaces that don't configure pystar. If a workspace ever needs
   it (precompilation features), point to a user-registered
   py_exec_tools_toolchain.

**Remaining execution-phase gaps (separate tickets).**

1. **py_binary runfiles tree missing at action time.** Once analysis
   clears, `clang:analysis_htmllogger_gen` fires a `run_binary` that
   invokes the `bundle_resources` py_binary; the generated stub
   script calls `FindModuleSpace()` which asserts on the presence of
   a `.runfiles/` directory next to the executable. Kuro does not
   create the Bazel-style runfiles symlink tree for py_binary
   outputs yet. This is the gate that lets the *execution* of
   `clang:clang` proceed; analysis is complete.

**Scope boundary held.** The implementation is general — no
`if toolchain_type == "python"` branches. The only Python-specific
code is (a) `local_config_python`'s bundled BUILD content, which
mirrors what Bazel itself auto-generates, and (b) the one-line
"if rules_python in module graph" conditional that decides whether
to prepend the auto-registration. Toolchain resolution, mandatory
handling, and `ctx.toolchains[T]` semantics are generic and will
serve future rules_java/rules_go/cc_test gaps without further
changes to the resolver.

### 15.5.21.1 `$(WORKSPACE_ROOT)` / `TemplateVariableInfo` from `ctx.attr.toolchains` (LANDED 2026-04-20)

**Status:** Landed. With 15.5.21 unblocking py_library/py_binary
analysis, `clang:clang`'s next failure moved to `cc_library` copts
expansion:

```
error: fail: llvm-project//clang:basic (local_config_platform//:host#...):
       $(WORKSPACE_ROOT) not defined
    --> bazel-external/rules_cc+0.2.17/cc/common/cc_helper.bzl:590:5
```

**Root cause.** Bazel's `RuleContext.getMakeVariables()` merges
`TemplateVariableInfo.variables` from every dep listed in a target's
implicit `toolchains = [...]` attribute into `ctx.var`. LLVM's
`@llvm-project//:workspace_root` rule publishes
`TemplateVariableInfo({"WORKSPACE_ROOT": ctx.label.workspace_root})`
and `cc_library` targets list it via `toolchains = [":workspace_root"]`.
rules_cc's `cc_helper._lookup_var` falls back to `ctx.var.get(var)`
when the caller-supplied `additional_vars` dict misses; if the var
is absent from both, it `fail()`s.

kuro declared the implicit `toolchains` attribute as
`AttrType::list(AttrType::any())`, so labels were stored as opaque
values and never resolved as deps. `ctx.var` therefore couldn't see
any `TemplateVariableInfo`.

**Changes.**

- `app/kuro_interpreter_for_build/src/rule.rs`: implicit
  `toolchains` attribute changed to
  `AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY))`.
  Kuro now resolves the deps and exposes them as
  `Dependency`/`FrozenDependency` values on `ctx.attr.toolchains`.

- `app/kuro_build_api/src/interpreter/rule_defs/platform_common.rs`:
  `TemplateVariableInfoInstance::variables()` public accessor
  returning `&SmallMap<String, String>`.

- `app/kuro_build_api/src/interpreter/rule_defs/context.rs`:
  new `collect_toolchains_template_vars()` helper iterates
  `ctx.attr.toolchains`, downcasts each item to
  `Dependency`/`FrozenDependency`, reads
  `TemplateVariableInfo` from the provider collection, and yields
  `(name, value)` pairs. Both `ctx.var` and `expand_make_variables`
  merge these pairs with user-provided substitutions winning.

**Scope boundary.** Generic — no rules_cc or LLVM-specific branches.
Any rule that publishes `TemplateVariableInfo` and is referenced in
a target's `toolchains = [...]` attribute now feeds its variables
into `ctx.var` / `$(VAR)` expansion. The existing hardcoded defaults
(`STACK_FRAME_UNLIMITED`, `CC`, etc.) still seed `ctx.var` for rules
that rely on the cc_toolchain's vars, until Phase 5's proper
`TemplateVariableInfo` gathering from resolved rule-level toolchains
lands (see `ctx_var_builtins.md`).

**End-to-end verification.** After this change,
`kuro cquery "deps(@llvm-project//clang:clang)"` resolves **916
targets** without analysis failures. `kuro build @llvm-project//clang:clang`
advances past analysis into action execution, where the
`bundle_resources` py_binary is invoked as a `run_binary` tool and
fails on a missing runfiles symlink tree — an orthogonal
execution-phase gap (see §15.5.21 "Remaining execution-phase gaps").

### 15.5.22 `@llvm-project//llvm:llvm` empty driver-tools select (LANDED 2026-04-20)

**Status:** Landed. `LLVMDriverTools.def` now emits all 42
`LLVM_DRIVER_TOOL(...)` entries (clang, llvm-ar, llvm-nm, …) when
`llvm:llvm` is built with the default `driver-tools` flag value.
`tools/llvm-driver/llvm-driver.cpp` compiles. The llvm:llvm build
advances into `clang-driver`'s dep chain and hits the same
execution-phase py_binary runfiles gap as §15.5.21 (documented there
as the next follow-up).

**Root cause.** Two bugs in
`app/kuro_analysis/src/analysis/calculation.rs::check_config_setting_flag_values`:

1. **Bare target names in `flag_values` keys rejected.** LLVM's
   `generate_driver_selects` generates `config_setting(name =
   "driver-tools-include-<tool>", flag_values = {name: tool})` where
   `name` is the bare string `"driver-tools"` (the sibling
   `_validated_string_list_flag` target in the same package). Bazel
   accepts bare names as relative labels; kuro routed them through
   `TargetLabel::parse`, which rejected with
   `Invalid absolute target pattern 'driver-tools' is not allowed`,
   so every `flag_values` check short-circuited to "no match" and
   `select_driver_tools()` returned `[]`.

2. **`build_setting_default` of list type fell through to the
   fallback error arm.** `_validated_string_list_flag` has
   `build_setting = config.string_list(flag = True)`, so its default
   is a Starlark list (all 23 tool names). kuro's lookup only
   handled `String`/`Bool` defaults and returned `Ok(false)` for
   anything else. Needs list-type handling where `flag_values[T] =
   "<value>"` matches when `<value>` is present in the list (Bazel's
   semantics for string_list/int_list flags).

**Changes.**

- `check_config_setting_flag_values` now normalizes the `flag_values`
  dict key before `TargetLabel::parse`: bare names (`"driver-tools"`)
  become `//<config_setting_pkg>:<name>`, `:name` becomes the same,
  and already-absolute labels (`@…//…`, `//…:…`) pass through
  unchanged. Matches Bazel's relative-label resolution rules for
  `flag_values`.

- Default-value lookup now handles `CoercedAttr::List` and
  `CoercedAttr::Int` in addition to `String`/`Bool`. A new
  `(scalar_actual, list_actual)` pair threads through the caller;
  the match is `actual == expected` for scalars and
  `list.contains(&expected)` for lists. CLI overrides
  (`--//pkg:target=value`) for list flags split on `,`, matching how
  kuro parses `string_list_flag` CLI values today.

**Parity source.**
- Bazel `ConfigSettingRule.java` — `flag_values` matching:
  https://github.com/bazelbuild/bazel/blob/master/src/main/java/com/google/devtools/build/lib/analysis/config/ConfigSetting.java
  The `matchesValues` helper handles both scalar and list-typed
  build settings with `.contains()` semantics for the latter.
- LLVM's `driver.bzl:73-95` (`generate_driver_selects`) — uses the
  bare-name form `flag_values = {name: tool}` that this fix
  unblocks.

**Verification.**
`kuro build @llvm-project//llvm:llvm` now compiles
`external/llvm-project/llvm/tools/llvm-driver/llvm-driver.cpp`
(previously a compile error at `:27:5`). The build progresses into
`clang-driver` and its transitive C++ sources; the next blocker is
the same py_binary runfiles tree missing at action time (§15.5.21
"Remaining execution-phase gaps"). `LLVMDriverTools.def` contents
(43 lines, 42 `LLVM_DRIVER_TOOL` entries + trailing `#undef`)
confirmed in
`buck-out/v2/gen/llvm-project/.../llvm/LLVMDriverTools.def`.

### 15.5.23 py_binary runfiles tree materialization (LANDED)

**Status:** Landed 2026-04-21. `DefaultInfo(executable=..., default_runfiles=...)`
now synthesizes a `<exe>.runfiles/<workspace>/<short_path>` symlink tree at
analysis time and wraps the executable so consumer actions pick the tree up
via `visit_artifacts`. rules_python's `py_binary` stub finds its runfiles
directory and successfully dispatches to the stage2 bootstrap → main Python
module.

Implementation, file:line:
- `app/kuro_build_api/src/interpreter/rule_ctx_storage.rs` (new): moved the
  thread-local that holds the current rule's `ctx` value out of
  `kuro_interpreter_for_build::subrule` so that `default_info_creator` in
  `kuro_build_api` can reach it.
- `app/kuro_build_api/src/interpreter.rs:12`: declares the new module.
- `app/kuro_interpreter_for_build/src/subrule.rs:80-85`: re-exports the
  storage API so existing callers (`kuro_analysis::analysis::env`) keep
  compiling unchanged.
- `app/kuro_build_api/src/interpreter/rule_defs/provider/builtin/default_info.rs`:
    - adds `SYNTHESIZE_RUNFILES_TREE: LateBinding<...>` (≈ line 90).
    - `default_info_creator` (≈ line 970): when `executable` is set and
      runfiles are non-empty, dispatches through `SYNTHESIZE_RUNFILES_TREE`,
      replaces the stored executable with the wrapped value, and appends the
      tree artifact to `other_outputs` as a safety net.
    - `RunfilesGen` gets `pub files()/symlinks()/root_symlinks()/empty_filenames()`
      accessors so the synthesis can read them cross-crate.
    - `runfiles_has_content` helper (≈ line 1016) cheaply gates the dispatch.
- `app/kuro_build_api/src/interpreter/rule_defs/context.rs:251-280`: adds
  `AnalysisContext::workspace_name_str()` (derived from the target cell name)
  and `AnalysisContext::actions_typed()` accessor for the synthesis.
- `app/kuro_build_api/src/interpreter/rule_defs/depset.rs:448`: relaxes
  `collect_depset_elements` from `pub(crate)` to `pub` so the synthesis can
  unpack the runfiles depset.
- `app/kuro_build_api/src/interpreter/rule_defs/artifact/starlark_artifact.rs`:
  adds `StarlarkArtifact::new_with_associated_artifacts(...)` constructor.
- `app/kuro_action_impl/src/context/runfiles_tree.rs` (new): late-binding
  implementation — builds the `srcs` dict, declares the tree output as a
  sibling of the executable via `AnalysisActions::state().declare_output(...,
  OutputType::Directory, ...)`, registers `UnregisteredSymlinkedDirAction::new(Symlink, srcs)`,
  and wraps the executable by constructing a fresh `StarlarkArtifact` whose
  `associated_artifacts` union the original associations with the tree.
- `app/kuro_action_impl/src/context.rs:24`: declares the new
  `runfiles_tree` submodule.
- `app/kuro_action_impl/src/lib.rs:28`: wires
  `context::runfiles_tree::init_synthesize_runfiles_tree()` into
  `init_late_bindings`.

Unblocker that also landed with §15.5.23:
- `app/kuro_external_cells/src/bzlmod.rs:319-349`: `declare_all_source_artifacts`
  was hardcoding `is_executable: false` on every bzlmod source it registered
  with the materializer, so materialized scripts under
  `buck-out/v2/external_cells/bzlmod/...` had no `+x`. rules_python's
  `build_data_writer.sh` (a `py_write_build_data` sub-action of every py_binary)
  failed to spawn with "Failed to spawn a process". Now reads
  `entry.metadata().permissions().mode() & 0o111` on Unix. Strictly orthogonal
  to the runfiles tree synth but kept in the same commit because the py_binary
  milestone cannot be verified without it.

**Design choices worth noting.**
- External-repo files in runfiles carry a `short_path` of the form
  `../<repo>/<rel>`. The synthesis strips the leading `../` and drops the
  workspace prefix in that case so the generated key is a valid forward-
  relative path. See `runfile_key` in `runfiles_tree.rs`.
- `empty_filenames` (used by Bazel for implicit `__init__.py` in py_library
  runfiles) is not materialized. Python 3 namespace packages cover the cases
  we've seen in LLVM; a follow-up task tracks wiring zero-byte placeholders
  through `ctx.actions.write` if a consumer needs them.

**Milestone verification.** From
`/var/mnt/dev/llvm-project/utils/bazel`, `kuro build
@llvm-project//clang:analysis_htmllogger_gen` now progresses through the
py_binary stage2 bootstrap and starts executing
`clang/utils/bundle_resources.py`. The build still fails, but at a distinct
layer:

```
File ".../bundle_resources.py", line 21, in <module>
    with open(outfile, "w") as out:
FileNotFoundError: [Errno 2] No such file or directory:
    '$(execpath lib/Analysis/FlowSensitive/HTMLLogger.inc)'
```

That is not a §15.5.23 problem — `run_binary` is passing its `args` through
to the executable without expanding `$(execpath ...)` tokens. Tracked as
§15.5.24 below.

**Follow-ups opened by this work.**
- §15.5.25: `runfiles.empty_filenames` is dropped on the floor in
  `SYNTHESIZE_RUNFILES_TREE`. Acceptable while Python 3 namespace packages
  suffice, but wire `ctx.actions.write("", output=...)` through the synth if
  a consumer needs real zero-byte entries.

---

### 15.5.24 `ctx.expand_location` resolves rule's own attrs / outputs (LANDED)

**Status:** Landed 2026-04-21. `ctx.expand_location(input, targets)` now also
consults the rule's own `ctx.attrs.*` values (for deps / source files) and
`ctx.outputs.*` (for `attr.output` / `attr.output_list` outputs) so
`bazel_skylib//rules:run_binary.bzl` — which only passes `[ctx.attr.tool]`
explicitly — can still expand `$(execpath <output>)` and
`$(execpath <source>)` in its `args`.

**Observed.** After §15.5.23 landed, `kuro build
@llvm-project//clang:analysis_htmllogger_gen` reached the bundle_resources
py_binary, then failed with `FileNotFoundError: '$(execpath
lib/Analysis/FlowSensitive/HTMLLogger.inc)'` — run_binary was passing the
raw token through to the tool. Kuro's `expand_location` only looked at the
`targets` list, so output labels in `outs=[...]` and source-file labels in
`srcs=[...]` were never resolvable.

**Fix, file:line.**
- `app/kuro_build_api/src/interpreter/rule_defs/context.rs:expand_location`
  (around lines 1609-1865). Three additions:
    1. **Lazy CtxOutputs init.** The Bazel rule impl may call expand_location
       before ctx.outputs, so `this.0.outputs` can still be `None` at the
       time we need it. We now allocate the CtxOutputs wrapper on-demand,
       mirroring the `outputs(...)` starlark attribute method.
    2. **Walk the attrs struct.** Iterate via `StructRef::iter()` (not
       `value.iterate`, which doesn't yield field names for structs), and
       for each attribute value downcast to `StarlarkArtifact` /
       `StarlarkDeclaredArtifact` / `Dependency`. For source-file `File`
       values we register the artifact's `short_path` as the lookup key so
       user-typed relative labels match.
    3. **Deferred output declaration.** For `attr.output` / `attr.output_list`
       attrs the raw value in `ctx.attrs` is a plain string / list[str], so
       we can't read them as artifacts until CtxOutputs declares them.
       Eagerly calling `outputs_val.get_attr(name)` for every list[str] attr
       turns `tags`, `args`, and similar into unbound declared outputs
       (triggers "Artifact must be bound by now" at analysis resolution). We
       snapshot `(attr_name, list[str])` pairs up-front and only call
       CtxOutputs when `find_paths` actually sees a query label equal to one
       of the snapshotted strings — exactly the attrs where declaration is
       legitimate.
    4. **Path-suffix matching.** Source files from external cells carry a
       short_path of `../<repo>/<pkg>/<rel>`; `find_paths` now matches when
       the recorded label ends with `/<query>` so
       `../llvm-project/clang/lib/Analysis/FlowSensitive/HTMLLogger.html`
       resolves for `$(execpath lib/Analysis/FlowSensitive/HTMLLogger.html)`.

**Milestone verification.** `kuro build
@llvm-project//clang:analysis_htmllogger_gen` → BUILD SUCCEEDED. Output at
`buck-out/v2/gen/llvm-project/9b5202f249973417/external/llvm-project/clang/lib/Analysis/FlowSensitive/HTMLLogger.inc`.
The bundle_resources py_binary receives the resolved buck-out paths as args
and successfully bakes the HTML/CSS/JS sources into the .inc file.

**Known limitations worth tracking.**
- `find_paths` does `ends_with(/query)` — if two targets share the same
  filename (e.g. both packages have a `foo.h`), the first recorded entry
  wins. Matches how `$(execpath …)` works in small Bazel rules; revisit if
  we see ambiguity bugs.
- The deferred CtxOutputs trigger assumes the list-of-strings index maps
  1:1 to the declared artifact list index. Holds today because
  CtxOutputs.get_attr declares in list order. If that changes we'll need
  to match by filename instead of index.

---

### 15.5.23 historical context (superseded)

*(Kept for reference — the "three stacked gaps" and the original design
sketch.)*

**Observed.** After §15.5.21 + §15.5.21.1 + §15.5.22 clear analysis,
`bundle_resources.runfiles/llvm-project/_bundle_resources_stage2_bootstrap.py`
is never created on disk, and the stub's `FindModuleSpace` asserts:

```
AssertionError: Cannot find .runfiles directory for
    buck-out/v2/gen/llvm-project/.../clang/__bundle_resources__/bundle_resources
```

**Root cause map (investigated, not fixed).**

1. **Runfiles aren't built.** kuro's
   `app/kuro_build_api/src/build/outputs.rs::get_outputs_for_top_level_target`
   collects only `DefaultInfo.default_outputs`,
   `default_outputs[i].associated_artifacts`, and `other_outputs`.
   `default_runfiles` / `data_runfiles` are tracked on `DefaultInfo`
   but never enumerated for building. rules_python declares the
   stage2 bootstrap (and the py_binary main source) as *runfiles only*,
   so the `expand_template`/`write` action that produces them never
   fires.

2. **No symlink-tree action type.** Kuro has
   `ctx.actions.symlinked_dir(output, srcs_dict)` which is the
   building block, but no analysis-time machinery synthesises a
   `<exe>.runfiles/<workspace>/<path>` tree from a target's
   `DefaultInfo.default_runfiles`. Bazel's `SymlinkTreeAction` is
   what produces this; kuro doesn't emit it from any rule today.

3. **`ctx.actions.run` doesn't propagate tool runfiles.**
   `app/kuro_action_impl/src/context/run.rs` accepts `executable=`
   as a `File` and `tools=[...]` as files-or-depsets. It reads
   `visit_artifacts` which already walks
   `StarlarkArtifact::associated_artifacts`, but nothing *sets*
   those associated_artifacts to the target's runfiles tree. The
   Bazel-equivalent is "tool expansion" — when `tools=[X]` is a
   runnable target, Bazel auto-pulls its `data_runfiles` into the
   action's input set and materialises the runfiles symlink tree
   next to X. Kuro's run action sees only the stub File.

**Approach (for a future session).**

Two changes, landable in sequence, each independently useful:

- **A.** `DefaultInfo(executable = X, default_runfiles = R)` auto-
  synthesizes a `symlinked_dir` action at
  `<X_parent>/<X_name>.runfiles/` whose `srcs` dict is built from
  `R.files` (keyed by `<workspace_name>/<short_path>`), `R.symlinks`,
  and `R.root_symlinks`. Wrap `X` via
  `StarlarkArtifact::with_associated_artifacts([tree])` before
  storing in `DefaultInfo.executable`. Requires threading an
  `AnalysisActions` handle into `default_info_creator`, which today
  only gets a `heap` / `eval`.

- **B.** `ctx.actions.run` / `run_shell`, when building the action's
  input set, walk `associated_artifacts` on each executable/tool
  File (already done via `visit_artifacts`). Because A makes the
  runfiles tree an associated artifact of the stub, the tree is
  built before the run action fires and the Python bootstrap
  finds it via the standard `<stub>.runfiles/` lookup.

**Scope warning.** The threading in (A) is where most of the
engineering is: the `default_info_creator` is defined via
`#[starlark_module]`/`#[internal_provider]` and needs access to the
current rule's `AnalysisActions` (reachable from
`kuro_interpreter_for_build::subrule::get_current_rule_ctx_raw` but
not idiomatic at that layer). Expect ~1 day of plumbing + testing
before any llvm-project result. Until landed,
`kuro build @llvm-project//clang:clang` analyzes cleanly but can't
execute py_binary tool invocations.

**Workaround for the short term.** None in-tree. A workspace-side
shim (user-defined `py_binary` macro that writes a plain shell
wrapper invoking `python3 $main_py "$@"`) would sidestep the
runfiles tree for the handful of rules_python tools LLVM uses at
build time. Not landed.

## Dependencies and ordering

**Status:** Open. Blocks `@llvm-project//clang:clang` after the
genrule fix.

**Observed.** rules_python ≥ 1.9.0 defines py_binary via a rule
builder in `py_executable.bzl` that passes
`cfg = dict(implementation=..., inputs=..., outputs=...)` inline
to `rule()`. Kuro creates a `Transition` object for the inline
`dict`, assigns it `TransitionId::MagicObject { path, name =
"_anonymous_transition" }`, and stores the path as "whatever bzl
was evaluating when `rule()` was called" — i.e.
`py_binary_rule.bzl` (the file where `.build()` ultimately runs).

When DICE later fetches the transition, it loads the module at
`path` and looks up the global `_anonymous_transition`. That global
does not exist: the transition is an inline argument to `rule()`,
never bound to a module-level name.

Result: `Transition object not found by id MagicObject { path:
…/py_binary_rule.bzl, name: "_anonymous_transition" }`. The dep
chain failing is `clang:clang → clang-driver → … → analysis →
analysis_htmllogger_gen → bundle_resources (py_binary)`.

**Investigation angles.**
1. Store the frozen `Transition` value directly in the
   `FrozenStarlarkRuleCallable` that owns it, so `fetch_transition`
   can bypass the module-lookup step for anonymous transitions. Keep
   the `MagicObject` id for named transitions (backwards compat).
2. Inject the frozen `Transition` as a module-level global at freeze
   time with the `_anonymous_transition` name, so the existing
   `get_any_visibility` lookup succeeds. Downside: mutating module
   globals post-evaluation is awkward in starlark-rust.
3. Treat anonymous rule-cfg transitions as no-ops (identity transition)
   and skip the whole fetch path. Correct for kuro today because we
   don't implement Starlark-driven config transitions anyway — the
   dict's `implementation` is never invoked. Simpler; possibly the
   right long-term answer until transitions are actually implemented.

**Recommended direction.** Option 3. Kuro doesn't execute
Starlark transitions, so honouring the anonymous dict as a no-op
matches what happens for every other transition (they're all no-ops
right now). Revisit when real transition support lands.

**Parity source.** Bazel's `StarlarkRuleClassFunctions.rule()`
stores the `StarlarkDefinedConfigTransition` inline on the `Rule`
class; lookup is by direct reference, not by module path + global
name. Kuro's detour through `(path, name)` is a kuro-specific
approximation that doesn't handle inline definitions.

## Bzlmod Load Labels and Apparent Repository Names

**Status:** fixed for the current zeromatter blocker on 2026-05-09.

**Observed.** A generated rules_rs git crate repo contained:

```
load("@crates__ts-rs-12.0.1//:crate.bzl", "crate")
```

Kuro resolved that load to the canonical repo
`rules_rs+crate+crates__ts-rs-12.0.1`, then rejected the apparent
spelling with the Buck-era "should use the canonical name" guard.
Bazel accepts this pattern because single-`@` repo names are apparent
names interpreted in the context repo's repository mapping.

**Parity source.**

- `Label.parseWithPackageContext` rewrites a present `@repo` through
  `packageContext.repoMapping()`.
- `BzlLoadFunction.getRepositoryMapping()` uses
  `RepositoryMappingValue.key(repoName)` for `.bzl` loads.
- `ModuleExtensionRepoMappingEntriesFunction` gives a repo generated
  by a module extension mappings for all repos generated by the same
  extension, keyed by their internal names, plus mappings visible to
  the module hosting the extension.

**Kuro fix.** In bzlmod load resolution, equivalent apparent/canonical
cells are now accepted, and the resolved `CellPath` is rewritten to
the reformed canonical path before constructing the load module path.
This preserves canonical `.bzl` identity while allowing Bazel-valid
apparent load labels from generated external BUILD files.

**2026-05-09 zstd label follow-up.** The first smoke read of
`/tmp/plan66-label-shorthand-zstd-1.log` was incomplete. The corrected
blocker was not just `zstd+1.5.7//:zstd+1.5.7`: the run eventually
failed coercing `crates__zstd-sys-2.0.16-zstd.1.5.7` deps because the
already materialized generated BUILD contained `deps = ["@@zstd//:"]`.
The source include file had the Bazel-valid shorthand `deps = ["@zstd"]`,
and the regenerated zeromatter lockfile entry now stores
`"@@zstd//:zstd"`. Bazel 9.1.0 repros confirm bare `@zstd` resolves to
the repo-root target named `zstd`, while explicit `@zstd//:` is an
invalid empty-target label.

Kuro now has focused repository-rule attr coverage for bare repo-label
canonicalization, extension repo successes write spec-hashed complete
markers, legacy `complete` markers remain accepted to avoid a global
crate-repo rerun, and legacy generated BUILD files with quoted
empty-target labels are repaired from current RepoSpec label attrs and
restamped. The bounded smoke
`/tmp/plan67-zstd-spec-hash-2.log` did not reach zstd because an earlier
over-broad invalidation attempt contaminated the shared zeromatter
`bazel-external` tree with stubbed crate repos; the observed failure is
now `crates__clap-4.5.60//:clap` resolving to a stub with zero targets.
The next narrow frontier is repository_ctx Label-tool materialization
for use_repo_rule-generated tools such as `@toml2json_linux_amd64`, plus
restoring/cleaning stale stubbed crate repos before using
`//sdk:sdk_contents` as a zstd signal again.

2026-05-09 update: Plan 36 follow-up 10 resolved that Label-tool
materialization frontier by registering precomputed `use_repo_rule()` repos
in the dynamic extension-cell registry. `/tmp/plan68-label-tool-2.log`
materialized `rules_rs+http_file+toml2json_linux_amd64/file/downloaded`,
advanced past `crates__clap-4.5.60//:clap`, and timed out later in the
already-tracked `rules_rust//ffi/rs:empty_allocator_libraries` toolchain
analysis wait.

2026-05-09 follow-up: the current dirty checkout already contained the
Plan 15 C++ toolchain cycle breaker, so a fresh bounded smoke no longer
reproduced the old `empty_allocator_libraries` timeout. It instead exposed
that startup lockfile spoke pre-seeding bypassed the existing invalid
empty-target-label cache guard: `MODULE.bazel.lock` still had
`deps = ["@@zstd//:"]` for
`crates__zstd-sys-2.0.16-zstd.1.5.7`, so Kuro pre-registered a bad
RepoSpec even though `Lockfile::get_extension_cache` would have rejected
the same cache. `app/kuro_bzlmod/src/pending_repo_cells.rs` now
canonicalizes lockfile spoke specs first and skips pre-seeding an extension
when any cached generated RepoSpec contains an invalid empty-target label.
That leaves the extension unseeded so the normal extension execution path
can regenerate the current RepoSpec from source. Focused verification:
`cargo test -p kuro_bzlmod invalid_empty_target_label -- --nocapture`,
`cargo build -p kuro`.

Bounded zeromatter smoke:
`/tmp/plan15-lockfile-preseed-zstd-1.log` with isolation
`plan15-lockfile-preseed-zstd-1` advanced past the previous zstd label
coercion. The materialized
`rules_rs+crate+crates__zstd-sys-2.0.16-zstd.1.5.7/BUILD.bazel` now has
`deps = ["@@zstd//:zstd"]` and a spec-hashed complete marker. The next
blocker is the existing Plan 54 class, now at `zstd//:zstd`:
`cc_common.create_linking_context_from_compilation_outputs` wraps
`cc_linking_outputs.library_to_link` in a depset and Kuro reports
`depset elements must not be mutable values`.

2026-05-09 follow-up: Plan 54 fixed the `LibraryToLink` mutable field shape by
recursively normalizing dicts in `_cc_internal.freeze` alongside the existing
list/tuple normalization. Focused cc_common/depset checks and
`cargo build -p kuro` passed. A fresh bounded zeromatter smoke from
`/var/mnt/dev/zeromatter`:

```sh
timeout 180s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/target/debug/kuro \
    --isolation-dir plan54-library-dict-freeze-1 \
    build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan54-library-dict-freeze-1.log
```

The old `zstd//:zstd`
`cc_linking_outputs.library_to_link` depset mutability failure did not recur.
The run reached `zeromatter//sdk:sdk_contents` analysis and timed out at the
already-tracked `rules_rust//ffi/rs:empty_allocator_libraries` analysis wait.
It also logged a non-terminal `llvm+llvm_source+llvm-raw`
`http_bsdtar_archive` `rctx.execute([rctx.path(host_bsdtar)] + args)` `No such
file or directory` repository-rule failure before creating a stub.

2026-05-09 follow-up: investigated the
`rules_rust//ffi/rs:empty_allocator_libraries` wait. The rule comes from
`rules_rust`'s `rust_allocator_libraries`; it requests the Rust toolchain and
an optional C++ toolchain via
`config_common.toolchain_type("@bazel_tools//tools/cpp:toolchain_type",
mandatory = False)`. Bazel 9's `ToolchainTypeRequirement` defaults
`mandatory` to true, preserves explicit optional requirements, and
`ResolvedToolchainContext` only rejects missing mandatory toolchains. Kuro was
using any unresolved toolchain, including optional misses, as a reason to load
the deferred toolchain pool and retry resolution. `resolve_toolchain_types()`
now retries only for a first-pass error, a missing exec group result, or a
missing mandatory requested type.

Focused verification passed:

```sh
cargo fmt -- app/kuro_analysis/src/analysis/env.rs
cargo test -p kuro_analysis deferred_retry --lib
cargo check -p kuro_analysis
cargo build -p kuro
```

A fresh bounded zeromatter smoke from `/var/mnt/dev/zeromatter`:

```sh
timeout 180s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/target/debug/kuro \
    --isolation-dir plan15-optional-toolchain-retry-1 \
    build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan15-optional-toolchain-retry-1.log
```

The build still timed out before completing `//sdk:sdk_contents`. The visible
frontier remained `rules_rust//ffi/rs:empty_allocator_libraries`, now with 6
other actions, and every observed `empty_allocator_libraries` configured target
still stopped at `toolchain_resolution_start` without reaching
`toolchain_resolution_complete`. No `analysis_starlark_eval_heartbeat` or
`analysis_starlark_call_sample` appeared for the stuck rule, so the wait is not
inside the Starlark rule implementation. The earlier LLVM `http_bsdtar_archive`
`No such file or directory` side signal did not recur in this smoke. Next slice:
add narrow checkpoints inside `resolve_toolchain_types()` around label
canonicalization, first multi-group resolution, deferred-load retry decision,
`ensure_deferred_toolchains_loaded`, and the retry resolution pass.

2026-05-09 follow-up: Plan 54's hashable dict-shaped `_cc_internal.freeze`
advanced the zeromatter smoke beyond the previous
`create_library_to_link.bzl:106 Object of type tuple has no attribute keys`
failure. Fresh bounded smoke from `/var/mnt/dev/zeromatter`:

```sh
timeout 220s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep 'kurod\[zeromatter\].*plan54-hashable-dict-freeze-1' \
    -- \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan54-hashable-dict-freeze-1 \
      build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan54-hashable-dict-freeze-1.log
```

The command exited with Kuro status `3` after 179s, peak RSS 771808 KiB.
`//sdk:sdk_contents` reached analysis and failed through
`rules_rust+0.69.0//ffi/rs:empty_allocator_libraries`: the mandatory Rust
toolchain impl analyzed far enough to enter
`rules_rust+0.69.0/rust/private/rust_allocator_libraries.bzl`, where line 118
builds depsets of `_ltl(...)` results. `_ltl` calls
`cc_common.create_library_to_link(static_library = library,
pic_static_library = library)`, and Kuro still reports
`depset elements must not be mutable values` for those direct
`LibraryToLink` provider elements. This supersedes the prior
toolchain-resolution wait as the current concrete frontier; continue in Plan 54
by finding the remaining non-hashable `LibraryToLinkInfo` field without
weakening depset validation.

2026-05-10 follow-up: fixed the next `_cc_internal.freeze` frozen-list
interop blocker. `rules_cc`'s `cc_static_library.bzl` called
`depset(lib.pic_objects)`, where `lib.pic_objects` was Kuro's hashable
list-shaped cc_common frozen-list wrapper. It reported `type = "list"`, but
the `depset()` Rust signature still only accepted native Starlark list/tuple
via `UnpackListOrTuple`, so the call failed before depset construction with:
`Type of parameter direct doesn't match, expected None | list | tuple, actual
list`.

The narrow fix keeps depset validation intact and does not accept arbitrary
iterables: `depset()` now uses a depset-local unpacker that accepts native
list/tuple first, then only the cc_common frozen-list wrapper. The same wrapper
is accepted for `transitive`, so `depset(transitive =
cc_internal.freeze([depset([...])]))` works without making raw mutable lists
depset-hashable or changing transitive-set streaming behavior.

Focused verification passed:

```sh
cargo fmt -- app/kuro_build_api/src/interpreter/rule_defs/depset.rs \
  app/kuro_build_api/src/interpreter/rule_defs/cc_common/actions.rs \
  app/kuro_build_api/src/interpreter/rule_defs/cc_common/mod.rs \
  app/kuro_build_api_tests/src/interpreter/rule_defs/cc_common.rs
cargo test -p kuro_build_api_tests interpreter::rule_defs::cc_common -- --nocapture
cargo test -p kuro_build_api_tests interpreter::rule_defs::depset -- --nocapture
cargo build -p kuro
```

A bounded zeromatter smoke from `/var/mnt/dev/zeromatter`:

```sh
timeout 220s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep 'kurod\[zeromatter\].*plan15-depset-frozen-list-direct-1' \
    -- \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan15-depset-frozen-list-direct-1 \
      build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan15-depset-frozen-list-direct-1.log
```

The old `cc_static_library.bzl:46 depset(lib.pic_objects)` parameter-type
failure did not recur. The smoke failed later through the same glibc
`c_nonshared` static-library path at `cc_static_library.bzl:174`, where
`actions.args().add_all(linker_inputs, map_each = map_each)` rejected a
list-shaped frozen value: `Expected ... str | list | RunInfo, but got list
(repr: [])`. Next slice: keep the fix narrow and make command-line/list
unpacking accept the cc_common frozen-list wrapper as a list-shaped value in
the `cmd_args.add_all(map_each=...)` path, without accepting arbitrary
iterables or changing raw mutable-list depset eligibility.

2026-05-09 follow-up: implemented that narrow command-line boundary fix in
`cmd_args` map_each sequence expansion. `append_map_each_result` now expands
the cc_common frozen-list wrapper in the same place it already expands built-in
list and tuple returns. This keeps direct command-line value validation intact
and does not broaden depset element hashability or TransitiveSet streaming.
Focused verification:

```sh
cargo fmt -- app/kuro_build_api/src/interpreter/rule_defs/cmd_args/typ.rs \
  app/kuro_build_api_tests/src/interpreter/rule_defs/cmd_args/tests.rs
cargo test -p kuro_build_api_tests map_each_sequence_returns_expand_as_items -- --nocapture
cargo test -p kuro_build_api_tests cc_common -- --nocapture
cargo test -p kuro_build_api_tests depset -- --nocapture
```

Next: rebuild and rerun a bounded zeromatter smoke. The expected signal is that
the `cc_static_library.bzl:174` `actions.args().add_all(..., map_each=...)`
failure is gone; continue from the next terminal `//sdk:sdk_contents` blocker.

2026-05-09 smoke follow-up: `/tmp/plan15-cmdargs-frozen-list-1.log` confirmed
the `cc_static_library.bzl:174` command-line frozen-list failure is gone. The
next terminal blocker is symbolic macro inherited-attribute parity in
`llvm+0.7.0//runtimes:BUILD.bazel`: `copy_to_resource_directory(...)` omits
the inherited `target_triple` attr, and Kuro reports a missing implementation
parameter. This is now tracked in
[55-symbolic-macro-inherit-attrs.md](55-symbolic-macro-inherit-attrs.md).

2026-05-10 follow-up: after Plan 55 removed the symbolic macro inherited-attr
blocker, the next visible SDK frontier was again the C++ toolchain-provider
cycle around `bazel_tools//tools/cpp:malloc`. Focused probes showed `malloc`
completed toolchain resolution, then stalled trying to analyze the selected C++
toolchain impl only to populate `ctx.toolchains`. The narrower active-key and
configured dependency-closure cycle breakers were racy because the first waiter
can reach `ctx_toolchain_provider_analysis_start` before the selected toolchain
impl analysis key is active, and because runtime-library edges cross
configurations.

Kuro now uses the existing C++ NativeShim provider facade at the
`ctx.toolchains` provider boundary for
`@bazel_tools//tools/cpp:toolchain_type`, instead of recursively analyzing the
C++ toolchain implementation there. This keeps toolchain resolution intact and
limits the shortcut to the C++ provider surface Kuro already constructs
intrinsically.
`app/kuro_analysis/src/analysis/calculation.rs` also drops the now-unused
active-analysis-key lookup helper while retaining the checkpoint count.

Terminology follow-up: this area must use `Native`, `Intrinsic`, or
`NativeShim` naming. The current implementation still has legacy C++ provider
type/function/checkpoint names from the earlier wording; rename those as part
of the systemic NativeShim work in
[56-native-intrinsic-provider-shims.md](56-native-intrinsic-provider-shims.md).
That work is broader than C++: every Bazel-native provider/API surface that Kuro
exposes through Starlark must be inventoried and modeled as an explicit
NativeShim boundary instead of an ad hoc per-label escape hatch.

Focused verification:

```sh
cargo fmt -- app/kuro_analysis/src/analysis/env.rs app/kuro_analysis/src/analysis/calculation.rs
cargo check -p kuro_analysis
cargo build -p kuro
timeout 90s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/target/debug/kuro \
    --isolation-dir plan15-cpp-toolchain-native-shim-all-1 \
    build bazel_tools//tools/cpp:malloc \
  2>&1 | tee /tmp/plan15-cpp-toolchain-native-shim-all-1.log
```

The focused build succeeded. The C++ NativeShim checkpoint should report
`status=cc_toolchain_native_shim` after the terminology cleanup, followed by
`analysis_key_complete` and `BUILD SUCCEEDED`.

Bounded SDK smoke:

```sh
timeout 260s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep 'kurod\[zeromatter\].*plan15-cpp-toolchain-native-shim-sdk-1' \
    -- \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan15-cpp-toolchain-native-shim-sdk-1 \
      build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan15-cpp-toolchain-native-shim-sdk-1.log
```

The SDK smoke advanced past the old `bazel_tools//tools/cpp:malloc` wait and
failed later in `rules_rust//util/process_wrapper:process_wrapper`:

```text
bazel-external/rules_rust+0.69.0/rust/private/rustc.bzl:1374
deps = depset(deps)
error: depset elements must not be mutable values
```

This is a Plan 54-class depset/frozen-value boundary. Continue there by
identifying which `rustc_compile_action` dependency value remains mutable and
fixing the systemic freezing/hashability path without weakening depset
validation or changing TransitiveSet streaming behavior.

2026-05-10 follow-up: Plan 56 C++ provider hashability and Plan 15
`ctx.attr` source-file target parity removed the `rustc.bzl:1374
depset(deps)` mutable-value failure. A focused fixture confirms
`attr.label_list(..., allow_files=True)` exposes both source files and rule deps
as depset-eligible `Target` values, with source-file `DefaultInfo.files` and
`ctx.files` projecting back to the underlying files.

The next SDK smoke from `/var/mnt/dev/zeromatter`:

```sh
timeout 420s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep 'kurod\[zeromatter\].*plan15-target-name-punctuation-zeromatter-1' \
    -- \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan15-target-name-punctuation-zeromatter-1 \
      build //sdk:sdk_contents
```

timed out rather than failing. The old source-file `Target`/`File` mismatch and
the rules_rust generated source path
`src/output_tests/expected/into_bytes_enum.repr(C).expected.rs` target-name
validation failure did not recur. The run was still making analysis progress
near `crates__arrow-54.3.1//:arrow`, `zeromatter//sdk/zeromatter_ffi`, and related
AWS/Arrow/Rust dependency targets; last sampled total RSS was ~873 MiB and max
Kuro checkpoint RSS was ~815 MiB. The smoke wrapper killed the
`kurod[zeromatter]` daemon after timeout, and no `kurod[...]` process
remained.

2026-05-11 follow-up: after Plan 57 module-extension fact reuse, the SDK
smoke reached action execution and failed linking `//zm_cli:zm` for the
linux-musl transition with `ld.lld: error: undefined symbol:
__isoc23_sscanf` from `aws-lc-sys` C objects. A Bazel 9 parity probe in the
same `../reactor-repo-kuro` checkout succeeds for `bazel build --config=linux
//zm_cli:zm`, and its `aws-lc-sys` build-script log shows musl-targeted
`CC`/`CXX` plus `CFLAGS`/`CXXFLAGS` containing `-target x86_64-linux-musl`,
`-nostdlibinc`, musl/kernel/compiler-rt include paths, and LLVM resource
headers. Kuro's generated `_bs.env` for the same crate was empty, so this is a
Plan 15 `cc_common.create_compile_variables` /
`get_memory_inefficient_command_line` toolchain-environment parity gap, not a
module-extension or lockfile issue.

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
