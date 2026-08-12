# Current Slug V2 Packet

Packet: `WP-5-host-selected-extension-evaluation-input-requests-r2-cap-design`
Milestone: M7 module-extension evaluation-input cap correction
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: freeze the smallest cap correction for the retained unaccepted diff.

## Active cap-correction contract

The first complete error-identity correction requires about 267 production
lines: every terminal after successful request computation must retain the
full accepted request aggregate, and join failures must retain the exact
request. The 240-line production stop has fired. Retain the unaccepted two-file
diff and freeze corrected caps of 280 production, 360 tests, and 640 total
against `a31cf3d9`, with exactly the same semantics, proof, files, and stops.

No Rust is authorized until independent correction acceptance and explicit r2
activation. No loading dependency, generic public consumer, heap/callable,
schema/evaluator/execution, I/O, generated repository/lockfile/materializer/
consumer, third Rust file, JVM/Java, or behavior expansion is permitted.

## Accepted design contract

This section is historical design authority interpreted only through the
active cap-correction contract above.

Perform a read-only ownership design for one callerless Bzlmod key, keyed by
normalized workspace, that publishes ordered module/tag inputs for a later
loading-owned definition/schema composition key. It must not publish or retain
a Starlark heap, module, value, callable, evaluator, or source handle.

Freeze:

- reuse of the accepted definition-load request aggregate and its retained
  selected predecessor, without a second usage grouping owner;
- first-encounter order for admitted root-owned ordinary nonisolated extension
  IDs and their exact load requests;
- one root module view containing exactly: selected graph key
  `HostGraphModuleKey::Root` from the accepted selected predecessor;
  canonical repository identity from its accepted root route; declared module
  name and declared normalized version from
  `RootModuleFiles.module.header`; constant `is_root = true`; and the
  source-ordered extension tags associated with this exact extension ID from
  `RootModuleFiles.extension_usages`. The root view carries no dependency,
  registration, override, mapping, file-path, lockfile, or unrelated usage
  field. A missing root header/name, unavailable required version
  normalization, absent root route, or usage/request mismatch fails closed;
- source-order tags retaining tag-class name, ordered raw
  `NonrootAttributeValue` map, dev-dependency bit, and logical location;
- compact equality over the complete predecessor, requests, module view, tags,
  values, dev flags, and locations;
- typed predecessor/projection failures, with Need invalid and non-self-equal
  before publication.

Do not coerce attributes, insert defaults, validate tag classes, or construct
`module_ctx`; those require the accepted loading-owned schema. Nonroot,
isolated, MVO-owner, and innate inputs fail closed.

## Compatibility boundary

Exact for the admitted Bazel 9.2 slice: selected grouping/identity, singleton
root module membership, source-order tags, tag-class names, raw retained
values, established dev filtering, and logical source identity. Slug-native:
private names, compact containers, diagnostics, and aggregate iteration without
a Bazel user-visible order. Deferred: schema validation/default insertion,
callable reacquisition, executable views, `module_ctx`, execution/events,
environment/OS reads, factors, generated names/RepoSpecs/existence,
override/inject final validation, lockfile, materialization, consumers,
nonroot/isolation/MVO/innate execution, and JVM identity bytes.

## Scope, caps, proof, and stops

This docs-only packet may edit exactly canonical, this manifest,
`04-starlark-loading-and-build-packages.md`, and
`05-bzlmod-and-repository-graph.md`. Cap net growth at 45 canonical, 220
manifest, 140 Stage 4, 220 Stage 5, and 625 total lines. Require live
owner/visibility and pinned Bazel 9.2 ordering audits, explicit compatibility
classification, compact/Buck2 review, future scope/caps/proof/stops, and
independent design review.

After acceptance, an implementation may edit only
`app/slug_bzlmod_v2/src/selected_repo_spec.rs`,
`app/slug_bzlmod_v2/src/lib.rs`, and bookkeeping. Initial Rust caps are 240
production, 360 tests, and 600 total. Require pure empty/duplicate/order/raw
value/fail-closed rows; real-DICE predecessor Need/error, absence/multiple
order, tag/value/order/dev/location and mapping-context A/B/A, plus A/B/A for
root key/canonical repository/declared name/normalized version/is-root/tags.
Assert unrelated dependencies, registrations, overrides, mappings, paths, and
lockfile fields do not enter the module-view projection. Require warm reuse,
validity/equality, full Bzlmod/loading suites, scope/forbidden-edge/cleanup
audits, and independent review.

No Rust is authorized now. `REPLAN` on a loading dependency, generic public
consumer, FrozenValue/callable, schema/evaluator/execution work, I/O, generated
repository/lockfile/materializer/consumer edge, third future Rust file,
JVM/Java work, or cap excess.

## Accepted predecessor evidence

Commit `bf2c36e9` accepts the loading-owned definition boundary at
432 production, 649 test, and 1,081 total lines. It computes requests first,
reuses the sole Host bzl loader, retains manifests and heap-free schemas, and
keeps callables lifetime-only. Post-request errors retain full request context.
Real-DICE lifecycle proof and the full loading suite pass; both independent
reviews return `ACCEPT`.
