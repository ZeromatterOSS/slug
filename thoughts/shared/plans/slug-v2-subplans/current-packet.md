# Current Slug V2 Packet

Packet: `WP-4-5-7A-external-bzl-source-observation-cutover-implementation`

Milestone: M7A registered-toolchain closure prerequisite.

Base: accepted Root/Canonical source-observation owner `9764f8a4f`, accepted
canonical policy convergence `fa896aca4`, accepted canonical loading adaptation
`79a36c580`, accepted selected-BCR realization `1599d730c`, and accepted
TestingBootstrap loading ABI `ecee4aca5`. The proof-only registration and
selected-context candidates remain dirty, parked, and read-only.

## Observable boundary

Two fresh-workspace/fresh-output-root rules_rust cqueries now clear the complete
TestingBootstrap lookup and stop identically while loading
`@@bazel_tools//tools/build_defs/cc:action_names.bzl`:

```text
built-in bazel_tools source requires its immutable source owner
```

`ExternalBzlModuleEvalKey` already retains the accepted
`HostRepositorySourceRoute::{Root, Canonical}` carrier. Canonical source reads
use `HostRepositorySourceObservationKey`, but the Root branch still uses the
legacy `HostRepositorySourceFileKey`. A root-apparent load resolved to the
built-in bazel_tools route therefore bypasses `BuiltinBazelToolsSourceFileKey`
and reaches the old physical-materialization guard. This is a generic
external-`.bzl` source-consumer defect, not a missing parser feature,
`action_names` special case, C++ rule engine, or TestingBootstrap failure.

R1 proposed making both Root and Canonical external `.bzl` routes use the
shared observation owner. Independent review returned `REPLAN`: accepted commit
`79a36c580` makes Root-request use of `HostRepositorySourceFileKey` and its
observed sibling—including their dependency identity/order—exact. Full
convergence would silently weaken that slice.

R2 freezes one disposition-aware consumer projection. Root requests keep their
legacy source-file keys exactly; Root built-ins and all Canonical routes use the
shared observation owner. The expected next replay boundary is the catalog's
exact `UnsupportedCatalog` result for `action_names.bzl`; importing the complete
direct `tools/build_defs/cc` package is a separate ordered category packet and
is forbidden here.

## Learned facts and authority

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior and content authority:

- `src/create_embedded_tools.py`, `src/BUILD`, `tools/BUILD`, and
  `tools/build_defs/BUILD` establish the immutable `@bazel_tools` embedded
  repository and select `//tools/build_defs/cc:srcs` into it;
- `tools/build_defs/cc/BUILD` defines one direct package whose complete direct
  file set is `BUILD`, `action_names.bzl`, and `cc_import.bzl`; its `tests` and
  two `whitelists` directories are distinct subpackages/categories;
- pinned `action_names.bzl` is 5,400 bytes/135 lines with SHA-256
  `ede4d3bd51a2a772180a0f3a47cf083e898d4104ec8de27f30ca36a5b8c13951`;
  pinned direct-package `BUILD` is
  `a24f1afcd5bfaaf9fc88ae3455213c83d61988bac5a80e58dd9f954281f6009d`
  and `cc_import.bzl` is
  `a11736b1cf82a1216b62b6c8af280d739721c6dde470ff83cd939112a0a84093`;
  and
- Bazel reads these files as ordinary Starlark sources from the immutable
  built-in repository. Slug must port their bytes verbatim in the later
  category packet; no generated substitute is lawful.

The accepted Slug architecture already owns the needed semantics:

- `BuiltinBazelToolsSourceFileKey` authenticates one catalog-relative file
  against the versioned snapshot and manifest identity;
- `HostRepositorySourceObservationKey` and its observed sibling accept Root or
  Canonical source input and retain the existing zero-copy
  `HostRepositorySourceObservation::{Builtin, Request}` result;
- `HostRepositorySourceRoute` already projects source keys inside Bzlmod
  policy consumers; and
- `ExternalBzlModuleEvalKey` already retains that route carrier, canonical
  label identity, request-local evaluator scratch, and observed frontier.

The relevant upstream embedded-tools dependency shell test is not copied: it
guards Bazel's source dependency inventory rather than Slug's DICE consumer
selection. Existing Slug source-observation tests and the real pinned BCR
dependent provide the stronger discriminating evidence for this cutover.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is concept/optimization guidance only. Its
`session_repository_file_source.zig` retains immutable producer bytes and
manifest identity behind one repository/source-root/path key;
`embedded_io.zig` supplies scoped authenticated reads; and
`session_intrinsic_source_physical_materialization.zig` materializes only for
consumers that require an OS-visible path. Slug follows that ownership lesson
through its already-accepted Rust owners, not Zabel's layout, scheduler, or
compatibility claims.

## Compatibility classification

- **Exact:** existing root-request external `.bzl` values, source names,
  diagnostics, load/cycle behavior and observation epochs remain unchanged;
  built-in external `.bzl` reads use exact catalog bytes and canonical label
  presentation; missing catalog paths return the existing exact typed catalog
  error. This packet makes no new content-completeness claim.
- **Slug-native:** the typed access-projection enum names/layout and the
  Root/Canonical route carrier. Existing Root-request and shared-observation
  key identities, activation order, hashing and equality remain unchanged.
- **Unsupported/deferred:** uncataloged `tools/build_defs/cc` content, physical
  materialization of embedded tools, configured testing/coverage invocation,
  Windows-only repository discovery, exact Java/HotSpot state, and all later
  action families.

## Frozen architecture

### Natural owner and typed consumer projection

Add two doc-hidden public scratch enums beside `HostRepositorySourceRoute`:

- one Legacy read projection containing either
  `RootRequest(HostRepositorySourceFileKey)` or
  `Observation(HostRepositorySourceObservationKey)`; and
- one Observed read projection containing either
  `RootRequest(HostRepositorySourceFileObservationKey)` or
  `Observation(HostRepositorySourceObservationEpochKey)`.

Pure `HostRepositorySourceRoute` methods construct them from one typed relative
path. Root with a request disposition returns the existing Root-request key;
Root with the built-in disposition converts its already-complete source
capability to `HostRepositorySourceInput` and returns the shared observation
key; Canonical reuses its retained `HostCanonicalRepositorySourceInput` and
returns the shared observation key. Re-export only these doc-hidden typed
projections. Loading must match them and may not inspect repository names,
paths, built-in identity, or materialization disposition.

`compute_external_bzl_source` constructs the relative path once and computes
the projected child for Legacy or Observed mode. Keep its existing Root result
branch and Canonical/shared-observation result branch: Root-request values,
errors and dependencies therefore do not change, while Root built-ins reuse the
same zero-copy observation finishing and canonical-label presentation already
accepted for Canonical built-ins. Built-in bytes retain their catalog `Arc`;
requested bytes retain their current producer `Arc`.

Preserve presentation at the final consumer boundary:

- Root request values use their existing producer `logical_path` for evaluator
  source name and presentation path;
- Canonical request and all built-in values use the canonical label, never a
  fabricated Host/output-base path;
- absent values remain `ExternalBzlModuleError::Absent`;
- Root request source errors preserve the existing `Source`/`SourceCompute`
  public shape without translation; canonical and built-in observation errors retain
  `SourceObservation`; and
- parse, load-label, child, cycle, evaluation and freeze behavior does not
  change.

### DICE, revision, and lifetime behavior

The route's full existing structural input plus typed relative path is the
immutable request projection. Root requests preserve their exact direct
dependency on `HostRepositorySourceFileKey` or its observed sibling and that
key's existing path-before-materialization/file dependency order. Built-in
observations depend only on the versioned catalog file key and have an empty
Host-path epoch. Canonical requested sources retain the accepted shared
observation materialization, resolved-path and file-byte dependencies. Observed
mode propagates the same frontier into the external module; Legacy mode remains
frontier-free. Overlapping requests share only immutable DICE values and retain
their existing transaction inputs.

There is no new key, lock, cache, retry loop, background task or filesystem
fallback. Retained memory remains the existing DICE-owned route/input,
catalog/materialized `Arc<[u8]>`, and frozen module. Relative-path conversion,
source-name selection and parsing are compute/evaluator scratch. Publication,
equality cutoff, invalidation, cancellation and shutdown remain owned by the
existing keys.

## Frozen implementation successor

Independent R2 design review returns `ACCEPT`. Implement only
`WP-4-5-7A-external-bzl-source-observation-cutover-implementation` with exactly:

- `app/slug_bzlmod_v2/src/source_preparation/canonical_repository_source.rs`
  blob `257bfbffeb0367fb8bae7c789df43068c52e4ca8`;
- `app/slug_bzlmod_v2/src/lib.rs` blob
  `be4a0f2df037b2d5980718d4a9fbd6d939f7f428`;
- `app/slug_loading_v2/src/bzl_module.rs` blob
  `8309f65c379a12e66fcd53eccfc49cd9f53cb889`; and
- `app/slug_loading_v2/src/canonical_repository_load_route_tests.rs` blob
  `359e6527d23e5b5f6adea2311cc95754a5b0724c`.

Cap additions at 260 production, 320 proof and 580 aggregate Rust lines. The
10,577-line `bzl_module.rs` exceeds the physical-size trigger but remains the
cohesive owner of external-module source finishing; this packet must shrink or
locally simplify its duplicated source branch and may not add a new
responsibility. The 2,706-line test module already owns Root/Canonical route,
source, listing and external-module parity; one bounded discriminating proof is
more cohesive than another crate-level test module.

No catalog Rust/asset, package/loading declaration, registration, analysis,
command, core, REAPI, Cargo, parser, starlark-rust, `set`, `cc_common`, or
`cc_internal` file may change. The thirteen pre-existing dirty files and all
parked proof/selected-context hunks remain byte-for-byte unstaged.

## Required proof and validation

Add two focused proofs. A Root-request external `.bzl` proof must record the
exact direct Legacy dependency row on `HostRepositorySourceFileKey` and the
exact Observed row on `HostRepositorySourceFileObservationKey`, plus their
existing downstream dependency order, source presentation and frontier; these
rows must be identical to the pre-cutover baseline. A root-apparent built-in
proof using existing cataloged `@@bazel_tools//tools:build_defs.bzl` must cover
both Legacy and Observed external-module keys, exact canonical module/source
presentation, empty built-in Host observations, a direct dependency on
`BuiltinBazelToolsSourceFileKey`, and absence of both legacy Host repository
source-file keys. Existing Root error/cycle and Canonical mapping/source/error/
lifecycle tests remain protected.

Run:

1. the focused Root-request and Root-built-in tests plus existing canonical
   external-source/error tests;
2. complete serial `slug_bzlmod_v2` and `slug_loading_v2` tests;
3. the parked four-registration-row proof;
4. `cargo build -p slug_cli_v2`, then two daemon-clean real rules_rust cqueries
   from fresh workspace/output roots; both must clear the immutable-owner error
   and stop consistently at the next independent boundary;
5. formatting, `git diff --check`, allowlist/blob/cap and dirty-isolation
   audits; and
6. `scripts/v2_archive_status.sh`, admitting only its already recorded retained
   documentation exceptions.

Independent terminal review is mandatory because this adds a cross-crate typed
DICE-child projection. Residual risk is an unprotected Root request diagnostic
or dependency-order change; any such change is a `REPLAN`, not an accepted
Slug-native divergence.

## Stops and ordered successor

`REPLAN` for loading-side disposition/name/path inspection; a new DICE key,
cache, physical materialization or copied source bytes; changed Root request or
Canonical behavior; catalog asset edits; an `action_names`, rules_rust,
TestingBootstrap, C++ or OS special case; parser/`set` changes; Rust-defined
rules; work outside the allowlist/caps; or inability to isolate the parked
dirty state.

At terminal `ACCEPT`, replay first. If it reaches exact `UnsupportedCatalog`
for `tools/build_defs/cc/action_names.bzl`, freeze the complete direct
`tools/build_defs/cc` package category—`BUILD`, `action_names.bzl`, and
`cc_import.bzl`, exact pinned bytes/modes/listing/manifest identity—in one
separate catalog packet. Do not import only the demanded file. After that
category clears, re-run the unchanged four-row registration proof and classify
the next actual dependent boundary before returning to selected-context work.

BCR Starlark remains the complete rules/control-flow owner, including
`cc_internal`; `cc_common` remains only a consumer of the generic Host/provider
ABI. Buck2-derived starlark-rust remains the `set` and language owner.
