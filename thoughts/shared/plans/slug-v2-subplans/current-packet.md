# Current Slug V2 Packet

Packet: `WP-5-m1-external-repository-query-routing`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted corrected direct external-repository query routing design,
Host source-preparation/materialization owners, typed query production
activation, opaque publication, and source-aware command events
Validation tier: public cross-crate DICE/daemon vertical plus one focused
Bazel 9.2 oracle row

Implement only:

- `RootRepositoryRouteKey` over the accepted Host root-module carrier for one
  direct non-nodep `bazel_dep` with `local_path_override`;
- `HostRepositorySourceFileKey`, constructing the exact route-carried native
  materialization request without any legacy root/snapshot key;
- a load-free, glob-free external BUILD package key;
- canonical query identity/diagnostics with separate apparent text rendering;
  and
- one apparent single-target query through the existing typed command,
  one-shot/daemon publication, and retry path.

Production allowlist:

- `app/slug_bzlmod_v2/src/{host_module.rs,source_preparation.rs,lib.rs}`;
- `app/slug_loading_v2/src/{bzl_module.rs,lib.rs}`; and
- `app/slug_query_v2/src/{evaluator.rs,graph.rs,loading_environment.rs}`.

Tests may change only the colocated bzlmod/loading/query tests, core runtime
tests, CLI/server tests, and the existing `module-local-override` fixture
TOML/expected JSON. Add no Cargo or protocol change.

Prove apparent `@dep//:target.txt` → canonical `@@dep+` lookup → apparent
output; exact unknown/missing diagnostics; BUILD fallback and lifecycle;
materialization/path Need progress; cold/changed event order; warm no-replay;
one typed root and publication owner; and zero activation of
`RepositoryMaterializationKey`, `RootModuleGraphKey`, `RootModuleFilesKey`,
`RootModuleEvaluationKey`, legacy workspace file/snapshot keys, or direct
filesystem owners.

Stop on registry transport, repository rules/extensions, transitive mapping,
canonical-label input, external patterns, `.bzl` loads, glob traversal,
cross-package/repository edges, eager snapshots, a second retry/publication
owner, build/execution behavior, JVM, Java bytecode, or Bazel delegation.
Finish with serial focused checks, the focused Bazel/Slug oracle row, scope
guards, and one independent terminal implementation review.
