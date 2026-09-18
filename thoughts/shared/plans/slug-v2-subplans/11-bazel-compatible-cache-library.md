# Stage 11: Bazel-Compatible Cache Library

## Outcome and scheduling

Provide a linkable Rust library through which Slug and external applications
share Bazel-compatible remote Action Cache/CAS services and a supported Bazel
`--disk_cache` directory on Linux.

Canonical Live Status selects work. M7A establishes the reusable core while
implementing only transfers and cache operations demanded by Slug's production
closure. M8 proves Linux self-hosting through that core. The standalone remote
and disk library release follows M8 and precedes M7B mixed-language repository
breadth. C/C++ ABI and Python bindings are subsequent consumer-driven work;
they do not gate self-hosting or the first Rust library release.

This is a subsystem contract, not an active implementation packet. Each selected
slice names its concrete files, protocol pins, evidence, commands and resource
limits under the plan-authoring guide. The active configured fixture packet and
the requested-root conflict prerequisite retain their own acceptance gates.

## Compatibility surfaces

| Surface | Contract and acceptance |
|---|---|
| Remote cache service interoperability | Exact admitted REAPI AC/CAS and ByteStream operations against the pinned protocol; gRPC/TLS first, SHA-256 initially. A caller supplies an Action digest and result/blob data without running Slug or an executor. |
| Bazel disk-cache interoperability | Exact read/write compatibility with the pinned Bazel 9.2 `--disk_cache` layout and ActionResult encoding, proved in both directions. The support matrix records platform, digest function and Bazel revision. |
| Independent Bazel/Slug action-result reuse | Separate deferred per-action claim requiring identical encoded Action, Command and input-tree bytes under aligned toolchain/path/environment/platform policy. Sharing a service or directory does not establish this claim. |
| Library API and cache policy | Slug-native Rust types, error model, caller-owned runtime and explicit backend policy; no public stability promise until a release contract is selected. |

CAS blobs are addressed by content digest; AC entries by REAPI Action digest.
Preserve structural Slug identity, path/display tokens, Bazel configuration
checksums, Bazel aquery ActionKeys and REAPI/CAS digests as distinct domains.
An exact aquery ActionKey is not required for remote or disk-cache access.

Bazel's internal incremental action-cache database and repository download cache
are outside this library's first release. HTTP cache transport, additional
digest functions, compression, remote execution APIs, cache servers, automatic
cross-tool action alignment and language bindings are separately admitted
extensions. Unsupported protocol features fail explicitly.

## Owners and dependency boundary

```text
Slug DICE/configured actions
  -> Slug REAPI projection and execution orchestration
      -> reusable protocol / CAS / Action Cache library
External Rust application
      -> reusable protocol / CAS / Action Cache library
```

The cache library has no dependency on Slug analysis, DICE, workspace, command,
CLI or daemon crates. It owns protocol bindings, digest validation, transport,
blob transfer, AC lookup/publication and disk encoding. It accepts immutable
protocol values, bytes, or explicit streaming blob sources. It neither discovers
workspace inputs nor lowers language rules nor decides to execute a cache miss.

Stage 6/Core owns semantic inputs and source certificates. Stage 7 lowers its
validated requested-root closure, owns Execute scheduling and decides whether
and where an admitted miss can execute. Slug's materializer owns requested paths,
provisional output publication and final source validation. Generic library
download helpers verify content and reject unsafe paths; they cannot publish a
successful Slug build or bypass its closure checks.

Use `app/slug_reapi_v2/src/{digest,proto,cas,action_cache,executor}.rs` as local
extraction inputs. Its `slug_core_v2`/`slug_build_api_v2` adapters belong above
the library boundary. Select a cohesive leaf crate during the M7A execution
packet and route Slug through it. Avoid duplicate cache implementations. Keep
Cargo and Bazel dependency declarations, generated bindings, licenses and the
Stage 10 production inventory synchronized.

WP-7-13 selects `app/slug_reapi_cache_v2` for this leaf. Its request-local
`CacheClient` accepts a caller-owned tonic channel and instance, returns the
full protocol ActionResult on AC hit, and handles verified CAS upload/download
with bounded batch or ByteStream transfers. The `slug_reapi_v2` adapter retains
graph-dependent input trees, action lowering, Execute and publication. The
upstream pins and copied-file checksums are in the leaf's `PROVENANCE.md`.
The WP-7-13 bounded leaf passed independent final review after canonical
digest, malformed CAS response, interrupted Write-status and cross-domain
ActionKey proof corrections. General cache-only writes and standalone release
remain the later slices below.

WP-7-29 adds verified AsyncRead uploads without retaining a complete ReapiBlob.
The caller retains source/provenance and missing-blob policy; the leaf owns a
bounded channel, SHA-256 verification and ByteStream completion. It drops the
reader on cancellation or early RPC completion and publishes success only after
local verification and exact committed size. Resumption and compression remain
unadmitted; interrupted RPC status is diagnostic only.

## Implementation slices

### 11.1 Bootstrap cache core — M7A/M8

- Pin authentic upstream REAPI and Google ByteStream/dependency definitions with
  provenance and licenses; generate bindings reproducibly. Protocol expansion
  preserves the accepted FileWrite wire/content/digest proofs.
- Extract the operations demanded by bootstrap and their tests. Prove the leaf
  dependency closure contains no Slug graph crates and its public API does not
  expose graph types. Full external packaging is not an M7A gate.
- Support capability-aware bounded batching and ByteStream transfers where
  demanded by compiler/toolchain artifacts. Verify digest and size, chunked and
  interrupted transfers, backend limits and generated-input reuse.
- Keep canonical protocol ordering and encoding explicit. Transport retry,
  endpoint, authentication and RPC deadline policy do not salt Action bytes.
- Reuse Stage 7's cold execution, warm replay, restart, missing/corrupt output,
  cancellation and zero-direct-local proofs for the admitted bootstrap slice.

### 11.2 Standalone remote cache client — after M8

- Provide CAS missing-blob discovery, upload and verified download; AC lookup
  and UpdateActionResult publication without requiring an execution endpoint.
  A cache-only miss returns a typed miss to the caller.
- Admit full result shapes needed for general artifact caching: exit status,
  files, output directories/Trees, symlinks, executable bits and inline/digested
  stdout/stderr. Preserve admitted protobuf fields through round trips. Do not
  silently drop unmodeled result fields and republish an incomplete entry.
- Verify all required referenced CAS data. Publish referenced blobs before an
  AC entry; distinguish missing/stale data, corruption, permission/authentication,
  unsupported capability, cancellation and transport failure. Honor explicit
  read/write and action cacheability policy.
- Scope lookup, transfer coalescing and any memoization by backend authority,
  instance, digest function and applicable authorization/policy. Credentials
  come from caller configuration and never enter evidence or persistent keys.
- Supply one standalone Rust consumer with no Slug graph/CLI dependency and
  documentation for cache-only lookup, publication and miss handling.

### 11.3 Direct Bazel disk cache — after 11.2

- Implement the pinned Bazel disk layout, digest-function namespace, AC record
  encoding and CAS storage beneath an explicitly supplied cache root. A private
  SQLite index is not the shared format or proof of compatibility.
- Audit atomic temporary-file publication, concurrent writers, existing corrupt
  blobs, timestamp/eviction interaction and disappearance during reads against
  the pinned disk client. Missing data during eviction is recoverable; partial
  or corrupt records cannot count as a successful hit.
- Publish only verified CAS content; validate AC references before successful
  materialization. Safe output extraction checks path containment and symlink
  handling, including traversal and collision negatives.
- Prove Bazel-written entries are read by the library, and library-written
  entries are reused by Bazel from isolated output bases. Use a controlled action
  with identical bytes and record action digests and execution/hit counts.
- Document independent remote and disk configurations. Automatic multi-tier
  fallback/promotion and library-owned garbage collection require separate policy
  and evidence; direct read/write interoperability does not depend on them.

## Oracle and evidence

Pin Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`.
Source anchors relative to that repository are:

- `third_party/remoteapis/build/bazel/remote/execution/v2/remote_execution.proto`
  for Action/Command/Directory/Tree/ActionResult and AC/CAS/Capabilities;
- `src/main/java/com/google/devtools/build/lib/remote/GrpcCacheClient.java`
  and its owning tests for transport/cache behavior;
- `src/main/java/com/google/devtools/build/lib/remote/disk/DiskCacheClient.java`
  and `src/test/java/com/google/devtools/build/lib/remote/disk/DiskCacheClientTest.java`
  for direct disk interoperability; and
- `src/test/shell/bazel/remote/` for selected cache-policy and replay fixtures.

An implementation packet verifies these pinned paths and selects exact upstream
test methods for its admitted slice. Stage 1 owns repository-owned fixtures and
provenance. Existing Slug FileWrite/protobuf/replay evidence remains regression
coverage; it does not establish external consumer or direct disk compatibility.

The standalone release requires bidirectional remote and disk evidence, a
cache-only client with zero Execute calls, digest/corruption/stale-entry
negatives, an external consumer compile/run proof and a documented support
matrix. Unknown-field handling and large-artifact transfer behavior need explicit
tests. Independent Bazel/Slug digest equality is tested separately from protocol
interoperability and is not a release prerequisite.

## Lifetime, scope and completion

Clients and connection pools are service-owned; immutable operation policy is
request-owned. Buffers and streams are bounded transfer-owned memory. Every
background task has cancellation/join ownership, and one canceled subscriber
cannot discard another subscriber's coalesced work. No task borrows expired
command scratch. No library cache becomes a second semantic graph. Release
buffers, temporary files and task handles on success, error and cancellation.

Implementation packets run focused owner/direct-consumer checks and selected
backend evidence under the canonical runtime/preparation limits. They name any
larger acceptance operation and resolve its resource requirements before running
it. No calendar completion estimate is inferred from bounded FileWrite evidence.

Complete 11.1 with the demanded bootstrap proofs and M8 fixed point. Complete
the standalone library milestone only when 11.2 and 11.3 pass their external
consumer and bidirectional interoperability gates. Missing source semantics,
uncertain wire/disk provenance, incomplete result preservation or an unsafe
publication contract is a concrete prerequisite to resolve within the owning
packet. Mixed-language repository support remains Stage 8 work after this release.
