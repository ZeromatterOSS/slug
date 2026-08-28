# Current Slug V2 Packet

Packet: `WP-4-5-7A-builtin-bazel-tools-selected-mapping-design-r2`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `959cbd889`.

Result: freeze one bounded Bzlmod-owned selected-repository-mapping projection
for builtin `bazel_tools`, then materialize a corrected implementation packet
that may retain the uncommitted target-platform candidate. This packet changes
plans only. It does not authorize Rust edits or acceptance of the candidate.

Independent Sol review: `ACCEPT`. The corrected graph-only owner is acyclic,
keeps RepoSpec/source metadata out of mapping publication, and retains only the
compact mapping carrier at the builtin boundary.

## Why R4 and the first design replanned

Independent terminal review returned `REPLAN`. The R4 candidate embeds exact
Bazel 9.2 `tools/BUILD` and `tools/build_defs.bzl`, but
`HostCanonicalRepositoryRoute::builtin()` publishes an empty repository
mapping. `tools/build_defs.bzl` eagerly loads
`@platforms//host:constraints.bzl`, so Slug cannot evaluate the builtin package
far enough to publish `host_platform -> @@platforms//host:host`. Focused tests
proved the fallback option label, local alias normalization and exact source
bytes separately; they did not prove their required composition. The failed
Slug smoke confirmed the missing mapping boundary. The pinned Bazel oracle
proves Bazel behavior, not Slug realization.

R4 stops on any further material correction. Do not commit its Rust candidate
or silently narrow an exact claim. Retain it only as uncommitted input while
this design is reviewed.

The first design proposed projecting the builtin mapping from
`HostSelectedModuleRoutesKey`. Independent pre-review returned `REPLAN` because
that owner also awaits every selected registry RepoSpec and its source
metadata. A mapping-only request would inherit unrelated materialization
Needs/errors and an overbroad observed frontier. R2 instead extracts the
mapping computation directly over the selected module graph, matching Bazel's
`BazelDepGraphValue` dependency boundary.

## Learned facts and authority

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is behavior authority:

- `RepositoryMappingFunction#computeForBazelModuleRepo` publishes a module
  repository's full mapping from the selected dependency graph;
- `BzlLoadFunction#getRepositoryMapping` consumes that full mapping for normal
  `@bazel_tools` `.bzl` and BUILD evaluation after bootstrap;
- `RepositoryMappingFunctionTest` covers selected dependency mappings; and
- the existing `toolchain-resolution-first-platform` oracle records
  `@bazel_tools//tools:host_platform -> @@platforms//host:host`.

Slug already computes the required semantic fact inside
`selected_routes_with_canonicals`: the selected module graph gives
`bazel_tools` its exact special canonical name and complete
apparent-to-canonical mapping, including `platforms -> platforms`. That
calculation is currently fused with selected registry RepoSpec consumption in
`HostSelectedModuleRoutesKey`. The selected-definition API deliberately returns
`BuiltinDeferred`, while the loading-owned canonical-route key bypasses the
mapping and constructs a static builtin route with an empty mapping. The fused
owner and shortcut are the defects.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is concept/test guidance only: Bzlmod workspace/repository-view producers own
selected mappings, and generic package/`.bzl` evaluators consume immutable
mapping contexts without selecting or inferring them. No Zig type, scheduler,
identity or behavior claim is imported. Buck2-derived `SmallMap`, immutable
`Arc` carriers, `Dupe` and `Allocative` remain the preferred compact substrate.

## Decision

Extract one graph-only Bzlmod mapping owner, then add one public, hidden
builtin projection family:

- `HostSelectedRepositoryMappingsKey(workspace)` consumes only the existing
  selected module graph key. It owns canonical-name derivation and the complete
  ordered mapping for every selected module. It does not request registry
  RepoSpecs, source metadata, materialization or packages;
- its observed sibling consumes only the existing selected-graph observation
  key and forwards exactly that graph frontier;
- `HostSelectedModuleRoutesKey` consumes that mapping owner plus its existing
  RepoSpec owner, joins registry RepoSpecs to the already-selected mapping
  rows, and preserves its current result, errors, order and observations;
- `HostBuiltinBazelToolsRepositoryMappingKey(workspace)` consumes only the
  graph-only mapping owner and selects the unique `bazel_tools` row;
- its observed sibling consumes the observed graph-only mapping key and
  forwards only that mapping frontier;
- the retained value clones only the existing compact mapping owner
  (`context_repo`, ordered apparent names and shared entry map), not the full
  selected graph, evaluator values or source bytes; and
- missing, duplicate, wrong-context or non-builtin routes fail closed with a
  typed semantic error. Need, outer error and cancellation publish nothing.

The loading-owned canonical-route key must consume that projection for the
`bazel_tools` canonical identity in both legacy and observed modes. The builtin
route retains its pinned snapshot identity plus the selected mapping. Its
existing `mapping_target()` and `bzl_repository_mapping()` methods then serve
generic label resolution and evaluator construction exactly like other route
families. Route equality and hashing include the mapping; no consumer rereads
MODULE, registry, lockfile or source state.

This is the full builtin module mapping, not a hard-coded `platforms` pair.
That keeps `tools/build_defs.bzl`, later autoloaded rule modules and all other
BCR dependencies on one architecture and prevents one dependency-specific
repair from becoming permanent.

## Non-decisions and compatibility

- **Exact:** selected full repository mapping for builtin `bazel_tools` on the
  admitted Bzlmod graph; generic eager `@platforms` load resolution; exact
  `host_platform -> @@platforms//host:host` alias realization.
- **Slug-native:** Rust/DICE keys, compact retained carriers, observed-frontier
  error wording and structural route identity.
- **Unsupported/deferred:** live network BCR materialization in direct CLI
  query/cquery, missing builtin packages outside the catalog, command platform
  flags, platform mappings, toolchain selection, providers and implementation
  analysis.

Do not change builtin source bytes, infer mappings from source labels, inject a
root mapping, special-case `platforms` in loading/evaluation, widen selected
registry materialization, or turn `cc_common`/`cc_internal` into Rust rule
control flow. Buck2-derived Rust remains the generic Starlark evaluator and
compact utility substrate. BCR Starlark owns every rule definition and control
path including `cc_internal`; `cc_common` is only a demanding generic host-ABI
consumer. Zabel is peer guidance, never authority.

## Request, revision and lifetime

The key identity is the immutable workspace path. The selected module graph
owns root and builtin MODULE inputs, version selection, canonical identities,
dependency edges and their observed path frontier. RepoSpec/source metadata is
not a dependency of the mapping owner. Concurrent requests deduplicate through
DICE. A mapping-changing revision invalidates the builtin projection and
route; equal selected mappings cut off downstream recomputation. A/B/A, cold
cancellation and same-graph repair must work without manual locks or
process-global state.

The mapping is DICE-retained semantic state. Ordered names and entries retain
their existing compact shared allocations; route consumers borrow or clone
only immutable `Arc` state. Lookup scratch is compute-local. No evaluator heap,
command scratch, cache, interner, task or filesystem handle is retained. No
lock may cross a DICE computation.

## Corrected successor proof contract

The implementation packet must prove the composition, not its parts:

1. Graph-only mapping tests publish complete ordered mappings for root,
   registry, nonregistry and builtin rows, including builtin self and
   `platforms`; reject missing, duplicate and wrong-context rows; and prove
   missing/erroring registry RepoSpec/source metadata cannot block mapping
   publication or enter its observed frontier.
2. Legacy and observed canonical routes retain that exact mapping, mapping
   changes affect structural identity, equal mappings reuse results, and
   Need/outer/cancellation/lifecycle behavior is preserved.
3. One Slug loading proof evaluates exact builtin `tools/BUILD` and its eager
   `tools/build_defs.bzl` load using a provenance-pinned selected `platforms`
   source, then observes alias actual `@@platforms//host:host`.
4. One configured-analysis proof starts from the default structural
   `host_platform` option and reaches the same actual configured platform
   through the generic route/package/alias/platform keys.
5. Existing selected, generated and root mapping suites remain unchanged;
   exact builtin hashes/modes and all R4 platform/cycle/action proofs remain.

Reuse the existing Bazel oracle and the provenance-pinned local `platforms`
module fixture; add no oracle fixture unless the implementation audit proves a
real evidence gap. The local fixture authenticates Slug composition but does
not support a claim of live network BCR materialization.

## Implementation packet to materialize after review

After independent Sol `ACCEPT`, replace this manifest with
`WP-4-5-7A-target-platform-and-exec-configuration-prerequisite-r5`. R5 may
retain the current uncommitted R4 candidate and add only the natural mapping
producer, route consumption and composition proofs above. Before editing,
record exact blobs/line counts and caps for:

- `app/slug_bzlmod_v2/src/selected_repo_spec.rs` and `src/lib.rs`;
- `app/slug_bzlmod_v2/src/canonical_repository_route.rs`;
- `app/slug_loading_v2/src/canonical_repository_route.rs` and its focused
  route/load tests; and
- only the minimum existing analysis test/harness files needed for the final
  configured composition proof.

Preserve every R4 allowlist entry and cap exactly. Do not authorize a new
fixture, Cargo/lockfile change, CLI/core production change, registry
materialization change or second mapping owner. Set separate production and
proof caps only after measuring the live owners; the new production addition
must remain below 500 physical Rust lines and the new proof addition below 500.

## Validation and stops

The design closes only after independent Sol review returns `ACCEPT`. The R5
implementation must run focused Bzlmod route, loading and analysis suites;
full affected crate suites serially; direct dependents; rustfmt;
`git diff --check`; source/hash/mode and cap audits; packet/canonical matching;
and `scripts/v2_archive_status.sh`. Rebuild `slug_cli_v2` before any smoke and
report unsupported CLI materialization honestly.

STOP and `REPLAN` for a selected-route/package-load DICE cycle; any RepoSpec,
source-metadata or materialization dependency in the mapping owner; a mapping
derived from source spelling or root visibility; `platforms`-only injection;
copied full graph/source/evaluator state; lost observations; changed
root/selected/generated mapping semantics; an unguarded lock across DICE;
live-registry or catalog expansion; Rust BCR/ruleset control flow;
`cc_common` specialization; Zabel authority; cap breach; or a second material
implementation correction.
