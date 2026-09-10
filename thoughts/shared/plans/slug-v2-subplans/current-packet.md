# Current Slug V2 Work Packet

Packet: WP-5-7A-selected-bcr-file-capture-design-r1

Status: reserved-boundary design selected after authentic-source audit REPLAN.
Docs/source only. No Rust, fixture, test/runtime, acquisition or replay authority.

## Observable result and learned boundary

Freeze an implementable, independently reviewed verified local-file capture
contract for the selected-BCR archive/MODULE/patch/overlay category, including
primary and mirror source URLs. Do not merely weaken a URL guard or deliver a
test-only cache bypass. This packet designs a new local-input/transport boundary;
implementation follows only when every safety/ownership question below is settled.

The accepted mode prerequisite0f45fab28 admits regular0640/directory0750
(2/80/82 gross additions, extractor11pass/1ignored/3.41s). It remains accepted.
Source audit then found: HyperRegistryIo reads file registries and Bzlmod retains
their MODULE URL/SRI, but core SelectedBcrArchive and native capture require HTTPS
for every payload. Local archive-backed registry modules therefore cannot supply
their file MODULE capture. This is a source-proved boundary, not a replay result.
Cached real platforms/rules_shell sources do not establish a full CLI closure.

Read Stage5's "Authentic-source audit stops at selected-BCR file capture" for
exact cached provenance, source hashes and producer anchors. Pinned Bazel9.2 is
/home/wgray/bazel git object8220c6198837d5c13d53fea211cf3282aa12408a.
IndexRegistry.java:245-255,448-538 and IndexRegistryTest.testFileUrl/
testGetArchiveRepoSpec establish local registry MODULE and file mirror projection.
HttpConnector.java:114-120 and HttpConnectorTest.localFileDownload establish
file payload transport. Preserve mirror order and existing checksum/transform
semantics; broad arbitrary schemes/URL forms are not implicitly accepted.

## Decisions required before implementation activation

1. Define one admitted file URL/path grammar and platform boundary for all four
   payload roles; reconcile Bzlmod primary source absoluteness with core parser
   and capture validation. Name unsupported authorities, credentials, ports,
   queries/fragments, Unicode/percent decoding and path forms explicitly from
   pinned evidence. Do not create divergent URL validators or silently broaden
   HTTPS/auth/redirect behavior. HTTP-to-file redirects are a separate decision.
2. Choose the existing natural capture/request owners and cohesive module split.
   Keep local file input separate from LocalTar override and local_repository.
   Preserve expected SRI, byte caps (archive128MiB/MODULE1MiB/patch8MiB/overlay64MiB),
   ordered mirror handling and exact successful capture. No production cache
   scanning, locator inference, dependency/source override or persistent side store.
3. Specify safe opening and reads of regular files: special-file blocking, symlink
   and path replacement races, descriptor identity, mutation during capture,
   finite buffers, overflow/size checks, cancellation and deterministic errors.
   No metadata-check-then-unbounded-blocking-open shortcut. Define unsupported
   OS/filesystem guarantees honestly; do not claim arbitrary historical snapshots.
4. Prove how verified immutable bytes enter existing AssociatedImmutable and Host
   final source-certificate publication. Existing request equality/digest and
   generation-scoped error retry may suffice, but source evidence must establish
   successful reuse, changed content, missing/create/delete/repair and overlapping
   sessions. Do not assume mutable paths equal content-addressed immutable inputs.
5. Keep file-only capture free of TLS initialization/DNS/network. Current public
   capture wrappers eagerly create NativeEnvironment; freeze a bounded lazy
   transport choice and its phase lifetime without a new service/global cache.
   Consult the utility skill/Stage9 only if proposing retained representation.
6. Freeze exact production/proof files, gross caps and a cohesion decision.
   Require tiny source-derived local-file red/green, byte/SRI mismatch, every
   payload cap, URI rejection, mirror ordering, no-network success, failure/
   cancellation cleanup and generation/recovery/publication proof. Reuse existing
   archive capture/extraction helpers and direct compile consumers, no full suite,
   giant fixture tree or external archive replay. All later commands timeout60;
   investigate over one minute, fifteen minutes absolute maximum, no longer retry.

Compatibility: pinned named local-file/mirror/verified-payload behavior may be
exact once the admitted subset is specified; Rust filesystem observations,
bounded failures, Unicode paths and structural identity remain Slug-native.
Everything not frozen stays unsupported/deferred. No new retained state/key,
fallback or ownership change is already authorized. If needed, justify it through
the plan-authoring checklist and independent reserved architecture review.

## Exact design scope and terminal gates

Writable docs: this manifest, canonical Live Status and relevant Stage5/6 owner
status; routing log only on REPLAN. <=180 added doc lines outside manifest.
Keep /home/wgray/PROGRESS.md <=500 lines. Read only named capture/source owners,
relevant existing helpers/tests and pinned sources needed for the six decisions.
No Rust/dependency/harness/fixture/cache/registry/source-input/vendored edits.
No tests, binary/CLI/Bazel/oracle/daemon/network commands, acquisition or replay.
Never inspect/print/copy ~/.bazelrc or derived secrets.
Validate source/structure, diff, candidate preservation and archive checker
(only known3 thoughts paths). Require independent terminal design review, then
commit/push accepted design and select its exact implementation contract.
An unresolved safety/identity owner, unbounded scope or second material
correction returns REPLAN; never substitute a fake-platform control for real CLI.

Complete unaccepted R2 remains /tmp/slug-conflict-r2.XZJWwv/candidate.patch,
base97dffd5d4, SHA-256
90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e.
Adjacent validation.txt owns all passed/failed/unrun gates. Do not restore or
partially ship R2 here. Actual one-shot/stable-daemon CLI and positive common
REAPI sharing remain open; only two fast core failures are baseline-attributed.
