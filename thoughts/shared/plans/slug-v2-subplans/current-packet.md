# Current Slug V2 Packet

Packet: `WP-4-5-6-host-generated-repository-definition-lookup-owner-implementation-r2`
Milestone: M7 canonical generated-repository definition lookup
Owners: `slug-v2-subplans/04-starlark-loading-and-build-packages.md` and
`slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: complete the accepted private canonical generated-definition lookup.

## Active implementation contract

Independent review accepts cap correction `99a5b898`. Run only
`WP-4-5-6-host-generated-repository-definition-lookup-owner-implementation-r2`
in new `app/slug_core_v2/src/runtime/generated_repository_definition.rs` and
existing `app/slug_core_v2/src/runtime/mod.rs` solely for the private module
declaration, plus the four ledgers. Mandatory caps are 260 production, 550
tests, and 800 total formatted net Rust lines against design `6678f54f`.

Implement the private callerless
`HostGeneratedRepositoryDefinitionKey { workspace, canonical_repo }` exactly
as frozen below: compute only the accepted hidden validation key; propagate
Need and opaque loading errors; completely scan the borrowed request/call
iterator; reject zero or duplicate canonical matches; retain only the full
certificate plus matched ordinal; and borrow canonical/internal names, the
original `RepoSpec`, and row mapping/context without copying a row, map, or
catalog. Preserve the complete field/order A/B/A, full-scan, lifecycle,
zero-event/source/materialization proof and every compatibility/stop boundary.
No third Rust file, server/loading/Bzlmod production edit, public export,
apparent route, execution, source/materializer I/O, lockfile, command/wire API,
or JVM work is authorized.

## Accepted r2 cap-correction contract

This section and everything below is historical context only, grants no file,
action, cap, or schedule authority, and is interpreted only through the active
implementation contract above.

The retained two-file implementation compiles and its focused proof passes,
but formatted accounting against design `6678f54f` is 222 production, 541
tests, and 763 total Rust lines. The 260 production cap holds; the 480 test and
740 total caps do not. The additional proof is structurally required: it
proves complete iterator exhaustion after a duplicate, field-specific
canonical/internal/RepoRuleId/attribute/Label/mapping/context A/B/A,
request/call and mapping order, and zero source/materialization activation on
the warmed lookup. Removing 61 test lines would weaken the frozen matrix or
duplicate a private loading fixture through a forbidden third seam.

Run only
`WP-4-5-6-host-generated-repository-definition-lookup-owner-r2-cap-design`
in the four ledgers. Authorize no Rust action or commit. Retain the unaccepted
two-file diff and every semantic, compatibility, proof, and stop requirement
below; change only future mandatory caps to 260 production, 550 tests, and 800
total formatted net Rust lines against `6678f54f`. `REPLAN` on production over
260, tests over 550, total over 800, a third Rust file, or any behavior/scope
widening. Independent review accepted this correction in `99a5b898`; the
active implementation contract above is its sole successor authority.

## Accepted predecessor implementation contract

This section and everything below is historical context only, grants no file,
action, cap, or schedule authority, and is interpreted only through the active
implementation contract above.

The predecessor activation accepted design `6678f54f` and ran only
`WP-4-5-6-host-generated-repository-definition-lookup-owner-implementation`
in new `app/slug_core_v2/src/runtime/generated_repository_definition.rs` and
existing `app/slug_core_v2/src/runtime/mod.rs` solely for the private module
declaration, plus the four ledgers. Mandatory caps are 260 production, 480
tests, and 740 total formatted net Rust lines against `6678f54f`.

Implement the private callerless
`HostGeneratedRepositoryDefinitionKey { workspace, canonical_repo }` exactly
as frozen below: compute only the accepted hidden validation key; propagate
Need and opaque loading errors; completely scan the borrowed request/call
iterator; reject zero or duplicate canonical matches; retain only the full
certificate plus matched ordinal; and borrow the canonical/internal names,
original `RepoSpec`, and row mapping/context without copying a row, map, or
catalog. Preserve the complete proof matrix, exact/Slug-native/deferred
classification, and every stop. No third Rust file, server/loading/Bzlmod
production edit, public export, apparent route, execution, source/materializer,
I/O, lockfile, command/wire API, or JVM work is authorized.

## Accepted docs-only design contract

This section and everything below is historical context only, grants no file,
action, cap, or schedule authority, and is interpreted only through the active implementation contract above.

Independent review accepts `b9a4a3fc`: loading now publishes the complete
validated canonical generated-repository definition input—internal name,
canonical name, original `RepoSpec`, row-specific mapping context, and one
shared exact mapping-entry allocation per extension request. The next semantic
owner is `slug_core_v2`, not server: core already depends on loading and
Bzlmod, owns workspace DICE and repository demand/materialization
orchestration, while server is the daemon/wire adapter.

The accepted design ran only `WP-4-5-6-host-generated-repository-definition-lookup-owner-design`
in the canonical plan, this manifest, Stage 4, and Stage 5 under
45/260/220/220/745 formatted net documentation lines. Authorize no Rust,
fixture, Cargo, activation, apparent route, materializer, lockfile,
command/wire API, or JVM work.

Freeze one private core
`HostGeneratedRepositoryDefinitionKey { workspace, canonical_repo }` that
computes only `HostValidatedModuleExtensionRepositoriesKey`, propagates Need
invalidity and opaque loading errors, scans the complete borrowed iterator in
request/call order, and fails closed on zero or duplicate canonical matches.
Success retains the full `Arc<HostValidatedGeneratedRepositorySpecs>` plus the
matched ordinal; borrowed access rescans and exposes the exact canonical and
internal names, original `RepoSpec`, and row-context mapping without copying a
row, map, or catalog. Errors retain the complete certificate/requested
identity and conflicting ordinals as applicable. No events or I/O.

Freeze future implementation in exactly new
`app/slug_core_v2/src/runtime/generated_repository_definition.rs` and
existing `app/slug_core_v2/src/runtime/mod.rs` solely for a private module
declaration, plus four ledgers, with mandatory 260 production/480 tests/740
total formatted net Rust lines against the accepted design commit. Require
empty/missing/one/multiple definitions; complete-scan duplicate rejection;
exact canonical selection independent of apparent/internal spelling;
overridden original spec and complete mapping/context; request/call order;
canonical/internal/RepoRuleId/attributes/Labels/mapping value/order/context
A/B/A; Need/upstream error/warm reuse/zero event data; zero registry,
materialization, source, filesystem, or execution activation; full
core/loading/Bzlmod/server dependents; and structural no copied catalog/map,
second loader, retained Starlark lifetime, reverse edge, or public export.

Exact compatibility is limited to admitted Bazel 9.2 canonical generated
definition selection and its original `RepoSpec`/mapping association.
Core-private value/error/ordinal representation, diagnostics, and DICE
scheduling are Slug-native. Apparent/root mapping resolution, replacement
route selection, `RootRepositoryRoute`, repository implementation/context,
source preparation/materialization, BUILD/package loading, lockfile,
command/wire consumers, stable public API, nonroot/MVO/isolation/innate
breadth, and JVM remain deferred. `REPLAN` on server/Bzlmod/loading production
edits, reconstructed identity, copied row/map/catalog, public API, third Rust
file, apparent routing/materialization/I/O, retained evaluator lifetime, or cap
excess.

## Accepted predecessor implementation contract

This section and everything below is historical context only, grants no file,
action, cap, or schedule authority, and is interpreted only through the active implementation contract above.

Independent review accepts `b9a4a3fc`. The exact namespace map is retained once
per request from its sole producer, included structurally in the existing
predecessor, and exposed through the no-copy hidden iterator with internal
name and row-specific context. Proof discriminates base/generated/override/
inject entries and order, original overridden specs, one-Arc sharing, Need/
error/reuse, and zero events/I/O. Full loading and Bzlmod suites pass within
280/520/800 against `9e12fe58`.

## Accepted docs-only design contract

This section and everything below is historical context only, grants no file,
action, cap, or schedule authority, and is interpreted only through the active implementation contract above.

The accepted route audit REPLANs at one smaller prerequisite. Pinned Bazel 9.2
commit `8220c619` gives `SingleExtensionValue` both internal-name/original
`RepoSpec` rows and canonical-to-internal identity. Its
`ModuleExtensionRepoMappingEntriesFunction` constructs shared mapping entries
in host-module mapping, all generated entries, then ordered override/inject
substitutions with keep-last order; each generated repository has those shared
entries with its own canonical context. `RepoDefinitionFunction` later selects
the original generated `RepoSpec`, not a replacement spec.

Slug's accepted instantiation owner already builds this exact complete mapping
before schema work for Label coercion, but discards it. The hidden validation
ABI exposes only `(canonical, RepoSpec)`. The final mapping does not contain
this namespace, and reconstructing it from retained base/generated/override
ingredients would replay the exact algorithm outside its sole producer.
`RootRepositoryRoute` is not the prerequisite: its module-name and
DirectLocal/Builtin source
assumptions feed source-preparation behavior, while an evaluated generated
definition is not yet a prepared repository source.

Run only `WP-4-5-host-generated-repository-mapping-retention-design` in the
canonical plan, this manifest, Stage 4, and Stage 5 under 45/260/220/220/745
formatted net documentation lines. Authorize no Rust, fixture, Cargo,
activation, route, materializer, lockfile, command/API, or JVM work. Freeze a
future implementation in exactly
`module_extension_repository_instantiation.rs`,
`module_extension_repository_validation.rs`, and `lib.rs` solely for hidden
exports, with mandatory caps 280 production/520 tests/800 total formatted net
Rust lines against the accepted design commit.

The natural owner remains `HostInstantiatedModuleExtensionRepositoriesKey`:
retain one compact immutable mapping-entry allocation per extension request at
the point the exact namespace is already built, and share it across that
request's rows. Extend the existing validation certificate's borrowed hidden
iterator, without a new key or row catalog, to expose a row view containing
internal generated name, canonical name, original `RepoSpec`, and a mapping
view whose entries are shared per request while context is the row's canonical
repository. Need/errors publish no iterator; success and terminals retain the
same complete predecessor identity; no event or Starlark lifetime enters
equality.

Require empty/one/multiple extensions and calls; same-extension shared entries
with distinct contexts; host/base entries, every generated name, and ordered
substitution keep-last behavior; overridden/injected mapping entries while the
original generated row remains; cross-generated Label visibility; collision
and canonical/internal bijection failure; field/order/context A/B/A; Need,
completed error, warm reuse, zero events/I/O/materialization; full loading and
Bzlmod dependents; an external-style hidden consumer; and structural proof of
one mapping allocation per request, no per-row clone, new key, copied catalog,
retained evaluator value, or reverse edge.

Exact compatibility is limited to the admitted Bazel 9.2 generated mapping
entries/context, canonical/internal association, original `RepoSpec`, and
request/call order. Hidden Rust row/mapping representation, diagnostics, and
DICE scheduling are Slug-native. Generated route lookup, replacement route
selection, `RootRepositoryRoute`, repository implementation/context, source
preparation/materialization, BUILD/package loading, lockfile, command
consumers, stable public API, nonroot/MVO/isolation/innate breadth, and JVM
remain deferred. `REPLAN` on reconstructing mapping/name state, per-row map
copies, a new key/catalog, Bzlmod/server/route/materializer edits, a fourth Rust
file, execution/I/O, retained Starlark lifetime, or cap excess.

## Completed route-boundary audit

This section and everything below is historical context only, grants no file,
action, cap, or schedule authority, and is interpreted only through the active implementation contract above.

The audit inspected pinned `SingleExtensionFunction`, `SingleExtensionValue`,
`ModuleExtensionRepoMappingEntriesFunction`, and `RepoDefinitionFunction`, plus
live Slug route/source-preparation ownership and crate dependencies. Both
independent reviews REPLAN before route work because the exact mapping and
internal-name association are currently transient. No generated-route owner,
`RootRepositoryRoute` widening, Bzlmod reverse edge, or Rust work was accepted.

## Accepted predecessor implementation contract

This section and everything below is historical context only, grants no file,
action, cap, or schedule authority, and is interpreted only through the active implementation contract above.

Independent review accepts `d2ed6ad3`. The existing validation key now exposes
only a hidden success certificate and borrowed exact-size iterator over the
original request/call-ordered `(CanonicalRepoName, RepoSpec)` rows; its opaque
error preserves private typed identity. Overridden rows retain their original
canonical identity and `RepoSpec`. The implementation added no key, copied row
store, route, materialization request, I/O, or stable public API, stayed within
220/420/640 against `433badeb`, and passed full loading and Bzlmod all-target
suites plus independent ABI/proof review.

## Accepted docs-only design contract

This section and everything below is historical context only, grants no file,
action, cap, or schedule authority, and is interpreted only through the active implementation contract above.

Independent review accepts `b2a153aa`: loading now owns a complete
heap-independent validation certificate over exact request-ordered generated
canonical names and `RepoSpec` rows. Bzlmod owns repository routes and
materialization but cannot depend back on loading. Run only
`WP-4-5-host-validated-generated-repository-spec-publication-design` in the
canonical plan, this manifest, Stage 4, and Stage 5 under 40/240/200/180/660
formatted net documentation lines. Authorize no Rust, fixture, Cargo,
activation, Bzlmod mutation, route, materializer, lockfile, consumer/API, or
JVM work.

Pinned Bazel 9.2 `SingleExtensionFunction` returns the validated
eval-only value unchanged. Its `generatedRepoSpecs` therefore retains every
generated internal-name/`RepoSpec` row, including a row whose apparent name
is later overridden. `SingleExtensionValue` independently derives each
generated canonical name from the accepted collision-safe extension unique
prefix plus the generated name. Ordered override/inject substitutions affect
repository mappings and later lookup, not the retained generated `RepoSpec`
or its canonical identity. Repository-rule callable reacquisition has already
finished before this value exists, so heap-free spec publication is separable
from route selection and repository implementation execution.

Freeze exactly one `#[doc(hidden)]` ABI over the existing DICE key and row
store. `HostValidatedModuleExtensionRepositoriesKey::new(workspace)` becomes
the hidden public normalized-workspace constructor but remains the same sole
key. Its value is
`SourcePreparationOutcome<Arc<Result<HostValidatedGeneratedRepositorySpecs,
HostValidatedGeneratedRepositorySpecsError>>>`. The hidden public success
wrapper retains the private validation certificate and exposes only
`iter(&self) -> impl ExactSizeIterator<Item = (&CanonicalRepoName, &RepoSpec)>`
in request then call encounter order. It stores no second row collection.
The hidden public error wrapper implements Debug/Display/Error/Eq/Allocative;
its private inner retains the complete typed validation terminal, but callers
may only propagate, display, and structurally compare it, never pattern-match
private instantiation/invocation/request/offender state. Need and every terminal
publish no success iterator.

The only permitted later consumer direction is a higher crate already allowed
to depend on both owner crates; currently slug_server_v2 -> slug_loading_v2 and
slug_server_v2 -> slug_bzlmod_v2. This packet adds no server edit or
consumer. A lower neutral/core crate may not depend upward on loading, Bzlmod
may not reverse-depend on loading, and the hidden ABI is not a stable public API.

Freeze the future implementation in exactly
`module_extension_repository_validation.rs`,
`module_extension_repository_instantiation.rs` for narrow borrowed
canonical-name/`RepoSpec` accessors, and `lib.rs` solely for the named
hidden exports, with mandatory caps 220 production/420 tests/640 total
formatted net Rust lines against the accepted design commit. Require exact
request/call order, canonical-name and full `RepoSpec` identity,
empty/multiple rows, validation error/Need nonpublication, opaque error
identity, A/B/A for canonical name, `RepoRuleId`, attributes/Labels and
request order, cold/warm reuse, an external-crate-style hidden consumer compile
row, full loading/Bzlmod suites, and structural proof of no copied store,
second key, retained Starlark lifetime, route/materialization request, event,
I/O, or reverse dependency.

Exact compatibility is limited to the already admitted validated generated
canonical-name/`RepoSpec` content and encounter order. The hidden Rust
projection, diagnostics, and DICE scheduling are Slug-native. Apparent-name
route resolution, final mapping publication, replacement route selection,
repository implementation/context, source preparation/materialization,
lockfile, command consumers, stable public API, nonroot/MVO/isolation/innate
breadth, and JVM remain deferred. `REPLAN` on a Bzlmod production edit, new
DICE key, copied semantic rows, unresolved override publication, route or
materialization construction, repository execution/I/O, public non-hidden API,
fourth Rust file, or cap excess.

## Accepted predecessor implementation contract

This section and everything below is historical context only, grants no file,
action, cap, or schedule authority, and is interpreted only through the active implementation contract above.

Independent review accepts `b2a153aa` at approximately 295 production, 612
tests, and 907 total formatted net Rust lines. It computes only the accepted
instantiation predecessor, exact-joins full requests, validates imports before
override/inject polarity using a transient generated-name set, retains only
the predecessor on success, and preserves complete typed error identity.
Focused proof covers true predecessor Need, join corruption, field/order/span/
polarity A/B/A, Evaluated-to-Reused with zero events, and zero registry or
materialization activation; both full all-target loading and Bzlmod suites
pass.

## Accepted docs-only design contract

This section is historical context only, grants no file, action, cap, or
schedule authority, and is interpreted through the active docs-only design
contract above.

Independent review accepts `ff55dcbf`: one compact local-name order spine now
owns root/nonroot import source order, and hidden requests retain aggregated
local/exported names plus import and override spans. Together with accepted
`d50f02a2` instantiated repository rows, no prerequisite remains for pinned
Bazel 9.2 `SingleExtensionFunction` validation.

Run only the four-plan docs packet
`WP-4-5-host-module-extension-generated-repository-validation-owner-design`
in canonical/current/Stage 4/Stage 5 under 40/220/180/180/620 formatted net
documentation lines. Authorize no Rust, fixture, Cargo, activation, Bzlmod
mutation, route, materializer, lockfile, consumer/API, or JVM work. Freeze a
successor or REPLAN only after independent acceptance.

Freeze one callerless private
`HostValidatedModuleExtensionRepositoriesKey { workspace }` in loading. It
computes only `HostInstantiatedModuleExtensionRepositoriesKey`, propagates
Need as invalid/non-self-equal, and propagates a completed predecessor error
before validation. Exact-join instantiated rows to their
embedded requests by count, encounter order, and full equality; corruption is
a typed complete terminal.

For each request in encounter order, build one transient compact membership set
of that request's generated apparent names. Validate in pinned order: first
scan every aggregated import row in retained usage/proxy/import order, accepting
its exported generated name only when present in that generated set or as a key
in the request's override/inject table; otherwise fail at the import span.
Then scan overrides/injects in retained order: `must_exist=true` fails when
the generated name is absent, while `must_exist=false` fails when it is
present, at the override span. Local import spelling is diagnostic identity,
not the membership key. Empty rows succeed. Advance the validated-prior-request count only after all of
its validation completes; retain no scratch set or duplicate generated map.

Success retains only the complete
`Arc<HostInstantiatedModuleExtensionRepositories>` predecessor; successful
key completion is the validation certificate, with no second retained row
view. A typed terminal retains the complete predecessor, the validated prior
request count, exact current instantiated per-request row, exact current hidden
request, offending import or override including span,
and missing-import/override-missing/inject-collision category. Events and I/O
are absent from this owner and from semantic equality.

Freeze a future implementation in exactly existing
`app/slug_loading_v2/src/module_extension_repository_instantiation.rs` for
narrow crate-private borrowed accessors, new private
`module_extension_repository_validation.rs`, and `lib.rs` solely for its
private declaration, plus the four ledgers. Mandatory caps are 320 production,
650 tests, and 970 total formatted net Rust lines against the accepted design
commit.

Require pure and real-DICE proof for empty/one/multiple generated names;
generated and override-backed imports; missing import and local-vs-exported
name discrimination; import-before-polarity precedence; override present/
missing and inject absent/collision; request isolation and exact-join
corruption; completed/current prefixes and spans; predecessor Need/error;
generated set, import name/order/location, override name/order/target/location/
polarity A/B/A; Evaluated-to-Reused behavior with zero key event data; zero
extra Bzl/registry/fs/materializer activation; full loading/Bzlmod suites; and
structural absence of retained Starlark lifetimes, scratch maps, route,
execution, I/O, or reverse dependency.

Exact compatibility is limited to the admitted pinned post-evaluation import
and override/inject predicates and their encounter order. Private compact
representation, diagnostics/suggestions, and DICE scheduling are Slug-native.
Generated route/mapping publication, replacement RepoSpec selection,
repository implementation/context, source preparation/materialization,
lockfile, public consumers/API, nonroot/MVO/isolation/innate breadth, and JVM
remain deferred. `REPLAN` on any Bzlmod production edit/reverse edge, second
loader/evaluator or additional DICE predecessor, retained lifetime/scratch
set, events/I/O, route/materialization/publication claim, fourth Rust file, or
cap excess.

## Accepted predecessor implementation contract

This section and everything below is historical context only, grants no file,
action, cap, or schedule authority, and is interpreted only through the active implementation contract above.

Independent review accepts the REPLAN design in `f14d3d7a`. Run only
`WP-5-extension-import-order-identity-owner-implementation` in
`app/slug_bzlmod_v2/src/interim_module.rs`, `selected_repo_spec.rs`, and
`lib.rs` solely for hidden import/request accessors, plus four ledgers.
Caps are 260 production, 450 tests, and 710 total formatted net Rust lines
against `f14d3d7a`. Preserve the compact order-spine, complete projection,
proof, compatibility, and stop contract below.

## Accepted docs-only REPLAN contract

This section is historical context only, grants no independent file, action,
cap, or schedule authority, and is interpreted only through the active implementation contract above.

The first compiling request widening adds 108 production/test lines in
`selected_repo_spec.rs` and two hidden `lib.rs` exports, but its required
real DICE reorder proof fails: swapping only two `use_repo` kwargs produces an
equal request. The root evaluator stores proxy imports in
`NonrootRepoImports.local_to_exported` and `exported_to_local`
`SmallMap`s; their equality intentionally ignores insertion order, so DICE
prunes the source-order edit before selected request construction. Iterating
the map later does not repair missing semantic identity.

Run only the four-plan docs packet
`WP-5-extension-import-order-identity-owner-design` under
40/220/180/180/620. Retain the unaccepted two-file diff but authorize no Rust,
fixture, Cargo, request widening, loading validator, routes, I/O, materializer,
lockfile, consumer/API, or JVM work until independent acceptance and explicit
activation.

Freeze one shared compact order spine on existing `NonrootRepoImports`: an
immutable `Arc<[CompactString]>` of local names in declaration order, built
once by `from_local_to_exported` before moving the existing maps. The maps
remain the sole local/exported lookup and bijection owners; the spine must not
duplicate exported names or form a third map. Equality/Allocative includes the
spine, so root and nonroot MODULE evaluation, selected mappings, and later
validation all invalidate on reorder. Selected validation rows iterate the
spine and look up each exported value from `local_to_exported`; a missing or
duplicate spine/map association fails closed rather than falling back to map
iteration.

Freeze a future corrected implementation in exactly
`app/slug_bzlmod_v2/src/interim_module.rs`,
`selected_repo_spec.rs`, and `lib.rs` solely for hidden import/request
accessors, plus four ledgers. Caps are mandatory 260 production, 450 tests,
and 710 total formatted net Rust lines against the accepted design commit.
Require pure empty/one/reordered/order-sensitive equality and malformed-spine
rows; root/nonroot directive preservation; duplicate-ID request aggregation;
local/exported/location and override-location identity; real DICE import
name/value/order/location and override location/polarity A/B/A; Need/error,
warm reuse, unchanged loading dependents, zero I/O, and structural compact
ownership proof.

Exact compatibility is limited to admitted MODULE import declaration order and
the validation-request inputs already frozen below. The compact local-name
spine, private errors, and DICE scheduling are Slug-native. Import existence,
suggestions, override/inject polarity, generated publication/routes,
repository execution/context, I/O/materialization, lockfile, nonroot
validation breadth, public APIs/consumers, and JVM remain deferred. `REPLAN`
on a fourth Rust file, new key/map/interner/cache/digest, duplicated exported
names, loading/reverse edge, validation/routes, I/O/materializer/lockfile/
consumer/API/JVM work, or cap excess.

## Accepted predecessor implementation contract

This section and everything below is historical context only, grants no file,
action, cap, or schedule authority, and is interpreted only through the active implementation contract above.

Independent review accepts the design in `533a9453`. Run only
`WP-5-host-selected-extension-validation-request-projection-implementation`
in `app/slug_bzlmod_v2/src/selected_repo_spec.rs` and `lib.rs` solely for
the existing hidden request/export accessors, plus the four ledgers. Caps are
220 production, 380 tests, and 600 total formatted net Rust lines against
`533a9453`. Preserve the complete ordering, identity, proof, compact
representation, compatibility, and stop contract below.

## Accepted docs-only design contract

This section is historical context only, grants no independent file, action,
cap, or schedule authority, and is interpreted through the active docs-only
design contract above.

Independent review accepts repository-rule instantiation in `d50f02a2` at
474 production, 799 tests, and 1,273 total within 480/900/1,380; full loading
and Bzlmod suites pass. Pinned Bazel 9.2
`SingleExtensionFunction` validates every `use_repo` import before scanning
override/inject polarity. The accepted hidden request exposes ordered
override/inject names and `must_exist` but drops the import exported name and
proxy location, and drops override locations. A loading validator now would
silently omit exact import failures and diagnostic identity.

Run only the four-plan docs packet
`WP-5-host-selected-extension-validation-request-projection-design` in
canonical/current/Stage 4/Stage 5, capped at 40/220/180/180/620 formatted net
documentation lines. Authorize no Rust, fixture, Cargo, loading validator,
route, materializer, lockfile, consumer, or JVM work. Freeze a successor or
REPLAN after independent acceptance.

The natural prerequisite widens the existing
`HostSelectedExtensionDefinitionLoadRequest`; it adds no key, graph, map
owner, or loading dependency. Retain one ordered immutable import row per
admitted root proxy import with local apparent name, exported generated name,
and exact `LogicalSpan`, aggregated across every usage matching the exact
extension ID in root source order, proxy order, and retained `SmallMap`
iteration order. Retain the existing generated canonical identity only once
through mappings/predecessor state; do not fabricate it from string slicing.
Add the exact `LogicalSpan` to each ordered override/inject projection.
Repeated equal extension IDs deduplicate the request only after all matching
imports are concatenated; empty imports/overrides succeed. Missing, duplicate,
or mismatched ID/namespace joins fail closed before publication.

Use `CompactString`, `LogicalSpan`, immutable `Arc` slices, existing
source-ordered `SmallMap` iteration, and `Allocative`; introduce no
`HashMap`, interner, cache, digest, or duplicated retained map. Structural
equality includes the complete selected predecessor plus ordered import names,
locations, overrides, locations, targets, and polarity. Need remains invalid
and completed predecessor errors remain typed.

Freeze a future implementation in exactly
`app/slug_bzlmod_v2/src/selected_repo_spec.rs` and `lib.rs` solely for the
existing `#[doc(hidden)]` request/export accessors, with mandatory caps of
220 production, 380 tests, and 600 total formatted net Rust lines against the
accepted design commit. Required proof: pure empty/one/multiple usage and
proxy/import order; duplicate-ID aggregation; local/exported spelling and
location; empty/ordered override locations and polarity; mismatch fail-closed;
real DICE import/name/order/location and override-location A/B/A, Need/error,
warm reuse, unchanged loading dependents, and zero registry/materialization
I/O. Structural scans must prove no new key/graph/loading edge and compact
retained ownership.

Exact compatibility is limited to the admitted root-main ordinary nonisolated
input identity and pinned usage/proxy/import/override encounter order.
Compact/private Rust representation, diagnostics, and DICE scheduling are
Slug-native. Import existence, suggestions, override/inject polarity,
generated result publication, routes, repository execution/context,
filesystem/network/environment/materialization, lockfile, nonroot/MVO/
isolation/innate breadth, public consumers/APIs, and JVM remain deferred.
`REPLAN` on a third Rust file, new key/graph/projection, duplicated map,
loading dependency/reverse edge, generated-set inference, validation/routes,
I/O/materializer/lockfile/consumer/API/JVM work, or cap excess.

## Accepted predecessor implementation contract

This section and everything below it are historical context only, grant no
file, action, cap, or schedule authority, and are interpreted only through the
active implementation contract above.

Independent review accepts the cap correction in `7cf2e45f`. Run only
`WP-4-5-host-module-extension-repository-rule-instantiation-owner-implementation-r2`
in existing `module_extension_repository_rule.rs`, private
`module_extension_repository_instantiation.rs`, and `lib.rs` solely for its
private declaration, plus the four ledgers. Caps are 480 production, 900
tests, and 1,380 total formatted net Rust lines against `7616136f`. Complete
every frozen proof row below without production growth or semantic expansion.
All prior fourth-file, Bzlmod mutation, extra key/loader, lifetime, execution,
I/O, existence/routes, materializer, lockfile, consumer/API, JVM, and cap stops
remain.

## Accepted r2 cap correction

This section is historical context only, grants no independent file, action,
cap, or schedule authority, and is interpreted through the active docs-only
design contract above.

The first compiling implementation is 474 production, 572 tests, and 1,046
total formatted net Rust lines against `7616136f`. Production is within the
frozen cap and independently reviewed as architecturally sound, but the
700-test/1,180-total proof budget cannot credibly contain the still-required
exact count/full-request join corruption, substituted-namespace and
namespace-before-schema tables, predecessor Need and zero-event lifecycle,
completed/current prefix identity, and mapping/schema/default/name/value/
kwargs-order/provenance A/B/A discriminators.

Retain the unaccepted diff in exactly
`app/slug_loading_v2/src/module_extension_repository_rule.rs`, private
`module_extension_repository_instantiation.rs`, and `lib.rs` solely for its
private declaration. Correct only the future r2 caps to 480 production, 900
tests, and 1,380 total formatted net Rust lines against `7616136f`; preserve
every semantic, proof, compatibility, and stop clause below. No Rust is
authorized by this docs packet. REPLAN on production growth, a fourth Rust
file, another key/loader, reduced proof, behavior breadth, or cap excess.

## Accepted predecessor implementation contract

This section is historical context only, grants no file, action, cap, or
schedule authority, and is interpreted only through the active docs-only design
contract above.

Independent review accepts `7616136f`. Run only
`WP-4-5-host-module-extension-repository-rule-instantiation-owner-implementation`
in existing
`app/slug_loading_v2/src/module_extension_repository_rule.rs`, one new private
`module_extension_repository_instantiation.rs`, and `lib.rs` solely for its
private declaration, plus canonical/current/Stage 4/Stage 5 bookkeeping. Caps
are 480 production, 700 tests, and 1,180 total formatted net Rust lines against
`7616136f`. Preserve the complete exact/Slug-native/deferred design, proof,
and terminal-stop contract below. No fourth Rust file, Bzlmod mutation, second
loader or additional DICE key beyond
`HostInstantiatedModuleExtensionRepositoriesKey`, retained Starlark lifetime,
repository implementation/context,
I/O, materializer, existence/final-route validation, lockfile, consumer,
public API, JVM, or cap excess.

Accepted `b7c70a1b` owns ordered raw calls, exact definition/schema identity,
generated apparent names, kwargs, and caller provenance in the sole loading
invocation receipt. Accepted `c7c55b17` exposes, through the same exact hidden
load request, the selected unique prefix, root pre-substitution mapping, final
mapping, and ordered override/inject metadata. This closes the namespace
prerequisite but owns no call-name set, RepoSpec, schema application, or
generated existence verdict.

Pinned Bazel 9.2 `ModuleExtensionEvalStarlarkThreadContext.createRepos`,
`RepoRule.instantiate`, `AttributeUtils.typeCheckAttrValues`, and
`SingleExtensionFunction` freeze one loading-owned callerless DICE projection
that computes the complete raw invocation owner first, joins each receipt to
the exact selected request, constructs the full generated-repository mapping
for every call before schema work, applies ordered root substitutions only
after base entries and generated names, then instantiates admitted scalar
attributes atomically in extension/call encounter order. The semantic result
must be heap-independent and retain complete predecessors, exact request and
definition identity, generated apparent/canonical names, caller provenance,
and the existing Bzlmod `RepoRuleId`/`RepoSpec` algebra without running the
repository implementation.

The exact namespace algorithm is request/receipt encounter order; start from
the request's root base mapping; add every call name as
`unique_prefix + "+" + generated_name`; apply the request's ordered override/
inject substitutions last with keep-last semantics; and only then instantiate
calls in encounter order. The exact request object embedded in each invocation
receipt must equal the same-index prepared input request; count/order or full
equality mismatch is terminal. No label/export-only join is permitted. The
request's final mapping remains structural predecessor identity and may be used
as an integrity discriminator, but namespace assembly never derives from it.

Pinned `RepoRule.instantiate` removes `name`, `tags`, `deprecation`, and
`visibility` before type checking and never stores them. In supplied raw
kwargs order, ignore those four and `None`; otherwise fail first unknown
attribute, scalar conversion, or Label conversion/allowed-value error. Then in
definition declaration order fail the first missing mandatory attribute,
select declared or intrinsic defaults for validation, and visibility-check the
final value. Defaults are not re-resolved and omitted/defaulted values are not
stored. The resulting `RepoSpec.attributes` contains only explicitly supplied
non-None, nonlegacy user attributes in raw kwargs order after conversion; no
built-in field is present. `name` is retained separately as the generated
apparent name. `RepoRuleId` is the definition's canonical bzl label plus
exported rule name.

For Label attrs, a supplied String resolves relative to the defining bzl
label's canonical package using the complete generated-then-substituted
mapping. A captured canonical Label is not rebound. A declared canonical Label
default is not reparsed or rebound, but must be visible from the defining
context or among the complete mapping's canonical values. Admit only the
already captured root-main ordinary nonisolated empty-factor String/Bool/i32/
Label slice. Every container/output/big-int/function/context/tag/cycle,
configurable/transition, allowed-values, file/provider/executable restriction,
repository implementation factor, or unmodeled descriptor fails closed.

The private owner is
`HostInstantiatedModuleExtensionRepositoriesKey { workspace }` returning a
complete-only
`HostInstantiatedModuleExtensionRepositories` or typed
`HostInstantiatedModuleExtensionRepositoriesError` through the existing
`SourcePreparationOutcome`. Success retains the complete raw invocation
predecessor plus extension/call-ordered immutable rows containing exact request,
raw call/provenance, generated apparent/canonical name, and `RepoSpec`.
Namespace/join failures precede all schema work. A failing call publishes no
partial row; its terminal retains the complete predecessor, all completed
extensions, the current extension's completed rows, and the exact failing raw
call/request/definition/mapping context. An upstream invocation Need remains
invalid and an upstream completed error yields no instantiation rows.

Pinned `createRepos` applies override/inject substitutions without checking
`must_exist`. Pinned `SingleExtensionFunction` validates override-missing and
inject-collision only after the eval-only generated RepoSpecs exist. Therefore
this packet retains ordered `must_exist` structurally but performs no existence
verdict; that later validator and final generated routes/mappings remain
deferred.

The future implementation is exactly existing
`app/slug_loading_v2/src/module_extension_repository_rule.rs`, one new private
`module_extension_repository_instantiation.rs`, and `lib.rs` solely for its
private declaration. Caps are 480 production, 700 tests, and 1,180 total
formatted net Rust lines against the accepted design commit; no fourth Rust
file or Bzlmod production edit. Require pure empty/one/multiple-call namespace,
collision-prefix, cross-call visibility, substitution/`must_exist`, exact
stored field/order, two-phase coercion/default/visibility, and atomic-prefix
tables; real-key predecessor Need/error,
mapping/schema/default/name/value/order/provenance A/B/A, cold/warm reuse,
zero events and zero I/O; full loading/Bzlmod dependents; and structural
absence of Heap/Value/FrozenValue/callable/context from retained state.

Exact compatibility is limited to the admitted pinned `createRepos` namespace
assembly and `RepoRule.instantiate` semantic projection. Private Rust
representation, diagnostics, and DICE scheduling are Slug-native. Repository
implementation/`repository_ctx`, environment/OS/facts, filesystem/network/
watch/download/execute, materialization, final generated routes, override/
inject existence validation/publication, lockfile, nonroot/MVO/isolation/
innate breadth, commands, public APIs, and JVM remain deferred. `REPLAN` on
Bzlmod mutation or
reverse dependency, reconstructing selected namespace state, a second loader,
retained Starlark lifetime, order-insensitive attributes, guessed built-ins,
repository execution/I/O, generated existence/final-route claims, a fourth
future Rust path, or cap excess.

## Accepted docs-only design contract

This section and everything below are historical accepted design context only,
grant no independent file, action, cap, or schedule authority, and are
interpreted solely through the active implementation contract above.

The definition-owner audit below found no truthful standalone definition DICE
leaf. Pinned Bazel 9.2 `repository_rule()` creates an immutable exported
Starlark callable, and its first semantic consumer is
`ModuleExtensionEvalStarlarkThreadContext.lazilyCreateRepo`; separating a
definition projection would duplicate the sole loader or retain a callable
without its natural invocation-local owner. Run only
`WP-4-5-host-module-extension-repository-rule-call-protocol-design` in
canonical/current/Stage 4/Stage 5 under 45/260/240/180/725 documentation lines.
Authorize no Rust, Cargo, fixture, generated output, or activation before
independent acceptance.

Freeze one root-main, ordinary, nonisolated, empty-factor protocol that reuses
the shared `.bzl` globals, `HostBzlModuleEvalKey`, and accepted pure-invocation
preflight. Audit the exact global parameters and construction order; export
binding and defining-label identity; lifetime-only implementation callable;
an ephemeral per-invocation sink/token; exact positional/context/export/name/
duplicate/deep-clone first-error order; and source-order capture of
heap-independent raw call records. Determine a complete first attr/option
surface rather than guessing breadth. Schema type/default/visibility
application belongs to later `RepoRule.instantiate` and must not be moved into
the capture phase.

The design must freeze an implementation allowlist no broader than
`app/slug_loading_v2/src/package.rs`, existing private
`module_extension.rs`, one new private
`module_extension_repository_rule.rs`, and `lib.rs` solely for its private
declaration, with explicit production/test/total caps. Require definition,
export/private, context ownership, one/two-call order, duplicate name,
call-before-throw prefix, all-request Need/error zero-call, definition/source/
mapping/name/raw-attr A/B/A, complete-only heap-free equality, event ownership,
cold/warm reuse, full loading/Bzlmod dependents, and structural absence of a
second loader, retained Starlark lifetime value, RepoSpec, repository
implementation execution, or I/O.

Compatibility is exact only for the admitted Bazel 9.2 definition/export and
capture surface. Private Rust representation, compact call records,
diagnostic framing, and DICE scheduling are Slug-native. `RepoRule.instantiate`,
`repository_ctx`, schema coercion/default insertion, environment/filesystem/
network/watch/download/execute, generated canonical names/RepoSpecs,
materialization, override/inject existence, final mappings, lockfile,
nonroot/MVO/isolation/innate breadth, commands, public API, and JVM are
unsupported/deferred. `REPLAN` on any RepoSpec/existence inference, repository
implementation execution, second loader/evaluator/graph, retained heap or
callable, public/wire surface, Bzlmod mutation/reverse dependency, repository
I/O, fifth Rust path, unresolved pinned order, unbounded attr/options, or cap
excess.

### Completed owner audit and frozen implementation successor

The smallest cohesive owner is the existing callerless
`HostPureModuleExtensionInvocationsKey`, extended in place. It already computes
prepared inputs, reacquires every exact frozen Host module, validates all
requests before invoking any callable, owns invocation events, and publishes a
heap-free receipt. Add no DICE key. During each extension invocation, install
one evaluation-local `RepositoryRuleInvocationState` in `Evaluator.extra`.
The state itself is the ephemeral invocation capability and owns a mutable
scratch vector but no lock and no DICE computation; after the callable returns
or throws, project it
once into ordered immutable records carried by that extension's receipt or
terminal prefix. Every preflight Need/error still performs zero invocation and
zero capture.

The shared `repository_rule` global is available only while the sole Host bzl
loader supplies `BzlEvaluationContext`. Its exact admitted signature is
`implementation` (positional or named callable) plus optional `attrs=None`;
`local` and `configure` must remain false, `environ` empty, and `doc` None.
Pinned-default semantics do not expose flag-gated `remotable`. Callable arity
is not checked at definition time. `attrs` may contain source-ordered public
ASCII identifier names and only `attr.string`, `attr.bool`, `attr.int`, or
`attr.label` descriptors with no explicit configurable policy, transition,
or file restriction and with kind-correct scalar/None defaults. Reject the
legacy built-in names `name`, `tags`, `deprecation`, and `visibility` in
source order. Private attrs, containers, computed/late defaults, values/
providers/executable/cfg/aspects/file restrictions, every other kind, nonempty
environment, true local/configure, non-None doc, and remote-exec semantics are
deferred. These rejections are fail-closed Slug terminals, not Bazel parity
claims.

`RepositoryRuleDefinitionGen<V>` retains the lifetime callable, canonical
defining bzl label, source-ordered admitted schema, and optional exported name.
Assignment uses `export_as`; anonymous values may freeze but fail only when
called. A leading-underscore top-level rule remains internally exported and
callable by its extension; the pinned `use_repo_rule` private-lookup rejection
is external lookup behavior and remains deferred. Definition/export location
is not retained because Bazel's `RepoRule` does not use it. The existing
prepared predecessor already structurally owns the selected request/mapping,
complete transitive `BzlLoadManifest`, source bytes, and extension definition,
so the call record retains the compact repository-rule projection rather than
duplicating a digest or frozen module.

For an admitted call, preserve this order: Starlark argument formation;
unexpected positional rejection; invocation-context lookup; exported-name
check; ordered kwargs extraction; `name` default-to-None/type check; exact
user-provided repository-name validation
`[A-Za-z0-9][A-Za-z0-9_.-]*`; duplicate-name check within that extension;
caller location and call-stack projection with the repository-rule native
frame removed; then recursive-value projection and atomic append. The first
slice admits `None`, bool, signed i32, valid-Unicode string, and the accepted
canonical `InvocationLabel`; list, tuple, dict, big integer, function, context,
tag, cycle, and every other value fail before append. Unknown or undeclared
scalar kwargs are captured rather than schema-checked. Retain `name` both as
the lookup key and in the ordered kwargs because later instantiation consumes
the original map.

Each `RepositoryRuleCallRecord` retains the projected definition, generated
name, ordered `Arc<[(CompactString, RepositoryRuleCallValue)]>`, one
heap-independent caller span, and ordered compact stack frames/spans.
`Arc<[RepositoryRuleCallRecord]>` preserves source/call order. Do not use
`SmallMap` equality for retained kwargs: it intentionally ignores insertion
order, while Bazel's later supplied-attribute first-error order observes it.
Use `CompactString`, typed `CanonicalLabel`, immutable `Arc` slices, cheap
clones, and `Allocative`; scratch `Vec`/linear duplicate lookup is bounded to
one invocation and is not retained. No interner, hash cache, global registry,
or strong/content digest is warranted.

An extension success still must return strict `None`; its receipt gains its
ordered call slice. An invocation error retains the complete prepared
predecessor, exact request, all earlier-extension receipts, and the current
extension's captured prefix before its typed failure. Calls emit no events;
existing loader batches remain loader-owned and prints/throws remain fresh
invocation evaluation data outside equality. Complete structural values/errors
participate in DICE equality and cutoff; Need remains invalid/non-self-equal.
Warm reuse does not rerun callables or republish batches.

After independent design acceptance, freeze
`WP-4-5-host-module-extension-repository-rule-call-protocol-implementation`
against the accepted design commit. It may edit exactly:

- `app/slug_loading_v2/src/package.rs` for narrow descriptor projection and
  shared-global registration only;
- `app/slug_loading_v2/src/module_extension.rs` for preexisting invocation
  state/receipt/error integration and a read-only canonical Label projection;
- one new private
  `app/slug_loading_v2/src/module_extension_repository_rule.rs` for the
  definition value, schema, context/sink, raw projection, records, and focused
  tests;
- `app/slug_loading_v2/src/lib.rs` solely for the private module declaration;
  and canonical/current/Stage 4/Stage 5 bookkeeping.

Cap Rust at 650 production, 850 tests, and 1,500 total formatted net lines.
Require pure definition rows for positional/named callable, wrong callable,
anonymous/exported/private rules, exact option defaults/rejections, schema
order/defaults, legacy collisions, and every deferred descriptor family.
Require invocation rows for foreign context, positional/export/name/name
syntax, one/two call order, duplicate before unsupported value, ordered kwargs,
every scalar and deferred value, caller/stack provenance, internal private
rule calls, empty calls, strict result, call-prefix-before-throw, and separate
extension namespaces. Through the real key prove prepared/loader Need and
terminal zero-call, definition/source/manifest/mapping/schema/export/name/
value/order/location A/B/A, complete error-context A/B/A, cold/warm reuse,
fresh print/error batches, and full loading/Bzlmod direct dependents. Run
`cargo fmt --all -- --check`, archive/diff checks, and a structural scan for
retained `Heap`, `Value`, `FrozenValue`, `FrozenModule`, token, event,
`RepoSpec`, repository context, I/O, and second-key state.

The exact slice is the admitted Bazel 9.2 global construction/export, internal
callability, scalar capture, repository-name/duplicate validation, call and
kwargs order, and provenance semantics. Private type names, compact layout,
unsupported diagnostics, and DICE scheduling are Slug-native. Every deferred
surface above remains unsupported. `REPLAN` on a fifth Rust path; a new DICE
key or lock; callable/heap/event retention; schema checks during capture;
repository implementation/context, RepoSpec, generated canonical name/
existence/mapping, override/inject, I/O/materialization/lockfile/consumer/API/
JVM work; loss of ordered identity; production over 650, tests over 850, or
total over 1,500.

## Completed repository-rule definition owner audit

This section and everything below are historical context only, grant no file,
action, cap, or schedule authority, and are interpreted solely through the
active implementation contract above.

The audit uses Bazel 9.2 tag commit `8220c619`:
`RepositoryModuleApi.repository_rule`,
`StarlarkRepositoryModule.repositoryRule`/`StarlarkRepoRule`,
`RepoRule`, `ModuleExtensionEvalStarlarkThreadContext`, and focused
`ModuleExtensionResolutionTest` export rows. The global accepts required
callable `implementation`, `attrs=None`, `local=False`, `environ=[]`,
`configure=False`, flag-gated `remotable=False`, and `doc=None`.
Construction retains ordered descriptors, defining label, transitive bzl
digest, repository-mapping entries, options, and lifetime-only callable;
`export_as` supplies the rule name. A call rejects positional arguments,
foreign context, and unexported rules before validating `name`, duplicate
names, call location, and recursively cloned raw kwargs.

Only later `createRepos` builds the full generated-repository mapping and
invokes `RepoRule.instantiate` in insertion order to type/default attributes
and produce RepoSpecs. Slug already owns the defining source label in
`BzlEvaluationContext`, the sole frozen module and transitive manifest in
`HostBzlModuleEvalKey`, and an ephemeral invocation-context lifetime. It lacks
only the shared global/value and invocation-local capture sink. Therefore a
separate definition key would be a second owner with no semantic consumer;
the smallest truthful next design composes definition/export and capture while
stopping before instantiation.

## Accepted pure-invocation implementation evidence

This section and everything below are historical context only, grant no file,
action, cap, or schedule authority, and are interpreted solely through the
active implementation contract above.

Independent review accepts the event correction in `f36ec593`. Implement only
`WP-4-5-host-pure-module-extension-invocation-owner-implementation-r4` in the
same four app Rust paths plus canonical/current/Stage 4/Stage 5, under
730/850/1,580 against `40def0e7`. Rename the overclaiming focused test to
publication plus semantic reuse; fresh evaluated activations publish exactly
one prefix, reused activations carry no duplicate batch, event content stays
out of equality, and command-output lineage remains deferred to its real
consumer. Preserve every other accepted semantic, proof, compatibility claim,
cleanup, and stop. No fifth Rust path or behavior expansion.

### Final implementation evidence

The formatted four-path delta measures approximately 724 production, 846
tests, and 1,570 total lines against `40def0e7`, within 730/850/1,580. The full
`slug_loading_v2 --all-targets` suite passes 92 owner tests plus every loading
integration, and `slug_bzlmod_v2 --all-targets` passes 349 owner tests plus all
integrations. The renamed event-lineage test passes and proves one evaluated
batch followed by semantic reuse with no duplicate batch. Formatting and diff
checks pass. Cleanup removed two unrelated visibility widenings; structural
review finds `FrozenModule`/`FrozenValue` only in ephemeral preflight rows and
none in the retained receipt. Independent implementation reviews accept the
architecture, ABI, errors, events, scope, caps, and all stops.

## Accepted docs-only event-contract correction

This section is historical correction context only, grants no file, action,
cap, or schedule authority, and is interpreted solely through the active implementation contract above.

Final implementation review accepts the invocation architecture but rejects
the phrase that the invocation key itself replays its batch on warm reuse.
Existing DICE ownership intentionally attaches evaluation data only to the
fresh `Evaluated` activation; a `Reused` activation has no batch. The existing
`CommandEffectOwner` later selects reachable earlier evaluated batches within
one command/retry lineage, while a fresh owner does not replay them. This
callerless invocation packet has no command consumer and must not duplicate
events in its heap-free semantic receipt or fabricate per-activation replay.

Run only `WP-4-5-host-pure-module-extension-invocation-event-contract-r4-design`
in canonical/current/Stage 4/Stage 5 under 30/140/100/80/350 docs lines. It
authorizes no Rust or commit before acceptance and explicit r4 activation.
Freeze the same four Rust paths, 730/850/1,580 caps against `40def0e7`, all
semantics/proofs/stops, and one bounded test-name/assertion correction: fresh
complete success/failure publishes exactly one invocation-owned print prefix;
warm semantic reuse yields `ActivationKind::Reused` with no duplicate batch;
command-lineage selection/replay remains owned and already proved by
`CommandEffectOwner`, and invocation command-output integration is deferred
until a real consumer exists. Rename the overclaiming `replays_prints` test to
describe publication plus semantic reuse. `REPLAN` on events in semantic
equality, duplicate warm batches, a command consumer, fifth Rust path, cap
excess, or any behavior expansion.

## Superseded r3 implementation activation

This section is historical context only, grants no file, action, cap, or
schedule authority, and is interpreted solely through the active docs-only
event-contract correction above.

Independent review accepts the final cap correction in `86f478c0`. Implement
only `WP-4-5-host-pure-module-extension-invocation-owner-implementation-r3` in
`bzl_module.rs`, `package.rs`, private `module_extension.rs`, and `lib.rs`
solely for its private declaration, plus canonical/current/Stage 4/Stage 5
bookkeeping. Caps are 730 production, 850 tests, and 1,580 total against
`40def0e7`. Preserve the complete r2 semantics, proof, compatibility boundary,
and stops; cleanup may not reintroduce the two unrelated visibility widenings.
No fifth Rust path, public API, second evaluator, repository/global/output/I/O/
consumer/JVM breadth, or cap excess.

## Accepted docs-only cap-correction contract

This section is historical correction context only, grants no file, action,
cap, or schedule authority, and is interpreted solely through the active implementation contract above.

The frozen 720/800/1,520 stop fired after the complete required proof was
formatted: the exact four-path diff measures approximately 724 production, 846
tests, and 1,570 total lines against `40def0e7`. The excess is the already
required immutable ABI, all-request preflight/lifetime boundary, and frozen
Need/error/drift/factor/result/event/A-B-A tests; cleanup removed two unrelated
callable-visibility widenings, and no safe 50-line mechanical reduction remains
without weakening the accepted discriminators or auditability. Retain the
fully passing Rust diff unaccepted and run only
`WP-4-5-host-pure-module-extension-invocation-owner-r3-cap-design` in
canonical/current/Stage 4/Stage 5. This correction may edit only those four
plans under 30/120/100/80/330 docs lines and authorizes no Rust or commit before
independent acceptance and explicit r3 activation.

Freeze the same four Rust paths, semantics, proof, and stops at 730 production,
850 tests, and 1,580 total lines against `40def0e7`, leaving only 6/4/10 lines
of measured contingency and no margin for a field, ABI member, owner, consumer,
or behavior family. `REPLAN` on a fifth Rust path, any semantic expansion,
production above 730, tests above 850, total above 1,580, or inability to retain
the passing complete proof.

## Superseded r2 implementation activation

This section is historical context only, grants no file, action, cap, or
schedule authority, and is interpreted solely through the active
implementation contract above.

The shared string prerequisite is accepted in `40def0e7`. Resume only
`WP-4-5-host-pure-module-extension-invocation-owner-implementation-r2` in
`app/slug_loading_v2/src/bzl_module.rs`, `package.rs`, private
`module_extension.rs`, and `lib.rs` solely for its private declaration, plus
canonical/current/Stage 4/Stage 5 bookkeeping. Measure 720 production, 800
tests, and 1,520 total against `40def0e7`; no fifth Rust path or cap excess.
Preserve the accepted prepared-first all-request preflight, optional Label
None, context-owned tags, immutable exact-list ABI including negative indexes,
canonical Label str versus repr, strict None, complete-only heap-free receipt,
and invocation-owned event ordering/replay. Require the full frozen ABI,
forbidden-member/callable, Need/terminal zero-invocation, drift/factor/result/
throw, A/B/A, cold/warm, formatting, full loading/Bzlmod, and structural
heap/callable-absence proof. All repository rules, generated outputs,
environment/OS/facts, I/O, consumers, public API, second loader/evaluator, JVM,
and earlier stops remain forbidden.

## Accepted string-protocol prerequisite

This section is historical accepted evidence only, grants no independent file,
action, cap, or schedule authority, and is interpreted through the active implementation contract above.

Independent review accepts the scope correction recorded in `6215fe03`.
Implement only `WP-4-starlark-custom-string-protocol-implementation-r2` in the
six vendored starlark-rust files frozen below plus canonical/current/Stage 4/
Stage 5 bookkeeping. Caps remain 90 production, 220 tests, and 310 total
formatted net lines against `73b22cec`. Require the complete synthetic global
str/repr, percent, format, print, nesting, and cycle matrix. Preserve all
default-to-repr, string-fast-path, repr/hash/equality/type, single-protocol,
derive, public-API, and behavior stops. No app Rust, InvocationLabel, loading,
or DICE proof is authorized; those resume only with the invocation packet.

### Implementation evidence

The isolated six-file delta against `73b22cec` is approximately 31 production
and 76 test lines, within 90/220/310. The synthetic protocol matrix passes;
focused `starlark` string, interpolation, and format suites pass 73/73, 8/8,
and 9/9. Full `slug_loading_v2 --all-targets` (92 owner tests plus all
integrations) and `slug_bzlmod_v2 --all-targets` pass. Full vendored
`starlark --all-targets` passes 808 tests and retains 29 unrelated existing
profiler/bytecode golden failures; no string, interpolation, format, value, or
protocol test fails. Formatting and diff checks pass. Independent review
accepts the isolated six-file implementation and confirms the dirty app paths
remain unaccepted and unstaged.

## Accepted docs-only scope-correction contract

This section is historical correction context only, grants no file, action,
cap, or schedule authority, and is interpreted solely through the active implementation contract above.

Independent implementation review found the six shared runtime files sound but
rejected the frozen eight-file Git boundary. `module_extension.rs` is a wholly
new retained invocation owner and `bzl_module.rs` carries its retained lifecycle
tests, so landing the authorized Label override/tests would also land production
that this packet explicitly keeps unaccepted. Record only the bounded correction
`WP-4-starlark-custom-string-protocol-implementation-r2-scope-design` in
canonical/current/Stage 4/Stage 5. It may edit exactly those four plans under
30/120/100/80/330 documentation lines and authorizes no Rust, Cargo, fixture, or
commit before independent acceptance and explicit r2 activation.

Freeze the r2 implementation to the first six vendored starlark-rust files in
the successor list below, keeping 90 production/220 test/310 total caps against
`73b22cec`. Require the synthetic custom value to prove default and overridden
global `str`/`repr`, `%s`/`%r`, optimized/default and general format, print,
nested-container repr, and recursive-cycle fallback. Defer InvocationLabel and
all loading/DICE proof until the invocation packet legitimately lands. Preserve
every existing protocol semantic and stop. `REPLAN` on any app Rust file,
seventh Rust file, derive edit, reduced shared-consumer proof, public API, second
protocol, changed default behavior, or cap excess.

## Superseded initial implementation activation

This section is historical only and grants no file, action, cap, or schedule
authority; it is interpreted solely through the active docs-only correction.

Independent design review accepts the protocol owner frozen in `73b22cec`.
Implement only `WP-4-starlark-custom-string-protocol-implementation` in the
exact eight Rust files named below plus canonical/current/Stage 4/Stage 5
bookkeeping. Caps are 90 production, 220 tests, and 310 total formatted net
lines against `73b22cec`. Preserve the single default-to-repr vtable protocol,
all shared consumer proofs, the exact InvocationLabel str/repr boundary, and
every stop. A ninth Rust file, derive-crate change, public Slug API, behavior
expansion, or cap excess requires `REPLAN`.

## Accepted docs-only REPLAN contract

This section is historical design context only, grants no file, action, cap,
or schedule authority, and is interpreted only through the active docs-only
scope-correction contract above.

The r2 implementation stop fired while completing the exact Label ABI proof.
The accepted slice requires `str(label)` and `%s` to render the canonical label
while `repr(label)` and `%r` render `Label("@@repo//pkg:target")`. Live
starlark-rust hardwires non-string `str` to `collect_repr` in
`values/layout/value.rs`, the standard `str` global in
`values/types/string/globals.rs`, and percent-string interpolation in
`values/types/string/interpolation.rs`. A loading-only global override would
not affect `%s` or the other shared formatting paths, while allocating Labels
as strings would destroy type, repr, equality, and attribute semantics. The
frozen exact surface is therefore not implementable in the active four Rust
paths or 720 production lines.

Retain the current four-path Rust diff as compiling but unaccepted evidence;
it grants no implementation authority. Run only the docs/evidence packet
`WP-4-starlark-custom-string-protocol-design`. Audit one backward-compatible
starlark-rust protocol in which every value has a distinct custom `str`
projection defaulting exactly to its existing `repr`, strings preserve their
unquoted fast path, and custom values may override `str` without changing
`repr`. Enumerate every standard formatting consumer that must use the same
protocol, at minimum global `str`, `Value::to_str`, `ValueLike::collect_str`,
`%s`, `str.format` default/`!s`, and print formatting; keep `%r`, global
`repr`, debug/error rendering, hashing, equality, and type identity unchanged.
Audit the generated StarlarkValue vtable/derive path rather than adding a
Slug-only downcast or a second formatter. Require existing starlark-rust
string/format suites plus a synthetic custom value proving distinct str/repr,
and a later InvocationLabel row proving global str/repr, `%s`/`%r`, format,
print, equality/hash, and warm DICE behavior.

Compatibility is exact for the admitted formatting operations and unchanged
existing values; the private hook name, vtable layout, and Rust diagnostics are
Slug-native. Java UTF-16 edge behavior and all module-extension surfaces
already deferred by the predecessor remain unsupported/deferred. This packet
may edit exactly canonical, current, Stage 4, and Stage 5 under 45/220/180/100
and 545 total documentation lines. It authorizes no Rust, Cargo, fixture,
loading, Bzlmod, evaluator, or consumer edit before independent design
acceptance and explicit successor activation. Freeze a future Rust allowlist,
caps, tests, and stop conditions; `REPLAN` if exact semantics require a
type-specific downcast, a second string formatter, public Slug API, non-Rust
runtime, or unrelated Starlark behavior changes.

### Completed owner audit and frozen successor

The smallest implementation seam is one `StarlarkValue::collect_str` method
whose default calls that value's existing `collect_repr`. The existing
`starlark_value_vtable` derive automatically owns the new function pointer; no
derive-crate edit or parallel registry is required. Add the erased dispatch in
`values/layout/vtable.rs`, override `ValueLike::collect_str` in
`values/layout/value.rs` to preserve the string fast path and repr-stack cycle
guard while dispatching non-strings, and make `Value::to_str` use that shared
operation. The standard `str` global, percent `%s`, and the optimized/default
`str.format` paths must call the same operation. Existing `%r`, `repr`,
`collect_repr_cycle`, type errors, `fail` framing, hashes, equality, and string
allocation/interning remain unchanged. `print` already composes through
`Value::to_str`; the general format `!s` path already composes through
`ValueLike::collect_str` and receives only regression coverage.

After independent acceptance, activate only
`WP-4-starlark-custom-string-protocol-implementation` in exactly:

- `starlark-rust/starlark/src/values/traits.rs`;
- `starlark-rust/starlark/src/values/layout/vtable.rs`;
- `starlark-rust/starlark/src/values/layout/value.rs`;
- `starlark-rust/starlark/src/values/types/string/globals.rs`;
- `starlark-rust/starlark/src/values/types/string/interpolation.rs`;
- `starlark-rust/starlark/src/values/types/string/dot_format.rs`;
- `app/slug_loading_v2/src/module_extension.rs`; and
- `app/slug_loading_v2/src/bzl_module.rs` for focused invocation tests only,
  plus canonical/current/Stage 4/Stage 5 bookkeeping.

Cap the successor at 90 production, 220 tests, and 310 total formatted net
lines against the eventual accepted design commit. Require the full existing
starlark test suite and synthetic values proving default str==repr, overridden
str!=repr, nested/list/cycle behavior, global str/repr, `%s`/`%r`, optimized and
general format, and print. Require InvocationLabel canonical `str`, `%s`,
format, and print alongside unchanged `Label("...")` repr/`%r`, type,
attribute, equality, hash, and DICE success/error identity; prove cold/warm
event lineage and source A/B/A. Structural scans must show one protocol, no
Label downcast outside its override, and no retained Starlark value in DICE.
Run full loading plus direct Bzlmod dependents, formatting, diff, and archive
classification before independent implementation review.

`REPLAN` on a ninth Rust file, derive-crate change, type-specific standard
formatter branch, second protocol/registry, changed default output for any
existing value, repr/hash/equality/type-error behavior change, retained heap or
callable, public Slug API, non-Rust runtime, loading/Bzlmod semantic breadth, or
90/220/310 excess. The retained pure-invocation diff remains unaccepted and
must not resume in the same packet beyond the exact InvocationLabel override
and tests needed to prove this prerequisite.

## Accepted r2 correction contract

This section is historical correction context only, grants no file, action,
cap, or schedule authority, and is interpreted only through the active implementation contract above.

The first compiling implementation is 630 production lines against `db45d182`
before tests: 597 lines in the required private invocation/ABI owner and 33 in
narrow existing-owner accessors plus the private module declaration. Independent
review finds no credible 110-line mechanical reduction without collapsing
distinct Starlark values, typed error/event orchestration, or lifetime
boundaries. Retain that unaccepted four-path Rust diff unchanged while this
docs-only packet corrects the future caps to 720 production, 800 tests, and
1,520 total lines against `db45d182`. The margin is solely for the semantic
corrections below, not another owner, field family, or behavior surface.

Freeze four corrections for r2. First, optional Label attributes prepared as
`CoercedAttributeValue::None` allocate Starlark `None`; no accepted prepared
value reaches `unreachable!`. Second, reacquire and validate every exact Host
module, factor set, manifest, export, and definition into ephemeral lifetime-
only preflight rows before invoking any callable. Any Need or terminal during
preflight performs zero user invocation and publishes zero invocation events;
only a complete preflight invokes rows in encounter order. Third,
`is_dev_dependency(tag)` and `tag_sort_key(tag)` accept only tags minted by that
exact ephemeral context. Use an invocation-local nonsemantic ownership token;
reject foreign/captured tags without retaining the token in DICE equality.
Fourth, `ctx.modules` and every `module.tags.<class>` value are immutable
Starlark lists with exact list iteration/index/length behavior; mutation such
as `.append` fails closed. The module, tags structure, tag instances, Labels,
and sort keys remain immutable as already frozen.

Require omitted optional-Label `None`; two-request first-print/second-Need
zero-invocation then restoration-order; cross-context and captured-tag
rejection; and mutation negatives for both modules and tag-class lists. Keep
all original ABI positives, every forbidden-name/captured-callable probe,
strict-None/wrong-result/throw/print ordering, lifecycle/A-B-A, event replay,
heap-absence, full-suite, cleanup, and independent-review proofs.

This correction may edit exactly canonical, current, Stage 4, and Stage 5,
under 30/150/120/120/420 docs caps. It authorizes no Rust, fixture, Cargo,
BUILD, public API, or semantic implementation until independent acceptance and
explicit r2 activation. Preserve the same future four Rust paths and every
existing stop. `REPLAN` on production above 720, a fifth Rust path, retained
heap/callable/token, a second loader/evaluator, public API, repository/global/
I/O/generated-output breadth, or inability to make all preflight terminals
side-effect-free.

## Accepted design contract

This section is historical design context only, grants no file, action, cap,
or schedule authority, and is interpreted only through the active docs-only
scope-correction contract above.

Perform a read-only ownership audit for one callerless loading-owned DICE leaf
that computes `HostPreparedModuleExtensionInputsKey`, reacquires each exact
request through the sole `HostBzlModuleEvalKey`, verifies manifest/export/
definition identity, creates ephemeral read-only module/tag/context Starlark
values, invokes the lifetime-owned callable, and accepts only a `None` result.
The retained result must be heap-independent and include the complete prepared
predecessor, exact request/manifest/definition factor identity, invocation
outcome, and complete typed success/error context.
Never retain a `FrozenValue`, heap, callable, or runtime context in DICE.

Admit only the root-main singleton ordinary nonisolated input already prepared
by the accepted scalar owner, definitions with `environ = []`,
`os_dependent = false`, `arch_dependent = false`, and `facts_version = 0`, a
read-only `ctx.modules`, no repository-rule calls, and an implementation that
returns exactly `None`. Freeze preparation-before-load, request-order
reacquisition, module/tag/attribute/dev/location visibility, callable error and
print event ownership, strict result validation, and complete-only equality/
validity. A prepared terminal or Need must perform zero reacquisition work.

Pinned Bazel 9.2 commit `8220c619` anchors the admitted ABI in
`ModuleExtensionContext`, `StarlarkBazelModule`, and `TypeCheckedTag`. The
ephemeral `module_ctx` exposes exactly `modules`, `is_dev_dependency(tag)`, and
`tag_sort_key(tag)`. `modules` is an immutable one-element root-BFS list.
`is_dev_dependency` reads the prepared tag bit; `tag_sort_key` returns an
immutable opaque value ordered by `(module_index, tag_index)`. `facts`,
`is_isolated`, `root_module_has_non_dev_dependency`, `extension_metadata`, and
all inherited external-context members are unsupported in this slice. In
particular `wait`, `download`, `download_and_extract`, `extract`, `file`,
`getenv`, `path`, `read`, `watch`, `report_progress`, `os`, `execute`,
`load_wasm`, `execute_wasm`, and `which` are absent and access fails before any
side effect. The shared `.bzl` globals continue to omit `repository_rule` and
repository-rule callables; require a negative probe for every forbidden
context/global name, including a callable captured through a load.

The immutable root `bazel_module` exposes exactly `name: string`, normalized
`version: string` (including the empty sentinel), `is_root: bool = true`, and
`tags`. `tags` has one field for every declared tag class, including an empty
immutable list when unused; each list preserves source order. A tag is an
immutable structure with exactly the declaration-order schema fields and the
prepared String/Bool/i32/Label values. Dev-dependency is not a tag field and is
visible only through `ctx.is_dev_dependency`; logical location is not a field
and participates only in tag debug/error rendering. Cross-class source order
is visible only through `ctx.tag_sort_key`. Unknown tag-class and attribute
accesses fail with their typed class/attribute distinction; mutation of the
context, module, tags container, tag lists, or tag values fails closed.

An admitted Label is an immutable canonical-label Starlark value: equality,
hashing, `str` as the unambiguous canonical label, `repr` as
`Label("@@repo//pkg:target")`, and the pure `name`, `package`, `repo_name`,
deprecated `workspace_name`, and `same_package_label(target_name)` surface are
exact. `workspace_root`, deprecated mapping-sensitive `relative`, construction
through a global `Label`, and every filesystem/target lookup are unsupported;
probe each. The main repository uses `@@//...` and an empty repo name. No label
operation may observe a package, target, route, or filesystem.

Invocation owns a fresh local event capture. Loader events remain solely the
existing Host-bzl key's evaluation data. Successful or failed invocation
publishes its complete print prefix (including print-before-throw ordering) as
the invocation key's evaluation data and replays that batch on warm reuse. The
heap-independent semantic receipt retains only complete structural inputs and
the typed invocation outcome; event content is not semantic equality and no
event identity is stored in that receipt. Preparation and reacquisition
terminals publish no invocation events.

Exact compatibility is limited to that Bazel 9.2 slice: preparation/load/
invocation order, root module and tag identity/order, admitted scalar values,
dev/location state, callable failures, and strict `None`. Private Rust wrapper
layout, diagnostic wording, event carrier, and nonobservable internal
scheduling are Slug-native. Deferred are nonroot/MVO/isolation/innate inputs,
environment/OS/arch/facts observation, extension metadata, repository-rule
proxies/calls, generated names/RepoSpecs/existence, override/inject final
validation, lockfile replay/write, filesystem/network/download/execute work,
materialization, commands/consumers, and exact JVM identity bytes.

This docs-only packet may edit exactly canonical, this manifest, Stage 4, and
Stage 5. Cap growth at 45 canonical, 260 manifest, 240 Stage 4, 220 Stage 5,
and 765 total lines. Require pinned Bazel 9.2 source/test anchors; live owner,
visibility, callable-lifetime, event, and representation audits; an explicit
future allowlist/caps/proof/stops; and independent design review.

A credible future implementation may use only
`app/slug_loading_v2/src/bzl_module.rs`,
`app/slug_loading_v2/src/package.rs`, one new private
`app/slug_loading_v2/src/module_extension.rs`, and
`app/slug_loading_v2/src/lib.rs` solely for `mod module_extension;`, initially
capped at 520 production, 800 tests, and 1,320 total lines. Require prepared
Need/error with zero bzl activation; exact ordered reacquisition; missing,
private, wrong-kind, manifest, export, and definition drift; unsupported factor
preflight; callable-visible module/tag/attribute/dev/location order; empty and
multiple tags; `None`, wrong-result, throw, and print rows; contextual error
A/B/A; source/callable/manifest/prepared-tag A/B/A; cold/warm reuse; Need
invalidity; structural absence of retained heaps/callables and repository-rule
globals; field-by-field ABI positives plus every forbidden-name probe; cold/
warm print replay and throw-with-prior-print order; full loading/Bzlmod direct-
dependent suites; cleanup and independent review.

`REPLAN` on any environment/OS/facts observation, repository-rule global or
call, generated output or metadata, I/O, retained Starlark heap/value/callable,
second loader/evaluator, Bzlmod mutation or reverse dependency, public generic
API, need for broader attribute containers, result other than strict `None`, a
fourth Rust file beyond the three semantic files plus private `lib.rs`
declaration, cap excess, or inability to make the invocation receipt fully
heap-independent. No Rust or fixture is authorized before independent design
acceptance and explicit implementation activation.

## Accepted composition implementation evidence

This section is historical evidence and grants no file, action, cap, or
schedule authority. Independent review accepts the implementation at 414
production, 529 test, and 943 total formatted net lines against `aee502ff`.
The callerless owner computes raw inputs first, borrows the sole definition
loader, performs the exact supplied-map then schema-order scalar coercion and
label visibility checks, retains every predecessor/error context, publishes no
events itself, and keeps callables, contexts, execution, I/O, and generated
repositories absent. Focused and full loading tests, full prior Bzlmod tests,
format/diff/scope/cleanup checks, and two independent reviews pass.

## Predecessor design record

This section is historical context only and grants no files, actions, caps, or
scheduling authority.

Perform a read-only ownership audit for one callerless loading-owned DICE key
that composes the accepted heap-free definition aggregate with the accepted
heap-free selected evaluation-input aggregate. It must prepare typed root
module/tag views but must not reacquire or publish a callable, construct
`module_ctx`, or execute an extension.

Pinned Bazel 9.2 `SingleExtensionEvalFunction` obtains the selected usage value
before `RegularRunnableExtension.load`; `StarlarkBazelModule.create` then walks
modules/tags, looks up each tag class, and calls `TypeCheckedTag.create` with a
label converter for that module's repository mapping. Audit and freeze:

- raw selected-input computation before definition loading, including completed
  raw-input error/Need precedence and zero Host-bzl observation on that terminal;
- exactly one join by the complete accepted load request, rejecting absent,
  duplicate, reordered, or extra definition/input rows rather than matching
  only label or exported name;
- root-module encounter order, source-order tags, tag-class declaration order,
  module/tag sort indices, dev-dependency and logical-location retention;
- tag-class lookup, unknown/missing attribute, mandatory/default, raw-value
  type checking, and exact first-error order for the admitted schema;
- label/default conversion through the exact request context repository and
  immutable selected mapping, with every semantic input retained structurally;
- a heap-independent prepared value retaining both complete predecessors,
  exact load request, manifest/schema identity, module identity, typed/defaulted
  attributes, dev flag, location, and ordering identity;
- complete-only DICE equality/validity and contextual typed terminals. Need is
  invalid and non-self-equal.

The first successor admits exactly this matrix. `None` in the supplied map is
omission for every admitted kind; a mandatory omitted value fails. Declared
defaults are the already-coerced definition-owned values below; absent optional
defaults use the listed intrinsic value.

| kind | supplied raw shape | declared/intrinsic default | conversion owner |
|---|---|---|---|
| `String` | `String` only | `String` / `""` | scalar projection |
| `Boolean` | `Bool` only | `Boolean` / `false` | scalar projection |
| `Integer` | `Int::Small(i32)` only | `Integer` / `0` | scalar projection |
| `Label` | `String` or retained `Label` token | canonical `Label` or `None` / `None` | supplied values use the module context repository plus the request's immutable selected mapping; declared defaults remain the definition-load owner's already-canonical value and are not re-resolved |

Reject a mismatched declared-default variant. Defer `LabelList`,
`StringKeyedLabelDict`, `LabelKeyedStringDict`, `LabelListDict`, `Output`,
`OutputList`, `StringList`, `StringListDict`, and `StringDict`; raw list and
tuple stay distinct but both fail closed, as do every dictionary shape,
big-decimal integer, float token, builtin-print token, extension proxy, and
self-list. `allow_single_file` remains structural schema identity but causes no
file observation or target validation in this pre-execution owner. Definition
loading already rejects nondefault unprojected restrictions including allowed
values, so the admitted phase has no allowed-value predicate to run.

Freeze Bazel's two-phase per-tag algorithm exactly. First walk the retained
supplied `SmallMap` order, skip `None`, and fail at the first unknown name or
raw type/label-conversion error. Then walk declaration-order schema slots, fail
on the first missing mandatory value, insert the declared or intrinsic default,
and fail on the first non-visible label. Publish only after all source-order
tags complete. Duplicate raw names are impossible in the retained `SmallMap`
and are rejected by the existing MODULE evaluator/syntax owner; composition
adds no fabricated duplicate check. Reuse the existing loading schema and
compact/Buck2-derived containers; do not add a second raw-value or schema owner.

## Compatibility boundary

Exact only for the admitted root-main, ordinary, nonisolated, singleton-module
Bazel 9.2 slice: usage-before-load ordering, tag-class/type/default semantics,
module and source tag order, label resolution, dev identity, and structural
invalidation. Slug-native: private key/type names, compact layout, diagnostic
wording, and internal scheduling where Bazel exposes no user-visible order.
Deferred: nonroot/MVO/isolation/innate inputs, callable reacquisition,
`module_ctx`, facts/environment/OS inputs, implementation execution/events,
repository rules, generated names/RepoSpecs/existence, override/inject final
validation, lockfile replay/write, materialization, commands/consumers, and
exact JVM identity bytes.

## Scope, proof, successor, and stops

The completed docs-only packet edited exactly canonical, this manifest,
`04-starlark-loading-and-build-packages.md`, and
`05-bzlmod-and-repository-graph.md`. Cap net growth at 45 canonical, 240
manifest, 220 Stage 4, 220 Stage 5, and 725 total lines. Require pinned Bazel
9.2 source/test anchors, live visibility and error-order audit, exact versus
Slug-native/deferred classification, compact-utility review, explicit future
allowlist/caps/proof/stops, and independent design review.

The active successor is limited to
`app/slug_loading_v2/src/bzl_module.rs` and
`app/slug_loading_v2/src/package.rs`, with colocated tests and initial caps of
420 production, 700 tests, and 1,120 total lines. Require paired positive and
negative pure rows for every matrix/default/error/order branch and every named
fail-closed family; real
DICE raw-error/Need-before-load, definition error/Need, absence/multiple order,
label-mapping/default/tag/order/dev/location A/B/A, retained-error-context
A/B/A, cold/warm reuse, and events. A raw terminal performs zero Host-bzl
observation; definition loading after successful raw input may publish only its
accepted loader events; composition publishes none. Require full Bzlmod/loading
suites, format/diff/scope/forbidden-edge/cleanup audits, and independent review.

No other Rust, Cargo/BUILD, fixture, schema widening, callable/heap handle,
`module_ctx`, execution, I/O, generated-repository, lockfile, materializer,
consumer, JVM/Java, or source-owner change is authorized. `REPLAN` on a
second loader/evaluator, Bzlmod mutation, public generic API, a third future
Rust file, unbounded attribute coercion, unresolved error order, or cap excess.

## Accepted predecessor evidence

The loading definition owner accepted in `bf2c36e9` retains complete request,
manifest, schema, factor declaration, and error identity while the callable
remains frozen-lifetime-only. The selected raw-input r2 implementation is
accepted at 263 production, 304 test, and 567 total lines against `a31cf3d9`;
it retains complete request/error context, exact root identity and source-order
raw tags, excludes unrelated graph/files/lockfile state, passes the full
Bzlmod/loading suites, and has independent `ACCEPT` review.
