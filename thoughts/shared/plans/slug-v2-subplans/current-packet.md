# Current Slug V2 Packet

Packet: `WP-2A-m1-root-package-source-frontier-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: freeze one callerless doc-hidden Bzlmod root-package source
frontier carrier/key as the uniquely required prerequisite for loading-side
anchor consumption, without activating loading or changing the legacy source
key.

## Accepted predecessor and REPLAN

Commit `c6e61d60` accepts the callerless
`RootModuleLoadingAnchorObservationKey`: the Bzlmod-to-loading boundary now can
carry the complete root-MODULE frontier, preserve outer frontier errors, and
reuse the exact semantic/observation Arcs without activating loading.

The loading-consumer audit against `a1e58d60` rejects direct activation.
`RootPackageLoadKey` is anchor-first, but then consumes legacy
`RootPackageSourceKey`, parses BUILD bytes, recursively evaluates every
discovered `.bzl`, and executes Host glob attempts before publishing a
`LoadedPackage`. Those mutable predecessors do not share the accepted frontier
boundary. Replacing only the anchor edge would discard its epoch or attach a
partial certificate, and its outer `ObservedPathFrontierError` cannot be
laundered into the public semantic `RootModule` error.

The uniquely smallest prerequisite is the finite root-package source producer.
Accepted private `HostRootPackageLookupObservationKey` and
`HostFileBytesObservationKey` already own its exact package-selection
negatives and selected source bytes. No recursive evaluation, glob, event,
loading, core, or public activation belongs in this packet.

## Design questions

1. Freeze one `#[doc(hidden)] pub ObservedRootPackageSource` in Bzlmod with one
   semantic `Arc<Result<RootPackageSource, RootPackageSourceError>>` and one
   accepted `PathObservationEpoch`, plus borrowed result/observation accessors.
   Freeze one `#[doc(hidden)] pub RootPackageSourceObservationKey` with the
   legacy workspace/request identity and explicit `for_build`/`for_bzl`
   constructors for later loading use.
2. Its Value must preserve
   `SourcePreparationOutcome<Result<ObservedRootPackageSource,
   ObservedPathFrontierError>>`, complete-only equality, and complete-only
   validity. Need/cancellation publishes no carrier; child or union frontier
   failure remains a completed outer error; every completed semantic
   success/error retains its decisive observation prefix.
3. Preserve the legacy candidate order exactly. BUILD inspects only its
   declared package. `.bzl` inspection walks containing-package candidates
   from deepest to declared. Union each completed observed package-lookup epoch
   before interpreting its semantic result, and stop at the first legacy
   terminal. Later candidates may remain ordinary child cache state but cannot
   enter the parent certificate.
4. After package selection, preserve target/path construction and error order.
   A platform path error retains the completed lookup prefix. Compute only the
   observed Host-file sibling, union its epoch before interpreting
   present/missing/error, and retain the full prefix on semantic completion.
5. Decide the smallest shared driver/factoring that keeps the legacy and
   observed source paths behaviorally aligned without making either DICE key
   compute the other. Do not accept a duplicated full source driver, a generic
   public certificate framework, or a change to the legacy key/value/error.
6. Preserve exact Arc identity and cheap-clone/memory accounting. The completed
   carrier may retain only the semantic Result Arc and existing Arc-backed
   epoch; source bytes remain the same shared bytes already owned by the
   semantic result/observation. Retain no child carrier, policy value,
   evaluator, event batch, transaction, candidate vector, path scratch, or
   second collection.
7. The source key owns no events. Prove zero legacy package-lookup/Host-file/
   source activation from the observed path and no evaluation data on either
   Need, outer error, or semantic completion.
8. Freeze the exact implementation files, production/test/total caps, physical
   ceilings, focused proof, direct dependents, large-file cohesion decision,
   residual platform risk, and the single docs-only recursive `.bzl` frontier
   successor.

Existing serial package-source selection, diagnostics, Need/error order, bytes,
and exact Host observation values remain exact. The callerless certificate
association, aggregation, and equality are Slug-native. Loading consumption,
recursive `.bzl`, glob/directory unions, core final validation, public overlap,
routed/materialized repositories, and exact Bazel identity bytes remain
unsupported/deferred.

## Evidence, authority, and caps

Reuse `308b409a`, `0875728b`, `c6e61d60`, and the existing root-package source
tests. No Bazel oracle is required because this design changes no admitted
behavior. Use live source/DICE ownership as authority.

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Read only this packet and owner section, the plan-authoring guide,
`docs/developers/dice.md`, the Buck2 utility-reuse skill, the matching
Stages-3/6 row of `slug-v2-subplans/09-v1-extraction-ledger.md`,
`gazebo/dupe/src/lib.rs`, `allocative/allocative/src/lib.rs`, Bzlmod
`src/{host_package,host_file,lib}.rs`, workspace
`src/{lib,path_observation,path_resolution}.rs`, root `Cargo.toml`, app
`slug_{workspace,bzlmod}_v2/Cargo.toml`, and directly referenced focused tests.

Ledger caps are 40 canonical, 300 current, 260 Stage 2, and 600 total net
lines. No Rust, Cargo, test, oracle, or fixture write is authorized.

## STOP / REPLAN

STOP on every code/oracle write; loading/core changes; changing the accepted
anchor, lookup, Host-file, or legacy source key/value/error/caller; public
user-facing API/wire/output/diagnostic behavior; a second key or carrier;
partial decisive prefixes; outer-error laundering; direct/reconstructed or
historical Host reads; reverse dependency; new graph/store/container/interner;
retained evaluator/event/source-text/AST/transaction/candidate/path scratch;
recursive `.bzl`, glob/directory, repository/materializer, watcher, JVM, or
combined consumer work.

REPLAN if exact lookup/file epochs cannot be consumed without recomputation;
the semantic result cannot reuse one Arc; a complete semantic error cannot
retain its decisive prefix; legacy and observed paths cannot share semantic
orchestration without a second key or behavior change; doc-hidden visibility
would become user-facing; more than `host_package.rs` plus `lib.rs` is required;
or the first implementation cannot be bounded independently.

## Immediate successor

On acceptance, activate only one Bzlmod-side callerless root-package source
frontier implementation. Completion returns to docs-only recursive `.bzl`
frontier design; do not combine loading activation, glob aggregation, core
publication, or another consumer.
