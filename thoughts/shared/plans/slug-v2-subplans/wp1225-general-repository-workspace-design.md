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
The inactive linear native primitive binds the complete reservation, operation
index, predecessor and original backing. Pending successors expose no usable
lease before a borrowing quiescent-completion receipt. The owned runner alone
joins an opaque exact-request/root-bound reaped capture to actual native progress
and constructs this receipt. Native descriptor/retained credits follow every consuming
transition and retire after their actual descriptors and immutable payloads.

No detached-anchor cloning or mount pool is used. The fixed Rust three-launch
gate exercises owning-namespace attachment, pre-write successor transfer, setup
FD retirement, actual reap, original-root snapshot and accepted record before
the next lease becomes usable. The fixed-write thread clears effective/permitted/
inheritable/ambient capabilities and sets no-new-privileges; trusted libtest
infrastructure may retain other privileged threads. General-runner privilege
retirement and untrusted confinement remain production gates. The fixture does
not prove detached creation from fstat facts alone: trusted setup provenance
remains explicit. The command/quota and owned runner below integrate initial
admission, same-backing shrink, pre-payload handoff, general completion/next-token
return and trusted non-execute advancement. The actual CLI proof covers full
helper-process capability retirement and representative confinement/lifecycle;
the older fixed libtest fixture alone still makes none of those claims.
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

The inactive physical-root owner consumes a producer-credited FD from trusted
quiescent setup. It independently checks a directory at the actual tmpfs root
(inode 1 and supported statx mount-root/mount-ID facts), 4096-byte pages and
the exact selected quota pair. A mount-root bit alone does not admit a bind
of a tmpfs subdirectory. It never opens a display path or retains mount tokens.
An explicit native resource bank pays syscall scratch, complete retained guards,
FD slots and declared backing-capacity allowances. One closed shared backing
credit binds the full reservation and original physical identity; builder,
sealed root and all bound mount tokens retain it until their last pin closes.
Arc clones reserve no second quota. Trusted bootstrap prepays backing before
creation and keeps its receipt guard through attestation/binding. No FD, IO or
mount capability enters this shared accounting owner. Rooted observation also
keeps its existing source-scope capture credits; no source-bank cap is increased.
Retained-byte reservations reuse the existing descriptor-budget utility. Backing
allowance is a virtual quota reservation, not a kernel/slab or RSS bound.

Consuming seal compares the full reservation and final mode, verifies every
expected node and every actual directory child, compares file chunks directly
with existing immutable bodies, and checks literal links, modes, EOF and identity.
Mount, root identity and quota facts must still match after verification. The
sealed owner keeps only the final state, not the transcript/history. Its shared
read-only surface offers observation and snapshot-member readers; it exposes
neither File/raw FDs, RootIo, mutation nor mount authority. Each reader holds its
exact root lease and positive credits, compares bytes to the sealed snapshot,
and checks current rooted/FD identity. Descriptors and owned payloads close before
their credits can be reused.

The inactive consuming native progress owner begins only with an actual empty
Archive root. It applies File/Template/Read/Exists/Delete through the rooted
kernels and samples the complete actual tree before accepting each new record.
Exact completed prefix requests replay without rewinding the current tree. It
retains invocation-wide deletion and decoded-archive counters; all errors,
cancellation and partial mutations consume the entire mutable generation.
Archive uses the closed verified native capture handoff below. Execute consumes
the whole command through the owned runner below. There is no generic mutation
or immutable process-result injection callback.

Actual capture records directories, modes, bodies, deletions and admitted literal
internal links under the selected Archive/Process envelope. A prior same-path
body is reused only after actual byte/EOF/identity comparison. Changed bodies
receive exact prepaid Arc allocations; bounded rows and transfer arrays stay
credited. Bzlmod forecasts successor transcript storage from the same accounting
used by completion. Native progress keeps old transcript and captured storage
credited while admitting/constructing the successor; credits accompany both
success and failure ownership paths. Only immutable transcripts project to DICE.

Archive acquisition reuses the existing checksum-verifying Host/HTTP/distdir
producer and transfers its private temporary file without a compressed Arc or
second capture copy. The native handoff verifies bounded actual SHA/EOF and
single-link regular identity on that same FD before rooted extraction. Existing
source acquisition policy remains unchanged; it is not newly confined. Workspace
mutation uses only the held root FD and the existing shared decoder with
512MiB/call and 1GiB/invocation decoded limits. The original SourceServiceScope
identity is pinned before every operation, replay and seal; a fresh scope cannot
reset transport/decode budgets. Exact replay performs no acquisition.

Capture reserves separate FD/control/scratch and128MiB compressed-storage
allowances. Decode retains its existing source buffer/codec credits plus native
temporary-FD/path-set admission. Explicit close precedes handoff. If capture
unlink fails, actual FD/control charges retire but the storage-only allowance
stays conservatively charged, matching existing fail-closed source ownership;
there is no orphan retry registry or cleanup-success claim for that path. Trusted
capture creation excludes outstanding writers until retirement. Complete actual
snapshot and prepaid transcript completion still follow every successful decode.

Trusted deterministic setup excludes other writers. Snapshot equality alone
never proves descendant death or absence of write aliases; Execute requires the
owned runner's exact quiescence capability before capture. Registry association,
cross-repository read views and production activation remain packet gates.

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

The inactive [command/quota component](wp1225-quota-command-acceptance-2026-11-07.json)
implements that first admission with one non-Clone native command owner. It
creates its own empty epoch and owns all unfinished roots and never-used initial
mount seeds. Immutable epoch publication accepts exactly the existing complete
transcript prefix plus one record; equal final trees cannot substitute rewritten
history. Before first Execute, the consuming owner compares the full selected
transcript, asks the current epoch for the exact request (including cumulative
body retention across Process roots), and verifies the actual complete tree
under Process RootIo limits without copying existing bodies. Old projected epochs
and individual native roots cannot mint this admission. Epoch/root/control and
transfer allocations are positively credited; no DICE transaction or lock spans
native work. Archive-only completion keeps its larger envelope.

Trusted setup attaches the consumed seed, checks actual allocated bytes and
charged inodes, shrinks the same superblock and mints a bound successor. The
parent independently observes the Process quotas through its original root FD
before ACK; a closed proof permits one atomic shared backing allowance/mode
reduction with no fallible operation after that update. No expansion or rollback
exists. Failure after commit and before ACK reaps the helper before poisoning
and retiring transition authority, retaining the original root in the consumed
command until disposal. The ready value is closed pending first-Execute
ownership, not a generic process result or quiescence constructor. Only the
owned runner below can consume it with an actual bound reaped capture, complete
the snapshot/record and return the command with its accepted next token.

Fixed Rust native checks cover the real2GiB/262144→64MiB/8192 remount,
unchanged root identity, shared-credit release, failure after commit, page
exact/+1 refusal and measured inode charge8192→8193 before shrink. Process
creates reach ENOSPC at actual used8192. Counts of visible filenames are not
inode-capacity units: [tmpfs](https://docs.kernel.org/filesystems/tmpfs.html)
charges this space for extended attributes as well. Logical entry and physical
inode bounds remain separate. A deleted64MiB+1 history is refused before Execute
while its Archive-only tree can still seal. These are bounded mechanism tests,
not full Archive-envelope exhaustion or equivalent-command memory evidence.

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
actual socket/device/FD/nested-mount evidence must stay paired with the installed
profile. The owned-runner receipt supplies its current discriminators; later
registry/read-view changes require their corresponding boundary tests.

The inactive `repository_process_policy` leaf now emits one static
LinuxReadOnlyHostV1 cBPF artifact for64-bit Linux x86_64, with strict architecture,
x32 and argument-width checks, default EPERM, restricted process/signal/control
operations and clone3 ENOSYS fallback. Thirteen independent literal-UAPI
interpreter/verifier tests cover emitted decisions and control flow. The closed
helper below installs it for every payload, with no caller-selectable profile.
Uniform setsid is admitted; PID1/group signals remain denied and namespace
cleanup is independent of sessions.

### Owned general runner contract

The inactive [owned runner](wp1225-owned-runner-acceptance-2026-11-07.json) uses
the actual Rust CLI image's exact internal helper
entry before ordinary CLI/Tokio/DICE initialization. A private native platform
owner chooses the current trusted Slug image and trusted bwrap installation;
payload argv cannot select either. Alternate compiled CLI/launcher paths are
test-only and require artifact provenance. Pinned Host/image descriptors may
select those setup mounts/images through proc FD paths; this grants no proc
reopen/alias route for workspace roots or source data. Trusted installation
excludes external image writers; descriptor pinning prevents path replacement,
not in-place modification of an otherwise writable image.

One prepaid owning Rust worker thread starts before any native child. It owns
the complete command, root/backing/lease, plan, captures and control credits until
actual cleanup. Async cancellation only sets its cancellation flag; dropping a
caller cannot drop native authority. Thread creation failure precedes launch.
The helper is actual PID1 (`bwrap --as-pid-1`), checks single-thread entry, and
sends its own pidfd over an anonymous Unix seqpacket channel. Parent stores that
exact kernel handle and ACKs HELLO before any root creation/transfer is allowed.
No numeric PID reopening is used. Before this ACK, no selected repository root
authority may reach the helper;
trusted Host/image/plan/control/status descriptors and setup-userns authority may
already be inherited. Their credits survive until exact helper death or trusted
terminal monitor status plus owned monitor reap. Monitor termination alone never
proves namespace death.

New general roots occupy component-bounded descendants of the declared virtual
`/__slug_repositories__` prefix, reserved before Starlark sees path text. The
namespace explicitly masks that Host prefix. A fresh bounded tmpfs root mirrors
at most128 pinned Host top-level entries/64KiB names and literal targets, with
actual identity checks before READY. Every Host binding is recursively
RO/nosuid/nodev/private. Fresh private proc is identified by actual mount identity;
bounded escaped mountinfo parsing finds and masks all other Host procfs directory
views, including `/run/host/proc`. Uncovered proc-file binds refuse setup. The
private proc stays RO/nosuid/nodev/noexec. Minimal devices are exactly null, zero,
random and urandom, checked by type/device number; nodev is cleared only there.
Root and reserved scaffold each have1MiB/512 charged-unit quotas, and temporary
storage has16MiB/4096. Device tmpfs has4KiB capacity and exactly four readonly
device mounts; no separate16-inode quota is claimed. A prepaid28MiB+4KiB virtual
allowance covers plan/root/scaffold/tmp/dev plus up to2048 tiny4KiB proc masks. Final scaffold is RO while the selected same-backing root
and temporary mount remain the declared writable storage. No Host mkdir occurs.

A sealed immutable2MiB plan on FD4 contains actual argv, complete effective env,
cwd, setup role, nonce/index and pinned mirror facts. Parent forecasts the plan
from borrowed semantic data before CString allocation, prepays8MiB encoded/
decoded/table overlap plus separate2MiB memfd storage and FD peaks. Control uses
FD3; payload semantic output uses independent prepaid1MiB streams. Typed fixed
frames have exact ancillary counts and correlation, never a semantic digest
substitute. Initialized Archive setup has no fabricated Execute request/result.
First Execute uses the current quota admission; later Execute consumes the
accepted rolling token. After successor+quota verification/ACK, rooted cwd
creation precedes payload and enters the actual next snapshot.

Trusted setup temporarily retains SYS_ADMIN and SETPCAP, then retires every
bounding/effective/permitted/inheritable/ambient capability and sets no-new-privs
before fork. Hard and soft limits become min(inherited soft and hard,512MiB AS,
64 processes,64 FDs,64MiB file size,8MiB stack,remaining<=27s CPU), with core size0. Shared UID
pressure/E2BIG/EAGAIN are infrastructure refusals; caps never widen automatically.
These per-process/virtual limits and native allowances do not prove aggregate
RSS or equivalent-command fit. The actual native profile gate proves fork
exhaustion below64 children after capability retirement, without a root exemption.

Payload alone installs LinuxReadOnlyHostV1. A trusted CLOEXEC exec-error pipe
closes atomically before successful payload code, leaving only0/1/2; it separates
failed exec from a real exit127. PID1 remains nondumpable, closes capture writers,
and installs a distinct minimal cleanup policy. It retains main raw status,
kills every remaining namespace member, and waits with __WALL to ECHILD before
DONE. Parent additionally requires PID1 death, owned monitor reap and stream EOF.
Fatal/cancel paths signal the pinned PID1 and await actual namespace teardown;
no cleanup deadline permits refund of a possibly live root. Caller cancellation
or the original scope/30s deadline returns an error while the already-owned worker
retains every field through cleanup or exceptional quarantine. No new cleanup
thread or runtime is created. The opaque reaped
capture binds the full executed request and original physical root. Completion
then verifies/captures the actual tree, accepts the immutable record and next
mount token, and republishes the original command epoch. Deterministic mutations
advance a Process token only through their actual accepted native progress.
Registry/cross-repository read views and original activation remain later gates.

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
Remaining gates: full Archive envelope/inode workload, selected physical-root
registry and cross-repository read views, A/B same-name distinct-instance source/digest/image/
REAPI reads, generalSource certificate restore, unrelated originals plus existing
GCC/Homebrew through SAME route, warm Host freshness, selected event replay,
independent final diff review and appropriate owner/direct-dependent checks.
Root alone schedules Cargo/native proof, atomic integration, fresh CLI/cquery
and equivalent whole-command memory checks; internal probes close none of those.
