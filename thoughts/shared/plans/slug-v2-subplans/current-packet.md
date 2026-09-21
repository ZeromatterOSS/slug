# Current Slug V2 Work Packet

Packet: WP-7-64-m7a-structural-cpp-compile-inputs-r1
Status: accepted

## Observable outcome and compatibility

Expose real structural Cpp compile inputs and pass a genuine published
CcToolchainInfo through unchanged get_copts and dotd_files_enabled. This removes
the compile-variable/configuration prerequisite on the actual default (dotd-on)
C/C++ path; it does not substitute a non-discovering Spawn for CppCompile.
WP763 at10dcaac34 accepted complete constructor publication with39 focused tests,
Core/CLI checks and final review. Its source/capture/configuration ownership stays.

Exact: public immutable non-callable copts/conlyopts/cxxopts, omitfp prefix,
private dotd/save_temps/process_headers getters, admitted command conversion and
both exec-transition mappings, File.extension and empty per_file_copts binding.
Slug-native: existing complete configuration/path/Host domains. Deferred:
nonempty per-file regex state, ObjC, actions2ctx/root/naming adapters, actual
CppCompile action production, discovery/inclusion validation and execution.
No claim of actual bootstrap compile argv/discovery evidence: existing filtered
Solib observations do not establish those facts. M7A remains partial, M8 unproved.

## Evidence and decision

Pinned Bazel9.2 revision8220c6198837d5c13d53fea211cf3282aa12408a:
CppConfigurationApi.java42–64 and CppConfiguration.java515–536 establish public
option lists; copts prepends -fomit-frame-pointer,-fasynchronous-unwind-tables,
-DNO_FRAME_POINTER when experimental_omitfp. CppConfiguration.java395–404,
833–836,927–930 establishes private boolean methods. CppOptions.java229–264,
552–567,604–641,948–956 owns defaults and repeated raw-string conversion.
builtin_exec_platforms.bzl291–347 maps copt=host_copt plus -g0 and cxxopt=host_cxxopt
plus -g0 on non-Windows observed Host OS, no suffix on Windows; conlyopt=host_conlyopt and
per_file_copt=host_per_file_copt. cc_dotd_files propagates; save_temps and
process_headers_in_dependencies reset to defaults. Both existing Slug exec paths
must share this correction; no parallel derived configuration.

CcStarlarkInternal.java972–990 owns typed positional Cpp/File/Label binding for
per_file_copts, with no private caller gate. Empty retained per_file_copt returns
empty; nonempty remains explicitly rejected until regex semantics are admitted.
Validate genuine arguments before returning empty. Artifact.java405 delegates
File.extension to PathFragment.java762; implement that source algorithm, including
root/nested dotfiles, rather than assuming Rust Path::extension equivalence.

Unchanged rules_cc0.2.17 compile_build_variables.bzl379–415 owns option/rule/
language/per-file ordering and extension override. cc_compilation_helper.bzl647–658
owns dotd option and parse_showincludes/no_dotd_file suppression. compile.bzl398,
1615–1668,1760,2153–2160 demonstrates process-headers/save-temps/default-discovery
demand. Existing54-body corpus and sources.json remain unchanged. Upstream Java
getter/converter tests are represented by pinned-source focused Rust cases;
actual compiler execution is outside this prerequisite's phase.

## Ownership and scope

SlugConfiguration remains sole DICE-retained option owner; all fields already
exist structurally. Admit conlyopt,cxxopt,host_copt,host_conlyopt,host_cxxopt,
cc_dotd_files,save_temps,process_headers_in_dependencies through existing command
overlay/converters. Preserve raw repeat order/commas and both exec paths; no
Java runtime, new DICE key, cache, lock, or fallback. Read docs/developers/dice.md.
Existing request certificates, validity/cutoff, retry/cancellation and eviction
apply unchanged. Native projections own cheap shared configuration; returned
immutable lists are evaluator-phase allocations. No new retained representation,
interner or performance claim; Stage9 existing configuration/Arc projections apply.

Configuration worker owns app/slug_configuration_v2/src/command.rs,
native/cpp_fragment.rs and cohesive child tests, native/configuration.rs shared
exec projection and direct command/converter dispatch/test files required by the
eight admissions. Native cc_feature_tests.rs/configuration.rs/tests.rs test Host fixtures
may be upgraded to explicit observed OS where newly required. Commands
app/slug_commands_v2/src/common.rs admits focused binder tests only.
Large configuration modules get bounded hooks plus child tests, not a new owner.

Loading worker owns app/slug_loading_v2/src/analysis_fragments.rs and new
analysis_fragments/compile_input_tests.rs; cc_common.rs thin per_file_copts
binding (helper in cohesive child if needed); subrule_invocation.rs thin
File.extension projection and cohesive child tests. BuildAPI's existing
AnalysisArtifact may expose a shared extension getter in analysis_value.rs with
focused child tests only if needed to avoid a second artifact algorithm.
No new function-publication recipe, actions2ctx or action activation.

Root owns integration tests/rustc_map_each/cc_compile_inputs.rs and .bzl,
mod.rs registration, minimal reuse/export changes in cc_toolchain.rs/.bzl,
BuildAPI export wiring if needed, scheduling/Stage6/bootstrap/Stage9 docs and
integration corrections. Existing complete constructor must be reused as a
dependency; no authored Cpp/feature envelope and no copied upstream body.
Reviewer owns independent design/final review. Root serializes Cargo.
Estimate350–700 production and500–1000 proof lines; review growth and ownership.

## Acceptance and validation

Configuration: defaults/nondefaults/repeated raw values/invalid bools, exact
omitfp ordering; both existing exec pathways with Linux/Windows discriminators,
host-to-exec replacement, retained dotd and reset save/process flags. Target
and exec restoration must compare full structural identities.
Loading: field vs method shape, immutable/noncallable lists, current private
caller provenance, typed binder negatives, empty-only per-file rejection,
File extension dot/empty/Unicode/path edge cases. Preserve existing link getters.
Integration: unchanged get_copts on real C/C++/headers with override-extension
ordering, published genuine toolchain, omitfp/raw repeats and source File;
unchanged dotd predicate all suppression branches; same-DICE A/B/A and unused
compile option invalidation. Protect the4 full-toolchain and8 Rustc/linker tests.

Resolve pinned nightly through rustup. Serialized offline/locked Cargo JSON
no-run for Configuration/Commands/Loading libs and Analysis starlark_rule integration;
Core/CLI checks. Exact preflight before each executable batch, then execute every
selected test.60s preparation operation cap; tests expected seconds, no new
user deadline. No Bazel/Slug process, daemon or action execution needed.
Format/diff/status/archive and independent final ACCEPT before atomic commit.
REPLAN only for a new semantic owner/compatibility decision; normal compile/test
corrections remain in packet. Full CppCompile needs mandatory/spawn/cache-only
input partitions and discovery-aware lifecycle, not generic Spawn relabeling.

Independent design ACCEPT confirms this bounded prerequisite. File.extension
uses the artifact exec-path, including prefix-sensitive dotfiles. The exec -g0
branch uses structural observed Host OS; missing observation fails closed, never
inferred from CPU, path flavor, target OS or selected execution-platform OS.

## Acceptance receipt

Accepted against10dcaac34 on2026-09-21 after independent final ACCEPT. The
observable gate is authentic unchanged compile-option/dotd helper consumption of
the published complete toolchain, with same-DICE change/restoration. No parity
class widened; production remains Rust-native. Source corpus54 bodies unchanged.
The implementation is about220 production lines with cohesive child tests;
proof growth stays within the declared estimate. No new retained representation.

Pinned nightly-2025-09-14, offline/locked, Cargo JSON no-run: Configuration lib
exit0/1.267s; Commands lib0/10.844s; Loading lib0/17.458s; Analysis starlark_rule
integration0/7.026s. Two initial test-only borrow/coercion compile errors were
corrected before execution; their0.565s/4.576s receipts are preserved locally.
Exact preflight plus individual direct execution: Configuration16/16,
Commands2/2, Loading9/9, Analysis14/14 (2 new,4 toolchain,8 Rustc/linker).
All41 selected tests passed, exit0,1.886s total; no failing runtime invocation.
Core check0/7.575s and CLI check0/7.826s. Total preparation/check57.137s including
the corrected compile attempts; maximum operation17.458s, below60s cap. Commands,
outputs and receipts are in ignored target/wp764. Formatting, git diff --check,
plan status and archive checks pass. No Bazel/Slug action or daemon run.

Packet elapsed approximately17 minutes from predecessor acceptance through close;
parallel design/source/implementation/final review time was not separately timed.
No REPLAN. Next bounded prerequisite is actions2ctx/root/output naming ownership;
the actual default dotd-on CppCompile action must later own its separate input
partitions, discovery and inclusion validation. M7A remains partial; M8 unproved.
