# Current Slug V2 Packet

Packet: `WP-5-m1-external-repository-source-identity-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted route-keyed Host repository materialization/source owner;
accepted operational path resolver and bytes-only semantic source projection;
public root `BzlModuleIdentity`/manifest equality; accepted REPLAN showing an
external module cannot honestly populate that identity from the current source
value
Validation tier: read-only retained-representation/equality/source-owner design
with pinned source and one independent reserved-boundary review

Design the smallest exact prerequisite that lets a future route-keyed external
Bzl module retain the source identity required by module manifests. Decide
between extending `HostRepositorySourceFileKey`'s semantic value with the
normalized absolute logical path actually submitted to `ResolvedPathKey`, or
a reviewed generalization of public `BzlModuleIdentity`; do not presuppose
either result.

Read `docs/developers/dice.md`, the repo-local Buck2 utility-reuse skill, the
materialization/source observation path, `BzlModuleIdentity` manifest/
fingerprint/lifetime consumers, and every equality/hash/invalidation test
before proposing a representation. Distinguish the requested logical
materialized path from the resolved physical path, namespace, symlink chain,
observation instance, and apparent rendering. Freeze which fields are semantic
and which remain operational, with exact old-value retention behavior across
equal bytes, route/root changes, local versus immutable materialization,
symlink retargeting, and generation changes.

The design must name exact production/test/downstream allowlists, public API
effects, memory accounting, DICE key/value equality and validity, lifecycle
and cross-platform evidence, and a migration plan for every constructor and
consumer. Reuse existing compact/path/Arc representations; add no parallel
cache or manual lock. No external Bzl key, query projection, cycle detector,
fixture, oracle row, loader activation, or command behavior belongs in this
prerequisite.

This design packet changes only the owner plan, canonical scheduling row, and
this manifest. Stop with `REPLAN` if exactness requires exposing resolved
physical/namespace/materialization-instance state as semantic source equality,
direct filesystem observation, a second source owner, unbounded route
discovery, or an implementation bundled with external Starlark loading.
Obtain one independent retained-representation/DICE review before authorizing
any implementation.
