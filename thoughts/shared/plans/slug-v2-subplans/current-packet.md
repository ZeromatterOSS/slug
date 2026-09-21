# Current Slug V2 Work Packet

Packet: WP-7-65-m7a-cc-context-and-artifact-naming-r1
Status: accepted

## Outcome and boundary

Recover the original analysis ctx from genuine actions and expose toolchain-owned
artifact naming to unchanged rules_cc helpers. WP764 at9e97b5f8c accepted compile
input projection with41 focused tests and Core/CLI checks. This packet advances
two independent prerequisites on the demanded native compile path; it does not
claim full compile invocation, discovery, action production or execution.

Exact: actions2ctx_cheat positional binding/no caller gate, original ctx identity,
same-evaluation root/subrule behavior and locked outer context; named-only
get_artifact_name_for_category over genuine keyed CcToolchainInfo and its retained
feature model, exact uppercase category names/defaults/overrides and normalized
parent/basename composition on observed non-Windows Host. Slug-native: existing configuration/artifact identity
and path domains, valid-Unicode strings and Rust Host observations.
Unsupported/deferred: expired/foreign-evaluation actions, roots/bin/genfiles,
configuration facade expansion, compute_output_name_prefix_dir,
declare_compile_output_file, actual CppCompile/discovery and execution.

## Evidence and architecture decision

Pinned Bazel9.2 source8220c6198837d5c13d53fea211cf3282aa12408a:
CcStarlarkInternal.java346–361,487–494; CcToolchainProvider.java70–77,140–146,
389–390; CcToolchainFeatures.java1338–1344; ArtifactCategory defaults and
PathFragment normalization/getParentDirectory/getChild. StarlarkSubrule.java455–456
unwraps the outer rule; StarlarkRuleContext.java586–587 locks that same ctx during
subrule calls. Adapt source contracts rather than Java implementation assertions.
Unchanged rules_cc0.2.17 compile.bzl213,225,248 and repeated artifact category
queries establish demand; cc_helper_internal and private cc_common wrapper supply
the genuine naming integration. Existing54-body source corpus remains unchanged.

Live BuildAPI AnalysisArtifact.path/short_path returns bin-relative derived output
paths, not a modeled root. Core's configured physical bin root is not a retained
artifact projection. ArtifactRoot.java159–163 rejects an empty derived root.
Therefore an empty getter or getter-only prefix would break unchanged helper
slicing and accepted paths. Root/path policy needs a separate coherent design
across artifact projection, declarations, Args and transport; do not invent it in
this packet. declare_compile_output_file additionally takes a distinct passed
configuration/root but current ctx action owner and cannot ignore either.

## Ownership and lifecycle

Existing AnalysisEvaluationContext and token stack remain the only phase-local
context owner. Add a GC-traced slot in existing evaluator_roots module.extra,
installed with the exact AnalysisContext Value before invoking its implementation.
A scoped guard clears the slot and invalidates the root frame on success/error;
registration rejects an occupied slot so guards cannot clear another registration.
Never store moving Values in Evaluator.extra, a host Arc/map or DICE. Do not clone
or reconstruct ctx. Retrieval verifies genuine AnalysisActions, same stack identity
and a still-live frame (not necessarily top); subrule lookup returns the locked
original outer ctx. Existing field/action checks retain top-frame restrictions.
The rooted slot carries a private existing-token marker: action, evaluator and
registration stack identities must all match, including swapped-evaluator cases.
Missing registration, foreign or expired actions fail closed. Guard/module release
owns cleanup, including exceptional exits; no asynchronous lock/await or new cache.

CcToolchainModel's existing artifact_name_patterns and default table own naming.
Add only a pure query; no parallel retained model, interner or cache. Validate
native provider identity/schema and native feature field. A valid raw constructor
with that provider key is allowed; pinned initializer authority is required for
callable recipes only, not this native query. Unknown/malformed category/provider/
model/arguments fail closed. Loading requires the existing observed analysis Host;
missing observation and Windows Host naming fail closed. BuildAPI exposes an
explicit POSIX pure query, with no ambient OS read or toolchain exec_os inference.
Windows normalization and NUL paths are deferred. Returned names are evaluator
scratch, never identity.
Existing DICE source certificates, full configuration keys, provider publication,
cutoff/restoration and eviction remain unchanged. Read docs/developers/dice.md.

Provider publication currently erases schema classification. Preserve one boolean
on existing ProviderOccurrence, produced from Loading's genuine schemaful versus
schemaless constructor and carried through lowering/materialization. Restored
Loading field storage may remain compact schemaless storage with the preserved
classification; do not infer a schema from populated field names or provider key.
Publication equality includes this observable native classification. Existing
Starlark equality/hash remain unchanged, with no new mixed schema/schemaless parity
claim: StarlarkInfoWithSchema.java214–230 overrides StructImpl equality/hash and
that broader distinction remains deferred. Only the native schema gate is admitted.
No schema list copy, new provider identity or side cache. Existing SmallMap/Arc/
Allocative owners remain; boolean is DICE-retained semantic memory released with
the provider. Utility-reuse skill and Stage9 provider row apply to this narrow
representation correction. Test same-key/fields schema distinction across bridge
and same-DICE source change/restoration; schema-less lookalikes must not gain authority.

## Scope and routing

Context worker: Loading evaluator_roots.rs, subrule_invocation.rs and cohesive
child context lifecycle tests; exports in lib.rs only if needed; Analysis
starlark_rule.rs thin installation hook and cohesive child tests. Large modules
receive bounded hooks, not a second context owner. Do not edit cc_common.rs.
Affected tests/subrule.rs may refresh observation-shard injection and explicitly
assert the existing automatic OutputGroupInfo alongside DefaultInfo/ReturnedInfo;
no production output-group change or weakened lifecycle assertion.

Naming worker: BuildAPI cc_features.rs plus cc_features/metadata.rs/query child
and focused tests; Loading cc_artifact_naming.rs helper and tests plus lib.rs
module registration if needed (coordinate root). Do not edit cc_common.rs.
No new dependencies or retained representations; root reviews normalization edges.

Root: provider classification bridge in BuildAPI analysis_value.rs, Loading
provider.rs (and affected direct callers), Analysis analysis_value.rs with focused
child tests; thin cc_common.rs binding hooks, integration tests/rustc_map_each/
cc_context_naming.rs and .bzl, mod.rs, minimal cc_toolchain fixture reuse/override
hooks, scheduling and Stage6/bootstrap docs. Genuine published toolchain required;
no authored replacement provider/model and no copied upstream body.
Independent reviewer: design checkpoint then final actual diff/evidence.
Root serializes Cargo. Estimate250–500 production and500–1000 proof lines.

## Validation and acceptance

Context: original/repeated ctx identity across real GC; genuine positional binding,
foreign same-number token rejection, missing-root failure, success/error cleanup,
escaped root/subrule actions, live suspended frame lookup with locked outer ctx
and post-subrule restoration; callable/depset root coexistence.
Naming: all22 category defaults, override prefix/extension, normalized nested paths,
invalid child/category and provider/model/shape negatives, raw valid keyed provider.
Use source-exact path normalization; name any unsupported Host-specific edge.
Integration: unchanged public naming helper consumes complete published toolchain;
configuration/provider A/B/A and unused payload invalidation, root ctx identity
and ordinary analysis behavior. Preserve WP763/764 toolchain/compile proofs and
existing lifecycle/subrule tests affected by root cleanup.

Pinned nightly via rustup; serial offline/locked Cargo JSON no-run for BuildAPI,
Loading, Analysis lib and starlark_rule/subrule integrations. Exact preflight then direct test execution;
Core/CLI check.60s preparation operation cap, expected seconds/test; no new user
deadline. No Bazel/Slug/daemon/action execution. Formatting/diff/archive/status
checks and final independent ACCEPT before atomic commit.
REPLAN only for a new owner/path-policy prerequisite or contradictory parity
evidence. Normal compile/test corrections stay in the packet. M7A partial/M8
unproved; coherent roots remain the next explicit design gate.

Independent design ACCEPT: context identity/lifetime, observed non-Windows naming
and preserved native provider classification are bounded and ready to implement.

## Acceptance evidence

Independent final ACCEPT on2026-09-21 against9e97b5f8c confirms the bounded
context/naming/classification contract. Pinned nightly-2025-09-14/offline/locked Cargo JSON no-run:
BuildAPI lib exit0/3.023s, Loading lib0/31.230s, Analysis lib0/7.780s,
starlark_rule integration0/6.226s after adding the context-publication negative,
subrule integration0/1.469s after the fixture correction. Core check0/17.188s;
CLI check0/17.988s. All operations stayed below60s preparation cap. Total
preparation/check128.537s includes an initial Analysis unit compile failure
(private test-only imports, corrected to the existing public transfer API),
earlier successful preparation and the subrule rebuild.

Exact preflight and direct execution: BuildAPI6/6, Loading13/13, Analysis lib8/8,
starlark_rule17/17 (3 new,2 compile inputs,4 full toolchain,8 Rustc/linker),
subrule3/3.47 selected tests pass, exit0, latest runtime2.387s. One preflight
module-path typo was corrected before execution. One preserved runtime failure
was a stale subrule expectation of2 providers versus the already accepted
automatic OutputGroupInfo: the fixture now asserts exact3, DefaultInfo and the
single empty hidden group, retaining all ReturnedInfo/action assertions. It also
injects existing observation shards; all3 protected tests pass after correction.
Commands, full output and receipts remain in ignored target/wp765.

The genuine configured proof covers original ctx identity, complete published
toolchain naming, pattern override, unused configuration payload invalidation,
same-key/fields schema source change/restoration and ctx publication rejection.
Loading separately proves real GC, suspended/subrule frames, foreign/stale/occupied
registration and success/error cleanup. The corpus54 upstream bodies is unchanged;
only authored caller inputs grew. Roughly300 production and1000 proof lines remain
cohesive behind thin large-module hooks; no donor import, new cache or performance
claim. Utility review kept schema metadata to one boolean in the existing owner.
Formatting/diff/status/archive checks pass; no Bazel/Slug action or daemon run.
Packet elapsed approximately21 minutes including parallel source/design/final
review (review time not separately measured). No implementation REPLAN; root-path
architecture was explicitly separated before activation, not replaced by a shim.
