# Current Slug V2 Packet

Packet: `WP-5-m1-external-dependency-free-starlark-rule-projection-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: design worker
Evidence: accepted external repository source identity, route-keyed external
Bzl module owner and cycle family, request-local `QueryPackageIdentity`,
external package/query activation, exact bare query progress compatibility,
the accepted macro-query oracle, and the previously recorded Bazel 9.2
dependency-free non-test Starlark-rule probe.

Design only the smallest exact acceptance and query projection for one
explicitly public, same-package/same-repository-loaded external
`PackageTargetKind::StarlarkRule` whose retained capability is non-test and
non-executable and whose projected ordinary/dependency-reachable label set is
empty. Do not edit Rust, fixtures, tools, or generated evidence and do not run
Bazel in this packet.

Reuse the existing `RepositoryPackageLoadKey`, `ExternalBzlModuleEvalKey`,
retained frozen-module lifetime, `QueryPackageIdentity`, and fake BUILD/Bzl
provenance owners. Audit and freeze the exact whole-package gate predicate,
external graph-node projection, visibility predicate, all enabled generic
consumer and formatter behavior, equality/lifecycle implications, typed
negative stops, implementation/test allowlist, and line cap. Decide whether
the already recorded Bazel 9.2 literal/kind/self-only-deps/loadfiles/buildfiles
probe is sufficient or whether a later isolated permanent oracle subpackage is
required.

The design must not accept every `StarlarkRule`. Preserve complete stops for
test or executable capabilities, suites, generated outputs, ordinary or
dependency-reachable user/implicit labels, non-public visibility traversal or
content, globs, external patterns, cross-package/repository loads,
configuration, analysis/actions/execution, repository rules/extensions,
`@bazel_tools`, JVM, Java bytecode, and Bazel delegation. Do not schedule the
external test-rule packet; its test-base/tool-repository closure remains
unresolved.

If permanent oracle growth is selected, isolate it in a new dependency
subpackage rather than changing the mixed-kind dependency root package. Reuse
accepted missing-load, cycle, event, lifecycle, route, fake-consumer, and
formatter evidence rather than duplicating it. Obtain one independent
reserved-boundary review before naming any implementation packet.
