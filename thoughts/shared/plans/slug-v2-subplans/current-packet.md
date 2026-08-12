# Current Slug V2 Packet

Packet: `WP-4-5-host-module-extension-definition-loading-owner-implementation`
Milestone: M7 module-extension definition loading implementation
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: implement and validate the independently accepted two-file loading
owner.

## Active implementation contract

Implement the accepted design below only in
`app/slug_loading_v2/src/package.rs` and
`app/slug_loading_v2/src/bzl_module.rs`, with canonical/current/Stage 4/Stage 5
bookkeeping. Caps are 440 production, 650 tests, and 1,090 total formatted net
Rust lines, measured against `f17bd250`. Complete the frozen proof matrix,
protected suites, compact/cleanup/scope audits, and independent review.

No third Rust file, public API, source/evaluator key, selected-owner mutation,
extension execution, generated-repository work, I/O, materializer, lockfile,
consumer, JVM/Java, or behavior-family expansion is authorized. Cap excess or
any need for a second/purpose-split loader or retained heap/callable is
`REPLAN`.

## Accepted design contract

This section is historical design authority interpreted only through the
active implementation contract above.

Freeze one callerless loading owner that:

- computes `HostSelectedExtensionDefinitionLoadRequestsKey` first and preserves
  its Need/completed-error boundary before any bzl source work;
- converts each admitted root-main canonical label into the existing Host bzl
  label domain and borrows `HostBzlModuleEvalKey` as the sole source, transitive
  load, parse, evaluation, freeze, event, manifest, and lifetime owner;
- adds only the Bazel `.bzl` definition globals needed for `tag_class()` and
  `module_extension()` to the existing loading globals, without a second
  evaluator, source key, load graph, or frozen-module cache;
- validates the requested public export after the complete bzl load, retains
  the exact request and `BzlLoadManifest`, and projects a compact,
  heap-independent definition: exported name, ordered tag-class attribute
  schemas, ordered environment declarations, OS/architecture dependency bits,
  and nonnegative facts version;
- leaves the implementation callable only inside the existing cached
  `FrozenBzlModule` lifetime. No `FrozenValue`, heap, callable, module, or
  caller-supplied source/mapping may enter definition equality or the projected
  value.

The audit resolves those questions as follows:

- accept the implementation only through the vendored
  `StarlarkCallable` argument conversion; reject a noncallable at the
  `module_extension()` call before definition publication;
- accept ordered `tag_class` dictionaries whose values use the existing
  `AttributeDefinition` algebra with no transition and no explicitly set
  configurable policy. Project every modeled semantic field structurally:
  kind, mandatory, effective configurable value, coerced default, and
  allow-single-file. No concrete descriptor may be admitted while carrying an
  unprojected nondefault option. The current globals reject unmodeled
  values/allowed-values, allow-empty, providers, executable, cfg, aspects and
  other label/file restrictions during bzl evaluation; keep those failures
  closed rather than silently dropping an option or widening tag evaluation;
- use the shared bzl environment. Bazel's `RepositoryBootstrap` installs these
  methods in its ordinary bzl environment, so a purpose-split Slug key would be
  a second, less exact load graph. Existing Slug globals keep their current
  meanings; only the two missing exact names are added;
- compute admitted requests in retained encounter order. Within one request,
  source/load/parse/evaluation/freeze errors precede public export lookup;
  missing/private/wrong-kind exports are typed definition errors. Across
  independent requests, first retained terminal/Need is explicitly
  Slug-native because Bazel evaluates separate extension SkyKeys and exposes no
  aggregate error order;
- retain the entire accepted request aggregate and, per definition, the exact
  request plus complete `BzlLoadManifest`, ordered tag schemas, ordered
  environment strings, OS/architecture bits and facts version. Source,
  transitive-load, export, schema, declaration, and request changes therefore
  participate structurally without pointer identity.

Exact for the admitted Bazel 9.2 slice: selected-request-first ordering,
root-main canonical bzl resolution, source/load/export/type validation,
callable acceptance, tag-class schema construction, declaration values, and
transitive source identity. Slug-native: private key/type names, compact
containers, diagnostic text and cross-extension scheduling where Bazel exposes
no user-visible order. Unsupported/deferred: isolated/MVO/innate/nonroot or
registry definitions, external-repository bzl loads, repository-rule globals
beyond the current root-local loading slice, extension
execution/context/module/tag views,
environment or OS reads, generated repositories/names/RepoSpecs/existence,
override/inject final validation, evaluation factors, lockfile replay/write,
materialization, loading/command consumers, and exact JVM identity bytes.

## Evidence and feasibility anchors

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
constructs `ModuleExtension` from a callable, ordered tag-class map, environment
list, OS/architecture bits, and nonnegative facts version. Its
`RegularRunnableExtension.load` validates the bzl label, loads the complete bzl
module, then looks up the requested public export and verifies its
`ModuleExtension` type before environment observation or execution.

Slug already owns that root-main source/load closure in
`bzl_module.rs::HostBzlModuleEvalKey`; `FrozenBzlModule` equality is exactly its
complete `BzlLoadManifest`, while frozen modules are lifetime-only state. The
shared `package.rs::loading_globals()` already owns `attr.*` descriptors and
the freeze/export patterns needed for a compact definition value, but has no
`module_extension` or `tag_class`. The accepted hidden Bzlmod request boundary
in commit `d0d7bde7` exposes only workspace, ordered canonical label/export,
context repo, and immutable selected mapping while retaining its predecessor
privately.

## Scope, caps, proof, and stops

This design packet may edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest;
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`; and
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`.

Cap net design growth at 45 canonical, 240 manifest, 220 Stage 4, 180 Stage 5,
and 685 total lines. Require pinned-source citations, live owner/visibility and
crate-edge audit, compact/Buck2 representation review, explicit exact/
Slug-native/deferred classification, an auditable future file allowlist/caps,
discriminating pure and real-DICE proof, and independent design review.

After independent acceptance, the implementation successor may edit only:

- `app/slug_loading_v2/src/package.rs`;
- `app/slug_loading_v2/src/bzl_module.rs`;
- canonical/current/Stage 4/Stage 5 bookkeeping.

Cap that successor at 440 production, 650 tests, and 1,090 total formatted net
Rust lines. Require pure rows for callable/noncallable, tag and attr insertion
order, duplicate/malformed descriptors, negative facts version, environment
order, and missing/private/wrong-kind exports. Add a negative row for each
unprojected option family accepted by neither the frozen descriptor nor output,
and A/B/A structural-equality rows for every retained kind/mandatory/
configurable/default/allow-single-file field. Require real-DICE selected-
request error/Need with zero bzl observation; root source and transitive-load
change/restoration; export/schema/environment/factor A/B/A; absent and multiple
requests in encounter order; Need invalidity/non-self-equality; typed child
source/load errors; event publication only from complete bzl evaluation; and
cold/warm reuse. Run the full loading and Bzlmod suites plus formatting,
diff/scope, forbidden-edge, compact and cleanup audits and independent review.

No Cargo/BUILD, fixture, public
API, source/evaluator key, selected-owner
mutation, extension execution, generated-name/spec fabrication, I/O,
materializer, lockfile, consumer, JVM/Java, or generic loading activation is
authorized. `REPLAN` if exactness requires a purpose-split or second bzl loader,
a retained Starlark heap/callable, repository-rule/evaluation breadth, a public
definition surface, more than two Rust files, or an unresolved
callable/schema/error-order boundary.

## Accepted predecessor evidence

Commit `d0d7bde7` is independently accepted at 205 production, 236 tests, and
441 total lines in the authorized two files. It computes selected mappings
once, publishes only the hidden heap-independent request boundary, preserves
typed predecessor errors and invalid Need validity, fails closed on every
deferred usage shape, and proves order/dedup/change/restoration/warm reuse.
Focused tests, the full Bzlmod and loading suites, formatting/diff/scope audits,
and independent implementation review pass.
