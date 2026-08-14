# Current Slug V2 Packet

Packet: `WP-2A-m1-host-package-marker-frontier-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: freeze the smallest callerless Bzlmod-private observed root
package-marker/lookup sibling over immutable policy, the accepted observed
repository-ignore frontier, and ordered `BUILD.bazel`/`BUILD` path-resolution
frontiers. This packet is docs-only and cannot implement or activate a caller.

## Accepted predecessor

Commit `43adf74b` accepts the callerless
`HostRepositoryIgnoreObservationKey` from design `8ac5c30f` and exact
activation `c4ecd395`. It
preserves legacy REPO -> policy -> ordered `.bazelignore` semantics and
retains the complete accepted Host epoch with no parent event authority.

The sole Rust file changed by +708/-16 raw lines: 243 production and 449
in-module test lines, 692 net total, with 2,783 physical lines. These are within
280/450/730 and 2,821. Focused observed proof passes 4/4; all 568 Bzlmod
unit/integration tests pass; `slug_core_v2` checks; formatting and diff hygiene
pass. Strict Clippy stops first in unchanged `allocative_derive`; its
no-dependency run reports only pre-existing crate warnings after the new local
warning was removed. The archive checker reproduces the inherited missing
archive-ref/non-V2-thoughts baseline. WSL has no Windows Rust target, so the
cfg-windows duplicate/operation-mismatch proof is source-checked but not
executed. Independent ownership and AI-cleanup review accepts the 2,783-line
repository-ignore file as one cohesive owner.

This predecessor is still a private producer. It does not certify package
lookup, MODULE loading, source bytes, BUILD evaluation, or any public command.

## Live source boundary

`HostRootPackageLookupKey { workspace, package }` in `host_package.rs`
currently owns one exact ordered terminal:

1. compute `RootPackageLookupInputsProjectionKey`;
2. return structural policy errors, invalid package names, configured deleted
   packages, or the special `external` no-build-file terminal before Host work;
3. compute legacy `HostRepositoryIgnoreKey`, preserving Need/error and mapping
   a match to `Deleted`;
4. for each configured package root in order, probe `BUILD.bazel` and then
   `BUILD` through legacy `ResolvedPathKey`;
5. select the first regular/special marker, preserve resolution error
   precedence, or return `NoBuildFile` after every negative probe.

The accepted `HostRepositoryIgnoreObservationKey` supplies the complete
predecessor epoch. The accepted `ResolvedPathObservationKey` supplies each
exact symlink/lstat/negative resolution prefix. Policy, package identity,
deleted packages, root order, and build-file-name order are immutable semantic
inputs, not reconstructed Host observations.

Root MODULE anchoring, BUILD bytes/evaluation, recursive `.bzl`/glob work,
package-source selection, loading, core finalization, and public commands are
downstream of this lower lookup terminal and remain outside this packet.

## Frozen design

Add exactly one crate-private
`HostRootPackageLookupObservationKey { workspace: NormalizedAbsolutePath,
package: PackagePath }` in `host_package.rs`. Its Display is
`bzlmod-observed-host-root-package-lookup:{workspace}//{package}`; its value is
`PathOutcome<Result<ObservedHostRootPackageLookup,
ObservedPathFrontierError>>`; equality is `complete_eq` and validity is
`is_complete`.

`ObservedHostRootPackageLookup` derives `Debug`, `Clone`, `PartialEq`,
`Eq`, `Allocative`, and `Dupe`. It retains exactly one
`Arc<Result<HostRootPackageLookup, HostRootPackageLookupError>>` plus one
`PathObservationEpoch`, with crate-private borrowed `result()` and
`observations()` accessors. Neither sibling computes the other. Do not export
either new type or change the legacy key, value, error, equality, callers, or
diagnostics.

Preserve this exact compute and terminal algebra:

1. Compute `RootPackageLookupInputsProjectionKey`. A policy error completes as
   the existing inner `PolicyInput` error with an empty epoch.
2. Preserve invalid-package, configured-deletion, and special `external`
   early exits in their existing order. Each has an exact empty epoch and
   activates neither observed ignore nor marker resolution.
3. Compute only `HostRepositoryIgnoreObservationKey`. Forward Need and outer
   frontier error without a parent carrier. Map its semantic error to the
   existing inner `RepositoryIgnore` error while retaining the exact epoch.
   An ignore match completes `Deleted` with that epoch and activates no marker.
4. For each configured root in order, probe `BUILD.bazel` and then `BUILD`
   only through `ResolvedPathObservationKey::new(Host, logical_path)`. Union a
   completed child epoch before inspecting its semantic result.
5. A resolution semantic error completes the existing inner
   `Resolution { logical_path, error }` with the full prefix. Regular/special
   selects the existing `HostPackage`. Missing and every other non-file
   terminal continue as negative probes. Preserve the legacy terminal-symlink
   invariant. Exhaustion completes `NoBuildFile` with every negative prefix.

Use one private `union_observations(left, right)` helper backed exclusively by
`PathObservationEpoch::from_shared`. Input order is accepted ignore epoch,
then root-major/name-major marker epochs. Equal duplicates coalesce and retain
the first exact result Arc; conflicting results and operation mismatch stay
completed outer `ObservedPathFrontierError` values with no carrier. The
accepted epoch utility owns sorting/allocation; do not add custom cardinality
arithmetic or another retained Vec/map/cache/interner/graph/store.

Need is the only invalid/self-unequal value. Need, cancellation, and child outer
failure drop local prefix/epoch scratch and publish no parent carrier. The key
stores no evaluation data and owns no events. DICE-terminal retained state is
only the semantic-result Arc and the existing Arc-backed epoch; policy, ignore
matcher, child resolved values, transaction, event data, and scratch release at
compute return. Child observation Arcs pointer-share through `from_shared`;
no deep-copy or historical-host-read claim is permitted.

Focused proof must cover policy error, invalid name, configured deletion, and
`external` empty epochs with zero observed-child activation; observed-ignore
success/error/Need/outer and ignored deletion; root/name precedence;
missing/wrong-kind negatives, regular/special selection, and resolution-error
prefixes; exact Arc retention, duplicate first-Arc, conflict, and mismatch; zero
legacy lookup/ignore/resolution activation; complete equality/validity;
A/B/A/warm; Need/cancellation; and unchanged legacy diagnostics/order.

No new Bazel oracle is required. Existing serial package-marker selection and
admitted Host observations remain exact regression invariants. Aggregated
frontier identity/equality is Slug-native. Root MODULE anchoring, package-source
bytes, BUILD evaluation, `.bzl`, glob, loading/core/public activation,
routed/materialized repositories, overlap/final-validation, and exact Bazel
identity bytes remain unsupported/deferred.

The future implementation is exactly `app/slug_bzlmod_v2/src/host_package.rs`
with colocated tests. Formatted hard caps are 250 production, 430 test, and 680
total net lines; the physical ceiling is 4,035 from the live 3,355-line
baseline. Require independent cohesion/AI-cleanup review before and after
implementation because the file already exceeds 2,000 lines. The change stays
cohesive only while adjacent to the private root lookup owner and existing
fixtures; a generic frontier module or public seam is a REPLAN.

## Authority and caps

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`; and
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Read only the active packet, this owner section,
`docs/developers/dice.md`,
`.codex/skills/slug-buck2-utility-reuse/SKILL.md`, the matching Stages-3/6
row of `slug-v2-subplans/09-v1-extraction-ledger.md`,
`gazebo/dupe/src/lib.rs`, `allocative/allocative/src/lib.rs`, Bzlmod
`src/{lib,host_package,package_policy,repository_ignore,host_file}.rs`,
workspace `src/{lib,path_observation,path_resolution}.rs`, the Bzlmod and
workspace manifests, and directly referenced focused tests in those files.

The frozen future Rust allowlist is exactly
`app/slug_bzlmod_v2/src/host_package.rs` with colocated tests, but this design
packet cannot activate it. Future caps are 250 production, 430 test, 680 total,
and 4,035 physical lines, with a 180-line completion-ledger cap. Current design
ledger caps are 40 canonical, 320 current-packet, 280 Stage 2, and 640 total net
lines. No code, Cargo, oracle, generated evidence, or Stage 9 write is
authorized.

## STOP / REPLAN

STOP on every code or oracle write; a second package consumer; root MODULE,
source bytes, BUILD evaluation, `.bzl`, glob, loading, core, request-revision,
events, public command/API/output, external/routed/materialized repositories,
legacy key/value/error changes, public export, Cargo/dependency change, reverse
edge, generic certificate framework, new container/cache/interner/graph/store,
reconstructed or direct/historical Host reads, watcher, JVM, or cap excess.

REPLAN to one smaller docs-only prerequisite if the accepted observed
repository-ignore or resolved-path carrier cannot be consumed without changing
legacy keys; policy/root/name precedence cannot be represented structurally;
some lookup terminal has an additional mutable predecessor; exact negative
probes are unavailable before completion; union requires another retained
container; visibility escapes Bzlmod; or one cohesive `host_package.rs`
implementation cannot be bounded.

## Immediate successor

On design acceptance, activate exactly one implementation of the frozen
private package-marker sibling in `host_package.rs`. Its completion may schedule
only docs-only root-module frontier design. Do not combine package source,
MODULE implementation, loading, core, or public activation.
