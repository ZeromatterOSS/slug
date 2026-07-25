# Stage 5: Bzlmod Checkpoint Evidence, Part 3

This companion file continues detailed landed evidence for
[05-bzlmod-and-repository-graph.md](./05-bzlmod-and-repository-graph.md).

Use this file for new Stage 5 checkpoint entries after the accepted repository
materialization request/result design. Earlier evidence is in
[Part 1](./05-bzlmod-checkpoint-evidence.md) and
[Part 2](./05-bzlmod-checkpoint-evidence-2.md). Keep each evidence shard below
1000 lines.

## Checkpoint Evidence

### Stage 5 repository materialization request/result implementation

Status: Accepted in `5150dd8f`

Implementation: `slug_bzlmod_v2` now owns a complete-only structural
source-preparation carrier, exact normalized workspace/repository/`RepoSpec`
requests, an immutable per-workspace injected result epoch, and a real cached
per-request DICE projection. Missing or stale results return materialization
Need; Local success derives its logical Host root from the request; Immutable
success retains exact source identity, generation root, and observation
instance. Persistent spec errors are generation-independent, transient
transport/materialization failures are generation-tagged, and repository IO no
longer runs inside DICE.

Equality and evidence: Materialization equality is exact through immutable
root and instance, while source bytes, absence, and typed semantic errors remain
the pruning boundary. Tests prove lawful partial hashing with full request
equality, epoch construction failures, exact-spec isolation, unused and omitted
repository laziness, zero DICE IO, transient retry, logical Local symlink
retargeting, exact immutable-instance selection, cumulative materialization
then path Needs, byte pruning/change/restoration, and pure spec precedence.
One retained graph regression proves unrelated repository result additions and
changes do not invalidate the selected projection, while changing its exact
immutable root or instance does.

Validation: focused `source_preparation_dice` 26/26; full
`slug_bzlmod_v2` 226 tests plus zero doctests; downstream `slug_core_v2` 27
tests plus zero doctests; formatting, diff, exact three-file implementation
allowlist, forbidden repository-IO/obsolete-error/`RepoSpec`-surrogate scans,
and archive guards passed. Independent DICE and pinned Bazel 9.2 terminal
rereviews returned `ACCEPT`.

Residual risk: The runtime still does not produce cumulative result/path
epochs, validate Local roots before observation, retain final immutable
instances across retries, detect exact materialized-output dirtiness, preserve
captured archive bytes through extraction, or publish effects only for the
terminal attempt. Design only
`WP-5-m1-runtime-materialization-preflight-retry-design` next; do not activate
source preparation or edit Rust during that packet.

### Stage 5 runtime materialization preflight/retry design

Status: Replanned before Rust

Frozen command contract: One external command allocates fixed workspace,
registry, and failure-only materialization generations plus one effect owner.
Every retry uses a fresh updater, committed transaction, and exact-version
activation closure. Result and path epochs are complete replacements, never
deltas. Materialization Needs are satisfied outside DICE before path Needs;
each retry must add one exact request result or new path demand, otherwise it
fails with typed internal non-progress rather than an arbitrary retry cap.
Transient failures terminalize only their current generation and are retried
on the next command.

Retention and dirtiness: Runtime state retains each exact request, immutable
success/root/instance, and only its accepted closure's validation demands.
Command start reobserves previous terminal demands, groups materialization
demands by repository, and compares each previously observed operation/path.
Existence or kind changes always dirty only the owning repository. Exact
`FileBytes`, or a digest derived from those exact bytes, takes precedence over
node/mtime proxies; otherwise regular/special lstat compares size plus node and
mtime while ignoring ctime and permissions. `ReadLink` compares exact targets,
and directory observations compare exact entries. Clean immutable repositories
remain offline-reusable; changed `RepoSpec` values cannot reuse them. Local
results retain the logical Host root, validate existence/directory/boundary
before source observation, and reobserve it every command. Obsolete-instance
and abandoned-attempt demands never enter retained state.

Terminal policy: While the accepted transaction remains live, compute the
ordered-root activation closure and select only closure-reachable semantic
event batches and materialization/path demands. Success and final Starlark
failures may publish their selected semantic events; preflight, path, package,
I/O, internal, and closure failures publish none. Immediate transport
progress/debug output is excluded from this semantic-effect claim. REAPI
execution and output materialization occur only after terminal acceptance.
Abandoned retries, cancellation, and incomplete attempts contribute no new
retained state or effects. Terminal Starlark failures follow the accepted
final-closure event and retention policy. Preflight, path, package, I/O,
internal, and closure failures preserve previously accepted retained state and
commit or publish nothing new.

Pinned Bazel correction: Bazel records and rechecks only materialized paths
observed by the retained graph, not an unconditional whole-tree digest.
`DirtinessCheckerUtils`, `FileStateValue`, `FileContentsProxy`, and
`ExternalFilesHelper` establish exact per-repository dirtiness and ctime-only
non-dirtiness. `RepositoryFetchFunction`, `DigestWriter`, and
`RepoRecordedInput` establish exact-spec/recorded-input reuse and final-only
success. `local.bzl`, `FileFunction`, and `FileFunctionTest` establish logical
Local path and symlink-retarget freshness. Archive materialization must capture
the source once and use that same private artifact for checksum, identity, tar
inspection, and extraction; the current core implementation hashes bytes and
then reopens a mutable caller path.

Reason for `REPLAN`: Live Slug has the pure request/result projection and exact
activation closure, but no production path-observation producer, no retained
outside-DICE materializer, no event/demand attempt sidecar or evaluation-event
producers, and no production source-preparation caller. One runtime patch could
not honestly prove complete terminal replacement or publication. The frozen
serial sequence is: Local lifecycle oracle; outside-DICE path producer;
corrected retained runtime materializer including captured archives;
attempt/effect sidecar; event and demand producers; then one shared one-shot
and daemon retry/publication driver. Ordinary build/query activation remains
deferred to the discovery owner.

Next evidence: Run only
`WP-5-m1-local-repository-lifecycle-oracle`. Extend the existing
`module-source-preparation` retained-daemon fixture without runner changes.
Keep one logical Local request while proving valid A, symlink retarget A→B,
missing, regular-file wrong kind, existing boundaryless directory, and exact A
recovery. Failure rows must distinguish directory preflight from repository
boundary validation and exclude downstream MODULE/graph markers. Move the
existing A payload rather than copy it; add only one B `MODULE.bazel`, one
boundaryless `BUILD.bazel`, and three relative symlinks. Do not add an
unused-invalid override: neither a demanded dependency nor an override of an
absent module proves unused-repository laziness.

### Stage 5 Local repository lifecycle oracle

Status: Accepted in `dcc19327`

Oracle: The retained-daemon `module-source-preparation` fixture now keeps one
exact `local_path_override(... path = "local-route")` request across five new
rows. A relative logical-root symlink retargets from the already-mutated A
payload to a B-only `rules_license` graph, becomes missing, becomes a regular
file, points to an existing directory with no repository boundary, and then
restores the exact mutated A graph. Missing and regular-file states both pin
Bazel's path missing/not-directory error before boundary validation; the
boundaryless state pins the distinct missing `MODULE.bazel`, `REPO.bazel`, or
`WORKSPACE` error. Negative assertions exclude the alternate preflight class
and downstream A/B graph markers.

Fixture hygiene: All eleven prior normalized command records remain
byte-identical. The existing A payload moved without copying; B reuses the
fixture's registry scaffold, and no runner/schema or registry topology changed.
Net growth is two regular files, three relative symlinks, five entries, and 282
regular-file lines: 82 TOML, 197 expected JSON, and three asset lines. This is
the second accepted oracle packet after fixture-growth checkpoint `42e38bc3`
(`9fa4fbde`, `dcc19327`), so another hygiene checkpoint is not due.

Validation: Pinned Bazel 9.2 generation and two fresh normalized replays
passed. Direct harness assertions loaded all 16 commands, proved the exact
five-row suffix, all eleven old records by name, and all three relative
symlinks; JSON and `git diff --check` passed. The environment had no `pytest`
module, so the unchanged harness's pytest suite was unavailable and is not
acceptance-blocking. Independent pinned-source and fixture-maintainability
reviews returned `ACCEPT`.

Residual risk: Runtime still cannot produce structural Host or
Materialization observations. Design only
`WP-5-m1-runtime-path-observation-producer-design` next; do not combine the
producer with retained materializer correction, epoch injection, or retries.

### Stage 5 runtime path-observation producer design

Status: Replanned before Rust

Preserved contract: The smallest producer remains a private synchronous
runtime kernel in one new `path_observation.rs` plus one private module
declaration. A compact sorted shared slice maps nonzero materialization
instances to retained roots for authorization only; the producer prevalidates
all instances and duplicate demands before deterministic filesystem work.
Host and Materialization demands retain distinct identities while observing
their exact normalized absolute physical paths, including materialization
symlink escapes. The future caller must retain the materialization owner for
the whole synchronous call and may not store the snapshot independently.

Operation contract: Exact `Lstat`, raw `ReadLink`, exact `FileBytes`, and
sorted raw-OS-native `DirectoryEntries` results form one complete epoch or a
typed structural/runtime error. Every syscall retries `Interrupted`.
`Lstat` maps missing paths and non-directory ancestors to Missing. ReadLink
and DirectoryEntries use an auxiliary no-follow stat only to distinguish a
present wrong kind from a missing target or ancestor; FileBytes similarly
distinguishes a present directory from a missing ancestor. Directory
collection discards partial results and restarts without a cap for
`Interrupted` or Unix `EIO`. The producer never fabricates
`InconsistentState`, performs containment or canonicalization, touches DICE,
or filters ctime/permissions from exact epoch equality.

Pinned metadata contract: Unix lstat retains no-follow kind, signed size/node,
millisecond mtime/ctime using wrapping Bazel arithmetic, and `mode & 0777`.
Windows must retain node `-1`, Bazel permission bits, name-surrogate reparse
links/junctions, and Device or other reparse nodes as special. Mtime uses the
OpenJDK two-stage signed FILETIME formula while ctime uses Bazel's direct
signed formula.

Reason for `REPLAN`: Rust's unstable `MetadataExt::change_time()` returns
`None` on ordinary desktop Windows and cannot provide Bazel's native ctime.
A private raw Win32 query is ABI-feasible without Cargo or crate features, but
the first correction still omitted Bazel's `WindowsPathOperations.asLongPath`
transformation and misclassified path/query failures as outer producer errors.
Pinned Bazel and feasibility reviews require raw UTF-16 verbatim-path
transformation before no-follow `CreateFileW`, immediate error capture,
`FILE_BASIC_INFO`, and operation-level Missing/InvalidInput/PermissionDenied/
exact-I/O projection.

Next evidence: Run only
`WP-5-m1-runtime-path-observation-producer-design-correction`. Correct and
rereview the Windows path/query and operation-result contract while preserving
the private two-file boundary; do not edit Rust or combine the producer with
runtime materialization, epoch injection, retry, or publication.

### Stage 5 runtime path-observation producer design correction

Status: Replanned before Rust

Accepted corrections: Exact desktop-Windows ctime is feasible with a private
std-only raw ABI, Bazel's raw UTF-16 `asLongPath` transformation, no-follow
`CreateFileW`, `FILE_BASIC_INFO`, immediate error capture, and RAII close.
Interior NUL and native query failures remain exact Lstat operation results,
not outer producer failures. The initial `symlink_metadata` read supplies
size, OpenJDK-formula mtime, attributes/permissions, and kind; after
`Interrupted` retry, any other initial Windows metadata failure becomes
Missing. The later native query supplies only Bazel-formula ctime. Architecture
and Rust-feasibility reviews accepted the private two-file, no-dependency
boundary.

Reason for `REPLAN`: The focused source correction first repaired metadata
staging and initial-failure behavior, then terminal rereview found a second
material mismatch: Bazel 9.2's native `IsSymlinkOrJunction` classifies every
`FILE_ATTRIBUTE_REPARSE_POINT` as a symlink, while the reviewed draft used
Rust's name-surrogate-only link classification and treated other reparse
points as special. The correction budget ended before Rust.

Next evidence: Run only
`WP-5-m1-runtime-path-observation-producer-windows-lstat-design-correction`.
Freeze the exact order ReparsePoint→Symlink, Device→SpecialFile,
Directory→Directory, otherwise RegularFile; preserve every other accepted
producer, operation, platform, test, and two-file constraint.

### Stage 5 runtime path-observation producer Windows lstat correction

Status: Accepted before Rust

Corrected contract: Exact Bazel 9.2 Windows lstat validates interior NUL, then
retries the first no-follow metadata read on `Interrupted` and maps every other
initial failure to Missing. That first read alone owns size, OpenJDK-formula
mtime, attributes/permissions, and kind. Kind classification tests the
reparse-point attribute first and maps every such node to Symlink, then maps
Device to SpecialFile, then Directory, otherwise RegularFile. This deliberately
does not use Rust's narrower name-surrogate-only link decision.

The subsequent private raw Win32 query applies Bazel's UTF-16 long-path
transformation and exact no-follow open flags, contributes only native ctime,
captures the last error before RAII close, and maps missing, access denied, and
other raw failures into the Lstat operation result. Pinned Bazel and
architecture reviews returned `ACCEPT`, including overlapping reparse/device/
directory classification evidence and the unchanged std-only two-file scope.

Next evidence: Implement only
`WP-5-m1-runtime-path-observation-producer` in one new private runtime module
plus its private declaration. Preserve every accepted compact authority,
preflight, operation, platform, test, lifetime, and exclusion boundary; do not
wire DICE, retained materialization, retries, or publication.

### Stage 5 runtime path-observation producer implementation

Status: Replanned before retained Rust

Draft evidence: A private two-file implementation reached 15 focused passing
tests for compact sorted authority, complete preflight, exact Unix operations,
metadata, retries, lifecycle freshness, and pure Windows lstat helpers. Root
review corrected portable ReadLink candidates, retained allocation accounting,
inside/escaped authority, explicit deletion, retry-discard, raw-error,
directory-validation, time, and link-retarget evidence. The draft never wired
DICE, runtime materialization, or public APIs.

Reason for `REPLAN`: Final pinned-source review proved Rust
`std::fs::read_link` is not Bazel-compatible on Windows. Rust handles NTFS
symlink and mount-point tags, while Bazel's native
`ReadSymlinkOrJunction` also decodes `IO_REPARSE_TAG_LX_SYMLINK`, treats
`ERROR_NOT_A_REPARSE_POINT` and `ERROR_INVALID_FUNCTION` as not-a-link, treats
ProjFS as not-a-link, and owns distinct open/query/output normalization.
The accepted design had reviewed native Win32 ctime only, not a reparse-buffer
ReadLink parser. Review also found that Bazel retries Unix `EIO` only during
`readdir`, not `opendir`, and two supposedly portable tests used a Unix root
and Unix raw access-denied code. The unaccepted draft was removed, focused
Cargo was not rerun after removal, and the worktree returned clean.

Next evidence: Design only
`WP-5-m1-runtime-path-observation-windows-readlink-design-correction`.
Freeze exact native Windows ReadLink ABI/parsing/output/error semantics and
the ProjFS schema boundary, plus iterator-only Unix EIO retry and portable
tests. Do not edit Rust or combine the producer with materialization, retry,
injection, or publication.
