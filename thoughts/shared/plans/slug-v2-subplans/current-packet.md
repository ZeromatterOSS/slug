# Current Slug V2 Packet

Packet: `WP-5-m1-loading-host-package-key-input-ownership-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted Host glob transactional attempt design and private owner;
existing Host root-module, path, package-boundary, traversal, and adapter keys
Validation tier: reserved DICE identity and ownership design

Allowed files:

- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- exceptional routing update only if the reviewed route changes

Result: design only the parallel private `HostPackageLoadKey` and the Host
inputs it needs for root-module readiness, package markers, BUILD selection and
bytes, load-label resolution, and loaded `.bzl` modules. Freeze one
caller-owned DICE transaction, exact key identity/equality/validity, typed
`Need`/terminal/event propagation, package/load-cycle ownership, and the
boundary with the accepted transactional evaluator owner. Explain same-graph
create/edit/delete/restoration and why no lock, direct IO, injected
post-startup semantic value, fresh graph, or legacy `Arc<Result<...>>` package
path can carry Host state.

Keep activation for a later packet: add no Rust, fixture, dependency, API,
DICE key, production caller, command/query/analysis root, JVM/Bazel delegation,
legacy `PackageLoadKey` change, or external-repository breadth. Use pinned
Bazel 9.2 source plus the live V2 key graph, and obtain one independent
reserved-architecture review. Stop if exact ownership requires command
activation, changing the legacy key, or combining repository materialization
with this root-package design.
