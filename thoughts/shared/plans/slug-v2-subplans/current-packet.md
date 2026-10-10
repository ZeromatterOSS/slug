# Slug V2 current work packet

Packet: WP-7-293-m7a-original-immutable-offline-materialization-r1
Status: accepted — portable materialization; real archive write-quota boundary
Baseline: 8c2357f53b60a5641cb34b24b87370e1cec84bde (accepted WP1079).
Owner: M7A bootstrap-readiness.md; original immutable materialization.
Compatibility: Slug-native Linux scoped offline source capability; cfg(test)
diagnostic activation. Existing archive semantics unchanged; Windows deferred.

Outcome: the original protobuf+ selected-BCR request can consume explicitly owned
local archive backing through the normal scoped materializer. Preserve original
HTTPS URL, complete RepoSpec, request/result epoch and namespace. Prove successful
normal materialization in a portable small fixture and failure/cleanup under caps.
The real continuation may materialize or report a genuine source/resource boundary;
a passing diagnostic does not establish normal compiler-producer success.

Research: WP1079 captured the supported nine-attribute shape, no rule mapping,
original @@bazel_tools//tools/build_defs/repo:http.bzl/http_archive. Its native
7.003039177s/209312KiB fits262144KiB; new owned production strings0. The accepted
receipt owns the full observation. Full Rustc/actions/certificates/producer and
whole-workload fit remain open. This packet advances functionality.

Explicit backing authority: tests/v2_oracle/fixtures/configured-cli-authentic/fixture.toml:190–196
and tools/v2_oracle/configured_cli_fixture.py:141 bind the original URL
https://github.com/protocolbuffers/protobuf/releases/download/v33.4/protobuf-33.4.bazel.tar.gz
to the fixture-relative mirror leaf, size6889704/SHA256
687e98a471973b5c5fd711750c40b8b82c0ade33f649db65e00b290f29345a2b.
Research found that exact regular leaf with matching SHA. The observed MODULE is
target/wp954/registry/modules/protobuf/33.4/MODULE.bazel, size13194/SHA256
114775b816b38b6d0ca620450d6b02550c60ceedfdc8d9a229833b34a223dc42.
Root qualifies both before activation. No acquisition, download, guessed cache
path, extracted-tree substitution or source override. Pinned Bazel9.2
RepositoryCache.java:39/DownloadCache.java:183/BazelRepositoryModule.java:325 are
reference-only: raw download and contents caches have separate authority. They
justify no cache-path inference or new parity claim.

Natural owners: repository_archive.rs:85 validates the plan; scoped.rs:141 captures
original URLs; repository_archive_file.rs:83 pins a regular inode, reopens the held
descriptor, streams64KiB with counted transfer/write, verifies EOF/SRI;
repository_archive_realize.rs:39 owns extraction/final MODULE/AssociatedImmutable.
repository_io.rs:786 keeps scoped session/current generation/postcheck;
source_preparation.rs:5538–5588 keeps exact request/current workspace result epoch.
Read docs/developers/dice.md before ownership changes; no lock spans compute.

Decision: constructor-owned optional OfflineArchiveBacking on the test/Linux
SourceServiceScope, holding a static original URL, copied32-byte SRI digest and
existing shared NormalizedAbsolutePath. Validate HTTPS grammar, then match exact
borrowed URL+digest. Factor the existing file capture's physical-path entry, keeping
ordinary file-URL grammar and the same pin/length/descriptor/SRI/counter/cancellation
algorithm. Original Spec stays intact. Unconfigured/mismatched requests retain
NetworkDisabled; missing/corrupt configured backing fails without network fallback
or partial publication. Normal realization/request association/postchecks remain.

First-offender admission borrows and checks original workspace/canonical repo/kind,
rule/mapping and all nine typed attributes, including prepared MODULE path/SRIs.
Skip only that exact matching request with its configured backing; preserve whole
Need order/progress and all other refusals. Do not allocate another ArchivePlan for
eligibility. Backing is immutable construction plumbing; it cannot change a valid
semantic result because the original SRI is verified each use. Delete/recreate,
corruption and newer generations use normal capture/session behavior; equal verified
content preserves existing equality. No new DICE key or injected semantic result.

Memory: at most one shared service/attempt backing descriptor, shared existing path,
static URL and POD digest; no owned request/plan duplicate or payload Arc. One64KiB
buffer/tempfile is phase scratch. Last scope/backing/task Drop retires ownership;
no evaluator/command-scratch borrow. Borrow/transfer first, use existing scoped
interners for repeated retained strings when lifetime and cleanup fit. No new pool
or global cache. Measure changed scope/view layout and bounded native RSS, without
whole-heap claims. Stage9 bounded Host capture row is guidance; no donor import.

Rust allowlist: app/slug_core_v2/src/runtime/source_service.rs;
app/slug_core_v2/src/runtime/source_service/scope.rs;
app/slug_core_v2/src/runtime/source_service/offline_archive.rs (new bounded owner);
app/slug_core_v2/src/runtime/repository_archive_file.rs;
app/slug_core_v2/src/runtime/repository_archive_http/scoped.rs;
app/slug_core_v2/src/runtime/tests/platforms_module_preparation_tests.rs;
app/slug_core_v2/src/runtime/tests/platforms_module_preparation_tests/immutable_archive_preparation.rs (new);
app/slug_core_v2/src/runtime/tests/platforms_module_preparation_tests/public_probe_preparation_tests.rs;
app/slug_core_v2/src/runtime/tests/repository_archive_file_session_tests.rs.
Root owns target/wp1080, scheduling docs and compact receipt. Large parent/child
files get narrow wiring and a bounded child owner. Estimate150–250 implementation
plus200–350 test lines is a review trigger. Other folders remain references.

Discriminators: exact/default/foreign/missing/corrupt backing; unchanged Spec and
attributes Arc; every guard field plus mixed Need order/progress0; tiny archive
normal materializer/root/MODULE success; warm reuse after source deletion/newer
recapture; stale scope/cancellation/quota cleanup without root publication;
last-Drop release. Reuse selected-BCR file-session, capture-file, streamed-realize
and source-service tests; unchanged fixture provenance, no new oracle workspace.

Validation: independent DESIGN, disjoint implementation/evidence, pinned formatting
and freeze, independent final review. Resolve nightly2025-09-14 tools explicitly,
offline jobs1; Core --no-run/Cargo JSON and Core/CLI dependent check, separate60s
preparation caps. Batch fresh exact-selector preflight, then serial bounded owner
tests. Prior evidence gets zero fresh credit. Before ONE native continuation,
freeze root-qualified original inputs plus exact archive backing, executable and
helpers; review launch bindings. Reuse existing supervisor/framing without replaying
historical test chains. Preserve actual failures; review actual semantic evidence,
accept with portable capability versus real producer qualification, local commit.

Unchanged caps: native10s/cleanup3s/RSS262144KiB/AS512MiB/work5s/life8s;
source256/registry128/object1MiB; transfer/decode/write/live bank16MiB/FSIZE16MiB;
logs1MiB/report4MiB and current census/diagnostic bounds. Archive+MODULE capture
alone writes6902898B; decoded/extraction-write fit is unknown. A real limit informs
a subsequent explicit resource decision, never silent cap growth or identical
retry. No source/helper/fixture writes during freshness gates or shared-target
concurrent Cargo. Replan for a new semantic owner, incompatible source authority,
or inability to bound the capability; routine correction stays in this packet.

Immediate predecessor: accepted WP1079 at8c2357f53, receipt
thoughts/shared/plans/slug-v2-subplans/wp1079-immutable-spec-readiness-2026-10-10.json
SHA25620f167a615f282539a917c015d99d2eb4ea308ff3834197dcf81dfd24947fd7b.
Independent FINAL a83ac5ff9aa38837dfe13d88f8969685349329406c7fcf3e20fd01495b7d3485.
Its first local commit failed for missing identity; it succeeded with the same
per-command Codex identity as predecessor, without changing global Git settings.
Full implementation goal remains active. Windows-specific work stays minimal.

Acceptance: WP1080 receipt thoughts/shared/plans/slug-v2-subplans/wp1080-original-immutable-offline-materialization-2026-10-10.json; eight ordinary Core tests,
three helper controls and one real native observer pass. Native5.126s/209720KiB;
original archive+MODULE captured/verified, normal realization refuses cumulative
WrittenBytes16MiB. Full realization/producer remain open; independent FINAL
bb216f933ac3fb4caa6363426f8de4a951e556d62dab327d2602c088e493cb88. Full goal remains active.
