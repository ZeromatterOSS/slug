# Current Slug V2 Packet

Packet: `WP-4-5-7A-canonical-source-policy-convergence-implementation-r3`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `9764f8a4f`.

Result: converge the Bzlmod repository source/policy chain over one compact
Root/Canonical carrier, preserve every root constructor and behavior, migrate
alias-free canonical policy callers, delete the four temporary canonical
source/listing wrappers, and stop canonical built-in package-source projection
at its authenticated catalog-relative address for Stage B adaptation.

## Accepted basis and learned facts

Commit `9d55b7157` accepts the two-stage architecture: Bzlmod source/policy
convergence precedes loading/package adaptation. The first Stage A preflight
stopped because root source keys and canonical wrapper keys have different
fixed DICE values. Commit `9764f8a4f` supplies the bounded prerequisite instead:
one zero-copy `HostRepositorySourceObservation` owner accepts Root or Canonical
input, its observed sibling retains the exact path epoch, and all legacy root
keys and loading/core consumers remain unchanged.

The remaining policy chain is REPO, repository ignore, private package lookup,
public external package boundary, selected BUILD source, direct path lookup and
directory listing. Each still stores `RootRepositoryRoute`; canonical callers
must not fabricate an apparent alias. Existing accepted Bazel 9.2 regressions
already establish deletion-before-policy, REPO-before-ignore, BUILD marker
priority, selected mapping, and source-observation behavior. Add structural
canonical regressions only; no new oracle fixture is needed.

The R2 implementation review rejected two proof/ownership gaps. First, an
embedded catalog path cannot lawfully populate the existing absolute Host path
field; the draft's `.slug-builtin` projection invented identity and crossed
the Stage B address boundary. Second, load-route cancellation did not exercise
the newly generalized policy/package keys. R3 otherwise retains the R2 code,
allowlist, caps and semantics, removes `builtin_repository.rs` from scope, and
adds the canonical chain cancellation/recovery proof.

Pinned Bazel 9.2 remains the behavioral authority. Buck2 DICE guidance in
`docs/developers/dice.md` requires one semantic owner, complete-only equality,
ordered observation composition and no lock across compute. Zabel is
concept/test-only guidance for authenticated-source/policy separation and
compact retained carriers; copy no behavior or implementation.

This is generic BCR Starlark loading architecture. Bazel 9 BCR Starlark owns
all rule definitions and control flow, including `cc_internal`; `cc_common` is
only a demanding consumer of the generic host-builtin ABI. Builtins remain
organized by reusable capability category.

## Decision and non-decisions

Add one retained `HostRepositorySourceRoute` with exactly:

- `Root(RootRepositoryRoute)`, preserving the complete existing apparent route;
- `Canonical(HostCanonicalRepositorySourceInput)`, preserving the accepted
  source-complete apparent-free route and materialization disposition.

Generalize the existing path/listing and policy keys over this carrier. Keep
their current `new(RootRepositoryRoute, ...)` constructors as exact Root
adapters and add explicit canonical constructors. Root computations continue
to use the accepted root source/path children; canonical computations use the
shared observation owner and the same materialization/path/listing owners.
After all canonical callers migrate, delete both canonical source-file keys,
both canonical listing keys, their observed values/errors and projections.
Retain `HostCanonicalRepositorySourceInput` and its constructor as the
canonical carrier input.

Canonical built-in policy may select a BUILD marker and the shared source
observation may retain its exact catalog value, but Stage A must not project
that value into `RepositoryPackageSource`: the existing result requires a Host
`NormalizedAbsolutePath`, while the catalog authenticates only a repository-
relative address. Return one typed deferred-address package-source error that
retains the exact catalog path. Stage B must design the Root/Canonical source-
address result and Starlark source-name adaptation before canonical built-in
package loading succeeds. Never synthesize a workspace, execroot or filesystem
path for embedded content.

Do not retype `RootRepositoryRoute`, make its apparent name optional, migrate
legacy loading/core source-file consumers, bypass REPO/ignore policy, infer a
physical root, add direct IO, or activate Stage B. Registration, target-pattern
expansion, external `.bzl`/package loading, configured semantics, rules and
actions remain deferred.

## Natural owners, identity and lifetime

The source-route carrier is DICE-retained semantic state and contains only
existing retained values. Generalized keys retain it plus their package/path
identity. The shared source-observation key remains the sole canonical byte
owner; the existing materialization result, resolved path, file observation
and directory-listing keys remain the sole lower owners. Policy results keep
their existing `Arc`/compact representations and complete-only equality.

Hash workspace, root or canonical route, disposition, selected specification,
final mapping, generated plan and path structurally. Never hash or copy source
bytes.
No interner, global cache, side table, manual eviction, dependency or command
scratch is admitted. Need and cancellation publish no complete value;
overlapping requests share only immutable DICE state.

Observed order remains REPO source/listing, repository ignore, package marker,
then selected BUILD source. Outer infrastructure error precedes Need, which
precedes semantic terminal. Merge child epochs in that order. Root key outputs,
errors, display identity and child dependency order remain exact.

## Exact allowlist and caps

Allow exactly:

- `app/slug_bzlmod_v2/src/source_preparation/canonical_repository_source.rs`;
- `app/slug_bzlmod_v2/src/source_preparation.rs`;
- `app/slug_bzlmod_v2/src/lib.rs`;
- `app/slug_bzlmod_v2/src/repo_file.rs`;
- `app/slug_bzlmod_v2/src/repository_ignore.rs`;
- `app/slug_bzlmod_v2/src/host_package.rs`;
- `app/slug_bzlmod_v2/src/host_package_observation_tests.rs`;
- `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`;
- `app/slug_bzlmod_v2/src/host_external_package_boundary/mod.rs`;
- `app/slug_bzlmod_v2/src/host_external_package_boundary/tests.rs`; and
- `app/slug_loading_v2/src/canonical_repository_load_route_tests.rs`.

Cap production additions at 1,200, proof additions at 1,600 and aggregate
additions at 2,800 lines; deletions count separately and buy no unrelated
scope. Keep functions at 120 lines. `source_preparation.rs` and
`host_package.rs` exceed the complexity trigger: permit only carrier plumbing,
bounded helper extraction and the existing path/listing/package-source drivers;
perform no unrelated cleanup.

Add no fixture, oracle, dependency, core, query, loading production, builtin
catalog content, module-extension, registration, evaluator, configured, rule
or action edit.

## Required proof

Prove:

- all existing Root constructors, outputs, errors and child dependency order
  remain unchanged for built-in, local, immutable-registry and generated routes;
- a root-unmapped selected-registry canonical input reaches REPO, ignore,
  package boundary and selected BUILD source without an apparent alias;
- built-in canonical policy uses catalog listing/source owners, retains the
  exact catalog-relative BUILD address, and returns the typed deferred-address
  package-source terminal without fabricating an absolute path;
- canonical local, immutable and generated source/path/listing dependencies
  terminate at the accepted materialization/path owners with no wrapper;
- exact REPO -> ignore -> marker -> BUILD-source observation ordering, outer
  error/Need/terminal polarity, cancellation and complete-only validity,
  including drop-before-publication and successful recovery through the newly
  generalized canonical policy/package-source chain;
- independent workspace, canonical name, disposition, selected specification,
  final mapping, generated plan and package/path A/B/A key/hash restoration;
- carrier/key/observed retained-size bounds and `Allocative` coverage;
- no production reference or export remains for any
  `HostCanonicalRepositorySourceFile*` or
  `HostCanonicalRepositoryDirectoryListing*` wrapper; and
- source bytes, authenticated Host logical paths, built-in catalog-relative
  paths, SHA/executable metadata and lower owner identity are retained without
  copying or address invention.

Tests may reuse existing constructors and dependency instrumentation. They may
not invent a root alias for a canonical success or replace owner assertions
with value-only parity.

## Compatibility

- **Exact:** existing root results, diagnostics, policy/dependency order,
  package-marker selection and observations; canonical selection/mapping follows
  Bazel 9.2 semantics; built-in catalog bytes remain exact.
- **Slug-native:** carrier layout, key names, structural hashes, observed
  carriers, the typed canonical built-in deferred-address terminal and
  retained-memory accounting.
- **Unsupported/deferred:** Stage B subtree/`.bzl`/package-load adaptation,
  canonical built-in package-source address/source-name adaptation,
  target-pattern expansion, registration, configured semantics, rule/action
  execution and exact output identity.

## Validation and stops

Run focused source/listing, REPO, ignore, package and public-boundary tests,
then full `slug_bzlmod_v2` and `slug_loading_v2`, named core/query dependents and
locked `slug_cli_v2` serially. Run formatting, `git diff --check`, exact
allowlist/cap/function-size/duplicate-owner/no-lock/direct-IO guards and
`scripts/v2_archive_status.sh`; only its three accepted thoughts rows may
remain. Require independent DICE/public-boundary/retained-representation review.

STOP and `REPLAN` for a fabricated apparent alias; changed root output/error or
dependency order; copied source bytes; a fabricated absolute path for catalog
content; a second materialization/path/listing
owner; loading-layer/catalog IO; bypassed REPO/ignore policy; missing epoch
order; lock across compute; dependency, allowlist or cap expansion; a surviving
temporary canonical wrapper; or activation of loading/registration/configured/
rule/action behavior.

On acceptance, select only
`WP-4-5-7A-canonical-loading-package-adapter-implementation` from the accepted
two-stage design. The shared registration expander follows only after Stage B.
