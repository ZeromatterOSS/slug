# Current Slug V2 Packet

Packet: `WP-5-host-effective-module-override-owner-implementation`
Milestone: cross-stage M7 prerequisite implementation
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: implement one effective module-override DICE owner and route every
accepted discovery/source classifier through it without activating a graph.

## REPLAN predecessor

`WP-5-host-selected-module-graph-owner-design-r2` ends `REPLAN`. Commit
`b319b551` gives the existing command policy an exact normalized effective
`--override_module` map, but every live per-module consumer still reads only
root-MODULE declarations:

- `HostDiscoveredModuleKey` chooses built-in, root nonregistry, or registry
  discovery from `RootModuleFiles.overrides`;
- `HostNonregistryModuleClosureKey` and
  `HostNonregistryPackagePreflightKey` require a root
  `RootModuleOverride::NonRegistry`; and
- `RepositoryMaterializationRequestKey` extracts its `RepoSpec` from that same
  root map, so downstream source-file and repository-ignore ownership cannot
  route a command path.

The command input is therefore invisible to discovery. Command-over-root
precedence and explicit `bazel_tools` built-in bypass cannot be represented,
and a selected-graph key cannot repair that without duplicating source routing.

## Source authority

Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` remains authoritative. The accepted
source audit establishes this order:

1. root MODULE evaluation produces the root-declared override map;
2. normalized command overrides overlay it after root evaluation, with command
   values winning and an override of the evaluated root module failing at this
   boundary;
3. a command path becomes the same local-path/nonregistry source form used by
   Bazel discovery, while retaining command provenance;
4. only absence of any explicit root or command override permits the default
   built-in `bazel_tools` sentinel; and
5. discovery rewrites nonregistry dependencies to the empty effective version
   before source preparation and evaluation.

The existing exact inputs remain immutable and separate:
`RootModuleFilesKey` owns root declarations and evaluated root identity;
`RootModuleCommandPolicyKey` owns the canonical command map. The live reusable
source chain is `RepositoryMaterializationKey`/`RepositorySourceFileKey`,
route-independent package preflight and include-BFS helpers, and
`HostDiscoveredModuleKey`. Legacy `resolution.rs::ResolvedGraph` is ineligible.

## Design to freeze

Add one crate-private DICE leaf:

`HostEffectiveModuleOverrideKey { workspace, module_name } ->
Result<HostEffectiveModuleOverride, HostEffectiveModuleOverrideError>`.

It computes `RootModuleFilesKey` and `RootModuleCommandPolicyKey`, rejects a
winning command override of the evaluated root module at the pinned post-root
point, and folds without mutating either accepted map. Its compact retained
classification is:

- `Command { path, override_: RootModuleOverride::NonRegistry(RepoSpec) }` for
  a winning normalized command path;
- `Root { override_: RootModuleOverride }` for a root-only declaration; or
- `None` when neither input supplies the module.

`Command.path` remains the accepted normalized absolute identity. Project it
exactly once into the Bazel local-path `RepoSpec`: canonical
`@@bazel_tools//tools/build_defs/repo:local.bzl`, rule `local_repository`, and
one string `path` attribute containing that normalized path. The projection is
semantic, not a fabricated selected repository: no canonical repository name,
materialization result, bytes, or filesystem state enters this leaf. Retaining
provenance prevents a root declaration and an equal-looking command projection
from collapsing when their path interpretation/error ownership differs.

Absence stays `None`; the built-in `bazel_tools` default is installed only by
downstream discovery after this leaf. Root registry and nonregistry values pass
through unchanged. Exact effective equality contains the evaluated root
identity dependency, normalized command input, provenance, and sole effective
override; discarded command spelling/history and map insertion order remain
outside equality. Complete values and typed errors are valid/equal DICE values.

The bounded successor must make this leaf the sole override classification
edge for `HostDiscoveredModuleKey`, `ModuleSourcePreparationKey`,
`HostNonregistryModuleClosureKey`, `HostNonregistryPackagePreflightKey`,
`RepositoryMaterializationRequestKey`, and any repository-ignore guard that
currently rereads `RootModuleFiles.overrides`. A command nonregistry value
follows the existing materialization/source/package/include/evaluator chain. The request-kind
adapter may admit the normalized absolute local path only when the effective
provenance is `Command`; root-declared local paths preserve their accepted
workspace-relative validation. No second merge, source reader, evaluator, or
graph is allowed.

Need from root evaluation remains nonterminal through existing root ownership;
the injected command policy is complete. Root evaluation failure and
root-name-override rejection precede materialization. Downstream Need/error,
cycle capability, source order, and complete-only event publication remain
unchanged.

## Compatibility

Exact: Bazel 9.2 command-over-root precedence, post-root root-name rejection,
command local-path `RepoSpec` shape, explicit `bazel_tools` bypass, default
built-in only on effective absence, nonregistry empty-version routing, and
structural invalidation for the actual Slug source chain.

Slug-native: Rust type/error spelling, DICE key names, compact enum/Arc-backed
representation, OS-native normalized path storage, event framing, and
non-Bazel identity/display bytes.

Unsupported/deferred: native Windows command-path semantics, additional
command override kinds, selected graph/MVS, canonical/full repository mappings,
selected RepoSpec consumers, extension unique names/execution, lockfile
products, package/Bzl loading, configured analysis/toolchains/actions, command
output changes, Test, execution/results/BEP/coverage, JVM/Java, and exact Bazel
identity bytes.

## Active implementation contract

Implement only
`WP-5-host-effective-module-override-owner-implementation`.

Production and colocated-test allowlist:

- `app/slug_bzlmod_v2/src/module_eval.rs`; and
- `app/slug_bzlmod_v2/src/source_preparation.rs`.

No third file, public export/API, `dice.rs`, command/CLI/server/wire change,
fixture, oracle, Cargo/BUILD metadata, dependency, cache, lock, interner,
global, raw filesystem/network access, selected graph, mapping, loading, or
consumer is authorized. Cap formatted net growth at 280 production lines, 420
test lines, and 700 total.

Required proof:

- effective table for absent, root, command, both, command removal, root module,
  and ordinary versus `bazel_tools`, with command path/category A/B/A;
- exact command local-path `RepoSpec` projection and provenance/equality proof;
- real-DICE absent/root/command A/B/A, distinct input-order equality, and
  cold/warm reuse;
- command-over-root source selection, explicit `bazel_tools` command bypass
  with zero built-in activations, and restoration to the default built-in;
- preserved root-only registry/nonregistry/built-in behavior;
- command-local materialization, MODULE/source/package/include/closure/
  discovery success plus missing, wrong-kind, Need, error-order, cycle, and
  evaluation-failure lifecycle;
- structural proof that `ModuleSourcePreparationKey` and every other affected
  owner depend on the effective leaf and no longer classify
  `RootModuleFiles.overrides` directly;
- full Stage 5 owner suite and direct core/runtime dependents, formatting,
  archive, exact scope/cap/diff and forbidden-edge scans; and
- independent DICE/source-identity implementation review.

## Accepted design evidence

The three-document owner audit, live classification scan, exact command-path
projection review, scope/cap check, and independent reserved-architecture
review returned `ACCEPT`. This historical scope grants no implementation file.

## Terminal stops

Return `REPLAN` on command-path `RepoSpec` mismatch, a second merge/projection,
downstream raw command-map or root-map classification, filesystem work in the
effective leaf, inability to route the command projection through the accepted
materialization/source chain, public API, third Rust file, selected-graph/MVS
or consumer breadth, cap excess, or independent-review blocker.
