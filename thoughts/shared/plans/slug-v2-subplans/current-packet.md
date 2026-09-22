# Current Slug V2 Work Packet

Packet: WP-7-75-m7a-package-specification-provider-r1
Status: accepted

## Outcome and demand

Publish genuine PackageSpecificationInfo from native package_group analysis,
including external groups and tracked includes. Expose contains and preserve full
semantics through provider publication into the unchanged rules_cc toolchain
helper, replacing its authored None allowlist with a real group provider.
This removes a full cc_toolchain prerequisite, not a full-construction or execution
claim. WP774 at421bb60f1 accepts ctx.file/files with49 tests and Core/CLI checks;
its receipt remains in Git. M7A partial, M8 unproved.

## Compatibility and sources

Exact within already-admitted package-spec syntax: native-only provider creation,
Target membership/indexing, stable transitive group composition and per-group
negative exclusions; repository-aware exact/subtree/public/private membership.
contains is positional-only, Label or string. Strings parse canonical labels
(// means main; @repo and @@repo are canonical), never caller mapping; Label
arguments carry already-resolved mapping. Relative strings and bad types fail.
dir exposes contains; type PackageSpecificationInfo, repr struct(). NativeInfo
ignores callable fields: distinct instances have equal Starlark equality/hash
despite differing membership. DICE/publication equality MUST compare all group
semantics. Global builtin key stays nonconstructible. Immutable captured provider
data survives context teardown, freeze/GC and downstream publication.

Slug-native: structural publication/configuration identity, diagnostics and current
error transport. Raw Java hash bytes are not claimed. Deferred: Loading's rejected
explicit repository package-spec syntax, external declaring visibility,
TemplateVariableInfo, status transforms, embedded tools, full cc_toolchain and
execution. No empty-provider fallback or untracked repository inference.

Pinned Bazel9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
PackageSpecificationProvider.java40–60,73–113 (checked includes/stable union/
canonical contains); PackageSpecificationProviderApi.java31–49;
NativeInfo.java58–137 (method-excluding equality/hash/repr); Allowlist.java94–98;
PackageSpecification.java464–596 (unordered exact sets, ordered subtree lists,
local negatives); Label.java150–165. Adapt AllowlistTest.java120–149,155–230,
286–321 as authored BUILD/Starlark regressions, not Java probes.
rules_cc0.2.17 cc_toolchain.bzl60–67,117,350–353 indexes the provider; unchanged
cc_toolchain_provider_helper.bzl295 retains it. Membership is an API regression,
not observed allocator execution. Reuse WP773 provenance; no new upstream bodies.

## Ownership and representation

Move sole PackageGroupContents value/matcher below Loading into BuildAPI. Keep
Loading's parser and RuleVisibility adapters, reexport the type for Query users.
Preserve parsing/rebinding. SmallSet exact members, immutable Arc subtree slices,
Allocative; hash exact sets unordered and subtree lists ordered, consistently with
Eq. Reuse Stage9 visibility dispositions f9ae7337/76025ede and existing dense
Depset utilities; no donor semantic code, duplicate matcher or V1/global registry.

PackageSpecificationInfo wraps shared stable Depset<Arc<PackageGroupContents>>.
Groups remain separate leaves: OR whole groups, never combine negative buckets.
Includes are transitive children followed by direct local contents. Existing
ConfiguredNodeAnalysisKey package-group compute_join/cycle guard supplies tracked
includes, Need/errors and ordered edges. Reject missing/wrong-kind includes;
retain analysis cycle errors, not Query's different cycle suppression.
External groups use canonical tracked package loading and contents rebinding.
Admit only external null PackageGroup and include references, not unrelated
declaring visibility or target kinds.

One typed ProviderValue and AnalysisValue payload supports direct forwarding and
nested publication. Starlark equality is deliberately separate from complete
semantic publication equality, including depset/map-containing payloads. No new
DICE key, injection, cache, lock, interner, occurrence counter, filesystem scan or
evaluator borrow. Immutable data is DICE-retained; builders/traversal/comparison
buffers are scratch. Existing invalidation, publication cutoff, cancellation and
graph lifetime own release. Dense graph sharing/cheap handles, no performance claim.

## Scope and routing

Base421bb60f1; codex/wp775-package-specification-review. Independent design ACCEPT
2026-09-22 before production edits; final actual-diff/evidence review before integration.
Worker1: BuildAPI new package_specification.rs child/tests, lib.rs exports,
analysis_value.rs closed-kind wiring, providers/mod.rs typed payload/lookup;
Loading visibility.rs matcher extraction/adapters/tests and reexport if needed;
package.rs single parser-call adapter since the moved type cannot own Loading parsing.
Worker2: Analysis dice.rs minimal native branch/gate wiring, new cohesive
package_specification.rs child/module hook; analysis_value.rs materialize/lower
wiring and child facade/tests; starlark_rule.rs native return decoding. key.rs
comment only if needed. A Loading facade/hook may replace the Analysis-local facade
if required by existing ownership; root must approve before crossing worker scopes.
Root: new tests/package_specification.rs; tests/starlark_rule.rs hook and obsolete
package-group predicate expectation; protected delegating fixture's baseline-stale
source DefaultInfo assertion now checks the exact singleton source File (base
421bb60f1 already publishes it; production unchanged). rustc_map_each/compile_context/provider.bzl
and provider.rs genuine allowlist proof; Stage6/bootstrap/Stage9 and scheduling.
Other edits need root reassessment. Large parents add only closed dispatch/wiring;
new semantics belong in children. Estimate350–650 net production and350–650 test
lines plus matcher relocation; no new copied fixtures or crate dependencies.

## Evidence and validation

Cover empty/public/private, exact/subtree boundaries, repository mismatch;
diamond includes and local-negative versus included-positive; missing/wrong-kind/
cycles; external groups and mapped Label versus canonical string; binding/type/
relative-string rejection; key nonconstructibility; type/repr/dir/equal-hash and
different contains results; immutable freeze/GC, direct/nested forwarding.
Same-DICE group/include A/B/A, missing/recreate, external restoration and unchanged
helper publication must discriminate complete semantic cutoff despite constant
Starlark equality. No fresh graph replaces invalidation proof.

Protect Loading visibility/group invalidation, repository rebinding and global
partition; Query shared matcher compile; Analysis delegating nodes/runfiles/
ruleclass-bypass and WP774 helper/compile selectors. Root serializes pinned nightly
offline/locked Cargo JSON --no-run BuildAPI/Loading/Analysis builds and batches
exact nonignored preflight before test execution. Core/CLI/Query checks.
Preparation60s/operation; focused runtimes estimated seconds, not user deadlines.
Rustfmt, diff/archive/status checks. No daemon/Bazel/compiler action/network.
REPLAN only for new ownership/source contradiction needing broader activation;
ordinary scoped implementation/invocation/test corrections stay in packet.

## Acceptance evidence — 2026-09-22

Pinned nightly-2025-09-14 tools resolved via rustup. Serial offline/locked Cargo
JSON --no-run builds, then exact nonignored preflight and actual execution:
**73 selected / 73 passed**,5.610s aggregate runtime. BuildAPI5 in0.005s; Loading
library4 in0.012s, bzl_invalidation1 in0.008s, build_file_loading2 in0.011s;
Analysis library14 in0.108s and starlark_rule47 in5.466s. The latter includes all
WP774 integration39 plus new package-group4, helper allowlist1, protected
delegating/runfiles/ruleclass3. Analysis library includes new facade/GC2,
protected bypass/external-visibility2 and WP774 library10.

Receipts/logs under target/wp775: tests-1790047148323337319,
1790047148410699623,1790047189512316365,1790047189557584844,
1790047605522647252 and1790047605700833066.receipt.json record commands,
exact selectors, exit0 and counts. Final relevant build receipts all exit0:
compile-slug_build_api_v2-2 (2.268s), compile-slug_loading_v2-1/-2
(32.576s/21.764s), compile-slug_analysis_v2-4 (lib+integration7.734s).
Query check1 exit0 in18.641s; fresh Core/CLI check2 after parser correction exit0
in1.017s/1.268s. Eight compile attempts117.512s, five checks57.959s, all runtime
attempts11.153s including superseded failures. Preparation stayed under60s each.
No Bazel/JVM, daemon, compiler action, CLI invocation or network operation.
Rustfmt/diff/archive/plan-status checks exit0.

Distinct proof: whole-group OR restores a locally excluded package through a
diamond include; canonical strings differ from BUILD-mapped Label arguments;
main/external group and included-leaf A/B/A plus missing/recreate retain semantics
through direct native provider forwarding, structs/lists/dictionary keys/depsets
and unchanged toolchain-helper publication. Native equality/hash remains constant
while full publication differs. Freeze, source-heap disposal/rematerialization and
supported forced GC retain membership. Invalid includes/cycles and binding reject.

Corrections: root-repository test helper uses root(), not empty-name constructor;
Loading test names required bzl_module prefix (failed preflight executed zero);
equal-key dictionary literals correctly reject, so replacement/hash proofs use
sequential assignment. Root .bzl global mapping is not admitted: the integration
proof instead resolves a genuine BUILD attr Label and publishes it. One production
correction replaces insufficient canonical-string normalization with the existing
validated FirstRoundCanonical parser after an absolute-input guard; this rejects
recursive patterns while admitting native no-colon/repository shorthand. The
baseline-stale source DefaultInfo assertion was corrected as scoped above.
All relevant gates were rebuilt/rerun; no failure waived or owner broadened.

Growth: roughly330 net production lines plus matcher relocation; cohesive new
BuildAPI/Analysis children and small parent dispatch/branch wiring. About780
test lines exceed the650 estimate for separate immutable native/semantic equality
container proofs, GC and main/external/helper invalidation; no copied upstream
body or new external fixture. Utility reuse preserves the sole matcher and dense
graph ownership, with no interner/cache/performance claim. Independent design and
final source/correction reviews reused one reviewer; terminal evidence ACCEPT
at03:28Z confirms73/73 and Core/CLI/Query checks, separate equality domains and
tracked invalidation. About22min from research through acceptance; reviews were
interleaved, not separately timed. No REPLAN or claimed workflow speedup.
