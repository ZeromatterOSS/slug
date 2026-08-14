# Current Slug V2 Packet

Packet: `WP-2A-m1-host-repo-file-frontier-key-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: freeze exactly one callerless Bzlmod-private observed `REPO.bazel`
sibling key and its complete success/error frontier. Preserve the legacy key,
callers, events, and public behavior. Activate no Rust until the design and
independent ownership review are accepted.

## Fixed predecessor and audit result

Commit `308b409a` accepts the callerless lower frontier chain:

- doc-hidden workspace `ResolvedPathObservationKey` retains the exact
  Lstat/ReadLink prefix for complete resolution success/error;
- Bzlmod-private `HostFileBytesObservationKey` consumes only that sibling and
  appends the exact final FileBytes result when required; and
- `PathObservationEpoch` remains the sole retained compact observation map,
  with stable exact-Arc union and typed mismatch/conflict errors.

Commit `a6aaa844` activated the hierarchical composition audit. Inspection
records a prerequisite `REPLAN`:

- root `HostRepositoryIgnoreKey` first consumes legacy `HostRepoFileKey`,
  then immutable ignore policy, then ordered `.bazelignore` Host-file probes;
- `HostRepoFileKey` consumes root `REPO.bazel` through legacy
  `HostFileBytesKey`, evaluates it, and returns only semantic value/error plus
  per-transaction events, discarding the exact observation epoch;
- reconstructing that predecessor in repository-ignore would duplicate work
  above its natural owner;
- root-module `include()` preflight already computes
  `HostRootPackageLookupKey`, so root-module composition cannot precede
  repository-ignore and package-marker composition; and
- visible lockfile and selected package source are separate downstream
  frontiers, not dependencies of `RootModuleLoadingAnchorKey`.

The smallest prerequisite is one observed root REPO-file sibling.
Repository-ignore, package markers, dynamically sealed MODULE includes,
lockfile, selected BUILD/`.bzl`, loading, core, and public migration remain
later packets.

## Design objective

Freeze one crate-private `HostRepoFileObservationKey` in
`app/slug_bzlmod_v2/src/repo_file.rs`. It has the same workspace identity and
semantic ordering as `HostRepoFileKey`, but consumes exactly
`HostFileBytesObservationKey(workspace/REPO.bazel)` and never the legacy
Host-file key.

Freeze private `ObservedHostRepoFile` containing one retained
`Arc<Result<HostRepoFileValue, HostRepoFileError>>` plus the accepted
`PathObservationEpoch`. Its accessor exposes only `&Result`; the sibling keeps
its own produced result allocation and retains exact observation Arcs without
copying them.
The sibling value is
`PathOutcome<Result<ObservedHostRepoFile, ObservedPathFrontierError>>`.
`Need` is the only incomplete state. Dependency frontier error is a completed
outer error with no partial carrier. Legacy policy/Host-file/parse/evaluation
errors remain inner semantic errors with unchanged Display/source behavior.

## Required ownership and ordering

Preserve this order exactly:

1. compute `RootRepoFileSemanticsProjectionKey`;
2. policy failure completes with the same semantic error plus an empty epoch
   and activates neither Host-file key;
3. compute the observed Host-file sibling for root `REPO.bazel`;
4. forward `Need` or outer frontier error without evaluation;
5. retain its exact epoch for missing, wrong-kind, resolution, FileBytes,
   parse, evaluation, and successful terminals; and
6. store the same completed per-transaction event batch as the legacy key when
   capture is enabled, retaining no event batch in the carrier.

Factor only the smallest private semantic evaluation/event-finalization leaf
needed to prove identical result and event behavior. Do not make one DICE key
compute the other, activate a legacy dependency through the sibling, duplicate
Starlark evaluation, or change the legacy value/equality.

The epoch stays deterministic structural identity. Retain no new map,
certificate framework, provenance table, cache, interner, evaluator,
transaction, event batch, extra source bytes, worker, lock, or lease. Carrier
lifetime is only its callerless DICE value and test references.

## Required design output

Freeze all of the following:

- exact private type signatures, constructor/accessor visibility, equality,
  validity, Display, and error-conversion boundaries;
- one shared compute/evaluation adapter, or a precise `REPLAN`, preserving
  legacy and observed activation and event ownership;
- policy failure, missing, wrong kind, resolution/read error, parse/evaluation
  error, success, outer error, Need, and cancellation behavior;
- exact Arc/epoch clone boundaries and compact lifetime;
- one-file Rust allowlist if feasible, production/test/total and physical-line
  caps, proof, compatibility, STOP/REPLAN, and completion records; and
- a docs-only repository-ignore frontier design successor that unions the
  observed REPO epoch with ordered observed `.bazelignore` and
  platform-normalization observations.

## Focused proof to require

Require policy failure with empty epoch/zero Host activation; present, missing,
wrong-kind, resolution/FileBytes/parse/evaluation errors and success with
legacy semantic parity; exact epoch and final Arc identity; outer error with
no carrier/events; Need invalidity and cancellation cleanup; A/B/A equality;
capture-on/off event parity and exactly one completed batch; zero
`HostRepoFileKey`/`HostFileBytesKey` activation through the sibling; compact
allocation/clone accounting; and unchanged legacy tests/callers.

## Compatibility

Preserve accepted serial root `REPO.bazel` missing/present/error/evaluation
behavior, diagnostics, events, and exact Host observation values. Existing
exact slices remain exact. Frontier identity and future validation/retry are
Slug-native. Routed/nonroot repository sources, materializer results,
`.bazelignore`, package markers, MODULE/includes, lockfile, BUILD/`.bzl`,
loading/core/public overlap, and exact Bazel identity bytes remain deferred.

## Authority, allowlists, and caps

Write only canonical, current-packet, and Stage 2 ledgers.

Read only:

- `docs/developers/dice.md`;
- `app/slug_bzlmod_v2/Cargo.toml`;
- `.codex/skills/slug-buck2-utility-reuse/SKILL.md`;
- the matching Stages-3/6 utility row in
  `slug-v2-subplans/09-v1-extraction-ledger.md`;
- `third-party/buck2/gazebo/dupe/src/lib.rs` and
  `third-party/buck2/allocative/allocative/src/lib.rs` only for retained clone
  and memory-accounting reuse;
- Bzlmod `src/{host_file,repo_file,package_policy}.rs`;
- workspace `src/path_observation.rs`; and
- directly referenced focused tests inside those files.

If a named file moved, record the live substitution. Do not read V1/archive
sources or unrelated Stage rows.

The future Rust write allowlist is exactly
`app/slug_bzlmod_v2/src/repo_file.rs`. Its formatted caps are 200 production,
370 in-module test, and 570 total net added lines; the physical ceiling is
2,328 lines. Completion-ledger caps are 40 canonical, 260 current-packet, 220
Stage 2, and 520 total net lines. No cap correction is reserved.

## STOP / REPLAN

STOP on Rust/Cargo/BUILD/oracle/generated-evidence writes; direct
repository-ignore implementation; package/MODULE/lockfile/BUILD/`.bzl`/
loading/core/public caller; routed/nonroot repository/materializer work;
legacy key/value/equality/caller or public API/output/event change; generic
certificate framework; second observed key; new retained container/graph/
store/watcher/historical Host read/JVM; reconstructed demands;
sibling-to-legacy DICE compute; duplicated evaluator; retained
event/evaluator/transaction; or cap excess.

REPLAN if shared evaluation cannot preserve exact event storage/order, policy
failure cannot retain an empty epoch without semantic change, the observed
Host-file epoch cannot be retained by private Arc clones alone, or one
callerless sibling cannot fit a bounded existing-file implementation.

## Immediate successor

After independent design acceptance, activate only the callerless observed
root REPO-file sibling implementation. Completion schedules a docs-only root
repository-ignore frontier design and must not activate that consumer.
