# Plan 58: Rule-based C++ toolchain feature parity

## Goal

Evaluate Bazel/rules_cc C++ toolchain features from the toolchain's declared
configuration instead of relying on Kuro's current native approximation.

The immediate parity failure is `../zeromatter-kuro`'s `//sdk:sdk_contents`:
Kuro builds the target, but Rust link actions miss LLVM toolchain-declared
arguments and data inputs such as `resource_dir`, CRT search directories, musl
runtime search paths, and libc++/libunwind archives. The broader failure class is
any rule-based `cc_args` or action config whose semantics are dropped because
Kuro only preserves shallow feature/action names.

## Source of truth

- Bazel 9 C++ feature expansion:
  `src/main/java/com/google/devtools/build/lib/rules/cpp/`
- Bazel toolchain config APIs:
  `src/main/java/com/google/devtools/build/lib/rules/cpp/CcModule.java`
  and `CcToolchainFeatures.java`
- `rules_cc` rule-based toolchain conversion:
  `@rules_cc//cc/toolchains:args.bzl`,
  `@rules_cc//cc/toolchains/impl:legacy_converter.bzl`, and
  `@rules_cc//cc/toolchains/impl:toolchain_config_info.bzl`
- Zeromatter SDK LLVM toolchain declarations:
  `@llvm//toolchain/args:BUILD.bazel`,
  `@llvm//toolchain/args/linux:BUILD.bazel`, and runtime targets under
  `@llvm//runtimes/...`

## Current Kuro Gap

- `app/kuro_build_api/src/interpreter/rule_defs/cc_common/feature_config.rs`
  stores enabled feature names but not the configured feature graph.
- `app/kuro_build_api/src/interpreter/rule_defs/cc_common/actions.rs`
  extracts feature/action names from `CcToolchainConfigInfo` but discards
  `flag_sets`, `env_sets`, action-config tools, tool runfiles, artifact name
  patterns, make variables, and action file deps.
- `get_memory_inefficient_command_line` hardcodes a partial Linux/LLVM link
  command. This hides missing generic feature expansion and will keep drifting
  as soon as another toolchain declares different `cc_args`.
- `CcToolchainInfo` shims expose enough fields for some rules to analyze, but do
  not preserve the action-specific `files` deps that rule-based toolchains
  compute for `cc_args(data = ...)`.
- Kuro physical output paths (`buck-out/...`) leak into Rust debug/source data
  where Bazel uses `bazel-out/...` exec paths.

### 2026-05-11 iteration: feature `env_set` preservation

Latest state rediscovery found a clean worktree at `c2ae8fb9`; the latest
bounded SDK smoke log present locally,
`/tmp/plan57-sdk-contents-dedup-ext-retry-20260511-120704.log`, reaches action
execution and succeeds. The older visible frontier remains the Plan 15
`aws-lc-sys` musl build-script/link gap: Bazel derives toolchain C/C++ build
environments and Kuro has historically returned an empty `_bs.env`.

Class boundary for this iteration:

- Missing Bazel semantic: feature-declared `env_sets` and `env_entries` from
  `CcToolchainConfigInfo` are not preserved in `CcToolchainFeatures` and are
  never expanded by `cc_common.get_environment_variables`.
- Owning subsystem: `cc_common` feature configuration, specifically the
  `cc_toolchain_features` conversion and `FeatureConfiguration` action-env
  query path.
- Broader failure class: any rule/toolchain that uses C++ features to set
  action environment variables such as `CC`, `CXX`, `CFLAGS`, SDK roots, or
  compiler wrapper state will silently lose those variables even if the feature
  is enabled.
- Rejected symptom fixes: hardcoding rules_rust, LLVM, zeromatter labels,
  specific env var names, generated output hashes, or observed SDK paths.

This iteration intentionally takes the narrow systemic first slice: preserve
rule-based feature `env_set` data, select enabled feature env sets during
configuration, and expand scalar `%{variable}` substitutions for matching
actions. Full flag-set expansion, `cc_args(data = ...)` inputs, and action-config
tool/runfiles remain subsequent Plan 58 slices.

Verification for this slice:

- `cargo test -p kuro_build_api_tests cc_common_feature_env_sets_expand_for_matching_action -- --nocapture`
- `cargo test -p kuro_build_api_tests interpreter::rule_defs::cc_common -- --nocapture`
- `cargo test -p kuro_build_api feature_config -- --nocapture`
- `cargo check -p kuro`
- `cargo build -p kuro`
- `git diff --check`
- `/run/host/var/mnt/dev/zeromatter-kuro`: bounded SDK smoke
  `plan58-envsets-sdk-20260511-124517`, log
  `/tmp/plan58-envsets-sdk-20260511-124517.log`, status 0,
  `BUILD SUCCEEDED`.

## Failure Class

A toolchain argument is wrong whenever it depends on any of these Bazel/rules_cc
features:

- `cc_args(data = ...)` requiring generated files or directories as action inputs.
- `cc_args(format = {...})` replacing placeholders with artifact or variable
  exec paths.
- `requires_not_none`, `requires_none`, `requires_true`, `requires_false`,
  `requires_equal`, or `requires_any_of`.
- `iterate_over` expansion for list-valued variables such as library search
  directories.
- Nested argument groups and ordered action-config flag sets.
- Action-config tool selection and tool runfiles.
- Toolchain `env_sets`.
- Toolchain-provided artifact name patterns or make variables.

The SDK mismatch is one visible instance, not the bug boundary.

## Implementation

### 2026-05-11 output-mode normalization slice

The latest `../zeromatter-kuro` SDK smokes build `//sdk:sdk_contents` under
Kuro, so the active Plan 58 frontier is output parity rather than a terminal
analysis failure. Existing manifests show Bazel's staged SDK tree materialized
without owner/group/other write bits (`555` for the current files and
directories), while Kuro leaves local action outputs writable (`755`). This is
not an SDK rule issue: it is local action output materialization/metadata
behavior. The systemic boundary is Kuro's local executor after a successful
action has produced declared outputs and before those outputs are observed by
downstream materialization or final build reports.

Class boundary:

- Missing Bazel semantic: local action outputs materialized in the output tree
  are read-only after action completion, while preserving executable bits.
- Owning subsystem: local executor output collection plus filesystem cleanup
  for stale outputs.
- Other affected cases: any tree artifact or file action output whose mode is
  compared, archived, copied, or used as an SDK/package input.
- One-off workaround rejected: chmodding `//sdk:sdk_contents` after the build
  or special-casing SDK output paths.

Patch classification: systemic parity fix. Normalize local action output paths
before output hashing/metadata capture so Kuro records the same `555` modes
that Bazel exposes in the SDK tree, and make cleanup robust by allowing removal
of previously read-only output directories.

Verification for this slice:

- `cargo test -p kuro_execute_impl test_normalize_local_output_permissions_sets_bazel_output_mode -- --nocapture`
- `cargo test -p kuro_fs remove_all_readonly_dir -- --nocapture`
- `cargo test -p kuro_build_api_tests cc_common_feature_env_sets_expand_for_matching_action -- --nocapture`
- `cargo check -p kuro_build_api -p kuro_execute_impl -p kuro_fs`
- `cargo build -p kuro`
- `/run/host/var/mnt/dev/zeromatter-kuro`: 30-minute-bounded SDK smoke
  `plan58-outputmode30-sdk-20260511-133024`, log
  `/tmp/plan58-outputmode30-sdk-20260511-133024.log`, status 0,
  `BUILD SUCCEEDED` in `9m39s`, peak RSS `2396208 KiB`.
- SDK manifest compare against `/tmp/bazel-sdk-contents-manifest3.txt`:
  559 entries on both sides, no diff. Remaining content hash diff is limited
  to `./bin/zerobuf`, `./bin/zerosystem`, `./bin/zm`, and
  `./lib/libzeromatter_ffi.so`.

### 2026-05-11 iteration: legacy flag-set preservation

Hash investigation of the remaining four SDK executables showed the files are
not merely debug-path different. The three binaries are static PIEs with no
dynamic `NEEDED` entries; `libzeromatter_ffi.so` links against different runtime
libraries. Bazel's Rust final-link params include toolchain-declared LLVM args
such as `resource_dir`, CRT search directories, musl/glibc search directories,
and libc++/libc++abi/libunwind runtime archive/search args. Kuro's params only
contained the old partial hardcoded Linux LLVM defaults.

Class boundary for this iteration:

- Missing Bazel semantic: legacy `action_config.flag_sets` and
  `feature.flag_sets` produced by rules_cc's rule-based toolchain converter are
  discarded by `cc_common_internal.cc_toolchain_features()`.
- Owning subsystem: `cc_common` feature configuration and
  `get_memory_inefficient_command_line` flag expansion.
- Other affected cases: any rule-based `cc_args` or nested arg group using
  action-scoped flags, `with_features`, `%{variable}` substitutions, or
  `iterate_over` variables.
- One-off workaround rejected: adding SDK-specific LLVM flags or target labels
  directly to Rust link actions.

Patch classification: systemic parity fix. Preserve legacy flag groups from
toolchain action configs and enabled features, then expand matching action
configs before feature flag sets using Bazel-shaped conditions and variable
substitution. `cc_args(data = ...)` action inputs and logical-vs-physical path
mapping remain separate Plan 58 slices.

Follow-up smoke `plan58-modernflagsets-sdk-20260511-152514` still left the four
content hashes different. Its Rust final-link params showed only Kuro's old
hardcoded partial Linux LLVM defaults, not the rule-based `resource_dir`, CRT,
glibc/musl search path, or libc++/libunwind args. The immediate source is not
the link action itself: rules_rust obtains its `FeatureConfiguration` from
public `cc_common.configure_features(ctx, cc_toolchain, ...)`, and Kuro's public
implementation currently ignores `cc_toolchain._toolchain_features`, returning
a fresh name-only configuration. The previously preserved flag sets therefore
never reach `get_memory_inefficient_command_line` for Rust final links.

Additional patch classification: systemic parity fix. Public
`cc_common.configure_features` must derive its configuration from the selected
`CcToolchainInfo`'s `_toolchain_features` when present, applying requested and
unsupported features to that toolchain-owned feature graph. Falling back to a
name-only configuration remains only for incomplete native shim cases.

Follow-up smoke `plan58-modernprefer-sdk-20260511-160348` proved the public
configuration handoff was necessary but not sufficient. The SDK still built
within the corrected 30-minute bound, but Rust final-link params still lacked
the path-backed LLVM runtime args. The source is the C++ toolchain lookup path:
Kuro's analysis layer unconditionally returns `CcToolchainInfoNativeShim` for
resolved C++ toolchains, so rules_rust receives an empty `_toolchain_features`
object even when the real rule-based `cc_toolchain` target is available.

Class boundary for the next slice:

- Missing Bazel semantic: `ctx.toolchains["@bazel_tools//tools/cpp:toolchain_type"]`
  should expose the analyzed toolchain implementation's real
  `CcToolchainInfo`; the native shim is only a cycle breaker for C++ toolchain
  self-dependencies or absent optional toolchains.
- Owning subsystem: analysis-time toolchain provider materialization in
  `app/kuro_analysis/src/analysis/env.rs`.
- Other affected cases: any rule using toolchain resolution to inspect C++
  toolchain features, action files, make variables, or tool paths will see
  empty shim data instead of the selected toolchain provider.
- Rejected workaround: teaching rules_rust/Rust link actions to reconstruct
  LLVM runtime paths from labels or host paths.

Patch classification: systemic parity fix. Analyze the real resolved C++
toolchain implementation when it is not the target currently being analyzed,
and retain the native shim for the self-dependency case it was introduced to
break.

Attempted verification with that patch, `plan58-realcc-sdk-20260511-232124`,
did not fail fast but also did not progress: after discovering LLVM runtime and
startfile targets, Kuro stayed in analysis on
`rules_rust//ffi/rs:empty_allocator_libraries` with the same 39 other actions
for more than six minutes. The run was stopped manually rather than waiting for
the 30-minute error bound. This proves the native shim is guarding more than a
literal self-edge; analyzing the full C++ toolchain implementation from a Rust
rule opens the toolchain/runtime dependency cone and can cycle back through
rules_rust.

Revised boundary:

- Keep the native C++ toolchain provider shim for `ctx.toolchains` to preserve
  the existing cycle break.
- Enrich the shim, or the data passed into it, with `CcToolchainFeatures`
  produced from the selected toolchain's `CcToolchainConfigInfo` without
  analyzing the full `cc_toolchain` implementation target and its runtime deps.
- This should source metadata from the rule-based `cc_toolchain_config` target
  or from toolchain registration metadata, not from SDK labels or reconstructed
  host paths.

Current narrower patch:

- While loading registered `toolchain()` packages, record a rule-based C++
  implementation's generated `toolchain_config` target when that target is
  visible in the same package.
- During C++ toolchain provider materialization, keep returning
  `CcToolchainInfoNativeShim`, but analyze only the recorded
  `cc_toolchain_config` target and attach its native `CcToolchainConfigInfo` to
  the shim.
- Build `_toolchain_features` lazily from that attached config provider when
  Starlark reads the shim field.

Follow-up smoke `plan58-modulemaptype-sdk-20260511-233921` advanced through
rules_cc module-map collection but failed while expanding Rust final-link C++
flags:

```
Cannot expand C++ toolchain variable
'buck-out/.../runtimes/libcxx/libcxx_library_search_directory'
```

The new failure class is in the rule-based metadata extractor, not in
rules_rust or the SDK. `cc_args(format = {"name": label})` has two distinct
meanings after rules_cc conversion:

- labels for declared C++ variables remain build-variable references, for
  example `@rules_cc//cc/toolchains/variables:runtime_library_search_directories`
  becomes `%{runtime_library_search_directories}`;
- labels for toolchain data/directories become literal exec paths, for example
  LLVM's `//runtimes/libcxx:libcxx_library_search_directory` is substituted
  directly into `-L{libcxx_library_search_path}`.

Kuro's metadata extractor currently converts every label substitution into a
`%{...}` placeholder, so path labels become impossible variable names. The
systemic fix is to preserve this distinction while extracting rule-based
`cc_args`: C++ variable labels stay placeholders, and non-variable labels render
as metadata exec paths. SDK-specific labels or Rust-link hardcoding remain
rejected.

Follow-up smoke `plan58-formatliteral-sdk-20260511-234347` passed the
literal-path substitution failure and then failed during C++ header parsing for
`@zstd//:zstd`:

```
Artifact must be bound by now ... Artifact:
(zstd+1.5.7//:zstd ...)/_objs/cc_library-compile/zdict.h.h
```

This is a new systemic action-registration boundary exposed by enabling the
toolchain's real header-module/header-parsing feature graph. rules_cc declares
generated/processed header parse outputs and passes those output files to
`create_cc_compile_action`; Kuro's native action bridge currently places output
artifact objects in the command line (`-o <output artifact>`, `-MF <depfile
artifact>`) as well as in the `outputs` list. Bazel command lines need the
output exec path string there, while ownership/binding belongs only in the
`outputs` parameter. The fix should render output artifacts as paths in compile
arguments and keep `.as_output()` only for the declared outputs list. Do not
disable header parsing/modules, skip zstd, or special-case `.h.h` names.

Follow-up smoke `plan58-outputpathargs-sdk-20260511-234846` confirmed the
output artifact/input problem was hiding a second bridge issue: the run action
category derived from Bazel's `c++-header-parsing` action name became
`c++_header_parsing`, which Kuro rejects because action categories must be
snake-case identifiers. The missing semantic is generic normalization from
Bazel C++ action names to Kuro action categories for action registration only;
feature matching must keep using Bazel action names. The fix should convert
`c++` to a valid token such as `cpp` and hyphens to underscores when naming the
Kuro action category, without changing feature-configuration action matching.

This narrower patch still used normal analysis for the `cc_toolchain_config`
target. The smoke `plan58-shimconfig-sdk-20260512-044916` showed why that is
too broad: rules_cc's `cc_toolchain_config` returns
`DefaultInfo(files = depset(transitive = toolchain_config.files.values()))`, so
normal analysis expands all `cc_args(data = ...)` runtime deps and reopens the
`rules_rust//ffi/rs:empty_allocator_libraries` toolchain cycle/stall.

Next implementation slice:

- Missing Bazel semantic: rule-based C++ toolchain metadata is provider data,
  but Kuro currently requires normal target analysis to observe it.
- Owning subsystem: analysis-time C++ toolchain provider materialization and
  `cc_common` feature configuration.
- Broader failure class: any rule that reads C++ toolchain features through
  `ctx.toolchains` loses metadata whenever the selected toolchain must be
  represented by the cycle-breaking native shim.
- One-off workaround rejected: reconstructing LLVM flags from specific SDK or
  repository labels, or linking Rust targets directly against observed runtime
  paths.

Patch classification: systemic parity fix. Synthesize typed
`CcToolchainFeatures` from configured `cc_toolchain_config`, `cc_args`,
`cc_args_list`, `cc_feature`, `cc_feature_set`, and `cc_action_type(_set)`
attributes, without calling `get_analysis_result` for the config target. This
slice preserves flags and formatted path strings. The action-file deps behind
`cc_args(data = ...)` remain the next Plan 58 slice if the SDK link then lacks
materialized runtime directories.

Smoke `plan58-metadatafeatures-sdk-20260512-053542`, log
`/tmp/plan58-metadatafeatures-sdk-20260512-053542.log`, was stopped at status
143 after more than ten minutes. The log repeated the same stalled frontier as
the normal-analysis attempt:

`Waiting on rules_rust//ffi/rs:empty_allocator_libraries
(//bazel/platforms:linux-gnu-host#926e4d6bc2ea1c83) -- running analysis
[evaluate_rule], and 81 other actions`

RSS was flat around `852600 KiB`. This proves the first metadata walker was
still too broad: `get_configured_target_node` for `cc_toolchain_config`,
`cc_args`, or adjacent metadata labels configures dependency edges, including
`cc_args(data = ...)` runtime targets. The next slice must read unconfigured
`TargetNode`/`CoercedAttr` package data from interpreter results instead of
configured target nodes. Over-selecting all select branches is acceptable only
as an intermediate diagnostic; the systemic fix should eventually evaluate
selects without traversing deps.

Smoke `plan58-unconfiguredmetadata-sdk-20260512-055838`, log
`/tmp/plan58-unconfiguredmetadata-sdk-20260512-055838.log`, restored the healthy
SDK build path: status 0, `BUILD SUCCEEDED`, `Phases: load=2m50s
analyze=4m07s execute=4m04s materialize=4m04s total=8m15s`,
`memory_smoke_summary elapsed_s=508 peak_rss_kib=2656540 final_rss_kib=634216`.

The final Rust link params still lacked path-backed toolchain args
(`-resource-dir`, `-B{crt_objects_directory}`, runtime `-L...` search paths).
ELF linkage therefore remained mismatched for `lib/libzeromatter_ffi.so`:
Bazel needs `librt.so.1`, `libpthread.so.0`, `libm.so.6`, `libdl.so.2`, and
`libc.so.6`; Kuro still needs `libgcc_s.so.1`, `libm.so.6`, `libc.so.6`, and
`ld-linux-x86-64.so.2`.

Focused `uquery @llvm//toolchain:all --output-attribute '^args$'` showed why:
rule-based toolchain metadata attrs such as
`@llvm//toolchain:_bootstrap_linux_x86_64_cc_toolchain_config.args` are stored
as `CoercedAttr::Concat` containing `CoercedAttr::Selector` values. The
unconfigured extractor walked lists/dicts/labels but not select/concat nodes, so
it only saw direct args such as `ignore_unused_command_line_argument` and missed
the `:toolchain_args` graph. Next narrow fix: teach the metadata attr walkers to
traverse unconfigured select/concat values without resolving or configuring
their deps.

This keeps the cycle-breaking behavior for `static_runtime_lib()` and
`dynamic_runtime_lib()` while giving `cc_common.configure_features()` real
rule-based feature/arg metadata.

Smoke `plan58-selectmetadata-sdk-20260512-061019`, log
`/tmp/plan58-selectmetadata-sdk-20260512-061019.log`, failed quickly with status
3 after about 21 seconds, peak RSS `920276 KiB`. The failure was:

`The C++ toolchain 'llvm++toolchain+llvm_toolchains//:linux_x86_64_cc_toolchain'
unconditionally implies feature 'module_maps', which is unsupported by this
rule.`

This exposed two narrower metadata-shim bugs:

- The unconfigured extractor over-approximated `select()` by walking every arm.
  That is not Bazel semantics; mutually exclusive runtime/platform feature
  branches become unconditional if flattened together.
- The selected LLVM toolchain legitimately default-enables the `module_maps`
  feature in the complete Linux toolchain, but Bazel only treats it as supported
  when the toolchain's `module_map` attr is present. Kuro's native shim exposed
  an empty `_cc_info.compilation_context._module_map`, so rules_cc rejected the
  same feature set that Bazel accepts for the real toolchain.

Next slice classification: evaluate unconfigured metadata selects against the
current target configuration, falling back to the default branch instead of all
branches, and carry rule-based `cc_toolchain(module_map = ...)` presence into the
cycle-breaking shim. This is still a metadata/provider parity fix, not an SDK
label workaround.

Smoke `plan58-selectmodulemap-sdk-20260511-232142`, log
`/tmp/plan58-selectmodulemap-sdk-20260511-232142.log`, still failed with the
same `module_maps` unsupported error after 15 seconds, peak RSS `732756 KiB`.
The implementation carried a module-map field through the shim, but the
registration-time attr extractor only read direct label/string attrs. The
selected toolchain's module-map attr can arrive wrapped by Bazel-compatible attr
coercion (`OneOf`/selector forms), so the shim still saw no module map. The
fix remains in the same class: make metadata attr extraction preserve the
declared toolchain attr shape without analyzing the toolchain target.

Smoke `plan58-modulemapextract-sdk-20260511-232417`, log
`/tmp/plan58-modulemapextract-sdk-20260511-232417.log`, advanced past the
previous `module_maps` validation failure and failed 11 seconds in, peak RSS
`716916 KiB`, while analyzing `rules_cc//:link_extra_lib`.

New failure:

`cc_compilation_helper.bzl:196: at index 0 of transitive, got element of type
list, want depset`

This is still the native C++ shim provider-shape class. rules_cc's
`_collect_module_maps` appends each dependency's
`compilation_context._exporting_module_maps` into a `depset(transitive = ...)`;
Kuro's `EmptyCompilationContext` returned an empty list for that private field.
The next fix is to expose `_exporting_module_maps` as an empty depset, matching
the shape rules_cc expects, without enabling any target-specific module-map
behavior.

Smoke `plan58-modulemapdepset-sdk-20260511-232534`, log
`/tmp/plan58-modulemapdepset-sdk-20260511-232534.log`, advanced further and
failed while creating a `CppModuleMap` action:

`Missing parameter tree_expander for call to
_module_map_struct_to_module_map_content`

The source is a generic Bazel `Args.add_all(map_each = ...)` API gap, not a
module-map provider-shape issue. Bazel passes a tree-expander object to
`map_each` callbacks that declare the second parameter; Kuro always invokes
`map_each` with only the current item. rules_cc's module-map writer uses the
two-argument form to expand tree artifacts while still accepting one-argument
callbacks elsewhere. Next patch classification: systemic command-line args
parity fix. Support both one-argument and two-argument `map_each` callbacks for
`add_all` and `add_joined`, with a conservative tree-expander shim, without
changing SDK labels or module-map-specific behavior.

Smoke `plan58-mapexpander-sdk-20260511-233224`, log
`/tmp/plan58-mapexpander-sdk-20260511-233224.log`, failed after 15 seconds
while analyzing the Rust toolchain through `rules_rust`'s `find_cc_toolchain`.
The failure returned to:

`The C++ toolchain 'llvm++toolchain+llvm_toolchains//:linux_x86_64_cc_toolchain'
unconditionally implies feature 'module_maps', which is unsupported by this
rule.`

The source is now narrower than the earlier missing module-map provider shape.
rules_cc's `configure_features.bzl` computes `requested_features` by adding
default-enabled toolchain features except those listed in
`unsupported_features`; `rules_rust` lists `module_maps` as unsupported, so
Bazel does not request that feature. Kuro's
`CcToolchainFeatures.configure_features(requested_features=...)` shim then
incorrectly re-adds every `default_enabled_features` entry, making
`module_maps` enabled again. Next patch classification: feature-configuration
parity fix. Preserve default-enabled features for Kuro's public
`cc_common.configure_features(... unsupported_features=...)` path, but make the
rules_cc-facing `_toolchain_features.configure_features(requested_features=...)`
honor the already-filtered requested set and not re-enable unsupported defaults.

Smoke `plan58-requestedfeatures-sdk-20260511-233626`, log
`/tmp/plan58-requestedfeatures-sdk-20260511-233626.log`, passed the unsupported
`module_maps` validation and failed 15 seconds in while collecting dependency
module maps for `rules_rust//util/process_wrapper/private:bootstrap_process_wrapper`:

`cannot add an item of type 'CppModuleMap' to a depset of 'struct'`

This is another provider-shape mismatch. rules_cc's `create_module_map()`
returns its private `_ModuleMapInfo` provider instance, which Kuro's depset
typing currently reports as `struct`; Kuro's native toolchain module-map shim
reports a distinct `CppModuleMap` type. The direct module-map list and
transitive `_exporting_module_maps` depsets therefore have incompatible element
types. Next patch classification: native shim provider-shape parity. Make the
toolchain module-map shim present the same Starlark type shape as rules_cc's
module-map provider while preserving its `name` and `file` fields.

Verification after resume showed this narrower patch is still too broad. The
bounded smoke `plan58-shimconfig-sdk-20260512-044916`, log
`/tmp/plan58-shimconfig-sdk-20260512-044916.log`, was stopped after the active
analysis set stayed flat for nearly three minutes on
`rules_rust//ffi/rs:empty_allocator_libraries` plus 81 other actions. The log
shows Kuro had already configured LLVM runtime data targets such as
`//runtimes:resource_directory`, `//runtimes:crt_objects_directory`,
`//runtimes/glibc:*_shared_library`, and libcxx/libunwind search-directory
targets. That confirms analyzing the generated `cc_toolchain_config` target
still traverses `DefaultInfo`/`cc_args(data = ...)` runtime deps and reopens the
same cycle as analyzing the full `cc_toolchain` implementation.

The direct config-target analysis hook is therefore disabled again. The next
implementation slice must extract the rule-based `ToolchainConfigInfo` metadata
without requesting normal analysis of the config target. Acceptable sources are
configured target-node attributes or a metadata-only provider path that does not
walk `DefaultInfo(files = toolchain_config.files.values())`.

After rebuilding `target/debug/kuro`, the restored baseline smoke
`plan58-restoredshim2-sdk-20260512-050327`, log
`/tmp/plan58-restoredshim2-sdk-20260512-050327.log`, succeeded in `8m41s`
(`elapsed_s=527`, peak RSS `2703448 KiB`). This confirms the disabled hook
restores the known working SDK build baseline and stays well under the
30-minute error bound.

Additional ELF comparison of the last successful Kuro SDK smoke
`plan58-restoredshim2-sdk-20260512-050327` against Bazel shows:

- `bin/zerobuf`, `bin/zerosystem`, and `bin/zm` are the same ELF kind in both
  systems: x86-64 static PIE, `DYN`, `BIND_NOW`, `FLAGS_1 NOW PIE`, no dynamic
  `NEEDED` entries. Their hash drift is not a linked-library/PIC mismatch; it is
  remaining final-link content, including embedded Kuro physical paths such as
  `./execroot/.../buck-out/...` where Bazel embeds `./bazel-out/...`.
- `lib/libzeromatter_ffi.so` is still a real link-surface mismatch. Bazel
  records `librt.so.1`, `libpthread.so.0`, `libm.so.6`, `libdl.so.2`, and
  `libc.so.6`; Kuro records `libgcc_s.so.1`, `libm.so.6`, `libc.so.6`, and
  `ld-linux-x86-64.so.2`. Both are `DYN` shared objects with `BIND_NOW`.
- Fresh Kuro Rust final-link params still only show the plain rule-based flags
  `-rtlib=compiler-rt`, `-nostdlib++`, and `--unwindlib=none`. They still omit
  path-backed args such as `-resource-dir`, `-B...crt_objects`, libc search
  dirs, and libcxx/libunwind search dirs.
- The remaining two systemic blockers are therefore distinct: metadata-only
  rule-based toolchain arg extraction for link-library parity, and logical
  Bazel-shaped exec paths for byte-for-byte Rust artifact parity.

1. Preserve the full legacy C++ toolchain config

   Extend `FeatureConfiguration`/`CcToolchainFeatures` to retain the structures
   passed to `cc_common.create_cc_toolchain_config_info`: features, action
   configs, tool paths, make variables, artifact name patterns, and builtin
   include directories.

   Do not reduce features to names during `cc_common_internal.cc_toolchain_features`.
   Names are still useful indexes, but command-line expansion must operate on
   structured flag/env/action data.

2. Implement Bazel feature selection

   Configure features using Bazel semantics:

   - default-enabled toolchain features
   - requested and unsupported features
   - `implies`
   - `requires`
   - `provides` / mutually exclusive categories
   - enabled action configs

   Keep Kuro's compilation-mode features (`fastbuild`, `dbg`, `opt`) aligned
   with Bazel's requested feature flow. Do not reintroduce always-on mode
   defaults.

3. Expand flag sets generically

   Replace the hardcoded `get_memory_inefficient_command_line` link defaults
   with a Bazel-style expander:

   - select action-config flag sets first, then enabled feature flag sets in
     Bazel order
   - apply `with_features` constraints
   - expand `flag_group`s and nested flag groups
   - support `expand_if_available`, `expand_if_not_available`,
     `expand_if_true`, `expand_if_false`, `expand_if_equal`, and iteration
   - resolve `%{variable}` and rules_cc `format` substitutions from
     `CcToolchainVariables`
   - preserve ordered adjacent arguments exactly

   Keep existing direct handling for unavoidable platform tool quirks only after
   the generic expansion path has produced the Bazel-shaped command line.

4. Carry toolchain data inputs into actions

   Model the action-specific `files` map generated by
   `rules_cc//cc/toolchains/impl:toolchain_config_info.bzl`.

   Expose those files through `CcToolchainInfo` so callers such as rules_rust
   include `cc_args(data = ...)` artifacts in the action inputs. This must cause
   LLVM runtime directory and archive targets to build when their formatted args
   are used.

5. Make tool selection action-config driven

   Update `get_tool_for_action` to choose the configured action tool and include
   its runfiles. Host LLVM path probing can remain only as a fallback for native
   shim cases where Bazel would also use a discovered host toolchain.

6. Separate logical exec paths from physical materialization paths

   Toolchain-expanded artifact strings must use Bazel exec paths:
   `bazel-out/...` and `external/...`.

   Kuro may materialize the corresponding files under `buck-out/...`, but Rust,
   C++, and linker command lines that embed debug/source paths need Bazel-shaped
   logical paths. Implement this as an artifact path mapping layer, not
   SDK-specific `--remap-path-prefix` patches.

7. Normalize output permission materialization

   Preserve Bazel-compatible executable metadata and read-only materialized
   output modes for files and tree artifacts. Treat this as materializer/artifact
   metadata behavior, not a post-build SDK directory chmod.

8. Remove obsolete hardcoded LLVM defaults

   Once generic feature expansion produces the LLVM Linux/musl args, delete the
   partial `use_llvm_linux_link_defaults` branch. Keeping both paths risks
   duplicate or misordered flags.

## 2026-05-11 follow-up: public configure smoke

After preserving legacy and modern flag-set data in `CcToolchainFeatures`, the
zeromatter SDK still had four content hash differences. `rules_rust` does not call
the internal `CcToolchainFeatures.configure_features` method directly; it calls
public `cc_common.configure_features(ctx, cc_toolchain, ...)`. Kuro's public
implementation returned a name-only `FeatureConfiguration`, so the first patch
still discarded toolchain flag sets for real Rust actions.

The current patch routes public `cc_common.configure_features` through
`cc_toolchain._toolchain_features` when available and expands configured
action-config/feature flag sets in `get_memory_inefficient_command_line`.
Focused tests cover legacy action-config ordering, modern `toolchain.args`
expansion, and public configure preservation.

Validation:

- `cargo test -p kuro_build_api_tests interpreter::rule_defs::cc_common -- --nocapture`
  passed.
- `cargo test -p kuro_build_api feature_config -- --nocapture` passed.
- `cargo check -p kuro && cargo build -p kuro && git diff --check` passed.
- Zeromatter `//sdk:sdk_contents` Kuro smoke
  `plan58-publicconfigure-sdk-20260511-154056` succeeded in `10m33s`
  (`elapsed_s=658`, peak RSS `2570024 KiB`), below the 30-minute error bound.

Result:

- Manifest parity remains achieved.
- The same four SDK content hashes still differ:
  `bin/zerobuf`, `bin/zerosystem`, `bin/zm`, and
  `lib/libzeromatter_ffi.so`.
- The three executables have no dynamic `NEEDED` entries in either Bazel or
  Kuro output, so their hash drift is from different static/final link command
  content rather than different dynamic libraries.
- `lib/libzeromatter_ffi.so` is still linked differently:
  Bazel has `librt.so.1`, `libpthread.so.0`, `libm.so.6`, `libdl.so.2`,
  `libc.so.6`; Kuro has `libgcc_s.so.1`, `libm.so.6`, `libc.so.6`,
  `ld-linux-x86-64.so.2`.
- Kuro params now include some rule-based link args such as
  `-Wl,-no-as-needed`, `-Wl,-z,relro,-z,now`, `-lpthread`, and `-ldl` for the
  musl binaries, proving action-config flag-set preservation is partially
  wired.
- Kuro params still omit the larger LLVM/rules_cc path-backed args:
  `-resource-dir`, `-B...crt_objects`, libc search dirs, libc++/libc++abi
  search/runtime dirs, and libunwind search/runtime dirs.

Immediate remaining source:

- Plan item 4 is now the main blocker: rule-based `cc_args(data = ..., format =
  ...)` and `ArgsListInfo`/feature aggregation are not yet represented deeply
  enough in Kuro. Some plain action-config flags survive, but path/data-backed
  formatted args and runtime archive/search-directory inputs do not fully reach
  the Rust final-link params.
- Latest smoke `plan58-actioncategory-sdk-20260511-235143` reaches execution with
  rule-based LLVM link args present, but `clang++` falls through to host
  `/usr/bin/ld` and rejects `--icf=safe`. The extracted command contains
  `-target` with no following target triple; the triple comes from
  `llvm_target_triple.bzl`'s `select()` over composite config-setting labels
  such as `@llvm//platforms/config:linux_x86_64_musl`. Kuro's lightweight
  metadata selector matcher only recognized raw OS/CPU constraint labels, so it
  dropped the selected string while preserving the preceding `-target` flag.
- This is still Plan 58 scope. It is not a packaging issue and should not be
  fixed with SDK-specific link flags.

Next slice classification:

- Missing Bazel semantic: for rule-based `ToolchainConfigInfo`, the direct
  `args.by_action` structure is the source of ordered toolchain arguments.
  Legacy `action_configs.flag_sets` are a compatibility conversion product.
- Missing Bazel semantic: metadata extraction of unconfigured rule-based
  `cc_args` must resolve selectors the same way the configured target would,
  including `config_setting` labels whose names encode a conjunction of platform
  constraints.
- Owning subsystem: `cc_common_internal.cc_toolchain_features()` conversion from
  Starlark `ToolchainConfigInfo` into Kuro's `CcToolchainFeatures`.
- Other affected cases: any modern rules_cc toolchain whose legacy conversion
  loses or reshapes action args, especially args with formatted file/directory
  paths.
- One-off workaround rejected: appending hardcoded LLVM path flags to Rust link
  actions.

Patch classification: systemic parity fix. Prefer parsed modern
`toolchain_config_info.args.by_action` flag sets when present, and use legacy
`action_config.flag_sets` only for legacy configs without structured rule-based
args.

Smoke `plan58-compositeselect-sdk-20260512-000317`, log
`/tmp/plan58-compositeselect-sdk-20260512-000317.log`, confirms the composite
selector fix moved the SDK build past final-link argument shape and into local
execution: status 3 after `314s`, peak RSS `3071668 KiB`. The Rust build-script
environment now contains the ordered rule-based LLVM link args, including
`-target x86_64-linux-gnu`, `-fuse-ld=lld`, `-resource-dir ...`, CRT `-B...`,
and libc++/libunwind search directories.

The new failure is a materialization/input gap rather than a flag-expansion gap.
`aws-lc-sys` invokes clang through `cc-rs`, and clang fails to find builtin
headers:

```
/usr/include/stdio.h:34:10: fatal error: 'stddef.h' file not found
```

The referenced Kuro path for `@llvm//runtimes:resource_directory` does not
exist under the smoke output tree. That path came from
`cc_args(data = ..., format = ...)`: the metadata extractor now formats the
label into the command line, but the cycle-breaking native C++ toolchain shim
still exposes empty `all_files`, `_compiler_files`, and `_linker_files` depsets.
rules_rust correctly appends `cc_toolchain.all_files` to build-script tools, so
the missing Bazel semantic is that `cc_args(data = ...)` file/directory targets
must be represented as toolchain file inputs without analyzing the full
`cc_toolchain` target and reopening the rules_rust cycle.

Next slice classification: systemic provider/input parity fix. Carry
rule-based `cc_args(data = ...)` labels through metadata extraction into the
native C++ toolchain shim as action/toolchain file deps. Expose them through
`all_files` and the compiler/linker file attrs so actions that consume
`cc_toolchain.all_files` materialize the LLVM runtime directories and archives.
Do not remove `-resource-dir`, bypass `cc-rs`, or special-case `aws-lc-sys` or
SDK labels.

Smoke `plan58-datafiles-sdk-20260512-090508`, log
`/tmp/plan58-datafiles-sdk-20260512-090508.log`, failed with status 3 after
`280s`, peak RSS `2902232 KiB`. The previous `resource_directory` not found
failure did not recur. The new failure is caused by over-broad data exposure:
the first native-shim data patch put every extracted `cc_args(data = ...)` label
in `cc_toolchain.all_files`, including compiler/header include-directory data
for actions that the Rust build-script tool path does not need.

The immediate failed label was:

```
llvm+0.7.0//runtimes/libcxx:libcxxabi_headers_include_search_directory
```

That target loads `llvm+llvm_source+libcxxabi//...`, which in this workspace
tries to load `llvm+llvm+llvm-project//vars.bzl` and fails. Bazel does not
force this label merely because a Rust build script consumes
`cc_toolchain.all_files`; Kuro's shim is over-approximating the rule-based
toolchain `files` map.

Next slice: preserve `cc_args(data = ...)` with action ownership and expose a
conservative action-set union for shim file attrs. `all_files` should not be the
union of every data label from every compile/include/link action. For the
current rules_rust build-script path, include the data labels attached to link
actions that produced `LDFLAGS` (`-resource-dir`, CRT objects, runtime search
directories, and libc/libunwind data), and avoid unrelated compile/header
include-directory data until action-specific `_compiler_files` semantics are
implemented. This remains a provider-shape/action-files parity fix, not an SDK
label workaround.

Smoke `plan58-linkdata-sdk-20260512-091608`, log
`/tmp/plan58-linkdata-sdk-20260512-091608.log`, failed quickly with status 1
after `16s`, peak RSS `1995132 KiB`. The old missing `resource_directory`
failure and the over-broad `libcxxabi_headers_include_search_directory` load
failure did not recur. The terminal failure was an allocation abort while
reading `cc_toolchain.all_files`:

```
memory allocation of 140587381398976 bytes with alignment 1 failed
... CcToolchainInfoNativeShim::toolchain_files_depset
... app/kuro_build_api/src/interpreter/rule_defs/context.rs:2289
```

The bug class is still Plan 58's native C++ toolchain provider/input boundary:
the shim now lazily exposes target default outputs, but `toolchain_files_depset`
allocates fresh `CtxCheatArtifactStub` values on each attribute read and clones
path strings out of the shim. Under Starlark value tracing/heap use this produced
an invalid apparent string length and OOM. The systemic fix is to make the
toolchain data payload heap-safe and cheap to copy, for example by storing
ref-counted immutable path data and allocating only small command-line wrapper
values per read. Do not special-case the crashing target or remove link-action
data labels.

Follow-up smoke `plan58-linkdata2-sdk-20260512-092329`, log
`/tmp/plan58-linkdata2-sdk-20260512-092329.log`, used ref-counted immutable path
payloads for toolchain data. It still failed quickly with status 1 after `15s`,
peak RSS `871900 KiB`, but the impossible-allocation stack did not recur. The
client only reported a daemon event-bus broken pipe and no semantic analysis
error appeared in the captured log or `command_report.json`. This points to a
daemon process crash/abort during early analysis rather than a normal Starlark
or action failure. The next action is diagnostic, still in Plan 58 scope: rerun
the same target with direct high-verbosity daemon stderr/backtrace capture to
recover the actual crash site before changing semantics again. If the crash is
again inside native-shim file depset materialization, the likely systemic fix is
to avoid allocating native `File` wrapper values on repeated `CcToolchainInfo`
attr reads by caching the frozen depset on the shim provider heap or by using a
dedicated frozen NativeShim file value for toolchain data labels.

Diagnostic repro `plan58-linkdata-strace-20260512-102524`, log
`/tmp/plan58-linkdata-strace-20260512-102524.log`, confirmed the daemon is dying
with `SIGSEGV` rather than returning a Kuro error. The strace files show the
daemon process `2999049` and many analysis worker threads killed by `SIGSEGV`;
the first captured fault was `SEGV_MAPERR` at address `0x1c` in thread
`2999735`. No coredump was available through `coredumpctl`. This strengthens
the native-value safety classification: repeated live-heap construction of
toolchain data file stubs from a frozen NativeShim value is unsafe enough to
corrupt native process state. The next patch should cache a frozen depset of
toolchain file stubs when the NativeShim provider collection is constructed,
then return that frozen depset for the file attrs instead of rebuilding it on
each `get_attr` call.

Follow-up smoke `plan58-linkdata3-sdk-20260512-103536`, log
`/tmp/plan58-linkdata3-sdk-20260512-103536.log`, used the cached frozen depset
patch. It still failed quickly with status 1 after `26s`, peak RSS `864164 KiB`.
The prior impossible-allocation stack did not recur, but the client again saw
only a daemon event-bus broken pipe. The last useful log region is still early
analysis of rules_rust/rules_cc toolchain deps, with dense `cc_toolchain`
`all_files` reads and native value freezing checkpoints before the daemon exits.
This means caching the depset is not sufficient: the representation still
freezes a native `File` stub that carries Rust-owned `Arc<str>` and
`ConfiguredTargetLabel` data inside the native C++ toolchain provider heap.
The next slice is to remove that unsafe boundary by representing shim toolchain
inputs as plain frozen Starlark-compatible path records for rule analysis, and
only converting them into Kuro artifact groups at the command-line/input visitor
boundary where lifetimes are controlled. If a direct backtrace is available,
recover it first; otherwise change the shim file value shape rather than
expanding or special-casing the toolchain data set.

Follow-up smoke `plan58-staticfile-sdk-20260512-104619`, log
`/tmp/plan58-staticfile-sdk-20260512-104619.log`, replaced the frozen provider
payload with a pointer-stable toolchain-only `File` stub backed by leaked path
and configured-label records. It still failed at the same boundary: status 1
after `26s`, peak RSS `857792 KiB`, with only daemon broken pipe in the client
log. `gdb`/`lldb` were not available in the environment, so no native backtrace
could be recovered. This falsifies the narrow theory that `Arc<str>`/owned
label payloads inside the frozen File stub were the sole crash cause.

Next classification: isolate whether the crash is caused by any non-empty
native shim file depset on `cc_toolchain.all_files` versus a different
concurrent analysis bug now exposed by Plan 58. Before another semantic patch,
run a bounded diagnostic build with the rule-based C++ toolchain feature
metadata still enabled but shim file attrs returning empty depsets. If that
survives past the 26s crash window and reaches the old missing-resource-dir
failure, the file depset representation/interaction is still the culprit. If it
still crashes, the active crash is independent of toolchain file inputs and the
next fix should target the common rules_rust/rules_cc analysis path visible in
the last memory samples.

Diagnostic `plan58-emptyfilesdiag-sdk-20260512-104857`, log
`/tmp/plan58-emptyfilesdiag-sdk-20260512-104857.log`, set
`KURO_DISABLE_CC_TOOLCHAIN_FILE_SHIM=1` and did not crash at the 26s boundary.
It was stopped by the diagnostic `300s` timeout while still in analysis, with
peak RSS `990674944` bytes, which proves the daemon crash is tied to the
non-empty native shim file depset rather than the broader rule-based feature
metadata extraction. The next fix is to narrow the file depset exposure: provide
the link-action `cc_args(data=...)` files through `cc_toolchain.all_files` for
rules_rust build-script tools, but stop returning the same non-empty depset from
every private C++ toolchain file attr such as `_compiler_files` and
`_linker_files`, which drives much larger rules_cc analysis paths.

Follow-up smoke `plan58-allfilesonly-sdk-20260512-105646`, log
`/tmp/plan58-allfilesonly-sdk-20260512-105646.log`, returned the non-empty
toolchain data depset only from `cc_toolchain.all_files` and empty depsets from
the private file attrs. It still failed with the same daemon broken pipe after
`27s`, peak RSS `859620 KiB`. This rules out private attr overexposure as the
crash trigger. Since an empty `all_files` depset avoids the crash and any
non-empty depset of shim `File` values triggers it, the next fix is native-value
safety: make the shim-only `File` and root values explicitly no-op traceable (no
live Starlark heap references) before considering an action-side materialization
path that avoids `all_files` entirely.

Follow-up smoke `plan58-tracefile-sdk-20260512-105953`, log
`/tmp/plan58-tracefile-sdk-20260512-105953.log`, added explicit no-op tracing
for the shim-only `File` and root values. It still failed with daemon broken
pipe after `26s`, peak RSS `879456 KiB`. This rules out missing tracing as the
primary issue. The next native-value safety check is hashing/equality: the shim
`File` equality is path-only, so its Starlark hash should also be path-only and
must not hash the configured-label payload while depsets/dicts dedupe files.

Follow-up smoke `plan58-pathhash-sdk-20260512-110141`, log
`/tmp/plan58-pathhash-sdk-20260512-110141.log`, made shim `File` hashing match
path-only equality. It still failed with daemon broken pipe after `26s`, peak
RSS `922708 KiB`. This exhausts the cheap native `File` safety fixes. The next
slice should avoid exposing fake `File` values through Starlark depsets at all:
return a Kuro-only command-line/input carrier from `cc_toolchain.all_files` and
teach Bazel-compatible `ctx.actions.run(tools=...)` collection to preserve that
carrier as hidden action input materialization. This keeps rule-based
`cc_args(data=...)` targets materialized for rules_rust build scripts without
forcing Starlark depset operations over native shim `File` values.

Follow-up smoke `plan58-carrier-sdk-20260512-111124`, log
`/tmp/plan58-carrier-sdk-20260512-111124.log`, returned the Kuro-only carrier
directly from `cc_toolchain.all_files`. It avoided the previous daemon crash and
failed normally after `20s` during analysis:
`rules_rust/rust/toolchain.bzl` builds `depset(transitive = all_files_depsets)`
and therefore requires each `all_files` value to remain a depset. The next slice
is to preserve Bazel's public type contract by returning a depset whose direct
member is the crash-free Kuro-only command-line/input carrier. That should let
rules_rust compose the value transitively while avoiding fake `File` values in
Starlark depsets.

Follow-up smoke `plan58-carrierdepset-sdk-20260512-111643`, log
`/tmp/plan58-carrierdepset-sdk-20260512-111643.log`, returned a `depset` whose
single direct member is the Kuro-only carrier reporting Starlark type `File`.
This passed the previous crash/type-contract failures and reached hidden
toolchain data analysis, then failed after `582s` on
`llvm+0.7.0//runtimes:resource_directory`: analyzing
`llvm+0.7.0//runtimes/compiler-rt:clang_rt.builtins.static` led to
`llvm+llvm_source+compiler-rt//:clang_rt.builtins.static`, whose BUILD file loads
`llvm+llvm+llvm-project//vars.bzl`; Kuro reported that file missing. The next
slice is not to remove the toolchain data input, but to fix repository/load
resolution or compatibility handling so the same LLVM runtime data targets Bazel
can analyze are also analyzable in Kuro.

Follow-up smoke `plan58-reporule-sdk-20260512-113546`, log
`/tmp/plan58-reporule-sdk-20260512-113546.log`, changed non-root
`use_repo_rule` canonical names to Bazel's `module++rule+repo` shape and
validated the focused bzlmod tests. The SDK smoke still failed after `609s` at
the same `llvm+llvm+llvm-project//vars.bzl` load. Therefore the bad
`@llvm-project` resolution is not coming from non-root `use_repo_rule`
canonicalization; it is coming from Kuro's global apparent-repo aliasing for
extension repos. The next slice must make apparent repo resolution scoped enough
for generated repos like `llvm+llvm_source+compiler-rt` to resolve
`@llvm-project` to the populated `llvm++llvm+llvm-project` repo imported by the
owning module's `use_repo(llvm, "llvm-project")`.

Follow-up smoke `plan58-scopedalias-sdk-20260512-115106`, log
`/tmp/plan58-scopedalias-sdk-20260512-115106.log`, registered scoped Bzlmod
repo aliases and used them from Bazel `Label()` resolution. It still failed
after `875s` at the same `llvm+llvm+llvm-project//vars.bzl` load from
`llvm+llvm_source+compiler-rt//:BUILD.bazel`. This narrows the active bug:
the bad repo is resolved while parsing Starlark `load("@llvm-project//...")`,
not while constructing a `Label()` value. The next slice must move scoped
apparent-repo resolution into the common cell/import alias resolver, and it
must prefer the owning module's `use_repo` mapping before the global apparent
alias table so generated repos do not inherit another extension's repo named
the same apparent string.

## Tests

Add focused tests before relying on the full SDK:

- A synthetic rule-based C++ toolchain test where `cc_args(data, format)` expands
  to a generated file/directory path and the generated target is an action input.
- A feature-condition test covering `requires_*`, `requires_any_of`,
  `with_features`, and an unsupported feature.
- An `iterate_over` test for list-valued link variables.
- An action-config tool-selection test that proves the configured tool and
  runfiles are used.
- A path-mapping test that verifies command-line artifact strings are
  Bazel-shaped while materialized files remain in Kuro's output tree.
- A materialization test for Bazel-compatible output executable/read-only modes.

Then validate against zeromatter:

1. Build the relevant LLVM runtime/search-directory targets with Bazel and Kuro.
2. Compare Rust final-link param files for `zerobuf`, `zerosystem`, `zm`, and
   `libzeromatter_ffi.so`.
3. Build `../zeromatter-kuro` `//sdk:sdk_contents` with Kuro.
4. Compare file list, modes, sizes, and hashes against Bazel's
   `bazel-bin/sdk/sdk_contents`.

## Acceptance Criteria

- `cc_common.get_memory_inefficient_command_line` is driven by configured
  toolchain features/action configs, not by SDK-specific LLVM hardcoding.
- `cc_args(data = ...)` artifacts are built and included as inputs to actions
  whose command lines reference them.
- LLVM musl final Rust links include Bazel-equivalent `resource_dir`, CRT search,
  musl search, libc++/libc++abi/libunwind search, and runtime archive args.
- Kuro command-line artifact strings use Bazel exec paths where Bazel embeds
  them in outputs.
- `../zeromatter-kuro` `//sdk:sdk_contents` builds successfully with Kuro and
  its output tree is byte-identical and mode-identical to Bazel.
- No new SDK-specific link-flag or path-remap shims are introduced.

## Risks

- Feature expansion order is observable. Implement and test action-config order
  before feature order; do not sort flag sets except where Bazel/rules_cc does.
- Starlark values inside legacy feature structs can contain depsets, files, and
  nested structs. Prefer converting once into typed Rust data at
  `cc_toolchain_features` construction rather than repeatedly reflecting through
  Starlark during action registration.
- Logical path mapping must not change actual materialization locations or CAS
  keys. It should affect command-line strings, not artifact identity.
- Output mode normalization can disturb incremental cleanup if files become
  read-only too early. Apply it at declaration/materialization boundaries where
  Kuro already knows an action has completed.
