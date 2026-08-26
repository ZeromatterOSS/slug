# Current Slug V2 Packet

Packet: `WP-4-7A-rustfmt-first-aspect-requirements-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: loading-owned frozen aspect declaration and exported provider identity
Base: `2cbdb148`

Result: load and freeze the first declaration in accepted rules_rust 0.73.0
`rust/private/rustfmt.bzl:96-127`. Extend the existing frozen aspect owner with
the fixed two-alternative `required_providers` predicate and the fixed `cpp`
configuration-fragment requirement. Preserve each provider's defining-module
export identity through the recursive imported `rust_common` value. Do not
apply an aspect, inspect a target's advertised providers, materialize
`ctx.fragments`, or advance into the second rustfmt aspect.

## Accepted starting point and first absent fact

Commit `2cbdb148` completes and freezes `rust/private/lint_test.bzl`. Recursive
external-Bzl evaluation returns to `rust/private/rustfmt.bzl`: function bodies
at lines 13-94 remain lazy, and the fixed `RustfmtTargetInfo = provider(...)`
at lines 96-102 already freezes. The implementation body at lines 104-117 is
also lazy. Slug reaches the first `rustfmt_srcs_aspect = aspect(...)` at lines
119-127 and rejects `required_providers` as the first unknown argument;
`fragments` is the adjacent second missing declaration fact.

The accepted expression supplies exactly two singleton alternatives in source
order, `rust_common.crate_info` or `rust_common.test_crate_info`, and one
fragment name, `cpp`. `common.bzl:26` imports `CrateInfo` and `TestCrateInfo`
from `providers.bzl`, then places those exact values into `rust_common` at lines
70-79. Their structural identities therefore remain the first exports in
`providers.bzl`, not `common.bzl` or the consuming rustfmt module. The aspect
implementation must not run while the module loads or freezes.

## Bazel authority and Zabel architectural guidance

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
The accepted rules_rust archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Use only those pinned objects and archive, not sibling HEADs.

The minimum authenticated Bazel chain is:

- `StarlarkRuleFunctionsApi.aspect` declares named `required_providers` and
  `fragments` sequences with empty defaults;
- `StarlarkAttrModule.buildProviderPredicate` treats an outer sequence as OR
  alternatives and each inner provider sequence as an AND set, validates
  exported provider constructors, and retains their provider keys;
- `StarlarkRuleClassFunctions.aspect` constructs the provider predicate and an
  immutable fragment-name set without running the implementation;
- `StarlarkDefinedAspect` retains both facts with the definition and transfers
  them into `AspectDefinition` only when the aspect is later applied; and
- `StarlarkRuleClassFunctionsTest.aspectRequiredProvidersSingle`,
  `aspectRequiredProvidersAlternatives`, and
  `StarlarkDefinedAspectsTest.aspectAllowsFragmentsToBeSpecified` authenticate
  predicate shape and fragment declaration behavior. Analysis-phase
  propagation and fragment access tests are deliberately not imported because
  those phases remain unsupported here.

Pinned Zabel commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Its `build_rule_declaration.zig` keeps `required_providers` and `fragments`
inside the complete producer-owned `AspectDefinition`, keeps
`AspectExportIdentity` separate, and retains imported provider identities
instead of rebinding them at the consumer. Slug follows that ownership split
using its existing provider IDs and frozen aspect lifetime. No Zig code,
representation, evaluator behavior, cache or analysis rule may be copied;
Bazel 9.2 remains sole compatibility authority.

## Compatibility classification

- **Exact:** acceptance, validation and freeze of the fixed nested
  `required_providers = [[rust_common.crate_info],
  [rust_common.test_crate_info]]`; preservation of both exported provider
  identities and alternative order; acceptance and retention of fixed
  `fragments = ["cpp"]`; the first aspect's existing implementation,
  documentation and export identity; lazy implementation behavior.
- **Slug-native:** existing `ProviderId`, `CompactString`, Arc-backed frozen
  representation, Rust equality/ordering used for duplicate normalization,
  complete-module fingerprint over-invalidation and nonrequired diagnostics.
- **Unsupported/deferred:** flat provider predicates, native providers, empty
  or wider predicates, duplicate-edge observability, `required_aspect_providers`,
  `provides`, `requires`, aspect attrs, other fragment names, aspect selection,
  propagation/application, advertised-provider matching, `ctx.fragments`,
  configured targets, toolchains/actions, the second and later rustfmt aspects,
  M8/M7B and exact Bazel configuration/output identity.

## Natural owner, lifetime and non-decisions

`UserProviderCallable::export_as` and `FrozenUserProviderCallable` already own
one structural `ProviderId` consisting of producer module label plus first
exported name. Add only a crate-private transient ID projection so aspect
validation can clone that existing identity; do not construct an ID from
display text or importer context. `AspectDefinitionGen` and
`FrozenAspectDefinition` remain the sole declaration/freeze owners. Retain the
predicate as an Arc outer slice of Arc provider-ID alternatives and fragments
as one Arc compact-string slice. These are DICE-retained semantic module data
released with the frozen recursive Bzl value; they borrow no evaluator heap or
request scratch.

No DICE key, source observer, repository mapping, I/O, cache, interner, hash
domain, lock, async task or command result changes. Existing recursive module
identity/fingerprint invalidates the whole declaration when any defining Bzl
source or route identity changes. No fallback is introduced. The Buck2
utility audit selects existing Arc slices, `CompactString`, `ProviderId`,
`SmallSet`/sort-and-dedup patterns and `Allocative`; no new retained utility or
representation family is admitted.

## Implementation boundary

1. Expose the already-bound `ProviderId` from transient
   `UserProviderCallable` as a crate-private borrowed projection. Reuse the
   existing frozen callable `id()` projection.
2. Add a loading-private converter that accepts only the fixed nested-list
   predicate shape, downcasts every element to an exported transient or frozen
   user-provider callable, clones its `ProviderId`, normalizes duplicates
   within one alternative, and retains outer source order. Reject unexported,
   mixed, empty-inner, non-provider and native-provider values.
3. Add named `required_providers` and `fragments` inputs to the existing
   `.bzl`-only `aspect` global. Omitted values remain empty. For this packet,
   nonempty provider predicates must match the nested user-provider form and
   nonempty fragments must be the fixed singleton `cpp` declaration.
4. Freeze both fields inside the existing aspect definition and expose only
   crate-private test projections. Preserve all accepted constructor behavior,
   first-export identity and BUILD absence.
5. Do not retain raw Starlark lists, do not add a provider side registry, do
   not reconstruct provider IDs, and do not wire either declaration fact into
   configured analysis.

## Discriminating proof

- Evaluate a caller-aware recursive chain where `providers.bzl` first exports
  two distinct providers, `common.bzl` imports and wraps those exact values in
  `rust_common`, and `rustfmt.bzl` imports that struct, defines the exact
  `RustfmtTargetInfo`, and freezes the exact first rustfmt aspect. Assert the
  IDs `@@dep+//rust/private:providers.bzl%CrateInfo` and
  `@@dep+//rust/private:providers.bzl%TestCrateInfo` in outer source order,
  fixed `cpp`, producer aspect identity and a failing lazy body that never
  executes.
- Prove that importer aliases do not rewrite provider IDs and that changing
  one producer module label changes only that retained provider identity.
- Prove omitted values remain empty and reject a transient unexported
  provider, mixed/nonnested/non-provider predicate, empty inner alternative,
  non-`cpp` fragment, and the same aspect call from BUILD globals.
- Keep the accepted aspect-definition and lint-label tests green. Add no
  fixture, registry response, network request or Bazel run.

## Allowlist and growth caps

Only these files may change from base `2cbdb148`:

| File | Base SHA-256 | Base lines | Final line cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/provider.rs` | `89b4dcc6857aeec22d32c16a8025dda65848759b9c5cf8daf0ebc2a64d43d41a` | 589 | 600 | borrowed transient provider-ID projection |
| `app/slug_loading_v2/src/package.rs` | `8184bc6373e816e85954d0dd3b1425993a1150b9758b5f24aed146646e4b1a8f` | 5,522 | 5,642 | retained aspect predicate/fragments and bounded conversion |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `6d833b6f0ecd43bce120a02ea830fcde5d78851ae8fbd4c42bd8ea290c8041e9` | 4,644 | 4,774 | recursive identity/freeze and rejection proofs |

Additions are capped at 125 production lines, 130 proof lines and 255 total
lines. Deletions do not buy addition budget. No touched function may exceed
150 lines. `package.rs` already exceeds the 2,000-line review trigger, but the
existing aspect declaration owner remains cohesive for two adjacent fields;
creating a second aspect module or metadata registry would split one semantic
owner. STOP if the converter cannot remain a small private helper.

## Serial validation and review

Run Cargo commands serially with one shared target directory:

```text
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 rustfmt_first_aspect
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 bazel_aspect_definition_validates_admitted_fixed_abi_and_build_absence
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 label_attribute_defaults_keep_defining_module_identity
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo check --locked -p slug_core_v2
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo build -p slug_cli_v2
cargo fmt --check
git diff --check
scripts/v2_archive_status.sh
```

The archive checker may report only its known three retained thoughts paths
plus active packet files. Recheck hashes and count additions, physical lines
and touched-function lengths before review. Independent terminal review is
mandatory before commit and must verify source order, pinned Bazel behavior,
pinned Zabel guidance-only use, provider producer identity, nested predicate
shape/order, fixed fragment, lazy implementation, frozen lifetime, BUILD
absence, caps, serial validation and absence of a new semantic side owner.

## STOP / `REPLAN`

STOP and `REPLAN` if completion requires a file outside the allowlist; native
or unexported provider identity; flat, empty-inner or wider provider-predicate
breadth; any fragment beyond fixed `cpp`; raw evaluator-value retention;
provider-ID reconstruction; aspect application, provider matching,
propagation, configured fragments, analysis/actions; a new DICE key, mapping,
cache, I/O path, interner, hash or lifetime owner; Java/JVM work; Zabel code or
behavior adoption; an unpinned source; a new fixture/oracle/network request; a
cap violation; or a public rules_rust success claim. After the first rustfmt
aspect freezes, stop and select the next source-order audit separately.
