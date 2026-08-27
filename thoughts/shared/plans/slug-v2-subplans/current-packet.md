# Current Slug V2 Packet

Packet: `WP-4-5-7A-loading-canonical-repository-route-owner-implementation`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `6b9dfd790`.

Result: implement the reviewed apparent-free canonical repository route and
move its sole definition/mapping DICE ownership from core to loading without
activating target-pattern expansion.

## Accepted architecture

Commit `6b9dfd790` freezes the prerequisite. Bazel 9.2 parses selected module
registration patterns in the declaring canonical repository and its complete
mapping. A root-apparent `RootRepositoryRouteKey` therefore cannot represent a
selected module that has no root-visible alias. The shared source route is
keyed only by workspace plus `CanonicalRepoName`; apparent spellings remain
contextual mapping, diagnostic and presentation inputs.

The route matrix is root, built-in `bazel_tools`, selected-registry, selected-
nonregistry/direct-local and generated. Selected definitions precede generated
fallback. Only the generated carrier retains the existing selected repository-
file effect seed; neither the route nor mapping owner computes the effect plan.
The later source adapter remains its sole consumer. The accepted legacy /
observed outer-error, Need, semantic-terminal, epoch merge, complete-only
equality/validity and A/B/A rules remain unchanged.

Pinned Bazel 9.2 remains behavior authority. Zabel is peer guidance for the
parse/context/discovery/consumer split and compact producer-owned state only;
copy no implementation or compatibility claim.

## One carrier and three loading owners

Add one immutable, doc-hidden `HostCanonicalRepositoryRoute` carrier in
`slug_bzlmod_v2`. Its structural state is workspace, canonical name and exactly
one source disposition. It contains no apparent repository spelling.

- Root retains the accepted selected root definition and mapping context.
- Built-in retains the pinned `BuiltinBazelToolsSnapshot` route identity and
  an empty built-in mapping.
- Selected-registry and selected-nonregistry retain their complete selected
  definition, repo specification, local-path policy and immutable mapping.
- Generated retains the complete generated repository row: owner/ordinal
  effect seed, internal name, repo specification, canonical mapping context
  and immutable mapping entries. It does not retain the whole certificate in
  addition to those immutable source facts, and it does not compute or retain
  a file-effect plan at this source-definition layer.

The carrier exposes a borrowed view for kind, workspace, canonical identity,
repo specification, local-path policy, mapping lookup and optional generated
effect seed. Constructors validate polarity and fail closed; they do not
perform DICE work or source IO. Reuse existing `CompactString`, `SmallMap` or a
compact immutable `Arc` mapping, structural hashing and `Allocative`; add no
interner, global cache or duplicate route graph.

Split the current 1,266 production lines of core's
`generated_repository_definition.rs` along their existing cohesion seams:

1. a private loading generated-definition owner scans selected extension
   demand/certificates and projects the one requested generated row;
2. public doc-hidden legacy/observed `HostCanonicalRepositoryRouteKey` owners
   handle built-in directly, otherwise selected before generated, and publish
   the bzlmod carrier; and
3. public doc-hidden legacy/observed any-context apparent-mapping owners use
   root mapping for the root context and the canonical route for nonroot
   contexts.

There is exactly one implementation of each semantic DICE key. Core's old
module becomes `#[cfg(test)]` fixture support only. Its production consumers
import the loading owners directly. Existing root-apparent definition/route
and generated-package-route values remain presentation adapters; their public
equality, diagnostics and ordinary/root-build admission do not change.

## Builtin and rule boundary

This is generic repository/loading infrastructure. Bazel 9 BCR Starlark owns
rules and control flow, including `cc_internal`; `cc_common` is one demanding
consumer of the generic Rust host-builtin ABI. Add no C++-specific parser,
evaluation, rule, provider, action or toolchain behavior.

Future builtin work remains grouped by reusable category: values/carriers,
provider/rule/aspect declarations, depset/collections, labels/target patterns,
actions/artifacts, configuration/fragments/toolchains and repository/loading
services. This packet implements only the last category's canonical route
prerequisite.

## DICE, request and lifetime contract

Follow `docs/developers/dice.md`. Each owner computes dependencies through
their natural keys, retains only immutable complete values, publishes no Need
as complete, relies on cancellation for abandoned compute and holds no lock
across a compute. Legacy and observed drivers share one semantic finisher.

Built-in routing is source-independent and must not activate selected or
generated lookup. Root uses the selected root definition. For nonroot/non-
builtin names, selected success terminates before generated lookup; only a
typed selected Missing admits generated fallback. Other selected errors are
terminal. Observed generated fallback merges selected then generated-
definition epochs; all other successful branches forward only their natural
predecessor epoch. No route or mapping branch computes
`HostSelectedRepositoryFileEffectKey`; doing so would couple name resolution
to source effects. The existing later source adapter consumes the retained
owner/ordinal seed and structurally retains the resulting effect plan.

Any-context mapping keeps its key identity as workspace, declaring canonical
repository and apparent name. That contextual apparent spelling is correct
mapping identity and is not canonical source identity. Root context uses
`HostRootRepositoryMappingKey`; nonroot context uses only
`HostCanonicalRepositoryRouteKey`. Mapping misses and context mismatches remain
typed and fail closed.

Retained route equality includes every configuration/source-affecting input:
selected definitions and mappings, local-path policy, generated owner/ordinal,
repo specification and generated mapping. Display names and diagnostics never
replace structural state. No command/session cache, physical root, materialized
path, source tree, request revision or manual lock is introduced.

## Compatibility

- **Exact:** no named surface is activated. Existing selected-before-generated
  resolution, contextual mapping, error/Need/epoch order and root-apparent
  adapter results remain byte/structurally unchanged. Built-in canonical lookup
  publishes the already pinned `bazel_tools` identity without source IO.
- **Slug-native:** Rust carrier layout, DICE key names, compact mapping carrier,
  structural hash/equality and observation epochs.
- **Unsupported/deferred:** target-pattern expansion, wildcard conflict
  warnings, family filters/deduplication, registration activation, configured
  validation, broader builtins, rules and actions.

## Implementation allowlist and caps

Only these production/test files may change:

- `app/slug_bzlmod_v2/src/canonical_repository_route.rs` (new carrier and
  focused value tests) and `app/slug_bzlmod_v2/src/lib.rs` (exports);
- `app/slug_loading_v2/src/generated_repository_definition.rs` (private moved
  owner), `canonical_repository_route.rs` (public route keys),
  `canonical_repository_mapping.rs` (public mapping keys),
  `canonical_repository_route_tests.rs` (moved and new proof), and `lib.rs`
  (wiring/exports);
- `app/slug_core_v2/src/runtime/generated_repository_definition.rs` (reduce to
  shared `#[cfg(test)]` fixture helpers only);
- `app/slug_core_v2/src/runtime/generated_package_route.rs` and
  `root_apparent_repository_definition.rs` (direct loading imports and
  structural-guard path updates).

Mechanical movement may relocate at most the existing 1,266 production and
2,817 proof lines from the core owner. Net-new caps are 500 production and 600
proof lines; caller rewiring is capped at 120 changed production and 160
changed proof lines. Each new production owner stays below 650 lines. No Cargo
dependency, fixture, oracle, query, CLI, evaluator or ruleset file may change.

If preserving core fixture helpers would require production test-support
exports from loading, STOP and split the fixture helper instead. Do not expose
private generated scan/certificate internals merely to keep old test pattern
matches. Moved loading tests may use module-private internals; cross-crate
consumers receive only the doc-hidden carrier/key/view/error contracts they
need.

## Required proof

Reuse the accepted owner tests and move them with the semantic keys. Add only
the demonstrated gaps:

- carrier constructor/hash/equality tests for all five dispositions, including
  no apparent field or apparent-alias influence;
- built-in legacy/observed direct success with empty epoch and proof that
  selected/generated/effect/source owners are not activated;
- selected-registry and selected-nonregistry route views, source policy,
  mapping and A/B/A;
- generated repo-spec/mapping/effect-seed identity, selected-before-generated
  ordering and proof that route/mapping lookup never activates the effect key;
- root route and contextual root/nonroot mapping A/B/A;
- outer-before-Need-before-terminal order, exact epoch forwarding/merge,
  complete-only equality/validity and cancellation nonpublication;
- unchanged root-apparent definition/route and generated-package-route results;
  and
- structural guards proving no canonical route/mapping `Key` implementation
  remains in core and no apparent spelling enters the carrier.

Run focused carrier and loading owner tests, full bzlmod and loading suites,
then locked core and query checks and a rebuilt locked `slug_cli_v2` serially.
Run Rust formatting, `git diff --check`, scope/cap/dependency/no-lock checks and
the archive-baseline gate. Require independent terminal DICE/public-boundary /
retained-representation review.

## Stops

STOP and `REPLAN` for an apparent name in carrier/key identity; a duplicate
semantic key; generated lookup after selected success or non-Missing failure;
generated-effect activation from any route or mapping owner; direct filesystem,
catalog or materialization access; a retained certificate plus duplicate
projected generated row; a physical root/source tree; changed root route
admission/diagnostics; target-pattern or rule activation; a new dependency;
production test-support exports; an owner above the complexity cap; cap /
allowlist expansion; global state; or a lock across DICE compute.

## Immediate predecessor

Commit `6b9dfd790` freezes this cross-crate identity/ownership decision after
independent review required selected-nonregistry/direct-local as a distinct
source disposition and rereview returned `ACCEPT`. Commit `4fabef5e0` supplies
the accepted external subtree consumer that will use this route in a later,
separately bounded adapter packet.
