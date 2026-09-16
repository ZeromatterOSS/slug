# Current Slug V2 Work Packet

Packet: WP-7-13-m7a-reapi-cache-core-leaf-r1
Status: accepted; graph-independent cache core and FileWrite consumer

## Outcome and compatibility

Establish the Stage 11 bootstrap cache core as a linkable Rust leaf and route
the already-admitted FileWrite execution path through it. The leaf owns pinned
REAPI/Google protocol bindings, SHA-256 digest and byte-bearing blob values,
CAS missing/upload/verified-read operations, and Action Cache lookup. It has
no dependency on Slug analysis, DICE, workspace, build-api, command, CLI, or
daemon crates and exposes none of their types. `slug_reapi_v2` remains the
Stage 7 adapter that lowers the validated action, schedules Execute, verifies
the selected output and publishes its result. This is exact REAPI/CAS wire and
digest behavior for the admitted FileWrite slice, with Slug-native client
errors. It admits no new action family or Bazel ActionKey projection.

The predecessor `WP-7-12-m7a-rustc-callback-proof-entrypoint-r1` closed
`REPLAN` at `6a6f48c1e`. Its callback candidate stays unaccepted at local
`review/wp-7-11-rustc-crate-root-map-each` (`ea5fcc6fd`); neither the callback
nor virtual Spawn parameter files are part of this packet.

## Source, owner and design boundary

Use Bazel 9.2.0 source commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and its `third_party/remoteapis` protocol source as the REAPI reference. Its
`MODULE.bazel` selects local `third_party/remoteapis` and Google APIs version
`0.0.0-20250604-de157ca3`. The embedded remoteapis tree is Git tree
`72e3f5671ce4f611759905b548d34fca26591543`; its
`build/bazel/remote/execution/v2/remote_execution.proto` is Git blob
`e9cab05dd2e500bb7d754ee33e004fc612adf5f6`, SHA-256
`9d2723bc91bd7fe154b52118cdc1818daa547c303d01cc346d7bf8fb6caa9cbc`.
Its `build/bazel/semver/semver.proto` is SHA-256
`22b2af125690142af1c8152ba3a4ca15ffaa1265111dedc6e39b5412989b5be1`.
Google APIs resolves to full commit
`de157ca34fa487ce248eb9130293d630b501e4ad`; Bazel's
`third_party/googleapis.patch` is SHA-256
`751c57e02dc8d8c1cd0375c971720d5184de2c05d1f627e7177dc946e19dafd5`
and changes BUILD/MODULE files, not these proto definitions. The effective
Google `bytestream.proto` is SHA-256
`961b833f35f4bdc51df4bca017cffdba299893e89762bf8041465560106dd3d6`.
The effective
import closure is REAPI `remote_execution.proto` and `semver.proto`; Google
`bytestream.proto`, `api/{annotations,client,http,launch_stage}.proto`,
`longrunning/operations.proto`, `rpc/status.proto`; and well-known
`{any,descriptor,duration,empty,timestamp,wrappers}.proto`. Pin each copied
file's SHA-256, license, recursive imports, and generator/toolchain in the new
crate's provenance record before changing adapter code. The current local
`reapi_v2.proto` projection has SHA-256
`eb83f9d37484384638977c3fc0b1f5249ff6301b815b004f1b5ca968ca497949`;
it is a compatibility baseline, not an upstream pin. Pin authentic REAPI,
Google ByteStream, and required dependency definitions; generate bindings
reproducibly for Cargo and Bazel. Preserve the accepted FileWrite `Command`,
`Directory`, `Action`, CAS blob and AC request bytes when replacing the local
projection. Use the pinned protocol's actual field numbers and RPC paths.

`ConfiguredNodeResult.actions` and `ValidatedActionClosure` remain the only
semantic action/closure owners. `ResolvedFileWriteSemanticView` supplies the
validated FileWrite and platform fact; `ReapiCommand`/`ReapiInputTree` compute
request-local wire objects. The new leaf accepts explicit immutable digests,
bytes, instance and transport inputs. It does not discover paths, source
certificates or action semantics, decide whether to Execute, write workspace
outputs, or create DICE keys. Its client holds only per-command transport and
transfer scratch; no process-global cache, long-lived graph memory, or hidden
cross-instance sharing. Stage 7 retains final source validation, conflict
rejection, Execute evidence and output publication.

Move `ReapiDigest`, the byte-bearing `ReapiBlob`, generated `proto`, and raw
CAS/AC transport clients into one leaf rather than duplicating them. Leave
`CasUploadPlan` with `ReapiInputTree`, and leave `GeneratedOutput`, the
projected `ActionResult`, and the in-memory `ActionCacheTable` above the leaf.
The leaf AC lookup returns a full protocol `ActionResult` or typed miss/error;
the adapter validates FileWrite's admitted result shape and fails closed on
unsupported fields before projecting its result. Keep graph-dependent
input-tree, command lowering, executor scheduling, configuration, evidence
and materialization above the leaf. Re-export moved public values from
`slug_reapi_v2` only where existing CLI/server/test callers need compatibility.

The leaf's CAS/AC client accepts a caller-owned tonic channel and instance
name and distinguishes AC `NOT_FOUND` from transport, permission and protocol
failures. It verifies digest and size, rejects missing/duplicate/mismatched
batch responses, and honors the server's batch limit where advertised.
Freeze its public operation boundary as `CacheClient` constructed from that
channel, immutable instance name and a bounded transfer policy, with
`find_missing(digests)`, `upload_missing(blobs)`,
`get_action_result(action_digest, inline_request)`, and
`read_blob_verified(digest, chunk_sink)`. The AC method returns the full
protocol result on hit, a typed miss only for gRPC `NOT_FOUND`, or a typed
error; it does not collapse arbitrary RPC errors into misses. The read sink
receives bounded chunks into caller-owned provisional scratch and no caller
may publish them until the method returns a verified digest/size success.
FileWrite currently admits arbitrary content size: the leaf must preserve
that surface with bounded ByteStream Write and verified Read fallback for
oversized blobs, not a new size rejection. Stream chunks have fixed bounded
size, Write checks committed size and uses `QueryWriteStatus` for interruption,
and Read verifies incremental SHA-256 and exact size before Stage 7
publication. Keep existing output bytes in request-local materialization
scratch only until publication; create no artifact-sized retained DICE value
or second long-lived cache. Endpoint, credentials, retry and RPC deadline
policy stay caller-owned and never salt Action bytes.

Review this shared public boundary before editing code, then review the final
delta. If authentic protocol generation changes accepted FileWrite bytes, or
the proposed leaf cannot own these operations without a graph dependency,
`REPLAN` with the exact conflicting fields/dependency; do not keep a second
handwritten schema or a parallel cache implementation.
Independent design review `ACCEPT` verified the source pin, leaf/adapter split,
large-blob transfer obligation and mandatory live FileWrite gate. It requested
that implementation treat a failed or cancelled provisional read sink as
unpublished and verify exact digest/size before successful return.

## Evidence and validation

Reuse the accepted FileWrite NativeLink and oracle receipts in Stage 7 and
Stage 1 for the baseline. Add small, discriminating leaf tests for digest and
size validation, AC hit/NOT_FOUND/error classification, CAS missing/upload/read
response completeness, instance isolation, forced small-chunk ByteStream
Write/Read, short/duplicate chunks, interrupted Write status and corrupt
download rejection. Force the ByteStream route using a tiny test threshold,
not a large test file. Add exact before/after
FileWrite Command/Directory/Action byte and digest vectors, a negative
ActionKey-vs-REAPI identity check, and a FileWrite cold/hit regression through
the routed consumer. Keep unsupported action families rejected before any RPC.
The focused live transport proof is the existing direct FileWrite NativeLink
test extended to exercise cold execution and a same-action AC hit. The older
`reapi-action-cache-hit` fixture currently declares `ctx.actions.run_shell`,
which lowers to typed Spawn and is rejected before RPC by this packet's
FileWrite-only admission; it cannot serve as this packet's transport gate.
The `simple-rule-action` fixture also fails before any action on the current
module-toolchain loader's unsupported `rules_python` `_rule_name` repository
attribute. Those fixture failures do not widen the action or loader allowlist.
Do not rerun broad oracle or Rust suites. Verify the Cargo/Bazel leaf dependency
graphs and compile named direct consumers. Compile once, select exact tests
with the repo preflight, then run only those tests. Target test runtime is a few
seconds; scrutinize any test over roughly 30 seconds for strict necessity.
Record compile and test times separately.

Allowlist: `app/slug_reapi_cache_v2/**` (new), existing
`app/slug_reapi_v2/{src,proto,tests,build.rs,BUILD.bazel,Cargo.toml}`,
`app/slug_cli_v2` and `app/slug_server_v2` only for direct import/consumer
proof, root `Cargo.toml`, `Cargo.lock`, `Cargo.Bazel.lock`, root `BUILD.bazel`
for protoc visibility, and this manifest/canonical status plus Stage 7/11
owner plans, Stage 10 production inventory and bootstrap readiness for the
new crate/generated-input closure. Preserve Stage 10's historical accepted
33-package receipt; record the new first-party crate and moved protoc owner
as a live delta with its own evidence. No fixture proliferation, daemon protocol,
rules_rust, callback, Spawn activation, disk-cache format, standalone public
release, or cross-tool AC sharing. Run `python3 scripts/v2_plan_status.py`,
format changed Rust, and `git diff --check`; commit and push only an accepted
packet checkpoint.

## Accepted implementation receipt (2026-09-16)

`slug_reapi_cache_v2` owns the pinned upstream protocol import closure and
request-local CAS/AC client. `slug_reapi_v2` now consumes it for the admitted
FileWrite route; the old handwritten schema and adapter build script are
retired. The frozen Command/Directory/Action wire vectors pass unchanged.
The pinned aquery oracle shows FileWrite ActionKey stable across an output-path
edit while the corresponding REAPI Action digests differ.

Focused leaf tests passed 10/10 (test execution below 0.01 seconds; final
incremental compile plus test command 1.13 seconds). The FileWrite wire and
ActionKey tests each passed as named tests. The direct NativeLink test passed
cold FileWrite execution, same-action AC hit with no upload, and forced
three-byte ByteStream upload/verified read (test execution 0.29 seconds).
The one CLI preflight build compiled the CLI and server in 4 minutes 51 seconds;
later named CLI/server `cargo check` completed in 1.6 seconds. A configured
Bazel leaf `cquery` completed in 3.4 seconds with the required nightly channel
flag; its closure contains the new protoc producer and no Slug graph crate.
Cargo/Bazel locks were refreshed. The two older fixture attempts stopped
before REAPI for the typed-Spawn and rules_python loader reasons above.
Independent final review returned `ACCEPT` after focused digest-safety and
malformed-response proof corrections. This receipt does not prove a Bazel
Rustc build or the refreshed CLI-root generated-input/coverage audit.
