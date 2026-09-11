# Stage 6: Analysis, Toolchains, and Actions

## Goal

Analyze configured targets with Bazel-compatible providers, depsets,
configuration transitions, toolchain resolution, and action declarations.

## Scope

### Current state

M2 configured analysis is accepted with Slug-native configuration/path identity.
Registration-error identity is implemented and accepted at `a06f3ddfc`; source
observation presentation is accepted at `ac6140f41`. Preserve the shared typed
Loading-owned error DAG, structural equality and bounded borrowed Display/Debug.
Those changes are not pending implementation.

Combined selected-toolchain request/output-conflict R2 is preserved and
unaccepted. [The fixture/gate ledger](./configured-cli-fixture.md) owns the
remaining prerequisite and validation state. The detailed contracts below own
semantics; the current manifest owns scheduling. Named/automatic execution groups
remain blocked on complete combined acceptance.

### Analysis surface

- configured target keys and analysis DICE keys;
- user-defined providers and native providers;
- depset semantics and validation;
- `ctx`, `ctx.attr`, `ctx.files`, `ctx.actions`, outputs, runfiles, and
  default providers;
- configuration fragments, build settings, and transitions;
- platform and toolchain resolution;
- aspects and aspect propagation after base analysis is stable.

## V1 Extraction Candidates

- depset/provider tests from
  `slug-v1-archive:app/slug_build_api_tests/src/interpreter/rule_defs/depset.rs`
  and
  `slug-v1-archive:app/slug_build_api_tests/src/interpreter/rule_defs/provider/collection.rs`;
- `rule(implementation=...)` tests from
  `slug-v1-archive:app/slug_interpreter_for_build_tests/src/tests.rs`, with
  implementation orientation from
  `slug-v1-archive:app/slug_interpreter_for_build/src/rule.rs`;
- selected `cc_common` and provider surfaces from
  `slug-v1-archive:app/slug_build_api_tests/src/interpreter/rule_defs/cc_common.rs`,
  `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/provider.rs`,
  and
  `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/provider/collection.rs`;
- action declaration plumbing from
  `slug-v1-archive:app/slug_build_api/src/actions/registry.rs` and
  `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/context.rs`,
  only after Stage 3 path semantics are clean;
- shared-DAG design and traversal from
  `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/nested_set.rs`,
  `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/transitive_set/traversal.rs`,
  and
  `slug-v1-archive:thoughts/shared/plans/slug-bazel-subplans/54-depset-transitive-set-shared-core.md`.

These paths are absent from the active clean root. Inspect them with
`git show slug-v1-archive:<path>` or an external archive worktree; do not search
for or import them from the active root. Use the matching
[Stage 9 extraction-ledger](./09-v1-extraction-ledger.md) row to choose the
import mode, oracle, and validation.

## Bazel Oracle Anchors

- `ConfiguredTargetFunction.java` for configured-target evaluation.
- `StarlarkRuleConfiguredTargetUtil.java` and
  `StarlarkRuleClassFunctions.java` for rule implementation and rule
  definition behavior.
- `RuleContext.java`, `ConfiguredTargetFactory.java`, and provider classes for
  `ctx` and provider semantics.
- `ToolchainResolutionFunction.java`, `SingleToolchainResolutionFunction.java`,
  `PlatformFunction.java`, and `PlatformKeys.java` for platform/toolchain
  selection.
- `AspectFunction.java` and `ToplevelStarlarkAspectFunction.java` for aspect
  propagation after base analysis is stable.

Migrate focused themes from Bazel 9.2.0 tests including
`RuleConfiguredTargetTest`, `StarlarkRuleContextTest`,
`StarlarkRuleClassFunctionsTest`,
`StarlarkRuleImplementationFunctionsTest`, `DepsetTest`, and the owning
platform/toolchain tests. Every fixture records its exact class/method in the
Stage 1 provenance manifest.

## Accepted Analysis Graph Invariants

These invariants define the accepted M2 graph and constrain every new
M7A analysis family. Earlier scaffold implementation instructions are historical.

1. `ConfiguredTargetKey` is a real DICE key over a Stage 3 label,
   configuration, transition inputs, repository mapping, toolchain/platform
   policy, and the loaded target revision.
2. Its computation obtains the loaded package from the shared Stage 2 graph,
   resolves configured attributes, and recursively computes configured
   dependencies. Use DICE parallelism such as `try_compute_join` for
   independent edges while preserving deterministic Bazel ordering.
3. The rule implementation runs through the real analysis registry/context.
   `ctx.attr`, files, executables, configuration, toolchains, and dependency
   provider collections are prepared inputs; Starlark-visible getters perform
   no filesystem or graph discovery.
4. The returned provider collection is authoritative. Do not synthesize
   `DefaultInfo` from declared outputs or infer providers from action side
   effects. Validate Bazel's required-provider and duplicate-provider errors.
5. Action declarations are registered during that rule evaluation and retained
   in `AnalysisResult`. Stage 6 computes deterministic action ownership and
   conflicts but performs no execution.
6. `cquery` reads these configured-target results and `aquery` reads these exact
   action objects. Separate command-only mock graphs are forbidden.
7. Same-daemon dependency, `.bzl`, configuration, toolchain, and repository
   mapping edits invalidate through named DICE dependencies, including create
   and delete transitions.

Preserve these invariants and focused tests when extending the existing graph.

### Buck2 and V1 reuse anchors

Before implementing this gate, inspect Buck2 commit
`088c75c7e36805df99c3de29062baa95db700b8b` at:

- `../buck2/app/buck2_analysis/src/analysis/calculation.rs` for the analysis
  key and recursive dependency-compute pattern;
- `../buck2/app/buck2_analysis/src/analysis/env.rs` and
  `../buck2/app/buck2_build_api/src/analysis/registry.rs` for analysis context
  and action/provider registry ownership; and
- `../buck2/app/buck2_interpreter_for_build/` for attribute coercion,
  interning, and prepared Starlark values.

Inspect the equivalent V1 archive paths as behavior/test sources where Stage 9
records them. Port the DICE/registry/compact-data patterns behind V2 Bazel
labels, configurations, providers, and output paths; reject cells, Buck labels,
Buck configurations, and Buck output semantics. Hot graph structures require
the repo utility audit: prefer retained `SmallMap`/`SmallSet`, Fx hashing,
`Hashed`, `ArcStr`/`ThinArcStr`, `Dupe`, and `Allocative` where their measured
shape fits instead of default owned `String`, `Vec`, or std hash collections.

## Implementation Slices

These are subsystem contracts and capability inventory, not a next-packet queue.
Dated activation/review passages describe historical evidence only; use Current
state and the canonical manifest for scheduling.

### 6.1 Configured Target Key and Configuration

- Define Bazel-shaped configured-target keys using Stage 3 labels plus
  configuration hash.
- Implement target, exec, and host-like transition policy only where Bazel 9
  still exposes it.
- Build setting values, command-line flags, and transition outputs are DICE
  inputs, not global process state.
- Initial modules: `app/slug_analysis_v2/src/{key.rs,result.rs,dice.rs,configured_target.rs}`
  and `app/slug_build_api_v2/src/providers/`.
- Use archived V1 analysis code such as
  `slug-v1-archive:app/slug_analysis/src/analysis/calculation.rs` and
  `slug-v1-archive:app/slug_analysis/src/analysis/toolchain_resolution.rs` only
  as pattern sources; do not port Buck labels or V1 configuration identity.
- Implement this as the DICE computation defined by the current-priority gate;
  a key-shaped serializable struct without `Key::compute` integration is only
  substrate.

### 6.2 Providers and Depsets

- Implement user providers, native providers, `DefaultInfo`, `OutputGroupInfo`,
  `RunEnvironmentInfo`, `FilesToRunProvider`, `PlatformInfo`, and the provider
  collection API needed by the first rulesets.
- Implement Bazel `depset` order, validation, flattening, equality constraints,
  and transitive nesting without implicit `transitive_set` coercion.
- Store the transitive structure as an immutable shared nested DAG: composition
  must not recursively copy children, and flattening is an explicit consuming
  operation. Selectively extract the archived shared traversal/depth lessons
  named above while keeping the Bazel depset facade V2-owned.
- Initial modules: `app/slug_build_api_v2/src/{ctx.rs,attrs.rs,providers.rs,runfiles.rs,depset.rs}`.
- Extraction candidates are the archived V1 depset/provider tests and
  implementation paths named above, but public types must be Bazel-shaped and
  Stage-3 label based.

#### 6.2A Zabel-informed retained depset core gate

Accepted architecture: `WP-6-7A-dense-retained-depset-action-import-r1` at
base `683538254`. It freezes one immutable dense store with ordered tagged
successor rows, local and external handles, structural alias-aware publication
equality, and a typed File/action-input view over the same owner. Authentic
rules_cc `all_files` forwarding selects this as the immediate prerequisite to
generic action builtins. Independent reserved-representation rereview returned
`ACCEPT`; the bounded
`WP-6-7A-dense-retained-depset-action-import-implementation-r1` packet is
implemented, proved and terminally `ACCEPT`.

Before the first broad ruleset consumer depends on nonempty transitive depsets,
implement one bounded Zabel-informed retained core behind the exact Bazel
`depset` facade. Bazel 9.2 remains the semantic oracle. Use Zabel commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` as the primary design reference,
but independently implement the Rust types and algorithms and copy no Zabel
production code. Zabel test scenarios may seed independently written V2
fixtures only when their provenance is recorded and their expected results are
regenerated with the pinned Bazel 9.2 oracle.

Start from a dense retained store informed by Zabel's packed row/bit layout,
compact node and edge indexes, distinct node-identity and leaf-equality
deduplication, Bazel-specific construction normalization and traversal,
generic/File specialization, producer-qualified external references, late leaf
materialization, direct action-input import, and invocation-local flatten
caches. Treat the current immutable per-node `Arc` DAG as migration scaffolding,
not the presumptive long-lived representation.

Construction must retain the user-declared Bazel order and perform Bazel's
construction-time validation at the exact public boundary. The internal
traversal algorithm may be selected and executed only when a consumer needs a
projection: delayed computation is not itself observable. Every consumer must
nevertheless receive the exact result for the retained declared order, and the
Bazel facade must not expose Buck2's ability to choose a different order at
consumption time. Keep node/topology identity, leaf equality deduplication,
semantic DICE equality, flattened values, action-input topology, and action or
REAPI fingerprints as distinct domains.

The gate must include:

- all four Bazel orders, mixed/default compatibility, empty-set behavior,
  singleton reuse, multi-child depth, validation precedence, and the configured
  depth limit;
- diamonds, repeated child aliases, distinct nodes with equal leaves, duplicate
  direct values, and cross-owner forwarding; in particular pin Bazel's
  topological `[a, b, c, b] -> [a, c, b]` alias result;
- a non-recursive or otherwise stack-safe traversal at the supported maximum
  depth;
- cold and repeated `to_list()`, direct action/Args consumption without
  flattening, provider freeze/materialization, and release of retained owners;
- retained bytes, allocations, construction cost, cold/warm consumption cost,
  and realistic rules_cc/rules_rust fan-in and diamond graphs; and
- `Allocative` coverage plus proof that caches are request/evaluator scoped or
  explicitly bounded and never become unmodeled DICE semantic state.

Record the resulting representation and measurements. After the exact
Zabel-informed baseline works, optional focused experiments may adopt isolated
Buck2 ideas only when they improve a measured Slug workload without widening
the surface or weakening Bazel semantics. Public Buck2 `transitive_set`
definitions, projections, reductions, BFS/DFS, and implicit `depset` coercion
remain unsupported/deferred; an internal late projection is admissible only
when its Bazel-visible values, errors, timing boundary, action inputs, and
identity projections remain exact.

### 6.3 Rule Implementation Context

- Implement `ctx.attr`, `ctx.file`, `ctx.files`, `ctx.executable`,
  `ctx.label`, `ctx.outputs`, `ctx.actions`, `ctx.fragments`, `ctx.toolchains`,
  `ctx.exec_groups`, `ctx.var`, `ctx.expand_location`, and
  `ctx.resolve_command` in priority order driven by fixtures.
- Starlark-visible methods that need prepared values must receive them from
  analysis inputs rather than doing filesystem or graph discovery.

### 6.4 Action Declaration IR

- Define an action IR that is independent of executor choice.
- Actions include mnemonic, argv, env, execution requirements, input digests,
  tools, paramfiles, output declarations, progress message, and exec
  properties.
- Stage 7 consumes this IR to build REAPI commands.
- Initial modules:
  `app/slug_build_api_v2/src/actions/{registry.rs,spec.rs,ctx_actions.rs,reapi_projection.rs}`.
- Implement `declare_file`, `declare_directory`, `declare_symlink`, `write`,
  `write_json`, `expand_template`, `run`, `run_shell`, `args`, output conflict
  checks, exec groups, toolchain action contexts, and action exec properties.
- Stage 6 emits deterministic action descriptions only; it does not execute
  actions or decide CAS/AC policy.

### 6.4A Immutable action-owner context

Schedule this owner after the first M1 request-revision/source-certificate
vertical and the just-in-time action/toolchain oracle subset, but before any
M7A packet admits broader action registration. Acceptance of the immutable
owner context is therefore an M7A entry gate. It is not blocked on the complete
Wave A fixture catalog, M8 bootstrap, M7B command breadth, or M9 exact identity
bytes. The current M7 repository source-consumer audit and its fixed cutover
remain unchanged.

Before Stage 6 admits another general action kind, named exec groups, applied
aspect actions, or multi-platform selection, retain one immutable owner
context at action registration. The target shape is:

```text
ActionOwnerContext {
    configured_owner,
    semantic_configuration_identity,
    admitted_checksum_or_display_projection,
    exec_group,
    execution_platform,
    exec_properties,
    selected_toolchain_context,
    aspect_provenance,
}
```

The exact Rust representation may be smaller or split into authenticated
projections, but every field that can affect action behavior must participate
structurally in equality and invalidation. The default exec group is an
explicit identity, not the absence of a context. An action retains the group-
selected platform, combined platform/target/group execution properties, and
selected toolchain context at creation time.

Do not later reconstruct an action's platform or properties from only its
label, the owner's current topology, a process-global current platform, or a
new toolchain-resolution run. `aquery`, Stage 7 execution, Slug-native action
provenance, progress observations, and future explain output must consume the
same retained owner context and action row. Bazel checksum and
configured-output bytes remain separate M9 domains.
Bazel ActionKey is a derived exact projection admitted per action family under
6.4B; neither projection permits semantic inputs to be omitted.

The design packet must audit the current `ConfiguredActionExecGroup::Default`
and `ConfiguredActionView` topology-derived platform path, then prove:

- default and named groups select and retain distinct contexts;
- two actions of one owner may use different groups/platforms/properties;
- `cfg = "exec"` dependencies use the group-selected exec configuration;
- aspect-created actions retain their applicable aspect/owner context;
- configuration, platform, property, registration, mapping, and toolchain
  edits invalidate through named DICE dependencies;
- aquery and REAPI projections are derived from the identical retained row;
- conflicting-output and other diagnostics preserve Bazel's error precedence;
  and
- retained data is immutable, compact, `Allocative`, and released with the
  owning analysis value.

Compatibility classification:

- selected platform/toolchain, exec-group behavior, property merging, and
  action-visible provenance are **exact** for admitted Bazel 9.2 slices;
- private Rust rows, compact identities, and added explain fields are
  **Slug-native**; and
- exact configuration checksum and configured output token remain
  **deferred to M9**; exact Bazel ActionKey projections are **exact per admitted
  action family** under 6.4B.

Use the Stage 1 provider, action-conflict, aquery-topology, and toolchain
fixture backlog before implementation. This section freezes a future owner
contract; it does not modify the accepted bounded FileWrite action surface or
widen the active M7 packet.

### 6.4B Exact Bazel ActionKey projection

An exact Bazel ActionKey is a derived compatibility projection of the retained
configured-action row plus immutable owner context. It is not the action's
structural semantic identity, a new action graph, a configured-output token, or
the REAPI Action digest. Historical M9 ActionKey deferrals below are superseded
by this current contract; configuration checksum and output identity stay M9.

Mirror Zabel's byte-feeding approach, but reverify every field and order against
Bazel 9.2 source and fresh discriminating oracle evidence:

- provide one small V2-owned `BazelFingerprint` leaf over SHA-256 with the
  exact protobuf `CodedOutputStream` no-tag bool/int64/string length framing,
  Java UUID word order, and Bazel internal-string-to-Java-UTF-8 conversion;
- implement each admitted action family's exact `computeKey` body, including
  its GUID, field order, collection order, conditional fields, and byte
  representation, then append the common `ActionKeyComputer` tail;
- the common tail must reproduce platform presence and `PlatformInfo`
  fingerprinting in Bazel order, including parent/constraint structures,
  maps/lists, settings, toolchain types, message booleans, action execution
  properties, and the current zero uniquifier;
- never hash debug output, formatter text, Slug canonical identity bytes,
  `StrongHash`, a weak/precomputed map hash, or the REAPI Action encoding as a
  substitute for Bazel's ordered fingerprint stream; and
- fail closed when any source-derived input is not modeled. No zero,
  hard-coded, truncated, or opaque fallback key is admitted.

FileWrite is the first projection. Its packet must distinguish the regular and
compressed GUIDs, executable bit, and exact logical content; reproduce Bazel's
Java compression threshold and deterministic `GZIPOutputStream` bytes; and
retain the accepted discriminator that content and platform affect ActionKey
while declared output path, owner configuration token, and output-root spelling
do not. Each M7A/M7B action family receives the same just-in-time source audit
and exact projection alongside semantic activation.

Reviewed Zabel donor commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` supplies bounded leaf and test
ideas from:

- `src/core/bazel_fingerprint.zig`;
- `src/core/bazel_internal_string_java_utf8.zig`;
- `src/analysis/action_key_fingerprint.zig`;
- `src/analysis/file_write_content.zig`;
- `src/analysis/file_write_action_key.zig`; and
- `src/analysis/complete_action_key.zig`.

These are donor inputs, not the oracle. The packet must record exact Bazel 9.2
source anchors, accepted output vectors, mutation discriminators for every
conditional field, and a cross-check that Zabel's result still matches the
pinned Bazel revision.

Keep the domains visibly separate:

```text
ConfiguredAction + immutable owner context
  -> Slug structural identity       (equality and DICE invalidation)
  -> Bazel ActionKey projection     (aquery and parity)
  -> REAPI Action projection
       -> REAPI ActionDigest        (remote Action Cache)
```

There is no direct ActionKey-to-ActionDigest mapping. Exact Bazel ActionKey
reproduction alone is insufficient for Bazel local-cache or remote-cache
interoperability. It is nevertheless a necessary input to any future Bazel
local ActionCache reader/writer. That work requires a separate packet covering
cache-record versioning and namespace, discovered inputs, environment and
execution metadata, output validation, and stale-entry behavior before the
exact key may be used for lookup or publication.

Compute the projection on demand in request/phase scratch unless measured reuse
and invalidation justify a DICE projection. Do not retain a second semantic
identity or duplicate the action row. Reuse compact immutable owner fields and
deterministic compact maps already retained by Stage 6, preserve
`Allocative` coverage for any new long-lived wrapper, and benchmark only after
semantic vectors pass. Buck2 fast hashers and `strong_hash` remain useful in
their existing domains but do not implement Bazel's SHA-256 fingerprint
protocol.

This contract does not displace the active M7 source-observation packet. It
schedules FileWrite and later family projections just in time with their owning
Stage 6/8 packets; only residual, not-yet-admitted action families may remain
under M9.

### 6.4C Generic Args/spawn/artifact-symlink category

Selected architecture packet:
`WP-6-7A-generic-args-spawn-symlink-category-architecture-r2` at base
`7b0db03e1`, now independently `ACCEPT`. R1 review returned `REPLAN` for an exact generated-File path
claim, missing effective default action-environment ownership, insertion-
ordered map identity and raw executable/absolute-symlink strings. The corrected
candidate freezes one evaluator-local Args recipe and one action-time
finalization seam into typed retained command-line segments, dense depset-
backed input/tool sources, a common run/run_shell SpawnSpec, canonical
environment/execution-requirement maps and distinct artifact, unresolved-path
and normalized private absolute-path symlink variants.

Before action construction, implement one bounded configured-action-
environment prerequisite from the sole structural native option vector plus a
retained Rust Host observation. It owns exact `action_env`/`host_action_env`
set/inherit/unset and Exec rewrite semantics, strict/default shell policy,
runfiles/shell-path inputs, key-sorted fixed bindings and inherited names, and
per-action env composition. Client inherited values remain command/execution
state, never configured identity; until their resolver lands, such actions fail
closed before execution. Map insertion order is not semantic identity.

Authentic rules_cc 0.2.17 FDO is the first discriminator only. Its chained
Args.add, File path fields, File executable, list/depset inputs, nested depset
tool, artifact symlink and cc_internal absolute-symlink route must use generic
Starlark/action owners with no C++ or parser branch. Plan the complete
non-callback category now, then land bounded environment, scalar/FDO,
vector/paramfile, complete spawn-envelope and unresolved-symlink successors.
String executables reproduce Host-flavored Bazel `PathFragment` normalization;
private absolute symlink targets additionally require absolute classification.
Generated File path/dirname/basename and rendered argv bytes remain Slug-native
until exact Bazel output paths land in M9, while typed relationships and path
operations are exact. Callback mappers, directory expansion, resource callbacks
and new execution/REAPI behavior stay explicitly deferred and fail closed.
Focused R2 rereview and the post-preflight command-surface narrowing rereview
both returned `ACCEPT`; implement only the configured-action-environment
prerequisite before selecting the scalar FDO action successor. That prerequisite
is accepted in `71d34affa`.

The scalar R1 candidate passes its serial Rust validation but terminal review
returns `REPLAN`. The authenticated FDO body exposes a pre-existing generic
provider defect: target-shaped source/generated files rematerialize
`DefaultInfo.files` as strings rather than Files. Configured evaluation also
lacks the recursive source manifest required by the already-designed private
caller check, and the action A/B/A proof did not count parent DICE computes.
Independent architecture review returned `ACCEPT`; activate only
`WP-6-7A-fdo-basic-args-run-symlink-implementation-r2`. It retains typed `AnalysisArtifact` leaves in the one
dense `DefaultInfo.files` depset for every file-target/rule producer, hands the
immutable source manifest into generic configured evaluation, and proves real
publication cutoff. Runfiles, OutputGroup, executable and FilesToRun typed
migration remain the next standard-provider category; add no FDO, rules_cc,
`cc_common`, parser or C++ branch.

The R2 implementation terminally passes. One checked dense
`DefaultInfo.files` owner now retains typed source/generated/declared artifacts;
configured evaluation receives the immutable recursive source manifest; the
A1/A2/B/A3/A4 proof counts real parent-key recomputation; and scalar Args,
Spawn, artifact symlink, and authenticated absolute symlink publication share
the generic typed action sink. The direct-source rules_cc FDO body reaches
configured analysis with no FDO/C++/parser special case. Generated path bytes
and structural identities remain Slug-native, while new-action aquery,
execution, ActionKey and REAPI projection remain deferred and fail closed.
Independent terminal review returned `ACCEPT`; author only the bounded
non-callback vector Args/param-file successor next. That successor is now
authorized after focused architecture correction rereview returned `ACCEPT`.
It fills only the already-reserved vector recipe, param-file policy, and Args-
backed write variants; callbacks, directory expansion, execution and the
remaining Spawn envelope stay deferred.

The R1 implementation terminal review accepts that retained/action
architecture but returns `REPLAN` on two bounded integration details. Bazel's
typed binding rejects an invalid supplied sequence/depset source before
entering unsupported callback handling, and the existing no-op loading action
sink needs a one-line signature adapter that R1 did not allowlist. Activate
only `WP-6-7A-noncallback-vector-args-paramfiles-implementation-r2`: move the
shared source conversion before callback rejection after the two-position name
check, formally admit the frozen adapter blob, and change no retained owner,
compatibility class, parser, C++ branch, callback semantics, or cap.
Focused correction and complete terminal rereview returned `ACCEPT`. The R2
implementation passes the focused ordering/consumer proofs, all four full
owner/direct-dependent suites, the public core check, and hygiene gates at
891/747/1,638 added Rust lines. The remaining Spawn envelope is the next
bounded action category; typed standard-provider breadth remains separate.
Commit `a01a23fe7` freezes that accepted implementation. The first complete-
envelope design returned `REPLAN`: Bazel's public `resource_set` parameter is
callable-or-`None`, not the callback-result dictionary, and an Artifact used as
an executable or direct-list tool may be associated with FilesToRun through an
`executable=True` configured attribute. R2 then returned `REPLAN` because
Bazel checks executable association for each File in a top-level tools depset
but deliberately skips that lookup for a depset nested inside a tools
sequence. Activate only
`WP-6-7A-complete-noncallback-spawn-envelope-implementation-r3`: unify
`run_shell` with the typed Spawn owner, admit only omitted/`None` resource
callbacks, and derive producer-owned executable-Artifact provenance so only
Files admitted by Bazel's exact container-specific association branch are
retained bare. Top-level depset leaves are visited as validation scratch
without flattening retained topology; nested depsets are not inferred.
Associated Files and direct
FilesToRun values fail closed for the later typed standard-provider/runfiles
category. Named/automatic exec groups, shadowed actions, resource callbacks,
execution, and C++ action families remain separate fail-closed categories.
Independent focused R3 correction review returned `ACCEPT`; implement only
that frozen envelope before selecting typed standard-provider/runfiles breadth.

Commit `bfe6f2690` terminally accepts that envelope. One pre-method binding
pass now preserves Bazel's public parameter order and outer-shape diagnostic
precedence before the common typed sink; the terminal correction rereview is
`ACCEPT`. Both action methods publish the same compact immutable Spawn schema,
with shell invocation, scoped executable provenance, environment, requirements,
unused-input discovery and every admitted argument/input/tool/output field in
publication equality. FilesToRun/runfiles, callbacks, named exec groups and
execution projection still fail closed. Select the typed FilesToRun/runfiles
standard-provider category next from actual BCR consumers, without a
`cc_common`, rules_cc, parser or C++ branch.

The complete typed DefaultInfo/runfiles/FilesToRun category architecture is
independently accepted in
`WP-6-7A-typed-files-to-run-runfiles-category-architecture-r1`. It freezes one
effective provider model, typed runfiles occurrences, producer-owned support
artifacts and scoped Spawn expansion, split into four bounded successors. The
first successor may migrate only the provider core and must retain an explicit
incomplete-support guard until runfiles-tree production is admitted.

The first successor,
`WP-6-7A-typed-files-to-run-provider-core-implementation-r1`, is terminally
accepted. It replaces executable path strings with typed Artifacts, retains a
stable files-to-run depset, materializes one dedicated FilesToRun Starlark
value, carries complete providers in root/subrule scope-local provenance, and
fails incomplete associated actions before publication. A bounded terminal
correction also reserves the exact typed RetainedRunfiles/RunfilesSupport
schema from the architecture, without constructing support or opening
runfiles behavior. Select successor 2 for typed runfiles values, merge
operations, and all five DefaultInfo parameters; support actions and Spawn
expansion remain successors 3 and 4.

Bazel 9.2 remains the sole semantic authority. Zabel
`0795445f3ab60f4e49070bdd0b94425c5610f73a` supplies peer phase-ownership,
typed-segment and input-topology lessons only; copy no Zig behavior or layout.
Keep evaluator objects out of retained values and keep semantic action/artifact
identity distinct from rendered paths, Bazel ActionKey and REAPI digests.

### 6.5 Toolchains and Platforms

- Implement constraint values/settings, platform target analysis, registered
  toolchains, toolchain type resolution, execution platform filtering, exec
  groups, and per-action exec properties.
- Registration order must come from Stage 5 bzlmod outputs and command-line
  flags in Bazel order.
- Initial modules:
  `app/slug_analysis_v2/src/toolchains/{registered.rs,resolution.rs,context.rs,exec_groups.rs,platform_constraints.rs}`.
- Replace V1 process-global toolchain state with DICE keys for registered
  toolchains, platform aliases, host fallback, optional and mandatory
  toolchains, target settings, default constraints, and per-exec-group contexts.

### 6.6 Aspects After Base Analysis

- Defer aspects until custom rules, providers, depsets, and toolchains are
  stable.
- First aspect fixture should cover attr propagation and provider requirements;
  advanced incrementality can be a later Stage 8/9 extraction.
- Initial modules: `app/slug_analysis_v2/src/{aspect_key.rs,aspect_analysis.rs}`.
- Model aspect dependency edges explicitly; do not smuggle aspect state through
  the configured-target cache.

## Exact Test Criteria

- `custom-rule-analysis-basic` fixture returns providers matching Bazel for:
  user provider, `DefaultInfo.files`, runfiles, and an output group.
- `ctx-attrs-files-executable` fixture compares `ctx.attr`, `ctx.file`,
  `ctx.files`, `ctx.executable`, `ctx.label`, and `ctx.toolchains`.
- `default-info-runfiles-executable` and `provider-output-group-basic` compare
  normalized provider keys, files, runfiles, and output groups.
- `depset-orders-and-rejections` fixture covers all Bazel orders, incompatible order
  failures, nested depsets, duplicate handling, and flattening order.
- `depset-orders-and-rejections` fixture compares `to_list()` order and
  rejection diagnostics.
- A focused structural test proves that combining nested depsets preserves
  shared child-node identity and does not flatten or recursively clone the DAG.
- `actions-api-basic` fixture declares write, run, run_shell, symlink, and
  expand_template actions and compares action IR to Bazel's normalized
  `ActionGraphContainer`.
- `action-declare-file-package-boundary`, `action-run-shell-basic`,
  `action-run-tool-exec-cfg`, and `action-conflicting-output` compare output
  paths, mnemonic, argv, env, tools, inputs, outputs, and diagnostics.
- `toolchain-resolution-first-platform`, `toolchain-resolution-host-platform`,
  `toolchain-resolution-platform-alias`, and `toolchain-mandatory-missing`
  compare selected execution platform, resolved toolchain labels, events, and
  missing-toolchain diagnostics.
- `exec-groups-action-platform` proves per-action exec-group platform selection.
- `transition-basic` fixture executes a user transition and proves outgoing
  configuration affects a dependency.
- `aspect-provider-propagation` runs only after base analysis is stable and
  compares aspect-produced providers and actions.
- Registered toolchain and platform edits invalidate through named DICE keys.
- `rg -n "std::fs|process-global|CellResolver|buck-out" <v2-analysis-crates>`
  returns no semantic production shortcuts.

## Acceptance Criteria

- Custom Starlark rule fixtures produce the same providers/actions as Bazel.
- Depset construction is cheap shared-DAG composition; flattening/copying is
  confined to explicit Bazel-visible operations.
- The retained depset core starts from an independently implemented,
  Zabel-informed dense/packed design and records Bazel 9.2 oracle evidence plus
  retained-memory and consumption measurements. Buck2 ideas are optional later
  experiments, and no Zabel production code is copied.
- Toolchain and platform fixtures match Bazel for focused public examples.
- Action declarations produce REAPI-ready command/input/output structures.
- No analysis shortcut depends on Buck cells or direct filesystem scans outside
  DICE-tracked inputs.
- A multi-target fixture proves recursive dependency analysis, provider flow,
  shared subdependency reuse, and deterministic action ownership in one
  same-daemon DICE graph.
- `AnalysisResult` contains the providers actually returned by each rule and
  the actions actually registered during its implementation; no output-derived
  provider synthesis or command-specific mock graph remains.
- Stage 8 `cquery` and `aquery` consume this graph without re-evaluating rules
  in separate command-owned state.

## Validation

```bash
cargo test -p slug_analysis_v2
cargo test -p slug_build_api_v2 depset
cargo test -p slug_analysis_v2 toolchain
slug-v2-oracle run --fixture custom-rule-analysis-basic --compare providers,actions,outputs,diagnostics
slug-v2-oracle run --fixture depset-orders-and-rejections --compare stdout,stderr
slug-v2-oracle run --fixture actions-api-basic
slug-v2-oracle run --fixture toolchain-resolution-first-platform --compare providers,events
slug-v2-oracle run --fixture exec-groups-action-platform --compare actions,providers
slug-v2-oracle run --fixture transition-basic
slug-v2-oracle run --fixture aspect-provider-propagation --compare providers,actions
```


## Selected-toolchain request contract (pending combined R2 acceptance)

#### Remaining group decisions resolved by pinned source

At Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a`:
`RuleClass.AutoExecGroupsMode.isEnabled:169-193` makes ordinary dynamic mode
use the private attribute when present (false and true both override the flag),
otherwise the native flag. Auto on empties default requirements and creates
one group per declared toolchain type; auto off retains the default requirements.
`skyframe/toolchains/ToolchainContextUtil.java:54-92,120-245` and
`DeclaredExecGroup.process` own this normalization and target group constraints.

`RuleContext.java:1091-1141` uses direct automatic-group lookup followed by
resolved requested-label alias lookup for `ctx.toolchains`; action
`toolchain=` does not get that alias-search fallback. `StarlarkRuleContext.java:
860-909` builds a composite automatic view or default view and thin
`ctx.exec_groups`. Explicit action group names still pass public-name validation;
the internal automatic label string is not a user-nameable `exec_group=`.
`StarlarkActionFactory.java:129-138,405-472,718-895` distinguishes omitted
toolchain from explicit None. Under auto policy, no explicit group and multiple
contexts, an unassociated executable/tool requires explicit toolchain or None;
recognized dependency runfiles and string executables do not trigger that check.
Nested tool depsets trigger it before element-type conversion. This is validation,
not automatic inference of a toolchain label from an artifact.
Slug's `subrule_invocation.rs::bind_toolchain` currently erases None/omission;
the future complete group packet must fix that at the binder, not in the sink.

`AutoExecGroupsTest:303-431,488-705,750-824,1279-1305,2500-2558` supplies the
policy/action/alias discriminators. Ordinary rule inheritance via `rule(parent=)`
is not currently admitted by Slug; preserve its rejection, not an invented copy
keyword. `rule(test=True)` injects a test group in Bazel
(`StarlarkRuleClassFunctions.java:1064-1080`) and has separate test-action
ownership; that category and configured aspect/subrule toolchain contexts remain
deferred. These decisions activate nothing in the selected prerequisite.

#### Contradiction and authoritative correction

Bazel's direct toolchain implementation edge does **not** apply the ordinary
Exec configuration transition. `analysis/producers/DependencyProducer.java:
161-176` passes the parent's configuration and separately the selected group's
execution-platform label; `PrerequisitesProducer.java:350-355` attaches that
label to the configured-target key. `skyframe/ConfiguredTargetKey.java:
127-139,188-220` includes it in equality/hash. The pinned
`skyframe/toolchains/README.md:31-45` explicitly distinguishes this mechanism
from a configuration transition. Toolchain-resolution tagged trimming
(`RuleTransitionApplier.java:187-218`) is not an implementation-edge Exec
transition; manual feature-flag trimming remains unadmitted in Slug.

In contrast, live `dice.rs::prepare_selected_toolchain_context:3525-3695`
constructs implementations in the selected platform's Exec configuration.
`tests/starlark_rule.rs::selected_toolchain_accepts_declared_actions_and_default_outputs`
asserts Exec and removal of target-scoped settings at 2625-2643. Those
assertions encode the defect, not compatibility evidence. The new packet must
replace them: preserve the parent's configuration/target-scoped settings on the
implementation edge; only the implementation's actual Exec attributes perform
Exec projection. Semantic option loss cannot be classified as a Slug-native
checksum/path-byte divergence.

The child default context receives the separate preference through
`DependencyResolver.java:477-510` and `ToolchainContextUtil.java:120-143`.
It is a **preference**, not unconditional forcing:
`ToolchainResolutionFunction.java:324-359` selects it first if suitable, otherwise
continues ordinary eligible-candidate selection. `PlatformKeys.java:255-305`
looks it up among the known target/execution platforms; absent preferences do
not trigger new discovery. Zero requirements still honor a suitable preference.
Allowed-toolchain-type platform policy is already outside Slug's admitted
platform schema; do not silently admit it while implementing this preference.

Propagation is edge-specific. Incoming rule-transition delegation preserves
the preference and the existing apply-transition marker
(`TargetAndConfigurationProducer.java:304-316`). Ordinary Target/Exec/Starlark
attributes, including alias.actual, do not copy it
(`DependencyProducer.java:268-334`; `rules/Alias.java:61-96`).
A direct alias wrapper may carry the preference, while its actual does not.
Each nested direct toolchain edge gets its own selecting context's platform.
Future named/automatic child contexts use fresh unforced resolution keys
(`UnloadedToolchainContextsProducer.java:100-124`), not the child default's hint.
Direct source/nonconfigurable implementations still fail the existing
ToolchainInfo/provider boundary; this packet does not invent a successful
nonconfigurable toolchain category.

#### Frozen prerequisite representation and producer contract

Add `toolchain_execution_platform: Option<Arc<CanonicalLabel>>` for the preference
to `ConfiguredTargetKey`, not `ConfigurationKey` or the native option vector.
Default construction is None; one explicit toolchain-key constructor/builder
sets it from the selected platform's canonical actual label. Allocate/share one
label Arc per selected context across implementation requests. Derived Eq/Hash/
Ord/Allocative must include label contents, not pointer identity. Preserve it
on same-node incoming-transition delegation, including non-idempotent final
keys. Keep user-facing label/configuration display unchanged; display is not
semantic identity.

The existing `ConfiguredNodeAnalysisKey` and observation wrapper inherit this
field through their node; no new DICE key family or global owner. Extend
`ConfiguredToolchainResolutionKey` and its observation wrapper with the same
optional structural preference, supplied from the analyzed owner's key.
Resolution retains existing tracked registration/platform/toolchain producers
and selection order, changing only suitable-preference priority, including
empty requirements. Known target-platform preference may need an Exec-shaped
platform view for Slug action ownership; obtain it through the existing tracked
platform producer without changing the candidate registration sequence.
Add `known_preferred_execution_platform: Option<ConfiguredTargetKey>` to
`ToolchainTopology`, produced only from the resolver's tracked known-platform
lookup and passed through analysis finalization. Its constructor validates Exec
shape and accepts a selected platform only from candidates or this explicit
known preference. Keep the field in structural equality/Allocative; do not
fabricate registration. A target platform outside the candidate sequence has no
resolved toolchain matches: empty/all-optional requirements can select it, but
do not run declaration matching against it as an extra registered candidate.
Preserve pinned `PlatformKeys.find` alias lookup: resolved target and host labels,
ordinary registered requested labels; the selected platform's actual label is
the preference on a direct implementation edge. An actual label reachable only
through an ordinary registration alias is not arbitrary discovery permission.

Selected implementation requests use the parent owner configuration unchanged,
plus the preference. Requested selection rows keep that full key; actual rows
keep the exact analyzed/delegated result key. Update the context invariants that
currently equate implementation configuration with execution-platform Exec
configuration: requested implementation configuration equals owner configuration;
actual identity is the producer's validated transition/alias result, not a
reconstructed key or an assumed unchanged configuration. Toolchain type
requests remain in owner configuration and action platforms remain Exec-shaped.

Replace lossy label/configuration reconstruction in `compute_actual_child` and
`observed_configured_result` with the existing full-node compute path.
Ordinary attribute construction deliberately starts with no preference; this is
not a blanket propagation rule. Preserve full keys through cycle tracking,
selected-toolchain edges, query/closure membership and runfiles package collection.

Extend the Arc-backed `AnalysisConfiguredTargetKeyData` with the same optional
shared label. Add one analysis-owned projection from full configured key and use
it at all four production conversion sites: dice artifact lowering, loaded-rule
owner creation and both materializer target routes. Derived artifacts, providers,
FilesToRun and runfiles then retain this owner distinction through existing
structural equality. Never append it to the configuration byte payload.

FileWrite semantic identity adds one optional tagged canonical-label field to
its configured-owner encoding; None preserves existing bytes, Some differs even
for identical label/configuration/content/platform. This is a Slug-native owner
projection, not Bazel checksum, output-path spelling, ActionKey or REAPI digest.
No new execution/action family or output-path algorithm is admitted.

#### Proof, lifetime, complexity and stop contract

Pinned tests: `ToolchainResolutionFunctionTest.resolve_forceExecutionPlatform`,
`_alias`, `_host`, `_host_alias`, `_noRequiredToolchains`;
`ToolchainsForTargetsTest.keepParentToolchainContext` is a synthetic-key source
discriminator, not a prior successful end-to-end Slug replay. Add source-derived
Rust regressions for the live consumer: two registered platforms A then B, parent
toolchain restricted to B, implementation with no toolchains and a FileWrite;
its options stay Target while its action platform is B. Include ordinary Target
and Exec child controls, nested toolchain selection, suitable preference over
greater optional coverage, unsuitable/absent preference fallback and no match,
platform aliases, incoming transitions, alias.actual clearing and missing source/
wrong-provider terminals. The existing named-group guard remains covered.

Required same-DICE cases: source and registration/platform/property A/B/A;
target-setting/native option preservation versus actual Exec projection;
None/A/B/A request interleaving; concurrent distinct preferences; cancellation/
Need/error and recovery without partial parent/provider/action publication.
Retained provider/derived-file identity and FileWrite owner bytes must distinguish
None/A/B even when all other fields match; None preserves old controls. Use full
retained keys, not helper reconstruction that omits the field.

Memory is DICE-retained semantic state: one nullable Arc per configured key/
retained target payload/resolution key plus the topology's optional known key,
sharing label/configuration storage rather than new maps.
No command cache, new interner, global registry or unowned evaluator borrow.
Constructor/key scratch and joined child buffers release with their phase; views
expire with existing tokens; DICE reachable versions/results own retained release
and service shutdown. Cancellation may leave valid DICE child cache entries but
publishes no incomplete parent; existing scoped joins/cycle guard and final source
certificate remain authoritative. Never hold a shared lock over a DICE await.
Reuse current Arc/Allocative utilities (Stage 9 Stages 3/6 utility row); V1/Buck2
are concept/utility guidance only, no donor code or new dependency is imported.
DICE worker equality/dependency/cancellation tests cited in the prior section
remain applicable, not Bazel semantics.

The original source-derived scope estimate was 900 production / 1,800 proof /
2,700 aggregate additions. Combined R2 has its own preserved receipt; do not
apply the superseded standalone estimate as a hard rejection gate.
The seven production files stay cohesive: key/carrier definitions, four thin
projections, existing resolution/delegation orchestration, result invariants and
the existing owner encoder. `dice.rs` stays the established single producer;
no new policy dispatcher or helper graph. Its selection helper must remain local
and bounded rather than copying the 460-line resolver. Large carrier/evaluator
files receive only fields/projections/invariants; tests reuse existing scaffolds.

The implementation must first add the nonfirst-platform/parent-option regression
and observe its expected failure, then fix the complete request category.
No archive/real-workspace replay is needed to discriminate this prerequisite.
No source overrides, fresh graphs, disabled provenance or longer-timeout workaround.
A new configuration domain, alternate forced-key side store, arbitrary preference
discovery, missing retained consumer, a contradicted contract requires REPLAN. Size estimates and focused
corrections follow the orchestration recovery policy. Group constraints/policy/views/action routing remain the immediate
post-correction design, not silently forgotten or implemented piecemeal.


## Configured-action closure integrity contract (pending combined R2 acceptance)

Independent architecture review ACCEPTS the combined selected-request/conflict
successor after one correction: retain the normalized missing-toolchain message.
This contract supersedes the preceding reserved conflict decisions, not the
accepted selected-request edge/resolution contract. No candidate Rust is restored
by this docs milestone. The implementation must deliver both changes together.

#### Source semantics and compatibility boundary

Authority is Bazel git object `8220c6198837d5c13d53fea211cf3282aa12408a`, not
the working-tree Java files. `actions/Actions.java:85-115` compares mnemonic,
Action implementation eligibility and action key, then ordered mandatory inputs
and outputs ownerlessly for two shareable actions. There is no explicit Java
class comparison. Its both-unshareable branch instead permits equal ownerful
input/output aliases; one-shareable/one-unshareable cannot share. The earlier
read-only audit's simplified "both must be shareable" is not the full predicate.
`Artifact.java:690-697` defines ownerless identity as root plus exec path;
`MapBasedActionGraph.java:49-66` registers by that identity, not configured owner.

For admitted string `ctx.actions.write`, `StarlarkActionFactory.java:360-379`
always calls the same FileWrite factory. `FileWriteAction.java:190-202,249-355`
selects compression deterministically from string length and fixed policy;
regular/compressed keys use distinct GUIDs plus executable bit and content or
compressed content. Equal text under this one factory therefore has equal
representation/key inputs even above 256 bytes; comparing structural text does
not require recreating gzip or claiming exact ActionKey bytes. Native custom
compression policies, lazy text and input-bearing writes are not admitted here.
Args writes are ParameterFileWriteAction, not this family despite their mnemonic.

`ActionKeyComputer.java:37-57` adds execution-platform presence/full PlatformInfo,
exec properties and a process-constant uniquifier. `AbstractFileWriteAction.java:
124-127` returns EMPTY action exec properties: target/group property overrides
must not cause false FileWrite conflicts. Raw platform properties still matter.
`PlatformInfo.java:164-179`, `ConstraintCollection.java:275-284` and constraint
value/setting `addTo` establish actual platform label, ordered setting/value
labels, raw platform properties and normalized missing-toolchain error message
for Slug's admitted no-parent/default-policy platforms. `Platform.java:98-100`
supplies the native attribute; `PlatformInfo.java:363-369` maps empty string to
None, preserving nonempty text. Slug already accepts explicit native overrides
(package.rs:2289-2294,8971-8991); retain them, do not add a new rejection guard.
Configured-key configuration, preference, toolchain provenance and
merged owner properties are not substitutes for these inputs. Existing platform
guards keep parents, remote properties, flags and toolchain policies unsupported.

Exact named behavior: scalar FileWrite same-output sharing/conflict, generic
strict-segment output-prefix rejection and runfiles-tree/MANIFEST exemption.
Slug-native: structural configuration/root identity, deterministic first-error
diagnostics, structural equality rather than Bazel fingerprint/hash collisions,
and existing output/display bytes. Exact checksum/ActionKey bytes remain deferred.
All registered outputs in the requested dependency closure enter the inventory,
including unused outputs and File/Directory/Symlink/RunfilesTree kinds. Prefixes
are checked within the same structural output root; equal paths are handled by
exact collision checking. Only a RunfilesTree outer output exempts its immediate
MANIFEST path, never an arbitrary Directory or other nested child (Actions:291-368).

The accepted executable/aquery family remains scalar FileWrite. Typed Spawn,
ArgsWrite, artifact/absolute symlinks and four runfiles-support families retain
their configured-analysis admission and existing execution/formatter guards.
Noncolliding outputs are not newly rejected. Their cross-owner exact-output
equivalence is explicitly unsupported in this packet: a collision requiring
one of those family keys returns an unsupported-equivalence terminal, NOT a
claim that Bazel rejects it and NOT ActionSpec/REAPI-digest equality. Legacy
Run/RunShell/WriteJson/ExpandTemplate/Symlink rows receive the same treatment.
This leaves broader-family sharing for its family admission work; do not
silently omit their outputs, invent a blanket owner inequality rule or broaden
their executor. Configured aspects remain unsupported at their existing guard.

#### Natural owner, publication and consumers

Conflict freedom is a ROOT-SET-dependent fact, not a ConfiguredNodeAnalysisKey
invariant. A and B may each build successfully while A+B conflicts; an unchanged
child reused from DICE must still participate in a new root-set validation.
Keep `compute_build_action_closure` as the single traversal/producer, after all
roots and transitive configured children complete and before its Complete(Ok).
Extract a private `runtime/configured_action_closure.rs` for a pure constructor
of `ValidatedActionClosure`, with private fields, owners Arc slice and compact
shared-duplicate action coordinates. Empty/loading-only closures use its empty
constructor. No unchecked nonempty conversion or success boolean beside raw data.
BuildCommandEvaluation retains this immutable value; every observed singleton,
legacy and observed multi-root constructor uses the same producer. No new DICE
key family: existing command-root keys already retain ordered roots/workspace/
configuration policy and depend on all full configured node keys.

Retain raw platform facts separately from merged execution properties in
ConfiguredActionOwnerContext, supplied by its existing constructor BEFORE merge;
do not recover them by closure lookup or reverse a target-property override.
Add one optional shared PlatformSemanticFact (Arc-backed properties) and accessor.
Extend PlatformSemanticFact itself with `missing_toolchain_error: Option<Arc<str>>`,
read/normalize it at platform_semantic_fact from the loaded native attribute, and
carry it unchanged through ConfiguredPlatform and property merges. Include both
changes in structural equality/Allocative. Unresolved contexts retain None.
FileWrite equivalence uses optional actual platform label, ordered constraint
labels and this full raw fact, mnemonic, exact content and executable bit. Scalar
shape validation proves one File output and empty inputs/tools/other execution
fields; no-platform pairs can be compared during analysis without admitting
them to the selected-platform executor. Missing/inconsistent raw selected facts
fail closed. FileWriteSemanticIdentity must also encode raw facts when they differ
from its already encoded merged properties (new optional tag). A second optional
tag encodes the normalized message when unequal to the pinned native default;
its framing distinguishes None from nonempty text. The default constant is only
an encoding elision rule, never substituted for the loaded source fact. This
preserves old default-platform bytes while distinguishing empty/custom messages.
No broader no-toolchain diagnostic-format claim is made. Neither this identity
nor full owner equality decides sharing.

Output identity is `(owner structural SlugConfiguration, bin root, relative
output path)` inside one workspace; preferences never change the output root.
Use borrowed scratch output rows sorted by structural configuration bytes,
path SEGMENTS and existing owner/action/output encounter ordinal. Exact collision
pass precedes prefix pass; report the first incompatible pair in that order.
For an equivalent FileWrite group choose its first closure-order action as the
execution representative. Retain all owners/actions for analysis, action counts,
literal/deps aquery and diagnostics, plus only nonrepresentative coordinates in
an immutable Arc slice. This is explicit sharing, not lost owner/action identity.
CLI/server use a new execution-view accessor that filters those coordinates;
existing semantic-view accessors remain owner-complete. Run consumes validated
evaluation but keeps its current one-action/executable restrictions. The low-level
test-only/document-hidden view constructor is not a graph validation certificate.
No materializer/REAPI executor repair or command-side collision scan is added.

Cquery deliberately remains independent: pinned `CqueryCommand.java:191` sets
checkForActionConflicts=false. Query/loading do not own an action closure either.
Aquery uses the validated build closure even when its formatter selects literal
owner rows. CLI/server/build/run cannot receive a successful conflicting closure,
so failure precedes execution RPC, action-output materialization and formatting.
Exact errors and unsupported-equivalence errors become structured retained
BuildCommandError variants, with path and both full owners, exit 2 and existing
source-certificate publication. Need/analysis failure precedence is unchanged:
validate only a completed closure; cancellation/outer errors publish no value.

#### Lifetime, proof and implementation boundary

Scratch is O(outputs + path depth) borrowed rows/ancestor stack and phase-local
duplicate coordinates; sorting is O(outputs log outputs), no pairwise all-action
scan. Retained growth is one shared raw-platform fact per action context, one
optional shared message per platform fact (cloned without text copies), and
O(shared duplicate actions) coordinates per command-root result. No retained
output map, global registry, cache, interner, new dependency or mutable service
state. Reuse Arc/Allocative and Stage 9 Stages 3/6 utilities; Buck2/V1 are concept
guidance only, no code imported. Unchanged derived equality permits DICE cutoff;
raw fact edits invalidate even when owner overrides mask the merged properties.
Results release with reachable DICE versions/command tokens, scratch on return,
cancellation or error; existing scoped joins/shutdown own async release. No lock
crosses an await. Existing observation/final-certificate checks remain mandatory;
unavailable historical Host state is rejected, never synthesized.

Adapt OutputArtifactConflictTest invalidation/new-target/overlap, unused action,
repeated/null build and directory-nesting tests to existing inline temporary
workspaces; preserve source attribution, with no copied oracle tree. Add same-
path equal-content share versus changed-content/executable/platform/raw-property
conflicts, equal raw/unequal merged-property sharing and unequal raw/equal merged
conflicts, default/empty/custom missing-toolchain message A/B/A and unchanged-value
cutoff, long/unicode content, distinct configuration/path controls, deterministic
multi-conflict order, runfiles exemption and non-FileWrite unsupported-equivalence
classification. Prove A and B alone succeed, combined fails, warm repeats fail,
source A/B/A restores and concurrent disjoint requests do not poison one another.
Preserve cquery success and all owners in aquery; execution emits one representative
per shared group. The saved selected-toolchain red regression must fail at the
new producer before RPC, not be weakened to a later accessor assertion.

Retained Buck2 worker tests `when_equal_return_same_instance`,
`test_detecting_changed_dependencies`, `mismatch_epoch_results_in_cancelled_result`
remain guidance; reuse Slug observed Need/error/cancellation and source certificates.
CLI one-shot/stable-daemon build/run/aquery conflict tests use local fixture inputs,
zero accepted mock-transport calls and no output changes. Run positive sharing
through the common core execution-view/REAPI plan boundary without external replay.
Full owner and direct-dependent gates remain required after focused discriminators.
No performance improvement is claimed; large-closure benchmark work is not reopened.

Exact original files and test results are preserved in candidate
`27e9e9c0c`; the fixture/gate ledger records outstanding acceptance. Existing giant analysis/core dice files receive only cohesive producer
integration and field plumbing; the new validator and separate core proof module
prevent another policy subsystem accumulating in either file. No fallback exists.
Missing source facts, a newly admitted action-family key, broad execution change,
output suffix or a key side store requires REPLAN. Routine corrections
and growth-estimate review follow the orchestration recovery policy. Named/automatic group design resumes immediately after combined acceptance.
Design validation: pinned-source/structure checks and git diff --check pass;
archive check retains exactly three known thoughts-path failures. Saved candidate
hash/applicability remain verified. No Rust, build, test, replay or materialization
ran for this design; implementation and previously unrun gates remain unaccepted.


## Execution-group successor (blocked on R2)

### Rule execution-group runtime prerequisite: named-only REPLAN (2026-09-10)

The selected 0.2.17 cc_library has named `cpp_link` **and** an explicit true
automatic-group policy. Stage 4 authenticates the entire declaration and
corrects the named-transition attribution. The named-only audit terminates in
`REPLAN`: before runtime activation, design the shared rule execution-group
category including automatic policy, requirements and action selection.
Successor `WP-4-6-7A-rule-execution-group-runtime-design-r1` is docs/source only.
It must freeze a bounded implementation contract, not another carrier-only
loading bypass. No guard moves, runtime code, test fixture or representation
change is admitted by this audit.

Independent terminal architecture review returns `ACCEPT` for this docs-only
REPLAN and successor selection; it does not approve runtime implementation.

#### Pinned semantic basis and discriminators

Authority is Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`.
Use that git object in `/home/wgray/bazel`; its working HEAD is different.
Paths below are relative to Bazel's `com/google/devtools/build/lib` Java main
or test trees. These are source-established requirements, not new passing
Slug tests or a whole-category Bazel parity claim.

| Concern | Source and discriminating test |
|---|---|
| Named requirements/constraints and internal default-copy | `packages/DeclaredExecGroup.java:43-149`; `packages/RuleClassBuilderTest.testDuplicateExecGroupsThatInheritFromRuleIsOk`. `COPY_FROM_DEFAULT` is internal, not a public `copy_from_rule` keyword. |
| Independent resolution, including no-toolchain groups | `analysis/producers/UnloadedToolchainContextsProducer.java:100-124`; `skyframe/toolchains/ToolchainResolutionFunctionTest.resolve_noToolchainType`, `.resolve_execConstraints`, `.resolve_noMatchingPlatform`. Each group has its own requirements/exec constraints under the owner's target configuration. |
| Named dependency transition | `analysis/DependencyResolutionHelpers.java:231-255`; `analysis/StarlarkExecGroupTest.testExecGroupTransition`, `.testInvalidExecGroupTransition`. Resolve the named group's execution platform; unknown name errors, never default fallback. |
| Provider view | `analysis/starlark/StarlarkExecGroupCollection.java:43-159`; `starlarkbuildapi/platform/ExecGroupCollectionApi.java`. String-indexed thin views expose resolved `.toolchains`, exclude default and report unknown names. Collection absence differs from an empty resolved toolchain map. |
| Properties | `analysis/ExecGroupCollection.java:70-180,195-245`; `StarlarkExecGroupTest.testSetExecGroupExecProperty` and unknown/inherited/override tests at 605-712. Validate target group prefixes; merge using the selected platform for each group. |
| Action routing/failure | `analysis/starlark/StarlarkActionFactory.determineExecGroup:855-895`; `StarlarkExecGroupTest.testExecGroupActionHasExecGroupPlatform`, invalid-action tests at 395-438. Validate existence/public name, then bind that group's owner/platform; automatic mode also checks explicit toolchain/group agreement. |
| Automatic policy/group derivation | `DeclaredExecGroup.process:131-148`; `analysis/AutoExecGroupsTest` flag/attribute matrix at 303-431, group membership at 756-770, action inference/mismatch at 505-704/1281-1304. Per-toolchain generated names are distinct from public identifiers. These are required successor research, not admitted behavior. |

Property precedence must account for `computeProperties` filling missing
target-group keys from target defaults before its final merge. Effective
precedence is platform-default < platform-group < target-default < target-group;
a target-default value therefore beats a same-key platform-group value. Add
that discriminator, not just the simpler target-group override case.
Toolchain selection invariants may reuse `SingleToolchainResolutionFunctionTest`
multiple-platform/requirement equality tests; they do not prove the public
group API. Aspect group propagation and `AspectAutoExecGroupsTest` remain
deferred configured-aspect breadth. Built-in test-runner groups and inheritance
must be explicitly classified by the successor before any generic rule claim;
neither can be inferred from this non-test cc_library declaration.

#### Live owners and required replacement boundary

In `slug_loading_v2/src/package.rs`, `DeclaredExecGroup` currently retains
only toolchains; nonempty group constraints reject. Frozen declarations retain
the sparse group/name-index carriers, then `FrozenRuleDefinition::invoke:7636`
rejects them before initializer/computed-default checks and publication.
`StarlarkRuleImplementation:855` and its structural equality have no group
carrier. `attrs.rs::AttributeDependencyConfiguration` has Target/Exec/Starlark,
not a named execution-group edge. Builtin target constraints/property schemas
already exist; declaration shape alone is not a configured semantic owner.

In `slug_analysis_v2/src/dice.rs`, `ConfiguredToolchainResolutionKey:216`
contains workspace/configuration/requirements but **no group exec constraints**.
`compute_configured_toolchain_resolution` selects the first platform immediately
for empty requirements; it cannot implement a constraint-only named group by
calling the unchanged key. `root_declared_dependency_keys:1444` accepts one
exec configuration and marks every Exec edge Default. Root preparation at
5713-5995 resolves one context; `prepare_selected_toolchain_context:3525`
and the no-toolchain branch manufacture Default action contexts.

`starlark_rule.rs::AnalysisContextGen` exposes one `toolchains` view and no
`exec_groups`; `validate_default_spawn_context:1378` rejects explicit groups
and nondefault toolchain selection. `result.rs::ConfiguredActionOwnerContext`
already retains structural group/platform/toolchain identity and validates
matches; `with_action_contexts` can match multiple contexts, but this is not
evidence that the live producer supplies them. Its current property helper
is not proof of Bazel's group-prefix/default-fill semantics. The detached
`toolchains/exec_groups.rs` prototype has a mutable second context map, owned
String/Vec/BTreeMap storage and no live DICE provenance: avoid activation.

#### Cross-stage ownership and proof obligations

The natural declaration producer is the loaded rule/package, with compact
detached requirements, constraints and named transition identity participating
in `StarlarkRuleImplementation`/schema equality. The natural normalization
producer is configured-target preparation: combine declaration, resolved target
attributes and the immutable native command configuration, never CLI-side
reconstruction. The native option registry already contains
`incompatible_auto_exec_groups`; the successor must establish its runtime
projection and attribute precedence rather than introduce ambient flag reads.

Resolution must consume all normalized requirements and group constraints
through tracked keys, preserving ordered registrations, alias normalization,
target settings and distinct target/selected Exec configurations. The successor
decides whether to extend the existing structural resolution key or extract
its cohesive owner; it must justify equality sharing when two groups have
identical resolution inputs without collapsing their group/action identities.
One immutable configured-target-owned group collection must supply named
dependency transitions, provider views, property selection and action routing.
No second semantic registry, command cache or evaluator-owned retained map.

Default/named/automatic identity, normalized inputs, selected platform,
configured toolchain providers and merged action properties must remain
structural at their natural owner and affect downstream equality/invalidation.
Configuration identity, output-path/display projections, Bazel ActionKey and
REAPI/content digests remain separate domains. No exact checksum/path claim.

New declarations/contexts are DICE-retained semantic memory, not command
scratch. Prefer current CompactString/Arc-slice/SmallMap/Allocative utilities
(Stage 9's Stages 3/6 utility and Stage 6 platform-property rows); Buck2 is
utility/concept guidance only. Views/normalization/join buffers are evaluator
or phase scratch; no retained borrow of scratch or unowned heap. Publish only
a complete immutable collection after all required group resolutions/children
and final source-certificate validation. DICE may retain independently valid
child results on parent failure, but no partial parent/provider/action escapes.
Cancellation joins scoped work; no manual lock crosses a DICE await. DICE
version reachability owns retained release; views expire with the analysis
token, scratch with its phase, and service shutdown drops runtime-owned state.

Required proof is same-DICE source/configuration/platform A/B/A (including
constraints, automatic policy, toolchain payload and properties); absent/edit/
delete/recreate and unavailable-history rejection; overlapping request policies
without cross-contamination; equality cutoff for unchanged normalized inputs;
Need/error/cancellation and default-only nonregression. Reuse retained Buck2
`dice/dice/src/impls/worker/tests.rs` tests `when_equal_return_same_instance`,
`test_detecting_changed_dependencies`, `mismatch_epoch_results_in_cancelled_result`
as ownership guidance, not compatibility evidence. Existing Slug
`tests/starlark_rule.rs` default-exec all-label-shapes, selected-platform terminal,
zero-toolchain, recursive invalidation and cancellation tests are controls;
`tests/toolchain.rs` prototype collection tests do not prove the live path.

The successor must freeze executable proof commands and upstream adaptations,
Rust/test ownership scope, production/proof growth estimates, all direct
consumers and a concrete split/cohesion decision for `package.rs` (11,851 lines)
and `dice.rs` (6,055 lines), before implementation review. This audit proposes
no final representation/API or cap by guesswork. No fallback is introduced;
guard removal is contingent on complete reviewed runtime ownership. Later
computed defaults, C++/Java providers/actions, aspects and execution remain
separate unsupported owners. The path-epoch performance issue is unchanged;
do not restart checkout-wide replay, exceed the user's test limits, disable
provenance or use a fresh graph to hide it.


## Historical evidence

The original 23,981-line owner record is retained at `c5e7414d7`.
[Plan history](../plan-history.md) indexes the exact baseline sections and
acceptance inventories. The R2 snapshot is commit `27e9e9c0c` on
`review/output-conflict-r2`, with base `97dffd5d4` and original patch/receipt.
No preservation or document edit accepts that candidate.
