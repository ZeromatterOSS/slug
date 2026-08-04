# Current Slug V2 Packet

Packet: `WP-5-m1-external-query-package-identity-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: read-only design worker
Evidence: accepted external source identity commit `980373f9`, the prior
dependency-free non-test external Starlark-rule Bazel 9.2 evidence, and the
accepted REPLAN audit in the owner plan. The 17-row, 598-line
`module-local-override` fixture is frozen.

Design the compact, private repository-qualified query package/candidate owner
needed before any external Starlark-rule projection can resume. Read
`AGENTS.md`, the orchestration skill and reviewer reference,
`docs/developers/dice.md`, and
`.codex/skills/slug-buck2-utility-reuse/SKILL.md` before deciding the retained
representation. Read the owner appendix, live query graph/provenance/loading
environment/formatter/parser/function registry, package and repository route
owners, and direct tests. This packet is design only; it may edit **only** the
owner plan. Do not edit Rust, Cargo metadata, tests, fixtures, oracle
manifests/harnesses, protocol, CLI/server, canonical scheduling, this current
manifest, or routing logs. Run `git diff --check` and report the exact
docs-only diff.

The design must select one compact private `QueryPackageIdentity` or equivalent
candidate-owner representation that preserves both canonical package identity
for equality/lookup and the apparent repository route required for output
rendering. It must explicitly specify real and fake candidate equality,
hashing, ordering, materialization, retained memory/clone cost, and root versus
external identity. Preserve the established canonical graph identity plus
apparent rendering convention; do not solve this by a second graph, cache,
route lookup bypass, or public cross-crate identity API.

Trace route-aware ownership through all currently lossy generic consumers:
`siblings`, `same_pkg_direct_rdeps`, `loadfiles`, `buildfiles`, reachable Bzl
labels, BUILD companions, and fake load/build provenance. Name which existing
root or external DICE key supplies every graph/package/provenance/companion
value, and prove root candidates retain their current behavior while an
external owner never reaches a root package graph or `RootPackageLoadKey`.
Specify canonical internal identity and apparent output spelling for literal,
label_kind, package, graph, BUILD, Bzl, and fake candidates, including
deduplication and deterministic order.

Audit the entire enabled generic surface, not only the consumer fixes:
`deps`, `rdeps`, `allpaths`, `somepath`, `some`, `siblings`,
`same_pkg_direct_rdeps`, `loadfiles`, `buildfiles`, `labels`, `executables`,
`tests`, `visible`, set algebra/let, target-literal patterns, and all enabled
outputs. The inventory must state that default `QueryOutputFormat::Text`
renders as label stdout, explicit `--output=text` is rejected, and the accepted
flag-selected outputs are label, graph, label_kind, and package; `attr`,
`filter`, and `kind` remain deferred and `allrdeps` is unregistered. Each
consumer must have exact bounded external behavior and coverage or reject the
node before partial output.

Preserve the demonstrated Bazel Private rule: an external caller and root
private target with the same package fragment are visible to each other. Do not
infer a repository-qualified replacement for that Private comparison.
`RuleVisibility::Restricted` direct-package/package-group caller identity is
unsettled because current code fabricates a root canonical caller; before
deciding or changing that branch, obtain pinned Bazel 9.2 source evidence and a
discriminating restricted-visibility Bazel probe. Visibility-content evaluation
remains out of scope. A future external non-test rule may be only explicitly
public, dependency-free, non-executable, non-test, and without suite/test or
unsupported package companions until this audit proves more.

Name exact future production and focused-test allowlists, direct downstream
checks, serial Cargo/lifecycle/GNU-Windows/format/archive/diff commands, and
old-value semantics for cold/warm, route mapping, edit/delete/recreate, fake
candidate, and recovery transitions. The design must keep DICE as the sole
semantic owner and specify complete/Need/error equality and validity for every
new or changed retained boundary. Obtain one independent retained-
representation/query review before scheduling implementation.

The external Bzl owner is expressly deferred. Its later separate design may
use `HostRepositorySourceFileKey`'s accepted requested logical path to form one
private route-plus-validated-canonical-external-Bzl-label key with its matching
cycle guard/detector in `app/slug_loading_v2/src/bzl_module.rs` and
`app/slug_loading_v2/src/cycle_detector.rs`; it must separately prove
manifest/fingerprint/frozen lifetime, events, and cold/warm/edit/delete/
recreate/recovery behavior. This packet must not activate that key, loader,
projection, fixture, or query behavior.

Stop with **REPLAN** if exactness needs a public cross-crate identity or
ownership change, a second source/observation/graph owner, direct filesystem
access, a fresh-graph bypass, non-local override routing, unbounded
package/repository discovery, an unproven Restricted visibility decision, or
partial generic-consumer behavior. Cross-package/repository loads, external
patterns/discovery, visibility-content evaluation, implicit/user dependencies,
executable/test rules, test suites, generated outputs, configuration,
analysis/actions/execution, repository rules/extensions, `@bazel_tools`, JVM,
Java bytecode, and Bazel delegation remain out of scope.
