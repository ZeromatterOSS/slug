# Current Slug V2 Work Packet

Packet: WP-7-29-m7a-streamed-cas-upload-r1
Status: final ACCEPT; checkpoint ready to commit

## Outcome and evidence

Add CacheClient::upload_reader_verified(expected_digest, reader), a bounded-memory
ByteStream upload for ordinary compiler/toolchain inputs. Existing upload_missing
requires a complete ReapiBlob, so streamed digest observations alone cannot avoid
file-sized transfer ownership. Demanded by bootstrap-readiness ordinary source/
generated input transfer and shared cache core rows for //app/slug_cli_v2:slug.
This is the next transfer prerequisite after WP-7-27 digest observations and
WP-7-28 repository-aware source admission. Artifact digest staging, generated/tree
inputs and validated-closure Spawn activation remain required; do not admit a
family or claim bootstrap from this leaf boundary.

Pinned protocol: slug_reapi_cache_v2/PROVENANCE.md, google/bytestream/bytestream.proto
Write/WriteRequest/WriteResponse (54-74, 120-161); resource name, monotonically
contiguous offsets, finish_write exactly once, committed size. Bazel 9.2 commit
8220c6198837d5c13d53fea211cf3282aa12408a ByteStreamUploader.java
startAsyncUpload and checkCommittedSize, Chunker.java file/input stream chunking.
Exact uncompressed SHA-256 bytes/offsets and full committed size are admitted;
Slug-native source verification/cancellation policy requires local EOF, digest
and size verification plus RPC success. Resumption, compression and early-server
success without completed local verification remain deferred/fail closed.

## Owner, integrity and lifetime

The graph-independent cache leaf owns protocol and verified transfer. Caller
supplies an AsyncRead and validated expected ReapiDigest; no path, artifact, DICE,
configuration or source-certificate type enters the leaf. Core will own opening
observed source files and validating request provenance before publication. A
matching CAS digest does not validate the source route or requested revision.

Use a private reader_upload module, a bounded one-slot channel, and an explicit
producer/RPC select state machine in the calling task; no spawned producer or
detached reader. Poll the producer first when both futures are ready. Producer
error cancels the RPC; RPC error cancels/drops the producer before running the
existing QueryWriteStatus diagnostic. RPC success before producer EOF/hash/size
verification and final send cancels/drops the reader and fails closed. Only
verified producer completion followed by matching committed-size RPC success
returns Ok; never join a stalled producer after an early RPC response. Reserve channel capacity before reading. One bounded data chunk plus
one queued chunk (and bounded transport buffering) replace file-sized retention;
SHA-256 state, offsets and resource strings are transfer-local scratch. No new
cache/interner/retained graph, dependency or lock. Vec owns each protobuf payload;
existing Tokio/futures/SHA-256 utilities suffice. No donor code or perf claim.
Reader is owned by the operation; cancellation/error drops producer and reader,
and cancels the request future. Channel/RPC state cannot outlive its transport
cancellation beyond existing library internals.

Emit data chunks without finish_write. At EOF require exact length and SHA-256;
only then emit one empty final request with finish_write=true at the full offset.
Empty input emits a single resource-bearing final request. Bound reads to the
remaining expected length plus one byte so growth fails promptly. Interrupted
reads retry; short reads accumulate; source errors/truncation/growth/corruption
must return errors and never send finish_write. Server committed size mismatch or
RPC failure never succeeds. Preserve existing interrupted-write QueryWriteStatus
diagnostic, without resuming or treating status as success. Existing inline
ReapiBlob upload remains unchanged. Caller decides FindMissingBlobs/AC policy.

## Scope and proof

Allowlist: cache_client.rs for bounded dispatch/error integration; new
cache_client/reader_upload.rs and reader_upload/tests.rs; existing REAPI executor.rs
NativeLink test only for an additional streamed-reader CAS round trip; canonical,
manifest, Stage 7/11 architecture and Stage 9 compact utility disposition. No proto,
Cargo/BUILD dependency, action semantic, CLI, daemon or backend implementation edit.

Focused deterministic tests drive the same producer/RPC coordination helper used
by the public method: tiny chunks, empty input, short/interrupted reads, exact
resource/offset/finish bytes; corruption/short/extra/read error never finish;
wrong committed size, transport failure and early RPC success; blocked source cancellation and
reader drop; stalled RPC backpressure bounds reader demand. No sleeps needed.
Preserve existing inline ByteStream/error tests. Extend the existing ignored
NativeLink FileWrite test with a real public-reader upload/download using 3-byte
chunks, preserving its cold execution/AC-hit proof. This is an existing selected
subsecond harness, not a full suite. Exact ignored selector checked separately,
backend supervised and cleaned; ordinary selectors use v2_test_preflight.py.

Independent design and final review for public async transfer/lifecycle boundary.
Compile pinned nightly separately --no-run JSON, <=60s preparation operations.
Focused tests expected subsecond; >few-second tests infrequent, >~30s need strict
necessity. NativeLink startup/test supervisor <=15s operational cap, not a user
cutoff. Direct REAPI and Core compile coverage. Format/archive/plan/diff checks.
If backend preparation or runtime exceeds its cap, preserve evidence and diagnose;
do not substitute pure unit checks for the declared public wire gate.

Predecessor WP-7-28 accepted/pushed at 9ff7b172c: repository-aware source paths and
materialized external SourceFile routing; 11 focused gates, direct dependents and
independent final ACCEPT. M7A partial and M8 unproved.

## Acceptance receipt

Independent corrected design and final review ACCEPT. Gate advanced: public
bounded, digest-verified reader-to-ByteStream upload with cancellation ownership
and a real CAS round trip. Existing inline and FileWrite gates remain accepted.

Baseline 9ff7b172c; review/wp729-streamed-cas-upload. Direct pinned
nightly-2025-09-14 Cargo/rustc/rustdoc; JSON compile-only preparation under
60-second operation caps. Local raw receipts: target/wp729 (not committed).

- cargo test -p slug_reapi_cache_v2 --lib --no-run: first exit 101 in 1.055s
  for a temporary test digest borrowed past its statement; corrected binding
  preparation exit 0 in 1.429s. Seven exact ordinary selectors preflighted.
  Initial batch had six passes and one mock-lifetime failure in 0.003s: the
  early-RPC fixture dropped its receiver before polling the RPC. Keep the
  receiver in that future; corrected preparation exit 0 in 1.415s and sole
  affected selector re-preflighted/passed in 0.003s. No production correction.
- Seven proved selectors: reader_upload::tests::{
  verified_reader_offsets_eof_and_empty_finish_match_wire_contract,
  source_corruption_truncation_growth_and_read_failure_never_finish,
  wrong_committed_size_and_early_rpc_response_fail_closed,
  stalled_rpc_backpressures_reads_and_cancellation_drops_reader,
  source_failure_cancels_stalled_rpc}; existing cache_client::tests::{
  tiny_chunks_keep_offsets_and_finish_once,
  interrupted_write_queries_status_before_failing_closed}.
- cargo test -p slug_reapi_v2 --lib --no-run: first preparation exited 124
  at its 60s cap after successful cache/API/loading/analysis/query/Core library
  artifacts. Verified terminal process/no surviving compiler. Retained those
  artifacts; second preparation completed the REAPI executable, exit 0 in
  1.646s. This supplies direct REAPI/Core compile coverage.
- Exact ignored selector
  executor::tests::nativelink_file_write_bytes_digest_and_materialized_mode_match_oracle
  checked with --list --ignored --exact, then 1/1 passed in 0.340s. Existing
  NativeLink backend binary, fresh temporary store, 15s backend/test supervisor;
  setup/test/cleanup 0.419s, backend terminal 143 after stop, directory removed.
  Socket creation was denied EPERM in sandbox, so this gate used the normal
  reviewed execution exception. The public reader API uploads three-byte chunks
  and verified download matches; prior FileWrite cold/AC-hit/mode and inline
  ByteStream roundtrip assertions remain. No Slug daemon or CLI/Bazel build ran.
- Changed Rust rustfmt --check, git diff --check, v2_plan_status.py and
  v2_archive_status.sh pass. No dependency/proto or copied fixture change.

Compilation operations totaled 65.558s including the capped operation and
compiler correction. Test batches totaled about 0.35s including the corrected
fixture failure. Continuous packet/review wall time was not recorded. Design
review required explicit early-response cancellation instead of a plain join;
the implementation and blocked-reader tests preserve that accepted correction.
Source routing/certificate integration, artifact digest staging, generated/tree
transfer, Spawn execution and broader bootstrap remain open.
