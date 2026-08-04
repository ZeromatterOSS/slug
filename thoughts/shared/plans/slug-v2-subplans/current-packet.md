# Current Slug V2 Packet

Packet: `WP-5-m1-public-selected-build-source-loading-migration`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: public selected-BUILD source owner and atomic external loading migration
Evidence: the private routed path owner in `00e85153`, the private external
package-policy/lookup graph in `42ef64cd`, accepted external source/loading
lifecycle evidence, and the pinned Bazel 9.2 deletion/policy/BUILD ordering.
Add no new oracle.

Edit exactly these four Rust files:

- `app/slug_bzlmod_v2/src/host_package.rs`
- `app/slug_bzlmod_v2/src/lib.rs`
- `app/slug_loading_v2/src/bzl_module.rs`
- `app/slug_loading_v2/src/host_package_load_tests.rs`

The packet cap is **260 production lines / 650 test lines / 910 total lines**
(formatted net additions; slack authorizes no behavior, evidence family, or
file beyond this contract). Land the public source owner and loading migration
atomically; do not leave either a second marker selector or a public source
without its existing loading consumer.

Expose opaque public `RepositoryPackageSourceKey`, `RepositoryPackageSource`,
and `RepositoryPackageSourceError` types from bzlmod. The key identity is the
full `RootRepositoryRoute` plus canonical
external `PackageIdentifier`; constructors enforce that the package repository
equals `route.canonical_repo()`. Fields and private policy results remain
hidden. The value retains only the selected BUILD logical path/name and shared
bytes needed by loading, with accessors for those semantics—no physical root,
unselected marker state/bytes, raw label/span, policy event batch, activation
data, or mutable carrier.

Compute the private `ExternalRepositoryPackageLookupKey` first. Map its typed
`InvalidPackageName`, `Deleted`, and `NoBuildFile` outcomes and its typed lookup
error into distinct opaque `RepositoryPackageSourceError` variants. A failure
to compute the lookup is a separate `LookupCompute { package, message }`
variant; never stringify a semantic `Lookup { package, error }` into it. Only a
successful selected `HostBuildFileName` may compute
`HostRepositorySourceFileKey` for that one marker. Preserve three distinct
selected-source failures carrying the selected logical path: semantic
`Source { logical_path, error }`, `SourceCompute { logical_path, message }`,
and `SelectedSourceAbsent { logical_path }` when the chosen marker disappears
between lookup and byte read. Forward lookup and source
`SourcePreparationNeeds` unchanged. Complete values and errors use semantic
equality; every transient Need remains invalid and self-unequal. The public
source key owns no event batch. Cover each structural error equality/display/
source-chain class without adding a fault hook for unreachable compute errors.

Migrate `RepositoryPackageLoadKey` to compute only
`RepositoryPackageSourceKey`; remove its direct `BUILD.bazel`/`BUILD` probing.
Map the opaque source error into a dedicated typed
`RepositoryPackageLoadError` branch without flattening its source chain or
display context. Preserve selected BUILD origin, bytes, UTF-8/parse/load-label/
Bzl/target/glob errors, result equality, and the loader's existing local BUILD
evaluation event batch. Routed REPO policy events remain owned by the private
policy child and are selected through DICE activation; neither the public
source nor loader copies, stores, suppresses, or replays them.

Cover BUILD.bazel priority, BUILD fallback, canonical deletion and route-policy
short circuits before selected bytes, typed lookup/source/compute mappings,
exact Needs, selected source edit/delete/recreate, route A-to-B-to-A, warm
reuse, and captured/uncaptured routed REPO print without event duplication.
Retain current root loading and external Bzl behavior.

Stops: no second lookup or marker selector, direct filesystem IO, root lookup
reuse, private policy-order/event/equality change, fragment/package horizon,
include closure, evaluator defaults/validation/print semantics, contextual
mapping, registry/JVM transport, fixture/oracle, or file outside the allowlist.
Run focused public-owner and loading tests, formatting, GNU-Windows check, and
scope/cap/archive/diff gates. After acceptance, resume only the route-aware
package horizon; occurrence-preserving closure and evaluator/event correction
remain later serial packets.
