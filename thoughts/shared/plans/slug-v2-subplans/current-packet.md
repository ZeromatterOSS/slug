# Current Slug V2 Packet

Packet: `WP-4-5-7A-repository-qualified-external-bzl-load-route-implementation-r3`

Milestone: M7A registered-toolchain closure prerequisite.

Base: accepted direct `tools/build_defs/cc` catalog `84190d95c`, accepted
external-`.bzl` source-observation cutover `1b997c5ef`, and independently
reviewed repository-qualified route design `5fd9b9f0d`. The proof-only
registration and selected-context candidates remain dirty, parked, and
read-only.

R1 implemented the reviewed Root/Canonical child-route handoff within its five
files. Its first discriminating generated-child test proved the mapping and
canonical route selection, then stopped before source access at the packet's
explicit `REPLAN` boundary:

```text
HostSelectedExtensionOwnerInputsError(
  OwnerInputsError::Unsupported {
    owner: HostSelectedExtensionOwner {
      id: @@parent+//:compatibility.bzl % compatibility,
      ...
    }
  }
)
```

The R1 route implementation is retained as a valid prerequisite. Independent
R2 review returned `REPLAN`: canonical definition-host selection was correct,
but R2 proposed the host's pre-override row as the generated-repository
namespace base. Bazel starts from the host's full substituted mapping. R3
corrects that ordering and adds only the natural Bzlmod owner correction below;
it does not add a loading special case or widen rule semantics.

## Observable boundary

The selected-registry parent mapping resolves
`compatibility_repo -> parent++compatibility+compatibility_repo`. Loading then
demands the existing canonical generated-repository route, whose effect asks
for the selected extension owner certificate. The owner-input projection
rejects solely because `owner_inputs` requires at least one root usage and
always selects the root base/final mappings.

That root restriction is not Bazel behavior. It also selects the wrong
namespace mapping when an extension is defined in one selected repository and
used by another. The canonical extension id already names the defining `.bzl`
repository, and the retained selected graph already owns every route and every
usage-owner mapping needed to correct the projection.

This remains a generic ordinary-module-extension boundary. The rules_cc
`compatibility_proxy` extension and public `cc_common.bzl` are only the real
discriminator. Bazel 9/BCR Starlark remains the complete rule and control-flow
owner; this packet changes no parser, builtin, provider ABI, C++ rule engine,
or source catalog.

## Learned facts and authority

Bazel 9.2 is sole semantic authority:

- `ModuleExtensionId` retains the canonical extension `.bzl` label and export
  name independently of which modules use it.
- `SingleExtensionUsagesFunction` groups all usages by that canonical id,
  retains participating modules in selected graph order, and projects
  `getFullRepoMapping` separately for every usage owner.
- `RegularRunnableExtension.load` loads the canonical
  `extensionId.bzlFileLabel()` directly; it does not require a root usage or a
  root-visible apparent alias.
- `RegularRunnableExtension.createContext` creates one Starlark module row for
  every participating root or nonroot module using that module's exact
  repository mapping.
- `ModuleExtensionRepoMappingEntriesFunction` identifies the module hosting
  the extension from the canonical repository of the definition label and
  composes generated-repository mappings in exact order from that host
  module's full mapping, sibling generated repositories, and overrides.
- `BazelDepGraphValue.getFullRepoMapping` includes selected dependencies and
  admitted `use_repo()` imports before generated-output validation.

The live Slug trace matches those ownership shapes except for one deliberate
legacy guard:

1. `HostSelectedExtensionMappings` already groups ordinary usages by canonical
   `HostSelectedExtensionId` and retains selected route order.
2. Its `base_mappings` and `mappings` arrays are index-aligned with selected
   routes and preserve, respectively, pre-override and fully substituted
   per-module mappings. Only the latter matches Bazel's starting namespace.
3. `owner_inputs` already emits one module row per participating selected route
   and uses that module's final mapping for tag-label coercion.
4. It nevertheless rejects when no usage owner is Root and chooses Root rows
   for the definition request and generated-repository namespace. Replacing
   Root with the host's pre-override row would still be wrong when a visible
   imported generated repository is overridden.
5. The owner-scoped certificate/effect path does not demand the older
   `HostSelectedExtensionDefinitionLoadRequestsKey` root-wide aggregate.

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` remains
**concept/test only** peer guidance:

- `session_grouped_extension_execution_inputs.zig` groups execution by
  canonical extension identity, retains selected-module order, and selects
  each tag repository context by canonical usage owner;
- `session_selected_extension_source_execution.zig` selects one canonical
  producer module for the definition closure and keeps evaluator state
  invocation-local; and
- `session_selected_repository_view_projection.zig` projects one complete
  mapping per canonical selected repository from the retained resolution.

Do not reuse Zabel code or representation. Avoid its scheduler, service
families, stores, semantic tokens, and physical-source machinery. The useful
lesson is only the same split natural in Slug: one canonical definition host,
many usage-owner module mappings, and no root-specific execution owner.

## Decision and compatibility

Retain R1's reviewed projection unchanged:

```text
RootRepositoryBzlLoadRoute
  Root(RootRepositoryRoute)
  Canonical(CanonicalRepoName)
```

For ordinary non-isolated `HostSelectedExtensionOwnerInputs`, replace the root
usage/mapping requirement with a canonical definition-host projection:

1. locate exactly one selected route whose `canonical_repo` equals
   `owner.id.bzl_file.package().repo()`;
2. select the fully substituted final mapping at that same route ordinal;
3. construct the existing definition source from that host final mapping and
   canonical label;
4. populate both the existing namespace-base and definition-mapping fields
   from that final host mapping; existing instantiation then overlays sibling
   generated repositories and current-extension overrides in Bazel order;
5. continue emitting every participating root and nonroot module in selected
   graph order, with each usage owner's own final mapping and ordered tags.

A missing or duplicate canonical definition-host route, missing aligned final
mapping row, missing selected self alias, inconsistent unique name, isolation,
or non-module-extension owner fails closed before `.bzl` source access. Root-
defined extensions continue to select the Root route even when only nonroot
modules use them. Selected-registry definitions continue through
`RootRepositoryRoute::for_selected_extension_definition`; no source kind,
extension name, repository name, or canonical spelling is special-cased.

Compatibility classes:

- **Exact:** canonical definition identity; definition-host fully substituted
  namespace/definition mapping; sibling/current-override overlay order;
  per-usage-owner tag mappings and graph order; nonroot-only ordinary extension
  execution; selected/generated child label and route; Legacy result; Observed
  frontier union; missing/duplicate failure before source access; unchanged
  selected dependency, built-in, and same-repository Root child routes.
- **Slug-native:** Rust projection/error types, DICE key names, structural
  equality/hashing, `Arc` layout, observation carrier internals, and retained
  legacy aggregate organization.
- **Unsupported/deferred:** isolated extensions; nonregistry definition sources
  not admitted by existing selected routing; the older root-wide aggregate's
  nonroot execution breadth; physical materialization beyond existing owners;
  unrelated repository/ruleset breadth; C++ action semantics; configured
  testing/coverage invocation; exact Java/HotSpot state; and later action
  families.

Non-decisions: no parser or builtin work; no Rust-defined rule semantics; no
new key, mapping graph, or owner certificate; no loading-side RepoSpec/source/
path synthesis; no physical fallback; no copied bytes; no aggregate-to-owner
adapter; and no compatibility-proxy, rules_cc, `cc_common`, or `cc_internal`
special case.

## DICE, request, and lifetime contract

No DICE key is added or changed. The generated-target dependency row remains:

```text
ExternalBzlModuleEvalKey (Root selected parent)
  -> HostCanonicalRepositoryLoadRouteKey
       -> HostGeneratedRepositoryDefinitionKey
            -> HostSelectedExtensionDemandKey
            -> HostSelectedExtensionOwnerCertificateKey
                 -> HostSelectedExtensionOwnerPureKey
                      -> HostSelectedExtensionOwnerInputsKey
                           -> HostSelectedExtensionMappingsKey
                      -> ExternalBzlModuleEvalKey (definition host)
       -> HostSelectedRepositoryFileEffectKey
  -> ExternalBzlModuleEvalKey (Canonical generated child)
```

Observed mode substitutes the observation siblings and unions their exact
frontiers before child source demand. The route does not activate the legacy
root-wide definition-request/prepared-invocation aggregate. Same-repository,
selected-registry dependency, and built-in loads retain their current direct
dependency rows.

The owner-input key already structurally retains the owner id and complete
selected mappings. A definition-host mapping or usage-owner mapping revision
therefore changes the result and invalidates the certificate, effect, route,
and child evaluation naturally. A/B/A restoration must reproduce request,
module rows, generated route, and result equality. `Need` remains
non-cache-valid. Overlapping requests deduplicate through DICE; no mutex, side
cache, global registry, task, or await-under-lock is introduced.

The route enum and parse results remain evaluation scratch. Owner inputs retain
only existing owned mapping/route/module values. No retained value borrows
evaluator scratch; no new service cache, eviction, shutdown, or transfer-owned
memory exists.

## Errors and proof matrix

- Missing/duplicate definition-host route: owner-input semantic error and zero
  definition `.bzl` or generated child source access.
- Missing aligned final host mapping or selected self alias: same fail-closed owner
  boundary with the complete predecessor retained.
- Root-defined/nonroot-only owner: Root definition load plus only nonroot
  invocation-module rows.
- Selected-defined/nonroot-only owner: selected definition load from its
  canonical host mapping plus only nonroot invocation-module rows.
- Shared root/nonroot owner: one definition request and one module row per
  participating selected route in graph order, each with its own mapping.
- Generated effect or child failure: existing Route/Child error ordering and
  observed prefix remain unchanged.

Repository-owned tests must prove:

1. pure Bzlmod projection admits root-defined/nonroot-only,
   selected-defined/nonroot-only, and shared root/nonroot ordinary extensions;
2. definition request namespace/source use the canonical definition host's
   fully substituted final mapping, while module tag rows use each usage
   owner's final mapping;
3. a host-visible imported generated alias that is overridden names the
   replacement in the generated repository after sibling/current overrides;
4. missing and duplicate definition-host routes, missing aligned final maps,
   isolation, and mismatched unique names fail before source demand;
5. request/module/route equality changes for mapping A/B and restores for A;
6. the R1 selected parent loads one extension-generated child in Legacy and
   Observed modes with exact canonical identity, provider value, route/source
   dependencies, and frontier union;
7. that route activates the owner-scoped certificate path and not the old
   root-wide definition-request aggregate; and
8. direct selected dependency, built-in, same-repository, cycle, Need,
   overlapping-request, and error proofs remain green.

Use existing synthetic registry, extension, materialization, dependency-trace,
and observation-epoch scaffolding. No new copied fixture subtree or external
fixture file is admitted. Reuse the accepted two cold real rules_rust replays
as the exact integration oracle; add no Bazel oracle unless implementation
exposes behavior not covered by pinned source/tests and the real rules_cc
mapping evidence.

## Allowlist, caps, validation, and stops

Frozen base blobs at `5fd9b9f0d`:

- `app/slug_bzlmod_v2/src/host_module.rs`
  `504885b531d6deb2874102aac4b125a3dbfe2ba0`;
- `app/slug_bzlmod_v2/src/selected_repo_spec.rs`
  `05387c9c888118421f1aa087eb8ada006a3a32e6`;
- `app/slug_bzlmod_v2/src/selected_repo_spec/selected_extension_demand.rs`
  `02d56046a5981f896e650ea91ffef24b1c0abb19`;
- `app/slug_bzlmod_v2/src/lib.rs`
  `f258307feea1ef0b4a5352071994f54be7999eb8`;
- `app/slug_loading_v2/src/bzl_module.rs`
  `bd7b919bca8ed905f9da88f0705825c608fceb2d`; and
- `app/slug_loading_v2/src/canonical_repository_load_route_tests.rs`
  `6f4be47551650325525c07217a3c9672ce12047c`.

Only those six Rust files may differ from `5fd9b9f0d`. Cap net production
growth at 120 lines and net test growth at 500 lines across the packet. No new
crate, dependency, key, fixture file, unsafe code, background task, lock,
cache, fallback, or public stability shim.

The touched production owner files exceed the 2,000-line review trigger. Keep
the changes colocated because each edit is a bounded extension of its existing
mapping projection or recursive-load matcher. Do not add another responsibility
or touch a function over 150 lines. This is not a demonstrated hot-path or
retained-representation change, so performance measurement and Buck2 utility
adoption are inapplicable.

Validation after implementation:

1. targeted owner-input and Bzlmod route projection tests;
2. targeted generated canonical load-route/external-module tests in both modes;
3. complete `slug_bzlmod_v2` and `slug_loading_v2` suites;
4. one direct compile dependent and `cargo build -p slug_cli_v2`;
5. clean stale `slugd`, then two fresh-workspace/fresh-output-root real
   rules_rust cqueries with identical outcome and no
   `@cc_compatibility_proxy` route/owner failure;
6. `cargo fmt --check`, `git diff --check`, cap/allowlist/dirty-isolation
   checks; and
7. independent terminal patch review.

`REPLAN` if canonical definition-host selection or fully substituted namespace
ordering cannot represent the real nonroot rules_cc extension; if loading, the
legacy aggregate, a new key, or a second owner must change; for any extension/
repository/ruleset spelling or path special case; parser/`set` changes;
Rust-defined rule semantics; loading-owned RepoSpec/source synthesis; physical
materialization work; copied bytes; weakened ambiguity/missing-map failure;
changed selected/built-in Root dependencies; parked dirty-file work; cap/
allowlist overflow; or a second material architecture correction.
