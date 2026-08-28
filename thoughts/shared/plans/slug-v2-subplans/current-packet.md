# Current Slug V2 Packet

Packet: `WP-4-5-7A-configured-registration-consumer-architecture`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `3c6779966`.

Result: freeze the smallest repository-aware configured-consumer architecture
for the accepted toolchain and execution-platform expansion families. Select
the general loading package-carrier prerequisite and a bounded cutover
sequence; add no Rust, oracle, activation or compatibility claim.

## Immediate predecessor

Commit `3c6779966` accepts the two independently keyed loading expansion
families. One driver consumes selected MODULE declarations, the contextual
target-pattern grammar, canonical routes, root/canonical subtree owners and
general package inventories. It retains ordered canonical labels and warning
facts, preserves observed result `Arc`s, and publishes no configured meaning.

The terminal review required one bounded proof correction. The accepted matrix
now discriminates a missing package referenced only by the unrequested family,
canonical route -> subtree -> inventory order, multi-module declaration order,
root-package stable postorder, and row-terminal suppression. Full loading,
Bzlmod, identity, query and read-only analysis gates pass.

## Live consumer audit

The current configured path in `slug_analysis_v2/src/dice.rs` cannot safely
consume the new canonical labels by replacing its parser call:

- `direct_root_registration(s)` reparses only root exact patterns and is called
  independently from execution-platform preparation and toolchain resolution;
- `RootPackages` is keyed only by `PackagePath`, so two canonical repositories
  with the same package path would alias;
- `load_root_native_packages` computes only `RootPackageLoadKey` and
  `require_root_native_reference` rejects every canonical nonroot reference;
- configured Starlark implementation analysis likewise obtains its package
  through the root-only package input; and
- the live path bypasses broken registration state only when
  `!has_toolchain_requirement && local_declarations.is_empty()` and must keep
  that exact short circuit. A local declaration without a required toolchain
  still expands registrations to determine whether the declaration is active.

Deleting the parser without replacing those ownership boundaries would either
collapse semantic repository identity or create an analysis-side source
loader. Both are stops.

## Authority and peer guidance

Pinned Bazel 9.2 remains compatibility authority:

- `RegisteredToolchainsFunction` and
  `RegisteredExecutionPlatformsFunction` own MODULE/option registration
  expansion and precedence before configured validation;
- `SingleToolchainResolutionFunction` keys compatibility by configuration and
  requested toolchain type and retains candidate execution-platform order;
- `ToolchainResolutionFunction`, `ToolchainContextUtil`, `PlatformFunction`
  and `PlatformKeys` own configured platform, alias, constraint and target-
  settings behavior; and
- the focused owning tests remain the source for zero-requirement bypass,
  provider/settings errors, alias behavior and selection order. Reuse existing
  accepted evidence until a concrete configured distinction lacks coverage.

Clean Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance only:

- `request/configured_analysis_registration_graph.zig` keeps registration
  sources and the loaded native-toolchain family as explicit configured-graph
  inputs instead of embedding rule-host logic;
- `toolchain/loaded_native_toolchain_inputs_projection.zig` separates
  repository-aware loaded declarations from configuration/type-keyed
  compatibility;
- `starlark_host/engine/build_invocation_capture.zig` retains declared
  toolchain requirements while deferring registered selection to configured
  resolution; and
- `tests/workspaces/cquery_zero_toolchains_skip_broken_registration` reinforces
  a useful bypass test theme.

Copy no Zig code, arena/store ID, service-registration graph, diagnostic or
compatibility claim. Slug keeps Rust DICE keys, existing canonical labels and
loading owners. Bazel 9.2 alone decides semantics.

## Frozen ownership architecture

### 1. General repository-aware loading package carrier

Add a loading-owned key family whose semantic input is
`NormalizedAbsolutePath workspace + PackageIdentifier`. Root packages compose
`RootPackageLoadKey`; canonical packages compose
`HostCanonicalRepositoryLoadRouteKey` and the crate-private general
`RepositoryPackageInventoryKey`. Its observed sibling preserves child order,
outer/Need polarity and exact observation result `Arc`s.

The retained carrier is a small Root/Canonical enum over the existing child
result `Arc`; it exposes a borrowed `LoadedPackage` result without copying the
package, evaluator heap or events. Root and canonical error variants remain
typed and distinct. The key never reads a path, source or BUILD file directly,
never uses the old public external policy adapter, and never reconstructs a
route from label spelling.

This owner is reusable by configured analysis and future canonical package
consumers. Do not call it `toolchain` or retain registration labels in it. The
existing expander may continue using its scratch route/package caches; the
new carrier is a DICE package boundary, not a replacement for per-compute
dedupe.

### 2. Repository-aware configured package closure

Replace the private path-only `RootPackages` shape with a package closure keyed
by full `PackageIdentifier`. Every lookup compares the label's complete
package identity. Direct registered candidates and transitive native
references use the general loading carrier; no analysis code computes a source
route or filesystem address.

Generalize the configured target's package input through the same carrier so a
canonical Starlark toolchain implementation can enter the existing configured
rule evaluator. This does not make arbitrary external configured graphs exact:
existing dependency/provider/transition guards remain fail-closed until their
own packets. No root/canonical error is flattened before semantic publication.

Use immutable child `Arc`s and bounded `SmallMap`/`SmallSet` scratch already
present in analysis. Retain no second package map in the final configured
result, process cache, mapping copy, source route or evaluator state. No lock
may cross a DICE computation.

### 3. One prepared registration input per configured owner

Bypass registration dependencies only when
`!has_toolchain_requirement && local_declarations.is_empty()`. A rule with a
local declaration does not bypass even when it has no required toolchain,
because the toolchain family decides whether that declaration is registered.
Otherwise compute the execution-platform expansion family first and the
toolchain family second.
Carry the two expansion result `Arc`s together through platform preparation and
toolchain selection, borrowing their ordered label slices only at each use.
Remove `direct_root_registration(s)` only when every consumer uses this
prepared input; do not reparse raw MODULE text in analysis.

The two loading keys remain independently reusable and event-owning. Analysis
does not combine them into a new workspace-global registration key. The
configured owner/configuration and requested toolchain type remain the natural
identity of compatibility and selection, while expanded MODULE labels are
ordinary DICE dependencies. Warning events stay with expansion keys; analysis
stores only its local evaluation batch.

Need/outer/semantic ordering is execution-platform family, toolchain family,
package closure, configured provider/settings validation, then selection.
Cancellation publishes no analysis result. Warm equality must prune when
expansion and loaded/configured semantics are equal, and A/B/A must restore
the prior selected topology.

### 4. Configured semantics remain category-specific

The cutover admits only the already represented native `platform`,
`constraint_setting`, `constraint_value`, `toolchain_type`, `toolchain` and
bounded Starlark marker implementation shapes. Exact registration targets may
reach configured validation and fail there, preserving the loading/configured
split.

Execution-platform alias resolution, advertised `PlatformInfo`, target
settings, command-line extra registrations/signed precedence, optional and
named exec groups, broader external dependency graphs and general returned
`ToolchainInfo` implementations remain separate configured packets. They may
reuse the prepared label inputs and package closure but must not widen the
loading expander or add a rule parser.

BCR-delivered Starlark continues to own all rule definitions and control flow,
including `cc_internal`. `cc_common` remains a demanding client of the generic
evaluator and host ABI. Native platform/toolchain declarations are configured
facts consumed beneath that general rule graph, not a Rust C++ ruleset.

## Bounded sequence after this design

1. `WP-4-5-7A-repository-aware-loading-package-carrier`: implement only the
   general Root/Canonical loading key, observed sibling, zero-copy carrier and
   lifecycle proofs. Keep analysis unchanged.
2. `WP-4-5-7A-configured-package-identity-convergence`: key the private native
   package closure by `PackageIdentifier` and generalize configured package
   input, while preserving every existing root behavior and fail-closed guard.
3. `WP-4-5-7A-expanded-registration-consumer-cutover`: compute the two loading
   families unless
   `!has_toolchain_requirement && local_declarations.is_empty()`, carry labels
   once, remove the raw analysis parser, and prove root/canonical candidate
   order and selection. Its matrix must include a broken-registration case
   bypassed with no requirement/local declaration and a no-requirement local
   declaration case that does not bypass either requested family.
4. Add alias/platform-provider and target-settings semantics in separate
   configured packets, then configuration-option precedence. Only then claim
   those rows or retry the remaining ordinary Stage 10.3 graph.

Each implementation packet requires its own exact allowlist, caps, terminal
review and applicable Buck2 utility audit. STOP rather than merge steps 1-3
into one patch.

## Compatibility classification

- **Exact:** no new runtime behavior in this design packet. The frozen sequence
  preserves accepted registration expansion order, zero-requirement bypass,
  repository identity, candidate order and existing admitted native configured
  semantics as Bazel 9.2-governed targets.
- **Slug-native:** Rust key/carrier names and layout, typed wrapper errors,
  observation transport, structural hashes, scratch collections and staged
  packet boundaries.
- **Unsupported/deferred:** alias and custom platform-provider resolution,
  target settings, command-line registration precedence, optional/named exec
  groups, general external configured dependency graphs, broader toolchain
  implementations, actions/input trees and exact configuration/output bytes.

## Exact allowlist, caps and validation

Documentation files only:

1. `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
2. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
3. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`

No Rust, Cargo, BUILD, fixture, oracle, other plan or Zabel file is admitted.
Caps: at most 360 net lines in this manifest, 80 canonical-plan additions, 120
Stage-owner additions and 520 total net lines. Run targeted source/structure
searches, plan/canonical packet-ID agreement, `git diff --check`, allowlist/cap
checks and `scripts/v2_archive_status.sh` against its recorded three-file
baseline. Require independent architecture review before acceptance.

## Stops

STOP and `REPLAN` for Rust or oracle edits; a direct raw-pattern parser in
analysis; package identity without canonical repository; analysis-side route,
source or filesystem discovery; use of the public external policy adapter;
copying a loaded package/evaluator/event batch; a workspace-global combined
configured registration key; family warning-event replay; activation before
the exact `!has_toolchain_requirement && local_declarations.is_empty()` bypass
decision, or bypass when either conjunct is false; alias/provider/settings
claims in the first three packets; one monolithic carrier/identity/cutover
patch; a lock across DICE; a process cache/interner; Rust ownership of BCR
rules or `cc_internal`; a C++ parser/rule engine; or treating Zabel as
compatibility authority.
