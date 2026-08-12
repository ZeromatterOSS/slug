# Current Slug V2 Packet

Packet: `WP-5-root-extension-usage-semantic-owner-design`
Milestone: cross-stage M7 prerequisite design and oracle evidence
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: freeze the missing root MODULE extension-usage semantic owner before
selected extension mapping composition.

## Active design contract

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

This packet may edit only:

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
representation may begin before independent `ACCEPT` and explicit
implementation activation.

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
