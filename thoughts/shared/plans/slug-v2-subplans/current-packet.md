# Current Slug V2 Packet

Packet: `WP-4-5-7A-loading-canonical-repository-route-owner-design`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `4fabef5e0`.

Result: freeze a loading-owned canonical repository-definition and route
boundary before registration target-pattern expansion. This is a docs-only
architecture packet; it changes no production behavior.

## Why this prerequisite exists

Commit `4fabef5e0` accepts source-neutral recursive package discovery for an
already authenticated external `RootRepositoryRoute`. The next registration
step must expand each selected module's retained target-pattern text in that
module's canonical repository and final apparent-to-canonical mapping.

`RootRepositoryRouteKey` cannot be the shared source-identity owner. It is
keyed by a root-apparent spelling, carries that spelling in route equality and
has root-query-specific admission modes. A selected module's own `//pkg:t`
pattern has a canonical repository context but need not have any root-visible
apparent alias. Requiring one would make registration semantics depend on the
root module's presentation namespace.

Slug already computes the needed selected-before-generated canonical
repository definition and any-context apparent mapping, but their DICE keys
are private to `slug_core_v2` in
`runtime/generated_repository_definition.rs`. Core is an orchestration
consumer and cannot be the reusable owner for loading and query. Adding a
second implementation would create competing semantic keys and invalidation
paths.

Pinned Bazel 9.2 remains behavior authority. Its registration functions parse
each declaration using the declaring module's canonical repository and full
repository mapping, then expand through the common target-pattern machinery.
Zabel is peer guidance for keeping parsing, contextual name resolution,
package discovery and consumer filtering separate; it is not a source of
behavior or compatibility claims.

## Required ownership decision

Design one immutable canonical repository route value whose semantic identity
contains:

- workspace identity and canonical repository name;
- root, built-in, selected-registry, selected-nonregistry/direct-local or
  generated source disposition;
- the complete selected definition or generated definition/effect plan needed
  for source preparation; and
- the repository's own immutable mapping context where applicable.

The canonical value must not contain an apparent repository spelling.
Apparent names belong only to contextual mapping inputs, diagnostics and
presentation adapters. The value shape should live with existing repository
route data in `slug_bzlmod_v2`; the DICE producer and canonical-definition /
any-context-mapping owners should live in `slug_loading_v2`.

The loading key is exactly workspace plus `CanonicalRepoName`. It resolves
root and built-in repositories directly, then selected-registry and selected-
nonregistry/direct-local definitions before generated definitions. The
nonregistry route retains its exact local source policy and immutable mapping
without acquiring a root apparent alias. A generated route consumes the
already accepted selected repository-file effect owner; root, built-in and
both selected route families do not activate generated effects. Legacy and
observed forms preserve the existing outer-error / Need / semantic-terminal
order, exact observation merging, complete-only equality and validity, and
A/B/A invalidation.

`RootRepositoryRoute` remains an admitted adapter for existing root-apparent
callers during this prerequisite. The design must state how it projects from
or shares the canonical route without changing its accepted public equality,
diagnostics or ordinary-versus-root-build admission. There must be exactly one
canonical-definition DICE owner after extraction.

## Builtin-category architecture

This work is general loading infrastructure, not a C++ parser or Rust rule
implementation. Bazel 9 BCR Starlark supplies the rules and control flow,
including `cc_internal`. `cc_common` is one demanding consumer of the generic
Rust host-builtin ABI.

Future builtin work is planned by shared capability category rather than by
individual ruleset: value constructors and immutable carriers; providers and
rule/aspect declarations; depset and collection operations; labels and target
patterns; actions/artifacts; configuration, fragments and toolchains; and
repository/loading services. Category-level ABI, diagnostics, lifetime and
invalidation contracts must be reusable by BCR rulesets. A rules_cc need may
discriminate a missing generic capability, but must not create C++-specific
parsing, evaluation or rule control flow in Rust.

## Registration sequence after this prerequisite

Keep one shared pipeline for toolchain and execution-platform registrations:

1. parse retained target-pattern text with the declaring canonical repository
   and final apparent mapping;
2. obtain root or external subtree package membership through loading-owned
   producers;
3. load packages and resolve package-wildcard ambiguity, where an explicit
   same-name target wins; and
4. apply the consumer family's wildcard filter, stable signed-pattern order
   and deduplication before configured validation and activation.

Explicit targets bypass wildcard-only family filters, matching Bazel's
registration behavior. Toolchain and execution-platform consumers share parse
and expansion but retain distinct rule/provider filters. No part of this
prerequisite activates that sequence.

## Retained-state and DICE constraints

Follow `docs/developers/dice.md`: immutable complete values, natural-owner
dependencies, transient Needs, no lock across compute, cancellation without
partial publication and equality cutoff only on complete values. Reuse the
existing Buck2-derived compact strings, immutable `Arc` slices/maps and
structural hashing. Do not add an interner, global cache, second retained
repository graph, route copy keyed by apparent name, or manual synchronization.

The implementation design must identify which production definitions move
from core to loading, how test support is split without preserving a private
production owner in core, and how existing core adapters import the new owner
without a dependency cycle. Movement should be mechanical where semantics are
already accepted; new route projection should be separately bounded and
proved.

## Compatibility classification

- **Exact:** no named command surface is activated. The extracted selected-
  before-generated definition order, contextual mapping behavior, source
  preparation and observation/error ordering remain as already accepted.
- **Slug-native:** Rust/DICE key and value types, immutable carrier layout,
  structural hash/equality and observation epochs.
- **Unsupported/deferred:** registration pattern expansion, wildcard conflict
  warnings, family filtering/deduplication, configured provider/settings
  validation, activation, broader builtin categories, rules and actions.

## Required design proof and stops

Inventory every production symbol in the current core owner and every caller
before selecting an implementation allowlist. Prove one key for root,
`bazel_tools`, selected-registry, selected-nonregistry/direct-local and
generated canonical identities. For selected-nonregistry, prove source-policy
and mapping retention, legacy/observed behavior and A/B/A explicitly. Also
prove selected-before-generated and effect-activation order; exact legacy /
observed precedence; complete-only equality/validity; canonical A/B/A
independent of apparent aliases; contextual mapping A/B/A; and unchanged root-
route adapter behavior. Require independent DICE/public-boundary review before
selecting Rust implementation.

STOP and `REPLAN` for a core-owned expander, a root/query-only external slice,
an apparent name in canonical route identity, duplicate canonical-definition
keys, changed selected/generated order, direct filesystem access, target-
pattern activation, rule-specific parsing/evaluation, a new dependency cycle,
global state, or a lock across DICE compute.

## Immediate predecessor

Commit `4fabef5e0` accepts `ExternalSubtreePackageSetKey` at 528 production and
747 proof additions. Eleven focused tests plus full loading, bzlmod, query,
core and rebuilt CLI validation pass; independent review returned `ACCEPT`.
That producer closes recursive package membership for an authenticated route
and exposes the canonical source-route ownership gap described here.
