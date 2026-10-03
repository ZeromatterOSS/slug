# WP899 first buffer-limit provenance

DESIGN checkpoint, baseline4d913035a; fullWP899 remains unaccepted.
Compatibility: Slug-native bounded Linux test diagnostic, no Bazel exact claim.
Inherit [post-transfer frontier](wp899-post-transfer-buffer-frontier-2026-10-03.json),
[timeout recorder](wp899-timeout-provenance-checkpoint-2026-10-03.json),
[registry ownership](wp899-registry-association-checkpoint-2026-10-03.json), current
packet, orchestration and Buck2 utility reuse skills. No actual replay selected.

Observable result: one optional fixed POD distinguishes first BufferLimit creation
from later sticky propagation and identifies the creating bank reservation or
explicit bound. Global10 proves firstBufferLimit/outerDownloadCapture,192distinct/
16source attempts, peak15221766B/live4211111B, but exact producer is unknown.
Selected download boundary retains String errors, not an exact chosen native cause.
Current peak cannot justify a repair or limit increase.

## Producer and representation

New source_service/buffer_provenance.rs defines finite SourceBufferSite tags:
Unattributed; RegistryCapture/RegistryArc/RegistryAddress/RegistryFileRead;
HttpParser/HttpDuplexParser; ArchiveAddress; SelectedPayload/SelectedScratch;
PatchCapture/PatchTarget; SharedIoBuffer; AssociationRecorder; and explicit bounds
RegistryUrlLength/HttpUrlLength/ArchiveUrlLength/RedirectLength/DnsCardinality/
SelectedPayloadGrowth. Unit is unknown/bytes/entries. Origin kind is unknown,
reservation-ceiling, reservation-overflow, or explicit-bound. One POD retains site,
kind, requested/bound optional usize, live_bytes and bank_ceiling_bytes at creation,
and unit. No location/PID/path/URL/payload/string/exception/trace storage. Unknown
legacy producer has no requested/bound/unit inference and cannot prove a named site.

SourceServiceScope owns cfg(all(test,target_os="linux")) BufferState under its
original state mutex, adjacent to timeout state. reserve_buffer keeps its existing
API and delegates with Unattributed to reserve_buffer_at(bytes, site); named natural
call sites use the latter. Inside the original failed addition/ceiling branch,
record exact bytes/live/ceiling/overflow before returning the same error and
cancellation. Success/partial refusal/counter/highwater behavior stays identical.

poison_buffer_limit_at(site, requested, bound, unit) reuses the existing first-poison
owner/precedence/cancellation. Its same locked first-creation transaction writes
the tagged POD once after poison_locked and before releasing the lock. A prior
poison never gains or changes buffer origin. Generic poison(BufferLimit) separately records an unattributed
origin, with missing bound facts. No tag inferred from error strings or paths.

Named explicit call sites pass only already-computed facts from their existing
branch: actual URL/redirect byte length versus8192; observed65th DNS address versus64
(entries); selected initialized-prefix growth versus original capture size (bytes).
Existing capture arithmetic-overflow/string errors remain unchanged; do not relabel
them as Scope BufferLimit. Unreachable old Some(scope) branch in unscoped transfer
stays byte-identical and unattributed. Shared IO helper is a named reservation owner,
not its caller's semantic leaf. No claim that first creator is the selected leaf.

Arm only with successful exact registry-recorder attachment; clear on exact installed
Weak-pointer lastDrop alongside timeout state, before credit release. Competing failed
arm cannot clear a winner. No extra Arc/lock/cache/observer allocation or await.
Recorder adds BufferSnapshot to its existing scalar snapshot and charges enlarged
Self construction/retention headers plus Scope BufferState diagnostic retention
before recorder allocation/arming, in the same16MiB bank. The inline POD is physically
part of the earlier Scope allocation, as existing timeout state is; the bank does
not claim total Scope/heap memory when unarmed or after clearing. Current attachment and Scope/recorder ownership are unchanged.
One Scope-locked tuple snapshots counters/timeout/buffer consistently; no lock is
held across async/DICE work. Existing descriptor/engine/clone/lastDrop semantics stay.

No donor import: reuse the accepted fixed POD/Weak recorder/charged-header pattern.
This is nonsemantic service diagnostic storage; default/non-Linux production retains
no BufferState. Public tags/narrow methods exist for call-site compilation, but no
source authority/key/value/certificate/cache/message/None Debug changes. No Windows
work selected. Metric is reservation credit, never heap/RSS or all retained downloads.

## Proof and validation

Actual harness emits only numeric finite fields after existing snapshot/raw restore,
worker drain and actual engine retirement. buffer_creator_proved requires settled,
current same installed scope, armed snapshot, first_error_is_buffer_limit, matching
actual first Scope poison, named site, complete structurally valid kind/unit facts.
Whitelist reservation sites1–13 versus explicit sites20–25; a site/kind mismatch
proves nothing. Reservation facts require bytes unit, bound==bank_ceiling, creation
live<=bank_ceiling, and checked-add result exactly overflowing or exceeding it.
Explicit facts also require live<=bank_ceiling; URL/redirect sites use bytes with
bound exactly8192; DNS site uses entries with bound64; growth uses bytes and its
captured-size bound, with both observed prefix and size<=the preceding32MiB guard.
Every explicit observed value must exceed its bound. Unknown/unit/gapped or
structurally inconsistent metadata proves nothing. Emit raw fields
and presence flags even when proof absent. selected_leaf_call_identity_proved stays0.
Separate selected-native pairing requires exact Scope BufferLimit chosen native cause;
outerDownloadCapture or generic strings alone cannot certify this pair. No success/
source-fit/repair/cap change or historicalglobal8/global9/global10 attribution.

Allowlist: new buffer_provenance.rs and buffer_provenance/tests.rs; source_service.rs,
source_service/{scope,registry,http,registry_association}.rs; repository_io/source_service.rs;
repository_archive_{realize,patch}.rs; repository_archive_http/scoped.rs;
source_preparation/genuine_root.rs narrow emitter/call/receipt tests; source_service/
registry_association/tests.rs only if fixed header-size expectation must be updated;
source_service/timeout_provenance/tests.rs only for the added exact BufferState
charge and the extended observation tuple, preserving all protected assertions;
this contract/receipt, Stage9 existing source-service row and scheduling status.
Do not refactor adjacent large owners. Scope/helper remain cohesive; genuine_root
already>2000 lines is a test-only harness: add only one finite emitter and focused
receipt case. Estimate~300–420 production/~250–400tests; review triggers are estimates.

New discriminators cover exact reservation boundary/overflow/success+sticky, explicit
bound/unknown producer/unit/site-kind/fixed-bound/growth-guard mismatch proof, unrelated-first failure, attachment failure/
competing loser/lastDrop/same-bank header charge; concurrent reservation-versus-explicit
creation selects one indivisible first origin; normal file selected-download refusal
proves named payload site/requested/live/ceiling; existing protected transfer tests
retain capture-deletion proof; real overlength
archive URL and DNS-cardinality branches; numeric receipt settled/current/first/error/
chosen-native gating and selected-leaf unknown. Reuse55 transfer/registry/timeout
controls on fresh Core artifact (avoid unchanged15s TLS execution unless affected).
Default Core+CLI all-target check; focused source_service/transfer/genuine receipt
selectors share one exact preflight/artifact; root serializes fixtures. New helper/
receipt tests are colocated Rust regressions, not Bazel oracle assets.

Root owns all edits/serial Cargo/tests and commits. Resolve pinned nightly2025-09-14
cargo/rustc/rustdoc/rustfmt; prepare/no-run separate under60s TERM/KILL3; execute30s
TERM/KILL3. Resource/time/capture/source/supervisor caps unchanged. Reuse accepted
unchanged heavy tests; defaultcheck proves non-test tags/call-site compilation.
Independent DESIGN before source edits, FINAL after executed evidence. REPLAN if
provenance requires new semantic/identity/lifecycle ownership, extra dynamic storage
or altered error/limit behavior. Any subsequent actual observation needs a separately
reviewed contract/freeze after acceptance; current global1–10 evidence stays immutable.
