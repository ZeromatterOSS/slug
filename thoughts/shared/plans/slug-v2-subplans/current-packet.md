# Current Slug V2 Packet

Packet: `WP-5-host-selected-extension-mapping-owner-design-r2`
Milestone: cross-stage M7 prerequisite design
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: audit and freeze the first additive selected-extension mapping owner,
or return `REPLAN` at the first missing post-selection semantic leaf.

## Active design contract

Perform a read-only ownership and pinned Bazel 9.2 audit now that commit
`11be92b9` retains complete root extension usages. Determine whether one
callerless private Host owner can compose:

- `HostSelectedModuleRoutesKey`, including each selected module's exact
  canonical identity and contextual dependency mapping;
- root usages from `RootModuleFilesKey`; and
- nonroot usages already retained inside each discovered selected module.

The audit must pin the exact ordering and identity for resolving every usage's
bzl label through its owning module context, grouping nonisolated usages,
forming isolated identities, constructing Bazel collision-safe unique
extension names, projecting local/exported imports, and applying root
`override_repo`/`inject_repo` semantics. It must distinguish what can be
known before extension evaluation from generated-repository names and
existence that require evaluation.

Freeze structural equality, complete-error-over-Need ordering, DICE validity,
compact retained representation, and A/B/A/cold-warm proof. Classify every
surface as exact, Slug-native, or unsupported/deferred. End by freezing one
explicit implementation successor with at most three Rust files, caps,
proofs, and terminal stops, or `REPLAN` into the first smaller prerequisite.

This packet may edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`.

Cap formatted net growth at 320 manifest lines, 320 owner-plan lines, 45
canonical lines, and 685 total. Read-only inspection may cover pinned Bazel
9.2 source, the accepted selected graph/registry-spec/route owners, root and
nonroot MODULE evaluators, and checked-in extension/mapping oracle evidence.
Obtain fresh independent reserved-architecture review.

No Rust, Cargo/BUILD, fixture mutation, public API, legacy graph/catalog,
second selected graph/route, extension evaluation, repository rule execution,
generated RepoSpec/repository materialization, lockfile/final-module
publication, loading, consumer, command, analysis, execution, or JVM/Java work
is authorized. Return `REPLAN` if exact ownership needs an absent
usage/identity/ordering/override/generated-name leaf, extension execution or
I/O, another graph, a public seam, more than three future Rust files, or cannot
preserve complete-error-over-Need semantics. Return `REVISE` on one bounded
design correction; a second material correction is `REPLAN`. No production
representation may begin before independent `ACCEPT` and explicit
implementation activation.

## Accepted predecessor evidence

This evidence is historical and grants no file, action, cap, or scheduling
authority.

Commit `11be92b9` accepts the private root extension-usage owner at 386
production, 327 tests, and 713 total formatted net lines, within the frozen
520/750/1,270 caps. One shared evaluator state now serves root and nonroot
MODULE evaluation while preserving their distinct dev and override semantics.
Root/include evaluation retains Arc-backed ordered usages, normalized bzl
labels, proxy bindings and logical locations, import bijections, isolation
exports, ordered tags, root override/inject `must_exist`, and synthetic
repo-rule usages only on the private root evaluation result and
`RootModuleFiles`.

`EvaluatedRootModule`, `RootModuleGraph`, the selected graph/routes, public
exports, extension execution, and consumers remain unchanged. Six focused
root tests include real-DICE A/B/A and warm reuse; all 339 owner unit tests and
every integration suite pass; the complete direct-loading suite passes. The
pinned eight-file Bazel 9.2 root fixture and six protected extension/mapping
fixtures pass. AI cleanup and independent review accept the compact one-file
implementation. Archive content checks pass; the local clone still lacks the
known V1 archive tag/branch baseline.

The earlier selected-extension audit correctly stopped because root usages
were absent. That prerequisite is now closed, but this packet does not assume
that generated-repository mappings are derivable before extension evaluation.

## Completed owner audit

Pinned Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
builds its extension-usage table by walking the selected dependency graph in
retained graph order and each module's usage list in source order. Every
usage's bzl label is resolved only through that module's Bazel-dependency
mapping. The extension ID is the canonical bzl label, extension name, and
optional isolation key of the owning selected module key plus exported proxy
name.

The dependency-graph owner then assigns unique names in first-ID encounter
order. The first candidate is `<bzl-repo>+<extension>`; collisions append the
smallest integer starting at 2. Innate extensions use the repository rule name
after the embedded space. Isolated candidates are
`<bzl-repo>+_<extension><disambiguator>+<module-name>+<module-version>+<proxy>`.
This guarantees that appending one plus sign makes no unique name a prefix of
another.

Root override/inject targets are resolved through the root mapping containing
only Bazel dependencies, self, the empty/root alias, and well-known repos.
For every module usage and proxy import, the full module mapping adds
`local -> <unique-name>+<exported>`, replacing that destination with the
resolved root target when the same exported name is overridden or injected.
Duplicate apparent mappings fail rather than overwrite. These products require
no generated repository list or extension execution.

The `must_exist` difference is retained structurally but validated only after
extension evaluation reveals generated repository names. Generated RepoSpecs,
generated-repository internal mappings, lockfile extension results, and exact
override-missing/inject-collision terminals therefore remain deferred. This
owner must not guess them.

Slug already has every admitted input. `HostSelectedModuleRoutesKey` retains
the roots-first selected entries, canonical module identity, and each owning
module's Bazel-dependency mapping. A route's root source is paired with the
private ordered usage slice from `RootModuleFilesKey`; every discovered
nonroot source already retains its ordered usage slice. Both root and nonroot
records contain proxies, import bijections, tags, isolation, and root-only
override metadata.

The smallest implementation is one new private key/value family colocated in
`selected_repo_spec.rs`. Compute selected routes first. Return route Need
unchanged and invalid, or the completed typed route error before requesting
the already-reused root files. Then walk routes and usages in their retained
order, resolve IDs, assign unique names, resolve root overrides against the
deps-only root mapping, and add extension imports to a copy of each route
mapping. Retain the complete selected routes and root usage slice once, plus
Arc-backed extension entries and full mappings; retain no transient collision
or lookup maps.

Exact surfaces are selected usage membership/order, bzl-label resolution,
extension and isolation identity, first-encounter collision disambiguation,
unique-name spelling, proxy import projection, root override/inject target
projection, module full mappings, structural equality, and error-over-Need
precedence. Slug-native surfaces are private Rust type names, compact
collections, and deterministic typed diagnostic wording. Extension loading
and evaluation, generated names/existence validation, RepoSpecs, generated
repository mappings/materialization, extension lockfile/final-module products,
loading, and consumers remain unsupported/deferred.

## Frozen implementation successor

After independent acceptance, activate only
`WP-5-host-selected-extension-mapping-owner-implementation` in:

- `app/slug_bzlmod_v2/src/selected_repo_spec.rs` for the private owner,
  pure projection helpers, and colocated pure/real-DICE tests.

Cap formatted net growth at 520 production lines, 800 test lines, and 1,320
total. No second Rust file, public export, predecessor mutation, second graph/
route/root-usage owner, extension evaluator, or consumer is granted.

Required proof:

- pure tables for root/nonroot/innate/nonisolated/isolated IDs, MVO isolation,
  first-encounter candidate collisions, exact unique-name spelling, and route/
  usage order;
- module full mappings for root/self/well-known/dependency plus extension
  aliases, multiple proxies, root override and inject targets, and unchanged
  modules without usages;
- typed missing/invisible bzl repo, invalid label, duplicate extension/import
  mapping, missing root override target, root-source mismatch, and route/root
  input failures in pinned order;
- real-DICE root and nonroot usage A/B/A, include/import/override restoration,
  collision-order change/restoration, route Need invalidity, completed route
  error precedence, and cold/warm equality/reuse;
- structural scans proving one dependency on selected routes and one reused
  root-files edge, with no selected-graph duplication, extension evaluation,
  file/network observation, RepoSpec/materializer, loading, or consumer edge;
  and
- protected root/nonroot/selected graph/registry-spec/route tests, full owner
  and direct-loading suites, pinned extension/mapping oracles, formatting,
  diff/cap/archive checks, compact-representation and AI-cleanup audits, and
  fresh independent implementation review.

Return `REPLAN` on a second Rust file, public API, predecessor mutation,
another graph/route/usage owner, inability to derive exact IDs/order/mappings,
any generated-name existence validation or extension evaluation/I/O, RepoSpec/
materializer/loading/consumer edge, or cap excess. One bounded implementation
defect is `REVISE`; a second material correction is `REPLAN`.
