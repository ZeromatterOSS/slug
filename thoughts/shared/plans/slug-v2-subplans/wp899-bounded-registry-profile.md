# WP899 bounded registry profile amendment

Baseline: `317e5f116`. Status: component validated; independent DESIGN and FINAL accepted. Parent packet remains
WP-7-199-m7a-request-selected-source-profile-r1; full WP899 remains unaccepted.

Global7 proved 128 distinct normal registry demands and refusal of distinct129,
with no repeats. That is a lower bound, not full closure. The operational policy
is Slug-native; normal Bazel-compatible discovery, version selection, structural
DICE identity, registry IO and source authority remain unchanged.

Choose an explicit actual-root diagnostic profile of 256 registry attempts.
Keep Default at128 and make256 the finite validation ceiling for this one counter.
All other validation ceilings, byte limits,16MiB live bank, HTTP steps and request
deadlines remain unchanged. 256 is a bounded next capacity, not a promised fit;
the frozen192-entry lock inventory is context, not a demand oracle. No retry is
part of this amendment. Any actual observation needs its own reviewed freeze.

SourceServiceLimits owns immutable operational policy before Scope/DICE creation.
The existing Scope locked charge owns inclusive admission and first poison. A
Linux test-only `actual_root` constructor selects the profile in the genuine-root
harness; a numeric receipt field exposes its configured registry allowance. No
production command activation, semantic key/value, cache, bypass or new input.

The optional association keeps its existing exact-byte comparison and fixed row
metadata, increasing the metadata ceiling to256. Allocate its slab for the
immutable Scope allowance (allowance * existing16384-byte row) rather than always
allocating4MiB. Infer row capacity from slab length; no second mutable capacity.
Charge the entire slab,256-row headers, construction overlap and timeout POD
before allocation in the same16MiB bank. Default slab stays2MiB; actual profile
uses4MiB; added metadata is charged. Credit pressure can truthfully refuse.
Existing Weak attachment, pointer disarm, clone/history and last-Drop release
remain. This is diagnostic command-retained storage, never source authority.
No new donor/cache/interner/container is needed; Stage9 diagnostic accounting row
records this bounded amendment. No performance or total RSS improvement claim.

Allowlist: scope.rs, source_service/tests.rs, registry_association.rs and its
tests.rs, source_preparation/genuine_root.rs; this contract, compact receipt,
matching Stage9 row and current manifest/canonical status. Root owns all edits,
serial Cargo, validation and preservation; reviewers are read-only.

Discriminators: default128 and explicit256, inclusive/zero/one-over validation;
unchanged other ceilings; 256 distinct normal local adapter reads then refused257
with no source open/bytes, exact association and sticky first poison; proportional
slab credit/default versus profile and clone last-Drop release. Reuse protected
association, Scope, timeout/lifecycle and root projection controls on rebuilt
artifact. No extra slow TLS run unless its relevant implementation changes.

Compile pinned nightly separately under60s, preflight exact selectors once per
artifact, execute bounded focused groups under30s. Default Core+CLI all-target
check, changed Rust formatting, archive and diff checks; independent DESIGN before
source edits and FINAL after evidence. No test growth elsewhere, new network
fixture, Windows implementation, supervisor change or broad suite repetition.
Estimated delta under180 Rust lines; actual net209 lines reflects expanded boundary
coverage (75 limit-control lines and116 association-control lines). Independent
FINAL accepts the cohesive existing owners; no production responsibility moved.

REPLAN if finite profile requires semantic skipping, larger byte/bank/time limits,
different source authority or broader command activation. A failed actual-root
admission remains evidence rather than acceptance. Preserve on review branch;
no partial main integration or push.
