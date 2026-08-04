# Current Slug V2 Packet

Packet: `WP-5-m1-external-repository-alias-query`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted direct local-override external source-file and native
filegroup queries, accepted root-package alias projection and Bazel 9.2 alias
semantics, complete route hashing, native materialization/path retries, and
end-to-end no-legacy guards
Validation tier: one-file private query projection plus focused public
query/core and exact Bazel/Slug oracle rows

Implement only one direct same-package external native `alias` projection in
`app/slug_query_v2/src/graph.rs`. Reuse the ordinary `LoadedPackage`,
`PackageTargetKind::Alias`, accepted external graph value/key, and existing
query/render owners. Remap `actual` only when its loader spelling is in the
root repository and the currently loaded package; retain canonical semantic
identity and route-specific apparent rendering.

Project one `QueryNodeKind::Rule("alias rule")` with its retained native rule
capability, one ordered `actual` `QueryAttribute` with the stored explicitness,
and one ordinary edge. Do not project a query-visible visibility attribute.
The accepted observable slice is the alias literal, node-local
`labels(actual, ...)`, and forward `deps` within this already loaded external
package. A direct alias may resolve to an accepted filegroup or source node;
same-package undeclared source synthesis may reuse the existing generic edge
closure without observing the physical path.

Production allowlist: `app/slug_query_v2/src/graph.rs`. Tests may change only
`app/slug_query_v2/src/graph.rs`, `app/slug_query_v2/tests/loading_query.rs`,
and `app/slug_core_v2/src/runtime/dice.rs`. Oracle changes are limited to the
existing `module-local-override` fixture TOML, `workspace/dep/BUILD.bazel`, and
expected JSON; add no asset. Do not alter Cargo metadata, public APIs, DICE
keys, repository routes, loading/source owners, CLI/server adapters, protocol,
formatters, analysis, actions, execution, or another fixture.

Extend the fixture with `alias(name = "files_alias", actual = ":files")` and
exact Bazel/Slug rows for `@dep//:files_alias` and
`labels(actual, @dep//:files_alias)`. Protect all existing normalized fixture
semantics. Focused Rust evidence must additionally prove the exact alias kind,
capability, attribute explicitness, single edge, canonical identity, apparent
output, and forward `deps` closure through the accepted filegroup; preserve
the existing warm/build-edit/source-noninvalidation lifecycle and all current
external-query stop gates.

Reject cross-package or named-repository `actual`, nontrivial visibility,
unsupported alias destinations or chains, and collisions. Stop and `REPLAN`
on a need for alias-cycle semantics, another retained representation or key,
new package/repository discovery, source observation, external patterns or
functions, external loads/globs, registry transport, repository
rules/extensions, build/execution, JVM, Java bytecode, or Bazel delegation.

Finish with serial focused query/core tests, the full query suite, quiet
direct-dependent checks, the required `slug_cli_v2` rebuild before Slug oracle
replay, GNU-Windows query/core no-run linkage, formatting, `git diff --check`,
archive/scope/no-Cargo guards, fixture generation plus distinct-root replay,
and one independent terminal implementation review.
