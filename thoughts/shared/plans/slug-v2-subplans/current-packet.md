# Current Slug V2 Work Packet

Packet: WP-7-84-m7a-nonvisible-alias-query-r2
Status: accepted (independent r2 design and final ACCEPT; r1 REPLAN retained)

## Outcome and compatibility

Load the authentic `@@bazel_tools//tools/cpp` package without resolving the
unselected `:toolchain` alias's nonvisible `@local_config_cc` repository, then
make root-visible Query distinguish an independent target, the alias itself,
and traversal of that alias. Bazel 9.2 on the current root returns
`@bazel_tools//tools/cpp:empty_lib` and `:toolchain_type` for
`deps(@bazel_tools//tools/cpp:empty_lib, 1)` (exit 0), returns `alias rule
@bazel_tools//tools/cpp:toolchain` for its `label_kind` query (exit 0), and
fails `deps(@bazel_tools//tools/cpp:toolchain, 1)` with the nonvisible
`local_config_cc` repository requested from `@@bazel_tools` (exit 7). The
WP783 rebuilt Slug CLI instead stops while evaluating BUILD:119. These three
fresh, discriminating Bazel observations are the packet oracle; no cache
path, user RC content, or authentication value is evidence.

Exact: Bazel 9.2's separation of package/alias declaration from traversal of
a nonvisible literal `alias.actual`, the three named Query outcomes, visible
same-package alias behavior already admitted, same-repository cross-package
filegroup references, direct-package restricted visibility needed by this
BUILD, and default Starlark rule-class toolchain requirements with an admitted
same-repository Query route as implicit dependencies. Slug-native: structural
retained identity, Host observed epochs and
DICE equality rather than Java object bytes. Unsupported/deferred: general
nonvisible `attr.label`/selector values, visible cross-repository alias
traversal, unrelated cross-repository filegroup/visibility cases, configured
toolchain resolution, foreign-root rule-class toolchain route projection,
declared execution-group toolchains, the CLI
`--noimplicit_deps` selector, configured C++ rules, module-tool binaries, execution/REAPI
and M8. A typed deferred
edge must not be silently omitted or recast as a visible canonical repository.

Pinned Bazel 9.2 `RepositoryMapping.java:133-139` creates a nonvisible
repository name when an apparent name is absent; `Label.java:234-245` retains
it in the label and `Alias.java:74-78` accepts the LABEL `actual`. The embedded
`src/MODULE.tools` has no `use_repo(local_config_cc)`; its `tools/cpp/BUILD.tools`
declares the alias at lines 119-122. The rules_cc 0.2.17 extension imports
`local_config_cc` into rules_cc's mapping, not bazel_tools's mapping. WP781's
catalog bytes and WP783's canonical source route remain exact prerequisites.
Pinned Bazel 9.2 `LabelVisitationUtils.java:30-37,59-74,105-112` treats a
rule class's default toolchain types as synthetic implicit label edges;
`SkyQueryEnvironment.java:236-239,543-553` includes them by default and
filters them under `--noimplicit_deps`. Bazel 9.2's fresh
`deps(:empty_lib,1) --noimplicit_deps` emits only `:empty_lib`, while default
Query also emits `:toolchain_type`. The pinned built-in `cc_library.bzl:954-956`
declares `cc_helper.use_cpp_toolchain()`, sourced from `cc_helper.bzl:47,98`.

## Owner, decision and invariants

Use the existing `ResolvedOptionLabel` nonvisible repository representation
as a V2-owned typed value, with a borrowed package-context mapping parser;
do not invent a `CanonicalRepoName`, copy the selected mapping per alias, or
change global label coercion. Only a literal scalar string `alias.actual` in
a complete Host package can retain this nonvisible value. Add a compact
`AliasActual::{Visible(CoercedAttributeValue), NonVisible(Arc<ResolvedOptionLabel>)}`
to the existing `PackageTargetKind::Alias`; visible values, selectors, Label
objects, Legacy packages and all other attributes retain their current
paths. `PackageRecorder`/`RepositoryPackageLoadKey` own the declaration and
its complete repository mapping. The native-attribute row cannot encode the
nonvisible value: omit that row only for the nonvisible alias while retaining
the typed target, and make every consumer explicitly handle or reject it.

`ExternalUnconfiguredPackageGraphKey` owns Query's graph. Its retained
per-alias deferred marker distinguishes a nonvisible actual from an otherwise
visible but not-yet-admitted external alias; place it in an alias-specific
node variant, not an `Option` added to every Query node. Direct lookup and
`label_kind` may inspect the alias rule; dependency/attribute-edge consumers
must error before returning an incomplete result. Preserve the existing exact
same-package visible alias projection and destination checks. Admit the
same-repository cross-package filegroup source edges and direct-package
restricted visibility present in `tools/cpp/BUILD` using their existing
typed Query/visibility owners. This admits source-edge retention only, not
cross-package traversal of a selected filegroup, until separately proved. Do
not globally loosen cross-repository or package-group validation to reach
`empty_lib`.

The r2 selected-rule correction belongs to the same Query graph owner.
`StarlarkRuleImplementation.required_toolchains()` already retains the
default rule-class requirements in Loading and package equality. Project
those source-derived labels after attribute edges as a distinct internal
`QueryEdgeKind::RuleToolchain`, an implicit-class edge consumed by default
`deps`/`same_pkg_direct_rdeps` but not a fabricated native attribute. Do not
project declared execution-group toolchains. In root projection, admit only
main-repository requirement labels through `QueryLabel::from_canonical`;
fail closed on a foreign repository because a canonical label alone has no
observed root apparent route for Query traversal. In external projection,
retain the observed apparent owner for same-repository labels and fail closed
if a foreign repository has no observed apparent mapping. The distinct edge kind
must be excluded from root and external attribute-created source-node
synthesis, so a missing toolchain target fails on traversal instead of
becoming a synthetic source file. No new DICE key, Loading production
owner, configured resolver or toolchain instance is needed.

The identity and package target values are DICE-retained; the graph marker
participates in graph equality. Mapping missing/visible/missing A/B/A,
source and route changes invalidate through existing package/graph keys;
Need and observed-frontier errors propagate before graph publication. No
new DICE key, lock, registry, interner, process cache, materializer or event
owner is chosen. Command result sets remain request-local. No shared lock
spans a DICE compute (`docs/developers/dice.md`). Reuse `Arc` and
`Allocative`, and avoid a per-node memory tax or donor import under the
Stage 9 proposed label-utility row and accepted borrowed parser pattern;
there is no performance claim.

## Scope and stops

Base `9ca39910f`; review branch `codex/wp784-nonvisible-alias-review`.
Identity worker owns only `app/slug_identity_v2/src/{label,repo_mapping}.rs`
and `app/slug_identity_v2/tests/label_roundtrip.rs`. Loading worker owns only
`app/slug_loading_v2/src/{package,lib}.rs` and
`app/slug_loading_v2/src/host_package_load_tests.rs` and the existing
`app/slug_loading_v2/tests/build_file_loading.rs` alias-construction test
owner plus `app/slug_loading_v2/src/canonical_repository_load_route_tests.rs`
(reviewed compile fallout of changing the retained alias field).
Query worker owns only
`app/slug_query_v2/src/{graph,loading_environment}.rs` and
`app/slug_query_v2/tests/loading_query.rs`. Root owns these plan files,
integration, the smallest needed explicit Analysis consumer guards in
`app/slug_analysis_v2/src/{build_setting,dice}.rs`, serial Cargo/CLI/Bazel
evidence and commit. If additional consumers require edits, amend the
allowlist after review; do not use a generic fallback. Independent design
and final review are required for retained identity and public Query graph
semantics. Large `package.rs`, `graph.rs` and Query integration tests keep
their existing cohesive owners; additions over 180 production or 300 test
lines per worker trigger a bounded split/review, not an automatic cap.

The r2 amendment adds only the existing Query worker's `graph.rs` and
`loading_query.rs` scope. The independently reviewed r1 implementation remains
uncommitted on this branch. The new edge kind and root/external route handling
received an independent r2 design ACCEPT before production edits.

The oracle fixture checkpoint is due: since reset `3ac0a85b`, 105 new
physical fixture files and 31 command rows exist. A read-only review found
no demonstrably removable files or rows: repeated registry scaffolding is
fixture-isolated, while distinct markers/licenses have distinct paths. Reuse
the authentic Bazel observations above first. A portable synthetic fixture
may add only the minimal MODULE/BUILD/manifest/expected files after design
review and pinned Bazel 9.2 generation; no copied registry or archive tree.

Do not edit embedded Bazel bytes, rules_cc/cc_configure sources, root MODULE
or lockfile, generic repository mappings, `CanonicalRepoName`, configured
Analysis semantics, action execution, parser-wide coercion or REAPI. Stop and
REPLAN if the alias-only retained type cannot preserve mapping owner and
diagnostic identity, if a consumer silently drops a dependency, if Query
needs a new cross-repository route/identity owner, or if authentic
`tools/cpp` has further independent whole-package blockers beyond the named
alias/filegroup/visibility/rule-class-toolchain families. A typed selected-target unsupported
terminal is preferable to invented semantic visibility.

## Acceptance

An unselected nonvisible alias does not fail BUILD package Loading. Its
retained actual distinguishes apparent name, canonical requesting owner,
package/target, suggestion and mapping changes; missing/visible/missing
A/B/A restores the original result. Visible aliases and selectors keep their
existing source/order/edge behavior; Legacy and other label attributes remain
fail-closed. No native attr value or canonical label is fabricated for the
nonvisible actual. Query graph publication retains a typed marker, and all
accepted edge/attribute consumers either traverse exact edges or return a
typed deferred/visibility failure—none publish a partial success.

On a freshly rebuilt `slug_cli_v2`, compare the three authentic Bazel 9.2
queries above: `deps(:empty_lib,1)` returns exactly `:empty_lib` and
`:toolchain_type`; `label_kind(:toolchain)` returns `alias rule`; and
`deps(:toolchain,1)` fails because `@local_config_cc` is not visible from
`@@bazel_tools`, without attempting to fetch or generate that repository.
Focused synthetic owner tests distinguish same-package visible alias,
visible external deferred alias, same-repository cross-package filegroup,
direct-package visibility, nonvisible selected alias, Query A/B/A, and
unrelated target selection in the same package. For the nonvisible alias,
`same_pkg_direct_rdeps` and `labels(actual, ...)`/`attr(actual, ...)` must
return a typed failure rather than bypassing the deferred marker; direct
lookup and `label_kind` must still succeed. Check direct Loading/Query/Analysis/Core
dependents and inherited DICE Need/event behavior; preserve accepted exact
slices.
The r2 tests must show default rule-class toolchain edges in admitted root
same-repository and external same-owner graph projection, their implicit-class ordering after attribute
edges, no invented attribute or source target for a missing toolchain type,
and a typed missing-target traversal failure. A root rule-class requirement
on a foreign repository must fail closed rather than publish an unusable
canonical-only edge. Declared exec-group requirements
remain separate. Do not treat the Bazel `--noimplicit_deps` discriminator as
an admitted Slug CLI flag.

Resolve the pinned nightly before Rust validation. Use offline/locked Cargo
JSON `--no-run` preparation under 60 seconds, exact nonignored preflight and
counted execution; rebuild the CLI before any binary comparison, and clean
stale `slugd` before/after. Run changed-file rustfmt, plan/archive and diff
checks, then independent final review. Bazel commands may use ordinary RC
discovery; never inspect or print `~/.bazelrc` or auth material. NativeLink
and BuildBuddy real-transport tests remain outside this Loading/Query packet.

WP783 is the immediate predecessor at `9ca39910f`: ordinary built-in source
routing accepted with 7/7 route and catalog/Query/event-owner regressions,
direct Analysis/Core checks and independent design/final review. Its fresh
CLI reached the genuine BUILD:119 nonvisible alias frontier.

## REPLAN evidence (2026-09-23)

The independent design review accepted the original bounded alias,
filegroup-source, and direct-package-visibility scope. Identity parser tests
(2/2), focused Loading tests (2/2), Analysis check, Query graph and observed
A/B/A tests passed. A fresh `cargo build -p slug_cli_v2` then showed
`label_kind(@bazel_tools//tools/cpp:toolchain)` and
`deps(@bazel_tools//tools/cpp:toolchain, 1)` matching the Bazel 9.2 oracle,
including the latter's nonvisible repository error. However,
`deps(@bazel_tools//tools/cpp:empty_lib, 1)` emitted only `:empty_lib`, whereas
Bazel also emits `:toolchain_type`. The authentic `:empty_lib` is a `cc_library`
whose rule declaration requests the C++ toolchain. This selected-rule
toolchain edge is a new family outside the packet's three approved projection
gates, so the packet's exact three-query outcome is **not accepted**. Preserve
the uncommitted reviewed implementation while the Query toolchain-edge owner
and a revised compatibility boundary receive independent design review; do
not hard-code an `empty_lib` edge or claim Query parity. The r2 amendment above
implements the source-derived retained-toolchain projection, independently
reviewed and accepted after the authentic CLI check.

## r2 validation receipt (2026-09-23)

The revised Query projection passed no-run compilation, focused root and
external toolchain tests (three tests), the earlier graph/deferred-alias and
observed A/B/A tests, Query lib tests (59/59), and the loading_query integration
target (68/68 with one inherited baseline failure excluded). The excluded
`invalid_relative_package_target_fails_in_loading_label_conversion` assertion
expects an `invalid label 'raw':` prefix; the unchanged custom-rule attribute
path has emitted the bare parser message since `5f9f9a98ad`, after the older
test was written in `40ac1cd2ec`. WP784's new parser is alias-only and does
not touch that path. Do not weaken or silently mark this inherited test pass;
its four expected diagnostics need a separate reviewed correction.
Identity borrowed-parser tests passed 2/2, focused Loading tests passed 2/2,
the Loading external test target compiled, `slug_analysis_v2` checked, and
the changed `slug_cli_v2` binary built successfully.
Two existing visible-alias Loading regressions passed. After a final-review
correction made deferred-alias attribute reads fail closed for every attribute
name, Query lib still passed 59/59 and integration passed 68/68 with the same
one inherited baseline failure excluded; the corrected focused regression
passed 1/1. Independent final review returned ACCEPT.
Post-review exact nonignored executable preflight verified the selected Query
integration (68), Query lib (59), identity (2), Loading lib (2), and visible
Loading integration (2) tests; unchanged executables then reran with those
same pass counts. Packet elapsed time was approximately 44 minutes from the
20:04 UTC design draft to the 20:48 UTC acceptance close, including the r1
REPLAN/r2 design cycle. Compile and review phases were not separately timed.

A freshly rebuilt `slug_cli_v2` now returns exactly `:empty_lib` plus
`:toolchain_type` for `deps(@bazel_tools//tools/cpp:empty_lib, 1)` (exit 0),
returns `alias rule` for `@bazel_tools//tools/cpp:toolchain` label-kind (exit
0), and returns the nonvisible `local_config_cc` requested-from-`@@bazel_tools`
error for `deps(@bazel_tools//tools/cpp:toolchain, 1)` (exit 7). These match
the named Bazel 9.2 oracle outcomes. `slugd` was absent before and after the
CLI smokes. NativeLink and BuildBuddy were not needed for this packet.
