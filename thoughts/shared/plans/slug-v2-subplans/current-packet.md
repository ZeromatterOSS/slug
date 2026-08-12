# Current Slug V2 Packet

Packet: `WP-5-host-module-extension-definition-owner-design`
Milestone: cross-stage M7 Bzlmod prerequisite design
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: freeze the first heap-independent module-extension definition owner or
REPLAN at the first missing compile/load prerequisite.

## Active design contract

Perform one read-only ownership audit for the definition input required after
commit `75a431d6`. Pinned Bazel 9.2 ordering is:

1. selected pre-evaluation extension mapping and identity;
2. canonical bzl-label load under the selected owner mapping;
3. exported `module_extension` lookup and tag-class/schema validation;
4. ordered module/tag view assembly for that extension ID and isolation;
5. replay-input/evaluation-factor ownership;
6. one implementation execution producing ordered generated names, RepoSpecs,
   metadata, and events;
7. only then override/inject generated-existence validation and final products.

Slug owns step 1 but has no production `module_extension()`, `tag_class()`,
extension-context evaluator, loaded extension definition value, or generated
repository result. MODULE-side proxies, lockfile schemas, digest scaffolding,
and BUILD/.bzl rule-definition loading are not substitutes.

Audit the smallest callerless private definition leaf for an admitted
root-main-repository, nonisolated ordinary Starlark extension with one
source-controlled `.bzl` definition, one exported extension, and statically
declared tag classes. Start with no `load()` unless the accepted Bzl closure
can be reused without creating a second loader.

Freeze:

- a key structurally identified by workspace, canonical bzl label, selected
  route/mapping context, and exported extension name;
- routes/selected-extension-mapping computation first, with Need invalid and a
  completed predecessor error before any package/source/load work;
- existing package/Bzl source ownership, with source/load errors before export
  lookup and schema validation;
- a compact heap-independent result retaining exact transitive definition
  source identity, exported extension name, implementation schema/IR boundary,
  ordered tag-class attribute schemas, declared environ/OS dependencies, and
  typed load/export/schema errors;
- structural equality/invalidation for the complete selected mapping context,
  transitive bytes/semantics, export, schemas, and declarations;
- the explicit feasibility question: no Starlark heap, frozen callable, or
  evaluator-lifetime value may cross DICE. If the implementation cannot be
  represented as a compact replayable heap-independent program/schema, return
  `REPLAN` and name the first compile/execute owner rather than fabricating a
  definition.

Exact: canonical label resolution, package/source/load/export/tag-schema error
order, extension ID association, and retained source-defined schemas for the
admitted Bazel 9.2 slice. Slug-native: private DICE/type names, compact
containers, diagnostic wording, and collision-safe internal identity bytes.
Unsupported/deferred: nonroot or registry-resident extension definitions,
isolated/MVO execution factors, repository-rule execution, generated
names/RepoSpecs/existence checks, network/environment/OS inputs, lockfile
replay/write, final module products, materialization, loading consumers, and
commands.

## Scope, evidence, and stops

This design packet may edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`.

Cap formatted net growth at 260 manifest lines, 320 owner-plan lines, 45
canonical lines, and 625 total. Inspect pinned Bazel 9.2 source and live Slug
owners read-only. Record exact source anchors, reusable loading/package/DICE
seams, retained identity, Need/error/event ordering, representation choice,
proof matrix, successor allowlist/caps/stops, and independent review.

No Rust/Cargo/BUILD, fixture mutation, selected-owner mutation, generic loading
consumer activation, retained Starlark heap/callable, extension execution,
generated-name/spec/existence fabrication, registry/network/environment I/O,
materialization, lockfile write/replay, command/loading consumer, JVM/Java, or
public API is authorized.

A proven heap-independent definition seam may freeze a future implementation
in at most three explicit Rust files with measured caps. `REPLAN` on an
unresolved callable lifetime, need for a second loader/graph, non-root source
materialization, evaluation/I/O, public surface, fourth Rust file, or cap
excess. No implementation may begin before independent design acceptance and
explicit activation.

## Accepted predecessor evidence

This section is historical and grants no separate file, action, cap, or
scheduling authority.

Commit `75a431d6` is independently accepted. Its private routes-first owner
uses only resolved selected entries and the root/nonroot retained usage
owners; groups ordinary and isolated IDs; assigns exact first-encounter names
including non-`extension` isolated collision suffixes; builds complete
no-overrides mappings; resolves root targets through the completed root
mapping; and performs final substitution while retaining `must_exist`.
Growth is 454 production and 516 net test lines in one file, within
520/800/1,320. Five new focused rows, all 345 owner tests plus integrations,
the full loading suite, formatting/diff/scope/compact/cleanup audits, real-DICE
Need/error/A-B-A/reuse, and independent review pass.

Generated repository existence remains deliberately unknown. It may be
validated only after a future exact extension execution owner returns its
generated name set.
