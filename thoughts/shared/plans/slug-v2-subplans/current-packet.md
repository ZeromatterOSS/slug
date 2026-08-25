# Current Slug V2 Packet

Packet: `WP-6-7A-generated-package-source-preflight-polarity-correction-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design base: retained generated-package/registry-input/selected-owner candidate,
accepted source-preflight polarity REPLAN, and independent design review

Result: make the existing repository-package source owner honor the admitted
route-source polarity, so an extension-generated route reaches its existing
lookup/materialization children without being reconstructed as a direct-local
root route.

## Learned facts and frozen state

The prior owner-pure implementation remains retained and non-writable. Its
focused Bzlmod, loading and core checks pass, and independent correction review
accepted the bounded producer proof after it was restricted to the generated
definition producer. The mandatory rebuilt Bazel 9.2
`module-extension-use-repo` fixture nevertheless fails before generated
repository materialization with
`DirectLocalModuleInspectionError::Input(DirectLocalModuleFileError::Route(_))`.

The failure is a pre-existing package-source ownership defect exposed by the
new route:

1. core's `GeneratedPackageRouteKey(@generated)` successfully constructs the
   existing `RootRepositorySource::Generated` route;
2. core passes that route to `RepositoryPackageLoadKey` and then
   `RepositoryPackageSourceKey`;
3. `drive_repository_package_source` unconditionally calls
   `direct_local_module_support(_observed)`;
4. that helper reconstructs `RootRepositoryRouteKey(@generated)`, whose root
   mapping correctly has no direct-local route for the extension-only apparent
   name; and
5. the opaque route error becomes the fixture's direct-local MODULE inspection
   terminal.

No change in the selected-owner loading keys can repair this without bypassing
the accepted package-source owner. The prior packet therefore reached its
declared formal-REPLAN stop rather than widening its nine-file authority.

Freeze all currently dirty Rust paths and the accepted fixture. The sole new
write surface is clean
`app/slug_bzlmod_v2/src/host_package.rs`, currently 4,967 lines with SHA-256
`8a4ba796e230ab4b8b0136c07e1a1b749e0a13fd7316ff4c6e3df40a652b5299`.
No Cargo/BUILD, fixture, oracle, core, loading, route, source-preparation or
selected-owner file is writable.

## Authority and implementation

Change only `app/slug_bzlmod_v2/src/host_package.rs` and its colocated proofs.
At the natural `RepositoryPackageSourceKey` producer, add one private pure
route-source discriminator and use it at the start of
`drive_repository_package_source`:

- `RootRepositorySource::Generated` skips only the direct-local MODULE-support
  preflight, begins with an empty observation prefix, and proceeds through the
  existing `ExternalRepositoryPackageLookup`, materialization, BUILD-marker
  selection and source-read path unchanged;
- `RootRepositorySource::DirectLocal` retains the existing Legacy/Observed
  support child, Need/error precedence, unsupported-cycle behavior, event
  ownership and observation prefix exactly; and
- `RootRepositorySource::BuiltinBazelTools` also retains its current support
  path exactly. Do not infer that every non-Generated route is DirectLocal.

Add no key, retained value, error variant, export, adapter, cache, side store,
fallback scan or command repair. Do not change `RootRepositoryRoute` identity,
the generated constructor/capability, package lookup/materialization/source
semantics, or any global aggregate. The route's already-retained source enum is
the complete discriminator; no mutable request input or host read is added.

The generated branch retains no replacement support value or epoch. Its empty
prefix is phase scratch and is merged only with the existing observed lookup
and source children. DirectLocal/Builtin retained state, equality cutoff,
invalidation, cancellation and event lifetimes are unchanged. Existing
`CompactString`, `Arc`, compact maps and `Allocative` remain sufficient; add no
Buck2/V1 utility or Stage 9 row.

## Evidence, compatibility and validation

The accepted fixture remains the exact authority. It pins Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` and
`ModuleFileGlobals#useRepo`, `SingleExtensionEvalFunction#compute`,
`ModuleExtensionRepoMappingEntriesFunction#compute`, `PackageFunction#compute`,
`TargetCompletor#createSucceeded`,
`ModuleExtensionResolutionTest#generatedReposHaveCorrectMappings` and
`BuildViewTest#testTopLevelInputFile`. It requires exit zero, canonical
`@@+ext+generated//:generated.txt` source-file classification and successful
completion. Add no oracle because this evidence already discriminates the
defect.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is concept-only architectural
guidance: `session_generated_repository_materialization` keeps a generated
repository under its natural retained producer, and
`session_selected_graph_extensions_root_direct_routes` consumes retained
generated demand identities without reconstructing a second route identity.
Copy no Zig code, representation, scheduler or output vector.

Add a focused pure discriminator proof covering Generated, DirectLocal and
BuiltinBazelTools polarity. Preserve and run the existing direct-local package
source lifecycle and module-error-chain proofs plus applicable builtin route
proofs. Then run full Bzlmod, loading and core lib/runtime/build/query/cquery
baselines serially, rebuild `slug_cli_v2`, clean stale `slugd` before and after,
and rerun the immutable fixture. Finish with formatting,
`scripts/v2_archive_status.sh`, `git diff --check`, exact one-file scope,
baseline hash/accounting, forbidden-secret/stale-JVM scans and independent
implementation review.

Caps are <=20 production, <=120 proof and <=140 aggregate additions, with
physical `host_package.rs` <=5,110 lines. Add no `rustfmt::skip`.

Imported generated-repository package/source behavior remains **exact Bazel
9**. The private discriminator and empty-prefix plumbing are **Slug-native**.
Generated query/publication breadth, other platforms and exact
configuration/output identity remain **unsupported/deferred**.

STOP a second file, new key/value/error/export, DirectLocal/Builtin behavior or
event/epoch drift, command-side bypass, generated route reconstruction,
fixture/oracle edit, cap/proof waiver, Java/JVM, milestone closure, M8/M7B or
identity-byte work. `REPLAN` before widening. After implementation acceptance,
resume the retained selected-owner packet's complete validation and terminal
review; M7 remains partial and M7A -> M8 -> M7B remains.
