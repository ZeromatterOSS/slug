# Current Slug V2 Work Packet

Packet: WP-7-71-m7a-cpp-dependency-reconciliation-r1
Status: accepted

## Outcome and demand

Pure no-scanning regular-file input-set projections and supplied-dotd-byte
reconciliation for published CppCompile recipes. Return immutable discovered
and post-discovery inputs, not execution authority or recipe mutation.
Predecessor WP770 at b5a6c77f0 accepts argv/environment with42 tests and Core/CLI
checks; its full contract/receipt remains in Git.

Authenticated rules_rust allocator BUILD21–28 demands regular compilation/dotd.
allocator_library.cc deliberately has no includes (release/loaded SHA256
b74910c052d9937d4a36fafe1a054b0a9646a112684af517907dec2eb08e3d94).
This does not exclude compiler-injected dependencies. Solib inventories do not
prove allocator argv/toolchain/.d/execution. No new production observation is
needed for this pinned-source algorithm prerequisite.

## Compatibility and source

Pinned Bazel9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
DependencySet.java137–237, DependencySetTest46–238;
CppCompileActionBuilder290–297; CppCompileAction318–335/636–678/1085–1160/
1213–1224/1675–1689/1802–1853; HeaderDiscovery68–283 and HeaderDiscoveryTest.
Exact named slice: token parsing, POSIX lexical paths, stable first-occurrence
artifact sets, regular allowed-derived/source resolution and post-result
inclusion checks. NOOP mapping, nonsibling layout, non-Windows case-sensitive
execution, no modules/scanning. Slug-native: valid UTF-8 paths, diagnostics,
structural artifact/configuration identity. Invalid UTF-8/NUL fail closed, not
Latin-1 reinterpretation or exact arbitrary-byte Bazel paths.

Parser preserves order/duplicates across stanzas before artifact dedup. Tabs
and literal backslashes are not generic whitespace. Nonempty input needs final
LF; empty/targetless input succeeds. Escaped spaces/continuations/$$ and colon
lookahead follow source. Stanza output names are not authenticated.

## Owner and API

CppCompileSpec remains immutable configured-node DICE data. Add pure methods:
- input_sets(): mandatory (mandatory spawn + cache-only), additional (declared +
  prunable), spawn (mandatory spawn + additional), allowed-derived (mandatory +
  prunable + declared + source, with first path identity at resolution).
- reconcile_dotd_posix(exec_root, bytes, resolve_source): parse/resolve/validate,
  return discovered plus post-discovery (mandatory + discovered) inputs.
Unused declared/prunable headers disappear; mandatory cache-only remain.
Builtin inputs already belong to mandatory spawn; do not duplicate that policy.

The synchronous source resolver receives normalized exec-relative paths after
allowed-derived lookup. It returns an existing authoritative Source artifact or
None, propagating errors. Reject non-source/mismatching-path results; never
manufacture labels from paths. Tests supply explicit genuine artifact maps.
This is NOT Core source-resolution admission. Its future consumer must use
repository/package/source ownership and tracked DICE observations.

Normalize paths/root lexically with existing POSIX utility, no filesystem reads.
Absolute builtin-prefix paths are ignored on component boundaries before root
relativization. Outside-root errors precede unresolved errors when validation is
enabled. Resolve derived first then source; exclude primary source; ignore
unresolved .cppmap except the root-relative `.cppmap` (PathFragment extension
rule). Stable artifact dedup preserves canonical identity, not
path-only equality. Tree/directory/runfiles inputs fail closed in this regular
slice; no guessed tree-child mapping.

Final inclusion validation checks mandatory + declared + prunable membership or
ignored directory prefixes. New pure CppFragmentProjection getter reads
strict_system_includes from the action output root's full configuration, not
the toolchain producer's Cpp fragment: strict uses builtin
dirs only, nonstrict also context system_includes/external_includes.
Preserve source module suffix exemptions without admitting module actions.
Missing/malformed consumed context fields fail closed, not empty defaults.
Disabled validation omits ordinary unresolved/outside paths as source does,
not resolver errors or unsupported byte/artifact types. No partial result on error.

SmallMap/SmallSet, CompactString, shared Artifact handles and existing POSIX
normalization; all indexes/tokens are call scratch and returned vectors caller-owned.
No retained layout, global cache/interner, DICE key, lock, filesystem observation,
donor, performance or memory claim. Stage9 WP769/WP770 disposition applies.

## Exclusions and future execution

Not complete discoverInputs: pre-execution include-flag/path validation, modules,
tree discovery, Windows/sibling/mapped paths, verified dotd acquisition, Core
source-resolution hookup, DICE discovered-input replay/cache hits and atomic
execution publication remain deferred. Do not open generic argv/aquery/ActionKey,
REAPI, prerequisite/staging/execution guards. Reconciliation never authorizes
action success. Future execution must own discovered facts and full frontiers
under DICE; attempt-owned transport results are not a discovery side cache.

## Scope, routing and proof

Base b5a6c77f0; branch codex/wp771-cpp-dependency-reconciliation-review.
Independent design ACCEPT precedes implementation; final diff/evidence review
precedes atomic integration. Design ACCEPT2026-09-22 confirms action configuration
owns strict-system policy and explicit POSIX API admission. Root owns getter/hooks/docs/serial Cargo.
Worker A owns cpp_compile/dependencies.rs and input_sets.rs/tests; worker B owns
dependencies/dotd.rs/parser tests; worker C reviews. Root owns configured tests
unless delegated to a freed worker with disjoint ownership.

Allowlist: BuildAPI actions/cpp_compile.rs, actions/mod.rs, lib.rs thin hooks;
cpp_compile/dependencies.rs and children/tests; existing cpp_compile/tests.rs
fixture visibility only. Configuration native/cpp_fragment.rs getter,
command.rs single StrictSystemIncludes boolean admission, and existing
native/configuration/compile_input_tests.rs focused policy regressions;
Commands common.rs existing boolean-capture test row only. Independent design
delta ACCEPT admits this previously default-only policy through the existing
typed command/configuration owner, including inherited exec behavior.
Analysis tests/rustc_map_each/compile_context.rs hook,
compile_context/dependencies.rs and authored fixtures only for actual recipe/
header/config restoration. No upstream corpus copies or production Core/REAPI
edits. Root docs Stage6/7/9/bootstrap/canonical/current.

Estimate350–550 production,450–650 test lines; growth triggers review.
Parser, artifact reconciliation and actual publication are separate modules.
Required discriminators:
- Parser continuations/spaces/dollars/colons/tabs/duplicates/targetless/missing LF,
  valid Unicode and rejected invalid bytes.
- Distinct mandatory/spawn/prunable/cache roles, first derived path owner,
  source/generated/cross-root identity, primary-source exclusion, stable dedup.
- Builtin component boundaries, outside/unresolved error order, both validation
  modes, strict system dirs, resolver errors/forgery and tree rejection.
- Unused-header pruning/cache-only retention, published public compile with
  supplied dotd, immutable recipe and same-DICE header/config restoration.
- Protected WP769/770 construction/command/refusals. Synthetic dotd is not
  compiler-produced evidence.

Pinned nightly, serial offline/locked JSON no-run, exact batched preflight/tests,
Core/CLI checks and rustfmt/diff/archive/status checks. Reuse unaffected evidence.
Preparation60s/operation, seconds-scale tests; no user hard12/15s deadline.
No Bazel, daemon, compiler or network invocation selected.
REPLAN for new semantic ownership, contradictory source, unowned policy or
execution activation. Ordinary corrections remain within this contract.
M7A partial, M8 unproved.

## Acceptance evidence — 2026-09-22

Observable gate: actual published regular CppCompile recipes expose distinct
mandatory/additional/spawn/allowed-derived sets and immutable discovered/
post-discovery inputs from supplied dependency bytes. Unused headers are pruned,
cache-only mandatory inputs retained, source identity restored through an explicit
authoritative callback, and action-configuration inclusion policy enforced.
Pinned-source regressions cover the named slice above, including the root-level
`.cppmap` extension edge from PathFragment762–771. This ordinary source correction
was rebuilt and repreflighted; no execution surface or contract was widened.

49 exact nonignored selectors pass: Configuration5 (defaults, conversion,
malformed policy, both exec paths, structural change/restoration), BuildAPI24
(5 parser,7 reconciliation,7 protected construction,5 protected command),
Analysis19 (3 new published pruning/cache-only/header-policy restoration,
16 protected command/context/cache) and Commands1 (typed boolean capture).
Published tests cover main/external target/exec roots and nonvacuous native
three-action recipes; source maps contain genuine published Source artifacts.
Opposing action/toolchain strict policies and same-path distinct canonical source
identities discriminate semantic ownership. Existing unaffected WP769 GC and
Core/REAPI refusal evidence is reused; those owners remain unchanged.

Latest test receipts under target/wp771:
tests-1790041500173447643 (Configuration5), tests-1790041848534688301
(BuildAPI24), tests-1790041884985668647 (Analysis19),
tests-1790042088834568440 (Commands1), all `.receipt.json`.
Selected tests total3.325s, maximum0.382s; no runtime failures.
Pinned nightly/offline/locked JSON no-run preparation receipts preserve all
attempts, including builds superseded by focused source/test corrections.
All10 compile/check operations pass:87.462s total,31.080s maximum. Final no-run
receipts are compile-slug_configuration_v2-2, compile-slug_build_api_v2-3,
compile-slug_analysis_v2-2 and compile-slug_commands_v2-1; direct-dependent
check-slug_core_v2-1 and check-slug_cli_v2-1 pass (20.401s/20.399s).
Rust format, diff, archive and plan-status checks pass.

Growth review:416 lines across input projection, byte parser and pure dependency
reconciliation production modules, with thin owner hooks/policy getter.
759 lines in three new test modules exceed the estimate because parser quirks,
artifact/policy semantics and real configured publication need separate proofs.
Configuration/Commands extend existing tables plus one structural restoration
test. No copied feature engine, corpus or retained representation. Utility reuse
keeps compact indexes/tokens call-local and reuses shared Artifact handles; no
new DICE owner or performance claim. Independent final ACCEPT covers actual
source, closure,49 passing selectors and Core/CLI gates. Recorded interval begins
01:36:38 UTC and acceptance is01:57 UTC (~21 minutes), excluding prior source
research/design; concurrent review time is not separately measured. All exclusions
remain; M7A partial and M8 unproved. The next packet must resolve the remaining
CppCompile discovery/execution prerequisites before activation.
