# Current Slug V2 Packet

Packet: `WP-2A-m1-observed-host-glob-input-frontiers-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: freeze the two natural-owner observed input frontiers required before a
complete Host-glob traversal can exist, without implementing code or activating
glob traversal, BUILD evaluation, package loading, core, or public overlap.

## Accepted predecessor and audit conclusion

Commit `b9fda97d` accepts the callerless recursive Host-`.bzl` frontier.
The subsequent Host-glob audit found no admissible direct traversal
implementation. Two independent lower owners erase observations required by
the same future glob terminal:

- workspace `PathDirectoryListingKey` retains only semantic
  resolution/listing output, not its exact resolved-path epoch plus final
  `DirectoryEntries` result; and
- Bzlmod `HostRootPackageBoundaryKey` retains only ignored/deleted/package/
  no-package semantics, not its exact repository-ignore plus package-lookup
  epochs.

`ResolvedPathObservationKey`,
`HostRepositoryIgnoreObservationKey`, and
`HostRootPackageLookupObservationKey` already provide the lower exact
frontiers. Reconstructing either missing frontier in loading would violate the
natural owner and can change ignored-versus-deleted or listing/symlink
semantics. A listing-only or boundary-only packet still leaves the future glob
frontier partial. The uniquely smallest complete prerequisite is therefore one
joint, callerless, two-owner input-frontier packet.

This is an authority `REPLAN`, not Host-glob acceptance. Existing glob
validation/traversal behavior and all callers remain unchanged.

## Candidate workspace boundary to freeze

Design one doc-hidden public workspace sibling adjacent to
`PathDirectoryListingKey`:

- `ObservedPathDirectoryListing { result:
  Result<PathDirectoryListing, PathDirectoryListingError>, observations:
  PathObservationEpoch }`;
- borrowed `result()` and `observations()` accessors;
- `PathDirectoryListingObservationKey { namespace, logical_path }` with the
  legacy structural identity, a distinct Display, and public constructor for
  the later loading packet; and
- Value `PathOutcome<Result<ObservedPathDirectoryListing,
  ObservedPathFrontierError>>`, `complete_eq` equality and `is_complete`
  validity.

Freeze one mode-aware directory-listing driver used by both legacy and
observed wrappers. Legacy mode computes only `ResolvedPathKey`, keeps its
current Value/error/API and discards the guaranteed-empty transient epoch.
Observed mode computes only `ResolvedPathObservationKey`.

Preserve exact order and polarity:

1. resolution Need forwards with no carrier;
2. completed resolution error, missing or wrong-kind retains only the complete
   resolution epoch in observed mode;
3. only a present directory computes the existing exact
   `DirectoryEntries` demand;
4. final Need forwards with no carrier;
5. observed mode appends the exact returned result Arc before interpreting
   Present/Missing/Error; and
6. operation mismatch or epoch conflict is a completed outer
   `ObservedPathFrontierError`, never a panic or legacy semantic error.

The legacy fixed-operation mismatch remains its existing unreachable invariant.
No key computes its sibling.

## Candidate Bzlmod boundary to freeze

Design one doc-hidden public Bzlmod sibling adjacent to
`HostRootPackageBoundaryKey`:

- `ObservedHostRootPackageBoundary { result:
  Arc<Result<HostRootPackageBoundary, HostRootPackageBoundaryError>>,
  observations: PathObservationEpoch }`;
- borrowed `result()` returning `&Result<...>` and
  `observations()` accessors;
- `HostRootPackageBoundaryObservationKey { workspace, package }` with legacy
  structural identity, distinct Display and public constructor; and
- Value `PathOutcome<Result<ObservedHostRootPackageBoundary,
  ObservedPathFrontierError>>`, complete-only equality and validity.

Freeze one mode-aware boundary driver used by the legacy and observed wrappers.
Legacy mode computes only `HostRepositoryIgnoreKey` followed, when not
ignored, by `HostRootPackageLookupKey`. Observed mode computes only the
matching observed siblings.

Preserve exact order and polarity:

1. repository-ignore Need/outer error forwards without a parent carrier;
2. completed repository-ignore semantic error retains its epoch inside the
   unchanged boundary error;
3. a matching ignore entry completes `IgnoredDirectory` with the ignore epoch
   and does not activate lookup;
4. otherwise compute lookup, union ignore then lookup epochs before semantic
   interpretation, and map Package/Deleted/NoBuildFile/Invalid/error exactly as
   legacy; and
5. lookup Need/outer error or epoch conflict publishes no parent carrier.

Duplicate observations use `PathObservationEpoch::from_shared` with the
already-accumulated left epoch first, retaining its exact Arc for an equal
duplicate. No key computes its sibling, and neither owner stores event data.

## Visibility, memory and non-decisions

Reexport only the two observed carriers/keys under `#[doc(hidden)]` from their
existing crate roots. This is app-internal Rust visibility along the existing
workspace -> Bzlmod -> loading dependency direction, not a user-facing API,
wire, CLI or output change.

Each completed observed value retains only its semantic result (inline for
listing, one Arc for boundary) plus the existing Arc-backed epoch. Child
carriers, policy/matcher values, resolved paths, listing entries beyond their
semantic result, evaluator, transaction, event batch, union vectors and
mode/driver scratch remain compute-local. Need, outer error and cancellation
retain/publish no parent carrier. `Dupe` and `Allocative` follow the existing
accepted sibling patterns. Do not add another container, cache, interner,
graph, store, lock or task.

This packet does not design or implement observed segment candidates,
traversal, adapter projection, BUILD evaluation retry, package aggregation or
final validation. Those remain deferred until both lower siblings are
accepted.

## Required design proof

Before activation, prove from live source and focused tests:

- listing present/missing/wrong-kind/resolution-error and
  `DirectoryEntries` Present/Missing/Error parity with the legacy key;
- exact resolution/listing Arc retention, Need, outer mismatch/conflict,
  complete-only equality/validity, warm and A/B/A;
- boundary ignored short-circuit, repository-ignore error, package lookup
  Package/Deleted/NoBuildFile/Invalid/error parity and deterministic union;
- zero legacy-key activation from both observed graphs and zero parent event
  data;
- cancellation/drop safety from source ownership without a new controllable
  key;
- direct loading compile feasibility through only doc-hidden exports; and
- exact cfg-aware caps and cohesion decisions for the 4,346-line workspace
  owner and the existing boundary module.

Candidate future Rust allowlist is exactly:

- `app/slug_workspace_v2/src/path_resolution.rs`;
- `app/slug_workspace_v2/src/lib.rs`;
- `app/slug_bzlmod_v2/src/host_package_boundary/mod.rs`;
- `app/slug_bzlmod_v2/src/host_package_boundary/tests.rs`; and
- `app/slug_bzlmod_v2/src/lib.rs`.

Target ceilings to confirm or tighten are 115 workspace production, 180
workspace tests, 120 Bzlmod production, 240 Bzlmod tests, eight total lib
reexport lines, 243 aggregate production, 420 aggregate tests and 663 total net
Rust lines. Physical ceilings are 4,641/576 for workspace path-resolution/lib
and 398/1,090/391 for Bzlmod boundary mod/tests/lib. No Cargo change or loading
write is admissible.

## Compatibility boundary

Preserve admitted serial directory-listing, path-resolution,
repository-ignore, package-marker, package-boundary, Host-glob order/Need/error
and event behavior exactly. Existing admitted Host observation values remain
exact. The two callerless carrier associations, epoch union and exact-Arc
identity are Slug-native. Observed segment/traversal/adapter, BUILD retry,
package loading, core publication, public overlap, external/routed repository
globbing, repository/materializer work, native-Windows byte ordering and exact
Bazel identity bytes remain unsupported/deferred.

## Authority and caps

Write exactly canonical, this manifest and Stage 2. Read only:

- `AGENTS.md`, `docs/developers/dice.md`, those three ledgers and directly
  referenced focused tests;
- workspace `src/{lib,path_observation,path_resolution}.rs` and Cargo
  manifest;
- Bzlmod `src/{lib,host_package,repository_ignore}.rs`,
  `src/host_package_boundary/{mod,tests}.rs` and Cargo manifest;
- loading `src/host_glob/{mod,adapter,traversal}.rs` for downstream fit only;
  and
- the utility-reuse skill, matching Stages 3/6 extraction row,
  `gazebo/dupe/src/lib.rs`, `allocative/allocative/src/lib.rs`, and the
  existing `starlark_map` compact collection sources directly used by these
  owners.

Ledger caps are 40 canonical, 300 current, 260 Stage 2 and 600 total net lines.
No correction is authorized.

## STOP / REPLAN

STOP on Rust/Cargo/oracle/generated writes; loading implementation; a third
semantic key/carrier; any legacy Value/error/caller/output change; key-to-key
sibling compute; a generic certificate framework; reconstructed/direct/
historical Host reads; a second retained container; event ownership; reverse
dependency; glob traversal/BUILD/package-load/core/public/repository/
materializer/watcher/JVM work; or ledger cap excess.

REPLAN if either legacy driver cannot be shared without behavior change; exact
listing or boundary observations cannot be exposed at their natural owner;
outer/inner/Need polarity cannot remain distinct; ignored short-circuit or
duplicate-epoch order changes; doc-hidden one-way visibility is impossible;
the two sibling implementations cannot remain independently testable within the
joint cap; or cleanup finds a real split requiring another file.

## Immediate successor

On design acceptance schedule only
`WP-2A-m1-observed-host-glob-input-frontiers-implementation` in the exact five
Rust files above plus completion-only canonical/current/Stage 2. After
implementation acceptance return to docs-only
`WP-2A-m1-host-glob-frontier-design`; do not combine traversal or BUILD work.
