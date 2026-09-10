# Current Slug V2 Work Packet

Packet: WP-5-7A-abseil-payload-locator-audit-r1

Status: source-only successor to the payload audit's unresolved-locator stop.
No production, fixture/source writes, acquisition, tests, CLI, daemon or replay.

## Observable result and exact stop

Resolve one source-backed alternate Slug-cache locator for the first unverified
catalog payload, abseil-cpp20250814.1, and verify its bytes or record exact absence.
Do not claim that this catalog member is required by the CLI conflict fixture.

Predecessor WP-5-7A-authentic-payload-recipe-audit-r1 reverified27 descriptors,
8277bytes, declaring27 archives/20 ordered patches/0 overlays and27 matching
MODULE hashes. Payload walk stopped on its first exact Bazel CAS path: ENOENT,
1 locator attempt,0 verified payloads/0 payload bytes read. Targeted live app
search did not locate a Slug download-cache filename owner. No alternate path
was guessed or tried; neither global unavailability nor CLI necessity is proved.
Stage5 records the exact input and source-projection bindings. The preceding184
metadata gate remains accepted at6fb6749a1; complete R2 remains unaccepted.

## Source authority and read scope

Pinned Bazel9.2 object8220c6198837d5c13d53fea211cf3282aa12408a in /home/wgray/bazel
remains semantic authority. Default lock src/test/tools/bzlmod/MODULE.bazel.lock:
SHA-256 d7cbba1d746f5522d7dde4a2f7ea7a24d8f0befdf23d7cb4984689b48781049a.
Only this descriptor and matching MODULE row are relevant here:
https://bcr.bazel.build/modules/abseil-cpp/20250814.1/source.json
descriptor SHA-256 cea3901d7e299da7320700abbaafe57a65d039f10d0d7ea601c4a66938ea4b0c;
MODULE SHA-256 51f2312901470cdab0dbdf3b88c40cd21c62a7ed58a3de45b365ddc5b11bcab2.
Archive URL:
https://github.com/abseil/abseil-cpp/releases/download/20250814.1/abseil-cpp-20250814.1.tar.gz
SRI sha256-FpL3fRc5us8/lDNxiLeFg88JurfkINLcbFYFpPhnhaE=; strip abseil-cpp-20250814.1.
Archive SHA-256 1692f77d1739bacf3f94337188b78583cf09bab7e420d2dc6c5605a4f86785a1.

This packet explicitly permits read-only archive provenance, not V1 migration:
verify V1_ARCHIVE.md's ref e218054d4c796655939b968d90208b185decb352, then use
targeted git grep/ls-tree to locate only download-cache naming code in that ref.
Read at most3 source files/1MiB total through git show. No archive checkout,
restoration, code execution or adoption. V1 is locator evidence only; its
semantics/runtime/cache implementation are avoid for production reuse.

## Procedure, limits and next decision

1. Derive the exact filename algorithm from archived source, including integrity
   encoding, character substitutions and cache-root join. Bind source path/blob
   hash and lines. A matching-looking cache name is not sufficient authority.
   Compare the derived name with Stage5's already verified rules_shell0.6.1
   locator as a control; do not enumerate or rehash unrelated cache objects.
2. If derivation/control succeeds, freeze the single abseil alternate path under
   /home/wgray/.cache/slug/downloads before touching it. Inspect only that path
   and the already known Bazel CAS <archive-hash>/file path under
   /home/wgray/.cache/bazel/_bazel_wgray/cache/repos/v1/content_addressable/sha256.
   At most2 target locator attempts; no glob, cache scan, search by size or URL
   guess. A present mismatch/nonregular/escape is a stop, not fallback permission.
3. For a present regular target: safe pin/open identity and in-root path checks,
   preflight128MiB object/total payload-read cap,64KiB stream scratch, exact
   EOF/length/SHA-256 comparison. No extraction/listing/transform or source use.
   Metadata reads1MiB total; no other payloads followed. Every command timeout60;
   on timeout investigate, never automatically extend or retry.
4. If matching bytes exist, select bounded resumption of the47 declared-payload
   audit using the now evidenced locator rule and preserving its original caps.
   If both exact locators are absent, record scoped absence, not global absence,
   and select only a source-backed CLI demand/necessity review before proposing
   acquisition. If provenance cannot resolve a path, report missing authority.
   Do not equate full catalog availability with required CLI inputs or insist on
   downloading an unproved-needed archive to advance the conflict milestone.

## Classification, ownership and validation

Exact/Slug-native/deferred classes remain unchanged. Existing selected RepoSpec,
immutable requests, Host observations, verified SRI capture and source-certificate
publication remain owners. No production cache lookup, key, retained state, lock,
request revision or fallback changes. Read/hash scratch is command-local; no
async/retained-memory/fixture/representation/performance change. Reuse Stage5
IndexRegistry/HttpConnector source tests; no new oracle is relevant to a filename
provenance audit. No file-presence-based runtime/parity success claim.

Writable: this manifest, canonical Live Status, relevant Stage5/6 status; routing
only for REPLAN. <=120 added doc lines outside manifest; PROGRESS.md <=500 lines.
All source caches/candidates read-only. Source/hash/structure, git diff --check,
R2 hash/applicability and archive checker (only known3 thoughts paths).
Independent terminal review; commit/push accepted findings and exact successor.
Unbounded inventory, missing source authority or second material correction is
REPLAN. Future test/compiler commands start timeout60, serialized; over one minute
is a red flag, fifteen minutes absolute maximum. No tests/runtime/network here.
Never inspect/print/copy ~/.bazelrc or derived secrets.

Preserved complete R2: /tmp/slug-conflict-r2.XZJWwv/candidate.patch,base97dffd5d4,
SHA-256 90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e.
Adjacent validation.txt owns all gates. No partial restoration/shipping.
Actual one-shot/stable-daemon conflicts, common execution-view/REAPI sharing and
complete relevant gates remain open. No fake sources, local_repository/version/
source overrides, runtime bypass or checkout-wide/bounded replay.
