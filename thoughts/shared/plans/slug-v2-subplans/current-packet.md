# WP1097 normal external-child allocation deferral

Packet: WP-7-309-m7a-external-child-copy-deferral-r1
Status: accepted — FINAL reviews and all selected gates pass; next readiness selection underway
Baseline: eb7c736a7d8ae6fa00d0352d4fa98c88fd314c22 (WP1096 accepted)

Packet WP-7-309-m7a-external-child-copy-deferral-r1. Baseline
eb7c736a7d8ae6fa00d0352d4fa98c88fd314c22 (WP1096 accepted). DESIGN required
before source activation. Source research target/wp1096/next-work-frontier.md,
SHA6da92b45b9b5ea0c2c260a1079882e1aafec32ea67cbdd4d45cc21160ee9eedf.
Outcome: normal producer retries keep original completed child result handles
until all siblings and final parent parse settle, avoiding unusable owned path
and load-string copies. Slug-native storage change; existing admitted exact
semantic identities, output, order, errors, observations and DICE remain exact.
Linux priority; Windows deferred.

Source-established seam: bzl_module.rs562–575 FrozenBzlModule derives Clone and
owns PathBuf/Vec<String>. ExternalBzlModuleCarrier4491 is already an immutable
Arc<Result<FrozenBzlModule,ExternalBzlModuleError>>. Private
compute_external_bzl_children5233/5392 clones every completed child into a scratch
prefix before later sibling Need/error/cycle/observation conflicts may discard it.
The owned evaluator5463 and settled conversion5642 remain the existing independent
ownership boundary. Current native prefix fits supervisor226.69/phase227.55MiB
under256 but stops at WorkDeadline caller7:296/source252. Detector timing does not
identify the expensive key, next source, frequency or cause of the copies.

Decision: return private Vec<(&str,ExternalBzlModuleCarrier)> from the child loop.
In the existing success match arm move the existing immutable Arc into the
temporary prefix; preserve the original error arm, full observation merging,
route/context/mode, source-before-syntax, cycle/detector/dependency calls and
first-failure/Need order. After all children settle and final parent parse succeeds,
the existing conversion owns the raw name and clones the successful Frozen value
once, immediately before the original evaluator. A local documented expect is
permitted only for the private invariant that each pushed carrier was checked Ok;
no untrusted error can become a panic, swallowed error or silently removed row.
No whole-parent shortcut, child reordering, bypassed computation or evaluator on
partial attempts. Success still owns path/loads and its original lifetime closure;
this is deferred cloning, not zero-copy successful evaluation.

The temporary Vec replaces existing owned-module scratch. It owns ordinary Arc
refs until Need/error/cancellation/drop or settled conversion; no retained field,
AST/source-body/cache/interner/pool/key/lock/API/dependency/profile/cap change.
Holding the result wrapper through an attempt is bounded by the existing load
count; full DICE graph/module equality/invalidation/events/lifetime semantics stay
unchanged. Borrow/move/share strings and use existing scoped interners only where
their retention/cleanup fit; no unrelated representation migration. Stage9 existing
Arc/Dupe/FrozenModule closure/Allocative and no-op reuse are concept anchors, no
donor import. docs/developers/dice.md remains graph ownership authority.

Two-file allowlist: app/slug_loading_v2/src/bzl_module.rs (private return type,
success push and settled conversion only) and
app/slug_loading_v2/src/host_package_load_tests/external_bzl_frontier_tests.rs
(one small proof). Estimate10–25 runtime and60–120 proof lines; large existing
file remains cohesive for this seam, no extraction or fixture tree. New basename
external_bzl_child_prefix_retains_original_carriers_until_settlement. Reuse current
in-memory frontier transactions and graph-produced successful child values.
The proof must observe the actual normal parent computation, not manually mirror
its temporary Vec. Precompute the completed left child in the same transaction,
inspect original nonempty path/load buffers and retain only its Weak; drop temporary
strong results before recording baseline. A tiny fixture-only ActivationTracker
wrapper delegates existing FrontierTracker callbacks and samples Weak::strong_count
without upgrade when the exact later missing sibling FileBytes demand activates.
Use distinct never-computed normal parent labels for Need/error/success, retaining
the same warmed left carrier across epochs. This excludes historical parent
dependency validation before ordered child loading; every exact later callback
must see baseline+1, with nonzero samples and baseline after parent discard.
The normal success evaluator/strict closure retains independent final owned values.
Existing protected same-parent repair remains its own authority; this new proof
covers outcome/ownership, not same-parent repair. No direct
private-driver call, cycle guard bypass, new test key or manually mirrored slot.
Reuse existing early semantic-error/Need, synopsis/current epoch and finisher
controls. Any Weak final-drop claim must release actual graph/original owners;
graph-retained lifetime is explicitly qualified. Existing early poll-drop control
remains reuse and is not relabeled populated-prefix cancellation. The bounded
Weak/count fixture probe is test-local instrumentation only; production capture
fields/counters/keys/instrumentation remain excluded. Source placement and actual
carrier ownership establish avoided premature copies, not allocator totals,
CPU cost, native frequency or a benchmark.

Gates: stopped source/helper/fixture writers; pinned format/30-owner freeze;
independent DESIGN/source/prelaunch/FINAL review. Serial pinned
nightly2025-09-14 offline/locked/jobs1 CargoJSON no-run; current unique artifact
lists/exact preflights/actual nonzero bodies. Loading6 = new proof plus current5
WP1096 sibling/semantic-order/synopsis/finisher controls. Bzlmod7 exact WP1096
controls and Core13 exact WP1096 protected/generated-file controls remain fresh
for a26-test total; production Bzlmod/Loading/Core/CLI check. Ordinary Bzlmod and
Loading opt0 uses accepted test-only recovery, native original Loadingopt1 remains.
Prep60s separate uncapped RSS, preserve capped/failed records and split completed
dependency stages as needed. No preparation peak can become native RSS evidence.

Fresh standalone ignored singleton list and ONE changed-source native under all
WP1096 caps: operation10/work7/default5/lifetime8/cleanup3/RSS256MiB/AS512MiB,
original350 ordered source triples/18 bindings/held noninherited FD brackets,
current three artifacts/source30/callers/observer/gates/reviews and v2raw64/status.
Root reviews helpers before use; supervisor/decoder ROOT-only accepted copies.
No identical native replay or cap raise. Separate supervisor and phase prefix
RSS, exact frontier and qualified windows. Actual borrowed buffer layout/counts
are finite fixtures; no guaranteed speed, native savings, full producer/time/fit,
JDK/namespace/certificates/actions/Rustc/bootstrap or fullgraph retirement claim.

REPLAN only if deferral requires a new public/retained representation, cannot
preserve full identities/error/observation order or cannot safely retain/drop the
original carrier. Routine fixture/type/invocation corrections stay in contract
and preserve first evidence. Format/archive/diff/plan checks, compact receipt,
Stage9 ownership note and accepted commit. Goal ACTIVE; this normal producer
string-copy seam advances allocation discipline while finite authentic-root/full
producer closure remains open. A narrower root is a separate explicitly scoped
readiness claim, not current full-closure acceptance.

Immediate predecessor: [WP1096 receipt](wp1096-no-op-request-map-union-2026-10-10.json), accepted in eb7c736a7.25fresh focused/production/native-profile/FINAL gates pass; native supervisor226.69/phase227.55MiB under256, same source252/WorkDeadline. No full-workload or speed claim.

Acceptance: [WP1097 receipt](wp1097-external-child-copy-deferral-2026-10-10.json).26fresh focused passes, production/current native-profile/FINAL ACCEPT. Native supervisor225.63/actual phase226.84MiB under256; source252/WorkDeadline beforebatch persists, no speed/savings/frontier/fullfit claim. Both failedproof generations preserved; source-grounded coldparent correction retains strictcounts. Source30 owners frozen, runtime18+/4-/proof232+, no new persistent representation. Root report registrybytecounter/RSS error corrected from exact phase tags before acceptance, no replay. Next normal root-package producer/readiness seam under source review; goal ACTIVE.
