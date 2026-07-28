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

### Stage 5 retained native materialization bridge

Status: Accepted

The retained materializer now performs one sorted, deduplicated native
preflight over accepted repository validation and caller demands, returning
the complete path epoch plus only clean exact reusable repository results.
Full requests bind validation and acceptance; inherited clean entries can be
replaced by the newly demanded same-ID request while competing identities
within the current command fail with a typed conflict. Root snapshots remain
pinned across unlocked observation and exact post-I/O token checks.

The runtime-private native entrypoint preserves lexical, allocation-free Local
success and stages HTTP/Git outcomes as stable Spec or current-generation
Transport/Materialization results. HTTP retains the accepted capture-before
saved-checksum precedence. Git retains its byte-identical command/stdout,
temporary archive, and external-tar path, including real Git archive success
and deterministic extraction-failure classification.

Validation passed 23 focused repository tests, all 79 core unit and 13
integration tests, GNU-Windows compilation, formatting, diff/utility/scope
checks, and archive guards. Independent source/contract and
architecture/hot-path reviews corrected inherited A-to-B replacement,
malformed-checksum dual-failure precedence, and Git materialization-stage
evidence; both latest-diff rereviews returned `ACCEPT`.

### Stage 5 runtime native demand session

Status: Accepted

The retained runtime now owns a dormant, tokenized native-demand command
session with one fixed workspace/registry/repository generation bundle, one
command effect owner, cumulative complete repository/path epochs, deterministic
repository-first progression, and restore-before-discard failure transitions.
Opaque owner-branded attempt, seal, and terminal-selection wrappers preserve one
exact event/demand closure and return selected events privately only after
ordered materializer acceptance and accepted-snapshot replacement.

The real-DICE handshake proves a combined two-repository-plus-path Need,
repository priority, exact cumulative epochs, fixed injected generations across
attempts and changed generations for the next command, typed repository/path
nonprogress, inherited request replacement and competing-request conflict,
post-injection acceptance restoration, fail-closed restoration/materializer
ownership, closure failure cleanup, and foreign-sidecar rejection. Validation
passed six focused session tests, all 85 core unit and 13 integration tests,
zero doctests, the 23 retained-repository tests, GNU-Windows compilation,
formatting, diff, utility, and scope gates. Independent lifecycle and
architecture rereviews both returned `ACCEPT`.

Next evidence: Design only a bounded residual-sidecar checkpoint for the shared
build/query retry/publication driver. Freeze typed Need propagation through
entrypoints, source-preparation/discovery activation, legacy snapshot
retirement, cancellation ownership, and terminal REAPI/event publication order
before activating any production command surface.

### Stage 5 shared retry/publication driver checkpoint

Status: Accepted

The live checkout cannot activate the accepted native-demand session as one
implementation packet. The leaf substrate is typed, but no production command
root can currently return `SourcePreparationNeeds`:

- `ModuleSourcePreparationKey`, `RepositorySourceFileKey`, and
  `RepositoryMaterializationKey` return `SourcePreparationOutcome`, but
  `ModuleSourcePreparationKey` has no production discovery caller.
- `RootModuleGraphKey`, loading package and `.bzl` keys,
  `ConfiguredTargetAnalysisKey`, and query graph/environment APIs erase
  transient work into `CompactString`, `LoadingError`, `AnalysisError`,
  `QueryError`, or `anyhow`.
- root MODULE evaluation is the only marker-conditional semantic event
  producer. Loading and analysis still use direct print handlers, so activating
  retries now would leak speculative output.
- build and query each own a duplicated one-attempt passive updater and allocate
  registry/repository generations independently of the retained native command.
- one-shot CLI and retained server own separate formatting and REAPI paths;
  there is no command-owned selected-event output envelope.
- root, loading, and query semantic reads still depend on eager
  `WorkspaceSnapshot`, `WorkspaceRawSnapshot`, and
  `WorkspaceDirectorySnapshot` injection.

#### Command root and typed control boundary

Each activated surface must have one explicit DICE command-root key, or one
deterministic root bundle containing an always-present anchor. A terminal valid
query may otherwise record no root, which makes exact closure selection
impossible. Pure parsing, flag normalization, output-format validation, and
other work that cannot emit an event or Need runs before lease acquisition.
Everything that can emit an event or Need must be reachable from the sealed
root closure.

The command root returns only:

```text
SourcePreparationOutcome<Result<TerminalValue, TerminalSemanticError>>
```

`Need` is transient control state: it is invalid, unequal even to itself, never
cached as a user error, and never converted to a string/error wrapper. Eligible
Starlark failures remain `Complete(Err(...))` so their exact terminal closure
can retain semantic demands and events. Infrastructure, native-I/O,
nonprogress, conflict, closure, and cancellation failures abort and publish
nothing.

Discovery and joins must union the Needs from all independently reached
branches with `SourcePreparationNeeds::try_union` in deterministic order; they
must not force an otherwise-lazy branch merely to discover more work.
Returning the first Need from branches already reached loses conflict and
cumulative-work evidence. Build, loading, analysis, and query adapters must
preserve the typed outcome through every layer; query may use a private
internal carrier across its `QueryError`-fixed generic boundary, but only the
outer command-root facade may unwrap it.

#### Attempt, cancellation, and failure ownership

One synchronous command owner holds one logical
`Option<NativeDemandCommand>` abort guard across all of its attempts. Every
ordinary return funnels through exactly one of consuming
`progress`, accepted terminal finish, or restore-before-discard. Retry seals the
attempt, clones/extracts the exact `SourcePreparationNeeds`, drops the outcome,
returned semantic roots, transaction, and sealed attempt values, then performs
repository-first native work. Terminal selection keeps the transaction and
every returned root alive through the one exact closure read, then drops them
before materializer acceptance.

The current daemon is synchronous and serial (`&mut Daemon` in one listener
loop); no extra server mutex is needed. It transports no cancellation token,
and a disconnected client is noticed only when the completed response write
fails. Initial activation therefore covers DICE cancellation/compute errors,
native errors, normal early returns, and unwind cleanup only.
Request-ID/client-disconnect cancellation requires a later protocol/server
packet; no acceptance claim or test may name disconnect cancellation before
the server transports a token and observes disconnect before response write.

Attempt transaction, returned roots, outcome, and sealed values are scoped and
declared so they drop before the outer abort guard. On unwind, the guard takes
the command and invokes the same synchronous restore/discard funnel. It closes
only after coherent restoration and discard. If cleanup/restoration fails,
runtime/materializer ownership stays non-Idle with required pins retained.
Drop must never silently call `close`. If this cannot be implemented without a
nested-runtime panic, stop before activation and design a nonblocking outer
cleanup owner.

No mutex lock guard or borrowed state lock spans a DICE
commit/compute/closure await, native observation or materialization, external
execution, output write, or `TempDir` destruction. The logical abort owner
intentionally spans the semantic command. Busy is decided before generations,
request inputs, effect owner, or materializer session allocation.

#### Terminal acceptance, publication, and REAPI order

Freeze this semantic transition:

1. compute one typed command root under a fresh owner-branded attempt;
2. on `Need`, seal retry, drop the attempt transaction/root values, and consume
   the command through strict repository-first/path-second progress;
3. on `Complete`, seal and select the one exact event/demand closure while the
   terminal transaction and returned roots remain live;
4. build and commit selected complete repository/path injections and
   validations while materializer roots remain pinned;
5. drop the terminal transaction, returned roots, and unselected value owners;
6. materializer-accept only selected exact requests/instances;
7. atomically replace the accepted request-input/injection/scope snapshot;
8. logically publish selected events into an infallible command-owned output
   buffer;
9. close the workspace lease and return the semantic result plus buffer.

REAPI execution, output materialization, CLI writes, and socket writes occur
only after step 9. Logical publication in step 8 is not a stdout/socket write.
The private-driver packet deliberately refactors the dormant `accept` return so
the buffer move occurs after accepted-snapshot replacement and before close,
while remaining externally inaccessible until the closed command returns.
External execution and writes are irreversible effects and cannot run under
the DICE/materializer session or roll back accepted semantic state. No
transaction, returned root, materializer session, or abort guard may reach
`execute_action`, output materialization, stdout, or socket write. The final
response composer retains selected Starlark output even when later execution
fails and orders it before the execution diagnostic, subject to the required
Bazel 9 oracle below. If the oracle disagrees, replan the phase split before
implementation.

Preflight, path, native, internal, closure, and explicit cancellation failures
restore the prior complete request inputs and native injections, drop all
attempt/restoration roots, discard the materializer session, preserve the prior
accepted snapshot, close the lease, and expose no event buffer. Restoration
failure remains fail-closed with the lease/session/root pins retained.

#### Fixed inputs and legacy state

Before production activation, the command bundle and accepted snapshot must
cover every injected bzlmod request input: command policy, environment policy,
lockfile mode, registry URLs, registry generation, repository generation, and
workspace revision. Every attempt reinjects the same bundle; failure restores
the prior accepted bundle. The current independent
`inject_bzlmod_request_inputs` allocators must not survive on activated paths.
The private-driver packet defines one complete normalized initial accepted
bundle: default command/environment policy, update lockfile mode, empty
registry URL list, zero generations/revision, and empty native epochs/scopes.
First-command failure must restore that exact bundle.

Production activation must not mix this retained snapshot with a newly injected
eager workspace snapshot. Root MODULE, loading file/directory, analysis, and
query semantic reads first migrate to Host path outcomes. Only after `rg`
proves no production consumer remains may cleanup delete:

- the three workspace snapshot values/keys and their file/raw/directory keys;
- core `WorkspaceObservation`, recursive eager traversal, public observation
  wrappers, and both snapshot-injection blocks;
- the daemon `FilesystemObservationAdapter`;
- loading snapshot-key reexports;
- compatibility-only `RepositoryIo`, `LocalRepositoryIo`, their result/error
  adapters, and no-op installation.

The visible daemon `invalidated_files` metric needs an explicit replacement or
deliberate protocol decision before its eager adapter is removed.

#### Serial packets and stop gates

1. `WP-5-m1-terminal-event-execution-oracle-design` and its separate oracle
   implementation.

   Extend the existing retained-server/REAPI fixture
   `tests/v2_oracle/fixtures/load-invalidation`. The implementation allowlist is
   exactly its `fixture.toml`, `workspace/**`, and `expected/oracle.json`; a
   runner change requires the design review to prove why fixture-local capture
   cannot express the evidence, then freeze the smallest exact extra file
   before implementation. Capture stdout and stderr separately without
   normalization that erases channel order. Freeze exact root-MODULE,
   dependency `.bzl`, parent `.bzl`, BUILD, and rule-implementation print order
   for success, eligible Starlark failure, fresh-command warm reuse, semantic
   edit, and execution failure. Query rows include valid empty output, warm
   reuse, and semantic edit. Prove whether selected semantic output precedes
   the execution diagnostic and that a fresh command never replays cached
   prints. No Slug Rust changes enter the oracle packets.

2. `WP-5-m1-loading-event-producer`.

   Edit only `app/slug_loading_v2/Cargo.toml` and
   `app/slug_loading_v2/src/bzl_module.rs`, with focused evidence only in
   `app/slug_loading_v2/tests/{bzl_invalidation.rs,build_file_loading.rs}`.
   `BzlModuleEvalKey` owns one marker-conditional local `.bzl` event batch and
   `PackageLoadKey` owns one local BUILD batch; dependency batches stay on
   their own nodes. Marker absence preserves current direct printing. Explicit
   empty batches clear prior versions, and runtime failures retain only prints
   executed before the failure. Do not change key values/equality, typed Need,
   runtime, commands, or publication.

3. `WP-5-m1-analysis-event-producer`.

   Edit only `app/slug_analysis_v2/Cargo.toml` and
   `app/slug_analysis_v2/src/{dice.rs,starlark_rule.rs}`, with focused evidence
   only in `app/slug_analysis_v2/tests/starlark_rule.rs`.
   `ConfiguredTargetAnalysisKey` owns one marker-conditional local rule
   implementation batch. Marker absence preserves direct printing; dependency
   targets retain separate nodes. Do not change analysis values/equality,
   loading, runtime, execution, or publication.

4. `WP-5-m1-private-shared-retry-driver`.

   Edit only `app/slug_core_v2/src/runtime/{dice.rs,events.rs,demands.rs}`.
   Drive synthetic typed build/query command-root keys through one private
   retained loop. Add the single-exit abort guard, fixed complete input bundle,
   fresh updater/transaction per attempt, exact root recording, no-cap strict
   progress, eligible terminal `Complete(Ok/Err)`, selected output buffering,
   and every restoration/fail-closed seam. Do not activate production roots,
   public APIs, CLI/server, REAPI, discovery, or legacy retirement. Stop rather
   than introduce higher-ranked async closure machinery; concrete adapters or
   an owned boxed attempt are preferred.

5. `WP-5-m1-host-directory-semantic-projection`.

   Edit only `app/slug_workspace_v2/src/{path_resolution.rs,lib.rs}`. Add the
   directory-listing analogue of `PathFileBytesKey`, preserving exact
   missing/wrong-kind/symlink/error ordering and `PathOutcome` invalidity.
   `slug_workspace_v2` must not depend on bzlmod.

6. `WP-5-m1-root-raw-host-migration-design`, then
   `WP-5-m1-typed-nonroot-discovery-composition-design`.

   First design the bzlmod root/raw Host migration. It must freeze either
   parallel typed Host root/policy/file keys for source preparation and
   discovery while the legacy production keys remain, or the complete
   propagation allowlist. The latter necessarily includes
   `app/slug_bzlmod_v2/src/{module_eval.rs,source_preparation.rs,registry_dice.rs}`,
   exports, and every live `RootModuleFilesKey`/`RootModuleGraphKey` consumer:
   `RegistryPolicyKey` and local `RegistryFileKey` must not stringify a Host
   Need into terminal `RegistryFileError`. This is a named design checkpoint
   with exact tests and forbidden scans before Rust. Do not combine its
   implementation with discovery.

   The later discovery design remains gated on resolved repository path state,
   deleted-package request ownership, repository-ignore ownership, package
   lookup, and omitted-`module()` defaults. The accepted discovery-boundary and
   package-policy oracles plus Bazel 9.2
   `ModuleFileFunction.advanceHorizon` prove that every included MODULE
   fragment's package is validated before its bytes are read or executed, and
   that omitted `module()` has ordered validation semantics. Design one
   DICE-owned nonroot preparation/evaluation key and breadth-first discovery
   command root over the accepted private nonroot evaluator. It consumes
   `ModuleSourcePreparationKey`, preserves registry/nonregistry provenance,
   composes includes through exact source keys, unions all independently
   reached Needs, and returns a typed final graph. Before Rust, this design
   checkpoint must freeze its exact new owner file, implementation allowlist,
   prerequisite owner commits, oracle rows, and implementation split.

7. Separate loading and analysis typed-propagation designs, with
   external-repository loading kept separate.

   Schedule `WP-5-m1-loading-typed-propagation-design` for the root repository
   without expanding current repository capability, then
   `WP-5-m1-analysis-typed-propagation-design`. Each is a separate design
   checkpoint that freezes an exact allowlist before Rust. The analysis design
   must account for
   `app/slug_analysis_v2/Cargo.toml`, where `slug_bzlmod_v2` is currently only a
   dev-dependency. Both designs preserve `SourcePreparationOutcome` through
   their layer and freeze focused create/edit/delete/recreate, symlink,
   equal-Need, and event-suppression evidence. No entrypoint switch.

   The loading implementation must also migrate root-repository file reads and
   package-directory listings from eager snapshot projections to Host
   `PathOutcome`, either in its exact allowlist or in an immediately following
   bounded Host-loading packet. No Host Need may become `LoadingError`.

   External repository identity, path, visibility, and typed-loading acceptance
   stay in later loading design/implementation packets around
   `app/slug_loading_v2/src/{bzl_module.rs,package.rs,visibility.rs,load_label.rs}`.
   They remain gated on the resolved repository path-state, ignore,
   deleted-package, and package-lookup owners; the root-repository propagation
   packet must preserve the existing external-repository guards.

8. Production build and query typed command roots as separate
   design/implementation pairs.

   `WP-5-m1-build-typed-command-root-design` must freeze the core analysis and
   package-root bundle, its always-present anchor for an empty target set, its
   exact core/Cargo/test allowlist, and deterministic Need union before Rust.
   No event- or Need-producing compute may remain above or beside that root.

   `WP-5-m1-query-typed-command-root-design` must freeze
   `app/slug_query_v2/Cargo.toml`,
   `src/{graph.rs,loading_environment.rs,generic.rs,evaluator.rs,lib.rs}`, and
   focused tests in `tests/{loading_query.rs,query.rs}`. The implementation adds
   one always-rooted typed query command key, preserves ordering and lazy
   traversal, never exposes Need as `QueryError`, and proves valid empty queries
   still seal a nonempty exact closure. It must decide whether the bzlmod
   production dependency is direct or comes through an accepted loading-crate
   reexport. Neither packet activates a runtime/server entrypoint.

   After the typed query root, a separate query Host-migration
   design/implementation pair migrates every direct workspace file/directory
   consumer from eager snapshot projections to Host `PathOutcome`; no Need may
   become `QueryError`.

9. Preactivation Host gates, inactive opaque-envelope plumbing, then vertical
   build and query activation.

   Before either surface activates, an exact forbidden scan plus transitive
   command-root audit must prove that surface reaches no semantic
   `WorkspaceSnapshotKey`, `WorkspaceRawSnapshotKey`,
   `WorkspaceDirectorySnapshotKey`, file/raw/directory projection of those
   keys, or API that accepts or injects `WorkspaceObservation`. This gate
   follows the accepted root/raw, loading, analysis, and query Host migrations;
   activation stops if any eager semantic path remains.

   First add only the private-to-opaque core envelope needed to make output
   consumption structurally mandatory; do not activate a producer while any
   caller can drop its buffer.

   Build activation is one atomic vertical packet covering core
   `runtime/{dice.rs,events.rs,mod.rs}`, CLI `commands/build.rs`, server
   `src/{lib.rs,reapi.rs,tests.rs}`, and the exact focused CLI tests frozen by
   its design. It switches one-shot and daemon build together, consumes the
   event buffer exactly once, and preserves the existing post-core REAPI helper
   boundary. Query activation follows as its own atomic vertical packet over
   the corresponding core runtime files, CLI `commands/query.rs`, server
   `src/{lib.rs,tests.rs}`, and exact focused CLI tests. Each packet removes
   independent generation allocation and passive updating only for its surface
   and proves one-shot/daemon equivalence, repository-before-path retries,
   fixed generations, eligible terminal failure, restoration, nonempty empty
   roots, and no lost or duplicate output.

   The daemon filesystem scan may remain temporarily only to preserve an
   explicitly accepted `invalidated_files` metric; its values must not enter
   DICE or affect the activated command.

10. `WP-5-m1-reapi-phase-alignment`.

    Only after both vertical activations and the oracle are accepted, align and
    deduplicate one-shot/daemon execution and materialization. Core exposes only
    the opaque terminal result/output envelope. DICE IDs, Needs, session
    tokens, generations, roots, materializer owners, and selected sidecars
    remain private; server wire DTOs stay primitive. Execution-failure channel
    order must match the oracle, and no request-disconnect cancellation claim
    enters this packet.

11. Legacy retirement as explicit gated subpackets.

    Verify the already accepted Host migrations with exact forbidden scans;
    do not reschedule root/raw, loading, analysis, query, or typed propagation
    here. Retire only remaining loading compatibility consumers/reexports in
    `slug_loading_v2/src/{bzl_module.rs,keys.rs}`, then any remaining query
    compatibility consumers only in `slug_query_v2/src/graph.rs`. The final
    definition/adapter deletion is
    limited to workspace `src/lib.rs`, core
    `runtime/{dice.rs,mod.rs,repository_io.rs}`, bzlmod
    `src/{source_preparation.rs,lib.rs}`, and server `src/lib.rs`, plus exact
    tests frozen by its design. Forbidden scans must prove the corresponding
    production consumers are gone before each deletion. Keep neutral workspace
    owner tests until the final packet and decide `invalidated_files` ownership
    before removing the daemon adapter.

Items 6 through 11 are routing checkpoints, not executable Rust packets. Each
must first produce an independently accepted design with prerequisite commits,
an exact allowlist, focused evidence, and a stop gate. No later item inherits
permission to edit the files named here.

Evidence: Live audits found typed leaf outcomes but no production typed command
root, speculative loading/analysis print handlers, duplicated passive
build/query snapshots and generation allocation, no command-owned output
envelope, synchronous serial daemon ownership without disconnect cancellation,
and REAPI effects that must remain post-lease. The accepted checkpoint freezes
one abort-owned retry loop, fixed complete request inputs, exact closure
selection, accepted snapshot replacement followed by infallible logical event
buffering and lease close, post-close external effects, and vertically atomic
output consumption. It serializes the retained oracle, loading and analysis
event producers, private driver, directory projection, Host migrations,
discovery prerequisites, typed loading/analysis/build/query roots, activation,
REAPI alignment, and legacy deletion behind exact design gates. Independent
discovery/source, lifecycle/architecture, and live-feasibility rereviews all
returned `ACCEPT` on the corrected latest diff.

Next evidence after terminal acceptance of this checkpoint:
design only `WP-5-m1-terminal-event-execution-oracle-design`.

### Stage 5 terminal event/execution oracle design

Status: Accepted

#### Observed Bazel boundary

Two independent retained Bazel 9.2.0 runs of a confined copy of
`load-invalidation` established one stable stderr sequence:

- cold build: root MODULE, dependency `.bzl`, parent `.bzl`, BUILD, then the
  selected rule implementation;
- unchanged warm build/query: no semantic print replay;
- an edited dependency followed by an empty-result query: dependency `.bzl`,
  parent `.bzl`, then BUILD, with no root or rule-implementation print;
- the following build: only the invalidated rule implementation;
- eligible analysis failure: the selected rule prefix before its Starlark
  diagnostic, with cached loading prints absent; and
- action failure: the selected rule prefix, Bazel's `Action ... failed`
  diagnostic, then action stderr, all on stderr.

The valid empty query has empty stdout. The harness captures stdout and stderr
through separate pipes, so this packet asserts order only within each channel;
it makes no cross-channel temporal claim. Bazel receives a local failing action
while the fixture's NativeLink service is Slug-only, so the oracle freezes
Bazel's semantic-output-before-execution-error phase/channel order, not Bazel
remote-executor wording.

Pinned source is Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`:

- `ModuleFileFunction.execModuleFile` installs the root/nonregistry DEBUG print
  handler while registry module printing is silent;
- `BzlLoadFunction` computes loaded dependencies before parent execution,
  installs the `.bzl` DEBUG handler, and keeps events outside cached value
  equality;
- `PackageFactory.executeBuildFileImpl` stores BUILD prints in the package-local
  handler;
- `RuleContext.createStarlarkThread` and
  `StarlarkRuleConfiguredTargetUtil.evalRule` store rule prints and then
  classify implementation failure;
- `StarlarkActionFactory.runShell`, `ActionExecutionFunction`,
  `BuildTool`, and `SkyframeActionExecutor` place action execution after
  analysis and report the action failure before captured process stderr; and
- `QueryCommand`, `QueryEnvironmentBasedCommand`, and `SkyQueryEnvironment`
  write results to stdout, report empty results on stderr, and consume package
  values without configured-target analysis.

#### Required harness prerequisite

Fixture-local TOML cannot express the mixed REAPI applicability. With
`[reapi] remote_executor = true`, `runner.py` currently appends the Slug remote
flag to every command and `compare.py` requires positive completed REAPI
evidence for every row. Ordinary query performs no action, analysis failure
never reaches execution, and failed execution emits an error rather than the
successful completed-evidence object.

Implement first, as
`WP-5-m1-terminal-oracle-reapi-applicability`, in exactly:

- `tools/v2_oracle_lib/runner.py`;
- `tools/v2_oracle_lib/compare.py`; and
- `tests/v2_oracle/test_v2_oracle.py`.

For a remote-executor fixture, append remote flags only when the fixture
command's first argument is `build`. Require and structurally validate positive
REAPI evidence exactly when the first argument is `build` and the command
contract's `expected_exit` is zero. The normal actual-exit comparison remains
independent, so an expected-success build cannot evade evidence checking by
failing. An expected-failing build still receives the remote flag and remains
governed by its exit, stderr, and manifest assertions; query receives neither
the flag nor an evidence requirement. Preserve the existing strict behavior
for every current successful REAPI build fixture. Add focused regressions for
successful build, failed build, and query applicability. Do not add a schema
field, fixture special case, command-name check, tool-output heuristic, or
production Slug change.

#### Nine-row retained oracle

Then implement `WP-5-m1-terminal-event-execution-oracle` by replacing the
current two-row sequence with exactly these cumulative rows:

1. `cold_success_v1_order`: `build //pkg:message`, exit zero, empty stdout,
   manifest `one\n`, and exact once-only stderr order
   `ROOT_MODULE`, `DEP_BZL_V1`, `PARENT_BZL`, `BUILD`,
   `RULE_MESSAGE_ONE`.
2. `unchanged_warm_build_no_replay`: the same build and byte-identical
   manifest, exit zero, empty stdout, and the complete unique-prefix namespace
   absent.
3. `valid_empty_warm_query`: query
   `//pkg:message except //pkg:message`, exit zero, exactly empty stdout, and
   no unique-prefix sentinel.
4. `empty_query_after_dependency_edit`: mutate both the dependency sentinel
   V1 to V2 and `MESSAGE` from `one` to `two`, repeat the empty query, and
   require exit zero, exactly empty stdout, and exact stderr order
   `DEP_BZL_V2`, `PARENT_BZL`, `BUILD`, with root and every rule sentinel
   absent.
5. `unchanged_warm_query_no_replay`: repeat row 4 without mutation; stdout is
   exactly empty, exit is zero, and the complete unique-prefix namespace is
   absent.
6. `build_after_query_edit_only_analysis`: build `//pkg:message`; only
   `RULE_MESSAGE_TWO` is present, exit is zero, stdout is exactly empty, and
   the manifest is `two\n`. This proves the query retained the loading result
   while the downstream configured target still invalidated.
7. `eligible_analysis_failure_prefix`: build a distinct
   `//pkg:analysis_failure` target; only
   `RULE_ANALYSIS_FAILURE_PREFIX` precedes `TERMINAL_ANALYSIS_FAILURE`, exit is
   one, stdout is exactly empty, cached loading prints are absent, and no
   action sentinel appears.
8. `execution_failure_after_semantic_output`: build a distinct
   `//pkg:execution_failure` target whose rule prints
   `RULE_EXECUTION_FAILURE_PREFIX` and registers one `run_shell` output that
   writes `TERMINAL_ACTION_FAILURE_DIAGNOSTIC` to stderr and exits nonzero.
   Require the rule prefix before stable `Action ... failed` shape before the
   action sentinel; expected exit is exactly one, stdout is exactly empty, and
   analysis-failure and cached loading sentinels are absent.
9. `warm_message_after_failures_no_replay`: build `//pkg:message`, retain the
   row-6 `two\n` manifest, exit zero, exactly empty stdout, and reject the
   complete unique-prefix namespace. This proves neither failed command
   replays selected output into a later command.

Use three sibling rule implementations rather than a mode attribute. Keep
manifest roots command-local and exactly
`["bazel-bin/pkg/message.txt"]` on rows 1, 2, 6, and 9; remove the fixture-level
manifest root so query and failure records do not retain stale-output
assertions. Set `stdout_patterns = ["\\A\\Z"]` on every row. Use one globally
unique prefix for every semantic print, the analysis-failure token, and the
action-stderr token. The same anchored negative over that complete namespace
governs warm rows 2, 3, 5, and 9; eventful rows use anchored negative-tempered
`stderr_patterns`. Semantic or message-shape comparison alone does not compare
normalized stderr.

The exact oracle allowlist is six existing files:

- `tests/v2_oracle/fixtures/load-invalidation/fixture.toml`;
- `tests/v2_oracle/fixtures/load-invalidation/expected/oracle.json`;
- `tests/v2_oracle/fixtures/load-invalidation/workspace/MODULE.bazel`;
- `tests/v2_oracle/fixtures/load-invalidation/workspace/pkg/BUILD.bazel`;
- `tests/v2_oracle/fixtures/load-invalidation/workspace/pkg/defs.bzl`; and
- `tests/v2_oracle/fixtures/load-invalidation/workspace/pkg/message.bzl`.

The root `workspace/BUILD.bazel`, other fixtures, new files/symlinks,
registry/platform scaffolding, Slug Rust/Cargo, server/CLI/runtime code, and
BUILD metadata are forbidden.

#### Growth, validation, and stop gates

The latest fixture-growth checkpoint is accepted tree `42e38bc3`: 1,272
regular files, ten symlinks, and 31,208 lines. The three later accepted oracle
packets are root-patch `9fa4fbde`, Local lifecycle `dcc19327`, and root-MODULE
include events `699c3a8e`; current growth is six regular files, four symlinks,
610 lines, and eleven discriminating rows. This is packet four, below every
hygiene trigger. The retained fixture has seven regular files, no symlinks, and
121 lines; the verified nine-row prototype remained below 400 total fixture
lines. Cap net fixture growth at 350 lines and add no entry.

Generate with pinned Bazel 9.2, then run two independent fresh-root normalized
replays and the focused fixture. Require nine unique command names and expected
records, exact cumulative mutations, exact V1/V2 manifest relations, stable
exit and diagnostic shapes, source-anchor existence, JSON/schema validation,
terminal newlines, `git diff --check`, exact allowlists, tracked archive
inventory, and credential guards.

Stop and replan on more than nine rows, more than 350 net fixture lines, any
new entry/scaffold, a cross-channel decisive ordering, unstable action
diagnostic order, missing successful-build REAPI evidence, a query/failure
evidence requirement, changed old REAPI-fixture behavior, or any allowlist
escape. Do not weaken an exact sentinel negative to accept replay.

Evidence: A two-run confined Bazel 9.2 probe passed the exact nine-row
event/failure/query sequence, including dependency-edit query retention,
analysis-only follow-up, eligible failure prefix, action diagnostic order, and
warm nonreplay. Live harness inspection proved a fixture-local configuration
cannot distinguish successful action builds from query and expected-failing
builds, so the design splits one generic three-file applicability prerequisite
from the six-file oracle. Fixture-growth review places this as packet four at
only +6 regular files, +4 symlinks, and +610 lines before implementation.
Independent fixture/discovery, Bazel source/observation, and
architecture/maintainability rereviews returned `ACCEPT` on the corrected
latest diff.

Next evidence after terminal acceptance of this design:
implement only `WP-5-m1-terminal-oracle-reapi-applicability`.

### Stage 5 terminal oracle REAPI applicability

Status: Accepted

The generic harness now appends NativeLink remote flags only to Slug build
commands in a remote-executor fixture. Positive completed REAPI evidence is
required exactly for a build command whose declared `expected_exit` is zero;
the ordinary actual-exit comparison remains independent. Expected-failing
builds still receive the remote flag, while query receives neither the flag nor
an evidence requirement.

Focused regressions prove successful-build strictness, expected-failing build
behavior, query exclusion, and both expected/actual exit mismatch directions.
The two top-level oracle-harness test modules pass 52 tests. Formatting,
diff-check, and exact three-file scope gates pass. Independent contract and
architecture latest-diff reviews returned `ACCEPT`; the architecture reviewer
withdrew an actual-exit suggestion after rereading the accepted contract and
the bidirectional discriminator.

Next evidence: Implement only the six-file, nine-row
`WP-5-m1-terminal-event-execution-oracle`.

### Stage 5 terminal event/execution oracle first implementation

Status: Replanned

The six fixture files generated and replayed the accepted nine Bazel 9.2 rows
twice from fresh roots. Exact producer/failure ordering, nine exits, empty
stdout, command-local V1/V2 manifests, warm nonreplay, zero new entries, and
net fixture growth of 321 lines all passed. Runtime assembly of the analysis
and action failure sentinels prevented echoed source/command text from
satisfying their diagnostic patterns.

The required downstream harness run exposed one direct consumer outside the
allowlist: `test_fixture_parser_reads_commands_and_mutations` hard-codes two
commands and the old mutation at command index one. The accepted six-file
allowlist forbade correcting it, and the stop gate explicitly required replan
on any escape. Both latest-diff reviews returned `REPLAN`. All six draft files
were returned to clean `7a1ba309`; no oracle change was retained.

### Stage 5 terminal event/execution oracle allowlist correction

Status: Accepted

Retry the semantically unchanged nine-row oracle with one exact seventh file:
`tests/v2_oracle/test_v2_oracle.py`. In that file only, strengthen the direct
fixture parser regression to:

- assert the exact nine command names in accepted order, rather than only a
  count;
- select `empty_query_after_dependency_edit` at command index three; and
- assert its exact two ordered `pkg/message.bzl` mutations: sentinel V1 to V2,
  then `MESSAGE = "one"` to `"two"`.

Preserve the separate daemon/REAPI parser regression. No runner, comparator,
schema, other fixture, production code, new entry, row, sentinel, manifest,
provenance, or growth contract changes. Validate the focused parser and both
top-level harness modules in addition to pinned generation and two fresh-root
replays. The exact retry allowlist is the original six fixture files plus this
one test file.

Two independent terminal reviews agreed that the six fixture files otherwise
fully conform and that this is the sole correction. Fixture growth remains zero
entries and 321 lines because the test edit is outside the fixture tree.

Next evidence: Retry only the corrected seven-file
`WP-5-m1-terminal-event-execution-oracle`.

### Stage 5 terminal event/execution oracle corrected retry

Status: Accepted

The corrected seven-file retry retains nine exact Bazel 9.2 records, closes
the direct parser consumer with all command names and both ordered row-4
mutations, and adds no entry. Pinned generation, two fresh-root replays, 52
harness tests, exact exits/manifests/sentinels, diff/allowlist gates, and
321-line fixture growth passed. Both terminal reviews returned `ACCEPT`.

Next evidence: Implement only `WP-5-m1-loading-event-producer`.

### Stage 5 loading event producer first implementation

Status: Replanned

The exact four-file candidate added marker-conditional local event batches to
`BzlModuleEvalKey` and `PackageLoadKey`. Focused evidence discriminated
dependency/parent/BUILD ownership, marker-off direct printing, explicit empty
replacement, pre-evaluation empty publication, and runtime prefixes. The first
focused run exposed that loading's globals did not actually contain `print`;
the first correction added a copied-value Print overlay. Downstream
`slug_core_v2` then segfaulted because frozen modules outlived that overlay's
base-owned values. A process-static owner corrected the lifetime and passed all
54 `slug_loading_v2` tests plus all 98 `slug_core_v2` tests, but this was the
packet's second material correction. The terminal architecture review therefore
returned `REPLAN` under the orchestration policy. No producer code is accepted
or committed from this packet.

Next evidence: Design only a loading-globals owner correction that expands the
allowlist to `app/slug_loading_v2/src/package.rs`, constructs the standard
loading globals plus `LibraryExtension::Print` in one owning `Globals` heap,
and removes the copied-value overlay before retrying the unchanged producer.

### Stage 5 loading event producer globals-owner correction design

Status: Accepted

Retry the producer in exactly five files: the prior Cargo, `bzl_module.rs`, and
two focused test files plus `app/slug_loading_v2/src/package.rs`. In
`package.rs`, build `loading_globals()` with
`GlobalsBuilder::extended_by(&[LibraryExtension::Print])`, then add the
unchanged package/select/native/attr/provider globals on that same builder. In
`bzl_module.rs`, remove every copied-value/process-static globals overlay and
bind the one-heap `loading_globals()` result locally at each evaluator. Preserve
the existing `.map(|_| ())`, evaluator teardown before capture consumption,
outer finalizer, event batches, tests, key values/equality/validity, and all
other surfaces unchanged.

Require the two focused loading tests, all 54 `slug_loading_v2` tests, the
13-test serial core runtime, all 98 `slug_core_v2` tests, GNU-Windows loading
compilation, formatting/diff/archive checks, and an exact five-file gate. Stop
on any copied/static globals owner, extension beyond Print, event/equality/test
change, sixth file, crash, panic, or downstream failure. Independent ownership
and architecture reviews accepted the one-heap design; the scope reviewer’s
sole revision was already incorporated by explicitly adding `package.rs`.

Next evidence: Implement only the corrected five-file
`WP-5-m1-loading-event-producer`.

### Stage 5 loading event producer corrected retry

Status: Accepted

The corrected five-file producer builds standard, Print, and loading-specific
globals in one `package.rs`-owned `Globals` heap; no copied-value or
process-static overlay remains. `BzlModuleEvalKey` and `PackageLoadKey` store
one marker-conditional local batch, dependency events remain on child nodes,
marker absence keeps the default direct handler, explicit empty batches replace
prior versions, and runtime failures retain only their executed prefix. Key
values, equality, validity, typed Need, runtime, commands, and publication are
unchanged.

The focused `.bzl` and BUILD suites passed 23 and 19 tests; all 54 loading tests,
the serial 13-test core runtime, and all 98 core tests passed. All GNU-Windows
loading test executables linked. Formatting, diff, archive, static-owner, and
exact five-file gates passed. Independent lifetime/evidence and architecture
terminal reviews both returned `ACCEPT`; residual publication integration
remains owned by later packets.

Next evidence: Implement only `WP-5-m1-analysis-event-producer`.

### Stage 5 analysis event producer

Status: Accepted

`ConfiguredTargetAnalysisKey` now owns one marker-conditional local rule
implementation batch. Capture begins only after package and dependency DICE
awaits, is borrowed only by the synchronous local evaluator, and is consumed
before result propagation. Dependency targets therefore retain separate nodes;
marker absence preserves direct printing, pre-local failures store explicit
empty batches, and local runtime or validation failures retain only their
executed prefix. Analysis values, equality, validity, loading, runtime,
execution, and publication are unchanged.

The focused analysis rule suite passed 4 tests, the full analysis crate passed
12 tests, and the downstream core crate passed 85 unit plus 13 runtime tests.
All GNU-Windows analysis test executables linked. Formatting, diff, archive,
and exact four-file gates passed. Independent lifetime/evidence and
implementation reviews both returned `ACCEPT`; retry and publication
integration remains owned by later packets.

Next evidence: Implement only `WP-5-m1-private-shared-retry-driver`.

### Stage 5 private shared retry driver

Status: Accepted

Synthetic typed build and query roots now run through one private retained
driver with a fixed complete command bundle, fresh transaction per attempt,
exact terminal-root selection, cap-free strict progress, and terminal
`Complete(Ok/Err)` handling. Selected injections commit under the live terminal
transaction; materializer acceptance, accepted-snapshot replacement, output
buffer movement, and lease close follow the frozen order. Cancellation,
unwind, selected-injection failure, restoration failure, and irreversible
post-accept failures preserve the required restore-or-fail-closed ownership.
The driver remains dormant: no production command, public API, server, CLI,
REAPI, discovery, or legacy-snapshot path is activated.

The full core suite passed 92 unit, 13 runtime, and zero doc tests. Both
GNU-Windows test executables linked. Formatting, diff, archive, and exact
two-file scope gates passed. Independent lifecycle and architecture/policy
terminal rereviews both returned `ACCEPT`.

Next evidence: Implement only
`WP-5-m1-host-directory-semantic-projection`.

### Stage 5 Host directory semantic projection

Status: Accepted

`PathDirectoryListingKey` is now the directory-listing analogue of
`PathFileBytesKey`. It resolves the logical path first, returns semantic
Missing for resolved absence, classifies only a resolved terminal
regular/special file as wrong-kind, follows symlinks, and requests exact
physical `DirectoryEntries` only for a resolved directory. Post-lstat Missing
is inconsistent state; observation and resolution failures retain complete
logical semantic identity. Every Need remains invalid and self-unequal.

The value reuses the existing sorted, unique, `Arc`-backed OS-native
`PathDirectoryEntries`; no bzlmod dependency or production consumer was added.
The workspace owner passed 32 unit plus zero doc tests, downstream core passed
92 unit, 13 runtime, and zero doc tests, and all three owner/core GNU-Windows
test executables linked. Formatting, diff, archive, dependency, consumer, and
exact two-file gates passed. Independent semantic/lifecycle and
architecture/hot-path reviewers both returned `ACCEPT`.

Next evidence: Design only
`WP-5-m1-root-raw-host-migration-design`.

### Stage 5 root/raw Host migration design

Status: Replanned before Rust

The parallel dormant Host spine, private special-file-compatible byte leaf,
Need ownership, pinned Bazel 9.2 root/include stops, legacy-consumer inventory,
and production/discovery separation survived review. The packet nevertheless
reached terminal `REPLAN` after its one correction: all three latest reviewers
proved that the proposed two-file Host registry packet was not implementable.
`RegistryPolicy` internals, the installed `RegistryIoHandle`, remote policy,
fetch, and request-generation helpers are private to `registry_dice.rs`.
Delegating to legacy registry keys would restore the eager
root/policy/error-erasure chain, while embedding `RegistryFileError` would
admit its erased root and local-read variants. No Rust from this design was
started or retained. The rejected detailed draft was removed; only the compact
diagnostic inputs below remain.

#### Preserved inputs for redesign

- Full in-place root propagation is not bounded: legacy graph consumers in
  loading and core terminalize ordinary results, so the reviewed direction
  remains a dormant parallel Host spine.
- The private bzlmod raw-byte leaf must resolve Host paths and accept both
  regular and special files; public `PathFileBytesKey` rejects the latter.
- Only `ModuleSourcePreparationKey` and
  `RepositoryMaterializationKey` may translate path Need at their existing
  `SourcePreparationOutcome::path_need` boundaries. All Host keys retain
  invalid, self-unequal `PathOutcome::Need`.
- Public Host root keys remain gated on separately accepted missing-root
  create/warning ownership and include package-validation/repository-ignore
  ownership, as pinned by Bazel 9.2
  `8220c6198837d5c13d53fea211cf3282aa12408a`.
- The redesign must cover the exact four source-preparation legacy edges and
  preserve every other legacy consumer/export. Its new work is the private
  registry boundary: accessible shared policy/transport, unchanged legacy
  behavior, local Host isolation, and a remote-only typed error schema.

Next evidence: Design only
`WP-5-m1-root-raw-host-registry-boundary-redesign`.

Audit the live private registry policy/transport closure and freeze either an
exact crate-private policy-free remote bridge or a bounded shared-module move.
Name the exact allowlist, unchanged-legacy tests, local-branch forbidden scan,
and a Host-only remote error schema that cannot contain legacy root/local
erasure. Then resubmit the complete serial root/raw Host contract; do not
implement the otherwise viable private byte leaf in isolation.

### Stage 5 root/raw Host registry-boundary redesign

Status: Accepted

Three independent latest-text reviewers returned `ACCEPT`. The one semantic
correction added the exact bridge signature, the refresh-recorded-absence
generation row, private-until-projection API ownership, normalized Host key
identities/policy equality, and typed root-error propagation through both
preparation boundaries. A final mechanical correction made the future-file
call-site scan executable after every serial packet.

#### Corrected boundary

Retain the dormant parallel Host spine and the Bazel 9.2 root/include stops
recorded above. Correct only the failed registry boundary: first extract one
policy-input-only remote bridge inside `registry_dice.rs`, then let an isolated
`host_registry.rs` consume it. The bridge receives already-complete lockfile
mode and visible-lockfile inputs. It never computes a registry policy/file,
root-module, or workspace snapshot key.

The installed `RegistryIoHandle`, `RemotePolicy`, raw fetch/generation helpers,
and legacy `RegistryPolicy` fields remain private to `registry_dice.rs`.
`RegistryFileKey::compute_remote` continues to compute the legacy policy, then
calls the bridge and exhaustively maps the new remote-only error back to the
unchanged `RegistryFileError`. The Host remote branch calls the same bridge
only after a complete Host policy. The Host local branch never touches the
bridge, registry IO, or request generation.

Packets 1–5 add no public surface. Every new Host key, value, and error remains
crate-private while dormant. A separate accepted API-design packet must freeze
the public typed projection required by `ModuleSourcePreparationError` and
`RepositoryMaterializationError` before source preparation switches.

#### Exact remote bridge

`WP-5-m1-registry-remote-bridge` edits exactly
`app/slug_bzlmod_v2/src/registry_dice.rs`. It adds:

- crate-private `RootModuleRegistryUrls::urls`;
- crate-private `RegistryRemoteError`; and
- exactly:

  ```rust
  pub(crate) async fn read_remote_registry_file(
      ctx: &mut DiceComputations<'_>,
      workspace: &Path,
      url: &RegistryFileUrl,
      mode: &LockfileMode,
      visible_lockfile: &VisibleLockfileRead,
  ) -> Result<RegistryFileValue, RegistryRemoteError>
  ```

The bridge returns `Result<RegistryFileValue, RegistryRemoteError>`.
`RegistryRemoteError` has exactly:

- `MissingRequestGeneration { workspace: PathBuf, url: RegistryFileUrl,
  message: CompactString }`;
- `MissingIoCapability { url: RegistryFileUrl }`;
- `MissingChecksumInError { url: RegistryFileUrl }`;
- `InvalidLockfileExpectation { url: RegistryFileUrl,
  message: CompactString }`;
- `Transport { url: RegistryFileUrl, message: CompactString }`; and
- `ChecksumMismatch { url: RegistryFileUrl, expected: [u8; 32],
  actual: [u8; 32] }`.

It contains no legacy error, root, local-file, policy-input, or Need variant.
The legacy exhaustive mapping preserves its existing public enum and results.
The dependency matrix remains exact:

- Off, update/refresh unrecorded, and refresh recorded absence acquire
  generation before IO;
- update/error recorded absence performs neither generation nor IO;
- error unrecorded returns missing-checksum without either;
- recorded SHA reads first, acquires generation only after 404 or transport
  failure, and gives verified success and checksum mismatch no generation
  edge.

The unchanged full `tests/registry_dice.rs` suite is the focused regression,
especially
`registry_policy_matches_lockfile_mode_matrix_before_io`,
`unrecorded_remote_outcomes_retry_only_after_generation_changes`,
`known_sha_failure_acquires_generation_but_verified_success_drops_it`,
`known_sha_transport_failure_retries_after_generation_changes`,
`checksum_mismatch_is_typed_and_stable_for_the_same_expectation`, and
`remote_io_fails_closed_when_required_inputs_or_capability_are_missing`.
This is a private extraction over already discriminating tests; do not add a
duplicate matrix fixture.

#### Corrected serial implementation

1. Implement only `WP-5-m1-registry-remote-bridge` as specified above.

2. `WP-5-m1-bzlmod-private-host-raw-byte-projection` edits only new
   `app/slug_bzlmod_v2/src/host_file.rs` and
   `app/slug_bzlmod_v2/src/lib.rs`. Add only a private module, key, value, and
   `HostFileError`; reexport none of them. The normalized absolute logical Host
   path computes `ResolvedPathKey`, accepts regular or special, follows
   symlinks, returns semantic Missing, rejects Directory, and demands exact
   real-path bytes.
   Tests
   `host_file_bytes_cumulative_projection_accepts_bazel_file_kinds` and
   `host_file_bytes_semantic_lifecycle_prunes_physical_identity_and_restores`
   prove cumulative demand, Need invalidity, raw bytes, typed terminals,
   symlink retargeting, and A→B→Missing→error→A on one retained engine.

3. `WP-5-m1-host-root-module-parity-prerequisites-design` freezes retained
   Bazel 9.2 oracle rows, exact allowlists, and exact tests for two separately
   reviewable implementation owners:
   `WP-5-m1-root-module-missing-create-warning-owner` and
   `WP-5-m1-root-include-package-validation-owner`. Host root keys
   cannot start until both implementation commits have terminal `ACCEPT`,
   same-daemon create/edit/delete restoration, and zero owner TODOs. Design
   acceptance alone is insufficient.

4. `WP-5-m1-host-root-module-keys`, gated on both prerequisite commits, puts
   the new implementation in `app/slug_bzlmod_v2/src/host_module.rs`; packet 3
   owns any exact prerequisite bridge allowlist. `lib.rs` adds only the private
   module declaration. Crate-private `HostRootModuleFilesKey`,
   `HostRootModuleGraphKey`, and `HostRootModuleError` remain dormant; their
   values are exactly
   `PathOutcome<Arc<Result<RootModuleFiles, HostRootModuleError>>>` and
   `PathOutcome<Arc<Result<RootModuleGraph, HostRootModuleError>>>`.
   Private Host evaluation/visible-lockfile keys use the private byte leaf.
   Same-level Needs union without forcing undiscovered descendants;
   root/includes precede lockfile; Off requests no lockfile; typed event
   batches preserve executed prefixes and explicit pre-evaluation empties.
   Exact tests are
   `host_root_module_cumulative_need_union_and_lifecycle`,
   `host_root_module_missing_create_warning_owner_is_observed`,
   `host_root_module_include_package_validation_precedes_bytes`, and
   `host_root_module_lockfile_events_replay_a_b_a`.

5. `WP-5-m1-host-registry-keys`, gated on packet 4, edits only new
   `app/slug_bzlmod_v2/src/host_registry.rs` and the private module declaration
   in `app/slug_bzlmod_v2/src/lib.rs`; it adds no reexport.
   `HostRegistryPolicyKey`, `HostRegistryFileKey`, and
   `HostRegistryPolicy`, `HostRegistryPolicyError`, and
   `HostRegistryFileError` remain crate-private. Policy-key identity is exactly
   one `NormalizedAbsolutePath` workspace; file-key identity is exactly that
   workspace plus `RegistryFileUrl`. Both keys use `complete_eq` and
   `is_complete`. Their values are exactly
   `PathOutcome<Arc<Result<HostRegistryPolicy,
   HostRegistryPolicyError>>>` and
   `PathOutcome<Arc<Result<RegistryFileValue,
   HostRegistryFileError>>>`.

   `HostRegistryPolicy` has exactly `urls: RegistryUrls`,
   `mode: RootModuleLockfileMode`, and
   `visible_lockfile: VisibleLockfileRead`; derived equality compares all three
   fields and no other root semantics. The separate direct Host-root edge is
   retained only by the local file branch, never by remote.

   Crate-private `HostRegistryPolicyError` has exactly
   `RegistryUrlsInput { workspace: NormalizedAbsolutePath,
   message: CompactString }`, `LockfileModeInput { workspace:
   NormalizedAbsolutePath, message: CompactString }`,
   `RootModuleCompute { workspace: NormalizedAbsolutePath,
   message: CompactString }`, and
   `RootModule(HostRootModuleError)`.

   Crate-private `HostRegistryFileError` has exactly
   `InvalidFileUrl { url }`, `UnsupportedUrl { url }`,
   `PolicyCompute { workspace, message }`,
   `Policy(HostRegistryPolicyError)`,
   `RootModuleCompute { workspace, message }`,
   `RootModule(HostRootModuleError)`,
   `LocalFileCompute { logical_path, message }`,
   `LocalFile(HostFileError)`, and
   `Remote(RegistryRemoteError)`. Need enters no error variant.

   The Host policy consumes only injected URLs/mode plus
   `HostRootModuleFilesKey`. Remote waits for complete policy, then calls the
   bridge. Local waits for policy, retains a direct typed Host-root semantic
   edge, and maps private Host bytes Present/Missing/error/Need to
   Found/LocalAbsence/typed error/the same Need, with no transport or
   generation edge.

   Inline tests are
   `host_registry_policy_preserves_typed_input_root_and_lockfile_errors`,
   `host_registry_policy_propagates_root_need_before_remote_io`,
   `host_local_registry_has_no_transport_or_generation_dependency`,
   `host_local_registry_cumulative_host_lifecycle_is_typed`,
   `host_remote_registry_matches_legacy_policy_and_error_matrix`,
   `host_remote_registry_preserves_generation_capability_transport_and_checksum_errors`,
   `host_registry_file_replays_root_and_ordered_urls_a_b_a`, and
   `host_registry_need_is_transient`.

6. `WP-5-m1-source-preparation-host-error-api-design`, gated on packet 5, is
   design only. It reads the implemented crate-private Host error schemas and
   freezes the exact public, source-preparation-owned typed projections,
   variant fields, equality, exports, implementation allowlist, and mapping
   tests. At minimum it must reserve these exact public replacements:
   `ModuleSourcePreparationError::RootModule(SourcePreparationRootError)`,
   `RepositoryMaterializationError::RootModule(SourcePreparationRootError)`,
   `ModuleSourcePreparationError::RegistryPolicy(
   SourcePreparationRegistryPolicyError)`, and the existing structured
   registry-file variant with its payload changed to
   `SourcePreparationRegistryFileError`. It must prove no projection can hold
   a legacy `RegistryFileError` or root error string. Packet 7 cannot start on
   implementation readiness alone; this API design needs terminal `ACCEPT`.

7. `WP-5-m1-source-preparation-host-switch`, gated on accepted packet 6, edits
   only
   `app/slug_bzlmod_v2/src/source_preparation.rs` and
   `app/slug_bzlmod_v2/tests/source_preparation_dice.rs`, plus only the exact
   projection reexports accepted for `app/slug_bzlmod_v2/src/lib.rs`. It
   switches the callerless `ModuleSourcePreparationKey` to Host
   root/policy/file keys and makes private
   `RepositoryMaterializationRequestKey` a complete-only path outcome. Only it
   and `RepositoryMaterializationKey` convert Need at their existing
   `SourcePreparationOutcome::path_need` boundaries.

   Exact new tests are
   `root_host_need_precedes_materialization_and_registry_io`,
   `nonregistry_root_need_propagates_through_materialization_request`,
   `registry_host_needs_propagate_through_module_source_preparation`, and
   `module_source_preparation_host_need_is_transient`, plus
   `complete_host_root_error_remains_typed_through_materialization_and_preparation`.
   Existing cumulative
   local-source, ordered registry fallback/raw-byte, and root-patch ordering
   tests remain. The public preparation errors use only the accepted
   source-preparation-owned projections, not private Host errors.

#### Exact unchanged baseline and gates

Through packet 6, legacy policy callers remain
`registry_dice.rs:315,373`, `source_preparation.rs:1228`, and
`tests/registry_dice.rs:698`; legacy file callers remain
`source_preparation.rs:1325`, `tests/registry_dice.rs:176,189`, and
`app/slug_core_v2/src/runtime/dice.rs:4443`; public exports remain
`lib.rs:173,180`. IO installers and request-input injectors remain unchanged.
Packet 7 may replace exactly the four source-preparation edges currently at
`:627,1172,1228,1325`; every other legacy consumer/export remains.

After every implementation packet run focused owner tests, full bzlmod
tests/doctests, downstream loading/core suites, GNU-Windows no-run for changed
owners and downstream binaries, formatting, diff, archive, dependency, and
exact-file gates.

The bridge-body scan
`sed -n '/^pub(crate) async fn read_remote_registry_file/,/^}/p' app/slug_bzlmod_v2/src/registry_dice.rs | rg -n '\b(RegistryPolicyKey|RegistryFileKey|RootModuleFilesKey|Workspace[A-Za-z]*Key)\b'`
returns zero. Its error-schema scan
`sed -n '/^pub(crate) enum RegistryRemoteError/,/^}/p' app/slug_bzlmod_v2/src/registry_dice.rs | rg -n '\b(RootModule|Local|RegistryFileError|PathOutcome)\b'`
returns zero. This visibility scan
`rg -n 'pub\(crate\).*(RegistryIoHandle|RemotePolicy)|pub (struct|enum) (RegistryIoHandle|RemotePolicy)' app/slug_bzlmod_v2/src/registry_dice.rs`
returns zero.

The Host registry scan
`rg -n '\b(RegistryPolicyKey|RegistryFileKey|RootModuleFilesKey|Workspace[A-Za-z]*Key|RegistryFileError|RegistryIo|RegistryIoHandle|RegistryRequestGenerationKey|read_exact|std::fs|tokio::fs)\b' app/slug_bzlmod_v2/src/host_registry.rs`
returns zero. The Host activation scan
`rg -n '\b(HostRootModuleFilesKey|HostRootModuleGraphKey|HostRegistryPolicyKey|HostRegistryFileKey)\b' app/slug_loading_v2 app/slug_core_v2 app/slug_analysis_v2 app/slug_query_v2`
returns zero through packet 7.

The bridge call-site scan
`rg -n '\bread_remote_registry_file\(' app/slug_bzlmod_v2/src -g 'registry_dice.rs' -g 'host_registry.rs'`
returns exactly its definition plus one legacy caller after packet 1, and its
definition plus one legacy and one Host caller after packet 5. The legacy
baseline command
`rg -n '\b(RegistryPolicyKey|RegistryFileKey|install_registry_io|inject_registry_request_inputs|RegistryRequestGenerationKey|RootModuleRegistryUrlsKey)\b' app/slug_bzlmod_v2 app/slug_core_v2`
is compared with the inventory above after each packet.

After packet 7, both switched compute implementations pass this structural
scan:
`sed -n '/impl Key for RepositoryMaterializationRequestKey/,/^}/p; /impl Key for ModuleSourcePreparationKey/,/^}/p' app/slug_bzlmod_v2/src/source_preparation.rs | rg -n 'RootModuleFiles\(|RegistryFileError|CompactString::new\(error.to_string\(\)\)'`
with zero matches. The four named source-preparation edge removals and the
complete-root-error propagation test are mandatory together.

Stop and replan on public key/transport-capability widening, legacy result or
generation-order change, direct local IO, local generation dependency,
terminalized Need, special-file rejection, production activation, nonroot
discovery, loading/core/analysis/query edits, root/include work before both
prerequisite owners are accepted, new dependency, cache, map, or lock.

Next evidence after terminal acceptance: Implement only
`WP-5-m1-registry-remote-bridge`.

### Stage 5 registry remote bridge

Status: Accepted

The exact one-file extraction adds a crate-private remote-only error and a
policy-input-only bridge. The legacy remote key still computes its own policy,
then exhaustively maps every bridge error back to the unchanged public
`RegistryFileError`. Off, every unrecorded fetch, refresh-recorded-absence,
recorded absence, error-unrecorded, and recorded-SHA generation/IO ordering
remain exact. Local registry behavior, installed IO ownership, public exports,
callers, dependencies, and production activation are unchanged.

The focused registry suite passed 12 tests. Full bzlmod passed 233 tests plus
zero doctests; downstream loading passed 54 plus zero doctests, and core passed
92 unit, 13 runtime, and zero doctests. Every bzlmod/loading/core GNU-Windows
test executable linked. Formatting, diff, exact one-file, bridge-body,
error-schema, visibility, and two-call-site gates passed. Independent
semantic/source and architecture/implementation terminal reviewers both
returned `ACCEPT`.

Next evidence: Implement only
`WP-5-m1-bzlmod-private-host-raw-byte-projection`.

### Stage 5 private Host raw-byte projection

Status: Accepted

The exact two-file packet adds a dormant crate-private Host byte key with only
normalized logical-path identity. It resolves through the neutral path owner,
follows symlinks, accepts regular and special files, preserves raw bytes,
returns semantic Missing, rejects wrong terminal kinds, and projects
observation, inconsistent-state, cycle, and expansion failures without
retaining physical identity. Every Need remains invalid and self-unequal.

The two exact inline tests passed. Full bzlmod passed 235 tests plus zero
doctests; downstream loading passed 54 plus zero doctests, and core passed 92
unit, 13 runtime, and zero doctests. Every bzlmod/loading/core GNU-Windows test
executable linked. Formatting, diff, exact two-file, forbidden-reference,
private-surface, consumer, and dependency gates passed. Three independent
semantic, architecture/DICE, and implementation terminal reviewers returned
`ACCEPT` without correction.

Next evidence: Design only
`WP-5-m1-host-root-module-parity-prerequisites-design`.

### Stage 5 Host root-module parity prerequisites design

Status: Replanned before Rust

Three independent source, implementation, and architecture audits rejected the
proposed two-owner split. No Rust, fixture, harness, Cargo, or production file
changed.

Pinned Bazel 9.2 source at
`8220c6198837d5c13d53fea211cf3282aa12408a` proves that
`ModuleFileFunction.computeForRootModule` writes the exact 399-byte
`BZLMOD_REMINDER`, emits the exact missing-root warning, and evaluates those
bytes. A confined same-server observation confirmed missing→created/warned,
warm silence, edit preservation, delete→recreated/warned, and exact reminder
restoration with SHA-256
`0e3e315145ac7ee7a4e0ac825e1c5e03c068ec1254dd42c3caaecb27e921dc4d`.
The observation required a legacy workspace marker only to enter Bazel's
client workspace. It is diagnostic evidence, not a retained Slug fixture or
permission to support WORKSPACE.

There is no retained missing-root oracle: `empty-module-build` and
`negative-no-workspace` are ungenerated placeholders. More importantly,
missing-root creation is a mutating retry effect, while the frozen Host root
value can carry only path-observation Need. The runtime has no root-file
mutation request/result epoch, stale-Missing replacement, or warning sidecar;
`EvaluationEvent` contains only Starlark prints. Writing from a DICE compute,
encoding creation as a path observation, or publishing a warning during a
speculative attempt would violate the accepted ownership model.

The retained `nonroot-include-composition` rows and
`ModuleFileFunction.advanceHorizon` do prove that every reached package lookup
precedes include bytes and failures are reported in source order. They also
prove `BUILD.bazel` then `BUILD` selection, resolved symlink markers,
canonical deleted-package identity, `.bazelignore`, and `REPO.bazel`
invalidation. Those behaviors do not form one bounded owner. Live Slug lacks
the deleted-package request input, vendor policy, full no-load REPO evaluator,
Bazel wildcard-prefix matcher, ignored-prefix key, and repository package
lookup. Loading cannot be reused because it already depends on bzlmod and its
package/glob path omits these semantics.

The correction must therefore freeze a serial boundary instead of hiding the
gaps in two implementation commits: an outside-DICE missing-root bootstrap
request/apply/warning seam; a normalized deleted-package request owner; a
repository-ignore evaluator/matcher owner; and a repository package-lookup
owner. Only then may it resubmit the Host root-key gate and the later
command-side effect integration. Preserve the private Host byte leaf and
composed evaluator; do not copy the evaluator, read include bytes before all
same-horizon package checks, reject Bazel-compatible special-file include
fragments, add direct DICE IO, or activate discovery.

Next evidence: Design only
`WP-5-m1-root-module-effects-and-package-policy-owner-redesign`.

### Stage 5 root-module effects and package-policy owner redesign

Status: Accepted corrected implementation contract before Rust

#### Bazel authority and retained evidence

The source of truth is Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`.
`ModuleFileFunction.java:120-128,285-318,512-515,921-943` fixes the
399-byte `BZLMOD_REMINDER`, missing-root write and warning, logical root path,
and read/create failures. The confined same-server observation from the prior
packet fixes missing→created/warned, warm silence, edit preservation,
delete→recreated/warned, and exact restoration with SHA-256
`0e3e315145ac7ee7a4e0ac825e1c5e03c068ec1254dd42c3caaecb27e921dc4d`.
It remains diagnostic only: no retained fixture or Slug support may use
WORKSPACE to enter Bazel's client workspace.

`ModuleFileFunction.advanceHorizon` fixes parse-all, deduplicated
package-lookups-at-one-level, first-source-order failure, and package
preflight before any include-file value or bytes.
`PackageLookupFunction.java` fixes deleted packages before the main/external
split, reserved `//external`, ignored-subdirectory lookup, and, for the main
repository, package-root order outside `BUILD.bazel`/`BUILD` priority.
`FileValue.isFile()` accepts both RegularFile and SpecialFile.
`IgnoredSubdirectoriesFunction.java`, `RepoFileFunction.java`,
`RepoFileGlobals.java`, `RepoThreadContext.java`,
`IgnoredSubdirectories.java`, and `UnixGlob.java` fix first-readable
package-root `.bazelignore`, workspace-root `REPO.bazel`, contained vendor
prefixes, no-load REPO evaluation and events, literal prefixes, and exact
`matchesPrefix` patterns.

The retained `nonroot-include-composition` and
`nonroot-repository-path-state-symlinks` fixtures continue to prove the
shared horizon and resolved-path rules, but they use the external-repository
branch. Before root package Rust, add one discriminating main-repository
oracle and then run the required five-packet fixture-growth review.

#### Serial packet 0: root main-package-policy oracle

Run only `WP-5-m1-root-main-package-policy-oracle`. Add the isolated fixture
`tests/v2_oracle/fixtures/root-module-package-policy/` with exact tracked
allowlist:

- `fixture.toml`;
- `expected/oracle.json`;
- `workspace/MODULE.bazel`;
- `workspace/BUILD.bazel`;
- `workspace/pkg/policy.MODULE.bazel`; and
- `workspace/alternate/pkg/policy.MODULE.bazel`.

No harness edit, registry scaffold, checked output, WORKSPACE file, or
absolute special-file symlink is authorized. Fixture mutations may create
and remove the package-root BUILD markers, `.bazelignore`, `REPO.bazel`, and
contained vendor directory needed by a row; they must restore each prior
state before the next independent semantic transition.

Retain exact argv and rows for: missing package rejects the complete
root/policy print namespace; workspace fallback BUILD recovers; one exact
`--package_path=%workspace%:%workspace%/alternate` row where the first
root has only `pkg/BUILD` and the alternate has `pkg/BUILD.bazel` selects the
first root, proving package-root order outranks basename priority; removing
the first marker selects the alternate root; `BUILD.bazel` wins over `BUILD`
within one root; unqualified and `//`
deleted-package spellings are identical; canonical-main spelling is
identical while a nonmain literal repository is distinct; `//external`
cannot become a package; a contained vendor directory is ignored while an
absolute outside-workspace vendor directory is not; `.bazelignore` is a
literal prefix from the first package root containing it; REPO
explicit `repo(k = v, ...)` arguments may precede `ignore_directories()`,
while source-level `repo(**{...})` is rejected; duplicate/call-order errors
retain exact failure diagnostics; `*`, `?`, and `**` prefix matching reject before
any module print; REPO print is cold/change-only and warm-nonreplaying;
editing/deleting every policy source recovers; all same-horizon packages
preflight before any include event; and diagnostics retain the include label
even when the selected bytes live below the alternate package root.

If Bazel's available flags cannot express one row without a harness change,
stop and retain that row as pinned-source plus later observation-backed Rust
evidence; do not weaken package-root, REPO, event, or matcher semantics.
After oracle acceptance, run the focused fixture-growth inventory required
by AGENTS.md before packet 1. Record one compact baseline/result in the
oracle-harness owner plan; do not prune self-contained provenance or update
both routing history files.

#### Serial packet 1: bootstrap domain request and transient carrier

Run only `WP-5-m1-root-module-bootstrap-request-owner`. Edit exactly:

- new `app/slug_bzlmod_v2/src/root_bootstrap.rs`;
- `app/slug_bzlmod_v2/src/source_preparation.rs`;
- `app/slug_bzlmod_v2/src/lib.rs`; and
- `app/slug_bzlmod_v2/tests/source_preparation_dice.rs`.

The public bzlmod domain seam owns
`RootModuleBootstrapRequest { workspace: NormalizedAbsolutePath }`,
`RootModuleBootstrapApplyResult::{AlreadyPresent, Created(
RootModuleBootstrapWarning)}`,
the typed create error with normalized module path plus
`PathIoErrorKind`/raw OS code, the exact warning text, and the exact reminder
bytes/hash. The request derives only
`workspace.join("MODULE.bazel")`; no physical resolution, IO, generation,
event, retry, DICE key, or terminal policy enters these types.

Add one optional root-bootstrap request to `SourcePreparationNeeds`.
Identical workspace requests deduplicate; different workspaces produce
`ConflictingRootModuleBootstrap`. `try_union` remains cumulative with path
and repository needs. Every carrier containing the request remains invalid
and self-unequal through `SourcePreparationOutcome`; root creation is never
encoded as `NeedPathObservations` or a terminal error.

Tests pin request identity/path, exact 399 bytes/hash/warning, duplicate and
conflicting union, union with both existing need kinds, and nonvalidating
Need equality. No producer or core consumer is authorized.

#### Serial packet 2: dormant outside-DICE native bootstrap owner

Run only `WP-5-m1-root-module-bootstrap-native-owner`. Edit exactly:

- new `app/slug_core_v2/src/runtime/root_bootstrap.rs`; and
- `app/slug_core_v2/src/runtime/mod.rs`.

The runtime-private `RootModuleBootstrapOwner` is constructed for one
normalized workspace and applies only a matching bzlmod request outside
DICE. It performs Bazel's exact non-atomic algorithm: if the logical
`MODULE.bazel` currently exists, return `AlreadyPresent` without reading or
writing it; otherwise `std::fs::write` the exact reminder to the logical
path and return `Created(warning)`. The successful write result is Created
even if another writer entered Bazel's exists/write race window. Do not use
`create_new`, canonicalization, a temporary file, rename, a lock, or an
atomic replacement. Following an existing or dangling symlink is therefore
preserved rather than replaced.

Inline real-filesystem tests pin create, warm no-overwrite, edit
preservation, delete/recreate, exact bytes, a deterministic file-as-parent
create failure, foreign workspace, and on Unix existing and dangling symlink
behavior. The owner has zero DICE, demand, epoch, event, output, print, or
production call sites. Packet 2 does not activate warning publication.

#### Serial packet 3: complete root package-policy request input

Run only `WP-5-m1-root-package-policy-input-owner`. Edit exactly:

- `app/slug_identity_v2/src/package.rs`;
- `app/slug_identity_v2/tests/label_roundtrip.rs`;
- new `app/slug_bzlmod_v2/src/package_policy.rs`;
- `app/slug_bzlmod_v2/src/lib.rs`; and
- `app/slug_bzlmod_v2/tests/dice_inputs.rs`.

Add `PackageIdentifier::parse_bazel_package_identifier`, matching Bazel
`PackageIdentifier.parse`: reject targets; accept unqualified, `//`,
single-`@`, and double-`@@` repository spellings; make all main spellings
one canonical identity; and treat a single-`@` nonmain name as that literal
repository identity without repository mapping.

The injected value is one exact normalized workspace plus:

- ordered `Arc<[NormalizedAbsolutePath]>` package roots, including a valid
  empty list and without silently defaulting to the workspace;
- a compact canonical deleted-package `SmallSet<PackageIdentifier>`;
- an optional normalized absolute vendor directory; and
- `RootRepoFileSemantics { utf8_mode:
  RootRepoFileUtf8Mode::{Off, Warning, Error} }`, the exact sole Bazel-9
  parsing option consumed by the later REPO evaluator. Its default is
  `Warning`, matching `BuildLanguageOptions`; request normalization maps
  `off`/`warning`/`error` and boolean shorthand exactly; and the enum
  participates in structural equality and A→B→A replay.

Packet 3 accepts already normalized package-root and vendor paths; it does not
own raw `--package_path` strings, `%workspace%`, client cwd, relative-entry
warnings, or filesystem existence. It expands repeated/comma-separated
deleted-package occurrences, deduplicates structurally, and preserves the
supplied normalized root order and absolute outside-workspace vendor path.
Packet 9's outside-DICE command preflight later owns raw spelling,
`%workspace%`/cwd resolution, Bazel's relative-entry warning, and
command-boundary existence filtering before it constructs this value. A
private injected key is identified by workspace; a public injection helper
supplies the exact value. Missing injection fails closed.

Freeze three opaque projection keys from that injected value:
`RootRepoFileSemanticsProjectionKey(workspace)` returns only
`RootRepoFileSemantics`; `RootRepositoryIgnoreInputsProjectionKey(workspace)`
returns only ordered package roots plus vendor; and
`RootPackageLookupInputsProjectionKey(workspace)` returns only ordered roots
plus canonical deleted packages. Each projection computes the injected owner
itself, compares every projected field, and fails closed on missing input.
Thus a deleted-package-only change cannot replay REPO evaluation/events, a
semantics-only change cannot perturb package-root probing, and a vendor-only
change cannot replay REPO evaluation.
Do not add these fields to `BzlmodCommandPolicyKey` or
`RootModuleCommandPolicy`, and do not expose `--package_path`,
`--deleted_packages`, or `--vendor_dir` through command/server/CLI yet.

Tests pin Bazel spellings and target rejection, normalized root ordering and
empty roots, canonical deleted identity and A→B→A DICE replay,
contained/outside absolute vendor identity, exact semantics default/mode
inequality/replay, and missing-input failure. No raw-option parser, core edit,
or default injection is authorized.

#### Serial packet 3a: neutral diagnostic-event extension

Before the REPO evaluator, run only
`WP-5-m1-neutral-diagnostic-event-contract`. Edit exactly
`app/slug_events_v2/src/lib.rs`.

Add `EvaluationDiagnosticLevel::{Warning, Error}` and
`EvaluationEvent::Diagnostic { level, text: CompactString }` beside the
accepted Starlark-print variant. Preserve ordinary event Clone, batch-only
Dupe, structural ordered equality, and dependency-bottom ownership. Tests
pin level inequality, exact UTF-8/newline text, mixed print/diagnostic order,
and shared batch storage. Add no producer, DICE, policy, output, retry, or
publication edge. This neutral variant is required both for REPO parser/eval
diagnostics and for packet 9's later command-owned synthetic bootstrap
warning; it does not make either event DICE-reachable by itself.

#### Serial packet 4: ignored-subdirectory matcher/value owner

Run only `WP-5-m1-repository-ignore-matcher-owner`. Edit exactly:

- new `app/slug_bzlmod_v2/src/repository_ignore.rs`; and
- `app/slug_bzlmod_v2/src/lib.rs`.

The crate-private compact value owns sorted/deduplicated literal
repository-relative prefixes, ordered REPO patterns with precompiled compact
segments, and `matching_entry(&PackagePath)`. Prefix matching is
component-aware. Port Bazel 9 `UnixGlob.matchesPrefix`, including `**`,
segment `*`/`?`, leading-dot rules, regex-character escaping, and parenthesis
behavior, with an iterative or bounded-DP implementation; add no `globset`, regex dependency, loading
dependency, standard retained hash map, IO, DICE key, or events.

Tests port the discriminating Bazel table for exact/prefix/child-depth,
zero-or-more `**`, mixed wildcards, escaping/parentheses, ordering, and
literal-prefix component boundaries. Semantic equality includes prefixes and
ordered original patterns, not matcher scratch state.

#### Serial packet 5: private Host REPO evaluator and ignore key

Run only `WP-5-m1-host-repository-ignore-owner`. Edit exactly:

- new `app/slug_bzlmod_v2/src/repo_file.rs`;
- `app/slug_bzlmod_v2/src/repository_ignore.rs`; and
- `app/slug_bzlmod_v2/src/lib.rs`.

`HostRepoFileKey(workspace)` directly computes packet 3's semantics
projection, then reads workspace-root `REPO.bazel` through
`HostFileBytesKey`. Missing is the empty value. Present regular or special
bytes are parsed and evaluated in the full Bazel no-load REPO environment
needed here: the dot-bazel dialect rejects load/def/lambda/top-level
for/if and source-level `*`/`**` argument expansion; `repo(k = v, ...)`
accepts arbitrary explicit keyword values, requires at least one, is
once-only, and precedes any
`ignore_directories()` call; `ignore_directories()` accepts exactly one
string list/tuple and is once-only. Preserve Bazel's exact call-order and
cardinality diagnostics, including its source spelling where upstream has a
diagnostic typo. REPO parsing implements the projected modes exactly:
Off parses Bazel's Latin-1 byte projection without validation; Warning emits
the exact invalid-UTF-8 diagnostic then parses that same projection; Error
emits the exact diagnostic and returns the typed invalid-UTF-8 failure.
UTF-8, syntax, compile, evaluation, Host observation, and compute failures
remain distinct typed variants.

REPO Starlark prints and packet 3a parser/evaluation diagnostics follow the
accepted producer policy and remain outside semantic value equality. With
`CaptureEvaluationEvents` present, cold/change computes attach their exact
interleaved ordered batch to DICE evaluation data and unchanged graph/cache
reuse emits nothing. With the marker absent, the evaluator installs the
normal direct Starlark printer/diagnostic reporter and attaches no batch. It
never both prints and captures, suppresses marker-off output, or reconstructs
events in a caller.

`HostRepositoryIgnoreKey(workspace)` directly computes packet 3's
roots/vendor projection and the REPO key, then for every ordered package root
adds a vendor prefix only when the absolute vendor directory is beneath that
root. It probes `.bazelignore` roots strictly sequentially through
`HostFileBytesKey`: a later root is not computed or observed until every
earlier candidate is Complete Missing or a resolved Directory; the first
RegularFile/SpecialFile, including a resolved symlink target, is read and
stops the search. Other Host failures remain typed. As Bazel does through
`InputStreamReader(UTF_8)`, malformed bytes decode with replacement rather
than a UTF-8 error. Nonempty noncomment decoded lines are literal prefixes;
absolute/invalid path lines are typed errors. It composes those prefixes with
the ordered REPO patterns from packet 4.

Both values are
`PathOutcome<Arc<Result<_, typed error>>>`; Need is propagated, invalid, and
self-unequal. Tests pin missing/create/edit/delete/restore for both files;
strict sequential first-file `.bazelignore` including no later-root demands;
replacement decoding; contained/outside vendor across roots;
regular/special/symlink sources; exact typed syntax/call/runtime/path errors;
explicit REPO kwargs coexistence and `**` rejection; the wildcard table;
Need behavior; and marker-off direct versus marker-on cold,
warm-nonreplay, all three UTF-8 modes, syntax/evaluation failure order,
change, and A→B→A event behavior. Add no direct IO,
legacy workspace key, loading glob, Cargo dependency, or public Host key.

##### Stage 5 Host REPO evaluator and ignore-key first contract

**Status:** Replanned before Rust on 2026-07-25.

The pinned Bazel 9.2 source audit found three material mismatches in the
packet above, so no Rust, fixture, formatting, or Cargo command was started.
First, `IgnoredSubdirectoriesFunction.computeIgnoredPrefixes` contributes a
contained vendor prefix for the current visited root immediately before
probing that root's `.bazelignore`, and the first file terminates both later
probes and later vendor contributions; it does not add vendor prefixes for
every configured root in advance. Second, `.bazelignore` uses
`PathFragment.create(line)` normalization followed by native `Path.of(line)`
validation, so its semantic prefix domain includes an empty prefix and
surviving leading `..` components that `PackagePath` cannot represent.
Third, `ignore_directories()` accepts Bazel's exact `Sequence<String>`
implementations—list, tuple, and range—not only list/tuple. The correction
must also freeze the audited REPO globals, diagnostic ordering, Latin-1
projection, native line/path behavior, and REPO-before-roots demand order
before the same exact three-file implementation scope is retried.

##### Corrected retry contract: Host REPO evaluator and repository-ignore key

Run only `WP-5-m1-host-repository-ignore-owner-corrected-retry`. Edit exactly:

- new `app/slug_bzlmod_v2/src/repo_file.rs`;
- `app/slug_bzlmod_v2/src/repository_ignore.rs`; and
- `app/slug_bzlmod_v2/src/lib.rs`.

`HostRepoFileKey { workspace: NormalizedAbsolutePath }` first computes
`RootRepoFileSemanticsProjectionKey`, then reads
`workspace/REPO.bazel` through `HostFileBytesKey`. Missing produces no
patterns and no events. Its private complete value retains only the ordered,
duplicate-preserving `Arc<[CompactString]>` ignore patterns. Evaluated
`repo()` keyword values are deliberately discarded from this projection;
the later package-argument owner must extend this same evaluator with a typed
snapshot rather than create a second evaluator or retain debug `repr`.

Every successful mode parses a byte-for-byte Latin-1 scalar projection,
including input that is valid UTF-8. Invalid UTF-8 in Off emits nothing;
Warning reports
`not a valid UTF-8 encoded file; this can lead to inconsistent behavior and will be disallowed in a future version of Bazel`
before parsing that Latin-1 projection; Error reports that text plus
`. For a temporary workaround, see the --incompatible_enforce_starlark_utf8 flag.`
and returns typed `InvalidUtf8` without parsing or printing. UTF diagnostics,
syntax errors, restricted-syntax errors, compile/preparation errors,
evaluation errors, policy-projection failures, and `HostFileError` remain
distinct typed variants with the logical path retained where applicable.

Parse permissively, then run a custom source-order AST checker with
`where = "REPO.bazel files"`. It reports Bazel's exact messages for `load`,
`def`/lambda, statement `for`, statement `if`, and nested source-level
`*args` and `**kwargs`; nonliteral `**kwargs` adds Bazel's second literal-dict
diagnostic. Rejected def/lambda/for/if children are not revisited.
Comprehensions and conditional expressions remain allowed. The globals heap
contains exactly Bazel's REPO
globals `repo` and `ignore_directories` plus
`False`, `True`, `None`, `min`, `max`, `abs`, `all`, `any`, `sorted`,
`reversed`, `tuple`, `list`, `len`, `str`, `repr`, `bool`, `float`, `int`,
`dict`, `set`, `enumerate`, `hash`, `range`, `hasattr`, `getattr`, `dir`,
`fail`, `print`, `type`, and `zip`; `set` retains Bazel's flag-disabled
failure. `native`, `struct`, package/module/depset/select/rule globals, and
loads are absent.

`repo()` evaluates arbitrary explicit keyword values, rejects positional
arguments, requires at least one argument, is once-only, and, when present,
must precede any `ignore_directories()` call; calls to universal globals such
as `print` do not establish that ordering guard. `ignore_directories()` takes
exactly one `dirs` argument, is once-only, and admits exactly the Java
`Sequence<String>` counterparts provided by starlark-rust: list, tuple, or
`starlark::values::range::Range`. It then uses normal value iteration and
requires every element to be a string. A string, dict, set, or other merely
iterable value is not admitted. Preserve exact Bazel call arity/type/order
messages, including `'ignored_directories()' can only be called once`.
Patterns are not validated, normalized, sorted, or deduplicated.

A private `RepoEventReporter` is the single ordered seam for Starlark prints
and diagnostics. The pure evaluator accepts that reporter, production uses a
direct reporter, and tests use a recording reporter. With
`CaptureEvaluationEvents`, every Complete cold/change compute stores exactly
one ordered `EventBatch`, including an empty batch and batches ending in a
failure; graph reuse emits nothing. Without the marker, output goes only to
the normal direct print/diagnostic channels and no batch is stored. Need
stores no batch, and no event enters semantic equality.

Before adding the Host key, change `RepositoryIgnoreMatcher`'s private literal
storage from `Arc<[PackagePath]>` to a private compact normalized-slash prefix
type that can represent the empty prefix and surviving leading `..`
components. Construction still sorts and deduplicates prefixes; empty matches
every package, ordinary prefixes are component-aware, and a leading-up-level
prefix cannot match a normalized `PackagePath`. Keep
`matching_entry(&PackagePath)` and the exact ordered original REPO-pattern
semantics accepted in packet 4. Semantic equality includes normalized
prefixes and ordered original patterns, never compiled scratch state.

`HostRepositoryIgnoreKey { workspace }` first computes
`HostRepoFileKey`; a REPO Need or failure requests neither root inputs nor
`.bazelignore`. It then computes `RootRepositoryIgnoreInputsProjectionKey`
and visits package roots sequentially. For each root it first adds the vendor
path relative to that root when the vendor is component-contained, including
vendor equal to root as the empty prefix, then computes only that root's
`HostFileBytesKey(root/.bazelignore)`. Missing or Directory continues;
RegularFile or SpecialFile, including a resolved symlink target, parses and
terminates the entire loop. Later roots contribute neither demands nor vendor
prefixes after that first file. Observation/resolution failures and Need
terminate immediately.

`.bazelignore` parsing decodes UTF-8 with replacement and splits exactly like
Java `BufferedReader.readLine` across LF, CRLF, and bare CR. Skip only empty
lines and lines whose first scalar is `#`; do not trim, remove BOM, or treat
later `#` as a comment. For every retained line, first compute Bazel
`PathFragment.create(line)` normalization and perform native `Path.of(line)`
validation while reading the complete file. This collapses repeated
separators, trailing separators, `.`, and reducible `..`, while preserving
leading `..`; `.` and `a/..` normalize to the empty prefix. Native-invalid
lines win in read order even when an earlier normalized line is absolute.
After the read, reject the first unique normalized absolute prefix. Unix
treats backslash and drive-looking text as relative literals and rejects NUL;
Windows treats backslash as a separator and rejects rooted/drive-absolute and
native-invalid forms. Retain exact typed diagnostics:
`Invalid path in <file>: '<normalized>': cannot be an absolute path` or
`Invalid path in <file>: <native invalid-path message>`.

Both keys return `PathOutcome<Arc<Result<_, typed error>>>`; Need is invalid
and self-unequal, while Complete equality is semantic. Tests add the narrow
regression first and pin:

- missing/create/edit/delete/restore and A→B→A for REPO and `.bazelignore`;
- every audited REPO global boundary, exact arity/type/order/typo diagnostic,
  list/tuple/range admission, invalid sequence elements, arbitrary discarded
  repo kwargs, nested expansion diagnostics, comprehensions, and absent
  globals;
- byte-for-byte Latin-1 parsing for valid and invalid UTF-8, all three modes,
  warning/error-before-syntax behavior, prints before runtime failure, and
  marker-off direct versus marker-on cold/warm/change/failure ordering;
- Java line splitting, replacement decoding, comment/BOM/whitespace behavior,
  normalization/deduplication, empty and leading-up-level prefixes, Unix and
  pure Windows path tables, native-invalid-before-absolute precedence, and
  exact errors;
- REPO-before-roots demand order; current-root vendor-before-probe ordering;
  first-file termination of later probes and vendor contributions; inclusive,
  contained, outside, and multiple-root vendor cases; regular, special,
  directory, missing, and resolved-symlink candidates; and every Need/failure
  cut point; and
- the accepted wildcard matcher table, full semantic equality/pruning, key
  validity, private surface, compact Arc-backed storage, and exact file scope.

Run the focused new tests, then full `slug_bzlmod_v2`,
`slug_loading_v2`, and `slug_core_v2` unit/integration/doctests serially.
Rebuild and link every affected GNU-Windows no-run test executable. Run
formatting, diff/archive, exact three-file allowlist, no-new-dependency,
private-surface, no-standard-retained-map, bounded-scratch, no-filesystem-IO,
and no-activation/package-lookup gates. Direct reporter output is authorized;
filesystem IO inside DICE, a public Host key/reexport, Cargo dependency,
legacy workspace key, loading glob, command activation, or package lookup is
not.

##### Corrected retry contract status

**Status:** Replanned before Rust on 2026-07-25.

Pinned source/semantic and architecture reviews accepted the corrected REPO,
root-order, prefix-domain, event, and equality contract, but the live
implementation-feasibility review found one material unowned Windows edge.
Bazel's `WindowsOsPathPolicy` may expand 8.3-looking segments through
`WindowsPathOperations.getLongPath` while constructing the normalized
`PathFragment` for a `.bazelignore` line. A pure lexical Windows helper with
short-path expansion disabled would therefore diverge when the alias exists,
while a direct native call inside the DICE key would violate the accepted
observation boundary. No Rust, fixture, formatting, or Cargo command started.
Next design evidence must pin reachability and failures, then add only the
smallest injected Host observation prerequisite before retrying the otherwise
accepted three-file owner.

##### Windows long-path observation prerequisite design

Run only `WP-5-m1-windows-long-path-observation-owner`. Edit exactly:

- `app/slug_workspace_v2/src/path_observation.rs`;
- `app/slug_workspace_v2/src/path_resolution.rs`;
- `app/slug_core_v2/src/runtime/path_observation.rs`;
- `app/slug_core_v2/src/runtime/repository_io.rs`;
- `app/slug_bzlmod_v2/src/host_file.rs`; and
- `app/slug_bzlmod_v2/src/source_preparation.rs`.

This is a producer-free observation prerequisite. Add
`PathObservationOperation::WindowsLongPath` and
`PathObservationResult::WindowsLongPath(Arc<[u16]>)`. Extend
`PathObservationDemand` with one private optional lossless
`Arc<[u16]>` Windows resolver input and a dedicated Host-only constructor. Its
ordinary `NormalizedAbsolutePath` remains the lexically normalized absolute
identity used by the existing demand/authority machinery, while the extra
input retains the exact pre-normalization `.bazelignore` line as UTF-16 code
units, including slash choice, repeated/trailing separators, `.` and `..`.
The generic constructor cannot construct `WindowsLongPath`, and the dedicated
constructor cannot construct another operation.

The result is always Bazel's final Windows lexical normalization after the
resolver step: normalize either the successful long spelling after exact
prefix/slash postprocessing or the unchanged lossless input on resolver
failure. Port `WindowsOsPathPolicy.normalize` and
`Utils.removeRelativePaths` over UTF-16 units, including slash/repeated/
trailing normalization, dot/up-level reduction, ASCII drive uppercasing, and
absolute-root behavior. This intentionally erases the mechanism:
`WindowsOsPathPolicy.DefaultShortPathResolver` catches every `IOException` and
retains the original spelling before the shared lexical step. Failure and an
identity-success that differ only by slash choice, repeated/trailing
separators, or reducible dot components must therefore remain clean; only a
changed final normalized UTF-16 spelling is dirty. Arc-backed unit storage
preserves `Dupe`, semantic equality, and Windows filenames containing unpaired
surrogate units. Operation/result identity, input-sensitive demand ordering,
epoch construction, DICE complete equality, and Need invalidity remain exact.

Extend the existing outside-DICE observation kernel with one direct,
unrefined dispatch on the lossless input. First port exact
`WindowsPathOperations.asLongPath`: retain an existing `\\?\` prefix,
otherwise prepend it, and convert `/` to `\`. Port Bazel native
`IsAbsoluteNormalizedWindowsPath` eligibility before calling the API, so raw
dot/up-level input falls back even when its later lexical form would be
eligible. On Windows, follow the existing local kernel32 FFI owner and call
`GetLongPathNameW` with Bazel's exact one sizing call and one fill call—no
growth retry. Preserve non-BMP UTF-16 and safely treat a zero,
growth-overflowing, or unterminated fill as failure rather than reproducing
Bazel native undefined behavior. Check sizing conversion and allocation
against the documented Windows extended-path bound; an oversized size falls
back without allocation. Preserve every returned UTF-16 unit, including an
unpaired surrogate, exactly as JNI `NewString` does. A valid fill with
`0 < written < capacity` and a terminator at `written` succeeds
even when a concurrent shortening makes `written` smaller than the sizing
result; the bounded Java UTF-16 code-unit sequence is not converted through
Rust Unicode scalars. On success, port
`removeUncPrefixAndUseSlashes`: remove the first four scalars only for
`\\?\` or `\??\`, then replace `\` with `/`. Every failure returns the
unchanged input, and both paths then pass through the same lexical normalizer.
Do not call `canonicalize`, enumerate directories, inspect metadata, follow
symlinks, add a native handle owner, or add a Cargo dependency. On non-Windows
the adapter lexically normalizes the unchanged input; production is not
authorized to request this operation there.

Repository validation compares the effective spelling directly: stable equal
output remains clean regardless of whether native resolution failed or
identity-succeeded; only changed final UTF-16 spelling is dirty. Add explicit
unreachable compile-closure arms to the existing Lstat/ReadLink/FileBytes/
DirectoryEntries consumers in the five non-owner sites. Do not reinterpret
the operation as ReadLink, DirectoryEntries, Lstat, or FileBytes: none carries
alternate 8.3 spelling, and reuse would violate operation/result identity.

Tests pin operation/result pairing and mismatch rejection, demand
ordering/equality, exact one-call dispatch without Lstat refinement,
Need invalidity/self-inequality, failure/identity-success equality, changed
effective output, A→B→A replay, and repository-validation clean/dirty
transitions. The pure Windows helper pins the exact 8.3 grammar,
pre-normalization eligibility, `asLongPath`, dot/up-level, repeated and
trailing separators, one sizing/fill attempt, one-call race safety,
valid shortening success, oversized sizing and growth/unterminated fallback,
non-BMP UTF-16, both removable prefix forms, slash conversion,
unpaired-surrogate preservation, exact Windows lexical normalization,
success, and every native failure falling back through the same normalizer.
A native Windows
smoke proves the imported
`GetLongPathNameW` ABI on an existing ordinary long absolute path and a
missing path. When the worker supports creation of a distinct NTFS 8.3 alias,
also prove that alias expands to the long spelling; inability to create such
an alias is recorded separately and must not be reported as passing that row.

Run focused tests first, then full `slug_workspace_v2`, `slug_core_v2`,
`slug_bzlmod_v2`, and `slug_loading_v2` unit/integration/doctests serially.
Build and link every affected GNU-Windows no-run test executable. Run
formatting, diff/archive, exact six-file allowlist, no-new-dependency,
operation/result exhaustiveness, compact-result, no-DICE-IO, and no-producer
gates. Add no bzlmod consumer, public convenience reexport, demand-loop
special case, command activation, or package lookup.

After this prerequisite reaches terminal `ACCEPT`, retry only the existing
three-file Host REPO/repository-ignore contract with these additions:

- port Bazel's exact 8.3 candidate predicate;
- make no demand for relative, drive-relative, or native-ineligible input;
- after the complete-file native-validation scan, sequentially compute
  the dedicated `WindowsLongPath` Host demand with both the lexically
  normalized absolute identity and the lossless original input for eligible
  Windows absolute lines;
- propagate an absent observation as Need before continuing the file, then
  use the final normalized UTF-16 spelling before deduplication and absolute
  rejection; retain that lossless spelling through typed error storage and
  apply Bazel-compatible replacement only at the UTF-8 diagnostic-output
  boundary;
- preserve native-invalid-before-first-absolute precedence, sequential
  multiple-line demands, graph reuse, and changed diagnostic replay; and
- test relative no-demand, absolute Need/failure/success, dot/up-level input
  that must not be resolved after reduction, names resolving to themselves,
  backslash-failure→slash identity-success nonreplay, repeated/trailing
  normalization, unpaired-surrogate output formatting, short/long
  deduplication, first-unique-absolute selection, and exact expanded-path
  diagnostics.

**Status:** Accepted for implementation on 2026-07-25.

The pinned Bazel source, live Rust feasibility, and architecture/DICE
reviewers all accepted the latest six-file contract. Corrections replaced an
early optional normalized-path result with lossless pre-normalization
`Arc<[u16]>` demand input and final normalized `Arc<[u16]>` output; erased
resolver mechanism from semantic equality; preserved unpaired surrogate
units; froze exact `asLongPath`, native eligibility, shared lexical
normalization, and one sizing/one fill behavior; and bounded unsafe allocation
and fill races without changing valid shortening success. No Rust, fixture,
formatting, or Cargo command ran in the design packet.

##### Corrected retry implementation status

**Status:** Accepted on 2026-07-25.

The exact three-file packet adds the private Host REPO evaluator and the
repository-ignore key. It preserves Bazel 9.2.0's exact REPO globals,
Latin-1/UTF behavior, restricted-syntax locations and traversal, binder
diagnostics, ordered duplicate patterns, replacement-decoded
`.bazelignore`, native Unix/Windows path validation, lossless Windows 8.3
demands, current-root vendor ordering, first-file termination, transient
Needs, semantic equality, and marker-conditional event batches.

Review corrections removed the extra `chr`/`ord` globals, restored exact
tag-era AST visitation, source-ordered argument binding and spelling
suggestions, grouped malformed UTF-8 like Java's decoder, ported the relevant
OpenJDK Windows parser branches, retained source locations in diagnostics,
and admitted already-verbatim `\\?\C:` short-path candidates. Retained-DICE
tests prove cold/change/failure/empty/A-to-B-to-A batches, warm nonreplay,
REPO-before-roots, sequential root Needs, current-root vendor inclusion, and
later-root termination.

Validation passed 52 focused/unit tests and 266 total bzlmod tests, 54 loading
tests, 102 core unit plus 13 integration tests, and zero doctests. All 12
bzlmod, six loading, and two core GNU-Windows test executables linked.
Formatting, diff, archive, exact-file, privacy, dependency, no-direct-IO,
no-activation, and compact-storage gates passed. Three terminal latest-diff
reviews returned `ACCEPT`, including a source audit against exact Bazel 9.2.0
commit `8220c6198837d5c13d53fea211cf3282aa12408a`.

Next evidence: Implement only
`WP-5-m1-host-root-package-lookup-owner`.

#### Serial packet 6: private Host main-package lookup

Run only `WP-5-m1-host-root-package-lookup-owner`. Edit exactly:

- new `app/slug_bzlmod_v2/src/host_package.rs`; and
- `app/slug_bzlmod_v2/src/lib.rs`.

`HostRootPackageLookupKey { workspace, package: PackagePath }` first
computes packet 3's package-lookup projection, validates the exact Bazel
package-name domain and retains `InvalidPackageName` plus its diagnostic,
checks the canonical main `PackageIdentifier`, returns Deleted for configured
deletion, returns NoBuildFile for reserved `external`, completes packet 5,
and returns Deleted for an ignore match. It then iterates ordered package
roots outside `BUILD.bazel` then `BUILD` priority. Each candidate uses
`ResolvedPathKey(Host, normalized root/package/basename)`, never directory
listing or bytes. RegularFile and SpecialFile are files; Directory and
Missing fall through; symlinks use their resolved terminal kind.

Success retains the selected normalized package-path entry root and
`HostBuildFileName::{BuildDotBazel, Build}`. NoBuildFile and Deleted are
distinct. Physical resolved path, route, metadata, marker contents, and
package-policy diagnostic match are outside successful equality.
Resolution, observation, cycle, expansion, input, and ignore failures remain
typed; Need never enters an error.

Tests pin root/build priority, empty roots, create/delete/restore, metadata
pruning, symlink and special markers, deletion-before-ignore/build,
`external`, contained vendor and both ignore sources, typed failures, and
Need invalid/self-unequal. No public key/reexport, include label, or include
bytes is authorized.

##### Implementation status

**Status:** Accepted on 2026-07-25.

The exact two-file packet adds the private Host root-package lookup key. It
computes the projected package inputs first, preserves exact package-name,
canonical-main deletion, reserved `external`, and repository-ignore
precedence, then probes ordered package roots with `BUILD.bazel` before
`BUILD` through `ResolvedPathKey(Host, ...)` only. Success retains the
selected package-path entry root and basename; resolved physical identity,
route, and marker metadata do not enter successful equality.

The Bazel 9.2.0 source audit corrected two owner-contract gaps before
acceptance: exact invalid-character/all-dot `InvalidPackageName` values occur
before deletion, and successful lookup retains the package-path entry rather
than `root/package`. Tests additionally pin root-over-basename priority,
directory fallthrough, main/nonmain deletion, both ignore sources, contained
vendor, special and symlink terminals, all four typed resolver failures,
transient Needs, and retained missing/create/metadata/delete/restore pruning.

Validation passed 11 focused tests, 63 bzlmod unit and 214 integration tests,
54 loading tests, 102 core unit plus 13 integration tests, and zero doctests.
All 12 bzlmod, six loading, and two core GNU-Windows test executables linked.
Formatting, diff, archive, exact-file, privacy, no-direct-IO, no-bytes,
no-activation, and dependency gates passed. Three terminal latest-diff
reviews returned `ACCEPT`, including the source audit against exact Bazel
9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`.

Next evidence: Implement only
`WP-5-m1-host-root-include-horizon-owner`.

#### Serial packet 7: root include-label and horizon owner

Run only `WP-5-m1-host-root-include-horizon-owner`. Edit exactly:

- new `app/slug_bzlmod_v2/src/host_include.rs`;
- `app/slug_bzlmod_v2/src/module_eval.rs`; and
- `app/slug_bzlmod_v2/src/lib.rs`.

`module_eval.rs` adds only a crate-private parsed root include seam retaining
canonical main package, target, raw label, and logical diagnostic span.
Packet 7 validates the repo-relative label and `.MODULE.bazel`/non-dot
basename, derives package-key identity without raw label/span, and after a
successful package lookup derives the normalized Host read path from the
selected logical package root plus the label's package/target. The logical
module-file identity and diagnostics remain label-derived; never recover
them by stripping a physical resolved path.

The horizon adapter deduplicates every same-level package key, computes all
keys before any include bytes, then inspects their outcomes in original
source order. An earlier completed missing/deleted/error beats a later
unresolved package; an earlier unresolved package returns the union of every
unresolved package Need already requested by the group. Its success retains
original include order and selected logical read paths. Tests pin label
validation and target canonicalization, dedupe, full grouped Need union, both
terminal-before-Need and Need-before-terminal precedence directions,
first-source terminal failure, alternate-root logical selection, and a
downstream counter proving zero include lstat/bytes demands until the whole
horizon completes. Packet 7 may not call `HostFileBytesKey`.

##### First implementation attempt status

**Status:** Replanned on 2026-07-25 with no source retained.

The exact Bazel 9.2.0 audit found that the accepted packet contract inverted
one mixed horizon precedence and omitted two target parser boundaries.
`ModuleFileFunction.advanceHorizon` requests the whole deduplicated package
group, then scans results in source order: an earlier completed terminal
result beats a later unresolved result. The prior contract instead made any
Need dominate every terminal. Bazel also rejects an explicit empty target
such as `//pkg:` and canonicalizes a target ending in `/.` before applying the
`.MODULE.bazel` and non-dot-basename checks.

The provisional three-file Rust draft was removed before retention. No
fixture, dependency, public API, include-byte path, or activation changed.

Next evidence: Design only
`WP-5-m1-host-root-include-horizon-design-correction`.

#### Serial packet 7 correction: exact label and grouped-horizon precedence

Run only `WP-5-m1-host-root-include-horizon-design-correction`. Do not edit
Rust or fixtures and do not run Cargo.

Freeze a corrected retry contract with the same eventual three-file
allowlist. The parsed seam must reject explicit-empty targets, canonicalize a
valid trailing `/.`, and apply `.MODULE.bazel`/non-dot checks and logical path
derivation to the canonical target while retaining raw label and span only
for diagnostics. Parse every label in source order before requesting any
package key, return the first invalid label, and require the raw spelling to
start exactly `//`; `@//`, `@@//`, and `@repo//` reject. A no-colon
`//pkg/file.MODULE.bazel` uses package `pkg/file.MODULE.bazel` and default
target `file.MODULE.bazel`, so its path deliberately repeats that basename.
Suffix and non-dot checks use the basename after the last slash of the
canonical target; target subdirectories otherwise remain valid.

Package-key identity is canonical-main package only: target, raw label, and
span do not participate, so same-package targets deduplicate. The horizon
must request all first-seen unique package keys through one DICE computation
group before interpreting any outcome.

After the group returns, rewalk original includes in source order. If a
completed terminal occurs before the first unresolved package, return that
terminal. If an unresolved package occurs first, return the union of every
unresolved package Need from the already-requested group. Only complete
success reaches selected package-path entry root plus package plus canonical
target derivation, retaining every original include entry in source order.
Keep semantic NoBuildFile/Deleted/Invalid failures and typed operational
lookup errors distinct. Require discriminating tests for parse-all-before-
lookup, exact raw-prefix/default-target/canonical-target boundaries, both
mixed-order directions, grouped dedupe/Need union, first-source terminals,
alternate roots, and zero include observations before a successful whole
horizon.

##### Corrected design status

**Status:** Accepted on 2026-07-25.

The corrected retry retains the eventual three-file allowlist and freezes
parse-all-before-lookup, exact raw `//` spelling, explicit-empty rejection,
colonless default-target behavior, trailing-`/.` canonicalization, canonical
basename checks, package-only first-seen dedupe, one `compute_join` group,
both mixed terminal/Need source-order directions, full grouped Need union,
typed semantic versus operational failures, and selected-root plus package
plus canonical-target paths. Include observations and bytes remain gated
until the whole package horizon succeeds.

Three terminal reviews returned `ACCEPT`, including an exact Bazel 9.2.0
source audit against commit
`8220c6198837d5c13d53fea211cf3282aa12408a`. No Rust, fixture, Cargo,
dependency, public API, byte path, or activation changed in the correction
packet.

Next evidence: Implement only
`WP-5-m1-host-root-include-horizon-owner-corrected-retry`.

##### Corrected retry implementation status

**Status:** Accepted on 2026-07-26.

The corrected three-file owner parses the complete include-label horizon
before lookup, preserves raw labels and spans only for diagnostics, and uses
canonical package/target identities for lookup and selected paths. It
deduplicates packages in first-seen order, evaluates the unique package keys
in one `compute_join` group, retains every grouped Need, and then scans the
original includes so the first source-order terminal wins while an earlier
Need returns the full union. Semantic NoBuildFile/Deleted/InvalidPackageName
failures remain distinct from typed operational lookup errors. The owner is
private and dormant, with no include observation, byte read, public export,
consumer, dependency, or activation.

Six focused regressions cover exact label/canonical-target boundaries,
parse-all-before-lookup, operational failure identity, first-seen dedupe,
full grouped Need union, both mixed terminal/Need directions, first-source
terminals, alternate roots, and colonless repeated-basename path derivation.
Full validation passed 69 bzlmod unit plus 214 integration tests, 54 loading
tests, 102 core unit plus 13 integration tests, zero failing doctests, and
all 20 affected GNU-Windows test executables linked. Formatting, diff,
archive, privacy, no-IO, and exact-scope gates passed. Three terminal
latest-diff reviews returned `ACCEPT`, including the exact Bazel 9.2.0 source
audit against commit
`8220c6198837d5c13d53fea211cf3282aa12408a`.

Next evidence: Implement only `WP-5-m1-host-root-module-keys`.

#### Serial packet 8: dormant Host root keys

Only after packets 0-7 have terminal ACCEPT, run
`WP-5-m1-host-root-module-keys` with exact allowlist:

- new `app/slug_bzlmod_v2/src/host_module.rs`;
- `app/slug_bzlmod_v2/src/module_eval.rs`; and
- `app/slug_bzlmod_v2/src/lib.rs`.

The private root files/graph keys return
`SourcePreparationOutcome<Arc<Result<_, typed Host error>>>`, not the prior
fixed `PathOutcome`. Root bytes use `HostFileBytesKey`; a semantic Missing
becomes packet 1's bootstrap Need. No write or warning occurs in DICE.
At each breadth-first include horizon, inspect labels, run packet 7's entire
package preflight, and only after Complete success read every fragment with
the accepted `HostFileBytesKey`. Compile the full closure, execute inline,
then follow the accepted lockfile/graph ordering. Reuse crate-private
composed-evaluator/root-mapping seams from `module_eval.rs`; do not copy an
evaluator.

Tests pin cumulative root Need, missing→bootstrap request, package policy
before bytes/events, all same-horizon Need union, root and include
create/edit/delete/restore, selected alternate package root, SpecialFile
include acceptance, source-order error, repository-ignore event membership,
lockfile A→B→A, and complete-only equality. The keys remain crate-private
with zero loading/core/analysis/query consumer.

##### Packet 8 pre-implementation audit status

**Status:** Replan on 2026-07-26.

Exact Bazel 9.2.0 source at commit
`8220c6198837d5c13d53fea211cf3282aa12408a` disproved the frozen
files/graph boundary before Cargo. `ModuleFileFunction` produces only the
executed root-module value, overrides, non-registry routing indexes, and
canonical repository-relative module-file paths. It has no visible-lockfile,
environment-policy, lockfile-mode, resolved-graph, or final repository-mapping
edge. `RegistryFunction` requests the separate `BazelLockFileValue`, including
in Off mode, and final repository mappings come only from the selected
dependency graph. The legacy Slug `VisibleLockfileKey` Off shortcut and
pre-resolution `root_mapping` helper therefore cannot be copied into a Host
parity owner.

The audit also clarified that REPO.bazel policy events may occur during
include-package preflight; only include-fragment bytes and root/include MODULE
execution events are forbidden before successful preflight. Bazel deduplicates
package and file dependencies only within each breadth horizon. Every include
occurrence is still read, compiled, contributes its nested includes, and
executes inline. Successful module-file path identity is set-equivalent
canonical repository-relative paths and excludes selected package-root
identity.

The provisional three-file Rust draft was removed before retention. No Rust,
fixture, Cargo, dependency, public API, consumer, or activation changed.
Three terminal audits returned `REPLAN`.

Next evidence: Design only
`WP-5-m1-host-root-module-file-boundary-correction`.

#### Serial packet 8 correction: exact Host root-module-file owner

Run only `WP-5-m1-host-root-module-file-boundary-correction`. Do not edit
Rust or fixtures and do not run Cargo.

Freeze the corrected implementation as
`WP-5-m1-host-root-module-file-owner-corrected-retry` with the same eventual
three-file allowlist:

- new `app/slug_bzlmod_v2/src/host_module.rs`;
- `app/slug_bzlmod_v2/src/module_eval.rs`; and
- `app/slug_bzlmod_v2/src/lib.rs`.

Add only crate-private `HostRootModuleFileKey`,
`HostRootModuleFileValue`, and `HostRootModuleFileError`. The key is
workspace-identified and returns
`SourcePreparationOutcome<Arc<Result<HostRootModuleFileValue,
HostRootModuleFileError>>>`. The successful value retains exactly the
evaluated root module, the existing compact `RootModuleOverrides`, and a
sorted/deduplicated `Arc<[PathBuf]>` representation of Bazel's set-equivalent
canonical repository-relative module-file paths. It retains no raw label,
selected package root, physical resolution identity, visible lockfile,
environment or lockfile policy, resolved graph, provisional alias mapping,
or final `RepositoryMapping`. The compact evaluator's existing
`RootModuleOverrides` remains the sole override-definition owner; do not
invent a parallel non-registry lookup until an exact consumer packet owns it.

The root key computes only the ignore-dev command-policy projection before
the logical root `HostFileBytesKey`. A root path Need becomes a path Need;
semantic Missing becomes packet 1's sole bootstrap Need; a typed Host error,
UTF-8 failure, or syntax failure is terminal. No write, warning token,
lockfile read, environment-policy read, mapping construction, or repository
resolution occurs in DICE.

Inspect the root, then advance breadth-first include horizons. For every
horizon:

1. retain every include occurrence in source order;
2. run packet 7's full parse/package preflight, which deduplicates only
   first-seen package dependencies for its one grouped compute;
3. after complete package success, deduplicate selected logical include paths
   only for one grouped `HostFileBytesKey` compute;
4. retain the union of every same-horizon byte Need, then scan the original
   occurrences so an earlier terminal beats a later Need while an earlier
   Need returns the full union; and
5. read and compile-validate every occurrence in source order for UTF-8,
   syntax, identifier resolution, and include discovery, then append only a
   successfully validated occurrence's nested includes to the next horizon.

Do not use a global visited set. Equal raw labels and distinct raw labels whose
parsed Labels share one canonical repository-relative `toPathFragment` still
compile per occurrence and contribute nested includes. For example,
`//pkg:sub/x.MODULE.bazel` and `//pkg/sub:x.MODULE.bazel` use distinct valid
package identities but collapse to `pkg/sub/x.MODULE.bazel` in the successful
path set when both select the same root. Raw-label lookup may select the last
compiled occurrence exactly as Bazel's linked map does, while every
encountered `include()` call executes inline. Package/logical-file key dedupe
is dependency reuse, not discovery, compilation, or execution dedupe.
After root bytes complete, synchronously perform compile-equivalent root
validation before requesting any include package. At every horizon,
synchronously compile-validate each read occurrence before advancing to the
next horizon. Reuse a crate-private `module_eval.rs` validation helper that
parses the restricted dialect, checks MODULE/include syntax, and invokes the
same globals plus Starlark prepare/identifier-resolution path, but discard its
temporary Starlark Evaluator/Module/program state before the next DICE await.
After the whole breadth-first closure is validated and discovered, use the
shared synchronous evaluator to reparse/reprepare every occurrence before
executing index 0. This bounded duplicate preparation preserves Bazel's
failure/dependency order without retaining Starlark state across an await.
Reuse only crate-private prepared-evaluator and ignore-dev seams from
`module_eval.rs`; do not expose or copy `root_mapping`, `VisibleLockfileKey`,
the evaluator, or its globals.

Event ownership is explicit. With `CaptureEvaluationEvents`, a transient Need
stores no own batch and every Complete stores exactly one: empty on terminal
pre-execution failure, or the exact root/include Print prefix after execution
success or failure. Without the marker, store no batch and use the normal
direct root/include printer during execution; never both direct-print and
capture. Do not copy child batches: package preflight retains
`HostRepoFileKey` as a dependency, so activation-closure selection owns
REPO.bazel Print/diagnostic membership under its already accepted producer
policy. REPO events may precede include bytes and MODULE execution;
root/include MODULE events may not occur before the whole closure compiles.

The corrected regression matrix must discriminate:

- cumulative root observation Needs and Missing→sole bootstrap Need;
- root and include create/edit/delete/restore on one retained engine;
- a breadth-first package barrier before any same-horizon include bytes;
- one grouped same-horizon byte dependency set, full Need union, and both
  terminal/Need source-order directions;
- repeated equal raw labels and distinct raw labels with one canonical
  repository-relative path fragment compiling per occurrence, expanding
  nested horizons, and executing every call;
- a root prepare/identifier-resolution failure requesting no include package
  dependency, and a horizon-N prepare failure requesting no horizon-N+1
  observation;
- full-closure compile failure before an earlier root runtime failure;
- selected alternate package roots and SpecialFile includes without either
  identity entering successful equality;
- first-source package, byte, UTF-8, syntax, and evaluation diagnostics with
  raw label/span retained only in errors;
- REPO child-event membership across preflight terminal and Need/retry, empty
  root batches before execution, and ordered root/include Print prefixes;
- canonical relative path set identity independent of encounter order and
  collapsed across distinct raw labels with the same canonical
  repository-relative path fragment;
- lockfile A→B→A, environment-policy, and lockfile-mode changes producing no
  root-key dependency or value change, plus a forbidden-reference/direct-
  dependency scan for `root_mapping` and `RepositoryMapping`; and
- complete-only equality plus self-unequal/invalid Need.

Keep the module declaration private and retain zero
loading/core/analysis/query consumer. The post-root visible-lockfile boundary
becomes a separate design packet,
`WP-5-m1-host-visible-lockfile-boundary-design`, before Host registry keys.
That design must follow `BazelLockFileFunction`/`RegistryFunction`: registry
requests the lockfile owner even in Off, parsed/empty lockfile state is
separate from whether a consumer uses hashes, and no legacy
`VisibleLockfileRead::Ignored` shortcut is inherited. Final main/module
repository mappings remain deferred to the post-selection dependency-graph
owner, including extension imports/overrides.

##### Corrected root-module-file design status

**Status:** Accepted on 2026-07-26.

The corrected three-file retry now matches exact Bazel 9.2.0 root
`ModuleFileFunction` ownership: one private root-module-file value contains
only evaluated root semantics, compact overrides, and set-equivalent canonical
relative module-file paths. Visible lockfile, environment/mode policy,
resolution, and repository mappings are explicitly deferred. The design
freezes breadth-first package and logical-file dependency groups, per-
occurrence discovery/compile/expansion/execution, last raw-label lookup,
source-order terminals, full Need unions, selected-root-free equality,
marker-conditional root events, and dependency-owned REPO events.

Exact source review added both compile barriers: compile-equivalent root
validation precedes all include-package dependencies, and every horizon is
validated before any next-horizon dependency. A crate-private validation
helper may discard temporary Starlark state before awaits; the completed
closure is then reparsed/reprepared through the shared evaluator for inline
execution. Three terminal latest-text reviews returned `ACCEPT` against Bazel
9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`. No Rust,
fixture, Cargo, dependency, public API, consumer, or activation changed.

Next evidence: Implement only
`WP-5-m1-host-root-module-file-owner-corrected-retry`.

##### Corrected root-module-file implementation status

**Status:** Accepted on 2026-07-26.

The exact three-file implementation adds one private
`HostRootModuleFileKey` and value owning only evaluated root semantics,
compact overrides, and sorted/deduplicated canonical relative module-file
paths. It composes the accepted Host byte and package owners breadth-first,
keeps every include occurrence for compile validation, nested discovery, and
inline execution, and stores only marker-conditional root batches on terminal
results. A test-only validation observer directly proves root-once, repeated
shared-path occurrence, and next-horizon occurrence ordering without changing
production identity or state.

Focused Host tests passed 10/10. Full `slug_bzlmod_v2` passed 79 unit plus 214
integration tests, `slug_loading_v2` passed 54, and `slug_core_v2` passed 102
unit plus 13 integration tests; all doctest sets were empty and all 20
GNU-Windows test executables linked. Privacy, forbidden-dependency, exact-scope,
format, and diff checks passed. Exact Bazel-source, implementation/evidence,
and architecture/DICE terminal latest-diff reviews all returned `ACCEPT`
against Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`.

Next evidence: Design only
`WP-5-m1-host-visible-lockfile-boundary-design`, before any Host registry key.
The design must keep parsed/empty lockfile ownership distinct from consumer
hash use, preserve the registry request even in Off mode, and cite
`BazelLockFileFunction` plus `RegistryFunction`; no Rust or Cargo change is in
scope.

#### Serial packet 9: exact Host visible-lockfile boundary

Run only `WP-5-m1-host-visible-lockfile-boundary-design`. This is a design
packet; do not edit Rust or fixtures and do not run Cargo.

Exact Bazel 9.2.0 source at commit
`8220c6198837d5c13d53fea211cf3282aa12408a` supersedes the legacy Slug
Off-mode shortcut. `BazelLockFileFunction` always requests the visible
`MODULE.bazel.lock` file dependency, then the lockfile-mode input, including
in Off. Missing is the complete empty lockfile. Content with no recognized
version or a version other than 28 is empty in Off, Update, and Refresh but is
an unsupported-version error in Error. Recognized version 28 is fully decoded
and parsed in every mode, so caught JSON syntax/structural failures occur even
in Off. Ordinary read and caught current-version failures are persistent
`BAD_LOCKFILE`; absent, noncurrent, and valid structurally empty inputs
collapse to the same successful value. The exact custom-adapter exception
boundary is deferred to the schema prerequisite below.

`RegistryFunction` separately and unconditionally requests that full lockfile
value after its mode, vendor, and Refresh-only invalidation inputs and before
registry construction. Parsing and consumption are distinct boundaries.
`RegistryFactoryImpl` maps HTTP(S) Off and Update to `USE_AND_UPDATE`, Error to
`ENFORCE`, Refresh to `USE_IMMUTABLE_AND_UPDATE`, and `file://` to `IGNORE`.
Thus Off still passes and may consume recorded hashes and selected-yanked
versions; it never means `VisibleLockfileRead::Ignored`. Registry fetch,
scheme policy, `%workspace%`, mirrors, vendor state, refresh invalidation, and
yanked/hash consumption belong to a later Host registry packet.

The visible owner itself has no root-module dependency. Root-before-lockfile
ordering belongs to future resolution/nonroot consumers that obtain the root
module before requesting a registry. It also owns no hidden lockfile, write
plan, environment/yanked command policy, registry URL or IO state, selected
graph, extension import, override, provisional mapping, or final
`RepositoryMapping`.

The following was the provisional sequence reviewed by this packet. It is not
implementation authority unless the terminal status below accepts it.

1. `WP-5-m1-host-visible-lockfile-oracle` adds only the new
   `host-visible-lockfile-boundary` Bazel 9.2 fixture:

   - `fixture.toml` and generated `expected/oracle.json`;
   - root `workspace/MODULE.bazel`;
   - immutable
     `workspace/lockfiles/invalid-utf8-v28.lock`; and
   - minimal `workspace/registry/bazel_registry.json` plus
     `workspace/registry/modules/subject/1.0.0/MODULE.bazel` and
     `workspace/registry/modules/subject/1.0.0/source.json`.

   Use retained `bazel mod graph` requests through that local registry so
   `RegistryFunction` must request the visible lockfile even though the scheme
   later ignores hashes. Pin eight ordered rows: absent Off success; valid
   populated v28 Off success; malformed recognized-v28 Off failure;
   noncurrent malformed Off success as empty; the same noncurrent content in
   Error producing the unsupported-version diagnostic; recognized-v28 merge
   conflict producing Bazel's observed advice suffix; a valid JSON document
   containing malformed UTF-8 in an ignored string succeeding through Java
   replacement decoding; and deleted/absent Error success as empty. Record
   exact exit/diagnostic classes, successful graph output, the visible
   lockfile manifest, all cumulative mutations, source provenance, and the
   fixture growth. Do not add HTTP hash-use rows here; those discriminate the
   later registry consumer.

2. After oracle `ACCEPT`, implement only
   `WP-5-m1-host-visible-lockfile-owner` with this exact four-file allowlist:

   - new private `app/slug_bzlmod_v2/src/host_lockfile.rs`;
   - `app/slug_bzlmod_v2/src/lockfile.rs`;
   - `app/slug_bzlmod_v2/src/repository_ignore.rs`; and
   - the private module declaration in `app/slug_bzlmod_v2/src/lib.rs`.

   Add only crate-private `HostVisibleLockfileKey`,
   `HostVisibleLockfileValue`, and `HostVisibleLockfileError`. The key identity
   is one `NormalizedAbsolutePath` workspace. Its exact value is:

   ```text
   PathOutcome<
       Arc<Result<HostVisibleLockfileValue, HostVisibleLockfileError>>
   >

   HostVisibleLockfileValue {
       lockfile: Arc<BazelLockfile>
   }
   ```

   Freeze the private error variants as
   `LockfileModeInput { workspace: NormalizedAbsolutePath, message:
   CompactString }`, `File { error: HostFileError }`, and
   `BadLockfile { message: CompactString }`. Add only a crate-private
   `lockfile()` accessor returning `&Arc<BazelLockfile>`; expose no key, value,
   error, or parser.

   The successful value retains the complete structural v28 lockfile:
   registry hashes, selected-yanked versions, extension records, facts, and
   fact versions. It retains no mode, path, raw bytes, digest, formatting,
   missing/noncurrent discriminator, or `Ignored` variant. Structural `Arc`
   equality must compare separately allocated but semantically equal parsed
   values while allowing later consumers to `Dupe` the full maps.

   Compute `HostFileBytesKey(workspace/MODULE.bazel.lock)` first. A path Need
   returns immediately and is invalid/self-unequal. After any Complete byte
   result, compute `RootModuleLockfileModeKey` before interpreting a Missing,
   typed Host file error, or Present bytes. Missing mode, typed `HostFileError`,
   and parse/version failure remain distinct terminal Complete errors.
   Missing becomes `Arc::new(empty_bazel_lockfile())` in every mode. Present
   bytes use Bazel's Java `InputStreamReader` UTF-8 replacement semantics,
   exact first numeric version-marker scan and overflow behavior, then the
   existing full structural parser. Make only the existing pure
   `repository_ignore::java_utf8_decode` helper crate-private and reuse it;
   do not copy it or use Rust's behaviorally different `from_utf8_lossy`.
   Factor exactly
   `parse_visible_lockfile_bytes_for_host(mode: &LockfileMode, bytes: &[u8])
   -> Result<Arc<BazelLockfile>, CompactString>` in `lockfile.rs`; preserve
   every public and legacy `VisibleLockfileRead` API and its old Off behavior
   unchanged.

   Complete-only equality and validity are mandatory. Focused tests cover
   workspace key identity; cumulative observation Needs; absent and
   SpecialFile/symlink inputs; ordinary operational errors; missing mode after
   a complete file dependency; absent→A→B→delete→A on one retained DICE
   engine; all four mode/current/noncurrent/malformed cells; first numeric
   marker and integer overflow; ordinary versus merge-conflict diagnostics;
   malformed Java UTF-8 replacement; every full-value field; and
   formatting/key-order changes that recompute bytes but prune a downstream
   semantic projection. Dependency/event scans prove no root, registry,
   loading/core/analysis/query consumer, hidden lockfile, write, direct IO,
   event marker/batch, mapping, or activation edge.

3. After owner `ACCEPT`, design only
   `WP-5-m1-host-registry-function-boundary-design`. That packet must freeze
   root-before-registry composition; mode/vendor/Refresh invalidation order;
   unconditional visible-lockfile acquisition in all modes; URL validation
   and `%workspace%`; mirrors; the exact HTTP(S)/file hash-mode table;
   selected-yanked reuse; registry construction versus later fetch ownership;
   and the typed Host IO/error/equality surface. It must explicitly remove the
   legacy Host-path proposal to consume `RootModuleFilesKey` or reuse
   `remote_policy`'s Off early return. Final mappings stay with the
   post-selection dependency-graph owner.

The existing Stage 5/Stage 9 statements that Off has no visible-file
dependency, that Host registry policy obtains lockfile state through
`RootModuleFilesKey`, or that Off returns `FetchUnrecorded` before inspecting
the lockfile are legacy-only and superseded for the Host path. The public
legacy path remains unchanged until a separately accepted activation packet.

##### Host visible-lockfile boundary design status

**Status:** Replan on 2026-07-26.

Exact source review accepted the separate path-only Host owner, Off-mode read
and parse behavior, `PathOutcome`, Java UTF-8 utility reuse, full-value
`Arc` identity, root/registry/mapping exclusions, and oracle-first ordering.
It rejected the proposed implementation sequence before Rust because the live
`BazelLockfile` is not Bazel 9.2's full semantic value.

The live parser drops every non-`general` OS/architecture extension factor and
`moduleExtensionMetadata`; retains extension IDs, factors, Base64 digests,
recorded inputs, generated repo specs, facts, and fact versions with weaker or
raw identities; parses selected-yanked keys as unchecked strings rather than
typed `ModuleKey`/`Version`; and retains registry checksum spellings instead
of normalized optional SHA-256 values. Some malformed types survive as raw
`serde_json::Value`. Consequently two inputs equal as Bazel
`BazelLockFileValue`s may remain unequal in Slug, while distinct or invalid
Bazel inputs may collapse or parse. The proposed claims of a complete v28
value and every-field equality are not implementation authority.

The exact Bazel exception surface also needs a source-bounded decision:
`BazelLockFileFunction` catches `JsonSyntaxException`, `NullPointerException`,
`IllegalArgumentException`, and `IOException`, while some custom Gson adapters
throw the broader `JsonParseException`. Do not generalize every recognized-v28
adapter failure to the same caught `BAD_LOCKFILE` path without executable
evidence.

All three terminal latest-text audits returned `REPLAN`. Next evidence:
Design only `WP-5-m1-bazel-lockfile-v28-schema-design`. Audit
`BazelLockFileValue`, `GsonTypeAdapterUtil`, `ModuleExtensionId`,
`ModuleExtensionEvalFactors`, `LockFileModuleExtension`,
`LockfileModuleExtensionMetadata`, `Facts`, recorded inputs, `RepoSpec`,
`ModuleKey`/`Version`, and optional-checksum adapters against the live parser,
renderer, replay consumers, and public tests. Freeze an oracle-first exact
schema fixture, semantic types/equality/defaults/error contract, and the
smallest parser/renderer migration allowlist before returning to the Host
read/mode oracle. Do not edit Rust or fixtures and do not run Cargo.

#### Serial packet 10: exact Bazel v28 lockfile semantic owner

Run `WP-5-m1-bazel-lockfile-v28-schema-design` before returning to the Host
visible-lockfile boundary. This is a design packet; do not edit Rust or
fixtures and do not run Cargo.

##### Pinned Bazel 9.2 read and value contract

Source of truth is Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`, especially
`BazelLockFileFunction`, `BazelLockFileValue`, `GsonTypeAdapterUtil`,
`ModuleKey`, `Version`, `ModuleExtensionId`,
`ModuleExtensionEvalFactors`, `LockFileModuleExtension`,
`LockfileModuleExtensionMetadata`, `Facts`, `FactsAdapter`,
`RepoRecordedInput`, `RepoSpec`, `RepoRuleId`, and
`AttributeValuesAdapter`.

The raw visible file is decoded with Java UTF-8 replacement behavior and
scanned before structural JSON parsing. The first textual match of
`"lockFileVersion":\s*(\d+)` anywhere in the file is the version gate.
Recognized 28 enters Gson; a missing or other version produces the empty
value outside Error mode and an unsupported-version error in Error mode.
An overflowing matched decimal is a caught bad-lockfile failure. A nested or
unknown matching field may admit an object with no top-level version, whose
builder default is still 28.

AutoValue-Gson starts from `BazelLockFileValue.builder()`. Missing or explicit
JSON-null top-level properties therefore retain these defaults: signed
`lockFileVersion = 28` and empty `registryFileHashes`,
`selectedYankedVersions`, `moduleExtensions`, `facts`, and `factsVersions`.
Unknown fields are skipped. Duplicate properties are encounter-ordered and
the last non-null property wins. Rendering always emits all six top-level
fields in that order, including empty `facts` and `factsVersions`, with
Gson's two-space pretty format, disabled HTML escaping, and one final newline.

The semantic value must retain exactly:

- normalized optional SHA-256 registry entries, distinguishing an absent URL
  from recorded `"not found"` and from 32 decoded checksum bytes;
- typed selected-yanked module keys, including `<root>`, `_`, Bazel version
  validation, unsigned-64-bit numeric identifiers, and discarded `+build`
  metadata;
- typed extension IDs with canonical label, extension name, and optional
  typed module/usage isolation key;
- every normalized factor entry for an extension. `general` is empty
  OS/architecture; recognized `os:` and `arch:` components are last-wins,
  unknown components are ignored, and rendering is OS then architecture;
- required decoded `bzlTransitiveDigest` and `usagesDigest` byte slices,
  ordered recorded inputs, insertion-preserving generated repo specs, and
  optional complete extension metadata;
- recursively sorted, object-root Facts with string keys, JSON/Starlark
  null, Boolean, arbitrary integer, finite float, string, list, and dictionary
  values through depth seven, plus Starlark numeric equality and signed-i32
  fact versions;
- every FILE, DIRENTS, DIRTREE, ENV, and REPO_MAPPING recorded-input key,
  its nullable string value and exact escape grammar, plus the single
  normalized parse-failure sentinel;
- nullable `RepoSpec` fields, a typed optional-label `RepoRuleId`, and a
  distinct AttributeValues domain supporting None, Boolean, narrowed i32
  integer, string, canonical label, sequence, and insertion-preserving
  dictionary values. Strings that resemble labels and strings already
  wrapped in single quotes retain the adapter's extra quote layer.

Map/set equality is order-independent and list equality is ordered. Original
JSON spelling, whitespace, unknown fields, map insertion order where Bazel
sorts before construction, and checksum/Base64/version spellings are not
semantic identity. Map iteration order still matters to rendering where
Bazel preserves producer insertion order, notably generated repo specs and
AttributeValues dictionaries. Do not use one generic JSON enum for Facts and
AttributeValues.

The retained Rust owner must use the existing compact utility family
deliberately: `CompactString`, immutable `Arc<[T]>`, `SmallMap` for
order-preserving iteration with order-independent equality, `SmallSet` for
metadata sets with the same equality/iteration split, and
`SortedMap`/`SortedSet` only where the pinned producer or recursive Facts
normalization is actually sorted. Deep values are `Clone + Allocative`;
`Dupe` is reserved for pointer-sized or explicitly Arc-backed owners. Add no
interner, Starlark heap, cache, DICE edge, filesystem read, or retained
`serde_json::Value`. Lockfile module/version/label adapters stay private to
this abstraction and do not widen the resolution graph's current `ModuleKey`.
Existing `CanonicalLabel`, `CanonicalRepoName`, and `NonrootAttributeInt` may
be reused only after focused parity proves the needed adapter identity; in
particular root-label shorthand and Bazel's unvalidated recorded repository
names must not inherit a stricter Slug parser accidentally.

The exact parser cannot delegate to the current strict
`serde_json::Value` path. It owns a non-retained Gson-compatible token layer
that preserves encounter order, duplicate and null behavior, lenient grammar,
and raw arbitrary-size number spelling until typed normalization. The only
expected dependency addition is the already-workspace `base64`; do not enable
`serde_json` arbitrary-precision or another global feature.

##### Exact adapters and exceptional values

Checksums normalize valid hex to lower-case decoded SHA-256 identity.
Module-key parsing uses `<root>` specially, otherwise the first two `@`
components while ignoring later ones. Extension IDs similarly use percent
components zero through two while ignoring later ones; isolation keys use
the first two plus components while ignoring later ones. Missing delimiter
components are not accepted values. Digests are standard Base64 byte arrays
of arbitrary length, not fixed SHA-256 values.

The first four extension properties are required:
`bzlTransitiveDigest`, `usagesDigest`, `recordedInputs`, and
`generatedRepoSpecs`; metadata defaults absent. Metadata distinguishes null
dependency sets from empty sets, requires `useAllRepos` in `NO`, `REGULAR`,
or `DEV`, and defaults missing `reproducible` to false. Empty metadata is
omitted when absent.

Recorded-input strings split at their first literal space, unescape `\0`,
`\\`, `\n`, and `\s`, drop the backslash from other escapes, and drop a
terminal unmatched backslash. Malformed and unknown strings all become the same
parse-failure key with empty-string value. That sentinel is intentionally
not renderable. Replay validation that reaches it reports stale, and forced
reevaluation can replace that entry, but `combineModuleExtensions` may retain
an unevaluated old extension entry. Rendering any value still containing the
sentinel throws `UnsupportedOperationException`; the pure owner returns a
typed non-renderable error instead of inventing source spelling.

`RepoSpec` missing/null fields remain nullable. A `RepoRuleId` without `%`
has a null label and the whole string as rule name; the first `%` otherwise
delimits the label and all remaining percent characters belong to the name.
Rendering a present ID with a null label follows Bazel's exceptional
boundary rather than fabricating a label. Attribute JSON numbers use Gson's
`getAsInt()` narrowing and are a separate domain from arbitrary Facts
numbers.

Facts parse through Bazel's Starlark JSON decoder and normalize every nested
dictionary. Normal lockfile construction omits empty per-extension Facts and
emits fact versions only for retained nonempty Facts and nonzero versions,
but the schema parser retains explicit empty-Facts and zero-version map
entries and the renderer must not silently filter them.

The outer visible/hidden SkyFunction catches only `IOException`,
`JsonSyntaxException`, `NullPointerException`, and
`IllegalArgumentException`. Caught visible failures become persistent
`BAD_LOCKFILE`; caught hidden failures become empty. Gson wraps adapter-read
`IllegalStateException` as caught `JsonSyntaxException`, including
wrong-token/null `nextString()` failures and missing required extension
properties. Direct custom-adapter `JsonParseException` and delimiter-driven
`IndexOutOfBoundsException` remain uncaught holes. The recorded-input
sentinel's `UnsupportedOperationException` is a later rendering failure
outside this read catch. Executable evidence must distinguish, at minimum,
invalid checksum or version/extension adapter failures and missing delimiter
components from malformed JSON, invalid Base64, invalid Facts, missing
metadata enum, and missing required properties. Do not flatten those Bazel
9.2 exception holes into one Slug parse error.

##### Frozen serial migration

1. `WP-5-m1-bazel-lockfile-v28-schema-oracle` adds exactly:

   - `tests/v2_oracle/fixtures/bazel-lockfile-v28-schema/fixture.toml`;
   - `tests/v2_oracle/fixtures/bazel-lockfile-v28-schema/http_registry.py`;
   - `tests/v2_oracle/fixtures/bazel-lockfile-v28-schema/expected/oracle.json`;
   - `tests/v2_oracle/fixtures/bazel-lockfile-v28-schema/workspace/MODULE.bazel`;
   - `tests/v2_oracle/fixtures/bazel-lockfile-v28-schema/workspace/BUILD.bazel`;
   - `tests/v2_oracle/fixtures/bazel-lockfile-v28-schema/workspace/ext.bzl`;
   - `tests/v2_oracle/fixtures/bazel-lockfile-v28-schema/workspace/input.txt`;
   - `tests/v2_oracle/fixtures/bazel-lockfile-v28-schema/workspace/input_dir/entry.txt`;
   - `tests/v2_oracle/fixtures/bazel-lockfile-v28-schema/workspace/registry/bazel_registry.json`;
   - `tests/v2_oracle/fixtures/bazel-lockfile-v28-schema/workspace/registry/modules/subject/1.0.0/MODULE.bazel`;
   - `tests/v2_oracle/fixtures/bazel-lockfile-v28-schema/workspace/registry/modules/subject/1.0.0/source.json`; and
   - `tests/v2_oracle/fixtures/bazel-lockfile-v28-schema/workspace/registry/modules/subject/metadata.json`.

   It uses one self-contained extension, that fixture-local loopback registry,
   and command-local lockfile mutations embedded in `fixture.toml`. No harness,
   lockfile-template asset, archive, symlink, or other registry file is
   authorized. This is the first oracle packet after accepted fixture-growth
   checkpoint `df812c2c`.

   The fixture must first make Bazel 9.2 generate and manifest a comprehensive
   v28 value. Its retained rows then prove minimal/default/null behavior,
   unknown fields, duplicate/first-marker anomalies, the complete extension
   value with multiple factors, both digests, all recorded-input kinds and
   escapes, typed repo attributes, metadata, Facts and fact versions,
   canonical rewrite shape, one Gson-default-lenient-only accepted spelling,
   one genuinely malformed rejection, caught failures, uncaught
   custom-adapter holes, and forced-evaluation malformed recorded-input stale
   replacement. Exact output assertions
   cover field names/order, lower-case checksum, build-suffix removal,
   canonical labels/factors, padded Base64, string/label quoting, recursively
   sorted Facts, all six top-level fields, and final newline.

   Keep pure adapter algebra in later source-derived unit rows: delimiter
   extras/missing components, numeric/version bounds, factor collisions,
   every escape, RepoRuleId without `%`, AttributeValues narrowing, Facts
   depth 7/8 and numeric equality, null/empty metadata sets, normalized key
   collisions, and explicit zero fact version. Stop and replan instead of
   changing the harness or claiming a field that real Bazel cannot
   generate/retain.

   Generate and validate serially with:

   - `SLUG_V2_ORACLE_ROOT=target/v2o-lockfile-v28-generate python3 -B -m tools.v2_oracle run --fixture bazel-lockfile-v28-schema --tool bazel --bazel /usr/bin/bazel --update-expected`;
   - `SLUG_V2_ORACLE_ROOT=target/v2o-lockfile-v28-replay-a python3 -B -m tools.v2_oracle run --fixture bazel-lockfile-v28-schema --tool bazel --bazel /usr/bin/bazel`;
   - `SLUG_V2_ORACLE_ROOT=target/v2o-lockfile-v28-replay-b python3 -B -m tools.v2_oracle run --fixture bazel-lockfile-v28-schema --tool bazel --bazel /usr/bin/bazel`;
   - `scripts/v2_archive_status.sh`; and
   - `git diff --check`.

2. `WP-5-m1-bazel-lockfile-v28-pure-owner` changes exactly:

   - `app/slug_bzlmod_v2/Cargo.toml`;
   - `app/slug_bzlmod_v2/src/lockfile_v28.rs`;
   - `app/slug_bzlmod_v2/src/lockfile_v28_tests.rs`; and
   - `app/slug_bzlmod_v2/src/lib.rs`.

   Add only `base64 = { workspace = true }`. The new owner and tests are
   private.
   Implement the complete immutable value, every reader adapter, semantic
   equality, typed parse/render errors, canonical renderer, and
   parse-render-parse evidence together. There is no public re-export,
   production caller, old-owner edit, planner, replay validator, registry,
   Host, or DICE change. Parser and renderer remain one owner because
   normalization, escaping, omission, map order, and non-renderable sentinels
   are one adapter contract.

   Validate serially with `cargo fmt --all -- --check`,
   `cargo test -p slug_bzlmod_v2 lockfile_v28`,
   `cargo test -p slug_bzlmod_v2`, `cargo test -p slug_bzlmod_v2 --doc`,
   `cargo test -p slug_bzlmod_v2 --target x86_64-pc-windows-gnu --no-run`,
   `scripts/v2_archive_status.sh`, `git status --short`, and
   `git diff --check`.

3. `WP-5-m1-bazel-lockfile-v28-live-cutover` atomically switches live
   parse/render/read/planning, registry expectation, and retained replay
   consumers to the accepted owner and deletes the old structs, parser, and
   renderer in the same commit. Exact allowlist is:

   - `app/slug_bzlmod_v2/src/lockfile_v28.rs`;
   - `app/slug_bzlmod_v2/src/lockfile.rs`;
   - `app/slug_bzlmod_v2/src/lib.rs`; and
   - `app/slug_bzlmod_v2/tests/lockfile.rs`.

   `registry_dice.rs` and its tests stay unchanged because
   `registry_file_expectation()` preserves their exact accessor boundary. No
   alias, compatibility adapter, dual production schema, retained raw JSON,
   or literal-general extension wrapper may survive.

   Preserve `registry_file_expectation()` as the typed consumer projection if
   possible: missing URL is `Unrecorded`, recorded absence is distinct, and a
   checksum is 32 decoded bytes. Replay APIs must become factor-qualified and
   cover all five recorded-input domains or be deleted when exhaustive search
   proves them test-only. A general-only traversal or silent validation of
   DIRENTS, DIRTREE, REPO_MAPPING, or the parse-failure sentinel is forbidden.
   Existing visible/hidden mode and fail-open behavior remains unchanged
   except where the exact oracle corrects it.

   Validate serially with `cargo fmt --all -- --check`,
   `cargo test -p slug_bzlmod_v2 lockfile`,
   `cargo test -p slug_bzlmod_v2`, `cargo test -p slug_loading_v2`,
   `cargo test -p slug_core_v2`, `cargo test -p slug_bzlmod_v2 --doc`,
   `cargo test -p slug_loading_v2 --doc`,
   `cargo test -p slug_core_v2 --doc`,
   `cargo test -p slug_bzlmod_v2 --target x86_64-pc-windows-gnu --no-run`,
   `cargo test -p slug_loading_v2 --target x86_64-pc-windows-gnu --no-run`,
   `cargo test -p slug_core_v2 --target x86_64-pc-windows-gnu --no-run`,
   `scripts/v2_archive_status.sh`, `git status --short`, and
   `git diff --check`. Require no matches from:

   - `rg -n 'BazelLockfileModuleExtensionGeneral|\.general\b' app/slug_bzlmod_v2/src/lockfile.rs app/slug_bzlmod_v2/src/lockfile_v28.rs app/slug_bzlmod_v2/src/lib.rs app/slug_bzlmod_v2/tests/lockfile.rs`;
   - `rg -n 'serde_json::Value|use serde_json::Value' app/slug_bzlmod_v2/src/lockfile.rs app/slug_bzlmod_v2/src/lockfile_v28.rs app/slug_bzlmod_v2/tests/lockfile.rs`; and
   - `rg -n 'general_extension_entries|general-only' app/slug_bzlmod_v2/src/lockfile.rs app/slug_bzlmod_v2/src/lockfile_v28.rs app/slug_bzlmod_v2/tests/lockfile.rs`.

The cutover end gate also requires semantic A-to-B-to-A tests,
formatting/order equivalence pruning, every factor/metadata/fact in value
equality, typed registry expectation parity, and the full hidden/visible mode
regression. Host remains blocked until this gate accepts; its later value
wraps `Arc<BazelLockfile>` and never clones the full maps.

Stop and replan on a required harness change, inability to generate every
claimed field, unresolved numeric/map-order identity, mismatch in a proposed
shared label/repository/integer helper, global serde-feature side effects, a
retained Starlark heap, pure-owner scope beyond its named private files, a
cutover requiring root/mapping/activation changes, or any temporary second
production schema.

##### Lockfile v28 schema design status

**Status:** Accepted after correction on 2026-07-26.

Pinned-source, live consumer/representation, and orchestration-policy audits
accepted the core typed value/equality and private-owner-plus-atomic-cutover
sequence, then latest-text review corrected Gson's lenient read mode and
`IllegalStateException` wrapping, conditional recorded-input replacement,
compact utility semantics, exact file manifests, parser substrate, and
per-packet validation. The live production owner remains unchanged and
intentionally blocks Host lockfile ownership. No Rust, fixture, Cargo,
dependency, public API, or activation changed. All three terminal latest-text
rereviews returned `ACCEPT`.

Next evidence: Run only
`WP-5-m1-bazel-lockfile-v28-schema-oracle`.

##### Lockfile v28 schema oracle first-attempt status

**Status:** Replan before retained fixture evidence on 2026-07-26.

The exact pinned-source generation audit activated the accepted stop gate:
`module_ctx` can naturally record FILE through watched `read`, DIRENTS through
watched `readdir`, ENV through `getenv`, and REPO_MAPPING through an apparent
label. DIRTREE is exposed only through `repository_ctx.watch_tree`; a
generated repository implementation runs after extension evaluation and
cannot put that recorded input into its owning lockfile extension value.
Therefore the first command cannot honestly claim that real extension
evaluation generated all five recorded-input kinds.

A coordination race left an ignored, untracked 11-of-12-file working draft
after the stop request but before any acceptable oracle output. Its sole
generation attempt failed in harness postprocessing because the initial HTTP
logger format was not JSONL; the allowed logger draft corrected that local
format before the hold arrived. There is no generated `expected/oracle.json`,
tracked fixture, accepted command row, Rust, Cargo, harness, symlink, archive,
or external-network change. Preserve the draft without treating it as
evidence until the corrected contract accepts.

Correct only `WP-5-m1-bazel-lockfile-v28-schema-oracle-correction`:

- keep the exact twelve-file allowlist and no-harness boundary;
- make the generated baseline claim exactly registry hashes,
  selected-yanked state, one current OS/architecture factor, both digests,
  FILE/DIRENTS/ENV/REPO_MAPPING inputs, typed generated repo specs and
  attributes, metadata, Facts, and fact version;
- hand-inject one valid DIRTREE entry into the current factor, force replay to
  report it stale, and force update reevaluation before asserting its
  replacement. Inject a separate valid foreign factor only to prove
  multi-factor parse/retain/render behavior; do not claim it was evaluated;
- cap the cumulative retained-daemon fixture at 15 commands, twelve regular
  files, zero symlinks, and 2,500 net text lines including expected output;
- use only the fixture-local loopback registry plus Bazel's embedded modules.
  Do not add BCR or any other external registry/network fallback;
- put caught failures before the final three uncaught custom-adapter holes,
  use a non-object Facts row rather than deferred depth algebra, and retain
  the exact generation/two-fresh-replay/archive/diff commands already frozen.

Rows remain: generated comprehensive baseline; semantic rewrite with a valid
foreign factor; current-factor DIRTREE stale and forced replacement;
default/null/unknown/duplicate plus first-marker anomaly; Gson-lenient
acceptance; overflowing marker; malformed JSON; invalid Base64; invalid
non-object Facts; missing metadata enum; missing required extension property;
invalid-checksum and invalid-version direct `JsonParseException` holes; and a
missing-extension-delimiter `IndexOutOfBoundsException` hole. Combine only
when discrimination and cumulative mutation order remain exact; never exceed
the caps.

Stop again on a thirteenth file, harness/template/archive/symlink addition,
ambiguous replacement, dynamic registry token inside mutation content,
foreign-factor loss, unnormalized stack/port/path output, caught/uncaught
behavior that differs from the pinned source, server destabilization before a
later row, any external network, or a need to claim pure adapter algebra.

Next evidence: Design-correct and terminally rereview only
`WP-5-m1-bazel-lockfile-v28-schema-oracle-correction` before resuming the
preserved draft.

##### Lockfile v28 schema oracle embedded-closure status

**Status:** Replan before retained fixture evidence on 2026-07-26.

The accepted correction's loopback-only network gate is infeasible with the
exact twelve-file fixture. Pinned Bazel 9.2's embedded `bazel_tools` module
requests its ordinary buildozer, platforms, protobuf, rules_cc, rules_java,
rules_license, rules_python, rules_shell, apple_support, and zlib module
closure through the command registry list. The first corrected generation
therefore exited 48 at missing `buildozer@8.5.1`; no extension or schema
evidence ran. A race after the next hold added an in-memory HTTP draft for
synthetic one-line embedded module declarations and a second failed probe.
That invention is not Bazel source-of-truth content, is not accepted
scaffolding, and must be removed before the next run. Neither probe produced
`expected/oracle.json` or an accepted row.

Correct only
`WP-5-m1-bazel-lockfile-v28-schema-oracle-embedded-closure-correction`:

- preserve the exact twelve files, 15 commands, 2,500-line cap, generated
  four-input plus injected-DIRTREE contract, and no harness/template/archive
  changes;
- restore `http_registry.py` to serving only checked fixture registry bytes;
  it must not synthesize embedded module files or proxy another registry;
- append `--registry=https://bcr.bazel.build` after the loopback `/missing`
  and `/registry` URLs on graph/extension commands, matching accepted
  `registry-yanked-lockfile-mode` precedent;
- use BCR only for Bazel's embedded-module closure. All asserted checksum,
  recorded-absence, yanked, module, extension, rewrite, stale, and exception
  evidence stays fixture-local. Filter the fixture request log to its unique
  `subject` paths so embedded fallback attempts do not become asserted rows;
- do not inspect, print, copy, or depend on the user's bazelrc contents.

The BCR fallback is immutable upstream scaffolding for the pinned Bazel
release, not a second asserted registry. Stop again if a non-embedded module
requires BCR, if BCR-derived content enters a semantic assertion or mutation,
if the fallback makes normalized output unstable, if the synthetic server
behavior cannot be removed within `http_registry.py`, or if any prior
allowlist, cap, discrimination, or exception gate fails.

Next evidence: Terminally rereview only
`WP-5-m1-bazel-lockfile-v28-schema-oracle-embedded-closure-correction` before
resuming the preserved draft.

##### Lockfile v28 schema oracle BCR-provenance status

**Status:** Corrected after terminal replan on 2026-07-26.

Pinned-source rereview rejected one overstatement in the embedded-closure
correction. An exact generated `MODULE.bazel.lock` manifest necessarily
retains `registryFileHashes` for BCR files used by the embedded-module
closure. Those bytes therefore do participate in the expected manifest and
cannot be called unasserted. The earlier list of ten requested modules also
described only the first failed probe, not the complete closure reachable
from Bazel's pinned `src/MODULE.tools`.

Correct only
`WP-5-m1-bazel-lockfile-v28-schema-oracle-bcr-provenance-correction`:

- keep BCR last in the registry list solely to resolve the embedded-module
  closure, and describe that closure by pinned source rather than a closed
  handwritten module-name list;
- add pinned Bazel
  `src/test/tools/bzlmod/MODULE.bazel.lock` and `src/MODULE.tools` to fixture
  provenance. Before acceptance, classify the complete embedded-closure
  scaffold by URL suffix. Every retained BCR URL/hash must match the
  corresponding pinned 9.2 source-lockfile entry, and each such suffix must
  have exactly the two deterministic `"not found"` companions recorded from
  the loopback `/missing` and `/registry` prefixes. Record the compact trio
  comparison; do not copy the source lockfile into the fixture;
- accept those exact BCR successes plus paired loopback misses only as
  immutable/deterministic upstream scaffolding in the whole-file manifest.
  The fixture's unique `subject` suffix is excluded from that closure set and
  remains the sole semantic recorded-absence/checksum/yanked registry row.
  Every other semantic contains/pattern assertion, case-normalization
  mutation, stale/replacement transition, and exception row must target the
  subject or root extension fields;
- filter request-count evidence to loopback subject paths. Do not assert the
  order, count, or handwritten membership of embedded fallback requests;
- remove the synthetic embedded-module server behavior before any new
  generation.

All prior twelve-file, no-harness, generated-versus-injected DIRTREE,
15-command, 2,500-line, row-order, replay, and validation gates remain exact.
Stop again if a retained BCR entry lacks a pinned source-lockfile match, if
its two loopback-miss companions are absent or extra, if a non-embedded user
module reaches BCR, if a semantic mutation can match closure scaffolding, or
if upstream scaffold content is duplicated into the fixture.

Next evidence: Terminally rereview only
`WP-5-m1-bazel-lockfile-v28-schema-oracle-bcr-provenance-correction` before
resuming the preserved draft.

##### Lockfile v28 schema oracle scaffold-classification status

**Status:** Replan before retained fixture evidence on 2026-07-26.

The first successful real BCR-backed generation disproved the corrected
packet's universal companion rule. Its diagnostic visible lockfile contains
500 registry entries: 156 BCR `modules/<name>/<version>/MODULE.bazel`
successes with exactly two same-path loopback `"not found"` entries each; 27
BCR `source.json` successes with no loopback companions; one BCR
`bazel_registry.json`; one checked loopback registry descriptor; and three
exact subject entries. This follows Bazel's separate owners: module discovery
tries the ordered registry list, while `RepoSpecFunction` fetches
`source.json` directly from the registry retained on the selected module.

All 184 BCR URL/hash pairs exactly equal pinned commit
`8220c6198837d5c13d53fea211cf3282aa12408a`'s
`src/test/tools/bzlmod/MODULE.bazel.lock`; the earlier 175-match result came
from comparing a working-tree file rather than the pinned object. The
two-command generated `expected/oracle.json`, visible lockfile, and target
trees are diagnostic only. No semantic mutation row, fresh replay, tracked
fixture, Rust, Cargo, harness, public API, or activation is accepted.

Correct only
`WP-5-m1-bazel-lockfile-v28-schema-oracle-scaffold-classification-correction`:

- require exact set and value equality between every BCR entry and the pinned
  tools lockfile, without a copied checker, lockfile, or handwritten module
  list;
- partition the remaining URLs exactly: each BCR `MODULE.bazel` entry has the
  `/missing` and `/registry` same-path `"not found"` entries; BCR
  `source.json` entries have neither; the BCR descriptor has no loopback-miss
  claim; the checked loopback descriptor equals its fixture bytes; and the
  only subject entries are `/missing/.../MODULE.bazel = "not found"` plus
  checked `/registry/.../{MODULE.bazel,source.json}` hashes;
- reject every unclassified registry URL, BCR hash mismatch, loopback success
  mismatch, missing/extra class member, unexpected source companion, or
  nonembedded user module on BCR. Counts are compact diagnostics; set equality
  is the acceptance rule;
- retain the whole-file manifest while anchoring every semantic assertion and
  mutation to the complete subject URL or root extension ID. Never mutate a
  generic hash, suffix, `"not found"`, `MODULE.bazel`, or `source.json`;
- preserve subject-filtered request evidence and remove every synthetic-server
  behavior. Fold the separate lockfile-print probe into the canonical
  rewrite/foreign-factor row so the exact baseline, rewrite, DIRTREE stale and
  replacement, adapter, and final three uncaught-hole sequence stays within
  15 commands.

All prior twelve-file, no-harness, 2,500-line, generated-versus-injected
DIRTREE, replay, archive, diff, and exception-order gates remain exact.
Terminally rereview this correction before resuming the held ignored draft.

##### Lockfile v28 schema oracle mutation-anchor status

**Status:** Replan before retained fixture evidence on 2026-07-26.

Terminal implementability review accepted the class/set checker and combined
rewrite/print row, but rejected one infeasible mutation rule. With
`http_registry_port = 0`, the complete subject URL contains a runtime-selected
port. The unchanged harness expands `{{http_registry}}` in argv and staged
workspace files, but mutation `find`/`replace` expands only
`{{workspace_uri}}`. Therefore a pre-command lockfile mutation cannot anchor
on the complete subject URL without a harness change or fixed collision-prone
port. The ignored two-row output remains diagnostic and no generation resumed.

Correct only
`WP-5-m1-bazel-lockfile-v28-schema-oracle-mutation-anchor-correction`:

- preserve complete dynamic subject-URL anchoring for post-generation
  class/set checks, manifest assertions, request evidence, and every mutation
  whose target does not contain a runtime token;
- for the checksum case-normalization mutation only, derive the exact subject
  `MODULE.bazel` SHA-256 from its checked fixture bytes. Before mutation,
  require that exact lowercase 64-hex value occurs once in the visible
  lockfile and its uppercase spelling occurs zero times. Replace that one
  exact value with uppercase, then require Bazel's canonical rewrite to
  restore the exact lowercase value;
- this is an exact checked subject-value anchor, not permission to replace a
  generic checksum, hash pattern, suffix, `"not found"`, `MODULE.bazel`, or
  `source.json`. Stop on zero or multiple matches, a preexisting uppercase
  match, another dynamic-port mutation need, fixed-port use, or harness
  expansion;
- retain the accepted 500-entry URL-class partition, 184/184 pinned BCR map
  equality, exact loopback success hashes, fully anchored root-extension
  mutations, combined rewrite/print row, twelve files, 15 commands, and
  2,500-line cap unchanged.

Terminally rereview this exact mutation exception before resuming the held
draft.

##### Lockfile v28 schema oracle canonical-write status

**Status:** Replan before retained fixture evidence on 2026-07-26.

Pinned-source review accepted uppercase checksum parsing, lowercase rendering,
the exact static subject-hash anchor, and the URL-class contract, but found a
missing write precondition. `BazelLockFileModule` writes only when the newly
computed semantic lockfile differs from the parsed old value. Uppercase and
lowercase checksum spellings parse to the same value, and retaining a valid
foreign factor also creates no inequality. A case-only row therefore cannot
claim that Bazel will rewrite the raw uppercase bytes.

Correct only
`WP-5-m1-bazel-lockfile-v28-schema-oracle-write-trigger-correction`:

- preserve the exactly-once lowercase subject hash and zero-uppercase
  precondition, then uppercase that exact hash and inject the valid foreign
  factor in the combined rewrite/print row;
- in the same row, mutate checked `input.txt` to a second exact value. Make
  `//:print_lockfile` depend explicitly on both `@alpha//:value.txt` and
  `@beta//:value.txt`, run it in update mode with the same allowlisted
  extension environment, and require the extension-evaluated marker;
- require the watched FILE input and generated repo content/spec to change the
  current factor semantically, forcing `newLockfile != oldLockfile`. The valid
  compatible foreign factor remains retained. Stdout may capture the injected
  pre-write spelling, but the after-command manifest must contain the exact
  lowercase subject hash, changed current factor, and retained foreign factor;
- row 3 must find that exact canonical retained state before adding DIRTREE.
  Stop if the print target does not force both generated repositories,
  extension evaluation is absent, current-factor semantics remain equal,
  canonical lowercase output is absent, or foreign-factor retention fails.

The unchanged harness, exact twelve files, combined 15-row sequence,
500-entry scaffold partition, dynamic URL assertions, no generic mutation,
2,500-line cap, and all later DIRTREE/adapter/exception/replay gates remain
exact. Terminally rereview this explicit semantic write trigger before
resuming the held draft.

##### Lockfile v28 schema oracle write-trigger correction

**Status:** Accepted on 2026-07-26.

Pinned-source, held-fixture implementability, and orchestration/evidence
latest-text reviews all returned `ACCEPT`. The combined row can add the two
generated repository labels to the existing print target, mutate the checked
FILE input, uppercase the exactly-once subject checksum, inject a compatible
foreign factor, and run in update mode with the same environment. The changed
FILE digest and generated repo spec make the current factor unequal, forcing
Bazel's write gate while `combineModuleExtensions` retains the foreign factor.
Stdout may show the injected pre-write bytes; only the post-command manifest
proves canonical lowercase output. No fixture edit or generation occurred
during review.

Next evidence: Resume only
`WP-5-m1-bazel-lockfile-v28-schema-oracle` under the complete corrected
twelve-file, 15-row, 2,500-line contract.

##### Lockfile v28 schema oracle

**Status:** Accepted on 2026-07-26.

The exact twelve-file fixture retains 15 ordered Bazel 9.2 commands. Its
generated baseline covers all six top-level fields, selected-yanked state,
current OS/architecture factor, both digests, natural
FILE/DIRENTS/ENV/REPO_MAPPING inputs, typed generated repo specs and
attributes, metadata, recursively normalized Facts, and fact version. The
canonical row changes a watched input, forces both generated repositories,
normalizes an exactly-once uppercase subject checksum, and retains a compatible
foreign factor. Error mode then rejects an injected current-factor DIRTREE;
update reevaluates the extension and restores the canonical row digest.
Minimal/default/unknown/duplicate-first-marker and Gson-lenient rows converge
canonically, followed by six caught parser/adapter failures and the exact
invalid-checksum, invalid-version, and missing-delimiter uncaught holes.

The fixture uses semantic comparison, so message-shape assertions remain while
all five normalized visible-lockfile manifests are regression gates. A direct
comparator test proved synthetic baseline-manifest drift fails. The complete
500-entry registry map is exact set/value evidence: 184/184 BCR entries equal
the pinned tools lockfile; 156 BCR module files have 312 paired loopback
recorded absences; 27 selected-registry source files have no companions; one
BCR descriptor, one checked loopback descriptor, and three exact subject
entries complete the set with zero unclassified URLs. Request evidence remains
filtered to subject paths, and no synthetic module server or semantic BCR
mutation remains.

Validation: semantic generation
`20260726-111357-3325340-bazel`; final fresh-root replays
`20260726-111556-3333495-bazel` and
`20260726-111641-3337458-bazel`; exact exits
`0,0,48,0,0,0,48,48,48,48,48,48,37,37,37`; 23/23 pinned
source anchors; scaffold classifier; transition-digest equality; archive and
staged diff checks. Growth is twelve regular files, zero symlinks, and 1,311
lines, the first accepted oracle after checkpoint `df812c2c`; no fixture-growth
checkpoint is due. All three corrected latest-diff source,
implementation/evidence, and orchestration/maintainability reviews returned
`ACCEPT`. No Rust, Cargo, harness, archive, public API, or activation changed.

Residual risk: production still has the incomplete live schema. Implement only
`WP-5-m1-bazel-lockfile-v28-pure-owner` under the accepted private four-file
contract; the atomic live cutover and Host ownership remain later packets.

##### Lockfile v28 pure owner

**Status:** Accepted on 2026-07-26.

The private four-file owner now implements the complete immutable v28 value,
streaming Gson-compatible typed reader, direct canonical renderer, semantic
equality, and typed caught/uncaught error surfaces. Retained state uses compact
strings, shared slices, and Buck2-derived compact/sorted collections; no generic
JSON tree or standard hash collection is retained. The owner remains private
and dormant: only the workspace `base64` dependency was added, with no live
lockfile caller, public re-export, Host/DICE change, or `Cargo.lock` drift.

Validation passed 98 focused owner tests, 177 complete owner-crate unit tests
plus every integration suite, zero doctests, every GNU-Windows owner-crate test
executable link, formatting, archive integrity, whitespace, exact-scope, and
forbidden-pattern checks. Corrected terminal source, native-observation, and
orchestration/hot-path reviews all returned `ACCEPT`; their final checks covered
domain-sensitive lone-surrogate normalization, Java decimal/suffix/hex
`Double.parseDouble` fallback with exact rounding, Gson leniency, adapter
identity/order, and the private direct-streaming architecture.

Residual risk: the live `lockfile.rs` owner is still incomplete and intentionally
unchanged. Next implement only
`WP-5-m1-bazel-lockfile-v28-live-cutover` in `lockfile_v28.rs`, `lockfile.rs`,
`lib.rs`, and `tests/lockfile.rs`; preserve the accepted private value while
atomically replacing the live parse/render surface. Host ownership remains a
later packet.

##### Lockfile v28 live-cutover pre-implementation gate

**Status:** Replan before Rust on 2026-07-26.

The exact four-file cutover cannot activate the accepted reader on the current
production DICE path. `read_lockfile_v28` requires raw bytes so Java-compatible
UTF-8 replacement precedes the first textual version-marker scan, but
`VisibleLockfileKey` obtains `WorkspaceFileValue::Present(Arc<String>)` from
the strict-UTF workspace-file owner. Invalid bytes have already become a read
error and cannot be reconstructed inside the four-file allowlist. This reaches
the accepted stop condition for a cutover requiring activation-boundary
changes.

The remaining semantic replacement is bounded once that read boundary is
corrected. `registry_file_expectation()` can preserve the unchanged
`registry_dice.rs` consumer; exhaustive search proved the legacy general-only
replay types and validators are test-only and should be deleted. The old
`serde_json::Value`/`BTreeMap` schema, parser, renderer, registry validators,
and exports must disappear. Planning must compare full v28 values so
formatting/order-equivalent inputs prune while every factor, metadata, fact,
and facts-version change remains semantic.

Two independent source/implementation reviews returned `REPLAN`; the policy
review accepted the four-file semantic architecture but explicitly left
invalid-UTF-8 acquisition behind the existing text boundary. No Rust, test,
fixture, Cargo, dependency, public API, DICE, or activation change was made.

Next evidence: Design only
`WP-5-m1-bazel-lockfile-v28-raw-read-live-cutover-design`. Audit the existing
`WorkspaceRawFileKey`/`WorkspaceRawFileValue` path and freeze the smallest
atomic cutover, provisionally adding only `module_eval.rs` and
`tests/root_module_dice.rs` to the original four files. It must preserve exact
caught versus direct-adapter/delimiter error surfaces and cover invalid-byte,
mode, formatting/equality, and retained A-to-B-to-A behavior before any Rust
edit or Cargo command.

#### Lockfile v28 raw-read live-cutover correction

Run only `WP-5-m1-bazel-lockfile-v28-raw-read-live-cutover-design`. This is a
design packet; do not edit Rust or fixtures and do not run Cargo.

##### Authority and one atomic result

Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`
`BazelLockFileFunction.java`, `BazelLockFileValue.java`,
`BazelLockFileModule.java`, and `GsonTypeAdapterUtil.java` remain the source
authority. The accepted `bazel-lockfile-v28-schema` oracle and private
`lockfile_v28.rs` owner remain the executable/schema authority. This correction
adds no fixture breadth; the last fixture-growth checkpoint remains
`df812c2c`.

The implementation packet must atomically make the accepted v28 value the sole
live schema and feed raw observed bytes to it on the existing active DICE path.
Its exact eight-file allowlist is:

1. `app/slug_bzlmod_v2/src/lockfile_v28.rs`;
2. `app/slug_bzlmod_v2/src/lockfile.rs`;
3. `app/slug_bzlmod_v2/src/lib.rs`;
4. `app/slug_bzlmod_v2/tests/lockfile.rs`;
5. `app/slug_bzlmod_v2/src/module_eval.rs`; and
6. `app/slug_bzlmod_v2/tests/root_module_dice.rs`;
7. `app/slug_bzlmod_v2/tests/registry_dice.rs`; and
8. `app/slug_loading_v2/tests/glob_invalidation.rs`.

No `slug_workspace_v2`, `slug_core_v2`, loading implementation, registry
implementation, Cargo, fixture, Host, root-module evaluation, mapping, or
command transport file may change. The implementation stops if any such edit
is required.

##### Existing raw DICE ownership

`WorkspaceRawFileKey { workspace, path }` already computes one
`WorkspaceRawFileValue` from `WorkspaceRawSnapshotKey`. The injected snapshot
retains `Arc<[u8]>`, Missing, and ReadError distinctly in an
`Arc<SortedMap<PathBuf, _>>`. Normal runtime observation reads bytes once,
derives text independently, and injects raw and text snapshots on one updater
before its sole commit. There is no manual lock, filesystem read, fallback
scanner, new key, or new identity decision in this packet.

For non-Off modes, `VisibleLockfileKey` must replace only its
`WorkspaceFileKey` dependency with `WorkspaceRawFileKey` and pass the raw bytes
to one `lockfile.rs` helper. Preserve the legacy Off early return and lack of a
file dependency; exact Bazel Off ownership remains reserved to the already
accepted later Host visible-lockfile owner. Preserve root-before-visible
composition and every other key dependency/order.

Every direct-DICE helper that reaches the visible key must inject a raw snapshot
on the same updater as its existing text snapshot. This includes
`tests/root_module_dice.rs`, `tests/registry_dice.rs`, and
`slug_loading_v2/tests/glob_invalidation.rs`; the latter two changes are
test-harness plumbing only, not registry/loading behavior. Existing text-only
cases derive raw values mechanically; only new invalid-byte cases provide an
explicit raw override. No helper may fall back from missing raw state to the
text key, and source-file UTF-8 behavior must not change.
The loading test tracker must classify `WorkspaceRawFileKey` as its existing
file-dependency identity so the active-mode raw edge and legacy Off absence
remain directly asserted.

##### Sole value, public surface, and deletions

Rename the accepted sole production struct to `BazelLockfile`. A
`cfg(test)`-only re-export may preserve the `BazelLockfileV28` spelling for the
accepted private unit module, which is outside this packet's allowlist; no
non-test alias, wrapper, adapter, or second schema may exist. Keep the complete
six fields, compact strings/shared slices, `SmallMap`/`SmallSet`/`SortedMap`,
custom Facts equality, `Allocative`, and direct streaming parser/renderer
unchanged.

Make the accepted parse/render error and error-surface types public through
bounded read-only accessors, then route public `parse_bazel_lockfile` and
`render_bazel_lockfile` directly through the sole owner. Keep
`empty_bazel_lockfile`, visible/hidden input/read/snapshot types, planning and
atomic apply, `BAZEL_9_LOCK_FILE_VERSION`, `RegistryFileExpectation`, and
`registry_file_expectation()`. The registry accessor projects typed
`RegistryFileHash`: missing URL is `Unrecorded`, `NotFound` is
`RecordedAbsent`, and `Sha256` returns the decoded 32 bytes. `registry_dice.rs`
stays byte-for-byte unchanged; its test file changes only to inject the raw
snapshot already supplied by production runtime observation.

Delete the old `BazelLockfileModuleExtension*`,
`BazelLockfileRecordedInput`, `BazelLockfileRepoSpec`,
`ModuleExtensionReplayInputs`, raw `serde_json::Value`/`BTreeMap` schema,
parser, renderer, marker scan, registry validators, all six general-only
replay validators, JSON helpers, and their exports. Exhaustive active-root
search proved those replay/validator surfaces have no production consumer.
Do not retain a deprecated spelling, conversion constructor, raw JSON field,
or compatibility export.

##### Exact read and error matrix

One byte helper must call `read_lockfile_v28` after the mode decision:

- absent is the exact empty v28 value;
- Update and Refresh use `ReturnEmpty`, so missing/noncurrent first markers
  become the exact empty value;
- Error uses `Error`, mapping only `UnsupportedVersion` to Bazel's exact
  unsupported-version diagnostic;
- `CaughtJsonSyntax`, `CaughtNullPointer`, and
  `CaughtIllegalArgument` use the visible `BAD_LOCKFILE` wrapper;
- `DirectAdapterJsonParse` and `DelimiterIndexOutOfBounds` remain
  distinguishable direct failures and are not rewritten as caught failures;
- Java-compatible replacement occurs before marker scan and typed parsing.

The visible string helper, if retained for callers/tests, delegates by
borrowing its UTF-8 bytes; it is not a second parser. Visible and hidden input
wrappers retain raw optional bytes and digest those exact bytes. Hidden parsing
uses Update/`ReturnEmpty`: absent, noncurrent, and caught failures fail open to
empty, while direct-adapter and delimiter failures propagate. Change the
test-only/public hidden helper to a fallible byte boundary rather than
flattening those uncaught holes. Merge-conflict advice, exact Bazel Off reads,
and the separate Host path/error owner remain explicitly outside this
correction; do not claim them.

##### Semantic planning and discriminating evidence

Update/Refresh planning must parse an existing value and compare the complete
semantic v28 value before rendering. Formatting, object order, checksum case,
build-suffix removal, factor spelling/order, and Starlark-equal numeric facts
prune to `Keep`; an unsupported/missing marker compares as empty. A recognized
v28 caught parse failure remains an error, not an overwrite. Error mode keeps
only an equal full value and otherwise returns the existing missing,
unsupported-version, or stale diagnostic. Rendering always emits the fixed six
fields and terminal newline.

Rewrite `tests/lockfile.rs` around the public production surface. It must prove:

- the accepted comprehensive extension/Facts slice renders exactly;
- all five recorded-input domains and every factor survive parse/render;
- the parse-failure sentinel and null-label repo rule remain non-renderable;
- every factor, metadata field, fact, and fact version participates in value
  equality and planner Write/Error decisions;
- semantic A-to-B-to-A planning and formatting/order-equivalent pruning;
- all three registry expectation outcomes;
- visible absent/current/noncurrent/malformed/marker-overflow behavior across
  Update, Refresh, Error, and unchanged legacy Off;
- hidden absent/noncurrent/caught fail-open and direct/delimiter propagation;
- raw visible/hidden input digests and invalid UTF-8 replacement;
- atomic apply writes only a prior `Write` plan and errors never write.

Strengthen the retained-DICE test in `tests/root_module_dice.rs` with raw
invalid-byte success, current caught/direct/delimiter failures, noncurrent
Update/Refresh empty versus Error diagnostic, formatting-equivalent downstream
reuse, and semantic A-to-B-to-A restoration on one engine. Prove the visible
key depends on the raw key in active modes and preserves the legacy Off
no-file-dependency shortcut. The registry and loading direct-DICE tests must
retain their existing assertions after raw-snapshot injection; unchanged core
tests remain downstream/public-wrapper evidence.

##### Validation, reuse, and stop gates

Reuse the retained Stage 9 compact-collection row and
`WorkspaceRawFileKey`/`WorkspaceRawSnapshotKey` substrate directly; reject the
V1 raw JSON/general-only replay shape. Run serially:

- `cargo fmt --all -- --check`;
- `cargo test -p slug_bzlmod_v2 lockfile`;
- `cargo test -p slug_bzlmod_v2`;
- `cargo test -p slug_loading_v2`;
- `cargo test -p slug_core_v2`;
- doctests for all three crates;
- GNU-Windows `--no-run` for all three crates;
- `scripts/v2_archive_status.sh`;
- `git status --short`; and
- `git diff --check`.

Require no matches from the three prior forbidden scans. Also require no match
from a multiline scoped scan
`rg -nU 'impl Key for VisibleLockfileKey[\\s\\S]{0,2500}WorkspaceFileKey\\s*\\{[\\s\\S]{0,500}MODULE\\.bazel\\.lock' app/slug_bzlmod_v2/src/module_eval.rs`
and no exact-name alias match from
`rg -n 'type BazelLockfile(V28)?\\b|(?:pub(?:\\([^)]*\\))?\\s+)?use .* as BazelLockfile\\b'`
over the eight-file scope.

The sole spelling bridge is exactly:

```rust
#[cfg(test)]
pub(crate) use BazelLockfile as BazelLockfileV28;
```

It must occur exactly once immediately after the renamed struct and be the
only `as BazelLockfileV28` match. This is private test compilation support for
the accepted out-of-allowlist unit module, not a production alias.

Stop and replan on any new key/lock, raw/text snapshot injection change outside
the test helper, altered root-module source semantics, registry consumer edit,
Host/Off activation, merge-conflict claim, generic JSON retention, non-test
alias, second parser/renderer, inability to preserve direct error surfaces, or
ninth implementation file.

##### Raw-read live-cutover correction design status

**Status:** Accepted after correction on 2026-07-26.

Exact-source, implementation-feasibility, and architecture/DICE latest-text
reviews all returned `ACCEPT`. The original six-file draft was corrected before
implementation because the direct-DICE registry and loading tests also injected
only text snapshots. The accepted eight-file boundary now gives every direct
caller same-updater raw state, teaches the loading tracker the raw file edge,
and preserves production's existing atomic raw/text snapshot injection without
a fallback, new key, lock, filesystem read, or core/workspace edit.

The packet freezes one sole renamed v28 value, typed parse/render errors,
selective caught/direct hidden and visible behavior, complete semantic planner
equality, registry tri-state projection, deletion of the old raw/general-only
schema and test-only replay APIs, the exact private test spelling bridge, and
multiline/alias forbidden scans. Exact Off reads, merge-conflict advice, and
the separate Host owner remain explicitly unclaimed. No Rust, tests, fixtures,
Cargo, dependency, API, DICE, or activation changed during design.

Next evidence: Implement only
`WP-5-m1-bazel-lockfile-v28-raw-read-live-cutover` under the exact eight-file
contract and serial validation matrix above.

#### Lockfile v28 direct-DICE test closure correction

Run only
`WP-5-m1-bazel-lockfile-v28-direct-dice-test-closure-correction-design`.
This is a read-only correction packet. Preserve the held eight-file
implementation draft, but do not edit another Rust file or run another Cargo
command until this correction receives terminal review.

##### Observed stop and retained implementation

The held draft implemented the accepted sole-schema/raw-read cutover in exactly
the eight allowed files. Its test-first focused run passed 123 matched tests,
formatting, whitespace, old-schema, alias, and raw-versus-text-key gates. Parent
validation then ran the complete bzlmod crate and found three failures in
`tests/source_preparation_dice.rs`: direct DICE updaters injected text but not
`WorkspaceRawSnapshotKey`, so active `VisibleLockfileKey` correctly failed
closed on the missing injected raw snapshot. One earlier manually constructed
registry updater had the same defect and was corrected inside its already
allowed test file before the complete-crate rerun.

`source_preparation_dice.rs` is a ninth implementation file, so the accepted
stop gate fired. No production fallback, new key, lock, filesystem read,
snapshot-owner change, or dependency removal is permitted. The implementation
draft remains uncommitted and no validation beyond the failing complete
bzlmod run is claimed.

##### Exact corrected closure

An app-wide scan of every `inject_root_module_request_inputs` caller and its
downstream keys expands the atomic implementation boundary to exactly sixteen
files:

1. `app/slug_bzlmod_v2/src/lockfile_v28.rs`;
2. `app/slug_bzlmod_v2/src/lockfile.rs`;
3. `app/slug_bzlmod_v2/src/lib.rs`;
4. `app/slug_bzlmod_v2/tests/lockfile.rs`;
5. `app/slug_bzlmod_v2/src/module_eval.rs`;
6. `app/slug_bzlmod_v2/tests/root_module_dice.rs`;
7. `app/slug_bzlmod_v2/tests/registry_dice.rs`;
8. `app/slug_loading_v2/tests/glob_invalidation.rs`;
9. `app/slug_bzlmod_v2/tests/source_preparation_dice.rs`;
10. `app/slug_loading_v2/tests/build_file_loading.rs`;
11. `app/slug_loading_v2/tests/bzl_invalidation.rs`;
12. `app/slug_analysis_v2/tests/starlark_rule.rs`; and
13. `app/slug_query_v2/tests/loading_query.rs`;
14. `app/slug_analysis_v2/Cargo.toml`;
15. `app/slug_query_v2/Cargo.toml`; and
16. `Cargo.lock`.

The first eight files retain their accepted contract unchanged. The five added
Rust files are test-harness plumbing only. In each existing active-mode updater,
derive a `WorkspaceRawSnapshot` mechanically from the same text snapshot and
inject it with `WorkspaceRawSnapshotKey` on that updater before its sole
commit. Also correct the third manual updater already inside
`glob_invalidation.rs`. Preserve explicit raw overrides only in the accepted
invalid-byte lockfile tests. Do not change production loading, analysis, query,
source-preparation, core, workspace, or registry behavior.

Analysis and query do not directly depend on `slug_workspace_v2`, and Rust must
not name the raw types through an unexposed transitive dependency. Add exactly
`slug_workspace_v2 = { workspace = true }` to each crate's
`[dev-dependencies]`. `Cargo.lock` may change only by adding
`slug_workspace_v2` to the existing local `slug_analysis_v2` and
`slug_query_v2` dependency arrays; no package, version, checksum, source, or
other dependency may change. Do not re-export raw workspace types from
production loading or add a production helper merely for tests.

The required sites are the three text-only source-preparation updaters, the
single build-file-loading updater, two bzl-invalidation active package
updaters, two loading-query updaters, three Starlark-rule analysis updaters,
and the third glob-invalidation updater. Direct `.bzl` evaluation helpers that
do not inject a root request and do not compute `PackageLoadKey` remain
unchanged. `host_module.rs` retains its separate dormant Host path. Normal core
runtime observation already injects raw and text snapshots together and does
not change.

This closure is semantically required: source preparation consumes
`RootModuleFilesKey`; loading, analysis, and query reach `PackageLoadKey`,
which unconditionally computes `RootModuleGraphKey`. Both paths legitimately
retain the active visible-lockfile edge. Removing or bypassing it would change
failure ordering and dependency ownership rather than correct the test
transaction.

##### Validation and stop gates

After terminal acceptance, resume the held implementation and run serially:

- `cargo fmt --all -- --check`;
- `cargo test -p slug_bzlmod_v2 lockfile`;
- `cargo test -p slug_bzlmod_v2`;
- `cargo test -p slug_loading_v2`;
- `cargo test -p slug_analysis_v2`;
- `cargo test -p slug_query_v2`;
- `cargo test -p slug_core_v2`;
- doctests for all five crates;
- GNU-Windows `--no-run` for all five crates;
- the original old-schema, generic-JSON, general-only, multiline text-key,
  exact-alias, and one-bridge scans over the thirteen Rust files;
- an app-wide inventory of root-request injection sites proving every active
  path has matching raw snapshot injection or is production's existing atomic
  runtime path;
- `scripts/v2_archive_status.sh`;
- `git status --short`; and
- `git diff --check`.

Stop and replan on a seventeenth implementation file, any production edit
outside the original five Rust source files, a raw/text fallback, new key,
lock, read, altered root/source/package/query/analysis semantics, Cargo or
fixture change beyond the exact two dev-dependency manifest lines and two local
lockfile dependency-array entries, Host/exact-Off activation, or inability to
keep every added test snapshot on the existing same updater and commit.

##### Direct-DICE closure correction design status

**Status:** Accepted after two closure corrections on 2026-07-26.

Exact-source/call-graph, implementation-feasibility, and
architecture/orchestration latest-text reviews all returned `ACCEPT`. The
first correction expanded the observed ninth test file to the complete
thirteen-Rust-file active direct-DICE closure. The second added direct
test-only `slug_workspace_v2` dependencies for analysis and query plus the
strictly bounded local `Cargo.lock` entries, avoiding a production loading
re-export. No Rust implementation, fixture, production API, DICE key, lock,
filesystem read, or Cargo command changed during correction design; the
uncommitted eight-file implementation draft remains held.

Next evidence: Resume only
`WP-5-m1-bazel-lockfile-v28-raw-read-live-cutover` under this exact
sixteen-file correction and restart the serial validation matrix after all
twelve missing updater sites have matching same-updater raw snapshots.

##### Raw-read live-cutover implementation status

**Status:** Accepted after terminal corrections on 2026-07-26.

The cutover now has one sole production Bazel v28 value and live raw-byte
visible-lockfile key. It deletes the legacy general-only schema and replay
parser/renderer while preserving caught versus direct failures, exact byte
digests, registry tri-state projection, semantic update planning, and atomic
keep/delete/write apply. All twelve missing active direct-DICE updater sites
now inject mechanically derived raw snapshots on the same updater and commit.
Analysis and query own only their direct test-time `slug_workspace_v2`
dependencies. The local root `Cargo.lock` is intentionally ignored and carries
only the corresponding two dependency-array entries, so the accepted local
boundary is sixteen files and the tracked implementation boundary is fifteen.

Focused lockfile coverage passed 123 matched tests. The complete native suites
passed bzlmod 361, loading 54, analysis 12, query 61, and core 115 tests; all
five doctest suites passed with zero tests; all 27 GNU-Windows test executables
compiled and linked. Formatting, archive status, diff checks, exact file and
forbidden-symbol scans, direct-dependency checks, and the app-wide active
updater inventory passed. The first full bzlmod run exposed the incomplete
source-preparation test harness and expanded the provisional eight-file
boundary to the accepted sixteen-file closure. Terminal review then corrected
atomic apply evidence to include Keep and corrected marker scanning to compare
Bazel's parsed first textual integer, including adversarial leading-zero
markers. Exact-source/parity, implementation/evidence, and
architecture/orchestration latest-diff rereviews all returned `ACCEPT`.

No fixture, production re-export, fallback, new key, lock, filesystem read,
Host or exact-Off activation, or merge-conflict claim entered. Next evidence:
design and rereview only `WP-5-m1-host-visible-lockfile-boundary-design` now
that the exact full-v28 value/error owner is live; do not resume the old
provisional Host contract without that review.

#### Host visible-lockfile boundary correction

Run only `WP-5-m1-host-visible-lockfile-boundary-design`. This correction
supersedes the provisional sequence and historical `REPLAN` under Serial
packet 9. It is design only: do not edit Rust or fixtures and do not run Cargo.

Commit `6100c33b` removes the old blocker. Production now has one sole complete
Bazel v28 value, Java-compatible raw-byte reader, parsed first textual integer
marker, semantic equality, and typed caught/direct error surfaces. Pinned
Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`,
especially `BazelLockFileFunction.java` and `GsonTypeAdapterUtil.java`, remains
the parity authority.

##### Oracle-first boundary

First run only `WP-5-m1-host-visible-lockfile-oracle`. Add exactly seven
regular files under a new `host-visible-lockfile-boundary` fixture:

- `fixture.toml` and generated `expected/oracle.json`;
- root `workspace/MODULE.bazel`;
- immutable `workspace/lockfiles/invalid-utf8-v28.lock`; and
- minimal `workspace/registry/bazel_registry.json`,
  `workspace/registry/modules/subject/1.0.0/MODULE.bazel`, and
  `workspace/registry/modules/subject/1.0.0/source.json`.

Use cumulative `bazel mod graph` commands with
`--registry=file://%workspace%/registry` so `RegistryFunction` requests the
visible value while the file-registry consumer later ignores recorded hashes.
The small fixture-local registry is intentional isolation for this call graph;
reuse established deterministic file-registry shapes, but do not depend on or
mutate another fixture's state. Pin exactly these nine ordered rows:

1. absent lockfile in Off succeeds with the exact graph and no visible
   manifest;
2. a populated valid v28 lockfile in Off succeeds with the same graph and
   retains the visible manifest;
3. recognized-v28 malformed JSON in Off is caught as exit 48
   `BAD_LOCKFILE`;
4. a first textual noncurrent marker followed by malformed/current-looking
   content succeeds as empty in Off;
5. that same noncurrent content in Error is exit 48 with the exact unsupported
   version diagnostic;
6. recognized-v28 merge-conflict content is caught as exit 48 with Bazel's
   merge-conflict advice suffix;
7. move the immutable invalid-UTF8 v28 asset into place and prove replacement
   bytes inside an ignored string succeed in Off;
8. delete the visible file and prove Error succeeds as empty; and
9. create recognized-v28 invalid-checksum content in Off and prove the direct
   `JsonParseException` escape as Bazel's exit-37/internal-error class, without
   the caught `BAD_LOCKFILE` wrapper.

The accepted `bazel-lockfile-v28-schema` oracle remains the complete value and
adapter authority, including its other two direct-hole rows. This fixture
adds only Host read/mode discrimination. Pin release/commit/source provenance,
exact exit and message shapes, graph output, every cumulative mutation, and
the visible manifest. Run one pinned generation and two fresh-root replays,
then source-anchor, fixture-schema, exact-file, mutation, manifest, archive,
and diff checks. Cap growth at exactly seven regular files, zero symlinks, and
900 newline-counted lines. The accepted fixture-growth checkpoint remains
`df812c2c`; `eb8c2d23` is the first accepted oracle after it and this is the
second, so no new checkpoint is due.

Stop and replan on an eighth file, tenth row, HTTP server, harness/schema
change, shared mutable fixture state, external network, generated lockfile
template, archive/symlink, registry-consumer hash claim, unstable
path/port/stack text, or an observed exit/message shape that differs from the
pinned contract.

##### Private Host owner

Only after oracle `ACCEPT`, implement
`WP-5-m1-host-visible-lockfile-owner` in exactly three files:

- new private `app/slug_bzlmod_v2/src/host_lockfile.rs`;
- `app/slug_bzlmod_v2/src/lockfile.rs`; and
- private `mod host_lockfile;` in `app/slug_bzlmod_v2/src/lib.rs`.

Do not edit `repository_ignore.rs` or `lockfile_v28.rs`. The live
`read_lockfile_v28` already owns Java UTF-8 replacement before marker
selection and full typed parsing; no second decoder, parser, value, or
collection may enter.

Add only crate-private `HostVisibleLockfileKey`,
`HostVisibleLockfileValue`, and `HostVisibleLockfileError`. The key identity is
one `NormalizedAbsolutePath` workspace. Its result is:

```text
PathOutcome<
    Arc<Result<HostVisibleLockfileValue, HostVisibleLockfileError>>
>

HostVisibleLockfileValue {
    lockfile: Arc<BazelLockfile>
}
```

The error variants are exactly:

```text
LockfileModeInput {
    workspace: NormalizedAbsolutePath,
    message: CompactString,
}
File {
    error: HostFileError,
}
BadLockfile {
    message: CompactString,
}
UncaughtParse {
    error: LockfileParseError,
}
```

Expose only a crate-private `lockfile() -> &Arc<BazelLockfile>` accessor. The
full value retains all six semantic fields. It retains no mode, path, bytes,
digest, formatting, missing/noncurrent discriminator, write state, or
`Ignored` variant. Derive or implement `Allocative`; cheap duplication is the
containing `Arc`, not cloned maps. Add no retained collection, standard
hash-map/set, interner, cache, lock, dependency, or public re-export.

In `lockfile.rs`, add one crate-private typed Host parser entry that calls
`read_lockfile_v28` directly. Off, Update, and Refresh use `ReturnEmpty`;
Error uses `Error`. Absent and noncurrent inputs produce
`Arc::new(empty_bazel_lockfile())`. Unsupported Error mode maps to Bazel's
exact unsupported-version message. `CaughtJsonSyntax`, `CaughtNullPointer`,
and `CaughtIllegalArgument` map to `BadLockfile` with Bazel's exact
`BAD_LOCKFILE` wrapper, choosing merge-conflict advice when the caught message
contains `<<<<<<<`, `=======`, `|||||||`, or `>>>>>>>`. Preserve
`DirectAdapterJsonParse` and `DelimiterIndexOutOfBounds` as
`UncaughtParse { error }`; never add the caught wrapper to them.

Compute `HostFileBytesKey(workspace/MODULE.bazel.lock)` first. A path Need
returns immediately and creates no mode dependency. After every Complete file
outcome, compute `RootModuleLockfileModeKey` before interpreting Missing,
Present, or typed file error. A missing injected mode, file failure,
unsupported/caught failure, and direct parse hole remain distinct Complete
errors. The key uses complete-only equality and validity: every Need is
invalid and self-unequal, while separately allocated semantically equal
`Arc<BazelLockfile>` values compare equal.

Focused evidence must cover workspace-only key identity; file-before-mode
order and no mode edge during Need; mode acquisition after Missing, Present,
and file-error Complete outcomes; cumulative Needs; missing mode; missing,
regular, special-file, symlink, and operational-error paths; one retained
engine across create/edit/delete/recreate and mode A→B→A; all four
mode/current/noncurrent/caught cells; first marker, leading-zero marker, and
integer overflow; ordinary and merge-conflict caught diagnostics; malformed
Java UTF-8; both direct error surfaces; every full-value field; separately
allocated semantic equality; and formatting/key-order byte recomputation that
prunes a downstream semantic projection.

Validate serially with focused Host tests, the complete bzlmod suite, bzlmod
doctests, GNU-Windows `--no-run`, formatting, archive status, diff checks,
exact-three-file scope, private-symbol/dependency scans, and forbidden
root/registry/loading/core/analysis/query/hidden/write/direct-IO/event/mapping
and activation references. Stop on a fourth file, fixture or Cargo change,
new dependency, public surface, direct filesystem IO inside DICE, mode
dependency during Need, incomplete equality, copied UTF decoder, flattened
direct failure, retained raw/formatted state, registry consumption, root
ordering, mapping, or Host activation.

Only owner `ACCEPT` advances to the existing
`WP-5-m1-host-registry-function-boundary-design`. That later packet owns
root-before-registry composition, unconditional visible-value acquisition,
registry policy/IO, hash and yanked consumption, and final handoff; this
packet changes none of them.

##### Host visible-lockfile correction design status

**Status:** Accepted after terminal correction review on 2026-07-26.

Pinned-source/parity review resolved the outer caught exception set against
the already accepted exit-37 direct adapter and delimiter evidence.
Implementation review confirmed the three-file seam can call the sole v28
reader without another decoder or collection owner. Architecture/orchestration
review accepted the nine-row isolated oracle, exact fixture-growth accounting,
file-before-mode Need boundary, typed uncaught errors, complete-only semantic
equality, validation matrix, and later registry handoff. All three latest-text
reviews returned `ACCEPT`; no Rust, fixture, Cargo, dependency, or activation
changed.

Next evidence: Run only `WP-5-m1-host-visible-lockfile-oracle` under the exact
seven-file, nine-row contract above.

##### Host visible-lockfile oracle embedded-closure correction

**Status:** Accepted after stopped first generation and terminal correction
review on 2026-07-26.

The exact seven-file draft stopped before exercising a lockfile row. All nine
generated commands exited 37 while resolving
`<root> -> bazel_tools@_ -> rules_license@1.0.0`: the accepted sole local
registry could not supply Bazel's injected embedded module closure. The
generated expected file records only that prerequisite failure and is not
evidence. No replay ran and no draft file is accepted.

A native temporary-copy observation found the smaller boundary:
`bazel query //:data.txt --lockfile_mode=off` in the existing
`lockfile-mode-off` fixture reads a recognized malformed v28 lockfile while
computing the main repository mapping and exits 48 with Bazel's caught
`BAD_LOCKFILE`, without resolving the mod-graph embedded closure. Correct only
`WP-5-m1-host-visible-lockfile-oracle-embedded-closure-correction` by removing
the held untracked `host-visible-lockfile-boundary` draft and extending the
existing fixture in exactly three paths:

- modify `tests/v2_oracle/fixtures/lockfile-mode-off/fixture.toml`;
- regenerate
  `tests/v2_oracle/fixtures/lockfile-mode-off/expected/oracle.json`; and
- add
  `tests/v2_oracle/fixtures/lockfile-mode-off/workspace/lockfiles/invalid-utf8-v28.lock`.

Keep its existing `MODULE.bazel`, `BUILD.bazel`, and `data.txt`
byte-identical. The final fixture has exactly six regular files, zero
symlinks, and at most 700 newline-counted lines. Net growth is one regular
file, zero links, plus the measured line delta. The fixture-growth checkpoint
remains `df812c2c` at accepted baseline tree `c039c347`;
`eb8c2d23` is packet one after it and this is packet two, so no checkpoint is
due.

Use exactly nine cumulative `query //:data.txt --noshow_progress` rows with
the same semantic order and byte mutations accepted above. Row 1 preserves
the original absent Off/no-write evidence. All successful stdout is exactly
`//:data.txt`. There is no fixture registry file, HTTP server, BCR/network
assertion, injected closure scaffold, or accepted registry policy ownership.
Only row 8 adds the exact scheme-less registry token named below as an
ordering sentinel; it creates no file or network state. The
populated row may retain an unrelated valid registry hash only as parser/value
evidence. Row 3 proves the ordinary caught wrapper and delete advice but not
merge advice. Rows 4 and 5 share the first-noncurrent-marker content without
an intervening mutation. Row 6 uses a recognized nested marker plus a
top-level integer field whose caught conversion message contains `<<<<<<<`,
proving merge advice and no delete advice. Row 7 deletes the current file then
renames the immutable malformed-UTF8 asset into place.

Row 8 deletes the visible file and adds exactly
`--registry=host-visible-lockfile-invalid-registry`. It remains exact
comparison and expects exit 48, empty stdout, and an absent manifest. It
asserts the exact `Invalid registry URL:
host-visible-lockfile-invalid-registry: Registry URL has no scheme` diagnostic
and supported-schemes suffix while negatively excluding unsupported version,
caught parse, merge/delete advice, and fatal/direct surfaces.
`RegistryFunction` requests the visible value before
`RegistryFactoryImpl.createRegistry`, so this proves only that missing-file
reading returned EMPTY and advanced to URL validation. It accepts no registry
policy, fetch, or production consumer ownership. Any different parsing layer,
exit, message, or replay is a stop. Row 9 remains last and proves exit-37
invalid-checksum `JsonParseException` without the caught wrapper.

Keep fixture-wide `manifest_roots = ["MODULE.bazel.lock"]`. Rows 1 and 8 use
`compare = "exact"` because the semantic comparator does not enforce an empty
expected manifest. Other successful rows may use exact normalized output;
present-file failure rows use semantic comparison plus strong positive and
negative message patterns so their manifest digests are checked without
pinning the complete crash stack. `--noshow_progress` suppresses the
nondeterministic progress-line count that otherwise makes exact empty-manifest
rows unstable; both fresh-root replays must still match exactly. Record Bazel
9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, the
lockfile/parser and observed query dependency seams through
`RepositoryMappingFunction`, `RegistryFunction`, and `RegistryFactoryImpl`,
reuse rationale, exact asset size/digest, row order, mutations, and manifests.

Run one pinned generation and two distinct fresh-root replays, followed by
fixture schema, exact row/order/mutation/manifest, source-anchor, exact
three-path/final-six-file, invalid-UTF8 regular-file, line-growth, archive,
diff, normalized path/stack, and credential scans. Stop on a different
exit/message shape, a fourth retained path, a seventh fixture file, symlink,
more than 700 lines, harness/other-fixture/plan/Rust/Cargo edit, registry or
network scaffold, or any claim beyond visible lockfile read/parse behavior.

Native feasibility and architecture/orchestration correction reviews returned
`ACCEPT`. Pinned-source review of the original nine rows had already accepted
their caught/direct semantics; the new native Bazel 9.2 observation replaces
only the command path and fixture owner. Two stopped generations first exposed
an absent-Error missing-checksum sentinel whose selected embedded module
drifted, then replaced it with the deterministic scheme-less URL sentinel
above. A native probe and pinned-source, native-feasibility, and
orchestration-policy reviews accepted the final narrowed boundary. Resume only
the corrected existing fixture.

##### Host visible-lockfile oracle correction implementation status

**Status:** Accepted after terminal latest-diff review on 2026-07-26.

The existing `lockfile-mode-off` fixture now owns nine cumulative query rows
in exactly three changed paths. Pinned Bazel 9.2 generation and two distinct
fresh-root replays passed with exits
`0,0,48,0,48,48,0,48,37` and visible-manifest counts
`0,1,1,1,1,1,1,0,1`. The final absent-Error row uses the exact scheme-less
registry URL only as a deterministic read-before-URL-validation sentinel; the
direct invalid-checksum crash remains last.

The fixture has six regular files, zero symlinks, and 553 newline-counted
lines, a net increase of one file and 500 lines over its prior state. The
immutable malformed-UTF8 asset is 39 bytes with SHA-256
`f07fad0a50495205bd772b0db81ca5f2cf0094aec83aa260d884e0f1c2eaaa20`.
Existing `MODULE.bazel`, `BUILD.bazel`, and `data.txt` remain byte-identical.
All source anchors, row/order/mutation/manifest, schema, asset, archive, diff,
path/stack normalization, credential, and exact-scope gates passed.
Pinned-source/parity and architecture/orchestration terminal reviews returned
`ACCEPT`; the writer's complete validation returned `ACCEPT`.

No failed draft or expected output, fixture registry, server, network claim,
harness, Rust, Cargo, dependency, registry consumer ownership, or activation
is retained. Next evidence: Implement only
`WP-5-m1-host-visible-lockfile-owner` under the exact three-file private-owner
contract above.

##### Host visible-lockfile owner merge-message replan

**Status:** Replanned before accepted Rust on 2026-07-26.

The exact three-file draft established the private Host key/value/error seam
and six of seven focused tests passed. Its retained-engine evidence covers the
file-before-mode dependency boundary, cumulative Needs, Complete file forms,
mode transitions, filesystem forms and errors, all four parse modes, full
value/equality, direct failures, and opaque downstream semantic pruning. The
seventh exact diagnostic test exposed an unimplementable boundary rather than
a Host-wrapper test defect.

Pinned Bazel 9.2 `BazelLockFileFunction.compute` selects merge advice only when
the caught exception message contains a conflict marker. For the accepted
`"lockFileVersion":"<<<<<<<"` row, Gson retains that spelling in the caught
message. Slug's sole `read_lockfile_v28` instead reaches
`json_reader_next_i32` and replaces the spelling with the fixed message
`invalid signed 32-bit integer`. The Host entry therefore sees no marker and
incorrectly selects delete advice.

Scanning the complete raw input in `lockfile.rs` is not equivalent: an ignored
field may contain `<<<<<<<` while an unrelated ordinary caught error occurs.
Contextual rescanning would be a forbidden second decoder/parser. Independent
pinned-source/parity and architecture/orchestration terminal reviews therefore
returned `REPLAN`. The held uncommitted draft is not accepted production; no
fixture, Cargo, dependency, public API, consumer, activation, direct IO, or
collection change is accepted.

Next run only
`WP-5-m1-host-visible-lockfile-owner-merge-message-correction-design`. It is
design only: do not edit Rust and do not run Cargo. Freeze the smallest
four-file correction by adding only
`app/slug_bzlmod_v2/src/lockfile_v28.rs` to the prior allowlist. That file may
change only the failed `java_parse_double` message arm in
`json_reader_next_i32`, using its existing decoded `spelling`, and only when
`domain == AdapterDomain::Version`. Every non-Version domain retains the
existing `invalid signed 32-bit integer` message byte-for-byte. For bytes
`{"decoy":{"lockFileVersion":28},"lockFileVersion":"<<<<<<<"}`, the full
`read_lockfile_v28(..., ReturnEmpty)` entry must return the unchanged
`CaughtIllegalArgument` surface,
`InvalidAdapterValue { domain: Version }` kind, and `None` position, with
exact Display text
`java.lang.NumberFormatException: For input string: "<<<<<<<"`. Add the
focused full-entry regression inline under `#[cfg(test)]` in
`lockfile_v28.rs`; do not edit the separate `lockfile_v28_tests.rs`, which
would be a fifth file. The Host wrapper must continue classifying only
`error.to_string()`.

The corrected evidence must pin the complete ordinary and merge
`BAD_LOCKFILE` messages. The exact merge result is
`Failed to read and parse the MODULE.bazel.lock file with error: java.lang.NumberFormatException: For input string: "<<<<<<<". This looks like a merge conflict. See https://bazel.build/external/lockfile#merge-conflicts for advice.`
Add the exact negative input
`{"decoy":{"lockFileVersion":28},"ignored":"<<<<<<<","lockFileVersion":"ordinary"}`
and pin its complete result to
`Failed to read and parse the MODULE.bazel.lock file with error: java.lang.NumberFormatException: For input string: "ordinary". Try deleting it and rerun the build.`
This row must contain no merge advice. Retain the separately allocated equal
Complete Host-key values and every other owner evidence/validation gate above.

Stop on raw/source marker inspection in the Host helper, another
decoder/parser/error type, any other parse-error message/surface/kind/position
change, any change to marker scanning, successful numeric parsing, the later
exact-range failure, or direct surfaces, a fifth file, fixture or Cargo
change, public surface, consumer, activation, or broadened parser work.
Terminal `ACCEPT` of this design authorizes only the exact four-file corrected
owner implementation. Only terminal `ACCEPT` of that implementation may
advance to `WP-5-m1-host-registry-function-boundary-design`.

##### Host visible-lockfile owner merge-message correction design status

**Status:** Accepted after terminal latest-text review on 2026-07-26.

The corrected packet adds only `lockfile_v28.rs` to the prior three-file Host
owner allowlist. The sole parser's failed Version-domain integer conversion
must retain the exact Gson/Java class-prefixed offending-spelling message
while preserving its surface, kind, position, all non-Version messages, and
every successful and range-failure numeric behavior. A full-reader inline
regression avoids opening the separate test file.

Exact complete Host merge/delete diagnostics and the ignored-field-marker plus
ordinary bad Version discriminator prove message-only classification without
raw-input inspection. The prior retained-DICE, equality, lifecycle,
projection, parser, scope, and serial validation gates remain unchanged.
Pinned-source/parity, implementation-feasibility, and
architecture/orchestration terminal latest-text reviews all returned
`ACCEPT`; no Rust edit or Cargo command occurred during the design packet.

Next evidence: Implement only
`WP-5-m1-host-visible-lockfile-owner-merge-message-correction` under the exact
four-file contract above.

##### Corrected Host visible-lockfile owner implementation status

**Status:** Accepted after terminal latest-diff review on 2026-07-26.

Exactly four Rust files now own the dormant private Host visible-lockfile
boundary. `HostVisibleLockfileKey` computes `HostFileBytesKey` first, returns
every path Need without a mode edge, and requests
`RootModuleLockfileModeKey` before interpreting every Complete file outcome.
Missing, file failure, mode-input failure, unsupported/caught parse failure,
and the two direct parse holes remain distinct Complete results. The retained
value is only an `Arc<BazelLockfile>` behind the containing result Arc, and
complete-only equality/validity uses the sole six-field semantic value.

The Host parser calls `read_lockfile_v28` directly. Off, Update, and Refresh
return empty for noncurrent input; Error retains the exact unsupported
diagnostic. Caught errors receive Bazel's exact delete or merge wrapper based
only on the typed error message. The sole reader's failed Version-domain
conversion now retains
`java.lang.NumberFormatException: For input string: "<<<<<<<"` while the
Facts/non-Version path remains exactly `invalid signed 32-bit integer`.
Inline full-reader regressions pin both branches, unchanged surface/kind/None
position, and exact Display text. The ignored-field marker plus ordinary bad
Version row proves no raw-input classification.

Focused Host/parser validation passed 9 tests. The complete bzlmod crate
passed 370 tests: 186 unit tests and all eleven integration suites; doctests
passed with zero tests. GNU-Windows no-run linked all 12 test executables.
Formatting, archive status, diff checks, exact four-file scope, private-symbol
and dependency scans, and the forbidden fixture/Cargo/repository-ignore,
separate-test-file, public API, direct-IO, collection, consumer, registry, and
activation gates passed.

Pinned-source/parity, implementation/evidence, and
architecture/orchestration terminal latest-diff reviews all returned
`ACCEPT`. No fixture, Cargo, dependency, public re-export, registry
consumption, root ordering, mapping, write state, or activation changed.
Next evidence: Run only
`WP-5-m1-host-registry-function-boundary-design`; it is design only.

#### Later activation gate: bootstrap and Host switch

After accepted Host visible-lockfile and registry ownership, design only
`WP-5-m1-root-module-bootstrap-activation`. It must reopen the private native
demand driver and name packet 2's stateless owner as a retained
`WorkspaceRuntime` field; native apply remains outside DICE. Bootstrap is the
sole first progress class when its request coexists with repository or path
Needs. Seal and drop the speculative transaction, apply the exact request,
and after either AlreadyPresent or Created rebuild the path epoch by freshly
reobserving and replacing all current Host-namespace observations while
retaining independent materialization-namespace observations. Merging a new
leaf into the stale epoch is forbidden: a dangling logical root may have
recorded Missing on its resolved target.

Record the exact request, apply result, and refreshed epoch in the command
transition. A repeated equal request may retry only when apply plus Host
replacement changed command state; equal request plus equal refreshed Host
epoch is typed bootstrap internal-nonprogress. There is no retry cap. A typed
native apply/create failure is a dedicated native-command error: it does not
retry or become a DICE error, restores/aborts through the accepted fail-closed
path, publishes no warnings or events, and does not attempt to undo any
already-performed filesystem effect.

The command owns any Created warning token across later retries. It is
neither a DICE value nor a DICE-reachable event batch. Multiple Created
results coalesce to one exact pending warning. After exact terminal closure
selection, selected success or semantic failure prepends one command-owned
synthetic packet-3a Warning diagnostic to the selected
REPO/root-module/Starlark batches and moves that deterministic prefix into
`CommandOutputBuffer` through the existing acceptance order. This preserves
Bazel's warning-before-reminder evaluation order. Warm and
AlreadyPresent-only commands are silent; internal abort/fail-closed paths
drop the token while preserving the already-performed filesystem effect.

Tests must cover bootstrap-before-repository/path progress, create/warn, warm
silence, edit preservation, delete/recreate/warn, dangling-target
replacement, AlreadyPresent refresh, repeated-equal strict nonprogress,
multiple-Created coalescing, typed apply failure, retry failure, terminal
success/failure ordering, cancellation, restoration, and exact once-only
publication.

Only terminal acceptance of activation may authorize public command-input
transport, Host source-preparation switch, or a production Host root
consumer. Package-root/deleted/vendor/REPO-semantics request injection must be
vertically atomic with that switch. The outside-DICE request preflight owns
raw package-path spelling, `%workspace%` and client-cwd resolution,
command-boundary existence filtering, and the exact deferred warning for a
relative entry when cwd differs from workspace. That warning joins the same
command-owned terminal prefix in Bazel order; no workspace default may mask
missing input.

#### Common validation and hard stops

Every Rust packet runs focused owner tests, full bzlmod tests/doctests,
downstream loading/core suites, GNU-Windows no-run linkage for changed owners
and downstream binaries, formatting, diff, archive, dependency, exact-file,
private-surface, and forbidden-reference scans. Cargo commands sharing a
target directory remain serial. New compact retained collections use
Buck2-derived `SmallMap`/`SmallSet`, `Arc` slices, and normalized OS-native
paths; add no standard hot-path map/set, surrogate string path identity,
interner, cache, lock, or dependency.

Stop and replan on direct filesystem IO inside DICE; a mutation-shaped path
Need; root creation from a key; stale Missing merge; a DICE-owned synthetic
bootstrap warning; direct or duplicate REPO output while capture is present;
suppressed normal REPO print/diagnostic output while capture is absent;
dropped captured REPO events; a literal-only REPO parser;
loading package/glob reuse; workspace-only package-root default; lost selected
package-root identity; physical-path-derived include identity; directory
listing or bytes for BUILD lookup; regular-only include semantics; public
Host keys before activation; WORKSPACE evidence; CLI/server activation before
the vertical switch; or any production discovery/source-preparation consumer
before packet 9 terminal acceptance.

Three independent latest-text terminal reviews returned `ACCEPT`. The single
bounded correction replaced source-level `repo(**kwargs)` with Bazel's
explicit-keyword-only surface, froze package-root/build-name cross-product
ordering and sequential replacement-decoded `.bazelignore`, added exact
Off/Warning/Error REPO parsing plus a neutral diagnostic event prerequisite,
made injected projections explicit, and separated captured DICE events from
the command-owned bootstrap warning prefix. No Rust, fixture, harness, Cargo,
or production file changed.

Next evidence: Implement only
`WP-5-m1-root-main-package-policy-oracle`.

### Stage 5 root main-package-policy oracle

Status: Accepted

The isolated `root-module-package-policy` fixture retains exactly the frozen
six files and 38 Bazel 9.2 commands in 1,650 newline-counted lines. It proves
same-horizon preflight before any root/include event, exact first-source
failure and ordered package-root search lists, package-root-before-BUILD-name
selection, alternate-root bytes with the logical include traceback,
`BUILD.bazel` priority without lower-priority invalidation, all four main
deleted-package spellings and a distinct nonmain literal, first-readable
`.bazelignore`, literal ignore entries, full REPO evaluation/call errors,
`*`/`?`/`**` prefix matching, cold/change-only REPO prints, and rejecting
policy deletion plus vendor-option recovery in one retained daemon.

Pinned-source `//external` evidence remains outside the fixture because the
fixed allowlist and mutation schema cannot seed its missing parent directory.
A literal absolute outside-workspace vendor argv remains source-backed because
the harness has no run-directory argv token; `--vendor_dir=..` proves the same
resolved-absolute containment branch without writing through `/tmp` or `/`.
There is no registry, dependency, platform, checked-output, WORKSPACE, harness,
special-file, or duplicated asset scaffold.

Generation at `20260726-034413` and independent replay at
`20260726-034549` passed. The corrected latest assertions passed fresh replays
at `20260726-035133` and `20260726-035413`. The bounded correction made
BUILD-name priority discriminating, made `.bazelignore` and REPO deletion
recover from rejecting states, pinned complete REPO diagnostics, added
contained-vendor recovery, and made both horizon failures retain the exact
logical label, source location, complete ordered root list, and no-event
boundary. All three terminal latest-text reviews returned `ACCEPT`.
`python3 -B -m pytest tests/v2_oracle` remains unavailable because the system
Python has no `pytest` module; this is an environment residual, while fixture
schema loading and every Bazel comparison passed through the oracle runner.

Next evidence: Run only the required focused five-packet fixture-growth
checkpoint against accepted baseline `42e38bc3` before any Rust packet.

### Stage 5 root-module bootstrap request owner

Status: Accepted

The public bzlmod domain seam now owns a normalized-workspace
`RootModuleBootstrapRequest`, its lexically derived logical
`MODULE.bazel` path, exact 399-byte reminder and SHA-256, fieldless exact
warning token, `AlreadyPresent`/`Created` apply result, and typed create error
with normalized logical path, `PathIoErrorKind`, and raw OS code. The private
module is re-exported explicitly; it contains no filesystem, physical-path,
DICE, event, retry, generation, or terminal-policy behavior.

`SourcePreparationNeeds` carries one optional bootstrap request independently
of path observations and the Arc-backed compact repository request map.
Identical workspace requests deduplicate, differing workspaces return
`ConflictingRootModuleBootstrap`, and the existing repository-conflict
precedence is preserved before bootstrap conflict detection. Successful union
retains bootstrap, path, and repository needs together. Bootstrap-bearing Need
outcomes remain invalid and self-unequal through the unchanged complete-only
DICE equality policy.

Two focused tests pin request identity/path, the independent reminder literal,
length, recomputed and fixed digest, exact warning, both apply-result shapes,
all create-error fields, duplicate/conflicting/cumulative union, repository
conflict precedence, and transient outcomes. Validation passed 237 bzlmod
tests and zero doctests, 54 loading tests, 92 core unit plus 13 integration
tests, and no-run linkage of every bzlmod/loading/core GNU-Windows test
executable. Formatting, archive, diff, exact four-file allowlist, dependency,
private-surface, and forbidden-reference gates passed. The compile-only digest
inference correction and source-audit digest/accessor corrections changed no
owner boundary; all three terminal latest-diff reviews returned `ACCEPT`.

Next evidence: Implement only
`WP-5-m1-root-module-bootstrap-native-owner`.

### Stage 5 dormant native root-module bootstrap owner

Status: Accepted

The runtime-private `RootModuleBootstrapOwner` binds one normalized workspace
and rejects a foreign request with a typed private mismatch before deriving or
inspecting any path. A matching request derives the logical module path once,
uses exactly `Path::exists()`, returns `AlreadyPresent` without reading or
writing when true, and otherwise uses ordinary `std::fs::write` of the pinned
reminder. Every successful write returns `Created(warning)` without a
post-write stat; write failures retain the logical path,
`PathIoErrorKind::from(error.kind())`, and raw OS code.

This preserves Bazel's non-atomic race and symlink behavior: no `try_exists`,
metadata precheck, canonicalization, exclusive create, parent creation,
temporary file, rename, lock, retry, readback, rollback, or atomic
replacement was added. Existing symlinks with live targets remain untouched;
dangling symlinks are followed by the write, their relative targets receive
the reminder, and the links remain links. The module and owner/error APIs are
private, narrowly dead-code-allowed, and have no non-test callsite, DICE,
demand, epoch, event, output, print, or warning-publication edge.

Six isolated real-filesystem tests cover initial create and exact bytes, warm
silence/no-overwrite, edit preservation, delete/recreate, deterministic
file-as-parent typed failure, foreign no-touch of both workspaces, and Unix
existing/dangling symlinks. Validation passed 237 bzlmod tests, 54 loading
tests, 98 core unit plus 13 integration tests, zero doctests, and no-run
linkage of every bzlmod/loading/core GNU-Windows test executable. Formatting,
archive, diff, exact two-file allowlist, dependency, private-surface,
no-callsite, and forbidden-primitive gates passed. All three terminal
source/lifecycle/architecture reviews returned `ACCEPT`.

Next evidence: Implement only
`WP-5-m1-root-package-policy-input-owner`.

### Stage 5 root package-policy input owner allowlist correction

Status: Replanned before retained Rust

The frozen five-file packet could not exactly implement Bazel 9.2
`PackageIdentifier.parse`. Bazel's
`PackageIdentifier.java:145-152`, `LabelParser.java:140-165`,
`LabelValidator.java:50-61,96-138`, and
`RepositoryName.java:55,178-195` accept literal repository names matching
ASCII `[A-Za-z0-9_.+-]*` except exact `.` and `..`, and strip only a terminal
`...` package segment before applying Bazel's printable-ASCII package
validation. Slug's public `CanonicalRepoName::new` deliberately has a
different established grammar, while `PackagePath::parse` accepts a different
general path domain. The provisional five-file draft therefore could not be
Bazel-exact without changing unrelated accepted identity behavior.

The corrected packet adds exactly
`app/slug_identity_v2/src/repo.rs` to the prior five-file allowlist. That file
adds only a crate-private, package-parser-specific literal repository
constructor: empty is main, exact `.`/`..` are rejected, ASCII
alphanumeric/`_-.+` are accepted, and no mapping occurs. Existing public
canonical/apparent repository constructors, label parsers, repository
mapping, and display behavior remain unchanged. `package.rs` owns the
package-identifier-specific ASCII validation and terminal-`...`
normalization without weakening public `PackagePath::parse`.

Focused identity evidence must accept leading/trailing-dot and `+` literal
repositories through both `@` and `@@`; reject exact dot names, `~`, `@`,
slash, non-ASCII, and controls; accept Bazel package punctuation; reject
Unicode, controls, DEL, backslash, colon, slash-boundary errors, and every
remaining all-dot component; and pin terminal `...` normalization for main
and nonmain identities. The other three bzlmod files and every normalized
owner, `Arc`/`SmallSet`, opaque projection, DICE replay, missing-input, and
forbidden-scope requirement from Serial packet 3 remain unchanged. Both
terminal source and architecture reviews returned `REPLAN`; no Cargo command
ran and no bzlmod implementation began.

Next evidence: Implement only
`WP-5-m1-root-package-policy-input-owner-correction` in the corrected six-file
allowlist.

### Stage 5 normalized root package-policy input owner

Status: Accepted

The corrected six-file packet adds exact Bazel
`PackageIdentifier.parse` ownership without changing Slug's established
public repository or package-path grammars. A crate-private literal
repository constructor accepts Bazel's ASCII `[A-Za-z0-9_.+-]*` domain except
exact `.`/`..`; the package parser rejects targets, preserves literal
single-`@` and double-`@@` nonmain identity without mapping, canonicalizes all
main spellings, applies Bazel's printable-ASCII package validation, and
strips only a terminal exact `...` component.

The public normalized policy value retains one exact workspace, ordered and
duplicate-preserving `Arc<[NormalizedAbsolutePath]>` roots, a direct compact
`SmallSet<PackageIdentifier>` of structurally deduplicated repeated/comma
deleted-package occurrences, an optional contained or outside absolute vendor
directory, and exact Off/Warning/Error REPO UTF-8 semantics. Missing mode
defaults to Warning; case-insensitive enum values and Bazel's complete
boolean shorthand map to Error/Off exactly.

One private normalized-workspace injected key owns the complete Arc-retained
value. The public injection helper stages it in one `changed_to` operation.
Three public opaque workspace keys compute that owner through
`compute_opaque` and private `ProjectionKey`s, exposing only semantics,
roots/vendor, or roots/deleted packages. Their equality compares all and only
projected fields, so equal injection reuses, deleted-, semantics-, and
vendor-only changes replay only their consumer, root changes replay both
root consumers, and A→B→A restores exact A. All three fail closed with a
typed workspace-specific missing-input result; independently injected
workspaces do not alias.

Validation passed all 15 identity tests, 241 bzlmod tests, 54 loading tests,
98 core unit plus 13 integration tests, and zero doctests. Every
identity/bzlmod/loading/core GNU-Windows test executable linked. Formatting,
archive, diff, exact six-file allowlist, dependency, private-injected-key,
no-consumer, no-filesystem, no-raw-path, and compact-collection gates passed.
The only validation correction separated the missing-injection proof from a
fresh two-workspace DICE graph because DICE memoizes a requested absent
injected key; production code was unchanged. All three terminal source,
implementation/evidence, and architecture/hot-path reviews returned
`ACCEPT`.

Next evidence: Implement only
`WP-5-m1-neutral-diagnostic-event-contract`.

### Stage 5 neutral diagnostic-event compile-closure correction

Status: Replanned before validation

The frozen one-file packet was not compile-closed. Adding the real public
`EvaluationEvent::Diagnostic` variant makes seven existing exhaustive matches
non-exhaustive: one test helper each in bzlmod root-module tests, two loading
integration tests, one analysis integration test, and three `#[cfg(test)]`
core runtime modules. There are no production exhaustive consumers,
diagnostic constructors, wildcard consumers, or diagnostic producers.
Marking the enum non-exhaustive would still require the same consumer edits.

The corrected allowlist is exactly:

- `app/slug_events_v2/src/lib.rs`;
- `app/slug_bzlmod_v2/tests/root_module_dice.rs`;
- `app/slug_loading_v2/tests/bzl_invalidation.rs`;
- `app/slug_loading_v2/tests/build_file_loading.rs`;
- `app/slug_analysis_v2/tests/starlark_rule.rs`;
- `app/slug_core_v2/src/runtime/dice.rs`;
- `app/slug_core_v2/src/runtime/demands.rs`; and
- `app/slug_core_v2/src/runtime/events.rs`.

Each of the seven added files receives only the exact explicit arm
`EvaluationEvent::Diagnostic { .. } =>
unreachable!("diagnostic events are not produced by this packet")` in its
existing test helper. This restores compile closure, preserves existing
print-only projections, establishes no silent diagnostic filtering or output
policy, and fails if this neutral packet accidentally activates a producer.
No wildcard arm, expected-output change, diagnostic construction, production
logic, Cargo dependency, or other consumer edit is authorized.

The event-owner value/test contract remains unchanged: public Warning/Error
levels, `CompactString` diagnostic text, level inequality, exact UTF-8 and
newline retention, mixed structural order, ordinary event `Clone`, and
batch-only `Dupe` sharing one Arc slice. Independent source and architecture
reviews returned `REPLAN` for the one-file scope and accepted the corrected
eight-file compile closure. No Cargo or formatting command ran.

Next evidence: Implement only
`WP-5-m1-neutral-diagnostic-event-contract-correction`.

### Stage 5 neutral diagnostic-event contract

Status: Accepted

The corrected eight-file packet adds public
`EvaluationDiagnosticLevel::{Warning, Error}` and
`EvaluationEvent::Diagnostic { level, text: CompactString }` beside the
existing Starlark-print variant. Event values retain ordinary `Clone`,
structural equality, and `Allocative` without `Dupe`; only `EventBatch`
retains cheap `Dupe` over its immutable `Arc<[EvaluationEvent]>`. Tests pin
same-text level inequality, exact Unicode plus LF/CRLF/trailing-newline text,
mixed print/diagnostic order and reordered inequality, and shared batch
storage.

Each of the seven frozen downstream test helpers now has only the exact
explicit `Diagnostic { .. } =>
unreachable!("diagnostic events are not produced by this packet")` arm.
Those compile-closure edits preserve the existing print projection and make
any accidental producer fail. No wildcard, filtering policy, expected-output
change, diagnostic construction, or production logic entered those files.
Repository-wide scans found no diagnostic producer, production exhaustive
consumer, DICE edge, output/publication mapping, retry, or activation.

Validation passed 7 event tests, 241 bzlmod tests, 54 loading tests, 12
analysis tests, 98 core unit plus 13 integration tests, and zero doctests.
Every affected event/bzlmod/loading/analysis/core GNU-Windows test executable
linked. Formatting, archive, diff, exact eight-file allowlist, dependency,
trait, storage, reference, and no-activation gates passed. All three terminal
source/contract, implementation/evidence, and architecture reviews returned
`ACCEPT`.

Next evidence: Implement only
`WP-5-m1-repository-ignore-matcher-owner`.

### Stage 5 repository-ignore matcher owner

Status: Accepted

The exact two-file packet adds one crate-private dormant
`RepositoryIgnoreMatcher` with sorted/deduplicated
`Arc<[PackagePath]>` literal prefixes and ordered
`Arc<[CompiledPattern]>` REPO patterns. Each compiled pattern retains its
exact `CompactString` source spelling and compact precompiled segment/atom
slices. The outer value is `Allocative`, cheap `Dupe`, and manually equal
only over normalized prefixes plus ordered original patterns; compiled state
and bounded matching scratch are excluded.

`matching_entry(&PackagePath)` checks component-aware literal prefixes first
and then preserves first-matching pattern order. Its rolling bounded DP ports
Bazel 9.2 `UnixGlob.matchesPrefix`: exhausted patterns match descendants,
exact `**` consumes zero or more segments, root is zero segments, and
ordinary segment matching preserves the exact empty, bare-star, leading-dot,
one-star prefix/suffix, and generic wildcard branches. The generic branch
drops parentheses, keeps regex metacharacters and backslash literal, and
matches Java regex-dot behavior including its five excluded line
terminators; fast paths retain literal parentheses and `?`. Actual REPO
patterns remain unvalidated, ordered, duplicate-preserving, and return their
exact original spelling.

Seven focused tests pin literal root/exact/descendant/component boundaries,
zero/many recursive segments, hidden entries, mixed wildcards, regex
literals, optimization-sensitive parentheses and fast-path `?`, Java line
terminators, malformed empty/absolute/trailing/doubled slash and embedded
`**` patterns, precedence/order/duplicates, and semantic equality. Validation
passed the focused 7, full bzlmod 248, loading 54, core 98 unit plus 13
integration tests, and zero doctests. Every bzlmod/loading/core GNU-Windows
test executable linked. Formatting, archive, diff, exact two-file allowlist,
dependency, private-surface, compact-storage, bounded-scratch, and
no-activation gates passed. The only compile correction stored literal
Unicode scalars as `u32` because `char` lacks this repository's `Allocative`
implementation; semantics remained unchanged. All three terminal
source/contract, implementation/evidence, and architecture/hot-path reviews
returned `ACCEPT`.

##### Windows long-path observation prerequisite implementation

**Status:** Accepted on 2026-07-25.

The exact six-file implementation adds lossless raw `Arc<[u16]>` demand
identity, final normalized `Arc<[u16]>` results, direct unrefined observer
dispatch, exact 8.3/as-long-path/native-eligibility/prefix/lexical helpers,
one checked sizing/fill `GetLongPathNameW` owner, effective-result validation,
and exhaustive existing-consumer closure. The sole source-backed correction
made the 8.3 predicate preserve Java's mixed semantics: regex-dot counts
Unicode code points and excludes all five default line terminators, while
total and capture limits remain UTF-16-unit based.

Validation passed workspace 35, core 102 unit plus 13 integration, bzlmod
248, loading 54, and zero doctests. Every affected GNU-Windows test executable
linked, and the core executable imports `GetLongPathNameW` from
`KERNEL32.dll`. Focused matrices cover lossless demand/result identity,
Need/A→B→A, operation pairing, native eligibility, dot/up-level and
separator normalization, both removable prefixes, one-call shortening and
growth races, bounded allocation, non-BMP and unpaired-surrogate units,
Java-dot astral/terminator cases, direct no-Lstat dispatch, and
repository-validation dirty transitions. Formatting, diff/archive, exact
six-file scope, no-dependency, no-producer, no-DICE-IO, and no-activation
gates passed. No native Windows or provisioned NTFS 8.3 worker was available,
so real alias expansion was not claimed; the pure matrix and GNU-Windows ABI
link are the retained evidence. All three latest-diff source,
implementation/evidence, and architecture/DICE reviewers returned `ACCEPT`.

Next evidence: Implement only
`WP-5-m1-host-repository-ignore-owner-corrected-retry`.

### Stage 5 Host RegistryFunction boundary redesign

#### Corrected boundary and source order

`WP-5-m1-host-registry-function-boundary-design` supersedes the historical
two-file Host registry proposal. Pinned Bazel 9.2
`ModuleFileFunction.compute` obtains the root module before it chooses a
registry override or registry list. Its later `getModuleFile` path constructs
all chosen `RegistryKey` values before sequential registry-file fetches.
`RegistryFunction`, however, has no root edge. Therefore the private
RegistryFunction owner below is root-free. A later composition owner must
compute `HostRootModuleFileKey` first, propagate its bootstrap/path Needs and
typed error, choose the override or registry list, construct every Host
registry, and only then fetch registry files in order. A non-registry override
bypasses every registry key. No root value, provisional or final repository
mapping, selection result, or source-preparation result enters registry
construction identity.

For each original registry URL, freeze this exact sequential construction
order from `RegistryFunction.compute`:

1. acquire `RootModuleLockfileModeKey`;
2. acquire a vendor-directory-only projection from the root package-policy
   input in every mode;
3. only for Refresh, acquire a dedicated RegistryFunction invalidation token;
4. acquire `HostVisibleLockfileKey` in Off, Update, Refresh, and Error and for
   HTTP(S) and file registries;
5. derive the resolved primary spelling by replacing `%workspace%` and borrow
   the registry-hash and selected-yanked projections from the accepted full
   lockfile;
6. acquire the complete module-mirrors input and select by the original,
   unsubstituted registry URL;
7. validate the primary URI, scheme, and path; select the hash mode; validate
   the selected mirror URI spellings; and construct the value.

The Refresh token is Bazel's RegistryFunction/object-cache invalidation edge,
not the existing per-command `RegistryRequestGenerationKey` used to retry
registry IO. Off, Update, and Error have no Refresh-token dependency. A
visible-lockfile path Need returns before mirrors or either URL validation.
Missing lockfile is the accepted full empty v28 value; no mode or scheme may
skip the visible owner. Mirror lookup uses the original key URL even when
`%workspace%` changes the primary URL. Java evaluates the complete mirror
input before entering `RegistryFactoryImpl`, so a missing mirror input
precedes both mirror and primary URI errors. Once the input exists, the
factory validates the resolved primary URI first: an invalid primary wins over
an invalid selected mirror, and only a valid primary advances to ordered
mirror URI validation.

Command registry URLs and module mirrors require a new dormant raw request
representation; do not reuse `RegistryUrls::from_request`, which currently
performs `%workspace%` substitution and validation too early. Command
registry normalization trims trailing slashes, keeps first occurrence order,
and deduplicates. Root override registry spellings do not pass through that
command normalization. Mirror registry keys and mirror bases trim trailing
slashes; a later duplicate flag entry wins; `""` is the default; an explicit
per-registry entry, including an empty entry, overrides the default; and an
unknown nonempty registry key is a typed request-input error. Mirror URI
syntax validation is distinct from primary registry scheme/path validation.
Do not apply `%workspace%` or a supported-scheme/path restriction to mirror
URIs. Construction only retains selected command mirrors. Later source
construction owns command-mirror-first ordering, registry metadata mirrors,
the original source URL, and `source.json` backups; no mirror fetch or URL
transformation belongs here.

#### Private RegistryFunction value

After the oracle and input prerequisites below, add one private
`HostRegistryFunctionKey` whose identity is the normalized workspace plus the
exact original supplied registry-key spelling. Its complete-only result is:

```text
PathOutcome<
    Arc<Result<HostRegistryFunctionValue, HostRegistryFunctionError>>
>
```

Every Need is invalid and self-unequal. Complete errors and values are
semantic. `HostRegistryFunctionValue` retains the original registry spelling,
the resolved/validated primary registry base and scheme, a private
`RegistryKnownFileHashesMode`, the accepted full `Arc<BazelLockfile>`, the
vendor directory, the compact selected mirror slice, and the Refresh token
only in Refresh. It exposes no unrestricted lockfile accessor: later private
consumers may inspect only registry file hashes and selected-yanked entries.
Custom equality compares original/resolved URL, scheme/hash mode, vendor,
mirrors, the optional Refresh token, registry file hashes, and
selected-yanked state. It ignores the other four lockfile fields, clones no
lockfile map, and permits a downstream registry projection to prune their
changes. Retaining the Refresh token ensures an hourly token change
invalidates the constructed registry object and its lazy metadata cache.

The construction error variants are limited to typed missing mode,
vendor-projection, Refresh-token, visible-lockfile, and mirrors inputs plus
distinct invalid primary-registry URL and invalid mirror URI errors. A
visible path Need is not an error. Construction has no registry IO,
filesystem read, request-generation, checksum, transport, local-read, yanked
fetch, or source-preparation error. Do not reuse the legacy
`RegistryPolicyKey`, `RootModuleFilesKey`, `VisibleLockfileRead`, or
string-erased `RegistryFileError` at this boundary.

The factory table is exact:

| registry scheme | Off | Update | Refresh | Error |
| --- | --- | --- | --- | --- |
| HTTP(S) | `USE_AND_UPDATE` | `USE_AND_UPDATE` | `USE_IMMUTABLE_AND_UPDATE` | `ENFORCE` |
| file | `IGNORE` | `IGNORE` | `IGNORE` | `IGNORE` |

Construction retains but does not consume hashes, selected-yanked state,
vendor, or mirrors. Later HTTP(S) file fetch uses `USE_AND_UPDATE` to replay a
recorded absence, fetch an unrecorded URL, and verify a recorded SHA in both
Off and Update. `USE_IMMUTABLE_AND_UPDATE` differs only by refetching a
recorded absence. `ENFORCE` rejects an unrecorded URL before IO, replays an
absence, and verifies a SHA. A file registry ignores hashes in every mode.
Selected-yanked reuse is also later: HTTP(S) Refresh refetches metadata; all
other HTTP(S) modes and every file mode first reuse an exact selected-yanked
reason, otherwise treat any recorded `source.json` entry—recorded absence or
recorded SHA—as not yanked, otherwise fetch metadata. Final repository
mappings remain post-selection graph state and are forbidden from every
registry key/value/error.

#### Oracle-first, bounded owner sequence

Do not edit Rust until focused Bazel evidence is accepted. Run these reviewed
packets serially:

1. `WP-5-m1-host-registry-function-oracle-design`, design only. Prefer
   strengthening `registry-yanked-lockfile-mode` and
   `registry-command-transport` instead of copying their registry/server
   scaffolds. Freeze the smallest rows that discriminate remote Off recorded
   absence/SHA and selected-yanked reuse from legacy FetchUnrecorded behavior;
   Refresh refetch/invalidation; default/per-registry/empty-override/unknown
   module mirrors; and registry vendor hit, fatal vendored-read failure with
   no network fallback, and non-vendored/checksum-absent network paths with
   request counts. Reuse the accepted lockfile-before-invalid-URL row. If
   mirror or vendor consumption is deferred beyond the construction owner,
   the oracle design may split those fetch-time rows into their exact later
   consumer packet, but it must not claim unobserved behavior.
2. Implement and accept that exact oracle before any Host registry Rust.
   This will be packet three after fixture-growth checkpoint `df812c2c`; no
   growth review is due unless the approximately 100-file/10,000-line
   threshold is crossed.
3. `WP-5-m1-host-registry-inputs-design`, design only. Freeze small, serial
   private owners for raw registry/mirror input normalization, a vendor-only
   package-policy projection, and a dedicated Refresh invalidation token.
4. Implement the accepted input/projection owners before the pure
   `HostRegistryFunctionKey`; keep all injection test-only and dormant.
5. Implement the accepted pure RegistryFunction owner in a new private
   `host_registry.rs` plus the private module declaration and only the exact
   prerequisite owner files authorized by packet 3. It performs no IO.
6. Design and implement a one-file private IO-bridge packet in
   `registry_dice.rs`. Factor a Bazel-shaped expectation-aware remote executor
   and a typed local executor while preserving every active legacy wrapper
   and its legacy Off behavior byte-for-byte. The Host remote wrapper uses the
   exact table above. The local wrapper reuses the existing nonsemantic
   `RegistryIo` capability: Found remains sticky and drops generation; local
   absence and read failure acquire generation.
7. Design and implement a separate private Host registry-file owner. Its
   result is
   `SourcePreparationOutcome<Arc<Result<RegistryFileValue,
   HostRegistryFileError>>>`: visible path Needs and the local branch's root
   bootstrap/path Needs propagate and remain invalid/self-unequal. It consumes
   the pure descriptor then the packet-6 bridge. Its local branch retains the
   redundant direct `HostRootModuleFileKey` semantic edge required by the
   accepted local replay oracle, but production composition must already have
   completed the root before calling any construction or file key. Typed
   construction, root, local, remote, vendor-path, vendor-file, and checksum
   errors remain distinct; a Need is never an error. Do not use
   `HostFileBytesKey` or direct filesystem IO for a local registry.
8. Only after both private owners are accepted, design root-bootstrap and
   atomic command-input activation, then the ordered discovery/source-
   preparation composition and typed public error projection. That later
   composition owns root first, override/list choice, all-registry
   construction before sequential fetch, and the Host switch.

Focused retained-DICE evidence for the pure owner must prove the exact input
order; visible Need before mirror/validation; a Refresh-only invalidation
edge; all four mode/scheme cells; original versus `%workspace%`-resolved
identity; mirror default/override and error ordering; typed Complete errors;
Need invalidity; mode, vendor, mirror, URL, hash, selected-yanked, and Refresh
token A→B→A; separately allocated equal lockfiles; recomputation but
downstream pruning for changes to each unrelated lockfile field; missing
mirror input before URL errors; invalid primary before invalid selected
mirror; ordered mirror validation; and no root, IO, request-generation,
mapping, source-preparation, write, or activation edge. The later file owner
must separately prove the complete remote
mode/expectation matrix and generation-before/after-IO order, typed
capability/transport/checksum failures, local absence/failure retry, Found
stickiness across raw mutation, root-triggered reread, exact propagation of
any redundant root bootstrap/path Need, and no path-observation Need
originating from local registry bytes.

Stop and replan on Rust before oracle acceptance; root inside
`HostRegistryFunctionKey`; request-boundary URL validation; mirror lookup by
the substituted URL; missing unconditional visible-lockfile acquisition;
request generation used as Refresh invalidation; legacy Off semantics on the
Host path; copied lockfile maps or standard map/set churn; direct IO or
`HostFileBytesKey` for local registries; public input/key/transport surface;
new dependency, cache, lock, or interner; construction-time fetch; root value
retention; discovery, MVS, final mapping, source URL, lockfile write, or
loading/core/analysis/query consumption; or any production activation.

#### Host RegistryFunction boundary redesign status

**Status:** Accepted after terminal latest-text review on 2026-07-26.

The historical root-owning Host registry proposal is replaced by a pure,
root-free RegistryFunction descriptor and a later root-first composition
boundary. The frozen sequence matches pinned Bazel 9.2: mode, vendor,
Refresh-only dedicated invalidation, unconditional full visible lockfile,
resolved primary/hash/yanked derivation, original-key mirror input, primary
validation/hash mode, then ordered mirror validation. Exact HTTP(S)/file
hash modes, selected-yanked reuse including recorded absence, fatal vendored
read behavior, raw command/mirror identity, complete-only equality, compact
full-lockfile retention, and mapping exclusion are explicit.

The accepted route is oracle first, then private raw input/projection owners,
the pure construction owner, a legacy-preserving one-file RegistryIo bridge,
and the separate Host file owner. The latter propagates redundant root Needs
while preserving sticky local Found and generation-gated absence/failure.
Pinned-source/parity, native-implementability, and
architecture/orchestration terminal latest-text reviews all returned
`ACCEPT`. No Rust, fixture, Cargo, dependency, public API, IO behavior,
consumer, or activation changed.

Next evidence: Run only
`WP-5-m1-host-registry-function-oracle-design`; it is design only.

### Stage 5 Host RegistryFunction oracle design

`WP-5-m1-host-registry-function-oracle-design` reuses exactly two accepted
fixtures and changes only these four paths:

- `tests/v2_oracle/fixtures/registry-yanked-lockfile-mode/fixture.toml`;
- `tests/v2_oracle/fixtures/registry-yanked-lockfile-mode/expected/oracle.json`;
- `tests/v2_oracle/fixtures/registry-command-transport/fixture.toml`; and
- `tests/v2_oracle/fixtures/registry-command-transport/expected/oracle.json`.

Add no fixture, server, workspace file, archive, generator, harness behavior,
or mutable external asset. Keep both existing HTTP servers and every existing
workspace byte unchanged. The final combined inventory remains exactly 29
regular files and zero symlinks. The current two-fixture inventory is 1,152
newline-counted lines; cap the final combined inventory at 1,800 lines, for
net growth of at most 648 lines. Update each touched fixture's existing
description, `oracle_notes`, and `translation_notes` in place to name the new
Off or `show_repo` evidence; retain every still-accurate existing claim.

#### Remote Off lockfile consumption

Insert exactly three rows into `registry-yanked-lockfile-mode` after its
Update priming/replay rows and before its accepted Refresh row. Preserve the
two existing registry arguments and BCR embedded-closure fallback.

1. `off_replays_recorded_absence` changes the root module version from
   `0.1.0` to `0.1.1`, uses `--lockfile_mode=off`, and allows
   `yyy@1.0.0`. The root semantic mutation must force nonroot discovery;
   without it, a zero-request result could be stale same-daemon
   `ModuleFileValue` reuse rather than Off lockfile evidence. Expect exit 0,
   `olddep@1.0.0`, and no `newdep@1.0.0`. Relative to the preceding
   cumulative request manifest,
   `/first/modules/aaa/1.0.0/MODULE.bazel` and
   `/first/modules/yyy/metadata.json` do not increase, while
   `/second/modules/aaa/1.0.0/MODULE.bazel` increases by one. Generate and
   retain the complete cumulative manifest; only these discriminating deltas
   are contractual because sibling module reads may be batched.
2. `off_reuses_selected_yanked_reason_a` keeps Off but removes the allow
   flag. Expect exit 37 and the exact accepted yanked diagnostic for
   `yyy@1.0.0` with `reason-a`; exclude `reason-b`. No HTTP counter increases,
   especially `/first/modules/yyy/metadata.json`.
3. `off_enforces_recorded_sha_before_yanked` restores the allow flag and
   changes the generated lockfile's unique yyy MODULE digest
   `9114f034663d930400ebc5993990181e4a83dc5f4d5e0c80f8b7b570ebe86969`
   to 64 zeroes. Expect exit 37 with the first-registry yyy MODULE URL and
   exact checksum order: actual `9114...6969`, wanted 64 zeroes. Exclude every
   yanked diagnostic. Only
   `/first/modules/yyy/1.0.0/MODULE.bazel` increases by one and yyy metadata
   does not increase.

Before the existing Refresh row executes, add mutations that restore both the
root version `0.1.1` to `0.1.0` and the zero digest to
`9114...6969`. Preserve its existing Refresh assertions and the final Error
checksum-before-yanked row. The complete nine-row cumulative request and
lockfile manifests must replay exactly. These rows distinguish Bazel's
HTTP(S) Off=`USE_AND_UPDATE` behavior from the live legacy Slug
FetchUnrecorded shortcut: Off consumes recorded absence, selected-yanked
state, and recorded SHA from the visible lockfile.

#### Command mirror value and RepoSpec projection

Insert exactly four rows into `registry-command-transport` after
`workspace_file_registry` and before its existing invalid-primary-URL rows.
Each row runs this base command; the first three add their successful mirror
flags and the fourth adds the unknown-registry entry:

```text
mod show_repo @@yyy+
--lockfile_mode=off
--registry={{http_registry}}/b
--registry=https://bcr.bazel.build
```

The existing b/yyy `source.json` supplies the immutable original URL
`https://example.invalid/yyy-1.0.0.tar.gz`; no archive is fetched.

1. `show_repo_default_module_mirrors` supplies exactly
   `--module_mirrors=https://default-one.example/mirror,https://default-two.example/mirror`.
   Expect exit 0 and the exact `urls` projection, in order:
   `https://default-one.example/mirror/example.invalid/yyy-1.0.0.tar.gz`,
   `https://default-two.example/mirror/example.invalid/yyy-1.0.0.tar.gz`,
   then the original URL.
2. `show_repo_per_registry_last_wins` supplies one default, then an older
   explicit b-registry entry, then a later explicit b-registry entry
   containing ordered `specific-one` and `specific-two` mirror bases. Expect
   exit 0 and exactly those two transformed specific URLs followed by the
   original URL. Exclude the default and older explicit mirror. This single
   row proves per-registry override, later-duplicate wins, and retained
   selected-list order. The three flags are exactly
   `--module_mirrors=https://default.example/mirror`,
   `--module_mirrors={{http_registry}}/b=https://stale.example/mirror`, and
   `--module_mirrors={{http_registry}}/b=https://specific-one.example/mirror,https://specific-two.example/mirror`.
3. `show_repo_explicit_empty_registry_override` supplies a nonempty default
   through `--module_mirrors=https://default.example/mirror` and then an
   explicit `--module_mirrors={{http_registry}}/b=` entry. Expect exit 0 and
   a `urls` projection containing only the original source URL; exclude the
   default mirror.
4. `module_mirrors_unknown_registry` supplies a mirror entry for
   `https://unknown.example`, which is absent from `--registry`, through
   `--module_mirrors=https://unknown.example=https://mirror.example`. Expect
   exact exit 2 and
   `--module_mirrors references registries not listed in --registries:
   https://unknown.example`. The cumulative HTTP manifest is unchanged. This
   is command-input rejection evidence, not RegistryFunction execution.

Assert the complete ordered `urls` list, not unordered substring presence.
Retain exact normalized request-count manifests. These rows prove command
mirror selection and the later RepoSpec URL projection without claiming an
archive download, mirror request, fallback, RegistryFunction internal edge
order, or vendor behavior.

Add `RegistryFunction.java` to the yanked fixture's retained source anchors.
Add exactly these mirror-specific anchors to the transport fixture while
retaining its current anchors:

- `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/IndexRegistry.java`;
- `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/modcommand/RepoOutputFormatter.java`;
- `src/main/protobuf/failure_details.proto`.

They source the command-mirror-first transformed URL list, the exact
`show_repo` projection, and
`FailureDetails.ExternalRepository.UNKNOWN_REGISTRY` exit 2 respectively.

#### Deferred evidence and validation

The hard-coded one-hour `RegistryFunction.LAST_INVALIDATION` turnover has no
bounded command-oracle hook. Keep it pinned-source evidence and require
focused retained-DICE token A→B→A evidence in the later private invalidation
owner; the accepted Update→Refresh row proves Refresh refetch policy, not
hourly token turnover.

Vendor is consumed only during later registry-file reads, not
RegistryFunction construction. Before any Host registry-file or IO-bridge
Rust, design and accept
`WP-5-m1-host-registry-file-vendor-oracle` for checksum-present vendored hit
with no network request, fatal vendored-read failure with no network
fallback, non-vendored or missing/wrong-kind vendor path network fallback,
and checksum-absent network behavior. Before source URL/download activation,
separately design and accept
`WP-5-m1-host-module-mirror-fetch-oracle` for actual command-mirror,
registry-mirror, original, and `source.json` backup request/fallback order.
`show_repo` does not satisfy that gate.

Run one pinned Bazel 9.2 generation for each fixture and two distinct
fresh-root replays of each final generated fixture. Validate exact row names,
order, arguments, exits, positive and negative output/message assertions,
root/digest mutations and restoration, ordered `urls`, complete cumulative
request/lockfile manifests, pinned source anchors, schema, normalization,
four-path scope, unchanged server/workspace bytes, final 29-file/zero-link
inventory, final line count and delta, archive status, diff checks, and
credential scans.

At fixture-growth checkpoint `df812c2c` / accepted baseline tree `c039c347`,
the repository held 1,284 regular files, 14 links, and 33,789 lines. Before
this packet it holds 1,297 regular files, 14 links, and 35,600 lines:
+13 files and +1,811 lines across accepted oracle packets `eb8c2d23` and
`d20f6557`. This is oracle packet three after that checkpoint and at the cap
remains no more than +13 files/+2,459 lines, so neither five packets nor the
approximately 100-file/10,000-line threshold fires. Record the measured final
delta after generation. If the later module-mirror-fetch oracle would be the
fifth accepted oracle, run the required focused fixture-growth review before
any sixth oracle packet.

Stop on a fifth changed path, server/workspace/harness edit, new file or
symlink, more than seven new rows, more than 1,800 combined lines, a missing
root recomputation discriminator or digest/root restoration, a different
exit/message/order/request delta, an archive request/download to any command
mirror, registry metadata mirror, original source URL, or `source.json`
backup in a `show_repo` row, sleep/clock manipulation, external mutable state,
vendor or hourly-token claim, Rust/Cargo/dependency/API edit, or any
implementation, consumer, or activation change.

#### Host RegistryFunction oracle design status

**Status:** Accepted after terminal latest-text review on 2026-07-26.

The exact four-path packet adds three discriminating Off rows to the existing
remote lockfile/yanked fixture and four `show_repo` mirror rows to the existing
registry transport fixture. A root-version mutation prevents warm-cache false
evidence; exact absence, selected-yanked, SHA, request-delta, restoration,
mirror URL order, per-registry/later-wins/empty, and exit-2 unknown-registry
assertions are frozen. Both fixture metadata blocks and exact pinned source
anchors remain inside the same four files.

The final inventory stays 29 regular files and zero links with a combined
1,800-line cap, net at most +648. This is oracle packet three after
`df812c2c`; no growth checkpoint is due. Vendor file reads, actual mirror
downloads/fallback, and hourly Refresh turnover retain explicit later oracle
or focused-DICE gates. Pinned-source/parity, native-implementability, and
architecture/orchestration terminal latest-text reviews all returned
`ACCEPT`. No fixture, expected output, Rust, Cargo, dependency, API, consumer,
or activation changed during design.

Next evidence: Implement only
`WP-5-m1-host-registry-function-oracle` in the exact four paths above.

#### Host RegistryFunction oracle first implementation replan

**Status:** Stopped cleanly and replanned on 2026-07-26.

The first pinned `registry-yanked-lockfile-mode` generation reached all three
new Off rows and exposed one mandatory contract failure. After the preceding
Update row, cumulative first-aaa, second-aaa, yyy-MODULE, and yyy-metadata
counts were `1,1,1,1`. The root-version-mutating Off absence row remained
`1,1,1,1`; the Off selected-yanked row also remained `1,1,1,1`; and the Off
SHA row became `1,1,2,1`. Thus selected-yanked reuse and SHA enforcement were
feasible, but changing only the root module version did not dirty the cached
nonroot `ModuleFileValue`. The frozen second-aaa `+1` discriminator failed and
the recorded-absence result remained explainable by warm reuse.

The writer stopped before transport generation or any fresh-root replay,
removed all three partial changed paths through `apply_patch`, and restored a
clean `8875ce19` worktree. No expected output or fixture change is accepted,
this stopped attempt does not increment oracle-packet growth accounting, and
no server, workspace, harness, Rust, Cargo, dependency, API, consumer, or
activation changed.

Correct only the three Off rows in
`WP-5-m1-host-registry-function-oracle-unused-registry-correction`. Remove the
root `0.1.0`→`0.1.1` mutation, its restoration, and the second-aaa `+1`
requirement. Append exactly
`--registry={{http_registry}}/unused-off` after the BCR registry in all three
Off rows; keep that same suffix stable across the later yanked and SHA rows.
Pinned `ModuleFileFunction` directly depends on the injected ordered
`REGISTRIES` value, so the transition into the first Off row dirties nonroot
module-file computation. Registry construction performs no IO, and aaa
resolves through first then second before BCR or the unused suffix.

The corrected absence proof is the inseparable combination:

- the prior mutation has made first-registry aaa physically present and would
  select `newdep@1.0.0` if fetched;
- the changed registry-list dependency forces a fresh nonroot module-file
  computation;
- the graph contains `olddep@1.0.0` and excludes `newdep@1.0.0`; and
- `/first/modules/aaa/1.0.0/MODULE.bazel` remains at cumulative count 1,
  delta zero.

Do not require a second-aaa request: its recorded SHA may be served from
Bazel's checksum repository cache. The later selected-yanked row keeps the
same registry list, reports `reason-a` rather than served `reason-b`, and
increments no HTTP counter, especially yyy metadata. The SHA row keeps the
same list and the accepted digest mutation, exact diagnostic, yyy-MODULE
`+1`, metadata `+0`, and no-yanked assertions. Restore only the digest before
Refresh. Every mirror row, provenance/metadata update, exact four-path and
seven-row scope, final inventory/line caps, generation/two-replay matrix,
claim deferral, validation, and stop gate from the accepted design remains
unchanged.

Pinned correction anchors are
`ModuleFileFunction.java:748-799`,
`BazelRepositoryModule.java:612-616,756`, and
`IndexRegistry.java:169-197`. Stop again on a first-aaa request, newdep graph,
different yanked/SHA behavior, any request to the unused registry before aaa
resolves, a changed second-aaa requirement, distinct unused suffixes that
redirty the later rows, or any expansion of the accepted packet.

#### Host RegistryFunction oracle clean-cache correction

**Status:** Replanned after native invalidation probe on 2026-07-26.

An isolated Bazel 9.2 probe applied the exact stable `/unused-off` correction.
The Off graph retained olddep and excluded newdep, yanked reuse retained
reason-a, and SHA enforcement remained exact, but cumulative
first-aaa/second-aaa/yyy-MODULE/yyy-metadata counts stayed
`1,1,1,1` through the first two Off rows and became `1,1,2,1` only for the
bad SHA. The registry-list transition did not force an observable fresh
registry-file decision; unchanged counts left warm reuse or cache pruning
possible. Therefore registry-list invalidation plus graph output and
first-aaa delta zero still does not prove a fresh recorded-absence decision.
The `/unused-off` correction is rejected; none of its probe files or processes
remain.

A second isolated probe found the bounded positive discriminator. After the
Update priming/mutation rows, `bazel clean --expunge` followed by Off with the
original registry list and exact flags
`--repository_cache=../off-repository-cache` and
`--repo_contents_cache=` produced olddep and no newdep while changing
first-aaa/second-aaa/yyy-MODULE/yyy-metadata from `1,1,1,1` to
`1,2,2,1`. Skyframe/output-base state was removed, and the Off rows were
isolated from the surviving shared checksum cache by a new sibling cache; the
generated visible lockfile and exact first/second URL keys remained in the
workspace. Shutdown alone, a fresh output base with the default shared
repository cache, fresh cache flags on the warm daemon, and the unused
registry transition did not produce this discriminator.

Correct only
`WP-5-m1-host-registry-function-oracle-clean-cache-correction` as follows:

1. Remove the accepted existing
   `update_replays_selected_yanked_reason_a` command and replace it in the
   same sequence position with `off_probe_expunge`, running exactly
   `clean --expunge`, expecting exit 0, and preserving the visible lockfile,
   every workspace mutation, and the unchanged cumulative request manifest.
   This keeps the yanked fixture at nine commands and the complete two-fixture
   packet at net seven added commands.
2. Remove every root-version and unused-registry mutation, argument,
   restoration, assertion, and stop gate. Each of the three Off rows uses the
   original first, second, and BCR registry list plus the same two fresh-cache
   flags above.
3. The first Off row is now a cold computation. Expect exit 0, olddep, no
   newdep, first-aaa delta zero, second-aaa `+1`, yyy-MODULE `+1`, and
   yyy-metadata delta zero. The physical first-aaa file already exists.
   With Skyframe and checksum caches cold, fetching legacy-Unrecorded first
   aaa would request it and select newdep; correct Off instead consumes its
   recorded absence and proceeds to the recorded-SHA second registry.
4. The next Off row removes the allow-yanked flag while keeping identical
   registry/cache arguments. Expect exit 37, reason-a rather than reason-b,
   and no HTTP counter increase, especially yyy metadata. It replaces the
   removed Update reason-a row while the later accepted post-Refresh Update
   reason-b row continues to prove Update reuse.
5. The SHA Off row keeps the same arguments and accepted unique digest→zero
   mutation. Expect the exact actual/wanted checksum error before yanked,
   yyy-MODULE `+1`, metadata `+0`, and no other contract change. Restore only
   that digest before the existing Refresh row.

Add exact yanked-fixture provenance for the fresh-cache boundary from
`src/main/java/com/google/devtools/build/lib/runtime/commands/CleanCommand.java`
and
`src/main/java/com/google/devtools/build/lib/bazel/repository/RepositoryOptions.java`
plus
`src/main/java/com/google/devtools/build/lib/bazel/BazelRepositoryModule.java`,
while retaining the previously frozen anchors and metadata updates. Record
`CleanCommand.java:235-264,305-307` and
`BazelRepositoryModule.java:316-330,704-721` in the translation detail rather
than in the literal anchor entry. `CleanCommand` owns output-base deletion and
shutdown;
`RepositoryOptions` documents the flags; and `BazelRepositoryModule` resolves
the relative cache path from the workspace and maps empty
`--repo_contents_cache=` to no contents cache. State explicitly that this
recorded-absence proof crosses an intentional cold
server/output-base boundary; it is not same-daemon invalidation evidence.
The cumulative request log and workspace lockfile survive outside the
expunged output base, as the native probe observed. The
`../off-repository-cache` directory is run-local, initially absent in every
generation or fresh replay root, untracked, outside all manifest roots, and
not an external or shared asset.

Every mirror row, exact four-path scope, final 29-file/zero-link and
1,800-line caps, packet-three accounting, one-generation/two-fresh-replay
matrix, actual mirror/vendor/hourly-token deferrals, and non-root validation
gate remains unchanged. Replace the stale root/list discriminator validation
with exact expunge position, unchanged clean-row counts, identical
fresh-cache flags across all three Off rows, absence-row `1,2,2,1`
cumulative counts, later zero deltas, digest restoration, and no retained
`off-repository-cache` manifest or tracked path.

Stop on a tenth yanked command, more than seven net new commands, retained
root/unused-registry logic, clean without expunge, a shared/default or
workspace-contained repository cache, a nonempty repo-contents cache, lost
workspace lockfile or request log, first-aaa `+1`, missing second-aaa or
yyy-MODULE `+1`, metadata `+1`, newdep, changed exact yanked/SHA behavior,
cache artifacts in the tracked/manifest inventory, a different exit, or any
scope/claim/implementation expansion.

##### Host RegistryFunction oracle clean-cache correction status

**Status:** Accepted after terminal latest-text review on 2026-07-26.

The first implementation and its `/unused-off` correction were both removed
after native probes proved that root or registry-list invalidation could
leave all relevant HTTP counts warm. The accepted correction replaces the
redundant pre-Refresh Update reason-a row with `clean --expunge`, then runs all
three Off rows against the original registries with a fresh run-local sibling
repository cache and disabled contents cache. Native Bazel 9.2 observation
produced the discriminating `1,1,1,1`→`1,2,2,1` absence transition,
reason-a with zero later requests, and exact SHA yyy-MODULE `+1`/metadata
`+0`.

Pinned source confirms synchronous output-base deletion/shutdown, surviving
shared-cache isolation, workspace-relative cache wiring, and empty
repo-contents-cache disabling. Pinned-source/parity,
native-implementability, and architecture/orchestration terminal latest-text
reviews all returned `ACCEPT`. The stopped drafts remain unaccepted and do
not advance fixture-growth accounting. The worktree contains only this plan
correction; no fixture, expected output, server, workspace, harness, Rust,
Cargo, dependency, API, consumer, or activation is retained.

Next evidence: Implement only the corrected
`WP-5-m1-host-registry-function-oracle` in the original exact four-path scope.

##### Host RegistryFunction oracle BCR-mirror correction

**Status:** Corrected after stopped transport generation and native probe on
2026-07-26.

The corrected yanked generation passed its full nine-row cold-cache sequence,
but the first transport generation stopped before replay. Although all three
successful `show_repo` rows produced the intended yyy URL lists, their default
command mirrors also applied to BCR's embedded modules and emitted archive
download/unknown-host warnings. This violated the unclaimed-download stop
gate. The formatter assertions also placed a comma inside the list before
`]`; pinned `RepoOutputFormatter` instead emits `urls = [...],`, with the
attribute comma after the closing bracket.

The writer stopped before any replay, retained no partial expected output,
and restored the exact four paths to clean `2976c63d` through `apply_patch`.
The stopped generation does not advance fixture-growth accounting. No
tracked fixture, expected-output, server/workspace/harness, Rust, Cargo,
dependency, API, consumer, or activation change remains; ignored stopped-run
artifacts and Bazel server processes are cleanup state, not accepted evidence.

Correct only the three successful `show_repo` rows. After each row's existing
mirror flags, append exactly
`--module_mirrors=https://bcr.bazel.build=`. Bazel's last/per-registry
selection gives BCR an explicit empty command-mirror list while preserving:

- row 1's ordered default list for the fixture b registry;
- row 2's later explicit b-registry list over its default and stale list; and
- row 3's explicit empty b-registry list over its default.

Keep the unknown-registry row unchanged because it terminates in command-input
processing. Correct each exact formatter assertion to the one-line
`urls = ["...", "..."],` form: no comma appears between the final URL and
`]`, and the attribute comma follows `]`.

An isolated pinned Bazel 9.2 generation of the BCR-empty flag correction,
using inline-list assertions through the closing bracket, passed.
Default produced default-one, default-two, then the original yyy URL;
per-registry/later-wins produced specific-one, specific-two, then original;
and explicit b-empty produced only original. All exited 0. Normalized stderr
contained only invocation/no-action lines: no `WARNING: Download from`,
`Unknown host`, or configured command-mirror archive domain. The 42-entry
fixture HTTP log contained only registry `bazel_registry.json`, MODULE,
metadata, and `source.json` paths. That logger intentionally filters to
selected registry paths, so its exact manifest is not archive-attempt
evidence. The isolated probe matched each inline URL list through its closing
bracket; captured stdout additionally showed the formatter's attribute comma
immediately after it. The final implementation assertion must end in
`\],`.

Replace the accepted design's overbroad archive stop/claim with this exact
subject boundary. The rows prove yyy RepoSpec URL projection with no observed
failed download attempt to the exact yyy original or configured
command-mirror domains. Ordinary embedded BCR registry/original/cache activity
with its command mirrors explicitly empty is scaffolding, not subject
mirror-download evidence and is not excluded or claimed. Actual
command-mirror, registry-mirror, original, and backup request fallback remains
gated on the later mirror-fetch oracle.

Rerun both pinned generations and two distinct fresh-root replays of each
fixture from the clean checkout. Add stderr-scoped negative assertions for
download/unknown-host diagnostics, every configured command-mirror domain,
and the exact original
`https://example.invalid/yyy-1.0.0.tar.gz`; do not exclude those domains from
stdout. Enforce the exact inline URL list, BCR empty flag/order, exact
registry-only fixture request log, and every prior scope, count, digest,
provenance, inventory, growth, archive, diff, and credential gate.

This is the single focused implementation correction allowed for the
clean-cache packet. Stop and terminally replan on another generation mismatch,
any observed failed-download/unknown-host warning or configured
command-mirror/yyy-original domain in stderr, BCR mirror in yyy stdout,
changed default/specific/empty/unknown behavior, an eighth net row, a fifth
tracked path, a new file/link, line-cap breach, or any broader claim or
implementation edit.

###### Host RegistryFunction oracle BCR-mirror correction status

**Status:** Accepted after terminal latest-text review on 2026-07-26.

The stopped transport generation proved the intended URL projections but
exposed default command mirrors leaking onto embedded BCR module RepoSpecs.
The accepted focused correction appends an explicit empty BCR command-mirror
entry to the three successful rows, corrects the formatter assertion to end
in `\],`, and keeps the unknown-registry row unchanged. An isolated pinned
probe produced the exact three URL lists, no observed failed download/host
diagnostic for the configured mirrors or yyy original, and an exact
registry-only request manifest.

The HTTP logger is explicitly not archive-attempt evidence, ordinary embedded
BCR registry/original/cache behavior remains scaffold-only, and actual mirror
fallback stays deferred. Pinned-source/parity, native-observation, and
architecture/orchestration terminal latest-text reviews all returned
`ACCEPT`. The tracked worktree contains only this plan correction; stopped
ignored run state is not evidence and no fixture change is accepted.

Next evidence: Rerun only the complete corrected
`WP-5-m1-host-registry-function-oracle` implementation from the clean
four-path baseline.

###### Host RegistryFunction oracle implementation status

**Status:** Accepted after terminal latest-diff review on 2026-07-26.

The corrected exact four-path implementation passed one pinned Bazel 9.2
generation and two distinct fresh-root replays for both fixtures. The yanked
fixture now has nine commands and proves cold-cache Off recorded absence
`1,1,1,1`→`1,2,2,1`, selected-yanked reuse, checksum precedence, digest
restoration, and Refresh refetch. The transport fixture now has twelve
commands and proves exact default, per-registry/later-wins, empty, and
exit-2 unknown-registry mirror projections with the explicit BCR empty
override and comma-inclusive formatter boundary.

The fixtures contain 29 regular files, zero links, and 1,659 lines, a
507-line increase within the accepted 648-line growth and 1,800-line final
caps. The exact four-path diff, schema/list, ten packet-validator checks,
source anchors, archive status, credential scan, and `git diff --check`
passed. All six generated/replayed fixture runs passed, all Bazel servers
were shut down, and source/parity, implementation/evidence, plus
architecture/orchestration terminal reviews returned `ACCEPT`. Host `pytest`
is unavailable, so the Python test module was not imported; its executable
schema and packet validators passed directly. No Rust, Cargo, dependency,
API, consumer, or activation changed, and packet three does not trigger the
fixture-growth checkpoint.

Next packet: run only `WP-5-m1-host-registry-inputs-design`; it is
design-only. Freeze small serial private owners for raw registry/mirror input
normalization, a vendor-only package-policy projection, and a dedicated
Refresh invalidation token before any Rust.

### Stage 5 Host registry-input owners design

`WP-5-m1-host-registry-inputs-design` freezes only three dormant private
prerequisites for the later pure `HostRegistryFunctionKey`. It adds no Rust
in this packet and does not revise the accepted RegistryFunction construction
order.

#### Pinned Bazel input semantics

At Bazel 9.2 commit `8220c619…`,
`RepositoryOptions.java:99-132,344-370` defines repeatable registry and
module-mirror inputs, lockfile mode, and vendor directory.
`Converters.java:525-574,577-597` parses each module-mirror occurrence at the
first `=`, represents no key as `""`, turns the sole empty value into an empty
list, and rejects an empty member in a multi-value list. That converter
remains later CLI/parser work: this packet consumes its structured
`(registry key, ordered mirror values)` result and does not parse raw flag
text.

`BazelRepositoryModule.java:130-133,612-646,677-684` supplies the exact
slash-retaining implicit default `https://bcr.bazel.build/`; explicitly
supplied registries, mirror keys, and mirror bases instead lose every trailing
`/`. Explicit registries and mirror values deduplicate while preserving first
iteration order, a later normalized mirror-key occurrence wins, unknown
nonempty mirror keys fail, `""` is the default mirror key, and an explicit
per-registry entry including an empty entry overrides that default. If there
is at least one mirror occurrence, Bazel materializes one selected mirror-set
entry for every command registry; no mirror occurrence instead injects the
distinct empty map.

The injected registry list and mirror map are immutable set/map values.
`PrecomputedValue.java:45-76,110-133` and
`AbstractInMemoryMemoizingEvaluator.java:141-151,432-448` therefore give them
order-insensitive equality and prune an equal replacement even though their
iteration order is retained. Slug must reproduce that surprising injected
edge. A freshly normalized registry or mirror value iterates in its current
occurrence order. A reorder-only value compares equal at injection, so the
retained graph does not replace its previously stored value: retained
consumers keep the old iteration order and do not recompute. This packet makes
no claim about a broader command-option invalidation edge and does not
activate the values.

`ModuleFileFunction.java:748-765`, `RegistryKey.java:25-47`, and
`RegistryFunction.java:77-85` show that a registry key contains the
post-option-normalization or override spelling, mirror lookup uses that exact
unsubstituted spelling, and `%workspace%` replacement occurs only afterward.
The implicit default consequently keeps its trailing slash, an explicit BCR
registry loses it, and a mirror key normalized to
`https://bcr.bazel.build` is unknown when registries were omitted. A
nonempty root override bypasses command-registry normalization entirely.

`RegistryFunction.java:37-51,65-72`,
`BazelRepositoryModule.java:152,660-669,738-768`, and
`RepositoryDirectoryValue.java:42-48` keep registry list, mirrors, vendor
directory, and the registry-cache invalidation instant as separate inputs.
The invalidation instant is read only in Refresh. Bazel recovers it from the
retained evaluator, starts from the epoch when absent, and advances it only
when `now` is strictly later than the previous instant plus one hour.
`IndexRegistry.java:166-242` and `VendorManager.java:146-190` keep vendor
consumption later: it is eligible only when a vendor directory is configured,
the registry file has a checksum, the registry scheme is not `file`, and the
mapped vendor path is a file. A missing or wrong-kind path falls through to
the downloader, but once a file is selected, a read or checksum failure is
fatal and never falls back to network.

#### Exact three-file implementation boundary

The later implementation may edit exactly:

1. a new private
   `app/slug_bzlmod_v2/src/host_registry_inputs.rs`;
2. `app/slug_bzlmod_v2/src/package_policy.rs`; and
3. `app/slug_bzlmod_v2/src/lib.rs` for only the private module declaration.

The final diff adds at most 900 lines. It adds no public re-export, Cargo or
dependency change, fixture, command/server/core/loading/analysis/query edit,
production injector, consumer, activation, clock, cache, interner, lock,
filesystem or registry IO, path observation, lockfile access, repository
mapping, source preparation, or RegistryFunction implementation. Use
Buck2-derived `SmallSet`/`SmallMap`, compact or shared strings and slices,
`Dupe`, and `Allocative`; do not introduce standard maps/sets or repeated
owned-string/vector churn.

Implement the owners serially inside that allowlist: registry/mirror
normalization plus the private module declaration first, the vendor
projection second, and the Refresh token third. Test-only direct DICE
injection is allowed; production injection is deferred to the later atomic
command-input activation packet.

#### Registry and mirror inputs

The new module owns a pure normalizer whose registry argument is the ordered
primitive command-registry occurrences and whose mirror argument is the
ordered structured post-converter occurrences. It returns separate private
semantic values for:

- the normalized command-registry ordered set; and
- the complete normalized command-registry-to-selected-mirror-set map.

An empty registry occurrence list produces exactly the slash-retaining
implicit BCR default and does not pass through explicit normalization. A
nonempty list trims every trailing slash, deduplicates normalized spellings,
and retains first iteration order. It preserves `%workspace%`, empty,
malformed, unsupported-scheme, and otherwise invalid URI spellings for the
later RegistryFunction validator. It performs no workspace substitution or
URI/scheme/path validation.

For mirror occurrences, normalize keys and values by trimming all trailing
slashes. Deduplicate each replacement value-set with first iteration order;
a later occurrence of the same normalized key replaces the earlier set,
including with the empty set. After all replacements, report one typed
normalization error containing every unknown nonempty normalized key in its
first-key iteration order. If no occurrence exists, the complete map is
empty. Otherwise, materialize each normalized command registry in registry
iteration order, selecting its explicit set when present and the `""`
default otherwise. Preserve the distinction between an absent map and a map
whose selected sets are empty. Do not retain `""` as a materialized registry
entry unless `""` itself is a command registry.

Expose exact-spelling lookup on the mirror value for the future private
RegistryFunction owner. It does not trim, substitute, or validate its lookup
argument. Thus the later key selects by its post-option-normalization or raw
override spelling, and an override spelling that is not an exact materialized
command-registry key receives no command mirrors.

Use three separate workspace-scoped private injected keys:

- `HostRegistryUrlsInputKey`, whose value is the normalized ordered registry
  set;
- `HostModuleMirrorsInputKey`, whose value is the complete mirror map; and
- `HostRegistryRefreshTokenKey`, whose value is the token below.

Their identity contains only the normalized workspace. Registry-list,
mirror-map, and token replacement must invalidate independently; the mirror
key must not compute from or depend on the registry-list key. Registry and
mirror values retain iteration order but implement literal Bazel
set/map-of-set equality: top-level and nested reorder-only replacements are
equal and prune without replacing the previously stored value; membership,
selected value, explicit-empty versus absent map, or normalized spelling
changes are unequal and install the new value with its current normalized
order. Do not use order-sensitive slice-derived equality for these injected
values.

The pure normalizer is the only atomic bundle at this stage: an unknown-key
error returns neither value. Test helpers may inject its two successful
values on the same updater, but separate typed injected keys remain visible
to future consumers. Missing injected values fail closed at DICE and will be
mapped to distinct typed input errors by the later RegistryFunction. No input
owner returns `PathOutcome::Need`.

#### Vendor-only projection

In `package_policy.rs`, add one private
`RootVendorDirectoryProjection` over the existing
`RootPackagePolicyInputsKey` and its private workspace-keyed
`RootVendorDirectoryProjectionKey`. Its complete value is exactly
`Result<Option<NormalizedAbsolutePath>,
RootPackagePolicyProjectionError>`. Projection equality compares only the
optional normalized vendor directory, so package-root, deleted-package, and
REPO UTF-8-policy changes recompute the projection but prune every equal
downstream consumer.

This projection adds no input authority and no path Need. It does not test
existence or kind, resolve symlinks, form `_registries` paths, read vendor
bytes, inspect URLs/checksums, or fall back to network. Those are later
registry-file responsibilities under the exact pinned boundary above:
missing/wrong-kind may fall through, while a selected vendor file's read or
checksum failure is fatal without downloader fallback. A missing aggregate
package-policy input is the existing typed projection error, not a Need.

#### Dedicated Refresh token

`HostRegistryRefreshToken` is a private opaque `u64` newtype with exact
equality and no ordering, clock, duration, request-generation, or IO meaning.
Its separate injected key is present independently of mode but the future
RegistryFunction may compute it only in Refresh. It neither reuses nor
depends on `RegistryRequestGenerationKey`.

The later production activation packet alone owns retained wall-clock
threshold state and atomic request injection. It must inject the token every
command, preserve it while `now <= previous + 1h`, advance it only for strict
`now > previous + 1h`, and advance the initial epoch state on the first real
server request. The opaque counter changes only when that threshold state
advances. No clock read, sleep, interval calculation, mode dependency, or
production injection belongs in this prerequisite implementation.

#### Focused retained-DICE evidence

The three-file implementation must prove:

- the exact implicit-default slash and the adversarial unknown mirror key
  `https://bcr.bazel.build` when registries are omitted;
- explicit registry all-slash trimming, normalized deduplication, current
  first iteration order, and preservation of `%workspace%`, empty, malformed,
  and unsupported spellings;
- mirror key/value trimming, normalized deduplication and current order,
  later-wins replacement, default fallback, per-registry and explicit-empty
  override, multiple ordered unknowns, and absent-map versus materialized
  empty-map shape;
- separately allocated equal values, normalized-equivalent inputs, and
  fresh top-level registry plus nested mirror reorder values that iterate in
  their own current orders but compare equal, followed by equal reinjection
  that preserves the old stored order and does not recompute the consumer;
- workspace isolation, fail-closed missing input, registry-only versus
  mirror-only replacement, and equal/A→B→A downstream counter behavior for
  both injected collection keys;
- vendor `None` and A→B→A, separately allocated equal paths, and downstream
  pruning for independent package-root, deleted-package, and UTF-8-policy
  changes, plus the existing typed missing-input error;
- Refresh-token equal and A→B→A behavior, workspace isolation, fail-closed
  absence, and no invalidation when only
  `RegistryRequestGenerationKey` changes; and
- activation/dependency evidence that these test consumers acquire only their
  named input or projection and no root evaluation, visible lockfile,
  filesystem/path observation, IO, repository mapping, source preparation,
  write, clock, or activation edge.

Run formatting, the focused new owner tests, the complete
`slug_bzlmod_v2` unit/integration/doctest surface, and GNU-Windows test-target
compilation serially. Enforce the exact three-file/900-line scope,
`git diff --check`, archive status, and a credential scan. Stop and replan on
raw flag-text parsing; normalization of the implicit default; order-sensitive
injected collection equality; workspace substitution or URI validation;
mirror DICE dependence on the registry-list key; a public re-export or
production injector; a Need from any owner; direct IO; clock/request-generation
reuse; any fourth file; or any deferred owner, consumer, or activation.

#### Host registry-input owners design status

**Status:** Accepted after terminal latest-text review on 2026-07-26.

The exact three-file, 900-added-line implementation boundary is frozen for a
new private input-owner module, the existing package-policy owner, and only
the private module declaration. Separate dormant registry-list, complete
mirror-map, vendor-directory projection, and Refresh-token identities retain
Bazel's implicit BCR slash, structured post-converter normalization, explicit
empty-map distinction, exact lookup spelling, and independent invalidation.

Pinned-source review corrected two non-obvious identities before acceptance:
set/map equality ignores reorder-only changes, and equal reinjection preserves
the old stored value and iteration order. It also pinned the deferred
vendor-file fatal-read boundary. Implementation/evidence review confirmed
`SmallSet`/`SmallMap` and the existing projection pattern can satisfy the
contract within scope; architecture/orchestration review froze structured
inputs, separate DICE keys, the opaque token lifecycle, and all deferrals.
After the corrections, all three exact-latest-text verdicts returned
`ACCEPT`. No Rust, Cargo, fixture, dependency, API, consumer, IO, clock, or
activation changed.

Next packet: implement only the accepted
`WP-5-m1-host-registry-inputs` three-file prerequisite, serially and within
the exact evidence and stop gates above.

#### Host registry-input owners implementation status

**Status:** Accepted after terminal latest-diff review on 2026-07-26.

The dormant prerequisite changes exactly the three accepted paths with 899
additions and eight deletions. The new private owner uses Arc-backed
`SmallSet`/`SmallMap` values for separate registry-list and complete mirror-map
injected keys, preserves the implicit BCR slash and structured
post-converter normalization, and provides the separate opaque Refresh token.
The package-policy owner adds only the private vendor-directory projection;
`lib.rs` adds only the private module declaration. No public export,
production injector, consumer, IO, clock, dependency, fixture, or activation
is present.

Four focused tests prove normalization, fresh-order versus set/map equality,
retained old-order pruning, unequal A→B→A installation, workspace and input
isolation, exact successful dependency edges, fail-closed absence, vendor-only
projection pruning, typed vendor absence, and request-generation-independent
Refresh tokens. The complete `slug_bzlmod_v2` surface passed 190 unit plus 184
integration tests, zero failures, and zero doctests. GNU-Windows compile-only
validation built all twelve test executables. Formatting, exact scope/growth,
diff, archive, credential, and forbidden-edge scans passed.

An isolated temporary audit, removed before acceptance, clarified the DICE
invariant behind missing inputs. A direct absent `InjectedKey` has no graph
node or dependency; a consumer that maps that setup failure is non-replayable
after first injection in the same retained graph. Missing input is therefore
an activation-order invariant diagnostic, not a recoverable request state,
and no missing→present same-graph recovery is claimed. The later production
activation owner must atomically preinject every required registry input
before exposing any Host RegistryFunction or root consumer transaction.
Successful initialized consumers each retain exactly one correct named
input/projection dependency; fail-closed missing probes acquire zero forbidden
edges.

Source/parity, implementation/evidence, and architecture/orchestration
terminal latest-diff reviews all returned `ACCEPT`. Next packet: implement
only the accepted pure, root-free `WP-5-m1-host-registry-function` owner.
Perform no registry IO and retain every later file/fetch/activation boundary.

#### Host RegistryFunction owner implementation status

**Status:** Accepted after terminal latest-diff review on 2026-07-26.

The dormant pure owner changes exactly three paths with 1,543 additions and
no deletions. The new private `HostRegistryFunctionKey` is identified by the
normalized workspace and exact original registry spelling. It acquires mode,
vendor projection, Refresh-only invalidation, the visible lockfile, and
module mirrors in pinned Bazel order; propagates the sole visible-file Need;
selects mirrors by the unsubstituted spelling; and constructs the complete
descriptor without root evaluation, registry IO, request generation,
repository mapping, source preparation, writes, or production activation.

The descriptor retains compact original and `%workspace%`-resolved spellings,
the exact HTTP(S)/file hash mode, shared full lockfile, optional vendor path,
ordered selected mirrors, and the Refresh token only in Refresh. Manual
equality observes only the descriptor fields plus registry-file hashes and
selected-yanked state. Narrow borrowed accessors expose hash expectations and
selected-yanked reasons without exposing or copying the lockfile maps. The
packet-local URI scanner preserves Java `URI(String)` construction semantics
needed by `RegistryFactoryImpl`, including opaque versus hierarchical paths,
registry-authority fallback, scoped and embedded IPv6, signed-32-bit ports,
empty authority before query/fragment, ordered syntax-only mirrors, and
verbatim spelling.

Eleven focused tests prove all twelve scheme/mode cells, exact successful and
failure dependency prefixes, zero edges for absent injected inputs, Need and
Complete-error equality, original/resolved identity, default/override/empty
mirror shapes, URI error ordering, retained A-to-B-to-A invalidation, and
forbidden construction edges. Retained DICE proves owner recomputation plus
downstream pruning for module extensions, facts, and facts versions.
Lock-file version uses direct descriptor equality only because the accepted
visible v28 parser cannot produce a Complete non-v28 value; no unreachable
retained transition is claimed.

The complete `slug_bzlmod_v2` surface passed 197 unit plus 184 integration
tests, zero failures, and zero doctests. GNU-Windows compile-only validation
built all twelve test executables. Formatting, exact three-path/1,550-line
cap, diff, archive, credential, utility-reuse, and forbidden-edge gates
passed. Source/parity and architecture/orchestration terminal latest-diff
reviews returned `ACCEPT` after the URI, allocation, equality, and retained
evidence corrections.

Next packet: design only the one-file private Host Registry IO bridge. It
must factor Bazel-shaped expectation-aware remote and typed local executors
inside `registry_dice.rs` while preserving every active legacy wrapper and
legacy Off behavior byte-for-byte. Do not implement the Host registry-file
owner or activate any Host consumer in that design packet.

### Stage 5 Host Registry IO bridge design

`WP-5-m1-host-registry-io-bridge-design` freezes exactly one implementation
path:

- `app/slug_bzlmod_v2/src/registry_dice.rs`

The implementation may add at most 850 lines and delete at most 120. Inline
tests remain under `#[cfg(test)]`. No second file, Cargo/dependency change,
public item or reexport, production key, injector, activation, cache, lock,
interner, Host registry-file owner, root/mapping/source-preparation/write
owner, path observation, or direct filesystem IO enters this packet.

Keep every public legacy type, function, signature, URL dispatch, error
shape, and equality unchanged. `RegistryFileKey` retains its existing
`RegistryPolicyKey` and redundant `RootModuleFilesKey` dependencies and their
order. Legacy file-URL parsing stays before those dependencies. The shared
capability layer begins only after the legacy wrapper has completed those
steps.

Factor a private closed execution plan with exactly these cases:

```text
FetchUnverified
ReplayRecordedAbsent
RejectUnrecorded
VerifySha256([u8; 32])
```

Add only crate-private dormant Host entrypoints equivalent to:

```text
read_host_remote_registry_file(
    ctx, workspace, url, RegistryKnownFileHashesMode, RegistryFileExpectation
) -> Result<RegistryFileValue, typed bridge error>

read_local_registry_file(
    ctx, workspace, url, already-derived native path
) -> Result<RegistryFileValue, typed bridge error>
```

The bridge error surface is closed and fully typed. It distinguishes remote
mode/scheme mismatch, enforced missing checksum, missing request generation,
missing IO capability, remote transport, checksum mismatch with expected and
actual digests, and local read with URL/path/message. It retains compact
strings and existing URL/path/hash types, derives full semantic equality and
`Allocative`, exposes no string-erased Host error, and has no Need variant.
Legacy adapters exhaustively preserve the existing public error projection;
legacy-only invalid-lockfile-expectation handling stays outside the Host
entrypoint.

The Host remote matrix is exact:

| hash mode | unrecorded | recorded absent | recorded SHA |
| --- | --- | --- | --- |
| `IGNORE` | typed remote-routing error | typed remote-routing error | typed remote-routing error |
| `USE_AND_UPDATE` | fetch unverified | replay absence | verify SHA |
| `USE_IMMUTABLE_AND_UPDATE` | fetch unverified | fetch unverified | verify SHA |
| `ENFORCE` | typed missing-checksum error | replay absence | verify SHA |

Pinned `RegistryFactoryImpl` makes `IGNORE` reachable only for file
registries. Although a manually constructed `IndexRegistry` in Ignore mode
would fetch without hashes, the Host HTTP(S) wrapper must reject Ignore
before generation, capability, or IO. The later file owner routes file
registries to the local entrypoint.

Legacy Off does not pass through the Host mode/expectation adapter. It selects
`FetchUnverified` directly before inspecting `VisibleLockfileRead`, preserving
its existing behavior for ignored, absent, and SHA-bearing lockfiles
byte-for-byte. Legacy Update, Refresh, Error, and every active caller/output
remain unchanged.

Execution order is exact:

- `ReplayRecordedAbsent` and `RejectUnrecorded` acquire no generation or
  capability and perform no IO.
- `FetchUnverified` acquires `RegistryRequestGenerationKey` before capability
  lookup and IO. Found returns bytes, actual SHA, and recordable
  `RecordedSha256`; 404 returns `Io404` plus recordable `RecordedAbsent`;
  transport remains typed. Every outcome retains the pre-IO generation edge.
- `VerifySha256` acquires capability and performs IO first. Matching Found and
  checksum mismatch acquire no generation. A 404 or transport failure
  acquires generation only after IO; missing generation therefore masks that
  outcome exactly as the current bridge does.
- Local execution acquires capability and performs one `read_exact`. Found
  returns bytes and actual SHA with no generation and no recordable remote
  expectation. Local absence and read failure acquire generation after IO;
  missing generation masks the outcome. Missing capability precedes both IO
  and generation.

Direct absent generation has no retained dependency and is a non-replayable
preinjection invariant. Missing-input tests use fresh graphs or distinct test
identities and never claim missing-to-present recovery. No new production
semantic key or equality is added.

Focused inline evidence must prove the complete four-by-three Host matrix,
legacy Off translation, exact generation/capability/IO order, missing
generation and capability precedence, exact values/sources/recordable
expectations, expected/actual digest fields, local typed path/message fields,
separately allocated equality, and retained retry/stickiness. Unverified
remote outcomes retry on generation changes. Verified remote 404/transport
and local absence/read failure retry after generation changes. Verified
remote Found, checksum mismatch, and local Found remain sticky across
generation and scripted response changes. Test-only direct-dependency
tracking permits only `RegistryRequestGenerationKey`; global `RegistryIo` is
a capability call, not a DICE dependency.

Run the focused inline bridge tests, the unchanged twelve-test
`tests/registry_dice.rs` legacy regression, registry-sensitive
source-preparation tests, complete bzlmod/loading/core suites and doctests,
and all corresponding GNU-Windows test executables serially. Then run
formatting, diff, archive, exact one-file/growth, credential, public-API
baseline, call-site, and forbidden-edge scans.

Stop and replan on Host remote Ignore fetching; legacy Off routed through the
Host adapter; any legacy output, diagnostic, dependency, URL-validation, or
generation-order change; generation before verified/local Found or checksum
mismatch; IO before generation for unverified fetch; generation before IO for
verified/local 404 or error; direct filesystem/path Need; vendor or mirror
claims; an erased Host error; Host file owner/consumer/activation; a public
surface; a new production key/dependency/cache/map/set/lock/interner; a second
file; or growth beyond the cap.

#### Host Registry IO bridge design status

**Status:** Accepted after terminal latest-text review on 2026-07-26.

Pinned Bazel 9.2 source, the live legacy seam, and DICE architecture reviews
agree on the closed plan, exact Host mode/expectation table, and generation
ordering above. Review corrected the provisional `IGNORE` interpretation:
the Host remote wrapper rejects this factory-impossible cell, while legacy
Off selects the internal unverified plan directly. The one-file bridge is
implementable without changing any active legacy wrapper, public surface,
dependency, consumer, or activation. All three terminal design reviews
returned `ACCEPT`; no Rust, Cargo, fixture, or production state changed.

Next packet: implement only
`WP-5-m1-host-registry-io-bridge` inside the exact one-file scope above.

#### Host Registry IO bridge implementation status

**Status:** Accepted after terminal latest-diff review on 2026-07-26.

The bridge changes only `app/slug_bzlmod_v2/src/registry_dice.rs`, with 833
additions and 96 deletions inside the accepted 850/120 cap. It factors the
closed four-case execution plan, adds dormant crate-private Host remote and
typed local entrypoints, rejects Host remote Ignore before generation,
capability, or IO, and preserves legacy Off by selecting unverified fetch
directly before lockfile inspection. Fetch, verify, replay, rejection, and
local execution retain the accepted generation-before/after-IO ordering and
exact typed values and failures. No public item, production key, dependency,
consumer, activation, direct filesystem owner, cache, lock, map, set, or
interner was added.

Four inline tests prove the complete Host matrix, legacy translation,
dependency and precedence order, exact values and typed equality, and
discriminating retained retry/stickiness. The focused registry-sensitive
source-preparation slice passed 5 tests. The complete bzlmod surface passed
201 unit plus 184 integration tests; loading passed 54 and core passed 115;
all doctests passed, and GNU-Windows built all 20 corresponding test
executables. Formatting, diff, archive, exact scope/growth, credential,
public-API baseline, call-site, and forbidden-edge gates passed.

The first terminal review round accepted production semantics but required
stronger evidence for complete result fields, remote error equality,
successful unverified refetch, and checksum-mismatch stickiness. One bounded
test-only correction made every transition discriminating. Source/parity and
architecture/orchestration terminal latest-diff rereviews then returned
`ACCEPT`.

Next packet: design only
`WP-5-m1-host-registry-file-vendor-oracle`. Reuse the accepted registry
fixtures where possible and freeze checksum-present vendored hit with no
network request, fatal vendored-read failure with no fallback, non-vendored
or missing/wrong-kind vendor-path network fallback, and checksum-absent
network behavior before designing or implementing the separate private Host
registry-file owner.

### Stage 5 Host registry-file vendor oracle design

`WP-5-m1-host-registry-file-vendor-oracle` strengthens only the accepted
`registry-yanked-lockfile-mode` fixture. Its exact six-path allowlist is:

- `tests/v2_oracle/fixtures/registry-yanked-lockfile-mode/fixture.toml`;
- `tests/v2_oracle/fixtures/registry-yanked-lockfile-mode/expected/oracle.json`;
- `workspace/vendor-hit/_registries/127.0.0.1/first/modules/yyy/1.0.0/MODULE.bazel`;
- `workspace/vendor-fatal/_registries/127.0.0.1/first/modules/yyy/1.0.0/MODULE.bazel`;
- `workspace/vendor-hit/_registries/127.0.0.1/first/modules/aaa/1.0.0/MODULE.bazel`;
  and
- `workspace/vendor-wrong-kind/_registries/127.0.0.1/first/modules/yyy/1.0.0/MODULE.bazel/wrong-kind.txt`.

The last four paths are new regular files. The wrong-kind sentinel retains a
real directory at the computed `MODULE.bazel` path without a symlink or
harness operation. Change no server, harness, existing registry byte, root
BUILD file, second fixture, Rust, Cargo, dependency, API, DICE owner,
consumer, or activation.

Pinned Bazel 9.2 establishes the boundary:

- `RegistryFunction.java:63-85` reads the vendor-directory input before
  constructing the registry, and `RegistryFactoryImpl.java:38-82` forwards
  it with the exact HTTP hash mode.
- `IndexRegistry.java:166-242` resolves the lockfile expectation before
  vendor or network IO. It selects vendor bytes only when the checksum is
  present, the URL is not `file:`, and the computed path is a file. A selected
  vendor read/checksum failure is wrapped and returned without network
  fallback; a missing or wrong-kind path falls through to the downloader.
- `VendorManager.java:41,146-190,213-235` uses
  `<vendor>/_registries/<lowercase-host>/<percent-decoded-path>` and omits the
  dynamic port. `isUrlVendored` is exactly `Path.isFile`, and selected bytes
  are verified against the recorded checksum. The nearby `registry_cache`
  prose is stale; `_registries` is the code constant.
- `ModuleFileFunction.java:778-805` continues to a later registry only for
  NotFound. A vendor IOException is immediately `ERROR_ACCESSING_REGISTRY`.
  `ModCommand.java:292-308` projects graph errors to `MOD_COMMAND_UNKNOWN`,
  whose `failure_details.proto` exit is 37.
- `RepositoryOptions.java:39-70,359-370` and
  `BazelRepositoryModule.java:316-330,557-586,704-721` source
  workspace-relative vendor paths and apply explicit empty
  download/content-cache disabling.

Retain every current source anchor and add only `VendorManager.java`,
`ModCommand.java`, and `failure_details.proto`.
Update the fixture description, `oracle_notes`, and `translation_notes` in
place. Preserve the accepted cold Off rows' absent relative repository cache;
only the five new vendor rows and the strengthened Refresh row use explicit
empty download and contents caches. Do not retain the now-false statement
that every Off row uses an absent relative cache.

Insert exactly five Off rows after
`off_reuses_selected_yanked_reason_a` and before
`off_enforces_recorded_sha_before_yanked`. Before the first row, replace the
root module with version `0.1.1` and only `yyy@1.0.0`; advance the root version
through `0.1.2`, `0.1.3`, `0.1.4`, and `0.1.5` in the following rows. Every
row keeps the three existing registry arguments,
`--allow_yanked_versions=yyy@1.0.0`, `--lockfile_mode=off`,
`--repository_cache=`, and `--repo_contents_cache=`.

1. `off_vendor_checksum_present_hit` uses `--vendor_dir=vendor-hit`.
   The vendor file is byte-identical to the accepted yyy MODULE, SHA
   `9114f034663d930400ebc5993990181e4a83dc5f4d5e0c80f8b7b570ebe86969`.
   Expect exit 0 and yyy in the graph. First-registry yyy MODULE remains
   exactly 2, while successful RepoSpec resolution adds exactly one request
   apiece for first-registry yyy `source.json` and first-registry
   `bazel_registry.json`. Generate the complete cumulative manifest.
2. `off_vendor_checksum_present_read_failure_is_fatal` uses
   `--vendor_dir=vendor-fatal`. Its parse-valid yyy bytes append
   `# vendored-corrupt` and have SHA
   `536562f16e2c06150bc110253312ec93acd3601d2ae0e9a519ed64794cc77d37`.
   Expect exit 37 and the exact outer registry base, file URL, normalized
   `_registries/.../MODULE.bazel` vendor path, actual/wanted hashes, and
   rerun-vendor sentence. All request counts remain unchanged. Exclude
   ordinary `Failed to fetch registry file`, module-not-found, successful
   graph, and later-registry fallback claims.
3. `off_vendor_missing_path_falls_back_to_network` uses
   `--vendor_dir=vendor-missing`, whose yyy target is absent. Expect exit 0,
   yyy in the graph, no vendored-read diagnostic, and exactly one added first
   yyy MODULE request, taking its cumulative count from 2 to 3. Successful
   RepoSpec resolution also adds exactly one first-registry yyy `source.json`
   request and one first-registry `bazel_registry.json` request.
4. `off_vendor_wrong_kind_path_falls_back_to_network` uses
   `--vendor_dir=vendor-wrong-kind`, where the exact yyy MODULE target is the
   sentinel-retained directory. Expect exit 0, yyy in the graph, no
   vendored-read diagnostic, and exactly one further yyy MODULE request,
   taking its cumulative count from 3 to 4. Successful RepoSpec resolution
   again adds exactly one first-registry yyy `source.json` request and one
   first-registry `bazel_registry.json` request.
5. `off_vendor_checksum_present_hit_restored` returns to
   `--vendor_dir=vendor-hit`. Expect exit 0 and yyy in the graph.
   First-registry yyy MODULE remains exactly 4, while successful RepoSpec
   resolution adds exactly one first-registry yyy `source.json` request and
   one first-registry `bazel_registry.json` request. This is
   A-to-B-to-C-to-D-to-A command-input restoration evidence, not a claim that
   equal restored state rereads the vendor file.

Freeze every new asset byte-for-byte. `vendor-hit`'s misleading aaa file
is exactly
`module(name = "aaa", version = "1.0.0")\nbazel_dep(name = "olddep", version = "1.0.0")\n`,
SHA
`03361572ed042feb527deec5fdaaa9f7eadd3bf1be441f573356fe0e0275e58e`.
The wrong-kind sentinel is exactly `wrong kind\n`, SHA
`3e8e23ed51f1f26f6c4749317ba34266bd085021c76da92b2e87bff4b79b29ff`.

Changing the checked root and `--vendor_dir` together is mandatory.
`RepositoryDirectoryValue.VENDOR_DIRECTORY` is a precomputed dependency of
`RegistryFunction`, and rebuilt registries are non-comparable, so these
same-daemon rows cannot be explained by a root edit or stale
`ModuleFileValue`. Direct mutation of one unchanged vendor directory is not
observed and is forbidden without a synchronous expunge redesign. Explicit
empty cache flags prevent repository-cache or contents-cache hits from
explaining zero network requests.

Before the existing checksum-before-yanked row, restore the exact original
root version and both original dependencies in the same mutation batch that
changes the yyy lockfile digest to zero. Preserve that row and the rest of
the accepted sequence, but regenerate its complete manifests with the
shifted first-registry yyy MODULE counts: the preserved
`off_enforces_recorded_sha_before_yanked` row adds 4 to 5; the Refresh row
and `update_replays_selected_yanked_reason_b` retain 5; and the final
`error_checksum_precedes_yanked_rejection` row adds 5 to 6. Do not retain the
old 3/3/3/4 values. Strengthen the existing
`refresh_refetches_metadata_and_recorded_absence` row with
`--vendor_dir=vendor-hit`, `--repository_cache=`, and
`--repo_contents_cache=`. The valid, semantically distinct first-registry aaa
vendor file depends on `olddep`; the now-live HTTP aaa depends on `newdep`.
Refresh turns the recorded absence into checksum-empty fetch, so the row must
request first aaa once, select `newdep`, exclude `olddep`, and never use the
present vendor file. State the claim narrowly as Refresh recorded-absence
checksum-empty vendor bypass; the accepted hash-mode matrix owns the broader
unrecorded policy.

The five inserted rows make exactly fourteen commands. Every new Off row
must preserve the normalized lockfile digest and size byte-for-byte. Require
the complete cumulative request manifest for every row, exact root mutations
and restoration, all positive and negative output/diagnostic assertions, and
the vendor-tree kinds and asset hashes above. The final fixture may contain
at most 22 regular files, zero links, and 1,444 newline-counted lines: net
growth is at most four files and 600 lines. The full tracked fixture tree cap
is 1,301 regular files, 14 links, and 36,707 lines, or +17 files, zero links,
and +2,918 lines from checkpoint `df812c2c` / baseline `c039c347`.

This is oracle packet four after the v28 schema, Host visible-lockfile, and
Host RegistryFunction oracles. No fixture-growth review is due. The later
module-mirror/archive-fetch oracle would be packet five; run the focused
growth review immediately after that packet and before a sixth oracle unless
the roughly 100-file/10,000-line threshold fires sooner.

Run one pinned Bazel 9.2 generation and two distinct fresh-root replays, then
shut down each Bazel output-base server. Run the focused oracle parser/harness
test if its environment is available; an unavailable pytest environment is a
recorded residual, not permission to weaken fixture acceptance. Validate
exact row order, arguments, exits, mutations, restoration, cumulative
manifests, lockfile bytes, normalized diagnostics, source anchors,
provenance, six-path scope, inventory/growth, schema, archive status,
credentials, host-path absence, and `git diff --check`. Terminal acceptance
requires source/parity, implementation/evidence, and
architecture/orchestration latest-diff review.

Stop and replan on a first-registry yyy MODULE request in either hit row; any
request-count change in the fatal row; no yyy request in either fallback row;
a fatal vendor read that reaches network or exits other than 37; Refresh using
the misleading aaa vendor bytes or failing to request first aaa; any Off
lockfile change; stale root or vendor bytes after restoration; a
server/harness/registry-byte edit; symlink, fifth asset, seventh path, sixth
row, fifteenth command, or cap overflow; permission, sleep, or mutable
external-state tricks; a new fixture; or any vendor-command, file-registry,
mirror, archive/source-download, Rust, Host owner, consumer, or activation
claim.

#### Host registry-file vendor oracle design status

Status: `REPLAN` after terminal latest-text review on 2026-07-26.

The stopped design retained a six-path, four-asset, fourteen-command fixture
boundary and made no fixture, Rust, Cargo, dependency, API, DICE, consumer, or
activation change. One focused correction added the cache-application source
anchor, complete RepoSpec request deltas, exact asset bytes and hashes,
shifted downstream yyy counts, and truthful fixture metadata requirements.
The final source and implementation/evidence reviews then found a second
material correction: Refresh with `vendor-missing` and disabled caches must
fetch the checksum-present yyy MODULE, and the broad yyy-request stop gate
also rejects the intended yyy `source.json` request. Per the orchestration
correction limit, this packet ends in `REPLAN`.

Next packet: design and terminally rereview only
`WP-5-m1-host-registry-file-vendor-oracle-correction`. Move the misleading
aaa asset into `vendor-hit`, use `vendor-hit` for Refresh so recorded-SHA yyy
remains vendored while checksum-empty aaa bypasses the present vendor file,
retain the intended downstream yyy MODULE counts 4→5/5/5/6, and narrow the
stop gate to yyy MODULE requests in hit rows plus every request-count change
in the fatal row. Preserve every other accepted boundary, asset byte, hash,
row, cap, source anchor, and exclusion from the stopped draft.

#### Corrected Host registry-file vendor oracle design status

Status: `ACCEPT` after terminal latest-text review on 2026-07-26.

The correction changed only the four scheduled text sites: the misleading aaa
asset now shares `vendor-hit` with the checksum-present yyy asset, Refresh
uses that vendor tree, and the stop gate distinguishes yyy MODULE requests
from permitted RepoSpec metadata while forbidding every fatal-row request
change. The six-path, four-asset, fourteen-command scope, exact hashes,
4→5/5/5/6 downstream yyy MODULE sequence, caps, anchors, metadata updates,
validation contract, and exclusions remain unchanged. Source/parity,
implementation/evidence, and architecture/orchestration terminal latest-text
reviews all returned `ACCEPT`.

Next packet: implement only
`WP-5-m1-host-registry-file-vendor-oracle` in the exact accepted six paths.
Run one pinned Bazel 9.2 generation and two distinct fresh-root replays before
terminal latest-diff review. Do not edit the server, harness, existing
registry bytes, Rust, Cargo, dependencies, APIs, DICE, consumers, or
activation.

### Stage 5 Host registry-file owner pre-implementation audit

Status: `REPLAN` before Rust on 2026-07-26.

Pinned Bazel 9.2 and the live accepted seams confirm that the private owner
must compute `HostRegistryFunctionKey` first, preserve every descriptor/root/
path Need, keep the redundant local `HostRootModuleFileKey` edge, resolve a
checksum expectation before vendor work, select vendor bytes only for a
recorded SHA, fall back for missing/wrong-kind vendor candidates, and keep a
selected vendor read/checksum failure fatal. The owner remains DICE-owned;
a proposed non-DICE vendor capability was rejected because it would hide
create/edit/delete/restoration transitions and could not propagate path
Needs.

The live bridge exposes a prerequisite blocker. `read_local_registry_file`
accepts a native `path` but uses it only in diagnostics; the required
`RegistryIo` capability remains URL-only, and the production implementation
re-derives a path by stripping `file://` without Java-compatible decoding.
Consequently Host resolution cannot control the bytes read, and exact encoded,
non-UTF-8, or Windows native paths cannot be claimed. The honest implementation
closure would otherwise force the file owner to change the bridge trait,
runtime implementation, and two external test implementations in the same
packet.

No Rust, fixture, Cargo, dependency, API, DICE, consumer, or activation
changed. Next packet: design only
`WP-5-m1-host-registry-local-native-io-bridge-correction`. Freeze the smallest
native-path capability addition and exact closure while preserving the remote
URL method, every legacy caller/result/error, local generation ordering, and
public API shape as far as the required trait method permits. Only after that
correction is accepted and implemented may the five-file private Host
registry-file owner be redesigned.

### Stage 5 local native-path Registry IO bridge correction design

`WP-5-m1-host-registry-local-native-io-bridge-correction` changes exactly:

- `app/slug_bzlmod_v2/src/registry_dice.rs`; and
- `app/slug_core_v2/src/runtime/registry_io.rs`.

The cap is 220 additions and 80 deletions. Change no fixture, harness, Cargo
manifest, lockfile, dependency, Host owner, DICE key, consumer, activation,
loading, analysis, query, or other runtime file. Add no map, set, vector-backed
retained value, cache, interner, lock, or direct filesystem IO inside a DICE
compute.

Extend the existing public `RegistryIo` capability with one defaulted async
method:

```rust
async fn read_local_exact(
    &self,
    url: &RegistryFileUrl,
    path: &Path,
) -> Result<RegistryIoOutcome, RegistryTransportError> {
    let _ = path;
    self.read_exact(url).await
}
```

The default is mandatory: every existing external scripted implementation
continues to compile and preserves its URL-only behavior without closure
edits. This is the sole unavoidable additive public-trait change; add no
public type, function, installer, reexport, or dependency. Keep
`read_exact(&RegistryFileUrl)` and every remote call byte-for-byte.

Change only the accepted typed local executor to call
`read_local_exact(url, path)`. Preserve its exact behavior: Found returns
without request generation; NotFound requests generation after IO and returns
`LocalAbsence`; transport failure requests generation after IO and returns
the original URL, supplied native path, and message as `RegistryLocalError`.
Missing capability still fails before IO or generation. The legacy
`RegistryFileKey` continues deriving the same undecoded native path before
calling this executor, so this prerequisite does not claim or change legacy
URL decoding. The later Host owner will supply its separately source-matched
decoded/resolved path.

`HyperRegistryIo` must override `read_local_exact` and pass the supplied
`&Path` directly to `tokio::fs::read`. Preserve `read_exact`'s existing
`file:` dispatch, URL validation, messages, and HTTP behavior by routing its
validated legacy path through the same private native-path helper. A native
NotFound remains `RegistryIoOutcome::NotFound`; every other IO failure
retains the existing `reading local registry file <url>: ...` message.
This boundary must accept Unix non-UTF-8 paths and Windows native paths
without formatting and reparsing them.

Inline `registry_dice.rs` tests must install a scripted IO override whose URL
names an absent decoy while the supplied native path names the intended file
identity. Prove the local executor passes the path verbatim and preserves
Found/no-generation, NotFound/IO-before-generation, and error/IO-before-
generation ordering. Existing default-only scripted implementations and all
legacy bridge tests must compile and remain unchanged. Inline core runtime
tests must prove the production override reads the supplied path rather than
the decoy URL, preserves absence/directory error behavior and URL-based
diagnostics, and on Unix accepts a non-UTF-8 native filename.

Validate focused bridge and core runtime tests, then the full bzlmod unit,
integration, and doctest surface, focused core runtime tests, loading and core
downstream suites, and GNU-Windows no-run compilation for affected crates.
Run formatting, `git diff --check`, exact two-file/growth, archive, credential,
dependency, public-API, call-site, and forbidden-edge scans. Confirm no
`RegistryIo` implementation outside the two allowed files changed, no remote
call uses `read_local_exact`, and no Host file-owner symbol or activation
appears.

Stop and replan on a required external implementation edit; a nondefaulted
trait method; changed remote IO or legacy local result/error/generation
semantics; path formatting/reparsing in the production override; direct IO in
`registry_dice.rs`; a third file; a manifest/dependency/public-type change;
Host owner/consumer/activation work; or cap overflow. Terminal acceptance
requires source/contract, implementation-feasibility, and
architecture/orchestration latest-text review.

#### Local native-path Registry IO bridge correction design status

Status: `ACCEPT` after terminal latest-text review on 2026-07-26.

The exact two-file design adds one defaulted native-path method to the
existing capability, routes only the typed local executor through it, and
requires the production runtime override to read the supplied `&Path`
directly. Existing scripted implementations, remote IO, legacy URL parsing,
results, diagnostics, and generation ordering remain unchanged. Focused tests
freeze path-versus-decoy selection, all local terminal orders, Unix non-UTF-8
support, Windows-native compilation, and the absence of Host owner scope.
Source/contract, implementation-feasibility, and architecture/orchestration
terminal latest-text reviews all returned `ACCEPT`.

Next packet: implement only
`WP-5-m1-host-registry-local-native-io-bridge-correction` in
`registry_dice.rs` and core runtime `registry_io.rs` under the exact
220-addition/80-deletion cap.

#### Local native-path Registry IO bridge correction implementation status

Status: `ACCEPT` after terminal latest-diff review on 2026-07-26.

Exactly `registry_dice.rs` and core runtime `registry_io.rs` changed by
113 additions and two deletions. The defaulted native-path method, sole local
executor routing, direct production `Path` read, legacy behavior, and focused
decoy/non-UTF-8/order tests passed. Bzlmod passed 201 unit and 184 integration
tests, loading 54, core 104 unit and 13 integration tests, all doctests, and
all 20 GNU-Windows executables. Formatting, diff, archive, scope, dependency,
API, callsite, credential, remote-call, and forbidden-edge gates passed. All
three terminal latest-diff reviews returned `ACCEPT`.

Next packet: redesign only `WP-5-m1-host-registry-file-owner-design` against
the accepted native-path bridge. Do not edit Rust or activate a consumer.

#### Host registry-file vendor oracle implementation status

Status: `ACCEPT` after terminal latest-diff review on 2026-07-26.

The exact six-path implementation adds four immutable vendor assets and five
Off rows to the accepted fixture. Pinned Bazel 9.2 generation and two
absolute, distinct fresh-root replays passed with all output-base servers
shut down. The new-row yyy MODULE counts are 2/2/3/4/4; yyy `source.json` and
first `bazel_registry.json` are 3/3/4/5/6; the fatal row exits 37 without a
request change; downstream yyy MODULE counts are 4/5/5/5/6; and Refresh
requests first aaa, selects `newdep`, excludes `olddep`, and keeps yyy MODULE
at 5. All five new Off rows preserve the normalized 77,574-byte lockfile.

The fixture is exactly 22 regular files, zero links, and 1,340 lines, for net
growth of four files and 496 lines. The full tracked fixture tree is 1,301
regular files, 14 links, and 36,603 lines. Direct fixture parsing, all ten
packet-validator tests, exact asset hashes, six-path scope, source anchors,
schema, archive, credential, normalized host-path, and diff gates passed.
`pytest` is unavailable; the direct parser and unittest validator provide the
available focused harness evidence. One generation-time assertion correction
matched the existing normalizer's collapsed fatal file-URL/workspace seam;
the fixture contract and production code did not change. Source/parity,
implementation/evidence, and architecture/orchestration terminal latest-diff
reviews all returned `ACCEPT`.

Next packet: design only
`WP-5-m1-host-registry-file-owner-design`. Freeze the separate private Host
registry-file owner described by Stage 5 step 7: it consumes the accepted
pure descriptor and Registry IO bridge, propagates visible/root/path Needs,
retains the required redundant local root edge, keeps construction/root/local/
remote/vendor/checksum failures typed and distinct, and adds no consumer or
activation.

### Stage 5 private Host registry-file owner design

`WP-5-m1-host-registry-file-owner` changes exactly:

- `app/slug_bzlmod_v2/src/host_registry.rs`; and
- `app/slug_bzlmod_v2/src/registry_dice.rs`.

The cap is 2,100 additions and 80 deletions. Keep all owner tests inline.
Change no fixture, harness, `lib.rs`, other Rust file, Cargo manifest,
lockfile, dependency, public item or reexport, legacy key/result/error,
runtime implementation, IO trait, consumer, composition, or activation.
Add no retained standard map, set, or vector, cache, interner, process
global, or lock. Perform no direct filesystem IO in a DICE compute.

This design supersedes the historical checksum-only and fatal vendor-path
proposals. Pinned Bazel 9.2 uses one `IndexRegistry.grabFile` owner for both
checksum-bearing immutable files and checksum-disabled mutable
`metadata.json`; `useChecksum` changes lockfile lookup, vendor eligibility,
download-event recording, and `ENFORCE` behavior. It is therefore semantic
DICE identity, not a caller-side hint.

Add private:

```rust
enum HostRegistryFileChecksumMode {
    UseChecksum,
    MutableNoChecksum,
}

struct HostRegistryFileKey {
    registry: HostRegistryFunctionKey,
    url: RegistryFileUrl,
    checksum_mode: HostRegistryFileChecksumMode,
}

type HostRegistryFileOutcome =
    SourcePreparationOutcome<
        Arc<Result<RegistryFileValue, HostRegistryFileError>>
    >;
```

The nested RegistryFunction key contributes the normalized workspace and
exact original registry spelling; the file key additionally retains the
exact file URL and checksum mode. Derive full hash/equality/`Allocative` for
key and private semantic types, using `Arc`, `CompactString`, and existing
compact path/URL/hash types. The value uses `complete_eq` and `is_complete`;
every Need, including self-comparison, is invalid and unequal.

Keep complete errors structurally distinct:

```text
Construction(HostRegistryFunctionError)
Root(HostRootModuleFileError)
Local(HostRegistryLocalFileError)
MutableEnforceInvariant { url }
Expectation { url, message }
VendorPath(HostRegistryVendorPathError)
VendorFile { url, logical_path, error: HostFileError }
Remote(RegistryRemoteError)
Checksum {
  source: Remote | Vendor { logical_path },
  url, expected, actual
}
```

`HostRegistryLocalFileError` distinguishes invalid URI, strict percent/UTF-8
decoding, unsupported nonlocal file authority, native-path normalization,
`PathResolutionError`, and `RegistryLocalError`.
`HostRegistryVendorPathError` is limited to pure URI, missing-host,
second-pass Java decoding, and normalized path-construction failures. It must
not contain operational lstat/readlink/stat errors. Intercept the bridge's
remote checksum mismatch into the common top-level checksum error; retain
every other remote bridge error unchanged. A Need never enters an error.

#### Exact compute order and local branch

Every compute first requests its `HostRegistryFunctionKey`. A descriptor
path Need becomes the same path Need in `SourcePreparationNeeds`; a complete
descriptor error becomes `Construction`. No root, URL conversion, expectation,
path, generation, capability, vendor, or IO work may precede it.

For a file descriptor:

1. compute the redundant direct `HostRootModuleFileKey` before local URL/path
   conversion or IO;
2. propagate its root-bootstrap or path Need unchanged, map its complete
   error to `Root`, and retain no successful root value;
3. convert the exact `file:` URL using the platform-specific JDK file URL
   handler reached by Bazel's `URI.toURL().openConnection()`, not
   `URI.getPath()` or `Url::to_file_path`;
4. normalize that logical absolute native path and compute
   `ResolvedPathKey(PathObservationNamespace::Host, logical_path)`;
5. propagate its path Need, map a complete resolution failure to `Local`, and
   pass only `resolved.real_path().as_path()` to the accepted
   `read_local_registry_file`; and
6. map its typed bridge failure to `Local`.

Do not inspect a lockfile expectation or vendor directory on this branch.
Do not use `HostFileBytesKey` for local registry bytes. The accepted bridge
preserves Found without generation and requests generation only after a
NotFound or read failure; a missing capability still precedes IO and
generation. Project a complete local value after the bridge exactly like
`IndexRegistry.grabFile`'s download event: `UseChecksum` Found records
`RecordedSha256(actual)` and `UseChecksum` NotFound records
`RecordedAbsent`, while both `MutableNoChecksum` results retain
`recordable_remote_expectation: None`. Neither local mode reads a lockfile
expectation or uses vendor bytes. The direct root edge is safe only because
the dormant `HostRootModuleFileKey` graph remains registry-free. Later
production composition must still complete root before exposing construction
or file requests.

The local converter is platform-exact:

- both platforms perform one strict JDK `ParseUtil.decode` pass: `%2520`
  becomes literal `%20`, `+` remains `+`, malformed escape pairs and malformed
  UTF-8 are distinct typed conversion errors, and no replacement character is
  introduced;
- Unix decodes `URL.getPath()`, so query and fragment are omitted; authority
  must be absent/empty, exact `~`, or case-insensitive `localhost`, otherwise
  return the wrapped unsupported-nonlocal local-access error before path
  observation, generation, capability, or IO;
- Windows decodes `URL.getFile()`, so the query is part of the filename while
  the fragment is omitted, replaces `/` with `\` and `|` with `:`, and removes
  leading slash(es) before a drive spec through native `File` normalization;
  a nonlocal authority forms `\\host\path`, and a missing UNC candidate
  returns the same disabled-FTP-fallback unsupported-nonlocal access error
  instead of ordinary local absence.

Freeze these pure/adversarial cases:

```text
Unix:
file:///tmp/a+b%20c%2520d?ignored#frag -> /tmp/a+b c%20d
file:///tmp/%FF -> strict UTF-8 conversion error

Windows:
file:///c%7C/a%20b?q=x#frag -> c:\a b?q=x
file://bad/path -> \\bad\path only when that UNC candidate exists;
                   otherwise unsupported nonlocal file URL
```

#### Remote checksum and mutable modes

For `UseChecksum`, resolve the exact URL's descriptor expectation before URI
or vendor-path construction. Expectation failure is typed and precedes all
vendor, generation, capability, and IO work. Only
`RecordedSha256(expected)` plus a present vendor directory is vendor-eligible;
unrecorded and recorded-absence states bypass vendor and enter the accepted
remote bridge unchanged.

For `MutableNoChecksum`, do not inspect the expectation and do not construct
or observe a vendor path. `ENFORCE` returns
`MutableEnforceInvariant` before URI parsing, generation, capability, or IO;
this is the private typed equivalent of Bazel's `Preconditions.checkState`,
not `MissingChecksum`. Every non-Enforce remote mode performs an unverified
fetch, obtains generation before capability/IO, and returns
`recordable_remote_expectation: None` for both Found and NotFound because
Bazel posts no `RegistryFileDownloadEvent`.

Add only one crate-private dormant bridge entrypoint in `registry_dice.rs`
for that non-recording mutable fetch. It reuses the existing closed
`FetchUnverified` executor and exact generation-before-IO behavior, then
clears the recordable expectation on complete Found/NotFound values. It adds
no plan variant, public surface, trait method, capability, key, or legacy
caller, and it must not change any existing Host or legacy result.

The checksum-bearing bridge behavior remains:

- unrecorded Update/Off and Refresh fetch unverified with generation before
  capability/IO;
- recorded absence Update/Off/Error replays without generation or IO;
- recorded absence Refresh fetches unverified with generation first;
- recorded SHA verifies through capability/IO first, returns success or
  checksum mismatch without generation, and requests generation only after
  404 or transport failure; and
- `IGNORE` never reaches the remote branch.

#### Exact vendor path and selection

Port the pinned two-stage Java path derivation, not `Url::path`, a single
percent decoder, or a plus-preserving decoder:

```text
<vendor>/_registries/
  <URI.getHost().toLowerCase(Locale.ROOT)>/
  <URLDecoder.decode(URI.getPath(), UTF-8) after removing one leading slash>
```

`URI.getPath()` first decodes escaped octets with Java UTF-8 replacement
semantics. `URLDecoder` then decodes `%HH` a second time and maps `+` to a
space. Omit port, query, and fragment. Remove exactly one leading slash
before applying Bazel `Path.getRelative` behavior, including its
absolute-second-path replacement. Missing host or illegal second-pass escape
is a typed `VendorPath` error before observation.

The mandatory adversarial pure case is:

```text
vendor: /V
URL: https://EXAMPLE.test:8443/a%2520b+c?ignored=yes#frag
path: /V/_registries/example.test/a b c
```

Also freeze `/%252Fescape`: two decodes produce `//escape`, the single strip
leaves `/escape`, and the absolute relative operand replaces the vendor
prefix exactly as Bazel does.

Observe the candidate with `HostFileBytesKey` so resolution and byte
observation remain DICE-owned. Classify its complete result by the operation
that failed:

- Missing, WrongKind, Cycle, InfiniteExpansion, and Observation or
  InconsistentState errors whose operation is not `FileBytes` are
  preselection failures and fall through to the checksum-verifying remote
  bridge;
- a regular or special file, including either behind followed symlinks, is
  selected;
- any path Need propagates unchanged;
- a selected `FileBytes` observation/inconsistency is fatal `VendorFile`
  without remote IO or generation;
- selected bytes with the wrong SHA are fatal `Checksum::Vendor` without
  fallback; and
- selected bytes with the expected SHA return Found with that SHA and a
  recordable recorded-SHA expectation, without remote IO or generation.

This matches `Path.isFile`, whose followed `statNullable` converts every
operational `IOException` to false while accepting regular and special files.
Do not compute an explicit vendor `ResolvedPathKey` and then make its
operational error fatal.

#### Focused retained-DICE evidence and gates

Inline focused tests must prove:

1. key identity includes workspace, exact original registry, exact file URL,
   and checksum mode; separately allocated complete values/errors compare
   equal, while every Need is invalid/self-unequal;
2. descriptor-first dependency prefixes, typed construction error, and
   visible path Need exclude root, path, expectation, vendor, generation,
   capability, and IO;
3. local root bootstrap/path Need and root error precede URL/path/IO, while a
   successful root is discarded and the exact resolved native path reaches
   the bridge with no local `HostFileBytesKey`;
4. strict platform file-URL conversion, local checksum-mode
   Found/NotFound recordability without expectation/vendor work, Found
   stickiness, NotFound/read-failure generation-after-IO, native symlink
   retargeting including a Unix non-UTF-8 physical target, Windows
   drive/query and UNC present/missing behavior, and a semantic root change
   causing the required reread;
5. the complete checksum-bearing mode/expectation/generation matrix, remote
   error projection, and common remote checksum source;
6. mutable non-Enforce skips expectation/vendor, generates before IO, and
   records no expectation; mutable Enforce fails before all of them;
7. exact two-pass vendor decoding, lowercase host, port/query/fragment
   omission, single-strip absolute replacement, and typed malformed/missing
   host cases;
8. vendor missing/directory/broken-or-cyclic symlink and injected operational
   selection failures fall remote, while regular, special,
   symlink-to-regular, and symlink-to-special targets select;
9. selected read/disappearance/checksum failure is fatal with no remote
   request or generation; and
10. vendor create/edit/delete/recreate, physical retarget, and A→B→A
    restoration invalidate or prune for a demonstrated DICE edge.

Prove structurally that the new owner is private and dormant, the local
branch has exactly the direct root and resolved-path edges but no
`HostFileBytesKey`, the remote branch has no root edge, `HostRootModuleFileKey`
has no registry edge, and no loading/core/analysis/query consumer or
activation appears. The accepted fourteen-command vendor oracle remains
pinned source evidence only until a later activation packet; do not claim
Slug command-path replay.

Validate focused owner and mutable-bridge tests first, then full bzlmod unit,
integration, and doctest suites, loading/core downstream suites, and all
affected GNU-Windows no-run test executables. Run formatting, diff, archive,
credential, dependency, public-API, exact two-file/growth, legacy-callsite,
remote-plan, local-native-path, no-direct-IO, no-lock, and forbidden-edge
gates.

Stop and replan on checksum mode omitted from identity; mutable output that
records an expectation; mutable Enforce performing ordinary fetch or
returning MissingChecksum; expectation/vendor work on an inapplicable branch;
vendor operational selection error made fatal; special-file rejection;
selected-file fallback; local `HostFileBytesKey`; changed legacy/public
behavior; a third file; cap overflow; consumer/composition/activation work;
new dependency, cache, interner, or lock; or direct filesystem IO in DICE.
Terminal acceptance requires source/parity, native DICE/evidence, and
architecture/orchestration latest-text review before Rust.

#### Private Host registry-file owner design status

Status: `REPLAN` before Rust on 2026-07-26.

The source and DICE audits corrected the stopped draft to include checksum
mode in key identity, non-recording mutable remote fetch, checksum-enabled
local recordability, platform JDK file-URL conversion, special-file vendor
selection, and remote fallback for every operational vendor preselection
failure. The native DICE reviewer accepted the bounded two-file closure, and
the architecture rereviewer accepted the corrected ownership and scope.

Terminal source review then found a second material prerequisite miss.
OpenJDK `FileURLConnection` returns directory-listing bytes for a local
`file:` directory, while the accepted production native-path bridge uses
`tokio::fs::read` and returns a directory read error. The owner cannot repair
or hide that transport mismatch without changing the accepted runtime bridge.
Per the one-correction packet limit, the design ends in `REPLAN`; the draft
above is preserved as evidence, not an implementation contract.

No Rust, fixture, harness, Cargo, dependency, API, DICE key, consumer, or
activation changed. Next packet: design only
`WP-5-m1-host-registry-local-directory-oracle-design`. Freeze the smallest
pinned Bazel 9.2 oracle that discriminates regular, absent, and directory
local registry-file transport, exact listing bytes/order and diagnostic
projection, checksum-enabled event/lockfile effects, mutable no-checksum
effects, and relevant same-daemon transitions. Do not edit Rust or redesign
the bridge until that oracle is accepted and implemented.

### Stage 5 local registry-directory transport oracle design

`WP-5-m1-host-registry-local-directory-oracle-design` strengthens only the
accepted `nonroot-interim-module-graph` fixture. That retained-daemon Bazel
9.2 fixture already contains the complete local registry and embedded-module
closure, six ordinary-file rows, and an ignored `devonly@1.0.0` module with
regular `MODULE.bazel` and `source.json`. It is the smaller owner than the
thirteen-row, two-registry discovery fixture and avoids copying any module or
registry scaffold.

The observed `/usr/bin/bazel` 9.2.0 distribution runs Azul OpenJDK
25.0.2+10-LTS. OpenJDK
`src/java.base/share/classes/sun/net/www/protocol/file/FileURLConnection.java`
handles a directory by taking its direct `File.list()` names, sorting them
with `Collator.getInstance()`, appending `'\n'` after every name, and encoding
the complete string with `String.getBytes()`. The oracle deliberately uses
two ASCII, Windows-safe names with distinct alphabetic prefixes and makes no
claim beyond the observed runtime, locale, and default charset:

```text
module(name = 'devonly', version = '1.0.0')
print('DIRECTORY_LISTING_SENTINEL')
```

The exact transport bytes are the two displayed lines in that order, each
newline-terminated: 80 bytes, SHA-256
`0bd130df32a894c40b5d19afab988c7c8beb4a134eccec4744e4619d66db1408`,
and SRI `sha256-C9Ew3zKolMQLXRmvq5iMfIvrShNOzOxHRORhnWbbFAg=`. The
ordinary `devonly` module file is 44 bytes with SHA-256
`ad25b8e864b5a6977648385006a87b8bb7b53b8a06e8d2bdd61bd35848ca6154`
and SRI `sha256-rSW46GS1ppd2SDhQBqh7i7e1O4oG6NK91hvTWEjKYVQ=`.

Pinned Bazel 9.2 source owns the projections:

- `IndexRegistry.java:145-162,245-255` sends `MODULE.bazel` through
  `grabFile(..., useChecksum=true)` and posts a
  `RegistryFileDownloadEvent` containing the listing SHA;
- `ModuleFileFunction.java:189-214,262-277` parses and executes those bytes,
  validates the declared name/version, and retains the event map;
- `IndexRegistry.java` `grabJsonFile`, `parseJson`, and
  `getYankedVersions` decode registry JSON as UTF-8 but fetch mutable
  `metadata.json` with `useChecksum=false`;
- `YankedVersionsFunction.java:47-61` projects a metadata parse `IOException`
  as an exact warning and fails open, so the command must succeed;
- `RepoSpecFunction.java:48-74`,
  `BazelModuleResolutionFunction.java:105-147`, and
  `ArchiveRepoSpecBuilder.java` carry the checksum-enabled module bytes into
  the `remote_module_file_integrity` printed by `mod show_repo`; and
- `BazelLockFileModule.java:167-176` deliberately removes every `file:`
  registry hash from the visible lockfile. Therefore the internal
  checksum-enabled event is observable through `show_repo`, while every
  successful local-registry row must retain visible
  `"registryFileHashes": {}`. Mutable metadata posts no event by source
  contract; the oracle can prove its byte and fail-open projection, not that
  hidden negative fact independently.

The exact six-path allowlist is:

- `tests/v2_oracle/fixtures/nonroot-interim-module-graph/fixture.toml`;
- `tests/v2_oracle/fixtures/nonroot-interim-module-graph/expected/oracle.json`;
- `workspace/registry/modules/devonly/1.0.0/MODULE.directory/module(name = 'devonly', version = '1.0.0')`;
- `workspace/registry/modules/devonly/1.0.0/MODULE.directory/print('DIRECTORY_LISTING_SENTINEL')`;
- `workspace/registry/modules/devonly/1.0.0/MODULE.directory-link`, a relative
  symlink to `MODULE.directory`; and
- `workspace/registry/modules/devonly/metadata.directory-link`, a relative
  symlink to `1.0.0/MODULE.directory`.

The two child files each contain exactly
`CHILD_CONTENT_IS_NOT_TRANSPORT\n`. Their non-Starlark contents are not
transport input; successful module evaluation additionally discriminates JDK
filename listing from child-content concatenation. Both symlinks remain
staged during the accepted six-row prefix, where `devonly` is an ignored
nonroot dev dependency.

Append exactly five rows. Each sets `compare = "semantic"` so its nonempty
command-level manifest is compared rather than merely captured:

1. `regular_local_registry_file_baseline`. Leave the terminal accepted row's
   now-unreachable `subject` collision untouched, replace the root's
   `subject` and `shared` dependencies with
   `devonly@1.0.0`, and advance root version `0.1.3` to `0.1.4`. Run
   `mod show_repo @@devonly+ --lockfile_mode=update
   --registry=file://%workspace%/registry`. Require exit 0, the ordinary
   module SRI above, no directory print or metadata warning, and a semantic
   `MODULE.bazel.lock` manifest whose visible `registryFileHashes` is empty.
   This row is the suffix restoration baseline.
2. `directory_module_listing_bytes_and_checksum`. Rename regular
   `MODULE.bazel` to `MODULE.regular`, promote `MODULE.directory-link` to
   `MODULE.bazel`, and advance root version to `0.1.5`. Repeat `show_repo` in
   Update mode. Require exit 0, the exact directory-listing SRI above, silent
   registry `print`, no directory-read diagnostic, and a byte-identical
   visible-lockfile manifest. The changed printed integrity and unchanged
   filtered lockfile jointly discriminate the internal checksum-bearing
   bytes from visible local-hash publication.
3. `mutable_metadata_directory_lists_then_fails_open`. Promote
   `metadata.directory-link` to `metadata.json`, advance root version to
   `0.1.6`, and run the same `show_repo` under
   `--lockfile_mode=refresh`. Require exit 0, the directory module SRI and
   successful `devonly` repo definition, and the exact warning prefix
   `Could not read metadata file for
   module devonly from registry`, the `metadata.json` URL, and the pinned
   Gson wrong-token diagnostic. Exclude `Is a directory`, the print
   sentinel, and any yanked rejection. The visible lockfile must remain
   byte-identical and keep no metadata URL/hash.
4. `parked_directory_links_are_absent`. Park both live symlinks back at their
   exact staged names, advance root version to `0.1.7`, and run
   `mod graph --lockfile_mode=off` with the local registry. Regular
   `MODULE.regular` remains parked, so require exit 37 and the exact
   `devonly@1.0.0` local-registry `MODULE.bazel: not found` chain. Exclude
   declaration, metadata, and directory-read diagnostics. The failed command
   must leave the visible-lockfile manifest byte-identical.
5. `regular_local_registry_file_restores_exactly`. Restore
   `MODULE.regular` to `MODULE.bazel` and restore root version `0.1.7` to
   `0.1.4`, making the root, module path, staged symlinks, and metadata state
   byte-for-byte equal to row 1. Repeat the Update `show_repo`; require the
   ordinary SRI, no directory print or metadata warning, and the exact row-1
   lockfile manifest.

The existing mutation harness may rename regular files and symlinks but
intentionally rejects directory rename/delete. This contract never mutates a
real directory: it moves only the ordinary module file and the two staged
symlinks. No harness change or directory special case is permitted.

Visible-lockfile replay requires no new normalizer. The pinned source filters
all local URLs before serialization, and two read-only Bazel 9.2 probes in
distinct `/tmp/slug-local-lock-probe.*` roots produced the same 1,228-byte
lockfile SHA-256
`0ef28bd1c9d2583bcb82da1cc393a5973b7c84839f5961157b4ac99b2c3aecb7`,
with `"registryFileHashes": {}` and no `file:` or temporary-root text.
Generation remains the authority for the exact fixture digest after the
five accepted mutations.

Implementation may change exactly two existing regular files, add two
regular child files, and add two relative symlinks. The final fixture is
exactly eleven commands, 54 regular files, two symlinks, and at most 998
newline-counted lines: net at most two regular files, two symlinks, and 600
lines from the current 52/0/398 baseline. The full tracked fixture tree is at
most 1,303 regular files, 16 symlinks, and 37,203 lines from the accepted
1,301/14/36,603 baseline; ignored Python bytecode is excluded.

Run one pinned Bazel 9.2 generation and two absolute, distinct fresh-root
replays, then shut down every used Bazel output-base server. Validate the
exact six-path diff, symlink targets and restoration, two child basenames and
contents, 80/44-byte hashes and SRIs, eleven unique rows in exact order,
arguments/exits/mutations, positive and negative diagnostics, all five
semantic lockfile manifests and their equality groups, fixture metadata and
OpenJDK/Bazel anchors, parser plus focused packet-validator tests, inventory
and line caps, schema, normalization, archive status, credential scan, and
host-path-free normalized generated fields. Raw captured stdout/stderr retain
the oracle run's diagnostic evidence by existing harness contract.

This is the fifth accepted oracle packet after fixture-growth checkpoint
`df812c2c` / baseline tree `c039c347`, following `eb8c2d23`, `d20f6557`,
`204ee408`, and `dd57518e`. After implementation acceptance, run the required
focused fixture-growth review before scheduling a sixth oracle packet.
Inventory growth by packet, fixture, and repeated subtree; revalidate that
each retained row remains discriminating; and preserve pinned provenance,
hermetic replay, failure isolation, and exact expected output.

Stop on different listing bytes/order/SRI, a metadata command failure instead
of fail-open success, a visible local registry hash or fresh-root-dependent
lockfile, different absence/recovery behavior, a twelfth command, a seventh
changed path, a third symlink, a cap overflow,
directory mutation, absolute symlink, harness/server/Rust/Cargo/dependency/API
edit, a fifth new asset entry, direct bridge correction, Host owner redesign,
consumer, or activation.
Terminal acceptance requires source/parity, native evidence, and
architecture/orchestration latest-text review.

#### Local registry-directory transport oracle design status

Status: `ACCEPT` after terminal latest-text review on 2026-07-26.

The accepted six-path/four-entry design reuses the pinned retained-daemon
`nonroot-interim-module-graph` fixture. Five semantic rows move the same
`devonly` registry URL through regular, directory, mutable-metadata
fail-open, absent, and exact regular/root restoration states. Exact
80-byte directory and 44-byte regular SRIs, visible local-hash filtering,
relative staged symlinks, manifest equality, fresh-root replay, inventory,
and stop gates are frozen without a fixture, harness, Rust, Cargo,
dependency, API, consumer, or activation edit.

Two distinct-root Bazel 9.2 probes closed the provisional lockfile-normalizer
question: both produced the same path-free visible lockfile, matching
`BazelLockFileModule`'s explicit `file:` filter. Source/parity,
implementation/evidence, and architecture/orchestration terminal reviews all
returned `ACCEPT`.

Next packet: implement only
`WP-5-m1-host-registry-local-directory-oracle` inside the exact contract
above. After acceptance, run the required five-packet fixture-growth review
before scheduling any sixth oracle.

#### Local registry-directory transport oracle implementation status

Status: `ACCEPT` in `22de3631` after terminal corrected latest-diff review on
2026-07-26.

Exactly the accepted six paths changed. The retained fixture now has eleven
commands, 54 regular files, two relative symlinks, and 780
newline-counted lines. The five semantic suffix rows produced exits
`0/0/0/37/0`; rows 1 and 5 printed ordinary module SRI
`sha256-rSW46GS1ppd2SDhQBqh7i7e1O4oG6NK91hvTWEjKYVQ=`, rows 2 and 3
printed directory-listing SRI
`sha256-C9Ew3zKolMQLXRmvq5iMfIvrShNOzOxHRORhnWbbFAg=`, and row 4 printed
none. The exact metadata fail-open Gson warning and local-registry absence
chain matched. Every row retained the same path-free 1,228-byte visible
lockfile, SHA-256
`0ef28bd1c9d2583bcb82da1cc393a5973b7c84839f5961157b4ac99b2c3aecb7`,
with empty `registryFileHashes`.

Pinned Bazel 9.2 generation and four absolute, distinct fresh-root replays
across the implementation and correction reviews passed. The original six
normalized evidence records remained equal except removal of a stale
output-base startup warning. The two child contents, 80-byte listing,
44-byte ordinary file, hashes/SRIs, symlink modes/targets, exact restoration,
schema, normalized host paths, credentials, diff, and server cleanup passed.
The focused parser/packet-validator suite passed all 52 tests. One bounded
TOML-only review correction removed an external OpenJDK path from the
Bazel-commit anchor list and strengthened the Gson assertion through exact
line, column, and path; all three terminal rereviews returned `ACCEPT`.

The mandatory fifth-packet fixture-growth checkpoint from `c039c347` to
`22de3631` is recorded in the oracle-harness owner plan. It accepted
1,303 regular files, 16 symlinks, and 36,985 newline-counted lines
(+19/+2/+3,196), all 61 affected-fixture rows, and pruning/replay sets
`none`.

Next packet: design only
`WP-5-m1-host-registry-local-directory-bridge-correction-design`. Freeze the
smallest runtime bridge correction that returns the pinned JDK directory
listing bytes for local registry paths while preserving exact regular,
missing, URL-conversion, generation-order, legacy, and cross-platform
behavior. Do not edit Rust until that correction receives terminal
latest-text review, and do not redesign the private Host registry-file owner,
consumer, or activation in the same packet.

#### Local registry-directory runtime bridge correction design status

Status: `REPLAN` before Rust on 2026-07-26.

The live runtime seam remains structurally bounded: production
`HyperRegistryIo::read_local_exact` already receives the selected native
`Path`, the default trait method preserves every legacy implementation, and
the existing DICE caller performs local IO before requesting generation.
The production correction plus its regular, missing, non-directory-error,
URL-diagnostic, and symlink-to-directory tests could therefore remain inside
`app/slug_core_v2/src/runtime/registry_io.rs`; the unchanged typed DICE caller
and its existing tests preserve IO-before-generation ordering.

The exact byte contract is not bounded by the accepted ASCII oracle, however.
Pinned Bazel 9.2 source at `8220c619` in
`src/main/cpp/blaze.cc:384,418-428,456` shows that the default launch sets
`file.encoding=ISO-8859-1` and the root locale before appending any explicit
user JVM options. OpenJDK 25.0.2 source at
`openjdk/jdk25u@405a5699ebd097464ed3fc9345414b0774a2edc9`,
`FileURLConnection.java:76-87,180-205`, follows a directory, obtains every
direct child name through `File.list()`, stable-sorts the names with the
root/default `RuleBasedCollator`, appends one newline per name, and encodes
the complete string through that Bazel-selected default charset. Empty
directories return zero bytes; child kinds and contents are not inspected.
Filename decoding still comes from the platform/JNU path boundary on Unix
(`UnixFileSystem_md.c:296-358`, `java_props_md.c:439-467`, and
`jni_util.c:699-835`) and native UTF-16 enumeration on Windows
(`WinNTFileSystem_md.c:695-813`).

The accepted oracle deliberately uses only two ASCII, Windows-safe names with
different lowercase prefixes and explicitly makes no broader collation or
charset claim. Rust lexical ordering plus UTF-8 emission would match those 80
bytes while disagreeing with Bazel for case, accents, characters outside
ISO-8859-1, and malformed/native-only names. The workspace has no
Java-compatible `RuleBasedCollator`, filename transcoder, or equivalent
pinned-data dependency; its transitive ICU components do not include a
collator and cannot be presumed equal to the OpenJDK provider and data.

An oracle-bounded one-file implementation was therefore rejected as a Bazel
9 subset, despite being mechanically feasible. No Rust, fixture, harness,
Cargo, dependency, API, DICE key, Host owner, consumer, or activation
changed.

Next packet: design only
`WP-5-m1-host-registry-local-directory-collation-charset-oracle-design`.
Freeze the smallest Bazel 9.2 oracle and source matrix that discriminates
root `Collator` ordering from Rust lexical ordering, the launcher's
ISO-8859-1 output and replacement behavior from UTF-8, empty and followed
directory behavior, stable collator ties/native enumeration order, direct
special-entry names, inaccessible-list diagnostics, and the Unix/Windows
filename-decoding boundary. Include a native Windows observation/source
contract and decide there whether explicit `--host_jvm_args` overrides are
part of the supported semantic input. Do not edit fixtures, harnesses, Rust,
Cargo, dependencies, the private Host registry-file owner, a consumer, or
activation before terminal latest-text review.

### Stage 5 local registry-directory collation/charset oracle design

`WP-5-m1-host-registry-local-directory-collation-charset-oracle-design`
strengthens only the accepted `nonroot-interim-module-graph` fixture. Preserve
its first eleven rows byte-for-byte and reuse the existing `devonly@1.0.0`
local-registry closure. Do not create another fixture, copy registry/module
scaffolding, or change the oracle harness.

Pinned Bazel 9.2 source at `8220c619`,
`src/main/cpp/blaze.cc:384,418-428,456`, sets the default server locale to
ROOT and `file.encoding` to ISO-8859-1; explicit user JVM options are appended
later. Pinned OpenJDK 25.0.2 source at
`openjdk/jdk25u@405a5699ebd097464ed3fc9345414b0774a2edc9` owns the default
transport:

- `FileURLConnection.java:76-87,180-205` snapshots all direct `File.list()`
  names, stable-sorts them with `Collator.getInstance()`, appends LF after
  every name, and calls `String.getBytes()`;
- `File.java:1004-1058` distinguishes an empty array from an inaccessible
  `null` listing, and `List.java:445-507` makes the comparator sort stable;
- `Collator.java:223-340`, `CollatorProviderImpl.java:94-124`, and
  `CollationRules.java:41+` own the default ROOT `RuleBasedCollator`;
- `String.java:867-877,1025-1058,1871-1885` selects the default charset,
  preserves ISO-8859-1 characters, and emits one `?` for each unmappable BMP
  character or valid surrogate pair;
- `UnixFileSystem_md.c:104-131,296-358`, `java_props_md.c:439-467`, and
  `jni_util.c:699-835` own Unix symlink following, direct enumeration, and
  JNU filename decoding; and
- `WinNTFileSystem_md.c:219-263,406-428,695-813` owns Windows reparse-point
  following and direct UTF-16 enumeration.

Add a staged `MODULE.portable` directory whose seven live direct names are:

- four ordinary files created by row mutation: `# a`, `# B`, `# Ã©`
  (code points `U+0023 U+0020 U+00C3 U+00A9`), and `# Ā` (final code point
  `U+0100`);
- the tracked ordinary file
  `module(name = 'devonly', version = '1.0.0')`;
- the tracked real directory `# subdirectory`; and
- the tracked relative file symlink `# file-symlink`, targeting the module
  declaration filename.

Every ordinary child and the file inside `# subdirectory` contains exactly
`CHILD_CONTENT_IS_NOT_TRANSPORT\n`. Successful module evaluation and a
negative content-sentinel assertion prove that `File.list()` transports
direct names without reading child contents or inspecting the child kind.

Under Bazel's default launch, the exact listing order and bytes are:

```text
# a
# ?
# é
# B
# file-symlink
# subdirectory
module(name = 'devonly', version = '1.0.0')
```

Every displayed line is LF-terminated. `# é` denotes bytes
`23 20 c3 a9 0a`, obtained by ISO-8859-1 encoding of the Java characters
`U+00C3 U+00A9`; `U+0100` becomes `?`. The result is 91 bytes, SHA-256
`5ef3d24c7f0fd0c25515f81f6103cc4ac2c0bd21c6a76b88b6cebdd05387631e`,
and SRI `sha256-XvPSTH8P0MJVFfgfYQPMSsLAvSHGp2uIts690FOHYx4=`. Rust
lexical ordering would begin `# B`, `# a`, while UTF-8 output would preserve
`Ā` and encode `Ã©` as four bytes, so the one digest independently rejects
both shortcuts.

Append exactly three semantic rows:

1. `portable_directory_root_collation_and_latin1_bytes`. Park regular
   `MODULE.bazel` as `MODULE.regular`, promote `MODULE.portable-link`, create
   the four comment-named ordinary files, and advance root version `0.1.4` to
   `0.1.8`. Run the accepted Update-mode `mod show_repo @@devonly+` command.
   Require exit 0, the exact 91-byte SRI, no child-content sentinel or
   directory-read diagnostic, and the accepted path-free visible-lockfile
   manifest.
2. `empty_followed_directory_is_found`. Park the portable link, delete its
   four mutation-created files, delete tracked `MODULE.empty/placeholder`,
   promote `MODULE.empty-link`, and advance root version to `0.1.9`. Run the
   same `show_repo`. Require exit 37 and exact
   `the MODULE.bazel file of devonly@1.0.0 declares a different name ()`,
   excluding the registry `not found` chain and directory-read diagnostics.
   This reuses the accepted omitted-`module()` diagnostic to distinguish a
   zero-byte found value from absence; OpenJDK's exact empty digest is
   `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`
   and SRI `sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=`.
3. `regular_file_restores_after_empty_directory`. Park the empty link,
   recreate its placeholder, restore `MODULE.regular` to `MODULE.bazel`, and
   restore root version `0.1.9` to `0.1.4`. Repeat the Update `show_repo`;
   require the accepted ordinary SRI
   `sha256-rSW46GS1ppd2SDhQBqh7i7e1O4oG6NK91hvTWEjKYVQ=`, no portable-child
   or directory diagnostic, and the exact pre-packet visible-lockfile
   manifest.

All three rows use `compare = "semantic"` and
`manifest_roots = ["MODULE.bazel.lock"]`. Because semantic comparison does
not compare expected stdout implicitly, the two success rows must explicitly
assert their exact SRIs. Generation owns the final expected JSON bytes and
must confirm whether all three visible manifests remain equal after local
registry hash filtering.

The exact eight-path allowlist is `fixture.toml`, `expected/oracle.json`, and
these six additions:

- `workspace/registry/modules/devonly/1.0.0/MODULE.portable/`
  `module(name = 'devonly', version = '1.0.0')`;
- `workspace/registry/modules/devonly/1.0.0/MODULE.portable/`
  `# subdirectory/CHILD_CONTENT_IS_NOT_TRANSPORT`;
- `workspace/registry/modules/devonly/1.0.0/MODULE.portable/# file-symlink`;
- `workspace/registry/modules/devonly/1.0.0/MODULE.portable-link`;
- `workspace/registry/modules/devonly/1.0.0/MODULE.empty/placeholder`; and
- `workspace/registry/modules/devonly/1.0.0/MODULE.empty-link`.

All three symlinks are relative. The final fixture is at most 57 regular
files, five symlinks, and 1,230 newline-counted lines: net exactly three
regular files and three symlinks, with at most 450 lines from the accepted
54/2/780 baseline. The full tracked fixture tree is at most 1,306 regular
files, 19 symlinks, and 37,435 lines from the accepted
1,303/16/36,985 baseline. This is oracle packet one after checkpoint
`22de3631`; it neither reaches the +100-file/+10,000-line trigger nor resets
that baseline.

Add only `src/main/cpp/blaze.cc` to the fixture's Bazel `source_anchors`;
`IndexRegistry.java` and the other registry projection anchors are already
present. Record
`openjdk/jdk25u@405a5699ebd097464ed3fc9345414b0774a2edc9` and the exact
source matrix above in `translation_notes`, not as paths in the Bazel-source
anchor list.

The two activation links are exactly
`MODULE.portable-link -> MODULE.portable` and
`MODULE.empty-link -> MODULE.empty`. `MODULE.empty/placeholder` contains
exactly `EMPTY_DIRECTORY_PLACEHOLDER\n`, and the restoration row recreates
those exact bytes.

Two read-only Bazel 9.2 `repository_ctx.download` probes independently pinned
the same seven-name directory. The default launch produced the 91-byte digest
above. `--host_jvm_args=-Dfile.encoding=UTF-8` preserved order but produced
94 bytes, SHA-256
`811bc9d8448f1687c3bc3a02330f4a735b31f1c24727da3d3d0104cfad870865`,
and SRI `sha256-gRvJ2ESPFofDvDoCMw9Kc1sx8cJHJ9o9PQEEz62HCGU=`.
Explicit `--host_jvm_args` is a supported Bazel semantic input: Bazel appends
it last, accepts it, and it observably changes registry bytes. It is
intentionally deferred from this default-launch oracle and immediate Registry
IO bridge packet because Slug does not yet own typed startup-property
identity. Record the named blocker
`WP-5-m1-host-jvm-registry-byte-input-design`, which must freeze ordered
last-wins charset/collator projection plus daemon identity, restart, and
invalidation before the bridge can claim full parity or activate a consumer.
The default bridge must remain parameterizable and must not silently accept,
ignore, or infer the startup option.

Do not add nondiscriminating locale rows. The pinned embedded JRE exposes
only `und`, `en`, and `en-US` collator locales and no `jdk.localedata`;
ROOT/en/sv/tr/da probes used identical rules, strength, decomposition, and
listing bytes. A `user.extensions=u-ks-level1` override can instead change
collator strength and expose filesystem-order-dependent stable ties, which is
another reason startup overrides require a separate owner rather than a
golden row.

Keep nonportable behavior in the source/native matrix rather than widening
the generic fixture DSL:

- stable collator-equal names retain their native `File.list()` order; later
  Rust unit evidence must inject and preserve both input orders rather than
  commit a filesystem-order-dependent golden;
- under UTF-8 JNU decoding, Unix filename suffix bytes `ED A0 80` become one
  Java replacement character and then one ISO-8859-1 `?`, so
  `# invalid-<ED A0 80>` yields exact bytes
  `23 20 69 6e 76 61 6c 69 64 2d 3f 0a`; later Unix-native evidence must not
  use Rust's three-replacement `from_utf8_lossy` result;
- inaccessible directory listing raises
  `<path> exists, but is not accessible`, which Bazel projects to the same
  registry-not-found surface as absence, so no privilege-dependent retained
  row can discriminate it;
- `File.list()` source includes dangling/directory symlink and Unix
  FIFO/socket names without reading their targets; the retained real
  directory and file symlink are the smallest byte-discriminating kind set;
  and
- before the bridge implementation design can claim native Windows
  completeness, require a real-directory Bazel/OpenJDK probe of the portable
  seven-name and empty sets plus a `CreateFileW` lone-surrogate case. The
  pinned WinNT source above is the current contract; GNU-Windows compilation
  is not runtime evidence.

Run one pinned Bazel 9.2 generation and two absolute distinct-root replays,
then stop every used server. Validate the exact eight-path diff, three regular
files and three relative symlinks, dynamic-name create/delete lifecycle,
seven names/types/content sentinels, 91/80/44/zero-byte hashes and SRIs, exact
fourteen-row order, exits `0/37/0`, arguments/mutations/diagnostics, explicit
success SRIs, all three manifests and equality groups, metadata/source
anchors, parser plus focused packet-validator tests, inventory/line caps,
schema, normalization, archive status, credentials, host-path-free normalized
fields, and server cleanup.

Stop on a copied/new fixture, a harness change, tracked exotic or invalid-byte
filename, filesystem-order-dependent golden, a fourth row, a ninth changed
path, a fourth regular file or symlink, cap overflow, missing Windows source
contract/future native gate, startup-override row or startup-owner
implementation, locale golden, direct
Rust/Cargo/dependency/API/DICE/Host-owner/consumer/activation edit, or changed
first-eleven-row evidence. Terminal acceptance requires source/parity,
implementation/evidence, and architecture/orchestration latest-text review.

#### Local registry-directory collation/charset oracle design status

Status: `ACCEPT` after terminal latest-text review on 2026-07-26.

The accepted default-launch design reuses the eleven-row
`nonroot-interim-module-graph` fixture and adds exactly three rows, three
regular assets, and three relative symlinks. Its 91-byte successful listing
jointly discriminates ROOT `RuleBasedCollator` order, ISO-8859-1
preservation/replacement, direct real-directory and file-symlink names, and
child-name rather than child-content transport. A followed empty directory
then proves a found zero-byte value before exact ordinary-file restoration.

Two distinct-output-base Bazel 9.2 probes pinned the 91-byte default and
94-byte UTF-8-override digests. The override is supported Bazel semantic
input but remains outside this default-only oracle behind named blocker
`WP-5-m1-host-jvm-registry-byte-input-design`; no full-parity or consumer
activation claim may bypass its ordered input and daemon lifecycle design.
Stable ties, Unix malformed names, inaccessible listing, and extra special
kinds remain pinned source/native or later injected-unit evidence rather than
nondeterministic fixture goldens. Native Windows portable, empty, and
lone-surrogate observations remain required before a bridge implementation
design can claim Windows completeness.

No fixture, harness, Rust, Cargo, dependency, API, DICE key, Host owner,
consumer, or activation changed. Source/parity, implementation/evidence, and
architecture/orchestration terminal latest-text reviews all returned
`ACCEPT`.

Next packet: implement only
`WP-5-m1-host-registry-local-directory-collation-charset-oracle` inside the
exact contract above. After its acceptance, design the named typed
startup-property blocker before retrying the runtime bridge correction.

#### Local registry-directory collation/charset oracle implementation status

Status: `ACCEPT` after terminal latest-diff review on 2026-07-26.

The exact eight-path implementation adds three semantic rows, three regular
assets, and three relative symlinks to the retained fixture. Pinned Bazel 9.2
generation and two absolute distinct-root replays passed with exits
`0/37/0`, identical 1,228-byte visible-lockfile manifests, the exact
91-byte ROOT/ISO-8859-1 SRI, the found-empty name-mismatch diagnostic, and
ordinary-file restoration. All packet output-base servers were shut down.

The first eleven expected records remain byte-identical. The fixture is
exactly 57 regular files, five symlinks, and 1,112 newline-counted lines;
the full tracked fixture tree is 1,306/19/37,317, making this packet one
after checkpoint `22de3631`. The parser and 42 focused harness tests, exact
scope/topology/bytes/hash/manifest/normalization/credential/server gates, and
`git diff --check` passed. The fixture records the full pinned OpenJDK
file-and-line matrix, including the verified
`sun/util/locale/provider/CollationRules.java` path. Source/parity,
implementation/evidence, and architecture/orchestration terminal
latest-diff reviews all returned `ACCEPT`.

Next packet: design only `WP-5-m1-host-jvm-registry-byte-input-design`.
Freeze ordered last-wins startup-property identity plus daemon
restart/invalidation without weakening the still-required native-Windows
portable, empty, and lone-surrogate observation gate. Do not retry the
runtime bridge or activate a consumer first.

### Host JVM registry byte-input design first attempt

Status: `REPLAN` after terminal latest-text review on 2026-07-26; no fixture,
harness, or Rust edit started.

`WP-5-m1-host-jvm-registry-byte-input-design` cannot freeze the requested
single ordered startup-property identity. Pinned Bazel 9.2 source at
`8220c619` has two intentionally different representations:

- `src/main/cpp/option_processor.cc:80-171,374-465,603-655`,
  `src/main/cpp/rc_file.cc:85-135,192-248`, and
  `src/main/cpp/startup_options.cc:205-286,407-441` preserve every
  `--host_jvm_args` occurrence. Sources are ordered system, workspace, home,
  comma-ordered `BAZELRC`, and occurrence-ordered explicit `--bazelrc`;
  generic `startup` entries across those files precede all matching
  `startup:<platform>` entries, and explicit CLI occurrences are last.
  Both `--host_jvm_args=X` and the separated unary form canonicalize to the
  same stored occurrence.
- `src/main/cpp/blaze.cc:360-465` emits fixed
  `file.encoding=ISO-8859-1` and empty country/language/variant properties
  before the ordered user occurrences. Pinned OpenJDK
  `openjdk/jdk25u@405a5699ebd097464ed3fc9345414b0774a2edc9`,
  `src/hotspot/share/runtime/arguments.cpp:1250-1326,1962-1981,`
  `2136-2156,2488-2515,3993-4015`, applies duplicate writable `-D`
  properties in order and retains the last value when a fresh JVM starts.
- Bazel emits those occurrences again in canonical server arguments at
  `blaze.cc:568-583,660-665,949-952`, but
  `blaze.cc:978-1067` compares nonvolatile canonical arguments as an
  order-insensitive multiset that retains occurrence counts.
  `blaze.cc:1072-1122,1530-1579` restarts only when that multiset differs.
  Source origin is diagnostic and `--option_sources=` is volatile.
- Independently, `blaze.cc:1987-1993` copies every current request occurrence
  and its source into the `RunRequest`; pinned line 1990 also appends a second,
  empty `StartupOption` for every logical occurrence.
  `src/main/java/com/google/devtools/build/lib/server/GrpcServerImpl.java:568-574,607-619`,
  `src/main/java/com/google/devtools/build/lib/runtime/BlazeCommandDispatcher.java:743-752`,
  and
  `src/main/java/com/google/devtools/build/lib/runtime/CommandLineEvent.java:269-284,317-324`
  preserve all populated and synthetic records internally, then emit only
  empty-source records through the original structured-command-line BEP
  startup-options section. Every synthetic record is therefore emitted as an
  empty `combinedForm`; a populated CLI record is emitted because CLI source
  is empty, while a populated RC-sourced record is filtered from this section.
  `BlazeCommandDispatcher.java:547-580` separately exposes populated RC
  options through `--announce_rc`, skipping every empty-source record and
  grouping consecutive populated options by source into one
  `Reading 'startup' options from <source>: <joined options>` info event.
  After reorder-only reuse, diagnostics can therefore expose the new request
  order while the live daemon retains its original launch order and byte
  semantics.

Two isolated Bazel 9.2 observations confirmed that split. An `A,B` request
followed by `B,A` retained PID `3841796`; replacing one occurrence restarted
as PID `3841958`. A separate conflicting
UTF-8-then-ISO-8859-1 launch retained its ISO-8859-1 bytes and PID after the
requested order reversed on the same output base, while the reversed order
on a fresh output base launched a new PID and produced UTF-8 bytes. Every
probe server was shut down and verified dead.

Therefore a pure reorder must reuse the old daemon, old effective byte
semantics, and old DICE graph. Recomputing and injecting the newly requested
last-wins value would disagree with Bazel. Adding, removing, or changing one
occurrence must complete shutdown before a fresh daemon, fresh effective
semantics, and fresh DICE graph can receive the command. There is no
same-graph startup-property invalidation transition.

The provisional closed property projection is also not an acceptable parity
boundary. OpenJDK source shows that direct registry-directory bytes can
depend on `file.encoding` including `COMPAT` and invalid-name fallback;
categoryless `user.language`, `user.script`, `user.country`, `user.variant`,
`user.extensions`, and legacy-dominant `user.region`;
`java.locale.useOldISOCodes`; `java.locale.providers`; embedded runtime
provider data; native filename decoding; and native enumeration order for
collator ties. Category-specific display/format properties do not feed
`Collator.getInstance()` in this path. Bazel additionally accepts arbitrary
JVM agents, module/classpath changes, `--extra_classpath`, and alternate
`--server_javabase` inputs that can alter providers or Java behavior. Silently
ignoring them is wrong, while a Slug-only unsupported-input error would make
Slug a forbidden Bazel-success subset.

Bazel sanitizes `_JAVA_OPTIONS`, `JDK_JAVA_OPTIONS`, and
`JAVA_TOOL_OPTIONS` and selects server `LC_ALL` in
`blaze.cc:1305-1382`; those launch-environment rules, `file.encoding=COMPAT`,
the platform-owned non-overridable `native.encoding`/`sun.jnu.encoding`, and
the exact embedded or explicit Java runtime are part of the boundary. A
finite Rust property fold may not claim completeness unless it proves the
entire accepted JVM/provider/data surface. A pinned OpenJDK lifetime helper
is only a candidate until its packaging, exact launch ordering, arbitrary JVM
argument behavior, provider/classpath/module behavior, failure diagnostics,
protocol, and Unix/Windows lifecycle are reviewed.

The eventual ownership split must keep at least these values distinct:

1. the current request's ordered startup occurrence stream with diagnostic
   source provenance;
2. the ordered occurrence stream that actually launched the retained daemon;
3. the occurrence-counted, order-insensitive canonical server-reuse identity;
4. the effective registry-directory semantics retained by the daemon that
   actually launched; and
5. a typed semantic DICE input on which directory-byte production depends.

The fifth value is installed in the graph-construction transaction before
any consumer can compute. It is never a process global, ambient lookup,
`DiceDataBuilder` capability value, or per-build/per-query
`BzlmodRequestInputs` field. A multiset mismatch discards the whole graph; an
equal multiset performs no reinjection. `RegistryIo` remains the IO
capability, not the semantic owner.

Live Slug has prerequisites that prevent a direct owner packet:

- `slug_cli_v2/src/commands/mod.rs` recognizes only `--output_base`, scans it
  beyond the startup prefix, and discards other prefix flags;
- build and query independently treat socket reachability as sufficient
  daemon identity;
- daemon start removes a socket without an output-base client lock, writes an
  unauthenticated PID before readiness, and shutdown is fire-and-forget;
- the server protocol has no tagged status/identity handshake; and
- `WorkspaceRuntime::new` installs one registry IO capability and one DICE
  graph for the daemon lifetime.

The lifecycle owner must serialize identity comparison, shutdown, completed
termination, startup, readiness, and command submission under an
output-base client lock. A tagged status response must authenticate the live
daemon and return its actual retained identity; a competing client must never
unlink or replace a live socket. A build/query wire request may carry a
primitive copy of the current ordered occurrences and source strings for
Bazel-shaped diagnostics only. That request-local copy must never replace the
retained launched stream, drive registry semantics, or become a DICE
injection.

Next packet: design only
`WP-5-m1-host-jvm-registry-byte-input-oracle-boundary-correction`.
It must freeze:

- the exact source/native matrix for startup-property grammar, RC generic /
  platform / CLI ordering, fixed-default precedence, `COMPAT`, invalid
  charset fallback, locale/region/extensions/provider behavior, arbitrary
  valid and JVM-invalid arguments, explicit Java/classpath inputs, sanitized
  environment, and exact JVM-start diagnostics;
- the smallest oracle-harness startup-argument seam and retained-fixture rows
  proving default 91-byte, UTF-8 94-byte, fresh conflicting last-wins,
  same-multiset reorder retaining old bytes, occurrence-change restart, and
  exact default restoration without copying registry scaffolding;
- a source/native or protocol discriminator showing reorder-only request
  diagnostics change while PID, directory bytes, retained launch semantics,
  and graph identity remain unchanged, using explicit CLI occurrences to pin
  Bazel 9.2's exact populated plus synthetic-empty startup-option BEP records;
  RC occurrences must pin their synthetic empty BEP records plus the
  source-grouped populated `--announce_rc` info events;
- whether exact parity uses a pinned OpenJDK lifetime helper or another
  reviewed mechanism, with no ignore/reject subset;
- the full canonical server identity plus locked, authenticated,
  shutdown-complete lifecycle boundary;
- graph-construction ownership and dependency for the semantic DICE value;
  and
- the still-required real-Windows portable seven-name, empty-directory, and
  `CreateFileW` lone-surrogate observations plus the native server transport
  decision.

Do not edit a fixture, harness, Rust, Cargo, dependency, API, DICE key,
runtime bridge, private Host registry-file owner, consumer, or activation
before terminal latest-text review. Stop on ordered-vector daemon equality;
set/deduplicated identity; origin-sensitive restart; new semantics after a
reorder-only request; a multiset change reaching the old daemon; request-local
startup injection; capability-only semantic ownership; missing client
serialization or shutdown completion; silent ignore or Slug-only rejection
of a Bazel-valid JVM input; a claimed `sun.jnu.encoding` override; a copied
fixture; or any bridge/completeness claim before native Windows evidence.

### Host JVM registry byte-input oracle-boundary correction

Status: `REPLAN` after terminal latest-text review on 2026-07-26, before
harness or Rust. The independent startup/reuse oracle below is bounded, but no
exact standalone JVM executor exists for the full Bazel-accepted surface
frozen by the preceding packet.

#### Corrected source and ownership boundary

Pinned Bazel 9.2 preserves startup options only before the command. Both
`--host_jvm_args=X` and its separated unary form become one append-only
logical occurrence; a missing value is `BAD_ARGV`. RC discovery is system,
workspace, home, comma-ordered `BAZELRC`, then occurrence-ordered explicit
`--bazelrc`. Top-level discovered candidates are canonical-path deduplicated
before parsing; imports expand in place, and a repeated import warns rather
than entering a global deduplication set. Parsing concatenates every generic
`startup` entry, every matching `startup:<platform>` entry, then explicit CLI
occurrences.

`host_jvm_args` remains ordered and append-only. `server_javabase` and
`extra_classpath` are instead last-wins scalars, so removing an overwritten
occurrence can change request diagnostics without changing the launched JVM
or reuse identity. Bazel's fixed JVM option prefix and defaults precede every
user argument; `host_jvm_debug` also precedes the user stream. The launcher
main/JAR/classpath suffix follows the user stream. `extra_classpath` replaces
the ordinary `-jar <server.jar>` suffix with Bazel's exact
`-cp <server.jar>:<extra> ...Bazel` suffix.

Daemon reuse is not a property projection. It first requires equal raw
canonical server-argument lengths and then compares the complete nonvolatile
canonical arguments as occurrence-counted, order-insensitive multisets.
Binary/install compatibility is an earlier gate. File-backed
arguments such as an agent JAR, argument file, provider/classpath JAR, native
agent, or explicit JDK are identified by their canonical argument/path, not
file content; modifying one in place does not itself force a restart.

The implementation must retain five separate values:

1. current request startup occurrences and diagnostic source strings;
2. the exact ordered argv and sanitized environment that launched the daemon;
3. the complete canonical reuse vector and exact comparator;
4. the retained JVM executor and its effective byte semantics; and
5. a graph-lifetime typed semantic DICE input paired to that executor instance.

Request source provenance is nonsemantic. Equal canonical identity never
replaces launched order, executor state, or the DICE input. A mismatch destroys
the daemon, executor, and graph before constructing their replacements.
`StrongHash` may authenticate a transport record but may not replace complete
vector equality.

On Linux Bazel selects the first installed locale among
`en_US.ISO-8859-1`, `C.UTF-8`, and `en_US.UTF-8`, otherwise retaining the
inherited locale. It removes `_JAVA_OPTIONS`, `JDK_JAVA_OPTIONS`, and
`JAVA_TOOL_OPTIONS` with warnings. Failed JVM startup occurs before a
`RunRequest`: RC startup provenance, the client crash header, and `jvm.out`
form the diagnostic boundary. An invalid explicit `server_javabase` can fail
earlier in the client sanity check.

Pinned OpenJDK `jdk25u@405a5699` further fixes these boundaries:

- `SystemProps.java:77-89` makes `native.encoding` and `sun.jnu.encoding`
  non-overridable and expands `file.encoding=COMPAT` to native encoding;
- `Charset.java:642-653` uses the standard provider for the default charset
  and falls back to UTF-8 when the configured name is unrecognized;
- `StaticProperty.java:95-124`, `Locale.java:1151-1186`, and
  `BaseLocale.java:103-184` own categoryless locale, `user.region`
  dominance, extensions, and old ISO-code behavior;
- `LocaleProviderAdapter.java:116-164,235-289` and
  `SPILocaleProviderAdapter.java:69-145,200-223` own locale-provider ordering
  and system-classloader SPI behavior; and
- every `-javaagent` initializes before the server main, enters the system
  class path, and may transform classes or change process state.

Successful request diagnostics remain nonsemantic primitive data. All
populated and synthetic records reach Java. Original structured-command-line
BEP emits only empty-source entries, while `--announce_rc` skips empty-source
records and groups consecutive populated RC options by source. Neither
surface can own launch order, reuse identity, executor state, or DICE.

#### Accepted independent oracle sub-boundary

Next packet: implement only
`WP-5-m1-host-jvm-registry-startup-reuse-oracle`.

The next implementation packet may change only:

- `tools/v2_oracle_lib/fixture.py`;
- `tools/v2_oracle_lib/runner.py`;
- `tools/v2_oracle_lib/compare.py`;
- `tests/v2_oracle/test_v2_oracle.py`;
- `tests/v2_oracle/fixtures/nonroot-interim-module-graph/fixture.toml`;
- its generated `expected/oracle.json`; and
- one nonregistry
  `workspace/startup-diagnostics.bazelrc`.

Add no registry, module, provider, agent, JVM, or Java asset and no symlink.
The exact cap is seven paths, one regular file, zero links, and 1,800 net
added lines; the fixture inventory may not exceed 58 regular files, five
links, or 2,250 lines. This is post-checkpoint oracle packet two after
`22de3631`; it does not trigger the next five-packet review.

Add an optional fixture-wide ordered `startup_argv` baseline and fixture-wide
environment overrides, then these optional per-command fields:

- ordered `startup_argv`, appended after the fixture baseline;
- `capture_server_epoch`; and
- `capture_startup_diagnostics`.

Emit the harness-owned `--output_base=<path>`, fixture baseline, command
startup argv, then the command. Apply fixture environment first and command
overrides second. Record both startup vectors separately. Do not infer startup
options from command argv. For every daemon command, including the existing
first fourteen, observe the live server identity even when the command does
not serialize it. For Bazel use PID plus `server.starttime` when present; for
Slug the eventual oracle must use authenticated Status and its instance token,
never a PID file. Map each first-seen live identity to a run-local ordinal.
Therefore the existing default daemon is epoch 1 and the first host-JVM
mismatch is epoch 2. Compare only records that declare
`capture_server_epoch`.

For diagnostic capture, the runner supplies one unique absolute
`--build_event_json_file=<run_dir>/startup-bep/<command-index>.json`.
Require exactly one `structuredCommandLine` event labelled `original` and one
`startup options` section. Extract only the ordered
`optionList.option[].combinedForm` values, preserving every duplicate and
empty string without sorting, deduplication, or filtering. Normalize only the
existing workspace, run, and output-base path replacements plus slash style.
Do not store or compare invocation IDs, timestamps, unrelated BEP events, or
the raw file. Extract complete ordered `--announce_rc` messages only in the
exact form
`Reading 'startup' options from <normalized source>: <comma-space-joined options>`.
The comparator checks the generated oracle's declared epoch, combined-form
list, and announcement list in addition to existing semantic fields.

The runner must shut both tools down in `finally`. Bazel cleanup invokes
`<bazel> --output_base=<path>`, the same fixture-wide RC-suppression startup
baseline, then `shutdown`, under the same fixture-wide empty/unset `BAZELRC`
environment and without user host arguments. It requires exit zero and waits
for every observed PID/starttime epoch to be dead. Slug cleanup eventually
uses authenticated shutdown, a termination-expected acknowledgement, and
verified process death; delete the current fire-and-forget socket/PID unlink
behavior rather than copying it into the harness. A remaining process or
reachable endpoint fails the run.

Reuse the existing `MODULE.portable`, empty directory, ordinary file, and
four dynamic names. Existing rows already pin default 91-byte/SRI
`sha256-XvPSTH8P0MJVFfgfYQPMSsLAvSHGp2uIts690FOHYx4=`, found-empty, and
ordinary restoration. The fixture-wide startup baseline supplies
`--nosystem_rc`, `--noworkspace_rc`, and `--nohome_rc`, and its environment
sets `BAZELRC` empty for all nineteen commands and cleanup. Thus epoch 1 and
the existing default 91-byte row are hermetic too. Row 3 alone additionally
supplies the explicit fixture RC. Append exactly five rows:

1. `fresh_conflicting_last_wins_utf8` reactivates the same portable directory
   and dynamic names. With hermetic automatic RC discovery disabled, CLI
   occurrences ISO-8859-1 then UTF-8 must restart to epoch 2 and produce
   94-byte SRI `sha256-gRvJ2ESPFofDvDoCMw9Kc1sx8cJHJ9o9PQEEz62HCGU=`,
   with exact populated/synthetic-empty BEP ordering.
2. `reordered_same_multiset_retains_utf8` requests UTF-8 then ISO-8859-1.
   It must retain epoch 2, the 94-byte result, and the retained launch/graph
   identity while its current CLI BEP pairs appear in the requested order.
3. `rc_source_change_same_multiset_reuses` supplies ISO-8859-1 in generic
   `startup` and UTF-8 in matching `startup:<platform>` through the explicit
   hermetic RC and includes command option `--announce_rc`. It must retain
   epoch 2 and 94 bytes while pinning RC synthetic-empty BEP entries plus one
   source-grouped populated `--announce_rc` message.
4. `occurrence_change_restarts_latin1` uses one CLI ISO-8859-1 occurrence.
   It must restart to epoch 3 and produce the exact 91-byte result.
5. `zero_occurrences_restores_default` disables RC discovery and supplies no
   host occurrence. It must restart to epoch 4 and restore the exact default
   91-byte result.

No mutation follows the initial portable reactivation. Fresh graphs cover
restart rows; unchanged workspace state makes the two equal-identity rows
discriminating. Bazel PID/source evidence does not prove Slug graph identity.
Later authenticated lifecycle tests must separately compare graph-instance
diagnostics and runtime factory construction counts. The graph instance is
diagnostic, not the semantic input; the local-directory byte key must directly
depend on the graph-construction input.

Parser, fixture/command startup merge and argv placement, fixture/command
environment precedence, BEP cardinality/filtering, RC grouping, identity
ordinalization, missing/stale identity, hermetic shutdown
success/failure/timeout, and path normalization all receive focused harness
tests before generation. Run one pinned Bazel 9.2 generation, two absolute
distinct-root replays, the focused harness/fixture suite, exact
scope/growth/hash/provenance checks, and terminal latest-diff reviews. Stop on
any copied registry scaffold, raw PID or BEP nondeterminism, inferred graph
identity, unverified shutdown, or harness support broader than the declared
fields.

#### Unresolved exact execution boundary

A long-lived OpenJDK helper would have the right lifetime for ordinary
properties, locale/provider inputs, explicit javabase, extra classpath,
sanitized environment, and directory `URLConnection`. JNI is not equivalent
to launcher execution. More importantly, a helper changes Bazel's fixed
main/JAR/classpath suffix. A Bazel-valid agent can inspect
`sun.java.command`, the Bazel main class/resources/classpath, or Bazel server
classes before changing locale or `FileURLConnection` behavior. Argument
files, module/classpath options, native agents, and an in-place-modified
explicit JDK can also depend on the exact launcher process.

Consequently a standalone helper has demonstrable
Bazel-success/helper-different inputs and violates the no-ignore/no-reject
gate. A sidecar helper workspace is still distinguishable. The only general
exact process found is the original Bazel 9.2 JVM executing the original
command, workspace, classpath, and main; that means delegating the original
command to Bazel, not implementing Slug's registry byte owner. Do not accept a
Rust fold, whitelist, silent ignore, fallback, or custom helper.

After the oracle, run design-only
`WP-5-m1-host-jvm-registry-byte-execution-feasibility`. It must add no fixture
or production code. Freeze an adversarial source/native matrix containing:

- invalid-name UTF-8 fallback and `COMPAT` against captured
  `native.encoding`;
- all three sanitized Java-option variables and exact warnings;
- unknown HotSpot and invalid explicit-javabase diagnostics;
- last-wins scalar diagnostic-versus-identity behavior;
- a deterministic SPI provider/classpath collator change;
- an agent whose `premain` branches on `sun.java.command` or Bazel-only
  classes; and
- pinned minimized-JDK artifacts/vendor patches, explicit-JDK compatibility,
  same-path provider/agent mutation, and startup-failure provenance.

Return `REPLAN` on any Bazel-success/candidate-failure or behavior difference.
No CLI, server, transport, parser, canonicalizer, helper, DICE, directory IO,
consumer, or activation work may begin without an exact mechanism and a new
terminally reviewed packet.

The later accepted sequence, if feasibility succeeds, is: real-Windows
portable/empty/lone-surrogate and transport evidence; dormant full startup
parser/canonical-vector/comparator; authenticated cross-platform loopback
transport plus output-base lifecycle; dormant executor plus graph-construction
semantic input; DICE-owned external-directory observation and byte dependency;
then one atomic daemon/one-shot/Host-consumer activation.

The lifecycle contract remains frozen for that later sequence: an `fs4`
output-base lock precedes metadata inspection; ACL-protected atomically
published server info carries protocol tag, loopback address, request/response
cookies, instance nonce, PID/start token, actual retained identity, and
executor/graph instance diagnostics. Equal identity holds the lock through
authenticated Run stream creation/submission, then releases before response
completion. Mismatch requires authenticated shutdown,
termination-expected acknowledgement, and verified exact-process death before
restart. Status remains responsive while a dedicated worker serializes
commands; no lock is held across DICE computation. Never unlink, kill, or
replace from reachability alone.

### Host JVM startup/reuse oracle pre-implementation stop

Status: `REPLAN` after terminal latest-text review on 2026-07-26, before
retained harness or fixture edits.

The seven-path
`WP-5-m1-host-jvm-registry-startup-reuse-oracle` stopped at its preflight
gate. Pinned Bazel 9.2 accepts `--build_event_json_file` on `mod graph` and
`mod show_repo`. `ModCommand.java:107-136` posts `NoBuildEvent`, but that alone
does not disable BEP. The decisive boundary is
`BazelBuildEventServiceModule.java:209-223`, which omits `mod` from
`ALLOWED_COMMANDS`; `BuildEventServiceModule.java:388-424` therefore returns
before `createBepTransports`, and its JSON-file creation path at lines 839-945
is never reached. A native `mod graph` probe exited zero without creating the
requested file. Therefore none of the five planned semantic `mod show_repo`
rows can satisfy the same-command requirement for exactly one original
structured-command-line event.

A separate pinned `build //...` probe did create exactly one
`structuredCommandLine` event labelled `original` with exactly one
`startup options` section. Its synthetic empty `StartupOption` values encoded
as empty JSON objects with absent `combinedForm`, not explicit JSON empty
strings. The observed ordered CLI projection was output base, synthetic empty,
each hermetic RC-suppression option followed by synthetic empty, then each
host-JVM occurrence followed by synthetic empty. Any eventual extractor must
map absent `combinedForm` to the semantic empty string while rejecting other
schema/cardinality drift. The populated/synthetic source chain is pinned at
`blaze.cc:2069-2075` and `CommandLineEvent.java:280-300`; the live JSON
observation pins the absent-field encoding.

Preflight also found that the packet cannot both forbid server/transport edits
and replace Slug's unauthenticated fire-and-forget cleanup. Authenticated
Status, instance tokens, termination acknowledgement, and verified Slug
process identity do not exist yet. The bounded oracle may implement exact
Bazel cleanup only. It must preserve the legacy Slug path without claiming it
as accepted lifecycle evidence, and the new fixture must fail closed as
unsupported for Slug epoch capture until the later transport packet. Epoch
observation must be gated to an opted-in fixture: observe every command in
this fixture to establish epoch 1, but do not broaden observation to the other
`daemon = true` fixtures.

The writer's temporary failing-test draft was fully removed. At the stop,
`git status --short`, `git diff --check`, and `git diff --name-only` were
empty at `571db092`; no test, generation, expected file, fixture inventory, or
production code changed, and every probe server was shut down and verified
dead.

Next packet: design only
`WP-5-m1-host-jvm-registry-startup-reuse-oracle-command-shape-correction`.
Before any edit, freeze:

- the smallest explicit `build //...` diagnostic companion commands that
  preserve the five semantic `mod show_repo` transitions, exact startup argv,
  epoch, and graph identity without hiding a second tool invocation inside one
  fixture command;
- the corrected total command/row count, exact CLI and RC combined-form lists,
  absent-field-to-empty projection, source-grouped `--announce_rc`, expected
  manifests, scope, growth, and line caps;
- fixture-opt-in epoch observation across every command in this fixture only;
- exact Bazel-only shutdown, primary-error preservation, exit/death/endpoint
  verification, and focused failure tests; and
- an explicit no-claim boundary for Slug cleanup/Status until authenticated
  transport exists.

Do not edit a harness, fixture, expected file, RC, Rust, Cargo, dependency,
API, DICE key, runtime, CLI/server, private Host owner, consumer, or activation
before terminal latest-text review. Stop on a hidden paired invocation,
same-command BEP claim for `mod`, explicit-empty-only JSON parser, global
daemon observation, PID-only Slug evidence, legacy Slug cleanup acceptance,
lost primary failure, or any production work.

### Host JVM startup/reuse oracle command-shape correction

Status: `ACCEPT` after terminal latest-text review on 2026-07-26, before
harness or fixture edits.

Pinned Bazel 9.2 `build //...` is not an acceptable diagnostic companion in
this fixture. It emits BEP but exits 1 while loading the intentionally minimal
embedded `@@bazel_tools//tools` closure because `@@platforms//host` is absent;
making it succeed would add unrelated registry scaffold. A live
`query //:BUILD.bazel` companion is smaller and discriminating:

- `BazelBuildEventServiceModule.java:209-223` allows `query`;
- `QueryEnvironmentBasedCommand.java:75-101` posts its no-build events, and
  `BuildEventServiceModule.java:388-450,839-945` creates the requested
  transport and JSON file;
- with `--lockfile_mode=off`,
  `--registry=file://%workspace%/registry`, `--output=label`, and
  `--noshow_progress`, it exits 0 with exact stdout `//:BUILD.bazel\n`;
- it emits exactly one original structured-command-line event and one startup
  section, retains the preceding PID/starttime epoch, and leaves a pre/post
  digest of every regular workspace file unchanged.

The corrected packet retains the same seven paths:

- `tools/v2_oracle_lib/fixture.py`;
- `tools/v2_oracle_lib/runner.py`;
- `tools/v2_oracle_lib/compare.py`;
- `tests/v2_oracle/test_v2_oracle.py`;
- `tests/v2_oracle/fixtures/nonroot-interim-module-graph/fixture.toml`;
- its generated `expected/oracle.json`; and
- one nonregistry
  `workspace/startup-diagnostics.bazelrc`.

The exact caps remain seven paths, one added regular file, zero added links,
and at most 1,800 net lines. The resulting fixture may not exceed 58 regular
files, five links, or 2,250 lines from the verified 57/5/1,112 baseline. This
remains post-checkpoint oracle packet two after `22de3631`.

#### Harness contract

Add strict optional fixture-wide ordered `startup_argv`, fixture-wide string
environment overrides, and `observe_server_epochs`, permitted only for
`daemon = true`. Add strict optional per-command ordered `startup_argv`,
`capture_server_epoch`, and `capture_startup_diagnostics`. On the opted-in
daemon path, emit exactly one subprocess per declared fixture row in this
order:

`<tool> --output_base=<path> <fixture startup> <command startup> <command argv>`.

Apply fixture environment before command overrides. Record the fixture and
command startup vectors and environment maps separately. The only runner
injection is one unique absolute
`--build_event_json_file=<run_dir>/startup-bep/<command-index>.json` for a
command declaring diagnostic capture, appended after its declared command
argv. Never infer startup options from command argv and never hide a companion
subprocess inside another row.

When `observe_server_epochs` is true, Bazel observes PID,
`server/server.starttime`, and `server/command_port` after every command in
the fixture. Parse the endpoint only as IPv4 `address:port` or bracketed IPv6
`[address]:port`; require a loopback address, nonzero port, and one stable
endpoint for each live PID/starttime identity. Validate the live identity and
assign first-seen tuples stable run-local ordinals, retaining each epoch's
endpoint internally for cleanup. Only rows declaring `capture_server_epoch`
serialize and compare the ordinal. Do not enable observation for another
daemon fixture. A Slug run of an opted-in fixture fails closed before
workspace copy or process launch because authenticated Status does not exist;
there is no PID fallback. `CommandServer.java:226-230,343-423` pins command
port publication, formatting, and deletion.

For each diagnostic row, require one JSON file, exactly one
`structuredCommandLine` event whose label is `original`, and exactly one
`startup options` section. Extract only ordered
`optionList.option[].combinedForm`. Map an absent field in the observed empty
JSON object to the semantic empty string; preserve explicit values, duplicates,
and ordering. Reject nonobject entries, nonstring present fields, extra
original events/sections, missing files, and all other cardinality/schema
drift. Normalize only existing workspace, run, and output-base path
replacements plus slash style; do not apply timing/UUID text normalization or
store raw BEP, invocation IDs, timestamps, or unrelated events.

Extract complete ordered RC messages in the exact normalized form
`Reading 'startup' options from <source>: <comma-space-joined options>`.
Do not treat the client prefix, unrelated INFO lines, or partial text as a
record. The generated comparator checks declared server epoch,
combined-form list, and announcement list in addition to existing semantic
fields. The source chain is pinned by `option_processor.cc:479-531`,
`startup_options.cc:414-445`,
`blaze.cc:2069-2075`, `CommandLineEvent.java:280-300`, and
`BlazeCommandDispatcher.java:588-620`.

Exact cleanup is Bazel-only and only for the opted-in fixture. In `finally`,
invoke `<bazel> --output_base=<path> <fixture RC-suppression startup> shutdown`
under the fixture-wide empty `BAZELRC` environment and without any row
host-JVM arguments, with `cwd=<workspace>`. Require exit zero; verify every
observed PID/starttime identity dead and every retained loopback endpoint
unreachable. Identity/death behavior is pinned at `blaze_util.cc:47` and
`blaze_util_linux.cc:189-225`. If execution and cleanup both fail, re-raise the
original execution exception as terminal with the cleanup failure chained and
visible. Preserve the existing Slug cleanup path only for non-opted fixtures
and make no acceptance claim about it.

#### Exact 22-command sequence

All twenty-two commands use fixture startup baseline
`--nosystem_rc`, `--noworkspace_rc`, and `--nohome_rc` plus fixture environment
`BAZELRC=""`. The existing first fourteen commands establish hermetic epoch 1.
Append five semantic `mod show_repo` rows and exactly three visible `query`
diagnostic companions:

15. `fresh_conflicting_last_wins_utf8` reactivates the accepted portable
    directory and four dynamic names, mutates root version `0.1.4` to `0.1.8`,
    then requests CLI ISO-8859-1 followed by UTF-8. It produces 94-byte SRI
    `sha256-gRvJ2ESPFofDvDoCMw9Kc1sx8cJHJ9o9PQEEz62HCGU=` at epoch 2.
16. `fresh_conflicting_cli_startup_diagnostics` uses identical startup input
    and literal argv
    `["query", "//:BUILD.bazel", "--lockfile_mode=off",
    "--registry=file://%workspace%/registry", "--output=label",
    "--noshow_progress"]`. It exits 0 at epoch 2 and captures:
    `["--output_base=<output_base>", "", "--nosystem_rc", "",
    "--noworkspace_rc", "", "--nohome_rc", "",
    "--host_jvm_args=-Dfile.encoding=ISO-8859-1", "",
    "--host_jvm_args=-Dfile.encoding=UTF-8", ""]`.
17. `reordered_same_multiset_retains_utf8` requests CLI UTF-8 followed by
    ISO-8859-1. It retains epoch 2 and the 94-byte SRI.
18. `reordered_cli_startup_diagnostics` uses that identical reordered input,
    exits 0 at epoch 2, and captures the same prefix followed by populated /
    empty UTF-8 and then ISO-8859-1 pairs.
19. `rc_source_change_same_multiset_reuses` adds only explicit
    `--bazelrc=startup-diagnostics.bazelrc`. The RC supplies generic
    ISO-8859-1 then matching `startup:linux` UTF-8. It retains epoch 2 and the
    94-byte SRI.
20. `rc_source_startup_diagnostics` uses identical startup input and adds
    command option `--announce_rc` to that literal query argv. It exits 0 at
    epoch 2 and captures
    `["", "", "--output_base=<output_base>", "", "--nosystem_rc", "",
    "--noworkspace_rc", "", "--nohome_rc", "",
    "--bazelrc=startup-diagnostics.bazelrc", ""]`.
    The leading empties are the two synthetic records paired with filtered
    populated RC options. It also captures exactly
    `Reading 'startup' options from <workspace>/startup-diagnostics.bazelrc: --host_jvm_args=-Dfile.encoding=ISO-8859-1, --host_jvm_args=-Dfile.encoding=UTF-8`.
21. `occurrence_change_restarts_latin1` returns to CLI-only ISO-8859-1,
    restarts at epoch 3, and produces the accepted 91-byte SRI
    `sha256-XvPSTH8P0MJVFfgfYQPMSsLAvSHGp2uIts690FOHYx4=`.
22. `zero_occurrences_restores_default` supplies no row startup occurrence,
    restarts at epoch 4, and preserves the exact 91-byte default SRI.

Only row 15 mutates the workspace. All five semantic rows retain the exact
existing `MODULE.bazel.lock` manifest: digest
`0ef28bd1c9d2583bcb82da1cc393a5973b7c84839f5961157b4ac99b2c3aecb7`,
size 1,228, mode `0o644`. Query companions have no mutations or manifest roots,
produce an empty manifest, and must retain raw stdout `//:BUILD.bazel\n`
(normalized stdout `//:BUILD.bazel`). Rows 21 and 22 have no diagnostic
acceptance; adding companions after them is nondiscriminating fixture growth.
No registry/module/provider/agent/JVM/Java asset, BUILD change, symlink,
hidden invocation, or production path enters the packet.

Before generation, focused tests cover strict fixture/command field parsing,
startup/environment merge and precedence, exactly one subprocess per row,
22-record ordering, unique BEP paths, absent-`combinedForm` mapping and every
schema/cardinality rejection, path-only normalization, exact RC extraction,
fixture-opt-in epoch mapping over every command, missing/stale identities,
missing/malformed/nonloopback/changed endpoints, Slug pre-execution refusal,
Bazel cleanup success/exit/death/endpoint failures, cleanup workspace cwd, and
primary-plus-cleanup exception chaining.

Then run one pinned Bazel 9.2 generation, two absolute distinct-root replays,
the focused harness/fixture suite, exact 91/94-byte hashes, all provenance
anchors, parser/scope/growth/archive/credential checks, process scans, and
terminal latest-diff reviews. Stop on a query output or digest difference,
unexpected epoch, any missing/extra diagnostic value, cleanup uncertainty,
scope/cap breach, or a Slug lifecycle claim.

After acceptance, run only design packet
`WP-5-m1-host-jvm-registry-byte-execution-feasibility`; no parser, transport,
helper, DICE, directory IO, Host consumer, or activation follows directly from
this oracle.

### Host JVM startup/reuse oracle implementation

Status: `ACCEPT` in `c67dc3a5` on 2026-07-27 after terminal latest-diff source
and architecture reviews. The exact seven-path packet generated 22 Bazel 9.2
rows with epochs `2/2/2/2/2/2/3/4`, the frozen CLI/RC diagnostics, exact raw
query stdout, 94/91-byte SRIs, and five unchanged 1,228-byte lock manifests.
Strict parsing, endpoint identity, Bazel-only cleanup, primary-error chaining,
checked raw-BEP purge, and missing capture fields are covered by 82 focused
harness tests.

One pinned generation, two worker replays, and four root review replays passed
from distinct absolute roots; the expected oracle remained
`0583b4743eb8ffd3d8ca1fe1dc0a503f3942ae3035d3d55186a743b9d4bd5e21`.
All three query shapes retained whole-workspace regular-file digest
`ee968db4322ee3033a2127db0e3df2339d84aeb83d7fdac1c5785e7582f36849`.
The packet is exactly +1,800 net lines including one two-line RC file, with
fixture inventory 58 regular files, five links, and 1,845 lines; archive,
credential, diff, and process gates passed. This is post-checkpoint oracle
packet two after `22de3631`.

Next packet: design only
`WP-5-m1-host-jvm-registry-byte-execution-feasibility`.

### Host JVM registry-byte execution feasibility

Status: `REPLAN` on 2026-07-27 after independent pinned-source,
native-observation/live-seam, and architecture reviews. No non-plan repository
file, fixture, parser, helper, transport, Rust, Cargo, dependency, API, DICE
key, directory bridge, Host consumer, or activation changed during the audit.
`/tmp/slug-jvm-feasibility` retains probe sources, compiled classes/JARs,
observations, and Bazel output-base artifacts; terminal scan found no Bazel,
slugd, helper, or charset-probe process.

No bounded Slug-owned standalone executor found can reproduce local-registry
directory bytes over Bazel 9.2's complete accepted JVM surface. The decisive
counterexample is an explicit `--server_javabase`: pinned
`startup_options.cc:458-505,538-595` gives that path first priority, accepts an
executable/readable `bin/java`, and Bazel's own
`src/test/shell/integration/bazel_java_test.sh:28-49` uses a shell launcher. A
compatible wrapper can branch on the exact Bazel JAR/main argv, alter the
registry directory only for that invocation, and then exec Java. Bazel
succeeds with changed bytes while a helper-main invocation differs. Supplying
the exact suffix launches the original Bazel command, which is delegation
rather than a Slug-owned byte executor.

Arbitrary `host_jvm_args` independently close the boundary.
`blaze.cc:360-465` appends user JVM options after fixed defaults; pinned
OpenJDK
`arguments.cpp:2136-2344`,
`InvocationAdapter.c:134-289,359-365,600-617`, and
`libjli/java.c:319-330,1789-1840` load Java/native agents, run `premain`, add
agent JARs to the system classpath, and expose launcher-selected
`sun.java.command`. A live Bazel-valid agent observed the Bazel server JAR,
main, classpath, and Bazel classes, while the same JDK/agent under a helper
observed the helper main/classpath and no Bazel class. Such an agent can then
transform `FileURLConnection` or collation. `--extra_classpath` and SPI
providers, argument files, native agents, patched/minimized or explicit JDKs,
and same-path artifact mutation are executable process state, not a finite
property projection.

The full parity predicate includes exact launch success/failure stage and
diagnostics; ordered launch argv and sanitized environment; main, JAR,
classpath, modules, resources, agents, native loader, working directory, and
lifetime; registry bytes/errors across launch, reuse, mutation, and restart;
and both Bazel-valid acceptance and Bazel-invalid rejection. A single
Bazel-success/candidate difference is terminal `REPLAN`. Canonical reuse in
`blaze.cc:978-1122` compares nonvolatile argv strings as an
occurrence-counted, order-insensitive multiset; it does not hash wrapper,
agent, provider, argfile, or JDK contents, so same-path mutation does not
itself restart or replace the executor. Its effects depend on when that
executor reads the path; another canonical mismatch kills it.

Live Slug cannot host such an executor today. Its CLI retains only
`--output_base`; daemon discovery trusts Unix-socket reachability; startup
unlinks the socket and writes an unauthenticated PID before readiness; the
protocol has no output-base lock, Status, cookie, nonce, start token, or
shutdown completion; one `WorkspaceRuntime`/DICE graph and Registry IO
capability live for the daemon; local Registry IO calls `tokio::fs::read` and
errors on directories; and the dormant Host file owner rejects a directory.
The Linux host has only GNU-Windows cross artifacts, not native Windows/Wine
execution. Direct JDK 25 probes confirmed `COMPAT`, invalid-file-encoding
fallback, and platform-owned native/JNU encoding, but native Windows
portable/empty/lone-surrogate evidence remains unavailable and cannot repair
the process-identity contradiction.

Explicit javabases plus Java and native agents are trusted local
arbitrary-code inputs. Any future authenticated transport must prevent their
remote injection, and the executor may receive neither extra Slug
credentials/capabilities nor a restrictive sandbox that changes behavior
Bazel permits. The former is a security superset and the latter a parity
subset.

Any future feasibility retry must present one Slug-owned mechanism that
defeats the explicit-javabase and agent counterexamples across the entire
success/failure domain without filtering, fallback, sandbox subset, or hidden
Bazel delegation. Only then may the sequence proceed through real-Windows
byte/transport evidence; a dormant full startup parser retaining request
diagnostics, ordered launch state, last-wins scalars, canonical reuse identity,
and sanitized environment; authenticated locked output-base lifecycle;
daemon-lifetime executor plus graph-construction semantic input; DICE-owned
external-directory observation; dormant Host ownership; and one atomic
daemon/one-shot/Host-consumer activation. None of those implementation
packets is authorized under this `REPLAN`.

Next packet: design only `WP-5-m1-loading-typed-propagation-design`. It is
independent of registry-directory JVM execution: preserve typed root
`SourcePreparationOutcome` and Host `PathOutcome` through `slug_loading_v2`,
migrate root file/listing reads from eager snapshots, retain current
external-repository guards, and freeze the exact allowlist plus lifecycle,
equal-Need, and event-suppression tests before Rust. No fixture-growth
checkpoint is due: accepted post-`22de3631` packets
`WP-5-m1-host-registry-local-directory-collation-charset-oracle` and
`WP-5-m1-host-jvm-registry-startup-reuse-oracle` total +4 regular files, +3
links, and +1,065 text lines, below all five-packet/+100-file/+10,000-line
triggers, and the selected loading design adds no fixture.

### Loading typed propagation design

Status: `REPLAN` on 2026-07-27 after independent live-call-graph,
Host-path/evidence, and architecture/orchestration audits. No non-plan
repository file, Rust, Cargo, dependency, API, fixture, DICE key, loading
consumer, or entrypoint changed.

The loading-only implementation boundary is not yet truthful.
`PackageLoadKey` first computes `RootModuleGraphKey` at
`app/slug_loading_v2/src/bzl_module.rs:897-925`; the resulting repository
mapping is presently unused, but the dependency anchors root `MODULE.bazel`
validity and event ownership before package listing or BUILD evaluation.
Keeping that legacy `Arc<Result<...>>` edge preserves eager snapshot state and
cannot propagate a future Host Need. Dropping it loses root semantics and
events. The accepted replacement, `HostRootModuleFileKey`, already returns
`SourcePreparationOutcome` with complete-only equality, validity, and event
storage, but its key, constructor, success value, and error remain
crate-private at `app/slug_bzlmod_v2/src/host_module.rs:50-120`.

Changing the existing loading keys in place is also outside this packet.
`PackageLoadKey` has direct analysis, query, and core consumers that require
its current `Arc<Result<...>>` value; the public evaluator methods return
`anyhow::Result`. `PackageListingKey` recursively consumes
`WorkspaceDirectoryKey`; BUILD fallback and every `.bzl` parse use
`WorkspaceFileKey`; and both companion-discovery paths use the eager
directory projection. `BzlModuleEvalKey`'s cycle detector recognizes only
that legacy key identity. A typed replacement must therefore be a parallel,
dormant, root-only surface until the separately scheduled analysis, query,
core, and companion migrations consume it.

Two further leaf prerequisites remain. The current `PackageListing` stores
UTF-8 `CompactString`, while pinned Bazel
9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a` preserves Unix
directory-name bytes as internal strings:
`src/main/native/unix_jni.cc:507,552-615` creates each `dirent::d_name` with
`NewStringLatin1`, `src/main/native/latin1_jni_path.cc:26-44` maps every
unsigned byte directly to one Java character,
`src/main/java/com/google/devtools/build/lib/unix/UnixFileSystem.java:54-63,88-110`
retains it in directory entries, and
`src/main/java/com/google/devtools/build/lib/util/StringEncoding.java:50-85`
defines the raw-byte invariant. `PathFragment.java:84-113` compares those raw
unsigned bytes. There is no UTF-8 decode or invalid-name diagnostic:
`StarlarkUtil.java:43-66` separately validates source contents, then
`UnixGlob.java:212-248` compares patterns and names in the same internal byte
representation. Raw non-ASCII directories remain traversable rather than
package boundaries under `LabelValidator.java:50-61,96-138`,
`PackageLookupFunction.java:91-96`, and
`DirectoryDirentProducer.java:95-116`. Neither rejection nor
`to_string_lossy` is authorized. A separate design must freeze a byte-capable
Unix/Windows internal component plus Starlark pattern/result representation
before `PackageListing` changes; it must reuse the existing compact
deterministic collections/shared slices or record an explicit V2-owned
utility extraction, not add standard map/string churn.
Filesystem-only Latin-1 mapping is insufficient: pinned
`StarlarkUtil.java:43-66` and `net/starlark/java/syntax/Lexer.java:274-425`
retain distinct internal bytes for literal UTF-8 `é` (`c3 a9`) and octal
`\351` (`e9`). If starlark-rust has already collapsed those spellings to one
Unicode scalar, the later design also needs a Bazel-internal-string
parser/evaluator seam; it may not reconstruct origin after evaluation.

`PathDirectoryListingKey` also returns only sorted names, while Bazel's
no-follow directory value retains direct kinds and resolves symlinks only
after an active glob fragment matches. Pinned
`DirectoryListingStateValue.java:84-90`,
`PatternWithWildcardProducer.java:102-139,172-208`,
`PatternWithoutWildcardProducer.java:68-97`, and
`FileValue.java:49-82,113-120` prove the split: an unrelated dangling or
cyclic symlink must not fail the glob; a matched final directory recurses
under the logical path, a matched final file participates, and a matched
dangling path is omitted. Direct special entries are skipped by a wildcard
but accepted through a matching literal FileValue. Therefore eager
whole-listing child resolution and terminalization is not exact. The later
loading design must choose either a prepared value that retains raw
no-follow kind plus deferred per-entry resolution/error state for synchronous
pattern use, or a pattern-keyed Host glob owner. Its exact allowlist may
include `glob.rs` and cannot be frozen by this packet.

The public `PathFileBytesKey` is not the missing file owner: it accepts only a
resolved `RegularFile`. Pinned Bazel
`src/main/java/com/google/devtools/build/lib/skyframe/BzlCompileFunction.java:117-123`
and
`src/main/java/com/google/devtools/build/lib/skyframe/PackageFunction.java:1287-1294`
read special `.bzl` and BUILD files as files, and the accepted private bzlmod
`HostFileBytesKey` already models regular-or-special behavior. Loading needs a
separately reviewed reusable Host byte projection rather than copying that
private owner or narrowing Bazel-valid input.

The first prerequisite is design-only
`WP-5-m1-bzlmod-root-loading-anchor-projection-design`. Its eventual
implementation allowlist is exactly:

- `app/slug_bzlmod_v2/src/host_module.rs`; and
- `app/slug_bzlmod_v2/src/lib.rs`.

Add one public opaque `RootModuleLoadingAnchorKey` over the private
`HostRootModuleFileKey`, returning
`SourcePreparationOutcome<Arc<Result<RootModuleLoadingAnchor,
RootModuleLoadingAnchorError>>>`. The opaque success and error wrappers retain
the complete private values internally so equality remains exact without
exposing evaluated-module, include/file-error, repository-mapping, lockfile,
or registry internals. Need passes through unchanged. The wrapper uses
`complete_eq` and `is_complete`, owns no event batch, and leaves the private
producer and its batch in the dependency closure. A DICE compute failure is a
fail-fast invariant, never a terminal wrapper. Focused tests remain in
`host_module.rs` and prove bootstrap/path Need pass-through, self-unequal
invalid Need, equal complete success/error, no wrapper batch, and retained
private-producer event closure. No Cargo change is permitted.

After that anchor and both leaf designs are accepted, loading still requires
parallel dormant root-only keys rather than an in-place conversion: at
minimum `HostBzlParseKey`, `HostBzlModuleEvalKey`, and
`HostPackageLoadKey`, each returning
`SourcePreparationOutcome<Arc<Result<T, LoadingError>>>`. The exact
listing/glob key and production/test allowlists remain owned by the
byte-capable pattern-lazy design. `cycle_detector.rs` must eventually
generalize its private node identity to legacy-or-Host without changing
legacy diagnostics. The Host package root must compute the public bzlmod
anchor before listing; actual Path Needs become only
`SourcePreparationNeeds::path`; infrastructure failures and Needs are never
stringified. A Need stores no local event batch, Complete success/error stores
one exact batch, dependency batches remain dependency-owned, and runtime
failure retains only its executed print prefix.

Future focused evidence must cover root-anchor ordering; BUILD
primary/fallback and `.bzl` create/edit/delete/recreate; regular and special
bytes; matching versus unrelated file/directory symlink
retarget/dangling/cycle behavior; raw non-UTF-8 names; literal-versus-wildcard
special entries; reached-frontier Need union without eager unmatched
resolution; self-unequal Need and semantic-equal Complete pruning; cycle
recovery; and a speculative Need attempt whose local events do not enter the
completed root closure. Existing external guards remain unchanged:
`resolve_local_load` rejects nonroot repositories before Host observation,
package paths stay normalized absolute descendants of the root workspace,
`package.rs` keeps dependency/output labels root-local, and `visibility.rs`
keeps external package specifications deferred. The Host blocks must contain
no eager workspace snapshot/file/directory key, legacy root graph/files key,
materialization namespace, direct filesystem IO, Need-to-string arm, or
entrypoint/downstream consumer.

No fixture-growth checkpoint is due: this design and its selected anchor
design add no fixture. Next packet: design only
`WP-5-m1-bzlmod-root-loading-anchor-projection-design`.

### Bzlmod root-loading anchor projection design

Status: `ACCEPT` on 2026-07-27 after independent source/API,
implementation/evidence, and architecture/orchestration audits plus corrected
terminal latest-text review. No non-plan repository file, Rust, Cargo,
dependency, fixture, DICE key, consumer, or entrypoint changed. This internal
projection changes no Bazel behavior and needs no new Bazel citation; the live
accepted private Host producer and its retained source evidence remain the
semantic authority.

Implement only `WP-5-m1-bzlmod-root-loading-anchor-projection` with an exact
two-file allowlist:

- `app/slug_bzlmod_v2/src/host_module.rs`; and
- `app/slug_bzlmod_v2/src/lib.rs`.

In `host_module.rs`, add public `RootModuleLoadingAnchorKey`,
`RootModuleLoadingAnchor`, and `RootModuleLoadingAnchorError`. The key contains
only a private `NormalizedAbsolutePath` workspace, derives
`Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe`, exposes only
`new(NormalizedAbsolutePath)`, and displays exactly
`root-module-loading-anchor:{workspace}`. Do not add a path-conversion error,
workspace accessor, or second identity field.

Both result wrappers have private fields and retain the original
`Arc<Result<HostRootModuleFileValue, HostRootModuleFileError>>`; a private
carrier alias is permitted. Construct each wrapper only from its matching
complete success/error branch. They use `Clone, PartialEq, Eq, Allocative,
Dupe`, share the carrier without deep cloning the evaluated module, overrides,
module paths, or error, and implement manual opaque `Debug`. They expose no
field, constructor, variant, typed accessor, conversion trait, dereference,
serialization surface, evaluated module, override, module path, repository
spec, or mapping. Error `Display` delegates byte-for-byte to the retained
private diagnostic and `Error::source()` delegates to the private error's
source behavior rather than exposing the private error as a new cause.

The key value is exactly
`SourcePreparationOutcome<Arc<Result<RootModuleLoadingAnchor,
RootModuleLoadingAnchorError>>>`. Its compute has exactly one direct semantic
dependency, `HostRootModuleFileKey::new(self.workspace.dupe())`, unwraps only
the DICE infrastructure result through the existing fail-fast
`dice_invariant`, and maps the private outcome so Need moves through unchanged
while Complete success/error receive the matching opaque wrapper. Do not use a
DICE projection/opaque compute, terminal infrastructure-error variant,
`anyhow`, stringification, or direct IO. Equality is `complete_eq`; validity
is `is_complete`. Complete success/error compare the full retained private
value, while every Need remains invalid and self-unequal.

The public key never inspects `CaptureEvaluationEvents` or stores evaluation
data. The private `HostRootModuleFileKey` remains its sole direct dependency
and sole owner of the exact Complete event batch, including empty and
failure-prefix batches. Neither key owns a batch on Need; the private Complete
batch remains reachable through the public key's activation closure.

Keep all focused tests in the existing Unix `host_module.rs` test module and
within the accepted total cap of 320 additions: at most 315 in
`host_module.rs` and five in `lib.rs`. Prove:

- normalized workspace identity, distinct-workspace inequality, and exact key
  display;
- unchanged first-path and root-bootstrap Needs with no fabricated repository
  need or wrapper error;
- invalid/self-unequal Need plus equal and unequal separately allocated
  Complete success/error values;
- manual success/error Debug omits private variants, fields, paths, module,
  overrides, and occurrences, while error Display/source exactly preserves the
  existing diagnostic behavior;
- the public key's direct dependency set is exactly the private root key;
- Need stores no public or private root batch; Complete stores no public batch
  while the private root batch owns an exact printed event; and
- one retained DICE graph advances from first-path Need through root-bootstrap
  Need, Complete success, Complete error, and restored semantically equal
  success, proving invalid Needs force recomputation; warm semantic reuse adds
  no replay batch.

Re-export exactly the three public anchor names from `lib.rs`; keep
`host_module` and every `HostRootModuleFile*` type private. Add no downstream
consumer or activation. Added production code contains one new `impl Key`,
owns no event storage, direct filesystem access, snapshot, graph, lockfile,
registry, global, cache, standard collection, owned string/vector, or new
dependency. Replan if implementation needs another file, public private-owner
type, copied semantic payload, wrapper event batch, downstream activation, or
more than the frozen addition cap.

Validate the focused anchor tests first, then the full
`slug_bzlmod_v2` and existing `slug_loading_v2` suites, doctests, GNU-Windows
test-link gate, formatting, diff, exact-scope/Cargo, public-surface,
implementation-block, archive, credential, and process guards. No oracle or
fixture is required, and no fixture-growth checkpoint is due.

Next packet: implement only
`WP-5-m1-bzlmod-root-loading-anchor-projection`.

### Public bzlmod root-loading anchor projection

Status: `ACCEPT` on 2026-07-27. The exact two-file implementation is +314/−4:
+311/−4 in `app/slug_bzlmod_v2/src/host_module.rs` and +3 in
`app/slug_bzlmod_v2/src/lib.rs`, within every frozen cap. No Cargo,
dependency, fixture, loading/downstream consumer, entrypoint, or other
repository file changed.

The public normalized-workspace key computes only the accepted private Host
root-module key through the fail-fast DICE boundary. It passes Need unchanged,
retains the original private `Arc<Result<...>>` in opaque success/error
wrappers, uses complete-only equality/validity, and stores no evaluation data.
The private producer remains the sole direct dependency and sole event-batch
owner. The three intended public names are the only exports; manual Debug
reveals no carrier structure, while error Display/source preserve the existing
diagnostic without exposing a typed private cause or accessor.

Focused regressions prove exact identity/display, invalid self-unequal Needs,
valid equal and discriminating unequal Complete success/error values,
diagnostic opacity/transparency, sole dependency ownership, and no public
batch. One retained DICE graph passes through first-path Need, root-bootstrap
Need, printed Complete success, Complete error, semantically equal restored
success, and warm no-replay reuse; Need owns no private or public root batch,
while Complete retains the exact private producer event.

Validation passed:

- focused anchor tests: 2 passed;
- `slug_bzlmod_v2`: 203 unit plus 184 integration tests and zero doctests;
- downstream `slug_loading_v2`: 54 tests and zero doctests;
- GNU-Windows no-run linkage: all 12 bzlmod and six loading test executables;
  and
- formatting, diff, exact two-file/addition cap, Cargo, archive, public-surface,
  implementation-block, forbidden-edge, credential, downstream-use, and
  process guards.

Independent source/API, implementation/evidence, and
architecture/orchestration terminal latest-diff reviews all returned
`ACCEPT`. No oracle or fixture was added, so no fixture-growth checkpoint is
due.

Next packet: design only
`WP-5-m1-loading-byte-capable-pattern-lazy-glob-design`.

### Byte-capable pattern-lazy loading/glob design

Status: `REPLAN` on 2026-07-27 after independent pinned-source/API,
implementation-feasibility, and architecture/orchestration audits. No
non-plan repository file, Rust, Cargo, dependency, fixture, DICE key, loading
consumer, or entrypoint changed.

A single loading implementation boundary is not truthful. Pinned Bazel 9.2
commit `8220c6198837d5c13d53fea211cf3282aa12408a` preserves internal
strings as raw bytes. On Unix,
`src/main/native/unix_jni.cc:507,552-615` and
`src/main/native/latin1_jni_path.cc:26-44` map every name byte to one Java
Latin-1 character; `StringEncoding.java:25-85,116-141` defines the internal
representation and Windows/platform conversion; and
`PathFragment.java:84-113` orders unsigned raw bytes. The eventual V2-owned
`BazelInternalString` must therefore retain shared raw bytes with raw-byte
equality, hashing, and ordering. `CompactString`, ordinary Rust `String`
identity, lossy conversion, and a global interner are not exact.

The parser/evaluator seam is separately blocking. Bazel
`StarlarkUtil.java:43-66`, `ParserInput.java:82-95`, and
`net/starlark/java/syntax/Lexer.java:274-425,443-495` preserve literal UTF-8
`é` as internal bytes `c3 a9` and octal `\351` as `e9`. Live starlark-rust
`starlark_syntax/src/lexer.rs:244-311,317-423` decodes both to Rust U+00E9,
then `starlark/src/eval/compiler/expr.rs:1045-1055` allocates the collapsed
`&str`. Loading cannot reconstruct origin after evaluation. A future opt-in
Bazel string-token lexer seam must map each string literal's unescaped source
bytes to Latin-1 scalars while the codemap retains the original source bytes
and spans; it must preserve exact escape behavior and convert actual evaluated
strings and returned glob results one internal scalar/byte at a time. It must
prove dynamic concatenation, slicing, length, hashing, ordering, macro
pass-through, raw/triple strings, Unicode/non-BMP platform behavior, and
diagnostics while leaving standard starlark-rust mode unchanged. Do not claim
lone-surrogate Windows parity without native Windows evidence.

Bazel's directory value retains sorted no-follow
`Dirent(name, FILE|DIRECTORY|SYMLINK|UNKNOWN)` under
`DirectoryListingStateValue.java:84-90` and `Dirent.java:20-31`. Slug's
`PathDirectoryEntries` already preserves sorted unique raw `OsString` names
but omits direct kind. For a wildcard fragment, Bazel first byte-matches the
name, skips direct `UNKNOWN`, handles direct file/directory inline, and
resolves only a matched symlink
(`PatternWithWildcardProducer.java:102-139,172-208`). A literal fragment
bypasses listing and follows `FileValue` directly
(`PatternWithoutWildcardProducer.java:68-97`): present regular, special, and
symlink-to-nondirectory entries participate; directories recurse; missing or
dangling paths are omitted. Unmatched dangling or cyclic symlinks cannot fail;
matched cycles, infinite expansion, and reached inconsistencies remain typed
errors. `UnixFileSystem.java:97-120` retains native no-follow `d_type` and
refines only native unknown inside the directory observation; wildcard glob
then stats only cached symlinks. A matching-only no-follow Lstat for every
matched name adds observation/race/Need edges and can see a different kind
after a listing race, so it is not an exact substitute. The next design must
freeze typed name-plus-direct-kind observation. Unix must refine native
`DT_UNKNOWN` inside the one listing observation while retaining special or
failed-refinement entries as semantic `UNKNOWN`; Windows must reproduce its
name-list plus no-follow-stat mapping under
`WindowsFileSystem.java:36,135-176` and `FileSystem.java:594-627`, where
special or null child status becomes `UNKNOWN`. Race/error ownership remains
with that observation rather than a later matched-name dependency.

The expected retained shape is a sorted unique shared slice of raw OS-native
name plus direct kind, deduplicated by name with kind included in semantic
equality; `Dupe` and `Allocative` remain required and no interner is justified.
The next design must distinguish tolerated native unknown/special
classification from child-classification errors that terminalize the whole
listing, keep symlinks unresolved, preserve complete-only equality and
self-unequal Need, and freeze same-name kind-only, unknown-versus-special,
Windows no-follow, equal-restoration, and directory-error recovery evidence.

The full matcher also exceeds the current reviewed subset. Pinned
`UnixGlob.checkPatternForError` permits `?`, standalone `**`, and literal
backslashes, braces, and brackets; it rejects empty/absolute patterns, empty
or dot segments, and non-standalone `**`. Leading-dot names require an
explicit dot, and the historical parenthesis behavior at
`UnixGlob.java:297-305` must remain exact. Matching and sorting operate on
internal raw bytes.

Dynamic glob patterns are constructed and passed through loaded `.bzl` macros
only during synchronous BUILD evaluation. Live starlark-rust exposes no async
Evaluator/native-call suspension seam, while reached symlink resolution must
await DICE. Direct IO or `block_on` in the builtin, eager unmatched
resolution, and hidden process state are forbidden. Before activation, a
separate reviewed design must choose either typed evaluator suspension/resume
or an explicitly transactional attempt-local abort/await/restart loop. Such a
loop may retain only compact request/result frontier state across awaits; it
must discard targets, used globs, prints, and the entire incomplete evaluator
attempt, publish no event batch on Need, and prove user code cannot catch or
observe the control transfer. Host glob owns filesystem dependencies and no
events; Host package loading owns only the final completed local batch.

The corrected serial boundary is:

1. design `WP-5-m1-loading-host-dirent-observation-design`, deciding exact
   typed-dirent API and freezing the exact allowlist after choosing whether to
   reuse the already-public `WorkspaceDirectoryEntryKind` or add and re-export
   a new Host/no-follow kind; the latter requires at least
   `app/slug_workspace_v2/src/{lib.rs,path_observation.rs}` and
   `app/slug_core_v2/src/runtime/path_observation.rs` plus in-file tests;
2. design a separate Bazel-internal byte-string oracle/feasibility and opt-in
   parser/evaluator seam, with no legacy parse activation;
3. design and implement a discriminating pinned Bazel oracle before the
   corresponding Slug loading implementation;
4. implement the accepted typed-dirent and byte-string prerequisites in
   separate reviewed packets only after their oracle evidence;
5. design and implement the pure pattern-keyed root-only Host glob owner
   returning
   `SourcePreparationOutcome<Arc<Result<HostGlobValue, HostGlobError>>>`; and
6. separately design transactional dynamic-glob package evaluation before any
   consumer activation.

The eventual pure Host glob identity is normalized workspace/package, one
exact internal pattern, and files-only versus files-and-directories operation.
It traverses only reached pattern frontiers, joins and unions reached Needs,
uses fail-fast DICE infrastructure handling plus complete-only
equality/validity, owns no event batch, never resolves unmatched entries, and
retains sorted shared internal paths with compact `SmallMap`/`SmallSet`
frontier state. It uses no standard retained map/string/vector, regex
dependency, direct IO, legacy Workspace key, consumer, or activation. The
regular-or-special Host BUILD/`.bzl` byte projection remains a distinct
prerequisite.

The future oracle must discriminate raw Unix `c3 a9` and `e9` names,
literal-UTF8 versus octal and dynamically constructed patterns/results,
direct special wildcard-versus-literal behavior, matched and unrelated
file/directory/dangling/cyclic symlinks, `?`, `**`, leading-dot and
parenthesis behavior, subpackage boundaries, same-daemon
create/rename/delete/recreate, and symlink retarget through
file/directory/dangling/cycle/restored. Retained-DICE evidence must prove
reached-Need union, absence of unmatched demands, invalid self-unequal Need,
semantic-equal Complete pruning, typed error recovery, and speculative
print-before-glob suppression. No fixture-growth checkpoint is due because
this stopped design adds no fixture.

Next packet: design only
`WP-5-m1-loading-host-dirent-observation-design`.

### Host typed no-follow dirent observation design

Status: `ACCEPT` on 2026-07-27 after independent pinned-source/API,
live implementation-feasibility, and architecture/orchestration audits plus
terminal latest-text review. No non-plan repository file, Rust, Cargo,
dependency, fixture, DICE key, loading consumer, or entrypoint changed.

The observation owner is `slug_workspace_v2`; the native producer is
`slug_core_v2::runtime::path_observation`. Keep the one existing
`PathObservationOperation::DirectoryEntries` demand. Native enumeration and
all direct-child classification belong inside that one observation and never
escape as separate DICE Lstat demands. `PathDirectoryListingKey` continues to
forward the typed observation unchanged. Existing complete-only
`PathObservationKey`/listing equality and self-unequal invalid Need remain;
repository revalidation compares the complete typed payload, so a same-name
kind transition invalidates.

Break the names-only public API atomically. Add dedicated
`PathDirectoryEntryKind::{File, Directory, Symlink, Unknown}` and
`PathDirectoryEntry { name, kind }` rather than reuse the legacy eager
UTF-8 `WorkspaceDirectoryEntryKind`. The kind is
`Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative, Dupe`.
The entry has private fields, public `new`, `name`, and `kind`, and derives
`Debug, Clone, PartialEq, Eq, Allocative`. `PathDirectoryEntries` retains
exactly one shared `Arc<[PathDirectoryEntry]>`, performs a stable sort by raw
OS-native name only, retains duplicate names and their input order, exposes
only `entries()`, and includes kind in equality. Its constructor returns
`Self`; remove the obsolete `DuplicatePathDirectoryName` error and re-export.
This matches `Dirent.java:20-31,51-75`,
`CompactSortedDirents.java:27-68,106-149`, and
`DirectoryListingStateValue.java:35-45,84-106`: comparison is name-only,
sorting is stable, duplicates remain, and value equality retains packed kind.
Preserve `PathDirectoryName`,
raw `OsString`, `Dupe`, and `Allocative`; a temporary construction `Vec` is
permitted, but no retained standard map/vector/string, compact map/set,
interner, or new hash wrapper is justified.

Remove `PathDirectoryEntries::new(names)`, `.names()`, implicit
`From<PathDirectoryName>`, and default-Unknown compatibility. Each would
fabricate or discard required semantic kind. Migrate every constructor and
accessor caller in the same implementation packet with explicit kinds; test
helpers may not silently default all names to File.

The eventual implementation allowlist is exactly five files:

- `app/slug_workspace_v2/src/path_observation.rs` — at most 150 additions;
- `app/slug_workspace_v2/src/lib.rs` — at most five additions;
- `app/slug_workspace_v2/src/path_resolution.rs` — test/helper migration only,
  at most 45 additions;
- `app/slug_core_v2/src/runtime/path_observation.rs` — at most 520 additions;
  and
- `app/slug_core_v2/src/runtime/repository_io.rs` — validation-test migration
  only, at most 30 additions.

The total cap is 750 additions. No Cargo, dependency, fixture, loading,
bzlmod, Starlark, new DICE key/operation, consumer, or activation change is
permitted.

On Unix, copy both `d_name` and `d_type` from each native readdir record.
Preserve `DT_REG`, `DT_DIR`, and `DT_LNK` directly; map native
character/block/FIFO/socket special types to `Unknown` without statting them.
Only native `DT_UNKNOWN` or an unrecognized type receives a no-follow child
stat after enumeration and directory-close handling. A present
regular/directory/symlink refines to the matching kind; present special,
ENOENT/FileNotFound, ENOTDIR, and ELOOP/Bazel-tolerated symlink-loop
refinement become `Unknown`; other child-stat IO terminalizes the whole
listing. Keep symlinks unresolved and preserve existing interrupted read,
transient `EIO`, partial read, close, and primary-error precedence. This follows
`src/main/native/unix_jni.cc:500-540,552-615`,
`UnixFileSystem.java:54-63,88-120`, and `UnixFileStatus.java:41-56`, and keeps
the enumeration/refinement race inside one observation.

On Windows, retain raw UTF-16/`OsString` enumeration and close it before
classification, then reproduce Bazel base `FileSystem.readdir`: no-follow
status each enumerated child, mapping regular/directory/symlink directly and
special or every null/IO child status to `Unknown`. Do not reuse the current
full Lstat helper because its change-time query adds behavior not present in
classification, and do not substitute `FindFirstFileW` attributes without
native proof for reparse-point, race, and error behavior. The source authority
is `WindowsFileSystem.java:36,135-176`,
`JavaIoFileSystem.java:84-101`, and
`FileSystem.java:469-492,594-627`. GNU-Windows compilation proves shape only;
native Windows and lone-surrogate ordering remain an activation gate.

Implement with pure/scripted classifier seams in
`runtime/path_observation.rs`, preserving the existing production wrappers.
Focused tests must prove:

- raw non-UTF-8 name plus kind, raw-name sorting, stable duplicate-name
  retention/order across equal/different kinds, separately allocated equality,
  same-name File-to-Directory inequality, and Unknown restoration;
- invalid/self-unequal Need, equal Complete pruning, and one retained
  transaction through Need, File, Directory, Unknown, and restored equal File;
- repository validation becomes dirty on kind-only change, stays clean on
  equal reorder/restoration, and path listing forwards the typed value;
- every Unix native type, no classification for known/special types,
  `DT_UNKNOWN` present/missing/ENOTDIR/special/tolerated-loop/hard-error
  behavior,
  enumerate-and-close-before-refine, partial/read/close precedence, raw names,
  and real file/directory/symlink/socket create/replace/delete/recreate;
- Windows file/directory/symlink/special/null/error mapping, no-follow
  symlink behavior, raw UTF-16, and name/stat ordering through the scripted
  seam; and
- existing base-directory missing/error/refinement recovery remains exact.

After the oracle prerequisite lands, validate focused tests first, then full
`slug_workspace_v2` and `slug_core_v2`, existing `slug_bzlmod_v2` and
`slug_loading_v2` suites and doctests, and GNU-Windows no-run linkage for all
four crates. Run formatting, diff, exact allowlist/caps, Cargo, archive,
credential, process, public-API, constructor/accessor-removal, dependency, and
forbidden-scope guards. Require independent source/API,
implementation/evidence, and architecture/orchestration terminal latest-diff
reviews.

Stop if exact Windows behavior requires a new observation operation, any child
classification escapes as a separate Need, implementation needs a production
file outside the five-file allowlist, the cap cannot hold, or the prerequisite
oracle expands into deferred raw non-ASCII strings, parser/evaluator,
pattern-matcher breadth, Host glob ownership, or package-evaluation retry.
Also stop if implementation deduplicates names, sorts equal names by kind,
stats direct Unix kinds/specials, refines Unix unknown before listing close,
terminalizes a Windows child-status error, resolves a symlink, or converts a
raw name lossily.

Oracle-first scheduling is mandatory. The accepted design for
`WP-5-m1-loading-host-dirent-glob-oracle` follows. Implement and pin that
focused ASCII oracle as post-checkpoint oracle packet three, then implement
only `WP-5-m1-loading-host-dirent-observation`. Resume the byte-string
oracle/feasibility design only after the typed observation is accepted. No
fixture-growth checkpoint is due for either design-only packet; the oracle
packet must record exact net growth against `22de3631`.

### Focused Host-dirent glob oracle design

Status: `ACCEPT` on 2026-07-27 after independent pinned-source/behavior,
fixture/harness feasibility, and architecture/orchestration audits plus
terminal latest-text review. No fixture, harness, generated expectation,
Bazel probe, Rust, Cargo, dependency, consumer, or entrypoint changed.

Extend only the existing Bazel 9.2 `glob-directory-invalidation` fixture.
Preserve every pre-existing field and value in its first four command records,
adding only a `server_epoch` field. Set fixture
`observe_server_epochs = true` and command `capture_server_epoch = true` on
all eleven rows; require every recorded epoch to be equal and nonzero. Append
seven commands proving direct-kind and matched-symlink behavior through one
daemon. Do not alter
`glob-callable-contract` or `glob-package-boundaries`, and do not create a new
fixture.

Add a fixture-level `required_host_os = "posix"` field. The parser accepts
only absent or `"posix"`; the runner rejects an unsupported host before any
workspace mutation or Bazel invocation. Add only one mutation operation,
`op = "fifo"`, accepting exactly `path` and creating the FIFO with
`os.mkfifo(path, 0o600)`, then applying ordinary `os.chmod(path, 0o600)` so
the exact mode is independent of `umask`. Immediately verify with `lstat`
that the collision-free fresh entry remains a FIFO and its permission bits are
exactly `0600`. It must reject escapes, absent or non-real parents, collisions,
extra fields, and unsupported hosts. Broaden the existing rename source
validation to use no-follow `lstat` and accept a regular file, symlink,
directory, or FIFO; retain all destination, escape, and collision checks. Do
not add generic directory, symlink, deletion, or special-file setup operations.
Every created FIFO is run-scoped transient cleanup state, never a manifest
input.

The immutable fixture topology adds exactly four regular assets:

- `workspace/pkg/state`;
- `workspace/pkg/links/good-match.txt`;
- `workspace/staging/dir/child`; and
- `workspace/cycle_error/BUILD.bazel`.

It adds exactly six checked relative symlinks:

- `workspace/staging/link` targets `specials/direct`, so after moving to
  `pkg/state` it resolves to `pkg/specials/direct`;
- `workspace/pkg/specials/link-to-direct` targets `direct`;
- `workspace/pkg/links/dangling-match.txt` targets missing
  `missing-match.txt`;
- `workspace/pkg/links/unrelated-dangling.bin` targets missing
  `missing-unrelated.bin`;
- `workspace/pkg/links/unrelated-cycle.bin` targets its own basename
  `unrelated-cycle.bin`; and
- `workspace/cycle_error/matched-cycle.txt` targets its own basename
  `matched-cycle.txt`.

`pkg/BUILD.bazel` keeps the accepted `txts` target and adds generated target
names for:

- `glob(["state*"])`, prefixed `state_files_`;
- `glob(["state*"], exclude_directories = 0)`, prefixed `state_all_`;
- `glob(["specials/*"])`, prefixed `special_wild_`;
- literal `glob(["specials/direct"])`, prefixed `special_literal_`; and
- `glob(["links/*-match.txt"])`, prefixed `links_`.

Each matched path replaces `/` with `_` in the target name. New success rows
query `//pkg:all except //pkg:txts` and assert exact ordered output. With
`pkg/state` regular and `pkg/specials/direct` a FIFO, the baseline is exactly:

- `//pkg:links_links_good-match.txt`;
- `//pkg:special_literal_specials_direct`;
- `//pkg:special_wild_specials_link-to-direct`;
- `//pkg:state_all_state`; and
- `//pkg:state_files_state`.

The appended same-daemon sequence is:

1. create FIFOs `pkg/specials/direct` and `staging/special`, then assert the
   baseline;
2. rename `pkg/state` to `staging/file` and `staging/dir` to `pkg/state`;
   assert only `state_files_state` disappears;
3. rename that directory to `staging/dir-used` and `staging/link` to
   `pkg/state`; assert the baseline, proving a matched symlink to a special;
4. rename that symlink to `staging/link-used` and `staging/special` to
   `pkg/state`; assert only the three non-state targets, proving wildcard
   direct-special omission while the literal special remains accepted; and
5. rename that FIFO to `staging/special-used` and `staging/file` to
   `pkg/state`; assert the baseline restoration.

The existing four rows plus these five rows are followed by an isolated
matched-cycle failure and recovery. `cycle_error/BUILD.bazel` defines a static
`probe` target whose source uses
`glob(["matched-*"], allow_empty = True)`. Querying
`//cycle_error:probe` must exit 7 and include both
`Symlink issue while evaluating globs: Symlink cycle:` and
`cycle_error/matched-cycle.txt`. Then rename the cycle to `.parked`, repeat
the query on the same server, and assert exact success
`//cycle_error:probe`. Total command count is exactly eleven.

Unmatched dangling and cyclic symlinks must never fail. A matched dangling
symlink is omitted. A direct FIFO is skipped by wildcard listing but accepted
by a literal pattern, while a wildcard-matched symlink to that FIFO
participates. The isolated matched cycle must fail with the pinned diagnostic
and recover after rename. Source-only follow-ups retain native `DT_UNKNOWN`,
duplicate-name stability, listing races, every Windows mapping and lone
surrogate case, raw non-ASCII names, and broader matcher semantics.

The source authority is Bazel 9.2:

- `packages/producers/PatternWithWildcardProducer.java:102-139,164-208`;
- `packages/producers/PatternWithoutWildcardProducer.java:68-97`;
- `packages/producers/DirectoryDirentProducer.java:95-156`;
- `vfs/Dirent.java` and
  `skyframe/{DirectoryListingStateValue.java,CompactSortedDirents.java}`;
- `unix/UnixFileSystem.java` plus the matching native Unix directory sources;
  and
- test `skyframe/GlobTestBase.java`, `skyframe/PackageFunction.java`, and
  `io/FileSymlinkCycleException.java`.

The implementation allowlist is exactly:

- `tools/v2_oracle_lib/fixture.py`;
- `tools/v2_oracle_lib/runner.py`;
- `tests/v2_oracle/test_v2_oracle.py`;
- `tests/v2_oracle/fixtures/glob-directory-invalidation/fixture.toml`;
- its `expected/oracle.json`;
- its `workspace/pkg/BUILD.bazel`;
- the four regular assets and six symlinks listed above.

The cap is +900/-100 total lines, exactly four regular and six symlink assets,
and at most 550 newline-counted regular-file lines in the fixture. Its
pre-packet baseline is six regular files, zero links, and 183 text lines.
Against accepted checkpoint `22de3631`, the prior two post-checkpoint oracle
packets total +4 regular, +3 symlinks, and +1,065 lines. This is
post-checkpoint oracle packet three; no five-packet/~100-file/~10,000-line
growth review is due.

Focused harness tests must cover the parser field/operation matrix,
unsupported-host rejection before mutation, FIFO absent/collision/escape/
existing-real-parent/mode/raw command record behavior, directory/FIFO rename
through no-follow classification, retention of existing create/delete/rename
behavior, exact command count eleven, and byte-identical preservation of the
projection of every pre-existing field and value in the first four generated
command records while permitting only the new `server_epoch` field. Assert
fixture epoch observation plus per-command capture for all eleven rows.

Generate once with pinned `/usr/bin/bazel`, then replay the checked expectation
from two distinct absolute fresh roots. Require byte-identical output and
hashes, the same nonempty Bazel server epoch for all eleven commands, no Slug
execution, and empty manifest output for every command. Clean Bazel, FIFO,
socket, and runner state before and after. Run focused harness tests and exact
fixture inventory/growth/provenance, credential, archive, diff, allowlist, cap,
symlink, and first-four-record guards.

Stop on POSIX unavailability, unsupported filesystem FIFO behavior, a pinned
Bazel contradiction, an unrelated link failure, a materially different
matched-cycle diagnostic, failed recovery, server-epoch change, mutation of
one of the first four records, cap failure, or required scope beyond the
allowlist. Do not weaken exact expectations, invoke Slug, broaden matcher
coverage, or absorb the byte-string/parser seam.

Next packet: implement only
`WP-5-m1-loading-host-dirent-glob-oracle`.

### Focused Host-dirent glob oracle implementation stop

Status: `REPLAN` on 2026-07-27 from clean baseline `fba55864`. One pinned
`/usr/bin/bazel` generation exercised the isolated self-cycle row. It exited
7 with:

`ERROR: no such package 'cycle_error': error globbing [matched-*] op=FILES:
[unix_jni.cc:382] <workspace>/cycle_error/matched-cycle.txt (Too many levels
of symbolic links)`

This materially differs from the frozen
`Symlink issue while evaluating globs: Symlink cycle:` diagnostic, so the
accepted stop gate fired. Renaming the matched cycle to `.parked` and querying
again recovered to exact `//cycle_error:probe` on the same nonzero server
epoch. The same generation also confirmed that a helper `def` in a BUILD file
is rejected; the eventual retry must use explicit or top-level comprehension
declarations inside the already-allowed BUILD file.

The failed generated expectation and every harness, fixture, BUILD, regular
asset, and symlink draft were removed. Every non-plan implementation path
returned byte-for-byte to `fba55864`; no implementation, fixture growth, Rust,
Cargo, dependency, API, DICE key, consumer, or activation was accepted. The
preserved ignored run artifact is
`target/v2o/runs/glob-directory-invalidation/20260727-034143-3965037-bazel/comparison`.
The stopped attempt does not count as post-checkpoint oracle packet three:
accepted growth after `22de3631` remains the prior two packets at +4 regular,
+3 symlinks, and +1,065 lines.

Next, design and terminally rereview only
`WP-5-m1-loading-host-dirent-glob-oracle-cycle-diagnostic-correction`.
Trace native `ELOOP` `FileSymlinkLoopException` packaging against Skyframe
logical-chain `FileSymlinkCycleException` and decide whether the retained
self-cycle pins the observed package-level form or needs a different bounded
cycle topology. Preserve the non-cycle contract, eleven rows, same-daemon
recovery, four-regular/six-link topology, existing harness/fixture allowlist,
and caps unless pinned source or a confined probe requires an explicitly
reviewed delta. Do not edit or regenerate the harness, fixture, expectation,
assets, Rust, Cargo, APIs, owners, consumers, or activation, and do not retry
implementation before terminal acceptance.

### Host-dirent glob cycle-diagnostic correction

Status: `ACCEPT` on 2026-07-27 after pinned Bazel 9.2 source tracing,
inspection of the isolated stopped-generation artifact, and independent
source/behavior, fixture/implementation, and architecture/orchestration
terminal latest-text reviews. No harness, fixture, expectation, asset, Rust,
Cargo, dependency, API, owner, consumer, or activation changed.

Keep the existing self-cycle topology and all eleven commands. Do not attempt
to force the friendlier Skyframe logical-chain diagnostic by changing the
topology or command mode. On a cold missing glob value,
`PackageFunctionWithMultipleGlobDeps.SkyframeHybridGlobber` delegates the
immediate missing result to `NonSkyframeGlobber`. `UnixGlob` follows the
wildcard-matched symlink through `statIfFound`; native `ELOOP` becomes
`FileSymlinkLoopException`, and `StarlarkNativeModule` packages that IOException
before a Skyframe restart can yield `FileSymlinkCycleException`. Upstream
friendly-cycle evidence uses keep-going evaluation, which is outside this
fixture's command contract.

Override only the accepted matched-cycle failure assertion. It remains exit 7
with empty stdout, empty manifest, and the same captured server epoch, but
stderr must contain these three stable semantic fragments:

- `error globbing [matched-*] op=FILES:`;
- `cycle_error/matched-cycle.txt`; and
- `Too many levels of symbolic links`.

Do not pin `[unix_jni.cc:382]`, an absolute path prefix, or
`Symlink issue while evaluating globs:`. Preserve the existing recovery rename
and exact `//cycle_error:probe` success on the same server. The unrelated
self-cycle remains unmatched and must not fail; the retained row therefore
still discriminates matched from unmatched resolution.

The source authority added to the fixture provenance is:

- `PackageFunctionWithMultipleGlobDeps.java:238-285,340-368`;
- `UnixGlob.java:824-870`;
- `StarlarkNativeModule.java:848-868`;
- `PackageFunction.java:536-588`;
- `FileFunction.java:309-389`;
- `unix/NativePosixFiles.java:72-118`;
- `unix/UnixFileSystem.java:158-175`;
- `vfs/FileSymlinkLoopException.java:19-31`;
- `src/main/native/unix_jni.cc:75-140,356-403`; and
- `PackageFunctionTest.java:1821-1905` plus
  `AnalysisFailureReportingTest.java:152-187` for the keep-going distinction.

The stopped BUILD helper was independently invalid: Bazel 9.2 forbids `def`
inside BUILD files. Replace it in the already-allowed `workspace/pkg/BUILD.bazel`
with exactly five top-level list comprehensions that call `filegroup`, following
the retained `glob-callable-contract` shape. Add no `.bzl` file or asset. The
first-four projection guard compares against the prepacket accepted oracle,
never the contaminated stopped-generation output.

Every other accepted design term remains exact: required POSIX/FIFO harness
surface, four regular and six exact relative symlink assets, eleven commands,
same-daemon kind transitions and recovery, empty manifests, epoch capture,
first-four pre-existing-field projection, source/growth/replay/cleanup guards,
allowlist, +900/-100 cap, and at most 550 fixture lines. The retry becomes
post-checkpoint oracle packet three only after terminal acceptance.

Next packet: retry only
`WP-5-m1-loading-host-dirent-glob-oracle`.

### Corrected Host-dirent glob oracle retry stop

Status: `REPLAN` on 2026-07-27 from clean baseline `245dfc09`. The corrected
pinned `/usr/bin/bazel` generation kept all eleven commands on server epoch 1
but exposed two further material semantic contradictions.

Rows one through four exited 7 before the FIFO setup because top-level
`glob(["specials/*"])` matched nothing and Bazel 9.2's active
`--incompatible_disallow_empty_glob` behavior defaults `allow_empty` to false.
The direct-FIFO row likewise exited 7 because files-only `glob(["state*"])`
was empty, rather than succeeding with the frozen three-label output.
Separately, the regular-to-directory row exited 0 but retained both
`//pkg:state_files_state` and `//pkg:state_all_state`, contradicting the frozen
expectation that the files-only target disappears. The baseline,
symlink-to-special, regular restoration, corrected matched-cycle failure, and
same-epoch recovery otherwise matched their corrected expectations.

Per the stop gate, no `allow_empty` flag, output, topology, or row was changed
and no replay ran. The generated expectation and every harness, fixture,
BUILD, regular-asset, and symlink draft were removed; every non-plan
implementation path returned byte-for-byte to `245dfc09`. No implementation,
fixture growth, Rust, Cargo, dependency, API, DICE key, consumer, or activation
was accepted. The preserved ignored evidence is
`target/v2o/runs/glob-directory-invalidation/20260727-035647-3971454-bazel/`.
This stopped retry still does not count as post-checkpoint oracle packet three:
accepted growth after `22de3631` remains two packets at +4 regular,
+3 symlinks, and +1,065 lines.

This is a second material semantic correction after the cycle/BUILD
correction, so the implementation packet ends instead of taking another
inline patch. Next, design and terminally rereview only
`WP-5-m1-loading-host-dirent-glob-oracle-semantics-redesign`.

Audit all five generated-target glob calls and explicitly freeze
`allow_empty` semantics for every state. Reproduce the regular-to-directory
result in isolated cold and same-daemon controls, trace why the warm row
retains `state_files_state` despite files-only operation, and distinguish true
Bazel incremental behavior from mutation or visibility artifacts. Then
refreeze every row and output, the exact topology/row count/allowlist/caps, and
whether the accepted typed-dirent observation implementation remains the next
prerequisite. A bounded reviewed delta is permitted if discriminating
cold-versus-warm evidence requires it. Permit only pinned-source inspection
and confined temporary Bazel probes. Do not edit or regenerate the harness,
fixture, expectation, assets, Rust, Cargo, APIs, owners, consumers, or
activation, and do not retry either the oracle or typed observation before
terminal acceptance.

### Host-dirent glob oracle semantic redesign

Status: `ACCEPT` on 2026-07-27 after pinned Bazel 9.2 source review, isolated
cold and same-daemon controls, a full corrected-core probe, and independent
source/behavior, fixture/implementation, and architecture/orchestration
terminal latest-text reviews. No harness, fixture, expectation, checked asset,
Rust, Cargo, dependency, API, owner, consumer, or activation changed.

The matching semantics remain source-exact. `glob` maps default
`exclude_directories != 0` to `Operation.FILES` and
`exclude_directories = 0` to `FILES_AND_DIRS`. A direct directory participates
only in files-and-directories; a direct native-special/FIFO `UNKNOWN` entry is
skipped by wildcard matching; a direct literal special participates; and a
matched symlink to an existing nondirectory participates in both operations.
`CompactSortedDirents` and `DirectoryListingStateValue` retain direct kind in
semantic equality.

The failed atomic replacement is an incremental visibility result, not an
alternate membership rule. Cold controls with explicit `allow_empty = True`
returned:

- regular `state`: `state_all_state` and `state_files_state`;
- directory `state`: only `state_all_state`;
- direct FIFO `state`: no state target; and
- symlink `state` to a FIFO: both state targets.

One-daemon controls reproduced stale regular membership after an atomic
regular-to-directory replacement even though the parent inode was stable and
its nanosecond mtime/ctime changed. Creating and deleting a nonmatching sibling
did not correct it. Querying the absent state between removal and addition did.
A one-absent 12-row control then exposed the same staleness for atomic
directory-to-symlink replacement. The final full control kept one Bazel
PID/starttime `3982243/110904713` and exited 0 throughout with exact state
sequence `regular 5 -> absent 3 -> directory 4 -> absent 3 -> symlink 5 ->
FIFO 3 -> restored regular 5`. Atomic symlink-to-FIFO and FIFO-to-regular
therefore need no additional split. Every confined server and temporary
workspace/output base was removed.

Revise the implementation contract as follows. Set `allow_empty = True`
explicitly on all five generated-target glob calls. Four calls require it in
some retained state; applying it to the always-nonempty links call makes the
contract uniform and changes no membership.

Keep the first four retained rows. Replace the seven appended rows with nine,
for exactly thirteen total:

1. `kind_baseline`: create FIFOs `pkg/specials/direct` and
   `staging/special`; expect the five baseline targets;
2. `kind_absent_after_regular`: rename `pkg/state` to `staging/file`; expect
   exactly the three non-state targets;
3. `kind_directory`: rename `staging/dir` to `pkg/state`; expect the three
   non-state targets plus `state_all_state`;
4. `kind_absent_after_directory`: rename `pkg/state` to
   `staging/dir-used`; expect exactly the three non-state targets;
5. `kind_symlink_to_special`: rename `staging/link` to `pkg/state`; expect
   the five baseline targets;
6. `kind_direct_fifo`: atomically rename `pkg/state` to
   `staging/link-used` and `staging/special` to `pkg/state`; expect exactly the
   three non-state targets;
7. `kind_regular_restored`: atomically rename `pkg/state` to
   `staging/special-used` and `staging/file` to `pkg/state`; expect the five
   baseline targets;
8. the corrected matched-cycle exit-7 row with empty stdout/manifest and only
   the three stable ELOOP fragments; and
9. the `.parked` rename plus exact recovery `//cycle_error:probe`.

All thirteen commands must capture one equal nonzero server epoch. The two
absent rows deliberately prove a reached removal followed by fresh
classification; do not claim that Bazel rechecks an atomic same-name
kind-only replacement. The typed observation implementation remains required
for snapshot classification, direct-special omission, symlink resolution,
directory inclusion/exclusion, and semantic kind equality. Its unit-level
Complete-value and repository-revalidation evidence may prove kind inequality
once an observation is recomputed, but neither it nor this oracle may cite
atomic same-name replacement as an automatic command-level invalidation
trigger.

Every other accepted term remains: one existing fixture, POSIX-only FIFO
harness extension, top-level BUILD comprehensions, four regular/six exact
relative symlink assets, empty manifests, first-four pre-existing-field
projection, corrected source anchors and ELOOP fragments, two distinct
fresh-root replays, exact cleanup/provenance/growth/credential/archive/diff
guards, and no Slug. The file allowlist is unchanged. Raise only the
newline-counted regular-file fixture cap from 550 to 750 for the two added
records; retain the packet-wide +900/-100 cap and exact four-regular/six-link
asset count. This becomes post-checkpoint oracle packet three only after
terminal acceptance.

Stop on any output or epoch difference from the confirmed 13-row matrix,
empty-glob error, any state target in an absent row, unexpected
atomic-transition staleness, cycle/recovery drift, cap or allowlist failure,
or scope expansion. Do not substitute a sibling signal, BUILD edit between
commands, restart, weakened expectation, or another row without a reviewed
replan.

Next packet: implement only
`WP-5-m1-loading-host-dirent-glob-oracle`.

### Focused Host-dirent glob oracle implementation

Status: `ACCEPT` on 2026-07-27 in `0a4aa0af` after one pinned Bazel 9.2
generation, two distinct absolute fresh-root replays, and source/parity,
native implementation/evidence, plus architecture/orchestration terminal
latest-diff reviews.

The existing fixture now has exactly thirteen commands on server epoch 1.
The state rows preserve the exact `5/3/4/3/5/3/5` matrix across the two
required absent observations, followed by the matched-cycle exit-7 row with
the three stable ELOOP fragments and exact same-daemon recovery. All manifests
are empty. The first four records preserve every prepacket field and value,
adding only `server_epoch`; their canonical projection SHA-256 is
`8b071ed78fd40d7046f2b7e4e96461b53ee85346e37d41006e22a53d4137b393`.
The checked expectation SHA-256 is
`05caae64afeca0364a5c127a5a7de46c48e67e10990384462e69d8be2c93f7b6`.

The POSIX-only harness adds strict FIFO parsing and creation plus no-follow
directory/FIFO rename support. Creation rejects collisions, escaping paths,
and missing or symlink parents, forces and verifies FIFO mode 0600, and rolls
back a failed post-create verification. The exact fixture inventory is ten
regular files, six relative symlinks, and 667 newline-counted regular-file
lines. Accepted packet growth is +669/-10 under the +900/-100 cap, with the
exact sixteen-path allowlist, four new regular assets, six new symlinks, and
twenty unique source anchors resolving at pinned Bazel commit
`8220c6198837d5c13d53fea211cf3282aa12408a`.

Generation and both fresh-root replays passed with path-free stable records;
review replays produced identical output hashes and left no Bazel, Slug,
FIFO, socket, or runner state. Focused direct assertions, Python bytecode
compilation, first-four projection, source/archive/credential/scope/growth/
symlink guards, and `git diff --check` passed. `pytest` was unavailable in
the environment (`No module named pytest`), so the focused assertions were
executed directly. All three terminal reviewers returned `ACCEPT`.

This is post-checkpoint oracle packet three after accepted checkpoint
`22de3631`; no fixture-growth checkpoint is due.

Next packet: implement only
`WP-5-m1-loading-host-dirent-observation`.

### Host typed no-follow dirent observation implementation

Status: `ACCEPT` on 2026-07-27 in `609da3e1` after tests-first implementation,
focused and full serial validation, and independent source/API,
implementation/evidence, plus architecture/orchestration terminal latest-diff
reviews.

The five-file packet atomically replaces the names-only directory observation
with `PathDirectoryEntryKind`, `PathDirectoryEntry`, and one retained
`Arc<[PathDirectoryEntry]>`. Stable raw-name-only sorting preserves duplicate
names and their input order while complete equality includes kind. The
obsolete duplicate-name error, re-export, `.names()` accessor, and names-only
constructor surface are removed without a compatibility or default-Unknown
shim.

Unix now copies `d_name` and `d_type`, directly maps regular, directory,
symlink, and known special types, closes enumeration before refining only
unknown or unrecognized types through no-follow metadata, maps missing,
ENOTDIR, ELOOP, and present special values to `Unknown`, and terminalizes other
child-stat IO. Windows preserves raw UTF-16 enumeration through close, then
uses no-follow metadata attributes plus the existing reparse-aware node-kind
classifier; special, null, and child-status IO map to `Unknown` without the
change-time query. The existing single `DirectoryEntries` demand and complete
DICE value own all classification; no child Need, key, operation, lock, or
consumer was added.

Focused regressions prove raw names, stable duplicate-kind order, separately
allocated and kind-sensitive equality, invalid/self-unequal Need, one retained
Need→File→Directory→Unknown→File lineage, complete restoration, typed listing
forwarding, repository kind-only dirtiness and equal-reorder cleanliness,
every Unix native type and unknown-status mapping after close, no-follow real
file/directory/symlink/socket lifecycles, and Windows raw-unit/status ordering.
Native Windows behavior and lone-surrogate ordering remain explicit activation
gates; the GNU-Windows result proves compilation and linkage only.

Validation passed 36 `slug_workspace_v2` tests, 109 `slug_core_v2` unit tests,
13 core integration tests, all `slug_bzlmod_v2` and `slug_loading_v2` suites
and doctests, and GNU-Windows no-run linkage for all four crates. Formatting,
diff, archive, public-API removal, dependency, process, scope, and forbidden
collection/owner guards passed. Exact additions are 506/520 in core path
observation, 18/30 in repository validation tests, 2/5 in the workspace
re-export, 144/150 in workspace path observation, and 28/45 in path-resolution
tests: +698/-139 total under the +750 addition cap. No Cargo, dependency,
fixture, loading, bzlmod, Starlark, DICE-key, consumer, or activation file
changed. All terminal reviews returned `ACCEPT`.

Next packet: design only
`WP-5-m1-loading-byte-string-oracle-feasibility-design`.

### Bazel-internal string oracle/feasibility design

Status: `ACCEPT` on 2026-07-27 after terminal pinned-source/API,
implementation-feasibility, and architecture/orchestration latest-text
reviews. No non-plan repository file, fixture, Rust, Cargo, dependency, DICE
key, loading consumer, or entrypoint changed.

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` does not decode ordinary BUILD
and `.bzl` source as Unicode before lexing. `ParserInput.fromLatin1` maps every
raw source byte to one Java U+00xx character. The lexer therefore makes a
literal UTF-8 `é` source sequence (`c3 a9`) equal to `"\303\251"` and
different from `"\351"`. Ordinary and triple string octal escapes consume
one through three octal digits greedily and reject values above `\377`.
`\x`, `\u`, `\U`, and unknown escapes are invalid; raw strings preserve the
backslash and digits. Dynamic concatenation, slicing, comparison, list/dict
storage, and macro argument/return preserve these internal byte-shaped
strings. Source authority is `StarlarkUtil.java:43-66`,
`ParserInput.java:82-95`, `Lexer.java:274-425,443-495`, and
`StringEncoding.java:25-85,116-141`, plus
`FileLocations.java:21-23,38-40,69-72,95-120` and
`Location.java:21-26` for byte-counted columns.
`PackageFunction.java:1331-1366` and `BzlCompileFunction.java:177-215`
establish BUILD and ordinary `.bzl` Latin-1 parser input;
`Parser.java:666-675,1042-1054`, `Eval.java:588-597`,
`EvalUtils.java:77-99,494-497`, and `Starlark.java:302-310,662-745`
establish the claimed dynamic operations. Live-seam authority is
`starlark_syntax/src/{lexer.rs:242-430,codemap.rs:247-253,309-320,382-416,
syntax/module.rs:203-255}` and
`starlark/src/eval/compiler/expr.rs:1044-1054`.

Live starlark-rust currently decodes both literal UTF-8 `é` and `"\351"` to
the same Rust U+00e9 scalar and also accepts the Bazel-invalid `\x`, `\u`, and
`\U` forms. Whole-source Latin-1 transcoding is forbidden: `CodeMap` owns the
original UTF-8 `String`, and expanding each non-ASCII byte into a different
UTF-8 sequence would shift byte spans and corrupt source excerpts. The
feasible seam is token-local and opt-in. It lifts every literal source UTF-8
byte to one Rust U+0000..U+00ff scalar, while an octal escape contributes one
such scalar byte. The original source and byte spans remain unchanged, while
the codemap stores an explicit opt-in byte-column reporting mode because Bazel
counts Latin-1 parser-input bytes and standard starlark-rust counts Unicode
scalars. Existing `resolve_span` and formatted rendering remain byte-for-byte
Unicode behavior; a separate opt-in reporting resolver returns Bazel byte
columns without feeding them to the Unicode-character caret renderer.
Existing AST strings and runtime strings can then preserve equality, order,
length, indexing, slicing, concatenation, multiplication, list/dict keys, and
function/macro pass-through for strings originating from opt-in literals and
opt-in-parsed pass-through. The shared runtime type cannot distinguish carrier
U+00e9 representing byte e9 from ordinary Unicode U+00e9 injected by a
standard parser, native, or global. This packet adds no ingress adapter. It is
a dormant parser carrier, not yet a filesystem-path value or Host glob owner;
production activation remains gated on separately reviewed byte-aware adapters
for every string ingress.

Split the result into two serial implementation packets with a terminal
review gate between them.

First implement only
`WP-5-m1-loading-bazel-internal-string-oracle`. Add the new Bazel-only fixture
`starlark-internal-string-bytes` in exactly six regular files and no links:

- `fixture.toml`;
- `expected/oracle.json`;
- `workspace/MODULE.bazel`;
- `workspace/BUILD.bazel`;
- `workspace/defs.bzl`; and
- `workspace/bad/BUILD.bazel`.

The fixture has exactly eight commands and no harness edit. Set fixture-level
`daemon = true` and `observe_server_epochs = true`; set command-level
`capture_server_epoch = true` on each of all eight commands. Two successful
query rows emit only exact ASCII target labels while proving:

- literal UTF-8 equals its two-octal-byte spelling and differs from one-octal
  `\351`;
- single, double, raw, triple, empty, NUL, boundary `\377`, greedy
  `\3777`/`\378`, and non-BMP source forms have the pinned
  length/equality/order behavior; and
- `.bzl` function/macro argument and return, concatenation, multiplication,
  slicing/indexing, list membership, dict-key equality, and a pattern-shaped
  returned string remain byte-shaped.

The remaining six rows repeatedly query only `//bad:*` on one Bazel server.
Five ordered text replacements in the one bad BUILD file independently pin
the failures for `\400`, `\x41`, `\u00e9`, `\U0001f600`, and `\q`; each row
must retain the generated exact diagnostic and source position. A final
replacement installs a multibyte literal plus triple-string prefix followed
by a deliberate identifier error and pins the generated exact line,
Bazel-byte-counted column, and message. The location must distinguish Bazel's
raw-byte column from starlark-rust's default Unicode-scalar column; do not
precompute a column for an AST span different from the generated diagnostic.
Every mutation must replace exactly one known prior spelling. Stop if cached
failure recovery requires a server restart or BUILD mutation outside this
single file.

Generate once with pinned `/usr/bin/bazel`, then replay the checked expectation
from two distinct absolute fresh roots. Require byte-identical normalized
records, exact ASCII stdout, empty manifests, no Slug execution, no host path,
exact server epoch 1 for all eight rows, and complete Bazel/runner cleanup.
Exact per-file addition caps are 260 for
`fixture.toml`, 750 for `expected/oracle.json`, 70 for the root BUILD, 90 for
`defs.bzl`, 25 for the bad BUILD, and one for `MODULE.bazel`; the hard packet
cap is +1,200/-0. Retain exactly six regular files, zero links, and at most
1,200 newline-counted fixture lines. This is post-checkpoint oracle packet
four after `22de3631`. This bounded packet crosses neither the at-least-100-file
nor 10,000-text-line threshold, so no fixture-growth review is due here.
Whichever oracle next reaches `ACCEPT` is packet five and must perform and
record the focused fixture-growth review.

Only after that oracle reaches terminal `ACCEPT`, implement
`WP-5-m1-starlark-bazel-internal-string-seam`. The exact future allowlist is:

- `starlark-rust/starlark_syntax/src/dialect.rs` — at most 45 additions;
- `starlark-rust/starlark_syntax/src/lexer.rs` — at most 300;
- `starlark-rust/starlark_syntax/src/codemap.rs` — at most 180;
- `starlark-rust/starlark_syntax/src/syntax/module.rs` — at most 120;
- `starlark-rust/starlark_syntax/src/lexer_tests.rs` — at most 220;
- `starlark-rust/starlark/src/syntax.rs` — at most five;
- `starlark-rust/starlark/src/tests.rs` — at most three; and
- new `starlark-rust/starlark/src/tests/bazel_internal_string.rs` — at most
  300.

The hard total cap is 1,200 additions and 100 deletions. Add one explicit
public parse-encoding enum and `AstModule::parse_with_string_encoding`;
ordinary `AstModule::parse`, `Lexer::new`, every `Dialect`, compiler/value
representation, and every production call site remain byte-for-byte.
The opt-in parser passes a crate-private lexer mode for string tokens and
stores a codemap byte-column reporting mode while retaining the original
source text, spans, existing `resolve_span`, and formatted rendering.
In that mode, preserve the Bazel octal grammar/range and reject Bazel-invalid
hex, Unicode, and unknown escapes with the oracle diagnostics. Do not add a
Dialect field, global/static/thread-local/environment switch, whole-source
pretransform, evaluator mode, or loading caller. Acceptance requires zero
non-test callers of the new parse entrypoint.

Focused seam tests compare standard and opt-in codemaps, original sources,
byte spans, source slices, unchanged ordinary resolved locations, and the
separate opt-in byte-column reporting locations, including the required
one-column UTF-8-literal difference; prove standard lexer/evaluator outputs
unchanged; and reproduce
the accepted oracle matrix in opt-in parse/evaluation, including separately
parsed equality and frozen/unfrozen function pass-through. Dict hashing is
proved only through equal-key lookup, never an absolute hash number. `chr`,
`codepoints`, numeric `hash`, canonical repr/print escaping, f-strings, and
arbitrary non-UTF-8 source files are not claimed by this first seam. If the
accepted oracle requires a compiler, runtime string, repr, numeric-hash, or
source-storage edit, stop and `REPLAN` instead of expanding the allowlist.
Formatted non-ASCII excerpt/caret parity is also excluded: mode-aware
`span_display.rs` character indexing is a separate diagnostic-display
boundary. Do not feed the opt-in byte column directly to a Unicode-character
caret renderer or claim exact formatted snippets.

Validate focused tests first, then full `starlark_syntax` and `starlark`
suites/doctests, unchanged `slug_bzlmod_v2`, `slug_loading_v2`, and
`slug_analysis_v2`, plus GNU-Windows no-run linkage for those five crates.
Run formatting, diff, archive, Cargo/dependency, public-API/default-mode,
exact allowlist/caps, credential, process, and forbidden-caller guards.

Both packets exclude actual non-UTF-8 filesystem names, `glob()` matching,
`?`/`**`/dot/subpackage/symlink/special breadth, typed-dirent edits, Host
glob/key ownership, BUILD/`.bzl` regular-or-special byte acquisition,
package-evaluation retry/suspension, DICE keys/Needs, direct filesystem IO,
consumer activation, and native-Windows parity claims. Actual raw-name and
pattern-lazy behavior remains the next oracle/design boundary after the
dormant seam is accepted.

Stop on a pinned Bazel contradiction, ordinary parser change, source/span
rewrite or failure of exact opt-in byte-column resolution, runtime
normalization of a covered dynamic operation, failure to
recover through the ordered bad-file mutations on one server, a required
compiler/value/hash/repr/source-storage edit, non-test adoption of the opt-in
API, allowlist/cap expansion, new dependency/global/interner/retained standard
collection, loading/Host/DICE scope, required mixed standard/native/global
string ingress without a reviewed byte adapter, or any claim that the carrier
alone implements Bazel path strings.

Next packet after terminal acceptance of this design: implement only
`WP-5-m1-loading-bazel-internal-string-oracle`.

### Bazel-internal string oracle implementation

Status: `ACCEPT` on 2026-07-27 in `98b8b0e1` after one corrected pinned
generation, two distinct absolute fresh-root replays, root validation, and
three terminal source/parity, implementation/evidence, and
architecture/orchestration latest-diff reviews.

The exact six-file Bazel-only fixture adds eight rows on one retained server.
Two ASCII query projections prove the accepted BUILD/`.bzl` literal and
dynamic byte-string matrix. Six cumulative unique full-line replacements then
pin the generated `\400`, `\x41`, `\u00e9`, `\U0001f600`, and `\q` failures
plus the multibyte/triple-string undefined-name diagnostic. Exact locations
are `3:17`, `3:15` four times, and the discriminating Bazel byte column
`3:24`; all exits are `0,0,7,7,7,7,7,7`, all epochs are 1, stdout is ASCII,
and every manifest is empty.

Generation and both fresh-root replays produced byte-identical normalized
projections and complete Bazel/runner cleanup. The first stopped generation
exposed only the harness's documented backslash-to-slash normalization; the
five message needles were corrected to the observed normalized forms without
changing Bazel semantics, locations, row order, or scope. The final inventory
is six regular files, zero links, and 519 added lines: 140/260 in
`fixture.toml`, 286/750 in the generated oracle, 14/70 in the root BUILD,
74/90 in `defs.bzl`, 4/25 in the bad BUILD, and 1/1 in `MODULE.bazel`.
No harness, Slug, parser, Cargo, dependency, Host, loading, DICE, consumer,
raw-name, or `glob()` surface changed. All terminal reviews returned
`ACCEPT`.

This is post-checkpoint oracle packet four after `22de3631`; no fixture-growth
review is due for this packet. Whichever oracle next reaches `ACCEPT` is
packet five and must perform and record the focused fixture-growth review.

Next packet: implement only
`WP-5-m1-starlark-bazel-internal-string-seam`.

### Bazel-internal string parser seam implementation

Status: `ACCEPT` on 2026-07-27 in `c8f13ee9` after focused and broad root
validation plus three terminal source/parity, implementation/evidence, and
architecture/orchestration latest-diff reviews.

The exact eight-file dormant seam adds public `StringEncoding` and
`AstModule::parse_with_string_encoding`, a crate-private token-local byte
lexer, and a separate opt-in byte-column reporting resolver. Original UTF-8
source, byte spans, ordinary `AstModule::parse`, `Lexer::new`, every
`Dialect`, `resolve_span`, formatted rendering, compiler/value representation,
and every production call site remain unchanged. Tests prove the accepted
literal, escape, diagnostic, source/span/location, dynamic carrier,
cross-parse runtime equality, equal-key lookup, and frozen/unfrozen function
pass-through matrix. The new entrypoint has zero non-test callers.

Focused lexer and runtime tests passed 2/2 and 5/5. Full
`starlark_syntax` plus doctest, `starlark` doctests, all unchanged bzlmod,
loading, and analysis suites, and five-crate GNU-Windows no-run linkage
passed. The current broad Starlark suite passes 822 tests after filtering the
same 11 golden/profile failures reproduced by exact name and signature in an
isolated clean `b51de40d` worktree, where 817 tests pass; the five-test delta
is exactly this packet. Formatting, diff, archive, dependency, default/API,
caller, credential, process, scope/cap, and utility-boundary guards passed.
Final scope is +696/-2, with every per-file cap satisfied and the new test at
299/300 additions. No fixture or oracle changed, so the post-checkpoint oracle
count remains four.

Next packet: design only
`WP-5-m1-loading-raw-name-pattern-lazy-glob-oracle-design`. Its eventual
oracle implementation is packet five and must perform and record the focused
fixture-growth review before terminal acceptance.

### Raw-name pattern-lazy glob oracle design

Status: `ACCEPT` on 2026-07-27 after pinned Bazel source and native behavior,
harness/fixture feasibility, and architecture/orchestration terminal
latest-text reviews. No harness, fixture, generated expectation, Rust, Cargo,
dependency, DICE key, loading caller, Host owner, consumer, or entrypoint
changed.

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` preserves Linux directory-name
bytes through `unix_jni.cc:480-540,545-615` and
`latin1_jni_path.cc:26-44,78-129`, then exposes them as internal Latin-1
strings under `StringEncoding.java:25-85,100-141,145-162`.
`StarlarkNativeModule.java:92-131` sorts final BUILD `glob()` results by those
internal string values. The accepted typed listing and 13-row POSIX
Host-dirent oracle already own direct kind, literal-versus-wildcard special
handling, matched good/dangling/cyclic symlinks, unrelated dangling/cyclic
symlinks, ELOOP recovery, and same-daemon kind transitions. Source authority
is `Dirent.java:20-31,34-75`,
`DirectoryListingStateValue.java:84-106`,
`PatternWithWildcardProducer.java:89-139,164-210`,
`PatternWithoutWildcardProducer.java:68-97`, and
`PackageFunctionWithMultipleGlobDeps.java:228-293`.

The new evidence corrects two historical assumptions. BUILD `glob()` rejects
every `?` before matching even though the lower `UnixGlob.matches` helper can
interpret it; authority is `GlobValue.java:40-57`,
`GlobsValue.java:108-124`, and `GlobCache.java:218-238`, and pinned native
observation exits 7 with
`Error in glob: invalid glob pattern '?.txt': wildcard ? forbidden`.
Also, this raw-name boundary is Linux-only: macOS path conversion reencodes
Unicode and cannot share a generic POSIX claim. Linux bytes such as
`ed a0 80` are not evidence for a native Windows UTF-16 lone surrogate.

Implement only
`WP-5-m1-loading-raw-name-pattern-lazy-glob-oracle` in these exact ten paths:

- `tools/v2_oracle_lib/fixture.py` — at most 120 additions;
- `tools/v2_oracle_lib/runner.py` — at most 180 additions;
- `tests/v2_oracle/test_v2_oracle.py` — at most 260 additions;
- new
  `tests/v2_oracle/fixtures/glob-raw-name-pattern-lazy/fixture.toml` — at
  most 220 additions;
- new
  `tests/v2_oracle/fixtures/glob-raw-name-pattern-lazy/expected/oracle.json`
  — at most 420 additions;
- new
  `tests/v2_oracle/fixtures/glob-raw-name-pattern-lazy/workspace/MODULE.bazel`
  — exactly one addition;
- new
  `tests/v2_oracle/fixtures/glob-raw-name-pattern-lazy/workspace/pkg/BUILD.bazel`
  — at most 90 additions;
- new
  `tests/v2_oracle/fixtures/glob-raw-name-pattern-lazy/workspace/pkg/defs.bzl`
  — at most 30 additions;
- new UTF-8-named
  `tests/v2_oracle/fixtures/glob-raw-name-pattern-lazy/workspace/pkg/é.txt`
  — at most five additions; and
- new
  `tests/v2_oracle/fixtures/glob-raw-name-pattern-lazy/workspace/qmark/BUILD.bazel`
  — at most ten additions.

The hard packet cap is +1,200/-100. The fixture retains exactly seven regular
files, zero links, at most 750 newline-counted lines, and no manifest roots.
Do not modify or narrow the accepted POSIX
`glob-directory-invalidation` fixture. The new one-line MODULE and two-package
scaffold are justified by Linux-only raw-name isolation and independent `?`
failure containment; copy no symlink, special-file, registry, module, or
subpackage topology.

Extend `required_host_os` with exact value `"linux"` while preserving absent
and `"posix"` behavior. Reject a Linux-only fixture before run-directory or
workspace creation unless `sys.platform == "linux"`. Add only two mutation
operations:

- `raw_create` takes an ASCII relative real-parent `path`, a nonempty
  canonical lowercase even-length hexadecimal `name_bytes_hex`, and UTF-8
  `content`; and
- `raw_delete` takes the same parent/hex identity and removes only an existing
  regular file.

The decoded final component must reject NUL, slash, empty, `.`, and `..`.
Reject unsupported fields, noncanonical or invalid hex, symlink or missing
parents, workspace escape, create collisions, wrong-kind deletes, and every
non-Linux execution. Build and operate on the final path as bytes without
decoding it through `Path`, Unicode, or a lossy display. Evidence records only
the ASCII parent and canonical hex. Do not add raw rename, directory, symlink,
FIFO, arbitrary full-path, fixture setup, or Windows-code-unit support.
Focused harness tests must prove all validation, parent/escape/collision/kind
failures, exact byte creation/deletion, ASCII-only records, unchanged existing
mutations, and the pre-copy Linux gate.

The fixture has exactly four Bazel-only commands on one retained daemon. Set
`daemon = true`, `observe_server_epochs = true`,
`required_host_os = "linux"`, and capture the server epoch on every row:

1. `raw_names_baseline` creates final-component bytes `e9 2e 74 78 74` under
   ASCII parent `pkg`, alongside the checked UTF-8 filename `é.txt` whose
   bytes are `c3 a9 2e 74 78 74`. The exact positive calls are
   `glob(["*.txt"], allow_empty = True)`,
   `glob(["é*.txt"], allow_empty = True)`,
   `glob(["\351*.txt"], allow_empty = True)`, and
   `glob([dynamic_pattern()], allow_empty = True)`, where loaded `defs.bzl`
   returns `"\303" + "\251*.txt"` and validates that result as
   `["\303\251.txt"]`. Exact ASCII conditional target labels prove raw-byte
   ordering and each distinct literal, octal, and dynamic match.
2. `question_mark_is_forbidden` queries isolated package `//qmark:*`, exits
   7 with empty stdout, and pins the exact stable error fragment above.
3. `raw_e9_deleted` deletes only the generated `e9.txt` and succeeds with
   exact ASCII conditional labels proving the UTF-8 name remains visible,
   ordered alone, and matched by the literal/dynamic patterns while the
   one-octal result is empty.
4. `raw_e9_recreated` recreates the same bytes and content and reproduces the
   complete baseline stdout byte-for-byte.

Require server epoch 1 on all four rows, empty manifests, ASCII stdout and
normalized diagnostics, no raw filename in JSON or text evidence, and no Slug
execution. Generate once with pinned `/usr/bin/bazel`, then replay the checked
expectation from two distinct absolute fresh roots. Require byte-identical
normalized projections, exact mutation records, and complete Bazel/runner
cleanup. Stop if the proposed loaded `.bzl` round trip differs from the
observed BUILD-only dynamic result, a raw transition restarts the server, or
an existing accepted fixture or record changes.

Do not add positive `?` rows. Standalone `**`, dot matching, historical
parenthesis behavior, brackets/braces/backslash breadth, and subpackage
boundaries are orthogonal matcher-language evidence and remain source-derived
or owned by existing fixtures, not this raw-segment packet. Full symlink
retarget breadth, Host equality/invalidation, speculative print suppression,
transactional evaluator retry, regular-or-special BUILD/`.bzl` byte
acquisition, production parser activation, mixed native/global byte ingress,
native Windows/raw UTF-16/lone-surrogate ordering, and reparse parity also
remain separate. This packet adds no Rust, DICE key or Need, Host owner,
loading caller, event assertion, direct production IO, dependency, or Stage 9
import. The dormant parser entrypoint retains zero production callers.

This is the fifth oracle packet after accepted fixture-growth baseline
`22de3631`, following `d262052d`, `c67dc3a5`, `0a4aa0af`, and `98b8b0e1`.
Before terminal oracle acceptance, compare tracked archives at `22de3631` and
the new implementation commit; record exact regular-file, symlink, and
newline counts by affected fixture and packet; review every retained row,
asset, mutation, manifest/expected field, and repeated subtree across the
22-row nonroot fixture, 13-row POSIX glob fixture, eight-row string fixture,
and new four-row raw fixture; and record the exact pruning allowlist and
affected replay set or `none` in the oracle-harness owner plan. Do not schedule
a sixth oracle until that focused review and its terminal reviews accept.

Stop on any pinned Bazel contradiction, server restart, unstable or non-ASCII
projection, lossy filename decoding, macOS/Windows claim, broader raw mutation
API, change to accepted POSIX evidence, Slug/Rust/DICE/loading/Host/consumer
scope, new dependency or imported V1/Buck code, cap/allowlist expansion, or
inability to complete the mandatory fixture-growth review.

Next packet after terminal acceptance of this design: implement only
`WP-5-m1-loading-raw-name-pattern-lazy-glob-oracle`. After that oracle and its
packet-five growth review accept, design only
`WP-5-m1-loading-pure-host-glob-owner-design`; transactional evaluator retry
remains later and separate.

### Raw-name pattern-lazy glob oracle implementation

Status: `ACCEPT` in `03684d84` with hygiene correction `e2cc891d` on
2026-07-27 after the mandatory fifth-packet fixture-growth checkpoint and
terminal source/parity, implementation/evidence, and
architecture/orchestration latest-diff reviews.

The exact ten implementation paths changed by +660/-7, within every per-file
and hard cap. The harness adds only Linux `required_host_os` and
final-component `raw_create`/`raw_delete`; byte-oriented no-follow directory
traversal rejects non-ASCII, missing, symlinked, or escaping parents,
noncanonical or special-component hex, collisions, and wrong-kind or
symlink deletes. Existing mutation behavior and its FIFO diagnostic remain
unchanged. Focused tests prove exact raw-byte creation/deletion, ASCII records,
all validation boundaries, preserved symlink targets, and pre-copy Linux
rejection.

The new fixture contains exactly seven regular files, zero links, and 261
newline-counted lines. Its four exact-comparison Bazel-only rows produce exits
`0/7/0/0`, server epochs `1/1/1/1`, and empty manifests. ASCII conditional
labels distinguish the checked `c3 a9` name from generated `e9`, the four
accepted wildcard calls, deletion, and byte-for-byte restoration; the
isolated package records the exact BUILD `?` rejection. Unit tests and the
reviewed generated JSON prove exact ASCII parent/hex mutation records without
serializing the raw filename.

Pinned Bazel 9.2 generation, two distinct absolute fresh-root exact replays,
an additional final exact replay, and the unchanged 13-row POSIX fixture
replay passed with complete daemon cleanup. The normalized projection hash
for the two independent implementation replays was
`f3dc914229fee7bd6ab38c2ed78284ea25876f8737fe8896fe8555225e217f83`.
The focused 32-test slice, full 97-test oracle harness, Python compilation,
diff/archive, path/cap, ASCII-evidence, process, and unchanged-fixture gates
passed; pytest emitted only the pre-existing unknown `asyncio_mode` warning.

Implementation reviews corrected the first draft to the exact wildcard calls and dynamic
helper, exact fixture comparison, the 30-line `defs.bzl` cap, unchanged FIFO
diagnostic, both no-follow symlink cases, and explicit `UnixGlob.java`
provenance. The required checkpoint from `22de3631` through implementation
`03684d84` and hygiene correction `e2cc891d` is recorded in the oracle-harness
owner plan: 1,303/16/36,985 grew to 1,314/24/39,304, or +11 regular files,
+8 symlinks, and +2,319 lines across 47 retained rows. The exact pruning
allowlist removed nine unused registry package markers, the unaddressed POSIX
root BUILD, and one redundant unmatched dangling link. All four affected
fixtures replayed; the exact post-prune replay set was
`nonroot-interim-module-graph` and `glob-directory-invalidation`.

Next packet: design only
`WP-5-m1-loading-pure-host-glob-owner-design`. Keep transactional package
evaluation retry/suspension, regular-or-special BUILD/`.bzl` byte
acquisition, parser activation, native Windows/lone-surrogate evidence, and
all consumer activation separate.

### Pure Host glob owner design

Status: `ACCEPT` on 2026-07-27 after terminal pinned-source/API,
implementation/evidence, and architecture/orchestration latest-text reviews.
No non-plan repository file, Rust, Cargo, dependency, fixture, DICE key,
loading consumer, parser entrypoint, or event owner changed.

The requested recursive package-aware owner is not yet a truthful
implementation packet. Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` separates one-fragment
classification from package-boundary ownership. Literal fragments resolve one
exact `FileValue`; wildcard fragments consume one typed
`DirectoryListingValue`, skip direct `UNKNOWN`, and resolve only matched
symlinks
(`packages/producers/PatternWithoutWildcardProducer.java:68-97` and
`PatternWithWildcardProducer.java:89-216`). Only after a directory candidate
is known does `DirectoryDirentProducer.java:76-168` apply ignored-directory
and `PackageLookupValue` boundaries before accepting or descending.

Slug does not yet expose an equivalent boundary. The private
`HostRootPackageLookupKey` collapses repository-ignore and
`--deleted_packages` into `Deleted`, although Bazel stops before lookup for an
ignored directory and continues through a deleted package. It also has no
equivalent of the nested `LocalRepositoryLookupValue` incorrect-repository
stop. Exporting that key or probing `BUILD` markers from loading would
therefore be wrong. Full multi-segment and standalone-`**` traversal remains
blocked on a separate public opaque Host package-boundary projection.

The accepted smallest pure owner is instead the exact fragment-candidate
producer, named `HostGlobSegmentCandidatesKey`. Its identity is one normalized
absolute logical Host directory and one validated raw-byte segment pattern.
It contains no workspace/package identity, include or exclude list,
`allow_empty`, files-only versus files-and-directories operation, evaluator
attempt, event batch, or consumer state. Its value is
`SourcePreparationOutcome<Arc<Result<HostGlobSegmentCandidates,
HostGlobSegmentError>>>`; every Need is invalid and self-unequal, while only
complete semantic values or errors compare equal.

The private byte carrier retains `Arc<[u8]>` with raw-byte equality, hashing,
and ordering. It is not constructed from ordinary Rust UTF-8 `.as_bytes()`:
the later dormant parser adapter must project each Latin-1 scalar to one byte.
The implementation packet is Unix-only for production behavior and uses
`OsStrExt` without lossy conversion. On non-Unix it returns a typed dormant
unsupported-host result before converting a name; GNU-Windows compilation
proves shape only. Native Windows platform-to-internal conversion and
lone-surrogate ordering remain activation gates.

This first candidate owner accepts only the already-oracled simple segment
language. A literal segment has no `*`. A simple wildcard segment contains one
or more non-adjacent `*`; every other accepted byte is literal. Its constructor
first performs Bazel's full-pattern validation, then classifies supported
versus deferred syntax. `?` is rejected first; empty, leading `/`, empty path
segments, `.`/`..` segments, and embedded `**` are Bazel-invalid. Standalone
`**`, an otherwise valid multi-segment pattern, NUL, parentheses, brackets,
braces, and backslash are Bazel-valid but deferred by this candidate owner.
Thus `a/b` is deferred multi-segment syntax, while `/a`, `a//b`, `a/../b`,
and `a/**x` retain their distinct Bazel-invalid classifications. This
distinction is required:
`GlobValue.java:41-57`, `GlobsValue.java:108-124`, and
`UnixGlob.java:170-190,212-313` make `?` forbidden but permit several of the
deferred characters and give parentheses historical wildcard behavior.
Broader grammar needs a discriminating oracle before implementation. Within
the simple language, matching is byte-native and exact: a bare `*` matches
every nonempty name including a leading-dot name, any other wildcard pattern
requires an explicit leading dot, and each permitted non-adjacent `*` has
zero-or-more-byte meaning. This covers the accepted `*.txt`, raw-prefixed,
`state*`, `specials/*`, and `*-match.txt` evidence without claiming
multi-segment or `**` ownership. Before Rust, strengthen a pinned Bazel 9.2
oracle to discriminate bare `*` including a leading-dot name, `*.txt`
excluding it, an explicit-dot wildcard including it, and multiple
non-adjacent stars in one segment.

Literal computation bypasses a directory listing and computes only
`ResolvedPathKey(PathObservationNamespace::Host, logical_child)`. Missing or
dangling is omitted; a present directory becomes a `Directory` candidate;
every present regular or special terminal becomes `NonDirectory`. Wildcard
computation lists the logical directory exactly once through
`PathDirectoryListingKey`, skips direct `Unknown`, byte-matches raw names,
classifies direct file and directory entries inline, and computes
`ResolvedPathKey` only for matched symlinks. A dangling matched symlink is
omitted. A matched symlink to a directory becomes `Directory`; a matched
symlink to a regular or special terminal becomes `NonDirectory`, except that
wildcard resolution of a directory with
`ResolvedPath::ancestor_expansion()` is immediately an infinite-expansion
error even for a non-`**` pattern, matching
`PatternWithWildcardProducer.java:181-197`. A wildcard non-directory with
ancestor-expansion metadata remains `NonDirectory`; the literal producer has
no equivalent check, so a literal directory remains a candidate. If the typed
listing said symlink but the resolution route did not resolve that logical
child as a symlink, return an inconsistent-state error.
`PathDirectoryListing::Missing` is likewise a complete semantic
directory-disappeared inconsistent-state error, not an empty match: Bazel's
`DirectoryListingFunction.java:30-56` assumes the reached base is a directory
and errors if it is no longer one. Unmatched dangling, cyclic, or infinitely
expanding symlinks create no dependency and cannot fail.

`HostGlobSegmentCandidate` retains the raw component and
`HostGlobSegmentCandidateKind::{NonDirectory, Directory}`. The result is one
shared `Arc<[HostGlobSegmentCandidate]>`, raw-name sorted and preserving the
typed listing's stable same-name order; this intermediate projection does not
claim final Bazel result-set deduplication or package-boundary acceptance.
Construction may use a temporary `Vec`; no retained standard
`HashMap`/`HashSet`/`String`/`Vec`, compact frontier map/set, interner, regex,
or matcher dependency is justified for one segment.

Convert operational resolver failures to exact semantic variants. Observation
retains logical directory, raw component, operation, and
`PathObservationError`. Inconsistent state retains logical directory, raw
component, operation, and before/after `PathLstat`. Cycle and infinite
expansion retain only logical directory plus raw component.
`ListingSymlinkResolutionMismatch` retains exactly the logical directory and
raw component; `DirectoryDisappeared` retains exactly the logical directory.
Directory-listing errors may retain the existing semantic
`PathDirectoryListingError`. No error retains namespace, physical root/path,
materialization identity, route, symlink chain, or a lossy rendering. DICE
infrastructure failures remain invariant panics. Every matched symlink in the
reached horizon is batched and every reached path Need is unioned. If any
complete error was reached, return the first one in stable raw-name and stable
same-name order even when sibling Needs exist; this matches
`GlobsFunction.java:210-215`, which handles a recorded glob error before an
incomplete computation restarts. Return the invalid unioned Need only when no
complete error exists.
There is no direct filesystem IO, blocking, lock across a DICE computation,
global cache, event, `PathObservationKey`, legacy `WorkspaceDirectoryKey`,
eager `PackageListingKey`, package lookup, or speculative dependency beyond a
Need horizon.

The implementation allowlist is exactly:

- `app/slug_loading_v2/src/host_glob/mod.rs`, at most 800 additions;
- `app/slug_loading_v2/src/host_glob/tests.rs`, at most 900 additions; and
- `app/slug_loading_v2/src/lib.rs`, at most two additions for a private module
  declaration only.

The hard packet cap is +1,750/-20. No Cargo, lockfile, public re-export,
workspace, bzlmod, fixture, existing `glob.rs`, parser, evaluator, package,
consumer, or entrypoint file may change.

The named Rust packet is
`WP-5-m1-loading-pure-host-glob-segment-candidates-owner`; its three-file
allowlist is not authorized until the matcher oracle below accepts.

Focused tests must prove all key dimensions; raw `c3 a9` versus `e9`
identity, matching, and ordering; literal versus simple-wildcard dispatch;
the exact invalid/deferred/supported constructor matrix including `a/b`,
`/a`, `a//b`, `a/../b`, and `a/**x`; bare-star and leading-dot behavior;
direct file/directory/unknown and
literal-special classification; matched file, directory, dangling, cycle, and
infinite symlinks; wildcard directory ancestor-expansion failure versus
wildcard non-directory and literal-directory acceptance; no demand for
unmatched dangling/cycle entries; union of all reached matched-symlink Needs;
complete-error precedence over a mixed error-plus-Need horizon; semantic error
equality; invalid and self-unequal Need; equal Complete pruning;
create/delete/recreate, kind change/restoration, symlink retarget/error
recovery, and equal restoration;
stable duplicate-name propagation; zero event storage; and zero non-test
callers. Validate focused loading tests, the full loading suite and doctests,
workspace and bzlmod downstream suites, formatting/diff/allowlist/cap/archive/
credential/process/forbidden-surface guards, and GNU-Windows no-run linkage.

Stop if implementation needs a fourth file, a new dependency, direct IO,
lossy conversion, unmatched resolution, package-boundary logic, multi-segment
or `**` traversal, broader syntax, a public export, a loading consumer, parser
activation, evaluator retry, native-Windows parity, or an event batch.

Next packet: design only
`WP-5-m1-loading-host-glob-segment-matcher-oracle-design`. Freeze the smallest
oracle-only extension that proves bare-star/leading-dot, explicit-dot, and
multiple-non-adjacent-star behavior without changing Rust. Implement and pin
that oracle before
`WP-5-m1-loading-pure-host-glob-segment-candidates-owner`.

After the candidate owner accepts, design only
`WP-5-m1-loading-host-package-boundary-projection-design`. That prerequisite
must separately preserve selected package root, ordinary no-package, deleted
package (continue), ignored directory (stop), actual subpackage (stop),
and operational error/Need propagation. Pinned Bazel 9.2's
`LocalRepositoryLookupFunction` always returns the main repository, so a
nested `MODULE.bazel` is not a repository boundary and must continue;
the retained incorrect-repository producer branch is unreachable and does
not authorize a Slug detector.
Only after it accepts may a later owner compose multiple segments,
standalone `**`, package boundaries, operation filtering, and final unique
results. Regular-or-special BUILD/`.bzl` acquisition, parser byte ingress,
transactional evaluator retry/suspension, include/exclude and `allow_empty`
composition, callable formatting, and consumer publication remain later.
No fixture-growth checkpoint or Stage 9 edit is due for this design-only
packet. After implementation acceptance, add one Stage 4 landed subsection
for the private Host segment-candidate owner, citing Bazel `8220c619…`, Buck
baseline `088c75…` only for already-approved compact utility patterns, V1
`e218054…` as rejected/reference-only, and the V2 implementation/evidence
commits. That record authorizes no new Buck/V1 extraction and does not claim
consumer activation.

### Simple Host glob segment matcher oracle design

Status: `ACCEPT` on 2026-07-27 after terminal pinned-source/behavior,
fixture/implementation, and architecture/orchestration latest-text reviews.
No fixture, generated expectation, Bazel process, harness, Rust, Cargo, DICE
key, parser, Host owner, loading consumer, event owner, or Stage 9 row changed.

Implement only
`WP-5-m1-loading-host-glob-segment-matcher-oracle` by extending the existing
pinned Bazel 9.2 `glob-callable-contract` fixture. Add one isolated
`//segment_matcher` package and one fifth query command. Do not create a new
fixture or change `glob-directory-invalidation`,
`glob-raw-name-pattern-lazy`, or `glob-package-boundaries`.

The exact implementation allowlist is:

- `tests/v2_oracle/fixtures/glob-callable-contract/fixture.toml`, at most
  +20/-8;
- `tests/v2_oracle/fixtures/glob-callable-contract/expected/oracle.json`,
  generated only, at most +45/-12;
- new
  `tests/v2_oracle/fixtures/glob-callable-contract/workspace/segment_matcher/BUILD.bazel`,
  at most 30 lines;
- new
  `tests/v2_oracle/fixtures/glob-callable-contract/workspace/segment_matcher/.hidden.txt`,
  exactly one newline-terminated line; and
- new
  `tests/v2_oracle/fixtures/glob-callable-contract/workspace/segment_matcher/m-left-id-right-end.txt`,
  exactly one newline-terminated line.

The hard aggregate cap is +100/-25. This adds exactly three regular files, no
symlinks, and one command. No MODULE, `.bzl`, Python, mutation, manifest
capture, server-epoch capture, Rust, Cargo, parser, workspace observation,
package boundary, Host owner, consumer, or other fixture path may change.

Update the fixture description and notes only enough to include simple
terminal-segment matcher membership while retaining broader pattern syntax,
ordering, raw names, symlinks, invalidation, package boundaries, and query
semantics as out of scope. Add pinned commit
`8220c6198837d5c13d53fea211cf3282aa12408a` anchors for:

- `src/main/java/com/google/devtools/build/lib/vfs/UnixGlob.java:212-248`,
  where exact `*` returns before the leading-dot guard, `*.suffix` runs after
  it, and other wildcard patterns use the general matcher;
- `UnixGlob.java:257-312`, where each non-adjacent `*` becomes `.*`;
- `src/test/java/com/google/devtools/build/lib/skyframe/GlobTestBase.java:248-285`,
  covering middle and multiple non-adjacent stars; and
- `GlobTestBase.java:499-511`, where bare `*` includes hidden names and
  `BUILD` while a non-dot-prefixed suffix pattern excludes leading-dot names.

The new BUILD file uses four top-level list comprehensions and no local
`def`. For every result, declare a `filegroup` whose name is the prefix plus
the returned path and whose `srcs` contains that path:

- `glob(["*"])` with prefix `bare_`;
- `glob(["*.txt"])` with prefix `txt_`;
- `glob([".*.txt"])` with prefix `dot_`; and
- `glob(["m*id*end.txt"])` with prefix `multi_`.

Do not use `exclude` or `allow_empty`; every intended positive disappears
loudly if its asset or matcher behavior is wrong. The visible filename forces
both non-adjacent stars to consume nonempty spans. The earlier raw-name oracle
already proves a single star consuming an empty span. Query sorting owns only
the stable evidence projection; this packet claims matcher membership, not
the order returned by `glob()`.

Append exactly:

```toml
[[commands]]
name = "simple_segment_matcher"
argv = ["query", "//segment_matcher:all"]
expected_exit = 0
stdout_patterns = ["\\A//segment_matcher:bare_\\.hidden\\.txt\\n//segment_matcher:bare_BUILD\\.bazel\\n//segment_matcher:bare_m-left-id-right-end\\.txt\\n//segment_matcher:dot_\\.hidden\\.txt\\n//segment_matcher:multi_m-left-id-right-end\\.txt\\n//segment_matcher:txt_m-left-id-right-end\\.txt\\Z"]
```

The six exact labels jointly prove that bare `*` includes the leading-dot
file and the package BUILD file; `*.txt` includes the visible file but excludes
the leading-dot file; the explicit-dot wildcard includes it; and the
multiple-non-adjacent-star pattern matches with both spans nonempty. Retain
the fixture's `message_shape` comparison. The new record must exit zero, have
the exact normalized stdout, no mutation, and an empty manifest. The existing
four command definitions remain byte-for-byte unchanged; regenerated raw
run fields may vary, but their names, argv, exits, normalized stdout,
diagnostic claims, and manifests must retain the accepted semantic
projection.

Generate once with pinned `/usr/bin/bazel`, then replay the complete five-row
fixture from two distinct fresh absolute roots. Validate the exact fifth
record and the protected first-four projection; run the focused and full
oracle harness tests, Python compilation, fixture/schema/listing, exact
allowlist and per-file/aggregate caps, archive inventory, diff, credential,
process/daemon-cleanup, unchanged-protected-fixture, no-Slug, and
forbidden-surface guards. Do not read or record the user's Bazel RC.

At implementation closeout, record the measured line delta rather than an
estimate in the oracle-harness owner plan. The accepted fixture currently has
17 regular files, zero symlinks, 228 newline-counted lines, and four rows; the
whole fixture tree at checkpoint `e2cc891d` has 1,314 regular files, 24
symlinks, and 39,304 lines. This implementation is oracle packet one after
that checkpoint and adds three regular files, zero symlinks, and one row. It
remains below five packets, roughly 100 files, and 10,000 lines, so no
fixture-growth review is due.

Stop on any source contradiction, unexpected label, nonempty manifest,
fixture-command mutation, cap or allowlist expansion, protected-fixture
change, harness/Rust/Slug scope, broader matcher syntax, raw-name or
Windows claim, package-boundary or invalidation claim, new dependency, or
credential/process leak.

After terminal oracle acceptance, implement only the already-accepted private
three-file
`WP-5-m1-loading-pure-host-glob-segment-candidates-owner`. Package-boundary
projection, multi-segment and `**` traversal, regular-or-special BUILD/`.bzl`
acquisition, parser activation, transactional evaluator retry, and every
consumer remain later and separate. No Stage 9 edit is due for this design or
oracle-only implementation.

### Simple Host glob segment matcher oracle implementation

Status: `ACCEPT` in `9f42c3e5` on 2026-07-27 after terminal source/parity,
implementation/evidence, and architecture/fixture-hygiene latest-diff
reviews.

The exact five implementation paths changed by +86/-8, within every per-file
and aggregate cap. The isolated package contains exactly a 28-line BUILD and
two one-line assets. Its four direct comprehensions generate the exact six
query labels proving bare-star hidden and BUILD membership, ordinary suffix
hidden exclusion, explicit-dot inclusion, and two nonempty non-adjacent star
spans. The fifth record exits zero with no mutations and an empty manifest;
the accepted semantic projection of the first four callable records is
unchanged.

Pinned Bazel 9.2 generation and two distinct fresh-absolute-root callable
replays passed. The protected 13-row POSIX and four-row Linux raw-name
fixtures were unchanged and both replayed successfully. The 97-test oracle
harness passed through the repository test requirements; direct schema,
Python compilation, expected-projection, fixture listing, archive, scope/cap,
credential, process cleanup, and diff guards passed. No Slug command ran.

The callable fixture is now 20 regular files, zero symlinks, 306
newline-counted lines, and five rows, a delta of +3/+0/+78/+1 row. The whole
fixture tree is 1,317 regular files, 24 symlinks, and 39,382 lines. This is
oracle packet one after checkpoint `e2cc891d`, below every review trigger, so
no fixture-growth checkpoint is due.

### Private Host glob segment-candidate owner implementation

Status: `ACCEPT` in `bd12c015` on 2026-07-27 after terminal source/parity,
implementation/evidence, and architecture/orchestration corrected
latest-diff reviews.

The exact three implementation paths changed by +1,605/-0:
`host_glob/mod.rs` +715, `host_glob/tests.rs` +889, and the private `lib.rs`
module declaration +1. The Unix-dormant key retains a validated raw-byte
literal/simple-`*` pattern, stable raw-name candidates and direct kinds,
matched-only symlink resolution, wildcard-directory ancestor-expansion
failure, semantic resolver errors, complete-error-before-Need precedence, and
complete-only DICE equality. It has zero production callers, public exports,
events, direct IO, locks, package-boundary logic, parser/evaluator activation,
or new dependencies.

Focused 19, full loading 73, workspace 36, and bzlmod 387 tests passed,
including all doctests. GNU-Windows no-run linkage passed for all 20 loading,
workspace, and bzlmod executables. Formatting, diff, exact scope/per-file/
aggregate caps, archive, credential, forbidden-surface, zero-caller, and
process guards passed.

Reviews corrected physical matched-child identity beneath a symlinked base,
made an unresolved terminal symlink an invariant failure, and added retained
DICE evidence that equal Complete retargets prune a dependent consumer while
the Host key stores no evaluation data. The final regression set additionally
proves raw identity/order, syntax taxonomy, leading-dot and multi-star
matching, direct and symlink kinds, dangling/cycle/expansion behavior,
Need union and error precedence, semantic error equality, duplicate
propagation, and create/delete/recreate, kind, retarget, error-recovery, and
restoration transitions.

Next packet: design only
`WP-5-m1-loading-host-package-boundary-projection-design`. It must preserve
selected root, ordinary no-package, deleted-package continue,
ignored-directory stop, actual-subpackage stop, and typed error/Need
propagation before any recursive glob traversal or consumer activation. A
nested `MODULE.bazel` without a BUILD marker continues under pinned Bazel 9.2;
do not invent an incorrect-repository detector.

### Public Host root-package boundary projection design

Status: `ACCEPT` on 2026-07-27 after terminal pinned-source/behavior,
public-API/evidence, and architecture/orchestration corrected latest-text
reviews. No fixture, generated expectation, Bazel process, Rust, Cargo, DICE
key, loading consumer, parser, event owner, or new Stage 9 landed subsection
changed. The existing private Host glob owner's false nested-repository
residual was corrected to the pinned Bazel 9.2 behavior.

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` corrects the prerequisite stated
by the earlier pure-owner design. For one directory candidate,
`DirectoryDirentProducer.java:76-116` tests the repository ignore matcher
first and stops without requesting package lookup on a match. Otherwise it
looks up the candidate `PackageIdentifier`, stops for a successful package,
and continues for every unsuccessful lookup, including deleted,
no-BUILD-file, invalid-name, and repository-not-found values. The unsuccessful
taxonomy and `packageExists() == false` contract are in
`PackageLookupValue.java:49-68,220-389`.

`PackageLookupFunction.java:91-122,157-180,182-325` validates the package name,
applies `--deleted_packages`, handles `//external`, loads repository-ignore
policy, and then searches package-path roots in root-major and marker-minor
order: within each root `BUILD.bazel` precedes `BUILD`, while an earlier-root
`BUILD` precedes a later-root `BUILD.bazel`. A regular or special marker,
including one reached through a final symlink, selects that root and marker;
a missing or directory marker continues
(`src/main/java/com/google/devtools/build/lib/actions/FileValue.java:49-82,
113-120`). `GlobsFunction.java:113-139,210-216` loads ignore policy before
traversal and gives a reached complete glob error precedence over restart.

The incorrect-repository checks retained in
`PackageLookupFunction.java:262-304` and
`DirectoryDirentProducer.java:95-101` are latent in stock Bazel 9.2:
`LocalRepositoryLookupFunction.java:27-35` unconditionally returns
`mainRepository()` for every path, and
`LocalRepositoryLookupFunctionTest.java:159-169` confirms this for an
arbitrary nested path. Therefore a nested `MODULE.bazel` without a BUILD
marker continues traversal. This packet removes incorrect-repository and
nested-module detection from the live Slug contract; it does not expose a
latent variant, scan for WORKSPACE/MODULE markers, or claim behavior Bazel
9.2 cannot produce.

The live Slug split otherwise has the right owners but not the right public
composition. `HostRepositoryIgnoreKey` retains repository-scoped ignore
policy independently. Private
`HostRootPackageLookupKey { workspace, package: PackagePath }` retains
ordinary no-BUILD, deleted, invalid-name, and selected
`HostPackage { package_root, build_file_name }`, but intentionally collapses
an ignore match to `Deleted`. Exporting it would make an ignored directory
continue, while probing BUILD markers from loading would duplicate bzlmod
policy and lose ordered package-root selection.

The accepted projection is root/main-repository only and belongs to
`slug_bzlmod_v2`, named `HostRootPackageBoundaryKey`. Its exact identity is one
private normalized workspace and one `PackagePath`; the public constructor is
`new(NormalizedAbsolutePath, PackagePath)`. This matches Bazel's
package-identifier semantic key. The selected package-path root is a lookup
result, not key identity. Do not add a selected physical root, candidate
absolute Host path, repository mapping/name, BUILD basename, ignore entry, or
raw-byte carrier to the public key.

The later loading traversal must construct each candidate `PackagePath` in
Bazel's internal-string domain: on Unix, lift each raw path byte to the
same-valued Latin-1 scalar, preserving `e9` as U+00E9 and `c3 a9` as
U+00C3/U+00A9 without UTF-8 decoding or lossy conversion. Both remain
distinct identities, and both are invalid package names in the live Host
lookup, so they continue before any marker path is joined. Ignore matching
must still run first and sees those distinct internal strings. That adapter,
native-Windows/lone-surrogate conversion, and multi-segment path assembly are
later loading-owned gates; `PackagePath` comes from the lower identity crate
already shared by loading and bzlmod, so this projection introduces no
dependency inversion.

Add one public
`HostRootPackageBoundaryKind::{NoPackage, DeletedPackage, IgnoredDirectory,
Package}` and opaque public `HostRootPackageBoundary` and
`HostRootPackageBoundaryError` wrappers. The success wrapper privately
retains:

- ordinary no-package continue;
- deleted-package continue;
- ignored-directory stop; and
- actual-package stop, retaining the selected package-path root.

Only `HostRootPackageBoundary::kind()` and
`selected_package_root() -> Option<&NormalizedAbsolutePath>` are public; the
latter returns `Some` only for `Package`. The marker basename, ignore match,
private state, and invalid-name diagnostic have no accessor, variant,
conversion, serialization, or dereference surface. Invalid package name and
ordinary no-BUILD lookup both project to `NoPackage`, because the glob
producer observes only unsuccessful lookup and continues. Switching
`BUILD.bazel` to `BUILD` within the same selected root is likewise
semantically equal at this boundary and may prune a dependent consumer;
changing the selected root is unequal. Success and error Debug are manually
opaque. Error Display and source behavior delegate to the retained typed
private error rather than stringify it into equality. The key value is
exactly
`PathOutcome<Arc<Result<HostRootPackageBoundary,
HostRootPackageBoundaryError>>>`;
equality is `complete_eq`, validity is `is_complete`, and every Need remains
invalid and self-unequal.

Computation has this exact order:

1. Compute `HostRepositoryIgnoreKey::new(workspace)`. Need passes through
   unchanged and a typed error becomes the opaque public error.
2. If `matching_entry(package)` succeeds, return the retained
   ignored-directory stop without computing `HostRootPackageLookupKey`.
3. Otherwise compute the existing private
   `HostRootPackageLookupKey::new(workspace, package)`. Need passes through
   unchanged and a typed error becomes the opaque public error.
4. Map `Package` to `Package` with its selected root, `Deleted` to
   `DeletedPackage`, and both `NoBuildFile` and `InvalidPackageName` to
   `NoPackage`.

The private lookup's repeated nonmatching ignore dependency is already
DICE-cached and changes no semantics. Do not refactor either private owner in
this packet. The outer ignore check is required both for ignored-before-
deleted/invalid/marker precedence and for proving that an ignored candidate
does not request package lookup or BUILD-marker observations.

Implementation is gated on a focused Bazel oracle. Next, design only
`WP-5-m1-loading-host-package-boundary-oracle-design`; then implement and pin
`WP-5-m1-loading-host-package-boundary-oracle`. Reuse the existing stale
Bazel 9.1.1 `glob-package-boundaries` fixture rather than add another fixture.
Inventory its two old commands and every asset. Fold the first row's unique
ordinary-directory and actual-subpackage evidence into one pinned Bazel 9.2
exact `**/*.txt` membership row; remove that old command and prune its
duplicated `keep.txt`, `skip.txt`, explicit-exclude, and other scaffolding
unless the oracle design documents a genuinely distinct retained boundary
claim. Remove the duplicate allow-empty diagnostic, which
`glob-callable-contract` already owns. The replacement row discriminates:

- an ordinary directory continues;
- an actual BUILD-bearing subpackage stops;
- a BUILD-bearing directory named by `--deleted_packages` continues;
- a `.bazelignore` directory stops;
- a directory both ignored and deleted stops, proving ignore-first
  precedence; and
- a directory containing nested `MODULE.bazel` but no BUILD continues.

The oracle-design packet must freeze the exact asset/command/expected
allowlist, source anchors, caps, fresh-root replay, protected-fixture,
cleanup, and growth accounting before any fixture changes. It is oracle
packet two after checkpoint `e2cc891d`; no fixture-growth review is due unless
measured growth unexpectedly reaches a repository threshold.

After oracle acceptance, implement only
`WP-5-m1-loading-host-package-boundary-projection` with this exact allowlist:

- new `app/slug_bzlmod_v2/src/host_package_boundary/mod.rs`, at most 600
  additions;
- new `app/slug_bzlmod_v2/src/host_package_boundary/tests.rs`, at most 850
  additions; and
- `app/slug_bzlmod_v2/src/lib.rs`, at most eight additions for the private
  module declaration and exact public re-exports.

The hard aggregate cap is +1,450/-20. No Cargo, dependency, loading,
workspace, identity, private package/ignore owner, fixture, parser, evaluator,
consumer, or entrypoint file may change. No direct filesystem IO, blocking,
lock across DICE, global cache, event batch, standard retained collection,
new Buck2/V1 extraction, or copied lookup implementation is permitted.

Focused tests must prove exact key identity/display; all four public semantic
kinds and their continue/stop use; selected-root retention and accessor
opacity; root-major/marker-minor selection;
ordinary missing/directory marker continuation; regular, special, and
symlink-to-file subpackage stops; deleted-plus-marker continuation;
ignored-before-deleted, ignored-before-invalid, and ignored-before-marker
precedence with zero package-lookup/marker demand; distinct Latin-1-lifted
non-ASCII invalid-name continuation with zero marker demand; typed ignore and
lookup error plus exact Need propagation; opaque Debug and delegated
Display/source; complete-only equality and validity; BUILD-marker changes
within one selected root compare equal and prune a downstream counter, while
selected-root changes propagate; one retained DICE graph covering
create/delete/recreate,
ignore/delete/marker changes, error recovery, and A-to-B-to-A restoration;
zero public/private wrapper event data; and zero non-test callers.

Validate focused boundary tests first, then the full bzlmod, loading, and
workspace suites and doctests, GNU-Windows no-run linkage for those crates,
formatting, diff, exact scope/per-file/aggregate caps, public-surface,
dependency/caller, implementation-block, archive, credential, process, and
forbidden-surface guards. Stop on any pinned-source or oracle contradiction,
incorrect-repository state, nested-module stop, lossy conversion, physical
path key, ignore-after-lookup ordering, new dependency, direct IO, event
owner, caller activation, cap/allowlist expansion, native-Windows claim, or
need for a fourth implementation file.

This design and its oracle packets require no new Stage 9 landed subsection.
Correct the existing private Host glob owner's false nested-repository
residual as source maintenance during this design closeout. After accepted
projection implementation, add one Stage 4 landed subsection citing pinned
Bazel `8220c619...`, the accepted boundary oracle and implementation commits,
Buck baseline `088c75...` only for already-approved utility patterns, and V1
`e218054...` as rejected/reference-only. That record authorizes no V1/Buck
extraction and no recursive glob or consumer activation. Only after the
projection accepts may a later design compose multi-segment and standalone
`**` traversal, boundary pruning, operation filtering, and final unique
results. Regular-or-special BUILD/`.bzl` acquisition, parser activation,
transactional evaluator retry, include/exclude and `allow_empty` composition,
callable diagnostics, and production publication remain separate.

### Host package-boundary oracle design

Status: **ACCEPT** for design-only
`WP-5-m1-loading-host-package-boundary-oracle-design` on 2026-07-27 after
independent source/fixture review and correction of the inventory arithmetic
to 15 regular files at net +5. This packet changes only this owner-plan
section. It authorizes neither fixture nor generated-oracle edits;
implementation must follow the exact contract below as
`WP-5-m1-loading-host-package-boundary-oracle`.

Pinned Bazel 9.2.0 release commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the source of truth.
`DirectoryDirentProducer.java:76-116` checks the repository ignore matcher
before requesting `PackageLookupValue`, stops on that match, stops on a
successful package, and continues every unsuccessful lookup.
`PackageLookupFunction.java:91-122,157-180,253-325` validates first, maps
`--deleted_packages` to the unsuccessful deleted value, obtains ignore
policy, and finds an actual package from BUILD markers. The unsuccessful
`packageExists() == false` taxonomy is
`PackageLookupValue.java:49-68,220-389`.
`IgnoredSubdirectoriesFunction.java:75-102,127-199` reads repository-root
`.bazelignore` prefixes, while `IgnoredSubdirectories.java:169-185` retains
the matching prefixes. `GlobsFunction.java:113-139` supplies that policy to
glob work. `PackageOptions.java:94-111` uses the comma-separated package-name
converter for `--deleted_packages`. The cross-repository path is latent for
this observation: `BazelSkyframeExecutorConstants.java:29-33` selects ERROR,
but `LocalRepositoryLookupFunction.java:27-35` always returns the main
repository. Thus nested `MODULE.bazel` without BUILD is an ordinary
continuing directory, not a repository boundary.

The retained independent Bazel 9.2 observation is sufficient: `/usr/bin/bazel`
accepts the command options after `query` and comma-separated package names;
removing the stale `pkg/subpkg/BUILD.bazel` makes
`//pkg:subpkg/hidden.txt` visible. No new fixture, generated output, or
behavior probe belongs in this design packet.

#### Exact replacement fixture contract

Replace both stale commands with exactly one unmutated query row named
`root_package_boundary_six_state_projection`. The command is exactly:

```text
query --noshow_progress --deleted_packages=pkg/deleted,pkg/ignored_deleted --output=label_kind 'labels(srcs, //pkg:globbed)'
```

Its `fixture.toml` argv serialization is exactly
`["query", "--noshow_progress", "--deleted_packages=pkg/deleted,pkg/ignored_deleted", "--output=label_kind", "labels(srcs, //pkg:globbed)"]`.
It uses fixture and command comparison `exact`, exits zero, has no mutation,
no environment additions, and an empty manifest. Its raw stdout is exactly
these three lines plus the final newline:

```text
source file //pkg:deleted/deleted.txt
source file //pkg:nested_module/nested.txt
source file //pkg:ordinary/ordinary.txt
```

The generated `normalized_stdout` contains the same three lines without a
final newline because the harness normalizer strips surrounding whitespace.
`fixture.toml` must contain provenance `bazel_release = "9.2.0"`, the pinned
commit, the source anchors above, a translation note tying the six directory
states to their visible/absent labels, and these exact generation and
verification commands respectively:

```text
python3 -B -m tools.v2_oracle run --fixture glob-package-boundaries --tool bazel --bazel /usr/bin/bazel --update-expected
python3 -B -m tools.v2_oracle run --fixture glob-package-boundaries --tool bazel --bazel /usr/bin/bazel
```

The command's `stdout_patterns` must be anchored to precisely that normalized
three-line output with `\A` and `\Z`, without a trailing-newline expression,
and no stderr pattern may claim unstable startup text.
`expected/oracle.json` is generated only by the generation command; it owns
the one exact normalized command record, including empty manifest and
normalized stderr, while its raw `stdout` retains the final newline.
Hand-writing it, retaining either old record, or weakening either the fixture
or command comparator to `message_shape` or `semantic` is forbidden.

The sole `pkg/BUILD.bazel` content is exactly
`filegroup(name = "globbed", srcs = glob(["**/*.txt"]))\n`; it has no
`exclude`, `allow_empty`, helper, or second glob. Therefore one row
simultaneously discriminates all and only these states: ordinary
`pkg/ordinary` continues; BUILD-bearing `pkg/subpkg` stops; BUILD-bearing
`pkg/deleted` continues by deleted-package treatment; `.bazelignore` stops
`pkg/ignored`; ignored-plus-deleted `pkg/ignored_deleted` stops (the
ignore-first proof); and `pkg/nested_module/MODULE.bazel` without BUILD
continues.

The implementation asset allowlist is exact:

- Retain and rewrite `fixture.toml` and generated `expected/oracle.json` for
  the one exact row; retain `workspace/MODULE.bazel` with
  `module(name = "glob_package_boundaries")`; retain and rewrite
  `workspace/pkg/BUILD.bazel` to the sole glob above; retain
  `workspace/pkg/subpkg/BUILD.bazel` exactly as
  `# subpackage boundary\n` and `workspace/pkg/subpkg/hidden.txt` with
  `hidden\n` as its excluded witness.
- Add `workspace/.bazelignore` containing exactly
  `pkg/ignored\npkg/ignored_deleted\n`; add
  `workspace/pkg/ordinary/ordinary.txt` containing `ordinary\n`; add
  `workspace/pkg/deleted/BUILD.bazel` containing exactly
  `# deleted package boundary\n` and `deleted.txt` containing `deleted\n`;
  add `workspace/pkg/ignored/ignored.txt` containing `ignored\n`; add
  `workspace/pkg/ignored_deleted/BUILD.bazel` containing
  `# ignored and deleted package boundary\n` and `ignored_deleted.txt`
  containing `ignored_deleted\n`; and add
  `workspace/pkg/nested_module/MODULE.bazel` containing
  `module(name = "nested_module_boundary")\n` plus `nested.txt` containing
  `nested\n`.
- Delete `workspace/BUILD.bazel`, `workspace/pkg/keep.txt`,
  `workspace/pkg/skip.txt`, and `workspace/pkg/sub/child.txt`; remove the
  now-empty `pkg/sub` directory. Delete both stale commands
  `query_exported_glob_files` and
  `allow_empty_false_reports_subpackage_boundary_miss`, their mutation and
  every explicit-exclude, keep/skip, sub-directory, and duplicate
  `allow_empty` diagnostic claim. `glob-callable-contract` remains the sole
  owner of that diagnostic evidence.

No other fixture, root workspace, module, `.bzl`, harness, generated asset,
or source path may change. The resulting fixture has exactly 15 regular files,
zero symlinks, one command, and at most 120 newline-counted lines: at most 45
in `expected/oracle.json`, 38 in `fixture.toml`, five in `pkg/BUILD.bazel`,
and one line in every sentinel/marker text file. The only generated file is
the oracle JSON. Relative to the stale 10-file/103-line/two-command fixture,
the design estimate is +5 regular files, zero symlinks, no more than +17
lines, and -1 command; implementation records the measured result instead.

#### Required evidence and stop gates

Generate once with pinned `/usr/bin/bazel`, then run two complete, exact
replays from distinct fresh absolute run roots. Both replays must match the
generated oracle byte-for-normalized-field under exact comparison and retain
the three stdout lines in the stated Bazel order. Run the focused and full
oracle-harness tests, fixture schema/listing and Python compilation checks,
fixture asset/row/mutation inventory, exact per-file and aggregate cap checks,
and `git diff --check`. Protect every other fixture and every harness path as
unchanged. Before and after all Bazel runs, clean stale `slugd` and Bazel
processes associated with the runs; do not inspect, print, copy, or record
any Bazel RC, BuildBuddy credential, home configuration, run-root absolute
path, or derived secret.

This is oracle packet two after fixture-growth checkpoint `e2cc891d` (whose
tree was 1,314 regular files, 24 symlinks, and 39,304 lines). Even the capped
result remains below five packets, 100 files, and 10,000 lines; no growth
review is due unless measured scope unexpectedly crosses a threshold.

Stop and replan on a pinned-source contradiction; any label other than the
three exact lines; a nonempty manifest; command mutation; stale-row or
duplicate-allow-empty retention; asset/cap expansion; protected fixture or
harness change; replay drift; process/credential/path leak; an ignored
directory continuing; a deleted BUILD-bearing directory stopping; a nested
module stopping; or a need to broaden the six-state claim. Do not run Rust or
Cargo, edit DICE, Host owners, loading consumers, parser/evaluator, recursive
traversal, native-Windows support, a new fixture, or Stage 9. The exact next
implementation packet after this design accepts is
`WP-5-m1-loading-host-package-boundary-oracle`.

### Host package-boundary oracle

Status: **ACCEPT** for
`WP-5-m1-loading-host-package-boundary-oracle` on 2026-07-27 after
independent implementation review. The stale Bazel 9.1.1 two-row fixture is
replaced by the exact pinned Bazel 9.2 one-row contract above. Its three
ordered `label_kind` lines prove ordinary, deleted-BUILD, and nested-MODULE
continuation; the absent witnesses prove actual-subpackage, ignored, and
ignored-plus-deleted stops. The generated record exits zero with exact
comparison, empty manifest/environment/mutations, and no retained
allow-empty or explicit-exclude evidence.

One pinned generation, the worker replay, two root-owned distinct fresh-root
replays, and the protected `glob-callable-contract` replay passed. The full
oracle harness passed 107 tests through `uv` with one unrelated unknown
`asyncio_mode` configuration warning; listing, schema parsing, Python source
compilation, exact asset/record assertions, diff, and process guards passed.
The fixture is exactly 15 regular files, zero links, and 88 newline-counted
lines, a measured delta of +5/+0/-15 from its stale state. The whole fixture
tree is 1,322 regular files, 24 links, and 39,367 lines, or +8/+0/+63 from
checkpoint `e2cc891d`; this remains packet two and no growth review is due.
No harness, other fixture, Rust, Cargo, DICE, Host/loading owner, parser,
consumer, or Stage 9 changed. The next packet is exactly
`WP-5-m1-loading-host-package-boundary-projection`.

### Public Host root-package boundary projection implementation

Status: **ACCEPT** in `ad6751ef` on 2026-07-27 after independent
implementation review and focused correction rereview. The exact three-file
implementation is +1,133/-0: `host_package_boundary/mod.rs` +278,
`tests.rs` +850, and `lib.rs` +5. The public root/main-repository key retains
only normalized workspace plus `PackagePath`, computes repository ignore
first, exposes the four opaque boundary kinds and selected package root only,
delegates typed private errors, and uses exact
`PathOutcome<Arc<Result<...>>>` complete-only equality and validity. It has
zero production callers, event data, direct IO, locks, new dependencies, or
private-owner changes.

Focused tests passed 7/7 in worker and root runs. The full Linux matrix passed
210 bzlmod unit tests plus every integration binary, 73 loading tests, 36
workspace tests, and all three zero-test doctest suites. Final GNU-Windows
no-run linkage produced all 20 executables. Formatting, exact public surface,
scope/per-file/aggregate caps, dependency/caller, archive, credential,
process, and forbidden-surface guards passed.

Review required one tests-only correction: exact activation evidence now
proves ignored deleted, invalid, and marker-bearing candidates never activate
the private package key or marker dependencies; the retained graph changes a
marker-bearing package through deleted policy and restoration; and exact Need
plus both opaque typed-error branches are discriminating. The accepted Buck2
utility disposition retains only existing `Arc`, `Dupe`, `Allocative`,
compact path, and DICE patterns; no Buck/V1 code or representation was
extracted. Recursive composition remains unimplemented. The next packet is
design only `WP-5-m1-loading-pure-host-glob-traversal-design`.

### Pure Host glob traversal design

Status: `ACCEPT` on 2026-07-27 after one independent design review, bounded
root correction, and independent correction rereview. This is a design-only
reserved DICE/ownership decision. No Rust, fixture, oracle, Cargo, public API,
consumer, or ledger file changed.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` makes the
owner boundary clear. `GlobComputationProducer.java:95-122` splits one full
pattern into slash fragments and creates a `(base, fragment-index)` work
state; it allocates duplicate-work tracking only when more than one standalone
`**` occurs (`:104-121`). `FragmentProducer.java:107-160` gives `**` both its
zero-segment transition (advance at the same base) and its one-or-more
transition (handle `**` as a fragment); `DirectoryDirentProducer.java:76-156`
prunes ignored directories before package lookup, stops packages, advances a
non-`**` index, retains the `**` index, and filters directories by operation.
`PatternWithoutWildcardProducer.java:68-97` and
`PatternWithWildcardProducer.java:89-215` respectively establish literal
resolution and typed-listing/wildcard/symlink classification. Final producer
matches are an unordered set (`GlobComputationProducer.java:139-144`), while
the callable sorts its returned strings (`StarlarkNativeModule.java:119-131`);
Slug's private owner must instead promise a deterministic raw-byte order.
`GlobsFunction.java:79-102,210-216` retains the first reached error and checks
it before restarting for missing values. `Globber.java:24-32` defines FILES,
FILES_AND_DIRS, and SUBPACKAGES; only the first two are in this packet.

The one private owner is `HostGlobTraversalKey`, not an include/exclude,
callable, evaluator-attempt, or aggregation key. Its exact identity is:

- `workspace: NormalizedAbsolutePath`;
- `logical_package_root: NormalizedAbsolutePath`, the selected source root
  containing the starting package;
- `package: PackagePath` (the root/main-repository package containing that
  traversal);
- `pattern: HostGlobPattern`, retaining the complete validated raw-byte
  pattern as `Arc<[u8]>` and its already-validated slash fragments; and
- `operation: HostGlobTraversalOperation::{Files, FilesAndDirs}`.

`HostGlobTraversalKey::new` is checked and returns a separate
`HostGlobTraversalKeyError::NonLatin1PackagePathScalar { scalar }` before a
DICE key exists. It maps every starting `PackagePath` scalar U+0000..=U+00FF
to its same-valued byte and rejects every larger scalar. The key retains the
selected package root rather than an independently supplied package
directory; it derives the one logical starting directory by appending those
validated package bytes to `logical_package_root`. Thus callers cannot
traverse one physical package while applying another package namespace.
This constructor error is not a compute result or a
`HostGlobTraversalError` variant.

`HostGlobPattern::new` is the separate checked pattern constructor. It
performs the accepted full-pattern validation once, splits on `/`, and stores
each fragment as either `RecursiveWildcard` for an exact standalone `**` or
the existing literal/simple-star segment representation. Its error retains
the full raw pattern, fragment index when applicable, and the accepted
invalid-versus-deferred reason. The existing invalid matrix remains exact;
valid unsupported parentheses, brackets, braces, backslash, NUL, or other
deferred segment syntax remains deferred for the whole pattern. No compute
path reparses or broadens the grammar.

It has no include/exclude sequence, `allow_empty`, callable source location,
boundary-selected candidate package root, event data, consumer state,
parser/evaluator attempt, or result ordering mode. The exact value is
`SourcePreparationOutcome<Arc<Result<HostGlobTraversal,
HostGlobTraversalError>>>`; `HostGlobTraversal` privately retains one
`Arc<[HostGlobTraversalMatch]>`, where each match is its package-relative raw
byte path. A temporary standard `Vec`/frontier set is permitted during one
compute, but no retained `HashMap`, `HashSet`, `String`, regex cache, global
cache, or lock is. Success/error equality is structural after intentionally
opaque diagnostic fields are omitted; `equality` is `complete_eq` and
`validity` is `is_complete`, so every `Need` is invalid and self-unequal.

The traversal starts at state `(derived logical package directory, package,
fragment_index = 0)`. On Unix only, append each raw component to the logical
path with `OsStringExt`, and append the same component to `PackagePath` by
lifting every byte to the same-valued Latin-1 scalar: `e9` becomes U+00E9 and
`c3 a9` becomes U+00C3/U+00A9. There is no UTF-8 decoding or lossy conversion.
Thus the path presented to `HostRootPackageBoundaryKey::new(workspace,
candidate_package)` is distinct in exactly the way its accepted projection
requires. The starting package path is never rechecked as a boundary.

For an ordinary literal or simple-star fragment, compute the existing private
`HostGlobSegmentCandidatesKey` for the current directory and that fragment.
For each candidate directory, construct its candidate `PackagePath`, then
compute `HostRootPackageBoundaryKey`. `IgnoredDirectory` and `Package` prune
it before matching/result insertion or descent; `NoPackage` and
`DeletedPackage` continue. A non-directory is a terminal match only at the
last fragment. A non-pruned directory is a terminal match only at the last
fragment and `FilesAndDirs`, then advances to the next fragment if one exists.
`Files` never emits a directory. `Subpackages` has no enum variant, key
identity, implementation branch, or test claim.

For standalone `**`, first take the zero-or-more transition: if it is not
last, enqueue `(same directory, same package, index + 1)`; if last, emit the
current directory only for `FilesAndDirs` and only when it is not the starting
package directory. Then obtain `HostGlobSegmentCandidatesKey` with `*` at the
same directory, apply the same boundary pruning to every directory candidate,
and enqueue each survivor at the unchanged `**` index. Files under `**` are
emitted only when that `**` is last. This is the exact Bazel `:116-136` /
`DirectoryDirentProducer.java:125-155` state shape. With two or more
standalone `**` fragments, a temporary raw `(relative-path-bytes,
fragment-index)` visited set suppresses duplicate work; with zero or one it
is absent. Final deduplication belongs solely to this full-pattern owner:
sort retained package-relative raw byte paths lexicographically, then remove
equal paths once. Candidate-owner same-name ordering is not an observable
final tie-break because equal final paths collapse here.

All candidate and boundary computations are DICE-owned. The traversal uses
one FIFO state deque. Initial state has discovery ordinal zero. Popping a
state computes its one segment-candidate dependency. For `**`, enqueue its
zero-segment successor first; then inspect recursive candidates in their
accepted raw-byte order. For an ordinary fragment, inspect candidates in that
same order. Independent boundary keys for one materialized candidate list may
compute concurrently, but their returned slots are scanned in original
candidate order; newly discovered states are appended in that scan order and
receive monotonically increasing discovery ordinals. The multiple-`**`
visited check occurs before enqueue and does not renumber retained states.
A state-level segment error ranks at that state's ordinal; boundary errors
rank by state ordinal and candidate slot. A Need blocks expansion only of its
own state or candidate, is unioned with every other reached Need, and does not
prevent already queued or sibling slots from completing. After the deque is
exhausted, return the lowest-ranked reached complete error before the unioned
Need; return the Need only when no reached complete error exists. This is the
entire first-error rank—completion timing and task scheduling never affect it,
and a branch hidden behind a Need is not reached in that generation.

No unmatched candidate is observed. A segment-owner error is wrapped as
`HostGlobTraversalError::Segment { logical_directory, fragment_index, error }`;
a boundary error is `::Boundary { candidate_package, error }`; both retain no
physical root, route, symlink chain, namespace, selected marker, or lossy
name. Its complete variant set is exactly `UnsupportedHost`, `Segment { .. }`,
and `Boundary { .. }`; `UnsupportedHost` is the non-Unix dormant result before
path conversion. Existing `HostGlobSegmentError` and opaque
`HostRootPackageBoundaryError` remain their owners. DICE infrastructure
errors are invariants. No direct filesystem IO, fresh graph, event batch,
lock, or lock held across `ctx.compute(...).await` is allowed. Reached path
observations, repository-ignore policy, package policy, marker changes, and
directory changes invalidate through those two keys; equal restored complete
values prune consumers, whereas every Need recomputes.

#### Exact traversal oracle gate

Implement
`WP-5-m1-loading-pure-host-glob-traversal-oracle` before Rust by extending
only the existing pinned `glob-package-boundaries` fixture. Its accepted
`pkg` tree, six-state command definition, and first generated semantic record
remain unchanged. Add one isolated `traversal` package and one exact command.
The implementation allowlist is:

- `tests/v2_oracle/fixtures/glob-package-boundaries/fixture.toml`, at most
  +28/-4;
- `tests/v2_oracle/fixtures/glob-package-boundaries/expected/oracle.json`,
  generated only, at most +45/-4;
- new
  `tests/v2_oracle/fixtures/glob-package-boundaries/workspace/traversal/BUILD.bazel`,
  at most 52 lines;
- new
  `tests/v2_oracle/fixtures/glob-package-boundaries/workspace/traversal/literal/leaf.txt`,
  exactly `leaf\n`; and
- new
  `tests/v2_oracle/fixtures/glob-package-boundaries/workspace/traversal/literal/deep/deep.txt`,
  exactly `deep\n`.

The hard aggregate cap is +130/-8. Add exactly three regular files, no links,
one package, and one command. In the new BUILD file, bind these five sorted
lists:

```starlark
multi = glob(["literal/*.txt"])
zero = glob(["**/literal/*.txt"])
files = glob(["literal/**"])
all_paths = glob(["literal/**"], exclude_directories = 0)
double = glob(["**/**/leaf.txt"])
```

For `multi`, `zero`, `files`, and `all_paths`, use four top-level list
comprehensions over `range(len(matches))`. Each declares a `filegroup` with
one source and a name of
`<prefix>_<zero-based-index>_<path-with-slashes-replaced-by-underscores>`,
where the prefixes are respectively `multi`, `zero`, `files`, and `all`.
Add one filegroup named `"double_count_%d" % len(double)` with `srcs = double`.
No `def`, include/exclude composition, `allow_empty`, select, load, macro, or
other target is allowed.

Append exactly:

```toml
[[commands]]
name = "traversal_state_and_operation"
argv = ["query", "--noshow_progress", "//traversal:all"]
compare = "exact"
expected_exit = 0
stdout_patterns = ["\\A//traversal:all_0_literal\\n//traversal:all_1_literal_deep\\n//traversal:all_2_literal_deep_deep\\.txt\\n//traversal:all_3_literal_leaf\\.txt\\n//traversal:double_count_1\\n//traversal:files_0_literal_deep_deep\\.txt\\n//traversal:files_1_literal_leaf\\.txt\\n//traversal:multi_0_literal_leaf\\.txt\\n//traversal:zero_0_literal_leaf\\.txt\\Z"]
```

The indices encode callable result order rather than relying on query order.
Together the nine labels prove literal-plus-simple-star traversal, the
zero-segment `**` branch, terminal-`**` FILES filtering, terminal-`**`
FILES_AND_DIRS directory inclusion, and unique output from two standalone
`**` fragments. The existing first row remains the sole six-state boundary
proof. Add pinned source anchors for
`GlobComputationProducer.java:95-143`,
`FragmentProducer.java:107-160`,
`DirectoryDirentProducer.java:76-156`, and
`GlobTestBase.java:513-573`; update fixture description/notes only enough to
name traversal state and operation evidence. Broader grammar, raw names,
symlink lifecycle, callable include/exclude/allow-empty, SUBPACKAGES, and
consumer/query semantics remain out of scope.

Generate once with pinned `/usr/bin/bazel`, then replay both exact rows from
two distinct fresh absolute roots. Validate the protected first semantic
record byte-for-byte, the exact second record, empty manifests/mutations,
fixture schema/listing, Python compilation, focused and full oracle harness,
exact assets/allowlist/caps, archive/diff/credential/process/no-Slug guards,
and clean Bazel/`slugd` state before and after. Do not inspect or record the
user's Bazel RC. This is oracle packet three after checkpoint `e2cc891d`;
the accepted tree is currently 1,322 regular files, 24 links, and 39,367
lines, so the capped +3/+0/+130 result remains below every growth-review
threshold. Stop on any output drift, first-row change, nonempty manifest,
fixture/harness expansion, extra asset/target, cap breach, or broader claim.
No separate oracle-design packet remains.

`glob-callable-contract` continues to own callable filtering,
`glob-directory-invalidation` owns lifecycle/symlinks, and
`glob-raw-name-pattern-lazy` owns raw-byte ingress/lifecycle.

After that oracle accepts, the future implementation packet is exactly
`WP-5-m1-loading-pure-host-glob-traversal-owner`, with this allowlist:

- `app/slug_loading_v2/src/host_glob/mod.rs`, at most +90/-45 for private
  module declarations, parent visibility, and shared constructor reuse;
- new `app/slug_loading_v2/src/host_glob/traversal.rs`, at most 950 lines;
- new `app/slug_loading_v2/src/host_glob/traversal_tests.rs`, at most 1,300
  lines.

The hard cap is +2,340/-45. Existing `host_glob/tests.rs` and `lib.rs` may not
change. No Cargo/dependency, bzlmod, identity, workspace, fixture, public
re-export, existing public `glob.rs`, parser, evaluator, consumer, entrypoint,
event, lockfile, or status file may change. Focused tests must cover checked
key construction and exact key identity; raw-byte-to-`PackagePath`
distinction; literal, simple-star, multi-segment, and standalone-`**`
transitions including zero and one-or-more; FIFO rank independent of
completion timing; visited-state duplicate suppression and final raw
ordering; FILES versus FILES_AND_DIRS; boundary kinds/ignore-first pruning;
complete segment/boundary error before mixed Needs; Need equality/validity;
create, delete, package-marker/policy change and equal restoration in one
DICE graph; and zero production callers. Run focused host-glob tests plus
direct bzlmod boundary dependents, full loading tests/doctests, GNU-Windows
no-run linkage, formatting, `git diff --check`, exact
allowlist/cap/dependency/caller, archive/credential/process,
no-direct-IO/no-lock, no-public-surface, and forbidden-SUBPACKAGES guards.
Stop on a fourth file, a new dependency, any include/exclude/callable
activation, regular-or-special BUILD/`.bzl` acquisition, parser byte ingress,
evaluator retry, native-Windows or lone-surrogate behavior, or a
consumer/public export.

Consumer handoff is only a future private loading adapter accepting one full
pattern plus operation and consuming its ordered raw matches. Include/exclude
composition, `allow_empty`, callable diagnostics/sorting, package BUILD/`.bzl`
acquisition, parser activation, evaluator transactions, event publication,
external repositories, SUBPACKAGES, native Windows, and lone-surrogate parity
remain explicitly out of scope.

### Pure Host glob traversal oracle

Status: **ACCEPT** for
`WP-5-m1-loading-pure-host-glob-traversal-oracle` on 2026-07-27 after
independent implementation review and correction rereview.

The exact five-path extension is +55/-3, within every per-file and aggregate
cap. The isolated 12-line traversal package and two one-line assets produce
the exact nine labels above: indexed results prove literal/simple-star
multi-segment traversal, zero-segment and terminal standalone `**`, FILES
versus FILES_AND_DIRS membership and order, while `double_count_1` proves
unique callable output from two standalone `**` fragments. Both generated
records exit zero with empty manifests and mutations. The accepted first
six-state command and its complete generated record remain exactly unchanged.

One pinned Bazel 9.2 generation and three root-owned exact replays from
distinct fresh absolute roots passed. The writer's same-output-base replay
encountered only Bazel's known workspace-switch server warning and stopped
without broadening expectations; every clean-root replay then matched. The
explicit oracle suites passed 107 tests with one unrelated unknown
`asyncio_mode` warning. Fixture parsing/listing, Python compilation, exact
record/assets, source anchors, scope/caps, diff, credential, archive, and
process-cleanup guards passed. Review found and corrected only a regenerated
raw Invocation ID in the protected first record; structural comparison
against its pre-packet form and correction rereview passed.

The fixture is now exactly 18 regular files, zero links, two commands, and 140
newline-counted lines, a measured delta of +3/+0/+52. The whole fixture tree
is 1,325 regular files, 24 links, and 39,419 lines, or +11/+0/+115 from
checkpoint `e2cc891d`. This is oracle packet three, below every growth-review
threshold. No harness, Rust, Cargo, DICE, parser, consumer, other fixture, or
Stage 9 row changed. Next implement only the accepted private three-file
`WP-5-m1-loading-pure-host-glob-traversal-owner`.

### Pure Host glob traversal owner

Status: **ACCEPT** for
`WP-5-m1-loading-pure-host-glob-traversal-owner` on 2026-07-27 after one
independent implementation review and focused correction rereviews.

The private three-file owner lands in this commit at +1,354/-0, below the
+2,340/-45 contract. Checked construction preserves the workspace, logical
package root, one-byte package identity, one validated full pattern, and
FILES/FILES_AND_DIRS operation. FIFO traversal composes the accepted segment
candidate and root-package-boundary keys, implements literal/simple-star and
standalone-`**` zero/recursive transitions, suppresses repeated multi-`**`
states, ranks complete errors independently of completion order, returns Need
only after reached complete errors are exhausted, and sorts/deduplicates raw
matches once.

Retained state uses shared `Arc` slices, `Dupe`, and `Allocative`; traversal
uses a temporary `VecDeque`, and only multi-`**` visitation uses the existing
`starlark_map::SmallSet`. No dependency, global cache/interner, Buck/V1 glob
implementation, direct filesystem IO, lock, event, public export, or production
caller was added. Complete-only DICE equality/validity and same-graph
create/delete/marker/deleted/ignore/restoration behavior are discriminatingly
covered.

Root validation passed formatting, 13 focused traversal tests, and all 7
direct Host boundary tests. Recorded pre-close validation also passed the full
loading suite (86 tests), workspace suite (36), bzlmod units/integrations
(394), doctests, and GNU-Windows no-run linkage; no production code changed
after that cross-target pass. The reviewer-required tests cover repeated
`**`, every boundary kind with ignore-first zero lookup, reverse-recorded FIFO
error rank, all key identity fields, and same-graph policy restoration. Final
rereview returned `ACCEPT`.

The owner remains dormant with zero parser, evaluator, callable, query, or
consumer activation. Future glob work begins with a separately bounded private
loading adapter; it does not reopen JVM execution, native-Windows byte order,
BUILD/`.bzl` acquisition, include/exclude composition, `allow_empty`, or
evaluator transaction ownership.

### Private Host glob loading adapter

Status: **ACCEPT** for
`WP-5-m1-loading-host-glob-loading-adapter` on 2026-07-27 after independent
implementation review.

The private adapter accepts one normalized workspace, selected logical package
root, `PackagePath`, complete raw-byte pattern, and FILES or FILES_AND_DIRS
operation. It uses the accepted checked constructors, computes the existing
traversal through caller-owned `DiceComputations`, preserves `Need` and typed
traversal failure, and projects successful ordered paths by duplicating only
their `Arc` handles into one immutable shared slice. It performs no UTF-8 or
path-byte copy.

No new DICE key, cache, interner, lock, event, IO, dependency, public export,
or production caller was added. The Buck2 utility-reuse gate retains the
accepted `Arc`, `Dupe`, and `Allocative` boundary; the temporary collection
exists only while projecting the immutable slice.

Focused adapter 5/5 and traversal 13/13 tests passed. The complete
`slug_loading_v2` suite passed 91 tests (37 unit, 19 build-file, 23 bzl
invalidation, five callable-boundary, six glob-invalidation, and one removed
native-rule test), plus doctests. Formatting, diff, archive, exact
four-file/caller/key/dependency/IO/lock/public-surface guards passed.
Independent review returned `ACCEPT`.

The adapter remains dormant outside tests. Next review only the synchronous
Starlark-evaluator versus asynchronous DICE retry/attempt ownership required
for callable activation. JVM execution, blocking DICE, direct filesystem
fallback, native-Windows ordering, raw-byte Starlark ingress, BUILD/`.bzl`
acquisition, external repositories, and SUBPACKAGES remain excluded.

### Host glob transactional package-evaluation design

Status: **ACCEPT** for
`WP-5-m1-loading-host-glob-callable-activation-design` on 2026-07-27 after
independent architecture review and focused correction rereview.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
establishes the observable contract without prescribing Slug's machinery.
`Globber.java:20-62` defines one `(includes, excludes, operation, allowEmpty)`
call and an unordered fetched result.
`StarlarkNativeModule.java:93-131` chooses FILES versus FILES_AND_DIRS, fetches
the matches synchronously, disambiguates leading `@`, and natural-sorts the
returned Starlark list. `GlobsValue.java:68-123` keys individual dependency
work by pattern plus operation.
`PackageFunctionWithSingleGlobsDep.java:98-155,174-218` records those requests
while a non-Skyframe globber evaluates the package, then validates the same
requests through Skyframe. Slug must match the result and dependency behavior,
not reproduce that direct-filesystem hybrid.

The live Rust evaluator has no async native-call suspension seam:
`starlark/src/values/types/function.rs:90-160` defines a synchronous
`NativeFuncFn` returning `Result<Value>`, and
`starlark/src/eval/compiler/module.rs:261-270` evaluates one module
synchronously. The complete Starlark statement enum at
`starlark_syntax/src/syntax/ast.rs:384-400` has no exception handler, so BUILD
or loaded-macro code cannot catch a native-call failure. The exact V2 seam is
therefore an attempt-local abort/await/restart loop, not `block_on`, a
placeholder value, direct IO, or evaluator emulation.

The first implementation remains private and dormant. It does not change
legacy `PackageLoadKey`, `PackageListingKey`, any consumer, or any command
root. It adds a crate-private Host request containing exactly one shared
raw-byte pattern and FILES/FILES_AND_DIRS operation. Request equality and
hashing contain both fields. The accepted adapter computes that request through
the caller's `DiceComputations`; pattern/key-construction failures and complete
traversal failures stay typed, while `Need` remains transient control state.

One temporary
`SmallMap<HostGlobLoadingRequest, Arc<Result<HostGlobLoadingMatches,
HostGlobTraversalError>>>` lives outside evaluator attempts but inside one
future Host package computation; `Need` is never stored. It is neither a DICE
value nor a cache. Each attempt reparses the same already-validated immutable
source and constructs a new Starlark `Module`, `PackageRecorder`, target state,
used-glob list, and print capture while reusing already loaded frozen `.bzl`
modules. The recorder borrows only the prepared map and owns one
`RefCell<Option<HostGlobAttemptControl>>` slot. The control enum is exactly
`Pending(HostGlobLoadingRequest)` or
`Terminal(HostGlobAttemptError)`. The typed terminal error distinguishes a
request-construction failure, a payload-preserving traversal failure, and an
unsupported non-UTF-8 result; the future outer Host package error may wrap it
but may not stringify it.

`glob()` first performs the existing argument/type and `GlobSpec` validation.
It examines include patterns in source order followed by excludes, all under
the operation selected from `exclude_directories`. On the first absent request,
it stores `Pending(request)` in the empty control slot and returns one private
control error. If a prepared request contains a traversal error, it stores
`Terminal(Traversal(error))`; if an otherwise successful path cannot convert
exactly to the current UTF-8 Starlark representation, it stores
`Terminal(UnsupportedPath { path })`. Both then return the same private control
error. The outer evaluator never recognizes that error by text: after
`eval_module` fails, it consumes the recorder's typed control slot. A control
slot with successful evaluation, a second control value, or the private
control error without a slot is an invariant failure. Starlark has no catch
construct, so the transfer is not user-observable.

Before awaiting, the outer loop converts the print capture to an attempt-local
batch and drops the evaluator, Starlark module, recorder, target/used-glob
state, and evaluation error. It then computes exactly the pending request
through the existing adapter with no lock or `RefCell` borrow alive. A complete
success or typed traversal error enters the temporary prepared map, discards
the pending batch, and starts a fresh attempt. A `Need` discards that batch and
returns unchanged to the future Host package owner. A typed pattern/key
construction failure becomes `Terminal(Input(error))` and returns with that
attempt's saved print prefix; targets remain dropped. The temporary map may be
dropped on Need because completed traversal keys remain DICE-owned and are
reused on the next computation. There is no retry cap: every complete retry
adds one previously absent request, so progress is finite for one finite
evaluation trace.

On `Terminal`, the outer loop does not await or retry. It consumes and returns
the exact typed error, retains that attempt's print prefix as its complete
local event batch, and drops the module, recorder, and all partially declared
targets without calling `finish`. A normal Starlark error with no control slot
does the same through the existing loading-error branch. Thus only `Pending`
discards its print capture; terminal glob and ordinary evaluation failures
retain executed prints while never publishing targets.

Once every reached request is prepared, callable composition preserves the
accepted `GlobSpec` behavior: union each include's sorted raw paths, remember
whether each include matched before excludes, remove the union of exclude
matches, diagnose the first empty include when `allow_empty` is false, then
diagnose an all-excluded result, sort, and deduplicate once. Only paths that
convert exactly to the current UTF-8 Starlark representation may complete this
dormant owner; a non-UTF-8 path is an explicit typed unsupported boundary,
never lossy text. Leading-`@` disambiguation remains a required composition
case. `used_globs` is appended only after successful composition.

Only a final successful attempt may call `PackageRecorder::finish`. A final
success, normal Starlark error, or typed terminal glob error may retain its
local print batch; every pending attempt publishes no batch and drops all
declared targets. Loaded-module event batches remain dependency-owned and are
not replayed by BUILD attempts. This preserves the accepted command-level rule
that a `Need` is invalid and never becomes `LoadingError`; eventual propagation
through a parallel Host package key and typed command root remains a separate
reviewed packet.

Implement next only
`WP-5-m1-loading-host-glob-transactional-attempt-owner` in:

- `app/slug_loading_v2/src/host_glob/mod.rs`;
- `app/slug_loading_v2/src/host_glob/adapter.rs`;
- `app/slug_loading_v2/src/package.rs`;
- `app/slug_loading_v2/src/bzl_module.rs`; and
- new `app/slug_loading_v2/src/host_package_attempt_tests.rs`.

Add no public export, dependency, DICE key, production caller, fixture, parser
change, or legacy behavior change. Focused tests must prove one and multiple
requests, repeated-request reuse, include/exclude ordering, both operations,
per-include and all-excluded diagnostics, leading `@`, explicit non-UTF-8
rejection, a payload-bearing typed traversal error and `Need`, typed identity
through the control transfer, pending-attempt print/target discard, terminal
traversal/non-UTF-8 print-prefix retention with target nonpublication,
final-success print/target retention, typed input failure with its print
prefix, loaded-macro requests, same-graph complete reuse/restoration, and zero
legacy/production callers.
Run the focused attempt/adapter/traversal tests, full loading crate, formatting,
diff/archive, exact allowlist, no-public/key/dependency/IO/lock/blocking, and
caller guards. Stop if the owner requires changing `PackageLoadKey`, a fresh
DICE graph, blocking evaluator work, speculative values, a sixth file, or
downstream propagation.

After this dormant attempt owner, separately design the parallel
`HostPackageLoadKey` and its root-module/package-marker/BUILD/`.bzl` Host
inputs. Only later typed query/analysis command roots and the accepted native
demand driver may activate it. No Host `Need` may pass through the legacy
`Arc<Result<...>>` package key or become a string error.

### Host package-loading key input and ownership design

Status: **ACCEPT** for
`WP-5-m1-loading-host-package-key-input-ownership-design` on 2026-07-27 after
one independent reserved-architecture review and focused correction rereview.
No Rust, API, DICE key, dependency, fixture, consumer, or runtime path changed.

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` fixes the observable dependency
shape. `PackageFunction.java:1044-1167` obtains repository mapping, selects and
compiles the active BUILD file, resolves its load labels, loads every direct
`.bzl`, and only then commits to evaluation with no further Skyframe restart.
`BzlLoadValue.java:45-64,101-195,214-220` keys an ordinary loaded module by its
absolute label and retains its transitive module/digest state.
`BzlLoadFunction.java:662-730` requires the label package to equal the file's
innermost containing package, while `:768-921` resolves each module's loads
relative to that module, computes the child module keys, retains their
transitive state, and then evaluates the module.
`ContainingPackageLookupFunction.java:32-62` checks the deepest directory and
walks parents across nonpackages, including invalid/deleted lookup results.
`BzlCompileFunction.java:105-127` accepts both regular and special source
files. The already accepted root-package lookup source pins ordered package
roots, deleted/ignored packages, invalid package names, and `BUILD.bazel`
before `BUILD`. Slug reproduces those dependencies in Rust; no JVM, Java
bytecode, or Bazel execution belongs in this runtime path.

The live public `HostRootPackageBoundaryKey` cannot supply direct loading.
It is intentionally a traversal projection: invalid and missing packages both
become `NoPackage`, and it hides the selected BUILD basename. Loading also
cannot use public `PathFileBytesKey`, which rejects `SpecialFile`. The exact
shared boundary is one new public bzlmod projection,
`RootPackageSourceKey`, over the existing private
`HostRootPackageLookupKey` and `HostFileBytesKey`. It owns package policy,
marker selection, source-path construction, and regular-or-special bytes once;
loading does not reproduce any of them.

The projection also exposes one small immutable
`RootPackageBzlTarget`, retaining a validated `Arc<[u8]>` relative target
path with raw-byte equality, hashing, and ordering. Its checked constructor
first applies Bazel `TargetName` validity and then maps every
U+0000..=U+00ff carrier scalar to the same byte. It rejects a larger scalar,
an empty or absolute target, `.` or `..` component, doubled or trailing
separator, backslash, colon, control/DEL byte, non-`.bzl` suffix, or any other
shape that could escape or ambiguously reconstruct the selected package
directory. Failed construction occurs before a DICE key exists. This is the
only raw target type accepted by the source key and private Host module key.

`RootPackageSourceKey` has two constructors:

- `for_build(workspace, package)`, whose semantic identity is the normalized
  workspace, root-repository `PackagePath`, and `Build` request kind; and
- `for_bzl(workspace, package, RootPackageBzlTarget)`, whose identity replaces
  `Build` with the complete validated target raw bytes.

Repository identity is deliberately absent because this first surface accepts
only the main repository. Mapping provenance, selected package root, BUILD
basename, bytes, capture mode, and path-observation generation are dependency
results, not key identity. The validated target path is split on raw `/`; on
Unix each nonempty component becomes one `OsStringExt` component without
UTF-8 decoding. On Windows each component uses Bazel's internal
UTF-8-to-platform shape; an invalid internal UTF-8 path remains an explicit
typed unsupported result. There is no lossy path conversion on either
platform.

Its value is
`SourcePreparationOutcome<Arc<Result<RootPackageSource,
RootPackageSourceError>>>`. Complete success privately retains the selected
package-path root, exact logical source path, selected relative source bytes,
and one shared `Arc<[u8]>` content. Read-only accessors expose those four
facts to loading. Complete typed errors preserve private lookup or Host-file
errors and separately distinguish no BUILD file, deleted/ignored package,
invalid package name, selected source missing, and unsupported platform path.
Path Need becomes only `SourcePreparationNeeds::path` and otherwise passes
unchanged. DICE infrastructure failure is fail-fast. Equality compares the
entire complete result; validity accepts only Complete, so every Need is
invalid and self-unequal. The projection captures and stores no events.

The projection first computes `HostRootPackageLookupKey`. A package result
supplies both its selected package root and active BUILD basename. `Build`
reads that basename. Before a `.bzl` read, the projection walks target-parent
package candidates from deepest to the declared label package through the same
private lookup owner. It constructs each candidate by lifting every raw byte
to the same-valued Latin-1 scalar, exactly as the accepted Host traversal
boundary does. A deeper `Package` is the typed
`LabelCrossesPackageBoundary` result; only an innermost package equal to the
declared package supplies the source root. This matches Bazel's containing-
package check for labels such as `//pkg:subpkg/file.bzl`; loading may not
approximate it after source acquisition. Intermediate `NoBuildFile`,
`Deleted`, and `InvalidPackageName` candidates continue to their parent; those
states remain distinct only for the declared label package. The projection
then appends the checked target components and computes the existing
regular-or-special `HostFileBytesKey`. A marker selected by the lookup but
missing at the byte edge is a typed selected-source inconsistency, never a
fallback probe performed by loading. Declared-package `NoBuildFile`,
`Deleted`, `InvalidPackageName`, and package-boundary crossing remain distinct
direct-load results. Ignore policy keeps the private lookup's accepted direct-
load behavior (`Deleted`); the public traversal projection remains unchanged
and continues to expose `IgnoredDirectory` only for traversal pruning.

Loading adds two private, dormant DICE keys in `bzl_module.rs`.
`HostPackageLoadKey` identity is exactly normalized workspace plus root
`PackagePath`; its value is
`SourcePreparationOutcome<Arc<Result<LoadedPackage,
HostPackageLoadError>>>`. `HostBzlModuleEvalKey` identity is normalized
workspace plus a private mapping-free root-label value containing
`PackagePath` and `RootPackageBzlTarget`; its value is
`SourcePreparationOutcome<Arc<Result<FrozenBzlModule,
HostBzlModuleError>>>`. Neither key includes selected roots, physical paths,
source bytes/digests, loaded modules, event-capture mode, prepared globs, or
command state. Both use structural complete equality and complete-only
validity. Existing `Arc` source/module closures, `Dupe`, `Allocative`,
`SmallMap`, and immutable slices remain the retained utility boundary; add no
interner, global cache, retained standard map/set, or copied Buck/V1 loading
tree.

Load-label resolution is a pure checked helper, not another DICE key: it has
no observable input beyond the requesting root package and one parsed load
string. It preserves the original Bazel-internal spelling, resolves `:x.bzl`,
`//pkg:x.bzl`, explicit apparent-main `@//pkg:x.bzl`, and canonical-main
`@@//pkg:x.bzl` to the same mapping-free private root-label identity, and
constructs the shared validated raw target only after full label/target
validation. Any named apparent or canonical repository is
`UnsupportedExternalRepositoryLoad`, retaining the spelling; `.scl`,
builtins, prelude, autoload, and external mapping remain explicitly outside
this root-only slice. The resolved label, not its physical selected path, is
the `HostBzlModuleEvalKey` identity, matching Bazel's label key.

Both BUILD and `.bzl` source bytes must first pass exact UTF-8 validation and
then enter
`AstModule::parse_with_string_encoding(..., StringEncoding::BazelInternal)`
unchanged. This activates the accepted token-local raw-byte carrier without a
whole-source Latin-1 transform. Arbitrary invalid-UTF-8 source content remains
the seam's explicit typed `UnsupportedSourceEncoding` boundary. On Unix,
codemap input names lift every logical-path byte to its same-valued Latin-1
scalar instead of using lossy display. The accepted transactional BUILD
attempt owner must use the same opt-in parser on every retry; standard parser
behavior elsewhere remains unchanged.

`HostPackageLoadKey::compute` performs this exact order in one caller-owned
`DiceComputations`:

1. compute `RootModuleLoadingAnchorKey`; return its Need unchanged or a typed
   root error before any package/source observation;
2. compute `RootPackageSourceKey::for_build`; preserve its Need/error and
   selected root/source;
3. validate and opt-in parse the BUILD source, resolve load statements in
   source order, and compute each `HostBzlModuleEvalKey` through the same DICE
   computation;
4. after every reached module is Complete, call the accepted
   `evaluate_host_package_attempts` with the selected package root, exact
   source/path, and loaded frozen modules; and
5. return its terminal package/error or its transient Need without converting
   either to the legacy `LoadingError` package path.

`HostBzlModuleEvalKey` computes `RootPackageSourceKey::for_bzl`, performs the
same source validation and parser mode, resolves direct loads in source order,
computes each child Host key, then evaluates and freezes the module with the
existing manifest and lifetime-closure helpers. Sequential direct-child waits
make one frontier Need active at a time and retain source-order error
selection; completed child/source keys remain DICE-owned across the next
caller round. No Need is stored in a temporary module or prepared-glob map.

The existing request-scoped `bzl_load_cycle_detector()` remains the single
installed `UserCycleDetector`, but its private backend must recognize a
legacy-or-Host node enum. Keep `BzlLoadCycle`, `BzlLoadCycleGuard`, their
legacy key fields, and all legacy diagnostics unchanged. Add a separate
`HostBzlLoadCycle` and `HostBzlLoadCycleGuard`; each guard records edges only
to its own key family, while the shared event loop stores the private node
enum. Mixed-family edges are impossible because neither implementation
computes the other. Host child waits race through the Host guard and compute
the existing always-invalid cycle poison on detection so edit/break/restore
recovers in the same DICE graph. A missing/mismatched detector is an
infrastructure invariant, not a Bazel terminal error. Host cycle rendering
uses the selected BUILD origin plus canonical root `.bzl` labels and preserves
the existing Bazel cycle message shape without altering legacy rendering.

Event ownership is local and complete-only:

- the private root-module producer owns root readiness/evaluation events;
- every Complete `HostBzlModuleEvalKey`, including a parse/load/evaluation
  error, stores exactly one local batch (empty when it executed no print);
- `HostPackageLoadKey` stores exactly the final BUILD attempt batch for a
  Complete success or terminal error; and
- every Need stores no local batch. Pending BUILD attempts discard prints and
  targets as already accepted, while dependency batches stay
  dependency-owned and are never merged or replayed by the package key.

No evaluator, `RefCell` borrow, recorder, Starlark module, cycle-detector
guard lock, or shared lock crosses a DICE await except the existing
request-local async cycle wait, which has one sequential outstanding child
and cannot re-enter its guard. The temporary prepared-glob `SmallMap` lives
only inside one package compute and may be dropped on Need; DICE owns every
completed semantic dependency. There is no direct filesystem IO, injected
post-startup semantic value, fresh DICE graph, blocking executor, process
global, or legacy workspace snapshot/file/directory key.

Implement next only
`WP-5-m1-loading-host-package-key-input-ownership` in exactly:

- `app/slug_bzlmod_v2/src/host_package.rs`;
- `app/slug_bzlmod_v2/src/lib.rs`;
- `app/slug_loading_v2/src/bzl_module.rs`;
- `app/slug_loading_v2/src/cycle_detector.rs`; and
- new `app/slug_loading_v2/src/host_package_load_tests.rs`.

Add no Cargo/dependency change, fixture, command/query/analysis/core caller,
public loading export, legacy key/value/diagnostic change, repository
materialization, JVM/Java/Bazel runtime path, or sixth file. Focused tests must
prove both source kinds and regular/special bytes; BUILD primary/fallback and
package-root selection; deleted/ignored/invalid/missing and nested-package-
crossing distinctions; exact key identity and complete equality/Need
invalidity; root-anchor-before-source ordering; relative and all explicit main
load spellings; absolute/up-level/dot/doubled-separator/backslash/control/
trailing-separator path rejection before key construction; external/`.scl`
and invalid-source boundaries; transitive loads, source-order errors, and
load-cycle render/recovery; local event ownership; pending glob print/target
discard; and one retained graph covering marker, BUILD, `.bzl`,
nested-package, load-edge, cycle, package-policy, and restoration transitions.
Reuse the accepted Bazel-internal-string and glob evidence; add no oracle
fixture.

Validate the focused bzlmod source and Host loading/cycle tests, full
`slug_bzlmod_v2` and `slug_loading_v2`, one direct `slug_core_v2` compile
dependent, GNU-Windows no-run linkage, formatting, `git diff --check`,
archive status, and exact scope/public/Cargo/dependency/caller/legacy/IO/lock/
blocking/JVM guards. Stop and `REPLAN` if implementation requires exposing a
private bzlmod owner, changing `PackageLoadKey` or `BzlModuleEvalKey`, command
activation, external repository mapping/materialization, arbitrary
invalid-UTF-8 source parsing, a fresh graph, or a sixth file.

#### Host package-loading key implementation status

Status: **ACCEPT** for
`WP-5-m1-loading-host-package-key-input-ownership` on 2026-07-27 after one
terminal independent implementation review.

The five-file implementation adds the accepted public bzlmod source projection,
private dormant Host package/module keys, and isolated Host cycle nodes without
changing legacy loading or adding a caller. Focused source and Host-loading
tests pass, including one retained graph covering marker, BUILD, `.bzl`,
nested-package, load-edge, cycle, package-policy, and restoration transitions.
Both changed crates passed their full suites before the final test-only
strengthening; the direct `slug_core_v2` compile dependent and GNU-Windows
no-run linkage pass. Formatting, diff/archive, scope/export/caller/Cargo/
dependency/legacy/IO/blocking/JVM guards pass.

Next design only
`WP-5-m1-loading-typed-propagation-design`. Freeze the minimum root-repository
typed loading boundary needed by later simple query work; keep external
repositories, analysis, command/runtime activation, and broader discovery out
of scope.

### Root loading typed-propagation design

Status: **ACCEPT** for
`WP-5-m1-loading-typed-propagation-design` on 2026-07-27 after one independent
reserved-boundary review.

The accepted `HostPackageLoadKey` is already the complete root-repository
loading owner: normalized workspace plus `PackagePath` identity, root anchor
before package input, Host BUILD/`.bzl`/glob dependencies, typed transient
Need, complete-only equality/validity, and one local final-attempt event batch.
Adding a wrapper DICE key would duplicate identity and event ownership without
adding a semantic dependency. This packet therefore exposes that owner rather
than copying or adapting it.

The live query seam demonstrates why the typed boundary is needed.
`slug_query_v2::graph::UnconfiguredPackageGraphKey` computes legacy
`PackageLoadKey` and immediately converts infrastructure or loading failures
to `QueryError`. `LoadingQueryEnvironment::loading_files` repeats that legacy
compute and calls the legacy `discover_build_file_companion` path. Those call
sites cannot accept Host Need. `slug_query_v2` has only a development
dependency on `slug_bzlmod_v2`, so making it depend directly on the bzlmod
preparation envelope merely to consume loading would also invert the intended
crate boundary.

Implement the public loading boundary with four root exports:

- rename the private `HostPackageLoadKey` to public `RootPackageLoadKey`;
  retain private fields, a typed public constructor taking
  `NormalizedAbsolutePath` and root `PackagePath`, the existing
  `host-package-load:` display prefix, and the exact accepted DICE identity;
- expose terminal errors as an opaque `RootPackageLoadError` struct over a
  private
  `RootPackageLoadErrorInner`; preserve structural equality, cloning,
  allocation accounting, messages, and the existing terminal variants;
- reexport `slug_bzlmod_v2::SourcePreparationOutcome` as
  `LoadingPreparationOutcome`; and
- reexport `slug_bzlmod_v2::SourcePreparationNeeds` as
  `LoadingPreparationNeeds`.

The key value is exactly
`LoadingPreparationOutcome<Arc<Result<LoadedPackage,
RootPackageLoadError>>>`. The aliases preserve one shared root/bootstrap/path/
repository Need representation through loading while keeping downstream query
dependent only on `slug_loading_v2`. Do not export `HostBzlModuleEvalKey`,
`HostRootBzlLabel`, Host source/load errors, the transactional attempt owner,
or any bzlmod private package/file owner. Do not add a second loading outcome,
convert Need to an error, or add a helper that computes in a fresh graph.

This visibility packet changes no compute body, dependency edge, equality,
validity, event batch, cycle family, parser mode, diagnostic text, or legacy
key. The existing `Arc<Result<...>>`, private enum, immutable slices,
`SmallMap`, and clone behavior remain the retained representation; add no
interner, cache, retained standard collection, or additional allocation layer.
No public caller is added yet. The following query Host-migration/typed-root
work must construct `RootPackageLoadKey` directly inside its caller-owned
`DiceComputations`, return `LoadingPreparationOutcome::Need` unchanged, and
format `RootPackageLoadError` only after Complete.

Implement next only
`WP-5-m1-loading-typed-propagation` in:

- `app/slug_loading_v2/src/bzl_module.rs`;
- `app/slug_loading_v2/src/lib.rs`; and
- `app/slug_loading_v2/src/host_package_load_tests.rs`.

The focused tests must consume the root exports rather than private `super`
names and preserve constructor identity, Complete structural equality, Need
self-inequality/invalidity, local event ownership, and the retained lifecycle
regression. Run those tests, the full loading crate only if production logic
changes, direct `slug_query_v2` compile coverage, GNU-Windows no-run linkage,
formatting, diff/archive status, and exact three-file/public-export/no-caller/
Cargo/dependency/legacy/IO/blocking/JVM guards.

Add no Cargo change, wrapper key, new DICE value, query/analysis/core/CLI/server
caller, external repository support, directory-discovery migration, fixture,
oracle, materialization, runtime driver, JVM, Java bytecode, or Bazel
delegation. Stop if Rust requires changing the accepted key identity/value,
exposing a nested private error, adding a fourth implementation file, or
converting Need to an error.

#### Root loading typed-propagation implementation status

Status: **ACCEPT** for `WP-5-m1-loading-typed-propagation` on 2026-07-27
after one terminal independent implementation review.

The three-file visibility-only patch root-exports `RootPackageLoadKey`, its
opaque terminal error, and the shared preparation outcome/Need aliases. It
adds no wrapper or caller and preserves the accepted compute, identity,
equality/validity, event, display, and diagnostic behavior. Focused tests pass
4/4; direct `slug_query_v2` compile coverage and loading/query GNU-Windows
no-run linkage pass. Formatting, diff/archive, exact scope/export/no-caller/
Cargo/dependency/legacy/IO/blocking/JVM guards pass.

Next design only `WP-5-m1-analysis-typed-propagation-design`, bounded to the
minimum typed analysis boundary required before the query command-root design.

### Root configured-analysis typed-propagation design

Status: **ACCEPT** for
`WP-5-m1-analysis-typed-propagation-design` on 2026-07-27 after one independent
reserved-boundary review and focused correction rereview.

The live `ConfiguredTargetAnalysisKey` is already a public production key with
`PathBuf` workspace identity and `Arc<Result<AnalysisResult, AnalysisError>>`
value. It computes legacy `PackageLoadKey`, recursively computes itself through
`try_compute_join`, and converts every loading or DICE failure to
`AnalysisError`. Existing core callers inject eager workspace snapshots and
cannot accept a preparation Need. Changing that key in place would activate an
unprepared runtime path and break the accepted no-entrypoint-switch boundary.

Add one parallel dormant public `RootConfiguredTargetAnalysisKey`. Its identity
is normalized `NormalizedAbsolutePath` workspace plus the complete existing
`ConfiguredTargetKey`; fields stay private and its public constructor accepts
those two typed values. Its value is exactly
`AnalysisPreparationOutcome<Arc<Result<AnalysisResult, AnalysisError>>>`, where
`AnalysisPreparationOutcome` and `AnalysisPreparationNeeds` are analysis-root
aliases of the loading-owned shared preparation envelope. This avoids a
production `slug_bzlmod_v2` dependency while preserving one root/bootstrap/
path/repository Need representation through loading and analysis.

The root key accepts only root-repository configured labels. It computes
`RootPackageLoadKey` for the configured label package in the same
`DiceComputations`. Need passes through unchanged. A Complete loading error
becomes the existing opaque `AnalysisError`; a Complete package selects the
same target, Starlark-rule implementation, configured dependency keys, and
analysis evaluator as the legacy key. External repositories, missing targets,
non-Starlark rules, evaluator failures, providers, actions, and diagnostics
retain the existing message and value shapes.

Refactor only pure, post-loading analysis preparation shared by the two keys:

- before dependency computes, one helper temporarily borrows the Complete
  package value, finds the configured target, validates its Starlark-rule kind,
  and returns owned declared dependency keys in declaration order;
- both key families retain only their immutable owned
  `Arc<Result<LoadedPackage, ...>>` dependency value across child computes,
  with every package/target/implementation borrow dropped; and
- after dependency computes, one helper reborrows that same completed package
  value, re-finds the implementation, projects prepared providers in
  declaration order, evaluates the rule, and owns the local print batch.

Do not recompute either package key after the dependency horizon: its completed
immutable value is already retained and DICE owns the dependency edge. No
package/target/implementation borrow, evaluator, `RefCell`, print capture, or
provider collection may cross a DICE await. The legacy key keeps its exact
identity, value, dependency family, equality/validity, local event behavior,
messages, and callers while using the shared pure helpers. For the root key,
package Need can arise only from its initial `RootPackageLoadKey` compute and
returns before dependency selection or local event capture.

The root key deduplicates dependency keys in first-seen declaration order and
computes every unique `RootConfiguredTargetAnalysisKey` with one
`compute_join`. Each joined item retains its key and raw DICE result. After all
complete:

1. union every dependency Need in first-seen order with
   `AnalysisPreparationNeeds::try_union`; conflicting Needs are a fail-fast
   infrastructure invariant;
2. if any Need exists, return the union and no local event batch;
3. otherwise select the first DICE or Complete analysis error in first-seen
   order; and
4. otherwise restore declared duplicate/order projections and evaluate.

This makes Need dominate a simultaneously observed terminal child result, as a
Skyframe restart does, while keeping deterministic error and dependency order.
Child event batches remain child-owned. Every root-key Complete success or
terminal error stores exactly one local batch, empty when evaluation did not
run or print; every Need stores none. Root-key equality is structural across
the entire Complete result and self-unequal for Need; validity accepts only
Complete. The existing legacy key continues to treat only successful results
as equal/valid.

The retained value stays one `Arc<Result<...>>`, existing `AnalysisResult`,
immutable dependency slices, `SmallSet`, and `SmallMap`. The new preparation
enum is an alias, not another wrapper or allocation. Add no interner, global
cache, retained standard map/set, copied analysis graph, or new dependency
value.

Implement next only `WP-5-m1-analysis-typed-propagation` in:

- `app/slug_analysis_v2/Cargo.toml`;
- `app/slug_analysis_v2/src/dice.rs`;
- `app/slug_analysis_v2/src/lib.rs`; and
- new `app/slug_analysis_v2/tests/root_analysis.rs`.

Move `slug_workspace_v2` from development-only to production dependencies for
the normalized key identity; keep `slug_bzlmod_v2` development-only for
focused Host input injection. Add no other dependency. The new focused test
must prove public constructor identity, root package loading, recursive
dependency order, two-child Need union, Complete equality/Need invalidity,
local event ownership and initial package-Need suppression, and same-graph
BUILD/`.bzl`/dependency edit-delete-restore behavior. Reuse the accepted
loading source, cycle, glob, and event evidence; add no oracle.

Run the focused root-analysis test, full `slug_analysis_v2` because shared pure
helpers change the legacy call flow, direct `slug_core_v2` compile coverage,
GNU-Windows no-run linkage, formatting, diff/archive status, and exact
four-file/public-export/no-caller/Cargo/dependency/legacy-identity/IO/lock/
blocking/JVM guards.

Add no existing-key replacement, core/query/CLI/server caller, command/runtime
activation, external repository, configuration transition, platform/toolchain,
directory discovery, execution/materialization, fixture/oracle, JVM, Java
bytecode, or Bazel delegation. Stop if Rust requires changing the legacy key
value/identity, holding evaluator state across an await, adding a fifth file,
or converting Need to `AnalysisError`.

#### Root configured-analysis typed-propagation implementation status

Status: **ACCEPT** for `WP-5-m1-analysis-typed-propagation` on 2026-07-27
after one terminal independent implementation review.

The exact four-file patch adds the dormant public
`RootConfiguredTargetAnalysisKey` and analysis preparation aliases while
leaving the legacy key identity, value, callers, diagnostics, dependency
family, and event behavior unchanged. Only the owned immutable completed
package value crosses child DICE awaits. The root family preserves
first-seen dependency order, unions all child Needs before selecting an error,
stores local events only for Complete results, and uses Complete-only
structural equality and validity.

The focused root-analysis regression passes 1/1 and the full analysis crate
passes its 4/1/4/4 integration groups. Direct `slug_core_v2` compile coverage,
analysis/core GNU-Windows no-run linkage, formatting, diff/archive, exact
scope/export/no-caller/Cargo/dependency/legacy/IO/lock/blocking/JVM guards all
pass.

Next design only `WP-5-m1-query-typed-command-root-design`, prioritizing the
smallest observable query path without runtime activation or Host migration.

### Typed query command-root design

Status: **ACCEPT** for `WP-5-m1-query-typed-command-root-design` on 2026-07-27
after one reserved-boundary review and focused correction rereview. The
correction keeps typed Needs in the private root environment and uses only an
inert sentinel to unwind the fixed `QueryError` call chain.

The live query facade is an async function over a caller-owned
`DiceComputations`, raw `PathBuf` workspace, query text, order, policy, and
output completion. It computes legacy `UnconfiguredPackageGraphKey`,
`PackageLoadKey`, and `SubtreePackageSetKey` values and returns only
`Result<QueryOutput, QueryError>`. An in-place replacement would activate
unprepared core/CLI/server callers and would either erase Host preparation
Needs or change the public evaluator contract.

Add one parallel dormant public `RootQueryCommandKey`. Its private identity is:

- normalized `NormalizedAbsolutePath` workspace;
- exact compact query source text;
- `QueryOrder`;
- `QueryPolicy`; and
- `QueryOutputCompletion`.

Its public constructor validates parsing and the supported loading-query
function set before returning the key, so invalid syntax/function requests
remain preflight work that can later run before a command attempt. Derive the
missing hash/retained-data traits only on the existing policy/completion value
types; do not add a second option representation. The key value is exactly
`QueryPreparationOutcome<Arc<Result<QueryOutput, QueryError>>>`, where
`QueryPreparationOutcome` and `QueryPreparationNeeds` are public query aliases
of the loading-owned preparation envelope.

Every valid root query first computes
`slug_bzlmod_v2::RootModuleLoadingAnchorKey` with the same normalized
workspace. Its Need passes through unchanged; its Complete error becomes the
existing package-loading-shaped `QueryError`; its Complete success permits
evaluation. This is the always-present DICE dependency that gives `set()` and
other valid empty-result queries a nonempty exact closure without inventing a
root BUILD package or eagerly loading the workspace. Query owns this
command-level anchor directly, so move `slug_bzlmod_v2` and
`slug_workspace_v2` from development-only to production dependencies rather
than reexporting a command concern through loading.

Keep every existing legacy key, facade, caller, identity, value, diagnostic,
ordering rule, and validity/equality rule unchanged. Add one private
root-package-graph key over normalized workspace plus `PackagePath`; its value
is the preparation envelope around the existing immutable graph result. Share
only a pure post-loading projection from `LoadedPackage` between the legacy
and root graph keys. The root graph computes `RootPackageLoadKey`; Need passes
through and a Complete loading error retains the existing query diagnostic
shape. It has Complete-only structural equality/validity and no public export.

Give `LoadingQueryEnvironment` a private legacy/root mode. Legacy construction
and all current public evaluator functions remain unchanged. Root mode:

- computes the private root-package-graph key for ordinary package graph
  access;
- computes `RootPackageLoadKey` for the direct package load used by
  `buildfiles()`/`loadfiles()`; and
- retains the existing `SubtreePackageSetKey` and build-companion discovery
  paths until the separate query Host-migration packet.

The last bullet is an explicit dormant-boundary limitation, not an activation
claim: recursive directory discovery and companion lookup still depend on
eager workspace projections and are forbidden by the later preactivation
scan.

The generic evaluator and traversal surface is fixed to `QueryError`, whose
public `message` and `exit_code` fields cannot themselves hold typed control
state without changing compatibility. Root-mode `LoadingQueryEnvironment`
therefore owns a private `Option<QueryPreparationNeeds>` side channel. On a
reached Need it unions the typed value into that slot with `try_union` and
returns one private `QueryErrorKind::PreparationRestart` sentinel solely to
unwind the fixed generic call chain.

The sentinel's mandatory public fields use reserved inert values: empty
message and `i32::MIN` exit code. Those fields encode no Need and are never a
semantic result. Only root-mode package access may create the sentinel;
message rewriting and context-classification helpers must recognize it and
pass it through unchanged without formatting. The private root evaluator must
match the sentinel, take the typed side-channel value, and immediately return
`QueryPreparationOutcome::Need`. A missing side-channel value, a semantic
error accompanied by one, or a sentinel reaching a Complete/public/legacy
result is an infrastructure invariant failure. Legacy construction cannot
create the sentinel. Thus the typed Need is never stored, formatted, or
exposed as `QueryError`; the sentinel is only an internal unwind token required
by the frozen generic signature. All existing semantic `QueryError` fields,
constructors, display, exit codes, and behavior remain unchanged.

The evaluator remains sequential and lazy. A Need unwinds at the first DICE
dependency actually reached; it must not evaluate a later union operand,
literal, function branch, recursive package, or completion lookup merely to
seek more work. A reached root package key already returns the deterministic
union of its internal independent Needs. If a future query operation joins
multiple already-reached DICE branches, it must union all of those Needs in
stable input order before selecting an error; this packet adds no such join.
Root-key equality is structural across the entire Complete result and
self-unequal for Need; validity accepts only Complete. The command root emits
no local event batch, while its exact dependency closure retains anchor and
loading events.

Implement next only `WP-5-m1-query-typed-command-root` in:

- `app/slug_query_v2/Cargo.toml`;
- `app/slug_query_v2/src/graph.rs`;
- `app/slug_query_v2/src/loading_environment.rs`;
- `app/slug_query_v2/src/generic.rs` only if required to pass the private
  carrier without rewriting it;
- `app/slug_query_v2/src/evaluator.rs`;
- `app/slug_query_v2/src/lib.rs`;
- `app/slug_query_v2/tests/loading_query.rs`; and
- `app/slug_query_v2/tests/query.rs`.

Focused regressions must prove public constructor identity and preflight
rejection, root-anchor Need/Complete behavior for `set()`, nonempty anchor
activation for its empty output, package Need rather than Complete
`QueryError`, private-carrier non-escape, lazy later-operand suppression,
existing Auto/Full result order, Complete equality/Need invalidity, and one
same-DICE BUILD/`.bzl` edit-delete-restore transition. Reuse existing query
fixtures and the accepted loading lifecycle evidence; add no fixture or
oracle.

Run the focused root-query regressions and full `slug_query_v2` because the
environment/error control boundary changes, direct `slug_core_v2` compile
coverage, query/core GNU-Windows no-run linkage, formatting, diff/archive
status, and exact allowlist/export/no-caller/Cargo/dependency/legacy-identity/
carrier-escape/IO/blocking/JVM guards.

Add no existing-key replacement, analysis dependency, core/CLI/server caller,
runtime activation, external-repository breadth, recursive Host migration,
new output format or query function, eager query preloading, evaluator
concurrency, event owner, fixture/oracle, JVM, Java bytecode, or Bazel
delegation. Stop if implementation requires changing the public legacy
facade, exposing the carrier, adding `traversal.rs` or another ninth file,
forcing an otherwise-lazy branch, or representing Need as text.

#### Typed query command-root implementation status

Status: **ACCEPT** for `WP-5-m1-query-typed-command-root` on 2026-07-27 after
one terminal independent implementation review and one focused test-only
correction rereview.

The seven-file patch adds the dormant public `RootQueryCommandKey`, its
always-present root-module anchor, private typed root-package graph, and
root-mode query environment. Typed Needs remain in the environment side
channel; the inert sentinel only unwinds the fixed generic call chain and
cannot escape as a Complete/public error. Legacy keys, facades, callers,
ordering, and eager subtree/companion paths remain unchanged.

The focused regression passes 1/1 and proves every identity field, both
preflight rejection classes, empty-query anchor activation, typed Need and
lazy later-operand suppression, Auto/Full order, Host-backed `loadfiles()`,
Complete/Need equality and validity, and same-DICE `.bzl`
edit/delete/restore. The full query crate passes 17 unit, 41 loading-query,
and 6 query tests. Direct core compile coverage, query/core GNU-Windows
no-run linkage, formatting, diff/archive, exact scope/export/no-caller/Cargo/
dependency/legacy/carrier/IO/blocking/JVM guards pass.

Next design only `WP-5-m1-query-host-migration-design`, bounded to the
remaining eager subtree discovery and build-companion query paths before any
runtime activation.

### Query Host-migration design

Status: **ACCEPT** for `WP-5-m1-query-host-migration-design` on 2026-07-27
after one reserved DICE-boundary review and focused correction rereview. The
correction matches Host regular/special marker kinds and exact
root-major/basename-minor precedence.

The accepted dormant `RootQueryCommandKey` uses Host ownership for its module
anchor, ordinary package graph, and direct `buildfiles()`/`loadfiles()`
package load. Two root-mode operations still deliberately reach legacy eager
projections: recursive target patterns compute `SubtreePackageSetKey`, and
`buildfiles()` calls `discover_build_file_companion` for each reachable
`.bzl` package. The legacy query facade must retain both paths unchanged, but
the typed root closure cannot activate while either remains reachable.

Add one private `RootSubtreePackageSetKey` in `graph.rs`. Its identity is
normalized `NormalizedAbsolutePath` workspace plus typed `PackagePath` prefix,
and its value is exactly
`QueryPreparationOutcome<Arc<Result<SubtreePackageSet, QueryError>>>`. It
computes `RootPackageLookupInputsProjectionKey` once to obtain the complete
ordered package-root set. A missing/projection error is Complete
`QueryError`, not Need.

Traverse relative directory paths in the same deterministic depth-first shape
as the legacy key. For each UTF-8 valid relative candidate, first compute
`HostRootPackageBoundaryKey`:

- ignored directories stop without listing or descending;
- selected packages are recorded but still descended for nested packages;
- deleted/no-package candidates are not recorded and still descend; and
- Need passes through before any child is reached.

For every candidate that continues, compute `PathDirectoryListingKey` in Host
namespace for that relative directory under every configured package root.
Issue all root listings for the candidate together, retain their root order,
union every reached path Need in that order before selecting the first
complete error, then merge only OS-native `Directory` entry names. Sort and
deduplicate the merged native names, push them in reverse order, and finally
sort/deduplicate recorded UTF-8 package strings exactly as the legacy result
does. Missing roots contribute no entries; wrong-kind, observation,
resolution, and inconsistent-state failures become deterministic Complete
`QueryError`.

Keep traversal identity OS-native. A non-UTF-8 candidate cannot enter
`PackagePath`/ignore ownership. Probe its markers with typed
`ResolvedPathKey` in the exact Host lookup ordinal: for each package root in
input order, probe that root's `BUILD.bazel` and then its `BUILD` before
advancing to the next root. A resolved `RegularFile` or `SpecialFile` is a
marker, matching current Host package lookup; other present kinds and Missing
are not. If no marker exists, continue native traversal; if one exists, return
the existing `package path is not UTF-8` semantic error. Issue the probes for
the candidate together, retain this root-major/basename-minor ordinal, union
every Need in that order before selecting the first complete error or marker,
and do not silently skip, lossily encode, or prematurely reject a non-package
native directory.

The root subtree key has Complete-only structural equality/validity. It stores
only the final compact sorted package slice, not directory listings, roots,
or a copied traversal graph. It emits no event batch.

Migrate root-mode companion discovery without loading or parsing the companion
BUILD. For each first-seen reachable `.bzl` package:

1. compute `HostRootPackageBoundaryKey`;
2. return no companion for ignored, deleted, or no-package results;
3. for a selected package root, probe `BUILD.bazel` then `BUILD` with Host
   `ResolvedPathKey`; and
4. construct the existing companion label/path from the first resolved
   `RegularFile` or `SpecialFile`.

The boundary already owns package-root precedence, deleted policy, ignore, and
marker selection; the typed probes recover only the selected basename and
symlink-resolved Host marker kind. A boundary claiming Package while neither
marker resolves to a regular or special file is a fail-fast DICE invariant.
Need enters the existing private query environment side channel and the inert
restart sentinel unwinds it; operational Complete errors retain the existing
query-evaluation diagnostic kind. Root mode must never call
`discover_build_file_companion`.

Keep the legacy `SubtreePackageSetKey`, `discover_build_file_companion`,
facade, keys, callers, ordering, diagnostics, and equality unchanged. Split
mode-specific helpers where needed so the root call graph has no semantic
`WorkspaceDirectoryKey`, `WorkspaceDirectorySnapshotKey`, or eager companion
edge. No query event, output, parser, generic evaluator, command key, or
public export changes.

Implement next only `WP-5-m1-query-host-migration` in:

- `app/slug_query_v2/src/graph.rs`;
- `app/slug_query_v2/src/loading_environment.rs`; and
- `app/slug_query_v2/tests/loading_query.rs`.

Focused scripted Host regressions must prove multi-root directory-name union
and package-root precedence, cumulative ordered Needs across roots,
ignored/deleted/package transitions, recursive create/edit/delete/recreate,
native-name and root-major marker precedence, stable package/output order, no
legacy subtree activation, and parse-independent companion
fallback/primary/symlink/special-file/missing/restore behavior including a
broken BUILD file. Reuse the accepted
Host boundary/path/symlink evidence and existing query fixtures; add no new
fixture or oracle.

Run the focused Host-query tests and full `slug_query_v2`, direct
`slug_core_v2` compile coverage, query/core GNU-Windows no-run linkage,
formatting, diff/archive status, and exact three-file/no-export/no-caller/
Cargo/dependency/legacy/eager-root-path/IO/blocking/JVM guards.

Add no Cargo/dependency change, public key, existing-key replacement,
core/CLI/server caller, runtime activation, external repository, new query
function/format, evaluator concurrency, event owner, fixture/oracle, JVM,
Java bytecode, or Bazel delegation. Stop if implementation needs a fourth
file, converts Need to `QueryError`, changes legacy behavior, evaluates a
companion BUILD, lists only the workspace root instead of all package roots,
or lossily converts an OS-native name.

### Query Host-migration implementation

Status: **ACCEPT** for `WP-5-m1-query-host-migration` on 2026-07-27 after one
terminal implementation review and one focused correction rereview.

Exactly `graph.rs`, `loading_environment.rs`, and `loading_query.rs` now keep
the dormant typed query root on Host-owned recursive package discovery and
BUILD-companion lookup while leaving the legacy facade unchanged. The private
subtree owner unions every package root, preserves native path identity,
Complete-only DICE validity, Need dominance, and exact
root-major/basename-minor marker order. Companion lookup accepts regular and
special markers without evaluating BUILD content.

Focused evidence covers multi-root union and precedence, ignore/deleted and
create/edit/delete/restore transitions, no legacy subtree activation, primary,
fallback, symlink, special, missing/restored, and syntactically broken
companions, plus a two-root non-UTF-8 ordinal discriminator. The focused 3
tests and full `slug_query_v2` 17-unit/43-loading/6-query suites pass; direct
core coverage, query/core GNU-Windows linkage, formatting, scope, and diff
checks pass. The correction rereview returned `ACCEPT` with no remaining
contract gap.

Design next only `WP-5-m1-build-typed-command-root-design`. Freeze the core
analysis/package-root bundle, always-present empty-target anchor, deterministic
Need union, and exact core/Cargo/test implementation allowlist before Rust.

### Build typed command-root design

Status: **ACCEPT** for `WP-5-m1-build-typed-command-root-design` on 2026-07-27
after one terminal DICE-boundary review and focused correction rereview.

The current core build path is still a retained-runtime facade, not a command
root. It separately computes legacy `RootModuleGraphKey` and
`WorkspaceEvaluationKey`, calls `BzlModuleEvaluator::evaluate_package`, then
conditionally computes legacy `ConfiguredTargetAnalysisKey`. An empty target
list therefore has no exact typed loading/analysis root, transient Host Needs
cannot reach the shared retry driver, and event-producing work remains above
or beside any future terminal closure.

Add one dormant private `BuildCommandRootKey` in
`app/slug_core_v2/src/runtime/dice.rs`. Its semantic identity is:

- normalized `NormalizedAbsolutePath` workspace;
- the canonical ordered target-pattern spellings, stored as an immutable
  string slice because `TargetPattern` is not a hashable DICE identity; and
- an explicit `ConfigurationKey`.

Its constructor accepts parsed `TargetPattern` values, retains their canonical
display spellings including duplicates, and rejects recursive patterns and
non-root repositories before DICE. Identity is semantic, not source-spelling
identity: `//pkg` and `//pkg:pkg` both parse and display as `//pkg:pkg` and
therefore name the same key; target order and duplicate count still differ.
Those checks are pure request-shape work and cannot emit an event or Need. Do
not change `slug_identity_v2` merely to derive key traits.

The key value is exactly:

```text
SourcePreparationOutcome<
    Arc<Result<BuildCommandEvaluation, BuildCommandError>>
>
```

`BuildCommandEvaluation` privately retains the opaque
`RootModuleLoadingAnchor` plus one ordered record per requested pattern. Each
record retains its canonical pattern spelling, `LoadedPackage`, and optional
`AnalysisResult`. It carries no workspace revision, eager workspace
evaluation, resolved module graph, command output buffer, or execution state.
`BuildCommandError` retains typed root-anchor, `RootPackageLoadError`, and
`AnalysisError` causes plus a structured target-not-found case containing the
canonical pattern spelling, typed `PackagePath`, typed `TargetName`, and BUILD
path; it does not stringify transient control or DICE infrastructure failure.

Compute `RootModuleLoadingAnchorKey` first on every request, including an empty
target list. A Need returns unchanged; a Complete anchor error is terminal; a
successful anchor is retained in the final value so an empty build has both a
nonempty exact activation closure and equality sensitive to root-MODULE
semantics.

After a successful anchor, issue all requested-pattern branches together in
input order. A package-wide pattern computes `RootPackageLoadKey`. A single
target first computes the same package key, preserves the existing
target-not-found diagnostic boundary, and computes
`RootConfiguredTargetAnalysisKey` with the explicit configuration only when
the selected target is a Starlark rule. Convert its accepted root apparent
label to the canonical configured label `@@//<package>:<target>` with no
repository-mapping provenance; no string other than that validated projection
may enter `ConfiguredTargetKey`. Native/file targets retain `None` analysis
exactly as the current facade does. Preserve duplicate request records and
input order in the completed bundle.

Collect every Need reached by those independent branches with
`try_union` in input order before selecting the first input-order Complete
error. Do not launch work past the initial anchor Need, but do not return the
first target Need while another already-independent target branch can add path
or repository work. Reversing target order reverses the selected Complete
error when no Need exists. A raw subordinate DICE/cancellation failure has
absolute precedence over accumulated Need and Complete semantic errors: fail
the root compute as an infrastructure invariant and publish nothing. A
`try_union` conflict is likewise an infrastructure invariant, never a
first-Need fallback or terminal semantic error. Factor the branch collector so
these total precedence rules can be tested directly without adding a test-only
semantic field to the DICE key. Need is invalid and unequal even to itself;
Complete successes and typed semantic errors use structural equality and
validity.

The command root stores no local event batch. Its exact dependency closure
contains the always-present root anchor and every reached root package,
`.bzl`, and root analysis producer; those accepted owners retain their own
marker-conditional batches. No event- or Need-producing compute may remain
above or beside this root when it is later activated. This packet does not
connect the key to the private retry driver, retained runtime, CLI, server,
REAPI, execution, or output publication.

The exact future implementation allowlist is one file:

- `app/slug_core_v2/src/runtime/dice.rs`, including its private unit-test
  module.

No Cargo or dependency change is needed, and no public reexport or integration
test file is allowed. The focused tests in that file must prove:

1. every identity field, constructor rejection, Complete-only
   equality/validity, `//pkg`/`//pkg:pkg` canonical identity, and an empty
   build's anchor/root activation;
2. cumulative deterministic Needs across two independent target packages,
   target order/duplicates, Need dominance over a Complete error, reversed
   Complete-error order, raw-infrastructure dominance over Need, and
   `try_union` conflict failure;
3. package-wide, native, missing-target, and Starlark-analysis results through
   only the typed root loading/analysis keys, with zero activation of the
   legacy root graph, workspace evaluation, package-load, and analysis keys;
4. same-DICE root-MODULE, BUILD, `.bzl`, and rule-implementation
   create/edit/delete/restore behavior, including anchor Need/error suppression
   of every target branch and equal-Complete pruning; and
5. one positive command-effect closure containing root-MODULE, BUILD/`.bzl`,
   and analysis producer batches exactly once in dependency order, even for
   duplicate target patterns, plus explicit empty-batch suppression. No retry
   attempt is terminally selected or published; a batch first evaluated on a
   retry may appear only when its producer remains in the exact terminal
   closure, while retry-only producers are excluded; and
6. GNU-Windows compilation of the private key and tests.

Run the focused tests, full `slug_core_v2`, direct query/analysis/loading
compile coverage, core GNU-Windows no-run linkage, formatting,
`git diff --check`, and exact one-file/no-Cargo/no-export/no-caller/
no-legacy-key/no-eager-snapshot/no-blocking-IO/no-JVM guards. Stop if the
implementation needs a second file, a public API, target-pattern identity
changes, Need stringification, a caller, or any JVM, Java-bytecode, Bazel
delegation, execution, or activation work.

The review correction froze canonical target identity and configured-label
projection, total infrastructure/Need/semantic-error precedence, anchor branch
suppression, and positive exact terminal event-closure evidence. The focused
rereview returned `ACCEPT` and confirmed the one-file boundary is feasible
with current dependencies and public typed-root prerequisites.

Implement next only `WP-5-m1-build-typed-command-root`.

### Build typed command-root implementation

Status: **ACCEPT** for `WP-5-m1-build-typed-command-root` on 2026-07-27 after
one terminal implementation review and focused discriminator corrections.

Exactly `app/slug_core_v2/src/runtime/dice.rs` now contains the dormant private
build command root, canonical ordered target/configuration identity, opaque
always-present root anchor, ordered package/optional-analysis bundle, typed
semantic errors, and total infrastructure/Need/Complete-error precedence. It
uses only the accepted root anchor, root package-load, and root analysis keys;
there is no public export, caller, activation, Cargo change, legacy-key edge,
eager snapshot edge, execution, JVM, or Bazel delegation.

Eight focused tests prove canonical identity and preflight, empty anchoring,
ordered duplicates, cumulative target Needs and union-conflict failure,
anchor branch suppression, root-MODULE/BUILD/`.bzl`/analysis lifecycle,
structured missing-target fields, zero legacy-key activation, exact one-time
terminal event selection, explicit empty-batch clearing, and retained versus
retry-only producer selection. The full core suite passes 118 unit and 13
integration tests; downstream loading/analysis/query checks, GNU-Windows
linkage, formatting, and diff checks pass. The final independent rereview
returned `ACCEPT`.

Design next only `WP-5-m1-preactivation-host-gate-design`. Freeze the exact
build/query transitive forbidden scans and stop conditions before any opaque
envelope or production activation work.

### Preactivation Host gate design

Status: **ACCEPT** for `WP-5-m1-preactivation-host-gate-design` on 2026-07-27
after one terminal activation-boundary review.

The live audit separates dormant semantic closure from current production
adapters. The accepted `BuildCommandRootKey` reaches only
`RootModuleLoadingAnchorKey`, `RootPackageLoadKey`, and
`RootConfiguredTargetAnalysisKey`; its focused tracker already reports zero
legacy root-graph, workspace-evaluation, package-load, and analysis
activations. The accepted `RootQueryCommandKey` reaches its Host anchor,
root-package graph, Host recursive subtree owner, and Host BUILD-companion
owner; its tracker reports zero legacy subtree activation. Legacy eager keys
still coexist in the same source files but are not dependencies of either
typed root.

Production is deliberately not ready to activate. One-shot build still calls
`evaluate_workspace_targets_with_bzlmod_inputs`; one-shot query calls
`evaluate_workspace_query_with_policy_and_bzlmod_inputs_and_output_completion`.
The daemon calls `evaluate_observations_with_bzlmod_inputs` and
`query_observations_with_policy_and_bzlmod_inputs_and_output_completion`.
Those adapters accept `WorkspaceObservation`, inject `WorkspaceSnapshotKey`,
`WorkspaceRawSnapshotKey`, and `WorkspaceDirectorySnapshotKey`, and compute
legacy projections. The daemon's filesystem scan also owns the visible
`invalidated_files` metric. These are activation blockers, not reasons to
weaken the typed roots or remove the metric in this gate.

Implement one test-only transitive closure gate in exactly:

- `app/slug_core_v2/src/runtime/dice.rs`; and
- `app/slug_query_v2/tests/loading_query.rs`.

Extend the existing build and query activation trackers to reject every
semantic activation of:

- `WorkspaceSnapshotKey`, `WorkspaceRawSnapshotKey`, and
  `WorkspaceDirectorySnapshotKey`;
- `WorkspaceFileKey`, `WorkspaceRawFileKey`, and `WorkspaceDirectoryKey`;
- legacy `RootModuleGraphKey`, `WorkspaceEvaluationKey`, `PackageLoadKey`,
  `ConfiguredTargetAnalysisKey`, and query `SubtreePackageSetKey`.

The build gate must reuse the accepted empty, package-wide, native,
missing-target, and Starlark-analysis cases. The query gate must reuse valid
empty, direct/lazy, recursive multi-root, `loadfiles()`, and `buildfiles()`
primary/fallback/symlink/special/missing/restore cases. Both assert one typed
command root and zero forbidden activations; do not add a synthetic facade or
source-name allowlist that can pass without computing the real roots.

Run this exact activated-surface call-site scan and retain every current match
as an explicit activation blocker:

```text
rg -n 'evaluate_workspace_targets_with_bzlmod_inputs|evaluate_workspace_query_with_policy_and_bzlmod_inputs_and_output_completion|evaluate_observations_with_bzlmod_inputs|query_observations_with_policy_and_bzlmod_inputs_and_output_completion' \
  app/slug_cli_v2/src/commands/{build.rs,query.rs} \
  app/slug_server_v2/src/lib.rs
```

Activation may proceed only when its atomic vertical packet removes those
calls from the activated surface and a repeated scan has no activated
call-site match. Definitions, exported legacy wrappers, and legacy-only tests
in core may remain until the separate retirement packets and therefore are not
part of this call-site scan. Separately run
`rg -n 'observe_workspace|WorkspaceObservation' app/slug_server_v2/src/lib.rs`;
those matches may remain only inside `FilesystemObservationAdapter::observe`
and the metric-only call chain that computes `invalidated_files`. Its returned
observations must not enter the typed command transaction or closure.

The implementation gate passes only if focused build/query trackers, full
core/query suites, direct loading/analysis checks, GNU-Windows no-run linkage,
formatting, diff, exact two-file scope, and no-Cargo/no-production-change
guards pass. A forbidden semantic activation is `REPLAN`; do not hide it with
tracker filtering or convert it into a tolerated legacy edge.

Add no production code, public API, caller, envelope, CLI/server behavior,
snapshot retirement, metric change, activation, execution, JVM, Java
bytecode, or Bazel delegation. After this test-only gate is accepted, design
only the private opaque terminal result/output envelope required before either
vertical activation.

The terminal review returned `ACCEPT`. It confirmed the two-file type-downcast
gate covers the full forbidden owner/projection/legacy set, the call-site scan
excludes permitted core definitions, and `invalidated_files` remains
metric-only. Implementation must snapshot tracker counts around each reused
multi-compute case and assert a one-root activation delta, not rely on a
cumulative root count.

Implement next only `WP-5-m1-preactivation-host-gate`.

### Preactivation Host gate implementation

Status: **ACCEPT** for `WP-5-m1-preactivation-host-gate` on 2026-07-27 after
one terminal independent implementation review.

The exact two-file test-only gate now wraps every accepted typed build/query
compute with an exact one-command-root activation delta and rejects every
named eager snapshot owner/projection and legacy semantic key. The combined
trackers cover workspace snapshot/raw/directory snapshots, file/raw/directory
projections, the legacy root module graph, workspace evaluation, package load,
configured analysis, and query subtree owner. They exercise the accepted
empty, package-wide, native, missing-target, Starlark-analysis, direct/lazy,
recursive multi-root, `loadfiles()`, and `buildfiles()` lifecycle cases.

Focused core 8 and query 3 tests pass. Full core passes 118 unit plus 13
integration tests; full query passes 17 unit, 43 loading-query, and 6
integration tests. Direct loading/analysis checks, GNU-Windows no-run linkage,
formatting, diff, exact two-file, and no-Cargo guards pass. The activated
CLI/server scan retains six expected legacy adapter matches as explicit
blockers, and the daemon observation scan confirms the existing observation
flow remains unactivated while `invalidated_files` ownership is unchanged.
The terminal independent review returned `ACCEPT`.

Design next only `WP-5-m1-private-opaque-terminal-envelope-design`. Freeze the
private terminal result/output boundary required for an atomic typed query
activation; add no production caller, CLI/server behavior, execution, JVM,
Java-bytecode, or Bazel delegation.

### Private opaque terminal envelope design

Status: **REPLAN** for
`WP-5-m1-private-opaque-terminal-envelope-design` on 2026-07-27.

The live checkout already has the required semantic pieces. A typed query root
returns
`SourcePreparationOutcome<Arc<Result<QueryOutput, QueryError>>>`; output
completion is part of root identity and the retained `QueryOutput` formats
text, graph, label-kind, and package output without re-entering DICE. The
shared retry owner already keeps `Need` outside terminal state, selects one
exact terminal event/demand closure, commits the selected native snapshot,
moves selected events into `CommandOutputBuffer`, closes the lease, and only
then returns. Its synthetic result nevertheless exposes `terminal` and
`CommandOutputBuffer` as separate private fields. Activating that shape would
let a future adapter consume the semantic result while silently dropping
selected output.

Implement the inactive envelope in exactly:

- `app/slug_core_v2/src/runtime/events.rs`;
- `app/slug_core_v2/src/runtime/dice.rs`; and
- `app/slug_core_v2/src/runtime/mod.rs`.

Add one `#[must_use]` public `AcceptedCommand<T>` with private terminal and
selected-event fields. Construction remains `pub(super)` and is possible only
inside the accepted terminal path after selected-snapshot replacement and
successful lease close. Expose no terminal getter, event getter, iterator,
clone, `into_parts`, or public constructor. Its only public semantic access is
a consuming `finish` operation:

```text
AcceptedCommand<T>::finish(
    self,
    FnOnce(T) -> TerminalOutput<R>,
) -> CommandOutput<R>
```

`TerminalOutput<R>` is constructed by the post-core query/build projection and
contains one retained value, exit code, stdout, and terminal stderr.
`CommandOutput<R>` contains the same retained value and primitive streams
after core has automatically rendered every selected event before terminal
stderr. It may expose consuming primitive parts only after this merge. This
makes selected output inseparable from the first access to the terminal value;
the callback cannot inspect or discard the event buffer. `finish` calls the
projection exactly once and does not clone the terminal value, events, or
streams.

Render batches and their events in selected closure order. A
`StarlarkPrint` contributes its exact text plus one line terminator to stderr.
A neutral diagnostic contributes its already-formatted exact text plus one
line terminator; invent no severity prefix. Selected event stderr precedes
terminal stderr. Terminal stdout stays a separate channel, so the accepted
Bazel oracle's within-channel claim is preserved without inventing
cross-channel order. Empty batches and an empty selected buffer add no bytes.

Refactor the dormant acceptance seam to
`accept_prepared<T>(prepared, terminal) -> AcceptedCommand<T>`. The buffer may
move into a local value after accepted-snapshot replacement and before close,
as today, but no envelope may be constructed or escape until close succeeds.
The synthetic driver stores only `AcceptedCommand<Result<...>>`, never
parallel terminal/output fields. `Need`, cancellation, DICE/closure/native
failure, restoration, materializer rejection, snapshot replacement failure,
and close failure return no envelope. Both `Complete(Ok(...))` and
`Complete(Err(...))` are terminal and enter the envelope after exact closure
selection. Retry seals and drops its transaction/root/outcome before progress;
it cannot publish or accumulate a command output buffer.

Focused core tests must prove:

1. success and typed semantic error each project once through the envelope;
2. selected batch/event order, exact multiline text, diagnostic order, event
   stderr before terminal stderr, stdout separation, and empty-buffer identity;
3. the retained generic value, exit code, and primitive streams survive the
   consuming merge without a terminal/event clone path;
4. retry-only events remain absent while terminal-reachable reused events
   remain, and a valid empty query still returns an accepted envelope;
5. every forced preaccept/accept/close failure exposes no envelope and retains
   the existing restoration or fail-closed behavior; and
6. the public surface exposes only the opaque envelope and post-merge output,
   with no public DICE IDs, Needs, generations, leases, roots, materializer
   owners, selected sidecars, batches, or raw events.

Run focused envelope/session tests, full `slug_core_v2`, direct query/loading/
analysis compile checks, GNU-Windows no-run linkage, formatting,
`git diff --check`, and exact three-file/no-Cargo/no-caller guards. Scan the
activated CLI/server adapters from the preactivation gate and require the same
six matches: this packet removes none and activates nothing.

Stop rather than add a second output owner, generic async/HRTB driver, event
clone, public raw buffer/event access, formatter re-entry into DICE, execution,
REAPI, CLI/server call, eager snapshot change, JVM, Java bytecode, or Bazel
delegation. This command-local single-consumption structure does not alter a
retained hot-path representation and needs no Buck2-derived collection.

After acceptance, design a query-first atomic vertical activation. This is an
intentional scheduling refinement from the older build-first wording: query
has no execution/materialization phase, its typed root and closure gate are
accepted, and prioritizing it reaches simple query operations sooner without
changing either build semantics or the shared envelope. That later design must
freeze exact core/CLI/server/test files, remove the one-shot and daemon query
legacy adapter matches together, keep the daemon observation scan metric-only,
and prove one-shot/daemon equivalence before Rust.

The terminal review returned `REPLAN` for two precise reasons. Captured
`StarlarkPrint` currently retains only message text while Bazel's accepted
stderr shape includes source location, so this packet cannot irreversibly
render selected events. The moving `FnOnce(T)` projection also could replace
the accepted terminal with unrelated output rather than retain it.

### Private opaque terminal envelope design correction

Status: **ACCEPT** for
`WP-5-m1-private-opaque-terminal-envelope-design` on 2026-07-27.

The correction preserves the same exact three-file implementation allowlist
and acceptance/failure ordering, but defers event rendering. Add one
`#[must_use]` public `AcceptedCommand<T>` whose private fields retain the exact
accepted terminal and `CommandOutputBuffer`. Construction remains
`pub(super)` after successful lease close. It has no terminal/event getter,
iterator, clone, public constructor, or `into_parts`.

Its only public operation is a consuming borrowed projection:

```text
AcceptedCommand<T>::project(
    self,
    FnOnce(&T) -> TerminalOutput,
) -> CommandOutput<T>
```

`TerminalOutput` has one exact public constructor
`TerminalOutput::new(exit_code: i32, stdout: String, stderr: String)`.
`CommandOutput<T>` privately retains the original `T`, the unrendered selected
event buffer, and those terminal streams. It is also `#[must_use]`, has no
clone, getters, iterator, public constructor, or consuming parts. Therefore
the callback can inspect but cannot move, replace, or stash the accepted
terminal, and no caller can reach terminal streams or value while dropping
selected events. Projection runs exactly once; event bytes are neither
rendered nor exposed in this packet.

Refactor
`accept_prepared<T>(prepared, terminal) -> AcceptedCommand<T>` and make the
synthetic driver store only that envelope. Focused tests prove borrowed
success/error/empty-query projection, exact terminal identity retention,
single projection, retry-only exclusion, terminal-reachable event retention,
and no envelope on every accepted failure seam. Private module tests may
inspect fields to prove retention; no test-only accessor enters the public
surface. Keep all earlier focused/full/downstream/Windows/scope/call-site
validation and stop gates except the superseded rendering assertions.

After this inactive envelope is accepted, design and implement one
source-aware command-event prerequisite before activation. The live Starlark
`PrintHandler::println(&self, text: &str)` supplies no source span, so that
design must cite the accepted Bazel 9.2 `DEBUG: <path>:<line>:<column>: <text>`
shape, freeze the smallest Starlark/event/producer representation change, and
prove exact location plus warm nonreplay without string reconstruction from
fixture text. Only after source-aware rendering can a consuming publication
method expose `CommandOutput<T>` as primitive streams. Query-first activation
remains the next vertical route after that prerequisite; the reviewer
confirmed that scheduling refinement is sound.

The focused correction rereview returned `ACCEPT`. Implement next only
`WP-5-m1-private-opaque-terminal-envelope`.

### Private opaque terminal envelope implementation

Status: **ACCEPT** for `WP-5-m1-private-opaque-terminal-envelope` on
2026-07-27 after one terminal implementation review and focused correction.

The exact three core runtime files now expose public `#[must_use]`
`AcceptedCommand<T>`, `TerminalOutput`, and `CommandOutput<T>` with private
construction and storage. Dormant terminal acceptance returns only the opaque
envelope after successful lease close. Its consuming projection borrows
`&T`, retains the exact original terminal and unrendered selected-event
buffer, and exposes no getters, parts, clone, raw events, or renderer. Public
debug formatting is fully redacted.

Focused tests prove one borrowed projection, `Arc::ptr_eq` terminal identity,
success, typed semantic error, valid empty query, retry-only event exclusion,
terminal-event retention, redacted formatting, and no envelope across the
accepted failure seams. Full core passes 119 unit and 13 integration tests;
direct query/loading/analysis checks, GNU-Windows no-run linkage, formatting,
diff, exact scope, and unchanged six activation-blocker matches pass. The
focused correction rereview returned `ACCEPT`.

Design next only `WP-5-m1-source-aware-command-event-design`. Freeze the
smallest source-span event representation and publication boundary required to
match Bazel 9.2 Starlark print diagnostics before query-first activation.

### Source-aware command event design

Status: **ACCEPT** for `WP-5-m1-source-aware-command-event-design` on
2026-07-27 after one terminal independent design review and focused correction
rereview.

This packet remains design-only. It adds no Rust and authorizes no production
caller, CLI/server behavior, semantic activation, execution, REAPI, JVM,
Java-bytecode, or Bazel delegation.

#### Bazel 9.2 source of truth

The pinned local Bazel tag `9.2.0` resolves to
`8220c6198837d5c13d53fea211cf3282aa12408a`. Its observable contract is:

- `Event.makeDebugPrintHandler` in
  `src/main/java/com/google/devtools/build/lib/events/Event.java` constructs
  `Event.debug(thread.getCallerLocation(), msg)`;
- `Eval.evalCall` in
  `src/main/java/net/starlark/java/eval/Eval.java` sets the enclosing frame's
  program-counter location to `CallExpression.getLparenLocation()` immediately
  before the call. `StarlarkThread.getCallerLocation()` returns that enclosing
  frame location;
- `Location` in
  `src/main/java/net/starlark/java/syntax/Location.java` is the apparent file
  name plus optional 1-based line and column, with columns measured in UTF-16
  code units and zero line/column omitted; and
- `UiEventHandler.handleLocked` in
  `src/main/java/com/google/devtools/build/lib/runtime/UiEventHandler.java`
  writes `<KIND>: `, then `<location>: ` when present, then the message, and
  appends a line terminator only when the message does not already end in
  `\n`.

The accepted
`tests/v2_oracle/fixtures/load-invalidation/expected/oracle.json` evidence
therefore has `DEBUG: <path>:<line>:<column>: <text>` with column 6 for
top-level `print(` and column 14 for an eight-space-indented `print(`. Those
columns identify the actual `(` token. They are not the beginning of the call
expression and must not be reconstructed by adding the spelling length of
`print`, scanning source text, or consulting fixture text.

#### Retained Starlark source contract

Preserve the call token at parse time. Extend `CallArgsP` with the zero-width
span at the actual opening parenthesis, captured with the grammar token's
`@L`. Carry it as a distinct `FrameSpan` on `CallCompiled`, including through
optimization and inlining. Full call-expression spans remain the bytecode
instruction/profiling/error spans; only the location supplied to
`with_call_stack` becomes the exact parenthesis span. Synthetic compiler calls
that have no parsed token use the enclosing expression's end span. This also
makes retained call-stack program-counter locations Bazel-shaped without
discarding the existing full expression span.

Break the prototype `PrintHandler` API directly; add no compatibility method:

```text
PrintLocation {
    file: Arc<str>,
    line: u32,
    column: u32,
}

PrintHandler::println(
    &self,
    location: PrintLocation,
    text: &str,
) -> starlark::Result<()>
```

Re-export `PrintLocation` beside `PrintHandler` through both `stdlib.rs` and
the crate root; every external handler must be able to name the public method
argument without reaching a private module.

`print` and `pprint` resolve `Evaluator::call_stack_top_location()` immediately
before invoking the handler. The filename is the exact retained codemap
filename. Line is 1-based. Column is computed from the source-line prefix
ending at the saved parenthesis byte offset by counting `char::len_utf16`, then
adding one. Do not use the ordinary Unicode-scalar resolver or the
Bazel-internal byte-reporting resolver for this value: neither implements
Bazel's UTF-16 `Location` contract.

Change the retained real `CodeMapData.filename` backing from `String` to
`Arc<str>` while preserving the existing borrowed `filename() -> &str` API.
Add one crate-internal shared-filename accessor so resolving each print clones
only that Arc pointer. `CodeMap::new` still accepts `String` and performs the
single conversion when the codemap is created. Native/no-frame fallback uses
one static shared `<builtin>` value with line and column zero. Thus every print
from one codemap shares filename storage without retaining or cloning source
text, adding an interner, or changing a Cargo dependency.

The retained default stderr handler writes `<location>: <text>` with its
existing `eprintln!` behavior, matching Starlark's direct fallback rather than
the Bazel UI renderer. Every Slug capture handler receives the same
source-aware callback. `RejectPrint` may ignore the location. No handler derives
a path or position from an evaluator input, workspace fixture, function
spelling, or message text.

#### Retained event representation

Add this owned value in `slug_events_v2`:

```text
StarlarkSourceLocation {
    file: Arc<str>,
    line: u32,
    column: u32,
}
```

It has structural equality, `Allocative`, one exact public constructor, and a
`Display` implementation matching Bazel's zero-omission rules. Change only the
print variant to:

```text
EvaluationEvent::StarlarkPrint {
    location: StarlarkSourceLocation,
    text: CompactString,
}
```

MODULE, REPO, loading, and analysis capture handlers move or clone only the
codemap's shared filename pointer into that event. The filename is the apparent
name already passed to `AstModule`; it is not canonicalized,
workspace-relative rewritten, or recovered from a DICE key. Message bytes
remain exact. Diagnostics retain their existing level and already-located
text.

Keep `EventBatch` as `Arc<[EvaluationEvent]>` and `Dupe`. An event is allocated
once in its producing compute; filename duplicates are Arc pointer bumps, and
closure/output-buffer copies share the whole batch. Add no global interner,
source map, path table, `String`/`Vec` sidecar, per-command deduplication, or
event clone path. Structural event/batch equality includes file, line, column,
message, level, and order.

The four source-owning producer families are:

- `RootModulePrintCapture` in `slug_bzlmod_v2::module_eval`;
- `RepoPrintHandler` and `RecordingRepoEventReporter` in
  `slug_bzlmod_v2::repo_file`;
- `LoadingPrintCapture` in `slug_loading_v2::bzl_module`; and
- `AnalysisPrintCapture` in `slug_analysis_v2::dice`.

Registry MODULE evaluation remains deliberately silent where Bazel makes it
silent. Existing marker-conditioned capture remains unchanged. A reused warm
key contributes its previously retained batch only when it is in the selected
terminal activation closure; it does not replay the evaluator or append a
second event.

`DirectRepoEventReporter` is an explicitly preserved uncaptured path, not a
fifth event producer. `RepoPrintHandler` passes it the source-aware callback,
but the direct reporter deliberately ignores the location and keeps its
existing raw `eprintln!("{text}")` bytes. Only
`RecordingRepoEventReporter` converts the location into an event. This packet
must not turn capture-disabled REPO evaluation into a new DEBUG-rendering or
CLI publication path.

#### Consuming publication boundary

After source-aware events exist, add a public `#[must_use]`
`PublishedCommand<T>` with private terminal, exit-code, stdout, and stderr
fields. Its only extraction API is consuming:

```text
CommandOutput<T>::publish(self) -> PublishedCommand<T>

PublishedCommand<T>::into_parts(self)
    -> (T, i32, String, String)
```

`publish` consumes the still-opaque `CommandOutput<T>`, renders every selected
batch and event in exact closure order, and then consumes the private event
buffer. No terminal/event getter, iterator, clone, public constructor,
alternate renderer, or pre-publication parts API is added.

Render a print as `DEBUG: {location}: {text}`. Render a diagnostic as
`WARNING: {text}` or `ERROR: {text}` from its retained level; its text already
owns any source location. For either kind, append `\n` only when the retained
text does not already end in `\n`; the appended separator is `\r\n` on Windows
and `\n` elsewhere, matching `UiEventHandler.crlf()` and
`System.lineSeparator()`. A retained message that already ends in bare `\n`
is not rewritten on Windows because Bazel performs the same `endsWith("\n")`
test. Prefix only the first line of a multiline message. Event stderr precedes
the projected terminal stderr. Terminal stdout stays independent, so no
cross-stream order is invented. An empty buffer adds no bytes.

The existing terminal envelope timing does not change. Both
`Complete(Ok(...))` and `Complete(Err(...))` may own the one selected terminal
closure only after accepted snapshot replacement and successful lease close.
Retry-only, canceled, restoration, DICE, closure, native, materializer,
snapshot-replacement, and close failures publish nothing. Publication performs
no DICE computation, filesystem read, formatting query, or semantic retry.

#### Implementation scope

Implement this accepted design as one serial packet with two locally
reviewable phases and one terminal independent review. Phase one preserves the
source token and converts all producers; phase two adds publication. Do not
write a second design/status checkpoint between phases.

Production allowlist:

- `starlark-rust/starlark_syntax/src/codemap.rs`;
- `starlark-rust/starlark_syntax/src/syntax/{ast.rs,grammar.lalrpop,validate.rs,payload_map.rs,module.rs}`;
- `starlark-rust/starlark/src/eval/compiler/{expr.rs,call.rs,def_inline.rs}`;
- `starlark-rust/starlark/src/eval/bc/compiler/call.rs`;
- `starlark-rust/starlark/src/{stdlib/extra.rs,stdlib.rs,lib.rs}`;
- `app/slug_events_v2/src/lib.rs`;
- `app/slug_bzlmod_v2/src/{module_eval.rs,repo_file.rs,host_module.rs}`;
- `app/slug_loading_v2/src/bzl_module.rs`;
- `app/slug_analysis_v2/src/dice.rs`; and
- `app/slug_core_v2/src/runtime/{events.rs,mod.rs}`.

Tests colocated in those files and the existing direct event assertions in
`app/slug_bzlmod_v2/tests/root_module_dice.rs`,
`app/slug_loading_v2/tests/{bzl_invalidation.rs,build_file_loading.rs}`,
`app/slug_loading_v2/src/{host_package_load_tests.rs,host_package_attempt_tests.rs}`,
`app/slug_analysis_v2/tests/{starlark_rule.rs,root_analysis.rs}`, and
`app/slug_core_v2/src/runtime/{dice.rs,demands.rs}` may change mechanically to
assert the new field. No Cargo manifest change is permitted.

#### Required evidence and stop gates

Add the narrow retained-Starlark regression first. It must distinguish:

1. top-level and eight-space-indented `print` parenthesis columns 6 and 14;
2. whitespace between the callee and `(`, proving the token was preserved;
3. `pprint`;
4. a non-BMP scalar before the call, proving UTF-16 rather than Unicode-scalar
   or UTF-8-byte columns;
5. `<builtin>` fallback; and
6. exact multiline message preservation without handler-side rewriting.

Then prove exact source identities and locations at root MODULE, REPO,
BUILD/`.bzl` loading, and analysis producers; structural location inequality;
shared filename pointer identity across multiple prints from one codemap;
shared `EventBatch::dupe` storage; cold capture; unchanged warm reuse without
reevaluation or event replay; and absence when capture is disabled or the key
is outside the terminal closure. This matches the pinned Bazel 9.2
`unchanged_warm_build_no_replay` and `unchanged_warm_query_no_replay` oracle
rows. A focused REPO test must also prove that the
capture-disabled direct path retains raw message output semantics and creates
no source-aware event.

Focused core publication tests must prove mixed batch/event order, DEBUG and
diagnostic prefixes, UTF-8 and multiline bytes, no double newline, event
stderr before terminal stderr, stdout separation, empty-buffer identity,
retained terminal identity, success and typed-error publication, and that
primitive parts become reachable only through the consuming publication
method. Platform-conditional assertions must freeze the appended system line
separator and the unchanged already-LF-terminated case.

Run focused retained-Starlark, event, producer, and core publication tests,
then quiet direct compile checks for `slug_bzlmod_v2`, `slug_loading_v2`,
`slug_analysis_v2`, `slug_query_v2`, and `slug_core_v2`. Do not rerun their
unrelated full suites before the query integration milestone. Then run
GNU-Windows no-run linkage, formatting, `git diff --check`,
`scripts/v2_archive_status.sh`, the exact allowlist/no-Cargo guard, and the
existing six-match CLI/server activation blocker plus metric-only daemon
observation scans. Cargo commands remain serial and focused output remains
quiet.

Stop with `REPLAN` rather than:

- infer an opening parenthesis from the callee span or source/message text;
- report scalar or UTF-8-byte columns as Bazel locations;
- clone a codemap/source into an event or retain an event outside its batch;
- introduce a second output owner, renderer, public raw event/buffer API, or
  non-consuming terminal access;
- alter marker capture, DICE key equality, closure selection, retry,
  acceptance, snapshot, lease, or materializer semantics; or
- add a production caller, query/build activation, CLI/server behavior,
  execution, REAPI, JVM, Java bytecode, or Bazel delegation.

After terminal acceptance, schedule the exact implementation above. After its
acceptance, design the query-first atomic vertical activation; do not interpose
build execution work.

The first terminal review returned `REPLAN` because the public location type
needed two explicit re-export files, one exhaustive Host test match was outside
the allowlist, the uncaptured REPO stderr path was not frozen, and a
`CompactString` filename would allocate repeatedly for long apparent paths.
The correction adds the missing files, preserves raw direct-REPO output, moves
real codemap and event filenames to one evaluation-shared `Arc<str>`, and
freezes Bazel's platform line-separator behavior. The focused correction
rereview returned `ACCEPT`.

### Source-aware command event implementation

Status: **ACCEPT** for `WP-5-m1-source-aware-command-events` on 2026-07-27
after one terminal independent implementation review and focused correction
rereview.

The retained Starlark parser and compiler now preserve the exact opening
parenthesis independently from the full expression span. `print` and `pprint`
produce 1-based Bazel-shaped UTF-16 locations; real codemaps share one
`Arc<str>` filename, while missing and native frames share static
`<builtin>:0:0`. MODULE, recorded REPO, BUILD/`.bzl`, and analysis producers
retain those locations without changing the capture-disabled REPO path.

`EvaluationEvent::StarlarkPrint` now owns the structural source location.
The consuming `CommandOutput::publish` boundary renders selected events in
closure order with Bazel DEBUG/diagnostic prefixes and platform line
separators, then appends terminal stderr while leaving stdout independent.
The terminal value and primitive streams are reachable only through the
opaque consuming publication path.

Focused retained-Starlark, event, producer, and seven publication tests pass;
direct bzlmod/loading/analysis/query/core checks, GNU-Windows no-run linkage,
formatting, diff/archive/scope/no-Cargo guards, and the unchanged six-match
activation scan pass. The initial review found native-frame fallback and three
missing discriminators plus a release-only constructor invariant. The bounded
correction adds native/multiline/raw-REPO/loaded-`.bzl` evidence and a hard
location invariant; the focused rereview returned `ACCEPT`.

No production caller, activation, execution, REAPI, JVM, Java-bytecode, or
Bazel delegation was added.

Design next only `WP-5-m1-query-first-activation-design`. Freeze one atomic
typed-query vertical slice across core, one-shot CLI, daemon, and focused
equivalence tests; preserve metric-only filesystem observation and do not
interpose build execution.

### Query-first atomic activation design

Status: **ACCEPT** for `WP-5-m1-query-first-activation-design` on 2026-07-27
after one terminal independent design review.

This packet is design-only. It adds no Rust and authorizes no build activation,
execution, REAPI, JVM, Java-bytecode, or Bazel delegation.

#### Existing accepted path

`RootQueryCommandKey` already owns the complete loading-query semantic result
as
`SourcePreparationOutcome<Arc<Result<QueryOutput, QueryError>>>`. Its identity
includes source, order, policy, and output completion. The preactivation gate
proves that it reaches the Host anchor/loading graph without any eager
workspace snapshot or legacy semantic key. The private retained retry owner
already resolves typed Needs, selects one exact terminal closure, accepts the
native/materializer snapshot, closes the lease, and returns an opaque
`AcceptedCommand<T>`. `CommandOutput::publish` now renders the selected
source-aware events before terminal stderr and exposes primitive streams only
by consumption.

The only live blockers are adapters. One-shot query still observes a complete
legacy workspace and calls
`evaluate_workspace_query_with_policy_and_bzlmod_inputs_and_output_completion`.
The daemon query still passes a complete `WorkspaceObservation` to
`query_observations_with_policy_and_bzlmod_inputs_and_output_completion`.
Both then format the returned `QueryOutput` without the accepted envelope.
The daemon observation adapter also computes the public
`invalidated_files` compatibility metric; its successful observation values
must be discarded rather than injected into the typed command.

#### Core command API and one retained loop

Activate the existing retry owner; do not add a parallel query loop. Replace
the synthetic-only dispatch with one private `NativeCommandRoot` abstraction
having an associated cloneable terminal and one async compute operation.
Implement it only for the existing synthetic root adapter and
`RootQueryCommandKey`. Generalize the current concrete loop to
`drive_command<R: NativeCommandRoot>` without an async closure, HRTB, boxed
future, second guard, or second acceptance owner. Existing synthetic
cancellation/failure seams and metadata remain test-only behavior of their
concrete adapter.

Add exactly these public entry points:

```text
WorkspaceRuntime::query_command_with_policy_and_bzlmod_inputs_and_output_completion(
    &self,
    expression,
    order,
    policy,
    command_policy,
    environment_policy,
    lockfile_mode,
    registry_urls,
    completion,
) -> Result<
    AcceptedCommand<Arc<Result<QueryOutput, QueryError>>>,
    QueryError,
>

evaluate_workspace_query_command_with_policy_and_bzlmod_inputs_and_output_completion(
    workspace,
    expression,
    order,
    policy,
    command_policy,
    environment_policy,
    lockfile_mode,
    registry_urls,
    completion,
) -> Result<
    AcceptedCommand<Arc<Result<QueryOutput, QueryError>>>,
    QueryError,
>
```

The one-shot wrapper constructs one retained runtime but performs no
`observe_workspace` call. Both entry points validate the query key and
registry URL set before acquiring a lease, build the fixed
`NativeDemandRequestInputBundle`, and map private session/infrastructure
failures once into a stable `QueryError::evaluation`. A terminal query
`Ok` or `Err` remains inside the accepted `Arc`; no `QueryOutput` or
`QueryError` clone is introduced. Parse/registry/preflight failures produce no
envelope and therefore no selected output.

Keep the legacy observation-based query wrappers and their legacy-only tests
temporarily, but remove every activated CLI/server call to them. This packet
does not retire workspace snapshots, old query functions, or build adapters.

#### Adapter publication

One-shot CLI imports only the new one-shot command entry point. Daemon query
calls only the new retained-runtime command method. Each adapter immediately
consumes the envelope through:

```text
accepted
    .project(|terminal| TerminalOutput::new(exit_code, stdout, stderr))
    .publish()
    .into_parts()
```

The borrowed projection matches the retained `Arc<Result<...>>` and formats
the already-computed output exactly once. Text/label, graph, label-kind, and
package retain their current formatters and completion choice; formatting
does not re-enter DICE. Query errors retain exit code, escaping, and the
`Evaluation of query` suffix. One-shot and daemon retain their current
`runtime_mode`; daemon errors also retain `invalidated_files`.

Terminal JSON stderr ends in one `\n`. CLI writes published stdout and stderr
as exact bytes rather than using an extra-line `eprintln!`. The daemon
response retains the published bytes and its client likewise writes them
without adding another newline. Selected DEBUG/diagnostic events therefore
precede the terminal error, successful query prints are visible on stderr,
and event-only success gains no blank line. Outer preflight/infrastructure
errors use the same adapter formatter but have no selected event prefix.

Daemon query still calls `FilesystemObservationAdapter::observe` before the
typed command to preserve its existing invalidation count and observation
failure behavior. Name and discard the returned observation value; only the
count enters the response/projector. It must not be passed to core, stored as
command input, or enter the selected closure. Build continues using its legacy
observation value unchanged. Reject an unsupported daemon output string before
metric observation or semantic query evaluation.

#### Exact implementation scope

Production files:

- `app/slug_core_v2/src/runtime/{dice.rs,mod.rs}`;
- `app/slug_cli_v2/src/commands/query.rs`; and
- `app/slug_server_v2/src/lib.rs`.

Focused tests may change only:

- colocated tests in `app/slug_core_v2/src/runtime/dice.rs`;
- `app/slug_cli_v2/tests/cli.rs`; and
- `app/slug_server_v2/src/tests.rs`.

No query-language, loading, analysis, Starlark, event, protocol-schema, Cargo,
build-command, or execution file changes are permitted.

#### Required evidence and guards

Core evidence must run a real `RootQueryCommandKey` through the generalized
driver and prove valid-empty plus direct-label success, typed missing-target
error, retry progress, one accepted terminal, selected MODULE/`.bzl`/BUILD
events once in dependency order on cold or changed evaluation, no replay on an
unchanged warm command, no retry-only event, and zero activation of every eager
snapshot/legacy key from the preactivation gate. Existing
synthetic cancellation, restoration, replacement, close, and publication
tests remain green.

One focused CLI integration must run the same small workspace one-shot and
through the daemon and compare exit code/stdout plus semantic stderr after
allowing only the frozen runtime-mode/metric JSON fields to differ. Cover
empty output, one direct label, one dependency query, one missing target, one
syntax error, and one source-aware print success/error row. Assert exact
single newlines and no replayed print on a warm daemon query. A focused server
test must prove the invalidation count remains correct while typed query
results follow create/edit/delete/restore and no observation value reaches a
forbidden key.

Run only focused core driver/publication, CLI equivalence, and daemon metric
tests; quiet direct checks for core, CLI, server, query, loading, and analysis;
GNU-Windows no-run linkage for changed crates; formatting, diff, archive,
exact-file/no-Cargo guards; and these activated-surface scans:

```text
rg -n 'evaluate_workspace_query_with_policy_and_bzlmod_inputs_and_output_completion|query_observations_with_policy_and_bzlmod_inputs_and_output_completion' \
  app/slug_cli_v2/src/commands/query.rs app/slug_server_v2/src/lib.rs

rg -n 'evaluate_workspace_query_command_with_policy_and_bzlmod_inputs_and_output_completion|query_command_with_policy_and_bzlmod_inputs_and_output_completion' \
  app/slug_cli_v2/src/commands/query.rs app/slug_server_v2/src/lib.rs

rg -n 'observe_workspace|WorkspaceObservation' app/slug_server_v2/src/lib.rs
```

The first scan must be empty, the second must identify only the new one-shot
and daemon calls, and the third may retain observation use only in build plus
the adapter/metric query chain. The broader preactivation scan must fall from
six matches to the three build-only matches.

Stop on any eager snapshot injection, legacy semantic key activation, raw
terminal/event access, terminal clone, second retry/accept/publication owner,
retry publication, metric ownership/protocol change, query evaluation during
formatting, build activation, execution, REAPI, JVM, Java bytecode, or Bazel
delegation.

The terminal review returned `ACCEPT`. It confirmed that the four-file
production boundary can activate both query adapters through one retained
retry/accept/publication owner while preserving opaque terminals, exact stream
bytes, metric-only daemon observation, and the no-JVM/no-Bazel-delegation
boundary.

Implement next only `WP-5-m1-query-first-activation`.

### Query-first atomic activation implementation

Status: **ACCEPT** for `WP-5-m1-query-first-activation` on 2026-07-27 after
one terminal independent implementation review and one focused correction.

One-shot CLI and retained-daemon query now use the existing typed
`RootQueryCommandKey` through the sole retained retry/accept/publication
owner. The public command entry points return only the opaque
`AcceptedCommand<Arc<Result<QueryOutput, QueryError>>>`; adapters consume it
once through projection and publication. No activated query caller injects a
legacy workspace snapshot. Daemon filesystem observation remains metric-only,
and invalid output is rejected before observation or semantic evaluation.

Focused evidence proves direct-label, dependency, valid-empty, typed
missing-target, syntax, exact JSON newline, cold MODULE/`.bzl`/BUILD event
order, exact changed-BUILD publication, and no unchanged warm replay. The
retained create/edit/delete/recreate and invalidation-metric tests remain
green. Synthetic retry, acceptance, restoration, and publication tests pass;
quiet core/CLI/server/query/loading/analysis checks, formatting, diff, archive,
scope, no-Cargo, activated-call, metric-only observation, and no-delegation
guards pass. GNU-Windows no-run linkage passes for the changed core; CLI/server
remain blocked by the pre-existing unguarded Unix-socket transport in
`slug_server_v2/src/server.rs`.

The independent review found one outer daemon preflight-error path without its
terminal newline. The correction adds the newline, freezes exact unsupported
output JSON, and strengthens changed-event/invalidation evidence; rereview
returned `ACCEPT`.

No build activation, execution, REAPI, JVM, Java-bytecode, or Bazel delegation
was added. Simple loading-query operations are now on the typed production
path.

Design next only `WP-5-m1-build-activation-design`. Freeze an atomic
typed-build CLI/server activation through the same accepted envelope and
metric-only observation boundary. It may expose loading/analysis results only;
it must not add action execution, REAPI, JVM, Java bytecode, or Bazel
delegation.

### Typed build atomic activation design

Status: **ACCEPT** for `WP-5-m1-build-activation-design` on 2026-07-27 after
one terminal independent design review.

This packet is design-only. It adds no Rust and authorizes no new execution,
action semantics, REAPI behavior, JVM, Java bytecode, or Bazel delegation.

#### Live boundary and execution preservation

`BuildCommandRootKey` already owns the complete Host loading/optional-analysis
terminal as
`SourcePreparationOutcome<Arc<Result<BuildCommandEvaluation,
BuildCommandError>>>`. Its accepted preactivation tracker proves zero eager
workspace snapshot or legacy loading/analysis activation. The query packet
activated the sole generic retry, exact closure selection, acceptance, and
consuming publication owner.

The remaining three activated legacy build calls are the one-shot import/call
to `evaluate_workspace_targets_with_bzlmod_inputs` and the daemon call to
`evaluate_observations_with_bzlmod_inputs`. Both create complete workspace
snapshots before semantic evaluation. Daemon observation also owns the public
`invalidated_files` compatibility metric.

Both adapters already support native REAPI execution after loading/analysis.
That is retained behavior, not part of the typed DICE transaction. The new
root stops at the accepted analysis terminal. Existing CLI and server REAPI
helpers remain the sole downstream execution/materialization owners and run
only from the terminal projector when `RemoteMode::Execute` was already
selected. Do not add a DICE execution key, action mutation, executor,
materializer, local fallback, JVM, or delegation path.

#### Public typed command boundary

Make `BuildCommandEvaluation` a public type with private fields. Replace the
private error enum at the public boundary with an opaque public
`BuildCommandError` wrapper around a private, equality-preserving kind; do not
export its variants or underlying loading/analysis error types. The evaluation
exposes only borrowed summary accessors:
`loaded_package_count`, `analyzed_target_count`, `declared_action_count`, and
an iterator over borrowed `AnalysisResult` values for the existing REAPI
helpers. It exposes no anchor, requested-target storage, DICE identity,
revision, event, demand, lease, or mutable action access. Give
`BuildCommandError` stable `Display`/`Error` behavior matching the current
adapter diagnostics for root-anchor, package, missing-target, analysis,
external-repository, recursive-pattern, and infrastructure failures.

Implement `NativeCommandRoot` only for the existing `BuildCommandRootKey`,
using the retained
`Arc<Result<BuildCommandEvaluation, BuildCommandError>>` terminal. Keep one
fixed target configuration,
`ConfigurationKey::target("first-build")`, matching the legacy adapter.
Normalize the root key and registry URLs before lease acquisition. Map key
preflight and retained-session failures once into the public build error; a
semantic `Ok` or `Err` remains inside the accepted `Arc`.

Add exactly these core entry points:

```text
WorkspaceRuntime::build_command_with_bzlmod_inputs(
    &self,
    targets,
    command_policy,
    environment_policy,
    lockfile_mode,
    registry_urls,
) -> Result<
    AcceptedCommand<Arc<Result<BuildCommandEvaluation, BuildCommandError>>>,
    BuildCommandError,
>

evaluate_workspace_build_command_with_bzlmod_inputs(
    workspace,
    targets,
    command_policy,
    environment_policy,
    lockfile_mode,
    registry_urls,
) -> Result<
    AcceptedCommand<Arc<Result<BuildCommandEvaluation, BuildCommandError>>>,
    BuildCommandError,
>
```

The one-shot wrapper creates one retained runtime and performs no
`observe_workspace` call. Keep legacy wrappers and legacy-only tests dormant
until a later retirement packet, but remove every activated CLI/server caller.

#### Adapter projection and exact streams

Parse command and environment inputs as today. One-shot retains its current
remote-configuration timing after semantic evaluation; daemon retains the
already-parsed request value. Each adapter consumes the accepted envelope
exactly once through `project(...).publish().into_parts()`.

For non-execute mode, the projector computes the existing counts and exact
`analysis_not_implemented` JSON from borrowed typed data. For execute mode, it
passes the borrowed evaluation to the existing CLI/server REAPI helper and
turns that helper's existing outcome into `TerminalOutput`; execution does not
escape the projector or become a second output owner. Adapt the helpers only
to consume borrowed typed analyses and return primitive outcome data. Do not
clone the accepted terminal, analyses, or actions.

Every terminal JSON stderr ends in exactly one `\n`. Selected source-aware
events precede it. One-shot and daemon clients write published bytes with
`eprint!`, not `eprintln!`. Preserve all current JSON fields, exit codes,
runtime modes, invalidation counts, completed-boundary values, REAPI evidence,
and materialized-output behavior. Outer parse/preflight/infrastructure errors
use the same exact newline rule and have no selected-event prefix.

Daemon still calls `FilesystemObservationAdapter::observe` before the typed
command. Discard the observation value and retain only its failure behavior
and invalidation count; no observation enters core or the selected closure.

#### Exact implementation scope and evidence

Production files:

- `app/slug_core_v2/src/runtime/{dice.rs,mod.rs}`;
- `app/slug_cli_v2/src/commands/build.rs`; and
- `app/slug_server_v2/src/{lib.rs,reapi.rs}`.

Focused tests may change only colocated core tests,
`app/slug_cli_v2/tests/cli.rs`, and
`app/slug_server_v2/src/tests.rs`. No event, query-language, loading,
analysis, command-parser, protocol-schema, REAPI-crate, Cargo, or execution
fixture file may change.

Core evidence must drive the real build root through the shared retained
driver and prove empty, package-wide, native direct, Starlark-analysis, and
typed missing-target results; retry progress; one accepted terminal; exact
cold and changed MODULE/`.bzl`/BUILD/analysis events; no unchanged warm or
retry-only replay; and the accepted zero-forbidden-activation gate.

One focused CLI test must compare one-shot and daemon native/Starlark/missing
rows, source-aware success/error ordering, counts, completed boundary, exit
code, and exact single newlines while allowing only runtime-mode and metric
fields to differ. Focused daemon evidence must retain create/edit/delete/
restore invalidation counts. Preserve the existing native REAPI result and
materialization shape with a focused helper/integration regression; do not
claim loading/analysis success as build success.

Run focused core/CLI/server tests; quiet direct checks for core, CLI, server,
loading, and analysis; available native REAPI evidence; core GNU-Windows
no-run linkage; formatting, diff, archive, exact-file/no-Cargo guards; and:

```text
rg -n 'evaluate_workspace_targets_with_bzlmod_inputs|evaluate_observations_with_bzlmod_inputs' \
  app/slug_cli_v2/src/commands/build.rs app/slug_server_v2/src/lib.rs

rg -n 'evaluate_workspace_build_command_with_bzlmod_inputs|build_command_with_bzlmod_inputs' \
  app/slug_cli_v2/src/commands/build.rs app/slug_server_v2/src/lib.rs

rg -n 'observe_workspace|WorkspaceObservation' app/slug_server_v2/src/lib.rs

rg -n 'slug_reapi|execute_action|materialize_outputs|RemoteMode' \
  app/slug_core_v2/src/runtime/{dice.rs,mod.rs}
```

The first and fourth scans must be empty. The second identifies only the new
one-shot and daemon calls. The third retains observation use only in the
metric adapter/query chain. Full CLI/server GNU-Windows linkage remains
separately blocked by the pre-existing Unix transport and is not evidence
against this packet.

Stop on a second retry/accept/publication or execution owner, eager snapshot
injection, terminal/action clone, output after publication, changed REAPI
semantics, event replay, metric/protocol ownership change, build success
without execution, new action execution, JVM, Java bytecode, or Bazel
delegation.

The terminal review returned `ACCEPT`. It confirmed that the synchronous
borrowed projector runs only after the retained driver's internal DICE
transaction, can preserve the existing native REAPI helpers and action order,
and returns primitive terminal output without exposing the accepted value or
creating another output/execution owner.

Implement next only `WP-5-m1-build-activation`.

Implementation-review correction: the original seven-file allowlist was too
narrow to preserve the accepted adapter diagnostic. The typed root's opaque
loading error exposed `root package source is missing` where the activated
legacy path emitted Bazel-shaped
`cannot load '//pkg:defs.bzl': no such file`. Expand production scope only to
`app/slug_bzlmod_v2/src/host_package.rs` for a typed missing-source predicate
and `app/slug_loading_v2/src/bzl_module.rs` for the owning load-error
projection. Adapter string rewriting is forbidden. The existing loading and
server missing-then-create regressions must retain the exact Bazel-shaped
message and same-daemon recovery.

### Typed build atomic activation implementation

Status: **ACCEPT** for `WP-5-m1-build-activation` on 2026-07-27 after one
terminal independent implementation review and two focused diagnostic
corrections.

One-shot CLI and retained-daemon build now use `BuildCommandRootKey` through
the same sole retained retry/accept/publication owner as query. The public
terminal and error are opaque; adapters receive borrowed counts and ordered
analyses only inside the consuming projector. No activated build caller
injects a legacy workspace snapshot. Daemon observation values are discarded
after retaining failure behavior and `invalidated_files`.

Non-execute output retains exact loading/analysis counts, exit code, runtime
mode, completed boundary, and one terminal newline. The existing CLI/server
native REAPI helpers now borrow the typed analyses inside the projector,
preserving target/action order, evidence fields, and materialization behavior
without adding an execution owner or moving execution into DICE. Selected
cold and changed MODULE/`.bzl`/BUILD/analysis events precede terminal JSON;
unchanged warm commands do not replay them.

Focused evidence covers the real retained driver, empty/native/Starlark/
missing terminals, eight dormant root cases, retry/acceptance behavior,
one-shot/daemon source events, changed invalidation, warm no-replay,
missing-then-create, and the no-action native REAPI projector. Direct and
transitive missing `.bzl` diagnostics retain
`cannot load '<deepest canonical label>': no such file` and same-daemon
recovery. The formatter follows the existing Bazel-shaped `load_error`
contract in `slug_loading_v2/src/bzl_module.rs`; no adapter rewrites strings.

Quiet bzlmod/loading/analysis/core/CLI/server checks, focused tests, bzlmod/
loading/core GNU-Windows no-run linkage, formatting, diff, archive, exact
scope/no-Cargo, activated-call, metric-only observation, and core
no-REAPI/no-delegation guards pass. Full CLI/server Windows linkage remains
blocked only by the pre-existing Unix transport. The independent review found
the direct diagnostic and transitive-label issues; both corrections passed
focused rereview and the final result is `ACCEPT`.

No new action execution, REAPI behavior, JVM, Java bytecode, or Bazel
delegation was added.

Design next only `WP-5-m1-external-repository-query-routing-design`. Freeze
the smallest observable typed-query vertical slice that resolves one external
repository label through Bazel 9 repository mapping and the accepted native
source-preparation/materialization owners. Do not broaden into build
execution, general repository discovery, JVM, or Bazel delegation.
