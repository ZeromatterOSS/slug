# Current Slug V2 Packet

Packet: `WP-4-5-7A-selected-external-subtree-package-owner-design`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Result: design the one loading-owned selected-external subtree package-set
producer needed by shared target-pattern expansion. Freeze its route/source,
DICE identity, observation, error and lifecycle contract. Make no Rust change
and activate no traversal or registration.

Terminal status: `REPLAN` before Rust. The live audit found no repository-
routed directory-listing DICE owner, and built-in `@bazel_tools` intentionally
has an authenticated catalog rather than a materialization root. The accepted
design and bounded successor are recorded in
`06-analysis-toolchains-and-actions.md`; direct subtree implementation is not
authorized from this packet.

## Learned facts and design question

Commit `b9736cb47` moves root subtree package discovery from query into loading
without changing behavior. Root query now consumes that natural producer. The
next missing primitive is its selected-external counterpart.

External packages already load through `RootRepositoryRoute` and
`RepositoryPackageLoadKey`. A route structurally carries canonical repository
identity, source capability and mapping across direct-local, selected-registry,
generated and built-in sources; observed variants retain route/source
observations rather than trusting a physical path. Recursive discovery must
reuse those owners and cannot reconstruct a repository root, scan the host
filesystem directly or make query/registration own a second traversal.

The design must decide the smallest semantic key/value and exact predecessor
chain that enumerate packages below one canonical external repository prefix
for all source kinds actually admitted by selected MODULE registrations. It
must also decide whether one source-owned directory-listing primitive is
missing and therefore needs a separate prerequisite.

## Required audit and decision

1. Trace `RootRepositoryRouteKey` and its observed form through every admitted
   route source, materialization/source capability, repository package lookup,
   marker/ignore policy and `RepositoryPackageLoadKey`. Name exact natural
   producers and retained Arcs; do not infer paths from display strings.
2. Trace the accepted root subtree key's Need, observed-outer, terminal,
   observation merge, equality/validity, cancellation, lexical ordering and
   lifecycle behavior. State which pieces are shared policy and which are root-
   specific.
3. Audit pinned Bazel 9.2 `RecursivePkgKey`, `RecursivePkgFunction`,
   `RecursiveDirectoryTraversalFunction`, repository package lookup and tests
   for repository identity, ignored subdirectories, package roots, ordering
   and errors. Reuse accepted oracle evidence unless a demonstrated observable
   gap remains.
4. Audit Zabel's `load/session_recursive_package_discovery.zig`, its routed
   source access and query/toolchain consumers as concept/test guidance only.
   Record useful ownership and compactness ideas separately from behavior.
5. Freeze one key/value/predecessor design with legacy and observed variants,
   complete-only equality/validity, outer/Need/terminal precedence, retained
   representation and public loading view. State request overlap,
   invalidation, cancellation and release.
6. Decide whether direct-local, selected-registry, generated and built-in
   routes fit one bounded implementation. If a source kind lacks a lawful
   observed directory-listing owner, select the smallest prerequisite and
   `REPLAN` rather than adding a bypass or false parity claim.
7. Specify the implementation allowlist/caps, direct owner tests, downstream
   consumer proof, source/oracle evidence, helper limits, complexity triggers,
   no-lock audit and exact stop conditions. Keep root query unchanged.

## Architecture and compatibility guardrails

This remains general Starlark/loading infrastructure. Bazel 9 BCR Starlark is
the source of rule definitions, including `cc_internal`; `cc_common` is only a
host-capability consumer. The design must support both toolchain and execution-
platform registration families through the later shared expander and must not
encode C++ or rules_rust policy.

- **Exact candidate:** repository-scoped recursive package membership,
  ignored-subdirectory and package-marker behavior, deterministic ordering and
  admitted Need/error/lifecycle behavior backed by Bazel 9.2 evidence.
- **Slug-native candidate:** Rust/DICE key and observation carrier shape,
  compact retained representation and source-capability plumbing.
- **Unsupported/deferred:** target-pattern expansion, wildcard-name conflict
  lookup, family filters, stable cross-pattern dedupe, configured provider or
  target-setting validation, option registrations, rule implementations and
  actions.

Zabel remains peer guidance, never source of truth. No Zabel type, session
store, allocator, diagnostic or compatibility claim may be copied. Reuse
Slug's Buck2-derived compact strings, immutable `Arc` slices, `Dupe`,
`Allocative` and small ordered collections where justified; add no interner,
global cache or manual lock.

## Allowlist, validation and stops

Base is `b9736cb47`. This is docs-only. Change only:

- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`; and
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md` for terminal
  scheduling state; and
- `.codex/skills/slug-agent-orchestration/references/routing-log.md` for the
  required terminal REPLAN row.

Caps are 0 Rust and 900 documentation additions. Read-only probes may compile
or run existing focused tests but add no fixture. Run source-anchor, structure,
scope, link, archive and diff checks. Because this selects a public retained
DICE owner, require independent architecture review before `ACCEPT`.

STOP and `REPLAN` for an unresolved source kind, path reconstruction, direct
filesystem/fresh-graph bypass, query-owned policy, a second traversal, missing
observed directory owner, copied mapping/source tree, lock across DICE compute,
new global state, new exact claim without evidence or implementation pressure
inside this docs packet.

## Immediate predecessor

Commit `b9736cb47` accepts `WP-4-5-7A-loading-root-subtree-package-owner-
extraction`. The loading crate is now the sole root recursive package producer,
query is a pure consumer, and observed outer errors remain ordered before
accumulated Needs and terminal errors. This design packet completes sequence
step 3 planning before any selected-external Rust.
