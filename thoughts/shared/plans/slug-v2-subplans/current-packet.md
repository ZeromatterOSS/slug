# WP1089 shared path-epoch coalescing

Packet: WP-7-301-m7a-shared-path-epoch-coalescing-r1
Status: accepted — coalescing complete; normal producer time fit open
Baseline: 3bf27920cd1a82308d87b36a4a07d743b4369ab5 (WP1088 accepted)
Owner: Workspace shared path-epoch constructor; normal M7A producer path-progress.
Compatibility: Slug-native temporary allocation implementation; all existing epoch
semantic identity, equality/error/observation ownership and admitted Bazel surfaces unchanged.
Windows: unchanged existing portable constructor; no platform implementation work.

Outcome: remove the independently visible second full-capacity coalescing Vec in
PathObservationEpoch::from_shared. Normal runtime/dice.rs7940–7960 rebuilds merged
path epochs after newly observed batches. At baseline, constructor824–855 collects and
stably sorts one Vec, then allocates a second full-sized unique Vec. This is a
concrete temporary-allocation opportunity, not identified deadline cause or claimed
runtime benefit. Prior native207.211MiB/256 hit GenericScope/Common/ScopeCheck deadline
at source243, before previous245; fullproducer/time/wholeworkload fit stays open.
Authority: WP1088 accepted receipt and target/wp1088/next-work-frontier.md
SHA ddd1bd05dd416b1a6a3b285aa79b51683370da731a4b93e624df9e6d2ceee2df;
actual current Workspace constructor, normal producer callsite, retained DICE docs.
Reuse existing SortedMap, Arc/Dupe, typed demand/results and Allocative. No donor
import, new interner/cache/map/key/public carrier, global allocator, pool or lock.

Decision: collect/stable-sort the same owned input vector, validate and compact
unique rows in place with safe swaps and a retained-prefix length, then truncate
and transfer that vector into the existing SortedMap collection/final Arc. A small
private helper may expose the same owned-vector phase to focused cfg(test) proof;
public signature/final representation stay unchanged. No unsafe code/dummy result,
body clone or second full-capacity Vec. Existing stable sort scratch and final map/
Arc allocations remain real costs; this is not zero-allocation construction.

Material invariants: stable demand order and first equal duplicate's ORIGINAL Arc
must survive (including equal values with distinct Arc allocations). Conflicting
same-demand values remain an error. For each sorted input row, preserve existing
InputImage issued-path check before operation mismatch check before duplicate
conflict check; earliest sorted-row error wins with exact demand/kind. Preserve
valid-input/certificate/equality/namespace/instance/key-specific DICE boundaries;
no current-path trust, reverse-epoch shortcut or changed dependency/Need semantics.
No observer/body/IO/source-admission/runtime resource-owner change. All moves and
Dupe metadata preserve independent retention; existing scoped string interners
remain untouched because these rows already share typed paths/result Arcs.

Exact source allowlist: app/slug_workspace_v2/src/path_observation.rs — only
from_shared, one cohesive private compaction helper if needed and focused existing
module tests. File2289lines exceeds complexity trigger: existing constructor/tests
remain cohesive; no additional subsystem or unrelated extraction. Expected20–60
production plus60–120proof lines, helpers below150; review actual growth. Root may
update matching Stage9 utility facts, receipt and scheduling only.

Proofs: reuse shared_epoch_coalesces_equal_arcs_and_rejects_conflicts and
partitioned_epoch_add_change_remove_and_restore_preserve_exact_path_values.
Add discriminators for safe in-place source Vec pointer/capacity retention on a
bounded duplicate-heavy cohort (8192rows metadata sharing, no bodies) and preserved
first duplicate Arc/order; mixed invalid-image/operation/conflict cases retain
exact earliest-row/per-row error precedence. Add only missing cases. Do not use a
new allocator/framework, fixture tree or runtime fields. Measure row size/count and
removed temporary capacity arithmetically; inline/heap/workload claims separate.

Gates: independent DESIGN before source; root actual diff/format/freeze, independent
actual-source/prelaunch review. Fresh pinned nightly; serial locked/offline/jobs1
Cargo JSON Workspace no-run/current exe; list and exact batched preflight ordinary
path_observation tests, actual nonzero selected/pass count. Direct named production
Bzlmod/Loading/Core/CLI check because public shared constructor is used across those
owners. Original-profile Core native-feature no-run/current exe+ignored singleton
list; reuse completed exact dependency artifacts to isolate preparation phases,
never count timed-out overall gates as passing. Focused test-only optimization0
per admitted recovery; native/direct Loadingopt1 and other accepted profiles stay.
ONE new candidate native with same350 source triples/18env and all original caps;
code changes make this a distinct measured candidate, not an identical retry.
No writers during gates, stream blob hashes64KiB/Cargo JSON linewise, preserve
actual memory/time/resource refusal and all held-FD/current pin brackets.
FINAL reviews actual source+discriminating evidence+small native delta. No broad
history/lifecycle replay; unchanged contracts0fresh credit. Format/diff/archive/plan
status required. Root owns integration and commit; full goal remains ACTIVE.

Caps unchanged: prep60s/RSSuncapped/no workloadcredit; native10s/RSS256MiB/AS512MiB;
live+transfer16MiB/decode+write64MiB/source256/registry128/object1MiB/admitted16MiB;
directory128entries/name1KiB/total16KiB; selectedwork7/default5/lifetime8/cleanup3/
FSIZE16MiB/log1/report4MiB. User12s guideline, no cap rise or duplicate native. Replan
only unmodeled ownership/semantic/error-order prerequisite or envelope requirement.
Routine compiler/fixture corrections preserve failed evidence and repair same owner.
Acceptance must report whether natural producer frontier advances; passing refusal
observer or Vec proof never establishes wholeworkload/bootstrap/retirement/time fit.

DESIGN accepted: target/wp1089/design-review.md SHA256
7b9145f0bc2ef58e9cf72758ff8a9c5f49e273321306a2578ea8cb672522c08f.
Frozen design.md b556a31425c8ed6c7e3138d7d582f127d1066bc6880ecf43f2313742915cadcd;
review accepted source implementation within this contract; DESIGN itself gives no gate credit.

Acceptance: independent FINAL target/wp1089/final-review.md SHA256
5e471f4f4c6a289b89ef7d92ef02f09cc73b21dbedf22f248f61817523ed1ad5.
22focused/currentdirect/currentCoreoriginalprofile gates pass.8192metadata-row
compaction retains originalbuffer and removes second1.25MiBcapacity only. ONE
native7.269355s/207.918MiB within256 hitsgeneric WorkDeadline/source246vs243
admissions, not bodycompletion/speed/cause/wholefit. Caps unchanged; no retry.
[Receipt](wp1089-shared-path-epoch-coalescing-2026-10-10.json). Full goal ACTIVE;
next normalproducer deadlinecaller/stage attribution research, no speculativepool.
