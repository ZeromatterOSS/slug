# Current Slug V2 Packet

Packet: `WP-4-5-7A-repository-aware-configured-package-identity`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `00f1453ef`.

Result: converge configured analysis's package input and retained collection
from root-only `PackagePath` identity to the accepted repository-aware loading
carrier keyed by full `PackageIdentifier`. Preserve all current root-only
configured behavior; activate no registration family or canonical target.

## Architecture and authority

Commit `104291321` freezes the sequence: general loading carrier, configured
package identity, two-family consumer cutover, then alias/provider/settings and
option precedence. Commit `00f1453ef` completes the first step. This packet is
only the second step.

Bazel 9.2 remains behavioral authority. Clean Zabel
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance for separating
repository-aware loaded inputs from configuration/type-keyed selection; copy no
Zig storage, IDs, scheduling or compatibility claim. BCR Starlark owns every
rule and control-flow surface including `cc_internal`; `cc_common` is only a
generic evaluator/host-ABI consumer.

## Required production change

In `slug_analysis_v2::dice`:

1. replace `compute_root_package_input` with a configured package helper taking
   full `PackageIdentifier` and composing `HostPackageInventoryKey` or its
   observed sibling;
2. replace `RootPackageValue`/`RootPackages` with a collection whose key is full
   `PackageIdentifier` and whose value retains the carrier `Arc`;
3. make every package lookup compare the label's full package identifier, then
   borrow `LoadedPackage` through the carrier without cloning evaluator-owned
   values or child errors;
4. pass explicit root `PackageIdentifier`s at existing root-only call sites;
5. rename root-specific private helpers only where their retained identity is
   now general; and
6. preserve complete-only outer DICE behavior, dependency order and event
   ownership.

The existing direct-root registration parser and root-only configured label
admission remain unchanged. A canonical label must still fail at the currently
accepted boundary before configured package loading. The next packet alone may
consume `ModuleRegistrationExpansionKey` for both families.

## Proof obligations

Prove:

- same package path in root and a canonical repository remains unequal in the
  configured collection and cannot cross-resolve;
- all current root configured targets, native references, toolchain/platform
  validation, cancellation, Need, warm reuse and A/B/A semantics are unchanged;
- legacy and observed configured package requests depend on the corresponding
  general carrier key, not directly on root package-load keys;
- observed loading result/epoch/event ownership remains below the carrier and
  configured analysis adds no replay owner;
- canonical configured targets and external native references remain rejected
  before package projection; and
- no registration-expansion key activates.

Reuse accepted child lifecycle evidence. Add only discriminating coverage for
the changed analysis owner and identity seam; no new Bazel oracle is required
because this packet admits no new Bazel-visible behavior.

## Compatibility classification

- **Exact:** existing accepted root configured-analysis behavior and rejection
  order remain unchanged.
- **Slug-native:** full configured package collection representation, helper
  names, structural hashing and DICE dependency transport.
- **Unsupported/deferred:** canonical configured targets, two-family
  registration consumption, alias/provider/settings semantics, option
  precedence, general external configured graphs, new builtins/actions and
  exact configuration/output bytes.

## Allowlist and caps

Production:

1. `app/slug_analysis_v2/src/dice.rs`

Proof:

2. `app/slug_analysis_v2/tests/root_analysis.rs`
3. `app/slug_analysis_v2/tests/starlark_rule.rs`

No loading, Bzlmod, identity, query, core, CLI, BUILD, fixture, oracle, Zabel or
plan file is admitted after this scheduling commit. Caps: 600 net production
lines, 750 net proof lines, 1,350 total; no function over 120 lines. Add no
dependency, map type, interner, global cache or second package representation.

## Validation

Run serially:

1. focused analysis identity/owner and rejection tests;
2. complete `slug_analysis_v2` tests;
3. direct `slug_query_v2` dependent tests;
4. complete `slug_loading_v2` tests if public carrier use exposes a gap;
5. `cargo fmt --all --check`, allowlist/cap/function checks,
   `git diff --check`, packet/canonical ID agreement and archive status against
   its recorded three-file baseline; and
6. independent terminal review before acceptance.

## Stops

STOP and `REPLAN` for registration-expansion activation; canonical configured
target admission; a path-only configured package key; direct route/source/path
discovery; reintroduction of the old external policy adapter; cloned
`LoadedPackage`; new event ownership; a second evaluator; alias/provider/
settings or option behavior; Rust ownership of BCR rules or `cc_internal`; a
C++ parser/rule engine; analysis files outside the allowlist; or cap overflow.
