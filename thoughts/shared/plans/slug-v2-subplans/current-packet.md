# Current Slug V2 Packet

Packet: `WP-6-7A-generated-package-selected-extension-demand-owner-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design base: retained bridge/input candidate over `4d83a829`, accepted Bazel
9.2 fixture evidence `6fd78a21`, terminal demand-owner REPLAN `518abd45`
and independently reviewed owner-first design

Result: implement one producer-authenticated selected-extension demand before
definition load, evaluation, instantiation or validation. Evaluate all and
only the selected structural extension owner's module usages, while preserving
the existing complete workspace-wide carriers unchanged.

## Retained state and exact authority

Retain the nine-file generated-package bridge/input candidate exactly except
where this packet names two of those files. Its accepted discriminators remain:

- the facade proof and all 533 Bzlmod unit tests plus integrations/docs pass;
- protected external-build lifecycle and generated-route/bridge proofs pass;
- core is at its exact accepted 278/279 query baseline and runtime at its exact
  accepted 12/13 `PathObservationEpochKey` baseline;
- cumulative accounting is +566 production/+508 proof/+1,074 aggregate from
  `4d83a829`; and
- the rebuilt `module-extension-use-repo` fixture reaches the Generated route,
  then fails only because global extension validation encounters unrelated
  unsupported xcode usage first.

The seven retained candidate files outside implementation authority stay
frozen: `host_module.rs` `185ec7685abd51851c570762e393df1d59892596854cf6c826603d00a2703c39`,
`registry_dice.rs` `c736f60743709040ca1f7b327bd02f4ac956c5576b02db3375a106df6c9f8933`,
core `dice.rs` `c10651ec7a5777dbed5db78df57a6d50b5c50f098191a44fe177379a48e8f914`,
runtime `mod.rs` `204fd7510b216b9794b6ce646c29ab30dcf2b453bb42c2b402a76da6f41ac651`,
`generated_package_route.rs` `27e6ee70e2b95c3b1e48bb6fcca8795fd2ba763cb6b0867ffd7fc9ba87f90818`,
`root_apparent_repository_definition.rs`
`a1cf060405c4a5d7be26acc4b23dda542c7c0fad20325fd6fa4b7369f8dc1f3a`
and `build_command_tests.rs`
`cf96c012f4de303b9b0b0d94d345ecfbc395dc1a81427ea32399503474a067f1`.
The private Host registry owner remains frozen at
`a253dba09c0c10e51525c268402cb237961130a867e808d0a768c5b7b15feac7`.

Write authority is exactly these eight files and their colocated tests:

| File | Frozen lines | Frozen SHA-256 | Physical cap |
|---|---:|---|---:|
| `app/slug_bzlmod_v2/src/selected_repo_spec.rs` | 13,397 | `25a0d0855ed83bc58942b02ec7daa1fcc78b50e604695a60b0e148b1edf24cad` | 13,820 |
| `app/slug_bzlmod_v2/src/lib.rs` | 430 | `1fa86c3c0f71e210adcd4aa618f238f032e445c36acdd2ad6aeb8ad31e81534c` | 450 |
| `app/slug_loading_v2/src/bzl_module.rs` | 9,120 | `20737363c9048fa5b5f81e6b8d4cdeb139e413ae0f053abc0cdaa1cc85cb9a58` | 9,500 |
| `app/slug_loading_v2/src/module_extension.rs` | 2,237 | `a7eec688b42258175704ad45558dd993a884891ad4f3bb3596ea5d8ac9f55480` | 2,490 |
| `app/slug_loading_v2/src/module_extension_repository_instantiation.rs` | 2,062 | `d3c35be63df4a05227f668319307d5b21ef3790d2cf940b89c0a946196849ae9` | 2,200 |
| `app/slug_loading_v2/src/module_extension_repository_validation.rs` | 1,822 | `8f8004ed00a9339b8418f6a0c57ea2b7d4f15d96ecfd68ec945e1c494362d1e5` | 1,960 |
| `app/slug_loading_v2/src/lib.rs` | 92 | `19b2b7179b1ea209fcb07a97d4d3114f46f11b9174b103abdbd6c396ae6ec08c` | 112 |
| `app/slug_core_v2/src/runtime/generated_repository_definition.rs` | 3,985 | `8166e0c83a0f86e50d251d25b649be18cfd37020434f163a1e06dde723ba27ad` | 4,160 |

Semantic caps are <=620 production, <=700 proof and <=1,320 aggregate from
these frozen files. All other Rust, tests, fixtures, oracles, Cargo/BUILD,
plans, and `../zabel` are read-only. Existing dirty files are retained user/
agent state, not general authority.

## Bzlmod demand and owner inputs

In `selected_repo_spec.rs`, add one opaque compact structural extension owner,
shared by `Arc` and derived from the existing typed extension id plus its
collision-resolved unique canonical namespace. Its semantic fields are the
canonical `.bzl` label, extension name, typed isolation identity and unique
name. The workspace belongs to DICE keys, not the owner. Use existing
`CompactString`, canonical identity types, `Arc`, compact maps/sets and
`Allocative`; add no `String`/`Vec` to retained identity, interner, global
cache, lock or new Buck2/V1 utility.

Add a Legacy/Observed demand key identified by workspace plus the requested
canonical repository. It consumes selected extension mappings and matches the
request exactly against producer-recorded canonical import facts. It never
parses the `+` spelling. Exactly one matching structural owner yields a demand
containing the requested repository and shared owner. Repeated imports that
resolve to that same owner are one match; Missing, distinct-owner Ambiguous and
inconsistent namespace cases are typed semantic terminals. Root override
replacement remains routed to its replacement before this bridge; if the
original generated canonical is directly demanded, its recorded import still
authenticates its owner. Demand authenticates but does not classify execution
support, so an unrelated unsupported owner cannot terminate this key.

Add a Legacy/Observed owner-input key identified by workspace plus owner. It
recomputes the selected mappings child and projects only usages whose complete
typed extension id equals the owner id. The value owns:

- exactly one definition-load request with the selected unique namespace,
  complete imports and root overrides for that id;
- one ordered module row for every selected graph module using that id,
  including canonical context repository, declared name/version, `is_root`,
  that module's final/post-override repository mapping for tag coercion and all
  tags in source order; and
- distinct final and base/pre-override definition-repository mappings selected
  by the canonical `.bzl` label's repository. Tag/label coercion uses the final
  mapping; repository-rule namespace construction starts from the base mapping
  and applies generated repositories/overrides. Missing or non-unique mapping
  ownership fails closed.

Merge repeated uses from the same module into one module row without changing
usage, proxy, tag or import order. Preserve root facts from `root_usages` and
non-root facts from each discovered route; do not use the root-only
`matching_root_extension_tags` shortcut. Repeated matching imports for one
structural owner authenticate that one owner and are not ambiguous. Validate
unique-name consistency and mapping-visible admission only for the selected
owner: it must have at least one root usage, no isolation, a root-repository
`.bzl` label and one extension-name token. Preserve every non-root usage that
shares that admitted id; only a demanded owner with no root use is the typed
non-root unsupported case. Definition environment/OS/architecture/facts are
not visible here and remain pure-invocation terminals. The result must not
retain the workspace-wide mappings aggregate, so unrelated-owner changes may
recompute projection but equality suppresses downstream invalidation when the
selected owner's complete input is unchanged.

Both keys are eventless. Need and observed child outer are carrierless. Legacy
has empty epochs. Observed Complete forwards the mappings epoch; compute and
semantic terminals retain the completed prefix. Promote only the doc-hidden
key/value/error/accessor surface required one-way by loading and core. Existing
workspace-wide mapping, definition-request and evaluation-input keys remain
semantically and representationally unchanged.

## Owner-keyed loading spine

In loading, add exactly two owner-keyed DICE keys; both carry the shared owner,
never the requested repository.

The private owner-pure key consumes the Bzlmod owner-input key, loads the one
definition through the existing Legacy/Observed Host Bzl child, validates the
named export/projection, prepares every module row and reacquires the same Host
Bzl child for drift before invoking once. Coerce each tag with its module row's
final mapping, assign graph-order `module_index` and source-order `tag_index`,
and expose all rows through `module_ctx.modules: Arc<[InvocationModule]>`.
Definition environment/OS/architecture/facts rejection stays a typed terminal
here because it is known only after load.

The doc-hidden final owner-validation key consumes owner-pure output, then
synchronously reuses the single-request instantiation and validation finishers
to validate every selected-owner import, override and output. It adds no third
owner-keyed DICE stage.

Extract and reuse the existing pure per-request/per-owner finishers and
singleton aggregate containers where their invariants match. Do not copy a
parallel semantic implementation and do not filter or weaken any existing
workspace-wide loaded/prepared/pure/instantiated/validated key. Export from
loading only the final doc-hidden owner-validation key, carrier, typed outer,
error and iterable certificate needed by core.

Observed Complete epochs compose strictly left-first: owner input, first Host
Bzl load and Host Bzl reacquisition inside owner-pure; owner-pure is forwarded
unchanged through synchronous validation; core later merges demand-left then
owner-right. The earlier duplicate Arc wins. A conflict or operation mismatch
is a typed carrierless outer retaining only already completed semantic Arcs.
Need retains nothing. A child outer retains only semantic prefixes completed
before that child: Host Bzl outer retains owner input (and any earlier completed
load projection), while core owner-child outer retains the demand Arc. Never
fabricate or retain a failed child carrier or epoch. Compute and semantic
terminals retain the exact completed semantic prefix. Legacy uses only legacy
children and empty epochs.

Host Bzl keys retain their existing local Complete event batches. Pure owner
invocation owns exactly one local Complete batch, including an empty batch when
capture is enabled. Bzlmod demand/projection, definition/preparation parents,
instantiation, validation and core remain eventless. Warm reuse does not replay
child batches; cancellation stores no parent or partial batch. Retain only
semantic Arc-backed owner inputs/results and compact epochs, never Starlark
heaps, child carriers, traversal scratch, event scratch, tasks or locks.

## Core activation and terminal algebra

Change `HostGeneratedRepositoryDefinitionKey` to compute demand first and then
the loading owner-validation key. In Observed mode merge demand-left then
owner-right before interpreting the owner result. From the owner-authenticated
certificate, select the requested canonical repository with the existing exact
unique-ordinal rule.

Preserve typed distinctions for demand compute/semantic errors, selected-owner
loading compute/semantic errors, observed child/merge outer, and final missing
or duplicate output. Demand Need/outer stops before loading. Loading Need stops
without retaining demand; a loading child outer retains exactly the already
completed demand Arc and no child carrier/epoch. Directly demanded unsupported
owners remain terminal; unsupported unrelated owners are never loaded or
evaluated. Protected Unknown/Unsupported route fallback and non-Generated route
diagnostics remain byte-identical.

## Compatibility and architectural basis

Requested imported generated-repository behavior is **exact Bazel 9**. The
opaque owner identity, private DICE keys/carriers and epoch plumbing are
**Slug-native**. Executing a directly demanded xcode, isolated or other
unadmitted owner is **unsupported/deferred**. Exact Bazel configuration/output
identity bytes, query/public publication, explicit module mirrors, other
platforms and unrelated ruleset breadth remain deferred.

Bazel 9.2 is the oracle: `RepoDefinitionFunction` maps a canonical repository
prefix to one `ModuleExtensionId` before requesting `SingleExtensionValue`;
`SingleExtensionUsagesFunction` projects all usages for that id; and
`SingleExtensionFunction` evaluates only those complete usages/imports/
overrides. No Java/JVM helper enters Slug.

Use `../zabel` only as architectural guidance after its `AGENTS.md`:
`session_selected_extension_graph_demand_seeds.zig` authenticates canonical
repository demand to an owner before execution,
`session_selected_graph_extensions_root_direct_routes.zig` keeps unrelated
owners demandable without executing them, and
`session_selected_extension_source_execution.zig` executes one owner index.
Copy no Zig code or representation and add no Zabel dependency.

## Required proof and validation

Colocated discriminating proof must cover:

- exact producer-recorded lookup without canonical-name parsing; missing,
  ambiguous and inconsistent owner identities; two repositories of one owner
  sharing the same owner key;
- a requested root extension beside unrelated unsupported non-root xcode and
  beside an unrequested failing root extension; direct unsupported demand;
- complete multi-module order, names/versions/root flags, per-module mappings,
  all tags, tag indices, imports, overrides and `ctx.modules` behavior;
- exact requested output selection plus missing/duplicate results and override
  routing/authentication;
- Legacy/Observed value parity; empty/request/full epoch shapes; first-Arc,
  conflict and operation-mismatch behavior; Need/outer/compute/semantic and
  cancellation terminals;
- exact child event owners, no parent batches, warm non-replay, held-carrier
  A -> B -> A restoration, and unrelated-owner mutation equality suppression;
  and
- protected Unknown fallback/non-Generated diagnostics and no activation of
  query/public/bootstrap owners.

Run serially: focused Bzlmod demand/owner tests; focused loading owner-chain
tests; focused core bridge/generated command proofs; full `slug_bzlmod_v2`,
`slug_loading_v2`, core lib/runtime/build/cquery/query baselines; then
`cargo build -p slug_cli_v2` before rebuilding the accepted
`module-extension-use-repo` fixture. The fixture must succeed with evidence
bytes unchanged. Clean stale `slugd` before and after daemon-sensitive runs.
Finish with formatting, diff/secret/archive/rustfmt-skip scans, frozen-scope and
line/cap accounting, and independent implementation review.

## Stops

STOP delimiter parsing, root-only module input, downstream xcode suppression,
global-carrier filtering or weakening, copied pipelines, new cache/interner/
lock/task, event movement, reverse crate dependency, test/fixture/oracle
weakening, authority/cap widening, Java/JVM work, milestone closure, M8/M7B or
exact identity-byte work. `REPLAN` before widening. M7 remains partial and
M7A -> M8 -> M7B remains.
