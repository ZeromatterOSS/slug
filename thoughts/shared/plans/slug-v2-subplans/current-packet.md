# Current Slug V2 Packet

Packet: `WP-4-5-7A-subrule-configured-hidden-dependencies-and-query-r4`

Milestone: M7A bootstrap-critical generic Starlark/ruleset closure.

Base: accepted loading producer `965cfde5e` plus accepted command/configuration
producer `4425d3bfb`. Unrelated dirty analysis/toolchain, evaluator-adapter,
loading, core and REAPI work remains parked. Stage and validate only this
packet's exact hunks.

## Observable result

For a rule with attached subrules, resolve every lifted literal or admitted
typed `configuration_field` label/default before Starlark invocation. Configure
target dependencies in the owner's configuration and exec dependencies in the
actual selected execution platform's Exec configuration. Reuse the existing
configured-child/cycle/Need/alias pipeline, validate providers, single-file
shape and executability, and retain ordered phase-scratch dependency facts.
Ordinary rule attributes using the same typed defaults take the same route.

Root-package loading `query` exposes deterministic synthesized attributes and
implicit edges for literal hidden defaults. Because this packet deliberately
terminates analysis before invocation, configured hidden edges are not
publishable until the direct-call successor succeeds. The packet creates no
evaluator-visible target, artifact or files-to-run wrapper and publishes no
configured result or action.

## Learned facts and authority

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the
sole semantic authority:

- `StarlarkSubrule.java:132-192,227-284` lifts transitive attributes, forbids
  overrides, and supplies configured target/list, single Artifact, or
  `FilesToRunProvider` values only at the later call boundary;
- `StarlarkSubruleTest.java:801-1219` proves hidden-in-`ctx.attr`, literal,
  single-file, executable/Exec and late-bound behavior;
- `CppConfiguration.java:184-196,308-319,632-759,958-961` defines the ten
  admitted `cpp` fields, suppression and derived `zipper`; and
- `FileConfiguredTarget.java:79-104` defines inherent file
  `DefaultInfo`/`FilesToRunProvider` behavior; `Attribute.java:2113-2127`
  defines tool edges; and
- `src/test/shell/integration/subrules_test.sh:98-119` proves hidden literal
  dependencies and default rows in XML. This packet admits the graph facts,
  not XML spelling.

Authenticated rules_cc 0.2.17 `cc/private/toolchain/fdo/fdo_context.bzl`
declares eight hidden rows using target configuration, provider predicates,
single-file and executable shapes. Its ordinary `_libc_top` and `_zipper` rows
prove the same producer is not subrule-specific.

Buck2/starlark-rust retained collection and DICE guidance applies to compact
shared identities and dependency recording. Zabel is concept/optimization
guidance only: sparse retained descriptors and borrowed projections are useful;
copy no Zig code, names, layouts, diagnostics, or behavioral claims.

## Compatibility boundary

**Exact:** the finite `cpp` fields `fdo_optimize`, `xbinary_fdo`,
`fdo_profile`, `cs_fdo_profile`, `fdo_prefetch_hints`, `propeller_optimize`,
`memprof_profile`, `proto_profile_path`, `libc_top`, and `zipper`; Bazel's
suppression/derived-zipper rules; absent projection as no dependency; literal
label/label-list defaults; target versus selected-platform Exec configuration;
OR-of-AND provider predicates; every label-list child; source/generated
single-file cardinality and extension constraints; executable availability;
hidden-attribute invisibility; and dependency-error precedence before the
unsupported invocation terminal.

**Slug-native:** structural configuration and configured-node identity, Rust
diagnostic wording where no discriminator freezes text, and internal query edge
representation.

**Unsupported/deferred:** subrule/rule invocation with resolved late-bound
values; kwargs override rejection; evaluator-visible `ConfiguredTarget`,
`Artifact`, and `FilesToRunProvider` values; subrule context, fragments,
toolchains and actions; aspect subrules; XML/query presentation; absolute-path
FDO artifacts and the enabling command flag; every non-`//`
`fdo_optimize` spelling; configured hidden-edge/result publication; broader
fragments/fields; exact Bazel configuration/output bytes; and rules_cc/C++
semantics. `cc_common`/`cc_internal` remain generic BCR Starlark consumers.

## Shared typed owner

`slug_configuration_v2` owns one closed `ConfigurationField` identity. Its Cpp
variant is a one-byte enum for the ten names; tools-repository identity remains
part of the field identity because it affects `zipper`. Loading parses
`configuration_field()` directly to this shared type and retains it in the
existing sparse ordinary/lifted rows. Delete the loading-owned string allowlist;
do not add a registry, raw native lookup/mutator, or parallel option store.

`SlugConfiguration::configuration_field_label` borrows the sole structural
native option vector. Direct label fields project existing canonical labels;
`fdo_optimize` projects only a valid raw `//` label. Every other non-null
spelling fails configuration-field projection because Bazel's still-default
`enable_fdo_profile_absolute_path=false` rejects it during
`CppConfiguration` construction. `xbinary_fdo`, `propeller_optimize`, and
`zipper` apply the pinned suppressor/derivation rules only after that global
validity check. The same check rejects simultaneous `fdo_optimize`/
`fdo_profile` and `fdo_instrument` with either optimization source, matching
`CppConfiguration.reportInvalidOptions`; the prerequisite's descriptor-owned
implicit `copt` already satisfies its remaining invariant. Projection is pure
phase scratch and does not change configuration identity.

## Loading and query projection

Expose one borrowed iterator over ordinary and lifted configured dependency
descriptors. It carries synthesized name, user name where applicable, kind,
default, file/single-file/executable/Exec flags, and canonical provider
alternatives. It borrows the existing package-owned rows; add no retained copy.

The root loading-query graph appends literal hidden rows in lifted order, with
deterministic synthesized names, `explicit = false`, and an `Implicit` edge
kind. Configuration-field rows have no unconfigured label and remain absent
from loading edges. Existing traversal includes implicit edges; same-package
source derivation must include them. The existing bounded external loading-query
route remains deferred; do not weaken its validation or add a fallback to claim
external unconfigured query parity.

## Configured dependency pipeline

Prepare ordinary configured attributes and the borrowed hidden/late-bound
descriptors before child computation. Resolve literal labels directly and
typed defaults from the owner's structural `SlugConfiguration`. Preserve row
and label-list order in phase-scratch dependency records.

Resolve the existing toolchain/execution-platform owner before making any
`cfg="exec"` child key, including rules requesting zero toolchain types. Target
rows use the parent configuration; Exec rows use
`to_exec_for_platform(selected_actual_label)`. Never select host/default state
independently. Feed both row kinds through the existing configured child key,
observed Need union, cycle guard, requested/actual alias, and publication path.
The generic child-preparation owner must normalize declared source-file labels,
including exported files and cross-package sources, to the existing null/source
key before computation; generated-file labels retain configured identity. This
is a general dependency correction, not a subrule-specific lookup.

Retain whether a scratch row is hidden and whether its configuration is Exec.
Hidden rows never enter `PreparedDependency`/`ctx.attr`. Do not add a
`ConfiguredEdgeKind` yet: Slug publishes edges only in a successful
`ConfiguredNodeResult`, and the mandatory pre-call terminal makes success
impossible in this packet. The direct-call successor must publish the ordered
implicit edges and set the tool bit exactly for Exec-transition rows, following
Bazel `Attribute.isToolDependency()`/`ExecutionTransitionFactory.isTool()`.

After every child is available, validate provider alternatives as OR-of-AND;
an empty alternatives slice is unrestricted and every label-list element is
checked. Source/generated configured-node kinds carry Bazel's inherent
`DefaultInfo` capability for predicate checks without allocating or publishing
a provider wrapper. For single-file rows, a source/generated node contributes
its one label-owned artifact; a rule/alias flattens its real
`DefaultInfo.files`. Accept exactly one file matching any extension constraint
and reject zero/multiple files. A direct file node is admitted only by
`allow_files` or `allow_single_file` and its extension filter; rule targets
remain admissible independently. Executable rule/alias rows require the
child's real `DefaultInfo.files_to_run.executable`. Bazel
`FileConfiguredTarget.createFilesToRunProvider` makes a source/generated file
its own executable, so an otherwise file-admitted node satisfies executable
with its label-owned artifact. All validation precedes the existing pre-call
unsupported terminal; the terminal publishes no result, edge, provider wrapper
or action.

## DICE, lifetime, and revision behavior

`ConfiguredNodeAnalysisKey` remains the sole retained computation. Loaded
descriptors and `SlugConfiguration` remain DICE-retained semantic inputs;
resolved rows, child-key batches and validation scratch die with the compute;
no new configured result is retained on the terminal error. No new key, cache,
side registry, lock, task, watcher or request carrier is allowed. Hold no lock
across a DICE compute. Same-DICE source or command A/B/A must invalidate and
restore error precedence without stale children, results or actions.

## Proof contract

Use the accepted source tests above; add no Java helper or copied oracle
fixture. Prove:

- all ten field projections, missing values, suppression/zipper, invalid
  string and mutually exclusive option states, and one-byte identity;
- borrowed retained rows and unchanged hidden invisibility;
- literal/late-bound target and real selected-platform Exec keys, including
  zero requested toolchains and requested-versus-actual alias identity;
- provider OR-of-AND/list validation, unrestricted predicates, source and
  generated single-file zero/one/many/extension cases, executable success and
  failure, and dependency-error precedence;
- loading implicit rows/edges, ordinary `_libc_top`/`_zipper`, the successful
  validation-to-pre-call terminal, no configured result/invocation/actions, and
  same-DICE A/B/A error restoration. Configured implicit/tool edge proof belongs
  to the direct-call successor because only it can produce a successful result.

Exact proof paths:

- `app/slug_configuration_v2/src/native/tests.rs`, base
  `d6815f7344a37d0dbeac5d24bebf08060197bc42`;
- `app/slug_loading_v2/tests/subrule_loading.rs`, base
  `49a375e1ab5514b6b93e01728d45f0d2ce75c95f`;
- new `app/slug_analysis_v2/tests/subrule.rs`;
- `app/slug_query_v2/src/graph.rs`, base
  `54cd452214127fe5429db46635dbe7e37f27c6e3`; and
- `app/slug_server_v2/src/tests.rs`, base
  `ee575cbe812227abd9bcaddf5a36905a65b1dae8`.

## Frozen implementation envelope

Production paths at `4425d3bfb`:

- new `app/slug_configuration_v2/src/native/configuration_field.rs`;
- `app/slug_configuration_v2/src/native/configuration.rs`,
  `046d569400b0ef0297df9fceb75309420a1628e3`; `src/native/mod.rs`,
  `328785bd1e7ac9117e0ee27eea251c69af62a176`; and `src/lib.rs`,
  `13181e449c8655b95a770e136dcf1cc12de8888d`;
- `app/slug_loading_v2/src/subrule.rs`,
  `0b2e4e4661e534ac1cd6279aff9e44526aa2a76a`; `src/lib.rs`,
  `d7da3a9b82ebcaea26327285bf65f0407ea7f646`; and only the
  fail-closed/accessor hunk of dirty `src/package.rs` (HEAD
  `191b2082de14e5f057d8183c1c156671bd4cbd2a`, admitted worktree base
  `bee28ca831f3d71ab3b8f1a29fab1559c4ce299f`);
- new `app/slug_analysis_v2/src/subrule.rs`, only exact orchestration hunks of
  dirty `src/dice.rs` (HEAD
  `70d59f60b3f4b06702eb347e0b615c6961e912d1`, admitted worktree base
  `2718df1109a4a543f5ac57c99b8c52aed52c8de3`), and the module/export
  hunk of dirty `src/lib.rs` (HEAD
  `f1144f085c47babc9d848d5aca662d496c500e2b`, worktree
  `b1d7e9acccd6e5fe87bdbdc2f0372be3cc9d6758`); and
- `app/slug_query_v2/src/graph.rs`,
  `54cd452214127fe5429db46635dbe7e37f27c6e3`.

Every other dirty hunk and file is excluded. Caps: 1,250 production additions,
1,200 proof additions, 2,450 aggregate additions, no new production function
above 140 lines, and no new retained descriptor copy. The touched `package.rs`,
`dice.rs`, and `graph.rs` exceed 2,000 lines; keep only orchestration/accessor
hunks there and place cohesive new logic in the named modules. No benchmark is
required because the packet adds no cache and no demonstrated hot-path
regression exists.

## Validation and stops

Run formatting and `git diff --check`; focused configuration/loading/analysis/
query/server tests; full serial tests for all five affected crates; named
build/cquery/query dependents; four-crate checks; retained-size and cap audits;
forbidden-surface scans; base/worktree-blob isolation; archive checker; and an
index-only repeat containing only packet hunks. Clean stale `slugd` around
daemon tests. Rebuild `slug_cli_v2` before any `SLUG_V2_BIN` replay.

Independent plan and terminal implementation review are mandatory. `REPLAN`
for invocation/value materialization/override handling; XML/aspects; a new key,
pipeline, cache, registry or raw native mutator; host/default Exec fallback;
C++-specific dependency code; partial publication; inability to distinguish
source/generated file cardinality or executable facts; evaluator-heap retention;
lock across compute; dirty-hunk overlap; unlisted files/cap overflow; Java; or
an exact claim not proved against Bazel 9.2.

## Immediate predecessor and successor

Commit `4425d3bfb` accepted the lawful thirteen-option CLI-to-DICE producer
after full validation and terminal review. R3 plan review returned `REPLAN`:
an error cannot publish configured edges, files can be executable, the
`fdo_optimize` grammar was too broad, and the query anchor/blob ledger was not
mechanically exact. R4 makes only those corrections. After this packet,
activate only the direct-call successor that materializes evaluator values,
invocation and successful configured edges while preserving this packet's
dependency owner and pre-call validation.
