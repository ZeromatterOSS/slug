# Current Slug V2 Packet

Packet: `WP-4-5-7A-selected-toolchain-context-cutover-architecture`

Milestone: M7A category 6, selected implementation exec-configuration and
`ctx.toolchains` payload cutover.

This is a zero-Rust architecture packet. The category-5 retained recursive
analysis-value/provider implementation is terminally accepted in the immediate
predecessor commit. Do not edit Rust until independent architecture review
accepts the implementation contract materialized by this packet.

## Observable result

Freeze one bounded implementation packet that deletes the marker-only
post-selection bridge, analyzes every selected toolchain implementation under
the execution configuration derived from category 4's chosen platform, retains
the exact category-5 builtin `ToolchainInfo` occurrence, and exposes the same
occurrence through a general multi-type `ctx.toolchains` view. The design must
be sufficient for the next rules_rust toolchain-owner vertical without adding
a ruleset-specific parser, payload or control path.

## Learned facts and research basis

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is behavior authority:

- `ConfiguredTargetFunction` lines 349-369 loads configured toolchain
  dependencies before invoking the requesting rule;
- `ResolvedToolchainContext.load` lines 48-105 obtains the builtin
  `ToolchainInfo` from each configured implementation, preserves requested
  type identity, rejects a missing mandatory implementation/provider, and
  keeps a requested missing optional type with a null value;
- `StarlarkRuleContext.toolchains` lines 878-897 and
  `StarlarkToolchainContext` define Label/String lookup, membership, missing-
  optional `None`, configured-type errors and immutable evaluator behavior;
- `DependencyProducer` lines 160-176 associates toolchain dependencies with
  the selected execution platform; and
- `ResolvedToolchainContextTest` covers mandatory/optional presence, missing
  optional values, aliases and additional providers, while `RuleContextTest`
  proves arbitrary `ToolchainInfo` fields are visible through the context.

The live Slug graph already has the required owners but the bridge discards
their information:

- category 4's `ConfiguredToolchainResolution` retains one chosen exec-
  configured platform and ordered requested/actual mandatory/optional rows;
- `prepare_marker_toolchain_bridge` accepts only one mandatory row,
  re-analyzes its implementation with the requester's target configuration,
  rejects ordinary dependency-bearing/provider-rich implementations, and
  extracts only a string `marker`;
- `AnalysisToolchains` reconstructs a fresh one-field provider and accepts
  only one root-apparent string key; and
- category 5 now provides a cheap-clone evaluator-independent
  `ProviderOccurrence`, complete publication equality and one inverse
  materializer for the admitted recursive value graph.

`docs/developers/dice.md` governs the recursive child compute: DICE owns
deduplication and invalidation, and no lock may cross a computation. The
existing configured-analysis cycle detector must guard the selected-child
future rather than introducing a lock, side key or process store.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is concept/test guidance only. Its `ResolvedToolchain` row usefully separates
requested and post-alias type identities, represents an unresolved optional
type with `None`, and materializes a caller-owned evaluator value from an
authoritative provider reference. Do not copy its Zig layout, ordinals,
scheduler, stores, error claims or compatibility authority.

## Decisions the design must freeze

1. The existing configured-analysis key remains the sole producer. It consumes
   the accepted resolution rows, derives the implementation configuration from
   `resolution.execution_platform().actual().configuration()` (which must be
   `Exec`), and computes selected implementation children through the existing
   legacy/observed configured-analysis path. No resolver/context DICE key is
   added.
2. Replace singular marker state with an immutable ordered retained context.
   Each row keeps requested and post-alias type keys plus mandatory status. A
   selected row additionally keeps declaration identity, requested and actual
   implementation keys, and the exact builtin `ProviderOccurrence`; an
   unresolved optional row keeps no declaration, implementation or provider.
   Invalid partial rows fail at construction.
3. Parent topology and action context share these rows by `Arc`; they do not
   copy/stringify provider fields. The parent publishes one requirement edge
   per requested row and one selected-implementation edge per selected row.
   The implementation edge targets the configured dependency occurrence;
   actual alias identity remains separately retained. Candidate-platform order
   and the one common selected execution platform remain unchanged.
4. Selected implementations are ordinary configured Starlark rules. Remove
   marker-leaf schema, zero-dependency/action/provider-cardinality and marker-
   field restrictions. Require the authenticated builtin `ToolchainInfo`
   occurrence but retain all other child providers/actions only in the child
   result. Category-5 unsupported values still fail closed during lowering;
   this packet does not add retained evaluator functions.
5. Prepare selected rows in declared requirement order. The DICE child work may
   join, but Need union and first outer/semantic error selection remain
   deterministic in that order. One configured-analysis cycle guard surrounds
   the complete selected-child future for a rule key and converts recursive
   toolchain cycles to the established typed configured-cycle terminal. No
   mutex or evaluator state crosses the await.
6. `ctx.toolchains` is one evaluator-scratch immutable view over the retained
   rows. It accepts canonical Label values and strings resolved against the
   rule definition's producer-owned `.bzl` label/repository mapping; loading
   must retain that compact definition context instead of analysis inferring a
   repository. Requested and accepted post-alias identities address the same
   row. Membership is true for every requested row, including unresolved
   optional rows; indexing that row returns `None`; an unrequested key errors.
   A selected lookup rematerializes the retained occurrence through the
   category-5 adapter and preserves shared provider class/identity semantics.
7. Delete `ConfiguredActionToolchainContext`'s marker and the singular
   `ToolchainTopology::selection()` shape rather than leaving compatibility
   shims. Public analysis/action consumers move to ordered rows in the same
   packet. Configuration identity remains complete structural identity; no
   checksum, display token or output path substitutes for it.

## Compatibility classification

- **Exact:** for the admitted category-5 value graph, Bazel 9.2 selected
  implementation configuration, required/optional multi-type rows, builtin
  provider authentication, alias-aware Label/String lookup, missing optional
  `None`, immutable membership/index behavior, provider occurrence payload,
  requirement/implementation dependency topology and deterministic errors.
- **Slug-native:** Rust `Arc` layout, structural configuration identity,
  publication equality, configured-cycle wording, action-context row layout
  and unproved diagnostic text.
- **Unsupported/deferred:** retained evaluator functions in provider fields,
  `ToolchainTypeInfo` index values, template-variable projection through
  `ctx.var`, exec groups/automatic exec groups, aspects/subrules, toolchain
  transitions beyond the accepted projection, broader action kinds and exact
  Bazel configuration/output bytes.

BCR Starlark owns all rule definitions and control flow, including
`cc_internal`. `cc_common` is a generic host/provider-ABI client of this
category-wide graph, never a parser or Rust C++ rule engine. Language builtins
such as `set` remain evaluator-global and outside the retained provider store.

## Ownership, revisions and memory

The existing configured-analysis DICE key owns resolution dependencies,
selected child dependencies, requester edges and the published row context.
Changing registrations, constraints, command overlays, implementation BUILD or
`.bzl` sources, configuration, repository mapping or any selected provider
payload must invalidate through existing tracked dependencies. A/B/A must
restore deep publication equality without replaying evaluator state.

Resolution rows, configured identities and provider occurrences are DICE-
retained semantic memory. Loading's rule-definition context is immutable
loading semantic memory. Child-result temporaries, join/error aggregation,
cycle futures, evaluator toolchain views and materialization memo tables are
phase scratch and drop before publication or on error/cancellation. No new
cache, interner, registry, lock, background task or async-owned transfer is
authorized.

Request/session-specific behavior is inapplicable: this packet adds no command
input or external observation and reuses the existing structural configuration
and observed/legacy analysis paths.

## Required architecture output

Materialize the successor implementation manifest in this file with:

- exact retained row/context types, constructor invariants, deep equality and
  pseudocode for configuration derivation, child computation, Need/error order,
  provider extraction, edges and evaluator lookup;
- the exact implementation/proof file allowlist, baseline blobs/physical line
  counts from the accepted category-5 commit, per-file and aggregate caps;
- a fallback-deletion ledger naming every marker helper/field/assertion that
  must disappear;
- focused proof for target-versus-exec configuration discrimination, two
  selected types plus one absent optional type, requested/post-alias lookup,
  arbitrary admitted provider payload identity, dependency-bearing selected
  implementations, child action ownership, recursive toolchain selection,
  missing builtin provider, cycle/recovery, Need/error order and A/B/A; and
- direct-dependent analysis/core/action-query gates and explicit stops.

Reuse existing category-4 selection fixtures, category-5 provider round-trip
proof and the pinned source regressions above. Add a Bazel oracle only if the
design identifies a genuinely uncovered observable; copied rules_rust/BCR
content is not authorized in this design packet.

## Docs-only allowlist and validation

Only these files may change:

- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`;
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`.

Run `git diff --check`, targeted packet/canonical ID agreement, archive status
and independent reserved-architecture review. No Cargo, Bazel, fixture,
production Rust or Zabel edit is authorized.

Return `REPLAN` before Rust for a new DICE key, target-configuration
implementation analysis, lossy marker/string payload, inferred repository
mapping, evaluator/heap retention, process store/cache/interner, missing
optional-row semantics, per-builtin arbitrary field struct, ruleset-specific
control flow, lock across DICE compute, unguarded recursive selection,
configuration/display/digest identity substitution, or an implementation scope
that cannot be bounded by an exact allowlist and caps.
