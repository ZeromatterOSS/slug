# Current Slug V2 Work Packet

Packet: WP-7-62-m7a-cc-header-info-and-compilation-context-r1
Status: accepted

## Outcome and compatibility

Execute unchanged rules_cc 0.2.17 cc_info.bzl:create_compilation_context with
artifact-bearing headers, ordinary and exported dependencies, publish its real
provider, and consume it after rematerialization. compile.bzl:282 requires this
producer; current native support is only an empty HeaderInfo sentinel. This
advances the finite C++/Rust bootstrap prerequisites in bootstrap-readiness.md.
Predecessor WP761 at eeb7dd680 accepted link variables/Rust linker-helper with
38 focused tests, Core/CLI checks and independent review; receipt is in Git.

Exact: HeaderInfo constructors, attributes, immutable lists, occurrence equality,
ordered header normalization and dependency merge, unchanged compilation-context
builder for admitted existing inputs. Slug-native: existing structural artifact,
configuration/path identity and Rust Host observations. Unsupported/deferred:
transitive native header scanning/memoization, compile/link action production
and execution, actions2ctx, full CcToolchainInfo callable publication and full
cc_toolchain producer/FDO, and Host-dependent header normalization during module
loading without an observed Host. No full cc_library/Rustc/bootstrap claim; M7A remains
partial and M8 unproved.

## Source, ownership and invariants

Bazel 9.2 revision 8220c6198837d5c13d53fea211cf3282aa12408a:
CcStarlarkInternal.java:1167–1252 binds both named-only constructors and validates
artifact sequences, derived-only optional module slots and HeaderInfo deps.
CcCompilationContext.java:558–698 owns four optional modules, four header lists,
direct dependency nodes and immutable public projections; :743–759 owns identity
and immutability; :828–900 owns create. CppFileTypes.java:93 and
util/FileType.java:115–126 own .inc classification: Windows HOST case-insensitive,
other hosts case-sensitive. Use existing observed Host, never target/exec OS or
ambient filesystem/platform inference. No upstream HeaderInfo tests were found;
pinned-source regressions supply discriminators. Native scanner/flattening tests
are deferred because no scanner or transitive-collection API is activated.
Existing pinned rules_cc corpus and sources.json supply cc_info.bzl:338,429
and compile.bzl:282; no upstream body additions or substitutions.

Normalize public/private independently, ordered-dedup each, move .inc files into
textual after explicit textual inputs. Append merged dependencies' normalized
public/private/textual entries in input order with independent dedup. Preserve
separate-module-header sequence including duplicates. Keep only the supplied
direct deps (not original header_info deps); merged_deps are merge inputs, not an
extra retained graph. Separate module requires corresponding main module.
Module slots reject source artifacts; derived tree artifacts are not rejected.
List/tuple and existing sequence bindings are admitted, depsets rejected.
Module attrs and immutable lists are eight noncallable fields, with no deps attr.
Each constructor has fresh identity; aliases preserve equality/hash across
freezing and publication. Identity never depends on equal header contents.

BuildAPI owns an immutable Arc/Dupe CcHeaderInfo model: complete owned
AnalysisArtifact slots and boxed immutable sequences, shared direct dependency
nodes, and existing occurrence handles. Loading owns validation and native value
facades; Analysis bridges the same model through ordinary AnalysisValue/provider
publication. Generic Starlark CcCompilationContextInfo is not replaced.
Full publication equality checks every artifact owner/slot/list and dependency
edge plus global occurrence bijection, including aliases outside the HeaderInfo
graph. Use visited pairs to avoid repeated DAG traversal. Never retain evaluator
values, action sinks, contexts, raw pointers or borrowed heap data in the model.

This is DICE-retained semantic memory under the configured-node producer and
its existing observed source/configuration/dependency inputs. Normal publication
cutoff, invalidation/restoration, retry/cancellation and graph eviction release
apply. No new DICE key, mutex, cache service or semantic side store. Follow
docs/developers/dice.md. Stage9 rows for WP726/WP758 own reuse: Arc/Dupe,
immutable slices, SmallSet ordered normalization, FxHashMap traversal scratch,
Allocative. No donor import, interner or performance improvement claim.

## Scope and coordination

Root: app/slug_build_api_v2/src/cc_header_info.rs, analysis_value.rs, lib.rs and
cohesive model tests and tests/analysis_value.rs empty-proof migration;
app/slug_analysis_v2/src/analysis_value.rs and cohesive bridge tests;
starlark_rule.rs only projects existing structural configuration Host into the
evaluation context. Rename empty-only variants/functions; no migration shims needed.
Large analysis_value modules receive bounded dispatch and child tests only.

Loading worker: app/slug_loading_v2/src/cc_header_info.rs plus child tests,
cc_common.rs removal/delegation, lib.rs exports, and only necessary updates to
existing empty-header tests. subrule_invocation.rs carries an optional observed
Host OS projection in the evaluator-local AnalysisEvaluationContext, supplied by
the existing configured producer. Missing stays missing; no ambient fallback.
This does not require a rule to declare the Cpp fragment and does not retain a
second configuration. Only nonempty public/private normalization demands Host.
Root provides shared model API before edits.

Integration worker: app/slug_analysis_v2/tests/rustc_map_each/cc_headers.rs and
optional sibling .bzl; mod.rs registration/shared harness only. Use complete
already-vendored bodies and authored caller inputs, genuine configured ctx/File
and real dependency providers. No copied production function fragments.

Reviewer: independent design/final audit. Root owns serialized Cargo, docs,
integration and atomic commit. Docs: manifest/canonical, bootstrap-readiness,
Stage6 current owner and matching Stage9 reuse row only if contract changes.
Estimate 350–650 production and 500–900 proof lines; growth triggers responsibility
review. No action-execution, configuration or provider-callable changes.

## Evidence and validation

Model/Loading tests discriminate order, cross-list independence, Host-sensitive
.inc relocation, separate duplicates, ordinary versus merged deps, fresh versus
aliased identity, hash/freeze/GC/immutable-list behavior, all module slots/invalid
pairs, wrong artifact/sequence/node types and named-only binding. Protect empty
behavior and established no-Host empty initialization.
Publication tests discriminate hidden dependency header/module changes, artifact
owners, repeated DAG aliases and outer value aliases, equal structural rebuilds,
and producer-heap disposal/rematerialization.
Authentic builder integration covers public/private/textual and separate headers,
ordinary/exported dependencies, provider consumption and same-DICE source-input
change/restoration. Existing corpus provenance remains unchanged; no new oracle
workspace assets beyond a short authored invocation. Pinned-source regressions
suffice; no Bazel process needed. Protect prior Rustc/linker proofs.

Resolve pinned nightly through rustup. Serialized offline/locked Cargo JSON
no-run: BuildAPI lib and analysis_value integration, Loading lib, Analysis lib
and starlark_rule integration;
Core/CLI compile checks cover direct dependents. Batch preflight exact selectors
once per executable, then execute selected tests. Preparation cap60s/operation;
tests expected seconds, not an explicit user ceiling. No daemon or binary smoke.
Format, diff, plan-status and archive checks. Independent final review includes
complete ownership/equality and executed evidence, not counts alone.
REPLAN if real compilation-context invocation needs a new semantic owner outside
the allowlist, or evidence contradicts the declared source/ownership contract.
Ordinary implementation/selector failures are corrected within this packet.

Independent design ACCEPT: source-sensitive normalization, derived module slots,
merge ordering and complete alias-aware DAG publication fit existing owners.
Partial final review changed publication traversal to an iterative worklist;
shared-diamond and deep-chain tests discriminate visited-pair and call-stack
behavior separately. Normal Arc release conventions remain; no bounded-stack
deep teardown or retained-size improvement is claimed.

## Terminal evidence

Pinned nightly-2025-09-14 tools resolved through rustup. Serialized offline/locked
Cargo JSON no-run preparation covered BuildAPI lib and analysis_value integration,
Loading lib, Analysis lib and starlark_rule integration. Final preparations exit0:
BuildAPI lib0.966s and integration1.417s, Loading3.921s, Analysis lib6.376s and
integration1.218s. Direct dependent Core and CLI checks exit0 in12.984s/13.635s.
All preparation attempts stayed below60s (maximum23.510s). Commands and full local
receipts are in ignored target/wp762; command families are
`cargo test --offline --locked -p <owner> <target> --no-run --message-format=json`
and `cargo check --offline --locked -p <dependent> --message-format=json`.

33 distinct exact selectors passed, each exit0/one selected/one passed following
batch preflight against its unchanged executable:

- BuildAPI: all7 cc_header_info::tests and migrated empty-header integration.
- Loading: all5 cc_header_info::tests, protected2 empty Cc initialization tests
  and2 cc_common::solib_tests. GC runs supported Auto/Never/Always modes; billion-
  element ranges reject after the first invalid integer instead of allocating.
- Analysis lib: new nonempty heap-disposal/hidden-edge proof, protected empty
  roundtrip and4 native feature publication/GC/hash tests.
- Analysis integration:2 rustc_map_each::cc_headers and protected5 original Rustc
  plus3 cc_linker tests. New authored rule declares no Cpp fragment; actual
  configured Host still reaches normalization through the existing producer.

Latest local test receipts: tests-1790024123351893815 (BuildAPI7),
tests-1790024339988902598 (BuildAPI1), tests-1790024662699590282 (Loading9),
tests-1790024639208900915 (Analysis6), tests-1790024729699719391 (integration10).
Together1.290s runtime. Failures preserved locally: two preparation failures from
missing test imports/temporary borrow; one preflight with incomplete module paths;
three runtime failures (obsolete empty-header hash rejection, GC helper import
not reexported, duplicate BUILD label before native separate-header inputs).
Corrections preserved/increased assertions: positive identity hash, separate
explicit helper import, and single label expanded into duplicate Files in the
authored caller. No upstream body changes or production fallback was needed.

Independent review required and verified bounded iterative DAG comparison and
early invalid-range rejection. Roughly440 added production and850 proof lines
remain within estimates, split across natural BuildAPI/Loading/Analysis owners.
Only the existing observation/configuration producer supplies Host; no lock,
new semantic key, Java helper, daemon, Slug/Bazel binary or build action ran.
Utility-reuse review preserved Arc/Dupe sharing, immutable slices, ordered
SmallSet scratch and Allocative. No performance improvement claim.
Observable gate: full artifact-bearing native HeaderInfo and unchanged public
compilation-context producer/consumer publication with hidden-edge restoration.

Independent final ACCEPT verified the actual diff,33 passing receipts and
Core/CLI checks. Formatting, diff, plan-status and archive checks pass. About
21 minutes from packet selection to acceptance (20:46–21:07 UTC); initial source
research preceded selection, review overlapped work and was not separately timed.
All15 preparation/check operations totaled130.198s including failed attempts.
