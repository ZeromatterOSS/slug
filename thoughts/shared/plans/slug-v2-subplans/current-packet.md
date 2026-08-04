# Current Slug V2 Packet

Packet: `WP-5-m1-external-repository-starlark-rule-query-redesign`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: read-only design worker
Evidence: the prior dependency-free non-test external Starlark-rule Bazel 9.2
evidence and REPLAN, plus accepted commit `980373f9`, which now supplies the
Host requested logical source path without changing legacy immutable pruning.

Retry the prior proposal for one same-repository `.bzl` load defining a
dependency-free non-test external Starlark rule, now that exact external source
identity exists. This is a fresh design audit, not authorization to revive the
old implementation sketch. Read the live loading, Bzl-module, package,
repository-route, DICE event/lifecycle, query graph, visibility, and BUILD
integration code and tests from the checkout. Reuse the accepted Bazel 9.2
success, missing-load, cycle, `deps`, `loadfiles`, and `buildfiles` evidence;
add no oracle row.

The design must resolve all of these boundaries together:

- Specify one private DICE key identified by `RootRepositoryRoute` plus a
  validated typed canonical external Bzl label. Prove normalization of every
  allowed same-package relative/absolute spelling before key construction and
  rejection of cross-package, apparent-repository, and canonical-repository
  load spellings before source lookup.
- Trace `HostRepositorySourceFileValue.logical_path` into public
  `BzlModuleIdentity`, the direct/reachable manifest and fingerprint, retained
  frozen-module lifetime, and the final `LoadedPackage` owner. Prove that source
  paths and frozen values neither disappear nor migrate into `QueryNode`.
- Specify same-repository load recursion, canonical identity, deterministic
  direct/reachable order, missing-file diagnostics, ordered cycle diagnostics,
  and cycle termination for BUILD → `.bzl` → `.bzl` loads.
- Freeze Complete/Need/error equality and validity, activation events, event
  replay, and cold/warm/edit/delete/recreate/recovery lifecycle behavior. Use
  existing DICE source owners; add no direct filesystem or fresh-graph bypass.
- Trace the existing repository BUILD loader through Starlark module loading,
  rule declaration/evaluation, dependency-free non-test capability checks,
  manifest attachment, and `LoadedPackage` construction without activating
  unrelated rule classes.
- Define exact public visibility semantics for the external rule and every
  route-aware check. Do not use the current external-visibility prohibition as
  a substitute for semantics, and do not claim visibility-content evaluation.
- Audit every enabled generic consumer reachable from the new node before any
  fixture or implementation is authorized: `deps`, `loadfiles`, `buildfiles`,
  `siblings`, `same_pkg_direct_rdeps`, `visible`, and every remaining enabled
  graph function. Each must either have exact external behavior and coverage or
  reject the node before partial output; enumerate the complete registered set
  from live code rather than assuming the prior list is exhaustive.

The completed design must append to the owner plan and name exact production,
test, downstream, and oracle allowlists; public/private API effects; DICE key,
value, equality, validity, event, and lifetime contracts; fixture growth and
hygiene bounds; serial native, lifecycle, dependent, GNU-Windows, formatting,
archive, and diff commands; and explicit old-value retention across the stated
lifecycle transitions. The oracle allowlist must remain empty in this design
packet, and the existing 17-row, 598-line `module-local-override` fixture is
frozen. Obtain one independent reserved-boundary review before scheduling any
implementation.

This packet may edit only
`thoughts/shared/plans/slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`.
Do not edit Rust, Cargo metadata, tests, fixtures, expected outputs, oracle
manifests/harnesses, protocol, CLI, canonical scheduling, current manifest, or
routing logs while performing the design. Run `git diff --check` and report
the exact docs-only diff for review.

Stop with `REPLAN` if exactness requires a public cross-crate identity or
ownership change, root-key reuse, non-local override routing, a second source
or observation owner, direct filesystem access, unbounded package/repository
discovery, or partial generic-consumer behavior. Test rules and test-base/
`@bazel_tools` closure, test suites, implicit/user dependencies,
cross-package/repository loads, discovery and external globs/patterns,
visibility-content evaluation, generated outputs, configuration,
analysis/actions/execution, repository rules/extensions, registry transports,
JVM, Java bytecode, and Bazel delegation remain out of scope.
