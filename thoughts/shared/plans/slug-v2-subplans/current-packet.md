# Current Slug V2 Work Packet

Packet: WP-7-180-m7a-daemon-metadata-metrics-memory-r1
Status: accepted; independent DESIGN/FINAL accepts memory repair and concrete frontier; M7A remains partial

## Outcome and evidence

At accepted baseline d337c2ce5 (production8541e15dc), eliminate the daemon's
pre-semantic whole-workspace payload snapshot used only for invalidated_files.
The ordinary actual-root cquery reached49.3GiB RSS high-water and9.5GiB swap
before a root resource stop at76.597s. Preserved resource/integrity/cleanup
evidence: target/wp879/acceptance.json and
/tmp/slug-wp879-20261001-r3/receipts. A metadata-only inventory finds target
contains298,339 regular files/544,837,921,435 logical bytes. Source trace:
server lib.rs FilesystemObservationAdapter::observe calls Core observe_workspace;
Core runtime/dice.rs collect_workspace_observations reads every regular file,
retains raw bytes and duplicates valid text. Only root bazel-out is excluded.
The server retains both metric snapshot and previous text values across requests.
This is a demonstrated reporting-allocation owner; no phase trace proves the
precise stopping instruction, and no DICE graph leak is asserted.

## Ownership and compatibility

Replace only the private server metric adapter with a cohesive workspace_metrics
module. Its recursive no-symlink traversal collects compact per-regular-file
metadata (length/modified time and Unix inode/device/change timestamp as
available), never opens/reads file contents. Preserve root canonicalization
failure, root bazel-out exclusion, create/delete/recreate and warm count behavior.
Directory/read/stat failures remain local to the advisory scanner; typed commands
still own and report their actual semantic errors. Keep hidden/target paths
in the metadata inventory; no arbitrary semantic source exclusion.

invalidated_files is explicitly a Slug-native advisory metadata-change count:
metadata-only touches may count and metadata-indistinguishable content edits
may be missed. It never supplies semantic bytes, cache validity or invalidation.
Content-backed DICE path observations/source certificates, Need fulfillment,
strict terminal selection and admitted exact source semantics stay unchanged.
Do not alter Core observe_workspace (legacy explicit snapshots), keys/equality,
identity, locks, action/REAPI or archive owners. No Bazel oracle is needed for
this Slug-native reporting change; existing daemon semantic regressions are
the authority. Full root configured completion/bootstrap remain unproved.

Memory is private service-retained nonsemantic reporting state, O(regular-file
count/path length), independent of payload size. Current scan/map is request
scratch, replaces previous only on successful root observation, and drops
old records after comparison. No global interner/cache or retained evaluator
values; use existing compact utility guidance where it serves the shape.
BTreeMap ordering is retained only if necessary; a private HashMap counts rather
than emits ordered output and may use existing workspace fast hash utilities.
The existing large server lib.rs gets import/module/callsite changes and removal
of its old adapter only; new scanner/tests live in cohesive leaf files. Estimate
under200 production/200 focused proof lines, a review trigger not a hard limit.

## Scope and validation

Allowlist: app/slug_server_v2/src/lib.rs, workspace_metrics.rs and its tests;
app/slug_server_v2/src/tests.rs one hook plus metric_semantics_tests.rs; Stage2
owner architecture paragraph and matching Stage9 disposition only if needed;
this manifest and canonical Current packet. No dependency/pin/lock/build-input
changes. Root owns scheduling/integration/serial Cargo; worker owns the scanner
and wrapper edits; root owns independent semantic regression/measurement.

Independent DESIGN before implementation, FINAL before acceptance. Compile
pinned nightly server lib tests --no-run with JSON, preparation60s; preflight
selected exact ordinary tests, then focused scanner lifecycle/binary/symlink/
root-error tests and daemon build/query/cquery/aquery restoration/error gates.
Regression uses a huge sparse nonsemantic binary: scanning retains metadata
only, no file reads, and genuine configured results/source edits remain correct.
A same-length content edit with restored source mtime must still refresh
semantic DICE. Unix ctime can distinguish such edits in advisory metadata, so
the private regression also observes the post-edit metadata before the genuine
command (without updating DICE), forcing invalidated_files=0 while configured
output changes. Repeat for A/B/A restoration; no new testing/semantic API.
Run complete server suite once as bounded integration if fits; attribute any
failures against accepted baseline rather than waive. Rebuild slug_cli_v2
before its use; compile direct dependent, no concurrent shared-target Cargo.
Format, archive/diff/plan checks.

Measure an isolated scanner probe on the actual checkout and sparse fixture
with wall/peak RSS including warm replacement while the previous metadata
map is retained, before/after metadata inventory and payload-size comparison.
A baseline full-payload run would reproduce dangerous host pressure and is
forbidden; reuse WP879 actual source/resource evidence plus a safe small control.
Acceptance: scanner real-checkout cold/warm peak RSS below1GiB, sparse-file scan unchanged
by payload size, no production file-content reads in scanner, focused semantic
and metrics gates pass. This is a declared memory-reduction criterion, not a
claim about full configured graph memory. Any later genuine root replay needs
independent review, no observer semantic changes, supervised180s cap and hard
owned-daemon RSS/swap stop well below host pressure. No identical pre-fix retry.

Stop/replan only for new semantic ownership decisions or baseline contradiction.
Correct routine invocation/test/compiler issues in this packet. Material changes
to metrics classification or validation require reviewer inspection.

## Immediate predecessor

WP879 accepted resource-stop evidence at d337c2ce5: query status130, actual
query returncode unobserved, empty output, unchanged3907 tracked/CLI hashes,
isolated group cleanup84.090s/no processes. R1/R2 interrupted gates remain
unproved. Source audit selected this reporting owner; user explicitly requests
fixing the untenable memory footprint. M7A/M8 remain open.

## Validation checkpoint and protected replay

Independent DESIGN accepts scanner and protected changed-binary replay. Root
serial pinned preparation preserved two60s dependency caps and a private-field
compiler correction; final server no-run passes4.172s and CLI build7.575s.
Five scanner gates pass0.004s. Six individually selected daemon gates pass
(advisory-zero configured A/B/A14.083s, warm build9.276s, aquery6.022s, cquery
recovery8.827s, request preflight0.008s, query content edits9.076s). Genuine
128/256GiB sparse binary cquery passes9.018s/~55MiB RSS. All exact ordinary
selectors were preflighted against unchanged test executable. Aggregate batches
at20s completed5/69 and1/7 or1/2 before operation timeout; no assertion failure
or broad green claim, and only their unproved required gates were partitioned.

Balanced64MiB control A/B/B/A: legacy peak276824/275984KiB versus metadata
13060/13052KiB;1TiB sparse metadata12936KiB. Actual checkout cold plus warm
replacement peaks256540KiB in3.568s, below1GiB criterion. Each ignored probe
was explicitly listed and executed1/1. Receipts: target/wp880/memory-measurements.json
and /tmp/slug-wp880-measure-ncis2ftf. This proves the metrics owner, not yet
full configured-root memory.

The reviewed next gate is one genuine rebuilt default-feature CLI root query
with unchanged ordinary argv/180s operation cap. Private supervisor
target/wp880/production_memory_frontier.py freezes staged new Rust inputs and
CLI/build evidence, samples only its exact owned process group every250ms and
stops above4GiB combined RSS+swap. This is a sampled safety threshold, not a
kernel maximum; between-sample overshoot is possible. Dedicated fresh
/tmp/slug-wp880-20261001/output-base, status saved before isolated cleanup,
source/CLI postcomparison and group disappearance are required. No workspace
writer/Cargo runs during this gate. A concrete terminal or resource attribution
is recorded without widening M7A/bootstrap acceptance.

## Actual-root memory verification

The rebuilt genuine production-root cquery returns an actual exit2 terminal
at10.507s; cleanup finishes11.020s, dedicated shutdown sent, owned group gone
and no remaining Slug processes. All3910 tracked source/input hashes (including
staged new Rust) and CLI bytes match.43 resource samples record zero swap,
maximum daemon VmHWM308796KiB (301.56MiB) and maximum sampled group
RSS315379712bytes (300.77MiB); the4GiB sampled stop never triggers. This
verifies the footprint repair through this concrete production frontier.

The source-owned cquery_runtime_error is the @slug_crates load route
@@rules_rust++crate+slug_crates refusing module_ctx.path for generated
repository Labels. Stdout is empty, stderr is2179009bytes of existing nested
route diagnostics. No configured result, deeper graph memory bound or bootstrap
claim follows. Preserve target/wp880/acceptance.json and runtime receipts at
/tmp/slug-wp880-20261001/receipts. The selected successor identifies that
generated-Label demand and its natural materialization owner; diagnostic
formatting is separate debt, not a reason to widen this footprint repair.

Independent FINAL accepts the actual diff and all12 distinct focused gates,
scanner memory measurements, rebuilt direct CLI dependent and genuine root
resource/terminal evidence. Format, source/diff, archive and plan checks pass.
Accept only this private Slug-native advisory reporting repair and generated-
Label frontier evidence. The complete server suite remains unproved after its
aggregate operation cap; individual required gates passed. Next select a bounded
source-grounded generated repository Label demand/owner audit, with no root
query repetition or semantic admission from this packet.
