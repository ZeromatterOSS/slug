# Current Slug V2 Work Packet

Packet: WP-5-7A-selected-bcr-file-capture-impl-r1

Status: implementation selected after independent terminal design ACCEPT.
The design milestone changed docs only; implementation and its gates remain unrun.

## Observable result

Implement verified Linux local-file capture for selected-BCR archive, MODULE,
patch and overlay payloads, including explicit primary/mirror source URLs.
Stage5's "Verified selected-BCR local-file capture contract" is the frozen
architecture/grammar/safety/lifecycle authority; read it before implementation.

Accepted predecessor: archive modes0f45fab28, then audit checkpointcfd4de0ec
proved that file-registry MODULE URLs reach HTTPS-only selected-BCR capture.
No authentic complete CLI closure is established. Do not restore preserved R2.

## Source basis, classification and decisions

Pinned Bazel9.2 /home/wgray/bazel git object
8220c6198837d5c13d53fea211cf3282aa12408a:
IndexRegistry.java:134-142,245-255,448-538; IndexRegistryTest.testFileUrl and
testGetArchiveRepoSpec; HttpConnector.java:114-120 and
HttpConnectorTest.localFileDownload. Stage5 records exact paths/source hashes.
Adapt local-byte download, registry MODULE/SRI and ordered-mirror themes in tiny
unit fixtures. No Java runtime/implementation-detail port or new oracle fixture.
Query-bearing file mirrors in upstream projection are not admitted in capture.

Exact named subset: explicit local payload bytes/SRI and ordered mirror selection.
Slug-native: narrow Unicode URL grammar, Linux descriptor safety, limits/errors,
filesystem observations and existing structural immutable repository identity.
Unsupported/deferred: other OS capture, arbitrary file authorities/URL forms,
query/fragment, HTTP-to-file redirects, remote/FUSE/pseudo-filesystem latency
guarantees and interrupting stuck kernel calls. Existing HTTPS admission stays
unchanged except error wording that must name the newly admitted file subset.

Decisions: one new private file grammar/capture owner; O_PATH regular-file pin
then checked procfd reopen; <=64KiB streaming into existing NamedTempFile with
exact SHA-256/EOF; existing four role caps; common ordered capture_urls dispatcher;
I/O-free NativeEnvironment construction plus per-payload lazy TLS, never file DNS.
No temporary bridge/fallback. Failure may only try the next explicit mirror.
Do not change LocalTar/local_repository, scan caches or infer source locators.

## Ownership, revisions and memory

RepositoryMaterializationRequest retains full URL/spec/SRI identity; existing
RepositoryMaterializationResultKey and generation-key dependency own retry.
No new key/semantic field or representation. Verified immutable reuse survives
original-source deletion/mutation only while existing root observations validate;
changed URL/spec/SRI requires a distinct request. Failures bind current generation;
newer generation retries. No mutable-source historical snapshot claim.
Native materializer captures outside locks, rechecks session, owns provisional
AssociatedImmutable roots and publishes only via existing observation/final
source-certificate acceptance. Stale/discarded sessions cannot publish.
Read docs/developers/dice.md and dice/dice/docs/{writing_computations,transients}.md;
these are ownership concepts, not code donors or generic transient replacement.

New path/descriptor/fixed buffer/temp capture/TLS cell are phase scratch; descriptors
and captures drop on error/cancellation, successful captures transfer to the existing
realizer, TLS cell drops after its payload's mirror group. Existing root lifetime,
equality cutoff, generation invalidation and session shutdown/discard stay intact.
No service/global cache, background task, blocking worker, body Vec or mmap.
No retained-representation change; utility/Stage9 work and benchmarks inapplicable.

## Exact file allowlist and caps

All paths below are relative to the checkout. Production and colocated proof:
- app/slug_bzlmod_v2/src/selected_repo_spec.rs: <=10 production added lines in
  archive_repo_spec primary absoluteness only; focused projection tests.
- app/slug_core_v2/src/runtime/repository_archive.rs: all-role plan admission
  through shared file grammar; existing HTTPS guard stays unchanged.
- app/slug_core_v2/src/runtime/repository_archive_http.rs: explicit file dispatch,
  lazy NativeEnvironment TLS and default prepare_https seam; keep HTTPS lifecycle.
- app/slug_core_v2/src/runtime/repository_archive_file.rs (new): shared grammar
  and safe capped capture; <=260 production lines, no unsafe syscall wrappers.
- app/slug_core_v2/src/runtime/mod.rs: new private module declaration only.
Proof-only:
- app/slug_core_v2/src/runtime/tests/repository_archive_tests.rs.
- app/slug_core_v2/src/runtime/tests/repository_archive_http_tests.rs.
- app/slug_core_v2/src/runtime/tests/repository_archive_file_tests.rs (new).
- app/slug_core_v2/src/runtime/tests/repository_archive_file_session_tests.rs (new).
- app/slug_core_v2/src/runtime/repository_io.rs: test-only include inside existing
  test module, reusing its private native request/session/archive helpers.

Gross additions <=500 production/1100 proof/1600 aggregate. New proof files each
<=550 lines. Stage5 records the inspected size/cohesion decision; do not grow the
large repository_io owner with another inline test subsystem.
Docs: this manifest, canonical Live Status and relevant Stage5/6 status; routing
only for REPLAN. <=180 added doc lines outside manifest; PROGRESS.md <=500 lines.
No dependency, extraction/patch semantics, DICE/source-preparation, registry IO,
CLI, harness/fixture, vendored source, cache/source-input or unrelated edits.

## Required discriminating proof

1. Bzlmod primary file URL and ordered file/HTTPS mirrors, including primary-file
   plus mirror projection/capture with its doubled interior slash (keep source
   projection unchanged); registry MODULE URL/SRI,
   patches and overlays retain exact projection. Preserve old relative/missing
   source rejection. All four core roles share grammar; table-test its complete
   admitted/negative boundary including raw/encoded normalization traps.
2. First tiny file success is red on baseline and green through real native
   capture. Empty/nonempty/Unicode and escaped filenames, exact/mismatching SRI,
   truncation/growth, per-role sparse oversize and streamed overflow rejection.
   Use tiny configured limits for stream boundaries and sparse set_len for native
   caps; no giant physical body, ignored artifact test or external archive.
3. Missing/mismatching file falls back in explicit order; first verified file
   stops before later capture/TLS/DNS/connect. NativeEnvironment TLS cell stays
   empty for file-only success/failure; fake transport counters prove mixed-order
   behavior. Preserve existing HTTPS redirect/auth/lifecycle tests; explicitly
   reject HTTP-to-file redirect. Only existing loopback test servers are allowed.
4. Reject directory/FIFO/socket/device before data-open; symlink-to-regular works.
   Deterministic replacement/unlink after pin and mutation of unread bytes via
   existing active callback; no sleeps, timing races or new production test hook.
   Cancellation before pin, after reopen, during streaming and finalization drops
   temporary capture and both descriptors. Missing procfs branch fails closed;
   do not mutate mounts or require privileges to exercise it.
5. Native materialize_native_with_runtime with tiny complete archive/overlay/
   patch/MODULE source establishes actual immutable root contents/transform order.
   Exact-request accepted reuse after source deletion, changed-SRI A/B/A, missing/
   create and mismatch/repair in newer generation, invalid source then retry;
   inspect result generation, root/instance and cleanup. Reuse real observations/
   accept helpers; compare current writer output, not hard-coded marker formats.
   Add stale-token/discard and overlapping-session rejection around real capture
   using existing callback seam, not synthetic Materialized success. No lock across
   capture. Reuse existing Bzlmod DICE request/generation tests for key behavior.

## Focused validation and stops

Every test/compiler command starts timeout60; serialize Cargo. Investigate any
command exceeding one minute; fifteen minutes is the absolute ceiling, never an
automatic extension/retry. Stop on timeout, unexpected failure, unsafe/blocking
open or scope overflow; report exact evidence rather than retrying a broad suite.
Use installed pinned nightly-2025-09-14. Record exit/count/duration, not full logs.

- timeout 60s cargo +nightly-2025-09-14 test -q -p slug_bzlmod_v2 --lib selected_repo_spec::tests::archive_
- timeout 60s cargo +nightly-2025-09-14 test -q -p slug_bzlmod_v2 --lib observed_repository_materialization_
- timeout 60s cargo +nightly-2025-09-14 test -q -p slug_core_v2 --lib runtime::repository_archive
- timeout 60s cargo +nightly-2025-09-14 test -q -p slug_core_v2 --lib selected_bcr_file_session
- timeout 60s cargo +nightly-2025-09-14 check -q -p slug_cli_v2

Name new native session tests selected_bcr_file_session_* so the exact gate
selects them. New file tests are included in the repository_archive prefix gate.
Confirm selected tests actually ran; keep existing large ignored test ignored.
No full-core/full-suite retry, network acquisition, CLI/Bazel/daemon execution,
external archive replay, source override or fake-platform control. The CLI check
is compile evidence only; any later actual CLI proof requires a fresh build.
Run formatting/diff, preservation hash/forward-applicability and archive checker
(only known3 thoughts paths). Independent terminal review precedes acceptance,
commit/push and successor selection. A new identity/public owner, unsafe fallback,
scope overflow or second material correction is REPLAN, not implicit authority.
Never inspect/print/copy ~/.bazelrc or derived secrets.

Complete unaccepted R2: /tmp/slug-conflict-r2.XZJWwv/candidate.patch,
base97dffd5d4, SHA-256
90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e.
Adjacent validation.txt owns all passed/failed/unrun gates. Do not restore or
partially ship here. Actual one-shot/stable-daemon CLI and positive common REAPI
sharing remain open; only two fast core failures are baseline-attributed.
