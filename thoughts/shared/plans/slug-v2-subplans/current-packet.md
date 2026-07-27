# Current Slug V2 Packet

Packet: `WP-5-m1-loading-host-glob-callable-activation-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted callable/lifecycle fixtures `glob-callable-contract` and
`glob-directory-invalidation`; private traversal `18f4b2db` and accepted
loading adapter in the current tree
Validation tier: docs/source/architecture

Allowed files:

- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
- terminal canonical, manifest, and exceptional routing updates

Result: inspect the current `PackageLoadKey`/`PackageRecorder` evaluation path,
the accepted private Host adapter, DICE ownership guidance, and pinned Bazel
glob restart semantics. Freeze the smallest exact ownership design by which a
synchronous Starlark `glob()` can request an asynchronous Host traversal
without blocking, direct IO, a fresh graph, or speculative placeholder values.

The design must name attempt-local recorder, target, event, and prepared-match
state; the caller-owned retained DICE transaction; how a missing one-pattern
result aborts and retries evaluation; how completed matches and errors are
reused; and why partial attempts cannot publish package or event state. It must
preserve current include/exclude, operation, sorting, and `allow_empty`
semantics over the accepted pattern subset and define exact same-daemon
invalidation/restoration evidence.

Do not edit Rust, Cargo, fixtures, generated records, Stage 9, dependencies, or
tests. Do not authorize JVM execution, blocking on DICE inside Starlark,
placeholder-driven speculative evaluation, raw-byte Starlark ingress,
BUILD/`.bzl` acquisition changes, external repositories, SUBPACKAGES,
native-Windows behavior, or broader glob grammar. Validate source anchors,
current call-flow and event ownership, exact docs-only scope, formatting,
`git diff --check`, archive status, and one independent architecture review.
