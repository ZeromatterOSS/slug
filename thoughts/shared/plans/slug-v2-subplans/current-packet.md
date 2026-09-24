# Current Slug V2 Work Packet

Packet: WP-7-90-m7a-selected-extension-generator-acquisition-r1
Status: accepted; independent design and final reviews ACCEPT

## Outcome and compatibility

Advance the authentic root `cquery` through the selected `rules_rust` 0.73.0
`crate` extension's immediate `module_ctx.path("cargo-bazel")` and
checksum-pinned `module_ctx.download(url=..., output=that_path,
executable=True, sha256=...)` pair. Acquire and verify the generator bytes
through a tracked DICE Need/replay effect, then report the next honest root
terminal. This packet does not claim complete extension execution, configured
root analysis, action execution, or M7A closure.

Exact: the admitted call's SHA-256 verification, downloaded bytes, success
versus checksum failure, and executable intent; the normalized lowercase
`sha256` and SHA-256 SRI result fields if exposed to Starlark. Slug-native:
an owner-scoped logical scratch path and its text, per-request-bounded verified-content
retention, structural request identity, and DICE invalidation/restoration.
Unsupported/deferred: absolute/parent-traversing paths, label inputs,
path child/basename/dirname operations, unchecksummed or multi-URL download,
other digest forms, `allow_fail`, nonblocking download, authentication, canonical_id,
archive extraction, physical path reads/watch/file/execute and full Bazel
extension-working-directory bytes. Reject unmodeled inputs before effect or
publication; do not silently widen the admitted call family.

## Evidence and semantic owner

WP789 at `83de69964` passes the real selected extension's declared Host
factors; the fresh root now stops at `extensions.bzl:851` because `module_ctx`
lacks `path`. The BCR-resolved rules_rust 0.73.0
`crate_universe/extensions.bzl:839-864` constructs the generator path and
immediately downloads it with `executable=True` and the host-triple SHA when
present. On ordinary x86_64 Linux its `private/urls.bzl:23,42` selects the
published musl artifact and SHA-256
`482b82fc521cf3bbc5b4b3946198c0f273608ee63f1691c54c2e7efc3bece656`;
the public artifact HEAD returned HTTP 200 and 19,234,336 bytes, below the
proposed 32 MiB cap. The source can omit SHA for an override, but this packet
rejects that branch. A later source call may require a Label path or `watch`;
only a fresh smoke establishes the actual next terminal.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`:
`StarlarkBaseExternalContext.java:1545-1578` constructs `path(String)` under
the extension workdir without observing a file;
`RegularRunnableExtension.java:341-345` and
`ModuleExtensionContext.java:97-103` own the ephemeral workdir;
`StarlarkBaseExternalContext.java:541-560,637-872` owns synchronous download,
output confinement, checksum validation, executable setting and result;
`HttpUtils.java` and `starlark_repository_test.sh:1104-1232` cover URL,
download bytes/mode and returned SHA. No JVM code enters Slug. The existing
Core BCR archive capture handles HTTPS/file URLs, bounded redirects, SHA-256,
size and cancellation (`repository_archive_http.rs:360-495`); adapt this
transport as a neutral raw-artifact leaf, not as a pseudo repository.

`HostSelectedExtensionOwnerPureKey` and its observed variant remain the
semantic invocation owners. A new Bzlmod typed request identifies workspace,
structural selected owner, normalized relative scratch output, one HTTPS/file
URL, exact 32-byte SHA-256 and executable bit. The selected implementation
may create an invocation-heap owner-scoped path value, but may only consume a
path from that same invocation and a confined relative output. Synchronous
Starlark evaluation records a typed Need, not a direct network request. A
Need returns a non-cacheable owner outcome without selected invocation prints,
repository-rule calls or generated-repository publication. Core drops the
DICE transaction, captures and verifies the artifact, injects the request's
result, then retries the owner. Never compute the downstream selected-owner
repository certificate from inside the owner (it would cycle).

Before synchronous `eval_function`, an owner already authorized by the
command frontier computes its injected per-selected-owner DICE download
epoch. A newly selected owner absent from the cold frontier skips that cell
read and uses an empty invocation-local projection; its first download call
emits Need, so Core authorizes/captures it before replay. An authorized
frontier paired with an `Unauthorized` cell is an error, not a cold miss.
The epoch holds exact request to
verified immutable `Arc<[u8]>` entries; the invocation-heap context receives
only the authenticated projection. Its `download` method looks up the full
request and, on a miss, records a typed Need in invocation scratch and returns
a sentinel Starlark error. The owner checks that Need before translating an
evaluation error or storing prints/calls. Frontier authorization is checked
before computing/using the epoch; a mismatched request or unauthorized result
is an error without revealing bytes. A separate used-download request DICE
key, computed by the owner only for actually called requests after invocation,
provides exact metadata to the terminal demand selector. Selection prunes
preflighted but unused content; the accepted snapshot retains only selected
bytes. A successful call is marked as used even if a later part of that
invocation errors; a merely authorized but uncalled request is never marked.
Only one request is admitted per selected owner per attempt and accepted
snapshot; a second distinct output/request in one invocation fails closed,
while a changed request on a later revision replaces the prior slot. On
replacement, deselection and aborted-attempt restoration Core
injects a changed owner epoch (including empty/unauthorized), revoking stale
bytes. A warm DICE owner may be reused only while the accepted snapshot still
owns the same verified content; a new command never inherits an orphaned
temporary file. Keep request/result equality structural and collision-safe,
including actual bytes; SHA-256 is integrity, not semantic identity. Bytes
are DICE-retained semantic memory, shared by `Arc`, bounded to 32 MiB per
request; temporary capture files/buffers are transfer-owned scratch, released
on success/error/cancellation. No global cache, interner, unbounded byte
clone or donor import is authorized. The accepted snapshot prunes deselected
owners; older DICE revisions and in-flight attempts may still hold their
`Arc` bytes until those owners are released. No global RSS cap or
memory-reduction claim is made.

Use the existing command revision, accepted-snapshot, and restoration
protocol. A changed URL, checksum, output, executable bit, owner or Host
factor must rerun the selected owner; A→B→A may reuse a valid prior semantic
result only if its verified content is still owned, otherwise it reacquires.
Conflicting requests for the same owner/output in one attempt fail closed.
An inactive session or checksum/size/transport failure leaves no accepted
artifact, Starlark events or partial output. The logical path cannot be
passed to physical consumers until a later packet adds an authenticated
materialization and watch/execute bridge.

## Scope, stops and validation

Base clean `main` at `83de69964`. Root owns integration, serial Cargo,
fresh CLI rebuild/smoke, reviews and commit. Production/test allowlist:
`app/slug_bzlmod_v2/src/source_preparation.rs`, a new
`app/slug_bzlmod_v2/src/selected_extension_download.rs`,
`app/slug_bzlmod_v2/src/lib.rs`,
`app/slug_loading_v2/src/module_extension.rs`,
`app/slug_core_v2/src/runtime/dice.rs`,
`app/slug_core_v2/src/runtime/demands.rs`,
`app/slug_core_v2/src/runtime/repository_archive_http.rs`, and directly
colocated tests, plus the existing generated-repository-definition test
transaction helper in `app/slug_core_v2/src/runtime/generated_repository_definition.rs`
to install an explicit empty download transaction. Scheduling allowlist: this manifest, the canonical Current
packet paragraph, and the matching Stage 9 reuse row if retained bytes need
a recorded disposition. No repository-rule semantic reuse, root MODULE or
lockfile edit, generated repository, action/REAPI, or transport dependency
change. The existing large Loading owner and Core command loop remain the
cohesive lifecycle sites; isolate the new typed Bzlmod value and raw capture
leaf, and avoid a general extension effect dispatcher. `demands.rs` is the
existing terminal-closure selector, so the new descriptor belongs there.
If implementation would enlarge another file or shared API, return for a
design correction.

Obtain independent design ACCEPT before Rust changes and independent final
review of the actual diff. Focused tests must exercise a real selected
external `.bzl` route: path owner/output confinement, typed Need with zero
selected-owner events, file-URL fixture byte/sha/executable result and replay,
same-request warm reuse, changed request and A→B→A, cell mismatch and stale
authorization, second-output rejection, same-DICE deselect/reselect,
checksum/oversize/unsupported-option failure, cancellation or abort
restoration, and no partial physical file or repository publication.
Use a small deterministic local artifact and exact digest; the real 19 MiB
GitHub artifact is an integration smoke, not the portable acceptance oracle.
The pinned Bazel source and existing upstream tests establish the admitted
API; add a new Bazel oracle only for a demonstrated gap. The fresh root smoke
must pass both old path/download stops and report the next terminal without
claiming success; preserve ordinary `~/.bazelrc` RC discovery but never
inspect, copy or print its authentication contents.

Stop and `REPLAN` if the authentic request has no SHA-256, needs auth or a
non-HTTPS/file transport, exceeds 32 MiB, requires a physical path consumer
before the bytes can be owned, or accepted-snapshot/revocation cannot make
warm DICE reuse sound. Diagnose normal compiler/fixture/transport invocation
failures within the accepted contract. Resolve pinned nightly, compile
affected crates offline/locked with Cargo JSON `--no-run`, preflight exact
nonignored selectors, run focused owner and direct dependent tests, then
`cargo build -p slug_cli_v2` before fresh smoke. Clean stale `slugd` before
and after smoke. Run `cargo fmt --all -- --check`, `git diff --check`,
`scripts/v2_archive_status.sh`, and `python3 scripts/v2_plan_status.py`.

## Immediate predecessor

WP789 at `83de69964` admitted tracked declared extension Host inputs and
advanced the authentic root to this path effect.

## Validation receipt (accepted)

Independent design review ACCEPTED the bounded Need/preflight/selected-closure
contract before Rust edits. Pinned-nightly, offline/locked JSON no-run compiled
the Bzlmod, Loading and Core library test binaries; exact nonignored selector
preflight passed for five Bzlmod, two Loading, four Core transport, one Core
native download lifecycle and one existing Core Need/retry handshake tests.
They passed 5/5, 2/2, 4/4, 1/1 and 1/1 respectively. The lifecycle test
exercised a real selected-extension owner identity, cold Need and verified
file capture, selected acceptance, warm reuse, A→B→A recapture,
same-DICE deselection/reselection Need and abort restoration. The fresh CLI
build passed. Its authentic root `cquery` (exit 2) passed the path/download
calls and stopped later at rules_rust `extensions.bzl:1129`, where Slug's
Starlark currently rejects `list + list`. This is a separate deferred
language behavior, not configured-root success. No stale `slugd` was present
before or after. `cargo fmt --all -- --check`, `git diff --check`, archive
status and plan status passed. Independent final review ACCEPTED the actual
diff, bounded ownership and honest successor boundary. No global memory or
configured-root/M7A closure claim is made.
