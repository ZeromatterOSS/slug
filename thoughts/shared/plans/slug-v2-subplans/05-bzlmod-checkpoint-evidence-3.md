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
