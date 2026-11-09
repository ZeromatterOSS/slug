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

### Selected repository process views

Cross-repository visibility uses the command's common owning user namespace,
with fresh mount/PID/net/IPC/UTS namespaces on each launch. The exclusive native
command owns an optional credited `NativeMountDomain`: its first successfully
reaped bootstrap seeds a checked CLOEXEC duplicate of the actual owning userns
descriptor; later roots must match its workspace lease, original source scope,
resource bank and namespace. No mutex spans launch. The descriptor-free source
claim domain, backing, sealed data root and source readers never retain this
setup authority. Native process-view controllers may retain sealed data roots;
there is no reverse reference from data to controllers.

`FinalizeArchive`/`FinalizeProcess` are trusted no-payload setup modes. They
consume the current initial/rolling RW token, attach it in its actual owner
context and mint a clone from that actual attachment. `mount_setattr` on the
detached clone sets and attests per-mount RO/NOSUID/NODEV, preserving executable
file modes and existing backing quotas. Finalization creates no Execute record,
advances no operation index and never creates cwd content. A pending RO clone
becomes a usable finalized view only after actual namespace/helper/monitor reap
and the existing full-history, complete-tree, same-root materialization seal.

A later setup consumes each selected RO token, attaches it at its stable display
and mints/attests its RO successor while attached. Transfer is sequential: one
incoming token FD, then two returned attached/token FDs per row; nonce, operation
index and slot authenticate the private exchange. Root and every view successor
must validate before the final payload ACK. Successors stay pending until actual
reap; cancellation retains the entire command, batch and credits until cleanup
establishes death. At most128 views fit the unchanged2MiB plan; the existing
512-inode scaffold and transient descriptor allowances are forecast before any
setup effects. No sealed-FD clone, Host alias, proc reopen, copying or keeper is
an alternative source of mount authority.

The inactive semantic path now requires a pure selected-view manifest on each
Execute: full Spec plus final State (including reservation/instance/display/
lease/policy/mode/tree), without recursively retaining source transcripts.
Canonicalization rejects duplicate/overlapping/self/foreign-command views and
keeps the128-view bound. Full request equality and exact prefix replay include
the manifest. Loading must supply the declaration set at each original prefix;
later discoveries must not rewrite an earlier completed Execute. Its original
integration remains pending.

Core snapshots the COMPLETE supplied selected request/result set and closed
native rows under the existing actor mutex, compares full foreign semantics
outside it, then authenticates every actual source root/controller/domain.
Every member is admitted before setup effects; argv never predicts visibility.
The exclusive command checks out all controllers locally. One covered request
and nonce/root binding is shared by the batch and ordinary runner. All RO
successors validate before payload ACK and actual reap precedes normal complete
snapshot/record acceptance and controller restoration. The actor rechecks the
same-token full associations before exposing the updated command; a refusal
discards private completion after actual death. No actor lock, evaluator or DICE
transaction spans child work.

Pure storage forecasts partition owned operation/Spec/reference metadata from
complete referenced source State allocations; standalone totals remain complete.
Native owned-only accounting requires canonical data-root coverage. Pointer-only
coverage can establish allocation lifetime before input credit, never semantic
authority; uncovered foreign input pays the total before exact comparison and
canonical rehoming. Mutable body/history visitors exclude source State bodies.
Data-only dependency guards follow covered requests, captured bindings, credited
transcripts (including later trusted mutation), terminal receipts and actor rows.
The command epoch drops before its dependency union. Terminal guards keep only
referenced source data, while sealed output data roots retain no history,
controllers or namespace authority. Current-manifest guards and accumulated
history guards remain separate, so changed declarations do not lose old inputs.

Foreign native association, image, selection, acceptance and seal comparisons
memoize exact State/body pairs in prepaid phase-local tables. Strong operands
prevent address reuse; misses compare source bodies in64KiB chunks with original
scope/activity checks, including on memo hits.131072 pairs per table is an
explicit operational bound. Other existing non-Execute/result byte fields keep
ordinary full equality; this is not a universal preemptible equality claim.
Pure/DICE derived equality is unchanged. Host/legacy acquisition/equality policy
and legacy-only selection budgets remain unchanged. Local/legacy process views
without actual post-profile visibility proof are explicitly unsupported.

[Physical view proof](wp1225-native-process-views-acceptance-2026-11-08.json) and
[selected admission proof](wp1225-selected-view-admission-acceptance-2026-11-08.json)
cover the real early CLI helper,65MiB Archive source, normal semantic records,
nonzero writes, exact replay, canonical foreign projections, unused declared
sources, prelaunch refusals, post-reap association changes and last-owner cleanup.
These close no original Loading, full archive envelope, public Rust, physical
CAS/runfiles under that original route or equivalent-command memory gate.

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
owned runner's exact quiescence capability before capture. Registry association and cross-repository read views now have inactive
implementation/proof below; original integration and production activation
remain packet gates.

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

### Caller-profile retirement boundary

The [retirement receipt](wp1225-profile-retirement-acceptance-2026-11-08.json)
records the deletion, refusal/replay regressions and original environment proof.

Retire the GCC and Homebrew adapters together, independently of enabling the
General policy. Live AGENTS bans existing caller-specific recipes and gives the
prototype no migration obligation. Removing prohibited fallbacks does not close
any General activation gate. Default `Disabled` repository Execute becomes
explicitly **unsupported**; prior adapter-dependent successes remain historical
evidence and do not establish general compatibility or Rust/bootstrap closure.

Loading refuses Disabled Execute before argument processing, legacy Need
creation or completed-record replay. It removes Homebrew owner/name/source
classification and template/environment vetoes. General remains the sole
execution owner, reached only through trusted internal configuration; its normal
environment observation and template APIs apply uniformly to every original.
Core defensively rejects any legacy Execute descriptor as the first complete-
frontier progress check under either policy, before observation, archive or
filesystem staging, workspace reservation or child setup. Do not translate
legacy records into Workspace histories. Unused pure legacy descriptor/epoch
carriers grant no execution authority; removing their schema is not a
compatibility requirement for this deletion boundary.

Discriminate mixed legacy Execute plus path/filesystem/workspace Needs with
untouched observations, transcript/state, resource admission and on-disk data,
including an injected completed legacy record and normal abort/restoration.
Protect Enabled legacy-realization refusal and the accepted general runner,
source-link, selected-view and real CAS/runfiles gates. Extend the unchanged
verified `unix_cc_configure.bzl` `get_env`/template proof with absent Homebrew
and present A/B/A snapshots, undeclared environment changes, quote/percent
escaping and `_`/dotted-key filtering; compare actual wrapper bytes/content
identity. These four original invocations use fresh runtimes on the same
workspace and one persistent bank, with exact complete refunds between cases.
They prove actual outputs, not warm Native-runtime capacity. A separate pure
General File/Read/Template A/B/A test keeps one DICE owner/reservation/lease and
declared cells/frontier unchanged; changing only the full undeclared snapshot
must invalidate completed A, refuse stale replay and restore matching B/A
bodies/identity without a fresh-epoch confound.

The attempted warm authentic sequence refused Resource at bootstrap after
7.7508s within the unchanged 8GiB bank. Its aggregate peak was 6,883,905,536
backing bytes; post-abort live backing was 2,415,919,104 bytes (one 2GiB Source
plus four 64MiB General roots). Retained prior data plus new bootstrap
reservations is the supported explanation, but the aggregate counters do not
identify the exact rejected root. That receipt preserves the refusal; the
Source contract below now closes this narrow repeated-compiler capacity gate.
Profile retirement itself introduced no quota increase, Source contraction or
eviction. Neither the refusal nor the later backing proof establishes
equivalent-command RSS fit. Source authority, complete
environment equality, caps, original clocks and lifecycle remain unchanged. This is retirement plus an inactive
exact API slice, not production execution or equivalent-memory acceptance.
Independent design and final review apply. The root owns the small DICE progress
entry edit; the existing large owner files lose code without adding another
central abstraction. No donor utility, retained representation or new cache is
introduced.

### Original Loading candidate join

The [scoped acceptance receipt](wp1225-original-loading-acceptance-2026-11-08.json)
records the original-to-native fixture and affected owner/CLI regressions. It
does not activate the policy or close public Rust, physical CAS/runfiles, archive
envelope or matching-command memory gates.

The inactive original route observes a workspace-scoped execution policy before
route/prepared reuse under both variants. Its ordinary DICE key computes
Disabled unless Core explicitly injects the trusted policy; attempt, acceptance
and restoration inject the current policy. LinuxReadOnlyHostV1 applies uniformly
to original repository-rule invocations and never falls back to a GCC/Homebrew
runner on refusal. Trusted configuration requires one workspace-retained native
bank. The crate-internal configuration
boundary now also supports ordinary Native construction through the command
scope below; fixed Diagnostic construction remains available. Default activation
remains gated below.

Enabled evaluation demands the fresh workspace observation before loading the
original definition or returning a prepared effect, including zero-Execute
rules. The reservation binds its expected full RepoSpec before exposing absolute
Path text. Stable spelling and conflict-slot identity exclude Spec; full equality
and bounded storage include it. A stale same-slot Spec refuses rather than
creating another reservation. Typed reservation Needs initialize the actual
claimed root outside DICE; operation Needs carry the exact request and an
independent canonical set of current selected request/result associations.
Generic repository path, file-read and directory-listing producers preserve the
materialization anchor. Anchored listing keys include that complete anchor in
identity and reject escapes explicitly; resolution never asks the native reader
to observe ancestors above the selected root. Host and legacy unanchored routes
keep their existing resolution behavior.

One invocation cursor starts at the transcript's initial State and replays each
exact record into that record's next State. File/read/exists/template/delete/
archive/Execute share the cursor; terminal projection rejects unused records and
carries the complete realization into GeneralWorkspace without authored-plan
postapplication. Own Labels resolve through the bound root before terminal
materialization, avoiding a self-cycle. Foreign typed paths retain their real
namespace and producer association. Declared sources start empty on every
retry and enter only when the original actually accesses the typed value;
later prepared values cannot expand an earlier Execute manifest. Unbridged
Local/legacy sources and cross-root symlink operations refuse explicitly. Foreign
Materialization Label get_child/dirname also remain unimplemented; above-anchor
own dirname refuses rather than returning a false missing-parent value.
They remain required integration work before default General activation.

quiet=False contributes a binary completion-output event at its replay position,
sharing captured result Arcs. stdout precedes stderr with no inserted newline or
text conversion. Its command lease and operation index preserve same-command
retry deduplication while equal bytes from a fresh actual command still deliver.
Need attempts discard their provisional batch. A consistent completed
invocation, including a later original Starlark error, retains interleaved
prints/output; source/environment consistency and publication failures suppress
unverified capture. Complete alone is not an output guarantee. quiet=True and file-only
print suppression retain existing selected-event behavior. Events, publication,
daemon responses and CLI stderr use one byte channel; text diagnostics become
UTF8 bytes at their producer boundary. The JSON stderr wire field is a byte array,
with no compatibility shim. The daemon now has a Slug-native 64 MiB serialized
frame limit, including newline, on a previously unbounded protocol. It may also
refuse large ordinary query output with General disabled. Counting precedes
response-sized allocation; exact/+1, byte expansion and bounded error framing
are separate from completed-command memory acceptance.

Native command retirement is independent of immutable data leases. The last
command field, bootstrap mutable builder and seed token, and standalone/renewed
finalized views retain credited retirement guards; a worker guard spans actual
work, oneshot delivery and failed-result disposal.
The driver drops its mutable command before awaiting the original cleanup
deadline, then restores observations/discards/closes the demand lease only after
all mutable worker/result custody retires. Deadline or exceptional quarantine
keeps the session Open/Busy with no restoration, early refund, cleanup-success
claim or late publication. Credits may release after actual later retirement.
No cleanup thread is spawned from Drop, and resource-bank counts are not used
as a substitute for actual worker retirement.

### Command-owned native source scope

The [command scope receipt](wp1225-command-source-scope-acceptance-2026-11-08.json)
records the inactive Native join. It keeps the Host/Registry acquisition owner and its
permanent unscoped services unchanged. An explicitly configured General command
mints one finite scope after Busy admission and binds its exact Arc to the actor
token before preflight. Diagnostic construction retains its original fixed scope
and never refreshes it. Default production policy stays Disabled; no CLI, caller,
source name or environment switch enables this candidate.

The command, General progress, workers and native completion capabilities retain
that same scope across retries. Existing limits remain: at most 27 seconds of
work and three seconds of cleanup, with no renewed clock. Command control and
every live scoped buffer/decoder additionally debit the existing persistent root
bank. Per-scope counters remain distinct and sticky; retained A allocations still
constrain B admission until their actual guards drop. Conservative double charges
are accepted. No resource owner enters DICE or a mutable runtime current-scope
slot. Host/Registry acquisition keeps its existing counters and byte policy;
command future supervision is a separate lifetime bound.

Builders and sealed physical data bind their original scope. Readers, images and
transport retain and check it, so B cannot remint A data under B's later deadline.
Pure metadata or a retained data Arc grants no mount authority or scope renewal.
Success drains computation/native/result custody and consumes the actor token
without cancelling valid data leases; those retain A's original clock. Abort
sticky-cancels the owned scope, drops the attempt and drains with its original
cleanup deadline before restoring the canonical prior snapshot and discarding
the actor session. Failed cleanup leaves Open/Busy. No kernel termination or
whole-command memory guarantee follows from timer supervision.

Proof must join two actual originals on a normal Native runtime, fresh A/B scopes
on the same bank, original-reader lifetime and B-remint refusal, sticky abort
isolation, shared retained-buffer/decoder admission, retries and queued worker/
view retirement. Existing Native acquisition and consumed Diagnostic behavior
remain protected within the recorded passing coverage. Five inherited Diagnostic
full-caller/counter fixtures fail identically on the clean baseline and candidate;
their success is not claimed. Derived Source Path and unsupported archive forms,
full archive, original-route CAS/runfiles, public Rust, equivalent-command memory
and default activation remain open.

### Selected-source snapshot bridge (accepted inactive component)

The [acceptance receipt](wp1225-selected-source-bridge-acceptance-2026-11-08.json)
records the independent review and actual original/native proof. The bridge uses
a distinct Source reservation and closed
digest manifest, never an invented General owner/history. Natural subjects are
Workspace (workspace, mapping and root identity), Local and Immutable (complete
selected provenance, Spec, mapping and admitted source policy), and Builtin
(pinned catalog identity and mapping). Route owners project these facts through
tracked DICE dependencies. Every admitted repository-context source projection
and prepared reuse demands the snapshot before exposing path text. The bridge
admits direct Label/Path text, read/template and Execute. Derived Source Path
attributes/methods, listings and reserved-path string read roundtrips explicitly
refuse; they acquire no Host fallback. Ordinary module and .bzl acquisition retain their existing producers
and final consistency certificates. Exposed paths are never rebased afterward.

The manifest retains complete canonical paths, modes, kinds, literal links and
FileContentDigest(size, SHA256), without file-body Arcs. Native sealing verifies
the complete actual tree after all transforms, including membership and framed
same-FD content. General exact body/history semantics remain unchanged. Bounded
exact manifest comparisons run outside actor locks and check scope/activity.
Partial Read/Seek is not an independent SHA proof; full-stream digest/EOF checks
are required for future Source CAS/image publication, whose original-route
integration remains open. Data retains original scope, credits and
backing, without namespace/controller authority; command retirement includes
pending acquisition, worker results, source views and queued installation.

The current native Source epoch classifies its exact image-observation namespaces
as command paths. Their complete facts remain in that command's terminal
association, source certificate and DICE publication, and validate under the
originating scope before publication. They are excluded from future-command IO
and repository reuse validation; B must capture again through its fresh epoch.
Original Host/module/.bzl and ordinary materialization certificates retain their
existing validation. No path spelling selects this lifetime class. Partition and
retained native snapshot metadata are prepaid on the same bank, without granting
IO authority or retaining a controller.

Each fresh command creates fresh backing. Immutable archives reacquire through
the existing selected BCR tar.gz producer and extract/overlay/patch/MODULE directly
into that backing, preserving its parser semantics. Legacy LocalTar source
bridging remains unsupported; its ordinary acquisition producer is unchanged.
Completed legacy TempDirs and
A-scoped captures cannot be promoted into B. Builtins stream the complete
validated static catalog verbatim. Local and Workspace use a nofollow whole-tree
capture with framed per-file hashing, then complete membership/content rewalk
and ordinary final Host/source-certificate validation. This Slug-native policy
certifies the closed captured image, not an atomic Host instant or writer
exclusion. Observed instability fails. Fresh B captures Host inputs anew.
Internal relative symlinks retain their literal text; absolute, escaping or
cross-source links, hardlinks and special files refuse. No live Host fallback or
source-name filtering (including .git/target) is permitted.

Native source setup is Linux x86_64/64-bit; other platforms have no source setup
factory or activation path. Pure metadata carries no native permission.
Source-specific per-root admission is 256 MiB final logical bytes, 64 MiB/file,
8192 entries including root/directories, 1 MiB aggregate path/link bytes,
256-byte paths, 255-byte components, depth32 and 8 MiB digest metadata. Native setup reuses the
existing Archive physical envelope of 2 GiB/262144 charged units and prepays its
complete capacity on the existing persistent bank; it does not borrow General
history allowances or shrink/refund the physical quota. The complete selected
set remains bounded by128 views and shared retained-byte/FD/backing admission
before helper launch. Existing 128 MiB compressed capture, BCR256 MiB decode,
source counters and original deadlines remain unchanged. Streaming scratch is
fixed64 KiB and credited; validation hashing is not a second decode charge.
Caps may explicitly refuse genuine Rust archives or large workspaces; this
bridge cannot close full-workload, public activation or matched-memory gates.

Implementation splits pure protocol/provenance in Bzlmod, SourceRoot/sealing and
rooted archive/Local kernels in Core private leaves, and route/Need/replay plus
catalog projection in existing source owners. Reuse Arc/Allocative, compact
strings/maps and RootIo hashing, with no global cache/interner or donor import.
Proof must cover actual original arbitrary argv using all source families,
fresh A/B ownership, Local mutation/link refusals, complete-set rejection,
cleanup/credits and a greater-than64 MiB source beside the64 MiB writable root.
Existing pinned Bazel9.2 StarlarkBaseExternalContext.getPath/getPathFromLabel,
read and execute Label projections supply exact API source authority; physical
identity, capture consistency and finite admission remain Slug-native.

### Source-backed symlinks (inactive implementation contract)

The inactive slice now covers the two source configuration links in
the unchanged `rules_cc/0.2.17` `unix_cc_configure.bzl:350-372`, before its first
compiler Execute. Pinned Bazel 9.2 `StarlarkRepositoryContext.java:197-240`
converts target and destination through `getPath`, checks the output directory,
creates parents and creates the actual symbolic link to the resulting absolute
target. The admitted exact slice preserves String/Label/Path conversion order,
literal link text, directory/dangling links and subsequent reads/existence.
Cross-root authority, quotas and refusal classes remain Slug-native. No profile
removal or default activation follows from this prerequisite alone.

Admit own-root links and canonical absolute targets under current, already
declared Source snapshots (Workspace, Local, selected BCR and Builtin). Host,
General-to-General and escaping relative external targets remain unsupported.
Source snapshots keep their existing internal-only link policy, so the external
graph is one-way and nonrecursive. No copying to a regular file substitutes for
a link, and no reserved prefix or available-but-undeclared Source grants access.

`RepositoryWorkspaceOperation::Symlink` owns the destination, actual absolute
target and optional typed Source read view. The pure request validates own-root
or exact Source bounds; native admission independently authenticates the full
Source projection against the operation's declared prefix and current Source
epoch/rows, lease, original scope, bank and claim. Full comparison remains
outside actor locks with final same-lock association rechecks. Every operation's
prefix is independent: later Need discovery cannot enlarge an earlier Execute.

General State retains literal Link nodes plus one sorted, unique, Source-only
binding table for roots referenced by actual external links. The complete-tree
sampler uses the same checked classifier as trusted Symlink creation, including
for payload-created links. It retains bindings for directory and missing
targets without traversing them. State binding construction requires a complete
independently admitted prefix; the full Source epoch authenticates that prefix
but never enlarges it. Unsupported/foreign/stale/overlapping targets refuse
before publication. Existing archive link admission is unchanged.

Operation and every predecessor/next/terminal State participate in full
structural equality, exact comparison visitors and source-state coverage.
Literal bytes, reference arrays and operation metadata remain positively owned;
Source allocations occupy the referenced partition only while exact canonical
data guards cover them. Copied Read/template bytes remain owned body history.
Historical guards retain the complete Source union after deletion. Seal filters
only the final DATA root's dependencies according to final State bindings;
terminal request/history guards retain their historical union until they drop.
Data dependencies contain neither controllers nor owning namespace authority.

The existing pure `MaterializationPathAnchor` gains an optional shared array of
at most128 flat terminal Source anchors, with checked canonical order, distinct
namespaces, disjoint roots and no nested contexts. Empty contexts retain current
behavior. Bzlmod's General materialization projection and actual native seal
derive identical contexts from used State bindings. Prepay the leaf array,
conversion peak and retained native context before allocation/cloning. No Spec,
body, scope, descriptor or controller enters this Workspace metadata type.

The common resolver retains the logical General namespace, requested path and
anchor, every exact link hop and mixed-namespace observation, and a distinct
terminal content origin. Only an actual canonical absolute ReadLink may cross
from General to one listed Source anchor; Source traversal remains internal.
Rooted traversal and Host resolution stay unchanged. Digest/read/listing use
the terminal origin while preserving the full General route certificate.
Existing cycle/expansion behavior and finite path/frame/work admission apply.
The synchronous native adapter prepays allocation peaks at machine phases,
including parent-frame construction, route growth, completion slices and joined
parent-route projection. Original scope/activity checks run at those phases and
within projection loops. A retained high-water credit covers modeled results;
direct small Own reads do not reserve a blanket Archive-sized working buffer.
The existing131072 demanded-fact and4MiB demanded-path limits remain demand
limits; internal allocation checkpoints do not consume those counters.

Native artifacts preserve General label/logical ownership and carry terminal
content origin separately. A General lease reconstructs and authenticates its
exact sealed link route and Source binding before accepting terminal origin,
path, regular-file kind or digest. A linked reader retains the General owner,
credited route frames and original scope beside its Source reader, checking
them before/after reads and seeks. All content opens use retained Source FDs.
Mutable-prefix Read/Exists follow the same bounded route; missing targets make
Exists false, reads enforce the existing2MiB limit and full size/SHA/EOF checks.
Their transient mutable facts do not enter an immutable final certificate.
Writes through a Source directory link refuse; Delete unlinks the General link.
General InputImage with external bindings refuses before IO. A selected General
Execute read view with Source bindings additionally requires every bound Source
to appear independently in that operation's exact declared Source manifest.
Available epochs or retained DATA dependencies never synthesize controllers or
enlarge that manifest; missing independent declarations refuse before replay,
quota IO or controller transfer.

Proof must cover the unchanged original's two links followed by normal linked
configuration Loading and general execution, then actual CAS/runfiles use.
Reuse existing original/reader/native fixtures for unrelated authored and
payload-created file/directory/dangling links, internal cycles, undeclared and
available-but-not-prefix targets, prefix replay after later discovery, immutable
route/digest forgery, link deletion with historical ownership, same-display
A/B, cancellation and exact last-drop refunds. Default/profile deletion,
General-target links, derived Source Path APIs, full Archive/public Rust and
matched-command memory remain separate gates. No new donor, cache, interner,
cap increase or whole-workload memory claim is selected.
The [source-link receipt](wp1225-source-links-acceptance-2026-11-08.json) records
the genuine original, declared fixture dependency graph, protected transport and
native ownership/resource discriminators. It closes this inactive prerequisite,
not production profile removal or default activation.

### Read-only General Archive quota contraction (inactive prerequisite)

The unchanged `cc_autoconf` configuration loads genuine `bazel_features`
version/globals repositories and the `rules_cc` compatibility proxy. The
[bounded role diagnostic](wp1225-source-links-acceptance-2026-11-08.json)
identifies a Process proof root, one immutable Source image and two tiny
non-Execute General roots retaining full Archive quotas before the next General
bootstrap. The fixed8GiB bank correctly refuses that next2GiB reservation.
No named-root eviction, synthetic globals, fake Execute, upstream rewrite or
cap increase may bypass this pressure.

Add optional physical quota contraction at the consuming read-only finalization
of every eligible General Archive root. This is a Slug-native lifecycle/resource
operation. Pure State.mode remains Archive, its exact history and logical
Archive envelope remain unchanged, and no Execute/index/result is invented.
That accepted General-only component preserved Source quotas; the Source
contract below extends physical finalization separately. Process finalization
is unchanged.
The eligibility decision uses actual quiescent tmpfs allocated block bytes and
used inode units: if they fit64MiB and8192 inodes, the trusted owner may lower
the SAME superblock to that sealed quota class; otherwise it keeps the full2GiB
Archive class. Apply by actual owner/lifecycle facts, never caller or repository
names. No performance or completed-command memory claim follows.

The private native frozen-quota class is distinct from semantic mutable mode.
Before contraction, authenticate the consuming full finalization request,
original scope/bank, exact root/devino/mount identity and sole current setup
custody. There are no live payload writers. Trusted setup remounts in the
existing owning user namespace, verifies exact new quotas plus unchanged
identity, allocated bytes and used inode units, then makes the successor view
read-only. The parent independently checks its original FD and received
successor/attestation against the same admitted owner and nonce/correlation.
Keep the full old debit through actual helper reap, parent original-FD quota and
identity re-attestation, and acceptance of the exact matching read-only successor.
Only that closed verified proof may atomically shrink the one shared backing
debit before sealed publication; root/token/data clones keep that same debit
until last drop. Physical quota
attestation, source/reader/image checks and read-only controller/domain checks
carry the actual frozen class; none may infer it from State.mode or a path.

Contraction is admitted only by consuming read-only finalization after real
quiescence. There is no path from its frozen class back to a mutable builder,
initial seed or rolling execution token. Every retained root, shared backing
guard and read-only successor carries and checks the same closed quota proof;
measured usage on a later read cannot mint the class. Existing Process64MiB
roots retain their distinct existing class. The quota class freezes before any
immutable namespace/data publication; mutable-prefix facts remain transient and
cannot become an immutable certificate. The existing fresh concrete instance
and exact native association bind that one frozen class for its whole lifetime;
content identity and pure Archive mode do not become quota authority. No data
root gains remount or mutable admission authority. Failure or ambiguous handoff
before the closed proof retains
the conservative old debit until existing kill/reap/drop cleanup settles. After
proof and debit commit, a later view-control allocation or validation failure
disposes the actual compacted owner under its new matching debit. Neither path
publishes a partial owner. Cancellation and deadlines retain their
original clocks. Verify actual quota/identity contraction and shared debit,
non-Execute Archive history preservation, large/over-inode fallback, forged or
foreign facts/owners/scopes, failure/cancel/last-drop behavior and an actual
read-only child. Reuse the existing Process transition and finalized-view
fixtures. The final original `cc_autoconf` and linked-artifact gates must then
pass within the unchanged bank before this prerequisite is accepted.

### Read-only Source backing quota contraction

The [Source quota receipt](wp1225-source-quota-acceptance-2026-11-08.json)
records actual native contraction/fallback/retirement proof and the restored
warm original sequence. Four real commands use one runtime and the unchanged
bank; steady accepted backing is320MiB, with observed peak4,803,530,752 bytes.
This is backing-capacity evidence, not an RSS comparison or default activation.

The next inactive delta addresses the recorded warm original-command capacity
refusal in the [profile-retirement receipt](wp1225-profile-retirement-acceptance-2026-11-08.json).
A retained Source at2GiB plus four sealed General roots at64MiB leaves avoidable
backing reservation pressure while the next authentic bootstrap runs. This is a
Slug-native physical-resource operation supporting the same general repository
capability and eventual authentic Rust cquery. Derived Source Path APIs do not
resolve this ledger; no caller identity may choose the quota class.

Every Source still bootstraps and copies under the full2GiB/262144 capacity.
Consuming Source finalization, with no payload writer remaining, may use the
existing same-superblock compaction leaf: actual allocated block bytes<=64MiB
and charged inode units<=8192 select the smaller read-only Archive class;
otherwise retain the full class. Source's existing256MiB logical manifest envelope,
64MiB/file and all path/entry bounds, complete manifest, semantic Eq/content identity,
source observations and original clocks stay unchanged. Never fabricate a
General reservation/history or an Execute result. Default General remains
Disabled; networking and other unsupported surfaces do not change.

Source owns a distinct pending/verified proof bound to its genuine reservation,
claim, Source backing association, bank and original scope, root/devino/mount,
FinalizeSourceArchive role, attempt nonce and exact readonly successor/domain.
Reuse the physical facts/classifier/remount and generic process lifecycle, not
General's reservation proof. Role6 requires exactly one ordered quota frame.
The parent independently re-attests the original root FD and successor; retain
the old full debit through actual bound helper reap, complete digest-tree seal
and exact readonly acceptance. Only the Source-specific closed verified proof
may atomically lower the one shared debit before publication. After commit,
later controller allocation failure releases the actual frozen owner under the
matching new debit. Before commit, refusal/cancellation keeps the conservative
old debit until kill/reap/drop completes. No IO or DICE compute occurs under
accounting locks.

Carry the closed frozen class through Source builder finalization, sealed data,
backing, readonly renewal, observation and member readers. Reads never infer or
mint a class from current usage. Both full and compact frozen owners refuse
mutable bootstrap/factory/rolling-token admission. Data roots gain no namespace,
remount or controller authority; A-scope data cannot remint under B's scope.
No eviction, interner, donor import, new cache or cap increase is introduced.
Existing Arc/Allocative and last-Drop credits remain the retention owners.

Scope is the Source root/launch/readonly-proof leaves, shared resource backing,
readonly Source view selection, role/helper protocol and focused Source tests;
root integrates docs and original/native/CAS proof. Keep the General proof
unchanged apart from exposing its numeric classifier for the separate Source
proof. Existing large modules receive only bounded ownership wiring; put the
new Source proof and its native discriminators in small owner leaves.

Discriminate actual small contraction and original-FD identity, pre-ACK full
bank debit, large body and measured charged-inode fallback, wrong Source owner/
nonce/facts, post-successor precommit cancellation, full seal failure, readonly
child/renewal, retained-reader original scope and exact final refunds. Preserve
General/Source binding separation, complete SHA/EOF transport and the existing
large Source/Source-link/CAS/runfiles gates. The actual same-runtime
absent/Homebrew A/B/A compiler sequence now passes within the unchanged8GiB
bank, without eviction or scope renewal; cold proof remains protected. This
closes only that specific capacity gap. Full Archive, public Rust and equivalent-
command RSS gates remain. Independent design and final review are required.

### Authentic Rust archive frontier

Use the existing Bazel9.2 `rules-rust-073-toolchain-owner` MODULE/BUILD and
`//pkg:probe` with `@rules_rust//rust/toolchain/channel=nightly`, lockfile off.
The unchanged fixture declares nightly2025-09-14 and ordinary toolchain
registration. No callback/toolchain-proof bodies, inert compilers, module-only
dependency stand-ins, forced extraction effects or source rewrites enter this
probe. Ordinary Native acquisition and private General configuration retain
current policies, banks and27s+3s clocks; default public activation stays Disabled.

The source-only streaming stager reuses the configured fixture's unchanged
bounded assembler and natural BCR corpus. A separate small inventory records
six unchanged workspace files plus26 cached metadata/patch/archive candidates;
64KiB copies check exact declared size/SHA/EOF and held nofollow FD/ancestor
identity. Full rules_rust release/tool payloads remain ordinary distdir inputs,
not semantic replacement outputs. One flat distdir contains verified hardlinks
from fresh owned staging copies; hierarchical mirrors and URLs remain intact.
All base/extra basename associations compare exact size/SHA before assembly.
Held source/flat directory descriptors constrain link creation; fresh checks
verify regular-file identity and complete bytes after the expected ctime change.
Logical pathname bytes and unique inode payload bytes are reported separately;
external cache objects are never linked. No large artifact is added to Git. The
unchanged owner's possible July2026 rustfmt registration does not force an
archive demand; actual Needs govern. Base fixture limits are not raised.

A private test wrapper delegates the genuine Cquery command-root contract and
normal owner. It may record bounded Need/operation/index/format summaries and
original-scope counters at completion, refusal and a default-no-op test-only
before/after progress hook. No lock spans compute/IO. A bounded private exact
content-demand census emits only ordinals and static namespace/owner classes;
overflow is explicitly incomplete. Progress deltas identify the actual returned
progress family, independently of demand-census completeness. Freeform errors
map to static diagnostic codes; input configuration is capped before allocation.
Distinguish typed driver
failure from accepted terminal semantic error and real success; retain exact
retirement/refund evidence. A finite supervised run chooses the next generic
API/archive implementation from its actual outcome, not inferred cap arithmetic.
General's exact-body state/history must not be replaced with digest-only Source
state to make extraction fit. Fixture preparation, input corrections and that
API work stay in this capability. Full Archive, authentic Rust completion and
equivalent-command RSS remain unproved until their own evidence is complete.

The [frontier receipt](wp1225-rust-frontier-acceptance-2026-11-08.json) establishes
a genuine admission conflict:43 initial states plus85 completed records fill
the aggregate128-version policy; the next root needs129 before bootstrap launch.
Existing exact/+1 regressions deliberately count each initial/successor state,
including unchanged trees. Pinned Bazel registers/configures all toolchain
labels, and the natural selected module registers37 remote-JDK configuration
repositories. Filtering these by requested Rust type would change semantics.
Storage sharing cannot reduce this logical count under the predecessor contract.
The reviewed replacement below changes logical admission explicitly; complete
history and the byte/resource limits remain.

### Retention admission dimensions

The genuine frontier contradicts using one aggregate128 count for both roots
and history. The selected **Slug-native** policy separates those dimensions.
`RepositoryWorkspaceRetentionLimits::new(max_roots, metadata_bytes,
archive_body_bytes)` retains the same explicit root capacity128 in Native
configuration. Every native claim domain, root/source slot, dependency union,
ticket forecast and controller uses `max_roots`, never a derived history bound.
The pure epoch also refuses a new root beyond its declared capacity before
initial-state allocation or native bootstrap. No caller identity selects limits.

Each retained genesis and successor still increments `state_versions` exactly.
Use one shared checked minimum metadata charge for State/Arc control and its
mandatory root-entry allocation, both in state validation and history admission:

```
state_versions * minimum_state_metadata <= retained_metadata <= metadata_limit
```

The existing16MiB metadata limit therefore supplies a finite layout-derived
history bound. All state/record tables, transfer overlap, operation/result
metadata and canonicalization scratch remain charged. Residual command budgets
subtract other transcripts' complete metadata/body charges once; they do not
subtract version counts again from a metadata-derived residual ceiling.
Keep every initial state, predecessor, successor and record; sealed, equal and
deleted history receives no refund. This revises the independent logical-version
admission policy; it does not raise byte limits or claim an allocation speedup.

The old count also incidentally constrained aggregate execution. Preserve the
per-transcript128-Execute check and add an explicit command-wide128-Execute
bound. An exact checked retention counter starts at zero and increments only
for a newly completed Execute. New-request admission checks the aggregate before
effects; completion/publication forecasts check it again before allocation or
publication. Authenticating an old record precedes replay without spending a
new execution. A failed forecast leaves the old epoch and native credits intact.
The new scalar participates in existing structural retention equality, not a
separate cache, identity digest or native authority. Fresh epochs/invalidation
and DICE ownership remain unchanged.

Protocol implementation belongs to `repository_workspace.rs`, `state.rs`,
`sequence.rs` and their focused tests. Core migrations are confined to
`repository_io/reservations.rs`, `repository_workspace_command.rs` and
`repository_workspace_driver.rs`, with root-controller boundary tests.
Existing Arc/Allocative and compact containers remain; no donor or new pool.

Discriminate more than128 fully retained versions across fewer than128 roots,
unchanged/no-op counting and replay, metadata exact/+1 and multiplication
overflow, root128/+1 before setup, and aggregate Execute128/+1 across roots.
Reuse protected Source64, Process64MiB, full-history, scope, quota and retirement
evidence, then repeat the unchanged original Rust cquery under the same Native
bank/byte limits. Source/process/backing caps and default Disabled activation
remain. The derived count bounds retained representation and traversal; it does
not establish acceptable latency or the completed-command1.25x Bazel RSS gate.

The [retention acceptance receipt](wp1225-retention-dimensions-acceptance-2026-11-08.json)
records focused protocol/Core/Loading and protected original compiler proof.
Unchanged original Rust crosses the predecessor128-state proxy under the same
byte banks. The following observation-cutoff component owns its measured graph
follow-up; developer-profile diagnostics do not establish production latency or
the completed-command memory ratio.

### Repository observation cutoffs

The bounded V2 phase probe attributes21.349s wall/21.365s parent CPU to root
compute within the unchanged27-second deadline; its final interval is aborted.
Progress consumes4.864s, including the legacy Need subset. A causal observed-key
regression now proves that querying completed original A recomputes it after
unrelated B reservation, completion, materialization and Source publication.
These are measured owners for investigation, not a performance acceptance.

The reviewed cutoff uses existing DICE `compute_opaque`/`ProjectionKey`, without
an accompanying whole-value read. A command-context projection exposes only full
workspace/lease/limits or an explicit error. A full-demand transcript projection
exposes that context and its exact optional transcript, validating complete Spec
and slot binding. Active absence remains tracked; revoked/foreign/conflicting
values remain errors. Every exposed field participates in full equality.

Loading replaces whole-epoch invocation carriers with explicit replay
context/transcript and typed prepared Source or Materialized rows. Resolve rows
only for actual Label/path/source accesses; compare full natural requests and
current context before exposing path text. Source lookup is keyed by context
and the full natural request. Materialization lookup is centrally keyed by
current context and the complete expected natural request; a conflicting row is
an error rather than absence. It validates repository/workspace and a General
row's command lease before use. No unchecked repository-only alternate or
hidden whole epoch
or catalogue may sit behind narrowed equality. Prefix declarations still begin
empty and grow only through original evaluation; prepared-but-unused rows grant
no authority. Missing rows yield the existing Need/error, never a Host fallback.

Pure replay reconstructs requests from the actual transcript; it never creates
a one-row epoch. Such requests are **unadmitted**. Core's actual full command
epoch retains aggregate authority. Existing new Execute/process-history and
root-capacity checks precede child/quota/bootstrap effects. Unknown snapshot
metadata/body forecasts still follow private mutation/capture and gate
publication; failure poisons/disposes the private root under existing staging
semantics. This cutoff adds no universal pre-IO completion gate. Aggregate
Execute/process-history refusal may move from pure invocation to Native
admission, an explicit Slug-native error-stage revision. Local validation and
authenticated historical replay remain. Parent effect Need values stay invalid
for DICE caching; a caller-held earlier Need must still pass current admission.

The [cutoff receipt](wp1225-observation-cutoffs-acceptance-2026-11-08.json)
records causal RED/GREEN, relevant-change/freshness/absence and foreign-request
refusals, held-request aggregate admission,162 ordinary tests, three actual native
guards and four warm original compiler commands. The held-request test is a
no-child protocol counter proof; only B's first operation is original. Existing
Core source and actual history/replay tests establish the pre-launch/quota join.

The same-input ABBA diagnostics retain 51 roots/100 completed records and762
root-compute intervals per run. Candidate wall intervals are19.627/19.650s;
controls21.548/21.561s. Candidates refuse at Source-attempt64 around25s; one
control reaches the unchanged work deadline and the other the same64 refusal
around27s. Profile qualifiers remain explicit, parent CPU excludes helpers, and
legacy Need timing overlaps progress. Every run refunds native FD/backing and
retained credits exactly. This is bounded attribution/cutoff evidence, not
completed-command latency, Rust success or equivalent Bazel RSS proof.

Next, attribute the64 charged source attempts to actual producers and distinguish
required reads from safe generic reuse under current custody/freshness rules.
No clock, Source, root, byte, process or backing expansion is selected. Analysis's
adjacent broad source projections remain a separate measured follow-up if the
public cquery still exposes them.

### Owned-file observation verification

The unchanged SourceAttempts64 refusal is attributed to 53 path captures
(51 General-family increments, two Source), nine successful Workspace
acquisitions and two increments in the final refused Immutable Source capture.
The census is incomplete; these totals prove neither duplicate demands nor
51 eligible regular files. The selected generic change separates verification
of already-owned complete General file bytes from content acquisition. This is
an explicit **Slug-native accounting and drift-error-stage revision**, not a
counter repair or cap increase. Default General activation remains Disabled.

Only `RepositoryPhysicalRoot::observe` may mint a closed borrowed verification
carrier from its actual sealed owner. It binds the exact full State, original
scope and anchor; it cannot be constructed from a body supplied by a generic
caller, a digest certificate, path spelling, repository name or source hash.
The carrier is temporary native call state, never a DICE value or retained
cache. The existing RootIo is borrowed from that same owner after backing,
origin and resource admission. Before any demand-path IO, the existing complete batch,
namespace/path, duplicate and storage preflight runs, and the carrier context
must match. Root descriptor/backing attestation retains its prior position. A direct own `State::File` member with its canonical body selects
verification; links, unknown members and non-file entries select the unchanged
acquisition route. Source snapshots, Host and legacy observers receive no carrier.

For eligible FileBytes or FileDigest, verify current terminal mode and exact
length, open nofollow, stream the actual FD and compare every chunk against the
owned body, check exact EOF and stable FD/name metadata, and revalidate the
original scope/activity and backing. FileBytes returns the same body Arc only
after success; FileDigest uses SHA-256 computed from those actual chunks.
Retain the FileBytes1MiB object ceiling and its sticky DecodedBytes refusal,
existing digest/RootIo bounds, the fixed scratch/descriptor credits and full
path/cancellation checks. Do not copy owned `ctx.read`'s separate decoded-byte
exemption: every actual verification chunk charges DecodedBytes **before** byte
comparison. SourceAttempts remains64 for actual acquisitions; this new closed
verification class spends no acquisition attempt. Already-poisoned scopes still
refuse. Failed verification never falls back to uncharged capture. A modeled
file that is missing, wrong-kind or differs in mode/length/bytes refuses as
Unstable/Snapshot instead of publishing a contradictory owned result. Unknown
and ineligible content attempts retain their pre-lstat debit and prior errors.

Native State credit covers the physical owner. A successful returned pure body
Arc independently receives the same monotonic DecodedBytes history charge for
every actual body byte as the prior fresh FileBytes capture; this permits it to
outlive native State destruction without claiming that refunded native credit
still covers that alias. Empty files have zero payload charge, with ordinary
epoch/result metadata admission and no second body allocation. Dropping aliases
never refunds decoded history. These payload counters do not claim permanent
Arc-header or live-bank coverage. This preserves an existing component accounting
class, not a global live-memory or completed Bazel/Slug RSS guarantee. No full body is replaced by a digest, no semantic
history is removed, and no FD/scope/credit/native setup authority enters DICE.

Implementation scope is Core's physical-root observation seam plus a small
borrowed carrier leaf, the shared rooted observer and a bounded owned-file
verification leaf. Tests use the existing rooted FD fixtures and real claimed
root/registry fixture; no synthetic native-root constructor is added. Prove a
causal native RED/GREEN for canonical Arc identity and exact attempt/decoded
counters, actual SHA and repeated verification; kernel negatives cover drift,
missing/wrong-kind/link/mode, object/decoded/cancellation bounds and atomic batch
refusal. Generic Source/Host/legacy acquisition64/+1 controls remain. Native
proof also holds the result after root/state/registry retirement, verifies
independent native refund and final body destruction without decoded refund.
Actual association-recheck, foreign-scope and warm compiler tests pass; root owns
serialized feature/default CLI builds and supervised native selection.

The [owned-verification receipt](wp1225-owned-observation-acceptance-2026-11-08.json)
records104 ordinary tests, four actual native owner guards and four original
compiler commands. With the same frozen inputs/helper/deadline, Rust General
path verification keeps its exact103265 decoded bytes and spends0 acquisitions
instead of51. Total Paths53→2 and Workspace9 stay exact. The pending Source
capture then decodes25,559,040 additional bytes and stops with IO PermissionDenied13
at14 total attempts and no sticky scope error. Completed51 roots/100records and
762 graph cycles remain unchanged; this advances private preparation within the
same pending request, not completed roots or a General Archive callback.
Exact refunds pass. No Rust completion, latency or completed-command RSS claim.
Next, discriminate the observed Source IO refusal. Static evidence suggests
final MODULE replacement over an extracted read-only regular member; its causal
producer and safe generic replacement require proof before implementation.

### Verified Source placement over read-only members

The original Rust Source refusal is discriminated by the existing original
Immutable archive fixture: changing only its packaged MODULE from0644 to0555
(and requiring the published registry MODULE to remain0644) reproduces raw
PermissionDenied13 with native FD/backing cleanup. This is an independent caller
of the same Source producer. The natural Kotlin tar likewise has0555 regular
members and a patched MODULE before final registry replacement.

Pinned Bazel9.2 `tools/build_defs/repo/http.bzl:203–218` orders extraction,
remote overlays and patching; `utils.bzl:215–242` applies registry patches then
explicitly deletes MODULE and downloads the integrity-checked registry copy.
`utils.bzl:86–115` says remote overlays overwrite existing files and marks them
executable. Exact admitted behavior is verified replacement bytes and that
ordering, independent of the old regular inode's writability. Slug's existing
normalization to0644 MODULE/0755 overlay remains. Same-root private staging,
finite namespace bounds, refusal of unsafe node kinds and unpublished failure
visibility are Slug-native; this is no new general filesystem permission or
arbitrary read-only-tree support. Unwritable parent directories may still refuse.

Repair only `SourceRootInput`'s closed placement helper. Do not change global
RootIo `WriteMode::ReplaceRegular`, process writes, patch semantics, Host policy,
privileges, caps or published Source identity. Population keeps its actual
builder/request/original-scope/resources checks and exclusive quiescent private
generation. Extracted archive, overlay-before-patch and final registry-MODULE
ordering stay unchanged. Whole Source state is still captured and sealed only
after every placement and capture close succeeds; no new DICE key/cache, native
constructor, source-body substitution or caller-specific recipe is introduced.

Before placement, check scope/activity, admit the known verified capture length
against the existing RootIo file bound, and validate the destination without
following it. Missing targets use atomic CreateNew, never an overwrite if a name
appears. Existing targets must be regular with nlink1; reject directories,
links, hardlinks and other kinds before a staging mutation. For an existing
regular target, create a fresh private root-level staging name with CreateNew.
The short root-level basename avoids deep destination path/depth growth while
remaining on the same admitted backing. A fixed8192-candidate search skips the
final destination and every preexisting candidate of any kind; it never removes,
chmods or overwrites a collision. Existing path/name/work checks bound the loop.

Stream the original capture through its unchanged frame/size/EOF validation,
count every actual write on WrittenBytes, and compare SHA-256 to its expected
digest. Set the requested mode on the staged FD only, flush it, and check the
original scope/activity immediately before commit. Recheck the original target
frame and reuse RootIo's existing `replace_regular` staged-FD/name, nofollow,
same-device, regular/nlink1 and post-rename checks. This is not atomic inode-CAS
against external writers; the exclusive private-generation precondition remains.
Close the writer and leave no stage name before full tree capture/seal. Missing
CreateNew placement keeps the same capture verification and publication gate.

The existing2*IO_SCRATCH/eight-FD admission covers placement's source/output and
RootIo scratch/handles; bounded stage-name/control storage stays inside that
fixed allowance. Old+new bytes and one temporary inode coexist under the still
live full Archive backing. No backing contraction/refund precedes final seal and
retirement; real ENOSPC still refuses. Errors may leave private staged or partial
new names, as the existing patch writer does: the whole unpublished generation
must be disposed, with all handle/capture/backing credits retained until actual
retirement. Never clean up a preexisting collision or expose a partial Source
epoch. No SourceAttempts/DecodedBytes reclassification is added by this repair.

Core scope is `repository_source_root/input.rs` plus a cohesive placement leaf
and tests; RootIo replacement is reused unchanged. Strengthen the existing
original archive fixture for native RED/GREEN and existing readonly overlay,
then preserve actual fresh commands and exact cleanup. Ordinary FD tests cover
regular/missing destinations, mode/content/inode results, forbidden kinds,
collisions/exhaustion/deep paths, capture-integrity/cancel/write-limit refusal
before commit and preservation of the old target. Root owns serialized Core
feature/default CLI builds, protected Source/patch/owner/compiler gates and the
same-input original Rust comparison under unchanged27+3/native/source/archive
limits. Pinned source plus the causal original regression establishes the named
slice; no Rust completion, production activation, latency or RSS acceptance is
implied. A new boundary is attributed before another generic change.

The [Source-placement receipt](wp1225-source-placement-acceptance-2026-11-08.json)
records70 ordinary tests, the actual original readonly MODULE/overlay fixture
across two fresh commands, protected large Source lifetime and four warm compiler
commands. The MODULE-only RED and overlay-strengthened GREEN are distinguished.
Same-input Rust now crosses PermissionDenied13, completes806 graph cycles and
retains52 roots/104 records (156 states), then reaches the existing27-second
work deadline. Root-compute intervals total21.255s wall/21.627s parentCPU and
explicitly include one aborted final interval among807 closed intervals.
Source acquisitions are last observed at17; no General Archive callback is
observed. Peak native backing/retained counters stay unchanged and every owner
refunds exactly. This is boundary progress, not a completed command, latency,
archive-envelope or equivalent-RSS proof. Attribute graph cost before the next
generic optimization; Analysis's adjacent broad Source projections are a
candidate to investigate, not an accepted cause or implementation selection.

### Bounded DICE family attribution

The existing bounded Rust frontier spends most of its work scope in RootCompute.
The accepted family attribution proves reachability in a stopped prefix, without
assigning time to any key family. CapturedSourceSnapshotKey remains unobserved
in these prefixes and is deferred as a candidate for this frontier.

Add fixed scalar counters only to the existing `probe_observer::Observer` under
`cfg(test)` in the existing native-probe-observer feature. Its current listener
already receives DICE events; do not add a runtime hook, replace/combine userdata,
change the listener selection, mapping ABI, semantic key/value or scheduler.
The existing install/disable/Arc lifetime and mapped counter/activity behavior
remain. Classify the static short key-type tag into a fixed table of at most24
named families plus `other`, using only scalar atomics and checked saturating
counts with overflow masks. Do not retain or report observed tags, concrete
keys, IDs, paths, source values, environment, addresses or arbitrary frames.
The fixed test-only state follows the existing Observer and its Allocative
self-size; no semantic cache/global store or unbounded collector is added.

Count all six existing DICE event variants: Started/Finished, CheckDepsStarted/
Finished and ComputeStarted/Finished. DICE `api/events.rs` supplies only static
type names; `impls/worker.rs:345–347` closes ComputeFinished by RAII even on
cancellation. Its synchronous projection path emits Started/Finished without a
Compute pair. Counts therefore indicate reachability and dependency/compute
activity, not per-key self-time, CPU, successful evaluations or exclusive spans.
No per-family timing is selected. Preserve imbalance/overflow qualifications;
finish events cannot certify semantic completion. Events beginning after
observer disable do not count; already-entered callback behavior stays aligned
with the existing counter semantics. Disable is not an atomic snapshot barrier
for callbacks already past that gate. Report observed per-cell post-disable
counts and possible imbalance, not transactional completeness or a new drain
guarantee; retirement remains a prerequisite rather than a counter barrier.

Read the fixed snapshot through the already-held profile Observer only after
outcome/runtime retirement, native refunds and guard disable. Emit static family
labels and six count columns with overflow/coverage qualifiers inside the same
512KiB total frontier+cleanup report ceiling. Reserve a fixed larger diagnostic
slice by reducing existing event-trace allowance; do not grow the report ceiling
or dump sampled activity frames. This adds no production result/credit/scope or
claim of complete key identity. Ordinary tests cover all phases, known/other
classification without payload output, concurrency, saturation, disable/last-drop
and preservation of existing mapped counters. The existing ignored original
frontier supplies bounded attribution under unchanged inputs/caps/clocks; a
stopped probe remains no public-command or memory-ratio proof.

The [result-cutoff receipt](wp1225-materialization-cutoff-acceptance-2026-11-08.json)
joins attribution, the selected causal regression and the semantic cutoff below.
Use observed reachability plus a causal regression before subsequent graph
changes. Preserve full identity/source custody and Need validity; family counts
are neither self-time nor successful-command evidence.

### Per-request materialization result cutoff

The bounded original Rust attribution records no CapturedSourceSnapshotKey
activity, so that Analysis candidate is deferred. It reaches the ordinary
RepositoryMaterializationResultKey (2072 compute starts in this stopped prefix).
These counts establish reachability only. The producer previously read the
whole result epoch while selecting one exact request association. The accepted
actual-key causal regression is RED on unrelated B publication and GREEN with
the request-bound cutoff, including retained-only inventory refusal.

Use a private result-entry ProjectionKey derived through compute_opaque from the
actual RepositoryMaterializationResultEpochKey. Its identity retains the full
request; hashing may reuse workspace/repository identity as the existing result
key does. Select only the actual current entry when the epoch workspace and
full request match. Missing injection, foreign workspace, absent association or
same-id/full-request conflict retain the existing parent Need behavior. Never
resurrect a retained generated entry or construct a reduced epoch. The selected
value retains the actual request Arc and complete result; structural equality,
General history, Source bindings, path policy and observation instance remain.
Debug/Display expose only the existing static/repository identity metadata.

This is a Slug-native DICE dependency cutoff with unchanged repository behavior.
Keep the existing context-bound public materialization projection separate:
this producer also serves Disabled and standalone acquisition without a fresh
Workspace context. Add no policy/context/native owner or admission edge. The
parent result key keeps Need invalidity and its existing Local/Immutable/error
conversion. Generation-bound transport/materialization errors still observe the
current generation and retry when it changes. Core retains full current-epoch
association and native authority checks before effects.

The [result-cutoff receipt](wp1225-materialization-cutoff-acceptance-2026-11-08.json)
records actual-key RED/GREEN, owner/dependent compilation, protected native
admission and original compiler checks. Same-input ABBA stopped probes show
fewer result computations but variable prefixes and unchanged work-deadline
refusal; command latency and equivalent RSS remain unproved. Preserve all
clocks, caps, original fixtures and default Disabled policy. Attribute remaining
module/dependency work before further generic changes. Registration batching
would additionally need an explicit resource decision because Core initializes
new reservations before later operations; more simultaneous Archive backings
must not bypass the existing bank or choose caller-specific pruning.

### Bounded observation-union attribution

Before selecting another graph change, distinguish the already accepted private
Loading no-op union from adjacent helpers that reconstruct their certificates.
The existing Core native-probe-observer feature may enable a separate Loading
diagnostic feature. A fixed three-owner scalar table observes existing Bzl
union, canonical route union and selected-effect union returns. Record only
calls, input/output row totals, successful no-new-key cases, left/right map
sharing, empty inputs and errors, with checked saturation and overflow masks.
Read lengths and pointer equality in constant time after the unchanged helper;
add no equality scan, retained input, clock, graph edge, listener or userdata.

This explicit measurement exception uses feature-only process-wide counters,
separate from the Observer-owned family table. There is no reset or enable
state. The ignored profile takes initial and post-retirement scalar snapshots;
their per-cell differences are neither atomic nor request-isolated. Preserve
initial/final overflow, preexisting saturation and reversed-delta qualifications.
Returned-helper counts and overlapping classes prove observed reconstruction
opportunities, not allocated bytes, self-time, drain or command completion.
Keep the existing 16 KiB diagnostic reservation and 512 KiB total ceiling.

Use actual original selected-effect certificate/map ownership RED and observed
rebuilding work before choosing pure union migrations. Preserve complete facts,
stable left demand/result owners, first sorted conflicts and current freshness.
Raw constructors, Need validity, default Disabled policy and all resource caps
remain unchanged. Keep diagnostics with the selected semantic proof, with
independent review and root-serialized Cargo/native execution.

### Validated observation union

The original-rule ownership RED preserves full facts and individual result Arcs
but detects a reconstructed certificate map. Bounded attribution observes 45
canonical route and 411 selected-effect unions returning no new keys without
map reuse. The existing private Bzl union already reuses contained inputs, but
also performs actual-growth unions. These are reconstruction opportunities,
not an attribution of RootCompute time or completed-command memory fit.

Provide `PathObservationEpoch::try_union` over two already validated immutable
epochs. Keep raw `new`/`from_shared` validation and error precedence unchanged.
Shared-map or empty-right unions retain the left map; empty-left may retain the
right map. Scan sorted right entries against the full left facts, using pointer
identity only as a shortcut for the same immutable result. Return the first
sorted conflicting demand. If no key is new, retain the left map after checking
all overlaps. Equal foreign results do not replace left canonical owners.

For growth, use checked unique-row capacity and a two-way sorted merge through
the existing OrderedMap/SortedMap utilities. Every duplicate selects the whole
left demand/result pair, including independently charged Image key and result
owners. Retain shared payloads without copying bodies; add no temporary full
sort vector, interner, global cache, retained field or donor dependency. Pure
union does not establish source freshness or grant native authority.

Migrate only the three measured Loading helpers, keeping their mode gates,
error wrappers and diagnostic counters. Other wrappers and raw append/capture
paths remain outside this selection. Workspace tests compare exact results and
errors against the existing constructor, protect two-bank key/result custody
and last-Drop refunds, and reuse per-demand change/revoke/restore controls.
Actual original-rule RED/GREEN, existing Bzl ownership controls, direct-dependent
compilation and protected native/original probes join root-serialized validation.
All configured clocks, caps, Need validity and default Disabled policy remain.

The [union receipt](wp1225-observation-union-acceptance-2026-11-08.json) records
216 ordinary tests, actual original ownership RED/GREEN, native admission and
four original warm compiler commands. In both bounded candidate prefixes,
46 route and 566 effect no-new-key returns retain the left map; both controls
reconstruct them. Every ABBA run retains the same recorded 51 roots/103 records
and reaches WorkDeadline. This closes the local representation/ownership gap,
without demonstrating deeper Rust progress, command latency or RSS fit. Further
work must discriminate prerequisite serialization rather than infer time from
map counts. The following bounded Lstat-only scheduling contract selects the
first discovery class; other Need families retain their own resource decisions.

### Ordered metadata prerequisite discovery

Select bounded serial lookahead in module and command registration rows, through
their existing DICE keys. After the first incomplete row, collect only whole
path-only Needs containing Lstat operations. An exhaustive Bzlmod classifier
requires the path channel and excludes all ten other channels; it neither
extracts a path subset from a mixed Need nor grants source authority. Other path
operations, mixed/non-path Needs, semantic/outer errors and oversized groups are
whole barriers. Selected-pattern/root-mapping prerequisites remain immediate.

At most 32 inspected row slots, including the first hole and later Complete
rows, may contribute. Bound the pending frontier to 32 unique full demands,
4096 path bytes per demand and a checked 64 KiB frontier/control construction
peak. The forecast includes actual retained PathBuf spare capacity, both input
arrays, duplicate path owners, union Vec/result Arc overlap and Source Need/map
controls. Check it before merging or cloning a large frontier. An ineligible or
oversized first Need is returned unmodified before any lookahead. A later
barrier or window stop returns only already pending earlier paths, without a
new limit error or truncating a row's Need.

Partial labels, seen sets, ambiguities, routes, packages and observations remain
ephemeral discovery scratch. Never finish or publish a Complete family result
while paths are pending; discard the scratch and restart the original ordered
expansion on retry. Preserve module order, signed command order after toolchain
reversal, full facts, first decisive semantic error and Need invalidity. Child
DICE computations retain their existing values and event custody; command event
publication must use the selected terminal closure rather than all speculative
activations. Do not change event-capture userdata or cache a print-free result.

This is a Slug-native operational scheduling/admission revision. The 64 KiB
forecast bounds frontier construction, not all speculative child DICE values,
recursive-row work or expansion scratch. Extra bounded metadata work can still
encounter an earlier dynamic bank/timeout refusal than serial discovery. Exact
old-Slug admission and Bazel restart/error timing are not claimed. All existing
caps and Core whole-batch authority checks remain; no poisoned reservation is
caught, cleared or retried. Content, digest, directory, image and Windows-path
Needs continue serially outside this first lookahead class. No label pruning,
new native reservation/effect batching or caller-specific policy is selected.

The existing feature-only phase report samples a fixed 40-byte process-wide
collector: eligible windows, inspected window rows, accepted extensions and
new unique demands per extension. Saturation, masks and reversed samples make
deltas unavailable. Samples are neither atomic nor request-isolated; no keys,
paths or epochs are retained, and no allocation/time or distinct-global-demand
measurement is claimed. Keep the existing 16 KiB report reservation and 512 KiB
combined ceiling. Original Rust evidence must show genuine frontier extension
before accepting this candidate as bootstrap progress.

Pinned Bazel 9.2 RegisteredToolchainsFunction and TargetPatternUtil establish
all-pattern expansion, ordered signs and reversed command-toolchain precedence.
The stricter deferred barrier policy is Slug-native. Use actual registration-row
RED/GREEN, complete-input recovery, ordered barrier/error/event tests and exact
first/extension boundary controls, then protected lifecycle/native/original
workloads under unchanged clocks and resources. Root serializes Cargo and native
execution; independent source and final evidence review apply.

The [lookahead receipt](wp1225-metadata-lookahead-acceptance-2026-11-08.json)
records actual row RED/GREEN, 66 ordinary tests, protected native/original
commands and unchanged-input ABBA. Both candidates add 18 demands across frontier
extensions and perform additional speculative child computations. All runs
retain 51 roots/103 records and reach WorkDeadline. This closes the bounded
registration-discovery behavior, without a claim of less total work, deeper progress,
latency or RSS improvement, or command completion.

### Configured toolchain prerequisite attribution

Before selecting another graph change, count the actual parent resolver's
returned Need by thirteen fixed stages: target platform, execution-only/full
registrations, candidate platforms, requested constraints, preferred platform,
initial package closure, required types, declaration targets, actual-declaration
package closure, declaration types, target settings and declaration constraints.
The candidate-platform marker follows the existing outer-error/Need/semantic
fold; three macro-return sites bind the existing outcome once before marking
and preserve the original return. No child evaluation, Need iteration or union
is introduced. Abandoned futures are not recorded as returned Needs.

The existing feature-only Core report samples seventeen fixed Analysis counters
and one overflow mask, totaling 144 bytes. The first fourteen are resolver entry
and the thirteen selected Need stages. Three appended cells record caller-side
candidate preparation entry and its full/execution-only registration Need exits.
That caller-side stage follows the joined resolver outcome fold in the inspected
node block; it is not a universal pre-resolver stage. Its later platform loop
may also return Need and is outside the three-cell extension.
No keys, labels, paths, Need values, clocks or callbacks are retained. Counts are
process-wide, non-atomic and not request-isolated; nested resolvers count
independently. Entry minus Need counts is neither success nor cancellation.
Keep null deltas on overflow, MAX or reversal and preserve the 16 KiB report
reservation and 512 KiB combined ceiling. Actual owner tests must distinguish
stages before the original Rust workload chooses the next causal discriminator.
A reached stage establishes provenance, not cost or an optimization decision.
Instrumentation remains coupled to the subsequently selected semantic proof.

A separate 48-byte feature-only package BUILD-child collector counts entries
and the actual returned whole singleton-Lstat, singleton-FileBytes or other
Need, plus added unique demands per discovery-frontier extension. The added
demand counter is neither globally distinct nor confirmed serviced. It borrows
the exhaustive channel classifier and checks length/first
operation only, with no Need iteration or retained input. Mixed, nonpath and
nonsingleton Needs remain in Other; this does not select content discovery.
Actual inventory A/B withheld facts and original-owner reachability must precede
any sibling-discovery policy change. Parent evaluation/publication is distinct
from legitimate transient child capture.

Select the same bounded Lstat discovery policy for repository package BUILD
imports. One shared crate-private collector/forecast owns the existing 32-row,
32-unique-demand, 4096-path-byte and 64 KiB frontier/control bounds. Its generic
outcome adapter only extracts/rebuilds a whole Need; every non-Need outcome
remains unchanged. Callers supply no admission or eligibility knobs. Optional
feature hooks identify counters only and cannot change scheduling.

Each package import row observes route and child completion once. Retain the
existing immutable observation carrier for the next row when an earlier Need
is pending, and update module/observation scratch only on Complete children.
Finish a pending Need before package evaluation or parent event publication;
retry starts from the original ordered imports. Load-label prevalidation remains
immediate. FileBytes and all other non-Lstat/mixed/nonpath Needs are whole
barriers, as are later semantic/outer/cycle errors. First ineligible Needs are
returned unchanged; later barriers return only prior pending paths. Preserve
current registration behavior while extracting its shared policy. Extra child
DICE work and earlier dynamic bank/time refusal retain the explicit Slug-native
admission qualification; no native cap, poison handling or source authority
changes. Acceptance requires actual inventory RED/GREEN, barrier/closure/cycle/
cancellation/repair proof and nonzero original-workload added demands at this
owner, followed by protected native and warm-original gates.

The report stores identical global sidecar qualifications once in
`scalar_counter_scope`, with explicit references from each sidecar. Observer
callback/lifecycle qualifications remain separate. DICE family labels, six event
counts and overflow masks use three index-aligned arrays, preserving every
scalar without repeated per-row field names. Both long exact deltas and full
MAX/overflow forms must fit the unchanged 16 KiB reservation with the existing
1024-byte test margin. No report ceiling or native admission cap increases.

The [package-child receipt](wp1225-package-child-lookahead-acceptance-2026-11-08.json)
records reached full-registration preparation, actual package-owner Lstat Needs,
A-only/A+B RED→GREEN and one-round returned-facts recovery. All 106 ordinary
checks, four protected native guards and four warm original compiler commands
pass. Each original candidate adds six demands across frontiers; all ABBA runs
stop at WorkDeadline with variable 49/98 versus final control 50/99 epochs. Final
candidate RootCompute totals include an aborted interval. Neither these counts
nor operational peaks establish reduced total work, speed, deeper progress,
completion, activation or equivalent-command memory fit.

### Immutable canonical-route hash reuse

Select bounded hash-work reuse for `HostCanonicalRepositoryRoute` only. An
actual user-IP CPU sample of the original Rust command finds a 4.88% Sip writer
group whose identified direct static callers resolve to route mapping code in
pinned executable inline source locations. No runtime callchain or proof
excluding every indirect/tail reference exists; this selects a hypothesis,
independently tested by the actual-route work regression. Flat samples do not
attribute all hash work to that owner. BUILD synopsis reuse, root-route hash changes and
FileBytes discovery remain unselected.

Every successful builtin, selected and generated route constructor computes the
existing complete structural field writer once into one private weak routing
word. Immutable clones retain that word. Repeated `Hash` calls emit exactly one
u64/eight bytes, independent of mapping or specification size; this is the
primary acceptance threshold declared before measurement. Construction pays the
existing writer once, including its existing sorted mapping hash scratch and
both specification identity traversals. Nested writers stay unchanged.

Equality compares all original workspace, canonical repository and source
fields; the cached word supplies no equality, content, source, persisted or
native authority. Top-level specification attribute membership and generated
mapping membership retain existing unordered equality. Nested specification
dictionary publication order and selected mapping order retain their existing
equality semantics. A forced candidate-hash collision must still distinguish
unequal routes and actual key lookups. No DICE cacheability, source certificate,
current-route freshness, admission cap or production activation changes.

Keep the scalar inline with existing Arc/Allocative ownership and account its
positive layout bytes. No new owner, global interner or cache is introduced.
Require actual DICE-produced route RED/GREEN fixed-work proof, all-constructor
order/collision/full-equality and clone/last-owner checks, relevant direct
consumer and protected native/warm-original gates. Alternating unchanged-input
original commands are secondary stopped-prefix evidence; construction and
repeated-use work must remain distinct. They cannot establish completed-command
latency, RSS, bootstrap completion or the 1.25-times-Bazel memory gate.

The [route-hash receipt](wp1225-canonical-route-hash-acceptance-2026-11-08.json)
records 69 passing focused checks, four native guards and four warm original
compiler commands. Actual registry routes reduce outer-hasher work for 64 calls
from 1,664 writes/20,608 bytes to 64 writes/512 bytes. The original field writer
is byte-identical and the observed inline layout delta is eight bytes. Original
ABBA records 681/889/884/197 cycles, all WorkDeadline; candidate epochs are 53/109,
first control 49/95 and final control 8/15 after a 30.2-second materialization
wait. Keep the outlier and stopped-prefix qualification. Next structural work
requires its own actual owner/discriminator; no source class or cap expands.

### Immutable observation-demand hash reuse

Select one private weak routing word on immutable `PathObservationDemand`.
The actual Rust CPU sample reaches Demand hashing directly and its static path
to the sampled OS path writer. Other path/hash/comparison callers share those
writers; do not attribute their complete sample groups to Demand. This owner
selection requires an actual inventory-returned Demand work regression.

Compute the original five fields once at each successful constructor: namespace,
normalized path, operation, raw Windows UTF-16 input and charged Image path.
Image eligibility validation remains before construction. Clone/Dupe retain the
scalar. Repeated hashes of each actual returned inventory Demand must write
exactly one u64/eight bytes per call (64 writes/512 bytes for 64 calls),
independent of path length; this is the acceptance threshold declared before
measurement. Construction still pays the original full field writer once.

Full equality and ordering compare all five original fields in their existing
order using platform Path semantics, ignoring the weak word. Keep Debug's
original fields, raw Windows spelling, operation/namespace identity, Image
selector identity and independent budget custody. No weak hash supplies source,
content, state, history, native admission or persisted identity authority.
`NormalizedAbsolutePath` storage and full generic Hash remain unchanged: the
repository workspace spelling candidate's pinned version-one SHA stream depends
on that full writer and must keep its exact bytes. Demand Hash is operational
map/DICE/shard routing only. Both shard injection and lookup retain the same
shared selector, whose bucket assignment is explicitly nonpersistent.

The positive per-value layout bytes flow through existing `size_of<Demand>` and
row-sized Image, Need, native/source and driver scratch forecasts. Measure
actual Demand/key/row deltas and retain Allocative ownership; add no heap owner,
interner, global cache, metadata exemption or cap increase. Existing path-owner
capacity charges remain exact because their representation does not change.
Prove full-field constructor binding, forced collisions/full OS ordering, raw
UTF-16 distinctions, actual DICE current-fact A/B/A/removal/cancellation and Image
left-key/result custody with exact final refunds. Relevant consumer builds,
native guards and warm originals precede acceptance. Original ABBA prefixes
remain secondary diagnostics, with outliers and aborted intervals retained;
completion, latency, RSS and General activation remain separate gates.

The [demand-hash receipt](wp1225-demand-hash-acceptance-2026-11-08.json)
records actual inventory RED→GREEN, 183 ordinary checks, five protected native
guards and four warm original commands. Demand/key layout grows 152→160 bytes
and rows 160→168; the original full path region and workspace identity production
remain byte-identical, and the real Linux v1 candidate golden stays exact.
An intermediate borrowed comparison tuple was found in codegen and removed via
direct original-field Eq/Ord before final builds and measurements. Final static
inspection qualifies remaining spills and makes no timing claim.
ABBA cycles609/832/853/853 retain two WorkDeadline refusals and two accepted
analysis terminal errors in the same bounded category/attempt/recorded epoch.
The full errors are withheld by the existing reason formatter, so identical full
error and successful completion are unproved. Next typed failure classification
must remain bounded/static and coupled to the demanded generic semantic fix;
no freeform data, new report cap or arbitrary error-output escape is selected.

### Bounded homogeneous FileBytes prerequisite discovery

Select a second homogeneous scheduling class for the same shared registration-row
and repository BUILD-import discovery boundary. First eligible whole Need fixes
its window class to Lstat or FileBytes. Keep the accepted synchronous Lstat
algorithm, forecast and four counter meanings unchanged. No caller, source,
repository, rule or function flag chooses the policy.

A whole FileBytes-only Need is eligible only when each new unique full demand's
exact terminal namespace/path Lstat key yields a current tracked Present regular
file with nonnegative size. Query those keys sequentially through DICE; never
inspect whole mutable epochs, Source credits or native IO, and never manufacture
a new metadata Need. Missing/revoked/unknown/error/nonregular/negative hints make
the whole candidate ineligible. Lock the class: mixed/class-mismatched/nonpath
Needs, semantic/outer/cycle errors and resource overflow are barriers. A first
ineligible Need returns unchanged; a later barrier returns only earlier pending
paths. Existing load-label prevalidation and terminal event closure stay exact.

Keep 32 inspected rows from the first hole, 32 unique full demands, 4096 bytes
per path and the unchanged 64 KiB frontier/control construction bound. The
FileBytes class additionally bounds the sum of known sizes for unique demands
to 256 KiB. Its only persistent hint state is that aggregate scalar; same-demand
duplicates already admitted in the same fixed transaction do not spend it again.
Before hint polling and before extension allocation, checked arithmetic includes
the old geometry plus actual wrapper/window/key/result/one-future layouts and
return storage. Synchronous ready/Lstat dispatch constructs no byte future. A
separate byte-only helper may allocate one ephemeral boxed future after checking
twice its actual unboxed frame for stack-to-heap overlap, alignment and box
controls within the same 64 KiB construction bound. Completion, refusal and
cancellation drop that job and its full pending/right owners. Use no future
vector, hint map or retained pool; do not raise or hide bytes in the old Lstat
1024-byte fixed allowance. Prove that ready/Meta dispatch fits that old allowance
for both concrete error adapters and all three production call sites; inspect
bounded final codegen to exclude an unconditional large byte frame in dispatch.
The fixed allowance measures the maximum of live semantic control phases:
incoming/returned error-or-ready plus one window, or consumed Need plus the
in-place window and synchronous candidate/iterator scratch. Those phases do not
sum after their owners retire. Drop rejected incoming Needs and operation-clone
temporaries before constructing a ready error. This object-layout forecast does
not bound compiler ABI stack slots, spills or whole-process RSS.

The 256 KiB total is a declared-hint scheduling envelope, not native permission,
prepaid data credit or a strict actual-capture/peak bound under drift. Every old
family-specific fresh FD/object/EOF/final-name/current-association check remains
authoritative. Generic content still consumes Source attempts before IO, including
failures; exact owned General verification keeps its existing exemption without
fallback while charging actual decoded bytes. Decoded history, live scratch,
body/backing/root/FD/process/stream/time caps and sticky refusal remain unchanged.
Whole batch authority validation precedes IO; quota admission is sequential and
an eventual failure publishes no partial epoch while retaining earlier spent
charges. Extra speculative capture can cause an earlier bank/time refusal than
serial discovery. This is an explicit Slug-native scheduling/admission revision,
not a claim of serial-equivalent memory pressure or Bazel restart timing.

Require actual Inventory and registration A-only/A+B withheld-content RED/GREEN
with truthful size facts, service only returned real facts, and complete in one
round with ordered targets and full source/certificate/event custody. Protect
exact/+1 totals, duplicates/zero-size files, current-fact replacement/removal,
class barriers, later errors/cycles/cancellation and native preflight/Source64/
drift/refund/General-owned verification. Original configured Rust runs must show
an installed unique FileBytes extension before acceptance. One feature-only
monotonic boolean records only a successful union adding a unique demand; first
holes and duplicate-only windows leave it false. Before/after snapshots remain
process-wide and nonisolated; preexisting true is unattributable, and an extension
is not proof of servicing or success. Keep this fixed report outside the unchanged
16 KiB phase reservation and within the existing aggregate/stream ceiling.

The [component receipt](wp1225-file-bytes-lookahead-acceptance-2026-11-09.json)
records the actual owner RED/GREEN, control-layout correction, protected native
proof and installed original-workload extensions. Its static terminal category
retains an opaque Registration wrapper; private leaf/API identity and successful
Rust completion remain unproved. Next use retained typed discriminants without
rendering or parsing payloads. Diagnostic work stays coupled to demanded generic
behavior, with no separate diagnostic-only integration or cap increase.

### Bounded typed Registration failure projection

The current original Rust failure retains an Analysis Registration wrapper.
Classify its borrowed causal variants before selecting the demanded generic
repair. A feature/test-only hidden Loading getter returns one fixed ASCII tag;
it changes no retained error, equality, DICE key, validity, event or source owner.
Use at most 32 inspected cause nodes including the initial Registration envelope.
Each Factored/Child wrapper consumes a step; exhaustion returns a fixed depth tag.
Match existing borrowed Node/diagnostic views without allocation, cloning, IO,
collection scans, labels, messages, plans, predecessors or success data. Never
call the presentation walker, Display/write helpers or recursive frontier lookup.
A private or flattened boundary remains explicitly opaque until a narrow typed
producer view is justified by the actual capture; infer no missing API from text.

Keep each static tag within 64 ASCII bytes and its complete terminal_leaf JSON
field within the existing 96-byte reserve. Core uses the hidden getter only for
native-probe-observer; default projection remains opaque. Keep all aggregate,
phase, native, work/cleanup and stream caps unchanged. Reuse natural wrapper,
privacy/poison, Arc custody and exact 32/33-node depth fixtures, then capture one
actual original command under the existing immutable inputs and supervisor.
Couple this instrumentation to the source-derived demanded generic behavior;
no diagnostic-only main integration or activation is selected.

The private ignored probe may use a strictly parsed terminal-only toggle to omit
only the optional InstalledProfile observer attachment/finish. Default profiling
stays unchanged; the classifier feature, Trace/Census and native progress/credit
counters remain active. Reduced mode is explicitly reported as enabled:false
with a fixed purpose; unavailable observer interval/DICE-family metrics are
absent rather than zero. Preserve the existing phase/aggregate reservations,
request/configuration/environment inputs, 27+3 clocks, admission and cleanup.
This tests instrumentation headroom; a different reached prefix or leaf proves
neither observer causality for an earlier error nor speed/RSS or Rust success.

### Shared immutable path ordering

The current bounded userspace IP profile exposes shared standard-library path
comparison work, not a complete owner attribution. Select only the natural
NormalizedAbsolutePath ordering boundary reached by real observation-demand
sorting/union. Its private Arc<PathBuf> is immutable and only borrowed as Path.
Pinned Arc::Ord always invokes PathBuf::cmp, even for the same allocation.
Use Arc::ptr_eq only as a sufficient Equal witness for that one path slot;
otherwise invoke the exact original PathBuf::cmp. PartialOrd delegates to cmp.
Keep derived Eq, Hash, Debug, Clone/Dupe and Allocative, all storage/constructors,
OS-native ordering, raw UTF-16 and subsequent Demand fields unchanged. Add no
cache, interner, field, native probe, permission or resource-cap change.

Before implementation acceptance, an actual PathObservationKey returned Need
must retain its original path owner. For 64 same-owner path/returned-demand
comparisons, old-equivalent fallback work is RED64→GREEN0. Independently owned
equal/different paths still take the full comparison and agree with std order;
operation/namespace/raw-input/Image distinctions survive shared path identity.
Private cfg(test)-only thread-local accounting measures deep fallback calls,
not components, bytes, time or native request totals. Preserve full Hash streams,
pinned workspace/config SHA bytes, layout and Image last-owner refunds. Validate
named direct dependents and protected native/warm originals before integration.
Balanced stopped prefixes and bounded codegen qualify cost and do not prove
whole-command latency, RSS, successful Rust analysis or activation. Other Path
users and distinct owners retain the original work.
The [component receipt](wp1225-shared-path-order-acceptance-2026-11-09.json)
records the accepted work boundary, preservation audits and native evidence;
it does not close the unreproduced Registration cause or whole-command gates.

### Operational configuration-key hashing

The fresh post-path IP-only profile directly samples `SlugConfiguration::hash`
at 13.18% self time. This is evidence of repeated full canonical-byte hashing,
not attribution of shared String/SipHasher samples or a command-speed claim.
Select the actual Analysis `ConfigurationKey` and Loading
`CommandRegistrationExpansionKey` lookup boundaries reached by configured
analysis and command-registration preparation. This is Slug-native operational
routing; all admitted semantic behavior and unsupported boundaries stay fixed.

Reuse the existing immutable 32-byte configuration projection as bucket input.
Analysis preserves the identity discriminant and delegates the Legacy arm to its
original complete writer; Loading preserves workspace/configuration/family order
and changes only the configuration slot. Full derived structural equality,
ordering, Debug, Clone/Allocative, constructors, layout and ownership stay intact.
Projection collisions only increase bucket contention. They cannot establish
configuration, function-owner or artifact equality. Global `SlugConfiguration`
Hash/StrongHash, canonical encoding, projection production, complete configured
identity and artifact/FileWrite writers remain byte-exact. Starlark owner hashing
is also operational bucket routing; its independent complete structural equality
and input-validity owner remain authoritative.

Buck2-derived `starlark_map::Hashed` supplies the concept of separating repeated
routing work from full equality/StrongHash. No donor code, extra cached scalar,
allocation, interner, constructor hash or owner is introduced: `finish_configuration`
already computes this projection once from all canonical fields. Existing positive
layout/accounting and last-owner lifetimes therefore remain unchanged. Keep tests
in separate owner-local modules; the large Loading module gets only a short Hash
implementation and test wiring, with no unrelated extraction.

Before acceptance, repeated fresh natural-key construction and hashing from the
same immutable configuration must reduce configuration-slot bytes from the full
canonical stream to exactly 32 per call for both families and observation keys.
The primary threshold is this exact fixed byte-work reduction, including fresh
constructors; no wall-time threshold is claimed. Verify Legacy exact writer,
forced bucket collisions with full equality, configuration mutations, workspace,
family, transition/platform distinctions, complete identity/artifact preservation
and actual registration A/B/A reuse. Global configuration tests and named direct
Analysis/Core/CLI dependents, protected native guards and warm originals must
pass. Balanced same-input bounded original captures qualify reached prefixes and
cleanup only. Full Rust cquery, archive/action closure, activation and equivalent
completed-command RSS/latency remain open. Replan only if an operational Hash
consumer actually requires a persisted/strong byte stream or full equality cannot
remain authoritative; routine test/compiler fixes stay within this boundary.

The [component receipt](wp1225-configuration-key-hash-acceptance-2026-11-09.json)
records both causal RED/GREEN owners, full identity/collision and direct-dependent
proof. All414 ordinary checks, five native guards and four warm originals pass.
Both bounded candidate captures reach a reproducible opaque Analysis Message
error at970 cycles; neither is a successful Rust cquery or proof of its cause.

### Current captured Source transport (accepted inactive component)

The [captured-source transport receipt](wp1225-captured-source-transport-acceptance-2026-11-08.json)
records the original-to-Analysis/native join and the discriminators below.

Analysis uses one tracked metadata projection per canonical repository. With the
inactive General policy enabled, it observes the fresh workspace and Source
snapshot epochs even when no Source is selected. It derives the full natural
request through the ordinary observed route/root mapping. An exact captured
state selects its anchored path for filesystem source existence, content and
staging; conflicting, foreign or revoked state refuses. An uncaptured subject
keeps its ordinary producer. A later real repository-context capture invalidates
that earlier empty selection and recomputes the complete action preparation
before effects. No exposed path is rebased. CatalogOnly remains an explicit
byte-owner API; module and .bzl acquisition keep their existing producers.
This is mixed acquisition for already required captures, not automatic capture
of every Analysis input or a fully closed workspace policy.

Native completion extends only the selected data lease with a Snapshot variant;
materialization requests/rows and General exact-body histories are unchanged.
The independent current Source epoch must match all actor Source rows and the
exact workspace/lease/scope/bank. Full request/state comparisons happen outside
actor locks; complete row/count and original-scope checks join the existing
materialization recheck before leases return. Selected Source members require
exact repository, namespace, anchor, containment, regular-file kind and digest.
Any other namespace for a captured repository refuses, including Host and an
older legacy materialization, even on a CAS hit. Local/BCR Source requests also
compare against any current ordinary request for that repository; genuine
absence is allowed, but a changed kind or full request refuses. Workspace and
Builtin captures refuse any same-repository ordinary request. The final joint
lock rechecks both request presence/absence and exact retained request Arcs.
Foreign current requests prepay their complete retained metadata before cloning;
shared requests reuse their Source root guard and retire before that guard.
Opaque Source readers use the existing original-scope framed FD kernel,
shared reader bank and whole blocking-job custody. CAS and durable runfiles
backing still independently verify complete length/SHA/EOF and modes.

Active Source rows and pure epochs retire as before. Already issued data leases
retain their exact Source root and original scope without controller authority;
fresh B cannot remint A. No accepted Source map, body Arc, source cache, resource
cap increase or ambient reopen is added. Source InputImage remains unsupported
before ambient IO and is not used by these action/runfiles consumers.

Proof must discriminate None-to-Some invalidation, full natural conflicts,
forged Host/anchor/digest/scope metadata and source/reader last-drop custody,
plus actual original-to-Analysis-to-NativeLink transport for Workspace, Local,
selected BCR and Builtin captures. Workspace capture still covers the complete
tree and may refuse subsequent publication trees containing unadmitted links;
no output/.git/target filter or test policy may conceal that boundary. A first
admitted Workspace publication and fresh A/B for the other producers establish
this bounded join; repeated Workspace publication, automatic capture, public
activation, authentic Rust and matched-command memory remain separate gates.

Ordinary selected BCR archive acquisition keeps its existing producer and bytes.
Its successful realizer establishes a resource-free GenerationRoot path policy
in the success and current native row. The path resolver projects that property
fallibly into the exact generation root and observation instance, using the
existing pure MaterializationPathAnchor. This Slug-native
observation-frontier correction retains every root/child fact while excluding
irrelevant ancestors such as the shared temporary directory. It is sound only
for that acquired producer guarantee: archive admission already rejects
absolute/escaping links and admits links only to extracted regular files.
Legacy native leases and images authenticate the exact current success/row
policy and bound, without changing their named legacy IO dispatch. Legacy Git,
local tar and generated file/filesystem producers keep LegacyUnbounded/None
and their existing link semantics; generated repositories retain their
transitive cross-root symlink DAG. General keeps its own physical-root contract.
Neither the policy nor an anchor carries physical IO or lifetime authority.

### Original General output transport (accepted inactive component)

The [transport receipt](wp1225-original-transport-acceptance-2026-11-08.json)
follows authored repository Starlark through ordinary analysis,
anchored digest observation and the existing checked Native completion wrappers
into real NativeLink CAS/action transport and durable runfiles publication.
General output source metadata must come from SourceArtifactInputObservationKey;
a metadata fixture constructor cannot satisfy this gate. The same adopted root
must supply opaque reader bytes even though its virtual display is absent.

A narrowly opt-in `repository-workspace-test-support` Core feature supplies only
fixed finite trusted test configuration on an ordinary Native runtime and rejects
Diagnostic ownership before allocating a bank. It uses the existing shared-bank/
configuration owner and compiled-CLI test selector;
REAPI enables it through its dev dependency. It exposes primitive bank metrics,
never roots, descriptors, scope renewal, native wrapper constructors or injected
semantic results. Default builds keep Disabled and their existing executable
selection. The feature is test harness wiring, not production activation.

Proof covers cold upload, verified CAS bodies, all-hit reuse, changed/restored
Host definitions, actual remote consumer bytes, and digest/mode-verified durable
runfiles after runtime/root retirement. A real CAS failure must suppress new
publication and preserve prior visible outputs; restoring the original inputs
must succeed through the original certificate/retry owners. Existing source
scope/RPC/reader cancellation and full-certificate validation stay mandatory.
The distinct digest-only SourceRoot join has its own evidence above; this
General output proof does not independently authorize it or close production,
full archive, authentic Rust or matched-command memory gates.

## Anchored source IO and native transport

Add immutable selected MaterializationPathAnchor and anchored ResolvedPath key:
stop ancestor recursion at anchor, observe REAL fstat(rootFD), resolve children
and record ordinary symlink route/epochs/cycle checks. Above-anchor/foreign/
revoked/escaped demands refuse; Host resolver unchanged. Internal relative or
absolute-under-same-display links may resolve within instance; no automatic Host
conversion. Common Bzlmod source route supplies anchor; Analysis follows it.

The existing materializer registry holds the exact root Arc plus FULL current
selected request/result/instance association. Accepted rows replace/prune old
rows; issued immutable data leases independently retain their old generation.
Legacy generated symlinks retain only their exact referenced legacy data owners
transitively in an acyclic owner graph, bound from current namespace/path
membership before effects. These pins preserve live selected symlink targets;
they retain neither old requests/history nor unrelated roots. Source readers,
image pins and observation snapshots carry the same owner graph. Legacy links
to physical virtual roots explicitly refuse until supported views exist.
Use checked process-wide non-reusing IDs for materializer sessions, observation
instances and empty workspace leases, with no global root map. Stable display
v1 derives from full workspace/selected-owner/repository/ordinal structural
identity; full-key equality guards candidate collisions in the native selection
domain. Claims survive through the actual shared backing, including cancelled
bootstrap, and weak claim control blocks remain credited until pruning.
Reservation tickets bind the expected RepoSpec independently of spelling.
Only a closed command seal matching the full current native transcript and epoch
can adopt the same actual root; generic metadata-only success refuses General.
Cache source identity once at immutable realization production after admission;
never hash complete retained bodies under the actor mutex.

Snapshot current owners under lock, compare foreign full requests unlocked,
prevalidate/prepay the COMPLETE demand batch before any Host/legacy/physical IO,
then recheck the same relevant request/result/generation/native-owner association
after IO. Token/scope alone cannot authenticate a replaced entry. Lstat/readlink/dir/bytes/digest/image all use checked
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
The observer debits the unchanged64 SourceAttempts only for FileBytes/FileDigest
content captures, before any IO and including missing/wrong-kind failures. This
reviewed operation classification supersedes the initial charge for every rooted
metadata operation: genuine original action transport otherwise exhausted capture
admission during repeated full-certificate checks. Lstat/ReadLink/DirectoryEntries
retain the existing complete batch/path/result, live-bank, native-FD and original
deadline bounds; no certificate revalidation is skipped or cached. FileBytes
calls the private already-charged entry point and keeps its
existing 1 MiB object cap. Digest streams through RootIo's existing SHA kernel,
with fixed scratch and cumulative byte charges up to the selected RootIo file
bound; it does not allocate a FileBytes object or inherit that object's cap.
Temporary handle/scratch credits do not establish physical FD or kernel-memory
admission. Sealed/exclusive-quiescent ownership, exact root/anchor association,
registry validation and final source/transport integration remain separate gates.

PreparedSourceActionInputs/PreparedActionChainInputs stay resource-free and have
no ambient open/upload method. Explicit physical anchors propagate from Bzlmod
through ResolvedPath, SourceArtifactInput and anchored digest resolution; namespace
or path-prefix guessing never substitutes for an anchor. At native completion,
mint NativeSourceActionInputs/NativeActionChainInputs from immutable metadata and
closed selected-root leases after FULL current request/result and complete source
namespace/anchor/digest membership checks. A source in a selected immutable repo
cannot fall back to Host; local external repositories keep their Host policy.
Readers retain the sealed root/minimal membership rather than terminal history.

The workspace-shared reader work bank admits 16 MiB retained storage and 128 FDs;
its unused backing allowance grants no mount/backing authority. Action/CAS reads
do not debit acquisition SourceAttempts or decoder DecodedBytes and do not inherit
the 1 MiB FileBytes cap. NativeSourceReader exposes Read/Seek without File/AsFd or
raw-FD conversion. Consuming into_async moves a pre-armed reader/buffer/credit
owner into each blocking job, so dropping its adapter or network future cannot
release resources while IO continues. InputImage uses a fixed physical-root
bridge retaining the actual root and credited duplicate descriptor, validates
that descriptor, and bypasses ambient anchor opens. No source receives setup or
mount authority.

SourceActionTransport::stage/ActionChainTransport::start and REAPI from_native
plans own wrappers through CAS hits, upload, execute and final source validation.
Every pending RPC checks the ORIGINAL scope before/after and while pending;
expiration drops the network future without detaching blocking reader ownership.
CAS and durable SourceBacking independently verify exact length/SHA/EOF from
opaque reader bytes. Published runfiles never symlink an inaccessible display
path. On supported Linux the Host/legacy acquisition policy is preserved.
Non-Linux native source reading/upload is explicitly Unsupported; portable pure
metadata/query and zero-source native preparation remain available. This is an
explicit support boundary for the migrated transports, not cross-platform parity.

Cross-repository PROCESS read views use the inactive manifest/admission contract
above: one exclusive-command-owned userns mount domain and separately typed read-only
view tokens minted from actually attached roots before sealing. The complete
declared selected set validates before effects, including full spelling identity/
current generation; overlapping generations and unbridged domains refuse.
Original Loading must supply that complete declaration set before activation.
Source-data leases never gain these capabilities.

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
Registry/cross-repository read views now have inactive proof above. Original
Loading integration and production activation remain later gates.

| Owner | Files under app/ (new leaves explicitly named) |
| --- | --- |
| Bzlmod protocol | `slug_bzlmod_v2/src/repository_{execute,filesystem}.rs`; new `repository_workspace.rs`; exports. |
| Loading API/events | `slug_loading_v2/src/repository_rule_context.rs`, its `{filesystem,deletion,archive}.rs`; `module_extension_repository_file_effect.rs`. |
| Anchor/shared route | `slug_workspace_v2/src/path_resolution.rs`; Bzlmod `source_preparation/repository_source_observation.rs`; root integrates `source_preparation.rs`. |
| Core physical/snapshot | `slug_core_v2/src/runtime/repository_filesystem_progress.rs`; new `repository_{physical_root,workspace_snapshot,process_policy}.rs`; `repository_generated_archive.rs`, `repository_delete.rs`, `repository_filesystem_manifest.rs` and rooted generated-file application in `repository_io.rs`. |
| Core rooted observers | `runtime/path_observation.rs`, its `{file_digest,scoped_directory}.rs`; `source_service/regular_file.rs`; `input_image.rs`, `input_image/linux/fs.rs`. |
| Native reader/REAPI | Core `repository_io/source_generations.rs`, `{source,action_chain}_{staging,execution}.rs`, `runfiles_manifest.rs`, `action_chain_staging/runfiles.rs`; new `native_source_access.rs`; REAPI `{source_staging,action_chain}.rs`. |
| Root integration | Core `runtime/{repository_io,dice,mod}.rs`, Bzlmod `source_preparation.rs`, exports/acceptance/current epoch; atomic activation and scheduling. |

Pinned API/source anchors are retained in the linked source contract. Reviewed protocol,
anchored resolver and rooted observer candidates can be authored inactive before
complete syscall proof once root accepts design; no unconfined execution follows.
The [native source registry receipt](wp1225-native-source-registry-acceptance-2026-11-08.json)
records accepted closed adoption, selected leases, actual A/B anchored digest/
reader/image seams and protected Host/legacy NativeLink transport. Physical
SourceArtifact metadata in the native fixture still uses a resource-free test
constructor after actual DICE anchor/digest production; it grants no root or
transport authority and does not prove original Loading activation.
Original General outputs now have the inactive original-to-analysis, real
NativeLink CAS/runfiles and corruption/restoration proof linked above. The
separate digest-only SourceRoot snapshot now has its own bounded ordinary
Analysis/native proof above. Remaining activation gates include derived Source
paths, repeated Workspace publication, full Archive envelope/inode workload,
all demanded freshness/event surfaces beyond the admitted slices, authentic
public Rust and equivalent-command memory. The bounded unrelated-original and
compiler/Homebrew proofs do not establish the complete activation envelope.
Independent final review and owner/direct-dependent checks apply to each accepted
implementation chunk.
Root alone schedules Cargo/native proof, atomic integration, fresh CLI/cquery
and equivalent whole-command memory checks; internal probes close none of those.
