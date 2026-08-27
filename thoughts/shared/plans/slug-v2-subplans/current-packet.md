# Current Slug V2 Packet

Packet: `WP-4-5-7A-canonical-loading-source-address-design`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `fa896aca4`.

Result: freeze the bounded Stage B implementation contract that adapts the
accepted Root/Canonical repository source carrier into external subtree,
package and recursive `.bzl` loading without fabricating an apparent alias or
a physical path for embedded catalog content.

## Accepted basis and newly exposed boundary

Commit `fa896aca4` accepts Stage A. Bzlmod REPO, ignore, package lookup,
boundary and selected BUILD-source policy now share one compact
`HostRepositorySourceRoute`; every root constructor and dependency order is
preserved, all four temporary canonical source/listing wrappers are deleted,
and alias-free selected-registry policy reaches the shared observation owners.
Canonical built-in selection intentionally stops at the authenticated
catalog-relative address because the existing package-source result requires a
Host `NormalizedAbsolutePath`.

The remaining Stage B consumers are two loading-owned chains:

1. `ExternalSubtreePackageSetKey` traverses package boundary plus directory
   listing and still stores `RootRepositoryRoute`.
2. `RepositoryPackageLoadKey`, `ExternalBzlModuleEvalKey`, their observed
   siblings and external cycle identities still store `RootRepositoryRoute`.
   Package source, recursive child resolution and cycle source reobservation
   therefore cannot consume the canonical carrier.

The existing repository package loader already uses an explicitly synthetic
`<output_base>/external/<canonical>/...` `PathBuf` only as evaluator and
published-package presentation. It does not use that path for source IO. Keep
that Slug-native presentation domain distinct from a Host absolute source
address, a built-in catalog-relative address and a canonical Starlark source
name.

## Architectural contract to freeze

Retain `HostRepositorySourceRoute` as the sole Root/Canonical semantic route.
Do not introduce a second loading route enum. Root constructors remain exact;
canonical constructors accept `HostCanonicalRepositorySourceInput` and form
the same carrier.

Redesign `RepositoryPackageSource` around an explicit source-address
discriminant:

- `Host(NormalizedAbsolutePath)` for existing local, immutable-registry and
  generated materialized source observations; and
- `BuiltinCatalog(repository-relative path)` for embedded catalog content.

Both variants retain the selected BUILD name and the producer-owned byte
`Arc` without copying. A catalog path is never converted into a workspace,
execroot, output-base or other absolute path. Delete the Stage A
`BuiltinSourceAddressDeferred` terminal only in the same implementation that
teaches loading to consume the address discriminant.

Add one pure loading-owned presentation adapter. It must derive parser and
evaluator names from semantic identity, not from access paths:

- root Host sources retain their accepted absolute source name;
- canonical BUILD and `.bzl` sources use a stable valid-Unicode canonical-label
  source name; and
- the already accepted repository `LoadedPackage.package_dir`/`build_file`
  presentation remains the explicit Slug-native `<output_base>` `PathBuf`
  projection and never becomes source authority.

The adapter is scratch/pure state. It is not a DICE key, cache, interner or
retained filesystem capability. This follows Zabel's useful separation in
`src/load/source_access.zig`, `session_build_file_source.zig`,
`session_selected_materialized_package_source.zig` and
`session_bzl_module_source_computation.zig`: semantic source-root and canonical
runtime identity are distinct from readable access, parsed syntax is
evaluation scratch, and retained values borrow producer-owned source bytes.
Zabel is peer architecture/optimization guidance only; Bazel 9.2 remains the
behavioral authority.

Generalize external subtree, repository package-load, external `.bzl` and
external cycle identities over `HostRepositorySourceRoute`. Existing root
constructors, output values, errors, display, hashes, event batches and child
dependency order remain exact.

For canonical recursive `.bzl` loads:

1. parse the same admitted Bazel load-label forms;
2. resolve an apparent repository through the current canonical route's final
   mapping;
3. demand the child `HostCanonicalRepositoryLoadRouteKey` or observed sibling
   by canonical name;
4. in observed mode merge the child route/effect epoch before demanding the
   child source; and
5. build the child external-module key from the returned canonical source
   input, never from a fabricated root alias.

Same-repository loads retain the current carrier. Root selected-registry loads
retain their accepted synchronous `selected_bzl_load_route` path. Need, outer
frontier error, semantic route/effect error, source terminal and child-module
terminal keep their existing precedence. Cycle identities contain the exact
carrier plus repository `.bzl` label, and observed cycle completion reobserves
sources through the carrier's shared source-observation owner.

This is generic BCR Starlark loading architecture. Bazel 9 BCR Starlark owns
all rules and control flow, including `cc_internal`; `cc_common` is only a
demanding consumer of reusable host-ABI capabilities. No C++ parser, native
rule implementation or language-specific rule engine is admitted.

## Design work and output

This packet is docs-only. Audit the live constructor/caller matrix for:

- `RepositoryPackageSource{,Observation}Key`;
- `RepositoryPackageLoad{,Observation}Key`;
- `ExternalBzlModule{Eval,Observation}Key` and
  `ExternalBzlCycleIdentity`;
- `ExternalSubtreePackageSet{,Observation}Key`;
- module-extension, query and core root-only callers; and
- built-in, local, immutable-registry and generated source variants.

Freeze one implementation packet with an exact file allowlist, production and
proof caps, 120-line function cap, protected root regressions, canonical
success/error/cancellation matrices and structural no-alias/no-physical-path
guards. Prefer helper extraction over widening the large loading drivers.

The implementation proof must include:

- byte-for-byte root result/error/display and exact child dependency-order
  regressions;
- built-in canonical BUILD source success retaining the exact catalog address
  and byte `Arc` with no absolute-path invention;
- root-unmapped canonical selected-registry package-load success;
- canonical same-repository and mapped child `.bzl` success, with child
  route/effect before source and exact observation-epoch order;
- canonical child route Need, route semantic error, effect error, missing
  source, parse/evaluation error and recursive cycle polarity;
- canonical external subtree traversal through the generalized boundary and
  listing owners;
- carrier/key/hash A/B/A across workspace, canonical name, disposition,
  selected specification, final mapping, generated plan, package and label;
- drop-before-publication and same-DICE recovery for canonical package and
  recursive `.bzl` loading;
- zero production activation/reference to any deleted temporary wrapper and
  zero fabricated apparent alias or absolute catalog path; and
- retained size plus `Allocative` coverage for every new enum/key/value.

Reuse accepted Bazel 9.2 source/loading evidence unless the audit demonstrates
a behavioral gap. Add no oracle merely for Rust ownership or representation.

## Compatibility classification

- **Exact:** Bazel 9.2 package-marker choice, repository mapping and admitted
  load-label resolution; BCR catalog bytes; root loading results, diagnostics,
  events and dependency order; canonical source-before-evaluation semantics.
- **Slug-native:** Root/Canonical carrier/key layout, source-address enum,
  canonical-label parser source names, explicit repository package presentation
  paths, structural hashes and retained-memory accounting.
- **Unsupported/deferred:** unadmitted load-label forms, broader glob/package
  traversal behavior not already owned, registration expansion, configured
  semantics, rule/action execution and exact output identity.

## Scope and stops

Allow only the three active planning ledgers for this design. Add at most 700
documentation lines and no Rust, fixture, oracle, dependency or lockfile
change. Run `scripts/v2_archive_status.sh` and `git diff --check`; only the
known three-row archive baseline may remain. Require independent
DICE/loading/retained-representation pre-review.

STOP and `REPLAN` for a second semantic route; a fabricated apparent alias;
conversion of catalog-relative content to `NormalizedAbsolutePath`; source IO
through presentation paths; copied source bytes; a lock across DICE compute;
changed root output/error/event/dependency order; loading-owned materialization
or catalog IO; retained parser/evaluator scratch; query/core production scope;
or activation of registration/configured/rule/action behavior.

On acceptance, implement only the frozen
`WP-4-5-7A-canonical-loading-source-address-implementation` packet. The shared
registration expander follows only after Stage B acceptance.
