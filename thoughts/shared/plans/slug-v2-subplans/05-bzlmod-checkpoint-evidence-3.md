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
