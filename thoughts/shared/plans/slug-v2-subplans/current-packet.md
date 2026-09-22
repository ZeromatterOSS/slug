# Current Slug V2 Work Packet

Packet: WP-7-74-m7a-configured-attribute-files-r1
Status: accepted

## Outcome and demand

Provide genuine configured-rule ctx.files and ctx.file projections from prepared
dependency DefaultInfo Files: stable order/dedup, single-artifact schema eligibility,
immutable lists and lifecycle enforcement. Consume them in the authentic toolchain
helper and public compile proof, replacing manual DefaultInfo traversal.
WP773 at2be82ece2 accepts authentic no-profile provider construction with58 tests
and Core/CLI checks; receipt remains in Git. M7A partial, M8 unproved.

Unchanged rules_cc0.2.17 cc/private/rules_impl/cc_toolchain.bzl69–75,96–119
demands these projections. Full rule also needs PackageSpecificationInfo,
TemplateVariableInfo, embedded helper binaries and unconditional build-info
translation. Pinned bazel_cc_build_info.bzl43–62 builds BOTH stamped and redacted
groups regardless --stamp. Never replace these prerequisites with empty providers
or default-None tools. This packet removes an attribute-file prerequisite only.

## Compatibility and sources

Exact admitted dependency kinds: label, label_list and already-supported
label-bearing dictionaries. ctx.files contains DefaultInfo.files only, not
runfiles or executable-only Files. Stable nested-set traversal uses the native
stored graph order and first structural artifact occurrence, not concatenated
child to_list results. A topological child[a,b] under stable parent yields[b,a];
adding preorder[a,c] and the repeated topological child yields[b,a,c]. Optional absent dependencies
expose []; eligible allow_single_file labels expose File or None in ctx.file.
Missing/nondependency/non-single names are absent, not empty fallbacks. Existing
dependency type/provider/visibility validation remains unchanged; context
construction additionally enforces SINGLE_ARTIFACT even when allow_single_file=False.
Native first-phase NO_FILE validation skips cardinality, but context construction
still calls getPrerequisiteArtifact. Present zero/multiple Files fail before
implementation; only absent/filtered prerequisites yield None. ctx.attr remains
a Target; ctx.executable remains separate.
Outputs never enter these inputs. Lists are immutable. Check native reflection/
field presence before claiming dir behavior.

Slug-native: structural artifact/configuration/path identity and diagnostics.
Deferred: full cc_toolchain, status transforms, package-spec/template providers,
new transition kinds, filesystem discovery, execution and full cc_library.
Pinned Bazel9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
StarlarkAttributesCollection.java383–443; PrerequisiteArtifacts.java39–71;
PrerequisitesCollection.getPrerequisiteArtifact; StarlarkRuleContextTest file/
runfiles callers2529–2589; native collection/context lifecycle. Adapt real
attribute/File semantics from broad upstream tests without their unadmitted action
families. Pinned-source regression suffices; no Bazel/compiler invocation selected.
Ordering anchors: NestedSet.java193–198/228–237/497 and
NestedSetBuilder.java190–198; singleton Directory admission follows
PrerequisitesCollection.java259–269 (cardinality only, no regular-file predicate).
The False discriminator follows StarlarkAttrModule.java433–442/127–134 and
RuleContext.java1964–1969: preserve lower-phase validation, complete the previously
missing context check. Prior single_false_multi analysis success was not full
native context parity; its protected expectation must now reject. Independent
source review requires this correction within the same owner and packet family.

## Ownership, representation and lifecycle

ConfiguredNodeAnalysisKey dependencies own resolved schema, selected target/exec
dependency order, filtering and provider data. Project only these values in
synchronous evaluation: no new DICE key/injection/lock, path lookup, provider owner
or configuration token. Preserve actual roots/owners and structural equality.
Use existing AnalysisDepset stable merge, not a second flatten/order engine.

Collection construction is evaluator scratch; plain immutable struct/list values
use existing frozen heap/materializer ownership. No new retained kind. Reuse
Buck2-derived SmallMap/CompactString/Arc/Allocative and existing dense depsets,
without donor import/interner/cache/performance claim. Inherit Stage9 configured
exec dependency utility decision20bbe8661 (evaluator-local ctx.executable).
Published structs/File/list data use existing AnalysisValue. Native
StarlarkRuleContext701–710 guards retrieval through ctx, while the captured
StarlarkAttributesCollection fileObject/filesObject are ordinary StructImpl data
(constructor69–99). Do not add a capability token to captured immutable data:
they may survive/publish after context teardown. Only further ctx access fails.
Success/error teardown, freeze/GC and forwarding preserve these distinct lifetimes.

## Scope and routing

Base2be82ece2; branch codex/wp774-ctx-file-projections-review. Independent design
ACCEPT2026-09-22 before source edits; final actual-diff/evidence review before integration.
Production worker: app/slug_analysis_v2/src/starlark_rule.rs, new child
starlark_rule/attribute_files.rs and local tests; context_lifecycle_tests.rs.
Mechanical AnalysisContext struct-literal updates in existing test children only
if required. No DICE/BuildAPI/Loading or dependency representation changes without
root design reassessment.
Test worker: new tests/attribute_files.rs (or child directory), tests/starlark_rule.rs
module hook and existing FILE_ADMISSIBILITY fixture/expectations for the newly
reached False-flag context phase. Root also repairs its stale no-Host precondition:
base2be82ece2 already supplies action Host facts in test_configuration. Explicitly
omit those facts for the suffix-policy negative, retaining the required conversion
path flavor and original analysis error. ANY/NO_FILE positives use populated Host:
they skip suffix validation but now reach downstream execution-OS fallback.
No configuration or dependency validation production change.
The protected exec/executable test also isolates its deliberate cold/warm graph:
its preceding dependency check already warms :ok now that typed/default fixtures
share structural Host inputs. Both evaluated and reused assertions remain.
Root: rustc_map_each/compile_context/provider.bzl and provider.rs
consume both genuine facades; Stage6/bootstrap docs and scheduling.
Estimate100–250 production and250–500 test lines. New logic belongs in a child,
not the large starlark_rule root; parent adds wiring only. No new upstream bodies.

## Evidence and validation

Real configured rules cover source/generated/Directory Files, target/exec owners,
absence versus missing/ineligible names, distinct ctx.attr/ctx.executable, no
runfiles leakage, label-bearing kinds, diamond depsets, stable order/dedup.
Invalid single-file cardinality fails before effects; mutation fails. Published
File/list data survives same-DICE input/metadata/config changes, A/B/A and missing/
recreate without fresh graphs. Expired ctx access fails on success/error; captured
immutable file structs/lists/Files remain usable and publish normally. Protect
public compile/dependency/exec tests.

Root rebuilds pinned nightly Analysis --lib and --test starlark_rule separately
with offline/locked Cargo JSON --no-run, exact batched preflight and serial tests.
No parallel Cargo. Core/CLI checks, rustfmt, diff/archive/status checks.
Preparation cap60s/operation; tests estimated a few seconds each, not a user hard
deadline. Reuse unaffected evidence; broad suites only at appropriate checkpoint.
REPLAN for new ownership/source contradiction/prerequisite outside this family;
ordinary scoped invocation/test/implementation corrections remain here.

## Acceptance evidence — 2026-09-22

Pinned nightly-2025-09-14 tools resolved with rustup; all Cargo commands serial,
offline/locked, JSON --no-run for tests. Final relevant preparations exit0:
Analysis lib1.217s, integration6.474s, Core check1.018s, CLI check1.268s.
Exact nonignored preflights plus actual execution: **49 selected / 49 passed**.
Library10/10 in0.097s: projection empty structs1, lifecycle success/error2,
unchanged Cargo-runfiles3, compile-action4. Integration39/39 in5.022s: new
projection4, authentic helper/provider/public-compile protected29, ordinary
file/provider/select/exec dependency6. Final receipts under target/wp774:
compile-slug_analysis_v2-5/-11, check-slug_core_v2-2, check-slug_cli_v2-2;
tests-1790045731263117150 and tests-1790046260970971749.receipt.json.
The latter records all exact selectors, commands, statuses and individual logs.
No compiler action, Bazel/JVM, daemon, network or CLI binary invocation.

Discriminators cover all five label-bearing shapes, mixed depset child orders,
diamond dedup, generated target/exec roots, singleton Directory, optional absence,
False-flag absent/zero/one/multiple, pre-implementation rejection, immutable
capture/forwarding, success/error expiry plus freeze/GC, same-DICE A/B/A source
and configuration changes, and missing/recreate. Existing lower validator remains
unchanged. Public helper consumes both real facades; no full-rule claim.

Corrections stayed within this owner: narrowed a leaf-only mutation, accepted the
runtime's capitalized Immutable diagnostic, completed source-required False-flag
context cardinality/presence, and repaired the two baseline-stale protected test
preconditions described above. Missing conversion path flavor was an unsuccessful
fixture attempt, rejected at construction; it is not the retained action-Host
boundary. Neither that assertion substitution nor production bypass was retained.
All failures and superseded receipts remain local. Eleven successful compile
attempts totaled75.651s; all runtime attempts20.397s. Implementation began02:44Z;
final validation03:04Z and acceptance03:05Z (about21min, preceded by about6min prerequisite research).
Independent design and source/correction reviews reused one reviewer; final
actual-diff/evidence ACCEPT: ordering, presence/cardinality, retrieval-only
lifecycle, fixture attribution and all49 tests checked. Review was interleaved
with implementation/corrections, not separately timed. No REPLAN or speedup claim.

Growth reviewed: production logic is a67-line child plus28 net parent wiring;
the child also has36 test lines. New integration444 lines, lifecycle87 net,
mechanical context literals14, existing fixture24 net: about605 test lines,
above the initial500 estimate for distinct mixed-order, phase/cardinality and
capture/GC discriminators. No new copied upstream bodies or external fixtures;
WP773's unchanged source provenance remains authoritative. Existing depset and
frozen-value owners avoid new retained representations or performance claims.
Rustfmt, diff, archive and plan-status checks exit0.
