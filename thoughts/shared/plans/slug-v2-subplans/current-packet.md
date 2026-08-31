# Current Slug V2 Packet

Packet: `WP-6-7A-typed-files-to-run-runfiles-category-architecture-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 standard providers and
action inputs.

Status: Design `ACCEPT`; zero Rust. Independent architecture, retained-identity,
and lifecycle review accepted the typed identity, incomplete-provider guard,
root/subrule separation, atomic support-action publication, and successor
order. Commit
`bfe6f2690` terminally accepts the common default-context non-callback Spawn
envelope and is this packet's base. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked;
do not edit or stage it.

## Observable result and category boundary

Design the complete generic category by which `DefaultInfo`, `runfiles`, and
`FilesToRunProvider` carry executable and runtime inputs from a configured
target into ordinary rules, subrules, and the accepted Spawn envelope. Freeze
one architecture now, then land bounded successors in dependency order:

1. typed `DefaultInfo.files_to_run` and FilesToRun Starlark materialization;
2. typed `ctx.runfiles`, merge operations, and all five `DefaultInfo`
   constructor parameters;
3. producer-owned runfiles-support tree/manifest artifacts and action
   registration; and
4. FilesToRun expansion into Spawn inputs/tools, while existing execution and
   REAPI gates remain closed until their own action-family packet.

This is the whole provider/runfiles category architecture, not a one-field
patch. Each implementation successor must leave the same retained types and
extension seams in place so later breadth adds variants rather than a second
provider, runfiles, or Spawn representation.

Bazel 9 BCR Starlark owns every rule body, including `cc_internal`.
rules_cc and `cc_common` are authenticated consumers of generic public
providers and action APIs, never reasons for Rust C++ rule logic, parsing, or
special dispatch. Buck2 starlark-rust continues to own parsing, binding,
evaluation, dispatch, and heap lifetime.

## Learned facts and authority

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole
semantic authority:

- `FilesToRunProvider.java` SHA-256
  `17f3bf0b0428f8ae8c73364209ca51ffbc95afd70fe1ea7a3109ae114d8f7501`
  owns a stable-order complete files-to-run nested set, optional executable,
  and optional runfiles support. Its public fields expose the executable,
  runfiles manifest, and repository-mapping manifest as Files, never strings.
- `DefaultInfo.java` and `DefaultInfoApi.java` SHA-256 values
  `749a01fa226ffe32990bbafeb00aee470b9196a80ba06e1cbec6b82f0fa7833e`
  and `bf2f13c9c1bb63a34f60a2b0c69f2c9a9cc177e841cb892dfe0439633dc88344`
  fix the public four-field view, five constructor parameters, legacy-versus-
  stateful runfiles mutual exclusion, and constructor-only executable.
- `RuleConfiguredTargetBuilder.java` SHA-256
  `c0fade587fb100fffd5cc49a425a3bb00b50f165dd10bfcbbb8fb6c5cc4bad6f`
  fixes effective default files, executable fallback, insertion of the
  runfiles tree and executable into the stable files-to-run set, and provider
  publication. Explicit `files` replaces the fallback output set.
- `Runfiles.java` and `StarlarkRuleContext.java` SHA-256 values
  `2b96361ea505eafa675ec52ff011c48f3ea3732df183c845acca0bbf1f28a0ff`
  and `5200266852f65ca66a958a3adaf82a29f9b5cbbd1a604a4e91d7815476985072`
  fix typed file/symlink/root-symlink/empty-name topology, compatible depset
  order, conflict policy, `ctx.runfiles` binding and merge behavior.
- `StarlarkActionFactory.java`, `StarlarkAttributesCollection.java`, and
  `StarlarkSubrule.java` SHA-256 values
  `bee52fa85442fe668c8573bbd2218dd454485ac8d4451ecf3553201fba6169a2`,
  `9b3b300d7e9c25dceafc8a9450dd2511f9b0b83088e11421b6dc3b5086cc7442`,
  and `9d2115fdf86f1807abaf0405d3a5b36fbb3d9f8abd87aa82440f72e6e46657b6`
  fix root executable-Artifact association, direct FilesToRun executable/tool
  expansion, container-specific tool behavior, and the stricter subrule rule
  that executable hidden dependencies arrive as FilesToRun values.
- `StarlarkRuleImplementationFunctionsTest.java`,
  `StarlarkRuleContextTest.java`, `StarlarkSubruleTest.java`, and
  `StarlarkIntegrationTest.java` SHA-256 values
  `89e6caf0c6d234be610ccb597a015610568c27f8071d572e55a7378a106597d8`,
  `d195e5d49aae52a92bd3abebfc8de7942aacb252b522cea315985d41277f082d`,
  `b4cad33b5eec81f34d53b17d8f7543d51dedbb41a9a8a5359908afd70e8060e9`,
  and `ced8fc27cbe35bf30174678800d29b73012f800bff00bcdff6a5cf8c78fef836`
  pin direct provider actions, runfiles-tree inputs, subrule typing,
  constructor conflicts, field types, and merge behavior.

The authenticated rules_cc 0.2.17 consumers are
`cc/toolchains/impl/collect.bzl` SHA-256
`e2b6265fd4005dcfc9a52251be84b715a2db066007a8c2fbf2718dbc4ed9023d`
and `cc/toolchains/tool.bzl` SHA-256
`09b1ffb8c27e9d7e93c35cf559fa4c26652b22c2614cdc061a79ddbb1cf3554f`.
They read `DefaultInfo.files_to_run.executable`, merge
`DefaultInfo.default_runfiles`, construct `ctx.runfiles`, and return an
executable `DefaultInfo`. They authenticate the generic category and do not
authorize C++-specific semantics.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance,
not a source of truth. Its `ARCHITECTURE.md` and `src/analysis/providers.zig`
demonstrate useful separation between raw constructor choices, effective
DefaultInfo, Runfiles occurrences, FilesToRun occurrences, and late runfiles-
manifest derivation, plus sparse retained rows and authenticated local
references. Slug adopts the ownership lessons only. Copy no Zig code, row
layout, IDs, algorithms, errors, compatibility claims, scheduler, action key,
or cache.

## Compatibility classification

**Exact:** Starlark parameter names/defaults and outer types; `DefaultInfo`
legacy `runfiles` exclusion with `data_runfiles`/`default_runfiles`; effective
default-file fallback and explicit-files override; executable validation;
public field types; canonical empty runfiles; compatible-order file topology;
runfiles merge/merge-all order and identity-preserving empty cases; symlink and
root-symlink conflict checking for admitted path shapes; producer-owned
FilesToRun association; stable files-to-run topology; root versus subrule
executable behavior; complete FilesToRun expansion into actions; and
publication equality/invalidation for every retained semantic input.

**Slug-native:** Rust valid-Unicode strings; configured generated paths and
runfiles-tree/manifest path spelling until M9; compact Rust retained layout;
structural DICE identity; and any later physical runfiles materialization path.
Public typed relationships, members, ordering, validation, and action input
membership remain exact.

**Unsupported/deferred:** directory/tree-artifact expansion; `collect_data`
and `collect_default` until their attribute-class traversal owner is admitted;
private `skip_conflict_checking=True`; unknown/absolute/up-level symlink paths;
runfiles manifest bytes, repository mapping bytes, Windows runfiles mode, and
physical runfiles-tree materialization until their bounded successor; action
templates; named/automatic exec groups; resource callbacks; shadowed actions;
Spawn execution, aquery, Bazel ActionKey, or REAPI projection; tests/run
command behavior; aspects; and C++ action families. Deferred values fail at
their owning boundary and may not be approximated by strings or omitted input.

## Frozen retained architecture

### Raw constructor values and effective DefaultInfo

The evaluator-owned `StarlarkDefaultInfo` accepts exactly `files`, `runfiles`,
`data_runfiles`, `default_runfiles`, and `executable`. It retains Starlark
values only until the implementation returns. One lowering pass validates all
fields and normalizes them into one effective Rust `DefaultInfo`; no evaluator
value crosses publication.

The effective provider owns:

```text
DefaultInfo = {
  files: AnalysisDepset<File>,
  default_runfiles: RetainedRunfiles,
  data_runfiles: RetainedRunfiles,
  executable: Option<AnalysisArtifact>,
  files_to_run: RetainedFilesToRun,
}
```

Omitted `files` uses the producer's predeclared regular outputs plus the
executable. Explicit `files` replaces that fallback, including omission of the
executable from `files`; the complete files-to-run set still adds the
executable and, when present, the runfiles tree. Legacy `runfiles` supplies
both public runfiles views after executable insertion. Stateful default
runfiles receives the executable for executable/test rules; data runfiles does
not receive it. Equivalent effective values may DICE-cut off even when built
from different temporary constructor forms.

### Runfiles occurrence and support

`RetainedRunfiles` is one immutable typed occurrence over existing dense
depset topology:

```text
RetainedRunfiles = {
  files: AnalysisDepset<File>,
  symlinks: AnalysisDepset<RunfilesSymlink>,
  root_symlinks: AnalysisDepset<RunfilesSymlink>,
  empty_filenames: Depset<String>,
  conflict_policy: Warn | Error,
  repository_prefix: CompactString,
}
RunfilesSymlink = { path: CompactString, artifact: AnalysisArtifact }
```

The canonical empty occurrence is shared. Nonempty merges preserve stable
transitive topology; they do not flatten or copy all leaves. Public Starlark
runfiles objects are dedicated heap values with fields `files`, `symlinks`,
`root_symlinks`, `empty_filenames` and methods `merge`/`merge_all`. Heap
occurrence identity is separate from retained structural DICE equality.

Runfiles support is a producer-owned optional semantic object, not three loose
string paths:

```text
RunfilesSupport = {
  runfiles: RetainedRunfiles,
  tree: AnalysisArtifact(kind = RunfilesTree),
  manifest: Option<AnalysisArtifact>,
  repo_mapping_manifest: Option<AnalysisArtifact>,
}
```

The tree and manifest artifacts are declared by the configured-target owner
and their generating action is published atomically with provider completion.
The representation reserves all fields from the first successor even when an
early implementation must mark support incomplete and fail closed. It never
uses path strings as semantic identity.

### FilesToRun value and action import

```text
RetainedFilesToRun = {
  files: AnalysisDepset<File, stable order>,
  executable: Option<AnalysisArtifact>,
  support: Option<Arc<RunfilesSupport>>,
  complete: bool,
}
```

`complete` is an internal fail-closed migration guard, not a public semantic
field. It may be removed once all support producers are admitted. A dedicated
Starlark FilesToRun value exposes only `executable`, `runfiles_manifest`, and
`repo_mapping_manifest` as File-or-`None`; it preserves provider identity and
cannot be constructed directly. Do not rematerialize it as a generic string-
field `BuiltinProviderView`.

Prepared root dependencies carry an immutable Artifact-to-FilesToRun map for
`executable=True` attributes. Subrule hidden dependencies carry their typed
FilesToRun values directly and retain their existing scope separation. No
global map, filesystem lookup, or path-based association is allowed.

The existing Spawn lowerer remains the sole action owner. It imports the
complete FilesToRun depset topology through the existing dense importer:

- direct FilesToRun executable: select its executable and add all files to
  tools/inputs;
- associated root File executable/tool: recover the scoped provider and add
  all files;
- direct sequence FilesToRun tool: add all files;
- top-level tool depset: keep the accepted per-File association behavior;
- sequence-nested depset: keep the accepted no-inference behavior; and
- subrule File associated with hidden dependency runfiles: reject, requiring
  the FilesToRun value.

An incomplete provider fails before action publication. Expansion never
flattens retained topology merely to rebuild it, never drops the tree, and
never adds a second Spawn representation.

## Successor order and natural owners

1. `WP-6-7A-typed-files-to-run-provider-core`: migrate public retained fields
   from strings to typed artifacts/depsets, add the dedicated Starlark value,
   normalize DefaultInfo fallback files, and carry scoped provider provenance.
   Existing support remains explicitly incomplete and action use fails closed.
2. `WP-6-7A-runfiles-value-and-default-info`: add `ctx.runfiles`, typed
   runfiles values, merge/merge-all, all constructor parameters, compatible
   file topology and admitted symlink dictionaries/depsets. Collection flags
   and unadmitted path shapes remain closed.
3. `WP-6-7A-runfiles-support-actions`: add typed tree/manifest artifact kinds,
   their atomic generating action, and complete FilesToRun construction.
4. `WP-6-7A-files-to-run-spawn-expansion`: open the already-designed Spawn
   branches and delete the temporary `complete` migration guard when no
   incomplete producer remains.

Natural production owners are
`app/slug_build_api_v2/src/{providers/mod.rs,analysis_value.rs,actions/spec.rs}`,
`app/slug_loading_v2/src/provider.rs`,
`app/slug_analysis_v2/src/{analysis_value.rs,starlark_rule.rs,dice.rs}` and
`app/slug_loading_v2/src/subrule_invocation.rs`. A support-action successor may
add one private build-API action module after separate review. DICE keys,
execution/REAPI code, parser code, and ruleset-specific files are not natural
owners.

The architecture reuses `Arc`, `CompactString`, `SmallMap`/canonical maps,
`SmallSet`, `Dupe`, `Allocative`, `AnalysisDepset`, and the iterative dense
importer. Every retained public type implements complete equality and memory
accounting. No interner or cache is added without a measured later packet.

## Review, proof, and stop conditions

This architecture packet may edit only this manifest plus canonical, Stage 6,
and Stage 9 plan status. It adds zero Rust. Independent review must verify:

1. the category covers public DefaultInfo/runfiles/FilesToRun construction,
   materialization, provider provenance, and action consumption without a
   second retained representation;
2. executable targets do not falsely claim complete files-to-run state before
   the runfiles-support tree exists;
3. raw constructor choices, effective fields, public occurrence identity, and
   DICE equality are not conflated;
4. root and subrule association rules remain distinct and scope-local;
5. the successor order has no temporary string compatibility shim and no
   C++/rules_cc/parser branch; and
6. Zabel contributes optimization/ownership guidance only while Bazel 9.2
   remains semantic authority.

Each implementation successor requires its own clean allowlist, measured
production/proof caps, exact or pinned-source discriminators, serial owner and
dependent suites, `cargo fmt --all -- --check`, metadata, archive-status,
`git diff --check`, physical-size accounting, independent terminal review, and
parked-file SHA-256 verification. `REPLAN` before adding a second provider or
runfiles owner, string-backed artifact field, global association, flattened
retained topology, unowned DICE key, unbounded symlink/materialization behavior,
execution projection, donor code, or any rule-family special case.

Independent review returned `ACCEPT`. Commit this zero-Rust design and activate
only the first successor. Do not begin Rust during this packet.
