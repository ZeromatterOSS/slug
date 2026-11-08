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
identify the exact rejected root. Preserve that refusal and leave repeated
authentic commands within the shared backing budget open. No quota increase,
Source contraction or eviction is introduced; neither this proof nor the
refusal establishes equivalent-command RSS fit. Source authority, complete
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
Source snapshots retain their existing quota. Process finalization is unchanged.
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
