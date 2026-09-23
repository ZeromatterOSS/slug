# Current Slug V2 Work Packet

Packet: WP-7-86-m7a-external-filegroup-query-r1
Status: accepted; independent final ACCEPT

## Outcome and compatibility

Make ordinary root-visible Query traverse the authentic Bazel 9.2 built-in
filegroup chain `@bazel_tools//tools/cpp:aggregate-ddi` →
`@bazel_tools//tools/cpp/modules_tools:aggregate-ddi-bin`, and the analogous
`generate-modmap` chain. The child `srcs = select(...)` must expose both
source arms and its `:windows` condition to unconfigured `deps`, while
`labels(srcs, child)` exposes only the two source arms. This is a finite
preparation for the configured module-tool dependencies named in
[bootstrap readiness](./bootstrap-readiness.md); it does **not** close that
M7A row or claim the module binaries and actions are available.

Exact: the named Bazel 9.2 Query labels and ordering for the two authentic
same-repository cross-package filegroup chains, all-branch `srcs`
dependencies and nondefault selector-key dependency, and label-attribute
separation for those native filegroups; the observed same-package missing
selector-key source and cross-package missing-destination negatives, including
a cross-package selector key. Slug-native: immutable graph and
repository-route identity, Host-observed DICE invalidation and publication
equality; other source-derived same-repository traversals have only their
existing typed Slug-native graph coverage, with exact parity unclaimed.
Unsupported/deferred: foreign-repository filegroup `srcs` and selector keys, a
general `--noimplicit_deps` Slug flag, configured C++/module-tool providers and selected
binary content, execution, REAPI, Bazel ActionKeys and exact output paths.
Do not infer general Query or configured-graph parity from this slice.

## Evidence and source basis

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
owns the exact declarations at `tools/cpp/BUILD.tools:83-90` and
`tools/cpp/modules_tools/BUILD.tools:3-22`; the checked-in Slug catalog
contains those BUILD bytes verbatim. `Rule.java:453-457` invokes
`AggregatingAttributeMapper.visitLabels`; its `:95-107,182-219` visit
includes nondefault selector keys and all branch values for `deps`.
`BlazeTargetAccessor.java:67-84` uses `getReachableLabels(attr, false)`
for `labels(srcs)`, excluding selector keys. The internal Query edge kind
is not a claim about Bazel's `--noimplicit_deps`: a fresh control confirmed
that Bazel retains `:windows` even under that flag.

Fresh Bazel 9.2 probes in an isolated copy of the existing
`rules-cc-basic` workspace returned exit 0. `deps(aggregate-ddi, 2)`
emitted, in order: parent, `modules_tools:aggregate-ddi`,
`modules_tools:aggregate-ddi-bin`, `modules_tools:aggregate-ddi.exe`,
`modules_tools:windows`. `deps(generate-modmap, 2)` emitted the analogous
five labels. `labels(srcs, aggregate-ddi-bin)` emitted only
`aggregate-ddi` and `aggregate-ddi.exe`; `deps(aggregate-ddi-bin, 1)`
also emitted `:windows`. `same_pkg_direct_rdeps(:windows)` emitted both
child filegroups. The Linux `cquery` selected the default `aggregate-ddi`
source and omitted `.exe`; that configured observation is a discriminator,
not this Query packet's admission. Ordinary Bazel RC discovery was used;
no RC contents or authentication bytes were inspected or copied.

A fresh Bazel 9.2 negative probe also found that
`deps(//:missing_selector, 1)` includes undeclared same-package
`:missing_condition` as a **source file**, with or without
`--noimplicit_deps`; `labels(srcs, //:missing_selector)` excludes it.
`deps(//:cross_missing, 1)` instead errors for an absent target in a
different package, and a declared `package_group` in filegroup `srcs` is a
valid Query dependency. Pinned `Package.java:873-955` synthesizes undeclared
same-package input files from rule labels; `TargetRecorder.java:230-231`
delegates to `Rule.getLabels`. These outcomes supersede the earlier
no-synthesis design assumption. The exact negative for this packet is the
cross-package missing destination; an unsupported wrong-kind guard is
Slug-native, not a Bazel parity claim.

A further isolated Bazel 9.2 probe with
`srcs = select({"//other:absent": ["local.txt"],
"//conditions:default": []})` and an existing `other` package returned
exit 0 for direct target lookup and `labels(srcs, target)` (only
`//:local.txt`), but exit 7 for `deps(target, 1)` with a missing
`//other:absent` target. This distinguishes selected cross-package
selector traversal from package loading and label-attribute projection.

## Owner and decision

Loading already owns the complete native filegroup fact: flattened branch
labels in `PackageTargetKind::Filegroup.srcs`, the original
`CoercedAttributeValue::Selector` and `$config_dependencies` in native
attributes (`package.rs:1547-1597`). Do not reparse BUILD text or infer a
condition from source filenames. The existing root and external
unconfigured package-graph DICE keys own the immutable Query nodes and
edges; external destination lookup uses the existing verified repository
route and independently loaded destination package. Query result sets and
traversal worklists remain request-local. Do not add a key, lock, cache,
interner, cross-package fallback source node in the caller package, or ambient
filesystem read. Preserve Need/error propagation, source observation and A/B/A
restoration through the existing graph/route dependencies.

Project selector keys from the Loading-owned native attribute into a
dedicated `QueryEdgeKind::SelectorCondition` after the authored `srcs` arms.
Both root and external same-package graph synthesis must treat an undeclared
condition label as an input source file, matching Bazel's package loading;
an existing declared condition resolves to its actual node. The separate
kind preserves key provenance and leaves `labels(srcs)` with only authored
arm values. Bazel's `--noimplicit_deps` still includes the key, so do not
equate this kind with a filtered implicit dependency or claim the
unimplemented Slug CLI flag. Preserve branch/source order,
stable deduplication and the typed attribute value. Cover both root and
external native-filegroup graph producers so the same selected family has
one rule, but claim exact CLI results only for the named Bazel-observed
external commands. A selected external same-repository cross-package edge
must ask the destination package graph for the real target. Remove the
current blanket selected-filegroup refusal only for source-derived paths
whose repository route and target lookup are observed. Missing
cross-package destinations and unmodeled foreign repositories fail closed;
do not synthesize a target in the caller package for either.

No new retained struct or per-node field is authorized: the new edge-kind
variant fits the existing enum layout and uses the existing
`Arc<[QueryEdge]>`, compact labels, `SmallSet` deduplication and
`Allocative` graph owner. The extra retained edges are proportional only to
real selector keys. Scratch label vectors and traversal sets die with their
compute/request. The Stage 9 compact-utility row is reuse-only; no donor
code, memory-reduction or throughput claim. `graph.rs` is already large,
but this stays in its existing native filegroup projection and one small
shared selector helper, not a new central dispatcher.

## Scope and stops

Base `9931f6a2d` on main; review branch
`codex/wp786-external-filegroup-query-review`. Root owns scheduling,
Bazel/Slug CLI evidence, serial Cargo, integration and commit. Production
allowlist: `app/slug_query_v2/src/{graph,loading_environment}.rs`.
Focused tests may change `app/slug_query_v2/tests/loading_query.rs` and
existing `graph.rs` tests; `app/slug_core_v2/src/runtime/tests/query_command_tests.rs`
may change only for a demonstrated command integration regression or stale
direct-dependent expectation exposed by it. No
Loading/Bzlmod/catalog, analysis/configuration, transport, root MODULE,
lockfile or oracle expected-byte edits. Independent design and final review
are mandatory for public graph semantics. Avoid synthetic fixture growth:
reuse the authentic Bazel observations and existing Query test scaffolding;
add a fixture only if a demonstrated comparator gap cannot be covered by
these commands and focused tests.

Stop and REPLAN if the destination package cannot be reached through the
existing observed route, a selected child requires an unowned foreign
repository edge within the named depth-two outcome, same-package selector
source synthesis cannot be distinguished from missing cross-package target
errors, or the authentic package has a new whole-package blocker. Do not
pass by hard-coding the two target names,
skipping child lookup, inventing binaries, or widening foreign-repo support.

## Acceptance and validation

After a rebuilt `slug_cli_v2`, the five named authentic Bazel Query outcomes
above must match on Slug, including label order, source arms, condition,
`labels(srcs)` exclusion and same-package reverse dependencies. Selected
child lookup must yield real external filegroup/source/config_setting nodes,
not parent-package synthetic files. Focused root/external tests cover
same-repository cross-package present/missing and wrong-kind destinations,
selector branch/condition order and deduplication, a native literal-srcs
control, missing same-package selector condition synthesized as a source
only in `deps` and not `labels(srcs)`, a missing cross-package destination
error, foreign-repo refusal, unselected unrelated target success and
observed destination edit/delete/recreate A/B/A with Need/error recovery.
Retain WP784's nonvisible-alias and rule-toolchain Query outcomes and the
prior typed deferred behavior for unrelated unsupported shapes. A negative
without an accepted Bazel counterpart is a fail-closed Slug-native guard,
not an exact diagnostic claim.

Resolve the pinned nightly before Rust validation. Compile offline/locked
with Cargo JSON `--no-run`; preflight exact nonignored test selectors and
record counted execution for Query owner, integration and one direct Core
dependent. Rebuild the CLI before binary comparison; check for stale
`slugd` before/after. Run `cargo fmt --all -- --check`, `git diff --check`,
plan/archive status and independent final review. A broad Query suite may
be run once if its baseline failure is separately attributed. No NativeLink,
BuildBuddy or credential-bearing command is needed for this packet.

## Immediate predecessor

WP785 at `9931f6a2d` independently accepted bounded Linux `run_shell`
remote execution through NativeLink 1.7.1: one REAPI action, zero direct
local actions, exact output content/mode under Slug-native roots, with a
raw manifest-only Bazel path difference. Its FileWrite direct transport
regression passed and no backend process remained. WP784 at `8fd6e0601`
accepted the authentic `tools/cpp:empty_lib` and nonvisible alias Query
outcomes, retaining cross-package filegroup source edges but deliberately
deferring selected traversal. This packet selects that next frontier only.
The baseline rebuilt Slug CLI in the same copied workspace exits 7 for
`deps(@bazel_tools//tools/cpp:aggregate-ddi, 2)` at the named typed
cross-package filegroup guard. The first independent design review ACCEPT
was superseded by the missing-condition Bazel oracle above; independent
rereview ACCEPTED the corrected same-package synthesis rule.

## Implementation and validation receipt

Root and external native-filegroup graph producers now append distinct
selector-condition edges after authored source arms, retaining Loading's
typed `srcs` attribute. Existing same-package graph synthesis turns an
undeclared condition into a source node; cross-package labels remain edges
to real destinations. The selected external filegroup path uses the
existing repository route and `resolve_single` to verify same-repository
cross-package sources before publishing them. Foreign repositories remain
guarded. No new DICE key, lock, interner, retained field or repository
source observation was added.

Offline/locked pinned-nightly JSON `--no-run` compiled the Query library,
Query integration and direct Core dependent. Exact nonignored preflight
passed for the new graph and integration selectors and one Core selector.
Counted tests passed: 60/60 Query library, the six selected Query
integration tests, and the direct Core command test. The broad Query
integration run passed 73/74: its only failure is the pre-existing
`invalid_relative_package_target_fails_in_loading_label_conversion`
expectation for a Loading/Starlark diagnostic prefix (`invalid label`)
that current Loading omits; no changed producer owns that conversion.
The direct Core test's obsolete cross-package filegroup refusal was removed;
its selected nonvisible-alias check now uses the actual alias and observes
the stop gate before restoration. Two already-stale external BUILD-load and
glob refusal blocks were removed after independent source review confirmed
those paths are admitted and covered by focused Loading tests. The remaining
Core lifecycle and UTF-8 checks pass.

After `cargo build -p slug_cli_v2`, an isolated copy of `rules-cc-basic`
produced exit-zero Slug output matching all five named Bazel 9.2 oracle
results in order: both depth-two parent chains, the child `labels(srcs)`,
child depth-one `deps`, and `same_pkg_direct_rdeps(:windows)`. No `slugd`
remained before or after these CLI checks. Full-workspace Rust formatting,
`git diff --check` and plan status passed. No NativeLink or BuildBuddy
backend was used for this unconfigured Query packet, and no RC credential
material was inspected.

Independent final review found two further boundary obligations: canonical
root selector keys authored in an external BUILD must not be rebound to the
external repository, and same-repository cross-package selector-condition
edges must resolve a real destination before `deps(..., 1)` publishes them.
The selector helper now checks Loading's canonical repository identity
without rebinding, and selected traversal verifies both ordinary source
and selector-condition cross-package edges through the existing route.
Focused root-key refusal and missing cross-package condition tests pass;
the latter is backed by the fresh Bazel 9.2 probe above. Generic native
attribute projection was not widened. Query library, direct Core and all
five named authentic CLI outcomes were rerun after correction and retain
the results above; no `slugd` remained. Independent final rereview returned
ACCEPT for the bounded Query admission and its explicit deferred boundaries.
