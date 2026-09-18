# Current Slug V2 Work Packet

Packet: WP-7-32-m7a-verified-local-cas-r1
Status: final ACCEPT; checkpoint ready to commit

## Outcome and authority

Make the local NativeLink oracle CAS reject invalid new content before persistent
publication. This closes the concrete backend configuration gap exposed by source
staging WP-7-31, without claiming concurrent Execute admission. Slug-native harness
configuration; exact SHA-256/size CAS integrity for Slug's actual bytes. No new Bazel
parity surface, semantic identity, DICE state, cache API or production executor.
Inherited source staging/verified reader contracts are in Stage 7 and accepted
WP-7-31 at 055cf7406e2c306baea8dd5ebc98b85560eb94c9.

Primary source: local NativeLink checkout 2c5496173036773d205c5a39e75d0b7fc1a08c8a,
nativelink-config/src/stores.rs Verify/RefStore; nativelink-store/src/verify_store.rs
withholds backend EOF until hash/size validation; filesystem_store.rs commits at
EOF. local_worker.rs requires a concrete FastSlowStore. fast_slow_store.rs
has_with_results reports in-flight writes as present; VerifyStore delegates that
lookup. ByteStream retains disconnected streams for a source-default minute; its
config documentation still says ten seconds. This is configuration reuse and
source evidence, not copied donor code.
The installed backend binary is also tested directly; its build revision is not
asserted from the source checkout revision.

## Owner and invariants

The shared tools/v2_oracle_lib/nativelink.py configuration owns local backend
wiring. Retain one named CAS_RAW FastSlowStore with the existing filesystem roots.
Public CAS is a VerifyStore referencing CAS_RAW with both verify_size and
verify_hash enabled. CAS, ByteStream and execution frontend services use CAS;
the local worker alone uses CAS_RAW for its required FastSlow interface. AC stays
unverified because its keys are action digests, not content digests. The local
ByteStream service retains disconnected streams for one second, with
a half-second sweep interval, to bound real cleanup checks. No unsafe configuration
switch or alternate upload implementation.

Preserve bounded client reader verification, cancellation and no-finish-on-mismatch.
A local upload failure can precede backend cancellation cleanup: the wire test
boundedly waits for missing state instead of requiring an immediate miss. A verified
wrapper does not cleanse pre-existing corrupt content and does not eliminate
NativeLink's in-flight presence advertisement. Tests use fresh roots. Future Execute
must establish completed verified staging, validate the complete source/build
frontier, and cannot infer either fact solely from FindMissingBlobs.

## Scope and discriminating proof

Allowlist: tools/v2_oracle_lib/nativelink.py; app/slug_reapi_v2/src/source_staging/tests.rs;
canonical plan, current manifest and 07-reapi-native-execution.md. Existing tiny
Core fixture and public wire selectors are reused; no new copied fixtures or deps.
No graph/cache production Rust changes. Backend process and temporary storage
remain operation-owned and always reaped/deleted by the supervised harness.

Strengthen the existing source staging wire negative: after local digest mismatch,
require CAS absence after bounded server cleanup, then restore the correct source
and prove upload/download succeeds for that same digest. Also send direct public
requests bypassing client validation: a wrong-hash BatchUpdateBlobs must return
a per-blob error with the verifier hash diagnostic; a correct-hash, wrong-size
ByteStream Write with finish_write=true must return the verifier size diagnostic.
Both require absence and exact verified recovery reads. NativeLink can merge a
verifier error with backend cancellation errors into INTERNAL, so exact status-code
parity is not claimed. BatchUpdate size validation happens before the store, which
is why the independent size discriminator uses ByteStream. Wrong-hash recovery uses the same digest. The wrong-size case uses a correct
hash with an incorrect size (an unrealizable key), then uploads under the corrected
size and confirms the malformed key stays absent. Protect existing FileWrite cold
Execute, AC hit and modes
to prove shared worker/public storage and unchanged AC semantics. Keep existing
source Merkle and inline plan ordinary checks.

Independent design/final review. Compile separately with pinned nightly --no-run
JSON, <=60s each preparation; select exact executable. Ordinary exact preflight and
explicit ignored-selector listing before supervised wire tests. Wire tests
expected under a few seconds, including a two-second monotonic cleanup deadline;
fresh local backend, 8s per-selector operation cap and 15s overall diagnostic cap. User guidance is few-second checks for frequent
use and strict justification above ~30s; these narrower operation caps are local
estimates, not user requirements. No broad suites or compiler action. Format,
diff, archive and plan checks. Raw receipts target/wp732.

REPLAN if the verifying boundary cannot reject invalid persistent content while
preserving the existing worker/AC path; do not weaken the absence proof or admit
Execute based on transient presence. Invalid invocation and test corrections stay
within this packet. M7A partial and M8 unproved throughout.

Predecessor WP-7-31 accepted/pushed 055cf7406: closure-owned source staging, ten
focused gates, all test batches under one second. Its backend negative proved local
verification failure but exposed the missing server-integrity gate addressed here.


## Acceptance evidence

Baseline 055cf7406e2c306baea8dd5ebc98b85560eb94c9;
review/wp732-verified-local-cas. Independent design review accepted the verifying
store wiring and the evidence-driven timeout/status corrections. Independent
final review ACCEPT. Production Rust and client streaming behavior are unchanged.

- Pinned nightly-2025-09-14 direct toolchain; the rustup snap launcher failed
  before preparation (exit46), so the already-installed pinned binaries were used.
  Three separate cargo test -p slug_reapi_v2 --lib --no-run --message-format=json
  preparations exited0 in 7.965s, 6.578s and 6.488s; longest under8s. Exact executable
  target/debug/deps/slug_reapi_v2-83de72d5efb378db selected from Cargo JSON.
- Ordinary exact preflight selected2; final batch exit0, pass2, elapsed0.003s:
  source_staging::tests::source_merkle_nodes_are_executable_and_paths_are_structural
  and executor::tests::file_write_plan_owns_canonical_nul_safe_reapi_objects.
- Ignored source_staging::tests::nativelink_closure_sources_merkle_and_verified_upload:
  exact ignored listing1, final pass1/1, exit0, runtime2.180s; fresh backend setup,
  test and cleanup2.258s. Retains source/tree/parameter roundtrip and deletion-hit
  proofs; now proves stale-source absence after disconnect cleanup and same-digest
  recovery. Raw wrong-hash BatchUpdate has a matching per-blob non-OK status with
  hash-verifier diagnostic. Raw wrong-size ByteStream finish has a size-verifier
  diagnostic. Both remain absent after cleanup; valid bytes upload/read exactly,
  and the unrealizable oversized key remains absent after corrected-size upload.
- Protected ignored executor::tests::
  nativelink_file_write_bytes_digest_and_materialized_mode_match_oracle:
  exact listing1, pass1/1, exit0, runtime0.346s; complete lifecycle0.424s. Cold
  Execute, AC hit, bytes/digest/mode and streamed/reader uploads pass through the
  shared verifying public store and raw worker store. This ran before the isolated
  disconnect-retention correction; it has no interrupted upload, so the passing
  evidence is retained. Both backends exited143 and their temporary roots were
  removed; loopback use received the normal reviewed sandbox exception.
- The first source wire run failed the1s absence deadline (1.578s); source evidence
  identified retained resumable uploads with a default minute, so the local
  harness explicitly sets1s retention and the test allows2s including sweeping.
  The next run proved cleanup/recovery but failed an over-specific status-code
  assertion (2.026s): NativeLink merges hash rejection with backend cancellation
  into INTERNAL. The corrected discriminator requires the verifier diagnostic
  and strict rejection/absence/recovery; it does not accept arbitrary RPC failure.
  Source inspection moved wrong-size proof from BatchUpdate's frontend check to
  ByteStream's store verification before another run. No integrity gate was waived.
- Changed Rust format, diff, plan and archive checks pass. No new dependency,
  copied fixture, Slug daemon, compiler action or broad suite. Longest test2.180s;
  compile time21.031s across three preparations. Continuous packet/review elapsed
  time was not recorded; no performance improvement claim. Raw local receipts:
  target/wp732.

Gate advanced: the local conformance backend now verifies new public CAS writes
and rejects hash/size mismatches with demonstrated cleanup/recovery. This does not
repair existing stores, validate worker-produced bytes, or make an in-flight CAS
hit safe for concurrent Execute. Completed verified staging and full-frontier
validation remain required before typed Spawn execution. M7A partial; M8 unproved.
