# Current Slug V2 Work Packet

Packet: WP-7-82-m7a-external-starlark-package-edges-r1
Status: accepted (independent design and final ACCEPT)

## Outcome and compatibility

Admit the modeled Loading and unconfigured-query graph of ordinary non-test,
non-executable Starlark rules in a multi-target external repository package.
The bounded M7A demand is the genuine `@@bazel_tools//tools/cpp/BUILD`: its
`empty_lib` inherits public package visibility, `link_extra_lib` has a real
ordinary `deps` edge to `:link_extra_libs`, and both coexist with native flags,
filegroups, aliases and toolchain types. WP781 installed its exact BUILD source;
this packet removes the generic post-evaluation policy rejection, not the
separate recursive `rules_cc`/generated-proxy source prerequisite.

Exact: accepted Bazel 9.2 Loading semantics for effective public visibility,
stable target declaration order, ordinary typed label dependencies and
unconfigured query edges/attributes on the selected external Starlark-rule
shape. Slug-native: existing structurally compared DICE source/mapping/load
identity and Host observations, not HotSpot or Bazel checksum bytes.
Unsupported/deferred: private/restricted external Starlark rules, test and
executable rule capabilities, unmodeled label-reachable attributes, complete
authentic `tools/cpp` Loading, configured `cc_library`, module-tool binaries,
actions/REAPI and M8. No stub `cc_library`, trimmed compatibility proxy or
post-hoc command graph repair may claim the deferred surface.

Pinned Bazel 9.2 `tools/cpp/BUILD.tools:14-95` owns the demanded declaration
shape. `PackageFactoryTest.testImplicitVisibility_worksWithPackageDefaultVisibility`
and `AbstractQueryTest.testAttrOnPackageDefaultVisibility`, `testDeps` and
`testNoImplicitDeps` provide source-pinned visibility/query anchors. The
already accepted `rustc-map-each` source inventory authenticates selected
`rules_cc` proxy/helper children; it does not prove the complete eager public
`cc_library.bzl` wrapper closure. Its generated proxy loads 14 private rules
eagerly. Existing canonical built-in load tests prove route/mapping selection,
and the accepted one-rule external policy/query tests provide the base slice.

## Semantic owners and invariants

`RepositoryPackageInventoryKey` remains the producer of a complete
`LoadedPackage`: exact BUILD and recursive `.bzl` source dependencies, observed
epochs, default visibility, ordered targets, retained schemas/values, ordinary
dependency labels, module lifetimes and load fingerprint. Its existing Need,
error, event and cancellation behavior remains unchanged. The public
`RepositoryPackageLoadKey` is a pure policy projection of that inventory; it
must validate **every** Starlark target before publishing success, not only
the first one. Use `LoadedPackage::effective_visibility` so declared public and
package-default public are equivalent at the permission boundary; keep the
source provenance distinct. For each rule, check capability and schema/value
integrity. Admit user and supported builtin ordinary dependency labels through
their retained typed schema. Preserve the existing builtin NODEP handling for
`visibility` and `transitive_configs`; a nonempty label in an unsupported
nonordinary user or hidden attribute still fails closed. Keep unsupported
target kinds and malformed schemas typed errors. A nonempty builtin
`$config_dependencies` remains deferred in this packet: it contains selector
keys in the retained aggregate but cannot be silently emitted as an ordinary
query edge. Preserve the existing exact root-repository selector slice.

`PackageEvaluation` already derives and deduplicates ordinary labels from
retained typed attribute values; do not add a second graph registry. The
existing `ExternalUnconfiguredPackageGraphKey` owns the DICE-retained semantic
package graph projection of those ordered labels and supported literal
hidden/visibility edges, with canonical repository binding and apparent-label
display kept distinct. Explicit canonical-main `@@//...` labels must remain
main-repository labels; package-relative `:...` labels resolve against the
external owner. Do not blindly rebind every root label in attribute or edge
projection. Query command result sets remain request-local. `deps`, `rdeps`,
`labels`, path queries and `attr` must not observe a missing edge, a synthetic
dependency or a wrong edge class. The CLI `--noimplicit_deps` option remains
unsupported; preserve its existing rejection, while retaining typed edge
classes internally. Source-file synthesis is allowed only under the existing
same-package rule; cross-package
traversal must use its tracked package route or remain a typed deferred error.
Named-repository edges without an observed apparent mapping fail closed rather
than displaying a guessed repository spelling. The Query validator must also
inspect every Starlark rule, so direct graph
construction cannot bypass the Loading policy.

No new DICE key, lock, cache, interner or long-lived side store is selected.
The inventory/module values and package graphs remain DICE-retained under
their existing keys; only command result sets are request-local. Changes to
BUILD bytes, repository mapping, default visibility, any dependency value or a second
target's validity must invalidate/recompute through existing dependencies and
restore on A/B/A. No lock may span a DICE compute. See `docs/developers/dice.md`.

## Scope, routing and stops

Base `03c9910fe`; branch `codex/wp782-external-starlark-edges-review`.
Worker1 owns `app/slug_loading_v2/src/bzl_module.rs` and the relevant section
of `app/slug_loading_v2/src/host_package_load_tests.rs`. Worker2 owns
`app/slug_query_v2/src/graph.rs` and
`app/slug_query_v2/tests/loading_query.rs`. Root owns the packet/canonical
docs, integration, serial Cargo, and final commit. Independent reviewer checks
the design before edits and the actual final diff/evidence. Existing large
owner/test modules are retained for small policy and scaffold-local edits;
splitting their DICE/epoch harness here would duplicate ownership. Review
triggers: over 180 Rust production lines, over 300 new test lines, or any new
public retained type. These are estimates, not caps.

Do not edit `@bazel_tools` source bytes, generated proxy content, repository
materialization, parser/coercion, configured Analysis, binary acquisition or
action execution. Reuse existing source-authenticated in-memory tests; no new
Stage 1 fixture is selected. A later packet must authenticate and evaluate the
public `rules_cc` wrapper, complete eager proxy descendants and their
repository materializations before claiming authentic `tools/cpp` Loading.
REPLAN if modeled ordinary labels cannot be projected without a new semantic
owner, if a nonempty hidden label would be silently omitted or misclassified,
or if the accepted external package route cannot preserve observed Need and
atomic multi-target validation.

## Acceptance

Loading: a multi-target external BUILD with a declared-public and a
package-default-public non-executable rule, a sibling label flag and an
ordinary `deps` list retains source-backed labels and stable order. A private
rule, test/executable rule, malformed schema or unsupported nonordinary label in **any**
target still rejects the whole public package; inventory may retain the
unvalidated declarations. Demonstrate missing/edited/restored BUILD or `.bzl`
source, mapping and default-visibility changes in the same DICE graph,
including Need/error and event-batch ownership without partial publication.

Query: the same loaded package exposes ordered ordinary edges, label-attribute
values and effective public visibility in `deps`, reverse/path, `labels`, and
`attr` queries and at least one admitted output format. Verify distinct internal
implicit versus ordinary edge kinds, `:sibling` versus explicit `@@//main:target`
identity, apparent/canonical display, same-package source synthesis and a
negative unmodeled or unmapped edge. Preserve the accepted dependency-free
external rule and native target cases. The genuine `modules_tools/BUILD` can be a pinned
source smoke for unchanged built-in package routing, but missing binary leaves
are not success fixtures. A `tools/cpp` query still requires the unadmitted
authentic proxy closure and must not be recorded as a passed gate.
An admitted nonempty literal hidden default must appear as an `Implicit` edge
and attribute with the selected external apparent display; otherwise reject
it before semantic graph publication. A nonempty `$config_dependencies` must
produce the packet's typed deferred result, not an ordinary query edge.

Resolve pinned nightly tools, prepare Loading/Query Rust tests serially with
offline/locked Cargo JSON `--no-run` within 60 seconds each, preflight exact
nonignored selectors and execute counted focused tests. Check direct
`slug_analysis_v2` and `slug_core_v2` compile dependents; use a fresh binary
build before any Slug CLI smoke. Run changed-file rustfmt, `git diff --check`,
archive/plan-status checks and independent final review. Every exact claim
must map to the pinned Bazel source tests or existing accepted oracle evidence.

Independent design review ACCEPT confirms the bounded effective-public,
all-target policy and existing DICE owners; it specifically requires the
ordinary/NODEP/hidden/selector classification and explicit-main versus
external-relative label discriminator before final acceptance.

## Acceptance receipt

The public Loading projection validates every retained Starlark target before
publication. Declared and package-default public rules coexist with native
targets and preserve ordered typed ordinary dependencies. Query projects those
dependencies, builtin `transitive_configs` NODEP labels, visibility NODEP
labels and supported hidden literal Implicit labels through the existing
DICE-retained package graph. Explicit canonical-main and external-relative
labels retain distinct identities; unobserved named-repository mappings,
selector keys and configuration-dependent hidden defaults fail closed. Same-
DICE dependency edits invalidate and restore the Query graph on A/B/A.

Pinned-nightly offline/locked Cargo `--no-run` preparations succeeded for
Loading and Query. Exact preflight and execution passed 9 focused Loading and
9 focused Query tests. Direct `slug_analysis_v2` and `slug_core_v2` checks,
changed-file rustfmt, diff, plan-status and archive-status checks passed.
Independent final review ACCEPT audited the final diff and the bounded claim.
An optional full Loading run had 664 passed, 5 failed and 1 ignored; a clean
`03c9910fe` worktree reproduced the same five failures (661 passed, 5 failed,
1 ignored). An optional full Query integration run had one malformed-label
diagnostic assertion failure among 64 tests; that exact failure also
reproduced on the clean base. These preexisting failures are not counted as
passing gates. Complete authentic `tools/cpp` Loading remains deferred pending
the eager `rules_cc`/generated-proxy closure; no Bazel or remote-transport
execution claim is made by this packet.
