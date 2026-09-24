# Current Slug V2 Work Packet

Packet: WP-7-134-m7a-build-module-metadata-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Admit exact Bazel 9.2 `module_name()` and `module_version()` BUILD globals
and `native.module_name()` / `native.module_version()` from a loaded
macro, for root and canonical selected Bzlmod module repositories. Expose
these selected names context-specifically: bare functions in BUILD but not
.bzl, and native methods in .bzl but not BUILD. Slug's pre-existing BUILD
`native` object remains a separate Slug-native environment divergence; it
must not acquire these new methods. The value
is the *calling BUILD package's* associated module's declared name and
normalized declared version, not the macro source's repo, apparent repo name,
archive URL or selected graph lookup key. A missing root declaration has
Bazel's empty name/version; an explicitly empty version returns `""`, not
`None`. These are exact selected-context value and binding claims. The
existing structural repository/package identities remain Slug-native; no
Bazel configuration or output-identity bytes are claimed.

For direct-local legacy routes, extension-generated repositories and
`@bazel_tools`, calls fail closed as unsupported/deferred until their
associated-module provenance is projected from a DICE owner. Do not substitute
apparent or canonical repo tokens, silently return empty strings, or claim
full BUILD/native global parity. Direct `MODULE.bazel` remains without
these globals; .bzl top-level calls have no target-definition context and
must fail rather than borrow the source repository's metadata. Other
platforms/rules_python bootstrap gaps and M7A remain open.

## Pinned source, owner and decision

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`:
`StarlarkGlobalsImpl.java:75-77,129-132` and
`BazelStarlarkEnvironment.java:252-303` distinguish BUILD globals from
the .bzl `native` binding; `StarlarkNativeModule.java:665-683` reads
`TargetDefinitionContext`, which restricts invocation to BUILD/macro
evaluation (`TargetDefinitionContext.java:170-195,224-249`).
`RepositoryMappingFunction.java:115-174` supplies associated module
metadata; `RepositoryMappingValue.java:57-88` normalizes the version;
`PackageFunction.java:1182-1201` passes it into package evaluation.
Upstream `bazel_module_test.py:713-798` demonstrates caller context and
an overridden nonregistry module whose declared version differs from its
empty lookup version. The live authentic Bazel 9.2 `@@platforms` 1.1.0
`BUILD:18-21` uses both globals in a package-metadata purl.

Stage 5's selected module route owns evaluated declared name/version in
`HostSelectedModuleRoute.entry.source`; expose a narrow borrowed projection
through both the canonical route view and the selected
`RootRepositoryRoute` view. Apparent and canonical package loads of a
selected-registry module must see the same declaration; local-path
overrides can still take the unsupported direct-local apparent route.
Do **not** use
`HostGraphModuleKey::Module.version`: nonregistry override lookup identity
can be empty while the module declares a nonempty version. Root
`RootModuleLoadingAnchorKey` already tracks the evaluated root module;
project its header through the retained anchor without a new filesystem
read, key, registry or lock. `RepositoryPackageInventoryKey` and
`RootPackageLoadKey` already consume those respective route/anchor
dependencies. Thread the immutable metadata into the evaluator-scoped
`HostPackageAttemptInput` and `PackageRecorder`, then make both global
and native calls read that caller package context. No evaluator-heap value,
command scratch, or temporary route may be retained by DICE. Existing
complete route and anchor equality/invalidation must account for module
declaration edits with unchanged BUILD bytes; an A/B/A proof is required.
Review generated/direct-local source ownership instead of inventing a
fallback. Vendored `dice/dice/docs/writing_computations.md` records that
`ctx.compute` creates dependencies and key equality controls resurrection;
existing root-anchor equality/restoration tests
(`host_module.rs:2844-2895,3440-3558`) and selected-route equality tests
(`selected_repo_spec.rs:9994-10030`) are protected. No Buck2
utility/representation change is planned. Selected canonical and selected
Root route Hash projections include the borrowed declared name/version
already retained by their definitions; complete structural Eq remains the
semantic identity even where hash collisions are possible. Add no new
retained selected-route allocation, collection, interner or content digest;
the evaluator-scoped metadata copies and returned strings are ephemeral.

## Evidence and validation

Before editing, retain the authentic ignored-RC/no-fetch Bazel 9.2
`query '@@platforms//:package_metadata' --output=build` evidence:
`purl = "pkg:bazel/platforms@1.1.0"`. The live source is selected
platforms 1.1.0, not the older mirrored 1.0.0 archive. Also retain the
independent `@@package_metadata` 0.0.3 purl oracle and pinned-source
root/extension/nonregistry behavior above. These cache-bound authentic
queries and upstream macro test do not portably discriminate bare BUILD
globals, so add one minimal Stage 1 Bazel-vs-Slug fixture
`module-metadata-build-context`: a versioned root, an empty-version edit
and restoration with unchanged BUILD/.bzl, and a macro loaded from a
different local module but invoked in the root BUILD. Derive exact
`filegroup` target names from both values so admitted exact-target
`query --output=label` discriminates them without relying on deferred
external `:all` or `--output=build` formats. Selected nonroot values are
covered by focused apparent/canonical Loading tests plus the pinned
upstream nonregistry test and authentic selected-module purls. Public
local-path-override package loads still use the deferred direct-local
route; public canonical `@@` target-pattern syntax is separately deferred.
Use ignored RC/offline Bazel 9.2 generation and independent verification;
record pinned source, command, exact outputs and unsupported exclusions in
`fixture.toml`. Keep at most the files needed for these discriminators,
run the applicable fixture-growth hygiene checkpoint, and do not copy the
authentic archive or credentials.

Focused tests must cover root declared and omitted metadata, selected
registry and nonregistry declared-versus-lookup version, nested package,
a macro loaded from another repository reporting its caller package,
empty-version truthiness, direct `MODULE.bazel` binding absence, .bzl
bare-global absence, BUILD `native.module_*` absence, .bzl
native top-level rejection, arity, and explicit fail-closed generated/
direct-local/builtin calls. Prove same-DICE and independent-transaction A/B/A
root package Loading across declaration edits with unchanged BUILD bytes,
and selected-canonical nonregistry route/definition metadata invalidation
and restoration across declared-version edits with unchanged source BUILD.
The latter is a route DICE proof, not a public nonregistry package-loading
claim; the selected route is consumed by the existing package key. Separately
prove apparent/canonical selected-registry route agreement and its declaration
invalidation when the selected version/key changes;
keep package/mapping controls. Require no direct filesystem or host-name
lookup from globals. Rebuild `slug_cli_v2` after Rust edits and replay the
authentic `//:probe` with clean `slugd` before and after. Because Loading is
parallel, record the next first error without claiming every path completed.

Serialize Cargo. Resolve pinned nightly with `rustup which`, compile
focused test executables with Cargo JSON `--no-run`, preflight exact
nonignored selectors, run focused/protected tests and dependent
`cargo check`. Run pinned rustfmt, diff/plan/archive checks, and
independent DESIGN and FINAL reviews for this public Loading/DICE boundary.
Use existing upstream tests as pinned-source regressions; do not copy Java
helpers or artifacts into Slug.

## Scope and stops

Base clean main `6a9a0960f`. Allowlist: this manifest and canonical
Current; one compact Stage 1 fixture-growth checkpoint note before
acceptance and compact Stage 4/5/9 owner notes after acceptance;
`app/slug_bzlmod_v2/src/{host_module,selected_repo_spec,canonical_repository_route}.rs`
and their existing focused tests; `app/slug_loading_v2/src/{bzl_module,package,host_package_load_tests,host_package_attempt_tests}.rs`;
`tests/v2_oracle/fixtures/module-metadata-build-context/**` (one
new portable fixture, roughly 7 source/manifest files). Roughly 150-300
production/test lines is a review trigger, not a cap. The large existing
Loading files remain their current cohesive owners; do not split merely to
evade size review. No generated-repo host inference, direct-local
module-discovery bridge, source-catalog rewrite, action/configuration work,
or remote-execution policy change. `REPLAN` if admitted values require a
new semantic key, selected-route equality lacks declared metadata, root
anchor is not a tracked dependency on every path, or the authentic call
needs a generated/direct-local/builtin branch.

## Immediate predecessor

WP833 at `6a9a0960f` accepted the exact selected seven-file
`@@bazel_tools//tools/python` runtime source/Loading closure. Its
authentic `//:probe` replay first stopped at
`@@platforms//:BUILD:21` with `Variable module_version not found`;
parallel Loading does not prove the rules_python constructor path ran.

## Acceptance (2026-09-24)

Independent DESIGN and FINAL reviews ACCEPT the bounded exact metadata
slice. Stage 5 projects evaluated declared name/normalized version from
tracked root/selected module owners, not selected lookup or repository
spelling; existing structural route equality remains complete and route
hashing includes the borrowed declaration. Stage 4 reads that caller BUILD
context for bare globals and loaded-macro `native` methods. Wrong-context
bindings and generated/direct-local/builtin calls fail closed. Root package
Loading and selected-canonical nonregistry route tests prove same-DICE
A/B/A declaration invalidation/restoration; selected-registry
apparent/canonical agreement, context/arity and mapping controls pass.
The nonregistry proof is route-level, not public package-loading parity.

The five-row, seven-file offline fixture passes fresh ignored-RC Bazel 9.2
generation, independent Bazel verification and rebuilt Slug replay. Its
exact-target names distinguish root BUILD globals from a foreign loaded
macro's caller across a version A→empty→A edit; authentic selected-module
purls and pinned source support the nonroot boundary. The required Stage 1
fixture-growth checkpoint is ACCEPT with explicit weak-row debt. Pinned
rustfmt, dependent checks, diff/plan/archive hygiene and `slugd` cleanup
pass. Rebuilt authentic `//:probe` moves past `@@platforms//:BUILD:21` and
first stops inside `@@package_metadata+//licenses/rules:license.bzl:106`:
`license()` rejects `visibility` as not a list of labels. Parallel Loading
does not establish that every other path completed. That separate Loading
gap, public direct-local/generated/builtin metadata, full root build and
M7A remain open.
