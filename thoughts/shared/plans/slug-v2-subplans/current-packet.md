# Current Slug V2 Packet

Packet: `WP-5-root-extension-usage-semantic-owner-implementation`
Milestone: cross-stage M7 prerequisite implementation
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: implement the accepted root MODULE extension-usage semantic owner
without activating selected mappings or consumers.

## Active implementation contract

Implement exactly the independently accepted one-file successor below. This
packet may edit only `app/slug_bzlmod_v2/src/module_eval.rs`. Cap formatted net
growth at 520 production lines, 750 test lines, and 1,270 total. Complete the
frozen pure and real-DICE proof matrix, protected oracle fixtures, full owner
and direct-loading validation, compact-representation and cleanup audits,
structural scans, and independent implementation review.

No second Rust file, public export, new DICE key/evaluator, selected graph/
route mutation, extension evaluation, materialization, lockfile/final-module
publication, loading, consumer, command, analysis, execution, or JVM/Java work
is authorized. Return `REPLAN` on any stop or cap excess; `REVISE` on one
bounded implementation defect; a second material correction is `REPLAN`.

## Accepted design contract

This accepted design and oracle scope are historical context for the active
implementation and grant no separate file, action, cap, or scheduling
authority.

Pin Bazel 9.2 root MODULE semantics for `use_extension`, extension proxy tag
calls, `use_repo`, `override_repo`, `inject_repo`, and `use_repo_rule`
across the complete accepted root include closure. The design must:

- identify the sole root evaluator state/value that retains ordered extension
  usages, proxy bindings and source locations, local/exported import
  bijections, isolation identity, ordered tags, root-only repo overrides, and
  innate repo-rule usages without duplicating the accepted nonroot algebra;
- preserve root command-policy dev filtering and the accepted behavior that
  nonroot override/inject calls validate and are ignored;
- pin nonisolated aggregation, isolated identity, proxy export requirements,
  use-repo aliases, override/inject precedence and must-exist behavior, and
  directive error order against Bazel 9.2 source plus discriminating evidence;
- define structural equality, complete-error/Need validity, event publication,
  source/include identity, compact retained representation, and A/B/A reuse;
- freeze one explicit implementation successor allowlist, production/test/
  total caps, proof matrix, compatibility classes, and terminal stops; or
  `REPLAN` at the first smaller missing evaluator prerequisite; and
- leave extension identifier resolution against selected routes, unique-name
  construction, extension evaluation, generated repositories, selected
  extension mappings, materialization, lockfile/final-module publication,
  loading, public consumers, commands, analysis, and execution deferred.

The closed design packet could edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest;
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`;
- at most eight files under a new
  `tests/v2_oracle/fixtures/root-extension-usage-semantics/` fixture:
  `fixture.toml`, `expected/oracle.json`, and workspace `.bazelrc`,
  `MODULE.bazel`, one included MODULE fragment, `BUILD.bazel`, `ext.bzl`, and
  `repo.bzl`.

The root may inspect pinned Bazel 9.2 source, existing
`module-extension-use-repo`, `module-repo-directives`,
`module-extension-tags`, `module-use-repo-rule-dev-dependency`,
`nonroot-module-extension-semantics`, and
`repo-mapping-canonical-names` fixtures, and live Rust owners read-only.
Reuse that evidence and add the new fixture only for the demonstrated isolation,
alias, precedence/error-order, include, and restoration gaps.

Cap net growth at 320 manifest lines, 320 owner-plan lines, 45 canonical lines,
350 fixture text lines, and 1,035 total. The new fixture is capped at eight
files and must be hermetic, source-pinned to Bazel 9.2, and free of copied
nondiscriminating assets. Obtain fresh independent reserved-architecture
review.

No Rust, Cargo/BUILD outside the fixture, public API, legacy graph/catalog,
registry/network dependency, production filesystem I/O, selected mapping/route
owner, extension implementation/evaluation in Slug, generated-repository
materialization in Slug, lockfile/final-module publication, loading, consumer,
command, analysis, execution, or JVM/Java work is authorized. Return `REPLAN`
if exact root semantics require extension evaluation, a second root evaluator,
public state, more than three future Rust files, or cannot reuse the accepted
include/evaluator/event owners. Return `REVISE` on one bounded design/evidence
correction; a second material correction is `REPLAN`. No production
representation could begin before independent `ACCEPT` and explicit
implementation activation. Both gates are now satisfied.

## REPLAN evidence

This section is historical evidence only and grants no file, action, cap, or
scheduling authority.

The selected-extension-mapping audit stopped at its first missing input.
`EvaluatedRootModule` retains only header, dependencies, and registrations;
`RecordedRootModule` and `root_module_globals` likewise have no extension
directive state or globals. Therefore the accepted selected graph cannot
structurally distinguish any root extension usage, proxy, import, isolation,
tag, override, injection, or innate repo-rule declaration.

The accepted nonroot evaluator already retains complete ordered
`NonrootExtensionUsage` values, proxies, logical locations, import
bijections, isolation keys, tags, and synthetic repo-rule usages. It
deliberately stores an empty override map after validating nonroot
`override_repo`/`inject_repo`, matching their ignored nonroot semantics.
`HostSelectedModuleEntry` retains the complete root/discovered source, so no
selected-graph widening is needed after the root leaf exists.

The checked-in repository-mapping oracle proves one ordinary root import and
generated canonical mapping, while existing directive fixtures prove basic
success/rejection shapes. They do not discriminate nonisolated versus isolated
identity, include/proxy export ownership, alias bijections, override versus
inject precedence/must-exist errors, or restoration. A selected mapping owner
before those inputs exist would silently omit configuration-affecting root
semantics and is forbidden.

## Completed owner design

Pinned Bazel 9.2.0 at `8220c6198837d5c13d53fea211cf3282aa12408a`
uses one `ModuleExtensionUsageBuilder` family during MODULE evaluation.
Nonisolated `use_extension` calls with the same normalized bzl label and
extension name share one usage; isolated calls always create distinct usages
whose identity adds the containing module key and exported proxy name. The
proxy records its containing MODULE file, directive location, dev bit, import
map, and export name. Tags retain call order, attributes, dev bit, and source
location.

`use_repo` records local-to-exported names on both the usage and proxy, expands
`{name}`/`{version}`, and maintains an injective reverse mapping.
`override_repo` stores `must_exist=true`; `inject_repo` stores
`must_exist=false`; both are root-only and are ignored early under the root
ignore-dev policy. `use_repo_rule` is represented as one nonisolated synthetic
usage per bzl/rule pair with ordered `repo` tags and same-name imports.
Includes execute in one semantic context while file-local proxy bindings and
export names remain scoped to their containing compiled MODULE file.

The smallest Rust leaf stays inside `module_eval.rs`. Add crate-private
`RootExtensionUsage` and `RootExtensionIsolationKey` values that borrow the
accepted compact nonroot proxy/tag/import/override value types without changing
their public exports. Retain the Arc-backed ordered root usage slice on the
private `RootModuleEvaluation` and as a crate-private field of
`RootModuleFiles`; do not widen `EvaluatedRootModule`, `RootModuleGraph`, the
selected graph, or a public interface. The existing root evaluation key remains
the sole DICE owner and already depends on complete root/include bytes,
inspections, command dev policy, and captured evaluation events.

The new pinned fixture proves that root and included nonisolated proxies share
the `+extension+` identity, while isolated proxies receive distinct
proxy-derived identities. It proves alias projection, override replacement,
stable-daemon A/B/A restoration, and the two opposite terminals: an override
of a missing generated repo fails with exit 7 and recommends injection; an
injection colliding with a generated repo fails with exit 7 and recommends
override. The fixture has exactly eight files and 342 text lines.

Exact surfaces are root directive parsing/evaluation, normalized usage
grouping, source order and logical locations, dev filtering, import bijections,
root override/inject `must_exist`, synthetic repo-rule usage representation,
and retained semantic equality. Slug-native surfaces are compact Rust
containers, private type names, and diagnostic wording around typed internal
failures. Extension identifier resolution through selected routes, unique-name
construction, extension evaluation, generated RepoSpecs/repositories,
extension mapping augmentation, materialization, lockfile/final-module state,
loading, and consumers remain unsupported/deferred.

## Frozen implementation successor

After independent acceptance, activate only
`WP-5-root-extension-usage-semantic-owner-implementation` in:

- `app/slug_bzlmod_v2/src/module_eval.rs` for the private retained values,
  root globals/drafts/finalization, and colocated pure and real-DICE tests.

Cap formatted net growth at 520 production lines, 750 test lines, and 1,270
total. The cap grants no second Rust file, public export, new DICE key, selected
graph/route mutation, extension evaluation, or consumer.

Required proof:

- preserve all existing root and nonroot evaluator tests, registrations,
  override behavior, include execution/event ordering, and diagnostics;
- pure root/include evaluation tables for normalized nonisolated aggregation,
  distinct isolated export identities, file-local proxy binding, ordered tags,
  logical locations, bidirectional aliases with placeholders, root override/
  inject `must_exist`, synthetic repo-rule usages, and dev-policy filtering;
- focused duplicate/reserved import, unexported isolated proxy, conflicting
  import/override, type, label, and directive-order terminals;
- real-DICE root/include usage A/B/A restoration, cold/warm equality/reuse,
  complete failure and recovery, Need-free complete values, and one captured
  event batch published only after complete evaluation;
- rerun the pinned `root-extension-usage-semantics` oracle and protected
  existing extension fixtures, full `slug_bzlmod_v2` plus direct loading tests,
  formatting/diff/cap checks, archive status, and structural scans proving no
  selected graph/route, extension-evaluation, materializer, lockfile, loading,
  or consumer edge; and
- fresh Buck2 compact-representation, AI-cleanup, and independent
  implementation review.

Return `REPLAN` on a second Rust file, public type/export, second evaluator or
DICE key, inability to retain exact root/include logical identity, selected
graph/route change, extension execution/materialization, consumer activation,
or cap excess. A single bounded implementation defect is `REVISE`; a second
material correction is `REPLAN`.
