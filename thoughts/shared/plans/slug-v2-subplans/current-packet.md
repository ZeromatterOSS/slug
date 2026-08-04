# Current Slug V2 Packet

Packet: `WP-5-m1-direct-local-module-file-handoff-implementation`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: one-file dormant implementation prerequisite before discovery composition
Evidence: accepted direct `local_path_override` route,
`HostRootModuleFileKey`, `HostRepositorySourceFileKey`, and the accepted
direct-local handoff design in the owner plan.

Change only `app/slug_bzlmod_v2/src/source_preparation.rs`, including inline
tests. Add private `DirectLocalModuleFileKey` with identity exactly normalized
workspace plus validated nonroot apparent repository. It computes
`RootRepositoryRouteKey` first, then only on complete success computes
`HostRepositorySourceFileKey` for literal `MODULE.bazel`.

The complete value retains the opaque complete route plus the existing Host
source value, including Present bytes/requested logical path or complete
Absent. Keep route/source/compute errors distinct and typed. Forward either
child's Needs unchanged; equality is `complete_eq`, validity complete-only,
and Need is invalid/self-unequal. Own no event data or bootstrap effect.

Do not add or infer a version. This is an unselected source input, not
`ModuleSourcePreparation`, evaluation, MVS, mapping, or discovery. A stable
outer key must observe root override A-to-B-to-A through its tracked route and
route-specific source child; a version-only root edit with unchanged route and
source must retain the same complete value.

Cap the one-file diff at 100 net production lines, 360 net test lines, and 460
total. Required evidence is the exact dependency/Need chain, complete
value/error semantics, requested-path provenance, create/edit/delete/recreate,
reroute/recovery, version-only pruning, forbidden-key absence, and no event
ownership. Run focused then full bzlmod tests/doctests, direct loading/core
checks, bzlmod GNU-Windows no-run, formatting, archive, scope/cap/dependency/
public-surface/consumer scans, and `git diff --check`, all Cargo commands
serially. Do not run or edit an oracle.

Stop with **REPLAN** on `ModuleSourcePreparationKey`, legacy root/registry or
materialization-request keys, selected/hard-coded version, another route,
registry/JVM transport, MVS/discovery/evaluation, root mapping as final nonroot
mapping, event/bootstrap-effect ownership, direct IO, public export/caller,
new dependency, second file, cap excess, configuration, analysis/actions/
execution, Java bytecode, or Bazel delegation.
