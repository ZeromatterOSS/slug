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

### Stage 5 native Windows ReadLink design correction

Status: Replanned before Rust

Preserved native contract: The exact Windows ReadLink seam is feasible in the
private producer file with no new dependency. It shares raw UTF-16 long-path
conversion, no-follow handle opening, RAII, and immediate error capture with
the ctime query. A checked four-byte-or-better-aligned 16 KiB buffer and pure
byte parser validate the common header, declared payload, tag-specific fixed
prefix, even substitute offset/length, and exact returned range before
decoding UTF-16. NTFS and LX symlinks use the symbolic-link layout, mount
points their layout, ProjFS is not-a-link, and unknown tags are unsupported
I/O. Output removes one `\\?\` or `\??\` prefix and converts every backslash
to slash without resolution, PrintName selection, or relative-flag branching.

Reason for `REPLAN`: Pinned source rejected two assumptions outside the
reviewed scope. Bazel lstat classifies every reparse point as Symlink while
ReadLink reports ProjFS as the distinct `NotASymlinkException`; existing
WrongKind cannot truthfully encode that, generic I/O loses the discriminator,
and Bazel does not fabricate inconsistency. The public workspace observation
schema therefore needs a reviewed NotALink-like result. Separately, Bazel
retains the same directory handle and already-collected names across
`readdir` EINTR/EIO, while pinned Rust `ReadDir` marks end-of-stream after any
iterator error. Exact behavior needs a raw Unix directory seam or approved
dependency, not whole-pass discard/restart. Opener EIO remains immediate, and
only `closedir` EINTR is ignored.

Next evidence: Design only
`WP-5-m1-path-observation-native-schema-and-unix-directory-design`. Freeze the
minimal public NotALink discriminator and every direct consumer/equality test,
plus an exact safe Unix native directory owner and dependency/file boundary.
Preserve the accepted Windows parser and all other producer contracts; do not
edit Rust or activate the runtime.

### Stage 5 native observation schema and Unix directory design

Status: Accepted before Rust

Schema contract: Add one fieldless `PathObservationError::NotALink`. Bazel
collapses native Windows raw 4390, raw 1, and ProjFS into one not-a-link result
even though no-follow lstat classifies every reparse point as Symlink. The
variant carries no path, tag, raw code, or message because the demand and
enclosing errors already own operational context. It remains structurally
distinct from `Io`, truthful `WrongKind`, and evidence-backed
`InconsistentState`; existing resolver, semantic byte, and repository-source
errors preserve it unchanged, with no reexport or production-consumer edit.

Land this prerequisite first in exactly
`path_observation.rs`, `path_resolution.rs`, and
`source_preparation_dice.rs`. Evidence must cover direct equality and every
existing error-class distinction, Symlink then ReadLink NotALink demand
propagation, semantic byte pruning plus transitions to and from I/O, and exact
complete repository-source projection. Do not restore the producer in this
packet.

Native producer contract: After schema acceptance, use a target-Unix workspace
`nix` dependency only for its `libc` ABI and `Errno::{clear,last_raw}` in the
private producer. One safe owner wraps `NonNull<DIR>` and contains all unsafe
calls. Retry only `opendir` EINTR; retain one handle and accumulated raw names
across `readdir` EINTR/EIO; clear errno before each read; distinguish EOF from
error; skip only byte-exact dot entries; and copy transient `d_name` bytes
immediately. An iterator error wins over the one ignored close result. After
EOF, call close once, accept close EINTR without retry, and report every other
close error. Consuming close disarms the Drop fallback even on failure.

Pure scripted backend tests must prove handle identity, retained partial names,
stale-errno clearing, open/read/close error rules, error precedence, and
exactly-one-close; one real Unix test preserves a non-UTF-8 filename. Do not
use `std::fs::ReadDir`, `nix::dir::Dir`, vendored layouts, another dependency,
or public unsafe. The later producer allowlist is exactly
`app/slug_core_v2/Cargo.toml`, `Cargo.lock`,
`app/slug_core_v2/src/runtime/path_observation.rs`, and
`app/slug_core_v2/src/runtime/mod.rs`; it also preserves every previously
accepted Windows parser/lstat and compact producer contract without
runtime/DICE/materializer wiring.

Independent pinned-source, Rust/ABI feasibility, and architecture reviews all
returned `ACCEPT`.

Next evidence: Implement only
`WP-5-m1-path-observation-not-a-link-schema`, then obtain independent
acceptance before scheduling the native producer.

### Stage 5 NotALink observation schema prerequisite

Status: Accepted

Implementation: The public structural observation error now has exactly one
fieldless `NotALink` variant. Direct evidence distinguishes it from every
existing observation error class. Scripted Symlink then ReadLink failure
preserves the exact namespace, requested path, ReadLink demand, and error
through the operational resolver. Retained DICE evidence proves equal
semantic byte errors prune across operational namespace changes while
NotALink-to-I/O and I/O-to-NotALink both invalidate. Immutable repository
source execution projects the same typed error and proves complete key
validity and equality.

Validation: `slug_workspace_v2` 29, full `slug_bzlmod_v2` 226, and
`slug_core_v2` 27 passed serially, including doctests. Formatting, diff,
three-file allowlist, and forbidden-scope checks passed. Existing unused
serde-import, perf-event-patch, and one core dead-code warning are unrelated.
Independent pinned-Bazel/schema and architecture/DICE corrected-diff reviews
both returned `ACCEPT`.

Next evidence: Implement only
`WP-5-m1-runtime-path-observation-native-producer` in the four frozen files.
Do not wire the private producer into DICE, runtime materialization, retry, or
publication.

### Stage 5 native path-observation producer implementation retry

Status: Final corrected design accepted; implementation current

The rejected draft was removed before this correction. Three independent
terminal rereviews accepted a private four-file implementation with no public
schema, DICE, materializer, retry, publication, fixture, or consumer change.
The producer owns a lifetime-bound `Allocative` sorted `Arc` slice of normalized
materialization roots, completely preflights sorted demands, then executes that
same order. Roots authorize instance identity only; exact demanded paths are
never rewritten or containment-checked.

ReadLink, FileBytes, and DirectoryEntries are primary operations. Auxiliary
no-follow lstat runs only after their frozen candidate failures and resolves
races to Missing or truthful WrongKind while otherwise preserving the original
operation error. Unix directory enumeration uses one raw handle with exact
open/read/close EINTR, iterator EIO, partial-name, errno, and disarmed-owner
precedence. Windows uses a copied pinned `windows-sys 0.61.2` kernel32 ABI for
raw Find enumeration, exact Bazel `0555`/`0755`, mtime/ctime and long-path
rules, checked reparse parsing, and a returned-length bounds check before every
slice.

Required evidence covers every pure failure/race cell, sorted preflight and
execution, retained-root allocation/authority, scripted Unix and Windows handle
lifecycle, Windows ABI/layout/path/time/parser/name behavior, portable
create/edit/delete/recreate, directory and symlink transitions, and distinct
Host/materialization paths. Terminal acceptance additionally requires a native
or correctly provisioned Windows `cargo test -p slug_core_v2 --no-run` that
links the test binary against kernel32; cross-target checking is not enough.

Next evidence: Implement only
`WP-5-m1-runtime-path-observation-native-producer-final` in
`app/slug_core_v2/src/runtime/path_observation.rs`, `runtime/mod.rs`,
`app/slug_core_v2/Cargo.toml`, and `Cargo.lock`, adding only target-Unix
workspace `nix`.

### Stage 5 native path-observation producer final implementation

Status: Replanned; draft removed

A bounded four-file writer produced a 1,167-line private draft. Linux validation
passed four focused tests, 18 unit tests, 13 integration tests, and doctests;
Windows `cargo check` also passed. Root provisioned
`x86_64-pc-windows-gnu` plus MinGW and independently proved the required real
`cargo test -p slug_core_v2 --target x86_64-pc-windows-gnu --no-run` link,
including both test executables and kernel32.

The single permitted correction fixed unterminated Windows find names, shared
race refinement, Unix close errno capture and interior-NUL handling, and pure
Windows kind/permission/time/path helpers. Six focused, 20 unit, 13 integration,
and zero doctests then passed with formatting, diff, and allowlist checks.

Reason for `REPLAN`: The correction still lacked the generic Windows Find
backend/owner and its raw-error/close/partial-result matrix, scripted Unix
open/read/close ownership evidence, sorted-execution and zero-operation
preflight evidence, the complete primary/auxiliary operation table, the
reparse/ABI/time/path parser matrix, and the full directory/symlink/escaped-root
real lifecycle matrix. The worker correctly stopped rather than treating broad
crate tests or a linked Windows artifact as substitutes for those
discriminating regressions. The complete unaccepted draft and manifest/module
edits were removed; the worktree returned clean.

Next evidence: Design only
`WP-5-m1-runtime-path-observation-native-producer-checkpoint-split`. Preserve
the accepted terminal contract while freezing serial platform-neutral, Unix,
Windows, and any necessary final lifecycle-test owners that can each land
complete private behavior without temporary activated-platform wrongness.

### Stage 5 native path-observation implementation checkpoint split

Status: Accepted

Independent neutral-kernel, Unix-owner, Windows-source, and terminal
architecture reviews accepted three serial checkpoints with no fourth
lifecycle packet. The split preserves the final four-file union and introduces
no observable partial platform behavior because the module and every native
entrypoint remain private and callerless.

Owner A lands only the unconditional private, dependency-free neutral module:
normalized retained-root authority, sorted complete preflight, exact demanded
paths, primary-versus-refinement state, auxiliary lstat tables, complete epoch
construction, and exhaustive scripted order/race evidence. Only unsupported
Lstat is an outer zero-operation preflight failure; unsupported non-Lstat
operations remain final adapter results. No native implementation exists in
this checkpoint.

Owner B then adds only target-Unix workspace `nix`, a complete cfg-Unix adapter
and raw same-handle directory owner, exhaustive open/read/errno/close/Drop
scripts, and all real temp-derived file, directory, symlink,
Host/materialization, inside-root, and escaped-path lifecycle evidence. Windows
still has no entrypoint or stub.

Owner C edits only the private module to add host-pure Windows helpers and
scripts plus cfg-Windows ABI/FFI adapters for staged lstat, ReadLink, checked
reparse parsing, and raw Find enumeration. It must pass the real provisioned
GNU-Windows `cargo test --no-run` kernel32 link gate. The exact Bazel 9.2
formulas, UTF-16 behavior, ownership/error ordering, terminal exclusions, and
previously accepted operation tables remain unchanged.

Next evidence: Implement only
`WP-5-m1-runtime-path-observation-neutral-kernel` in the new private module and
`runtime/mod.rs`. Do not add Cargo, native, public, or caller changes.

### Stage 5 platform-neutral path-observation kernel

Status: Accepted

The two-file dependency-free checkpoint adds one unconditional private dormant
module with the lifetime-bound `Allocative` sorted `Arc` root authority,
complete sorted duplicate/instance/Lstat-support preflight, exact demanded-path
execution, adapter-owned primary operations, and neutral auxiliary-lstat
refinement. Primary failures can only be Refine or Final, so adapters cannot
bypass the accepted Missing race tables. The module has no native
implementation, public export, caller, or activation.

Fifteen focused scripted tests prove exact structural errors and zero operation
calls, compact root ordering and shared roots, Host/materialization identity,
no rewriting, complete sorted execution with error continuation, direct Lstat
Present/Missing/I/O, every primary/refinement/no-auxiliary row across node
kinds, typed NotALink and generic-I/O preservation, special-file-compatible
bytes, and matching epoch variants. Root validation passed 29 unit, 13
integration, and zero doctests, host and GNU-Windows all-target checks,
formatting, diff, archive status, exact two-file scope, and forbidden-reference
scans. Existing perf-event, serde-import, and core dead-code warnings are
unrelated. Independent pinned-contract and architecture/performance reviews
both returned `ACCEPT`.

Next evidence: Implement only
`WP-5-m1-runtime-path-observation-unix-adapter` in the private module,
`app/slug_core_v2/Cargo.toml`, and `Cargo.lock` only if resolution changes.

### Stage 5 Unix native path-observation adapter

Status: Accepted

The cfg-Unix private adapter implements the neutral trait with exact native
lstat, primary-first ReadLink/FileBytes classification, and a raw same-handle
directory API/owner. Only target-Unix workspace `nix` was added; the lockfile
did not change. Four documented unsafe libc operations are confined to the
native API. Explicit close disarms before calling, Drop closes only a remaining
handle, and iterator/validation errors retain precedence over ignored cleanup.

Ten focused Unix tests prove Interrupted retry, opener EINTR/EIO/other errors,
one handle and partial names across iterator EINTR/EIO, clear-before-read and
EOF errno behavior, raw non-UTF-8 names, iterator and close candidate/error
precedence, EOF close success/EINTR/error, invalid/duplicate data, explicit
disarming, Drop fallback, and exactly-once close. Real temp-derived tests cover
all lstat kinds and metadata, relative ReadLink, followed file bytes,
create/edit/delete/recreate, observed directory mutation/delete/recreate,
symlink retarget/delete/recreate, wrong kinds, non-UTF-8 entries, distinct
Host/materialization identity, and authorized inside and escaped exact paths.

Root validation passed 39 unit, 13 integration, and zero doctests, GNU-Windows
neutral-boundary all-target compilation, formatting, diff, archive, exact
two-file scope, and forbidden-reference scans. Existing diagnostics are
unrelated. Independent pinned-Bazel and architecture/unsafe-owner reviews both
returned `ACCEPT`.

### Stage 5 Windows native path-observation adapter

Status: Accepted

The one-file checkpoint adds a safe host-pure Windows helper/script layer and
a cfg-Windows copied-ABI native adapter. It preserves exact staged lstat
classification and formulas, no-follow ReadLink with checked NTFS/LX/mount
reparse parsing, raw WTF-16 paths and names, and one-owner Find enumeration
with terminal partial-result discard. Unsafe remains confined to the copied
kernel32 ABI and native RAII boundary; there is no dependency, public, caller,
DICE, materializer, publication, fixture, or non-Windows change.

Ten focused Windows tests exhaust the eight attribute combinations, permission
and FILETIME boundaries, long-path forms, parser tags/ranges/capacity,
open/query/close precedence, Find initial/iterator/EOF/error/partial/Drop
behavior, and exact error refinement. Root validation passed 49 unit, 13
integration, and zero doctests. The real GNU-Windows no-run build produced
both unit and integration executables, and object inspection confirmed their
kernel32 imports. Formatting, diff, archive, and exact one-file scope passed.
Independent pinned-Bazel/windows-sys and architecture/unsafe-owner reviews
both returned `ACCEPT`.

### Stage 5 retained runtime materializer and captured-archive design

Status: Replanned before Rust

The read-only audits retained a bounded three-file outside-DICE owner:
one `RepositoryMaterializer` per `WorkspaceRuntime`, command-scoped provisional
sessions, full-request accepted reuse, append-only accepted roots, exact
per-repository validation dirtiness, logical Host-only Local success, and a
single private archive capture. A short owner mutex can snapshot, allocate
monotonic instance IDs, and terminally promote without crossing path I/O,
archive/Git work, awaits, or DICE. Provisional roots remain session-owned
across retries; once exposed in an epoch their IDs are burned even if the
session is abandoned. Complete epochs are cumulative replacements, and only a
future terminal-accept caller may promote closure-reachable results and
validation.

The focused correction fixed ID lifetime, terminal-only promotion, full
request equality, exact FileBytes/ReadLink/directory and ctime-ignoring lstat
dirtiness, captured-byte checksum/identity/inspection/extraction, and exact
prefix filtering. Terminal pinned-source rereview then found a second material
gap: overlapping sessions still lacked a single-lease/stale-token rule, and
archive extraction still lacked the exact Bazel create/capture/checksum/
extraction precedence plus normalize-and-relativize-before-prefix semantics.
Per the packet review limit, no Rust was started. Existing symlink/hardlink tar
support also remains an explicitly deferred Bazel parity gap rather than an
accepted property of the regular-file/directory subset.

Next evidence: Design only
`WP-5-m1-runtime-materializer-session-archive-order-design-correction`.
Correct only the single active-session lease and stale-token behavior, exact
archive stage precedence, and normalized/relativized exact-prefix extraction.
Preserve the three-file private implementation boundary and every already
accepted owner, dirtiness, lifetime, Local, epoch, capture, and exclusion
decision.

### Stage 5 retained materializer session/archive-order design correction

Status: Accepted before Rust

The corrected owner permits exactly one active nonzero session token per
`WorkspaceRuntime`. Busy begin and stale epoch/accept/discard operations are
typed no-mutation errors. Accepted state stores each full request beside its
ID, repo-keyed terminal validation, and append-only published roots. The
owner's short mutex covers only synchronous snapshot, token/instance
allocation, root registration, and terminal promotion; no guard crosses path
observation, archive/Git work, await, or DICE.

Fresh immutable roots remain provisional across cumulative retry epochs and
receive a unique instance before first DICE exposure; abandoned IDs are never
reused. Provisional TempDirs are physically quarantined under the active owner
token, so dropping a session handle cannot release a DICE-addressable path.
Only explicit matching-token accept or discard, after the future caller has
restored accepted injection and ended every transaction that can reference
unselected roots, may move selected roots to append-only accepted retention or
release the rest. Local success remains logical Host-only and allocation-free.

At session start, accepted validation demands are reobserved outside DICE with
the retained-root authority. Stable Missing is clean and any observation error
is dirty. Existence/kind changes dirty the owning repository. Exact FileBytes
takes precedence over lstat proxies; otherwise regular/special lstat compares
size, node, and mtime while ignoring ctime and permissions. ReadLink targets
and directory entries compare exactly. Only a clean full-request match reuses
the same immutable root/instance offline. Each session epoch is one sorted
cumulative complete replacement; only a future terminal caller may promote
closure-selected results and validation.

The supported archive sequence is structural strip/URL validation and saved
checksum parsing; unpublished output root and private capture creation; one
caller-source capture/hash; saved malformed-checksum or checksum result;
inspection and exact prefix selection; then extraction. Source/capture failure
wins over a saved malformed checksum; malformed checksum after successful
capture is persistent Spec, valid mismatch is current-generation Transport,
and later tar/filter/extract failures are current-generation Materialization.
Checksum, source identity, every inspection, and extraction use only the
private artifact.

Archive member and prefix bytes use Bazel's normalize-then-absolute-to-relative
component semantics before exact prefix selection and destination containment.
Outside-prefix entries are skipped and an absent prefix fails. Evidence must
cover overlapping sessions and stale tokens, zero/MAX allocation, implicit
handle drop retention, exact dirtiness/reuse, cancellation, complete epoch
order, mutable-source capture, dual-stage failures, absolute and
`foo/../bar` members, same-depth sibling prefixes, absent prefixes, normalized
collisions, and old-root readability. Symlink/hardlink and broader raw-tar
support remain explicit Bazel parity gaps after this regular/directory subset.
Independent pinned-source and architecture terminal reviews returned
`ACCEPT`.

Next evidence: Implement only
`WP-5-m1-runtime-materializer-session-archive-implementation` in
`app/slug_core_v2/src/runtime/repository_io.rs`,
`app/slug_core_v2/src/runtime/path_observation.rs`, and
`app/slug_core_v2/src/runtime/dice.rs`. The path file may add only one private
native-observer/root-authority entrypoint, and the DICE file only one private
owner field plus construction. Do not add a dependency, public API, DICE key,
source-preparation change, sidecar, producer, retry/publication, command
activation, fixture, or discovery behavior.

### Stage 5 retained materializer/session/archive implementation

Status: Replanned before Rust

No source edit was retained. The implementation contract remained sound, but
the pinned acceptance checklist proved that the declared regular/directory tar
subset itself requires a byte-oriented USTAR parser: raw non-UTF-8 name plus
prefix assembly, octal size and padded payload bounds, typeflag/name-suffix
directory classification, archive-order normalized collisions, exact
normalize/relativize/prefix/containment behavior, Bazel's accepted
checksum-only corruption and physical-EOF cases, and explicit PAX/GNU/link/
special rejection. That independently reviewable parser and mutation/error
matrix could not be combined honestly with the single-lease retained-session,
dirtiness, epoch, lifetime, cancellation, and native-owner bridge matrices.

Next evidence: Design only
`WP-5-m1-runtime-materializer-implementation-checkpoint-split`. Preserve the
accepted three-file union but freeze serial checkpoints for (1) the one-file
captured USTAR regular/directory subset, (2) the one-file pure retained-session
and dirtiness kernel with scripted materialization/observation, and (3) the
native observer plus `WorkspaceRuntime` owner bridge. Do not edit Rust or
weaken any accepted archive, session, lifetime, equality, error, or exclusion
gate.

### Stage 5 retained materializer implementation checkpoint split

Status: Accepted before Rust

The accepted union is split into three serial terminal checkpoints:

1. `WP-5-m1-runtime-http-archive-captured-ustar-subset` edits only
   `runtime/repository_io.rs`. It replaces only `http_archive` caller-path
   reuse with the accepted private capture and byte-oriented USTAR
   regular/directory parser/extractor. It owns stage/error precedence, raw
   names and USTAR prefix, numeric/padding bounds, type/name directory rules,
   physical EOF/checksum quirks, streamed normalization/prefix/containment and
   collision behavior, and explicit PAX/GNU/link/special rejection.
   `materialize_git` and its existing private stdout/NamedTempFile/external-tar
   path remain byte-identical because real `git archive` emits PAX metadata.
2. `WP-5-m1-runtime-retained-session-kernel` then edits only
   `runtime/repository_io.rs`. It adds the callerless single-lease owner,
   owner-held provisional and append-only roots, burned IDs, full-request
   accepted cache, exact validation dirtiness, Local result, cumulative
   complete epochs, explicit token-checked accept/discard, and scripted
   materialization/observation evidence. It adds no native observation or
   runtime field.
3. `WP-5-m1-runtime-retained-native-owner-bridge` finally adds only the
   `pub(super)` retained-root/native-observer entrypoint in
   `runtime/path_observation.rs`, the minimum call bridge in
   `runtime/repository_io.rs`, and one private `Arc<RepositoryMaterializer>`
   field plus initialization in `runtime/dice.rs`. It adds no DICE key,
   injection, retry, sidecar, command activation, or public API.

The focused correction kept Git off the HTTP USTAR parser after pinned review
proved even an empty real Git archive begins with a global PAX header.
Independent pinned-source, implementation-feasibility, and architecture
terminal reviews returned `ACCEPT`; no requirement leaks backward across the
three packets. Git PAX/GNU/external-tar parity and HTTP PAX/GNU/link/special
breadth remain explicit residuals.

Next evidence: Implement only
`WP-5-m1-runtime-http-archive-captured-ustar-subset` in
`app/slug_core_v2/src/runtime/repository_io.rs`. Do not change Git code or add
retained session/runtime/native-observer behavior.

### Stage 5 HTTP captured-USTAR implementation

Status: Replanned before Rust

The one-file draft preserved one caller-source read, private captured bytes,
checksum precedence, raw USTAR names and prefix fields, normalized prefix
selection, archive-order collisions, selected type rejection, outside-prefix
skip precedence, payload/padding bounds, checksum tolerance, complete-entry
physical EOF, and byte-identical Git extraction. Focused tests, the full core
suite, and a real GNU-Windows no-run build passed before terminal review.

The permitted Windows destination-containment correction was applied locally,
but two independent terminal reviews then found further pinned mismatches:
inspection and extraction were interleaved, nonempty initial short records were
rejected contrary to Commons Compress 1.26.1, `strip_prefix` used UTF-8 instead
of Latin-1/raw-like bytes, leading-NUL octal fields disagreed with
`TarUtils.parseOctal`, Windows drive-absolute members were not relativized, and
capture write/flush failures were classified as Materialization rather than
Transport and could not be injected. The draft was removed in full; no Rust or
tests were retained. A subsequent direct source check corrected one review
claim: Commons accepts a short next-header read as EOF after a complete padded
entry too, so that behavior in the removed draft was not a mismatch.

Next evidence: Design only
`WP-5-m1-runtime-http-archive-captured-ustar-design-correction`. Preserve the
accepted one-file/Git/private-capture boundary, but freeze parse-before-write,
short-record and numeric semantics, Latin-1 prefix encoding, Windows
normalization/containment, capture-writer injection/stage classification, and
their exact discriminating rows before another implementation attempt.

### Stage 5 HTTP captured-USTAR design correction

Status: Accepted before Rust

The retry remains private to `runtime/repository_io.rs`. It may add only
`ArchiveFailureStage`, `ArchiveMaterializationError`, `SavedChecksum`,
`CapturedArchive`, raw-path and planned-entry types, an `ArchiveIo` test seam,
a private `ArchiveDestination` execution seam, and private
capture/inspect/plan/extract helpers. `ArchiveIo` owns separately injectable
root, artifact, source-read, artifact-write, and artifact-flush operations;
`ArchiveDestination` counts or performs parent, directory, and regular-file
writes only after planning. `materialize_git` through its existing
`extract_tar` call remains byte-identical; no dependency, public API, session,
runtime, path-observation, DICE, source-preparation, sidecar, producer, retry,
publication, command, fixture, or discovery behavior enters this packet.

The exact order is structural URL/type/prefix validation plus saved checksum
parsing; unpublished output-root creation; private capture-artifact creation;
one caller-source open/read; artifact `write_all` and flush; saved malformed
checksum; valid checksum mismatch; exhaustive archive inspection and immutable
extraction planning; then archive-order extraction from the privately owned
captured bytes. Output-root and capture-artifact creation are Materialization;
source open/read and artifact write/flush are Transport; the delayed malformed
checksum is Spec; mismatch is Transport; inspection, filtering, planning, and
extraction are Materialization. Each operation is separately injectable, all
capture operations precede a saved checksum error, and every failure drops the
unpublished root and artifact.

Inspection performs no archive-entry or destination mutation. It reads a
complete 512-byte header or treats every shorter header-position read as
physical EOF, both initially and after complete padded entries. A complete
header with insufficient declared payload or record padding still fails.
Octal size follows Commons Compress 1.26.1: a leading NUL returns zero
immediately, leading spaces and trailing NUL/spaces are permitted, and invalid
digits fail. Base-256 remains an explicit regular/directory-subset rejection,
including outside the selected prefix because size is required to locate the
next header. Tar-header checksum corruption remains tolerated.

Name is the raw 100-byte field through its first NUL, joined after the raw
prefix field when present. Legacy/no-magic regular headers and arbitrary POSIX
version bytes remain accepted and still combine that prefix. GNU-magic and
XSTAR layouts are archive-wide subset rejections before their non-USTAR bytes
can be interpreted as a prefix; selected PAX/GNU metadata, links, and special
types remain rejected. Outside-prefix entries skip before ordinary type and
containment rejection, but not before format classification or the numeric and
payload bounds needed to continue inspection. NUL/`0` are regular files; type
`5` and trailing-slash names are directories.

`strip_prefix` uses Java `String.getBytes(ISO_8859_1)` semantics:
U+0000..U+00FF map byte-for-byte and each unmappable scalar becomes `?`.
Member and prefix paths then share one host-pure flavor normalizer. Unix uses
only `/` as a separator and preserves raw backslash bytes. Windows uses both
separators, makes leading-root and `C:/` or `C:\` drive-absolute paths
relative, normalizes dot/parent components, and leaves drive-relative `C:foo`
distinct. Exact component prefix selection precedes safe native-component
construction; residual prefix/root/parent/invalid components fail, and the
final destination must start with the unpublished root.

The immutable plan owns entry kind, normalized destination components, and
payload ranges into captured bytes. It rejects namespace collisions that could
only fail after partial extraction, permits repeated directories and repeated
regular-file destinations, and replays all valid entries in archive order so a
later regular file at the same normalized path wins. Explicit and implicit
directories remain compatible; file/directory and ancestor-file conflicts
fail before extraction. An absent prefix and any later malformed or selected
unsupported entry therefore produce zero destination-writer calls.

Focused evidence must cover empty, 1-byte, and 511-byte initial EOF; a complete
valid entry followed immediately by physical EOF; both short trailing sizes
after one complete padded entry; checksum-only header corruption acceptance;
truncated payload and padding; leading-NUL, ordinary, invalid, overflow, and
selected/outside base-256 size fields; no-magic prefix combination, arbitrary
POSIX version, archive-wide GNU-magic/XSTAR (including a discriminating
prefix-layout tail), and selected PAX/GNU-metadata/link/special rejection;
regular NUL/`0`, type `5` with and without a trailing slash, and
trailing-slash implicit-directory classification; the same ordinary
unsupported type skipped outside the prefix and rejected when selected; raw
non-UTF-8 name and prefix fields; U+00FF and U+0100 Starlark prefixes; Unix
backslash preservation; Windows `C:/`, `C:\`, rooted, UNC-like,
embedded-backslash, drive-relative, parent, and containment rows; prefix-root,
sibling, absent, and unsafe-outside-prefix order; repeated file order and every
namespace collision; zero destination writes after a late inspection failure;
one caller read plus post-capture mutation/deletion; every stage-precedence
pair; failure cleanup; exact Git diff; focused/full core, GNU-Windows no-run,
format, diff, archive, scope, and forbidden-reference gates.

Independent pinned-source, one-file implementation-feasibility, and
architecture terminal rereviews returned `ACCEPT` after the focused correction
made all short records EOF, replaced blanket Windows rejection with exact
flavor normalization, preserved artifact-creation staging, classified
GNU/XSTAR before USTAR prefix interpretation, and restored the complete
checksum/EOF/type/skip evidence rows.

Next evidence: Implement only
`WP-5-m1-runtime-http-archive-captured-ustar-subset-retry` in
`app/slug_core_v2/src/runtime/repository_io.rs`. Do not change Git code or add
retained session/runtime/native-observer behavior.

### Stage 5 HTTP captured-USTAR implementation retry

Status: Accepted

The one-file implementation now captures the caller source once into private
owned bytes plus a retained artifact, preserves the exact Spec/Transport/
Materialization precedence, exhaustively inspects and plans before destination
writes, and extracts only the accepted raw regular/directory USTAR subset.
It pins Commons short-record, checksum-syntax, leading-NUL octal, padding,
default/POSIX/GNU/XSTAR, ISO-8859-1, Unix/Windows path-flavor, exact-prefix,
containment, collision, mutation, cleanup, and archive-order behavior while
leaving Git's existing external-tar path byte-identical.

The focused correction added structural safe-prefix validation, selected-type
classification before suffix/prefix-root handling, checksum-field syntax,
full XUSTAR evidence, an end-to-end parent/directory/file destination seam,
dual-failure ordering, unsafe-outside-prefix rows, and the complete namespace
matrix. All three terminal rereviews returned `ACCEPT`.

Validation passed 12 focused repository tests, 55 full unit tests, 13
integration tests, and zero doctests. The GNU-Windows no-run gate produced
both unit and integration executables. Formatting, diff, archive, exact
one-file scope, forbidden-boundary, cleanup, and Git byte-identity checks
passed; only pre-existing workspace warnings remained.

Next evidence: Implement only
`WP-5-m1-runtime-retained-session-kernel` in
`app/slug_core_v2/src/runtime/repository_io.rs`. Add no native observation,
runtime field, public API, dependency, DICE key, sidecar, producer, retry, or
publication behavior.

### Stage 5 pure retained-session kernel

Status: Accepted

The callerless one-file kernel owns one checked nonzero session lease, explicit
Pending/InProgress/Complete validation, owner-quarantined provisional roots,
append-only accepted roots, burned instance IDs, exact full-request reuse,
logical rootless Local successes, sorted cumulative complete epochs, and
token-checked accept/discard. Only explicit terminal operations release
unselected provisionals after unlocking; dropping or losing the token cannot
release a DICE-addressable root.

Accepted validation is repo-local and reobserved outside the mutex. Stable
Missing is clean, observation errors and existence/kind/size changes are
dirty, exact FileBytes suppresses only node/mtime proxies, ctime and permissions
are ignored, and ReadLink targets and directory entries compare exactly.
Terminal accept replaces the logical cache with selected successes and
validation while physical accepted roots remain readable; selected failures,
discard, cancellation, Busy, stale, incomplete-validation, and allocator
errors preserve accepted state.

Scripted evidence covers cancellation after token exposure, Busy/stale and
zero/MAX no-mutation behavior, nonreuse of burned IDs, explicit cleanup after
unlock, clean-base plus dirty-replacement epoch equality, transport-to-
materialization replacement, exact request/spec/kind/logical-root matching,
repo-local dirtiness, Local allocation freedom, failure preservation, and old
root readability. The pinned Bazel source was checkout `b1acdef69e`.

Validation passed 17 focused repository tests, 60 full unit tests, 13
integration tests, and zero doctests. Both GNU-Windows test executables linked.
Formatting, diff, archive, one-file scope, and exact HTTP/Git function
byte-identity checks passed. Three terminal corrected-diff rereviews returned
`ACCEPT`; only pre-existing workspace warnings remained.

Next evidence: Implement only
`WP-5-m1-runtime-retained-native-owner-bridge`. Add the private native
retained-root observer entrypoint, the minimum repository materializer call
bridge, and one private runtime owner field plus initialization. Add no DICE
key, injection, retry, sidecar, command activation, public API, dependency,
fixture, or discovery behavior.

### Stage 5 retained native-owner bridge

Status: Accepted

The three-file bridge exposes one `pub(super)` synchronous native observation
entrypoint over the existing Unix and Windows adapters, adds the minimum
retained-session native validation/observation calls, and gives each
`WorkspaceRuntime` one distinct private `Arc<RepositoryMaterializer>` built
from the canonical normalized workspace. It adds no DICE key or injection,
retry, sidecar, command activation, dependency, source-preparation path,
fixture, discovery behavior, or public API.

Retained immutable roots are `Arc<TempDir>`-pinned from the locked snapshot
through normalization, unlocked native I/O, and the post-I/O token check.
Concurrent discard therefore yields stale state without destroying a root
under observation; final root release remains outside the mutex. Native
structural errors do not mutate accepted or provisional state, and duplicate
accepted demands are rejected before acceptance so malformed validation cannot
poison later cache reuse.

Evidence covers Host and exact materialization authority, escaped physical
paths, clean/edit/delete/recreate immutable validation, exact-byte recovery,
logical Local symlink retargeting, malformed validation rejection with prior
reuse preserved, and distinct exact runtime owners. Validation passed 19
focused repository tests, 63 full unit tests, 13 integration tests, and zero
doctests. Both GNU-Windows test executables linked; formatting, diff, exact
three-file scope, dependency, and HTTP/Git byte-identity gates passed. Three
independent corrected-diff terminal reviews returned `ACCEPT`.

Next evidence: Design only
`WP-5-m1-runtime-attempt-effect-sidecar-checkpoint-design`. Reconcile the
accepted command effect owner and exact-version activation closure with the
current runtime/materializer seams, then freeze bounded serial implementation
packets for the sidecar and its later event/demand producers. Do not edit Rust,
Cargo, fixtures, oracles, commands, server paths, source preparation, or
discovery behavior.

### Stage 5 attempt/effect-sidecar implementation checkpoint

Status: Accepted before Rust

The exact-version DICE activation API is already sufficient: rich callbacks
carry engine-owned node ID, version, Evaluated/Reused kind, evaluation data,
and ordered direct dependencies; parentless callbacks carry ordered roots; and
`activation_closure` returns the read-only dependency-first graph for one
transaction version. The Stage 9 retained-DICE boundary remains unchanged:
adopt this engine only behind the V2 runtime, with no Buck cell or label
surface and no DICE source edit.

Implement only `WP-5-m1-runtime-attempt-effect-sidecar` in exactly
`app/slug_events_v2/src/lib.rs`, `app/slug_core_v2/Cargo.toml`,
`app/slug_core_v2/src/runtime/events.rs`, and
`app/slug_core_v2/src/runtime/mod.rs`. Add a public dependency-bottom
zero-sized `CaptureEvaluationEvents` request marker; promote `starlark_map`
from the core dev dependencies and add direct `dupe` and `slug_events_v2`
dependencies. Replace the unused public core sink scaffold with a private
`CommandEffectOwner`, serial attempt trackers, typed sealing/selection errors,
and focused tests; make the module private. No `WorkspaceRuntime` field or
other file is allowed.

One owner spans one external command and multiple serial attempts, with exactly
one Open attempt at a time. Roots, version, and sealing are attempt-local;
event lineage is command-local. Installing an attempt places both its rich
tracker and `CaptureEvaluationEvents` in `UserComputationData`. Evaluated
`EventBatch` data replaces the node batch, including explicit empty; any other
Evaluated callback clears prior current-command data for that node; Reused
preserves the newest eligible evaluated batch. Sealing synchronizes once and
quarantines late callbacks. It copies ordered roots, releases every mutex, and
only then awaits `activation_closure`. Selection requires the exact sealed
version and roots, follows closure dependency-before-parent order, deduplicates
shared nodes, honors the terminal-version cutoff, and returns batches without
publishing them. Closure, stale-attempt, overlap, double-seal, root/version,
and allocator failures are typed and fail closed.

Real-DICE evidence must manually drive multiple transactions without adding a
retry loop. It proves an eventful child evaluated before a simulated Need and
reused by the reachable terminal attempt publishes once; a later evaluated
empty clears it; abandoned branches and post-seal callbacks publish nothing;
a fresh command owner does not replay a warm cached batch; roots retain ordinal
order and duplicates; dependencies publish before parent-local order; shared
nodes deduplicate; and exact-version/foreign/unavailable/dirty closure failures
select nothing. `SmallMap<DiceNodeId, ...>` stores only sparse command-local
event lineage; ordered vectors/shared slices retain roots, histories, and
selected batches.

The neutral marker is mandatory because always-on capture would suppress
today's direct evaluator output before a publisher exists. Marker absence
preserves current direct printing; marker presence selects capture-only
behavior in later producer packets. No producer, evaluator,
`store_evaluation_data` production call, runtime/command/daemon/CLI/server
integration, sink, publication, semantic value/equality, DICE key, retry,
generation, path/repository operation, source preparation, discovery, REAPI,
fixture, or oracle enters the sidecar packet.

Serial residuals are frozen separately:

1. A root-MODULE event-producer correction must conditionally capture only
   when the marker is present and attach explicit local batches outside
   semantic equality. It must stop and replan if the current separate-key
   include layout cannot preserve the accepted one-local-batch include rule.
   Nonroot registry/nonregistry evaluation and discovery remain deferred.
2. Loading `.bzl`/BUILD producers and configured-analysis producers follow as
   separate owner packets. Query adds no evaluator producer; it activates the
   loading graph. The duplicate legacy core BUILD evaluator remains unchanged
   until shared-driver activation can neutralize it atomically.
3. A separately corrected runtime-native demand producer must retain
   workspace-lifetime demand provenance keyed by DICE node ID and semantic
   repository scope while event lineage remains command-lifetime. Untouched
   cached closure nodes make a command-only demand set unsound. Local
   repository validation groups by reachable repository-source scope, never
   path prefix, because valid symlink escapes overlap Host paths. This packet
   must preserve fixed per-command generations, repository-first/path-second
   work, cumulative complete epochs, strict nonprogress, root pins, explicit
   accept/discard, and no lock across DICE/native I/O.
4. Only after those sidecars and producers pass may a new design packet own
   the shared build/query retry/publication driver, typed Need propagation,
   source-preparation/discovery activation, entrypoint convergence, legacy
   snapshot retirement, and terminal REAPI/publication order.

Independent live-DICE, runtime/materializer, and architecture audits found the
initial command/attempt wording too narrow; the correction made serial
cross-attempt lineage explicit. Three terminal rereviews returned `ACCEPT`.
No Rust, Cargo, test, fixture, oracle, command, or server file changed.

Next evidence: Implement only
`WP-5-m1-runtime-attempt-effect-sidecar` in the exact four files above. Stop on
any DICE edit, extra file, public core API, runtime field, producer, retry,
sink/publication, source-preparation/discovery activation, or direct output
change.

### Stage 5 runtime attempt/effect sidecar implementation

Status: Accepted

The exact four-file packet adds the dependency-bottom
`CaptureEvaluationEvents` marker and a private dormant core sidecar. One
command owner retains sparse event lineage across serial attempts; roots and
seals remain attempt-local. Rich Evaluated callbacks replace or explicitly
clear node batches, Reused preserves eligible lineage, and terminal selection
uses the exact DICE activation closure in dependency-first order without
holding a mutex across the computation.

Terminal review corrections made tracker installation an owner-locked,
single-use Open-attempt transition that leaves user data untouched on
occupied, duplicate, stale, or terminal failure. Selection consumes its sealed
token, the checked attempt-ID allocator fails before mutation, and a gated
real-DICE regression proves callbacks from a computation already in flight at
seal time are quarantined. The marker remains inert because no producer,
runtime field, retry, sink, publication, command/server path, or direct-output
change entered the packet.

Validation passed four event-crate tests, 67 core unit tests, 13 core
integration tests, and zero doctests. Both GNU-Windows core test executables
linked; formatting, diff, exact four-file scope, dependency, privacy, and
forbidden-activation gates passed. Three independent corrected-diff terminal
reviews returned `ACCEPT`.

Next evidence: Design only
`WP-5-m1-root-module-event-producer-design-correction`. Determine whether the
current separate-key root-MODULE include layout can preserve the accepted
one-local-batch include rule while capture is marker-conditional and outside
semantic equality. Do not edit Rust, Cargo, fixtures, oracles, runtime/command/
server paths, loading/analysis evaluators, nonroot evaluation, or discovery
behavior.

### Stage 5 root-MODULE event-producer design correction

Status: `REPLAN` before Rust

Bazel source inspected: pinned Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`, especially
`ModuleFileFunction`, `ModuleThreadContext`, and `ModuleFileGlobals`.

Bazel compiles the complete root include horizon before execution, then runs
the root and every included compiled program inline on one Starlark thread and
one print handler. Repeated `include()` calls execute repeatedly at their
textual sites. A missing or unparseable later include therefore fails before
any root directive or print executes.

The live Slug layout cannot own that event stream. `ModuleFileEvaluationKey`
executes the root immediately in its own evaluator; root `include()` only
records a label; and `RootModuleFilesKey` later evaluates deduplicated includes
as separate breadth-first DICE nodes. For root `before`, include `part`, and
root `after`, per-file batches can produce only `before, after, part`, not
Bazel's single `before, part, after` batch. Activation-closure ordering cannot
interleave batches at a dynamic call site, repeated includes are already
deduplicated, and marker-absent root output can escape before a later include
preflight failure. Adding events to semantic values or reconstructing them
from source positions would violate the accepted equality boundary and remain
wrong for nested, conditional, repeated, and failing execution.

The first producer must therefore wait for a composed root/include evaluator:
preflight and prepare the complete closure, execute includes inline with
per-file binding isolation, preserve repeated calls, and attach one explicit
`EventBatch` only to the composite execution node. Marker absence retains
direct print; marker presence selects one capture-only handler; semantic
values and equality remain event-free. No current per-file evaluation node
may become an event producer.

Three independent corrected terminal audits returned `REPLAN`. No Rust,
Cargo, fixture, expected-output, oracle, command, server, runtime, loading,
analysis, nonroot, or discovery file changed.

Next evidence: Design only
`WP-5-m1-root-module-include-composition-event-oracle-design`. Strengthen the
existing retained-daemon `module-include-change-invalidation` fixture with
exact nested/repeated inline order, warm nonreplay, print-only edit
whole-closure reexecution, missing/parse preflight suppression,
runtime-failure prefix events, and complete recovery before designing the
composed evaluator or event producer.

### Stage 5 root-MODULE include-composition event oracle design

Status: Accepted before fixture edits

Strengthen only the existing `module-include-change-invalidation` fixture and
set `daemon = true`. Root prints `ROOT_BEFORE`, includes
`deps.MODULE.bazel`, then prints `ROOT_AFTER`. The dependency fragment prints
`DEPS_BEFORE`, includes one new print-only `nested.MODULE.bazel`, prints
`DEPS_BETWEEN`, includes the same nested label again, prints `DEPS_AFTER`, and
then retains the fixture's existing dependency and local-path override. This
freezes one exact logical-module stream:

```text
ROOT_BEFORE, DEPS_BEFORE, NESTED_V1, DEPS_BETWEEN, NESTED_V1, DEPS_AFTER, ROOT_AFTER
```

Run exactly eight cumulative rows:

1. cold dep-one success with the exact V1 stream;
2. the existing included semantic version/path edit to dep two with the exact
   V1 stream and changed manifest;
3. unchanged warm dep-two success with zero event sentinels;
4. print-only nested V1-to-V2 edit with the complete V2 stream and byte-equal
   dep-two manifest;
5. direct dependency-fragment deletion with the existing missing-file shape
   and zero sentinels;
6. dependency-fragment recreation plus parser-invalid nested content, proving
   nested compile failure and zero root/dependency sentinels;
7. valid nested print, runtime failure, and unreachable after-print, yielding
   exactly `ROOT_BEFORE, DEPS_BEFORE, NESTED_RUNTIME_PREFIX`;
8. nested V2 recovery with the complete V2 stream and dep-two manifest.

Every success or runtime-prefix row uses one `(?s)\A...\Z` tempered-dot regex
whose gaps reject the complete V1/V2/runtime/after sentinel union. It therefore
proves exact order and cardinality, including two nested executions. Warm,
missing, and parse rows use an anchored negative lookahead rejecting the
entire event prefix. Stable diagnostic substrings and existing nonempty
manifests remain separate assertions; semantic comparison is retained because
exact Bazel progress text is not stable. The generated records must also prove
rows 2, 3, 4, and 8 have identical dep-two manifests while row 1 is dep one.

The exact implementation allowlist is:

- `tests/v2_oracle/fixtures/module-include-change-invalidation/fixture.toml`
- `tests/v2_oracle/fixtures/module-include-change-invalidation/expected/oracle.json`
- `tests/v2_oracle/fixtures/module-include-change-invalidation/workspace/MODULE.bazel`
- `tests/v2_oracle/fixtures/module-include-change-invalidation/workspace/deps.MODULE.bazel`
- new `tests/v2_oracle/fixtures/module-include-change-invalidation/workspace/nested.MODULE.bazel`

Add `CompiledModuleFile`, `ModuleThreadContext`, and the exact include
preflight/execution methods to provenance. No harness, BUILD, module payload,
Rust, Cargo, runtime, command/server, registry, other fixture, nonroot, or
discovery edit is allowed. This adds one regular file and remains far below
the fixture-growth review threshold.

Independent pinned-source, maintainability, and harness audits converged on
the compact eight-row/five-file design. The harness needs no extension; one
review correction made root-before/root-after sentinels explicit so a
separate root evaluator cannot pass. Three corrected terminal reviews returned
`ACCEPT`; no file changed.

Next evidence: Implement only
`WP-5-m1-root-module-include-composition-event-oracle` under the exact
five-file allowlist above. Stop on old semantic-evidence loss, unstable
sentinel order/count, warm replay, preflight event leakage, wrong runtime
prefix, manifest drift, scope growth, or any harness/production edit.

### Stage 5 root-MODULE include-composition event oracle

Status: Accepted

Bazel oracle: pinned Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`.

The strengthened retained-daemon fixture contains exactly eight cumulative
rows. Cold and semantic-edit rows emit the complete V1 root/dependency/nested
stream with the nested file executed twice; the semantic edit changes the
manifest from dep one to dep two. The unchanged warm row emits no event
sentinel. A print-only nested V1-to-V2 edit reruns the complete logical-module
stream while retaining the byte-identical dep-two manifest. Direct missing
input and nested parser failure emit no event sentinel. Runtime failure emits
exactly `ROOT_BEFORE, DEPS_BEFORE, NESTED_RUNTIME_PREFIX`; recovery emits the
complete V2 stream and restores the same dep-two manifest.

Anchored tempered-dot regexes enforce exact event order and cardinality rather
than mere containment. A terminal assertion correction also makes the parse
row reject the prior missing-file diagnostic, the runtime row reject missing
and parse diagnostics, and recovery reject every prior failure class. The
generated records show dep-one digest prefix `2c8b08` and shared dep-two prefix
`27dd8e` for semantic, warm, print-only, and recovery rows.

Generation and three independent Bazel replay runs passed all eight rows.
Fixture listing, exact five-file scope, expected-record inventory, cumulative
mutation uniqueness, source-anchor existence, JSON/schema, manifest
relations, and diff checks passed. Two independent terminal fixture reviews
returned `ACCEPT`. Growth is one regular file and 224 newline-counted lines,
remaining far below the fixture-growth review threshold. No harness, BUILD,
module payload, registry, Rust, Cargo, runtime, command/server, other fixture,
nonroot, or discovery file changed.

Next evidence: Design only
`WP-5-m1-root-module-composed-evaluator-event-design`. Freeze the bzlmod-local
preflight/preparation and single inline evaluator, per-file binding isolation,
repeated include execution, composite semantic ownership, marker-conditional
single event batch outside equality, DICE key transition, and retained-engine
evidence before any Rust or Cargo edit.

### Stage 5 root-MODULE composed evaluator/event design

Status: Accepted

Bazel source inspected: pinned Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`, especially
`ModuleFileFunction`, `ModuleThreadContext`, `ModuleFileGlobals`, and
`ModuleFileValue`.

The current per-file evaluator is replaced by one private
`RootModuleEvaluationKey` below the visible-lockfile join:

```text
RootModuleCommandPolicyKey -- ignore-dev projection --\
WorkspaceFileKey(root and discovered includes) --------+--> RootModuleEvaluationKey
                                                             |
VisibleLockfileKey ------------------------------------------+--> RootModuleFilesKey
                                                                  |
                                                                  +--> RootModuleGraphKey
```

This split is mandatory. `RootModuleFilesKey` also owns visible-lockfile
composition, so making it eventful would rerun or replay root Starlark prints
on a lockfile-only change. The private key depends on an opaque DICE projection
of only `ignore_dev_dependency`; yanked-policy and environment changes remain
outside root evaluation, while an ignore-dev flip reruns the logical module as
it does in Bazel. Validated dev dependencies and root overrides are omitted at
directive execution when the projection is true. The public graph still
retains the complete command/environment/mode values.

The private key reads and parser-inspects the root, discovers unique raw
include labels breadth-first, validates and reads the complete reachable
closure, and retains only exact shared source text, paths, labels, and compact
inspection data across DICE awaits. After the last await, one synchronous
helper reparses and prepares every program before executing any program. Root
and every distinct raw include label receive separate Starlark `Module`
binding stores. One evaluator installs all prepared programs; `include()`
saves the current file, dispatches the mapped opaque program index inline,
restores the prior file before propagating either success or failure, and
executes repeated calls to the same raw label repeatedly through the same
program. No evaluator, module, prepared program, `Rc`, or `RefCell` crosses a
DICE await, and no DICE call or lock occurs inside a Starlark global.

One shared directive context owns the logical module header, ordering state,
dependencies in execution order, and override uniqueness/materialization.
The rejected public per-file execution model is removed without a shim:
`ModuleFileEvaluation`, `ModuleFileEvaluationKey`, `evaluate_module_file`,
their exports, and `RootModuleFiles`/`RootModuleGraph` `root` and `includes`
fields are retired. The replacement public `EvaluatedRootModule` contains the
aggregate header and dependency slice. `RootModuleFiles` and
`RootModuleGraph` expose that value as `module`, plus a canonical sorted unique
set of repo-relative `module_file_paths` and their existing lockfile,
override, policy, and mapping fields. `root_mapping` consumes the aggregate
dependencies. This matches Bazel's one `InterimModule` plus override and
module-file-path set rather than preserving prototype compatibility.

Implementation is split into two independently reviewable packets.

1. `WP-5-m1-root-module-composed-evaluator` changes exactly:

   - `app/slug_bzlmod_v2/src/module_eval.rs`
   - `app/slug_bzlmod_v2/src/lib.rs`
   - `app/slug_bzlmod_v2/tests/root_module_dice.rs`
   - `app/slug_core_v2/tests/runtime.rs`

   It lands the private key, opaque ignore-dev projection, full
   preflight/preparation, inline dispatcher, aggregate schema, old-key
   retirement, and downstream test migration. Focused evidence must prove
   root/include/nested dependency order, distinct-file binding isolation,
   repeated-call semantic effects, canonical module-file paths, runtime stack
   attribution, missing/edit/delete/recreate and semantic A-to-B-to-A
   transitions, ignore-dev reevaluation, and private-evaluator reuse under
   yanked-policy, environment, and lockfile-only changes. The nonroot
   inspector/evaluator/dispatcher region remains byte-identical.

2. `WP-5-m1-root-module-composite-event-producer` then changes exactly:

   - `Cargo.lock`
   - `app/slug_bzlmod_v2/Cargo.toml`
   - `app/slug_bzlmod_v2/src/module_eval.rs`
   - `app/slug_bzlmod_v2/tests/root_module_dice.rs`

   It adds only the existing workspace `slug_events_v2` dependency and makes
   the private evaluation key the sole root-MODULE event producer. Marker
   absence leaves the evaluator's default direct-print handler selected and
   stores no evaluation data. Marker presence selects one capture-only handler
   and stores exactly one local `EventBatch`, including explicit empty batches
   for missing/read/parse/prepare/no-print outcomes and the exact executed
   prefix on runtime failure. Events, the marker, sources, prepared programs,
   and activation history remain outside semantic values and equality.
   `RootModuleFilesKey`, file leaves, and graph keys never store event data.

The event packet's retained-engine matrix must prove one cold
root/dependency/nested batch with repeated nested execution; warm and
fresh-owner nonreplay; print-only V1-to-V2-to-V1 evaluation with equal semantic
graphs; semantic edits and recovery; explicit empty missing/parse/prepare
batches; exact runtime prefix and full recovery; lockfile, yanked-policy, and
environment reuse; ignore-dev and source reevaluation; marker-absent
data absence; and exactly one event-bearing activation node. The accepted
command-effect owner already proves exact-version closure selection and retry
lineage, so neither packet edits runtime publication.

Stop and replan on execution before complete reachable preparation,
deduplicated call sites, shared bindings for distinct raw labels, fresh
programs for repeated identical labels, an event or marker in semantic
equality, event data on any node except the private evaluator, lockfile-only
Starlark replay, evaluator-owned state across an await, a new preparation key,
manual locking, a root-specific GC seam, or any edit to DICE, runtime,
command/server, registry/source preparation, loading/analysis, nonroot
evaluation, discovery, fixtures, oracle output, or the harness. Root include
package-policy and cycle parity remain named later gaps; this design does not
invent them.

Three independent terminal reviews returned `ACCEPT`. No Rust, Cargo, fixture,
oracle, runtime, nonroot, or other production file changed. No V1 code or
ownership is extracted; the existing V2 prepared-program dispatcher is only
the already accepted implementation mechanism.

Next evidence: Implement only
`WP-5-m1-root-module-composed-evaluator` in the exact first-packet allowlist.
Stop on any frozen invariant or scope violation before event activation.

### Stage 5 root-MODULE composed evaluator

Status: Accepted

The four-file packet replaced the public per-file root evaluator with private
`RootModuleEvaluationKey`, below the visible-lockfile join and dependent on an
opaque boolean projection of `ignore_dev_dependency`. It discovers the unique
raw-label closure through exact workspace-file leaves, finishes every await,
then parses and prepares the complete closure before executing root and
included programs through one inline evaluator. Distinct labels have stable
per-file binding modules; repeated textual calls reuse and reexecute the
mapped program; include errors restore the prior file state before
propagation.

The public result is now one source-free `EvaluatedRootModule` plus sorted
unique repo-relative `module_file_paths`. The old
`ModuleFileEvaluation`/`ModuleFileEvaluationKey`/`evaluate_module_file`
surface and `root`/`includes` partition were removed without a shim.
Validated ignored dev dependencies and all validated root overrides are
omitted during directive execution, and repository mapping consumes the
already-filtered aggregate dependencies.

Evidence covers full preflight precedence, inline dependency order,
post-nested binding isolation, repeated-call effects, ordered
root-to-child-to-nested runtime frames, canonical paths, missing/read/edit/
delete/recreate and full semantic A-to-B-to-A recovery, ignore-dev
reevaluation, and private-evaluator reuse for yanked-policy,
environment-only, and lockfile-only changes. Both independent corrected-diff
reviews returned `ACCEPT`.

Validation passed:

- `cargo test -p slug_bzlmod_v2` — 228 tests.
- `cargo test -p slug_core_v2` — 67 unit and 13 integration tests.
- `cargo check -p slug_bzlmod_v2 --target x86_64-pc-windows-gnu`.
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh`.

Next evidence: Implement only
`WP-5-m1-root-module-composite-event-producer` in its exact four-file
allowlist. Preserve the accepted evaluator/schema semantics and activate only
the private-key marker-conditional event sidecar.

### Stage 5 root-MODULE composite event producer

Status: Accepted

The private `RootModuleEvaluationKey` is now the sole root-MODULE evaluation
event producer. It reads the request-local `CaptureEvaluationEvents` marker as
an untracked boolean, funnels every semantic outcome through one completion
path, and stores exactly one local `EventBatch` when capture is selected.
Missing/read/parse/prepare/no-print outcomes store explicit empty batches;
runtime failure stores the exact executed prefix. Marker absence stores
nothing and leaves the evaluator's default direct-print handler selected.

The capture-only print handler is created only after the last DICE await and
is shared by the one root/include evaluator. It preserves raw print text and
inline nested/repeated order. No event, marker, source, handler, or activation
state enters the private key identity, semantic values, equality, file leaves,
visible-lockfile join, or graph key. The public root globals enable `print`
without changing the byte-identical nonroot evaluator region.

Retained-DICE evidence proves the exact seven-event inline stream, warm and
fresh-owner nonreplay, print-only V1-to-V2-to-V1 evaluation with equal graphs,
marker absence and untracked marker addition, explicit empty direct-include
missing/read and nested parse/prepare failures, exact runtime prefix and
recovery, yanked/environment/visible-lockfile reuse, ignore-dev
reevaluation/recovery, semantic source A-to-B-to-A recovery, and exactly one
private event-bearing activation node. Both independent implementation
reviews returned `ACCEPT`.

Validation passed:

- `cargo test -p slug_bzlmod_v2` — 231 tests.
- `cargo test -p slug_core_v2 runtime::events` — 4 focused sidecar tests.
- `cargo check -p slug_bzlmod_v2 --target x86_64-pc-windows-gnu`.
- `cargo fmt --all -- --check`, `git diff --check`, and
  `scripts/v2_archive_status.sh`.

The root `Cargo.lock` is intentionally ignored and untracked; its generated
local `slug_bzlmod_v2` entry already contains `slug_events_v2`, so the accepted
allowlist required no tracked lockfile delta.

Next evidence: Design only
`WP-5-m1-runtime-native-demand-producer-design-correction`. Reconcile
workspace-lifetime node-keyed demand provenance with the accepted retained
materializer and exact-version activation closure before any Rust edit.

### Stage 5 corrected runtime-native demand-producer design

Status: Accepted

This correction separates three kinds of state that the earlier runtime retry
design conflated:

- `WorkspaceRuntime` owns the retained `RepositoryMaterializer`, a
  workspace-lifetime sparse DICE-node provenance catalogue, and the last
  accepted complete injection/scope snapshot.
- One external command owns one materializer session, fixed workspace,
  registry, and repository-materialization generations, one command effect
  owner, one exclusive workspace command-lease token, and cumulative complete
  repository/path worksets.
- One serial attempt owns one fresh updater/transaction, returned root values,
  ordered root activations, and either a retry or terminal seal.

The provenance catalogue is an append-only
`SmallMap<DiceNodeId, DemandNodeMetadata>` for the lifetime of the one retained
DICE engine. It is never pruned in the absence of a DICE eviction notification.
Entries learned on speculative, abandoned, reused, or late activations are
harmless: they do not schedule work by themselves, and only membership in the
current exact-version terminal closure grants selection authority. A separate
accepted snapshot is replaced only after explicit terminal acceptance. Retry,
suppression, cancellation, closure failure, native failure, and discard leave
that accepted snapshot unchanged.

#### Key-static provenance and the sole tracker

Demand provenance uses the existing `Key::provide`/`DynKey::request_value`
channel, not transient `Need` values and not `store_evaluation_data`:

- `PathObservationKey` provides its exact `PathObservationDemand`.
- private `RepositoryMaterializationResultKey` provides its exact
  `Arc<RepositoryMaterializationRequest>`.
- `RepositorySourceFileKey` provides a public semantic
  `RepositorySourceScope` containing normalized workspace and module identity,
  deliberately excluding `repo_relative_path`.

The DICE node already identifies the exact source-file key; equal scope
descriptors deliberately union validation from multiple source files in the
same repository. Path namespace, immutable instance, full repository request,
and source-key identity are immutable key identity, so one node needs no
versioned demand history. Evaluated and Reused rich callbacks both insert or
confirm the descriptor. Absence supplies nothing. A different descriptor for
an existing node latches a typed internal catalogue failure, and every later
selection fails closed. Root-MODULE `EventBatch` remains the only current
evaluation-data payload because event contents, unlike key-static demand
metadata, can change across versions of the same node.

Every retained-runtime transaction that can warm these keys must install the
tracker from its first computation. One `RuntimeActivationTracker` is the only
object ever placed in `UserComputationData.activation_tracker`:

```text
RuntimeActivationTracker
├── Arc<WorkspaceDemandOwner>                 always present
└── Option<Arc<AttemptEffectTracker>>          eventful attempts only
```

Current query/evaluate updaters use a centralized
`WorkspaceRuntime::user_computation_data(None)` or equivalent factory. It
installs the existing loading cycle detector and provenance-only tracker, but
does not set `CaptureEvaluationEvents`; direct root `print` behavior therefore
stays unchanged. The later shared driver passes an attempt delegate, and only
that variant sets the capture marker. `AttemptEffectTracker` becomes the
command-local event/root delegate rather than a second `ActivationTracker`.
Demand recording always runs before optional event forwarding and remains
useful after a command attempt seals; event recording remains attempt-token
gated and quarantines late callbacks. A private owner/installer binds the
demand owner to the retained `Arc<Dice>` so it cannot be reused with a
replacement engine.

#### One exact closure, semantic scope, and untouched cached nodes

Terminal sealing copies ordered roots and one version under the attempt mutex,
then releases every mutex before the single `activation_closure` call. The
transaction and returned root values remain alive. Root order and exact
version are checked, the catalogue is copied under its own short lock, and the
same closure object is passed to both event and demand selectors. The closure's
dependency-first nodes and direct dependency IDs include valid untouched
cached intermediates even when they emitted no callback in the current
command; their workspace-lifetime catalogue entries recover exact demand
identity. Abandoned branches may remain catalogued but are absent from the
terminal closure and cannot be selected.

Selection first collects every reachable exact repository request, rejecting
different full requests with the same `RepositoryMaterializationRequestId`.
It then walks root-to-dependency reachability with an optional semantic scope:

- entering a repository-source anchor assigns its semantic scope;
- entering a different nested scope is a boundary and replaces, rather than
  inherits, the outer scope;
- equal scopes union;
- a path reached with no scope is unscoped;
- a path reached through one or more scopes belongs to every such scope;
- the same node reached both scoped and unscoped is retained in both roles.

Traversal deduplicates `(node, scope)` states rather than only node IDs. Every
path-bearing source anchor must have exactly one full request descendant;
zero or conflicting requests are typed internal failures. Globally observing
one exact path may be deduplicated, but its result is copied into every owning
repository validation and into the unscoped snapshot when both apply.

No path prefix, canonical path, longest-prefix rule, Host namespace, or
materialization namespace assigns semantic ownership. A Local repository may
follow an ancestor symlink outside its logical root, two Local repositories
may reach the same Host path, and an unscoped Host consumer may share that
node. The repository-source dependency edge remains the only sound validation
scope.

#### Command work, generations, and strict progress

One external command allocates exactly once its workspace revision, registry
generation, repository-materialization generation, effect owner, and
repository-session token. Every attempt reuses that bundle and creates a fresh
updater, committed transaction, and semantic root computation.

An explicit workspace command lease serializes the whole accepted-state
transition, not just access to `RepositoryMaterializer`. Acquiring it changes a
short locked phase from Idle to Open(token); no mutex guard remains held while
the command runs. Every materializer `begin` is reachable only through this
owner. The phase remains Open across preflight, every retry, selected-injection
commit, transaction/root-value drop, materializer accept or discard, and
accepted-snapshot replacement or restoration. It returns to Idle only after
the entire terminal transition succeeds. Overlap returns typed Busy without
allocating generations or opening a materializer session. A failed restoration
leaves the lease fail-closed and retains the active session/root pins rather
than exposing a mixed materializer/snapshot state to another command.

Command preflight begins the sole session, reobserves accepted
repository-scoped validation and accepted unscoped paths outside DICE,
and constructs fresh complete initial repository/path epochs from only clean
reusable state. Native observation may deduplicate the global exact-demand
union, but preflight returns the results needed for every retained semantic
scope. Local success remains allocation-free and rootless: request construction
owns only lexical workspace-relative shape, while exact selected Host demands
retain missing, wrong-kind, symlink-escape, boundaryless, and source-error
ordering. Accepted and provisional `Arc<TempDir>` owners remain pinned while
their instances can appear in an epoch.

Retry scheduling consumes the typed `SourcePreparationNeeds` returned by the
attempt; an invalid Need graph is never treated as a terminal closure.
Repository work has strict priority:

1. If any repository Need is present, materialize all newly demanded exact
   requests in deterministic repository order outside DICE and inject the
   entire cumulative result epoch. A repeated/conflicting repository Need
   cannot be masked by a path Need.
2. Only an attempt with no repository Need may observe newly demanded exact
   paths, after all required immutable roots exist, and inject the entire
   cumulative path epoch.
3. Each retry must add one previously unknown exact full request result or one
   previously unknown exact path observation. Equal/subset work yields typed
   repository- or path-`InternalNonProgress`; the same request ID with a
   different full request is a conflict. There is no retry count or cap.

Every injected epoch is sorted, duplicate-free, cumulative, and a complete
replacement, never a delta. Current-generation transport/materialization
failures become cumulative results and terminalize only that command; the next
command receives a new generation.

#### Terminal transitions and three pin classes

Accepted terminal success and an eligible final Starlark failure use this
order:

1. read and validate the one exact closure, then select both events and
   demands from it;
2. build and commit selected complete repository/path injections and
   repository validations while all active/provisional roots remain pinned;
3. drop the terminal transaction and every returned root value that could
   retain an unselected instance;
4. call materializer `accept` for only the selected exact requests/instances;
5. atomically replace the accepted injection/scope snapshot;
6. publish the already selected events;
7. close the workspace command lease.

Preflight, native-I/O, path, internal, closure, cancellation, or ineligible
terminal failure restores the prior accepted complete injections, drops all
attempt transactions/root values, explicitly discards the session, preserves
the prior accepted snapshot, closes the workspace command lease, and publishes
nothing. If restoration fails, fail closed with the lease non-Idle while
retaining the session/root pins rather than releasing a root that a DICE value
may still address.

The three distinct pins are:

- the terminal `DiceTransaction` and root values pin exact closure authority;
- copied `Arc<TempDir>` owners pin immutable roots across unlocked native I/O
  and the subsequent stale-token check;
- the active materializer session pins every provisional root across retries
  until the ordered accept/discard transition.

No mutex spans a DICE compute/commit, `activation_closure`, await, native
observation, Local/archive/Git materialization, or `TempDir` destruction.

#### Serial implementation packets

1. `WP-5-m1-demand-key-metadata`

   Edit only:

   - `app/slug_workspace_v2/src/path_observation.rs`
   - `app/slug_bzlmod_v2/src/source_preparation.rs`

   Add the three `Key::provide` descriptors and direct `DynKey` producer tests
   first. Do not change compute/value/equality behavior, evaluation data,
   runtime, Cargo, commands, fixtures, or oracles.

2. `WP-5-m1-workspace-demand-provenance-bootstrap`

   Edit only:

   - new `app/slug_core_v2/src/runtime/demands.rs`
   - `app/slug_core_v2/src/runtime/mod.rs`
   - `app/slug_core_v2/src/runtime/events.rs`
   - `app/slug_core_v2/src/runtime/dice.rs`

   Add the sparse owner, composite tracker, passive/eventful installation,
   centralized production user-data factory, exact-closure semantic selector,
   and one shared closure-read seam. Convert every production
   `WorkspaceRuntime` updater capable of bzlmod computation to the passive
   factory. Do not add native I/O, epochs, retry, accept/discard, event capture
   on current commands, publication, command/server APIs, or a DICE-core edit.

3. `WP-5-m1-retained-native-materialization-bridge`

   Edit only:

   - `app/slug_core_v2/src/runtime/repository_io.rs`

   Add the retained-session `materialize_native` entrypoint with exact stable
   Spec versus current-generation Transport/Materialization classification;
   batch accepted validation into one deduplicated native observation pass;
   return the complete preflight path epoch plus clean reusable repository
   results; retain allocation-free/rootless Local success; and expose only the
   private session operations required by the next packet. Preserve the
   existing root snapshots and post-I/O token checks.
   Stop and replan rather than collapse failures to Transport if exact HTTP or
   Git staging requires the deferred PAX/GNU/link/special parity surface.

4. `WP-5-m1-runtime-native-demand-session`

   Edit only:

   - `app/slug_core_v2/src/runtime/demands.rs`
   - `app/slug_core_v2/src/runtime/repository_io.rs`
   - `app/slug_core_v2/src/runtime/dice.rs`

   Add a dormant manually driven command/session owner, preflight reporting,
   the full-transition workspace command lease, cumulative complete
   worksets/epochs, fixed generation bundle, repository-first/path-second
   progression, typed strict nonprogress, accepted-snapshot
   restoration/replacement, and ordered
   accept/discard evidence, and a private real-DICE handshake that manually
   drives Need-to-repository-to-path-to-Complete attempts on one retained
   runtime. Do not activate the shared semantic retry or publication loop in
   build, query, daemon, CLI, server, source preparation, or discovery.

Only after all four pass may residual sidecar item 4 own the shared
build/query retry/publication driver, typed Need propagation through the
entrypoints, source-preparation/discovery activation, legacy snapshot
retirement, and terminal REAPI/publication order.

#### Required retained-engine evidence and stop gates

The four packets together must prove passive marker-absent direct printing;
one tracker in passive and eventful modes; preoccupied-slot failure without
marker/attempt mutation; warm untouched-descendant recovery; reused and
abandoned-command provenance; unreachable-branch exclusion; exact
foreign/replacement-engine and unavailable/dirty/not-verified closure failure;
same Host path under two repository scopes; Local symlink escape overlapping
another repository and an unscoped consumer; exact request/spec and immutable
instance A-to-B replacement; repository-before-path attempts; fixed
per-command/changing-next-command generations; cumulative complete epochs;
typed equal-Need nonprogress; clean/edit/delete/recreate and Local-symlink
retarget on one runtime; selected/unselected root lifetimes; ordinary Need
retry dropping only its attempt transaction/effects while retaining cumulative
command state and provisional roots; terminal cancellation, closure/native/
stale-token/acceptance failure restoring the prior accepted injections before
discard; whole-transition command-lease Busy and fail-closed behavior; and
event/demand selection from the identical closure.

Stop and replan on a DICE-core/API or new dependency edit; demand use of
evaluation data; two activation trackers; any production retained-runtime
updater bypassing the centralized tracker factory; command-local-only or
pruned provenance; `HashMap`/`HashSet` hot-path ownership instead of Buck2
compact collections/shared slices; path-derived repository ownership; a
path-bearing scope without one exact request; overlapping sessions or any
materializer `begin` outside the whole-transition workspace command lease; any
lock across DICE/native I/O; delta epochs; retry caps; per-attempt generations;
release of provisional roots before injection restoration and transaction
drop; eager Local-root existence/kind/canonical-containment validation without
separate oracle/design evidence; or shared command/server/query/build/
discovery/oracle activation before the dormant session evidence passes.

Independent DICE/source, architecture/lifecycle, and live native-feasibility
reviews corrected the draft on passive universal tracking, one whole-transition
workspace command lease, native failure-classification isolation, and the
superseding rootless Local contract. All three latest terminal rereviews
returned `ACCEPT`. This design packet changed no Rust, Cargo, fixture, oracle,
command, server, source-preparation, discovery, or DICE-core file.

Next evidence: Implement only `WP-5-m1-demand-key-metadata` in
`app/slug_workspace_v2/src/path_observation.rs` and
`app/slug_bzlmod_v2/src/source_preparation.rs`. Add direct `DynKey` producer
regressions first, then the three `Key::provide` descriptors. Stop on any
compute/value/equality, evaluation-data, runtime, Cargo, command, fixture,
oracle, or DICE-core change.

### Stage 5 demand-key metadata

Status: Accepted

The exact two-file packet adds three immutable `Key::provide` descriptors:
`PathObservationKey` supplies its exact path demand, the private materialization
result key supplies its exact shared full request, and
`RepositorySourceFileKey` supplies a normalized workspace/module scope that
deliberately excludes the source-relative path. Invalid relative workspaces
supply no scope. All producers use lazy `provide_value_with`, so unrelated
tracker probes neither clone nor normalize payloads.

Direct erased-key regressions prove exact materialization-instance path
identity, shared request identity, lexical workspace normalization, equal scope
across different source files, distinct workspace/module scope, and invalid
relative omission. No compute, value, equality, validity, evaluation-data, or
DICE-core behavior changed.

Validation passed `slug_workspace_v2` 30 tests, `slug_bzlmod_v2` 233 tests,
focused producer reruns, GNU-Windows `slug_bzlmod_v2` compilation, formatting,
diff/allowlist/forbidden-boundary checks, and archive guards. Independent
DICE/source and architecture/hot-path reviews corrected eager payload
construction to lazy providers; both latest-diff rereviews returned `ACCEPT`.

Next evidence: Implement only
`WP-5-m1-workspace-demand-provenance-bootstrap` in new
`app/slug_core_v2/src/runtime/demands.rs` plus
`app/slug_core_v2/src/runtime/mod.rs`,
`app/slug_core_v2/src/runtime/events.rs`, and
`app/slug_core_v2/src/runtime/dice.rs`. Add the sparse workspace owner,
single passive/eventful tracker, centralized production user-data factory,
one exact-closure demand selector/shared closure-read seam, and retained-engine
evidence. Stop on native I/O, epochs, retry, accept/discard, current-command
event capture, publication, public command/server APIs, Cargo, DICE-core, or
extra-file edits.

### Stage 5 workspace demand provenance bootstrap

Status: Accepted

The retained runtime now owns one canonical-workspace, weak-engine-bound sparse
catalogue of immutable DICE-node demand descriptors. One composite activation
tracker records evaluated, reused, abandoned, and late path/request/source
metadata before optionally forwarding event effects. Event reservation is
state-only; the capture marker and composite tracker are installed together
only after every fallible occupied-slot, engine, and attempt check. All five
retained-runtime updater sites use the centralized factory, while passive
query/build paths retain the loading cycle detector and marker-absent direct
printing.

Terminal sealing weak-binds the exact installed demand owner. Selection accepts
no replacement owner, reads one exact activation closure, validates event
version/ordered roots/terminal state, and selects both sidecars from that same
closure. The compact iterative selector copies only demand-bearing closure
metadata, validates every path-bearing source-file anchor against exactly one
boundary-local full request before merging equal semantic scopes, rejects
request identity/workspace conflicts, and returns deterministic shared slices.

Retained-engine evidence proves late post-retry catalogue enrichment, a cached
metadata-free parent whose untouched demand-bearing child emits no later
provider callback, unreachable abandoned-sibling exclusion, zero stale-event
replay, exact installed-owner selection, atomic foreign/expired-engine failure,
distinct nested equal-scope anchor validation, shared A/B/unscoped paths, and
typed request/scope conflicts. Validation passed seven focused demand tests,
four event tests, the runtime-factory regression, all 75 core unit and 13
integration tests, GNU-Windows compilation, formatting, diff/allowlist and
forbidden-boundary checks, and archive guards. Independent DICE/source and
architecture/hot-path reviews corrected partial marker installation, dense
catalogue reservation, retained-cache evidence, and caller-substitutable owner
selection; both latest-diff rereviews returned `ACCEPT`.

Next evidence: Implement only
`WP-5-m1-retained-native-materialization-bridge` in
`app/slug_core_v2/src/runtime/repository_io.rs`. Add the retained-session
`materialize_native` entrypoint with exact stable Spec versus
current-generation Transport/Materialization classification, one deduplicated
native validation pass, complete preflight path observations plus clean
reusable repository results, and only the private session operations needed by
the following dormant runtime-session packet. Preserve rootless Local behavior
and existing root/token ownership. Stop and replan if exact HTTP or Git staging
requires the deferred PAX/GNU/link/special parity surface.
