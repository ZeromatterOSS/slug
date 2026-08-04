# Current Slug V2 Packet

Packet: `WP-5-m1-external-restricted-visibility-query-consumer-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: design worker
Evidence: the terminal `visible()`-only package-group visibility REPLAN,
accepted standalone external package-group projection, root package-group
visibility lifecycle coverage, accepted external query package identity, and
pinned Bazel 9.2 query visibility sources recorded in the owner plan.

Design only the smallest exact shared unconfigured-query projection for one
existing direct-local external repository whose already-supported native
target has explicit visibility naming already-loaded package groups in the
same package. Enumerate every already-enabled consumer and output before
selecting evidence or implementation: raw `labels(visibility)`, effective
dependency edges and accepted NODEP-filter variants, same-package reverse
dependencies, bounded reverse/path functions, graph output, literal/sibling/
kind/package observability, and `visible()`. Freeze raw declared labels
separately from effective top-level package-group edges; includes remain only
`PackageGroupInclude` edges and must not be flattened.

Freeze canonical repository-relative exact/recursive positive and negative
matching, `public`/`private`, per-group negative precedence and include-union
behavior, same-package include cycles, root versus same- and different-
external caller identity, DICE equality/invalidation through the retained
route/package/graph owners, the minimum discriminating Bazel 9.2 oracle rows,
an implementation allowlist/cap, and stop gates. The design may schedule
evidence or implementation only after proving that every enabled consumer
remains exact.

Do not edit Rust, fixtures, tools, plans beyond the reviewed design result, or
generated evidence, and do not run Bazel unless a later reviewed evidence
packet explicitly authorizes it. Do not activate implicit/default visibility,
direct package pseudo-labels, missing or wrong-kind groups, direct named-repo
package specifications, cross-package or cross-repository package-group
loading/includes, wildcard target discovery, analysis, actions, execution,
JVM, Java bytecode, or Bazel delegation.

Obtain independent reserved-boundary review before scheduling implementation.
Stop with **REPLAN** if exact semantics require a new route/key/owner, direct
filesystem or fresh-graph observation, package enumeration, cross-package or
repository discovery, implicit/default visibility, an unenumerated enabled
consumer, configuration, analysis/actions/execution, JVM, Java bytecode, or
Bazel delegation.
