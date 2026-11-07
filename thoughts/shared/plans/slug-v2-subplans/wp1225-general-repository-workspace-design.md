# WP425 implementation design v2

Implementation design for `WP-7-425-m7a-general-repository-process`, based on
`d202a77e29b4fda077e69bc17ea4874c4635e22a`. Inactive implementation is selected;
production activation remains gated below. The [backing receipt](wp1225-repository-root-backing-2026-11-07.json)
records the three-launch rolling lease and scaled same-backing quota transition.
It proves fixed page-data/inode behavior, with metadata separate; it proves
neither confinement nor Starlark API/source observation. Detached-anchor cloning
failed with EINVAL and is rejected. Every later mount lease is minted from an
actually attached root before its namespace exits.

## Compatibility and semantic owner

Evaluate actual originals through one general API; remove GCC ProbeForm/argv/
expected-effect and Homebrew selectors together. No caller/source/tool profile
selects semantics or permission. Exact admitted Bazel 9.2: operation order,
String/Path/Label arguments, env override/removal, cwd/quiet suppression, actual
normal exit code and Latin1 streams, sequential effects including nonzero writes.
Slug-native: display/physical identity, bounded Linux Host sampling and quotas,
completion-batch quiet=False timing/cross-stream order, infrastructure refusals.
Networking, async, arbitrary Host mutation, cross-command process replay and
unadmitted nodes/escaping links are unsupported. Preserve admitted encoding;
do not infer arbitrary Java Unicode/template parity. Pinned API anchors are in
[the API contract](wp1225-repository-api-source-contract.md).

Bzlmod owns immutable full descriptor/reservation, operation, predecessor/next
state, record, transcript, epoch and terminal realization. Core alone owns root,
mutation, children, backing/FDs, quiescent capture, adoption and release. DICE
stores no native Arc/FD/reader/sink/registry/child. On Need, extract request,
drop transaction/drain cancellation workers, perform native work, inject complete
immutable observation and reevaluate. No shared mutex, transaction or Starlark
evaluation spans child work; see [DICE](../../../../docs/developers/dice.md).

## Physical root, spelling and repeated attachment

Closed Core `Arc<RepositoryPhysicalRoot>` contains display_root, nonzero instance,
checked rootFD/dev/inode/mount identity, backing lifetime and positively admitted
backing/control/FD credits. The rolling detached mount token and setup userns
authority belong exclusively to native mutable progress; sources retain sealed
backing without any mount operation or mutation method.
Reserve before original evaluation exposes Path text. All GENERAL workspace
roots start quota-backed in Archive mode; legacy plain archive realization stays
unchanged. No TempDir process bind, copied root, unsupported archive-mix shortcut,
privileged fallback or keeper process is selected.

Trusted setup attaches incoming detached lease DIRECTLY via move_mount in its
proven owner context, then creates/transfers NEXT detached clone from the ACTUAL
ATTACHED root before cap drop/payload. Current token is consumed, never reused.
Core authenticates/owns next token but cannot launch it until current child and
descendants quiesce, complete snapshot and current record acceptance. Exclusive
progress capability controls token rollover; sources gain no mount authority.
No detached-anchor cloning or mount pool is used; the fixed probe demonstrates
this chain. Pre-untrusted-exec transfer and adversarial lifecycle remain to prove.
Adoption transfers the SAME root Arc; sealed roots cannot mutate. Native source
leases/readers retain it until the last user closes; Arc clones duplicate no FD.

Use stable collision-safe normalized absolute spelling from FULL workspace/repo
owner identity; check structural collisions. Fresh command gets fresh backing
and instance at that SAME spelling. Materialization(A)/path and
Materialization(B)/path remain distinct while overlapping. Each child binds its
selected generation there; strings/argv/bodies are never rebased or rewritten.
Core may have no Host path at that name. No path-only lookup, canonicalize,
proc-magiclink alias or global replacement grants authority. Raw Host-path
runfiles consumers refuse inaccessible roots; published runfiles use existing
durable SourceBacking until a real alias has separate access/lifetime/ABA proof.

## Archive envelope and irreversible process transition

Legacy plain archive behavior remains 512 MiB/call, 1 GiB cumulative DECODED,
with its existing bounds. Select general-root Archive mode with **tmpfs size=
2 GiB, nr_inodes=262144, actual page size4096 ONLY**. Keep unchanged decoded
cap1 GiB, terminal131072entry/4MiB-path envelope and existing depth256 deletion
semantics; process limits do not apply before transition. Refuse other page sizes
until the bound is adapted/reviewed. This allowance covers allocation slack;
never debit it from admitted payload or claim size= bounds all kernel memory.

Source-derived data bound: [archive decoder](../../../../app/slug_core_v2/src/runtime/repository_generated_archive.rs)
has1024 headers/call and paths<=256bytes/32components; [terminal manifest](../../../../app/slug_core_v2/src/runtime/repository_filesystem_manifest.rs)
limits logical bodies1GiB and entries131072. For data-bearing entries, sum of
page-rounded storage <=1GiB +131072*(4096-1)=1610481664bytes (<1.5GiB).
The selected2GiB leaves >512MiB additional data-block allowance;262144inodes
exceeds the complete-tree131072entry bound, accommodating root/security-related
inode charging. Actual quota proof must check such charging and admitted literal
link/directory storage, not assume they are free. Kernel inode/slab memory and
owned metadata/control allocations need separate positive credits and workload
measurement. The formula bounds admitted complete states, not arbitrary transient
unbounded writer history; trusted archive mutation must stay bounded and close
replaced outputs. Existing extraction truncates in place, not copy-on-replace.
A scaled shrink proves mechanism, not worst-case envelope or whole-memory fit.

Before FIRST execute: quiesce mutation, authenticate exact lease/predecessor,
admit actual current COMPLETE tree and all cumulative retained process-domain
history, check physical allocated blocks/inodes, then trusted same-superblock
remount to size=64 MiB and nr_inodes=8192. RootFD/dev/inode and strings persist;
verify actual post-remount limits before child launch. Failed admission/remount
refuses generation BEFORE child; never copies/restarts/relabels it. No later
expansion from Process to Archive. Later archives retain old per-call/decoded
caps AND must fit process caps; successful mixed sequences use this SAME root.

Process snapshot bounds: **8192 entries INCLUDING root**, depth<=64 below root,
each logical absolute demand/path<=4096 bytes (including display prefix), and
aggregate root-relative path bytes plus literal link-target bytes<=1 MiB per
state. Body content retained across command history<=64 MiB, cumulatively with
no refund; shared observed-equal immutable bodies charged once, every new
version charged. Metadata/control arrays, state-version count, descriptors and
scratch also need positive checked credits before allocations; never infer their
memory cost from body bytes. Use 8192 total inode capacity as separate physical
cap; regular-file hardlinks remain unsupported, avoiding hidden inode aliases.

These proposed process bounds are intentionally NARROWER than accepted archive
manifest and [delete](../../../../app/slug_core_v2/src/runtime/repository_delete.rs)
131072 entries/4 MiB paths/depth256. Apply them on process admission and later
process state, not to legacy or pretransition archive-only completion. A valid
archive with 8193 entries/depth65 can complete archive-only but cannot execute.
Absolute4096 matches existing manifest interpretation; accepting a relative4096
PLUS display prefix would widen it. Body/history failure after large prior
archive states is a declared Slug-native process admission limit, not a rollback
or archive compatibility failure. Retain admitted exact/+1 archive regressions.

## Trusted mutation below the same root descriptor

For general roots absent from Core's Host namespace, every trusted filesystem
operation is descriptor-relative: generated file/template application, archive
extraction and prefix creation, delete/no-follow traversal, complete snapshot
and terminal manifest. Adapt `repository_io::apply_generated_repository_file`,
`repository_generated_archive.rs`, `repository_delete.rs` and
`repository_filesystem_manifest.rs` to closed rooted mutation/read primitives.
Reuse credited input-image openat/O_NOFOLLOW component handling; apply archive
budgets and regularity before writes. Do not use Host aliases, proc reopening,
ambient display paths or copying to a second root. Legacy plain roots keep their
existing adapter. Test file→archive→execute→delete→template on the same actual
root while its display path is absent or names an unrelated Host tree.

## Sequence, snapshot, API and freshness

One lease/descriptor/owner/ordinal/index/predecessor-bound sequence covers file,
template, read/exists, archive/delete and execute. Full structural equality,
not digest-only equality, authenticates replay. Complete state includes actual
directories/modes, all regular bodies, deletions and admitted literal links.
Loading deterministic effects consume current state; Core applies only exact
new suffixes. No stale file plan overwrites process output. Read/exists observe
process writes/deletes; normal nonzero exits return actual result AND next tree.
Private consuming QuiescedProcessResult requires descendant death/reap before
snapshot. Pipe EOF is insufficient. Unsupported nodes, instability, overflow,
timeout/cancel/setup/signal/delivery failure kill/reap then discard generation.

Keep128 executes, 1 MiB EACH stream, operational30 seconds. Requested timeout
default600 remains structural; validate argv then strict positive signed-i32
timeout then ALL env string keys then String/None values. Every argv position
accepts actual String/Path/Label; retain ordinary tracked path/Label Needs.
Full base env plus overrides/removals/cwd/quiet/options participate in equality.
Create cwd only in allowed writable root. Use immutable nominal exec_result;
normal bytes Latin1, no synthetic256 for infrastructure refusal.
Quiet=True suppresses forwarding only. Quiet=False selected completion event
delivers stdout then stderr through borrowed current stderr sink, between
surrounding original prints. Reconcile delivered selected-event prefix over
Need retries; no duplicates, sink in DICE or global stderr from Core.

Fresh active EMPTY workspace epoch is demanded BEFORE originals/prepared/
materialization reuse. New operation index ALWAYS samples Host, even equal argv;
only exact current completed index/predecessor replays. Never relabel old records
with a new nonce. Terminal realization must match current selected request/result
and SAME current root before adoption. Revoke at command end. Fresh equal
results may cut off semantic projections only AFTER actual sample; old physical
instance authority stays distinct. Untracked Host helper changes before warm
reuse must cause a new sample. Keep source/mapping/env/certificate dependencies.

## Anchored source IO and native transport

Add immutable selected MaterializationPathAnchor and anchored ResolvedPath key:
stop ancestor recursion at anchor, observe REAL fstat(rootFD), resolve children
and record ordinary symlink route/epochs/cycle checks. Above-anchor/foreign/
revoked/escaped demands refuse; Host resolver unchanged. Internal relative or
absolute-under-same-display links may resolve within instance; no automatic Host
conversion. Common Bzlmod source route supplies anchor; Analysis follows it.

Core registry holds exact root Arc plus FULL current selected request/result/
instance association. Snapshot owners under lock, drop lock before IO, recheck
scope/session afterward. Lstat/readlink/dir/bytes/digest/image all use checked
FD-relative fstatat/readlinkat/openat O_NOFOLLOW component walks; final resolved
open must not bypass recorded symlink route. Reuse input_image Linux primitives.
Split credited regular-file capture into FD kernel plus Host adapter; materialized
reads never call read_host_file or proc reopen. Initial/preflight/final checks
use SAME rooted registry, demand/result Arc, full epochs and source budgets.
Image variant retains root owner/FD and validates instance/sealed generation,
not an ambient reopened anchor. GeneralSource retains exact FileBytes Arc only.

The inactive rooted observation leaf borrows RootIo plus the already selected
anchor and scope. It validates the complete demand batch before IO: supported
operations, matching nonzero instance, component-contained absolute paths of at
most 4096 bytes, duplicates, at most 131072 demands and at most 4 MiB of retained
path capacity. Scoped credits cover batch construction and hold link/directory
result storage through epoch publication. These are Slug-native operational
bounds, not a change to the Host resolver or a proof of native root selection.
Directory collection counts borrowed names first, then admits allocation and
conversion/sort peaks before retaining them in a second checked pass.

Regular-file capture shares one credited FD read kernel. The Host/Registry
adapter retains its existing pin/reopen/name checks and diagnostics. The rooted
adapter consumes an already-readable descriptor with positional reads from zero;
it checks FD identity and rechecks the rooted name before publishing any result.
The observer charges each attempt before opening, including missing/wrong-kind
failures, and calls the private already-charged entry point. FileBytes keeps its
existing 1 MiB object cap. Digest streams through RootIo's existing SHA kernel,
with fixed scratch and cumulative byte charges up to the selected RootIo file
bound; it does not allocate a FileBytes object or inherit that object's cap.
Temporary handle/scratch credits do not establish physical FD or kernel-memory
admission. Sealed/exclusive-quiescent ownership, exact root/anchor association,
registry validation and final source/transport integration remain separate gates.

PreparedSourceActionInputs/PreparedActionChainInputs stay resource-free. At native
completion mint NativeSourceActionInputs/NativeActionChainInputs = immutable
metadata Arc + closed NativeSourceAccess from full current selected source set/
namespace/digest membership and NativeSourceGenerations. open_source(index)
returns NativeSourceReader(File+root lease+credit), with rooted no-follow reads
or unchanged Host policy. SourceActionTransport::stage/ActionChainTransport::start
receive wrappers; REAPI plans/sessions retain them through upload and verify
actual bytes/digest. No FD in DICE/global lookup; durable runfiles consume reader.

## Confinement and implementation split

Before untrusted execute, uniform LinuxReadOnlyHostV1 requires recursive RO/
nosuid/nodev Host mounts/private propagation, private proc/minimal devices and
bounded tmp, isolated PID/net/IPC/UTS, root as only persistent writable storage,
no_new_privs, cap drop and architecture/ABI-checked DEFAULT-DENY seccomp.
Explicitly refuse socket/connect/device ioctl/io_uring/ptrace/process_vm/
pidfd_getfd/keyring/BPF/perf/fanotify/file-handle/mount/setns/unshare escapes;
constrain clone flags, initially refuse clone3/socketpair. Unknown ABI/x32 and
syscalls refuse. Trusted setup consumes root/userns/mount/filter/status/control
FDs; payload inherits ONLY0/1/2. Reuse closefrom/reaper tests, prove detached
writers closing stdio dead before capture. Exact allowlist/setup binding and
actual socket/device/FD/nested-mount tests remain PREACTIVATION gates.

| Owner | Files under app/ (new leaves explicitly named) |
| --- | --- |
| Bzlmod protocol | `slug_bzlmod_v2/src/repository_{execute,filesystem}.rs`; new `repository_workspace.rs`; exports. |
| Loading API/events | `slug_loading_v2/src/repository_rule_context.rs`, its `{filesystem,deletion,archive}.rs`; `module_extension_repository_file_effect.rs`. |
| Anchor/shared route | `slug_workspace_v2/src/path_resolution.rs`; Bzlmod `source_preparation/repository_source_observation.rs`; root integrates `source_preparation.rs`. |
| Core physical/snapshot | `slug_core_v2/src/runtime/repository_{process,filesystem_progress}.rs`; new `repository_{physical_root,workspace_snapshot,process_policy}.rs`; `repository_generated_archive.rs`, `repository_delete.rs`, `repository_filesystem_manifest.rs` and rooted generated-file application in `repository_io.rs`. |
| Core rooted observers | `runtime/path_observation.rs`, its `{file_digest,scoped_directory}.rs`; `source_service/regular_file.rs`; `input_image.rs`, `input_image/linux/fs.rs`. |
| Native reader/REAPI | Core `repository_io/source_generations.rs`, `{source,action_chain}_{staging,execution}.rs`, `runfiles_manifest.rs`, `action_chain_staging/runfiles.rs`; new `native_source_access.rs`; REAPI `{source_staging,action_chain}.rs`. |
| Root integration | Core `runtime/{repository_io,dice,mod}.rs`, Bzlmod `source_preparation.rs`, exports/acceptance/current epoch; atomic activation and scheduling. |

Pinned API/source anchors are retained in the linked source contract. Reviewed protocol,
anchored resolver and rooted observer candidates can be authored inactive before
complete syscall proof once root accepts design; no unconfined execution follows.
Remaining gates: pre-payload lease transfer, archive envelope/inode and exact/+1 caps,
full confinement/lifecycle, A/B same-name distinct-instance source/digest/image/
REAPI reads, generalSource certificate restore, unrelated originals plus existing
GCC/Homebrew through SAME route, warm Host freshness, selected event replay,
independent final diff review and appropriate owner/direct-dependent checks.
Root alone schedules Cargo/native proof, atomic integration, fresh CLI/cquery
and equivalent whole-command memory checks; internal probes close none of those.
