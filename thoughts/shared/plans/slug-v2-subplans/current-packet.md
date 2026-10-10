# Slug V2 current work packet

Packet: WP-7-292-m7a-normal-rustc-immutable-spec-readiness-r1
Status: accepted — actual FINAL and operative receipt complete; successor follows local commit
Baseline: 64756bdd33e90dc2050e25a51128a1ff04ea3d0a (accepted WP1078).
Owner: M7A bootstrap-readiness.md, original immutable repository materialization request.
Compatibility: Slug-native, test-only Linux diagnostic projection. Windows deferred.

Outcome: expose the original typed RepoSpec of the first denied immutable request
as a bounded borrowed diagnostic, so the next immutable admission decision has
actual mapping and attribute evidence. The current native request is protobuf+,
@@bazel_tools//tools/build_defs/repo:http.bzl, http_archive, immutable_kind.
Its complete captured identity does not establish its attribute shape, source
availability, archive plan, transport permission, materialization, or output.
Keep the immutable refusal in place. No download, filesystem acquisition,
repository override, source append, or forced native success belongs to this packet.

WP1078 passed104 named tests; its one native probe took6.645985171s and used
210096KiB against262144KiB. The source table adds16 reference bytes and no new
literal storage or owned production strings. Representative current-prefix
memory fits. Full Rustc/actions/certificates/producer and whole-workload fit
remain open. Preserve source5/metadata6/environment2/union19 and the312 ordered
source leases without changes. Root authority and exact source snapshots are in
target/wp1079/baseline.json; accepted historical evidence is reused, not reexecuted.

Natural ownership: RepositoryMaterializationRequest retains the original
RepoSpec and attributes Arc. Its immutable classification is in
app/slug_bzlmod_v2/src/source_preparation.rs:5295–5310; exact-request/current
workspace result-epoch checks remain at5538–5588. Core repository_io.rs:2188–2197
checks kind and2279–2332 dispatches archive planning. repository_archive.rs:85
owns supported attribute validation; a diagnostic must not call that parser,
clone its owned archive plan, or replace it. module_eval.rs:797/911 owns RepoSpec
and the seven OverrideAttributeValue variants. Nested dictionary iteration
order remains observable; top-level attribute membership stays named.

Implementation allowlist: app/slug_core_v2/src/runtime/materialization_diagnostic.rs;
app/slug_core_v2/src/runtime/tests/platforms_module_preparation_tests.rs;
app/slug_core_v2/src/runtime/tests/platforms_module_preparation_tests/public_probe_preparation_tests.rs.
Root additionally owns target/wp1079 scaffolding, these scheduling documents,
and the compact tracked acceptance receipt. Other folders remain references.
The existing small test-only diagnostic module is the cohesive owner; do not
grow a new central framework or copy large test matrices. Parent and child stay
existing admission/emission and regression owners. Estimated Rust growth100–220
net lines is a review trigger. No production interface, DICE key, equality,
invalidation, locking, request, source certificate, permission or transport change.

Use a new record for the original immutable request's mapping provenance and
typed attributes. Preserve WP982 identity output and all prior record grammars.
Borrow from the same first-offender request returned by existing admission;
emit only while that request is alive. Reuse MaterializationWriter/BorrowedHex
and identity formatting where suitable. The diagnostic retains no Arc, String,
Vec, map, pool or request beyond the emission call. It must not format a whole
RepoSpec into an owned temporary string. Repeated retained strings should use
existing scoped interners when ownership and cleanup fit; temporary diagnostic
values should remain borrowed. Test-owned output buffers are bounded scratch.

The proposed projection covers None, Bool, Int, String, Label, Iterable and Map,
including label/mapping provenance and ordered nested entries. Review the exact
wire grammar before writing code. Bound top-level attributes32, visited nodes128,
depth8, each byte field1024, combined raw bytes4096 and marker10240 including LF;
use checked arithmetic and a complete allocation-free preflight. Empty, absent
and refused are distinct. Oversize/unsupported projection reports genuine bounded
unavailability with no partial values; no cap increase or invented completeness.
Do not treat observed order as chronology or the projection as a source certificate.

Discriminators: original pointer/Arc count1 and Weak retirement; first offender
and mixed Need ordering/progress0; every typed value/key and mapping variant;
Unicode/NUL/empty bytes; exact bounds and one-over/depth/nodes/combined limits;
all-or-none output and exact marker byte budget. Reuse WP982 borrowed identity
and WP983 generated-plan presentation tests. No Bazel parity claim is added:
this is a Slug-native test diagnostic over unchanged pinned source owners.

Validation: independent source/evidence proposals and root bindings, consequential
DESIGN, closed cohort/input/source baselines, implementation, pinned formatting
and freeze, independent STATIC, then pinned nightly2025-09-14 offline jobs1 Core
compile/check preparation with explicit tools/Cargo JSON and60s caps. Fresh focused
ordinary tests are the existing materialization refusal precedence/bounded tests,
generated-plan refusal order/bounded tests, and the new immutable projection test.
Batch preflight for the fresh executable; execute serially. Reuse unaffected
WP1078 suite results with zero fresh credit. Four decoder themes test the new
record's bounds/schema/identity/contradictions; do not replay accepted historical
controls. New decoder validation occurs before removing only the authenticated
new record for historical composition. Current full5 context remains original.
After the fresh gates and saved diagnostic layout/copy checks, independent
PRELAUNCH binds actual source, helpers, inputs/design and fresh executable.
Run ONE normal native probe under unchanged caps, then audit actual counts,
freeze actual successor notes, independently FINAL-review, accept and commit.
No identical native retry or selection of unobserved source/body targets.

Resource limits remain: native10s, cleanup3s, RSS262144KiB, AS512MiB, work5s,
life8s, object1MiB, source bank16MiB, source256, ordinary128, probe16,
production64, census257, raw131072, encoded524288, logs1MiB each/report4MiB,
FSIZE16MiB. Preparation is separate from runtime. No source/helper/fixture writes
during freshness tests; no shared-target concurrent Cargo. Actual errors/stops
are preserved and qualified, never credited as passing. Old maps stay immutable;
only exact changed Rust entries resolve accepted-Git snapshots, all other current
artifacts remain strict, historical executable digests stay opaque.

Root owns priority/config/formatter/freeze/activation/gates/audit/receipt/commit.
Disjoint source and evidence authors work after DESIGN; independent reviewer
checks DESIGN and material final delta, reusing passing evidence. Unknown source
availability is not a blocker for this diagnostic, and observed attributes do
not authorize a later materialization. Replan only for a new retained owner,
semantic/permission change or inability to produce a bounded faithful projection.
Full goal remains active. Windows-specific work remains minimal/deferred.

Immediate predecessor and baseline authority:
{"path": "thoughts/shared/plans/slug-v2-subplans/wp1078-apple-platforms-source-2026-10-10.json", "sha256": "2350bb618a637010b6ce12746ec784f845b969f9ffc2d0ef86a17c81134e893f"}
{"path": "target/wp1078/next-source.md", "sha256": "17a33e757cbfbaed23d6e94060c645e15c2104d673ef0a6e24bbc06f86908cc7"}
{"path": "target/wp1078/next-evidence.md", "sha256": "7606d8b81227b233e13e664232428e9d7a700a7e2e0e02031fa36eedfd15ff26"}
{"path": "target/wp1079/baseline.json", "sha256": "98d67b85ff817f6d967e1479e922dbe838112dce817e88db8d150b47ce9c77cb"}

Accepted current authority:
{"path": "thoughts/shared/plans/slug-v2-subplans/wp1079-immutable-spec-readiness-2026-10-10.json", "sha256": "20f167a615f282539a917c015d99d2eb4ea308ff3834197dcf81dfd24947fd7b"}
{"path": "target/wp1079/native/final-evidence.md", "sha256": "a83ac5ff9aa38837dfe13d88f8969685349329406c7fcf3e20fd01495b7d3485"}
{"path": "target/wp1079/native/operational-summary.json", "sha256": "c42e9e9b0c8c375bd8a8653514c0b89c4aee475a05c924c520309a46d6bb8a44"}
{"path": "target/wp1079/next-source.md", "sha256": "b450efa159c5f76132c84e8153a8dae2815e004310945d68cfd3bf4cf8a64f42"}
{"path": "target/wp1079/next-evidence.md", "sha256": "550e1686d1f64a2b6d9e13d09e4df437f3a6a8e464d3bd0eced296b3ef93d3c4"}
