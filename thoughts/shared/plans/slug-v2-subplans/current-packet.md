# Current Slug V2 Work Packet

Packet: WP-5-7A-authentic-payload-recipe-audit-r1

Status: source/docs only. No Rust, fixture/source-input writes, acquisition,
tests, CLI, daemon, source execution or replay.

## Observable result and learned boundary

Bind the 27 source descriptors in the verified upstream empty-root catalog to
their declared archive/patch/overlay payloads and existing MODULE metadata.
Report a hash-verified local recipe or the first evidenced missing/unsupported/
over-budget input. This is a finite candidate input catalog, not proof of Slug
selection, runtime support, complete module-extension inputs or CLI closure.

Predecessor WP-5-7A-authentic-registry-catalog-audit-r1 verifies all184 metadata
rows:156 MODULE.bazel,27 source.json,1 bazel_registry.json;276705bytes read,
184 Bazel CAS hits/184 paths, no fallback. Stage5 owns the reproducible locator/
ledger and evidence. No payload content was followed. Complete R2 is unaccepted.

## Pinned source and read scope

Bazel9.2 object8220c6198837d5c13d53fea211cf3282aa12408a in /home/wgray/bazel:
src/test/tools/bzlmod/MODULE.bazel.lock,50706bytes,SHA-256
d7cbba1d746f5522d7dde4a2f7ea7a24d8f0befdf23d7cb4984689b48781049a.
Its empty-root generator provenance is already recorded in Stage5; do not run it.
Use only its27 source.json registryFileHashes rows, matching MODULE rows and
registry config. Do not walk the156 MODULE dependency bodies or extension graph.
Reuse pinned IndexRegistry.java/IndexRegistryTest file-mirror/MODULE/repo-spec
anchors and HttpConnector.localFileDownload evidence in Stage5. Slug read scope:
app/slug_bzlmod_v2/src/{registry.rs,selected_repo_spec.rs} descriptor projection;
at most3 additional files located by targeted app search solely to resolve an
exact existing Slug download-cache filename rule, not production cache authority.
Read only preserved R2 CLI setup/sentinel/request hunks and adjacent validation
if needed to bind eventual request-policy obligations; never restore it.

## Deterministic procedure and caps

1. Recheck the pinned catalog and each of its27 descriptor hashes before parsing.
   Read descriptors in bytewise registry-URL order. Record source kind, original
   archive URL/SRI/type/strip prefix, ordered patches/strip and overlays/path/SRI,
   and matching authentic registry MODULE hash. Unknown/ambiguous projection,
   missing matching metadata or undeclared integrity is a concrete stop, not a
   reason to fabricate a descriptor, substitute a version or add a semantic fix.
2. Follow only payloads explicitly named by those descriptors: at most27 archives
   and128 total distinct payload objects, including patches/overlays. Before any
   payload reading, freeze its expected digest, original URL, role and size cap.
   Primary locator is exactly
   /home/wgray/.cache/bazel/_bazel_wgray/cache/repos/v1/content_addressable/sha256/<hash>/file.
   For absent primary only, permit one exact already-evidenced Slug download or
   registry URL-path counterpart; derive its filename from source, never scan.
   At most256 payload locator attempts; a present mismatch/unsafe object stops
   without fallback. Unresolved locator is not proof of global unavailability.
3. Stat/pin regular objects before bounded streaming; check descriptor identity,
   reject symlink escapes/nonregular files and size/race errors. Retain at most
   64KiB payload scratch. Caps: archive128MiB,patch8MiB,overlay64MiB; metadata1MiB
   each/4MiB total; payload256MiB aggregate, counted across all actual reads.
   Preflight remaining budget and bound each read to it; stop before exceeding.
   SHA-256/SRI comparison must succeed at EOF. No archive extraction/listing,
   transform application, module evaluation, repository execution or new payload
   dependencies beyond these descriptors. Unknown integrity algorithms stop.
4. Bind verified rows to intended explicit file-registry/mirror projections from
   the accepted transport contract, preserving authentic descriptor/MODULE bytes.
   Report counts/bytes and compact reproducible bindings, not a copied full tree.
   No URLs or digests may be guessed; no credential/config secret inspection.
   An incomplete prefix is not a complete recipe. Even complete payload presence
   does not prove this catalog covers Slug discovery or extension execution.
5. Select the next concrete bounded step based on the result: first missing/
   unsupported input only, or an explicit selection/fixture-proof route review.
   A future fixture packet must remove fake-platform bodies/override, carry the
   same explicit registry/mirror policy to direct-core sentinel_outputs and every
   CLI request, retain real generated host_platform/builtin sources, and freeze
   isolation, provenance, cleanup and actual command/result/time/pipe gates.
   Do not assemble fixtures or authorize runtime/R2 restoration in this audit.

## Ownership, classification and exclusions

Exact/Slug-native/deferred boundaries remain unchanged. Existing selected
RepoSpec/immutable requests, SRI capture, Host observations and source-certificate
publication own semantics. Audit locators are evidence only, never production
cache discovery, a new key, retained cache, lock, request overlay or fallback.
Memory is command scratch; no retained/async lifetime changes. No oracle fixture,
DICE representation, Buck2 donor, hot-path or source-file complexity change;
those implementation gates are inapplicable to source-only availability checks.
No runtime-success/parity claim follows from file presence or source reading.

## Files, validation and stops

Writable: this manifest, canonical Live Status, relevant Stage5/6 status; routing
only for REPLAN. <=120 added doc lines outside manifest; PROGRESS.md <=500 lines.
All sources/caches/candidates read-only. Every audit command timeout60; any overrun
requires investigation, no automatic extension/retry. No Cargo/test/binary/Bazel/
network acquisition. Future tests/compiler commands start timeout60, serialized;
over one minute is a red flag and fifteen minutes the absolute maximum.
Source/structure/hash, git diff --check, preserved R2 hash/apply checks and archive
checker (only known3 thoughts paths). Independent terminal review; commit/push
accepted findings and exact successor. First unsafe/missing/mismatch/size/count/
authority stop is reported honestly; unbounded scope or second material correction
is REPLAN. Never inspect/print/copy ~/.bazelrc or derived secrets.

Complete unaccepted R2: /tmp/slug-conflict-r2.XZJWwv/candidate.patch,
base97dffd5d4,SHA-256
90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e.
Adjacent validation.txt owns passed/failed/unrun gates. No partial shipping.
Real one-shot/stable-daemon conflicts, common execution-view/REAPI sharing and
complete relevant gates remain open. No fake sources, local_repository/version/
source overrides, runtime bypass or checkout-wide/bounded replay.
