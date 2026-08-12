# Current Slug V2 Packet

Packet: `WP-5-host-nonregistry-discovered-module-implementation`
Milestone: cross-stage M7 prerequisite implementation
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: compose the accepted callerless nonregistry closure into the existing
Host discovered-module leaf without activating recursive discovery or a graph.

## Accepted predecessor boundary

Commit `e7e4a772` owns one crate-private `HostDiscoveredModuleKey` for the
unoverridden built-in `bazel_tools@<empty>` module and versioned registry
modules. It computes root files before category selection, retains complete
built-in or ordered registry provenance, evaluates through the existing
nonregistry evaluator, and owns one optional captured event batch. It currently
fails closed for every `RootModuleOverride::NonRegistry`.

Commit `0231936f` owns the missing route-independent closure. Its
`HostNonregistryModuleClosureKey { workspace, module }` computes the exact
root nonregistry override and retained materialization/source/package owners,
then retains source category and `RepoSpec`, immutable source identity, exact
root bytes/inspection, ordered repeated fragment bytes/inspections/labels/
spans, logical identities, and an explicit unsupported-cycle capability.
Local/immutable/content/category A/B/A, Need/error order, and cold/warm reuse
are accepted. It remains callerless and owns no evaluation, graph, mapping, or
consumer.

Independent resume-design review accepts composition in the existing Host
leaf as the smallest truthful seam. No separate discovery leaf or public
boundary is needed.

## Active implementation contract

Extend only `HostDiscoveredModuleKey` in
`app/slug_bzlmod_v2/src/source_preparation.rs`.

After computing `RootModuleFilesKey`, preserve the current built-in branch
exactly: every explicit `bazel_tools` override remains
`ExplicitBuiltinOverride`, a nonempty built-in version remains invalid, and
the built-in key is not computed for an override. For every other module,
classify the exact root override before the registry-only missing-version
guard.

When and only when the override is
`RootModuleOverride::NonRegistry(_)`:

1. require the effective `NonrootModuleKey` version to be empty; a nonempty
   request is a typed complete terminal because Bazel 9.2 discovery rewrites
   nonregistry override requests to `ModuleKey(name, Version.EMPTY)`;
2. compute `HostNonregistryModuleClosureKey` and forward its complete/Need/
   compute-error algebra without recomputing root, materialization, package,
   or source state;
3. return a typed complete unsupported-cycle terminal before evaluation;
4. adapt the retained root and occurrence-ordered fragments to the existing
   `DirectNonregistryIncludeFile` input, using retained logical paths as
   logical module-file IDs and retaining repeated occurrences;
5. call only
   `evaluate_direct_nonregistry_module_closure_with_events` with the empty
   effective key and the current capture policy;
6. publish exactly one event batch for every complete captured evaluation,
   including typed evaluation failure, and none for Need/closure terminals;
   and
7. return the existing `HostDiscoveredModule` with one new
   `HostDiscoveredModuleProvenance::NonRegistry { closure }` variant. Do not
   duplicate source identity beside the closure: that closure already owns
   the exact `RepoSpec`/category/immutable-source/content/order identity.

Keep registry and built-in values, errors, events, equality, and lifecycle
unchanged. Complete values and typed terminals remain DICE-valid/equal; Need
remains invalid/non-equal. No lock may span a DICE compute.

Compatibility is exact for Bazel 9.2 empty-effective-version evaluation of an
admitted root nonregistry override and its accepted include closure.
Slug-native surfaces remain DICE/type/diagnostic names, logical source
namespace, Host observation framing, compact storage, and non-Bazel identity
bytes. Recursive discovery/MVS, command overrides, post-selection mappings,
extensions/registrations, lockfile products, package/Bzl loading, configured
toolchains, Test, execution/results/BEP/coverage, Windows, JVM/Java, and exact
Bazel identity bytes remain unsupported/deferred.

## Scope, caps, and proof

Edit only
`app/slug_bzlmod_v2/src/source_preparation.rs`. Add no file, public export,
Cargo/BUILD metadata, dependency, fixture, asset, cache, lock, interner,
process-global state, raw filesystem IO, graph, or consumer. Cap formatted net
growth at 220 production lines, 500 test lines, and 720 total. Count at the
file's main `#[cfg(test)]` module boundary; margin authorizes only the
described composition and focused corrections.

Focused real-DICE proof must cover:

- local root/fragment content A/B/A and local-to-immutable-to-local category
  restoration;
- immutable source-identity and content inequality while physical
  generation/observation changes with identical semantic input compare equal;
- the empty effective key accepting a file-declared version while a nonempty
  requested key fails before closure evaluation;
- include breadth-first/repeated occurrence effects and an unsupported-cycle
  terminal;
- closure Need forwarding and missing/wrong-kind/evaluation terminal recovery;
- cold captured evaluation, warm reuse, and exactly one event batch on
  complete evaluation success/failure;
- root override category flip and restoration; and
- unchanged protected registry and built-in lifecycle/bypass behavior.

Structural scans must prove the Host leaf contains only the accepted root,
built-in, registry-source, nonregistry-closure, evaluator, and event edges and
no `ResolvedGraph`, recursion/MVS, mapping, package/loading, command, or other
consumer. Run focused Host-discovered and Host-closure tests, the full
`slug_bzlmod_v2` suite, downstream `slug_loading_v2` and `slug_core_v2`
checks, formatting, scope/cap/diff checks, and independent latest-diff review.

## Terminal stops

Return `REPLAN` on a second evaluator or closure owner, duplicated provenance,
physical root/observation identity, accepted-empty-key contradiction, changed
built-in/registry behavior, lost Need/error/event order, lock-held compute,
recursive graph/MVS/mapping/consumer activation, public API, second file, or cap
excess.