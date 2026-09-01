# Current Slug V2 Packet

Packet: `WP-6-7A-files-to-run-spawn-expansion-design-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 standard-provider
action import.

Status: independent retained-representation review returned `REPLAN` only for
a mistyped pinned `SpawnAction.java` SHA-256; focused correction rereview
returned `ACCEPT`. All retained-representation, equality, completeness, scope,
split, cap, and Zabel-role decisions are accepted. Zero Rust. Commit
`f46a009a0` terminally accepts the four non-Windows
runfiles-support actions and is this packet's base. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked;
do not edit or stage it.

## Observable result and stop boundary

Complete the fourth and final successor of the accepted typed
DefaultInfo/runfiles/FilesToRun category. An admitted complete
`FilesToRunProvider` used as a `ctx.actions.run` executable or tool expands its
existing stable File depset into the sole retained Spawn tool/input owner. A
root-scope executable File recovers its scoped provider association; subrules
continue to require their hidden executable dependency as the provider value.

The packet covers the whole already-designed import category in one change:

1. direct FilesToRun executable;
2. root-associated File executable;
3. direct FilesToRun sequence tool;
4. root-associated direct File tool;
5. root-associated top-level tool-depset leaves; and
6. the accepted no-inference behavior for sequence-nested tool depsets.

It also completes `FilesToRunProvider.files` with the runfiles-tree Artifact
and removes the temporary internal `complete` migration bit. No incomplete
provider is published after `f46a009a0`: returned Starlark providers are
finalized transactionally before configured-result publication, while source
File targets remain the exact supportless singleton category.

This remains generic provider/action infrastructure. It adds no parser,
evaluator builtin, `set`, `cc_common`, `cc_internal`, rules_cc, C++ rule,
ruleset dispatch, execution, aquery, ActionKey, or REAPI behavior. Bazel 9 BCR
Starlark owns all rule bodies, including `cc_internal`; `cc_common` is only a
demanding consumer of the reusable host/provider/action ABI.

## Learned facts and authenticated evidence

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole
semantic authority. Pinned sources and SHA-256 values are:

- `FilesToRunProvider.java`
  `17f3bf0b0428f8ae8c73364209ca51ffbc95afd70fe1ea7a3109ae114d8f7501`;
- `RuleConfiguredTargetBuilder.java`
  `c0fade587fb100fffd5cc49a425a3bb00b50f165dd10bfcbbb8fb6c5cc4bad6f`;
- `SpawnAction.java`
  `64cbe2b26f16e51cd57f6cefbfb21c76c0b940c0bcd9b0b8109327aa5bf667c2`;
- `StarlarkActionFactory.java`
  `bee52fa85442fe668c8573bbd2218dd454485ac8d4451ecf3553201fba6169a2`;
- `StarlarkAttributesCollection.java`
  `9b3b300d7e9c25dceafc8a9450dd2511f9b0b83088e11421b6dc3b5086cc7442`;
  and
- `StarlarkSubrule.java`
  `9d2115fdf86f1807abaf0405d3a5b36fbb3d9f8abd87aa82440f72e6e46657b6`.

The pinned constructors establish the complete branch behavior:

- `RuleConfiguredTargetBuilder.buildFilesToRun` constructs one stable set from
  files-to-build, the singleton runfiles-tree set, and the Starlark executable;
- `SpawnAction.Builder.setExecutable(FilesToRunProvider)` selects the provider
  executable and adds `getFilesToRun()` as one transitive tool root;
- `setExecutable(Artifact)` and `addTool(Artifact)` add a direct tool;
- `addTool(FilesToRunProvider)` adds only the complete files-to-run set
  transitively;
- `StarlarkActionFactory` performs root Artifact association before choosing
  those branches;
- a top-level tools depset is flattened by Bazel before per-File association,
  while a depset nested in a sequence is added transitively without per-leaf
  inference; and
- subrule executable hidden dependencies are typed FilesToRun values, and a
  bare associated File is rejected.

Pinned tests are `StarlarkRuleContextTest.java`
`d195e5d49aae52a92bd3abebfc8de7942aacb252b522cea315985d41277f082d`,
`StarlarkSubruleTest.java`
`b4cad33b5eec81f34d53b17d8f7543d51dedbb41a9a8a5359908afd70e8060e9`,
and `RunfilesRepoMappingManifestTest.java`
`8df1c7f6cc4558fe35405f43e7130ffc4f0588f41e75f18709adf520146545df`.
They pin direct executable-provider use, subrule File rejection, and manifest
fields sourced from the actual provider. Existing Slug Spawn/provenance tests
are the discriminating regression surface; no new oracle fixture is required.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance,
not a source of truth. Its `ARCHITECTURE.md` (`9db6aaaf…`),
`src/analysis/providers.zig` (`7f0441f9…`),
`src/analysis/starlark_action_registration.zig` (`9c5302a1…`), and
`src/analysis/logical_actions.zig` (`cd01ea71…`) demonstrate two useful ideas:
retain the authenticated FilesToRun occurrence until action lowering, and use
one invocation-scoped depset importer across executable, inputs, and tools so
shared roots remain shared. Slug applies only those ownership lessons through
its existing dense Rust depset and publication comparator. Copy no Zig code,
row layout, IDs, errors, action representation, scheduler, cache, digest, or
compatibility claim.

## Compatibility classification

**Exact:** direct/provider and root-associated executable selection; direct
Artifact versus transitive FilesToRun tool classification; complete provider
File membership including the runfiles tree; top-level versus sequence-nested
depset association behavior; root/subrule scope separation; missing provider
executable failure; and validation before action publication.

**Slug-native:** collision-safe structural action identity; alias-aware dense
depset publication equality; retention of the original top-level tools depset
topology instead of reproducing Bazel's temporary flattened Java list; compact
Rust layout; and configuration-relative Artifact paths rather than exact
`bazel-out` bytes. These differences do not change File membership, Starlark
typing, association, validation, or invalidation.

**Unsupported/deferred:** directory expansion; action templates; named or
automatic exec groups; resource callbacks; shadowed actions; manifest bytes;
Windows runfiles; aquery formatting; physical execution/materialization;
Bazel ActionKey; and REAPI/CAS projection. Deferred consumers may not flatten
and rebuild FilesToRun, discard the tree, infer nested-depset associations, or
introduce a second Spawn/provider representation.

## Frozen retained architecture

`FilesToRunProvider` remains the sole provider owner. Completion reconstructs
its stable `files` root from the effective `DefaultInfo.files`, the singleton
runfiles-tree depset, and the executable. The temporary `complete` bool and
private `_complete` occurrence field are deleted; `support: Option<Arc<_>>`
continues to distinguish support-bearing and exact supportless categories.
The post-evaluation finalizer remains the atomic boundary for registering all
four support actions and replacing the effective `DefaultInfo` before
publication.

The existing action enums gain only reserved variants:

```text
SpawnExecutable::FilesToRun(FilesToRunProvider)
ArtifactInputSource::FilesToRun(FilesToRunProvider)
```

Rendering selects the provider executable. Input visitation follows the
provider's existing `AnalysisDepset` through `RetainedArtifactInputs`; it does
not flatten into an owned File vector. `SpawnExecutable`,
`RetainedSpawnInvocation`, `ArtifactInputSource`, `ArtifactInputs`, command
lines, inputs, and tools all compare with one `PublicationEqState`. This
preserves alias partitions when the same retained depset appears in more than
one Spawn domain.

`ExecutableArtifactProvenance` remains one `SmallMap` for root associations
and one `SmallMap` per subrule identity. The lowering split is:

- root associated File: retain the direct File where Bazel does, then append
  the associated provider's transitive root; an executable uses the provider
  variant directly;
- subrule associated File: reject and require the provider value;
- direct provider: retain the provider variant;
- top-level depset: retain its original dense topology, visit only in
  registration scratch to discover root associations, and append only the
  associated provider roots; and
- sequence-nested depset: retain it unchanged and perform no association
  lookup.

No evaluator `Value` crosses publication. Provider/depset/action values are
DICE-retained semantic memory with `Allocative`; association traversal and
temporary source collection are action-registration scratch. Existing clones
of `FilesToRunProvider` are bounded shallow clones of dense handles, Artifacts,
and `Arc<RunfilesSupport>`; no leaf vector, interner, cache, lock, task, or new
DICE key is added. A later measured packet may wrap the provider itself in an
`Arc`, but this packet must not change occurrence identity merely for an
unmeasured clone optimization.

## Allowlist, caps, validation, and stops

Production allowlist:

- `app/slug_build_api_v2/src/providers/mod.rs`;
- `app/slug_build_api_v2/src/actions/spec.rs`;
- `app/slug_analysis_v2/src/{analysis_value.rs,starlark_rule.rs,lib.rs}`; and
- one cohesive `app/slug_analysis_v2/src/files_to_run_spawn.rs` split.

Proof allowlist:

- `app/slug_build_api_v2/tests/{providers.rs,actions.rs}`; and
- `app/slug_analysis_v2/tests/{starlark_rule.rs,subrule.rs}`.

Scheduling/status edits may touch this manifest, canonical Live Status,
Stage 6, Stage 9, and the routing log only if review changes the route.
No loading, DICE, query, execution, REAPI, parser, or ruleset production file
is allowed. `starlark_rule.rs` starts at 1,970 physical lines, so Spawn
provenance/lowering moves into the cohesive new module and the original file
must finish below 2,000 lines.

Caps are 600 net / 750 gross production Rust lines, 650 net / 800 gross proof
Rust lines, and 1,250 net / 1,550 gross total Rust lines; the new production
module is capped at 360 physical lines. Validate serially with full
`slug_build_api_v2`, `slug_analysis_v2`, and direct loading/query dependents;
focused provider topology, all six Spawn branches, root/subrule diagnostics,
alias-partition equality, action atomicity, and warm A/B/A tests;
`cargo fmt --all -- --check`; Cargo metadata; core/REAPI compile checks;
`scripts/v2_archive_status.sh`; `git diff --check`; cap accounting; and parked
file SHA-256 verification.

`REPLAN` before changing the binder or DICE key, flattening retained topology,
adding a second provider/Spawn/importer, weakening root/subrule separation,
publishing an incomplete provider or partial action, adding execution
projection, exceeding a cap, copying donor code, or introducing a parser,
`cc_common`, C++ or ruleset-specific branch. Independent architecture review
is required before Rust; independent terminal review is required before
acceptance and commit.
