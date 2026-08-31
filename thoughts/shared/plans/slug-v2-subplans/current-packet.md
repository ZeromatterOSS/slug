# Current Slug V2 Packet

Packet: `WP-6-7A-four-runfiles-support-actions-design-r2`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 typed
DefaultInfo/FilesToRun/runfiles support.

Status: zero-Rust design independently `ACCEPT`; implementation active at
base `2483dd7e2`. The loading/package-metadata owner
landed in `80a6bfd3a`, and the complete configured transitive-package collector
landed with terminal review `ACCEPT` in `2483dd7e2`. The former three-action
draft is rejected: Bazel 9 Bzlmod always supplies transitive package metadata
and registers `RepoMappingManifest` before the other three default support
actions.

The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked;
do not edit or stage it. Remove only the untracked lockfile produced by the
fresh read-only Bazel oracle before committing this packet.

## Observable result and stop boundary

For every admitted Starlark configured target whose effective `DefaultInfo`
has an executable and nonempty default runfiles, normalization registers one
transactional four-action suffix after the rule implementation's actions:

1. `RepoMappingManifest`;
2. `SourceSymlinkManifest`;
3. `SymlinkTree`; and
4. `RunfilesTree`.

Only after all four actions validate does the target publish a complete
`FilesToRunProvider`. One `Arc<RunfilesSupport>` is shared by that provider and
all four typed recipes. A private typed occurrence carrier preserves the full
provider through configured-dependency, nested-value, and subrule transport;
public fields are views and are never used to reconstruct support.

This closes the representation and publication architecture for the current
DefaultInfo/FilesToRun/runfiles-support category. Spawn expansion is the next
packet and consumes the complete carrier. Later manifest writers, ActionKey
projections, aquery, execution, REAPI, test actions, `args`, and run-environment
support must extend these same typed owners rather than introduce a second
provider carrier, support graph, or action representation.

The packet is generic provider/action infrastructure. It adds no parser,
evaluator builtin, `set`, `cc_common`, `cc_internal`, rules_cc, C++ rule,
ruleset branch, or BCR special case. Bazel 9 rule bodies remain BCR Starlark;
Buck2-derived starlark-rust remains the sole language/parser/evaluator owner.

## Learned facts and authenticated evidence

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole
semantic authority. Pinned sources and SHA-256 values are:

- `RunfilesSupport.java`
  `429c7eb2809a46192d2fd757cece70cfeb0046a5396bbd8c5d4f15b9c6900659`;
- `RepoMappingManifestAction.java`
  `e8663c7ed8a341ae3337386a82ce29dfb2e35daca3bba211409a920e5b1ad23a`;
- `SourceManifestAction.java`
  `0a8b6d868d9702b3d6f08b7b33e46bd9de29353f37422064e4c62e13adb91a23`;
- `SymlinkTreeAction.java`
  `0279cdada9345d698dd86b803098c259caff20a4faea519ff8bb774b2ad153de`;
- `RunfilesTreeAction.java`
  `c882aff3494ac8acfbe204f65b6d220caf0fda815f8f37965befd575a8293780`;
- `FilesToRunProvider.java`
  `17f3bf0b0428f8ae8c73364209ca51ffbc95afd70fe1ea7a3109ae114d8f7501`;
- `StarlarkRuleConfiguredTargetUtil.java`
  `fbb2c4e8bf0b1fb49ba63f8a1b5f352c1c0ffbd71373c7d6dca3108c0785a1b6`;
- `CoreOptions.java`
  `89835ed74107b21f7c51b4723e16be8b96b3c1bf43855fc63220b1dd21f5c67a`;
- `RunfilesRepoMappingManifestTest.java`
  `8df1c7f6cc4558fe35405f43e7130ffc4f0588f41e75f18709adf520146545df`;
- `SourceManifestActionTest.java`
  `d8befe916188a8f41b34690245d57fa914b2846d9384dff14b09f9abf18dd9a5`;
  and
- `SymlinkTreeActionTest.java`
  `06d5b4e258e819629440a9fb572c007be17bc52e72cc6cf3886fe5f35b6fe4b5`.

`RunfilesSupport.create` first registers the repository-mapping manifest, then
declares the special runfiles-tree Artifact, then registers source manifest
and non-Windows symlink-tree actions under the default
`build_runfile_manifests=true` and `build_runfile_links=true` options, and
finally registers `RunfilesTreeAction`. `StarlarkRuleConfiguredTargetUtil`
creates support for an executable or test when computed default runfiles are
nonempty and only then publishes FilesToRun support. Under the accepted Bzlmod
boundary, the predecessor's complete `RunfilesPackageDepset` makes repository
mapping non-null.

Fresh public evidence reuses
`tests/v2_oracle/fixtures/default-info-runfiles-executable` and adds no fixture.
From its workspace, Bazel 9.2 command
`bazel --batch aquery --lockfile_mode=off --output=text //pkg:probe` succeeds
and reports, after the user `FileWrite`, exactly:

```text
RepoMappingManifest      [] -> pkg/probe.txt.repo_mapping
SourceSymlinkManifest    [] -> pkg/probe.txt.runfiles_manifest
SymlinkTree              [pkg/probe.txt.runfiles_manifest]
                         -> pkg/probe.txt.runfiles/MANIFEST
RunfilesTree             [pkg/probe.txt,
                          pkg/probe.txt.repo_mapping,
                          pkg/probe.txt.runfiles/MANIFEST]
                         -> pkg/probe.txt.runfiles
```

All four aquery rows report the host execution platform; `SymlinkTree` reports
the configured `PATH`. The fixture has no symlink Artifact, so the source
manifest's filtered input set is empty. Pinned source additionally proves that
source-manifest inputs are only runfiles Artifacts whose Artifact kind is
symlink, while the non-Windows symlink-tree input set is only the source
manifest. `RunfilesTree` inputs are runfiles Artifacts plus the public manifest
and repository-mapping manifest.

Upstream content/escaping tests are not implementation gates because manifest
serialization is deferred. Windows tests are skipped because Windows link
inputs and junction semantics are unsupported. Fileset tests are skipped as an
unadmitted runfiles shape. The fresh aquery plus pinned constructors are the
accepted graph/order evidence; no copied expected asset is added.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer
architecture and optimization guidance only. Its useful lesson is the phase
separation between provider normalization, complete package closure, action
projection, and later physical realization. Slug independently implements
that separation behind its accepted Rust owners. Copy no Zig behavior, code,
IDs, stores, digests, scheduler, cache, layout, action key, or compatibility
claim. V1 supplies no semantic owner. Buck2-derived compact collections,
`Arc`, `Dupe`, dense depsets, and `Allocative` remain bounded utility reuse.

## Compatibility classification

**Exact:** the default non-Windows support eligibility; four action mnemonics,
registration order, relative output suffixes, output roles, action dependency
graph, repository-mapping semantic inputs, source-manifest symlink-Artifact
filter, configured symlink-tree environment, public FilesToRun manifest views,
and failure-before-publication behavior. A source/exported file's existing
supportless FilesToRun provider remains a separate exact non-rule category.

**Slug-native:** collision-safe structural action/configuration identity;
configuration-relative output paths rather than exact `bazel-out` bytes; a
Rust `RunfilesTree` output-kind discriminator rather than Bazel's Artifact
subclass; compact Rust storage and publication equality; and the current
selected default action-owner context used for configured action rows.

**Unsupported/deferred:** manifest contents and ISO-8859-1 encoding; exact
NestedSet fingerprints and all four Bazel ActionKeys; aquery formatting;
materialization, execution, action cache, REAPI/CAS projection; Windows,
filesets, aspects, `run_under`, sibling-repository output layout, nondefault
manifest/link/compact-mapping flags, remotable source manifests, support
`args`, run-environment/test-action consumers, and exact Bazel output-root
identity. Deferral never permits partial action registration, reconstructed
support, an empty package substitute, or published incomplete executable
FilesToRun state.

## Frozen ownership and retained model

### Private FilesToRun occurrence carrier

Extend `ProviderOccurrence` with a private
`Option<Arc<FilesToRunProvider>>` carrier. `ProviderOccurrence::new` always
creates `None`; only `FilesToRunProvider::to_occurrence`, after constructing
the builtin identity and public fields, may attach `Some`. There is no public
setter, generic `Any`, downcast registry, path association, or reconstruction.
`FilesToRunProvider::from_occurrence` first validates the builtin identity and
then clones the typed carrier. A caller that fabricates public builtin fields
cannot create a valid FilesToRun value.

Publication equality compares the carrier with the same
`PublicationEqState` used for public fields so files/runfiles alias partitions
remain semantic. Ordinary `PartialEq` includes the carrier. `Hash` continues
to hash identity and public fields only; omitting the private carrier creates
lawful collisions and is not an identity digest. The `Arc` is required to
break the recursive `AnalysisValue`/provider type graph and makes transport a
cheap clone. Materialization reads the carrier and therefore preserves support
through dependency, nested, and subrule values.

### Shared support and typed action family

Extend the existing `RunfilesSupport`, rather than replacing it, with the
private/input source-manifest Artifact required by Bazel's topology. The
active default path requires:

```text
RunfilesSupport = {
  runfiles,
  tree:                  <executable>.runfiles          [RunfilesTree],
  input_manifest:        <executable>.runfiles_manifest [File],
  manifest:              Some(<executable>.runfiles/MANIFEST) [File],
  repo_mapping_manifest: Some(<executable>.repo_mapping) [File],
}
```

The existing optional public manifest fields remain reserved for later
nondefault flags, but this packet has no `None` branch for an admitted
executable rule. `DefaultInfo` gains a typed completion operation that keeps
its files/default/data runfiles and executable unchanged and replaces only
the incomplete FilesToRun value with one containing this support.

Add `ActionOutputKind::RunfilesTree` and one cohesive typed
`RunfilesSupportActionSpec` enum in a new action-family module. Its four
variants retain the shared support and only variant-specific semantic facts:

- repository mapping retains `RunfilesPackageDepset`, runfiles topology,
  workspace/repository prefix, and `compact=true`; its declared Artifact input
  list is empty;
- source manifest retains runfiles and the repository-mapping path and visits
  only symlink Artifacts as declared inputs;
- symlink tree retains source manifest, repository-mapping path, runfiles,
  configured action environment, and default create mode; its non-Windows
  declared input is only source manifest; and
- runfiles tree retains the complete runfiles topology and both manifests; its
  inputs are runfiles Artifacts plus public and repository manifests and its
  only output has `RunfilesTree` kind.

`ActionSpec` receives one typed payload variant and accessors for this action
family; do not encode these recipes through legacy argv/env/string fields.
The special output kind prevents later execution/REAPI code from treating the
tree as a regular file or directory. Existing projection code must explicitly
reject it; this packet does not project it.

### Producer and atomic publication

The natural producer is post-evaluation provider normalization inside
`evaluate_loaded_rule`. It already owns the returned provider collection, the
rule's action registry, configured action environment/owner, effective
DefaultInfo, and the mandatory current-node `RunfilesPackageDepset` supplied by
the accepted collector.

Move the support finalizer into a new cohesive
`slug_analysis_v2::runfiles_support` module so the 1,955-line
`starlark_rule.rs` does not cross the 2,000-line complexity trigger. The
finalizer:

1. validates eligibility, owner consistency, effective runfiles, derived
   output paths/kinds, and all four complete typed recipes;
2. builds the completed DefaultInfo candidate without publishing it;
3. calls `ActionRegistry::register_batch`, which preflights every action,
   every output, existing conflicts, and intra-batch conflicts before mutating
   either registry vector or owner map;
4. replaces DefaultInfo infallibly only after the batch commits; and
5. snapshots user actions followed by the exact four-action suffix into the
   configured result.

Any error drops the evaluation-local candidate; the registry and original
provider collection remain unchanged. No action becomes visible without a
complete provider and no complete provider becomes visible without all four
actions. The existing action mutex is held only for this synchronous preflight
and append; no DICE computation, await, filesystem read, or callback occurs
while locked.

No new DICE key, cache, global state, task, lock, filesystem observation, or
request overlay is added. Existing loaded-package and configured-analysis keys
own invalidation. A package/mapping/runfiles/configuration change changes the
retained recipe and configured result; equality cutoff restores identical A/B/A
state. Overlapping requests retain their own immutable results and share only
existing DICE values.

The support object, provider carrier, typed recipes, action outputs, and
package depset are DICE-retained semantic memory. `Arc` shares the support and
dense topologies; no full child result, flattened repository list, duplicate
runfiles graph, or evaluator value is retained. Batch vectors/maps and Artifact
deduplication are phase scratch released after finalization. `Allocative`
covers all retained additions. No async transfer or shutdown owner is added.

## Implementation succession, allowlist, and caps

Independent design review returned `ACCEPT`. Commit this zero-Rust contract,
then land one independently reviewed implementation commit.

Production allowlist:

- new `app/slug_build_api_v2/src/actions/runfiles_support.rs`;
- `app/slug_build_api_v2/src/actions/{mod.rs,ctx_actions.rs,registry.rs,spec.rs,reapi_projection.rs}`;
- `app/slug_build_api_v2/src/{analysis_value.rs,lib.rs}`;
- `app/slug_build_api_v2/src/providers/mod.rs`;
- new `app/slug_analysis_v2/src/runfiles_support.rs`;
- `app/slug_analysis_v2/src/{lib.rs,analysis_value.rs,starlark_rule.rs}`; and
- compiler-required exhaustive matches in direct build-API dependents only.

Proof allowlist:

- `app/slug_build_api_v2/tests/{actions.rs,analysis_value.rs,providers.rs}`;
- `app/slug_analysis_v2/tests/starlark_rule.rs`; and
- existing focused configured-analysis DICE tests only for mapping/runfiles
  A/B/A publication evidence.

Caps are 750 net / 900 gross production Rust, 600 net / 750 gross proof Rust,
and 1,350 net / 1,650 gross total Rust. Each new module stays below 300 lines
and each new helper below 150 lines. `starlark_rule.rs` must remain below 2,000
physical lines. No new crate, dependency, DICE key, public wire/schema,
parser/evaluator file, ruleset branch, executor, manifest writer, or structural
refactor is allowed. `REPLAN` before exceeding a cap, adding a second retained
graph/carrier, exposing the private carrier, reconstructing support from public
fields, or changing accepted runfiles/package semantics.

## Required proof and validation

1. an executable target publishes the exact four-action suffix, paths, kinds,
   input graph, configured environment, complete public fields, and no earlier
   support action;
2. `Arc::ptr_eq` proves one support allocation is shared by the completed
   provider and every recipe, while structural equality changes for runfiles,
   mapping, package, manifest, environment, or output changes;
3. builtin identity plus the private carrier round-trips incomplete and
   complete FilesToRun values through nested/configured/subrule materialization;
   public-field fabrication and user providers with the same name fail closed;
4. conflicts against each of the four existing output paths and an
   intra-batch duplicate leave action count/owner map unchanged and do not
   publish completed FilesToRun;
5. non-executable/empty categories add no support action, existing file-target
   supportless FilesToRun behavior stays unchanged, and executable/test error
   behavior remains exact;
6. source-manifest input filtering distinguishes regular, symlink, and tree
   Artifacts; non-Windows SymlinkTree and RunfilesTree inputs match pinned
   source and the fresh aquery;
7. warm same-DICE mapping-only and runfiles-only A/B/A changes alter then
   restore the configured action/provider result without replaying unrelated
   siblings; and
8. retained-size/accounting and mechanical scans prove one shared support,
   dense package/runfiles reuse, no flat repository list, no full-child
   retention, and no second carrier/graph/cache/interner.

Run serial:

- `cargo test -p slug_build_api_v2 --quiet`;
- `cargo test -p slug_loading_v2 --quiet`;
- `cargo test -p slug_analysis_v2 --quiet`;
- `cargo test -p slug_query_v2 --quiet`;
- `cargo check -p slug_core_v2 -p slug_reapi_v2`;
- `cargo fmt --all -- --check`;
- metadata, archive-status, cap/physical-size, parked-file SHA-256, and
  `git diff --check` gates.

Independent architecture review must return `ACCEPT` or `REPLAN` on the
carrier visibility/equality/hash law, shared-support shape, exact action
topology, atomic registry/provider publication, special output kind, natural
owner, retained memory, downstream boundary, caps, and successor sufficiency.
Independent terminal review must then inspect the implementation diff and all
recorded proof. A second material contract correction is `REPLAN` rather than
another in-place expansion.
