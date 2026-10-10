# WP1098 normal root BUILD syntax reuse

Packet: WP-7-310-m7a-root-package-syntax-reuse-r1
Status: accepted — 33 focused, production, native and FINAL review gates passed
Baseline: 3b52ddec231da3723a68f252148e80c6587e483b (WP1097 accepted)

Outcome: the existing normal requested root-package producer reuses compact BUILD
syntax/load metadata across child-only Need revisions, preserving current source
and every package/evaluator semantic boundary. Slug-native storage/recomputation;
existing admitted exact source/order/errors/events/identity remain exact. Linux
priority; Windows deferred. Full producer completion remains the broader goal.
Research target/wp1097/next-work-frontier.md SHA 6d22e75b1f4d1481764cde077369b67135162ca14d115fcb1f320a9d1ca6380a. Actual existing
//pkg:probe → SingletonRootSingleBuildCommandKey → RootPackageLoadObservationKey
is source-grounded; no requested root/dependency is removed to obtain a pass.

Source facts: bzl_module.rs6234 copies BUILD text to Arc<String>,6256 clones it
into AST, extracts owned load names, then awaits children. Need discards this
preparation. AST is loads-only and dropped6361; existing package-attempt evaluator
753 already reparses its source. A compact private syntax owner replaces that
pre-child AST; successful parse count stays two (synopsis + original evaluator),
not an added final preparse. Observed native225.63/phase226.84MiB fits256 but still
source252/WorkDeadline at caller7:296. That detector does not identify costly work,
key frequency or a cause. No speed, allocation total or full-fit claim.

Decision: private RootPackageSyntaxKey owns the full existing RootPackageLoadKey
(workspace/package) and private mode. Track the original RootPackageSourceKey or
ObservationKey::for_build directly. Parent still obtains anchor, current source,
left-first observation merge and forced observed outer hook BEFORE synopsis.
All source/outer failures retain original precedence; Need/nonpure failures are
invalid and unequal. Syntax computes no children/route/evaluation/IO bypass.
Private mode gets structural Eq/Hash/Allocative support, with no public field/API.

Value owns original shared bytes, exact logical/source filename/BUILD identity and
ordered Arc slice of moved load Strings or original pure encoding/parse error.
Retain only input identity needed for syntax; derive the exact host_source_name
and Bazel/BazelInternal parser behavior. A source-name String need not survive
parsing when original immutable logical path already owns the exact identity.
Pointer fast path followed by full bytes/path/name/encoding/result equality;
no digest-only/key-only/loads-only equality or ignored same-load body changes.
SourceBytes already has Arc ownership; this value adds DICE-retained semantic
metadata/history retention and must not borrow request/evaluator scratch.
No AST/CodeMap/sourceString/global cache/interner/lock/counter is retained.

Parent borrows load names during original declaration-order route/child awaits;
independently retained evaluator name/module values own their original copies.
Delay original source_text Arc<String> until children AND runfiles mapping settle,
immediately before existing HostPackageAttemptInput. Reuse source UTF8/name/error
precedence from the pure syntax result. Leave attempt/glob/policy/evaluator/event
publication and package source certificate closures unchanged. Existing graph
invalidation/cutoff/cancellation/shutdown remain ownership authority. Failed key
never becomes a reusable syntax success. No whole-parent cache or fresh-graph
shortcut; no unrelated HostBzl/externalBzl/repository BUILD migration.

Allowlist: app/slug_loading_v2/src/bzl_module.rs (private mode/module/root-driver
seam), new app/slug_loading_v2/src/bzl_module/root_package_syntax.rs (cohesive pure
owner), app/slug_loading_v2/src/host_package_load_tests.rs (module registration
only), new app/slug_loading_v2/src/host_package_load_tests/root_package_syntax_tests.rs
(two finite proofs). Large parent/test files remain wiring-only for this change;
new owner/proofs are split, not expanded into a broad existing file. Estimate
100–200 runtime/150–300 proof lines; actual growth triggers cohesive review.
Reuse external_bzl_syntax.rs concept at22fd34ebc, Stage9 existing Arc/Dupe/
Allocative/compact-synopsis rows; no donor import. docs/developers/dice.md owns
key/dependency/lifetime decisions; Stage4 root-package exact contract4052–4099
owns anchor→source→orderedchildren→evaluation semantics. Existing accepted Bazel9.2
BUILD loading controls remain oracle authority; no new semantic parity surface.

Two finite discriminators: actual normal observed root syntax activation/Weak
proof across child-only Need and unrelated epochs, same-parent repair/current
observations/Complete events, full same-load body A/B/A and source absence before
syntax; exact pure input identity/encoding/parse errors with pointer-distinct equal
bytes versus changed bytes/logical BUILD filename, plus real graph/held-byte
last-drop retirement. Use existing capture/ActivationTracker test seam exposing
only Weak/scalars; no new production instrumentation/key/helper or manual mirror.
Do not infer parser invocation count from parent activations. Prove actual syntax
key evaluations/reuse with meaningful nonzero current witnesses. Source-missing
Must not hit historical syntax. Cached deterministic encoding/parse errors keep
original diagnostics and precede child probing. Full-input equality preserves
comment/body edits even if loads equal. No broad parser matrix or fixture tree.

Reuse fresh focused controls: existing WP1097 Loading6 plus root
observed_root_package_preserves_semantics_arcs_order_events_and_families,
observed_root_package_reuses_restores_and_preserves_terminal_polarity,
observed_root_package_cancellation_publishes_no_parent_and_recovers,
host_package_need_is_transient_and_root_anchor_precedes_source,
host_package_loads_bzl_and_owns_only_local_complete_events; two new proofs make
Loading13. Bzlmod7/Core13 unchanged controls:33 exact nonzero ordinary passes.
Final selectors must be actual executable-list unique, batched preflight/body;
new selector spellings fixed by design/source handoff before gates. Actual
syntax/key/Arc/load layout and retained bytes/storage recorded, not heap totals.
Production Bzlmod/Loading/Core/CLI check and current native-feature compile.

Gates inherit WP1097 receipt caps/profiles/original350 ordered source triples/
18bindings/held noninherited FD brackets. Pinned nightly2025-09-14 offline/locked/
jobs1 CargoJSON no-run serial, prep60s separate uncappedRSS; ordinary Loadingopt0
only test lane/native originalLoadingopt1/Coreopt0. Add the two actual new owner
paths to current32 source freeze/helperpins; preserve original350vector unchanged.
Stopped source/helper/fixture writers before fresh standalone list and ONE
changed-source native10s/work7/default5/lifetime8/cleanup3/RSS256MiB/AS512MiB.
Remaining byte/source/directory/log caps unchanged. Independent DESIGN/source/
prelaunch/FINAL review, format/archive/diff/plan, compact receipt and Stage9 note.
No identical native replay/cap raise. All native metrics use exact grammar;
WP938_SCOPE registry bytes are separate from actual WP938_PHASE RSS tags.

REPLAN only if exact source-before-syntax/semantic/error/observation order cannot
be retained, compact value needs AST/global ownership/new public source API, or
actual reuse requires bypassing original dependencies. Routine invocation/type/
fixture corrections stay within this owner; second correction reassessed. New
persistent bytes/history retention and cold/warm tradeoffs reviewed before source
activation; full-workload fit/cancellation/native retirement/frequency/CPU/speed
are qualified unless directly established. Goal ACTIVE. Borrow/move/share strings;
use existing scoped interners when repetition and cleanup justify retention.

Immediate predecessor: [WP1097 receipt](wp1097-external-child-copy-deferral-2026-10-10.json)
accepted at3b52ddec231da3723a68f252148e80c6587e483b.26fresh/production/current-native/FINAL gates pass;
shared child scratch24B versus344B; actual native prefix225.63/226.84MiB under256,
source252/WorkDeadline persists. Both failedproof generations preserved0credit.

Acceptance: [WP1098 receipt](wp1098-root-package-syntax-reuse-2026-10-10.json). Four source owners; 28 inherited owners unchanged. Actual syntax832B/key40B plus shared storage/history; both normal-root reuse/source identity and graph/last-body retirement proofs pass. Corrected Need activation/publication assertion preserves strict nonzero/allNone and fresh Complete batch. First12pass1fail aggregate and Core60s capped49nullafter maps preserved0credit; actual Core-final43.54s strict successful preparation supplies current authority. Native ONE7.8855s/125stablepins/350stableheldFD/original18env/version2raw64status0; supervisor226.4375/actual five-phase227.52734375MiB under256. Still source252/WorkDeadline caller7:296; write+1004/rootwindows+2 are not cause, speed or frontier advancement. Full producer/time/wholefit/certificates/Rustc/actions/bootstrap open. Next normal producer candidate research target/wp1098/next-work-frontier.md (47681e27b3a8cb4c04e1ecb11d30ba9b3cbb45428a2424a8d1c21d743d9bfb35), conditional on concrete readiness priority; no new packet activated. Goal ACTIVE.
