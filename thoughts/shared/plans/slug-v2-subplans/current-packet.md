# Current Slug V2 Packet

Packet: `WP-5-m1-external-package-group-visibility-query-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: design worker
Evidence: accepted standalone external package-group projection, root
package-group visibility lifecycle coverage, accepted external query package
identity, and the terminal unsupported Bazel-tools/test-base closure decision
recorded in the owner plan.

Design only the smallest exact unconfigured-query slice for one existing
direct-local external repository whose already-supported native target has
explicit visibility naming already-loaded package groups in the same package.
Freeze canonical repository-relative package-spec matching, positive/negative
forms, same-package include closure and cycle behavior, existing root versus
external caller identity, DICE equality/invalidation through the retained
route/package owner, exact evidence, implementation allowlist/cap, and stop
gates. Activate only `visible()` in the proposed contract.

Do not edit Rust, fixtures, tools, plans beyond the reviewed design result, or
generated evidence, and do not run Bazel unless a later reviewed evidence
packet explicitly authorizes it. Do not activate implicit/default visibility,
cross-package or cross-repository package-group loading, wildcard target
discovery, general visibility traversal, analysis, actions, execution, JVM,
Java bytecode, or Bazel delegation.

Obtain independent reserved-boundary review before scheduling implementation.
Stop with **REPLAN** if exact semantics require a new route/key/owner, direct
filesystem or fresh-graph observation, package enumeration, cross-package or
repository discovery, implicit/default visibility, a consumer other than
`visible()`, configuration, analysis/actions/execution, JVM, Java bytecode, or
Bazel delegation.
