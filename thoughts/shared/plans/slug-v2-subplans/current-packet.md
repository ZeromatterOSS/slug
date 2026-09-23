# Current Slug V2 Work Packet

Packet: WP-7-83-m7a-builtin-canonical-load-route-r1
Status: accepted (independent design and final ACCEPT)

## Outcome and compatibility

Make root-visible `@bazel_tools` Loading and Query use the existing canonical
immutable built-in source route. A fresh WP782 `slug` one-shot query of
`@bazel_tools//tools/cpp:empty_lib` currently stops before BUILD evaluation:
`failed to inspect direct-local MODULE ... built-in bazel_tools source requires
its immutable source owner`. The root-visible route presents the built-in as a
generic root source even though canonical Loading already owns its catalog
bytes. This packet removes that first real bootstrap frontier without claiming
the later `rules_cc` wrapper/proxy closure.

Exact: Bazel 9.2's special embedded `bazel_tools` source, its pinned catalog
bytes, canonical repository identity and root apparent spelling for admitted
Loading/Query source selection. Slug-native: existing Host observation epochs
and structural DICE route equality. Unsupported/deferred: authentic complete
`tools/cpp` package evaluation, the 96-file/320-import recursive `rules_cc`
0.2.17/generated compatibility-proxy source graph, configured C++ rules,
module-tool binaries, execution/REAPI and M8. A typed child-source Need/error
is progress evidence, not a successful package-load claim.

Bazel 9.2 `NonRegistryOverride.java:40-53` treats `bazel_tools` as an embedded
sentinel whose ordinary repository spec must not be inspected;
`ModuleFileFunction.java:969` selects the built-in module. The pinned
`src/create_embedded_tools.py:46,67-71` installs `tools/cpp/BUILD.tools` as
`tools/cpp/BUILD`; WP781 verifies exact catalog bytes and markers. WP782
accepts external ordinary Starlark edges. A source-authenticated audit of the
0.2.17 archive/proxy identifies the remaining full closure, not an admitted
Slug load; `tools/cpp/BUILD:14` first imports the public `cc_library.bzl`.

## Owner, decision and invariants

`HostRootRepositoryLoadRouteKey` is the existing DICE producer of the
root-apparent to canonical source route used by Loading and Query. For its
ordinary admission only, select `HostRepositorySourceRoute::Canonical` for
the successful built-in root predecessor, deriving its
`HostCanonicalRepositorySourceInput` through the existing
`HostCanonicalRepositoryLoadRouteKey`. Retain the selected root
apparent name, root-route/mapping observations, and canonical source identity;
do not synthesize a host path or rebind to a command-local cache. Preserve
the existing Root route for direct-local and selected-registry sources and
the generated-repository canonical fallback. Keep the separate
`for_root_build` admission's built-in Root route unchanged: root BUILD `.bzl`
Loading already performs its deliberately narrow canonical child promotion
after a single admissible public load. Eager canonicalization there would add
selected-graph dependencies to bootstrap and risks a cycle. No new key,
lock, registry, hash, process cache or source owner is chosen.

The canonical source route already owns immutable catalog MODULE/BUILD/`.bzl`
bytes and repository mapping. Its downstream `RepositoryPackageSourceKey`,
`RepositoryPackageLoadKey` and `ExternalUnconfiguredPackageGraphKey` must
consume that route unchanged. Built-ins must not enter direct-local MODULE
support or generic host materialization; direct-local repositories retain
their include-cycle preflight. Canonical route Need/error and observation
frontiers propagate atomically, without a root-source fallback. DICE equality
and validity continue to use their existing complete/Need policy; root input,
mapping and source changes remain tracked dependencies. Command result sets
remain request-local. No lock spans a DICE compute; see
`docs/developers/dice.md`.

## Scope and stops

Base `b719a9dc3`; review branch
`codex/wp783-builtin-canonical-route-review`. Worker1 owns only
`app/slug_loading_v2/src/root_repository_load_route.rs` and
`app/slug_loading_v2/src/root_repository_load_route_tests.rs`. Worker2 may
add a focused consumer regression only in
`app/slug_query_v2/tests/loading_query.rs` once the design is accepted and
the route shape is fixed. Root owns these plan files, integration, serial
Cargo, a freshly built CLI diagnostic and the commit. Independent design and
final review are required because the DICE-retained source route changes.
The route module is under the 2,000-line complexity trigger; reuse its existing
root/canonical predecessor scaffold. The large Query integration file keeps
its already-owned epoch/fixture harness rather than duplicating it elsewhere.
Review triggers are >180 production or >300 new test lines, not caps.

Do not change the immutable catalog bytes, root MODULE/lock, generic host
materialization's intentional built-in rejection, module extension/repository
execution, `rules_cc` proxy bytes, parser/coercion, configured Analysis or
action execution. Do not special-case Query commands or skip the whole
repository route. REPLAN if the canonical route cannot retain the root
apparent identity and observed frontier, if it requires an unmodeled source
owner, or if avoiding direct-local preflight would hide a needed built-in
MODULE semantic input.

## Acceptance

The ordinary root-visible built-in route projects to canonical immutable source with
`@bazel_tools` display and `@@bazel_tools` semantic identity. A selected
`tools/cpp/BUILD` source read returns the exact WP781 catalog bytes through
the existing canonical owner with no generic host materialization. Show
ordinary and observed route equality/Need behavior, plus direct-local,
selected-registry, generated-route and `for_root_build` nonregression. The
adapter remains event-carrierless; existing selected-graph and canonical
predecessor owners retain their event replay contract, checked by
`observed_canonical_repository_route_real_order_events_and_parity`. A missing catalog child
or unresolved later import remains typed Need/error and cannot publish a
partial package. Preserve source/mapping A/B/A coverage from the existing
route suite; add only a discriminator the accepted tests lack.

Rebuild `slug_cli_v2` before one-shot CLI diagnosis. The fresh query of
`@bazel_tools//tools/cpp:empty_lib` must no longer fail at the built-in
direct-local MODULE/materialization error; record its next exact typed
terminal, not a success claim. The portable unit/consumer regressions, not
the local cache, are the acceptance gate. Prepare owner tests with pinned
nightly, offline/locked Cargo JSON `--no-run` under 60 seconds, exact
nonignored preflight and counted execution. Check direct Query and Core
dependents, changed-file rustfmt, diff/plan/archive checks, and independent
final review. Do not run Bazel or remote transport for this source-route
packet; WP783 has pinned-source evidence and no fresh oracle gap.

Independent design review ACCEPT: the Ordinary-only built-in canonical route
uses existing DICE owners without an identified bootstrap cycle. Final
evidence must trace Root plus canonical predecessors for Ordinary and a single
Root predecessor for `for_root_build`.

## Acceptance receipt

Independent design and final review ACCEPT. The pinned nightly offline/locked
Loading test executable compiled in 5.60 seconds after the final test edit;
exact nonignored preflight selected eight tests, and seven root-load-route
tests plus the catalog-owner test passed. The inherited event-owner test also
passed after exact preflight, and the existing Query nonfallback regression
passed 1/1 after exact preflight. Direct Analysis/Core checks passed. The
rebuilt CLI one-shot query moved from the old built-in direct-local MODULE
materialization failure to `@@bazel_tools//tools/cpp:BUILD:119`, where
`@local_config_cc` is not visible (exit 7); no full package success is claimed.
Changed-file rustfmt, plan/archive status and `git diff --check` passed; no
`slugd` remained after the smoke. The route adapter adds no event owner.

WP782 is the immediate predecessor at `b719a9dc3`: multi-target external
Starlark Loading/Query edges accepted with independent design/final review,
18 focused tests and direct Analysis/Core checks; authentic tools/cpp Loading
remains deferred.
