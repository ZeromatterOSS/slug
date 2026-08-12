# Current Slug V2 Packet

Packet: `WP-5-host-selected-module-graph-owner-implementation-r2-cap-design`
Milestone: cross-stage M7 correction design
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: correct the selected-graph implementation cap and typed leaf-error
contract after the first compiling implementation triggered the frozen stop.

## Active correction contract

The first compiling two-file implementation is retained but unaccepted at 874
formatted lines in `selected_graph.rs` plus one `lib.rs` declaration: 875 net
production lines, 115 above the frozen 760-line cap before tests. Independent
implementation and AI-cleanup review found no safe mechanical reduction that
preserves the distinct override/root adapters, breadth-first horizons, nodep
fixed point, MVO selection, validation walks, and retained graph rewrite.

This docs-only packet may edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`.

Freeze `WP-5-host-selected-module-graph-owner-implementation-r2` with the same
two Rust files and all accepted semantics, proof requirements, and terminal
stops, but corrected formatted net caps of 920 production lines, 1,050 test
lines, and 1,970 total. The increase grants no margin for another file, owner,
policy, consumer, or behavior family.

Also require the retained implementation to preserve complete
`HostDiscoveredModuleError` values structurally. Compute failures may retain a
typed predecessor module plus Slug-native message, but completed leaf failures
must use a distinct typed variant such as
`Leaf { module, error: HostDiscoveredModuleError }`; converting those failures
to `CompactString` is forbidden. Add a focused equality test that distinguishes
two typed leaf variants even when their display framing is similar.

Cap this correction at 120 manifest lines, 90 owner-plan lines, 20 canonical
lines, and 230 total. No Rust, Cargo/BUILD, test, public API, graph behavior,
consumer, mapping, or source/materialization change is authorized. Obtain
fresh independent acceptance before resuming Rust. Return `REPLAN` on any
semantic or file-boundary expansion; `REVISE` on one bounded bookkeeping
correction; a second material correction is `REPLAN`.

## Accepted prerequisite

Commit `c997f7e7` accepts one crate-private exact Bazel 9.2 module-version
domain. `BazelModuleVersion` owns normalized equality/hash and parsed
identifier ordering; root/nonroot evaluators and lockfile v28 share it, and
`HostDiscoveredModuleKey::try_new` rejects or normalizes before a DICE key can
exist. The effective override/source seam in `dbeb1fb9` already overlays
root and command declarations, retains provenance, projects command paths once
into the accepted local-path `RepoSpec`, and controls every source classifier.

## Accepted design contract

This accepted design is historical context for the active implementation. It
grants no independent file, action, cap, or scheduling authority.

Audit and freeze one crate-private Host selected-graph DICE owner that follows
pinned Bazel 9.2 discovery then selection without activating loading or any
consumer. The audit must:

- identify the exact key/value/error shapes and the one construction seam over
  `RootModuleFilesKey`, `RootModuleCommandPolicyKey`,
  `HostEffectiveModuleOverrideKey`, `HostDiscoveredModuleKey`, and
  `BazelModuleVersion`;
- preserve root-first override application, the protected `bazel_tools`
  default sentinel and explicit override bypass, ordinary and nodep dependency
  traversal, dev-dependency policy, source-order error/Need precedence, and
  complete-only DICE validity;
- own discovery recursion, minimum-version selection, single-version rewrites,
  multiple-version override ceilings, compatibility-level conflicts, and graph
  rewriting exactly once, with structural equality over every selected module
  key, rewritten edge, complete evaluated module, and retained source
  provenance;
- decide the bounded representation for ordered root/direct dependencies and
  order-insensitive selected identities without inventing canonical repository
  names, mappings, extension names, selected RepoSpecs, lockfile products, or
  consumer-facing public types;
- classify exact, Slug-native, and unsupported/deferred surfaces, and freeze a
  future implementation allowlist, production/test caps, proof matrix, and
  terminal stops; and
- return `REPLAN` rather than guessing if discovery requires a missing policy,
  yanked-version owner, registry metadata/source capability, compatibility
  level, multiple-version normalization, graph recursion/cycle contract, or
  post-selection identity that no accepted leaf retains.

## Accepted design scope

This scope is historical and non-authorizing.

This design packet may edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`.

The root may inspect Bazel 9.2 source and live Rust owners read-only. Cap net
manifest growth at 420 lines, owner-plan growth at 360 lines, canonical growth
at 45 lines, and 825 total. No Rust, Cargo/BUILD, wire/schema, fixture/oracle,
public API, legacy `ResolvedGraph`, source/materialization change, mapping,
loading, configured analysis, command, execution, or consumer edit is
authorized. Obtain a fresh independent reserved-architecture review.

Return `REPLAN` on a missing prerequisite or any required fourth file. Return
`REVISE` on one bounded design-contract correction. A second material
correction is `REPLAN`. No production implementation may begin before
independent `ACCEPT` and an explicitly activated implementation successor.

## Compatibility

Exact: admitted Bazel 9.2 discovery/selection behavior only if every semantic
input and retained identity is already owned and the audit freezes a bounded
implementation.

Slug-native: Rust/DICE type names, compact collection choices, lifecycle
instrumentation, and diagnostic framing outside named exact messages.

Unsupported/deferred: canonical/full repository mappings, extension unique
names/execution, lockfile production, package/Bzl loading, configured
analysis/toolchains/actions, Test, execution/results/BEP/coverage, native
Windows command-path semantics, JVM/Java, and exact Bazel identity bytes.

## Accepted implementation evidence

`c997f7e7` is independently accepted within its exact six-file boundary.
Focused grammar/equality/hash/order tests, retained root/nonroot normalization,
checked Host construction, real-DICE build-suffix and semantic A/B/A/reuse,
100 lockfile-v28 regressions, all 303 owner tests plus integrations/docs, and
the full loading suite pass. The two known core baseline failures remain the
unrelated deferred external-visibility and uninjected legacy path-epoch rows.

## Proposed completed design audit

Pinned Bazel 9.2.0 establishes that `compatibility_level` and
`max_compatibility_level` are deprecated no-ops. `module()` always stores
compatibility level `0`; `bazel_dep()` always stores max compatibility level
`-1`. A non-default value emits a warning only while evaluating the root
MODULE. The existing Slug values therefore already contain the exact selection
inputs; the missing root warning is an evaluator-diagnostic boundary, not a
selected-graph identity or ownership prerequisite.

The selected graph is one new crate-private
`HostSelectedModuleGraphKey { workspace: NormalizedAbsolutePath }`. It
computes, in order:

1. `RootModuleFilesKey`, then `RootModuleCommandPolicyKey`;
2. the candidate effective override names from root declaration order followed
   by command-only names in their canonical map order, asking
   `HostEffectiveModuleOverrideKey` for every value rather than merging or
   projecting either accepted input map;
3. a root interim adapter with `ModuleKey::ROOT`, source-ordered ordinary and
   nodep dependency specs, original dependency specs, and the reserved
   `bazel_tools@` ordinary dependency when it is not already present; and
4. discovery rounds and selection over `HostDiscoveredModuleKey::try_new`
   leaves only.

Computing the effective key for every candidate name also enforces the accepted
root-name rejection even for an otherwise empty graph. Effective nonregistry
values rewrite a requested version to empty; a nonempty single-version
override rewrites to its normalized pinned version; multiple-version and empty
single-version overrides preserve the request. A request naming the root module
rewrites to the root key before any leaf lookup. Effective absence for
`bazel_tools` installs the default empty sentinel; command `bazel_tools`
bypasses it through the accepted effective leaf. The already accepted explicit
root `bazel_tools` terminal remains unsupported and is not widened here.

### Discovery contract

Discovery is a roots-first breadth-first sequence. Each horizon preserves
first-seen dependency order, deduplicates exact typed module keys, computes all
leaf keys, unions every Need, and records complete failures in horizon order.
A complete failure wins over any Need from the same horizon, matching Bazel's
bulk lookup followed by ordered exception scan; otherwise the unioned Need is
transient, invalid, and self-unequal.

Ordinary edges always discover their transformed keys. A nodep edge discovers
only when its module name was present in the preceding completed discovery
round. Unfulfilled nodep names that become present cause a fresh whole-graph
round over retained DICE leaves; the fixed point filters all still-unfulfilled
nodep edges. The graph retains the complete discovered BFS sequence, including
modules later pruned by selection. Exact dependency cycles terminate through
the seen set and do not create recursive DICE keys or locks.

Root dev dependencies are already filtered structurally by
`RootModuleFilesKey` using the retained command policy. Every nonroot
dev-dependency directive is already discarded by its evaluator. No second
policy edge or filter is added.

After discovery, every effective root or command override name must identify a
discovered module. Multiple simultaneous unused-override diagnostic order is
Slug-native (root declaration order, then command-only canonical order); the
existence check and typed offending name are exact. Registry/module/source
errors remain the accepted leaf errors, wrapped with the predecessor key chain
without string classification.

### Selection contract

The selected owner uses the shared `BazelModuleVersion` for all keys,
ceilings, maxima, rewrites, equality, and ordering. Since compatibility levels
are pinned no-ops, every ordinary module-name selection group has compatibility
level zero and every dependency has max level minus one; no compatibility
cartesian-product implementation is admitted or needed.

For each root multiple-version override, every allowed typed version must exist
in the discovered graph. A discovered version maps to the lowest allowed
version greater than or equal to it; no ceiling is retained as an invalid group
until that group becomes reachable. Other versions of one module name form one
group. Each group selects its maximum discovered version. Each distinct
dependency spec then resolves to the selected version of its group; the pinned
no-op compatibility inputs make that result singular.

The owner walks the rewritten graph twice:

- the validation walk includes fulfilled nodep edges and rejects a reachable
  multiple-version group with no ceiling, two reachable versions of one
  non-multiple module name, and duplicate ordinary dependency keys under
  different apparent repo names; and
- the final walk ignores nodep reachability, producing the exact roots-first
  BFS resolved graph. The unpruned graph retains every discovered module in
  discovery order with the same rewritten dependency strategy.

Direct-dependency accuracy, Bazel-version compatibility warnings/errors, yanked
metadata/policy checks, selected-yanked lockfile products, canonical repository
names/mappings, extension unique names, RepoSpecs, and final module conversion
all occur after `Selection.run` in pinned Bazel and remain deferred. Registry
MODULE attempt URLs/hashes and nonregistry/builtin identity remain structurally
present through each accepted `HostDiscoveredModuleProvenance`; the selected
owner does not duplicate source bytes, materialization roots, or override
payloads.

### Retained representation

A crate-private `HostGraphModuleKey` is `Root` or
`Module { name: CompactString, version: BazelModuleVersion }`. Dependency
specs use that typed version domain and retain apparent repo name only for
ordinary edges. One `HostGraphModuleSource` owns either the evaluated root
module or an Arc-backed discovered module with its complete provenance.

Each rewritten module value shares its source and retains ordered transformed
ordinary deps, ordered original ordinary deps, and ordered fulfilled nodep
deps. `HostSelectedModuleGraph` retains two Arc slices of entries:
`resolved` in final roots-first BFS order and `unpruned` in discovery order.
Local maps/indices used during computation are not retained. Structural
equality includes entry order, typed keys, transformed and original edges,
nodep edges, evaluated module semantics, and source provenance. Clone cost is
Arc/slice dominated; there is no interner, global cache, lock, physical-root
identity, public export, or second production graph.

Complete successes and typed errors are stable DICE values. Needs are invalid
and unequal. Captured evaluation events remain owned by the computed root and
discovered leaves; the selected graph stores no duplicate batch and introduces
no consumer/publication edge.

## Unaccepted predecessor implementation contract

This retained predecessor contract is historical context only and grants no
file, action, cap, or scheduling authority.

The predecessor proposed implementation only in:

- new `app/slug_bzlmod_v2/src/selected_graph.rs`; and
- `app/slug_bzlmod_v2/src/lib.rs` for one private module declaration.

All implementation and tests are colocated in the new file. Cap formatted net
growth at 760 production lines, 1,050 test lines, and 1,810 total. A third Rust
file or any public export is a terminal `REPLAN`.

Required proof:

- pinned no-op compatibility/max-compatibility values and structural
  independence from non-default spellings, while explicitly preserving the
  deferred root warning boundary;
- root-name, ordinary, default-builtin, command-builtin, single-version,
  nonregistry-empty, and multiple-version rewrite tables;
- roots-first BFS, exact-key dedupe, diamond/cycle behavior, predecessor error
  chains, same-horizon complete-error-over-Need, Need union, and complete-only
  equality/validity;
- fulfilled/unfulfilled nodep multi-round fixed points and final nodep-only
  pruning;
- highest-version selection, numeric/spelling ordering reuse, multiple-version
  ceiling success/missing/unreachable behavior, unused override checks, and
  duplicate apparent-name dependency failures;
- resolved and unpruned order/edge/source/provenance equality, including
  registry order/hash, builtin identity, and admitted nonregistry lifecycle;
- real-DICE root/dependency/version/override/source create-edit-delete-restore
  and semantic A/B/A, spelling-equivalent reuse, cold/warm activation, and
  zero recursive graph/lock/fresh-engine bypass;
- protected full `slug_bzlmod_v2`, direct `slug_loading_v2` compile/suite,
  formatting, diff, exact allowlist/caps, archive classification, and
  structural scans proving no legacy `registry.rs::ResolvedGraph` or
  `compare_versions` production edge; and
- fresh independent representation/DICE/selection implementation review.

The implementation must stop with `REPLAN` on a required post-selection
policy, yanked/metadata fetch, mapping/RepoSpec/final-module conversion,
compatibility cartesian strategy, second override merge, raw filesystem or
network observation, graph recursion through DICE, lock across compute, public
type, third Rust file, cap excess, or reviewer blocker.

## Proposed compatibility classification

Exact: for the admitted callerless graph, pinned Bazel 9.2 override
application, reserved builtin default and command bypass, ordinary/nodep
discovery rounds, no-op compatibility inputs, shared version ordering,
single/multiple-version selection, reachability, roots-first ordering,
dependency rewriting, complete errors, and structural invalidation.

Slug-native: Rust/DICE type names, typed error wrappers/predecessor framing,
multiple-invalid-override error order, compact Arc/slice representation,
activation instrumentation, and diagnostics not named exact above.

Unsupported/deferred: explicit root `bazel_tools` overrides, root no-op
attribute warnings, post-selection direct-dependency checking,
Bazel-compatibility checking, yanked metadata/policy and lockfile products,
canonical/full repository mappings, final selected RepoSpecs/modules,
extension identities/execution, loading and every consumer, configured
analysis/toolchains/actions, Test, execution/results/BEP/coverage, native
Windows command-path semantics, JVM/Java, and exact Bazel identity bytes.
