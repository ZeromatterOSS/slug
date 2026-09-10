# Current Slug V2 Work Packet

Packet: WP-5-7A-authentic-registry-catalog-audit-r1

Status: source-only successor to the authentic-input recipe audit's scope REPLAN.
No Rust, fixture/source-input edits, acquisition, tests, CLI, daemon or replay.

## Observable result and learned boundary

Verify availability and SHA-256 of the exact 184 registry metadata objects in
pinned Bazel's empty-workspace default lockfile, or report the first unavailable,
mismatching or over-budget object. This bounded metadata catalog is upstream
evidence, not an exact Slug-selected graph, a complete CLI recipe, or authority
to inject a lockfile/copy every catalog row into a fixture.

Predecessor WP-5-7A-configured-cli-authentic-input-recipe-r1 stopped at its12
additional-module-version ceiling. No missing object or unsupported owner was
established. Several absent Slug-cache paths had verified Bazel CAS counterparts.
Stage5 records authentic patch/payload findings and the direct-core sentinel/
CLI request-policy obligation. Complete preserved R2 remains unaccepted.

## Exact read scope and procedure

Authority: Bazel9.2 object8220c6198837d5c13d53fea211cf3282aa12408a in
/home/wgray/bazel. Read via git show, never execute:
src/test/tools/bzlmod/update_default_lock_file.sh, SHA-256
2b91d85d3e012ea77392ca53b58df7d139209b25b15572dd16a94252b4ab249c;
src/test/tools/bzlmod/MODULE.bazel.lock, 50706bytes, SHA-256
d7cbba1d746f5522d7dde4a2f7ea7a24d8f0befdf23d7cb4984689b48781049a.
The generator creates an empty MODULE root and invokes external Bazel mod deps;
it is provenance only. Do not rerun it or substitute Bazel's own root-project lock.

1. Verify these two pins, then parse only registryFileHashes: exactly184 rows,
   all https://bcr.bazel.build URLs and64-hex SHA-256;156 MODULE.bazel,
   27 source.json and1 bazel_registry.json. Unexpected shape/count is a stop.
2. Visit rows in bytewise URL order, not dependency order. Primary explicit path:
   /home/wgray/.cache/bazel/_bazel_wgray/cache/repos/v1/content_addressable/sha256/
   <expected-hash>/file. If absent, try only the exact URL-path counterpart under
   /home/wgray/.cache/slug/registry/bcr.bazel.build/. Never enumerate caches.
   Validate URL components before path projection; no dot/parent components.
   A fallback is merely an audit locator, never production/request policy.
3. Accept only regular local objects; stat then bounded stream/hash, retaining
   at most64KiB scratch. Cap1MiB/object,32MiB total bytes read,368 candidate paths.
   Stop on first digest mismatch, nonregular/unsafe object, both-path absence,
   size/count cap or command timeout. Do not silently try alternatives after a
   mismatching existing object. No symlink traversal outside the two cache roots.
   Read/hash only; no module evaluation, dependency recursion or source execution.
4. Record exact verified counts/bytes and a compact reproducible locator rule.
   Bind any stop to URL, expected hash, attempted paths and actual observation;
   distinguish unavailable from uninspected. Do not dump184 rows into plans.
   No new archive, patch, overlay, module-extension or descriptor-SRI payload
   traversal is allowed in this packet, even if all184 metadata rows verify.
5. If complete, choose one bounded source-only descriptor/payload recipe audit
   from this exact catalog plus the preserved CLI roots; freeze its object/byte
   caps and selection uncertainty before starting it. Otherwise choose only the
   first evidenced prerequisite or request missing authority. Do not invent a
   semantic fix or infer missing bytes from only one cache's absence.

## Classification and ownership

Exact/Slug-native/deferred classes are unchanged; no runtime/parity claim.
Reuse Stage5's pinned IndexRegistry file-mirror/MODULE and HttpConnector source/
tests for later recipe projection. No new oracle is needed for metadata hashes.
Current selected RepoSpec/immutable requests, SRI capture, Host observations and
source-certificate publication remain semantic owners; no production cache
locator, key, equality, request overlap, lock, async task or retained value change.
Audit buffers are command scratch; no persistent inventory or fallback is added.
Buck2/Stage9 donor, fixture growth, runtime lifecycle and hot-path gates are
inapplicable to read-only metadata verification. No source tree is copied.

## Files, validation and stops

Writable: this manifest, canonical Live Status, relevant Stage5/6 status; routing
only for REPLAN. <=120 added doc lines outside manifest. PROGRESS.md <=500 lines.
Everything else is read-only, including caches and both preserved candidates.
Source/structure/hash, git diff --check, R2 forward-applicability and archive
checker (only known3 thoughts paths); no Cargo/test/binary/Bazel/network commands.
Bound every audit command at60seconds; any overrun requires investigation, not
automatic extension. Future tests/compiler commands start timeout60, serialized;
over one minute is a red flag, fifteen minutes is the absolute maximum.
Never inspect/print/copy ~/.bazelrc or derived secrets.
Independent terminal review for REPLAN, new reserved authority or milestone close.
Commit/push accepted findings with the exact successor. Unbounded inventory,
missing source authority or second material correction is REPLAN.

Complete unaccepted R2: /tmp/slug-conflict-r2.XZJWwv/candidate.patch,
base97dffd5d4, SHA-256
90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e.
Adjacent validation.txt owns all passed/failed/unrun gates. No partial restoration
or shipping. Real one-shot/stable-daemon conflicts, common execution-view/REAPI
sharing and complete relevant gates remain open. No fake platforms, stubs,
local_repository/version/source overrides, runtime bypass or broad replay.
