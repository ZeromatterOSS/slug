# Current Slug V2 Work Packet

Packet: WP-7-72-m7a-cpp-input-preparation-r1
Status: accepted

## Outcome and demand

Pure pre-execution input preparation for published no-scanning regular-file
CppCompile recipes: expand discovery compiler options, check include paths,
then return existing input-role sets and dangling-option warnings. This fills
the pre-execution check missing from WP771, not tracked discovery or execution.
WP771 at5c94981ec accepts supplied-dotd reconciliation with49 tests/Core/CLI
checks and independent acceptance; its complete contract is retained in Git.
The allocator demands this compile family. Inert configured fixtures establish
the pure boundary, not production allocator flags or full toolchain construction.

## Compatibility and source

Pinned Bazel9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
CppCompileAction636–678/790–907/1076–1077/1154–1204;
CompileCommandLine119–154/170–182; CcToolchainVariables304–312;
CcLibraryConfiguredTargetTest1362–1431 and CcCommonTest1648–1670.
Exact named slice: no-scanning regular-file original-cache option expansion,
ordinary/quote/system path extraction, ordered validation and ignored-prefix
rules under POSIX, NOOP mapping and nonsibling repository layout.
Slug-native: valid-Unicode/NUL-free consumed paths, structural identity, diagnostic
text and returned warning collection instead of process stderr. Warnings preceding
an error are not delivered by this pure Result API. Joined suffixes use Java trim
(characters <=U+0020), not Unicode whitespace trimming. MSVC option spelling
classification is admitted on POSIX; Windows path semantics are not.
Unchanged production Rust implementation only; no Java runtime/helper.

## Owner and invariant

Add CppCompileSpec::prepare_inputs_posix() returning
Result<CppCompileInputPreparation,String>. The result has private fields and
borrowed getters inputs(): &CppCompileInputSets, warnings(): &[CompactString].
No Clone requirement or cached result. This is a pure observation, never an
execution-readiness certificate. The configured-node recipe remains immutable.

Order follows discoverInputs: feature-expand compiler options using ORIGINAL
Variables/cache (not WP770 fresh-overlay argv and not including selected tool);
extract system paths/warnings even when needs_include_validation=false. If
validation is enabled, collect ignored builtin+context system_includes+
external_includes, then ordinary and quote paths, then validate ordinary→quote→
system. Return WP771 input_sets only after success. No partial result on error.
Disabled validation bypasses raw copts/context validation, not compiler-option
expansion, system-path normalization or existing regular-file restrictions.

Ordinary paths: context includes followed by combined raw user_compile_flags
-I or /I (uppercase I only), standalone flag ignored, case-insensitive msvc
suffix prefix excluded. Quote paths: context quote_includes followed by raw
-iquote joined trimmed or split untrimmed; dangling warns/skips. System paths:
original expanded options -isystem or case-insensitive -/imsvc, joined trimmed
or split untrimmed; dangling warns/skips. No generic shell/option parser. Raw
copts use existing Variables lookup/sequence/string semantics, absent→empty;
malformed consumed values fail closed. Framework/forced-include paths are not
checked in this no-scanning slice.

Normalize via existing POSIX utility. Ignored dirs ALWAYS include builtin+
context system+external regardless of strict_system_includes (unlike WP771
post-dotd policy). Match whole components. For other paths remove a leading
external component as source does, then reject absolute or remaining .. segments.
Empty and normalized in-root paths succeed. First violation wins. Missing or
malformed consumed context fails closed. No filesystem, inferred labels, new
configuration option, retained layout, DICE key/lock or discovery side cache.
Reuse existing compact strings, original Variables adapter, normalization and
private path/context helpers; output vectors are caller-owned scratch.
Ignored directories use existing SmallSet exact lookup before prefix scanning,
as pinned source does, avoiding an unconditional prefix scan for exact matches.
Stage9 WP770/WP771 reuse applies; no donor or performance claim.

## Scope, routing and proof

Baseline5c94981ec; branch codex/wp772-cpp-input-preparation-review.
Independent design ACCEPT2026-09-22 precedes implementation; final actual-diff/evidence
review precedes atomic integration. Root owns hooks/docs/serial validation.
Worker A: actions/cpp_compile/input_preparation.rs and child tests; may widen
dependencies.rs existing context_directories/beneath visibility to pub(super)
only for same-owner reuse.
Worker A may also expose expansion.rs test module and expansion/tests.rs
group/configuration factories to cpp_compile siblings for test-only reuse.
Worker B: Analysis compile_context/input_preparation.rs
and compile_context.rs hook; authored compile-context fixture extensions only
for discriminating cache proof if existing recipes cannot establish it.
Root: cpp_compile.rs/actions/mod.rs/lib.rs thin type exports; Stage6/7/9/bootstrap,
canonical/current docs. No production Core/REAPI/Loading/Configuration changes.

Estimate150–250 production and300–500 new test lines. Independent growth review
if exceeded; extraction, checks and preparation stay one focused owner module.
Required evidence:

- Source regression escaping relative/absolute include/system paths;
  context/raw flag ordering, -I combined-only, quote/system joined/split/dangling,
  MSVC spelling exclusions, Java-vs-Unicode trim, normalization and component
  boundaries. Framework/forced-include are not accidentally validated.
- Original cached options versus fresh argv; raw user flags versus feature flags;
  absent/malformed copts and context, needs-validation=false timing.
- Strict=true still ignores declared system dirs before execution; post-dotd
  strict behavior stays protected. First failure and immutable repeats/errors.
- Actual published main/external target/exec/PIC recipes return exact input_sets;
  same-DICE BUILD flags/configuration A/B/A changes reject/restore preparation.
- Protected WP770 command and WP771 input/reconciliation tests, generic command/
  REAPI refusals. Tests reuse existing authored workspace, no upstream copy.

Pinned nightly serial offline/locked JSON no-run BuildAPI/Analysis, exact batched
preflight/tests, Core/CLI checks; rustfmt/diff/archive/plan-status gates.
Preparation60s/operation, seconds-scale runtime estimates, no user hard12/15s
deadline. No Bazel, daemon, C++ compiler action or network invocation selected.
REPLAN for contradictory source, new owner/policy or execution activation.

## Remaining boundary

This does not compute/store commandLineKey or discovered-input replay, acquire
verified dotd bytes (including AC hits), resolve sources through Core, or publish
action success. Those facts must have tracked DICE ownership and complete source
frontiers, not attempt-local side caches. Modules/scanning/trees/Windows/sibling/
mapped paths and generic aquery/ActionKey/REAPI activation remain deferred.
Full production toolchain construction still has separate configuration/helper
gaps. M7A partial; M8 unproved.

## Acceptance evidence — 2026-09-22

Observable gate: genuine published CppCompile recipes now perform source-defined
pre-execution option/path checks before returning their complete input-role sets.
Original discovery cache versus fresh execution argv is discriminated in both
directions at the unit boundary and over three actually published native recipes.
Pre-execution ignored-system policy remains distinct from post-dotd strict policy.
Warnings are returned only on success, explicitly Slug-native delivery.

54 exact nonignored tests pass: BuildAPI32 (8 new preparation,5 dotd parser,
7 reconciliation,7 construction/refusal,5 command) and Analysis22 (3 new
preparation,3 dotd publication,16 protected command/context/cache). New public
matrix covers12 C/C++ PIC/main/external/target/exec cases; same-DICE BUILD flags
and configuration reject/restore, with whole-result equality and direct restored
preparation. Authored cache-proof mutation occurs before registry observation;
upstream source bodies stay unchanged. WP769 GC/Core/REAPI refusal and WP771
Configuration/Commands evidence is reused for unchanged owners, not claimed as
fresh full-suite proof.

Latest receipts under target/wp772: tests-1790043019986668259.receipt.json
(BuildAPI32), tests-1790043004866891317.receipt.json (Analysis22).
Selected successful tests total3.532s, maximum0.333s. Earlier receipt
tests-1790042954084333876 has one pass then one fixture-only failure: empty
options were incorrectly wrapped in an empty flag_group. Corrected the test
factory to pass no group, preserving production constructor rejection; rebuilt,
repreflighted and reran all32 tests. No unaccounted runtime failure.

The initial compile was superseded by a source-backed SmallSet exact-match
short-circuit for ignored dirs. Final BuildAPI no-run receipt is
compile-slug_build_api_v2-3; Analysis compile-slug_analysis_v2-1 is unaffected by
the cfg(test)-only fixture correction. Core check-slug_core_v2-1 passes17.339s
and CLI check-slug_cli_v2-1 passes18.139s. All6 compile/check operations exited0,
70.796s total,30.762s maximum, using pinned nightly/offline/locked Cargo JSON.
Rust format, diff, archive and plan-status checks pass. Independent final ACCEPT
covers the actual implementation, fixture correction,54 tests and Core/CLI gates.

Growth review:156-line production owner plus thin exports and same-owner helper
visibility. New tests total574 lines (389 unit,185 configured), above the estimate
because flag parsing, cache timing and genuine publication require distinct
proofs; shared feature factories are reused through cfg(test)-only visibility.
No feature-engine copy, retained owner or interner. Existing compact scratch
utilities and shared handles suffice; no performance claim. Recorded interval
begins02:01 UTC and acceptance is02:13 UTC (~12 minutes), excluding earlier
source research. Concurrent review is not separately timed. All remaining
discovery/execution/toolchain gates stay open.

Successor research points to unchanged get_cc_toolchain_provider production,
not another getter-only proof: genuine ctx/FDO subrule/configuration/Files must
publish into the public compile consumer. Candidate projection prerequisites are
minimum_os_version, is_tool_configuration and stamp_binaries; they do not alone
admit the full cc_toolchain rule, whose allowlist PackageSpecificationInfo and
currently unsupported TemplateVariableInfo envelope need separate accounting.
The other required architectural decision remains tracked execution/discovered
input identity and freshness under DICE, beyond attempt-local transport effects.
