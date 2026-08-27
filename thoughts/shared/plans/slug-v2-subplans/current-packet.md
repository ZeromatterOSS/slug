# Current Slug V2 Packet

Packet: `WP-4-5-7A-canonical-repository-load-route-implementation-r3`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `822ad6ca0`.

Result: add the one apparent-free canonical repository source/load route and
shared canonical source-file/directory-listing projections required before
selected-module registration patterns can reuse package loading. Activate no
package, target-pattern, registration, configured, rule or action behavior.

## Learned facts and accepted decision

The first implementation candidate reached terminal `REPLAN` after its one
allowed test-only correction; no Rust was retained. Its semantic shape fit the
accepted design and all serial validation passed, but independent latest-diff
review found three proof gaps: only the built-in source/listing wrappers logged
their exact deepest owners; route failure/Need and generated-effect failure did
not freeze short-circuit/epoch behavior; and structural hash plus retained-size
coverage did not exercise route source/mapping/effect-plan changes. The
candidate measured 897 production lines only by placing eleven cohesive
drivers/accessors under `rustfmt::skip`; normal formatting measures 1,118
production lines including wiring. R2 preserves the design and six-file
allowlist, raises only the honest formatting cap, and makes the missing matrix
an explicit pre-commit gate.

R2 compilation established one correction to that matrix. The selected
repository file effect has no observation input independent of its canonical
route: removing `ext.bzl` makes validated route discovery Need first, so the
load route must suppress the effect dependency. The admitted repository-rule
effect ABI performs only `ctx.file` and cannot create a later source-read Need.
Therefore an effect-only Need after route success is unreachable; proof must
freeze shared-input route Need/no-effect and generated semantic-effect failure
with exact route-then-effect observations. Do not fabricate a private effect
Need or widen the ABI for a test.

R2 then implemented the same boundary at 1,174 production and 1,097 proof
lines. Focused 13/13, full bzlmod 576/576, full loading 352/352, direct
dependents, locked CLI, formatting and archive-baseline validation passed.
Independent terminal review still rejected the proof contract: its local and
registry successes remained admitted by root aliases, so they did not prove
canonical-only selection, and its hash table did not vary a selected
repository specification independently from that selected repository's
mapping. This is the second material correction, so no R2 Rust is retained.

R3 changes no production decision. Its proof fixture must select a transitive
registry repository that is absent from the root module's apparent mapping,
materialize it through its canonical name, and demonstrate canonical source
file and directory-listing success. The test must first assert that the root
mapping cannot admit the repository. Separate A/B/A rows then keep canonical
identity fixed while changing only that selected repository's `source.json`
specification, and while changing only its final repository mapping through
its own MODULE dependency declarations. This requires an honest proof cap;
do not compress the registry graph into an alias-bearing surrogate.

Commit `496168758` supplies one loading-owned
`HostCanonicalRepositoryRouteKey` over workspace plus canonical repository.
Its carrier has no root-apparent alias and retains the five accepted source
dispositions. Generated routes retain only the selected owner/ordinal effect
seed; route and mapping lookup intentionally do not activate source effects.

The live external loading chain cannot consume that route. Its
`HostRepositoryDirectoryListingKey`, `HostExternalPackageBoundaryKey`,
`ExternalSubtreePackageSetKey`, `RepositoryPackageSourceKey`, external `.bzl`
keys and `RepositoryPackageLoadKey` all retain `RootRepositoryRoute`, whose
identity includes a root-apparent alias. A selected module may own or map a
canonical repository with no root-visible alias, so fabricating an apparent
name or sending canonical text through `RootRepositoryRouteKey` is invalid.

The lower materialization owner is already canonical: its request/result
identity is workspace plus canonical repository, and its Host/materialization
path observations are shared independently of the presentation route. Add one
bzlmod source input that retains an `Arc<HostCanonicalRepositoryRoute>` plus
the already-required materialization disposition. A generated input alone
also retains the computed `GeneratedRepositoryFileEffectPlan`; other source
dispositions reject such a plan. It contains no apparent spelling, physical
root, namespace or source tree.

Add one loading `HostCanonicalRepositoryLoadRouteKey`, with an observed
sibling, over workspace plus canonical repository. It computes the existing
canonical route first. Only a generated success then computes the existing
`HostSelectedRepositoryFileEffectKey`; built-in and selected successes project
directly, root is a typed non-external terminal, and no failure admits another
source lookup. Source-input/materialization projection runs only after those
predecessors and any projection failure is a typed terminal retaining their
observations. The resulting load-route value owns exactly one canonical source
input and exposes its predecessor route for mapping and canonical-label
context. It does not copy the route mapping or repository specification beyond
the existing materialization-request projection required by the Need protocol.

Add canonical source-file observation and direct-directory-listing keys in
bzlmod. They share the existing built-in catalog, repository materialization,
path-resolution and directory-listing drivers rather than creating another IO
or materialization owner. Existing `RootRepositoryRoute` keys remain exact
adapters and are not retyped in this packet. A later bounded migration will
make external boundary/subtree/package-loading consumers accept the canonical
load route before the shared registration expander is implemented.

This is a deliberate bridge, not a permanent parallel source graph. The
temporarily duplicated address-level wrappers violate the preferred single
public adapter shape, not source identity: both converge on the existing
canonical materialization and path DICE keys. Delete the canonical/root wrapper
split when `HostExternalPackageBoundaryKey`, `ExternalSubtreePackageSetKey`,
`RepositoryPackageSourceKey`, external `.bzl` keys and
`RepositoryPackageLoadKey` have moved to the canonical load route. That
migration is owned by the immediate successor
`WP-4-5-7A-canonical-external-package-loading-adapter`; structural dependency
tests in this packet must prove that neither wrapper owns a second
materialization or path observation.

## Bazel, Buck2 and Zabel basis

Pinned Bazel 9.2
`RegisteredToolchainsFunction#getBzlmodToolchains` and
`RegisteredExecutionPlatformsFunction#getBzlmodExecutionPlatforms` parse each
selected module's raw registrations with that module's canonical repository
and full mapping. `TargetPatternUtil.expandTargetPatterns` then preserves
signed-pattern order and first-seen set order before family-specific configured
work. `RegisteredToolchainsFunctionTest#testRegisteredToolchains_bzlmod` and
the corresponding execution-platform bzlmod test prove selected-module
context; their wildcard/order tests remain the later expander evidence. This
packet activates none of those named surfaces, so it needs pinned-source and
structural regression evidence, not a new oracle fixture.

Slug's `docs/developers/dice.md` and Buck2 DICE
`writing_computations.md`, `incrementality.md` and `cancellations.md` establish
dependency recording, equality cutoff, shared in-flight work and cancellation
release. Zabel's `selected_registration_patterns.zig`,
`registered_labels_projection.zig` and
`session_recursive_package_discovery.zig` are concept/test-only guidance for
keeping declaration context, authenticated source identity, expansion and
configured consumption separate. Copy no Zig implementation, diagnostic or
compatibility claim; Bazel 9.2 remains behavior authority.

## Builtin and rule boundary

This is generic repository/loading infrastructure. Bazel 9 BCR Starlark owns
rule definitions and control flow, including `cc_internal`; `cc_common` is one
demanding consumer of the generic Rust host-builtin ABI, not a Rust C++ parser
or rule engine. Future builtins remain grouped by reusable category: values and
carriers; provider/rule/aspect declarations; collections/depsets; labels and
target patterns; actions/artifacts; configuration/fragments/toolchains; and
repository/loading services. This packet changes only the last category's
source route.

## DICE, request and lifetime contract

Follow `docs/developers/dice.md`. Legacy and observed load-route drivers share
one finisher and preserve outer error before Need before semantic-terminal
order. Route failure is terminal. Generated effect failure is terminal and its
observed epoch follows the route epoch; all non-generated successes forward
only the route epoch. Source-input projection failure follows those natural
predecessors and retains the accumulated epoch. Canonical source-file/listing
observed keys own only their source/path epochs; a later composite consumer
must merge the observed load-route epoch first instead of storing observations
inside semantic key identity. Need and cancellation publish no complete value.
Keys use complete-only equality and validity, and no lock spans a DICE compute.

The load-route key identity is workspace plus canonical repository. The
retained value is DICE semantic memory: one immutable reference to the complete
canonical route and one existing materialization disposition; generated adds
one effect plan through that disposition. Mapping, repo specification, local
path policy, built-in snapshot and effect plan remain structural through the
predecessor/input equality. Scratch projections and epoch merges are compute
memory. There is no service cache, command heap borrow, async transfer, manual
eviction or shutdown owner.

Materialization Needs keep the existing immutable request projection and
request/revision behavior. Overlapping requests for the same canonical key
share DICE work. Existing source/result/path owners remain the final observed
input boundary, so this packet adds no historical filesystem assumption,
watcher, direct IO, materializer, request retry or final-validation policy.

## Compatibility

- **Exact:** no named Bazel surface is activated. Existing root-apparent
  route/source/file/listing results, errors, Need order, observations and
  diagnostics remain unchanged. Canonical source projections use the same
  built-in/materialization/path facts as an equivalent admitted root route.
- **Slug-native:** canonical source/load carrier layout, key names, typed root
  rejection, structural equality/hash, observation carriers and memory
  accounting.
- **Unsupported/deferred:** canonical external package boundary, subtree and
  package loading; target-pattern mapping/ambiguity/expansion; family filters
  and dedupe; configured providers/settings; options; rule implementations;
  actions and exact configuration/output identity.

## Implementation allowlist and complexity caps

Only these files may change:

- `app/slug_bzlmod_v2/src/source_preparation/canonical_repository_source.rs`
  (new carrier plus canonical file/listing owners);
- `app/slug_bzlmod_v2/src/source_preparation.rs` (module wiring and bounded
  extraction/shared-driver hooks only) and `app/slug_bzlmod_v2/src/lib.rs`
  (doc-hidden exports);
- `app/slug_loading_v2/src/canonical_repository_load_route.rs` (new DICE
  owner), `canonical_repository_load_route_tests.rs` (new focused proof), and
  `lib.rs` (wiring/exports).

No Cargo, fixture, oracle, identity parser, MODULE evaluator, query, core,
analysis, CLI, package loader, subtree, rule or action file may change.
Production growth is capped at 1,200 lines and proof growth at 1,450 lines; no
new function exceeds 120 lines, the canonical source module stays below 700
lines, and the loading route module stays below 500 lines. New production
drivers must be normally rustfmt-formatted; `rustfmt::skip` is not allowed in
the new modules. `source_preparation.rs` is already above the 2,000-line complexity
trigger: add at most 100 production lines there and put the cohesive new owner
in its submodule. Mechanical shared-driver extraction does not authorize
unrelated cleanup.

Reuse existing `Arc`, `Dupe` and `Allocative`; use existing structural hashing
and compact route/mapping state. Add no `String`/`Vec` copy of mapping or route
data, new interner, `HashMap`, global cache, dependency or Stage 9 ledger row.
The only extra retained request data is the already-defined source/materialize
Need projection. If implementation needs a copied mapping, second repo-spec
carrier, new collection utility or source side table, STOP and replan.
Record retained-size accounting for one route `Arc`, one disposition/request
handle and the generated-only plan. No benchmark is required for this
callerless substrate because no execution hot path is activated; wrapper-node
count remains a named residual until the immediate successor deletes the
address split.

## Required proof

- all nonroot built-in, selected-registry, selected-nonregistry and generated
  route dispositions construct an apparent-free canonical source input; root,
  missing generated plan and extraneous non-generated plan fail closed;
- load-route legacy/observed success, route-first failure/Need order,
  generated-only effect activation, exact epoch forwarding/merge,
  complete-only equality/validity, cancellation nonpublication and A/B/A;
- a table-driven failure matrix proves selected route failure and shared-input
  route Need publish no effect dependency, while generated semantic-effect
  error stops before projection and preserves exactly route then effect
  observation prefixes; effect-only Need is unreachable under the admitted
  ABI and must not be simulated;
- built-in, local, immutable-registry and generated canonical source-file and
  direct directory-listing results match equivalent existing ordinary or
  root-build adapters where that adapter admits the alias; additionally, a
  transitively selected immutable-registry repository absent from the root
  mapping succeeds directly by canonical name, with the absence asserted
  before source and listing evaluation;
- dependency logging for each of those four dispositions proves the exact
  built-in catalog or existing materialization-result/path-resolution/source/
  listing children, with no `HostRepositorySourceObservationKey`, second
  filesystem/materialization owner or route/mapping effect activation;
- structural guards prove no apparent spelling or physical source root enters
  the canonical input/load route and no package/subtree/target-pattern/
  registration key is activated; and
- table-driven hash/equality discriminators independently vary workspace,
  canonical repository, built-in/selected/generated disposition, the selected
  repository's source specification alone, its final mapping alone, and the
  generated effect plan; selected rows keep canonical identity fixed and
  derive spec/mapping variation from separate transitive-registry graphs;
  equality remains authoritative even if a weak hash collides; `size_of` plus
  structural-field guards account for the inline route `Arc` and disposition
  handle and prove that the generated plan is reachable only through the
  existing request, with no copied mapping, physical root or source bytes.

Run focused new owner tests, the full bzlmod and loading suites, locked checks
for loading/core/query, then a locked `slug_cli_v2` build serially. Run Rust
formatting, `git diff --check`, scope/cap/dependency/no-lock/duplicate-owner
guards and `scripts/v2_archive_status.sh`. Require independent terminal review
for the DICE/public-boundary/retained-representation change.

## Stops

STOP and `REPLAN` for an apparent name in canonical source/load identity; a
fabricated alias; source effect work before canonical-route success or for a
non-generated route; a second effect/materialization/path semantic owner;
direct filesystem/catalog/materializer access from loading; copied mapping or
duplicated retained repo specification beyond the existing request projection;
root-route behavior change; package, subtree, target-pattern, registration,
configured, rule or action activation; new dependency; public named surface;
global state; lock across compute; function/module/line cap; or allowlist
expansion.

## Immediate predecessor and successor

Commit `496168758` moved the sole canonical route/mapping DICE ownership to
loading and was independently accepted; `f86bbafdb` froze this source/load
design. Independent pre-review accepted the boundary, effect order, retained
identity and wrapper deletion contract. Both rejected implementations changed
no accepted behavior and retained no Rust; R3 owns only the corrected
alias-free and independent-identity proof contract above. The immediate
successor after this packet is the bounded canonical
external package-loading adapter, followed by the one shared toolchain/
execution-platform registration expander and only then configured consumers.
