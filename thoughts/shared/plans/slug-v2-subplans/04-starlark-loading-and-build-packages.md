# Stage 4: Starlark Loading and BUILD Packages

## Goal

Load and evaluate Bazel `BUILD.bazel` and `.bzl` files with starlark-rust,
using Bazel globals and package semantics rather than Buck loading semantics.

## Scope

- `load()` resolution through Bazel labels and repo mappings.
- `package()`, `exports_files`, `glob`, `subpackages`, visibility defaults,
  and package groups.
- `rule(implementation=...)` registration.
- Bazel `attr.*` surface.
- native module behavior for Bazel 9+.
- `.bzl` initialization caching through DICE.

## DICE Rule

Map Bazel's Skyframe restart behavior to explicit async DICE dependencies. If
a Starlark-visible method needs label resolution, file reads, repo mapping, or
repository materialization, route that through a named DICE key or async bridge
before returning the Starlark-visible value.

## Bazel `.bzl` global capability category architecture accepted (2026-08-30)

Independent R3 review accepts the category-wide architecture selected by the
authentic Bazel 9.2 `bazel_features` globals repository. Both `.bzl`
environments must expose `macro`, `PackageSpecificationInfo`,
`RunEnvironmentInfo`, `set`, `subrule`, and `DefaultInfo`; BUILD globals expose
only universe-owned `set` among those names. The first implementation therefore
also removes Slug's current BUILD `DefaultInfo` leak. Existing starlark-rust
`set` remains untouched.

Use one nonconstructible provider-key lane for `PackageSpecificationInfo` and
one defining-module export identity shape, while keeping lifecycles separate:
fresh-evaluator non-finalizer macro expansion mutates the existing package
owner and retains compact instance/target origin, visibility, and namespace
violation identity; dependency-only subrules retain sparse hidden attributes
and later execute through the existing configured dependency/action owner.
Macro finalizers, subrule toolchains/automatic exec groups, fragments,
attached/configured aspects, and provider instances remain explicitly deferred.

Commit `e34cfdc7a` terminally accepts the symbolic-macro/provider successor.
Both `.bzl` routes gain default non-finalizer `macro` and nonconstructible
`PackageSpecificationInfo`; BUILD loses its `DefaultInfo` leak and retains only
universe-owned `set` among the category. Fresh macro evaluators reuse the
package owner and print sink, while compact macro instance, target origin,
visibility, and namespace-violation facts participate in package equality.
Typed labels cross the real rule-coercion path, implementations must be
Starlark functions, and attribute inheritance requires `**kwargs`.

Full loading and analysis-lib validation passed. Two fresh authenticated
rules_rust 0.73.0 replays clear `macro` and stop deterministically at missing
`subrule`; independent correction rereview returned `ACCEPT`. The parked
28-line `package.rs` definition-source hunk remained outside the commit.

Commit `541fcfaf2` accepts the 18-production/64-proof configured namespace
successor. It consumes only the retained loading fact after exact target lookup,
rejects before configured semantic work, and passes exact failure plus A/B/A,
the full analysis suite, named cquery/build dependents, isolated staged build,
and independent terminal review. Parked analysis work remains outside the
commit.

Commit `4900ce46b` accepts
`WP-4-5-7A-subrule-declaration-and-analysis-architecture-r2` after focused
independent rereview.
The authentic first rules_cc consumer proves that a token-only global is not a
meaningful vertical: `create_fdo_context` has eight private
`configuration_field` label defaults, `cpp` fragment access, hidden rule
attachment, and later actions. R1 was `REPLAN`: Bazel late-bound identity is
typed fragment class plus field and tools repository, never the defining `.bzl`
module; descriptor lifting is ordered while rule/nested authorization and
publication are set-semantic; the first successor must freeze paths/caps and a
deterministic stop; and full FDO invocation additionally needs
`args`/`run`/`symlink` plus `cc_common.absolute_symlink` action families. R2
separates those owners and claims only the first loading successor after
review. Bazel 9.2 commit `8220c619…` alone defines semantics. Clean Zabel
`0795445f…` informs sparse spans, compact identity, typed late-bound ownership,
and borrowed context lifetime only; `cc_common`/`cc_internal` remain generic
downstream BCR Starlark discriminators, never parser or Rust C++ rule targets.

Commit `965cfde5e` terminally accepts the corrected loading successor after the
full live/index-only loading gates and focused independent rereview. It retains
separate direct/transitive identities and callable routes, canonical provider
predicates, ten typed `cpp` fields, ordinary late-bound rows, and persistent
`TemplateVariableInfo`, then fails closed before configured hidden resolution.

Configured hidden-dependency R2 preserved the correct pre-call and deferred-XML
boundaries, but independent review returned `REPLAN` until a real non-default
native producer existed. Commit `4425d3bfb` accepts that bounded thirteen-option
CLI-to-DICE closure. The R4 packet-only gate then proved that configured Exec
rule children require the generic selected-toolchain/action-context cutover.
Commit `ebd19e3b1` accepts that prerequisite with structural Target-or-Exec
configuration and selected implementation Exec configuration. Commit
`2bf86bfa8` then accepts the corrected configured-hidden successor: one
descriptor-order stream, one aggregate cycle guard, typed literal/ten-`cpp`
projection, provider/file/executable validation, and root loading-query facts.

`WP-4-5-7A-subrule-direct-call-and-value-materialization-r3` is terminally
accepted after one implementation `REPLAN` and corrected rereview. R1 required
a tagged Null/configured analysis Target identity; R2's inline multi-String
label was not compact. R3 uses an Arc-backed Null label with a two-word identity
ceiling and one admitted source-boundary allocation/clone. The accepted generic
category supplies ordinary/hidden evaluator values, all absence/file/executable
shapes, materialization-only source/generated `DefaultInfo`, argument binding
and override checks, nested authorization, an Arc-shared immutable dispatcher,
call-scoped restricted context/action lifetime, generic provider round trips,
and successful implicit/tool configured edges. The 256-call proof covers both
analysis routes; full loading/analysis and staged-only gates pass at
1,230-production/469-proof additions. Fragment/toolchain projection,
`args`/`run`/`symlink`, XML/aspects, and every rules_cc/C++ special case remain
deferred. Select fragment projection next.

R1 fragment architecture received `REPLAN`: it incorrectly treated target
`compilation_mode` as the selected Exec toolchain's mode and under-proved
Bazel's default private API allowlist. R2 received `REPLAN` after a newer Bazel
sibling checkout contaminated two Rust allowlist rows. R3 received `REPLAN`
because starlark-rust dynamic attribute lookup cannot raise Bazel's specialized
known-but-undeclared error. The corrected
`WP-4-5-7A-generic-fragment-projection-r4` freezes the pinned 9.2 inventory and
the 12 active fragment names, uses separate root/subrule facades, and explicitly
defers only the subrule diagnostic/`hasattr` distinction while preserving
declaration success/failure and exact `dir`. Independent architecture and
terminal implementation reviews returned `ACCEPT`; the implementation/proof is
complete at 665 production and 648 proof additions.
It selects the full six-method FDO fragment category consumed by authentic
rules_cc before its first action call, not a `cc_common` or rules_cc special
case. One structural `SlugConfiguration` owner feeds one evaluator-local typed
`cpp` value; cached rule and subrule collections retain separate declaration
authorization and Bazel-specific `dir` behavior. The packet also requires the
complete inventoried private Starlark API caller restriction, typed long-form
target/host compilation modes, and Bazel's bounded host-to-compilation Exec
rewrite. Absolute-path FDO producers remain fail-closed until Bazel
`PathFragment` ownership is designed; target default returns before actions,
while selected-Exec default `opt` selects the generic action-builtin successor.
Bazel 9.2 is the authority and Zabel is peer design/optimization guidance only.

## Accepted `.bzl` load-visibility design; implementation activated (2026-08-27)

The authenticated rules_cc traversal first requires Bazel's default-enabled
top-level `.bzl` `visibility()` at dependency-free
`cc/private/rules_impl/cc_toolchain_info.bzl:18`. A no-op global is invalid:
Bazel 9.2 captures normalized policy during module initialization, publishes it
with `BzlLoadValue`, and checks every direct Bzl and BUILD load before importer
execution.

Use the existing `BzlEvaluationContext` as evaluation scratch and
`FrozenBzlModule` as the durable semantic owner. A private
`bzl_visibility.rs` leaf may own only canonical immutable policy, declaration
parsing and pure matching. Extract the policy after successful evaluation,
default absence to public, include it in frozen-module equality, and retain no
Starlark heap value or context borrow. Existing source/child/mapping/route DICE
dependencies already own every input; add no key, cache, lock, registry or
manual invalidation. Compact immutable `Arc<[PackageSpec]>`, `Dupe`,
`Allocative` and canonical Slug identity types suffice without a new interner
or imported utility.

One direct-edge checker runs before importer evaluation in
`compute_host_bzl_module`, `compute_external_bzl_module`, local
`BzlModuleEvalKey`, shared root/repository `evaluate_host_package_attempt`, and
local `PackageLoadKey`. Bzl importers use their manifest root package. Package
attempts receive canonical package identity from the already-selected root or
repository route and never infer repository identity from filesystem paths.
Default exact behavior covers the one-positional-only-argument/`None` callable
ABI, implicit/explicit public, private/empty lists, string/list types,
exact/subtree and reserved-label package forms, declaring-repository mappings,
top-level/once restrictions, same-package override, fail-closed direct edges
and observable A/B/A restoration. Internal representation, DICE keys/equality
cutoff/invalidation, first-denial diagnostics and Rust error wrapping are
Slug-native;
`--noexperimental_bzl_visibility`, warning-only `--nocheck_bzl_visibility`,
Java event aggregation, `.scl` and target visibility remain deferred.

Pinned Bazel 9.2 `BazelBuildApiGlobals`, `BzlInitThreadContext`,
`BzlVisibility`, `BzlLoadValue`, `BzlLoadFunction` and the
`BzlLoadFunctionTest.testBzlVisibility_*` family are sole exact authority.
Clean Zabel `0795445f…` `bzl_visibility.zig` and
`engine_bzl_visibility_capture.zig` are concept/test-only peer guidance for
evaluation-scoped capture, immutable retained facts and pure edge checking;
copy no code, layout, parsing, diagnostics or behavior. Buck2 `088c75c7…` is
utility guidance only; existing Slug compact/shared primitives suffice and no
Stage 9 extraction row changes.

Commit `33b7009a2` accepts this reserved design after independent correction
rereview. Run only `WP-4-7A-bazel-bzl-visibility-loading` within the private
loading crate files and existing tests. Prove all five composition sites plus
source and imported-policy A/B/A invalidation, and `REPLAN` for a
starlark-rust change, raw-source scan, path-derived repository, retained
evaluator borrow, missing semantic equality, post-evaluation validation,
ignored flag, new key/lock/cache or public/cross-crate boundary.

## Implementation Slices

### 4.1 Bazel File Discovery

- Discover only `MODULE.bazel`, `BUILD.bazel`, `BUILD`, and `.bzl` files as
  Bazel allows for the chosen compatibility level.
- Do not read `BUCK`, `TARGETS`, `.buckconfig`, or Buck package files.
- Root detection requires `MODULE.bazel` unless an explicit test fixture is
  checking failure behavior.

### 4.2 Load Graph

Implement DICE keys for:

- parsing a `.bzl` file;
- resolving a `load()` label through the current repo mapping;
- evaluating a `.bzl` module with prepared dependencies;
- package-file parsing and package construction;
- package listing with watched directory inputs; individual `glob()` calls are
  pure filters over that prepared listing.

Each key records the file digest or watched directory state that invalidates it.

Initial concrete files:

- `app/slug_loading_v2/Cargo.toml`
- `app/slug_loading_v2/src/{keys.rs,file_discovery.rs,load_label.rs,bzl_module.rs,package.rs,glob.rs,lib.rs}`
- `app/slug_loading_v2/src/globals/{mod.rs,native.rs,package.rs,attr.rs,rule.rs}`
- `app/slug_loading_v2/tests/{build_file_loading.rs,bzl_invalidation.rs,glob_boundaries.rs,native_removed_rules.rs}`

The first DICE keys are `BzlParseKey`, `LoadLabelResolutionKey`,
`BzlModuleEvalKey`, `PackageListingKey`, and `PackageLoadKey`. The reviewed M1
bridge replaces the unused data-only `GlobExpansionKey` scaffold:
`PackageListingKey` owns watched-directory dependencies, while concrete glob
calls are recorded in `LoadedPackage` and resolved synchronously from its
listing. Use `slug_identity_v2` labels and repo mappings only; `CellPath`,
`CellAliasResolver`, and V1 `PackageLabel` do not enter the V2 loading API.

### 4.3 Globals and Native Module

- Start with `load`, `package`, `licenses`, `exports_files`, `glob`,
  `select`, `environment_group`, `package_group`, `filegroup`, and
  `alias` where required by fixtures.
- Implement `rule(implementation=...)`, `provider`, `attr.*`, and `depset`
  via Stage 6-owned APIs but expose enough stubs for loading fixtures.
- Bazel 9 removed language rules from `native`; missing native language rules
  should fail with Bazel-shaped diagnostics rather than invoking V1 native
  implementations.
- Package-owned values replace V1 process-global package registries.

### 4.4 Package Construction

- Package values contain targets, package metadata, loaded module digests, glob
  dependencies, visibility defaults, and repository mapping identity.
- All filesystem reads are tracked through DICE or a watched input registry.
- Package loading should be deterministic under parallel evaluation.


## Checkpoint Evidence

Stage 4 initial loading/file-discovery checkpoint:

- Added oracle fixture placeholders for `build-file-loading` and
  `no-load-native-cc-library` before implementation.
- Added `slug_loading_v2` with Bazel-only workspace root discovery requiring
  `MODULE.bazel`, `BUILD.bazel`/`BUILD` package-file discovery, `.bzl` load-label
  parsing through `slug_identity_v2`, initial DICE key-shaped structs, and stub
  globals for package defaults, attrs, rules, and removed native language rules.
- Local validation passed: `cargo test -p slug_loading_v2`, `py -3 -B
  tools/v2_oracle list`, and the Stage 4 forbidden-surface grep over
  `app/slug_loading_v2` returned no matches.

Stage 4 glob/package-boundary checkpoint:

- Added the `glob-package-boundaries` Bazel oracle fixture. The fixture queries
  `labels(srcs, //pkg:globbed)` for a `glob()`-backed filegroup, proves
  explicit excludes plus subpackage boundaries leave only `keep.txt` and
  `sub/child.txt`, and mutates `allow_empty = False` to capture Bazel 9.1.1's
  per-pattern empty-glob diagnostic for the skipped subpackage pattern.
- Implemented `slug_loading_v2::glob::expand_glob` as a deterministic loading
  substrate using Bazel-style slash paths, sorted traversal, package-boundary
  skipping, watched-directory recording, skipped-subpackage recording, and
  per-include `allow_empty` validation. This is still a substrate; DICE-owned
  package loading must consume the watched inputs before claiming same-daemon
  invalidation parity.
- Local validation passed: `cargo fmt -p slug_loading_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2`;
  `py -3 -B -m tools.v2_oracle list`; `py -3 -B -m tools.v2_oracle run --fixture glob-package-boundaries --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe`;
  bundled `python.exe -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; and `rg -n "BUCK|TARGETS|buckconfig|CellResolver|CellName" app/slug_loading_v2/src` returned no matches.

Stage 4 Bazel oracle refresh:

- Generated Bazel 9.1.1 expected oracle output for `build-file-loading`,
  `load-invalidation`, and `no-load-native-cc-library`. The
  `build-file-loading` query pattern was corrected to match Bazel's observed
  `//pkg:all` behavior: it lists the rule and alias targets, not the exported
  source file target.
- Validation passed: `py -3 -B -m tools.v2_oracle run --fixture build-file-loading --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe`;
  same command for `load-invalidation` and `no-load-native-cc-library`; bundled
  `python.exe -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`.

Stage 4 local `.bzl` DICE loading packet (pending review and commit):

- Replaced the key-shaped `BzlParseKey`, `LoadLabelResolutionKey`, and
  `BzlModuleEvalKey` scaffolding with real DICE keys. `BzlParseKey` owns the
  source read and parse, resolution accepts root-repository `//pkg:file.bzl`
  and package-relative `:file.bzl` forms, and evaluation recursively freezes
  the loaded modules for starlark-rust's `FileLoader`.
- `BzlModuleEvaluator` exposes an explicit file invalidation boundary. The
  focused regression proves a transitive local load is reused until
  `invalidate_path()` marks its parsed source key dirty, then proves the next
  evaluation observes the edited invalid syntax. External-repository loading
  remains rejected until the Stage 5 repository-mapping contract exists.
- Reused retained Buck2-derived DICE `Key`/transaction and starlark-rust
  `AstModule`/`FileLoader` primitives behind V2 workspace paths. The archived
  V1 calculation delegate was reference-only: its Buck cells, package labels,
  and filesystem APIs do not enter this boundary.
- Validation: `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target
  CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2` passed (9 tests), plus
  `cargo fmt --check -p slug_loading_v2` and `git diff --check`.
- This is not package loading or first-build acceptance: `BUILD` globals,
  package construction, repo mappings, Slug CLI wiring, and the
  `build-file-loading` Slug-vs-Bazel oracle comparison remain open.

Stage 4 local package-loading packet (pending review and commit):

- `PackageLoadKey` now evaluates a local `BUILD.bazel`/`BUILD` through the
  same DICE-backed local `.bzl` graph, records `package()` visibility,
  `exports_files`, `filegroup`, `alias`, and targets declared through a
  generic `rule(implementation=...)` callable. The target declaration is the
  boundary: rule implementations, providers, and depsets are explicitly
  loading-only placeholders until Stage 6 configured-target analysis owns
  them.
- The V2 build command maps root-repository single-package patterns into this
  evaluator and reports `dice_starlark_package_loading` before its intentional
  `analysis_not_implemented` result. The existing simple action rule therefore
  proves a real local `.bzl` rule definition and BUILD declaration are loaded,
  without misrepresenting action execution as complete.
- Validation: `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target
  CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 -p slug_core_v2 -p
  slug_cli_v2 --no-fail-fast` passed (14 loading, 2 runtime, and 6 CLI
  integration tests). External repository loading, repo mappings, attributes,
  glob consumption, same-daemon package invalidation, configured-target
  analysis, and execution remain open.
- The package result now has explicit watcher-facing invalidation boundaries:
  `invalidate_path()` dirties a loaded `.bzl` parse key and recomputes its
  dependent package, while `invalidate_package()` dirties a changed local
  BUILD package. Focused regressions prove both transitions in one retained
  evaluator; daemon ownership and filesystem watching remain later work.
- 2026-07-16 Stage 4 daemon ownership landed: `slug_server_v2::Daemon`
  retains the `BzlModuleEvaluator` across builds and performs filesystem
  watching by rescanning `.bzl`/`BUILD.bazel` files and comparing SHA-256
  digests. Changed paths call `invalidate_path`/`invalidate_package` (the
  Stage 4 DICE invalidation boundaries proven above) before each build. The
  `load-invalidation` oracle fixture passes end-to-end (gate clause 5).

### Reviewed next packet — `WP-4-m1-dice-glob-bridge` (2026-07-22)

Work packet ID: `WP-4-m1-dice-glob-bridge`

Owner stage and plan: Stage 4,
`thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`;
consumes the Stage 2 directory boundary landed in `35612655`.

Goal and gate link: make BUILD-file `glob()` consume a DICE-prepared,
subpackage-aware package listing and prove retained-runtime invalidation/reuse.
This is the semantic consumer half of the M1 directory packet. It does not
claim full Bazel glob syntax, symlink, repository, query, or analysis parity.

Prerequisites and oracle:

- `3659b0f9` supplies the single retained workspace DICE transaction;
- `35612655` supplies explicit compact directory values and the demand-driven
  `WorkspaceDirectoryKey`;
- `5ebf8db1` supplies the Bazel 9.2.0
  `glob-directory-invalidation` create/rename/delete oracle generated at Bazel
  commit `8220c6198837d5c13d53fea211cf3282aa12408a`; and
- `app/slug_loading_v2/src/glob.rs` is currently test-only substrate with
  direct filesystem reads, while no production Starlark `glob` global exists.

Reuse audit:

- selectively port Buck2 commit
  `088c75c7e36805df99c3de29062baa95db700b8b`
  `app/buck2_common/src/package_listing/{dice.rs,interpreter.rs,listing.rs}`:
  gather a shared sorted package listing asynchronously through DICE;
- selectively port Buck2
  `app/buck2_interpreter_for_build/src/interpreter/{module_internals.rs,globspec.rs,functions/path.rs}`:
  filter the prepared listing synchronously from the Starlark global;
- retain V2 `WorkspaceDirectoryKey`, `CompactString`, `Arc` slices,
  `starlark_map` sorted collections, `Allocative`, `Dupe`, and DICE
  `ActivationTracker`;
- use Bazel 9.2.0
  `DirectoryListingValue.java`,
  `GlobFunctionWithMultipleRecursiveFunctions.java`, and
  `PackageFunctionWithMultipleGlobDeps.java` as semantic authority. The latter
  explicitly documents why static/two-pass discovery misses dynamic and
  dependent glob calls; and
- reject V1 commit `e218054d4c796655939b968d90208b185decb352`
  globspec, calculation delegate, and watcher beyond their package-listing
  lesson because they import Buck identity, global interpreter state, and
  crawler freshness policy.

Reviewed architecture:

1. Replace the unused data-only `GlobExpansionKey` scaffold with an internal
   `PackageListingKey { workspace, package }`. Its compute recursively requests
   `WorkspaceDirectoryKey`, stops at nested `BUILD.bazel`/`BUILD` boundaries,
   and returns one immutable sorted listing of package-relative files,
   directories, watched directories, and subpackages.
2. `PackageLoadKey` awaits that listing before creating the synchronous
   evaluator. The listing is held by `PackageRecorder` for that evaluator
   lifetime. This deliberately follows Buck2's prepared-listing design; no
   Starlark native call suspends or reaches DICE.
3. Add global `glob()` and `native.glob()` using Bazel's include, exclude,
   `exclude_directories`, and `allow_empty` argument shape. Pattern compilation
   and matching are pure over the prepared listing, results are naturally
   sorted, and used specs are recorded in the loaded package.
4. Remove direct filesystem access from the production glob substrate.
   Directory absence/read errors remain explicit failures. Any symlink that
   could participate as a file, directory, or BUILD boundary produces a
   deterministic unsupported-symlink loading error; it is never followed or
   silently omitted in this packet.
5. `PackageLoadKey` depends on the package listing even for BUILD files that do
   not call `glob()`. This correctness-first Buck2 shape avoids speculative
   Starlark replay. Measure before designing a lazy Bazel-hybrid optimization.

Listing identity is exact for this packet:

- the key uses canonical workspace identity plus a normalized absolute
  root-repository package path;
- the value contains no absolute paths, only sorted package-relative
  `CompactString` paths for regular files, directories, watched directories,
  and nested package boundaries, retained through immutable shared slices;
- every traversed directory requests its `WorkspaceDirectoryKey`;
- a non-root directory with a direct regular entry named `BUILD.bazel` or
  `BUILD` is retained as a package boundary and never traversed below; and
- an absent/read-error value for any required directory fails the listing
  instead of becoming an empty directory.

The Bazel 9.2.0 oracle corrected the exact callable for both global and native
forms to:

```text
glob(include=[], exclude=[], exclude_directories=1, allow_empty=<unbound>)
```

Accept omitted include plus list or tuple string arguments and preserve
Starlark type errors. `allow_empty` is semantically unbound: OSS Bazel 9.2.0
defaults `--incompatible_disallow_empty_glob=true`, so omission behaves as
False; explicit True permits empty matches. The M1 pattern subset is normalized
UTF-8 forward-relative patterns made of literal path segments and `*`
wildcards within a segment, including the existing oracle's `*.txt` and nested
`sub/*.txt` forms. Reject empty patterns, absolute, dot/uplevel,
doubled-separator, trailing-separator, backslash, `**`, `?`, character-class,
brace, and escape forms until a Bazel 9 oracle packet approves their exact
semantics. `exclude_directories=0` filters both regular files and non-boundary
directories; the default filters files only. Per-include
`allow_empty=False` remains required. A `.bzl` macro invoked from a BUILD file
uses that caller's package listing; a top-level `.bzl` call without package
context errors.

The outer `PackageLoadKey` is the only async bridge. Prohibit a nested runtime,
blocking channel, injected input during compute, lock across DICE/evaluator
work, AST-only glob discovery, or speculative Starlark re-evaluation.

Exact scope:

- `app/slug_loading_v2/src/{keys.rs,glob.rs,bzl_module.rs,package.rs,lib.rs}`;
- `app/slug_loading_v2/tests/{glob_boundaries.rs,glob_invalidation.rs}` and
  focused compile fixes;
- `app/slug_core_v2/src/runtime/dice.rs` and
  `app/slug_core_v2/tests/runtime.rs` only if the production-wrapper proof
  needs a narrow API/test change; and
- this plan, the Stage 2 residual, and Stage 9 evidence after acceptance.

Exclude repository mappings, external repositories, symlink resolution,
ignored-path policy, filesystem watcher replacement, `subpackages()`, query,
configured analysis, execution, and unrelated Starlark globals.

Implementation and test order:

1. Add pure package-listing/glob tests for deterministic files/directories,
   excludes, per-pattern `allow_empty = False`, nested package boundaries,
   absence/read errors, unsupported symlinks, and the exact Bazel argument
   surface. Use the generated Bazel 9.2.0 `glob-callable-contract` oracle for
   defaults, sequence inputs, type errors, macro context, and
   `exclude_directories` assertions.
2. Add `PackageListingKey` and compact immutable listing values using only
   injected directory observations.
3. Pass the listing through package evaluation and implement global plus
   `native.glob`; a loaded macro calling `native.glob` must use the BUILD
   package's listing.
4. Add a retained-DICE regression using `ActivationTracker`, not process-global
   atomics. Assert events by concrete key identity and activation kind:
   identical observations do not activate the tested `PackageListingKey` or
   `PackageLoadKey`; an unrelated sibling-package mutation activates and
   reuses both; a file-content mutation below an established subpackage does
   not activate the listing and reuses the package load; matching
   create/rename/delete evaluates the affected directory key, listing key, and
   package key; and adding/removing a child BUILD boundary evaluates the parent
   listing/package exactly once in that committed revision. Assert filegroup
   `srcs` at each semantic transition.
5. Add or strengthen the production `WorkspaceRuntime` regression, then run
   the owning and downstream suites serially through one Cargo target.

Focused validation:

- `CARGO_TARGET_DIR=/tmp/slug-m1-glob-target CARGO_BUILD_JOBS=1 cargo test
  -p slug_loading_v2 -p slug_core_v2 -p slug_server_v2 -p slug_analysis_v2
  -p slug_cli_v2`;
- `cargo fmt --all -- --check`;
- ownership greps showing `glob.rs`, `PackageListingKey`, and Starlark glob
  globals contain no filesystem read, runtime creation, blocking bridge, or
  injected-input mutation;
- forbidden Buck-surface grep from this plan; and
- `scripts/v2_archive_status.sh` plus `git diff --check`.

Evidence and completion boundary: require Sol-low design approval before
implementation and Sol-low post-review before commit. Record exact activation
events, mutation results, utility reuse, validation, accepted commit, and
residual symlink/full-scanner behavior here and in Stage 9. The generated oracle
remains the parity authority until Stage 8 query can execute it under Slug.

Stop on external-repository identity, symlink traversal/resolution, a need for
Stage 5 mapping, evaluator suspension/blocking, a nested runtime, injection
during compute, a lock across DICE/Starlark work, swallowed directory errors,
silent omission of a participating symlink, unsupported pattern behavior
without a Bazel 9 oracle, or inability to distinguish sibling/subpackage reuse
through key-specific activation data.

## WP-4-8-M3-A: Load-Provenance Fake-Target Substrate

Status: reviewed next direction; no implementation evidence.

Goal: supply the loading-owned provenance required by Stage 8 `buildfiles()`
and `loadfiles()` without activating either query function. This is gate A of
parent packet `WP-4-8-m3-build-load-files`; gate B may start only after Sol
accepts A and the single combined Bazel 9.2 fixture is already committed.

Use Bazel `8220c6198837d5c13d53fea211cf3282aa12408a` as authority:
`src/main/java/com/google/devtools/build/lib/query2/common/AbstractBlazeQueryEnvironment.java`
(`transitiveLoadFiles`), `query2/compat/FakeLoadTarget.java`, and the
`BuildFilesFunction`/`LoadFilesFunction` engine classes. The compatibility
tests in `src/test/java/com/google/devtools/build/lib/query2/testutil/AbstractQueryTest.java`
cover transitive loads, broken companions, fake-target composition, and active
BUILD basename behavior.

The Stage 4 portion of A must preserve, without command-owned scans:

- canonical root label and physical path;
- direct loaded children and a deterministic transitive fingerprint in compact
  immutable `Arc` slices;
- `LoadedPackage` access to its BUILD's direct roots and reachable closure; and
- enough separate ownership to keep every relevant `FrozenModule` alive.

Stage 8, not `LoadedPackage`, owns the request-local fake-node table and the
candidate consuming package for each printed label. It must distinguish
zero-edge fake nodes from real package-graph nodes and preserve enough
`(printed label, consuming package, real/fake)` provenance for the combined
oracle to establish the winner through each function and set composition.
Do not preselect a request-global first-owner rule.

`LoadedPackage::PartialEq` must include the BUILD direct-root set and the
transitive manifest identity/fingerprint, so a direct load-edge change or
transitive `.bzl` content change recomputes package/query state even when
declared targets are unchanged. Frozen-module pointers and lifetime-only
storage remain excluded from semantic equality.

It reuses `BzlParseKey`, `BzlModuleEvalKey`, existing load-label resolution,
`PackageLoadKey`, `PackageListing`, and injected workspace observations.
Actual companion BUILD basename discovery must be parse-independent and must
not compute `PackageLoadKey` for the load-label package. A new key, cache,
lock, or filesystem seam is a reserved Sol decision. Do not put fake `.bzl`
or companion BUILD nodes in `UnconfiguredPackageGraph`, global `QueryLabel`
identity, `:all`, recursive patterns, or dependency edges. Function-produced
fake targets remain directly queryable inside their expression and have zero
edges, so `deps(fake)` contains only that target. The future query projection
is otherwise graphless except for real operand-evaluation edges, so FULL
formatting cannot invent fake-to-load or fake-to-BUILD edges.

Oracle-first matrix shared with gate B:

- direct and shared-transitive `.bzl` loads; `.bzl` cycles are failures, never
  a success claim;
- primary/fallback/root BUILD basenames, dual-file priority, and a broken
  syntax or broken `load()` in a loaded label's containing-package BUILD,
  whose basename still appears without a successful `PackageLoad`;
- `buildfiles` membership for the selected package BUILD, every transitive
  load label, and every load-label package's active BUILD companion;
  `loadfiles` excludes every BUILD companion;
- real/fake `siblings` operands in both orders, label-first uniqueness, and
  consuming-package provenance;
- duplicate/empty/multi-package arguments, set operations, zero-edge
  `deps(fake)`, AUTO/FULL order, missing/malformed/unsupported inputs, and
  empty stdout on failures; and
- exact DICE and same-daemon create/edit/delete/recreate transitions for a
  leaf `.bzl`, a direct load edge, and BUILD/BUILD.bazel replacement.

The equality gate must prove that a leaf content edit and direct/transitive
load-edge changes invalidate the owning `PackageLoad`/query through manifest
equality, without comparing retained `FrozenModule` pointer identity.

Hard stops: external mapping, `.scl` silent omission, direct filesystem reads,
global identity rewrites, whole-workspace traversal, a lifetime shortcut that
drops `FrozenModule`, synthetic rendering edges, or any attempt to treat a
`.bzl` cycle as successful.

Accepted implementation evidence (2026-07-22):

- Accepted implementation commit: `de835cdc` (`feat: add DICE-backed Starlark
  glob`).
- `PackageListingKey` now recursively consumes only
  `WorkspaceDirectoryKey`, stops before traversing nested packages, and returns
  sorted immutable package-relative `CompactString` slices for files,
  directories, watched directories, and subpackages. Required absence/read
  errors fail explicitly; participating symlinks and special entries produce
  deterministic unsupported-entry errors.
- `PackageLoadKey` is the sole asynchronous bridge. It prepares the listing
  before synchronous Starlark evaluation; production glob expansion performs
  no filesystem reads, starts no runtime, blocks on no channel, and mutates no
  injected input.
- Global `glob()` and `native.glob()` share the prepared listing, including
  calls made by a loaded macro in BUILD package context. The Bazel 9.2.0
  include/exclude defaults, `exclude_directories`, unbound `allow_empty`
  behavior, list/tuple inputs, per-include empty checks, and reviewed M1 pattern
  subset are covered by focused tests. Loaded packages retain the used compact
  glob specs.
- The retained-DICE regression distinguishes an untouched cached key from
  dependency-validation reuse. Identical observations activate neither listing
  nor package load; an unrelated sibling mutation reuses both; a content edit
  below an established subpackage leaves the listing untouched and reuses the
  load. Matching create/rename/delete and adding/removing a child BUILD
  boundary evaluate the affected listing/load identities and produce the
  expected filegroup sources.
- Root validation passed
  `CARGO_TARGET_DIR=/tmp/slug-m1-glob-target CARGO_BUILD_JOBS=1 cargo test
  -p slug_loading_v2 -p slug_core_v2 -p slug_server_v2 -p slug_analysis_v2
  -p slug_cli_v2`, `cargo fmt --all -- --check`, ownership/forbidden-surface
  greps, and `git diff --check`. `scripts/v2_archive_status.sh` retains the
  known absent local `v1-archive` branch and broad path-matcher failures; this
  packet introduced neither.
- Sol-low post-review accepted the implementation with no blockers after the
  root corrected the initial test assumption that every untouched cache hit
  emits `ActivationData::Reused`.
- Residual: the migration observation adapter still scans the workspace,
  symlink resolution and full Bazel glob syntax remain unsupported, and
  repository-aware listing identity waits for Stage 5. The Bazel oracle fixture
  remains authoritative until Slug query can execute it.

## Exact Test Criteria

- Oracle `build-file-loading` fixture covers `exports_files`, `filegroup`,
  `alias`, package default visibility, and a loaded macro creating a target.
- Oracle `bzl-load-invalidation` fixture runs build, edits a loaded `.bzl`,
  rebuilds in the same daemon, and observes changed package output.
- `glob` fixture verifies create/delete/rename inside a package invalidates
  package loading and crossing into a subpackage is rejected.
- Negative fixture for no-load `native.cc_library` fails with Bazel 9-style
  removed-rule diagnostics.
- `rg -n "BUCK|TARGETS|buckconfig|CellResolver" <v2-loading-crates>` returns no
  production-path matches.

## Acceptance Criteria

- Small `BUILD.bazel` packages load under V2 and match Bazel oracle output.
- `.bzl` load graphs invalidate when loaded files change.
- No BUCK/TARGETS file discovery remains in the V2 loading path.
- Error shapes are close enough for modern rulesets and exact where tests
  require exactness.

## Validation

```bash
cargo test -p slug_loading_v2
slug-v2-oracle run --fixture build-file-loading
slug-v2-oracle run --fixture bzl-load-invalidation
slug-v2-oracle run --fixture glob-package-boundaries
slug-v2-oracle run --fixture no-load-native-cc-library
rg -n "BUCK|TARGETS|buckconfig|CellResolver|CellName" app/slug_loading_v2
```

### Build/load provenance oracle evidence (2026-07-23)

`8f6f02b3` landed the shared base 58-row fixture; `e8014b25` corrects it to
64 Bazel 9.2 `query-build-load-files-provenance` rows by isolating a singleton
fake target. Update `051423-694832`, Terra clean `051521-700085`, and root
clean `051644-705470` passed; Sol-low returned `ACCEPT`. It proves companion
BUILD basenames are discoverable through broken syntax/load packages without
successfully loading them, and separately evaluated load functions can
associate one printed fake `.bzl` label with different consumers.

The `BinaryOperatorExpression` `evalPlus`/`evalMinus`/`evalIntersect`,
`QueryUtil` label-key set, `TargetKeyExtractor`, and `SiblingsFunction` show
that intersection retains the left representative, equal-label `except`
removes in both directions, and union sends both callback batches to siblings.
The earlier fake-left survivor is unmatched transitive `two.bzl`, not an
asymmetric real/fake operation. This oracle evidence itself implements no
additional Stage 4 substrate. Gate A must preserve `(printed label, consuming
package, real/fake)`, retain `seenBzlLabels` label dedup only within an
invocation, and use no request-global winner. Factored FULL uses
`--output=graph --graph:factored` and confirms zero fake edges/no synthetic
projection edges. Nine ordinary functions remain deferred.

### Gate A Stage 4 half accepted (2026-07-23)

`b0670e33` (`feat: retain load provenance manifests`) accepts only the Stage 4
half of Gate A. Public `BzlLoadManifest`/`BzlModuleIdentity` hold canonical
label + normalized path, source-order label-first direct identities,
first-seen closure, and `[u8; 32]` SHA-256. `LoadedPackage` direct roots,
reachable closure, and fingerprint are semantic equality: BUILD
comment/format-only edits remain equal; leaf/direct/transitive load-edge
create-delete-recreate changes then restores the value. An aligned
`FrozenBzlLifetimeEntry` structurally retains each transitive `FrozenModule`
outside Eq; identity/path are `Allocative`-accounted and opaque frozen modules
are skipped.

The companion helper reuses only `WorkspaceDirectoryKey`: primary before
fallback, regular/symlink acceptance, missing `None`, explicit read error,
broken-BUILD parse independence, and shared normalized-path validation. It
adds no key/cache/lock/filesystem/package-load seam. Worker/root loading tests
covered 27 integrations (worker's 26 omitted pre-existing `native_removed`);
root also passed analysis 11 and query 22 integrations. Sol-low accepted
symlink, validator, alignment, lifecycle/non-over-invalidation, and memory
accounting corrections. Stage 8 algebra and registry activation remain
pending; nine functions remain deferred.

## WP-4-8-m3-labels-metadata-foundation: Stage 4 Gate A (2026-07-23)

Authoritative next packet; oracle/implementation pending; Gate A activates no
query function. Replace `RuleDefinitionGen::has_deps` with a V2-owned ordered
immutable schema. Each entry retains declaration/query names (`_foo`/`$foo`),
an extensible exact attribute-kind enum, mandatory/default/configurability
state, and dependency reachability. Each `PackageTarget` rule instance retains
an ordered immutable value map with `Explicit | Default | Implicit`
provenance. Coerced values preserve scalar labels, label lists, explicit
non-label kinds, and `select()` branches/default/accepted concatenation;
canonicalize labels during package construction. Do not flatten the structure
to dependencies or a reachable-label list. Add exact output and output-list
attribute kinds: their coerced values create generated-file package targets
with retained declaring-rule ownership. Do not infer output identity or edges
from filenames.

Use compact `SmallMap`/`SmallSet` and immutable shared slices/strings where
appropriate, derive `Allocative`, and include schema, provenance, value
structure, selector order/defaults, and labels in `LoadedPackage` equality
(never frozen-module pointer identity). Prefer a new
`app/slug_loading_v2/src/attrs.rs` owned by the package loader.

Oracle: Bazel 9.2 `BlazeTargetAccessor#getPrerequisites` and
`AggregatingAttributeMapper#getReachableLabels`, and
`AbstractQueryTest#testLabelsOperator` at `8220c619…`: `attr.label`,
`attr.label_list`, explicit/omitted/empty/mandatory values, absent and existing
non-label attrs, `_implicit` versus `$implicit`, every configurable branch and
default, accepted selector concatenation, `attr.output`/`attr.output_list`,
duplicate/source/generated/cross-package labels, and missing prerequisites.
Fixture evidence decides whether condition keys enter each attribute's
reachable-label contract and fixes generated target kind, owner, and graph
edges.

Same-daemon edits cover explicit BUILD values, `.bzl` schema/default and
implicit-default changes, selector branch/default/type/name changes, deletion
and recreation, and unrelated/non-semantic edits. They prove the existing
`BzlModuleEvalKey` → `PackageLoadKey` → package-graph invalidation path; add no
DICE key, direct read, global interner, or configuration evaluator. Stop on
coercion/provenance ambiguity, unsupported exposed label-bearing forms, or any
attempt to evaluate only a guessed/default selector branch. A generated target
representation beyond the exact output/output-list surface required here is a
stop. Buck2 attribute files supply utility/traversal shapes only; reject Buck
cell, select, attr, and provider semantics. V1 `labels` is
unimplemented/reference-only.

`8dfae99c` accepts immutable 31-row Bazel evidence: all seven default public
label-bearing attrs, dormant exclusion, select-key false, valid dedup, two
output producers with distinct generated-file owners/edges, and fail-fast
missing/mandatory errors. 29 normal rows are future CLI evidence; two
label-kind rows require focused `QueryNodeKind::GeneratedFile` tests first.

### Gate A implementation accepted (2026-07-23)

`1b7c179c` lands the substrate without activating `labels`: ordered immutable
`Allocative` seven-label-kind-plus-String schemas/values, exact
defaults/configurability/provenance/select structure, canonical generated
identity/owner, outputs excluded from ordinary deps, and semantic equality.
Same-DICE tracker covers `BzlModuleEval`→`PackageLoad`→consumer/observer; a
preactivation query guard forbids leakage. Root passed fmt/diff, loading
35/query 39/analysis 11. Sol's six initial blockers were corrected; rereview
`ACCEPT`, including root's nested repeated-prefix order regression. Next Stage
8 is limited to 29 CLI rows plus two generated-kind assertions.

`f3e8ad48` is accepted as a narrow fixture prerequisite: native
`config_setting(values=...)` retains sorted compact values, is a zero-edge
`config_setting rule`, reuses reordered-equal maps, and invalidates on value
changes. It is
load-only, does not evaluate configuration, and fails closed for unsupported
attrs. Sol `ACCEPT`. Define/flag/constraint/common attrs and matching remain
deferred; Stage 8 labels resumes without changing its 29+2 claim.

Stage 8 `8fec2696` consumes this substrate through compact immutable
`Allocative` QueryNode attrs separate from deps: all selector branches/default,
not keys, and only output→own-generator generated edges. Same-DICE semantic/
reuse and same-daemon metadata transitions passed; no loading scope expands.

## `attr` typed attribute-string design replanned (2026-08-09)

`WP-4-8-m3-attr-typed-attribute-string-design` ends in `REPLAN` without Rust,
Cargo, fixture, or oracle changes. Pinned Bazel 9.2 source closes leaf
formatting, but current V2 has already discarded observable ordering and value
facts needed by the complete function.

`TargetUtils.getAttrAsString` visits every possible effective value and
stringifies the whole typed result: strings are unquoted; null is suppressed;
lists use `[a, b]`; dicts use `{key=value}`; labels use `//pkg:t` in the main
repository and `@@canonical//pkg:t` externally. Selector concatenation is
typed before formatting. Equal key sets are correlated, distinct sets form
cross-products, candidate duplicates remain, and entry/default position affects
observable order. Boolean/tristate integer compatibility is source-exact but
not currently admitted by V2 and must not be synthesized.

Current `CoercedAttributeValue` detaches the default branch from its original
ordered selector entry, while some string/native lists and maps are normalized
before any attr projection. `QueryAttribute` then drops all non-label values,
empty typed containers, dict keys/value shape, schema defaults, and
concatenation. Native/synthetic rules and the universal `name` attribute also
lack a total typed projection. A Starlark-only activation would therefore be
false default-function coverage.

Run next only `WP-4-8-m3-attr-candidate-order-oracle-design`. It must design a
focused Bazel 9.2 fixture for default-first/middle/last order, equal versus
distinct selector key sets, typed string/list concatenation with duplicates,
all admitted dict orientations, effective default/implicit values, universal
`name`, and every currently admitted native rule attribute. It must also freeze
the pre-normalization capture point and canonical label renderer. No production
representation or query activation is authorized until that evidence is
accepted.

## `attr` candidate-order oracle design replanned (2026-08-09)

`WP-4-8-m3-attr-candidate-order-oracle-design` reaches its required `REPLAN`
stop without fixture, oracle, Rust, Cargo, or JVM work. The requested ordinary-
query discriminator does not exist. `TargetUtils.getAttrAsString` does expose
`AggregatingAttributeMapper.visitAttribute` candidates in an internal order,
but `RegexFilterExpression` consumes them only through a side-effect-free
existential test and returns the target after the first match. Query output
contains target labels, never the matched candidate, its position, or its
multiplicity. Reordering candidates, moving the default entry, or deduplicating
equal candidates therefore cannot change an ordinary `attr()` result.

This is an observable-semantics correction, not a relaxed Bazel-parity claim.
Slug must still preserve order and duplicate elements *inside one formatted
typed value*, and it must reproduce equal-key-set correlation, distinct-key-set
cross-products, typed concatenation before formatting, null suppression,
schema/default/implicit values, canonical label strings, and the complete
rule-owned native attribute surface. It does not need to preserve an
unobservable candidate traversal order or duplicate equal candidates. A
default-first/middle/last CLI matrix can prove membership only and cannot
authorize an order representation or request-local ordered traversal.

The finite current native inventory is also wider than the earlier query
projection: `filegroup`, `alias`, `config_setting`, `test_suite`,
`constraint_setting`, `constraint_value`, `platform`, `toolchain_type`, and
`toolchain` are loadable rules. `package_group`, exported/source/BUILD files,
and generated files are not rules for `attr`; the universal `name` attribute
must nevertheless cover every rule class. Current query graph construction
rejects `NativeToolchainTarget`, so total-native activation requires a reviewed
graph-projection prerequisite rather than a Starlark-only formatter patch.

Run next only `WP-4-8-m3-attr-observable-candidate-oracle-design`. It must
replace the impossible order/multiplicity rows with paired positive and
negative rows for the observable candidate *set*: equal-key correlation versus
cross-product, typed string/list concatenation, duplicate elements within one
list value, list/map order before current normalization, every admitted dict
orientation, empty/null/effective defaults, `$implicit`, universal/native
`name`, all retained native attributes, and main/external canonical labels. It
must decide under the ordinary fixture-growth rules whether extending
`query-labels-attribute-metadata` or adding one isolated fixture is smaller.
Freeze a compact typed loading representation and request-local existential
early-exit traversal only after that evidence; authorize neither here.

## `attr` observable-candidate oracle design replanned (2026-08-09)

`WP-4-8-m3-attr-observable-candidate-oracle-design` reaches `REPLAN` before a
fixture design is accepted. The narrowed phrase “retained attributes” is not a
truthful boundary for Bazel's default `attr()` function. `AttrFunction` asks the
rule for its complete `RuleClass` attribute definition, so inherited, hidden,
computed-default, and automatically populated attributes are observable even
when V2's BUILD callable neither accepts nor retains them.

Every current native rule begins with `name` and the `NativeBuildRule` family:
`visibility`, `transitive_configs`, `deprecation`, `tags`, the three
`generator_*` strings, `testonly`, `features`, `:action_listener`,
`compatible_with`, `restricted_to`, `$config_dependencies`,
`package_metadata`, `aspect_hints`, `licenses`, `distribs`, and
`target_compatible_with`. Derived classes remove and override different
subsets. They also add more than the V2-carried fields: for example,
`filegroup` has `data`, `output_group`, and `output_licenses`;
`config_setting` has three dictionaries, two label lists, and a late-bound
alias list; platform/toolchain classes have parents, flags, settings,
compatibility, error text, dictionaries, and booleans. `test_suite` overrides
`testonly` to true. Config, constraint, platform, and toolchain classes override
`tags` to `[manual]`.

The same issue applies to current Starlark rules. Their base class adds
`expect_failure`, `toolchains`, execution properties/constraints, and common
attributes. Executable and test bases add further fields; tests include size,
timeout, booleans, shard count, args, and loading-time `@bazel_tools` label
defaults. Legacy macro calls can also populate `generator_*` values. These are
ordinary loading-query observations, not configured analysis. In particular,
BOOLEAN formatting is now required: `TargetUtils.convertAttributeValue`
renders `testonly` and other booleans as `0`/`1`, disproving the prior claim
that no currently admitted type needs that compatibility path.

The inventory is finite, but a 34-row retained-field fixture—or a mechanically
expanded 46-row draft without a closed schema ledger—would still omit real
queries such as `attr(testonly,^0$,//:fg)`, the `[manual]` `tags` value on a
platform, and the test-base hidden-label defaults. No fixture choice or row
count is accepted until removals, overrides, null suppression, computed and
late-bound loading defaults, order-independent normalization, macro provenance,
and canonical built-in labels are mechanically complete.

Run next only `WP-4-8-m3-attr-total-ruleclass-schema-source-ledger-design`.
Using pinned Bazel 9.2 source, it must enumerate the exact attr-visible schema
and loading value source for current normal/executable/test Starlark rules and
all nine current native rule classes; partition fields into shared typed/default
equivalence classes; identify every per-class removal/override and every V2
capture gap; and specify which finite discriminators the later observable-
candidate oracle must generate. It must preserve the accepted unobservable-
candidate-order boundary and isolate native-toolchain graph projection as a
later prerequisite. No fixture, oracle record, representation, query
activation, `@bazel_tools` content, JVM/Java artifact, or Rust change is
authorized.

## `attr` total RuleClass schema source ledger (2026-08-09)

`WP-4-8-m3-attr-total-ruleclass-schema-source-ledger-design` closes the finite
loading schema without fixture, oracle, Rust, Cargo, graph, DICE, JVM, Java
artifact, or production Bazel work. All facts below come from immutable Bazel
9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`; the neighboring
checkout's different HEAD is not evidence.

### Accessor and renderer closure

`RuleClass.Builder` installs mandatory nonconfigurable
`name: STRING_NO_INTERN` at index zero. `TargetUtils.getAttrAsString` finds the
final RuleClass definition, visits every possible whole typed loading value
through `AggregatingAttributeMapper`, and applies
`TargetUtils.convertAttributeValue`. BOOLEAN becomes `0` or `1`; null
contributes no candidate; everything else uses typed `toString()`. Strings are
unquoted, integers are decimal, lists are `[a, b]`, maps are `{k=v}`, the
no-license value is `[none]`, main labels are `//pkg:t`, and external labels,
including tools labels, use canonical `@@repo//pkg:t` spelling.

`BuildType.convertFromBuildLangType` sorts order-independent lists once. Other
list and map order remains inside the formatted whole value. That interior
order and duplicate elements remain observable; candidate traversal order and
multiplicity of equal whole candidates remain unobservable through the
existential filter and are not a Slug contract.

Computed values here are loading inputs. `deprecation`, `testonly`, and
`package_metadata` read package arguments; absent deprecation is null and is
suppressed. Test `timeout` reads `size` and maps recognized sizes to `short`,
`moderate`, `long`, or `eternal`, otherwise `illegal`. `Rule` derives the three
`generator_*` strings from legacy-macro call-stack state, or `""` outside a
macro. `visibility` is the declared list or effective package default through
a dedicated mapper path. Native `licenses` uses the package license unless the
class ignores licenses. `test_suite.$implicit_tests` is populated during
package finalization when `tests` is absent or empty.

Late-bound does not imply configured-query input. The ordinary loading mapper
calls `LateBoundDefault.getDefault(rule)`, never its configuration resolver.
Thus `:action_listener` and `config_setting.:flag_alias_settings` have loading
fallback `[]`; test coverage attrs use fixed tools-repository fallbacks; and
the two test `:run_under_*` attrs have null loading fallbacks and disappear.
No configured analysis or host JVM state is needed.

### Shared schema algebra

`N` means nonconfigurable, `C` configurable, and `OI` order-independent. The
common core `K` has these exact 16 attrs, including `name`:

| Query spelling | Type / flags | Loading default or source |
| --- | --- | --- |
| `name` | `STRING_NO_INTERN`, N | mandatory target name |
| `visibility` | `NODEP_LABEL_LIST`, N/OI | declared list or package default (normally private) |
| `transitive_configs` | `NODEP_LABEL_LIST`, N/OI | `[]` |
| `deprecation` | `STRING`, N | package default; null-suppressed when absent |
| `tags` | `STRING_LIST`, N/OI | `[]` unless overridden below |
| `generator_name` | `STRING`, N | legacy-macro generator name or `""` |
| `generator_function` | `STRING`, N | interior macro function or `""` |
| `generator_location` | `STRING`, N | relative macro location or `""` |
| `testonly` | `BOOLEAN`, N | package default, normally false |
| `features` | `STRING_LIST`, C/OI | `[]` |
| `:action_listener` | `LABEL_LIST`, C/late-bound | loading fallback `[]` |
| `compatible_with` | `LABEL_LIST`, N | `[]` |
| `restricted_to` | `LABEL_LIST`, N | `[]` |
| `$config_dependencies` | `LABEL_LIST`, N | selector-key labels, otherwise `[]` |
| `package_metadata` | `LABEL_LIST`, N | package default, normally `[]` |
| `aspect_hints` | `LABEL_LIST`, C | `[]` |

The native baseline is
`NATIVE = K + {licenses: LICENSE N, distribs: STRING_LIST N,
target_compatible_with: LABEL_LIST C}`. `licenses` is the package license and
the lists default to `[]`. `exemptFromConstraintChecking()` removes exactly
`compatible_with`, `restricted_to`, and `target_compatible_with`.

The Starlark base is
`STARLARK = K + {expect_failure: STRING C, toolchains: LABEL_LIST C,
exec_properties: STRING_DICT C, exec_compatible_with: LABEL_LIST N,
exec_group_compatible_with: LABEL_LIST_DICT N,
target_compatible_with: LABEL_LIST C}` with defaults `""`, `[]`, `{}`, `[]`,
`{}`, and `[]`. It does not inherit native `licenses` or `distribs`.

| Current Starlark shape | Built-in count | Delta from `STARLARK` |
| --- | ---: | --- |
| normal | 22 | none |
| executable | 25 | C `args: STRING_LIST=[]`, C `output_licenses: STRING_LIST=[]`, N `$is_executable: BOOLEAN=true` |
| test | 39 | override `testonly=true`; add N `size: STRING="medium"`, N computed `timeout: STRING="moderate"`, N `flaky: BOOLEAN=false`, C `shard_count: INTEGER=-1`, N `local: BOOLEAN=false`, C `args: STRING_LIST=[]`, the ten hidden entries below, and N `$is_executable: BOOLEAN=true` |
| root string build setting | 24 | mandatory N `build_setting_default: STRING`; N `help: STRING=""` |

The pinned OSS provider never calls `setNetworkAllowlistForTests`, so its
conditional test `$whitelist_external_network` attr is absent. The test hidden entries are
`$test_wrapper=@@bazel_tools//tools/test:test_wrapper`,
`$xml_writer=@@bazel_tools//tools/test:xml_writer`,
`$test_runtime=[@@bazel_tools//tools/test:runtime]`,
`$test_setup_script=@@bazel_tools//tools/test:test_setup`,
`$xml_generator_script=@@bazel_tools//tools/test:test_xml_generator`,
`$collect_coverage_script=@@bazel_tools//tools/test:collect_coverage`,
`:coverage_support=@@bazel_tools//tools/test:coverage_support`,
`:coverage_report_generator=@@bazel_tools//tools/test:coverage_report_generator`,
and null-suppressed `:run_under_exec_config` and
`:run_under_target_config`. A rule with a Starlark-defined transition also
gains configurable
`$allowlist_function_transition: LABEL=@@bazel_tools//tools/allowlists:function_transition_allowlist`.

User declarations extend these sets. Current V2 exposes `attr.label`,
`label_list`, `string_keyed_label_dict`, `label_keyed_string_dict`,
`label_list_dict`, `output`, `output_list`, and `string`; private `_x` is queried
as `$x`. Intrinsic defaults are null for scalar label/output, `""` for string,
and typed empty containers. Output types are nonconfigurable; the others are
configurable in Bazel 9.2. V2 currently accepts a `configurable=` descriptor
argument that pinned Bazel rejects, so that syntax is a parity gap, not an
additional Bazel schema family. Mandatory state, explicit defaults,
explicit/default/implicit provenance, selector correlation, and typed
concatenation remain part of the whole-value contract.

### Final native RuleClass sets

Counts include `name`; each row is the exact delta from `NATIVE`.

| Rule class | Count | Final delta |
| --- | ---: | --- |
| `filegroup` | 23 | add C `srcs: LABEL_LIST=[]`, `output_group: STRING=""`, `data: LABEL_LIST=[]`, `output_licenses: STRING_LIST=[]` |
| `alias` | 17 | remove `licenses`, `distribs`, `:action_listener`; add mandatory C `actual: LABEL` |
| `config_setting` | 21 | remove the three constraint attrs; override `tags=[manual]`; force `licenses=[none]`; add N `values: STRING_DICT={}`, `define_values: STRING_DICT={}`, `flag_values: LABEL_KEYED_STRING_DICT={}`, `constraint_values: LABEL_LIST=[]`, late-bound `:flag_alias_settings: LABEL_LIST=[]` |
| `test_suite` | 21 | override `testonly=true`; add N/OI `tests: LABEL_LIST=[]` and automatic N/OI `$implicit_tests: LABEL_LIST=[]` |
| `constraint_setting` | 16 | remove the three constraint attrs, `:action_listener`, `package_metadata`; override `tags=[manual]`; add nullable N `default_constraint_value: NODEP_LABEL` and `refines_constraint_value: LABEL` |
| `constraint_value` | 15 | same five removals and manual tag; add mandatory N `constraint_setting: LABEL` |
| `platform` | 23 | same five removals and manual tag; add N `constraint_values: LABEL_LIST=[]`, `parents: LABEL_LIST=[]`, `remote_execution_properties: STRING=""`, `exec_properties: STRING_DICT={}`, `flags: STRING_LIST=[]`, `missing_toolchain_error: STRING="For more information on platforms or toolchains see https://bazel.build/concepts/platforms-intro."`; add C `required_settings: LABEL_LIST=[]`, `check_toolchain_types: BOOLEAN=false`, `allowed_toolchain_types: NODEP_LABEL_LIST=[]` |
| `toolchain_type` | 17 | remove `licenses`, `distribs`, `:action_listener`; add N `no_match_error: STRING=""` |
| `toolchain` | 21 | remove the three constraint attrs and `:action_listener`; override `tags=[manual]`; re-add N `target_compatible_with: LABEL_LIST=[]`; add mandatory N `toolchain_type: LABEL`, N `exec_compatible_with: LABEL_LIST=[]`, N `use_target_platform_constraints: BOOLEAN=false`, C `target_settings: LABEL_LIST=[]`, mandatory C `toolchain: NODEP_LABEL` |

Exported/source/BUILD/generated files and package groups are not rules and have
no attr-visible RuleClass.

### V2 losses and finite oracle classes

The first loss is native call recording. It retains only `filegroup.srcs`,
`alias.actual`, sorted `config_setting.values`, sorted
`test_suite.tests`/`tags` plus implicit members, narrow constraint/platform
fields, and selected toolchain labels. It drops nearly every common, package,
macro, fixed-default, and class-specific field. Starlark recording retains user
schema/value structure but only `tags`, test `size`, and root
`build_setting_default` among built-ins. The
later `QueryAttribute {name, labels, explicit}` projection discards every
non-label value, typed empty container, map orientation, universal `name`, and
most native fields. Root graph construction rejects `NativeToolchainTarget`
before projection.

Capture is bounded at rule-call/package-finalization time before normalization
or reduction to `PackageTargetKind`. Package state must retain default
visibility/deprecation/testonly/metadata/license; macro state must retain the
three generator values; and automatic implicit tests remain package-finalized.
These are loading inputs, not configured analysis.

The next oracle design must assign paired membership/nonmembership rows to:

1. string/decimal/BOOLEAN `0` and `1`/license renderers, empty versus nonempty
   containers, and null suppression;
2. ordered list/map interiors versus OI sorting, duplicates inside one value,
   every admitted dict orientation, and canonical main/external/tools labels;
3. equal-key selector correlation, distinct-key cross-product, string/list
   concatenation before rendering, effective defaults, and `$implicit` names;
4. universal `name`, package defaults, legacy macro generator provenance, test
   timeout, fixed/late-bound test labels, and automatic `$implicit_tests`;
5. the five native inheritance families: full baseline, alias/toolchain-type
   removals, constraint-exempt config setting, constraint-defining removals,
   and toolchain's reintroduced `target_compatible_with`.

Pinned anchors are `RuleClass.java`, `BaseRuleClasses.java`,
`StarlarkRuleClassFunctions.java`, `AttributeProvider.java`, `Rule.java`,
`AggregatingAttributeMapper.java`, `AbstractAttributeMapper.java`,
`BuildType.java`, `TargetUtils.java`, and the nine native RuleDefinition files,
all at commit `8220c619...`.

Run next only
`WP-4-8-m3-attr-observable-candidate-oracle-design-retry`, a Stage 4/Stage 8
documentation and fixture-design packet derived from this ledger. It must pick
the smallest fixture arrangement, cover every equivalence class and native
exception, and isolate native-toolchain graph projection as a later
prerequisite. It must not generate fixtures, activate `attr`, freeze a
production representation, broaden the graph, or add Rust, Cargo, DICE, JVM,
Java source/bytecode/helper, or production Bazel delegation.

## `attr` observable-candidate oracle design retry (2026-08-09)

`WP-4-8-m3-attr-observable-candidate-oracle-design-retry` selects an extension
of the existing `query-labels-attribute-metadata` fixture. An isolated fixture
would duplicate its root module, Starlark rule definitions, selector keys,
package-context labels, dictionary shapes, filegroup/alias targets, payload
projection, and harness metadata. The accepted workspace currently has 39
Bazel rows, eight directories, eleven regular files, 127 logical source lines,
zero mutations, and one canonical payload projection. All 39 existing rows
remain protected.

The extension adds one isolated `attr/BUILD.bazel` package inside that same
workspace and one local external module under `modules/ext` with only
`MODULE.bazel` and `leaf/BUILD.bazel`. It edits the root `MODULE.bazel` to add
`bazel_dep(name="ext", version="1.0")` plus the relative local override, and
extends existing `pkg/defs.bzl` with the normal, executable, test, root string
build-setting, macro, selector, dictionary, and Starlark-transition probe
definitions. Existing `pkg` targets remain the empty/default-package controls.
The new `attr` package owns all positive package-default state and all nine
native representatives. No copied repository tree, action, source artifact,
mutation, toolchain resolution, configured query, or generated `@bazel_tools`
content is needed.

### Paired command construction

The later fixture adds exactly 18 ordinary-query commands, for 57 total. Each
command is a union of independently observable atoms of the form:

```text
attr(<query spelling>, "^<escaped whole value>$",
     set(//attr:<case>_yes //attr:<case>_no))
```

Every atom receives a distinct positive and negative rule instance, and no
label is the positive result of two atoms in the same command. Exact stdout
must enumerate every `_yes` label once and no `_no` label. This prevents union
deduplication from masking a missing positive or a false match. An atom may
instead compare a shared-schema representative with a class whose attr was
removed; the expected set then names only the representative. All patterns are
anchored. Literal list/map brackets and regex punctuation are escaped in TOML.

| Lane | Exact observable atoms |
| ---: | --- |
| 1 | `name` matches each of the four Starlark shapes and nine native classes by its exact target name; `attr(name,"^.*$",...)` rejects an existing source file, generated file, and package group as nonrules. |
| 2 | `expect_failure="boom"`, `shard_count=-1`, BOOLEAN `testonly=0/1`, native package `licenses=[notice]`, config-setting `licenses=[none]`, and package `deprecation="deprecated"`; the paired no-deprecation target is probed with `^.*$` and must remain absent because null is not an empty string. |
| 3 | Existing empty candidates match `^$`, `^\\[\\]$`, and `^\\{\\}$`; scalar label/output null defaults match no regex; an explicit nullable-label counterpart matches its canonical label; `$private` matches while `_private` does not. |
| 4 | Ordered `args=[z, z, a]` rejects `[a, z, z]`; OI `tags`/`features` declared as `[z, a, z]` match `[a, z, z]` and reject declaration order; ordered `STRING_DICT` matches `{z=1, a=2}` and rejects the reverse. |
| 5 | Whole rendered user dictionaries cover string-to-label `{a=//attr:leaf, z=//pkg:source.txt}`, label-to-string `{//attr:leaf=a, //pkg:source.txt=z}`, and string-to-label-list `{a=[//attr:leaf], z=[//pkg:source.txt, //attr:leaf]}`; each has a reversed-interior nonmatch. |
| 6 | Scalar/container leaves require main `//attr:leaf`, generic Bzlmod canonical `@@ext+//leaf:label`, and fixed `@@bazel_tools//tools/test:test_wrapper`. The update run must freeze the actual canonical token and stop rather than weakening the regex if it differs from pinned-source expectation. |
| 7 | One string attr concatenates two selects with the same complete key set. Positive candidates are the same-branch combinations; mixed combinations are negative. Both clauses operate on duplicate rule instances with identical values, never on separate attrs. |
| 8 | A second string attr concatenates selects with distinct key sets and must expose a cross-product-only combination. Separate string and executable/test `args` probes require formatting after concatenation, including `[p, a, p]` duplicate retention inside one list candidate. |
| 9 | Package-derived `visibility=[//visibility:public]`, `testonly=1`, `package_metadata=[//attr:metadata]`, `deprecation`, and `licenses` contrast with existing package defaults. A legacy macro target exposes exact `generator_name` and `generator_function`; `generator_location` matches `^attr/BUILD\\.bazel:[0-9]+:[0-9]+$`, while direct targets expose empty generator strings. |
| 10 | Normal Starlark shared fields cover a nonempty `expect_failure`, `toolchains`, `exec_properties`, execution compatibility/default dictionaries, `target_compatible_with`, and `$config_dependencies`. Executable probes cover `args`, `output_licenses`, and `$is_executable=1`; the root string setting covers its declared `build_setting_default` and empty `help`, never the command-line value. |
| 11 | The test family covers `testonly=1`, `size=medium`, computed `timeout=moderate`, `flaky=0`, `shard_count=-1`, `local=0`, ordered `args`, `$is_executable=1`, the six fixed test labels/list, and the two fixed coverage loading fallbacks. `:run_under_exec_config` and `:run_under_target_config` use `^.*$` and must select nothing. |
| 12 | An omitted/empty native suite proves automatic `$implicit_tests` and contrasts it with explicit nonempty `tests`. A rule with an admitted Starlark attr transition exposes `$allowlist_function_transition=@@bazel_tools//tools/allowlists:function_transition_allowlist`; the normal rule lacks that definition. |
| 13 | One native baseline/filegroup family covers shared `K`/`NATIVE` values and nonempty `srcs`, `data`, `output_group`, and `output_licenses`; equivalent empty shared defaults are not repeated per class. |
| 14 | `alias.actual` and `toolchain_type.no_match_error` are positive. `licenses`, `distribs`, and `:action_listener` match the baseline representative but not alias/toolchain-type, proving their removals. |
| 15 | `config_setting` covers manual tags, forced `[none]` license, `values`, `define_values`, label-keyed `flag_values`, `constraint_values`, and loading `:flag_alias_settings=[]`; `compatible_with`, `restricted_to`, and `target_compatible_with` match the baseline but not this class. |
| 16 | `test_suite` covers fixed `testonly=1`, `tests`, and automatic `$implicit_tests`. `constraint_setting` covers explicit and null `default_constraint_value`, explicit `refines_constraint_value`, and its five removals; `constraint_value` covers mandatory `constraint_setting` and the same removals. |
| 17 | `platform` covers manual tags, `constraint_values`, `parents`, `remote_execution_properties`, `exec_properties`, `flags`, exact `missing_toolchain_error`, `required_settings`, `check_toolchain_types=0`, `allowed_toolchain_types`, and all five inherited removals. |
| 18 | `toolchain` covers manual tags, reintroduced `target_compatible_with=[]`, mandatory `toolchain_type`/`toolchain`, `exec_compatible_with`, `use_target_platform_constraints=0`, `target_settings`, removal of `:action_listener`, and absence of the other two constraint attrs. |

The selector conditions use two ordinary `config_setting` labels. Equal-key
correlation and distinct-key cross-product are asserted only through candidate
membership. No row varies default-entry position, candidate traversal order,
or multiplicity of equal whole candidates; ordinary `attr()` cannot expose
those facts.

### Generation boundary and validation

The due fixture-growth hygiene checkpoint is closed at accepted tree
`51540963`. Compared with prior reset `8d84d336`, the logical payload-expanded
corpus is now 1,361 regular files, 24 links, 42,520 newline-counted lines, and
864 command rows: +36/+0/+2,888/+61 across six accepted row-bearing oracle
packets. The `b83935ab` payload migration remains internally complete: all 14
payload projections match their hashes, no stale physical workspace remains,
and payload-backed fixtures contain no links. Duplicate/reachability review
found no removable nondiscriminating asset; the only same-content external-
visibility BUILD pair distinguishes main from external repository identity.
This resets the checkpoint at `51540963`; generation is packet one and is
capped at three new virtual regular files, zero links, 18 rows, and 1,000
newline-counted lines. Review again before packet six or +100 files/+10,000
lines.

The generation packet may change only:

- `tests/v2_oracle/fixtures/query-labels-attribute-metadata/{fixture.toml,expected/oracle.json}`;
- `tests/v2_fixture_payload/fixtures.payload` for the five named virtual source
  edits/additions above;
- `tests/v2_oracle/test_v2_oracle.py` for derived payload counts/hash and this
  projection hash; and
- `tests/v2_fixture_support/src/lib.rs` for the same derived global/projection
  hashes and entry-count assertion.

The last two are mechanical test-integrity constants, not production Rust.
Runner, BUILD, CLI, server, query, loading, Cargo/lockfile, and plan code remain
unchanged. The fixture description/provenance must expand from `labels()` to
ordinary `attr()` and cite `AttrFunction`, `RegexFilterExpression`,
`TargetUtils`, the mapper/default path, base Starlark/native definitions, and
the nine native RuleDefinitions at `8220c619...`.

Generation must use ordinary Bazel RC discovery without reading or copying the
private home RC: one update run and one clean distinct-root verification of all
57 rows. Compare the first 39 decoded argv/exit/normalized stdout/stderr records
with their protected evidence; raw invocation IDs and absolute run roots are
not semantics. Run the frozen Python payload inventory/projection tests, the
Rust payload conformance consumers, the focused existing 29-row CLI and two
generated-kind CLI/server regressions, and `git diff --check`, serially where
they share Cargo state. The 18 new rows remain Bazel-only evidence and do not
authorize Slug `attr()` activation.

Native `toolchain` rows close oracle evidence only. V2 still rejects
`NativeToolchainTarget` before graph projection; a separately reviewed graph
prerequisite remains mandatory before implementation can consume that row.
Generic external attr rendering similarly proves loading evidence only and
does not authorize a new graph path; later production work must reuse the
already reviewed external-loading owners or stop.

Independent Sol review returned `ACCEPT`: the 18 isolated-operand union lanes
cover every accepted ledger class without dedup rescue, reuse the smallest
canonical workspace, and preserve the external/toolchain and permanent
Rust-native boundaries. Generation-time transcription density remains the
residual risk and is owned by the frozen-token, protected-row replay, and size
caps below.

Run next only `WP-4-8-m3-attr-observable-candidate-oracle-generation` after the
fixture-hygiene checkpoint is closed. Stop on more than 18 new commands, more
than three new virtual regular files, more than 1,000 logical fixture/TOML/
expected lines, any mutation or new fixture, any protected-row semantic change,
an external canonical token other than the pinned/generated exact value, a
need for configured analysis, or any production/Rust-semantic, graph, DICE,
JVM, Java artifact, or Bazel-delegation change.

## `attr` observable-candidate generation stop (2026-08-09)

`WP-4-8-m3-attr-observable-candidate-oracle-generation` reached a concrete
fixture-isolation prerequisite and returned `REPLAN`. A five-file draft used
the accepted shared-workspace arrangement, froze the generic external spelling
as `@@ext+//leaf:label`, and completed both a 57-row Bazel update and a clean
57-row distinct-root replay. It was not accepted: root integration review and
an independent Terra audit found that the dense first transcription omitted
many ledger atoms and reused positive labels inside unions, contrary to the
accepted discriminator invariant.

The permitted focused correction then reached the architecture stop. Extending
the existing workspace's shared `pkg/defs.bzl` with required constructors such
as `attr.string_list()` makes the protected 29-row Slug CLI consumer load syntax
outside Slug's current admitted Starlark attr surface; it fails before protected
row one. The transition probe initially exposed the same shared-consumer issue
through Slug's deliberately bounded transition declaration. Adding those
production semantics is forbidden in an oracle-only packet, while deleting the
atoms would weaken the total-schema evidence. Therefore the existing
`query-labels-attribute-metadata` workspace is not an honest host for this
oracle despite its smaller physical fixture delta.

All five unaccepted draft files were restored exactly to accepted tree
`6c9a529e`; no fixture, payload, expected record, integrity constant, Rust, or
generated content remains. The fixture-growth reset remains `51540963` because
the stopped packet added no accepted breadth.

Run next only `WP-4-8-m3-attr-isolated-observable-candidate-oracle-design`.
Design the smallest isolated Bazel-only payload workspace and fixture that is
not selected by any protected Slug CLI/server consumer, inventory its complete
module/package/Starlark/external source closure and derived payload consumers,
and remap every accepted lane to distinct positive/negative rule instances.
Preserve the frozen `@@ext+//leaf:label` evidence only if the new isolated
update independently reproduces it. Add no fixture, payload, expected record,
Rust, Cargo, graph, DICE, JVM/Java artifact, or Bazel delegation during that
design packet. Stop if discovery necessarily routes the isolated workspace
through an existing Slug semantic regression or if exact evidence would require
production activation first.

## Isolated `attr` observable-candidate oracle design (2026-08-09)

`WP-4-8-m3-attr-isolated-observable-candidate-oracle-design` selects a new
payload-backed fixture named `query-attr-observable-candidates`. The name is not
added to `v2_fixture_support::PROJECTIONS` and appears in no CLI/server case.
This is a semantic isolation boundary, not only a naming convention:
`tools.v2_oracle` discovers metadata globally but runs only the explicit
`--fixture`; packet validation likewise executes only its explicit fixture
list. The sole discovery-wide test reads expected-record metadata. Every Rust
`FixtureWorkspace` consumer is allowlisted by the static projection table, and
the current 29-row CLI, two generated-kind CLI/server, and other cases name
their workspaces literally. Therefore no current Slug process can materialize
or parse the new workspace.

### Complete standalone source closure

The isolated canonical-payload projection has five directories and exactly five
regular files:

| Virtual path | Required role |
| --- | --- |
| `MODULE.bazel` | Root module, `bazel_dep(name="ext", version="1.0")`, and relative local override to `modules/ext`. |
| `attr/defs.bzl` | Normal/executable/test/build-setting/output rules, all typed attrs, identity transition, compact pair macros, and the one legacy macro discriminator. |
| `attr/BUILD.bazel` | Base string-build-setting instances, positive package defaults and license, all Starlark/native/selector/macro/output/nonrule probes, exported BUILD source, and direct generator-empty controls. |
| `modules/ext/MODULE.bazel` | Local `ext@1.0` module declaration. |
| `modules/ext/leaf/BUILD.bazel` | `filegroup(name="label")`, supplying the generic external leaf without a seventh file. |

No root package is needed: “root string setting” names the base build-setting
rule family, not the workspace root, and `attr/BUILD.bazel` is itself the source
nonrule operand. The isolated workspace therefore has one main-repository
package plus the external leaf package. No physical source/leaf, action, copied
repository tree, registry, lockfile, mutation, or generated `@bazel_tools`
content is needed. Bazel's built-in tools repository supplies only the fixed
test and transition-allowlist labels.

The existing 18-lane table remains the exact semantic matrix. Its implementation
uses globally unique names `//attr:lNN_aMMM_yes` and `_no`; every atomic clause
receives one distinct pair, and no positive label is reused inside a lane.
Exact stdout enumerates every `_yes` exactly once and no `_no`. Helper macros
may compact mechanically equivalent declarations, but lane 9's direct targets
remain direct and its legacy-macro targets use only the named legacy macro so
`generator_*` provenance is not contaminated.

| Lane | Unique pairs | Closed family |
| ---: | ---: | --- |
| 1 | 13 | Four Starlark plus nine native names; source/generated/package-group nonrules remain negative-only operands. |
| 2 | 7 | Scalars, integer, both booleans, licenses, and explicit/null deprecation pair. |
| 3 | 5 | Empty string/list/map, explicit/null label pair, and private spelling; null output and `_private` are negative-only. |
| 4 | 4 | Ordered/OI lists and ordered dictionary interiors. |
| 5 | 3 | Three user-dictionary orientations and their reversed interiors. |
| 6 | 3 | Main, generic-external, and fixed tools labels. |
| 7 | 3 | Three complete equal-key selector branches plus mixed negatives. |
| 8 | 6 | Distinct-key cross-product and string/executable/test list concatenation. |
| 9 | 11 | Package defaults and macro/direct generator fields. |
| 10 | 12 | Normal, executable, and root-setting schema fields. |
| 11 | 16 | Test fixed/computed/loading fields; the two null run-under attrs are negative-only. |
| 12 | 3 | Suite automatic membership and transition allowlist. |
| 13 | 23 | Sixteen shared `K`, three `NATIVE`, and four filegroup additions. |
| 14 | 5 | Alias/toolchain-type additions and three removals. |
| 15 | 10 | Seven config-setting additions and three removals. |
| 16 | 16 | Suite and both constraint-class additions/removals; explicit/null default is one pair. |
| 17 | 15 | Platform additions and five removals. |
| 18 | 10 | Seven toolchain additions/reintroductions and three removals. |

The total is 165 pairs/330 distinct probe rule instances plus approximately 20
support targets for selector keys, leaves, metadata, constraints, toolchain
labels, test membership, package group, and generated output. This is the
smallest honest construction under the accepted no-pair-reuse invariant. A
pure identity transition is loading evidence only; its declared output names
the real base string build setting in `//attr`. Test rule-class names end in `_test`.
Ordinary query constructs native targets without configured analysis or
toolchain resolution.

### Generation allowlist, growth, and validation

The successor generation packet may add only:

- `tests/v2_oracle/fixtures/query-attr-observable-candidates/fixture.toml` and
  `expected/oracle.json`;
- the five-file projection in `tests/v2_fixture_payload/fixtures.payload`;
- derived global count/body-byte/SHA plus the new projection hash in
  `tests/v2_oracle/test_v2_oracle.py`; and
- derived global SHA and 275-to-285 entry-count assertion in
  `tests/v2_fixture_support/src/lib.rs`.

It must not add the new name to Rust `PROJECTIONS`, CLI/server cases, or any
production consumer. The payload grows by five directories, five virtual files,
and ten encoded entries: the global entry/directory pair becomes `(285,
117)`. Body bytes and hashes are generated, never predicted. Existing fourteen
Python/Rust projection hashes remain byte-identical. Including the new fixture
TOML/expected files, the payload-expanded corpus cap from hygiene reset
`51540963` is +7 regular files, +5 directories, zero links, exactly 18 rows, and
2,400 newline-counted source/TOML/expected lines. Because this one packet is
dense, run a new hygiene review before any later fixture packet rather than
waiting for the ordinary sixth-packet trigger.

Generation uses ordinary Bazel RC discovery without inspecting, printing, or
copying the private home RC. Run one explicit update and one clean distinct-root
replay of exactly 18 rows. The isolated run must independently reproduce
`@@ext+//leaf:label`; the stopped draft is guidance, not accepted evidence, and
any different spelling is a stop rather than permission to weaken the anchored
regex. Validate the frozen Python inventory/projection/metadata tests, Rust
global payload conformance with no new projection, all fourteen unchanged
projection hashes, the protected 29-row CLI and two generated-kind CLI/server
cases, and `git diff --check`, serializing shared Cargo state. All 18 rows must
pass; lane 9 must positively expose macro name/function/location plus direct
empty generator fields, and lane 12 must positively expose the transition
allowlist. The oracle remains Bazel-only loading evidence and authorizes no
Slug `attr()`, native-toolchain graph, or new external-production path.

Independent oracle-design review is required before scheduling generation.
Stop on a sixth virtual source, any link/mutation/registry/lockfile/new tools
content, more than 18 rows or 2,400 logical lines, a Rust projection or semantic
consumer, protected projection/output drift, a nonexact external token, a need
for configured analysis, or any production Rust, graph, DICE, regex, JVM/Java,
or Bazel-delegation change.

Independent Sol review first removed an unnecessary root `BUILD.bazel`; the
focused correction moved the base setting and source nonrule into `attr` and
retained the mandatory payload workspace-root directory. Rereview returned
`ACCEPT`: five files plus five directory records yields ten entries, `(285,
117)`, and a +7-file corpus delta. Residual risk is confined to generation
proving the complete 18 rows within the 2,400-line cap.

Run next only
`WP-4-8-m3-attr-isolated-observable-candidate-oracle-generation`.

The generation preflight found and corrected one arithmetic contract defect
before Bazel ran. Null/nonrule negative operands had been counted as standalone
pairs in lanes 2, 3, 11, and 16, while lanes 15 and 18 omitted named removal
pairs. A complete no-reuse audit fixes the vector to
`13/7/5/4/3/3/3/6/11/12/16/3/23/5/10/16/15/10`: 165 pairs/330 instances.
The 18-row, five-file, isolation, and 2,400-line boundaries are unchanged.
This is the generation packet's sole material contract correction; any second
material correction is `REPLAN`.

Generation then reached that second material contradiction before any Bazel
run. Pinned `BaseRuleClasses.deprecationDefault` always returns
`PackageArgs.defaultDeprecation`, and `AttributeProvider` ignores an explicit
Starlark `None`, leaving the computed package default active. One `//attr`
package therefore cannot simultaneously supply lane 9's package-derived
`deprecation="deprecated"` and lane 2's same-schema null-deprecation control.
Using a class that removes the attribute would test inheritance removal rather
than null default; weakening or conflating those atoms is not accepted.

The incomplete five-file/18-row draft was removed without running Bazel. No
fixture, payload, expected record, integrity constant, Rust, or generated
content remains. Run next only
`WP-4-8-m3-attr-two-package-observable-candidate-oracle-design`. It must design
the smallest isolated positive-default plus baseline-package layout, decide
whether an existing package can honestly host the baseline or a sixth virtual
file is necessary, remap pair labels without weakening the corrected 165-atom
ledger, and recalculate directory/entry/line caps. Edit only Stage 4 and Stage
8 owner plans; add no fixture, payload, Rust, Cargo, graph, DICE, JVM/Java
artifact, or Bazel delegation.

## Two-package `attr` oracle design (2026-08-09)

`WP-4-8-m3-attr-two-package-observable-candidate-oracle-design` retains the
five-file isolated layout by using the existing external leaf package as the
baseline package. Its `BUILD.bazel` loads the public main definition with the
canonical-main spelling `load("@@//attr:defs.bzl", ...)`; apparent `@//` is not
used because a nonroot module mapping does not map the main repository. Pinned
Bazel external-repository tests use this canonical-main load form, and an
unannotated `.bzl` remains publicly loadable.

The leaf keeps `filegroup(name="label")` unchanged for lane 6's exact
`@@ext+//leaf:label`. It additionally instantiates the same main-defined normal
Starlark rule without a package deprecation default. Lane 2's deprecation atom
therefore becomes:

```text
attr(deprecation, "^deprecated$",
     set(//attr:l02_a007_yes @@ext+//leaf:l02_a007_no))
```

The positive retains package-derived `deprecation="deprecated"`; the external
negative has the identical rule schema but a null computed package default and
must remain absent from stdout. Only that negative operand moves. The other 164
pairs, lane 6 external leaf, and exact expected positives remain unchanged.
No removal-only class, explicit `None`, sixth source, or second schema is used.

The corrected vector remains
`13/7/5/4/3/3/3/6/11/12/16/3/23/5/10/16/15/10` = 165 pairs. The projection
remains five files/five directories/ten encoded entries, taking payload
entries/directories from `(275, 112)` to `(285, 117)` and regular files from
163 to 168. Body bytes and hashes are generated. Including fixture TOML and
expected JSON, growth from reset `51540963` remains +7 regular files, +5
directories, zero links, 18 rows, and at most 2,400 lines; the absolute review
ceilings are 1,368 regular files, 24 links, 44,920 lines, and 882 rows. A sixth
`plain/BUILD.bazel` would add a redundant file/directory and is rejected.

Generation allowlist and isolation are unchanged: new fixture TOML/expected,
the five-file payload projection, Python global/projection integrity, and Rust
global SHA plus 275-to-285 count only. Do not add a Rust projection or semantic
consumer. Update plus clean distinct-root replay must pass all 18 rows, freeze
`@@ext+//leaf:label`, preserve fourteen projection hashes, and pass payload
metadata/integrity, the protected 29 CLI and two generated-kind CLI/server
cases, lane 9 provenance, lane 12 allowlist, and diff checks.

Independent design review is required before retrying generation. Stop on an
added source, restricted main `.bzl` visibility, repository mapping/registry/
lockfile need, changed 165-atom ledger, nonexact external token, Rust projection
or semantic consumer, configured analysis, cap/protected drift, production
Rust/graph/DICE/regex, JVM/Java, or Bazel delegation.

Independent Sol review returned `ACCEPT`: pinned Bazel tests establish the
public canonical-main `@@//` load from the external module, and the computed
deprecation default remains package-local. Residual risk is limited to the
isolated update/replay confirming the canonical load and null result. Run next
only `WP-4-8-m3-attr-two-package-observable-candidate-oracle-generation`.

Generation preflight then returned `REPLAN` without file changes or a Bazel
run. The owner text freezes all semantic families and lane totals, but it does
not assign the remaining 164 stable `lNN_aMMM` IDs to an exact attribute
spelling, anchored regex, positive rule schema/value, negative schema/value or
absence, and expected label. Only lane 2 deprecation `l02_a007` is frozen.
Inventing that mapping while writing the fixture would combine unreviewed
oracle design with generation, precisely the ambiguity that allowed earlier
green-but-incomplete drafts.

Run next only `WP-4-8-m3-attr-atomic-discriminator-manifest-design`. Add to the
Stage 4 owner plan a complete 165-row authoritative atom manifest keyed by the
corrected vector. Every row must freeze query spelling, regex, yes/no labels,
rule classes and values/absence, expected presence, support-target dependency,
and whether a control is negative-only. Stage 8 receives only a compact
checksum/count summary. The manifest must preserve the reviewed external
baseline, five-file layout, caps, isolation, and no-reuse rule. Add no fixture,
payload, expected record, Rust, Cargo, graph, DICE, JVM/Java artifact, or Bazel
delegation during that design packet.

## `attr` atomic discriminator manifest (2026-08-09)

This is the authoritative UTF-8/LF manifest.  Each semicolon-delimited record
has seven fields: stable ID; query spelling; anchored regex; positive operand;
negative operand; expected result; support targets.  `yes` means only the
positive operand is selected and `no` means the negative operand is not.  The
checksum scope is exactly the record lines between the two markers below,
joined with one LF and followed by one final LF; headings, blank lines, and the
negative-only controls are excluded. `normal`, `exec`, `test`, and
`string_setting` name the closed Starlark shapes from the ledger. Every listed
`_yes`/`_no` is a distinct probe instance unless a record explicitly names the
external two-package baseline.

### Constructor and support policy

Every manifest operand is a syntactically valid loading-phase declaration. The
global constructor fills are exact: every `string_setting` has
`build_setting_default=base`; every `alias` has `actual=//attr:leaf`; every
`constraint_value` has `constraint_setting=//attr:constraint_setting`; and
every `toolchain` has `toolchain_type=//attr:toolchain_type` plus
`toolchain=//attr:leaf`. An explicit field in a manifest record overrides its
corresponding global fill, including the paired names used to distinguish the
two operands. These globally supplied supports are part of the support
inventory even when a record's support field lists only row-specific
dependencies. The sole identity-transition normal rule declares its output as
`//attr:base_string_setting`, the same string build setting that is constructed
with `build_setting_default=base`; it does not name a configured value.

`//attr:required_config_setting` and `//attr:target_config_setting` are native
`config_setting` support targets. They are respectively the only nonempty
supports for `platform.required_settings` and `toolchain.target_settings`; a
build setting is not admitted there. The other named support labels are
ordinary constructible leaves, constraints, platforms, toolchain types, or
test rules. `//attr:l01_generated_nonrule` is produced by the normal
`//attr:l01_generated_owner` output declaration; `//attr:l03_null_output` is
an output-rule declaration with its nullable output omitted; and each null
run-under control is a test-rule declaration. Thus controls, too, have a
producer without becoming pairs.

For automatic suite membership, the only non-manual test in the package is
`//attr:implicit_member_test`, with `tags=[suite]` and `size=medium`.
`//attr:explicit_member_test` has `tags=[manual]`, as does every other test
probe including all lane-11 probes, so they are excluded by Bazel's
implicit-test accumulator. The implicit suites in lanes 12 and 16 have
`tests=[]` and `tags=[suite]`; their singleton implicit candidate is therefore
exactly `[//attr:implicit_member_test]`. Their explicit negative has
`tests=[//attr:explicit_member_test]` and the same suite tag, so automatic
population is suppressed rather than merely filtered. This is the pinned
`TestSuiteImplicitTestsAccumulator` rule: it retains non-manual test rules,
filters their `tags` plus `size`, then sorts the retained labels at package
finalization.

<!-- attr-manifest-records:start -->
```text
l01_a001;name;^l01_a001_yes$;//attr:l01_a001_yes,normal,name;//attr:l01_a001_no,normal,name;yes/no;-
l01_a002;name;^l01_a002_yes$;//attr:l01_a002_yes,exec,name;//attr:l01_a002_no,exec,name;yes/no;-
l01_a003;name;^l01_a003_yes$;//attr:l01_a003_yes,test,name;//attr:l01_a003_no,test,name;yes/no;-
l01_a004;name;^l01_a004_yes$;//attr:l01_a004_yes,string_setting,name,build_setting_default=base;//attr:l01_a004_no,string_setting,name,build_setting_default=base;yes/no;-
l01_a005;name;^l01_a005_yes$;//attr:l01_a005_yes,filegroup,name;//attr:l01_a005_no,filegroup,name;yes/no;-
l01_a006;name;^l01_a006_yes$;//attr:l01_a006_yes,alias,name,actual=//attr:leaf;//attr:l01_a006_no,alias,name,actual=//attr:leaf;yes/no;//attr:leaf
l01_a007;name;^l01_a007_yes$;//attr:l01_a007_yes,config_setting,name;//attr:l01_a007_no,config_setting,name;yes/no;-
l01_a008;name;^l01_a008_yes$;//attr:l01_a008_yes,test_suite,name;//attr:l01_a008_no,test_suite,name;yes/no;-
l01_a009;name;^l01_a009_yes$;//attr:l01_a009_yes,constraint_setting,name;//attr:l01_a009_no,constraint_setting,name;yes/no;-
l01_a010;name;^l01_a010_yes$;//attr:l01_a010_yes,constraint_value,name,constraint_setting=//attr:l01_a009_yes;//attr:l01_a010_no,constraint_value,name,constraint_setting=//attr:l01_a009_no;yes/no;l01_a009
l01_a011;name;^l01_a011_yes$;//attr:l01_a011_yes,platform,name,constraint_values=[//attr:l01_a010_yes];//attr:l01_a011_no,platform,name,constraint_values=[//attr:l01_a010_no];yes/no;l01_a010
l01_a012;name;^l01_a012_yes$;//attr:l01_a012_yes,toolchain_type,name;//attr:l01_a012_no,toolchain_type,name;yes/no;-
l01_a013;name;^l01_a013_yes$;//attr:l01_a013_yes,toolchain,name,toolchain_type=//attr:l01_a012_yes,toolchain=//attr:leaf;//attr:l01_a013_no,toolchain,name,toolchain_type=//attr:l01_a012_no,toolchain=//attr:leaf;yes/no;//attr:leaf,l01_a012
l02_a001;expect_failure;^boom$;//attr:l02_a001_yes,normal,expect_failure=boom;//attr:l02_a001_no,normal,expect_failure=nope;yes/no;-
l02_a002;shard_count;^-1$;//attr:l02_a002_yes,test,shard_count=-1;//attr:l02_a002_no,test,shard_count=0;yes/no;-
l02_a003;testonly;^0$;//attr:l02_a003_yes,normal,testonly=0;//attr:l02_a003_no,normal,testonly=1;yes/no;-
l02_a004;testonly;^1$;//attr:l02_a004_yes,normal,package_testonly=1;//attr:l02_a004_no,normal,testonly=0;yes/no;-
l02_a005;licenses;^\\[notice\\]$;//attr:l02_a005_yes,filegroup,package_licenses=[notice];//attr:l02_a005_no,config_setting,licenses=[none];yes/no;-
l02_a006;licenses;^\\[none\\]$;//attr:l02_a006_yes,config_setting,licenses=[none];//attr:l02_a006_no,filegroup,package_licenses=[notice];yes/no;-
l02_a007;deprecation;^deprecated$;//attr:l02_a007_yes,normal,package_deprecation=deprecated;@@ext+//leaf:l02_a007_no,normal,same_schema,package_deprecation=null;yes/no;@@ext+//leaf,load(@@//attr:defs.bzl)
l03_a001;empty_string;^$;//attr:l03_a001_yes,normal,string,empty_string=;//attr:l03_a001_no,normal,string,empty_string=x;yes/no;-
l03_a002;empty_label_list;^\\[\\]$;//attr:l03_a002_yes,normal,label_list,default=[];//attr:l03_a002_no,normal,label_list=[//attr:leaf];yes/no;//attr:leaf
l03_a003;empty_string_keyed_label_dict;^\\{\\}$;//attr:l03_a003_yes,normal,string_keyed_label_dict,default={};//attr:l03_a003_no,normal,string_keyed_label_dict={a=//attr:leaf};yes/no;//attr:leaf
l03_a004;nullable_label;^//attr:leaf$;//attr:l03_a004_yes,normal,label,nullable_label=//attr:leaf;//attr:l03_a004_no,normal,label,nullable_label=null;yes/no;//attr:leaf
l03_a005;$private;^secret$;//attr:l03_a005_yes,normal,_private=secret;//attr:l03_a005_no,normal,_private=other;yes/no;-
l04_a001;args;^\\[z, z, a\\]$;//attr:l04_a001_yes,exec,args=[z,z,a];//attr:l04_a001_no,exec,args=[a,z,z];yes/no;-
l04_a002;tags;^\\[a, z, z\\]$;//attr:l04_a002_yes,normal,tags=[z,a,z],OI;//attr:l04_a002_no,normal,tags=[z,a],OI;yes/no;-
l04_a003;features;^\\[a, z, z\\]$;//attr:l04_a003_yes,normal,features=[z,a,z],OI;//attr:l04_a003_no,normal,features=[z,a],OI;yes/no;-
l04_a004;string_dict;^\\{z=1, a=2\\}$;//attr:l04_a004_yes,normal,string_dict={z=1,a=2};//attr:l04_a004_no,normal,string_dict={a=2,z=1};yes/no;-
l05_a001;string_keyed_label_dict;^\\{a=//attr:leaf, z=//attr:BUILD\\.bazel\\}$;//attr:l05_a001_yes,normal,string_keyed_label_dict={a=//attr:leaf,z=//attr:BUILD.bazel};//attr:l05_a001_no,normal,string_keyed_label_dict={z=//attr:BUILD.bazel,a=//attr:leaf};yes/no;//attr:leaf,//attr:BUILD.bazel
l05_a002;label_keyed_string_dict;^\\{//attr:leaf=a, //attr:BUILD\\.bazel=z\\}$;//attr:l05_a002_yes,normal,label_keyed_string_dict={//attr:leaf=a,//attr:BUILD.bazel=z};//attr:l05_a002_no,normal,label_keyed_string_dict={//attr:BUILD.bazel=z,//attr:leaf=a};yes/no;//attr:leaf,//attr:BUILD.bazel
l05_a003;label_list_dict;^\\{a=\\[//attr:leaf\\], z=\\[//attr:BUILD\\.bazel, //attr:leaf\\]\\}$;//attr:l05_a003_yes,normal,label_list_dict={a=[//attr:leaf],z=[//attr:BUILD.bazel,//attr:leaf]};//attr:l05_a003_no,normal,label_list_dict={z=[//attr:BUILD.bazel,//attr:leaf],a=[//attr:leaf]};yes/no;//attr:leaf,//attr:BUILD.bazel
l06_a001;scalar_label;^//attr:leaf$;//attr:l06_a001_yes,normal,label,scalar_label=//attr:leaf;//attr:l06_a001_no,normal,label,scalar_label=//attr:BUILD.bazel;yes/no;//attr:leaf,//attr:BUILD.bazel
l06_a002;scalar_label;^@@ext\\+//leaf:label$;//attr:l06_a002_yes,normal,label,scalar_label=@@ext+//leaf:label;//attr:l06_a002_no,normal,label,scalar_label=//attr:leaf;yes/no;@@ext+//leaf:label,//attr:leaf
l06_a003;$test_wrapper;^@@bazel_tools//tools/test:test_wrapper$;//attr:l06_a003_yes,test,$test_wrapper=@@bazel_tools//tools/test:test_wrapper;//attr:l06_a003_no,normal,$test_wrapper=absent;yes/no;@@bazel_tools//tools/test:test_wrapper
l07_a001;equal_select_string;^aa$;//attr:l07_a001_yes,normal,select(cfg_a=a,cfg_b=b,default=d)+select(same_keys=a,b,d)->aa,bb,dd;//attr:l07_a001_no,normal,select(cfg_a=a,cfg_b=b,default=d)+select(same_keys=b,d,a)->ab,bd,da;yes/no;//attr:cfg_a,//attr:cfg_b
l07_a002;equal_select_string;^bb$;//attr:l07_a002_yes,normal,select(cfg_a=a,cfg_b=b,default=d)+select(same_keys=a,b,d)->aa,bb,dd;//attr:l07_a002_no,normal,select(cfg_a=a,cfg_b=b,default=d)+select(same_keys=b,d,a)->ab,bd,da;yes/no;//attr:cfg_a,//attr:cfg_b
l07_a003;equal_select_string;^dd$;//attr:l07_a003_yes,normal,select(cfg_a=a,cfg_b=b,default=d)+select(same_keys=a,b,d)->aa,bb,dd;//attr:l07_a003_no,normal,select(cfg_a=a,cfg_b=b,default=d)+select(same_keys=b,d,a)->ab,bd,da;yes/no;//attr:cfg_a,//attr:cfg_b
l08_a001;cross_select_string;^ap$;//attr:l08_a001_yes,normal,cross_select_string=select(cfg_a=a,cfg_b=b)+select(cfg_p=p,cfg_q=q); //attr:l08_a001_no,normal,cross_select_string=aa;yes/no;//attr:cfg_a,//attr:cfg_b,//attr:cfg_p,//attr:cfg_q
l08_a002;cross_select_string;^aq$;//attr:l08_a002_yes,normal,cross_select_string=select(cfg_a=a,cfg_b=b)+select(cfg_p=p,cfg_q=q); //attr:l08_a002_no,normal,cross_select_string=pp;yes/no;//attr:cfg_a,//attr:cfg_b,//attr:cfg_p,//attr:cfg_q
l08_a003;cross_select_string;^bp$;//attr:l08_a003_yes,normal,cross_select_string=select(cfg_a=a,cfg_b=b)+select(cfg_p=p,cfg_q=q); //attr:l08_a003_no,normal,cross_select_string=ba;yes/no;//attr:cfg_a,//attr:cfg_b,//attr:cfg_p,//attr:cfg_q
l08_a004;string_concat;^pa$;//attr:l08_a004_yes,normal,string_concat=select(cfg_p=p)+select(cfg_a=a); //attr:l08_a004_no,normal,string_concat=ap;yes/no;//attr:cfg_p,//attr:cfg_a
l08_a005;args;^\\[p, a, p\\]$;//attr:l08_a005_yes,exec,args=select(cfg_p=[p])+select(cfg_a=[a,p]); //attr:l08_a005_no,exec,args=[p,a];yes/no;//attr:cfg_p,//attr:cfg_a
l08_a006;args;^\\[p, a, p\\]$;//attr:l08_a006_yes,test,args=select(cfg_p=[p])+select(cfg_a=[a,p]); //attr:l08_a006_no,test,args=[p,a];yes/no;//attr:cfg_p,//attr:cfg_a
l09_a001;visibility;^\\[//visibility:public\\]$;//attr:l09_a001_yes,normal,package_visibility=[//visibility:public];//attr:l09_a001_no,normal,visibility=[//visibility:private];yes/no;-
l09_a002;testonly;^1$;//attr:l09_a002_yes,normal,package_testonly=1;//attr:l09_a002_no,normal,testonly=0;yes/no;-
l09_a003;package_metadata;^\\[//attr:metadata\\]$;//attr:l09_a003_yes,normal,package_metadata=[//attr:metadata];//attr:l09_a003_no,normal,package_metadata=[];yes/no;//attr:metadata
l09_a004;deprecation;^deprecated$;//attr:l09_a004_yes,normal,package_deprecation=deprecated;@@ext+//leaf:l09_a004_no,normal,same_schema,package_deprecation=null;yes/no;@@ext+//leaf,load(@@//attr:defs.bzl)
l09_a005;licenses;^\\[notice\\]$;//attr:l09_a005_yes,filegroup,package_licenses=[notice];//attr:l09_a005_no,config_setting,licenses=[none];yes/no;-
l09_a006;generator_name;^macro_case$;//attr:l09_a006_yes,normal,legacy_macro,generator_name=macro_case;//attr:l09_a006_no,normal,direct,generator_name=;yes/no;legacy_macro
l09_a007;generator_function;^legacy_macro$;//attr:l09_a007_yes,normal,legacy_macro,generator_function=legacy_macro;//attr:l09_a007_no,normal,direct,generator_function=;yes/no;legacy_macro
l09_a008;generator_location;^attr/BUILD\\.bazel:[0-9]+:[0-9]+$;//attr:l09_a008_yes,normal,legacy_macro,generator_location=attr/BUILD.bazel:line:column;//attr:l09_a008_no,normal,direct,generator_location=;yes/no;legacy_macro
l09_a009;generator_name;^$;//attr:l09_a009_yes,normal,direct,generator_name=;//attr:l09_a009_no,normal,legacy_macro,generator_name=macro_case;yes/no;legacy_macro
l09_a010;generator_function;^$;//attr:l09_a010_yes,normal,direct,generator_function=;//attr:l09_a010_no,normal,legacy_macro,generator_function=legacy_macro;yes/no;legacy_macro
l09_a011;generator_location;^$;//attr:l09_a011_yes,normal,direct,generator_location=;//attr:l09_a011_no,normal,legacy_macro,generator_location=attr/BUILD.bazel:line:column;yes/no;legacy_macro
l10_a001;expect_failure;^boom$;//attr:l10_a001_yes,normal,expect_failure=boom;//attr:l10_a001_no,normal,expect_failure=;yes/no;-
l10_a002;toolchains;^\\[//attr:toolchain_type\\]$;//attr:l10_a002_yes,normal,toolchains=[//attr:toolchain_type];//attr:l10_a002_no,normal,toolchains=[];yes/no;//attr:toolchain_type
l10_a003;exec_properties;^\\{cpu=k8\\}$;//attr:l10_a003_yes,normal,exec_properties={cpu=k8};//attr:l10_a003_no,normal,exec_properties={};yes/no;-
l10_a004;exec_compatible_with;^\\[//attr:constraint_value\\]$;//attr:l10_a004_yes,normal,exec_compatible_with=[//attr:constraint_value];//attr:l10_a004_no,normal,exec_compatible_with=[];yes/no;//attr:constraint_value
l10_a005;exec_group_compatible_with;^\\{group=\\[//attr:constraint_value\\]\\}$;//attr:l10_a005_yes,normal,exec_group_compatible_with={group=[//attr:constraint_value]};//attr:l10_a005_no,normal,exec_group_compatible_with={};yes/no;//attr:constraint_value
l10_a006;target_compatible_with;^\\[//attr:constraint_value\\]$;//attr:l10_a006_yes,normal,target_compatible_with=[//attr:constraint_value];//attr:l10_a006_no,normal,target_compatible_with=[];yes/no;//attr:constraint_value
l10_a007;$config_dependencies;^\\[//attr:cfg_a\\]$;//attr:l10_a007_yes,normal,selector_key=//attr:cfg_a;//attr:l10_a007_no,normal,no_selector_keys;yes/no;//attr:cfg_a
l10_a008;args;^\\[a\\]$;//attr:l10_a008_yes,exec,args=[a];//attr:l10_a008_no,exec,args=[];yes/no;-
l10_a009;output_licenses;^\\[notice\\]$;//attr:l10_a009_yes,exec,output_licenses=[notice];//attr:l10_a009_no,exec,output_licenses=[];yes/no;-
l10_a010;$is_executable;^1$;//attr:l10_a010_yes,exec,$is_executable=1;//attr:l10_a010_no,normal,$is_executable=absent;yes/no;-
l10_a011;build_setting_default;^base$;//attr:l10_a011_yes,string_setting,build_setting_default=base;//attr:l10_a011_no,string_setting,build_setting_default=other;yes/no;-
l10_a012;help;^$;//attr:l10_a012_yes,string_setting,help=;//attr:l10_a012_no,string_setting,help=describe;yes/no;-
l11_a001;testonly;^1$;//attr:l11_a001_yes,test,testonly=1;//attr:l11_a001_no,normal,testonly=0;yes/no;-
l11_a002;size;^medium$;//attr:l11_a002_yes,test,size=medium;//attr:l11_a002_no,test,size=large;yes/no;-
l11_a003;timeout;^moderate$;//attr:l11_a003_yes,test,size=medium,timeout=moderate;//attr:l11_a003_no,test,size=small,timeout=short;yes/no;-
l11_a004;flaky;^0$;//attr:l11_a004_yes,test,flaky=0;//attr:l11_a004_no,test,flaky=1;yes/no;-
l11_a005;shard_count;^-1$;//attr:l11_a005_yes,test,shard_count=-1;//attr:l11_a005_no,test,shard_count=0;yes/no;-
l11_a006;local;^0$;//attr:l11_a006_yes,test,local=0;//attr:l11_a006_no,test,local=1;yes/no;-
l11_a007;args;^\\[p, a, p\\]$;//attr:l11_a007_yes,test,args=[p,a,p];//attr:l11_a007_no,test,args=[p,a];yes/no;-
l11_a008;$is_executable;^1$;//attr:l11_a008_yes,test,$is_executable=1;//attr:l11_a008_no,normal,$is_executable=absent;yes/no;-
l11_a009;$test_wrapper;^@@bazel_tools//tools/test:test_wrapper$;//attr:l11_a009_yes,test,$test_wrapper=@@bazel_tools//tools/test:test_wrapper;//attr:l11_a009_no,normal,$test_wrapper=absent;yes/no;@@bazel_tools//tools/test:test_wrapper
l11_a010;$xml_writer;^@@bazel_tools//tools/test:xml_writer$;//attr:l11_a010_yes,test,$xml_writer=@@bazel_tools//tools/test:xml_writer;//attr:l11_a010_no,normal,$xml_writer=absent;yes/no;@@bazel_tools//tools/test:xml_writer
l11_a011;$test_runtime;^\\[@@bazel_tools//tools/test:runtime\\]$;//attr:l11_a011_yes,test,$test_runtime=[@@bazel_tools//tools/test:runtime];//attr:l11_a011_no,normal,$test_runtime=absent;yes/no;@@bazel_tools//tools/test:runtime
l11_a012;$test_setup_script;^@@bazel_tools//tools/test:test_setup$;//attr:l11_a012_yes,test,$test_setup_script=@@bazel_tools//tools/test:test_setup;//attr:l11_a012_no,normal,$test_setup_script=absent;yes/no;@@bazel_tools//tools/test:test_setup
l11_a013;$xml_generator_script;^@@bazel_tools//tools/test:test_xml_generator$;//attr:l11_a013_yes,test,$xml_generator_script=@@bazel_tools//tools/test:test_xml_generator;//attr:l11_a013_no,normal,$xml_generator_script=absent;yes/no;@@bazel_tools//tools/test:test_xml_generator
l11_a014;$collect_coverage_script;^@@bazel_tools//tools/test:collect_coverage$;//attr:l11_a014_yes,test,$collect_coverage_script=@@bazel_tools//tools/test:collect_coverage;//attr:l11_a014_no,normal,$collect_coverage_script=absent;yes/no;@@bazel_tools//tools/test:collect_coverage
l11_a015;:coverage_support;^@@bazel_tools//tools/test:coverage_support$;//attr:l11_a015_yes,test,:coverage_support=@@bazel_tools//tools/test:coverage_support;//attr:l11_a015_no,normal,:coverage_support=absent;yes/no;@@bazel_tools//tools/test:coverage_support
l11_a016;:coverage_report_generator;^@@bazel_tools//tools/test:coverage_report_generator$;//attr:l11_a016_yes,test,:coverage_report_generator=@@bazel_tools//tools/test:coverage_report_generator;//attr:l11_a016_no,normal,:coverage_report_generator=absent;yes/no;@@bazel_tools//tools/test:coverage_report_generator
l12_a001;tests;^\\[//attr:explicit_member_test\\]$;//attr:l12_a001_yes,test_suite,tests=[//attr:explicit_member_test],tags=[suite];//attr:l12_a001_no,test_suite,tests=[],tags=[suite];yes/no;//attr:explicit_member_test
l12_a002;$implicit_tests;^\\[//attr:implicit_member_test\\]$;//attr:l12_a002_yes,test_suite,tests=[],tags=[suite],implicit_tests=[//attr:implicit_member_test];//attr:l12_a002_no,test_suite,tests=[//attr:explicit_member_test],tags=[suite],implicit_tests=[];yes/no;//attr:implicit_member_test,//attr:explicit_member_test
l12_a003;$allowlist_function_transition;^@@bazel_tools//tools/allowlists/function_transition_allowlist:function_transition_allowlist$;//attr:l12_a003_yes,normal,identity_transition,outputs=[//attr:base_string_setting],allowlist=@@bazel_tools//tools/allowlists/function_transition_allowlist:function_transition_allowlist;//attr:l12_a003_no,normal,no_transition,allowlist=absent;yes/no;//attr:base_string_setting,@@bazel_tools//tools/allowlists/function_transition_allowlist:function_transition_allowlist
l13_a001;name;^l13_a001_yes$;//attr:l13_a001_yes,filegroup,name;//attr:l13_a001_no,filegroup,name;yes/no;-
l13_a002;visibility;^\\[//visibility:public\\]$;//attr:l13_a002_yes,filegroup,visibility=[//visibility:public];//attr:l13_a002_no,filegroup,visibility=[//visibility:private];yes/no;-
l13_a003;transitive_configs;^\\[\\]$;//attr:l13_a003_yes,filegroup,transitive_configs=[];//attr:l13_a003_no,filegroup,transitive_configs=[//attr:cfg_a];yes/no;//attr:cfg_a
l13_a004;deprecation;^deprecated$;//attr:l13_a004_yes,filegroup,package_deprecation=deprecated;@@ext+//leaf:l13_a004_no,filegroup,package_deprecation=null;yes/no;@@ext+//leaf
l13_a005;tags;^\\[a, z\\]$;//attr:l13_a005_yes,filegroup,tags=[z,a],OI;//attr:l13_a005_no,filegroup,tags=[a];yes/no;-
l13_a006;generator_name;^$;//attr:l13_a006_yes,filegroup,direct,generator_name=;//attr:l13_a006_no,filegroup,legacy_macro,generator_name=macro_case;yes/no;legacy_macro
l13_a007;generator_function;^$;//attr:l13_a007_yes,filegroup,direct,generator_function=;//attr:l13_a007_no,filegroup,legacy_macro,generator_function=legacy_macro;yes/no;legacy_macro
l13_a008;generator_location;^$;//attr:l13_a008_yes,filegroup,direct,generator_location=;//attr:l13_a008_no,filegroup,legacy_macro,generator_location=attr/BUILD.bazel:line:column;yes/no;legacy_macro
l13_a009;testonly;^1$;//attr:l13_a009_yes,filegroup,package_testonly=1;//attr:l13_a009_no,filegroup,testonly=0;yes/no;-
l13_a010;features;^\\[a, z\\]$;//attr:l13_a010_yes,filegroup,features=[z,a],OI;//attr:l13_a010_no,filegroup,features=[a],OI;yes/no;-
l13_a011;:action_listener;^\\[\\]$;//attr:l13_a011_yes,filegroup,:action_listener=[];//attr:l13_a011_no,alias,:action_listener=absent,actual=//attr:leaf;yes/no;//attr:leaf
l13_a012;compatible_with;^\\[//attr:constraint_value\\]$;//attr:l13_a012_yes,filegroup,compatible_with=[//attr:constraint_value];//attr:l13_a012_no,filegroup,compatible_with=[];yes/no;//attr:constraint_value
l13_a013;restricted_to;^\\[//attr:constraint_value\\]$;//attr:l13_a013_yes,filegroup,restricted_to=[//attr:constraint_value];//attr:l13_a013_no,filegroup,restricted_to=[];yes/no;//attr:constraint_value
l13_a014;$config_dependencies;^\\[//attr:cfg_a\\]$;//attr:l13_a014_yes,filegroup,selector_key=//attr:cfg_a;//attr:l13_a014_no,filegroup,no_selector_keys;yes/no;//attr:cfg_a
l13_a015;package_metadata;^\\[//attr:metadata\\]$;//attr:l13_a015_yes,filegroup,package_metadata=[//attr:metadata];//attr:l13_a015_no,filegroup,package_metadata=[];yes/no;//attr:metadata
l13_a016;aspect_hints;^\\[//attr:leaf\\]$;//attr:l13_a016_yes,filegroup,aspect_hints=[//attr:leaf];//attr:l13_a016_no,filegroup,aspect_hints=[];yes/no;//attr:leaf
l13_a017;licenses;^\\[notice\\]$;//attr:l13_a017_yes,filegroup,package_licenses=[notice];//attr:l13_a017_no,filegroup,licenses=[none];yes/no;-
l13_a018;distribs;^\\[internal\\]$;//attr:l13_a018_yes,filegroup,distribs=[internal];//attr:l13_a018_no,filegroup,distribs=[];yes/no;-
l13_a019;target_compatible_with;^\\[//attr:constraint_value\\]$;//attr:l13_a019_yes,filegroup,target_compatible_with=[//attr:constraint_value];//attr:l13_a019_no,filegroup,target_compatible_with=[];yes/no;//attr:constraint_value
l13_a020;srcs;^\\[//attr:leaf\\]$;//attr:l13_a020_yes,filegroup,srcs=[//attr:leaf];//attr:l13_a020_no,filegroup,srcs=[];yes/no;//attr:leaf
l13_a021;output_group;^group$;//attr:l13_a021_yes,filegroup,output_group=group;//attr:l13_a021_no,filegroup,output_group=;yes/no;-
l13_a022;data;^\\[//attr:leaf\\]$;//attr:l13_a022_yes,filegroup,data=[//attr:leaf];//attr:l13_a022_no,filegroup,data=[];yes/no;//attr:leaf
l13_a023;output_licenses;^\\[notice\\]$;//attr:l13_a023_yes,filegroup,output_licenses=[notice];//attr:l13_a023_no,filegroup,output_licenses=[];yes/no;-
l14_a001;actual;^//attr:leaf$;//attr:l14_a001_yes,alias,actual=//attr:leaf;//attr:l14_a001_no,alias,actual=//attr:BUILD.bazel;yes/no;//attr:leaf,//attr:BUILD.bazel
l14_a002;no_match_error;^no match$;//attr:l14_a002_yes,toolchain_type,no_match_error=no match;//attr:l14_a002_no,toolchain_type,no_match_error=;yes/no;-
l14_a003;licenses;^\\[notice\\]$;//attr:l14_a003_yes,filegroup,package_licenses=[notice];//attr:l14_a003_no,alias,licenses=absent;yes/no;-
l14_a004;distribs;^\\[internal\\]$;//attr:l14_a004_yes,filegroup,distribs=[internal];//attr:l14_a004_no,toolchain_type,distribs=absent;yes/no;-
l14_a005;:action_listener;^\\[\\]$;//attr:l14_a005_yes,filegroup,:action_listener=[];//attr:l14_a005_no,alias,:action_listener=absent;yes/no;-
l15_a001;tags;^\\[manual\\]$;//attr:l15_a001_yes,config_setting,tags=[manual];//attr:l15_a001_no,filegroup,tags=[];yes/no;-
l15_a002;licenses;^\\[none\\]$;//attr:l15_a002_yes,config_setting,licenses=[none];//attr:l15_a002_no,filegroup,package_licenses=[notice];yes/no;-
l15_a003;values;^\\{mode=fast\\}$;//attr:l15_a003_yes,config_setting,values={mode=fast};//attr:l15_a003_no,config_setting,values={};yes/no;-
l15_a004;define_values;^\\{feature=on\\}$;//attr:l15_a004_yes,config_setting,define_values={feature=on};//attr:l15_a004_no,config_setting,define_values={};yes/no;-
l15_a005;flag_values;^\\{//attr:flag=on\\}$;//attr:l15_a005_yes,config_setting,flag_values={//attr:flag=on};//attr:l15_a005_no,config_setting,flag_values={};yes/no;//attr:flag
l15_a006;constraint_values;^\\[//attr:constraint_value\\]$;//attr:l15_a006_yes,config_setting,constraint_values=[//attr:constraint_value];//attr:l15_a006_no,config_setting,constraint_values=[];yes/no;//attr:constraint_value
l15_a007;:flag_alias_settings;^\\[\\]$;//attr:l15_a007_yes,config_setting,:flag_alias_settings=[];//attr:l15_a007_no,normal,:flag_alias_settings=absent;yes/no;-
l15_a008;compatible_with;^\\[\\]$;//attr:l15_a008_yes,filegroup,compatible_with=[];//attr:l15_a008_no,config_setting,compatible_with=absent;yes/no;-
l15_a009;restricted_to;^\\[\\]$;//attr:l15_a009_yes,filegroup,restricted_to=[];//attr:l15_a009_no,config_setting,restricted_to=absent;yes/no;-
l15_a010;target_compatible_with;^\\[\\]$;//attr:l15_a010_yes,filegroup,target_compatible_with=[];//attr:l15_a010_no,config_setting,target_compatible_with=absent;yes/no;-
l16_a001;testonly;^1$;//attr:l16_a001_yes,test_suite,testonly=1;//attr:l16_a001_no,filegroup,testonly=0;yes/no;-
l16_a002;tests;^\\[//attr:explicit_member_test\\]$;//attr:l16_a002_yes,test_suite,tests=[//attr:explicit_member_test],tags=[suite];//attr:l16_a002_no,test_suite,tests=[],tags=[suite];yes/no;//attr:explicit_member_test
l16_a003;$implicit_tests;^\\[//attr:implicit_member_test\\]$;//attr:l16_a003_yes,test_suite,tests=[],tags=[suite],implicit_tests=[//attr:implicit_member_test];//attr:l16_a003_no,test_suite,tests=[//attr:explicit_member_test],tags=[suite],implicit_tests=[];yes/no;//attr:implicit_member_test,//attr:explicit_member_test
l16_a004;default_constraint_value;^//attr:constraint_value$;//attr:l16_a004_yes,constraint_setting,default_constraint_value=//attr:constraint_value;//attr:l16_a004_no,constraint_setting,default_constraint_value=null;yes/no;//attr:constraint_value
l16_a005;refines_constraint_value;^//attr:constraint_value$;//attr:l16_a005_yes,constraint_setting,refines_constraint_value=//attr:constraint_value;//attr:l16_a005_no,constraint_setting,refines_constraint_value=null;yes/no;//attr:constraint_value
l16_a006;:action_listener;^\\[\\]$;//attr:l16_a006_yes,filegroup,:action_listener=[];//attr:l16_a006_no,constraint_setting,:action_listener=absent;yes/no;-
l16_a007;package_metadata;^\\[\\]$;//attr:l16_a007_yes,filegroup,package_metadata=[];//attr:l16_a007_no,constraint_setting,package_metadata=absent;yes/no;-
l16_a008;compatible_with;^\\[\\]$;//attr:l16_a008_yes,filegroup,compatible_with=[];//attr:l16_a008_no,constraint_setting,compatible_with=absent;yes/no;-
l16_a009;restricted_to;^\\[\\]$;//attr:l16_a009_yes,filegroup,restricted_to=[];//attr:l16_a009_no,constraint_setting,restricted_to=absent;yes/no;-
l16_a010;target_compatible_with;^\\[\\]$;//attr:l16_a010_yes,filegroup,target_compatible_with=[];//attr:l16_a010_no,constraint_setting,target_compatible_with=absent;yes/no;-
l16_a011;constraint_setting;^//attr:constraint_setting$;//attr:l16_a011_yes,constraint_value,constraint_setting=//attr:constraint_setting;//attr:l16_a011_no,constraint_value,constraint_setting=//attr:other_constraint_setting;yes/no;//attr:constraint_setting,//attr:other_constraint_setting
l16_a012;:action_listener;^\\[\\]$;//attr:l16_a012_yes,filegroup,:action_listener=[];//attr:l16_a012_no,constraint_value,:action_listener=absent;yes/no;-
l16_a013;package_metadata;^\\[\\]$;//attr:l16_a013_yes,filegroup,package_metadata=[];//attr:l16_a013_no,constraint_value,package_metadata=absent;yes/no;-
l16_a014;compatible_with;^\\[\\]$;//attr:l16_a014_yes,filegroup,compatible_with=[];//attr:l16_a014_no,constraint_value,compatible_with=absent;yes/no;-
l16_a015;restricted_to;^\\[\\]$;//attr:l16_a015_yes,filegroup,restricted_to=[];//attr:l16_a015_no,constraint_value,restricted_to=absent;yes/no;-
l16_a016;target_compatible_with;^\\[\\]$;//attr:l16_a016_yes,filegroup,target_compatible_with=[];//attr:l16_a016_no,constraint_value,target_compatible_with=absent;yes/no;-
l17_a001;tags;^\\[manual\\]$;//attr:l17_a001_yes,platform,tags=[manual];//attr:l17_a001_no,filegroup,tags=[];yes/no;-
l17_a002;constraint_values;^\\[//attr:constraint_value\\]$;//attr:l17_a002_yes,platform,constraint_values=[//attr:constraint_value];//attr:l17_a002_no,platform,constraint_values=[];yes/no;//attr:constraint_value
l17_a003;parents;^\\[//attr:parent_platform\\]$;//attr:l17_a003_yes,platform,parents=[//attr:parent_platform];//attr:l17_a003_no,platform,parents=[];yes/no;//attr:parent_platform
l17_a004;remote_execution_properties;^remote$;//attr:l17_a004_yes,platform,remote_execution_properties=remote;//attr:l17_a004_no,platform,remote_execution_properties=;yes/no;-
l17_a005;exec_properties;^\\{cpu=k8\\}$;//attr:l17_a005_yes,platform,exec_properties={cpu=k8};//attr:l17_a005_no,platform,exec_properties={};yes/no;-
l17_a006;flags;^\\[--cpu=k8\\]$;//attr:l17_a006_yes,platform,flags=[--cpu=k8];//attr:l17_a006_no,platform,flags=[];yes/no;-
l17_a007;missing_toolchain_error;^For more information on platforms or toolchains see https://bazel\\.build/concepts/platforms-intro\\.$;//attr:l17_a007_yes,platform,missing_toolchain_error=default;//attr:l17_a007_no,platform,missing_toolchain_error=custom;yes/no;-
l17_a008;required_settings;^\\[//attr:required_config_setting\\]$;//attr:l17_a008_yes,platform,required_settings=[//attr:required_config_setting];//attr:l17_a008_no,platform,required_settings=[];yes/no;//attr:required_config_setting
l17_a009;check_toolchain_types;^0$;//attr:l17_a009_yes,platform,check_toolchain_types=0;//attr:l17_a009_no,platform,check_toolchain_types=1;yes/no;-
l17_a010;allowed_toolchain_types;^\\[//attr:toolchain_type\\]$;//attr:l17_a010_yes,platform,allowed_toolchain_types=[//attr:toolchain_type];//attr:l17_a010_no,platform,allowed_toolchain_types=[];yes/no;//attr:toolchain_type
l17_a011;:action_listener;^\\[\\]$;//attr:l17_a011_yes,filegroup,:action_listener=[];//attr:l17_a011_no,platform,:action_listener=absent;yes/no;-
l17_a012;package_metadata;^\\[\\]$;//attr:l17_a012_yes,filegroup,package_metadata=[];//attr:l17_a012_no,platform,package_metadata=absent;yes/no;-
l17_a013;compatible_with;^\\[\\]$;//attr:l17_a013_yes,filegroup,compatible_with=[];//attr:l17_a013_no,platform,compatible_with=absent;yes/no;-
l17_a014;restricted_to;^\\[\\]$;//attr:l17_a014_yes,filegroup,restricted_to=[];//attr:l17_a014_no,platform,restricted_to=absent;yes/no;-
l17_a015;target_compatible_with;^\\[\\]$;//attr:l17_a015_yes,filegroup,target_compatible_with=[];//attr:l17_a015_no,platform,target_compatible_with=absent;yes/no;-
l18_a001;tags;^\\[manual\\]$;//attr:l18_a001_yes,toolchain,tags=[manual];//attr:l18_a001_no,filegroup,tags=[];yes/no;-
l18_a002;target_compatible_with;^\\[\\]$;//attr:l18_a002_yes,toolchain,target_compatible_with=[];//attr:l18_a002_no,filegroup,target_compatible_with=[//attr:constraint_value];yes/no;//attr:constraint_value
l18_a003;toolchain_type;^//attr:toolchain_type$;//attr:l18_a003_yes,toolchain,toolchain_type=//attr:toolchain_type;//attr:l18_a003_no,toolchain,toolchain_type=//attr:other_toolchain_type;yes/no;//attr:toolchain_type,//attr:other_toolchain_type
l18_a004;toolchain;^//attr:leaf$;//attr:l18_a004_yes,toolchain,toolchain=//attr:leaf;//attr:l18_a004_no,toolchain,toolchain=//attr:BUILD.bazel;yes/no;//attr:leaf,//attr:BUILD.bazel
l18_a005;exec_compatible_with;^\\[//attr:constraint_value\\]$;//attr:l18_a005_yes,toolchain,exec_compatible_with=[//attr:constraint_value];//attr:l18_a005_no,toolchain,exec_compatible_with=[];yes/no;//attr:constraint_value
l18_a006;use_target_platform_constraints;^0$;//attr:l18_a006_yes,toolchain,use_target_platform_constraints=0;//attr:l18_a006_no,toolchain,use_target_platform_constraints=1;yes/no;-
l18_a007;target_settings;^\\[//attr:target_config_setting\\]$;//attr:l18_a007_yes,toolchain,target_settings=[//attr:target_config_setting];//attr:l18_a007_no,toolchain,target_settings=[];yes/no;//attr:target_config_setting
l18_a008;:action_listener;^\\[\\]$;//attr:l18_a008_yes,filegroup,:action_listener=[];//attr:l18_a008_no,toolchain,:action_listener=absent;yes/no;-
l18_a009;compatible_with;^\\[\\]$;//attr:l18_a009_yes,filegroup,compatible_with=[];//attr:l18_a009_no,toolchain,compatible_with=absent;yes/no;-
l18_a010;restricted_to;^\\[\\]$;//attr:l18_a010_yes,filegroup,restricted_to=[];//attr:l18_a010_no,toolchain,restricted_to=absent;yes/no;-
```
<!-- attr-manifest-records:end -->

Negative-only controls, excluded from the 165 records: `attr(name,"^.*$",
//attr:BUILD.bazel)` is no because a source file is not a Rule;
`attr(name,"^.*$",//attr:l01_generated_nonrule)` is no because that generated
output is not a Rule; and `attr(name,"^.*$",//attr:l01_package_group_nonrule)`
is no because a package group is not a Rule. `attr(nullable_output,"^.*$",
//attr:l03_null_output)` is no for the null output default; and
`attr(_private,"^.*$",//attr:l03_private_spelling)` is no because the private
schema spelling is `$private`. These five controls consume neither IDs nor pair
instances.

Lane 4 also has two negative-only spelling controls, excluded from the record
count: each OI declaration above is queried with `^\\[z, a, z\\]$` and selects
nothing. They establish that declaration order is not the rendered order.
The remaining two negative-only controls are
`attr(:run_under_exec_config,"^.*$",//attr:l11_run_under_exec_null)` and
`attr(:run_under_target_config,"^.*$",//attr:l11_run_under_target_null)`;
both have null loading fallbacks and select nothing. Together with the five
listed above and the two lane-4 controls, these are the nine controls.

Lane 7 is three independent ordinary-query rows, whose exact normalized stdout
is respectively `//attr:l07_a001_yes`, `//attr:l07_a002_yes`, and
`//attr:l07_a003_yes`. For every row the positive equal-key pair has candidate
set `{aa,bb,dd}`. Its negative is also two `select`s over exactly
`cfg_a,cfg_b,default`, with correlated candidate set `{ab,bd,da}`. The queried
`aa`, `bb`, or `dd` is absent from that correct set, while an erroneous
distinct-key cross-product of the two negative select value sets would include
all three. This is the correlation discriminator; no negative is a scalar
shortcut.

The three retained external baselines all live in the one existing
`modules/ext/leaf/BUILD.bazel` package, which loads `@@//attr:defs.bzl`; no
sixth source is introduced. `@@ext+//leaf:l02_a007_no` and
`@@ext+//leaf:l09_a004_no` are identical main-defined normal rules with the
external package's null deprecation default. `@@ext+//leaf:l13_a004_no` is a
native `filegroup` with that same null package default. They are distinct
external probe instances; `filegroup(name="label")` remains the lane-6
external leaf in the same file.

Correction-only independent rereview returned `ACCEPT` for the complete
165-record manifest, its constructibility and discriminators, and the retained
five-file/two-package boundary. Run next only
`WP-4-8-m3-attr-two-package-observable-candidate-oracle-generation`; generation
must reproduce SHA-256
`99b772e6a8a19540ad379792fe5db7c8683d50d6e8af282ba55766585242300d`
before transcribing any row.

Generation preflight then returned `REPLAN` before any fixture, payload, Bazel,
Cargo, or JVM action. The accepted seven-field records freeze semantic atoms,
but values such as `select(same_keys=...)` are not literal Starlark and do not
freeze the complete five source bodies: rule callable definitions and attrs,
support declarations, selector dictionaries, macro implementation/location,
and native BUILD syntax still require author choices. Inferring those choices
inside generation violates the packet contract.

Run next only `WP-4-8-m3-attr-five-source-template-oracle-design`. Preserve the
165-record vector and SHA above, but add exact LF source bodies with per-file
digests, literal 18 argv/stdout bindings, and a bijective record-to-declaration
audit. Validate the proposed bodies twice with pinned Bazel 9.2 from temporary
roots, then remove them. Add no fixture, payload, expected record, production
Rust, Cargo, graph/DICE/regex state, JVM/Java artifact, or production Bazel
delegation during that design packet.

Disposable source-template synthesis subsequently returned `REPLAN` without
checkout edits. Bazel 9.2 loaded the five proposed bodies, but ordinary query
proved `l12_a003`'s exact `$allowlist_function_transition` value is
`@@bazel_tools//tools/allowlists/function_transition_allowlist:function_transition_allowlist`.
The current shorter anchored regex selects nothing, so source syntax cannot
repair it and the accepted semantic record/checksum must change. Both temporary
roots and output bases were removed; no fixture, payload, source template,
Rust, Cargo, JVM artifact, or generated file remains.

Run next only `WP-4-8-m3-attr-transition-allowlist-manifest-correction`. Change
only `l12_a003`'s regex/rendered/support label, recompute the unchanged 165-row
vector's digest, and obtain latest-diff review before returning to
`WP-4-8-m3-attr-five-source-template-oracle-design`.

The focused correction changes only `l12_a003` to the Bazel 9.2-observed
canonical label
`@@bazel_tools//tools/allowlists/function_transition_allowlist:function_transition_allowlist`.
The new anchored regex selects `//attr:l12_a003_yes`; the superseded shorter
regex selects nothing. The 165 IDs and vector remain unchanged, and the exact
LF record stream now has SHA-256
`99b772e6a8a19540ad379792fe5db7c8683d50d6e8af282ba55766585242300d`.
Independent latest-diff review returned `ACCEPT`: only that row changed, the
count/vector and five-file/two-package boundary are intact, and no fixture,
code, JVM, or configured-analysis work entered the correction. Resume only
`WP-4-8-m3-attr-five-source-template-oracle-design`, bound to the corrected
digest above.

The first complete source-template diff was then discarded after independent
review and correction validation returned `REPLAN`. Its five bodies and all 18
primary commands loaded and passed in two roots, but the review found hidden
construction mismatches in paired lane-1 supports, package-derived licenses,
computed timeout, lane-13 macro provenance, and suite/manual tag closure. The
focused correction proved the semantic timeout row itself is invalid: Bazel
rejects `l11_a003_no` with `size="short"` as `size 'short' is not a valid size`
and derives illegal timeout. Valid size `small` is the source of computed
timeout `short`.

Run next only `WP-4-8-m3-attr-test-timeout-manifest-correction`. Change that
one negative construction to `size=small,timeout=short`, recompute the 165-row
digest, and review the latest diff. Retain the other four source-template
review blockers for the subsequent `WP-4-8-m3-attr-five-source-template-oracle-design`
retry. The entire unaccepted 1,901-line Stage 4/8 diff, all temporary roots,
outputs, lockfiles, and helpers were removed; no fixture, payload, Rust, Cargo,
JVM artifact, or generated source remains.

The focused correction changes only `l11_a003_no` from invalid `size=short` to
valid `size=small`; its `timeout=short` remains the Bazel computed default, not
an explicit declaration. The positive remains `size=medium` with computed
`timeout=moderate`. Count 165, unique IDs, and vector
`13/7/5/4/3/3/3/6/11/12/16/3/23/5/10/16/15/10` are unchanged; the corrected LF
record stream SHA-256 is
`99b772e6a8a19540ad379792fe5db7c8683d50d6e8af282ba55766585242300d`.
Independent latest-diff review returned `ACCEPT`: exactly one semantic row
changed, the vector/architecture are unchanged, and no fixture, template, code,
JVM, or configured-analysis work entered the correction. Run next only
`WP-4-8-m3-attr-five-source-template-oracle-design-retry`, retaining the four
source-template blockers above and binding all source work to this digest.

The source-template retry returned `REPLAN` before checkout edits. A minimal
Bazel 9.2 root accepted `licenses(["notice"])` beside the required
`default_package_metadata`, but an anchored ordinary `attr("licenses",...)`
query over its filegroup returned empty. Explicit rule licenses produce the
rendering but contradict the frozen package-derived construction. The probe
root and output were removed; no source template, fixture, payload, Rust,
Cargo, JVM artifact, or generated file exists.

Run next only `WP-4-8-m3-attr-license-default-source-evidence`. Audit pinned
license ownership and execute a minimal native/Starlark/config-setting matrix
with package metadata, package licenses, and explicit licenses. Select but do
not apply either an exact package-derived construction or a finite manifest
correction, retaining the other four accepted template obligations.

## Package-license default source evidence (2026-08-09)

Pinned Bazel source closes this loading-only question. At
`src/main/java/com/google/devtools/build/lib/packages/BuildGlobals.java:106-130`,
the BUILD-only `licenses()` global parses its string list as `BuildType.LICENSE`
and stores it in `PackageArgs`. `PackageArgs.java:176-195` separately handles
`licenses` and aliases `default_applicable_licenses` to
`default_package_metadata`, rejecting their simultaneous use. The rule
accessor keeps a `BuildType.LICENSE` attribute at
`AttributeProvider.java:354-380`; the relevant legacy-disable flag is
`BuildLanguageOptions.java:476-481` (`--incompatible_no_attr_license`), which
was not supplied to the oracle. These are loading/package declarations, not
configured analysis or toolchain resolution.

The fresh Bazel 9.2 matrix used one Starlark `normal`, one native `filegroup`,
and one `config_setting`, with and without `default_package_metadata`, with and
without `licenses(["notice"])`, and with explicit native/config-setting
`licenses`. Its exact positive command was:

```text
/usr/bin/bazel query 'attr("licenses", "^\[notice\]$", //pkg:all)'
```

Without package `licenses()`, only an explicit native `filegroup` rendered
`[notice]`; `normal` has no `licenses` attr, and `config_setting` renders its
native `[none]` default even when passed an explicit license. With package
`licenses(["notice"])`, both metadata/no-metadata layouts rendered `[notice]`
for the ordinary native `filegroup` (and the metadata filegroup); explicit
filegroup licenses rendered the same value. `config_setting` remained `[none]`
and the Starlark normal remained attr-absent. The earlier empty result used an
over-escaped query regex, not a Bazel semantic boundary. No diagnostic occurred
in the valid matrix; the only rejected construction is explicit `licenses` on
the Starlark rule, which lacks that schema field.

The bounded outcome is therefore an exact source construction: retain
`package(default_package_metadata=[":metadata"])`, add one package-level
`licenses(["notice"])`, and remove explicit notice arguments from exactly
`l02_a005_yes`, `l02_a006_no`, `l09_a005_yes`, `l13_a017_yes`,
`l14_a003_yes`, and `l15_a002_no`. Their `licenses=[notice]` atoms remain
package-derived; `config_setting` `[none]` rows and the external null baseline
remain unchanged. The 165 rows, fields, vector, and SHA
`99b772e6a8a19540ad379792fe5db7c8683d50d6e8af282ba55766585242300d` require no
digest work. The successor must apply this construction together with the four
retained obligations—paired lane-1 supports, computed medium/small timeout,
lane-13 `legacy_macro` provenance, and suite/manual tag closure—then rerun the
two-root source-template oracle. No JVM, Java helper, fixture, code, configured
analysis, or production Bazel delegation is admitted.

Independent review returned `ACCEPT` for the pinned source ranges, corrected
single-backslash matrix, exactly six package-derived filegroup operands, and
the no-manifest-change outcome. Run next only
`WP-4-8-m3-attr-five-source-template-oracle-design-retry-2`, applying this
license construction with every other retained source obligation and focused
hidden probe.

## WP-4-8-m3-executables-rule-capability: Stage 4 Gate A (2026-07-23)

Oracle gate `c8e469f5` is landed and Sol-accepted: 32 semantic rows plus eight
Bazel-only rule-class representation rows passed Terra update/clean and root
clean runs `085202-880190`, `085213-881221`, and `085303-889108`. The explicit
`test=True, executable=False` row observes accepted syntax plus `_test`
exclusion only; Bazel's pinned `createRule`/test-base source establishes that
test still implies executable capability. Gate A implementation `c86fc656` is
now landed and Sol-accepted.

Retain V2-owned immutable `Allocative`
`RuleCapability { rule_class: CompactString, executable: bool }` for every
loadable rule and include both fields in `StarlarkRuleImplementation` and
`LoadedPackage` semantic equality. For Starlark rules, `RuleDefinitionGen`
captures the exact exported `.bzl` name via `StarlarkValue::export_as`, using
the bounded Buck2 rule pattern and the existing V2 provider `OnceCell`/freeze
shape; never use implementation identity or BUILD target name. Export
validation requires test classes to end `_test` and non-test classes not to;
test implies executable even when `executable=False` is explicitly supplied.

Project exact fixed native classifications only: `filegroup`, `alias`, and
`config_setting` have their Bazel class names and `executable=false`; an alias
does not inherit its actual's capability. Source, BUILD, and generated targets
remain non-rules. Do not add `test_suite` while no native global exists.
`genrule` positive/negative executable behavior is separately oracle-gated;
this packet states its current-loadable-graph boundary and stops if a full
native-positive answer is needed. No configured evaluation, provider, global
registry, new DICE key, or query activation belongs here.

Gate A tests prove false→true executable, false→true test, exported-rule rename,
and target rename crossing `_test` with unchanged classification, plus
delete/recreate and semantically equal formatting reuse through
`BzlModuleEvalKey → PackageLoadKey → focused semantic consumer/observer`.
The implementation freezes evaluator-local `OnceCell` export identity into one
shared immutable `Arc<RuleCapability>`, while the sole public target accessor
borrows it. Native capabilities are allocation-free static `CompactString`
values derived from existing variants; alias does not inherit. Exact focused
DICE evidence isolates capability equality from `load_fingerprint`: target
rename and `.bzl` formatting evaluate the capability consumer but reuse its
observer, while field changes propagate through all four keys. Delete and
byte-identical recreate also pass.

Root passed the full 40-test loading suite, analysis/query downstream
compilation, fmt, and diff; Sol final review returned `ACCEPT`. No production
DICE key or Stage 8/query path changed. Archive-status still reports the
documented unrelated missing `v1-archive` branch and stale orchestration/server
allowlists. Stage 8 Gate B may now start.

## WP-4-8-m3-visible-visibility: Stage 4 representation design (2026-07-23)

Oracle commit `3ecfbfce` accepts 34 Bazel 9.2 commands: 32 future Slug
representation/`visible()` gates and two explicitly Bazel-only flag-structure
rows. Worker regeneration and clean verification, root clean verification, and
final Sol evidence review passed. Review added a standalone
`__subpackages__` lookup failure and corrected the pinned Java call-path
provenance before `ACCEPT`.

The executable oracle corrected the pre-fixture design. `labels(visibility,
rule)` reaches `AggregatingAttributeMapper.getReachableLabels` through
`LabelsFunction` → `TargetAccessor` → `BlazeTargetAccessor`; it projects the
stored raw rule attribute. Explicit loadable group labels project, omitted
visibility stays empty despite package defaults, and explicit direct
`__pkg__`/`__subpackages__` values fail non-loadable target lookup. Ordinary
`deps` separately uses effective loadable visibility labels, including
inherited groups, while direct package specifications are values rather than
dependency targets.

The current packet is read-only. Audit current Stage 4 producers, immutable
package/target values, package defaults, attribute provenance, target variants,
query-node edges, DICE equality, and invalidation against the pinned Bazel
visibility/package-group sources. Design the smallest typed root-repository
representation for raw/effective visibility, exact/subtree/all/negative
package specifications, recursive package-group includes with cycle-safe
resolution, source/BUILD/default behavior, generated inheritance, public
package-group/fake targets, distinct NODEP/include edges, compact storage,
`Allocative`, semantic equality, and same-DICE lifecycle evidence.

Do not implement the design, activate `visible()`, add a DICE key, or enter
repository mapping, symbolic macros, configured analysis/query, formatters, or
alternate flags. V1's global locked string-pattern registry and permissive
fallback remain rejected. Stage 8 will separately own universal filtering,
same-package/Java access, recursive diagnostics, and command activation after
the Stage 4 design and implementation are independently reviewed.

### Stage 4 design review replan

Sol found two material corrections before this design can be accepted. V2
already exposes `config_setting`, so it cannot stop on that rule's Bazel 9
special default: with visibility enforcement on and private-default visibility
off by default, omitted `config_setting.visibility` is effectively public
while an explicit restriction remains enforced. Add two oracle rows before
representing this producer; `bind` remains excluded because V2 does not expose
it.

The graph representation must also use one ordered immutable tagged edge slice,
not independent buckets. Tags distinguish ordinary, visibility NODEP,
package-group include, and generating-rule edges while preserving pinned
`LabelVisitationUtils` order and allowing source synthesis from ordinary edges
only. The remaining typed visibility/group model, existing-key DICE ownership,
12-command Stage 4 gate, and bounded allowlist were accepted in principle.

### Corrected Stage 4 visibility design accepted

Commit `a11b43da` extends the executable evidence to 36 commands: 34
future-Slug rows and two final Bazel-only structure rows. Its two new
`config_setting` cases pin Bazel 9's default-public omitted visibility and
enforcement of an explicit package-group restriction under a private package
default. Generation plus two clean verification runs passed, including root
run `20260723-160559-1242065-bazel`; all 26 pinned source anchors resolve and
the prior 34 normalized records are unchanged. Sol accepted the correction.

The re-reviewed Stage 4 representation is implementation-ready. Loading owns
typed raw/effective visibility, direct package contents, unresolved top-level
group labels, first-class package-group direct contents/includes, and explicit
producer provenance. `config_setting`, `exports_files`, source/BUILD,
generated, and package-group special cases follow the pinned Bazel producers.
Compact `SmallSet`/shared-slice storage is immutable, `Allocative`, and part of
package equality.

The query graph uses one ordered tagged immutable edge slice in exact target
visitation order. Visibility NODEP and package-group include edges never
synthesize source nodes; direct package specifications never become edges.
Neither package loading nor unconfigured graph construction dereferences
group labels or recursively resolves includes. Missing/wrong-kind references
and cycles remain stored topology. Stage 8 will later resolve them iteratively
and request-locally across existing DICE-owned package graphs with a per-walk
compact cycle set and exact diagnostics.

The Stage 4 gate is exactly 12 non-`visible()` oracle commands. The other 22
future-Slug rows belong to Stage 8 and the two flag rows remain Bazel-only.
Implementation is bounded to the loading visibility/package owners, query
graph/loading-environment projection, and their focused loading, invalidation,
query, and CLI-table tests. No new DICE key, global registry/interner, V1
semantics, repository mapping, formatter, generic evaluator, or command
activation is authorized.

### Stage 4 visibility representation accepted

Commit `f9ae7337` implements the accepted representation without activating
`visible()`. Loading now retains immutable `Allocative` public/private/
restricted visibility, direct exact/subtree positive and negative package
contents, ordered unresolved group/include labels, first-class package groups,
and declared/package-default/generating-rule/always-public provenance.
Ordinary and Starlark rules, native and macro `config_setting`, exports,
source/BUILD nodes, generated outputs, and package groups follow the pinned
producer defaults. Compact persisted state uses `SmallSet` and shared slices;
no registry, interner, lock, or new DICE key was added.

The unconfigured query graph projects effective visibility, raw rule
visibility attributes, distinct package-group nodes and direct contents, plus
one ordered tagged edge slice. Rule visibility precedes ordinary edges,
generators precede inherited visibility, package-group includes retain order,
and only ordinary edges synthesize implicit source nodes. Group labels and
includes remain unresolved; recursive lookup, wrong-kind handling,
missing-target diagnostics, and cycle state remain Stage 8 work.

Root passed all 48 loading tests, all 56 query tests, the exact counted
12-command Stage 4 CLI gate, all 26 CLI/graph tests, a fresh
`slug_cli_v2` build, formatting, diff, and archive checks. Same-DICE tests cover
semantic equality, visibility and package-group edits, definition deletion,
and recreation. Sol's one correction added the missing
`native.config_setting` macro producer and its omitted-public versus
explicit-restricted regression; final review returned `ACCEPT`.

### Stage 8 visible activation design audit replan

The read-only source and ownership audit accepted the Stage 4 representation
and the future request-local accessor boundary, but stopped activation on a
narrow oracle gap. The first executable correction disproved the initial
full-target-equality reading: ordinary Bazel query's
`TargetKeyExtractor`-backed set materializes predicate targets by label.
Slug's current printed-label `eval_all` is therefore the correct predicate
boundary, while the input must remain streamed so a later public fake can
survive an earlier private real target with the same label.

Before any Rust edit, append three Bazel rows: cross-package top-level and
included-group traversal, real-first real/fake same-label input identity, and
label-keyed materialization of two same-label fake callers while retaining the
first caller's consuming package. This makes the fixture 39 rows: 25 Stage 8
`visible()` rows, the accepted 12 Stage 4 rows, and two Bazel-only rows. The
activation design
otherwise passed review: non-recording existing-key lookup, singleton passing
deliveries, full error-discovering group walks, fresh compact cycle state per
top-level root, exact diagnostics, and no loading/graph/DICE redesign.

### Stage 8 visible oracle correction accepted

Commit `a376e30e` extends `query-visible-visibility` from 36 to 39 Bazel 9.2
commands. The 25 `visible()` rows now include cross-package top-level plus
included-group resolution, streamed real-first real/fake same-label input, and
label-materialized same-label fake predicate callers retaining the first
representative's consuming package. The 12 Stage 4 rows and two final
Bazel-only rows remain separate.

Generation `20260723-165513-1276896-bazel` and clean runs
`20260723-165704-1280783-bazel` and
`20260723-165747-1284410-bazel` passed. All stable fields from the prior 36
records are unchanged, all 27 pinned anchors resolve, and independent review
returned `ACCEPT`. The executable third row corrected the audit's
full-target-equality inference: ordinary query predicate materialization uses
the label-keyed `TargetKeyExtractor`, matching Slug's existing `eval_all`.
Stage 8 design re-review may resume; production remains unchanged.

### Corrected Stage 8 visible activation design accepted

Independent re-review returned `ACCEPT`. The generic function materializes the
once-evaluated predicate through existing label-keyed `eval_all`, retains the
input's callback batches, and delegates `TargetSet` plus `Set`. The loading
accessor owns non-recording real-node lookup, fake-public handling,
same-package/Java access, complete iterative group expansion, exact top-root
diagnostic wrapping, and singleton result deliveries.

Implementation is limited to query `expr`, `generic`, `loading_environment`,
their query/loading tests, CLI tests, and compact evidence. Same-DICE lifecycle
deletes and recreates the included package-group definition while its BUILD
package remains, so it exercises the pinned missing-target error. No loading
representation, graph/provenance representation, DICE key, formatter, command
policy, repository mapping, V1 code, or Cargo manifest is authorized.

## `attr` five-source template retry-2 REPLAN (2026-08-09)

The five exact bodies, reviewed semantic constructions, and the 165-record
vector/SHA-256 `99b772e6a8a19540ad379792fe5db7c8683d50d6e8af282ba55766585242300d`
remain viable. The first review corrected the 450-line accounting, separately
counted the nine controls, required distinct scratch parents with sibling
`workspace`/`out` paths, and changed the result to pending review. Correction
rereview then found a second material evidence defect: the tag/feature OI
control argv used `^\\[z, a, z\\]$` instead of the discriminating single-backslash
regex, while the recorded generator probe used `BUILD\\.bazel` although the
passing primary used `BUILD\.bazel`. Retry-2 has consumed its one correction,
so its entire unaccepted source-template text was discarded.

No fixture, payload, code, JVM/Java, configured-analysis, toolchain, or
production-Bazel change remains; all temporary material was removed. Run next
only `WP-4-8-m3-attr-five-source-template-oracle-design-retry-3`, design-only:
freeze the exact argv in one machine-readable/executable representation and
execute those exact bytes before rendering documentation; recreate the five
exact bodies; use two independent `mktemp` parents with sibling `workspace` and
`out`; run all 18 lanes twice, all nine controls, and focused probes; then
obtain terminal independent review. Do not add a second transcription layer.

### `attr` five-source template retry-3 terminal REPLAN (2026-08-09)

Retry-3 made no Stage 4 or Stage 8 edits, created no temporary root, and ran no
Bazel command. The exact deleted bodies and their unaccepted hash anchors are
unrecoverable from `HEAD`, reachable log history, and the unreachable-object
audit. Recovering them would require a manual second representation, which
violates retry-3's executable-source contract.

Run next only
`WP-4-8-m3-attr-five-source-executable-reconstruction-design`. It may
reconstruct fresh five LF bodies from the accepted 165-row semantic manifest
and reviewed construction obligations, with new hashes expected rather than
the old unaccepted hashes. One disposable machine-readable/executable source
must own those bodies, all 18 primary argv, nine zero-output controls, focused
probes, two independent `mktemp -d` sibling `workspace`/`out` executions, and
mechanical Stage 4/Stage 8 rendering. It must retain exact single-backslash OI
`^\[z, a, z\]$` and generator
`^attr/BUILD\.bazel:[0-9]+:[0-9]+$` bytes. Stop on a manifest semantic change
or any need beyond five files/two packages; add no fixture, code, configured
analysis, toolchain, JVM/Java, or production-Bazel work.

### `attr` five-source executable reconstruction terminal REPLAN (2026-08-09)

One disposable executable representation verified the immutable 165-record
stream and reached two operational Bazel 9.2 sibling `workspace`/`out` roots,
but strict ownership failed: missing positives were `l05_a003_yes`,
`l16_a007_yes`, `l16_a013_yes`, and `l17_a012_yes`; unexpected positives were
`l13_a011_no` and `l13_a017_no`. No candidate was rendered. The sole script,
JSON result, scratch roots, output bases, and processes were removed, leaving
clean `2f83f90b`.

`l13_a017` is a body-construction omission: its negative must be an explicit
native `licenses=[none]` beside package `licenses([notice])`. The three missing
explicit-empty filegroup `package_metadata=[]` rows (`l16_a007`, `l16_a013`,
and `l17_a012`) are likewise construction omissions. `l05_a003`
`label_list_dict` rendering and `l13_a011` alias `:action_listener` fallback
remain unresolved exact-semantics questions.

Run next only `WP-4-8-m3-attr-six-ownership-mismatch-evidence`: use at most
five minimal exact Bazel 9.2 constructions plus pinned source as needed to
classify those six rows, freeze one correction decision and exact affected
records, then obtain terminal review. Do not rebuild the full corpus or add a
fixture, code, configured analysis, toolchain, JVM/Java, or production-Bazel
work.

### `attr` six-ownership-mismatch focused evidence accepted (2026-08-09)

One disposable Bazel 9.2 workspace, with four focused constructions (therefore
within the five-construction cap), resolves all six ownership questions without
pinned-source escalation. The exact ordinary-query argv and stdout were:

```text
attr("label_list_dict", "^\{a=\[//attr:leaf\], z=\[//attr:BUILD\.bazel, //attr:leaf\]\}$", (//attr:l05_a003_yes + //attr:l05_a003_no))
//attr:l05_a003_yes
attr(":action_listener", "^\[\]$", (//attr:l13_a011_yes + //attr:l13_a011_no))
//attr:l13_a011_yes
attr("licenses", "^\[notice\]$", (//attr:l13_a017_yes + //attr:l13_a017_no))
//attr:l13_a017_yes
attr("package_metadata", "^\[\]$", (//attr:l16_a007_yes + //attr:l16_a007_no))
//attr:l16_a007_yes
attr("package_metadata", "^\[\]$", (//attr:l16_a013_yes + //attr:l16_a013_no))
//attr:l16_a013_yes
attr("package_metadata", "^\[\]$", (//attr:l17_a012_yes + //attr:l17_a012_no))
//attr:l17_a012_yes
```

The four constructions are: (1) a normal Starlark rule using
`attr.string_list_dict()` with the accepted `a`/`z` label strings and the
reversed negative; (2) a native filegroup positive and alias negative with
`actual = ":leaf"`; (3) package `licenses(["notice"])`, an inherited
filegroup positive, and a native `filegroup(licenses = ["none"])` negative;
and (4) three `filegroup(package_metadata = [])` positives against respectively
absent `constraint_setting`, `constraint_value`, and `platform` attrs. Thus
the sole correction decision is **no manifest correction**: retain exact rows
`l05_a003`, `l13_a011`, `l13_a017`, `l16_a007`, `l16_a013`, and `l17_a012`, the
165-row vector, and SHA-256
`99b772e6a8a19540ad379792fe5db7c8683d50d6e8af282ba55766585242300d` unchanged.
The prior six mismatches are source-synthesis/argv errors. Temporary workspace,
output base, harness, and captured output are removed; no fixture, code,
configured analysis, toolchain, JVM/Java, or production-Bazel work entered.

Independent terminal review returned `ACCEPT`: four focused constructions are
within cap, every literal query selected only its positive, and the evidence
supports one correction decision—no manifest change. Run next only
`WP-4-8-m3-attr-five-source-executable-reconstruction-retry`, applying the
accepted `string_list_dict`, alias, explicit-license, and explicit-empty-
metadata constructions in one executable source before any mechanical candidate
rendering. Preserve the five-file/two-package, hidden-construction,
single-backslash, two-root, 18-lane, nine-control, focused-probe, Rust-native,
and no-JVM/code/configured/toolchain boundaries.

### `attr` five-source executable reconstruction retry terminal REPLAN (2026-08-09)

The allowed correction closed the five prior source-contract blockers and two
independent Bazel 9.2 roots replayed all 18 primary lanes, nine literal-empty
controls, and focused probes. Correction rereview nevertheless found a second
material producer-identity divergence: the accepted generated-file owner is
`output_rule(name = "l01_generated_owner", nullable_output =
"l01_generated_nonrule")`, while the candidate declared
`l01_generated_nonrule_owner`. The empty generated-file control could not
observe that mismatch. The complete unaccepted candidate and every temporary
script, root, output base, log, and process are removed; no fixture, code,
JVM/Java, configured-analysis, or toolchain material is retained.

Run next only `WP-4-8-m3-attr-five-source-executable-reconstruction-retry-2`.
It preserves the immutable 165-row vector and SHA, exact five-file/two-package
layout, all accepted loading/source and six-row constructions, original nine
literal-empty controls, focused probes, sibling two-root replay, and mechanical
candidate/pending rendering. Its one executable representation must contain
the exact `output_rule(name = "l01_generated_owner", nullable_output =
"l01_generated_nonrule")` declaration and a focused producer-identity/source
assertion before any replay. Stop on any further material contract miss.

### `attr` five-source executable reconstruction retry-2 terminal REPLAN (2026-08-09)

The sole fresh emitter passed its 165-record checksum, five-body ownership,
source-fix, and exact generated-owner assertions. Before any primary query or
candidate rendering, Bazel 9.2 rejected its package load because it used
`attr.label(..., allow_none = True)`: `label()` reports that `allow_none` is
unexpected and suggests `allow_files`. Retry-2 has no correction budget, so no
body was repaired. The emitter, scratch root, output base, and process were
removed, leaving clean `1b1f5936`; no candidate, hash, fixture, code,
JVM/Java, configured-analysis, or toolchain material is retained.

Run next only `WP-4-8-m3-attr-five-source-executable-reconstruction-retry-3`.
Preserve every accepted retry-2 obligation, including the exact generated owner
and all prior source/load and six-row fixes. The sole source must assert that
`allow_none` occurs nowhere; its nullable label is exactly
`attr.label(default = None, allow_single_file = True)` or an accepted equivalent
without that invented keyword. A disposable Bazel package-load preflight must
pass before full two-root replay, with no correction budget.
### Built-in bazel_tools source-owner prerequisite (2026-08-11)

The Test closure audit proves external package loading cannot begin from a
mapping-only `bazel_tools` token. The verbatim 9.2 `tools/test/BUILD` loads
rules_shell and reaches platforms/config/toolchain/filegroup surfaces. Do not
prune or synthesize that package. Stage 4 next depends only on a Stage 5-owned
immutable canonical repository/source route; package and Bzl evaluation remain
deferred until that source owner is accepted.

### Built-in bazel_tools source owner accepted (2026-08-12)

Stage 5 now owns the canonical immutable route and seven exact pinned source
files without activating loading consumers. Stage 4 package and Bzl loading
remain deferred. The active closure design must enumerate the complete
`@@bazel_tools//tools/test` source/package dependency boundary before any
consumer dispatch or catalog expansion.

### Embedded test-tools closure audit REPLAN (2026-08-12)

Pinned Bazel 9.2 `buildfiles(@bazel_tools//tools/test:all)` owns the built-in
BUILD/default-toolchain Bzl and five `rules_shell+` BUILD/Bzl files;
`loadfiles` narrows executable loads to the default-toolchain Bzl plus the
three rules_shell Bzl files. Slug cannot yet represent that boundary:
repository-qualified external loads are rejected, external package labels are
canonicalized as root labels, and the external package policy rejects target
kinds present in this package.

Do not dispatch the accepted built-in source route yet. Stage 4 waits for a
Stage 5-owned injected-module/contextual repository mapping before designing
cross-repository Bzl loading and repository-context package coercion.


The module-injection audit confirms Stage 4 cannot derive context from a
root-only map: extension ids/imports and canonical module names are
post-selection state. Stage 4 remains parked while Stage 5 first retains the
complete callerless embedded MODULE value; no package/Bzl consumer dispatch is
authorized by that leaf.
## Module-extension definition loading owner audit (2026-08-12)

The accepted Bzlmod request projection in `d0d7bde7` resolves the selected
extension identity and complete owner mapping before loading. The smallest
loading-side composition seam is the existing crate-private
`HostBzlModuleEvalKey`: it already owns root-main source observation,
transitive `load()` resolution, parse/evaluation/freeze, event capture,
`BzlLoadManifest` equality, frozen-module lifetime, Need validity, and typed
source/load errors. A second evaluator, source key, or purpose-split frozen
module graph is not justified.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
constructs a `ModuleExtension` from a callable, ordered tag-class map, ordered
environment declarations, OS/architecture dependency bits, and a nonnegative
facts version. `RegularRunnableExtension.load` validates and loads the bzl
module before looking up the requested public export and verifying its
`ModuleExtension` type; environment observation and execution occur later.

Slug's shared `loading_globals()` already owns `attr.*` descriptors and the
freeze/export patterns used by retained rule definitions. It lacks only
`tag_class` and `module_extension`. The design must prove that adding those
exact Bazel definition globals to the shared bzl environment preserves the sole
Host loader and existing package-loading behavior. The exported definition may
retain the selected request, complete transitive manifest, ordered heap-free tag
schemas and factor declarations. Each admitted tag descriptor must retain kind,
mandatory, effective configurable value, coerced default, and
allow-single-file; transition/explicit-configurability and every unmodeled
restriction family fail closed during bzl evaluation. Its callable must remain
lifetime-only inside the cached `FrozenBzlModule`, never in semantic equality
or a projected value.

Run next only the docs-only
`WP-4-5-host-module-extension-definition-loading-owner-design`. Settle
callable validation; attribute admission; missing/private/wrong-kind export and
schema error ordering; retained-field A/B/A and unprojected-option negative
rows; transitive-load identity; multi-request Need/error
behavior; and exact versus Slug-native/deferred boundaries. A future successor
may use only `app/slug_loading_v2/src/package.rs` and
`app/slug_loading_v2/src/bzl_module.rs`, with colocated tests and no public
API, only if this audit proves one shared-loader implementation. Initial
credible caps are 440 production, 650 tests, and 1,090 total.

Stop on a purpose-split/second loader, retained heap/callable, repository-rule
or execution breadth, public definition surface, third Rust file, I/O,
generated repository/spec/existence work, materialization, lockfile, consumer,
or JVM/Java dependency. No Rust is authorized before independent design
acceptance and explicit implementation activation.

### Module-extension definition loading implementation activated (2026-08-12)

Independent architecture and schema-projection reviews accept the shared
Host-loader design and its complete fail-closed descriptor boundary. Run next
only `WP-4-5-host-module-extension-definition-loading-owner-implementation` in
`package.rs` and `bzl_module.rs`, under 440 production/650 test/1,090 total
caps relative to `f17bd250`. Preserve every proof and stop above; obtain fresh
independent implementation review.

### Module-extension definition loading accepted (2026-08-12)

Commit `bf2c36e9` adds the callerless loading-owned definition boundary. It
computes selected requests first, borrows the sole Host bzl loader, validates
the public extension export, and retains requests, transitive manifests, and
complete heap-free schemas while the callable remains frozen-lifetime-only.
Request terminals retain full context. Real-DICE lifecycle proof, the full
loading suite, 432/649/1,081 caps, and independent review pass.

Stage 4 now waits on the Stage 5-owned heap-independent ordered module/tag
projection. Do not publish a callable or add another loader. A later loading
composition owner may validate those raw tags against the accepted schemas and
reacquire the frozen export only at the execution boundary.

### Selected extension evaluation-input implementation activated (2026-08-12)

Independent review accepts the Stage 5 raw module/tag projection design. Run
only its two-file implementation under 240 production/360 test/600 total caps
relative to `a31cf3d9`. Stage 4 gains no loading consumer, callable, schema
composition, evaluator, or execution authority.

### Evaluation-input implementation cap REPLAN (2026-08-12)

The 240 production cap fired when post-request terminals were corrected to
retain full request/predecessor identity. Preserve the unaccepted Rust diff and
run only the docs-only r2 cap correction at 280 production/360 test/640 total,
same files and stops. Stage 4 remains inactive.

### Raw evaluation inputs accepted; composition design active (2026-08-12)

The Stage 5 r2 owner is independently accepted at 263 production/304 test/567
total lines. It publishes only heap-free selected request, root identity, and
source-order raw tag state, retains complete predecessor/error identity, and
keeps schema coercion and execution absent. Full Bzlmod/loading suites pass.

Run next only the docs-only
`WP-4-5-host-module-extension-evaluation-input-composition-design`. Audit one
callerless loading key that computes raw selected inputs before the accepted
definition loader, joins by exact load request, and type-checks/defaults the
admitted tags into heap-free module views. Freeze exact tag-class/attribute/
label/error ordering and fail closed on every unprojected kind or option. The
first slice is string, bool, i32 integer, and single label only; all containers,
outputs, oversized/deferred values, and unprojected restrictions remain
deferred. Supplied attributes use retained-map order before schema-order
mandatory/default/visibility checks. A raw terminal performs zero bzl work; an
invoked accepted loader keeps its normal event behavior. No
callable, `module_ctx`, execution, I/O, generated repository, lockfile,
materializer, consumer, second loader/evaluator, Bzlmod mutation, or Rust edit
is authorized before independent design acceptance.

### Evaluation-input composition implementation activated (2026-08-12)

Independent review accepts the bounded loading composition: selected raw
inputs precede definition loading; exact request identity joins the aggregates;
the scalar String/Bool/i32/Label matrix and two-phase Bazel error order are
frozen; every container/output/deferred family fails closed. Run only
`WP-4-5-host-module-extension-evaluation-input-composition-implementation` in
`bzl_module.rs` and `package.rs`, under 420 production/700 test/1,120 total
lines against `aee502ff`. Preserve every event, equality, proof, and terminal
stop; obtain independent implementation review.

### Evaluation-input composition accepted; pure invocation design active (2026-08-12)

Independent review accepts the loading-owned composition at 414 production,
529 test, and 943 total lines against `aee502ff`. Raw selected inputs precede
the sole definition loader; exact request identity joins the aggregates; the
String/Bool/i32/Label scalar matrix follows supplied-map then declaration-order
error semantics, including canonical-default visibility; every terminal keeps
complete predecessor/request identity. Full loading tests, the protected
Bzlmod suite, format/diff/scope/cleanup checks, and two independent reviews
pass. No callable, runtime context, execution, I/O, or generated repository is
retained or activated.

Run next only the docs-only
`WP-4-5-host-pure-module-extension-invocation-owner-design`. Audit one
loading-owned leaf that prepares inputs first, reacquires the exact export
through the sole Host bzl loader, constructs ephemeral read-only root module/
tag/context values, invokes only an empty-factor implementation, and requires
strict `None` with no repository-rule call or generated output. Retained DICE
state must remain heap-independent. Canonical/current/Stage 4/Stage 5 are the
only authorized files under 45/260/240/220/765 caps. No Rust or fixture is
authorized before independent design acceptance.

The design audit must freeze the actual Bazel 9.2 callable ABI rather than a
generic read-only context: only `ctx.modules`, `ctx.is_dev_dependency(tag)`,
and `ctx.tag_sort_key(tag)` are admitted; module name/version/is_root/tags and
declared scalar tag fields are immutable; dev is method-only and location is
debug/error-only. Every repository, external-context, facts, metadata,
isolation, and unowned root-usage member fails closed. Loader events stay with
the Host-bzl key; invocation print/throw prefixes belong to and replay from the
invocation key, while its semantic receipt stores no event batch or Starlark
lifetime state.

### Pure module-extension invocation implementation activated (2026-08-12)

Independent review accepts the pinned ABI, lifetime, event, and fail-closed
invocation design recorded in `db45d182`. Run only
`WP-4-5-host-pure-module-extension-invocation-owner-implementation` in
`bzl_module.rs`, `package.rs`, one new private `module_extension.rs`, and
`lib.rs` solely for the private module declaration, plus the four bookkeeping
plans. Cap Rust at 520 production/800 test/1,320 total formatted net lines
against `db45d182`. Preserve every admitted field, forbidden-name probe,
strict-None result, event replay, heap-absence, and terminal stop; obtain fresh
independent implementation review.

### Pure invocation production-cap REPLAN (2026-08-12)

The first compiling four-path owner is 630 production lines against
`db45d182`, before tests, so the frozen 520 cap fired. Independent review finds
the ABI wrappers, typed DICE/error/event orchestration, and frozen-lifetime
boundary irreducible by 110 safe lines. Retain the diff unaccepted and run only
the docs-only
`WP-4-5-host-pure-module-extension-invocation-owner-r2-cap-design` in the four
plans. Freeze 720 production/800 test/1,520 total caps plus four required
corrections: optional Label None allocation; all-request reacquisition/factor/
identity preflight before any invocation; ephemeral context ownership checks
for tag methods; and immutable list-valued `ctx.modules` and tag-class lists.
No Rust resumes before independent correction acceptance and explicit r2
activation. Preserve every prior ABI, event, proof, and terminal stop.

### Pure invocation implementation r2 activated (2026-08-12)

Independent review accepts the 720 production/800 test/1,520 total correction
and all four bounded fixes. Resume only the same four Rust paths against
`db45d182` as
`WP-4-5-host-pure-module-extension-invocation-owner-implementation-r2`.
Implement optional Label None, complete preflight before any callable,
ephemeral context ownership for tag methods, and immutable modules/tag-class
lists; preserve every prior ABI/event/proof/stop and obtain fresh independent
implementation review.

### Pure invocation exact-Label REPLAN (2026-08-12)

The r2 diff reaches the 720 production boundary and compiles with optional
Label None, all-request preflight, context-owned tags, immutable lists, exact
negative indexing, strict result/throw handling, and expanded lifecycle/ABI
proof. Completing the required Label row exposes the first missing shared
runtime seam: starlark-rust renders every non-string `str` and `%s` through
`collect_repr`, so a typed Label cannot have canonical string rendering and
`Label("...")` repr simultaneously. A loading-global override is insufficient
because interpolation and format paths bypass it.

Retain the four loading-path diff unaccepted and run only the docs-only
`WP-4-starlark-custom-string-protocol-design` in canonical/current/Stage 4/
Stage 5. Audit one default-preserving StarlarkValue string hook and every
standard str consumer, with synthetic distinct-str/repr proof and a bounded
future vendored-runtime allowlist/caps/stops. No Rust resumes before independent
design acceptance. Do not add a Label-specific downcast, second formatter,
JVM helper, or Slug loading divergence; exact admitted string/repr/interpolation
semantics remain required.

The audit finds one bounded seam: add a default-to-repr custom `collect_str`
operation to the generated StarlarkValue vtable, route Value/global str/%s/
format/print through it, and override it only on InvocationLabel. No derive
crate edit, type downcast, loading-global shadow, or second formatter is
needed. Freeze the exact eight Rust files and 90 production/220 test/310 total
successor recorded in current, with synthetic default/override/nesting/cycle
coverage and full Label str/repr/interpolation/format/print/DICE proof. Do not
activate Rust before independent design acceptance.

### Custom string protocol implementation activated (2026-08-12)

Independent review accepts the single-vtable protocol in `73b22cec`. Run only
`WP-4-starlark-custom-string-protocol-implementation` in the exact eight Rust
files and four plans frozen by current, under 90 production/220 test/310 total
lines against `73b22cec`. Preserve every shared consumer, default behavior,
Label proof, and stop; the retained invocation owner remains unaccepted beyond
the Label override and focused tests authorized by this prerequisite.

### Custom string implementation scope correction (2026-08-12)

Independent implementation review rejects that Git boundary: the two app paths
cannot land without also committing the retained unaccepted invocation owner.
Run only the four-plan docs correction
`WP-4-starlark-custom-string-protocol-implementation-r2-scope-design`.
Freeze the successor to the six shared starlark-rust files under the same
90/220/310 caps against `73b22cec`, require the complete synthetic global
str/repr/interpolation/format/print/nesting/cycle matrix, and defer exact Label
and loading/DICE proof to the invocation packet. No Rust resumes before
independent correction acceptance and explicit r2 activation.

### Custom string protocol implementation r2 activated (2026-08-12)

Independent review accepts the scope correction in `6215fe03`. Run only
`WP-4-starlark-custom-string-protocol-implementation-r2` in the six shared
starlark-rust files and four plans frozen by current, under 90/220/310 against
`73b22cec`. Require the complete synthetic protocol matrix and preserve every
stop. App Rust, InvocationLabel, loading, and DICE proof remain deferred to the
invocation packet.

The six-file implementation is independently accepted: isolated growth is
within 90/220/310; the complete synthetic matrix, focused 73/8/9 Starlark
suites, full loading and Bzlmod dependents, formatting, and diff checks pass.
Full vendored Starlark retains only 29 unrelated profiler/bytecode golden
baselines after 808 passing tests. Commit only the shared runtime files plus
bookkeeping; preserve every app path as unaccepted dirty state.

### Pure invocation implementation r2 resumed (2026-08-12)

The shared prerequisite is accepted in `40def0e7`. Resume only the four loading
Rust paths and four plans frozen by current as
`WP-4-5-host-pure-module-extension-invocation-owner-implementation-r2`, under
720/800/1,520 against `40def0e7`. Preserve every accepted ABI, preflight,
lifetime, event, proof, and terminal stop; no repository/global/output/I/O/
consumer/public/JVM breadth is authorized.

### Pure invocation final cap correction (2026-08-12)

The complete formatted proof passes but measures approximately 724 production,
846 tests, and 1,570 total against `40def0e7`, firing the r2 cap. Cleanup
removed two unrelated visibility widenings; do not compress the required ABI,
preflight, lifetime, event, or lifecycle discriminators. Retain the Rust diff
unaccepted and run only the four-plan docs correction
`WP-4-5-host-pure-module-extension-invocation-owner-r3-cap-design`, freezing
the same four paths/semantics/proofs/stops at 730/850/1,580. No Rust resumes
before independent acceptance and explicit r3 activation.

### Pure invocation implementation r3 activated (2026-08-12)

Independent review accepts `86f478c0`. Run only
`WP-4-5-host-pure-module-extension-invocation-owner-implementation-r3` in the
same four loading paths plus four plans under 730/850/1,580 against `40def0e7`.
Preserve every accepted semantic/proof/stop and the cleanup that removed
unrelated visibility widening; no fifth path or behavior expansion.

### Pure invocation event-contract correction (2026-08-12)

Final review finds the implementation follows the established DICE contract:
fresh evaluated activations publish invocation print batches, warm reused
activations carry no duplicate batch, and command-effect lineage, not this
callerless key, later selects reachable evaluated batches. Run only the
four-plan docs correction
`WP-4-5-host-pure-module-extension-invocation-event-contract-r4-design`.
Preserve the same four paths, 730/850/1,580 caps, semantics, proof, and stops;
rename the overclaiming test and defer command-output integration until a real
consumer exists. No Rust resumes before acceptance and explicit r4 activation.

### Pure invocation implementation r4 activated (2026-08-12)

Independent review accepts `f36ec593`. Run only
`WP-4-5-host-pure-module-extension-invocation-owner-implementation-r4` in the
same four loading paths plus four plans under 730/850/1,580 against `40def0e7`.
Rename the focused event test to publication plus semantic reuse and preserve
the accepted evaluated/reused/command-lineage boundary, all prior proofs, and
every stop.

Final implementation review accepts the four-path owner at approximately
724/846/1,570. Full loading and Bzlmod suites, the renamed evaluated/reused
event row, formatting, and diff checks pass. Retained state is heap/callable
free; lifetime values remain local to preflight. Commit the exact authorized
paths and no visibility, repository, output, I/O, consumer, or public breadth.

### Repository-rule definition owner scheduled (2026-08-12)

Pure invocation is accepted in `986ccebd`. Generated repositories do not come
from an extension return value; they are repository-rule call side effects,
and shared loading currently omits `repository_rule`. Run only the four-plan
docs audit `WP-4-5-host-repository-rule-definition-owner-design` under
45/240/200/120/605. Audit pinned definition parameters/defaults/errors,
callable/schema/export/definition identity, frozen lifetime versus heap-free
projection, and the smallest future loading-owned successor. No Rust, fixture,
repository call, generated `RepoSpec`, context, I/O, materializer, consumer,
public API, or JVM work is authorized before independent design acceptance.

### Repository-rule definition audit REPLAN (2026-08-12)

Pinned Bazel 9.2 tag commit `8220c619` shows that
`repository_rule()` does not yield a standalone semantic definition leaf: it
creates an immutable exported Starlark callable, and
`ModuleExtensionEvalStarlarkThreadContext.lazilyCreateRepo` is its first
semantic consumer. The callable captures name/raw kwargs and call provenance;
only later `createRepos` applies full mappings, schema defaults/type checks,
and `RepoRule.instantiate` to produce RepoSpecs. Slug's
`BzlEvaluationContext`, sole `HostBzlModuleEvalKey`, frozen module lifetime,
and pure-invocation preflight already own the corresponding inputs.

Run only the four-plan docs audit
`WP-4-5-host-module-extension-repository-rule-call-protocol-design` under
45/260/240/180/725. Design one shared loading global/exported lifetime value
plus ephemeral invocation-local sink that emits ordered heap-free raw call
records, with exact positional/context/export/name/duplicate/deep-clone order
for a bounded root-main ordinary slice. The future ceiling is `package.rs`,
existing private `module_extension.rs`, one private
`module_extension_repository_rule.rs`, and `lib.rs` solely for its private
declaration. No Rust resumes before independent design acceptance. RepoSpec,
schema application, repository implementation/context, generated naming or
existence, I/O, materialization, consumers, public API, and JVM remain
deferred; a second loader/key or retained heap/callable is forbidden.

### Repository-rule call-protocol design completed (2026-08-12)

The completed audit keeps the existing
`HostPureModuleExtensionInvocationsKey` as sole semantic owner and adds no
DICE leaf. The accepted shape installs one ephemeral evaluator-extra sink per
extension invocation, reuses the sole Host loader/frozen-module lifetime, and
projects only ordered heap-free scalar call records into success receipts or
typed terminal prefixes. All request/module/factor preflight remains before
any invocation or capture; schema application and `RepoRule.instantiate`
remain later.

The admitted definition surface is callable implementation, optional
source-ordered public scalar attrs, and default false/empty/None options.
Definitions retain canonical defining label, ordered schema, optional exported
name, and lifetime-only callable; anonymous values freeze but fail when called,
while internally private top-level rules remain callable. Calls preserve
positional/context/export/name/name-syntax/duplicate/provenance/raw-projection
order and admit only None/bool/i32/string/accepted canonical Label. Retained
kwargs and calls use ordered `Arc` slices, `CompactString`,
`CanonicalLabel`, and `Allocative`; `SmallMap` is scratch only because its
equality ignores insertion order. No new interner/cache/digest is warranted.

After independent acceptance, implement only
`WP-4-5-host-module-extension-repository-rule-call-protocol-implementation`
in `package.rs`, existing private `module_extension.rs`, one new private
`module_extension_repository_rule.rs`, and `lib.rs` solely for its private
declaration, plus four-plan bookkeeping. Caps are 650 production/850 test/
1,500 total. Require the complete definition, schema, context, error-order,
scalar/deferred, call-prefix, provenance, A/B/A, Need, events, reuse, heap-
absence, full loading/Bzlmod, formatting/archive/diff proof frozen in current.
No Rust is authorized before acceptance and explicit activation. A fifth path,
new key/lock, retained lifetime state, schema application, repository
implementation/context, RepoSpec/generated existence/mapping, I/O,
materialization, lockfile, consumer/API/JVM breadth, or cap excess is
`REPLAN`.

### Repository-rule call-protocol implementation activated (2026-08-12)

Independent review accepts the completed design in `7a49b5cd`. Run only
`WP-4-5-host-module-extension-repository-rule-call-protocol-implementation`
against that base in `package.rs`, existing private `module_extension.rs`,
one new private `module_extension_repository_rule.rs`, and `lib.rs` solely
for its private declaration, plus four-plan bookkeeping. Caps are 650
production/850 test/1,500 total. Preserve the existing sole invocation key and
all-request preflight; the exact admitted definition/export/scalar-capture
order; ordered heap-free call/error identity; lifetime/event boundaries; the
complete proof; and every generated-state, schema, I/O, public, and JVM stop.
A fifth Rust path, new key/lock, retained Starlark value, RepoSpec or repository
execution/context, behavior expansion, or cap excess is `REPLAN`.

### Repository-rule call protocol accepted; namespace prerequisite REPLAN (2026-08-12)

Independent implementation review accepts `b7c70a1b`. The shared loading
global/export value, evaluator-local raw-call sink, ordered heap-free success
and terminal prefixes, all-request preflight, lifetime/event boundaries, and
full loading/Bzlmod proof are complete within 650/850/1,500. The related
selected root input aggregation is accepted separately in `f5d64085`.

Do not proceed directly to `RepoRule.instantiate`. Loading lacks the accepted
selected extension's collision-sensitive unique prefix, pre-override namespace,
and ordered override metadata; deriving them from the final request mapping
would duplicate Bzlmod ownership. Run only the four-plan docs audit
`WP-5-host-selected-extension-generated-namespace-request-design` under
45/220/180/220/665. Stage 4 production remains unchanged. No loading Rust,
schema/default/visibility processing, RepoSpec, generated existence/final
mapping, repository implementation/context, I/O, consumer, public API, or JVM
work is authorized before the prerequisite is independently accepted and
explicitly activated.

### Selected extension namespace request implementation activated (2026-08-12)

Independent review accepts the prerequisite design in `fff82ecd`. Run only
`WP-5-host-selected-extension-generated-namespace-request-implementation`
in `selected_repo_spec.rs` and `slug_bzlmod_v2/src/lib.rs` solely for the
existing hidden accessor, plus four-plan bookkeeping, under mandatory
180/300/480 caps. Stage 4 production remains unchanged. The implementation may
retain and project selected namespace inputs only; loading, schema/RepoSpec,
existence, repository execution/context, I/O, consumers, APIs, and JVM remain
deferred.

### Generated namespace accepted; repository-rule instantiation design proposed (2026-08-12)

Independent implementation review accepts the selected namespace request in
`c7c55b17`; raw repository-rule calls remain accepted in `b7c70a1b`.
Together they supply the exact hidden request, unique prefix, root base/final
mappings, ordered override metadata, definition/schema, generated names, raw
kwargs, and provenance required to audit `createRepos` and
`RepoRule.instantiate`.

Run only the four-plan docs packet
`WP-4-5-host-module-extension-repository-rule-instantiation-owner-design`
under 45/260/240/220/765. Stage 4 production remains unchanged. Design one
loading-owned heap-free projection over the accepted invocation receipt, with
full namespace assembly before scalar schema/default/Label/visibility work and
atomic ordered RepoSpec publication. No Rust, repository implementation/context,
I/O, materialization, lockfile, consumer/API, or JVM work resumes before
independent design acceptance and explicit activation.

### Repository-rule instantiation owner audit completed (2026-08-12)

Pinned Bazel 9.2 freezes the loading-owned successor without another
prerequisite. Compute the accepted raw invocation owner first; exact-join its
embedded requests in encounter order; assemble base mapping, all generated
`unique_prefix + "+" + name` entries, then ordered substitutions; and only
then type-check and instantiate calls. `RepoRule.instantiate` ignores the four
legacy names, validates supplied kwargs before declaration-order mandatory/
default/visibility, and stores only explicitly supplied non-None nonlegacy
attributes in kwargs order. No implicit `name` or default is stored.

The future owner is private
`HostInstantiatedModuleExtensionRepositoriesKey` with heap-free success/error
values and atomic completed/current prefixes. Implement only in existing
`module_extension_repository_rule.rs`, one new private
`module_extension_repository_instantiation.rs`, and `lib.rs` solely for its
private declaration, under mandatory 480/700/1,180 caps after independent
acceptance and explicit activation. `must_exist` remains structural: pinned
`SingleExtensionFunction`, not `createRepos`, owns later override/inject
existence validation. Preserve all no-execution/I/O/lifetime/public/JVM stops.

### Repository-rule instantiation implementation activated (2026-08-12)

Independent review accepts `7616136f`. Run only
`WP-4-5-host-module-extension-repository-rule-instantiation-owner-implementation`
in existing `module_extension_repository_rule.rs`, one new private
`module_extension_repository_instantiation.rs`, and `lib.rs` solely for its
private declaration, plus four-plan bookkeeping. Caps are mandatory
480/700/1,180 against `7616136f`. Preserve exact same-index joins, namespace
then schema order, explicit-only RepoSpec attributes, atomic prefixes, complete
proof, and every no-execution/I/O/existence/public/JVM stop.

### Repository-rule instantiation proof-cap correction scheduled (2026-08-12)

Independent implementation review accepts the production topology but finds
the frozen proof incomplete. The retained, unaccepted diff measures 474
production, 572 tests, and 1,046 total against `7616136f`; exact join
corruption, substituted namespace precedence, predecessor Need/zero-event
lifecycle, complete error prefixes, and field-specific A/B/A rows cannot
credibly fit the remaining 128 test lines.

Run only the four-plan docs packet
`WP-4-5-host-module-extension-repository-rule-instantiation-owner-r2-cap-design`.
Retain the same three Rust paths and all semantics/stops, correct only the
future caps to 480 production/900 tests/1,380 total, and authorize no Rust
until independent acceptance and explicit r2 activation.

### Repository-rule instantiation r2 activated (2026-08-12)

Independent review accepts the cap correction in `7cf2e45f`. Resume only
`WP-4-5-host-module-extension-repository-rule-instantiation-owner-implementation-r2`
in the same three Rust paths plus four ledgers, under corrected mandatory
480/900/1,380 caps against `7616136f`. Complete the exact join, substituted
namespace precedence, predecessor Need/zero-event, completed/current prefix,
and field-specific A/B/A proof without production growth or semantic breadth.
All prior stops remain.

### Repository instantiation accepted; validation-input prerequisite scheduled (2026-08-12)

Independent review accepts `d50f02a2` at 474 production/799 tests/1,273 total.
Pinned `SingleExtensionFunction` validates imported generated repositories
before override/inject polarity, but loading's exact hidden request does not
expose exported import names or proxy/override locations. Stage 4 production
remains unchanged.

Run only the four-plan docs packet
`WP-5-host-selected-extension-validation-request-projection-design` under
40/220/180/180/620. Design an existing-request-only ordered import/location
and override-location projection; no loading Rust, validation, generated
publication/routes, repository execution/context, I/O, materialization,
lockfile, consumer/API, or JVM work resumes before acceptance and activation.

### Selected validation-request projection activated (2026-08-12)

Independent review accepts `533a9453`. Run only
`WP-5-host-selected-extension-validation-request-projection-implementation`
in `selected_repo_spec.rs` and its existing hidden `lib.rs` accessors plus
the four ledgers, under 220/380/600. Stage 4 production remains unchanged.
Preserve exact import/override order and location identity; no validator,
generated publication/routes, execution, I/O, materialization, lockfile,
consumer/API, or JVM breadth.

### Validation request REPLANs at import-order identity (2026-08-12)

The retained request widening compiles, but its real DICE reorder proof fails:
the shared `NonrootRepoImports` bidirectional `SmallMap` equality ignores
insertion order, so an import-order-only edit is pruned before projection.
Stage 4 production remains unchanged.

Run only the four-plan docs packet
`WP-5-extension-import-order-identity-owner-design` under
40/220/180/180/620. Design the shared compact local-name order spine and a
corrected three-file successor under 260/450/710. No loading Rust, validator,
routes, execution/I/O/materialization/lockfile/consumer/API/JVM work resumes
before independent acceptance and activation.

### Extension import-order identity implementation activated (2026-08-12)

Independent review accepts `f14d3d7a`. Run only
`WP-5-extension-import-order-identity-owner-implementation` in
`interim_module.rs`, `selected_repo_spec.rs`, and hidden `lib.rs`
accessors plus four ledgers, under 260/450/710. Stage 4 production remains
unchanged; no validator/routes/execution/I/O/materializer/lockfile/consumer/
API/JVM breadth.

### Import identity accepted; generated-repository validation design scheduled (2026-08-12)

Independent review accepts `ff55dcbf`: the selected request now retains exact
aggregated import and override rows, order, and spans. Together with accepted
`d50f02a2` instantiation, loading owns every input required by the admitted
pinned post-evaluation validation slice.

Run only the four-plan docs packet
`WP-4-5-host-module-extension-generated-repository-validation-owner-design`
under 40/220/180/180/620. Freeze one private loading key over the instantiation
owner: exact-join requests, build only transient per-request generated-name
membership, validate imports first against generated names or override keys,
then validate override/inject polarity in retained order. Freeze a three-file
successor in the instantiation file for narrow accessors, new private validation
module, and `lib.rs` declaration under mandatory 320/650/970. No Rust resumes
before acceptance/activation; no Bzlmod mutation, route/publication, repository
execution/context, events/I/O/materializer/lockfile/consumer/API/JVM breadth.

### Generated-repository validation implementation activated (2026-08-12)

Independent review accepts design 1f7165ed. Run only
WP-4-5-host-module-extension-generated-repository-validation-owner-implementation
in the existing instantiation file for narrow accessors, new private validation
module, and private lib.rs declaration plus four ledgers, under mandatory
320/650/970. Preserve import-before-polarity order, predecessor-only success,
typed exact terminals, zero events/I/O, and every route/materialization/public
boundary.

### Generated-repository validation accepted; hidden spec publication design scheduled (2026-08-12)

Independent review accepts `b2a153aa` within 320/650/970. The private loading
owner computes only instantiation, exact-joins requests, validates aggregated
imports before override/inject polarity, retains predecessor-only success and
typed contextual terminals, and activates no registry or materializer key.
Full all-target loading and Bzlmod suites pass.

Run only the four-plan docs packet
`WP-4-5-host-validated-generated-repository-spec-publication-design` under
40/240/200/180/660. Audit a `#[doc(hidden)]` borrowed projection over the
existing validation certificate and retained canonical-name/`RepoSpec` rows.
Freeze at most the validation module, instantiation module for narrow borrowed
accessors, and `lib.rs` hidden exports under mandatory 220/420/640. Add no
new key, copied catalog, Bzlmod edit/reverse edge, route, mapping publication,
repository execution/context, I/O/materialization, lockfile, consumer/API, or
JVM breadth. REPLAN if pinned override publication or callable lifetime cannot
be separated from routing/execution.

### Validated generated-spec hidden ABI frozen (2026-08-12)

Pinned `SingleExtensionFunction` returns the eval-only value unchanged after
validation; generated rows, including overridden rows, retain their original
generated canonical identity and `RepoSpec`. Override substitutions remain a
later mapping/lookup concern. Freeze the existing validation key as the sole
hidden public key with success wrapper
`HostValidatedGeneratedRepositorySpecs` and opaque
`HostValidatedGeneratedRepositorySpecsError`. Success borrows exact
`(&CanonicalRepoName, &RepoSpec)` rows in request/call order from the private
certificate; the error wrapper exposes no private terminal fields. A later
`slug_server_v2` owner may depend on both loading and Bzlmod; no lower-core or
Bzlmod reverse dependency is permitted. Future Rust is exactly the validation,
narrow instantiation-accessor, and hidden `lib.rs` export files under
mandatory 220/420/640; all existing route/execution/I/O/public stops remain.

### Validated generated-spec hidden publication implementation activated (2026-08-12)

Independent review accepts design `433badeb`. Run only
`WP-4-5-host-validated-generated-repository-spec-publication-implementation`
in the existing validation module, instantiation module for narrow borrowed
canonical-name/`RepoSpec` accessors, and `lib.rs` solely for exact hidden
exports, plus four ledgers, under mandatory 220/420/640 against `433badeb`.
Preserve the existing sole DICE key/store, opaque error wrapper, unchanged
eval-only rows including overridden rows, server-above-both dependency
direction, and all no-route/materialization/execution/I/O/public/JVM stops.

### Validated spec publication accepted; generated-route boundary audit scheduled (2026-08-12)

Independent review accepts `d2ed6ad3`: the sole loading validation key now
publishes a hidden no-copy borrowed view of request/call-ordered generated
canonical names and original `RepoSpec` rows, including overridden rows. No
route, copied catalog, second key, materialization request, I/O, or stable
public API entered loading.

Run next only the four-plan docs packet
`WP-4-5-host-generated-repository-route-boundary-design` under
45/260/220/220/745 documentation caps. Audit pinned Bazel 9.2 post-validation
result/route construction and the live loading/Bzlmod/server dependency graph;
freeze a bounded owner, exact compatibility slice, implementation allowlist,
mandatory Rust caps, proof, and stops or `REPLAN`. Stage 4 production remains
unchanged. Do not authorize route publication, repository execution/context,
source preparation/materialization, lockfile, command/API, or JVM work.

### Generated-route audit REPLANs at retained mapping identity (2026-08-12)

Pinned Bazel 9.2 and live Slug ownership show that the exact per-extension
mapping and canonical/internal association are prerequisites to route work.
Instantiation builds the correct host/base then all-generated then
override/inject keep-last mapping for Label coercion but discards it; the
accepted hidden iterator exposes only canonical name and original `RepoSpec`.
Do not widen `RootRepositoryRoute`, which still means an already
source-preparable direct-local or built-in repository.

Run next only the docs packet
`WP-4-5-host-generated-repository-mapping-retention-design` under
45/260/220/220/745 documentation caps. Freeze one shared retained mapping per
extension request and a no-copy hidden row view in the existing instantiation,
validation, and lib seams under future mandatory 280/520/800 Rust caps. Stage
4 production remains unchanged; route, Bzlmod/server, execution/context,
materialization/I/O, lockfile, command/API, and JVM work remain forbidden.

### Generated-repository mapping retention implementation activated (2026-08-12)

Independent review accepts design `9e12fe58`. Run only
`WP-4-5-host-generated-repository-mapping-retention-implementation` in the
existing instantiation and validation modules plus `lib.rs` solely for hidden
exports and four ledgers, under mandatory 280/520/800 against `9e12fe58`.
Preserve one shared mapping-entry allocation per request, row-specific context,
the no-copy hidden iterator, original generated `RepoSpec`, all proof, and all
no-route/Bzlmod/server/execution/I/O/materialization/public/JVM stops.

### Generated mapping accepted; core definition-lookup design scheduled (2026-08-12)

Independent review accepts loading mapping publication `b9a4a3fc`. The hidden
certificate now provides canonical/internal identity, original `RepoSpec`, and
exact shared mapping/context without copying. The next owner is core, which
already depends on loading and Bzlmod and owns workspace DICE; server remains
the daemon/wire adapter.

Run only the four-plan docs packet
`WP-4-5-6-host-generated-repository-definition-lookup-owner-design` under
45/260/220/220/745 documentation caps. Freeze a private core canonical lookup
over the sole validation key with a future exact two-file 260/480/740 ceiling,
complete-scan uniqueness, borrowed no-copy result, proof, and stops. Stage 4
production remains unchanged; apparent routing, repository execution/context,
source/materialization/I/O, lockfile, command/wire API, and JVM remain
forbidden.

### Core generated-definition lookup implementation activated (2026-08-12)

Independent review accepts design `6678f54f`. Implement only
`WP-4-5-6-host-generated-repository-definition-lookup-owner-implementation`
in new core runtime `generated_repository_definition.rs` plus `runtime/mod.rs`
solely for its private declaration, and four ledgers, under mandatory
260/480/740 against `6678f54f`. Preserve complete-scan canonical uniqueness,
the retained certificate-plus-ordinal/no-copy result, proof, compatibility,
and stops. Stage 4 production remains unchanged; no loading/Bzlmod/server,
route, execution, source/materializer/I/O, lockfile, public API, or JVM work is
authorized.

### Generated-definition lookup proof cap correction scheduled (2026-08-12)

The retained core lookup is 222 production, 541 tests, and 763 total formatted
Rust lines against `6678f54f`; required full-scan and field/order A/B/A proof
exceeds only the old test/total caps. Run only the four-ledger docs packet
`WP-4-5-6-host-generated-repository-definition-lookup-owner-r2-cap-design`.
Authorize no Rust. Preserve the exact two files, behavior, proof, compatibility,
and stops; correct only future caps to 260/550/800. Stage 4 production remains
unchanged until independent acceptance and explicit r2 activation.

### Generated-definition lookup implementation r2 activated (2026-08-12)

Independent review accepts cap correction `99a5b898`. Run only
`WP-4-5-6-host-generated-repository-definition-lookup-owner-implementation-r2`
in new core runtime `generated_repository_definition.rs` plus `runtime/mod.rs`
solely for its private declaration and four ledgers, under mandatory
260/550/800 against `6678f54f`. Preserve complete-scan uniqueness, the
certificate-plus-ordinal no-copy result, every field/order/lifecycle proof,
compatibility boundary, and stop. Stage 4 production remains unchanged; no
third Rust file, loading/Bzlmod/server, route, execution, source/materializer
I/O, public API, or JVM work is authorized.

### Canonical lookup accepted; apparent mapping design scheduled (2026-08-12)

Independent review accepts core canonical generated-definition lookup
`daefe6fc`. Pinned Bazel 9.2 next resolves a nonroot apparent name through the
selected generated repository's complete shared mapping before any repository
rule load, route, source, or materializer work.

Run only four-ledger docs packet
`WP-4-5-6-host-generated-repository-apparent-mapping-owner-design` under
40/240/200/180/660 documentation caps. Freeze a private core key that computes
only the accepted definition key, validates canonical/mapping context, and
borrows the post-substitution target without copying. Future Rust is limited to
the existing definition module under mandatory 220/450/670. Stage 4 production
remains unchanged; root mapping, public route, repository execution/context,
source/materializer/I/O, lockfile, command/API, and JVM work remain forbidden.

### Generated-repository apparent mapping implementation activated (2026-08-12)

Independent review accepts design `0af55eff`. Run only
`WP-4-5-6-host-generated-repository-apparent-mapping-owner-implementation` in
the existing core generated-definition module with colocated tests and four
ledgers, under mandatory 220/450/670 against `0af55eff`. Preserve the sole
definition-key dependency, nonroot/context validation, borrowed effective
target, full proof, compatibility, and stops. Stage 4 production remains
unchanged; no second Rust file, root mapping, route, execution,
source/materializer I/O, public API, or JVM work is authorized.

### Generated mapping accepted; selected-module lookup design scheduled (2026-08-12)

Independent review accepts private core apparent mapping `f468fa30`. Before
core can classify a canonical target, Bzlmod must provide the peer canonical
lookup over its existing selected-route catalog; root apparent
`RootRepositoryRouteKey` remains a later source-preparation owner.

Run only four-ledger docs packet
`WP-5-host-canonical-selected-module-definition-owner-design` under
40/240/180/220/660 documentation caps. Freeze one private Bzlmod route-catalog
key with complete-scan uniqueness, predecessor+ordinal/no-copy access, exact
root/registry/nonregistry identity, typed builtin fail-closed handling, warmed
predecessor zero-additional-source proof, future one-file 220/500/720 caps,
proof, and stops. Stage 4 production remains unchanged; no core/loading Rust, route,
repository execution/context, source/materializer I/O, public API, or JVM work
is authorized.

### Canonical selected-module lookup implementation activated (2026-08-12)

Independent review accepts design `dd8ca159`. Run only
`WP-5-host-canonical-selected-module-definition-owner-implementation` in
existing Bzlmod `selected_repo_spec.rs` with colocated tests and four ledgers,
under mandatory 220/500/720 against `dd8ca159`. Preserve exhaustive uniqueness,
predecessor+ordinal/no-copy access, typed builtin deferral, proof,
compatibility, and stops. Stage 4 production remains unchanged; no second Rust
file, public export, loading/core/server edit, route/source/materializer/I/O,
execution, or JVM work is authorized.

### Selected-module lookup accepted; hidden publication design scheduled (2026-08-12)

Independent review accepts private Bzlmod lookup `bd3ab8ee`. Before core can
compose selected and generated canonical domains, freeze a borrowed hidden ABI
over that sole key/store rather than copying selected routes.

Run only four-ledger docs packet
`WP-5-host-canonical-selected-module-definition-publication-design` under
35/220/180/180/615 documentation caps. Freeze opaque errors, one certificate
plus borrowed kind/module/version/canonical/mapping/original-RepoSpec view, a
two-file hidden successor under 180/380/560, proof, and stops. Stage 4
production remains unchanged; no loading/core/server Rust, route,
source/materializer/I/O, public stable API, or JVM work is authorized.

### Selected-module hidden publication implementation activated (2026-08-12)

Independent review accepts design `1d8758d5`. Run only the exact hidden ABI
in existing `selected_repo_spec.rs` and `lib.rs` hidden re-exports plus four
ledgers, under mandatory 180/380/560 against `1d8758d5`. Preserve sole-key
no-copy semantics, opaque errors, proof, compatibility, and stops. Stage 4
production remains unchanged; no third Rust file, loading/core/server edit,
route/source/materializer/I/O, stable public API, or JVM work is authorized.

### Selected publication accepted; absence-signal design scheduled (2026-08-12)

Independent review accepts hidden selected-module publication `bc822520` at
131 production/83 tests/214 total against design `1d8758d5`. Core composition
cannot distinguish selected-domain absence from the opaque route, compute,
duplicate, and builtin terminals without one smaller typed signal.

Run only four-ledger docs packet
`WP-5-host-canonical-selected-module-definition-absence-signal-design` under
mandatory 35/180/140/140/495 documentation caps. Freeze only a hidden Copy/Eq
`Missing | Terminal` disposition and opaque-error accessor over the existing
Bzlmod key/error; future Rust is exactly `selected_repo_spec.rs` plus `lib.rs`
hidden re-export under mandatory 50/120/170. Stage 4 production remains
unchanged. No payload leak, new key/store, core/loading/server edit, route,
source/materializer/I/O, stable API, or JVM work is authorized.

### Selected-module absence-signal implementation activated (2026-08-12)

Independent review accepts design `c466d864`. Run only
`WP-5-host-canonical-selected-module-definition-absence-signal-implementation`
in existing Bzlmod `selected_repo_spec.rs` plus `lib.rs` solely for the hidden
enum re-export and four ledgers, under mandatory 50/120/170 against
`c466d864`. Preserve the exact Missing-versus-Terminal accessor, opacity,
proof, and stops. Stage 4 production remains unchanged; no new key/store,
third Rust file, core/loading/server edit, composition, route,
source/materializer/I/O, stable API, or JVM work is authorized.

### Selected absence accepted; core definition composition design scheduled (2026-08-12)

Independent review accepts selected absence signal `35ff14f7`. Run only
four-ledger docs packet
`WP-4-5-6-host-canonical-repository-definition-composition-owner-design` under
mandatory 40/260/220/200/720 documentation caps. Freeze one private core key
that computes selected first, falls through only on typed Missing, then computes
generated, retaining original certificates without copies. Future Rust is only
existing core `generated_repository_definition.rs` under 260/520/780. Stage 4
production remains unchanged; no Bzlmod/loading/server Rust, second file,
route/source/materializer/I/O, public API, or JVM work is authorized.

### Core canonical definition composition implementation activated (2026-08-12)

Independent review accepts design `e05a0dfc`. Run only
`WP-4-5-6-host-canonical-repository-definition-composition-owner-implementation`
in existing core `generated_repository_definition.rs` with colocated tests and
four ledgers under 260/520/780 against `e05a0dfc`. Preserve selected-first,
Missing-only generated fallback, original certificates, proof, and stops.
Stage 4 production remains unchanged; no second Rust file, Bzlmod/loading/server
edit, route/source/materializer/I/O, public API, or JVM work is authorized.

### Canonical definition composition proof REPLANs at the hidden registry ABI (2026-08-12)

Independent review finds the retained core production diff sound, but its
design demanded a real core SelectedRegistry fixture even though the inputs
needed to construct that selected certificate remain crate-private to Bzlmod.
Do not widen the ABI or add a Bzlmod test hook for duplicate proof.

Run only four-ledger docs packet
`WP-4-5-6-host-canonical-repository-definition-composition-proof-correction-design`
under 30/180/120/120/450 caps. Inherit SelectedRegistry proof from the accepted
external-style Bzlmod key/view suite; require core real Root,
SelectedNonregistry, Generated, terminal/Need/fallback/lifecycle proof plus a
pure exhaustive same-canonical branch matrix. Retain the one-file r2 successor
and 260/520/780 caps. Authorize no Rust until acceptance and r2 activation.

### Corrected canonical definition composition implementation activated (2026-08-12)

Independent review accepts proof correction `63fedad6`. Run only
`WP-4-5-6-host-canonical-repository-definition-composition-owner-implementation-r2`
in existing core `generated_repository_definition.rs` with colocated tests and
four ledgers under 260/520/780 against `e05a0dfc`. Preserve the corrected
inherited-registry/core-real proof split, selected-first Missing-only behavior,
no-copy ownership, compatibility, and stops. Stage 4 production remains
unchanged; no second Rust file or adjacent breadth is authorized.

### Canonical apparent-mapping replacement design scheduled (2026-08-12)

Independent review accepts canonical composition `7ab6c615`. Run only
four-ledger docs packet
`WP-4-5-6-host-canonical-repository-apparent-mapping-composition-owner-design`
under 40/260/220/220/740 caps. Freeze one private core replacement over the
canonical definition key, deleting the callerless generated-only mapping key;
future Rust is only existing `generated_repository_definition.rs` under
240/520/760 with no material size/responsibility growth. Keep root mapping
fail-closed and route/source/materialization/public breadth deferred. Stage 4
production remains unchanged; no second key/file or adjacent Rust is authorized.

### Canonical apparent-mapping replacement implementation activated (2026-08-12)

Independent review accepts design `706da25d`. Run only
`WP-4-5-6-host-canonical-repository-apparent-mapping-composition-owner-implementation`
in existing core `generated_repository_definition.rs` with colocated tests and
four ledgers under 240/520/760 against `706da25d`. Delete the generated-only
mapping owner; preserve the sole canonical predecessor, borrowed lookup, proof,
compatibility, complexity gate, and stops. Stage 4 production remains unchanged.

### Canonical apparent mapping accepted; root-mapping publication scheduled (2026-08-13)

Independent review accepts canonical apparent mapping `fd8a7582`. Core cannot
yet resolve root-visible generated aliases: the complete post-extension root
mapping is retained only by Bzlmod's private selected-extension mapping owner,
and replaying it in core would duplicate its exact producer.

Run only four-ledger docs packet
`WP-5-host-root-repository-mapping-publication-design` under
40/240/180/200/660 caps. Freeze a hidden, borrowed Bzlmod publication over the
existing owner, including empty-extension roots, selected deps, generated
imports, ordered override/inject substitutions, Need/error/equality and no-copy
proof. Future Rust is only `selected_repo_spec.rs` plus hidden `lib.rs` exports
under 180/420/600. Authorize exactly one hidden projection key over the sole
existing mapping producer, and no additional key/store. Stage 4 production
remains unchanged; core consumption,
routes, source/materialization, commands, I/O, and adjacent Rust are deferred.

### Root repository-mapping publication implementation activated (2026-08-13)

Independent review accepts design `d624dc5b`. Implement only the hidden
`HostRootRepositoryMappingKey` projection in Bzlmod `selected_repo_spec.rs` and
`lib.rs` re-exports under 180/420/600 against that design. Preserve the sole
mapping producer, predecessor-plus-Root-ordinal certificate, borrowed exact
iterator, opaque errors, proof, compatibility, and stops. Stage 4 production
remains unchanged; no core/loading/server, route/source/materialization, I/O,
third Rust file, or additional key/store is authorized.

### Root mapping publication hits its production cap (2026-08-13)

The retained two-file implementation needs 211 production lines for the frozen
ABI and typed projection before tests, exceeding 180. Run only docs packet
`WP-5-host-root-repository-mapping-publication-r2-cap-design`; authorize no
Rust. Preserve all semantics/proofs/stops and change only the future caps to
240/420/660 against `d624dc5b`, pending acceptance and explicit r2 activation.

### Root mapping publication r2 implementation activated (2026-08-13)

Independent review accepts cap correction `054f70f7`. Resume only the two-file
root-mapping publication implementation under 240/420/660 against `d624dc5b`.
Complete the frozen proof without production or scope widening. Stage 4 remains
unchanged; no third file, extra key/store, consumer, route/source/materializer,
I/O, loading/core/server/Cargo, stable API, or JVM work is authorized.

### Root mapping accepted; root apparent-composition design scheduled (2026-08-13)

Independent review accepts root mapping publication `927c00af` at
201 production/360 tests/561 total lines with full Bzlmod/loading/server proof
and only the unchanged core external-visibility boundary. Run only four-ledger
docs packet `WP-4-5-6-host-root-apparent-mapping-composition-owner-design`
under 40/260/220/220/740 documentation caps.

Freeze an in-place Root branch on core's existing private canonical apparent
mapping key: Root computes only the accepted hidden root mapping; nonroot keeps
the existing canonical-definition predecessor; both borrow their exact retained
targets. Future Rust is only existing `generated_repository_definition.rs`
under 180/420/600 and a 2,600-line cohesion ceiling. Stage 4 production remains
unchanged. No new key/file, copied map, route/RepoSpec/source/materializer,
public API, I/O, command/server, or JVM work is authorized.

### Root apparent-mapping composition implementation activated (2026-08-13)

Independent review accepts design `57ef6bf1`. Implement only the in-place Root
branch of core's existing private canonical apparent-mapping key in
`generated_repository_definition.rs`, with colocated tests, under 180/420/600
and a 2,600-line final cohesion ceiling. Preserve exclusive Root versus
nonroot predecessor dispatch, borrowed targets, typed order/errors, proof, and
all stops. Stage 4 production remains unchanged; no new key/file, Bzlmod/
loading/server/Cargo, route/RepoSpec/source/materializer/I/O/public/JVM breadth
is authorized.

### Root apparent mapping accepted; definition-owner design scheduled (2026-08-13)

Independent review accepts core root apparent-mapping composition `59493b95`
within 63/271/334 and the 2,600-line ceiling. Run only four-ledger docs packet
`WP-4-5-6-host-root-apparent-repository-definition-owner-design` under
40/280/220/220/760 documentation caps.

Freeze one new private composition key in cohesive
`root_apparent_repository_definition.rs`, a minimal `pub(super)` seam in
`generated_repository_definition.rs`, and only its private `runtime/mod.rs`
declaration. It computes root apparent mapping first, short-circuits
Root/builtin as typed deferred terminals, then computes the identical canonical
definition and retains both predecessors without copied target/map/RepoSpec.
Future Rust is exactly those three paths under 340/700/1,040, with 2,400/900
physical ceilings for the old/new modules. Stage 4 production remains unchanged;
no route/source/materializer/command/public/I/O/JVM breadth is authorized.

### Root apparent-definition composition implementation activated (2026-08-13)

Independent review accepts design `512e40ed`. Implement only new private
`root_apparent_repository_definition.rs`, the minimal `pub(super)` predecessor
seam in `generated_repository_definition.rs`, and the private `runtime/mod.rs`
declaration under 340/700/1,040 and 2,400/900 old/new module ceilings. Preserve
mapping-first order, Root/builtin short-circuiting, borrowed predecessor identity,
proof, and every route/source/materializer/command/public/I/O/JVM stop.

### Root apparent-definition proof boundary REPLAN (2026-08-13)

Implementation review retains the unaccepted three-file diff but authorizes no
Rust. Run only docs packet
`WP-4-5-6-host-root-apparent-repository-definition-owner-r2-proof-design`.
After real Root mapping success, the identical selected/generated definition is
already owned by that complete closure; prove defensive second-position Need,
Missing, terminal, and context mismatch purely plus inherited predecessor
evidence, not fabricated graph state. Real consumer proof remains mapping
failures/short circuits, admitted successes, A/B/A, reuse/no-copy, and zero
additional registry/filesystem/materialization work after warming. Preserve
340/700/1,040, 2,400/900, scope, semantics, and all stops. Require acceptance
and explicit r2 activation before Rust resumes.

### Root apparent-definition implementation r2 activated (2026-08-13)

Independent review accepts proof correction `dfe5cad0` over design `512e40ed`.
Resume only the retained three-path core implementation under 340/700/1,040
and 2,400/900. Use the corrected pure/inherited/real proof split; preserve the
opaque sibling seam, no-map view, mapping-first/short-circuit semantics, and all
route/source/materializer/command/public/I/O/JVM stops.

### Root apparent definition accepted; route-carrier ownership audit scheduled (2026-08-13)

Independent review accepts composition `7c0c0e48` at 327 production, 610 tests,
937 total, and 2,373/897 physical lines. Run only four-ledger docs packet
`WP-4-5-6-host-root-apparent-repository-route-carrier-owner-design` under
40/260/220/200/720 documentation caps. Freeze a private core carrier over only
the accepted composition key, promoting opaque Main/Builtin deferred outcomes
and retaining admitted selected/generated definitions without copied state.
Future Rust is exactly a new cohesive route module, minimal composition seam,
and private mod declaration under 320/650/970 with 960/800 old/new ceilings.
Stage 4 production remains unchanged; no reverse edge, duplicate lookup/store,
owned Bzlmod route, source/package/materialization/I/O, command/public API, or
JVM work is authorized.

### Root apparent repository carrier implementation activated (2026-08-13)

Independent review accepts design `0c0c2402`. Implement only new private
`root_apparent_repository_route.rs`, the frozen minimal `pub(super)` seam in
`root_apparent_repository_definition.rs`, and the private `runtime/mod.rs`
declaration under 320/650/970 with 960/800 definition/route ceilings. Preserve
the sole-predecessor five-domain carrier, opaque Main/Builtin projection,
fail-closed consistency, retained Arc/request identity, proof, and all no-route/
source/materialization/command/public/I/O/JVM stops. Stage 4 production remains
unchanged.

### Private repository carrier accepted; consumer-boundary audit scheduled (2026-08-13)

Independent review accepts the private five-domain carrier at 318 production,
493 tests, 811 total, and 954/778 physical lines. Run only four-ledger docs
packet `WP-4-5-6-host-repository-route-consumer-boundary-design` under
40/240/200/200/680 documentation caps. Audit core/Bzlmod/loading ownership and
freeze one dependency-safe successor or a precise prerequisite before any
route/source/package/materialization or command work. Stage 4 production stays
unchanged; forbid reverse edges, duplicate lookups/stores, copied RepoSpec or
mapping state, public API, execution/I/O, lockfile, wire, and JVM breadth.

### Consumer audit selects Bzlmod source-capability prerequisite (2026-08-13)

Independent review accepts audit `5cd3c4ab`. Run only four-ledger docs packet
`WP-4-5-host-repository-source-capability-input-design` under
40/220/180/180/620 caps. Freeze a hidden computation-free Bzlmod value carrying
workspace/apparent/canonical plus builtin identity or one shared `Arc<RepoSpec>`,
with exact legacy-route projection and no module name. Future Rust is only
`host_module.rs` and `lib.rs` under 180/320/500. Stage 4 production remains
unchanged; no key/store, core/loading/server edit, source/package/materializer,
command/public API, reverse edge, I/O, or JVM is authorized.

### Repository source-capability implementation activated (2026-08-13)

Independent review accepts design `538b5231`. Implement only existing Bzlmod
`host_module.rs`, `lib.rs` hidden exports, and four ledgers under 180/320/500.
Preserve the computation-free owned identity, strict builtin/spec polarity,
manual structural hash, exact legacy projection, Arc-only clones, proof, and
all no-key/core-loading/source/materialization/command/public/I/O/JVM stops.
Stage 4 production remains unchanged.

### Owned source capability accepted; core publication designed next (2026-08-13)

Independent review accepts Bzlmod capability `3faa90dd` at 111 production, 148 tests,
and 259 total formatted net Rust lines. Full Bzlmod/loading/server suites pass;
core retains only its unchanged deferred external-visibility assertion. Run
only four-ledger docs packet
`WP-4-5-6-host-repository-source-capability-publication-design` under
40/240/180/180/640 documentation caps. Freeze one computation-free publication
inside the existing private core five-domain carrier: Main has no capability;
Builtin and selected/generated definitions retain exact owned Bzlmod capability
identity with one shared RepoSpec allocation. Future Rust is exactly existing
`root_apparent_repository_route.rs` under 180/360/540 against `3faa90dd` and a
960-line ceiling. The projection returns one owned capability disposition and
does not store duplicate derived state or change carrier equality.
Stage 4 production remains unchanged; no new key/store, Bzlmod/loading/server/
command edit, source/package/materialization/I/O, public API, reverse edge, or
JVM breadth is authorized.

### Core repository source-capability publication implementation activated (2026-08-13)

Independent review accepts design `7b6484ee` over capability `3faa90dd`.
Implement only existing private `root_apparent_repository_route.rs` with
colocated tests under 180/360/540 and the 960-line ceiling. Preserve the exact
fail-closed `Option<Disposition>` projection, unchanged carrier state/equality,
per-call allocation and Arc-only clone proof, and all no-key/second-file/source/
materialization/consumer/command/public/I/O/JVM stops. Stage 4 production
remains unchanged.

### Core source-capability publication accepted; consumer order audited next (2026-08-13)

Independent review accepts `0cba8fb8` at 42/111/153 and 931/960 physical
lines. Run only four-ledger docs packet
`WP-4-5-host-repository-source-capability-consumer-order-design` under
40/240/180/180/640 documentation caps. Audit the core projection, Bzlmod
materialization-request classifier and package-source owner, and loading
package-load owner; freeze exactly one computation-free successor or precise
prerequisite before any consumer migration. Stage 4 production remains
unchanged. Forbid reverse edges, copied RepoSpec/state, new stores, legacy
module-name rederivation, source/package/materialization/I/O, command/public
API, and JVM breadth.

### Consumer audit selects local-path provenance prerequisite (2026-08-13)

Independent review accepts audit `1cd140a6`: pure Bzlmod request projection
precedes loading migration, but the capability lacks the exact root-relative
versus command-absolute provenance consumed by `request_kind`. Run only
four-ledger docs packet `WP-4-5-host-repository-local-path-policy-owner-design`
under 40/260/200/200/700 documentation caps. Trace effective override policy
through Bzlmod nonregistry/selected ownership, generated RepoSpecs, the owned
capability, and core projection; freeze one compact fail-closed policy and one
dependency-safe producer chain. Stage 4 production remains unchanged. Forbid
path-shape inference, request/source/package/materialization work, new keys/
stores, copied state, reverse edges, command/public API, I/O, and JVM breadth.

### Local-path policy owner accepted; selected retention designed next (2026-08-13)

Independent review accepts audit `980654c8`. Run only four-ledger docs packet
`WP-5-host-selected-nonregistry-local-path-policy-retention-design` under
40/240/180/180/640 documentation caps. Freeze a compact hidden Bzlmod
WorkspaceRelative/CommandAbsolute/LocalUnsupported enum, retain it once from
the existing effective-override producer through the nonregistry closure, and
publish it through the hidden selected-definition view. Future Rust is exactly
`source_preparation.rs`, `selected_repo_spec.rs`, and `lib.rs` hidden export
under 220/450/670. Stage 4 production remains unchanged; no new edge/key/store,
core/loading/capability/request/source/materialization edit, inference, public
API, I/O, or JVM breadth is authorized.

### Selected local-path policy retention implementation activated (2026-08-13)

Independent review accepts design `fda0032e`. Implement only existing Bzlmod
`source_preparation.rs`, `selected_repo_spec.rs`, and `lib.rs` hidden export
under 220/450/670. Preserve the exact WorkspaceRelative/CommandAbsolute/
LocalUnsupported provenance, existing dependency order, required closure
field, hidden selected-definition accessor, proof, and all no-core/loading/
capability/request/source/materialization/command/public/I/O/JVM stops. Stage 4
production remains unchanged.

### Selected local-path policy retained; capability threading designed next (2026-08-13)

Independent review accepts `63de458b` at 43 production, 64 tests, and 107
total formatted net Rust lines. Full Bzlmod/loading/server suites pass; core
retains only its unchanged deferred external-visibility assertion. Run only
four-ledger docs packet
`WP-4-5-6-host-repository-source-capability-policy-threading-design` under
40/280/220/220/760 documentation caps. Freeze one four-file, borrowed-view
chain from the accepted selected policy into capability identity: DirectLocal
and root-origin SelectedNonregistry use WorkspaceRelative, command-origin selected uses CommandAbsolute,
Registry/Generated use LocalUnsupported, and Main/Builtin carry none. Future
Rust is exactly `host_module.rs`, `generated_repository_definition.rs`,
`root_apparent_repository_definition.rs`, and
`root_apparent_repository_route.rs` under 320/700/1,020 and physical ceilings
3,000/2,450/1,040/1,100. Stage 4 production remains unchanged; no request,
source/package/materialization, loading/server/command, new key/store, path/rule
inference, public API, I/O, or JVM work is authorized.

### Repository source-capability policy threading implementation activated (2026-08-13)

Independent review accepts design `c5853ecf`. Implement only existing
`host_module.rs`, `generated_repository_definition.rs`,
`root_apparent_repository_definition.rs`, and
`root_apparent_repository_route.rs` plus four ledgers under 320/700/1,020 and
physical ceilings 3,000/2,450/1,040/1,100. Preserve exact constructor and
borrowed policy accessors, structural capability identity/hash, strict
Main/Builtin/Registry/Nonregistry/Generated polarity, proof, and all no-fifth-
file/key/store/request/source/materialization/loading/command/public/I/O/JVM
stops. Stage 4 production remains unchanged.

### Repository source-capability policy proof boundary corrected (2026-08-13)

Retain the unaccepted four-file implementation over `c5853ecf`, but authorize
no Rust. Run only four-ledger docs packet
`WP-4-5-6-host-repository-source-capability-policy-threading-r2-proof-design`.
Real Bzlmod proof owns SelectedRegistry policy; real core proof owns
Generated, root/command Nonregistry, Main, and Builtin; production-used pure
tables exhaust Registry and corrupt vertical polarity. Core cannot inject the
private Bzlmod mirror-policy input solely for proof. Preserve all paths, caps,
ceilings, semantics, and stops; require acceptance plus explicit r2 activation.

### Repository source-capability policy threading r2 activated (2026-08-13)

Independent review accepts correction `5e88f2ad` over design `c5853ecf`.
Resume only the retained exact four-file implementation under 320/700/1,020
and ceilings 3,000/2,450/1,040/1,100. Preserve the corrected Bzlmod-real,
core-real, and production-used-pure proof split, root-definition fail-closed
policy boundary, semantics, ABI, and all prior stops. Stage 4 stays unchanged.

### Repository source policy threaded; request projection designed (2026-08-13)

Independent review accepts `65b1dd89` at 170 production, 292 tests, and 462
total changed Rust lines; Bzlmod/loading/server pass and core retains only its
accepted unrelated deferred diagnostic assertion. Run only four-ledger docs
packet
`WP-4-5-host-repository-source-capability-materialization-request-projection-design`
under 40/260/220/220/740 documentation caps. Freeze one computation-free
Bzlmod projection from the accepted capability into Builtin identity or the
existing owned materialization request. The one RepoSpec clone copies only its
compact shell and shares the Arc attribute graph. Future Rust is exactly
`source_preparation.rs` and hidden `lib.rs` exports under 180/420/600; Stage 4
behavior stays unchanged. No DICE/Need, consumer migration, package/source,
materialization, command, public wire, I/O, or JVM work is authorized.

### Repository materialization-request projection implementation activated (2026-08-13)

Independent review accepts design `9df81032`. Implement only existing Bzlmod
`source_preparation.rs`, hidden `lib.rs` exports, and four ledgers under
180/420/600 and physical ceilings 11,080/380. Preserve exact Builtin-or-request
ABI, policy/classifier/error order, renamed legacy wrapper and parity, compact
RepoSpec shell clone, proof, and all no-third-file/key/store/Need/consumer/
source/materialization/command/public/I/O/JVM stops. Stage 4 stays unchanged.

### Repository materialization-request projection proof correction scheduled (2026-08-13)

Retain the unaccepted two-file implementation over design `9df81032`, but
authorize no Rust. Run only four-ledger docs packet
`WP-4-5-host-repository-source-capability-materialization-request-projection-r2-proof-design`
under 40/260/220/220/740 documentation caps. Production ABI and ordering pass;
the required absolute-local, malformed-attribute, path-shape, exact-identity,
apparent-exclusion, and A/B/A proof does not fit the prior 11,080 source ceiling.
Freeze future ceilings 11,240/380 while retaining 180/420/600 and every prior
scope and no-consumer/materialization stop. Require acceptance and explicit r2
activation before Rust resumes. Stage 4 behavior remains unchanged.

### Materialization request projected; first source consumer audited next (2026-08-13)

Independent review accepts `06a5aa99` at 40 production, 189 tests, and 229
total formatted net Rust lines; Bzlmod/focused proof pass and direct dependents
retain their accepted baseline. Run only four-ledger docs packet
`WP-4-5-host-repository-source-consumer-boundary-design` under
40/280/220/220/760. Audit existing Bzlmod path/source owners, loading package
load, and the core call site read-only; choose exactly an atomic capability/
request input for the existing path owner or a smaller source-certificate
prerequisite. Authorize no Rust, loading/core/command change, key/store,
materialization/source/package/I/O, copied state, or JVM breadth. Stage 4 stays
unchanged.

### Repository materialization-request projection r2 activated (2026-08-13)

Independent review accepts proof correction `6a8353d7` over design `9df81032`.
Resume only retained Bzlmod `source_preparation.rs`, hidden `lib.rs` exports,
and four ledgers under 180/420/600 and ceilings 11,240/380. Preserve the exact
production ABI/order and corrected proof matrix with every prior no-consumer/
materialization breadth stop. Stage 4 behavior remains unchanged.

### Source consumer audit selects owned input certificate (2026-08-13)

Audit rejects immediate `HostRepositoryPathKey` migration: Builtin is
catalog-backed rather than `ResolvedPath`, and the legacy key publishes a
module-name demand scope absent from the accepted capability. Run only
four-ledger docs packet `WP-4-5-host-repository-source-input-certificate-design`
under 40/260/220/220/740. Freeze a computation-free hidden Bzlmod certificate
retaining exact capability plus Builtin identity or request Arc; future Rust is
only `source_preparation.rs` and hidden `lib.rs` under 140/320/460 and
11,400/390. Authorize no Rust/key/path/result/source/materialization/consumer/
core/loading/command/public/I/O/JVM work. Stage 4 stays unchanged.

### Repository source-input certificate implementation activated (2026-08-13)

Independent review accepts design `b9cffe52`. Implement only existing Bzlmod
`source_preparation.rs`, hidden `lib.rs` exports, and four ledgers under
140/320/460 and ceilings 11,400/390. Preserve the sole projection, exact owned
capability/disposition, borrowed view, proof, and all no-key/path/result/source/
consumer/materialization stops. Stage 4 stays unchanged.

### Repository source input accepted; core five-domain owner designed (2026-08-13)

Independent review accepts `f7566280` at 70 production, 145 tests, and 215
total formatted Rust lines; focused proof, all Bzlmod tests, and all loading
tests pass. Run only four-ledger docs packet
`WP-4-5-6-host-root-apparent-repository-source-input-owner-design` under
40/300/220/220/780 documentation caps. Freeze one private core DICE owner that
computes only the accepted root-apparent route carrier, forwards Need, retains
the exact completed carrier, and projects Main or the accepted Bzlmod source
input exactly once. Future Rust is only the new cohesive core owner, a minimal
`pub(super)` carrier seam, and `runtime/mod.rs` under 320/650/970 with
1,100/800 physical ceilings. No path/result/source/package/loading/command,
public API, I/O, reverse dependency, second lookup/store, or JVM work is
authorized. Stage 4 behavior remains unchanged.

### Root-apparent repository source-input owner implementation activated (2026-08-13)

Independent review accepts design `e659b0e3`. Implement only the new private
core source-input module, minimal `pub(super)` route-carrier seam, private mod
line, and completion ledgers under 320/650/970 with 1,100/800 ceilings.
Preserve route-only Need ordering, exact retained predecessor, Main-or-Input
success, four typed terminal kinds, one Bzlmod certificate projection, proof,
and every no-path/result/source/loading/command/public/I/O/JVM stop. Stage 4
behavior remains unchanged.

### Root-apparent source input accepted; path/source ownership audited next (2026-08-13)

Independent review accepts `e4292de7`: the private core owner forwards route
Need, retains the exact completed predecessor, validates full source
association, and constructs one accepted Bzlmod input certificate. Run only
four-ledger docs packet
`WP-4-5-6-host-repository-source-path-consumer-owner-audit` under
40/300/240/240/820. Map the exact Builtin/request path, result, source, package,
legacy-demand, and command boundaries and choose one smallest dependency-safe
successor or prerequisite REPLAN. Authorize no Rust, consumer migration,
key/store, materialization/source/package/I/O, public/command/server,
reverse-edge, or JVM work. Stage 4 behavior remains unchanged.

### Source-path audit selects shared relative-path prerequisite (2026-08-13)

The audit rejects downstream path composition because the accepted source input
has already projected, and rejects immediate path-key migration because legacy
demand scope is still module-name-shaped. Run only four-ledger docs packet
`WP-4-5-host-repository-relative-path-owner-design` under 40/240/200/200/680.
Freeze one computation-free hidden Bzlmod value using the sole existing
relative-path checker; future Rust is only `source_preparation.rs` and hidden
`lib.rs` exports under 100/240/340 and 11,540/380. No Rust/key/store, consumer,
request/source-input, path result, source/package/materialization/I/O,
core/loading/command/public/reverse-edge/JVM work is authorized. Stage 4
behavior remains unchanged.

### Repository relative-path owner implementation activated (2026-08-13)

Independent review accepts design `4d96d094`. Implement only Bzlmod
`source_preparation.rs`, hidden `lib.rs` exports, and completion ledgers under
100/240/340 with 11,540/380 ceilings. Preserve the pure path value/error ABI,
sole checker, post-validation Arc allocation, proof, and every no-caller/key/
source/materialization/I/O/core/loading/command/public/JVM stop. Stage 4 remains
unchanged.

### Relative path accepted; path-first core source input designed next (2026-08-13)

Independent review accepts `b46c2c63`; all Bzlmod tests pass. Run only docs
packet `WP-4-5-6-host-root-apparent-repository-source-path-input-owner-design`
under 40/300/240/240/820. Freeze one private core key that validates the pure
relative path before any await, then computes only the accepted source-input
key and retains exact path/predecessor identity. Future Rust is a new cohesive
module, minimal source-input seam, and mod line under 340/700/1,040 with
840/850 ceilings. Defer result/source/loading/command/public/I/O/JVM behavior.

### Root-apparent source-path input owner implementation activated (2026-08-13)

Independent review accepts design `68349398`. Implement exactly the new core
source-path-input module, minimal source-input seam, private mod line, and
ledgers under 340/700/1,040 with 840/850 ceilings. Preserve path-before-await,
exact path/predecessor ownership, ABI/proof, and every no-result/source/loading/
command/public/I/O/JVM stop. Stage 4 remains unchanged.

### Source-path input accepted; source observation ownership audited next (2026-08-13)

Independent review accepts `bd337622` within 340/700/1,040 and 840/850;
focused proof passes and core has only the accepted unrelated deferred-message
baseline. Run only four-ledger docs packet
`WP-4-5-6-host-root-repository-source-observation-consumer-owner-audit` under
40/320/240/240/840. Map the Builtin catalog-byte and request-backed
materialization/path/file branches, legacy module-name demand scope, exact
Need/error/identity boundaries, and choose one smallest dependency-safe
successor or prerequisite REPLAN. Authorize no Rust, consumer/key/store,
materialization/source/package/loading/command/public/I/O/reverse-edge/JVM
work. Stage 4 behavior remains unchanged.

### Source-observation audit selects hidden Bzlmod owner design (2026-08-13)

Independent review accepts audit `b6a90390`. Run only four-ledger docs packet
`WP-4-5-host-repository-source-observation-owner-design` under
40/360/260/260/920. Freeze a hidden Bzlmod key over exact accepted source input
plus validated path: Builtin uses the pinned catalog key once; Request uses the
private materialization-result key once and existing file observation. Preserve
branch-specific values, first Need/errors, identity, and no legacy demand
metadata. Future Rust is only `source_preparation.rs` and hidden `lib.rs` under
mandatory 420/800/1,220 and 12,250/430. No Rust/core/loading/command/public/
new-I/O/reverse-edge/JVM work is authorized. Stage 4 remains unchanged.

### Repository source-observation owner implementation activated (2026-08-13)

Independent review accepts design `7ef0c353`. Implement only Bzlmod
`source_preparation.rs`, hidden `lib.rs` exports, and completion ledgers under
420/800/1,220 and 12,250/430. Preserve exact hidden ABI, structural Hash/Eq,
one Builtin or request-result dependency, branch values/errors/proof, and every
no-caller/core/loading/command/legacy-demand/second-result/new-I/O/public/
reverse-edge/JVM stop. Stage 4 remains unchanged.

### Repository source observation accepted; core dispatch audited next (2026-08-13)

Independent review accepts `cbc44e43`: exact two-file hidden Bzlmod owner,
one Builtin or request-result dependency, complete branch values/errors/Hash,
focused 6/6 and full 365/365 proof, and no caller or legacy demand edge. Run
only four-ledger docs packet
`WP-4-5-6-host-root-repository-source-observation-consumer-audit` under
40/320/240/240/840. Audit the first private core caller from the accepted
source-path predecessor, including Main no-source ordering, exact observation
Need/error retention, and the owned-versus-borrowed certificate clone boundary;
choose one smallest atomic successor or prerequisite REPLAN. Authorize no Rust,
new key/store/caller, loading/package/command/public behavior, legacy demand,
new I/O/materialization, reverse edge, or JVM work. Stage 4 remains unchanged.

### Core source-observation dispatch audit selects compact consumer (2026-08-13)

The read-only audit found no prerequisite REPLAN. The accepted source input
and relative path clone shallow Arc-owned request, RepoSpec, and path
allocations, so the first consumer can be one callerless private core key. Run
docs-only packet
`WP-4-5-6-host-root-repository-source-observation-consumer-design` in the four
ledgers under 40/320/240/240/840. Freeze path-predecessor-first ordering, Main
with no observation, Input with one hidden Bzlmod observation, exact retained
predecessor/observation Arcs, typed Need/errors, complete structural identity,
and zero events. Future Rust is only a new private core observation module,
minimal source-path `pub(super)` seam, and one private `runtime/mod.rs` line
under 340/700/1,040 and physical 870/900/247 ceilings. No Rust is authorized
before independent design acceptance; package/loading/command/public/legacy-
demand/new-I/O/reverse-edge/JVM work remains deferred. Stage 4 behavior is
unchanged.

### Core source-observation consumer implementation activated (2026-08-13)

Independent review accepts design `b6d0ecae`. Implement exactly the new private
core observation module, minimal source-path `pub(super)` seam, and one private
`runtime/mod.rs` declaration under 340/700/1,040 and physical 870/900/247.
Preserve Main zero-observation, Input one-observation, split predecessor versus
observation compute terminals, exact retained Arcs, shallow clone boundary,
structural equality/Need/events proof, and every no-Bzlmod/caller/loading/
package/command/legacy-demand/new-I/O/reverse-edge/JVM stop. Stage 4 behavior
remains unchanged until independent implementation acceptance.

### Selected repository-rule file-effect owner design active (2026-08-25)

The accepted August 12 repository-rule call protocol remains exactly raw,
heap-free invocation capture; it did not authorize later implementation
execution. The generated-package fixture now proves the missing successor is a
selected-call effect owner, not another call-capture or definition leaf.

Run only cross-stage docs packet
`WP-4-5-6-generated-repository-file-effect-owner-design`. The future natural
loading owner must start from the authenticated selected-owner certificate plus
unique call ordinal, reload the defining observed Host `.bzl`, authenticate the
exported rule projection, invoke its frozen implementation once with a
compute-local context, and retain only a shared compact file-effect plan.
Loaded modules, `FrozenValue`s and Starlark heaps remain scratch. Host-Bzl load
events remain below; the invocation owns one complete local batch; Need,
cancellation and failed observed outers publish no partial plan.

Admit only exact Bazel 9.2 `repository_ctx.file` behavior required by the
pinned two-file fixture. All other repository-context APIs stay typed
unsupported. This packet is docs-only with cross-ledger caps in current; Stage
4 Rust, tests, fixtures, loaders, public APIs and JVM work are frozen.

### Selected repository file-effect producer design completed (2026-08-25)

The cross-stage audit selects a uniquely smaller prerequisite before route/root
activation. Implement one loading legacy/observed key pair keyed by workspace,
the accepted selected owner and exact retained repository ordinal. It computes
the existing owner certificate, reloads the call's defining root Host `.bzl`,
authenticates exported rule identity/schema, and invokes that frozen
implementation once with a compute-local context.

The admitted context exposes only Bazel 9.2 string-path
`file(path, content="", executable=True, legacy_utf8=False)` with Bazel's
positional-only `path` and positional-or-named trailing parameters;
`legacy_utf8` is a no-op. It records an ordered shared Bzlmod plan and rejects
nonroot definitions, Label/path arguments, repeated/invalid paths and all other
context APIs as unsupported/deferred.
Retained success/error state is heap-free; Host-Bzl events stay below and the
new key owns one local invocation print batch only after an invocation
terminal.

Activate only
`WP-4-5-selected-repository-file-effect-producer-implementation` in the seven
paths and 560/650/1,210 caps frozen in current. No core, route, source-
preparation, materialization, fixture or public activation is authorized. After
implementation ACCEPT, design only the effect-plan handoff/application packet.

### Selected repository file-effect producer cap STOP (2026-08-25)

The formatted seven-file candidate compiles and passes four focused tests, but
its required loading owner, nominal terminals and exact Starlark ABI use 900
production lines versus the stale 560 cap. Only 168 proof lines exist; the
accepted owner/ordinal, reload-drift, Legacy/Observed, epoch, lifecycle, event
and retained-shape matrix remains mandatory.

Independent review accepts the cap-only correction to 930/850/1,780 and the
exact per-file limits in current. Activate only
`WP-4-5-selected-repository-file-effect-producer-implementation-retry` over the
same seven paths. Named `path` remains an ordinary positional-only Starlark
binding rejection before the context method. No semantic compression, caller,
route, materialization, fixture or public activation is authorized. Pinned
`../zabel` `c7298478…` remains concept-only guidance for the natural selected-
call owner and heap-free effect result.

### Selected repository file-effect proof-contract STOP (2026-08-25)

The retained retry is +901 production/+562 proof, passes seven focused loading
tests and fixes named `legacy_utf8` exactly, but lawful transactions cannot
deliver stale projection fields or contradictory observation epochs to this
key. Run only docs packet
`WP-4-5-selected-repository-file-effect-producer-proof-contract-correction-design`.
Independent review accepts real upper proof for Need/no-batch, sibling
nonexecution, semantic-event ownership, key identity and retained shape, with
impossible projection/outer/merge cases composed from accepted Host-Bzl and
observation-algebra proof plus exact producer source-shape assertions. Activate
only `WP-4-5-selected-repository-file-effect-producer-proof-contract-correction-implementation`
in the same seven-file scope and unchanged caps; add no injection surface.

### Selected repository file-effect producer accepted (2026-08-25)

Independent terminal review accepts +901 production/+747 proof/+1,648
aggregate at 1,402 loading-child physical lines. The exact `ctx.file` ABI,
selected owner/ordinal isolation, reload/authentication, Legacy/Observed
semantics, Need/epoch/event/cancellation lifecycle, sibling nonexecution and
heap-free retained shape are proven. Full loading and Bzlmod suites and the
dependent core check pass.

Activate only docs packet
`WP-4-5-6-generated-repository-file-effect-handoff-application-design`.
Design the first demand-side caller and structural effect handoff; do not edit
loading Rust, execute globally or activate route/materialization/fixture state.
Pinned `../zabel` `c7298478…` remains concept-only ownership guidance.

### Selected-effect demand handoff design (2026-08-25)

The existing core generated route is the sole natural first caller. It retains
the already-authenticated demand owner plus unique ordinal through private core
views, then computes the accepted loading effect key only for a Generated
definition after mapping/definition success. Observations merge mapping ->
definition -> effect; Need and observed outers stay carrierless and loading
keeps its print batch.

Independent review accepts the exact seven files/caps in current. Activate only
`WP-4-5-6-generated-repository-file-effect-handoff-application-implementation`.
No loading edit, second scan, non-Generated execution, parent event replay or
public activation is admitted.

### Generated effect handoff proof/accounting correction (2026-08-25)

The retained handoff candidate keeps the accepted selected owner+ordinal and
loading producer unchanged. Independent terminal review found no loading
production defect, but the original dirty predecessor blobs cannot recover an
exact production/proof addition split and the upper route proof is incomplete.

Freeze all loading Rust. Activate only
`WP-4-5-6-generated-repository-file-effect-handoff-application-proof-accounting-correction-implementation`
with proof-only authority in `source_preparation.rs` and
`generated_package_route.rs`, plus the two bounded `repository_io.rs`
production corrections specified in current. Prove real route Need, semantic,
events, non-Generated nonactivation and order directly; compose unreachable
observed-outer/cancellation rows from the accepted producer proof plus exact
route source shape. Add no injection, loading caller, event replay or new key.
Pinned `../zabel` `c7298478…` remains concept-only guidance for selected demand
and private accepted-effect ownership.

### Generated effect handoff terminal REPLAN; oracle shape correction (2026-08-25)

The corrected handoff candidate is structurally accepted. Full Bzlmod and
loading suites pass, full core has only its byte-identical recorded query
diagnostic baseline, and the rebuilt Slug command exits zero at the exact
`dice_exported_source_file` boundary. The unchanged oracle harness still exits
nonzero because its Bazel-only stderr presentation assertions are applied to
Slug. Independent terminal review rejects a manual waiver.

Freeze every Rust file. Activate only
`WP-4-5-6-generated-source-oracle-tool-specific-message-shape-correction-implementation`.
Keep common exit-zero, preserve Bazel's exact canonical generated-target/source
classification and completion assertions, and require Slug's exact successful
JSON exported-source terminal for the same argv. Pinned `../zabel` remains
architecture guidance only and supplies no harness or output semantics.

### Generated repository file-effect vertical accepted (2026-08-25)

Implementation `3ac0a85b`, from harness design `0c7bb56e`, closes the mandatory
generated-source discriminator without changing Rust. Common exit-zero remains
shared; Bazel retains its canonical target/source/completion assertions; Slug
matches only its exact successful `dice_exported_source_file` JSON terminal.
Both fixture replays return `status: ok`.

The four-file harness correction is +59 production/+98 proof/+157 aggregate at
558/184/2,748/30 physical lines. Focused proof passes four tests; 122 unaffected
oracle tests pass, with only inherited stale fixture-count/host-path rows.
Independent terminal review accepts the fail-closed tool selection and all
frozen Rust hashes.

Activate only docs packet
`WP-6-7A-bootstrap-critical-repository-ruleset-frontier-audit`. Trace the exact
M8 developer graph back to the smallest remaining M7A repository/ruleset
semantic owner. Do not generalize `repository_ctx`, activate public breadth or
claim M7A closure. Zabel remains concept-only architectural guidance.

### Bootstrap frontier selects selected-registry extension Bzl source owner (2026-08-25)

The exact developer graph first fails before repository-rule semantics:
definition requests reject root `@rules_rust//rust:extensions.bzl`, and the
root-only loader cannot consume that selected registry source. The existing
external loader also rejects rules_rust's first same-repository cross-package
and mapped-repository loads.

Loading is the natural byte/evaluation owner, but it must consume an opaque
Bzlmod-selected definition/source association rather than infer a physical
root. Implement one selected-source Bzl owner shared by loaded-definition
projection and pure reacquisition. Preserve request -> source -> recursive-load
epoch order, child-only Bzl events, complete-only reuse and root behavior.

Pinned Bazel 9.2 remains exact behavior authority. Pinned Zabel `c7298478…`
guides only the producer-owned source/consumer-owned loading split. Activate
only `WP-4-5-6-7A-selected-registry-extension-bzl-source-owner-implementation`
in current; repository-rule schemas/APIs, builtin content and public activation
remain deferred.

### Selected-registry source implementation observable REPLAN (2026-08-25)

The no-edit implementation preflight found that real rules_rust evaluates
top-level `repository_rule(doc=...)` before extension export, then reaches
collection-valued repository-rule schemas. Both are explicitly deferred and
currently rejected. The selected-source loading owner remains correct, but the
packet cannot require successful rules_rust export or pure reacquisition.

Run only the docs scope-correction design in current. Select a focused external
selected-registry source/load observable, preserve the exact rules_rust
declaration terminal, and do not widen loading declaration/schema semantics.

### Selected-registry source observable requires one oracle (2026-08-25)

The correction audit found no accepted composite proof. The rules_rust fixture
crosses the deferred declaration/toolchain stack; the nonroot extension
fixture has no root request or recursive selected-source loads. Pinned Bazel
9.2's Bzlmod load regression proves mapped resolution, but not root association,
self cross-package loading and clean named extension export together.

Run only
`WP-1-4-5-6-7A-selected-registry-extension-source-observable-oracle`.
After the overdue fixture-hygiene reset, add one Bazel-only local-registry row:
root requests `@owner//:extension.bzl%probe`; owner source loads
`//shared:local.bzl` and `@mapped_dep//:mapped.bzl`, prints the combined marker
and exports a no-op module extension. Root has neither mapped dependency nor
shared package, so success cannot be produced from its view. No repository
rule, tag/schema, generated repository, Slug run or Rust is admitted.

The later loading design remains six-file and producer-view based, but it may
be corrected only after this evidence accepts. Actual rules_rust still stops
exactly at `repository_rule(doc=...)`. Pinned Zabel `c7298478…` continues to
guide typed producer-view consumption only; Bazel 9.2 owns the oracle bytes.

### Selected-registry source oracle accepted; corrected loading owner active (2026-08-25)

The exact 46-file Bazel-only fixture passes generation and two independent
fresh-root replays. Its root cannot see either recursive child, so the combined
marker and clean export authenticate selected-owner association, same-owner
cross-package loading and the mapped child's producer view without crossing
repository-rule or tag/schema semantics.

The corrected six-file design retains loading as the sole byte/evaluation and
event owner while consuming Bzlmod's typed selected-definition fact. Root
loading remains byte-stable; mapped edges switch producer view; scratch remains
compute-local; complete modules/manifests/epochs remain at existing owners.
Pinned Zabel guides this producer/source split only. Run the r2 implementation
in current; actual rules_rust keeps its existing declaration terminal.

### Selected-registry source-owner r2 authority REPLAN; r3 active (2026-08-25)

The r2 implementation preflight exposed one Bzlmod constructor outside its
six-file authority and restored every partial Rust edit to the accepted frozen
hashes. Loading ownership is unchanged: it remains the sole byte/evaluation
and event owner consuming a typed producer fact, with root loading preserved.

The selected source association must live on each individual request because
the pure-reacquisition owner retains that request. A container/global lookup
would reconstruct producer visibility outside request identity and lifetime.
Activate only the corrected seven-file r3 packet in current. Pinned Zabel
remains concept-only architectural guidance for the producer/source split;
Bazel 9.2 remains behavior authority.

### Selected-registry source owner accepted; loading frontier closed (2026-08-25)

The request-owned selected source now dispatches definition loading and both
pure reacquisitions through one loading owner. Same-repository loads retain the
current selected definition; mapped loads switch to the child definition's
structural route and producer view. Root loading preserves its original Bzl
errors, while selected loading has a distinct typed carrier.

The bounded source-shape proof authenticates the loaded-definition dispatch
only inside its production function, and the pure proof covers both
reacquisition sites. Existing external-Bzl cancellation, event, epoch, error
and A/B/A lifecycles plus the complete loading suite pass. No evaluator heap,
global cache/interner, task or DirectLocal disguise is retained.

This is the pinned Zabel-guided layering: semantic mappings stay with their
producer and loading consumes an immutable already-selected view; no Zig code
or behavior is adopted. Bazel 9.2 remains exact authority. Loading has no next
implementation packet; return only to the docs-only bootstrap-critical
repository/ruleset frontier audit.

### Bootstrap audit REPLAN: root package external Bzl loading is next (2026-08-25)

The resumed live replay corrects the prior declaration-first assumption. The
unchanged rules_rust command first stops on parked M8 wildcard toolchain
registration. With only that line removed in a disposable copy, root BUILD
evaluation rejects `@rules_rust//rust:defs.bzl` in
`resolve_host_load_label`; `repository_rule(doc=...)` is later.

`RootPackageLoadKey` remains the natural package owner, but it currently sends
every direct load through root Host Bzl keys. The accepted repository-package
path already demonstrates the required split: the package driver owns source
order and package evaluation, while a route-owned external Bzl child owns its
source, recursive module closure, events and manifest.

Run only `WP-4-5-7A-root-package-external-bzl-load-owner-design`. Loading may
consume one structural root route and the existing external Bzl owner, but it
must not reconstruct mappings, selected definitions, RepoSpecs or paths.
Preserve root/self behavior, left-first epochs, child event ownership, Need/
outer/semantic stops and complete-only lifecycle identity. Pinned Zabel guides
the immutable already-resolved-module layering only; Bazel 9.2 owns behavior.

### Root package external-Bzl consumer design accepted (2026-08-25)

`RootPackageLoadKey` remains the package/source-order owner. Preserve the
existing root-only resolver for recursive root `.bzl` loads. A new direct BUILD
load resolver keeps root/self children byte-equivalent and sends only
`@apparent//...` through a root-BUILD-admitted `RootRepositoryRouteKey` followed
by the existing external-Bzl eval/observation child. Canonical nonroot direct
loads remain deferred.

The exact order is root anchor, BUILD source, then each route and Bzl child in
declaration order, then package evaluation. Merge observations left-first and
stop on Need, path retry outer, typed route/Bzl terminal or semantic terminal.
The Bzl child retains its source, recursive closure, manifest and event batch;
the package owner stores only the package-attempt events. Scratch evaluation
state remains compute-local.

Selected observation carriers must preserve two distinct lifecycles: genuine
path-frontier failures return through the existing observed outer, while any
nested non-path DICE compute failure becomes a typed route-computation terminal.
No infrastructure failure is fabricated as a path retry. A selected-registry
integration uses existing public registry/materialization seams; no test-only
visibility or fixture is needed.

This implements pinned Zabel's package-source/resolved-direct-load layering as
architectural guidance only. Bazel 9.2 defines source-order and mapping behavior.
Run only `WP-4-5-6-7A-root-package-external-bzl-load-owner-implementation`;
do not reconstruct mappings/definitions in loading or activate broader loading.

### Root external-Bzl candidate retained; command proof REPLAN (2026-08-25)

The eight-file candidate keeps `RootPackageLoadKey` as source-order/package
owner and consumes the structural selected route through the existing external-
Bzl child. Focused identity/order/lifecycle proof and the full serial loading
suite pass; ordinary root recursion and child event ownership remain unchanged.

The mandatory real rules_rust replay does advance beyond the former Host-loader
rejection, but then stops before loading `rust/defs.bzl`: native materialization
rejects the standard BCR archive request. Loading cannot repair transport,
discard `RepoSpec` fields, infer a physical source or disguise the selected
source as local. Freeze the candidate and run only the docs-only native archive
frontier audit. Pinned Zabel still guides producer-owned resolved views only;
Bazel 9.2 owns behavior.

### Root selected external-Bzl loading accepted (2026-08-25)

The corrected frontier audit accepts the frozen route/load candidate for root
BUILD loading from an already materialized selected-registry source. Its direct
transaction proves route-before-child order, selected self/mapped recursion,
package-last evaluation, child-owned events and lifecycle; broad Rust proof is
green. Real rules_rust now reaches the exact materialization request, which is
the honest next command boundary rather than the previously claimed downstream
repository-rule declaration.

Loading is closed for this slice. It must not erase `RepoSpec` fields, inject a
physical root or participate in archive transport/extraction. Pinned Zabel
guides the producer-owned resolved-view handoff only; Bazel 9.2 owns behavior.

### Rules-rust keyword-only definition frontier active (2026-08-25)

Accepted selected-BCR realization now makes both fresh query and build reach
`@rules_rust//rust/platform:triple_mappings.bzl`. The first recursive parse
stops at `def _support(*, std = False, host_tools = False)`: pinned Bazel 9.2
accepts this bare-`*` keyword-only definition, while every live Stage 4
BUILD/`.bzl` parser currently passes starlark-rust `Dialect::Standard`, whose
only relevant disabled field rejects it before evaluation.

Run only docs packet `WP-4-7A-rules-rust-keyword-only-arguments-audit`.
Inventory every production BUILD/`.bzl` parse boundary, authenticate Bazel's
parameter ordering/call behavior and starlark-rust's retained parser, resolver
and evaluator support, then select the smallest centralized Bazel-loading
dialect owner or `REPLAN`. Do not enable unrelated extended syntax, change
MODULE dialects, or edit Rust during the audit.

Pinned Zabel `c7298478…` guides the architectural preference for one complete
typed semantics projection consumed by all relevant evaluators, rather than
scattered local toggles. It is not syntax authority and no representation is
copied; Bazel 9.2 alone defines admitted behavior. M7 stays partial and
M7A -> M8 -> M7B remains.

### Bazel keyword-only dialect audit accepted (2026-08-25)

The exact rules_rust stop is the production external-Bzl parse in
`compute_external_bzl_module`. Eight sibling Stage 4 parse calls cover Host
BUILD/Bzl, root BUILD, external repository BUILD and the retained legacy
package/Bzl route; the core preliminary root-BUILD evaluator also runs before
ordinary command loading. All pass `Dialect::Standard`, while MODULE parsing
is separately owned by Stage 5 and must remain unchanged.

The retained engine needs no parser, resolver, compiler or binder repair.
`Dialect::enable_keyword_only_arguments` gates only the bare-`*` AST form;
`DefParams`, compiled parameters and call binding already cover required and
defaulted named-only values and `*args` interaction. Pinned Bazel 9.2 source
tests authenticate those rows, ordering failures and keyword-only lambda
parameters. A retained `Dialect::Bazel` constant equal to Standard except for
this one field is the smallest centralized owner.

Run only `WP-4-7A-bazel-keyword-only-arguments`. Change the five frozen files,
prove the dialect flags and exact parameter matrix, exercise a real external
recursive route and the preliminary root-BUILD evaluator, then rerun fresh
rules_rust query/build to record the next honest terminal. Positional-only
parameters, types, f-strings, top-level forms, MODULE widening and later
ruleset semantics remain unsupported/deferred.

Pinned Zabel `c7298478…` guides one complete typed semantics/dialect value
consumed by the relevant evaluators, not scattered reconstructed toggles.
Bazel 9.2 remains behavioral authority. M7 stays partial and
M7A -> M8 -> M7B remains.

### Bazel keyword-only syntax accepted; `struct` frontier active (2026-08-25)

Commit `54d28477` implements the audited retained Bazel dialect and routes all
nine Stage 4 BUILD/`.bzl` parse sites plus the preliminary root-BUILD evaluator
through it while leaving MODULE parsing unchanged. Focused parameter, root and
recursive external-Bzl proofs pass, the full loading suite is 239/239, locked
core check and CLI build pass, and independent review accepts the result.

Fresh rules_rust query and build now evaluate past `_support(*, ...)` and stop
at `rust/platform/triple.bzl:28` with `Variable struct not found`. The retained
starlark-rust engine already has an immutable `struct` value and a separately
selectable `LibraryExtension::StructType`, while `loading_globals()` currently
selects only `Print` before adding Bazel package globals. That observation is
an audit input, not implementation authority.

Run only docs packet `WP-4-7A-bazel-struct-builtin-audit`. Authenticate Bazel
9.2's fixed `.bzl` binding and required struct behavior, inventory every
loading-global consumer, and decide whether one complete shared Bazel-loading
globals owner can admit the exact slice without enabling unrelated extensions
or widening BUILD/MODULE/repository environments. Do not edit Rust in the
audit.

Pinned Zabel `c7298478…` guides the architectural requirement that relevant
evaluators consume one complete typed semantics/global environment rather than
reconstructing symbols per call. It does not define `struct` behavior and no
Zig code or representation may be copied; pinned Bazel 9.2 remains sole
behavior authority. M7 stays partial and M7A -> M8 -> M7B remains.

### Bazel `.bzl` `struct` audit accepted (2026-08-25)

Pinned Bazel 9.2 `StarlarkGlobalsImpl` and `StarlarkGlobals` establish the
environment matrix: `struct` is fixed in `.bzl`, cquery and SCL globals, but
not in BUILD, MODULE or REPO globals. `BazelStarlarkEnvironmentTest` proves
BUILD-loaded and MODULE-loaded `.bzl` files declare the same names.
`StructProvider` plus the focused `StarlarkRuleClassFunctionsTest` rows define
construction, fields, immutability, equality, comparison rejection,
concatenation, representation and hashing behavior.

The current rules_rust load uses a smaller discriminating slice. At module
evaluation it constructs `_support` structs from two named bool fields, reads
`support.std` in a dictionary comprehension, stores structs as dictionary
values and freezes/exports that dictionary. `triple.bzl` defines further
named-field constructors, but does not invoke them before this load completes.
No struct comparison, concatenation, provider identity, representation, JSON
or struct-key hashing is required at this frontier.

Retained starlark-rust `LibraryExtension::StructType`, `register_struct` and
`StructGen` already supply named-only construction, immutable field access,
order-insensitive equality/hash and derived freeze. Do not claim its broader
surface exact: unlike Bazel it currently orders structs, lacks Bazel struct
concatenation/provider identity, and formats fields without Bazel's spaces.
Those rows remain unsupported/deferred until a demonstrated consumer requires
them.

Slug's `loading_globals()` is currently shared by BUILD and `.bzl` evaluation,
so adding `StructType` directly would incorrectly widen BUILD. Keep
`package.rs` as the natural complete-environment owner: introduce a sibling
BUILD environment preserving the current Print-only extension set, make the
existing environment the `.bzl` value with exactly Print and StructType, and
route only the two direct BUILD/package consumers to the sibling. Recursive
Host/external/legacy `.bzl` routes continue consuming the same complete value.
No request input, DICE key/equality, invalidation, event, source, evaluator-heap
or retained-module ownership changes. Struct instances freeze into the
existing retained `FrozenBzlModule`; globals remain evaluation-local.

No new oracle is needed: pinned Bazel source/tests discriminate placement and
behavior, while the accepted rules_rust source discriminates the required
operations. The implementation proves a recursive external module constructs,
reads and freezes the real `_support` shape and a Host BUILD still rejects
`struct`. `package.rs` and `bzl_module.rs` exceed the size trigger, but the
change adds one sibling to the existing globals owner and two call-site
selections only; splitting unrelated package/loading orchestration would widen
the packet.

Run only `WP-4-7A-bazel-bzl-struct-builtin`. Exact compatibility is `.bzl`
availability, named construction, field access and frozen recursive export for
the live bool-valued rules_rust slice. Rust storage and nonrequired diagnostics
are Slug-native. BUILD/MODULE/REPO exposure, struct ordering/concatenation/
provider/format/JSON breadth and later rules_rust semantics remain unsupported
or deferred.

Pinned Zabel `c7298478…` guides one complete typed environment owner projected
to each correct evaluator class. It supplies no behavior or representation;
Bazel 9.2 remains sole compatibility authority. M7 stays partial and
M7A -> M8 -> M7B remains.

### Bazel `.bzl` `struct` accepted; provider declaration frontier active (2026-08-25)

Commit `1a527089` implements the audited environment split. `package.rs`
remains the sole complete loading-globals owner: `.bzl` evaluation receives
exactly `Print` plus retained `StructType`, while the sibling BUILD value stays
Print-only. The Host and legacy BUILD routes select the latter; all Host,
external and legacy recursive `.bzl` routes continue to share the former.
MODULE, REPO, cquery and preliminary core environments are unchanged.

The recursive proof constructs the real rules_rust `_support` struct, reads
both bool fields and inspects the frozen value exported through its parent. A
Host BUILD proof keeps `struct` absent. Both focused tests, all 240 loading
tests, locked core check, rebuilt V2 CLI, formatting and hygiene pass;
independent implementation review returned `ACCEPT`.

Fresh rules_rust query and build both clear the struct boundary and converge at
`rust/private/providers.bzl:17`:

```starlark
CrateInfo = provider(doc = ..., fields = {...})
```

Slug's retained `provider` builtin currently accepts the required named
`fields` map but rejects `doc` as an extra named parameter. This live terminal
selects a read-only audit; it does not yet authenticate the complete Bazel
provider surface or authorize a signature change.

Run only docs packet `WP-4-7A-bazel-provider-doc-audit`. Inspect pinned Bazel
9.2 provider source/tests for `doc` and `fields`, trace every immediate
rules_rust provider declaration and operation to the next honest terminal, and
inventory Slug's callable-definition/export owner. Separate loading-time
provider callable construction from provider instances and configured-analysis
semantics; select one bounded implementation or `REPLAN`, with no Rust changes
in the audit.

Exact compatibility covers only the accepted `.bzl` struct placement and live
construction/field/freeze slice. Rust storage and nonrequired diagnostics are
Slug-native. Broader struct semantics, unauthenticated provider parameters,
provider-instance/analysis breadth, toolchains/actions, M8/M7B and exact output
bytes remain unsupported/deferred.

Pinned Zabel `c7298478…` guides a complete typed globals/semantic owner
projected to all relevant consumers; it supplies no provider behavior,
representation or identity. Pinned Bazel 9.2 remains sole compatibility
authority. M7 stays partial and M7A -> M8 -> M7B remains.

### Bazel provider `doc` audit accepted (2026-08-26)

Pinned Bazel 9.2 `StarlarkRuleFunctionsApi.provider` declares named `doc` as
`string | None` with `None` default, named `fields` as sequence/dict/`None`, and
named `init` as an independent optional callable. `StarlarkRuleClassFunctions`
trims a present doc string into `StarlarkProvider.Builder`; `StarlarkProvider`
retains it for `getDocumentation`, but its exported equality/hash use only the
`.bzl` key and exported name. `ProviderApi` exposes no Starlark field for this
metadata.

The discriminating upstream rows are
`StarlarkRuleClassFunctionsTest.declaredProviderDocumentation`,
`declaredProvidersDoc`, and `declaredProvidersBadTypeForDoc`, plus
`StarlarkProviderTest.documentedProvider_getDocumentation` and
`undocumentedProvider_getDocumentation`. `StarlarkDocumentationTest` and
`ModuleInfoExtractorTest.providerDocstring` prove that stored documentation is
observable to Bazel's separate documentation tooling, not to the loaded
provider callable. That tooling is not an admitted Slug command surface.

The accepted rules_rust 0.73.0 source archive has SHA-256
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Its `rust/private/providers.bzl` declares 18 top-level providers; all pass a
string `doc` and dictionary `fields`, including concatenated and parenthesized
strings. There is no `init`, list schema or provider construction in that
module. On successful freeze all 18 callables export; `common.bzl` then loads
six of them and stores the callables in the already accepted `rust_common`
struct. Provider instances appear only when rule implementations later run,
outside this loading packet.

Slug's `package_globals::provider` currently accepts the documented-fields
map and delegates to `UserProviderCallable::from_evaluator`. The latter drops
field prose, sorts/deduplicates semantic names, binds `ProviderId` from source
label plus exported variable name, and freezes only that identity and schema.
Analysis indexes dependencies with this callable identity. Neither provider
documentation nor field prose participates in Bazel provider identity or
Slug's admitted build semantics.

Run only `WP-4-7A-bazel-provider-doc-loading`. Add named
`doc: Option<Value<'v>>` to the existing global adapter: the outer option is
omission, and the adapter accepts only explicit Starlark `None` or a string.
Consume it without retention; do not touch `UserProviderCallable`, `ProviderId`,
analysis, globals placement or DICE. Prove string and `None` acceptance,
non-string rejection, and frozen recursive export using the existing
external-Bzl harness. A source edit still invalidates through the existing
observed module bytes; semantic cutoff may ignore prose-only changes because
documentation is not an admitted build fact.

Exact compatibility is named string/`None` call acceptance, type rejection and
unchanged callable export/freeze identity on the live dictionary-fields route.
Rust storage and nonrequired diagnostic wording are Slug-native. Bazel doc
trimming/retention and Stardoc extraction, field-doc access, fields list/`None`,
`init`, broad provider instances, configured analysis, toolchains/actions,
M8/M7B and exact output bytes remain unsupported/deferred.

Pinned Zabel `c7298478…` guides keeping the complete globals owner and
projecting only build-semantic provider identity/schema into retained values,
without a second metadata registry. No Zabel code, representation or behavior
is copied. M7 stays partial and M7A -> M8 -> M7B remains.

### Bazel provider `doc` accepted; rule documentation frontier active (2026-08-26)

Commit `a81b5823` adds named `doc` to the existing provider adapter, accepts
omission, strings and explicit Starlark `None`, rejects every other value and
retains no prose. The frozen callable continues to own only its sorted field
schema and source-label/exported-name identity. Recursive string/`None` export
and non-string failure proofs pass, as do all 242 loading tests, locked core
check, rebuilt CLI, formatting and hygiene. Independent implementation review
returned `ACCEPT` after the diff was corrected to fit all packet caps.

Fresh disposable rules_rust query and build load all 18 documented providers
and preserve their established public repository-session wrappers. The HTTPS
trace materializes only the exact rules_rust archive before failure. Recursive
source order then reaches `rust/private/rustc.bzl`, which loads
`rust/private/lto.bzl`; after its documented `RustLtoInfo` provider, line 40
calls `rust_lto_flag = rule(doc = ..., build_setting =
config.string(flag = True), ...)`. Slug's current `rule` adapter rejects the
extra named `doc` before changing the frozen rule definition.

Pinned Bazel 9.2 `StarlarkRuleFunctionsApi.rule` declares named-only `doc` as
`string | None` with `None` default. `StarlarkRuleClassFunctions.createRule`
trims and stores a present string through `RuleClass.Builder`, and
`StarlarkRuleClassFunctionsTest.testRuleDoc` authenticates short, multiline
and omitted docs. `RuleInfoExtractor` is the separate documentation consumer;
no admitted Slug command observes this prose.

Run only `WP-4-7A-bazel-rule-doc-loading`. Add string/explicit-`None`
validation to the existing call-shape adapter, deliberately project only the
unchanged build-semantic rule schema/capability into `FrozenRuleDefinition`,
and prove recursive freeze plus non-string rejection. Do not retain docs or
admit another rule parameter.

Exact compatibility is rule-doc call acceptance/type rejection for the live
build-setting declaration. Rust storage and nonrequired diagnostics are
Slug-native. Bazel doc retention/Stardoc, other missing rule parameters,
broader provider/rule analysis, toolchains/actions, M8/M7B and exact output
bytes remain unsupported/deferred. Pinned Zabel `c7298478…` guides the one
complete adapter/narrow semantic projection only; Bazel 9.2 remains sole
behavior authority.

### Bazel rule `doc` accepted; typed config-bool definition selected (2026-08-26)

Commit `6ab6f35d` accepts omitted, string and explicit `None` docs at the sole
`rule` call-shape adapter, rejects every other value and retains no prose.
`RuleDefinitionGen`, `FrozenRuleDefinition`, `RuleCapability`, invocation and
configured analysis remain unchanged. Recursive export/freeze and rejection
proof pass, as do all 244 loading tests, locked core check, rebuilt CLI,
formatting and hygiene. Independent terminal review returns `ACCEPT` within
the two-file 4/62/66 addition caps.

Fresh disposable rules_rust query and build pass `rust_lto_flag` and the next
documented `config.string(flag=True)` declaration. Source order reaches
`rust/private/rustc.bzl:3047-3055`, where the documented
`always_enable_metadata_output_groups` rule supplies
`config.bool(flag=True)`. A second bool descriptor follows before the first
`config.string_list` descriptor. The public query/build terminals remain the
accepted repository-session wrappers.

Pinned Bazel 9.2 `StarlarkConfigApi.bool` declares named-only boolean `flag`
with `False` default. `StarlarkConfig.boolSetting` creates a BOOLEAN
`BuildSetting`; `RuleClass.Builder` derives mandatory nonconfigurable
`build_setting_default` from the descriptor type; and
`ConfigSettingTest.buildsettings_convertedType` proves a boolean default is
typed rather than stringified. `ConfigRules` installs `ConfigBootstrap` through
the `.bzl`-only `addStarlarkBootstrap` path; fixed BUILD globals do not receive
the module.

Slug's complete `ConfigModule` currently exposes only
`RootStringBuildSetting`, while rule definitions retain a string-specific bit
through freeze, equality and target recording. Loading already owns exact bool
attribute coercion. Run only `WP-4-7A-bazel-config-bool-loading`: add the live
`flag=True` descriptor, replace the bit with one compact String/Boolean kind,
derive the corresponding default schema and preserve existing string
projections. The complete `.bzl` config projection gains bool; BUILD retains
its current string-only projection and must prove bool absent. Reject boolean
build-setting invocation before target recording; do not enter analysis, CLI
flags, transitions or config matching.

Exact compatibility is the `.bzl`-only live bool descriptor definition, BUILD
absence, distinct retained kind/schema and recursive export/freeze. Rust
representation, the fail-closed invocation boundary and nonrequired
diagnostics are Slug-native.
Omitted/False bool descriptors, boolean target invocation/analysis, other
config methods, toolchains/actions, M8/M7B and exact output bytes remain
unsupported/deferred.

Pinned `../zabel` `c7298478…` supplies architecture guidance only: keep one
complete typed config/rule owner and expose narrow schema/string projections,
never a second metadata registry. The Buck2 utility audit selects no import:
the enum replaces one bool, preserves existing `Allocative`/Arc/compact-string
ownership and adds no allocation, collection, hash or interner. Bazel 9.2
remains sole behavior authority.

### Repeatable StringList accepted; post-descriptor frontier audit selected (2026-08-26)

Commit `573c25c7` accepts only named `config.bool(flag = True)` in `.bzl`,
keeps bool absent from BUILD, and retains String versus Boolean as one compact
kind through rule-definition construction, recursive export/freeze, equality
and typed `build_setting_default` schema. Boolean invocation rejects before
`PackageRecorder` records a target. The existing string analysis accessor
remains a narrow projection, while BUILD and `.bzl` string construction now
share one private constructor.

Focused config-bool proof passes 3/3 and all 247 loading tests pass. Core
check, rebuilt CLI, formatting and diff checks pass; the archive audit retains
only its known three-path thoughts classification. Final additions are
116 production, 110 proof and 226 total, within the 120/110/230 contract.
Independent terminal review returned `ACCEPT` after correcting shared string
construction and the named-only bool ABI.

Pinned Bazel 9.2 `StarlarkConfigApi.string_list` declares named-only boolean
`flag` and `repeatable`, both defaulting to `False`.
`StarlarkConfig.stringListSetting` rejects `repeatable = True` without
`flag = True`, then retains `STRING_LIST` plus repeatability in the
`BuildSetting`. `RuleClass.Builder` derives a list-typed mandatory default,
and config tests authenticate repeatable accumulation and the invalid
repeatable-without-flag form.

The accepted rules_rust 0.73.0 archive reaches
`rust/private/rustc.bzl:3093` and `:3108`, where nonrepeatable
`config.string_list(flag = True)` descriptors are defined. Line 3120 is the
first `repeatable = True` use, followed by both forms elsewhere. Slug already
owns exact string-list attribute coercion, but its retained build-setting kind
does not yet own StringList or repeatability.

Commit `6811fa84` adds `BuildSettingKind::StringList` to the existing sole
retained definition/freeze/equality owner, derives list-default schema, and
accepts named `.bzl` calls with omitted/false repeatability. BUILD stays
string-only; false/omitted flag and positional calls reject; list targets fail
before `PackageRecorder`. Focused proof and all 248 loading tests pass with
locked core check, rebuilt CLI and hygiene. Final growth is 34/89/123 within
90/90/180; terminal review returns `ACCEPT` after explicit-false flag proof.

Commit `68e458b4` puts the repeatability boolean on the existing evaluation
descriptor and retained StringList variant. False/true definitions compare
unequal while sharing list schema; BUILD remains string-only and every list
target still fails before `PackageRecorder`. Focused proof and all 248 loading
tests pass with locked core check, rebuilt CLI, hygiene and archive baseline.
Final growth is 14 production, 23 proof and 37 total; independent terminal
review returns `ACCEPT`.

Fresh disposable query/build still surface only the generic repository-session
wrappers. The deterministic audit at `a8e18278` instead follows Slug's
recursive external-Bzl evaluator: `rust/defs.bzl` reaches
`rust/toolchain.bzl`, then `rust/private/rust_analyzer.bzl`, whose accepted
children finish before line 207 evaluates the first missing
`rust_analyzer_aspect = aspect(...)`. That definition precedes the file's
later rule calls and the later `rustfmt`, `clippy` and `unpretty` aspects.

Pinned Bazel 9.2 `StarlarkRuleFunctionsApi.aspect`,
`StarlarkRuleClassFunctions.aspect`, `StarlarkDefinedAspect.export`,
`StarlarkDefinedAspectsTest` and `StarlarkRuleClassFunctionsTest` establish a
`.bzl`-only declaration, user-defined Starlark-function implementation, fixed
ordered propagation attributes, a defining-context toolchain requirement,
string/`None` documentation and first producer export identity. No
implementation runs while the declaration loads or freezes.

Run only `WP-4-7A-bazel-aspect-definition-loading`: add the exact fixed
constructor subset adjacent to the first live declaration and retain
implementation lifetime, ordered `attr_aspects`, one canonical direct-string
toolchain requirement, defining module and optional first exported name in one
frozen owner. BUILD absence and recursive imported identity are proof
obligations. The live expression advances to missing `Label`; that global,
aspect attachment, selection, propagation, analysis, actions and every later
call shape remain deferred.

Pinned Zabel `c7298478…` is direct architectural guidance: its complete
`AspectDefinition` owner and distinct `AspectExportIdentity` keep declaration
semantics with the producer while importing aliases use narrow projections.
Slug follows that split with its existing frozen-Bzl lifetime and compact
owners; no Zabel code, representation or behavior is copied. Bazel 9.2 remains
sole compatibility authority. Exact compatibility adds only the admitted
constructor/export/freeze subset; Rust representation and diagnostics are
Slug-native; `Label`, the complete live expression, application plus
Boolean/StringList targets and analysis/CLI, M8/M7B and exact output bytes
remain deferred.

### Rustfmt test-aspect provides audit selects declaration loading (2026-08-26)

Commit `df654bfb` selected the audit at
`rust/private/rustfmt.bzl:194-216`. Pinned Bazel 9.2 establishes that
`provides` converts every already-exported provider to its producer
`Provider.Key` during aspect declaration, normalizes the sequence into an
immutable set, and retains that set in `StarlarkDefinedAspect` equality/hash.
Only later definition/application work advertises the provider and checks the
implementation result.

The fixed call is one same-module
`@@dep+//rust/private:rustfmt.bzl%RustfmtTestInfo` identity. Slug can reuse the
existing transient/frozen provider-ID projection and store one Arc slice in the
frozen aspect owner. Omission preserves earlier empty state. Explicit empty,
duplicate or wider lists, native providers and unexported/non-provider success
remain outside the admitted slice; provider production/matching, application,
propagation, configured dependencies/fragments/toolchains and actions stay
deferred.

Pinned Zabel `c7298478…` guides the architecture only: its complete
producer-owned `AspectDefinition` retains `provides` and follows it during
module freeze while aspect export identity remains distinct. Slug adds no
registry, consumer rebinding or new lifetime owner and copies no Zig code,
behavior, representation, cache or analysis algorithm. Bazel 9.2 remains sole
behavior authority. Run only
`WP-4-7A-rustfmt-test-aspect-provides-loading`.

### Second rustfmt aspect accepted; test-aspect provides audit selected (2026-08-26)

Commit `275e0b24` reuses the frozen rule-attribute schema for exactly
`_config` then `_process_wrapper` and freezes one complete required aspect
value. The two defaults remain canonical in the rustfmt defining repository;
single-file and exec/executable policy survive independently. The required
object preserves
`@@dep+//rust/private:rustfmt.bzl%rustfmt_srcs_aspect`, including the two
`providers.bzl` IDs, without a derived class key, side registry or importer
rebinding. Both implementations remain lazy and configured propagation stays
absent.

Focused proof and all 193 loading unit tests pass, as do the remaining 37
integrations, locked core check, rebuilt CLI and hygiene. The sole full-suite
failure remains the accepted baseline-stale `@external` assertion. Growth is
120 production and 93 proof additions within caps. Independent review returned
`ACCEPT` after the fail-closed matrix added explicit renamed and wider
attribute dictionaries.

This follows pinned Zabel `c7298478…` only as architectural guidance: one
complete producer-owned `AspectDefinition`, a distinct first-export identity,
and a freeze edge to the required value. Slug uses its own existing frozen
schema/value representation; no Zig code, behavior, storage, cache or analysis
algorithm is copied. Bazel 9.2 remains sole behavior authority.

Source order accepts `RustfmtTestInfo`, an ordinary string-list constant and
two lazy function bodies. The third aspect's implementation, ordered
`attr_aspects`, one exported `requires` edge and documentation are already
accepted. Its first unsupported argument is
`provides = [RustfmtTestInfo]` at `rust/private/rustfmt.bzl:214`. Run only
docs packet `WP-4-7A-rustfmt-test-aspect-provides-audit` to authenticate the
flat advertised-provider identity, declaration retention and exact next stop.
Provider production/matching, aspect application/propagation, configured
dependencies/fragments, actions and the later `rustfmt_test` rule remain
deferred.

### First rustfmt aspect requirements accepted; second aspect audit selected (2026-08-26)

Commit `d4d4d6dc` retains exactly the first rustfmt aspect's two singleton
required-provider alternatives and fixed `cpp` fragment in the existing frozen
aspect owner. Recursive proof keeps the two `providers.bzl` export IDs through
`common.bzl` and rustfmt imports; omitted state stays empty, wider and invalid
shapes fail closed, export identity remains producer-owned, and the
implementation stays lazy. All 192 loading unit tests and the remaining 37
integrations pass, with locked core check, rebuilt CLI and hygiene. The sole
full-suite failure is the already-proven baseline-stale `@external` assertion.
Independent terminal review returned `ACCEPT` after exact-shape tightening.

Source order skips `_rustfmt_aspect_impl` at lines 129-150 and reaches
`rustfmt_aspect = aspect(...)` at lines 152-192. The first unknown argument is
the fixed two-entry `attrs` dictionary at lines 170-182. The same declaration
then reuses the accepted two-singleton provider predicate, adds the first
`requires = [rustfmt_srcs_aspect]` edge, reuses fixed `cpp`, and supplies an
already-admitted canonical Label/string toolchain spelling.

Commit `d66059ac` selected the docs audit; it now selects
`WP-4-7A-rustfmt-second-aspect-loading`. Pinned Bazel 9.2 proves the two fixed
private Label descriptors are built and retained after implicit-default checks.
Their defining-module canonical defaults, single-file policy, exec transition
and executable bit remain independent declaration fields. `requires` retains
the required Starlark aspect object; `StarlarkDefinedAspect` derives its class
only during later definition construction, and applied-path assembly owns
duplicate/cycle checks.

Pinned Zabel `c7298478…` supplies architectural guidance only. Its complete
`AspectDefinition` retains named attributes and the required value, while
`AspectExportIdentity` separately records producer module plus first export
and module freeze follows the required child. Slug can therefore reuse its
existing frozen rule-attribute schema and retain the complete required aspect
value in the consumer's frozen-module lifetime, with no registry or importer
rebinding. Do not copy Zig code, representation, evaluator behavior, cache,
analysis algorithm or compatibility claims. Bazel 9.2 remains sole behavior
authority.

The implementation admits exactly `_config` then `_process_wrapper`, their
fixed policies/defaults, and one already-exported required aspect. Omitted
values preserve earlier aspects. Public/wider attributes, configurable or
computed defaults, multiple/unexported requirements, class derivation,
application/propagation, configured fragments/dependencies, actions and later
rustfmt declarations remain deferred. Reuse Arc slices, compact strings,
canonical Labels, frozen values and `Allocative`; add no DICE key, registry,
mapping, cache, I/O, interner, hash or lifetime owner.

### Lint-test attributes accepted; first rustfmt aspect requirements selected (2026-08-26)

Commit `2cbdb148` validates/discards the fixed lint bool documentation and
freezes both scalar label defaults through the defining module. The raw
`@bazel_tools` allowlist spelling resolves with that module's immutable
repository mapping; the typed no-colon runner Label remains exactly
`@@dep+//rust/private/lint_test_runner:lint_test_runner`. The implementation
bodies remain lazy. Focused proof, the remaining loading integrations, locked
core check, rebuilt CLI and hygiene pass; the one full-suite failure is
baseline-identical stale test ordering at `5e9039fe`. Independent terminal
review returned `ACCEPT`.

Recursive source order returns to `rust/private/rustfmt.bzl`. Functions through
line 94 remain lazy, `RustfmtTargetInfo = provider(...)` at lines 96-102 already
constructs, and its implementation body at lines 104-117 remains lazy. The
first unsupported expression is `rustfmt_srcs_aspect = aspect(...)` at lines
119-127: Slug rejects `required_providers` before reaching the adjacent
`fragments = ["cpp"]`.

Pinned Bazel 9.2 `StarlarkRuleFunctionsApi.aspect`,
`StarlarkAttrModule.buildProviderPredicate`,
`StarlarkRuleClassFunctions.aspect`, `StarlarkDefinedAspect`, focused
rule-class predicate tests and the aspect-fragment test establish declaration
behavior. The outer predicate is an OR of inner AND provider sets; provider
constructors must already be exported and retain their producer keys.
Fragments are retained as an immutable name set. Neither fact applies an
aspect or runs its implementation during loading.

Run only `WP-4-7A-rustfmt-first-aspect-requirements-loading`. Put the fixed two
singleton alternatives and fixed `cpp` fragment on Slug's existing transient
and frozen aspect owners. Clone the existing producer-owned `ProviderId` from
transient or frozen provider callables; never reconstruct it from display text
or importer context. Preserve first-export aspect identity separately. Prove a
recursive import through `rust_common`, exact provider IDs/order, fixed
fragment, lazy implementation, omitted empty state, invalid predicate shapes
and BUILD absence. No new DICE key, registry, cache, mapping, I/O or analysis
consumer is admitted.

Exact compatibility covers only the fixed nested predicate, fixed fragment,
producer identity, recursive freeze/export and lazy implementation. Arc-backed
Rust representation, duplicate normalization and diagnostics are Slug-native.
Flat/native/wider predicates, other fragments, aspect application,
advertised-provider matching, `ctx.fragments`, configured targets,
toolchains/actions, later rustfmt declarations, M8/M7B and exact output bytes
remain unsupported/deferred.

Pinned Zabel `c7298478…` is architectural guidance only. Its
`build_rule_declaration.zig` retains provider requirements and fragment names
inside one complete producer-owned `AspectDefinition`, while
`AspectExportIdentity` stays distinct and imported provider identities remain
producer-owned. Slug follows that ownership split with its existing
`ProviderId` and frozen aspect lifetime. No Zig code, representation, evaluator
behavior, cache or analysis semantics are copied; Bazel 9.2 remains sole
behavior authority. The Buck2 utility audit selects existing Arc slices,
compact strings, provider IDs and duplicate-normalization patterns; no new
retained utility family.

### Post-rust-analyzer audit selects defining-module scalar label defaults (2026-08-26)

The accepted recursive external-Bzl evaluator computes `resolved_loads`
serially in source order and stops at the first failed child. Once
`rust/private/rust_analyzer.bzl:484` returns, its caller
`rust/toolchain.bzl:11-14` reaches `rust/private/rustfmt.bzl`. Rustfmt's first
child `common.bzl` is already complete because rust-analyzer loaded it; the
first new child is therefore `rust/private/lint_test.bzl`.

The fixed `transition(...)` at `lint_test.bzl:37-41` and documented label at
lines 46-48 already evaluate. The first unsupported expression is the `doc`
argument on `attr.bool` at lines 49-52 because Slug's bool descriptor does not
yet expose the shared validation-only documentation parameter. Once admitted,
the next unsupported expression is `_allowlist_function_transition` at lines 53-55:
its raw external string default is sent to Slug's package-only label coercer,
which rejects `@` after the defining repository mapping has been discarded.
The immediately adjacent `_runner` default at lines 56-60 is a constructed
`Label("//rust/private/lint_test_runner")`; Slug's raw-value adapter cannot
retain that typed Label either. One coherent declaration-time packet must
admit the fixed bool doc and both scalar label forms or it will not complete
the newly selected child.

Pinned Bazel 9.2 `StarlarkAttrModule.createAttribute`,
`Attribute.Builder.defaultValue`, `BuildType.LabelType.convert` and
`LabelConverter.forBzlEvaluatingThread` establish exact behavior. Strings are
resolved through the innermost defining `.bzl` module's package context and
repository mapping; an already-constructed Label is returned unchanged.
Focused rule-class, remote-label-default and Bzlmod load tests authenticate
conversion at declaration time, before target lookup or rule invocation.

Run only `WP-4-7A-lint-test-label-default-loading-r3`. Add the named `doc` to
the existing bool descriptor and pass it through `discard_attribute_doc`; do
not retain or expose documentation. In the existing
`attribute_definition` owner, retain the full caller-aware
`BzlModuleIdentity`; route only scalar label strings through the shared pure
resolver and clone only an actual `StarlarkLabel`'s canonical identity into
the existing `CoercedAttributeValue::Label`. Preserve every non-label default
path and the accepted label-`None` case. Prove the selected registry's distinct
root alias, module self-name and canonical repo, the exact `@@bazel_tools`
allowlist default, the exact
`@@dep+//rust/private/lint_test_runner:lint_test_runner` runner default,
recursive freeze/export and missing/conflicting mapping failure. Add no map,
DICE compute, I/O, cache,
interner, hash domain or lifetime owner.

Exact compatibility covers validation/acceptance of this fixed bool doc, the
two scalar label-default input forms, their defining-module context, fixed
lint-test dictionary and canonical frozen values. Existing Rust enum/Arc
storage, complete-map over-invalidation and nonrequired diagnostics are
Slug-native. Documentation retention/extraction, label-list/dict defaults, canonical
raw-string breadth, computed/late-bound defaults, target invocation,
transition application/allowlisting, configured dependencies, providers,
rustfmt aspect arguments/analysis, actions and exact output bytes remain
unsupported/deferred.

Pinned Zabel `c7298478…` is architectural guidance only. Its producer-owned
declared-default spelling and retained canonical Label paths support resolving
strings at the defining module, preserving typed Label identity, and keeping a
later BUILD consumer out of repair ownership. No Zig code, representation,
mapping behavior, evaluator rule or DICE relation is copied; pinned Bazel 9.2
remains sole behavior authority. The utility audit selects Slug's existing
identity, mapping, enum and frozen owners without a new retained data structure.

Implementation proof correction: the selected loading fixture names its
synthetic root module `bazel_tools`, so its apparent built-in name intentionally
aliases the root. Renaming that root activates the complete pinned built-in
MODULE dependency closure and first requires absent `rules_license` registry
evidence; an explicit local override is separately unsupported. This does not
show a production mapping gap. Non-root MODULE finalization already injects
the singleton built-in, selected routes retain it in their ordered `SmallMap`,
and the accepted `selected_definition_source_is_request_owned_and_route_structural`
test proves the resulting built-in child route. Reuse that evidence, keep the
synthetic recursive fixture for the selected `rules_rust -> dep+` typed-Label
path, and freeze the exact `@bazel_tools` dictionary in a focused caller-aware
manifest context. Do not expand the registry fixture, change mapping identity,
or add another owner. This is a proof split only; exact/Slug-native/deferred
classification and implementation scope are unchanged.

Second implementation contract correction: the source spelling
`Label("//rust/private/lint_test_runner")` has no colon. Pinned Bazel 9.2
`LabelValidator.parseAbsoluteLabel` assigns the complete
`rust/private/lint_test_runner` path to the package and the last segment
`lint_test_runner` to the implicit target; `LabelParserTest.parserTable`
directly authenticates the same `//foo/bar -> package foo/bar, target bar`
rule. The exact canonical value is therefore
`@@dep+//rust/private/lint_test_runner:lint_test_runner`, not
`@@dep+//rust/private:lint_test_runner`. Pinned Zabel's distinct retained
package-path and target-name projection is architectural guidance for keeping
those identity parts producer-owned, never behavior authority. This is the
second material correction, so the prior packet is stopped and only
the `-r2` packet retained the unchanged two-file boundary and caps.

Third implementation stop: the exact fixed dictionary then failed before
either label default because Slug rejects `attr.bool(doc = ...)`. Pinned Bazel
9.2 `StarlarkAttrModuleApi.boolAttribute` declares string-or-None `doc`, and
`StarlarkAttrModule.boolAttribute` passes it to the common attribute factory;
the accepted rules_rust source supplies that fixed string at lines 49-50.
No Rust from the stopped attempt is retained. `REPLAN` to
`WP-4-7A-lint-test-label-default-loading-r3`: reuse the existing
validation-only `discard_attribute_doc` path, then perform the unchanged two
label conversions under the same files and caps. Documentation retention and
extraction remain deferred. Pinned Zabel continues to guide only the separate
producer-owned label identity architecture.

### Detect-sysroot rule accepted; post-rust-analyzer frontier audit selected (2026-08-26)

Commit `129ff448` makes the accepted pure `.bzl` Label resolver crate-visible
and routes only raw single-`@` rule-toolchain strings through it with the
innermost defining `BzlModuleIdentity`. Canonical `@@...` and relative-string
branches retain their accepted behavior. No second mapping, DICE compute,
lookup, I/O, cache, hash domain or lifetime owner is introduced.

The selected-registry proof retains root apparent `dep_alias`, module self-name
`rules_rust` and canonical `dep+` as distinct spellings. It recursively imports
and freezes `rust_analyzer_detect_sysroot` with ordered mandatory requirements
`@@dep+//rust:toolchain_type` and
`@@dep+//rust/rust_analyzer:toolchain_type`; both the detect implementation and
the prior current-toolchain implementation remain lazy. Missing and conflicting
mapping cases reject through the raw rule-string converter itself.

Focused proofs and all 256 loading tests pass with locked core check, rebuilt
CLI, formatting and diff checks. Archive status has only the known three
retained thoughts paths. Final additions are 7 production and 33 proof, 40
total, within every cap; independent terminal review returned `ACCEPT`.

Exact compatibility covers the fixed two raw string conversions, defining
module mapping, mandatory policy, source order, recursive freeze, doc value and
export. Existing Arc storage and nonrequired diagnostics are Slug-native.
Label objects, optional/duplicate requirements, invocation, `ctx.toolchains`,
selection, configured dependencies, provider/path semantics, JSON FileWrite,
`DefaultInfo`, aspect application and analysis remain unsupported/deferred.

Pinned Zabel `c7298478…` guided only reuse of the immutable defining-module
mapping and pure canonical projection. Its native BUILD `toolchain(...)`
resolver was explicitly not treated as the behavior analogue, and no Zig code,
mapping behavior, representation or DICE relation was copied. Bazel 9.2
remained sole behavior authority.

The accepted `rust/private/rust_analyzer.bzl` ends at line 484. Evaluation now
returns through the recursive load stack to `rust/toolchain.bzl`, whose next
source load names `//rust/private:rustfmt.bzl`. That module itself loads
`common.bzl` and `lint_test.bzl` before reaching its own provider/aspect/rule
declarations; already-completed modules may be memoized from the accepted
closure. Run only docs packet
`WP-4-7A-post-rust-analyzer-source-order-audit`: replay Slug's selected-route
recursive manifest against the accepted rules_rust archive, identify the first
newly evaluated unsupported expression, authenticate its behavior with pinned
Bazel 9.2, consult pinned Zabel only for bounded Rust ownership guidance, and
select one implementation or `REPLAN`. Do not edit Rust or assume that a later
rustfmt declaration is the immediate frontier.

### Current-toolchain rule accepted; detect-sysroot rule loading selected (2026-08-26)

Commit `61cb0ad0` projects the selected route's existing ordered repository
mapping into every recursive external `BzlModuleIdentity`, its equality/hash
and the manifest fingerprint. Local, generated and built-in identities remain
mapping-empty and fail closed. Native-call source provenance now returns the
complete innermost defining identity, so the shared `.bzl` Label resolves only
the bounded `@name//package:target` form through that identity's immutable
mapping. Missing and duplicate/conflicting entries reject without I/O, route
lookup or a DICE compute. The fixed `str(Label(...))` canonical spelling is
accepted only by the existing rule-toolchain string converter and retained in
the frozen rule definition.

The selected-registry proof discriminates three identities: root apparent
`dep_alias`, module-local self-name `rules_rust`, and canonical `dep+`. A
recursively imported declaration freezes one mandatory
`@@dep+//rust/rust_analyzer:toolchain_type` requirement and never invokes its
failing implementation. Complete-mapping changes alter module identity and
fingerprint, and a conflicting apparent mapping rejects.

Focused tests, all 545 `slug_bzlmod_v2` unit tests plus every integration
suite, and all 256 `slug_loading_v2` tests pass. Formatting, locked core check,
rebuilt CLI and diff gates pass; archive status has only the known three
retained thoughts paths. Final additions are 115 production and 85 proof, 200
total. Independent review first rejected a 168-line touched test; extracting a
30-line assertion helper reduced it to 142 lines, and re-review returned
`ACCEPT`.

Exact compatibility covers the fixed defining-module lookup, canonical
string handoff, mandatory direct requirement, recursive freeze and export.
The Arc representation, complete-map over-invalidation, fingerprint framing
and diagnostics are Slug-native. Wider mapping producers, raw apparent rule
strings, Label/toolchain input breadth, invocation, selection, configured
dependencies and analysis/actions remain unsupported/deferred.

Pinned Zabel `c7298478…` guided immutable mapping ownership on the defining
module, currently executing module lookup and a thin canonical declaration
projection only. No Zig code, mapping behavior, storage, evaluator or DICE
relation was copied; Bazel 9.2 remains sole behavior authority.

The accepted rules_rust 0.73.0 source next reaches the lazy
`_rust_analyzer_detect_sysroot_impl` body at
`rust/private/rust_analyzer.bzl:431-473`, then the next evaluated declaration:
`rust_analyzer_detect_sysroot = rule(...)` at lines 475-484. The implementation
body is retained but not called during declaration construction. Its fixed
toolchain list contains, in order,
`@rules_rust//rust:toolchain_type` and
`@rules_rust//rust/rust_analyzer:toolchain_type` at lines 478-479; its `dedent`
call produces the already-admitted string documentation value.

Pinned Bazel 9.2 `BazelModuleContext`,
`LabelConverter.forBzlEvaluatingThread`, `Label.parseWithPackageContext`,
`StarlarkRuleClassFunctions.createRule` and `parseToolchainTypes` establish
that both plain strings use the innermost defining module's package context
and repository mapping, become mandatory requirements and preserve first-label
order through the `LinkedHashMap`/immutable-set projection. The focused
add-toolchain and ordered-requirements tests authenticate the string,
mandatory and ordering behavior; duplicate/strictest behavior is dormant
because the two labels are distinct.

Slug now retains the exact selected mapping and already has a pure bounded
apparent-label resolver in `starlark_label.rs`. The first absent fact is only
that `rule_toolchain_requirement` sends noncanonical strings to its older
root-only package parser, which rejects `@rules_rust`. Run only
`WP-4-7A-rust-analyzer-detect-sysroot-rule-loading`: expose the existing pure
resolver crate-locally, reuse it only for raw apparent rule-toolchain strings,
and freeze the two canonical requirements in source order. Do not add a map,
cache, key, lookup owner or I/O path.

Prove the exact selected-registry mapping with a deliberately different
canonical spelling, the two ordered frozen requirements, recursive export,
lazy implementation, and missing/conflicting failure before freeze. Preserve
the accepted canonical handoff and relative rule-toolchain behavior. Label
objects, optional requirements, duplicate/strictest behavior, wider Label
forms, target invocation, `ctx.toolchains`, toolchain resolution, provider
access, path manipulation, JSON FileWrite action, `DefaultInfo`, M8/M7B and
exact output bytes remain deferred.

Pinned Zabel's immutable retained module repository context and pure shared
Label host guide reuse of the single selected mapping and canonical Label
projection. Its native BUILD `toolchain(...)` declaration resolver is not the
Bazel behavior under audit and supplies no mapping or rule-toolchain semantics.
No Zig code, representation or DICE relation is copied. The utility-reuse
audit selects the existing Arc slice, `CanonicalLabel` and frozen Arc order.

### Current rust-analyzer toolchain-rule audit selects defining-module mapping (2026-08-26)

Pinned Bazel 9.2 authenticates the exact
`current_rust_analyzer_toolchain = rule(...)` declaration at accepted
rules_rust lines 423-429. `BazelModuleContext` owns each `.bzl` module's label
and repository mapping; `LabelConverter.forBzlEvaluatingThread` selects the
innermost executing module, so imported functions retain their defining
mapping. Apparent self-names resolve through explicit mapping entries,
`str(Label(...))` emits canonical `@@...` spelling, and a plain string passed
to `parseToolchainTypes` is one mandatory requirement. Ordered/strictest
deduplication is authenticated but dormant for the fixed one-element list.

Slug's selected external route already owns the ordered mapping as an Arc and
recursive child routes already carry each child's own selected mapping. The
mapping participates in route/DICE equality but is discarded when the route
becomes a `BzlModuleIdentity`; evaluator caller provenance therefore supplies
only a canonical source label. The bounded Label builtin rejects the explicit
repository, and the rule-toolchain converter separately cannot accept the
canonical string handoff.

Run only `WP-4-7A-current-rust-analyzer-toolchain-rule-loading`: retain the
existing route mapping Arc with each recursive module identity, fingerprint it,
select the full identity through the existing native-call-source/`DefInfo`
path, resolve only mapped `@name//package:target`, and retain the resulting
canonical one-element requirement in the existing frozen rule owner. Prove a
module-local `rules_rust` self-name mapping to a deliberately different
canonical repository, recursive imported ownership, and missing/conflicting
failure. Do not guess aliases or add a second map, DICE compute or I/O path.

Exact compatibility covers the fixed selected-registry apparent-self Label,
canonical string handoff, one mandatory requirement, recursive freeze and
producer export identity. Arc storage, complete-mapping over-invalidation,
fingerprint framing and nonrequired diagnostics are Slug-native. Other route
families, direct apparent rule-toolchain strings, Label/toolchain input breadth,
target invocation, `ctx.toolchains`, selection, configured dependencies,
analysis/actions and later declarations remain deferred.

Pinned Zabel `c7298478…` is architectural guidance only. Its retained module
repository context supports the same explicit-input/currently-executing-module
shape, and its declaration resolver reinforces a thin canonical projection;
the latter is a native `toolchain(...)` surface, not behavioral authority for
`rule(toolchains)`. No Zig code, mapping behavior, storage or DICE relation is
adopted. Bazel 9.2 remains sole behavior authority, and the utility-reuse audit
selects Slug's existing Arc and canonical identity owners.

### Rust-analyzer toolchain declaration accepted; apparent-self Label audit selected (2026-08-26)

Commit `eda81a4d` accepts and freezes the exact six-attribute
`rust_analyzer_toolchain` schema. Attribute docs validate strings/`None` and are
discarded; executable and exec-transition fields survive descriptor and rule
freeze independently of mandatory, single-file, defaults and custom
transitions. Target invocation gates executable-true or exec-configured fields
before recording, while non-executable custom transitions remain unchanged.
Caller-aware canonical source identity now also owns external default coercion.

Focused proof and all 256 loading tests pass with locked core check, rebuilt
CLI, formatting, hygiene and the unchanged archive baseline. Growth is 96
production, 134 proof and 230 total. Independent review returned `ACCEPT` after
the frozen executable/custom-transition discriminator was added.

Pinned Zabel `c7298478…` guided only the single declaration-schema owner and
thin target-value projection. No Zig code, layout, mapping, DICE relation or
behavior was adopted; Bazel 9.2 remains sole authority.

The next evaluated source expression is the explicit apparent-self Label in
`current_rust_analyzer_toolchain = rule(toolchains = ...)` at
`rust/private/rust_analyzer.bzl:423-429`; the preceding lines 404-421
implementation body is not executed during declaration loading. The
toolchains list spans lines 426-428 and calls the apparent-self Label on line
427. Run only docs audit
`WP-4-7A-current-rust-analyzer-toolchain-rule-audit` to authenticate Bazel's
defining-module repository mapping, the Label/string handoff and retained
toolchain requirement before implementation or analysis.

### Rust-analyzer toolchain-rule audit selects fail-closed declaration loading (2026-08-26)

The accepted rules_rust archive reaches
`rust/private/rust_analyzer.bzl:359-402`, whose first post-aspect rule has four
documented label descriptors and two documented string descriptors. Pinned
Bazel 9.2 establishes that docs are trimmed metadata; executable labels require
a non-`None` configuration; `cfg = "exec"`, executable, single-artifact and
mandatory policy are retained independently; and string defaults remain typed
schema values. Rule freeze/export keeps the implementation and defining-module
class identity without invoking it.

Slug's first failure is the unknown label `doc` argument. It already owns the
mandatory, `allow_single_file`, default, custom-transition, implementation and
export fields, but its label `cfg` accepts only a custom transition and it has
no attribute-executable field. Run only
`WP-4-7A-rust-analyzer-toolchain-rule-loading`: accept string/`None` attribute
docs without retaining a documentation side store, add executable policy and
an exec-transition marker to the existing descriptor/frozen rule-schema owner,
and load the exact fixed declaration. Omitted and explicit `False` executable
values are identical false policy with either admitted cfg; `True` requires
exec or the existing custom transition. A rule with executable true or an exec
marker must reject target invocation before recording until configured exec
dependencies are implemented; non-executable custom-transition invocation
remains unchanged.

Exact compatibility covers the fixed named call shapes, invalid doc type and
executable-without-cfg rejection, structural retention, string defaults,
recursive freeze and producer export identity. Rust fields, diagnostics and
the fail-closed target boundary are Slug-native. Wider cfg forms, documentation
extraction, target invocation, executable prerequisite validation, configured
dependency/analysis/action semantics and later declarations remain deferred.

Pinned Zabel `c7298478…` supplies architectural guidance only: keep executable,
single-file and transition policy in one declaration-owned ordinary-dependency
schema, separate from target-local values and executable-module identity. No
Zig layout, code, DICE relation or behavior is adopted. Bazel 9.2 remains sole
behavior authority, and the utility-reuse audit adds no new collection,
interner, cache or hash domain.

### Bounded Bazel `Label` loading accepted; rust-analyzer toolchain-rule audit selected (2026-08-26)

Commit `84ddb6a3` installs a bounded `.bzl` `Label` constructor over one shared
`CanonicalLabel`-owned Starlark value. It admits only `//...`, `:...` and Label
idempotence, completes the fixed aspect toolchain expression, and keeps loaded
aliases unusable in BUILD. General repository mapping, explicit repositories,
wider value APIs and aspect application remain deferred.

`BzlLoadManifest` now supplies one byte-preserving source-name projection used
by the parser and evaluator context. Typed call-expression source wins over an
outer `DefInfo`, preserving the defining module when an imported function is
compiler-inlined inside a non-inlined caller; `DefInfo` remains the fallback
for non-inlined definitions. Missing and ambiguous mappings fail closed. A
cross-package recursive frozen-module proof discriminates direct alias caller
ownership from imported-function owner identity.

All loading tests, the focused vendored runtime proof, locked core check,
rebuilt CLI, formatting and diff gates pass; archive status retains only its
known thoughts classification. Growth is 295 production, 134 proof and 429
total. Independent terminal review returns `ACCEPT`.

Source order now reaches `rust/private/rust_analyzer.bzl:359-402`, the first
`rust_analyzer_toolchain = rule(...)` after the accepted aspect. Its schema
uses documented `attr.label` declarations with `cfg = "exec"`, executable,
single-file and mandatory policy, followed by documented string defaults.
Run only docs packet `WP-4-7A-rust-analyzer-toolchain-rule-audit` to establish
the Bazel 9.2 call/retention boundary and select a bounded implementation or
`REPLAN`. Pinned Zabel `c7298478…` guides the single retained attribute-schema,
declaration-owner and executable-module split only; no Zabel code or behavior
is copied.

### Fixed aspect definition accepted; Bazel `Label` audit selected (2026-08-26)

Commit `840d28e7` exposes `aspect` only during complete `.bzl` evaluation and
freezes one bounded definition owner with the implementation, ordered fixed
`attr_aspects`, one direct canonical toolchain requirement, defining source
and optional first export name. Recursive imports preserve producer identity;
an unexported nested definition remains unnamed. BUILD absence, imported
factory rejection, native-function rejection and fixed-ABI failures are
covered. Focused proof passes 3/3 and all 251 loading tests pass with locked
core check, rebuilt CLI, hygiene and the unchanged archive baseline. Growth is
153 production, 120 proof and 273 total; independent review returns `ACCEPT`.

Source order now stops inside that same accepted rules_rust declaration at
`str(Label("//rust:toolchain_type"))`. Run only docs packet
`WP-4-7A-bazel-label-global-audit`. Inspect pinned Bazel 9.2
`StarlarkRuleFunctionsApi.Label`, `StarlarkRuleClassFunctions.label`, the
Starlark `Label` value and their focused tests; determine the exact smallest
constructor/stringification slice and its BUILD boundary. Audit Slug's
`BzlEvaluationContext`, `CanonicalLabel`, existing module-extension Label
wrapper and repository-route ownership before selecting reuse or a bounded
split. Slug currently installs one outer-module evaluator context; the audit
must find typed innermost Starlark-function frame provenance or fail closed,
because an imported function containing `Label` resolves in its defining
`.bzl`, not the outer module. General apparent-repository mapping, wider
fields/methods, attribute conversion and all aspect application remain
deferred unless separately authenticated.

Pinned Zabel `c7298478…` is direct architecture guidance only: its generic
Label host keeps canonical identity on the value while the shared builtin
consults the executing function's defining module context rather than the
outer evaluator or builtin exporter. Its mapping observer/side effects and
runtime are not candidates for import. Bazel 9.2 remains sole compatibility
authority.

### Bazel `Label` audit accepted; bounded loading implementation selected (2026-08-26)

The audit confirms the live expression needs construction from
`"//rust:toolchain_type"`, canonical `str` and the aspect adapter's acceptance
of that same-repository canonical string. A package-relative `:target` form is
also admitted solely to discriminate Bazel's defining-function context from
the outer evaluator. Label input is idempotent. Bare labels, explicit apparent
or canonical repository spellings, repository mappings and wider methods stay
deferred.

Slug has a typed bounded route. Vendored Starlark `DefInfo` retains the
definition `CodeMap`; add one narrow evaluator accessor returning that filename
for the Starlark function directly calling a native builtin. The existing flat
`BzlLoadManifest.reachable` pairs every recursively retained module's exact
logical source path with its canonical label. Extend evaluator-scratch
`BzlEvaluationContext` with that projection: function calls look up their
definition filename, top-level direct aliases use the manifest root and every
missing/mismatched source fails closed. BUILD lacks this context. No call-stack
text parsing, filesystem inference, new DICE key or repository-map guess is
needed.

Move `InvocationLabel` unchanged into a small shared loading-owned Label
module and rename it; update both module-extension consumers and add the `.bzl`
constructor there. This preserves the already-accepted canonical owner,
string/repr/hash/equality, `name`, `package`, repository-name aliases and
`same_package_label` surface without a duplicate wrapper or new interner. The
Buck2 utility audit selects existing `CanonicalLabel`, compact/frozen values
and Arc manifest owners only.

Run only `WP-4-7A-bazel-label-global-loading`. Proof must distinguish a live
top-level call, a direct alias, an imported function defined in another
package, BUILD alias rejection, missing frame-map rejection, input boundaries
and the complete fixed aspect declaration. Pinned Zabel `c7298478…` supplies
concept/test guidance for value/context ownership only; no code or behavior is
copied, and Bazel 9.2 remains sole authority.

### Rustfmt test aspect accepted; target-attribute audit selected (2026-08-26)

Commit `50205fb3` extends the existing frozen aspect declaration with the
fixed singleton advertised-provider identity required by
`_rustfmt_test_aspect`. The recursive importer proof retains the third aspect,
its required `rustfmt_aspect`, the nested `rustfmt_srcs_aspect` and
`RustfmtTestInfo` under their defining module and first export names. Both
required producer labels are explicit. Omitted advertised providers stay
empty, and unsupported explicit shapes reject during loading. No aspect
application, provider matching or implementation execution is admitted.

Focused proof and all 194 loading unit tests pass. The unaffected integration
suites, locked core check, rebuilt CLI and format/diff gates pass; the sole
full-suite failure is the already-recorded stale `@external` expectation.
Growth is 23 production and 101 proof lines within caps. Independent terminal
review returned `ACCEPT` after the recursive aspect identity assertions were
made complete.

Source order reaches `rust/private/rustfmt.bzl:218-243`. Its `rustfmt_test`
rule merges the already accepted `LINT_TEST_COMMON_ATTRS` with one `targets`
label-list descriptor. Slug's label-list constructor currently lacks that
descriptor's first named argument, `doc`, and also lacks its coupled
`providers`, `aspects` and `cfg = platform_transition` facts. Run only
`WP-4-7A-rustfmt-test-target-attribute-audit`: authenticate dict merge and
duplicate behavior, Bazel descriptor validation/retention, provider and aspect
producer identities, and custom dependency-transition ownership before
selecting one bounded declaration-only implementation or `REPLAN`.

Pinned Zabel `c7298478…` guides only the owner boundary: its `AttrDefinition`
keeps provider, aspect and transition declaration values together, and its
configured capture later detaches their identities and provenance without
reconstructing them at a consumer. No Zig code, layout, behavior, evaluator,
cache or analysis algorithm is adopted. Bazel 9.2 remains sole behavior
authority.

### Rustfmt test target-attribute audit accepts bounded loading (2026-08-26)

Pinned Bazel 9.2 confirms the `targets` label-list call constructs one
immutable attribute factory. Its doc is trimmed metadata; the two nested
singleton provider lists are ordered OR alternatives of immutable producer-ID
sets; the attached aspect must already be exported and is retained with its
required-aspect closure; and `cfg = platform_transition` wraps the complete
Starlark transition object without executing it. Rule construction records
aspect propagation and custom-transition presence but remains lazy.

The exact dictionary merge is already supported: `dict(base, **overlay)`
updates a collision in place and appends a new key. `LINT_TEST_COMMON_ATTRS`
does not contain `targets`, so its four accepted descriptors remain first and
the fixed descriptor is fifth.

Run only `WP-4-7A-rustfmt-test-target-attribute-loading`. Reuse the existing
frozen rule schema, `ProviderId`, complete frozen aspect/transition values and
Arc/Option owners. Accept only the fixed two distinct singleton provider
alternatives and one exported aspect when those arguments are present;
omission remains empty for earlier label lists. Validate/discard docs as in
the accepted label path. Preserve every producer identity through recursive
freeze and reject target invocation before provider/aspect metadata could be
dropped. Do not apply aspects, evaluate transitions or match providers.

Pinned Zabel `c7298478…` remains architectural guidance only. Its single
declaration-owned `AttrDefinition` and later detached invocation capture
support the same phase boundary; no Zig code, representation, behavior, cache
or analysis algorithm is adopted. Bazel 9.2 remains sole authority. The
Buck2-utility audit selects existing Arc slices, `ProviderId`, frozen values,
compact strings and `Allocative`, with no new collection or interner.

### Rustfmt test target attribute accepted; post-rustfmt source-order audit selected (2026-08-26)

Commit `88304c2f` retains the fixed `rustfmt_test.targets` dependency schema in
the existing transient/frozen rule owner. Its two ordered singleton provider
alternatives, complete attached aspect and custom transition remain distinct
declaration facts. Recursive proof preserves every provider/aspect defining
label and first export. Target invocation rejects before configured loading
could drop provider or aspect metadata; no dependency policy is applied.

Focused proof, all 196 loading unit tests and unaffected integrations pass.
Locked core check, rebuilt CLI, formatting and diff checks pass; the one broad
integration failure is the already-recorded stale `@external` diagnostic
expectation. Growth is 66 production and 175 proof additions within packet
caps. Independent review returned `ACCEPT` after a direct duplicate-aspect
rejection row closed its only requested proof gap.

Recursive source order does not jump directly to another public ruleset. The
tail of `rust/private/rustfmt.bzl` contains lazy implementation bodies and two
toolchain rule declarations that appear to use accepted label schemas and
canonical toolchain strings. Evaluation then returns to `rust/toolchain.bzl`,
passes the alias-only rust-analyzer wrapper, and enters
`rust/rust_stdlib_filegroup.bzl`, which loads `rust/private/toolchain.bzl`.
That module's first mapped child is
`@bazel_skylib//rules:common_settings.bzl`; provider and string-attribute
declarations precede the first candidate absent expression,
`config.int(flag = True)` at line 71, followed by `config.int()` at line 81.

Run only docs packet `WP-4-7A-post-rustfmt-source-order-audit`. Authenticate
the complete recursive manifest/cached-child order, prove that mapped external
loading is already admitted, verify the remaining rustfmt declarations against
the live Slug surface, and establish the exact Bazel integer build-setting
descriptor contract before selecting one implementation or `REPLAN`. Do not
add Rust or advance to `attr.label_list(allow_files = True)` during the audit.

Pinned Zabel `c7298478…` supplies architectural guidance only: its evaluator-
free `BuildSettingDefinition` owns `BuildSettingKind.int` beside bool, string
and list kinds. This supports a single declaration-owned typed descriptor, not
any compatibility conclusion. No Zig code, representation, evaluator,
configured capture or analysis behavior is adopted; pinned Bazel 9.2 remains
sole behavior authority.

### Post-rustfmt audit accepts Bazel config-int loading (2026-08-26)

Commit `1e2759c2` selected the recursive source-order audit. The accepted
rules_rust archive completes both remaining `rustfmt.bzl` toolchain rules with
already-admitted docs, label attributes and canonical toolchain strings. Their
implementations remain lazy. Evaluation returns through the alias-only
rust-analyzer wrapper, enters `rust/rust_stdlib_filegroup.bzl`, and recursively
loads `rust/private/toolchain.bzl`.

That module's first child maps through the already-selected producer view to
`bazel_skylib@1.8.2`. `MODULE.bazel.lock` fixes its source JSON SHA-256 at
`34a3c8bcf233b835eb74be9d628899bb32999d3e0eadef1947a0a562a2b16ffb`.
The JSON names archive SHA-256
`6e78f0e57de26801f6f564fa7c4a48dc8b36873e416257a92bbb0937eeac8446`;
the reached `rules/common_settings.bzl` hashes to
`f3bcedef4b2b2cbe9750d61852917954499c4ba5e83d79fb975ec5814eb76d20`.
No new route, mapping, source owner, I/O or DICE key is required.

The selected child has no recursive loads. Provider/string-attribute
declarations through line 69 are already admitted or lazy. Its first absent
evaluated expression is `config.int(flag = True)` at line 71; the adjacent
`int_setting` uses `config.int()` at line 81. Pinned Bazel 9.2 declares `flag`
named-only with default `False`, creates one INTEGER build-setting descriptor
carrying that bit, and makes the enclosing rule add mandatory,
nonconfigurable integer `build_setting_default` plus string `help`.

Run only `WP-4-7A-bazel-config-int-loading`. Add the integer descriptor to the
existing `.bzl` Config module and retained build-setting kind. Accept named
`True`, omitted and explicit `False`; retain the bit through recursive freeze
and semantic equality; derive the Integer schema; keep BUILD absence; reject
integer target invocation before package recording. Positional, nonboolean,
`None` and unknown arguments reject through the typed Starlark ABI. Do not
admit CLI parsing, configured values, transitions, provider returns, analysis
or actions. After acceptance, source order stops at `config.bool()` on line
100; `attr.label_list(allow_files = True)` remains later.

Pinned Zabel `c7298478…` remains architectural guidance only. Its
declaration-owned `BuildSettingDefinition { kind, flag, ... }` corroborates
keeping the producer fact together, but supplies no behavior or code. The
Buck2-utility audit selects Slug's existing small Copy enum, frozen rule owner
and `Allocative` Starlark value; no collection, interner or utility import is
needed. Bazel 9.2 is the sole behavior authority.

### Config-int accepted; Bazel config-bool false identity selected (2026-08-26)

Commit `9685d9a7` adds the integer descriptor to the existing `.bzl` Config
module and retained `BuildSettingKind`. Named true, omitted and explicit false
forms preserve INTEGER kind and flag polarity through rule projection,
recursive freeze and equality. The existing schema builder derives mandatory,
nonconfigurable Integer `build_setting_default` and optional nonconfigurable
string `help`; BUILD still has no integer constructor. Integer target
invocation rejects in the small pre-recording gate, so implementations and
configured behavior remain untouched.

Focused config-int proof passes 2/2, adjacent typed-descriptor proofs and all
198 loading unit tests pass, and locked core check, rebuilt CLI, formatting and
hygiene are green. The broad loading integration remains 30/31 with only its
recorded stale `@external` diagnostic-order expectation. Final growth is 32
production and 108 proof additions within the 50/110/160 caps. Independent
terminal review returned `ACCEPT`.

Selected `bazel_skylib@1.8.2` source order then freezes `bool_flag` at lines
89-96 through the already-admitted `config.bool(flag = True)` descriptor. The
next declaration, `bool_setting`, evaluates `config.bool()` at line 100. This
is the first absent expression: Slug's typed method currently rejects omitted
or false flags and `BuildSettingKind::Boolean` cannot retain their polarity.
The same authenticated source JSON, archive and `common_settings.bzl` hashes
remain authoritative; no new mapping, source observer or I/O is required.

Pinned Bazel 9.2 declares Boolean `flag` named-only with default `False`, passes
it into `BuildSetting.create(flag, BOOLEAN)`, and derives mandatory,
nonconfigurable Boolean `build_setting_default` plus optional string `help`.
Run only `WP-4-7A-bazel-config-bool-false-loading`: turn the existing Boolean
descriptor into `{ flag }`, accept named true, omitted and explicit false,
preserve equality/discrimination through freeze, keep BUILD absence and reject
all Boolean target invocation before recording. Do not interpret the flag,
coerce defaults, parse CLI values, run implementations or add configured,
transition, provider, analysis or action behavior. Stop next at
`config.string_list()` on line 133.

Pinned Zabel `c7298478…` supplies architectural guidance only. Its one
evaluator-free `BuildSettingDefinition` keeps Boolean kind and flag together,
supporting the existing declaration-owned frozen descriptor rather than a side
registry. No Zig code, layout, diagnostics, evaluator behavior or configured
algorithm is adopted; Bazel 9.2 remains the sole behavior authority. The Buck2
utility audit selects the current Copy enum and `Allocative` Starlark value,
with no new collection, hash, interner or cache.

### Config-bool false accepted; Bazel config-string-list false selected (2026-08-26)

Commit `52d2c6f2` turns the existing Boolean build-setting descriptor into a
complete `{ flag }` semantic fact. Named true, omitted and explicit false forms
retain polarity through rule projection, recursive freeze and equality while
sharing the Boolean default/help schema. BUILD still exposes no Boolean
constructor. The unchanged Boolean invocation rejection was extracted from the
oversized invoke body into the small deferred-attribute helper, and no other
invoke line changed.

Focused config-bool, typed-freeze and ABI tests pass, as do all 198 loading
unit tests, locked core check, rebuilt CLI, formatting and hygiene. The broad
loading integration remains 30/31 with only the recorded stale `@external`
diagnostic-order expectation. Final growth is 15 production and 76 proof
additions within packet caps; independent terminal review returned `ACCEPT`.

Selected Skylib source order next evaluates the already-admitted
`string_list_flag` and `repeatable_string_flag` at lines 107-129. The first
absent expression is `config.string_list()` at line 133: Slug rejects an
omitted/false flag and its retained StringList kind owns only repeatability.
After this declaration, `_string_impl` remains lazy and `string_flag` uses
accepted descriptors; `config.string()` at line 172 is the next missing
expression. The same authenticated source JSON, archive and child hashes apply.

Pinned Bazel 9.2 declares `flag` and `repeatable` named-only and false by
default. It retains both on the STRING_LIST `BuildSetting`, rejects
`repeatable=True` without `flag=True`, and derives mandatory nonconfigurable
list `build_setting_default` plus optional nonconfigurable string `help`.
Pinned `ConfigSettingTest` supplies the invalid-pair diagnostic and
repeatability discrimination evidence.

Run only `WP-4-7A-bazel-config-string-list-false-loading`. Add `flag` to the
existing evaluation descriptor and compact retained StringList variant; accept
the complete valid flag/repeatable matrix and preserve the invalid-pair
diagnostic. Keep BUILD absence and the existing wildcard pre-recording
invocation rejection. Do not parse CLI values, accumulate repeats, run an
implementation or add configured, transition, provider, analysis or action
behavior. Stop next at `config.string()` on line 172.

Pinned Zabel `c7298478…` supplies architecture guidance only: its evaluator-
free `BuildSettingDefinition` owns StringList kind, flag and repeatability in
one producer value. No Zig code, layout, diagnostic, evaluator behavior or
configured algorithm is copied. The Buck2 utility audit selects Slug's current
Copy enum and `Allocative` value; no collection, interner, hash or cache is
introduced. Bazel 9.2 remains sole behavior authority.

### Config-string-list false accepted; Bazel config-string descriptor selected (2026-08-26)

Commit `297c2286` retains StringList `flag` and `repeatable` together through
rule projection, recursive freeze and equality. Every valid named pair loads;
false-flag/true-repeatable rejects with Bazel's pinned diagnostic. The exact
producer rule class, list default/help schema, omitted/explicit false equality
and all valid identity discriminators are covered. BUILD stays unchanged and
true and false list targets fail before recording.

Focused StringList, typed-freeze and ABI proof, all 198 loading units, locked
core check, rebuilt CLI, formatting and hygiene pass. The broad integration
remains 30/31 with its sole recorded stale `@external` diagnostic-order row.
Final growth is 7 production and 97 proof additions. Terminal review requested
non-Boolean integer ABI cases and exact imported producer identities, then
returned `ACCEPT` after correction.

The remaining selected `common_settings.bzl` body is lines 149-181. Its helper
implementation remains lazy, and `string_flag` at line 159 uses the accepted
true/single descriptor and StringList attribute schema. `string_setting` calls
`config.string()` at line 172, the first absent expression and the last
declaration in this child.

Pinned Bazel 9.2 declares named-only Boolean `flag` and `allow_multiple`, both
defaulting false. `StarlarkConfig` retains both with STRING kind; no Boolean
pair is invalid. Pinned integration tests distinguish true, omitted and
explicit false flag identities, while config-setting/options tests establish
the separate allow-multiple fact. Slug's descriptor and retained kind are unit
values, and its `.bzl` method currently accepts `flag` positionally and has no
allow-multiple argument.

The existing configured consumer supports only a true, non-multiple scalar
root string setting. Allowing other variants to record would incorrectly route
non-flags through explicit overrides or require list-valued configuration,
transition and `ctx.build_setting_value` semantics. Run only
`WP-4-7A-bazel-config-string-descriptor-loading`: retain both declaration bits,
but reject non-flag and allow-multiple target invocation before recording.
Keep the true/single path and existing BUILD true-only constructor unchanged.
Once common settings finishes, stop and audit the next loaded child of
`rust/private/toolchain.bzl`; do not jump to its rule body.

Pinned Zabel `c7298478…` remains guidance only. Its one evaluator-free
build-setting descriptor co-locates String kind, flag and allow-multiple,
supporting Slug's compact producer-owned value rather than a side registry. No
Zig code, configured behavior or algorithm is adopted. The Buck2 utility audit
selects the current Copy enum and `Allocative`; no utility or ledger change is
needed. Bazel 9.2 remains sole behavior authority.

### Complete toolchain-config library accepted; compilation helper selected (2026-08-26)

Commit `acca5cb68` adds 703 proof lines and no production. It embeds and hashes
all 622 authenticated dependency-free lines, verifies the exact 27-name public
set, all 13 provider identities/export names/source labels, 14 public functions
and seven private functions. It invokes no provider/function and claims no
specific retained schema or constructor behavior. Full 251/24/31 tests, locked
checks, CLI build, formatting and hygiene pass; independent review returned
`ACCEPT`.

The smaller-looking 84-line armeabi consumer is not dependency-complete: its
common `cc_common` and `CcToolchainConfigInfo` loads return to incomplete
compatibility-proxy/private children. The compile branch now has five accepted
children before 666-line `cc/private/compile/cc_compilation_helper.bzl`
(`2c484cad…`). Run only
`WP-4-7A-rules-cc-compilation-helper-complete-loading-proof` under 0/1050/1050
caps. Prove complete bytes, imported pointer identities, exact public/private
name sets, private constant/provider, all 12 lazy functions and the one-field
captured helper struct; invoke nothing. Clean `../zabel` `0795445f…` guides
defining-module import/capture ownership and recursive freeze only; no Zig code
or behavior is adopted. Bazel 9.2 and authenticated rules_cc remain exact
authority.

### Compilation-helper proof REPLAN; complete universal environment selected (2026-08-26)

The exact 666-line helper reached line 251 before its first missing global:
lazy `_module_map_struct_to_module_map_content` contains
`added_paths = set()`. Slug resolves lazy function globals during compilation,
so the absent binding stops complete module freeze without invoking the
function. The entire +855 proof candidate was removed byte-for-byte and the
checkout returned clean at `1fb05138a`.

The broader environment audit found that loading, root/nonroot MODULE, REPO and
the live core evaluator all assemble universes independently. Bare vendored
standard globals leak non-Bazel `chr`/`ord`; REPO's separate shim always
reports set disabled even though Bazel 9.2 enables the universal constructor by
default. Run only `WP-4-5-7A-bazel-universal-builtins-environment` under
220/300/520 caps. Add a low-level exact 30-name, process-stable Rust owner;
migrate every active evaluator; keep `StructType` in the `.bzl` overlay; and
prove positional construction, type, deduplication/order, membership, `add`,
non-aliasing copy, invalid inputs and module freeze. Remaining callable behavior
stays Slug-native or deferred and the helper proof remains removed. Clean
`../zabel` `0795445f…` is a peer implementation whose immutable universe and
predeclared/module separation inform the Rust architecture; Bazel 9.2 remains
sole compatibility authority and no Zig content is adopted.

### Complete C++ semantics accepted; toolchain-config library selected (2026-08-26)

Commit `9cc0d4ace` adds 363 proof lines and no production. It embeds and hashes
all 234 authenticated semantics lines, freezes both public Booleans, the private
canonical Windows label, all 30 private functions, the exact 43-field name/value
mapping, every one of 29 captured-function identities, exact scalars and
dictionaries, and exact list contents/order. No function or
`configuration_field` is invoked. A focused correction compares the exact
field-name set without claiming Bazel's sorted schemaless struct iteration;
Slug's constructor-order iteration remains explicitly Slug-native. Focused and
full 250/24/31 tests, locked checks, CLI build, formatting and hygiene pass;
independent correction rereview returned `ACCEPT`.

The source frontier now compares 666-line `cc_compilation_helper.bzl`, whose
five children are accepted, with the dependency-free 622-line
`cc/cc_toolchain_config_lib.bzl` (`f8418490…`) on the live toolchain branch.
Select the smaller library. Run only
`WP-4-7A-rules-cc-toolchain-config-lib-complete-loading-proof` under 0/850/850
caps: prove complete bytes, all 13 public provider-callable identities and all
14 public plus seven private function types/visibility without invoking an
exported callable. Clean `../zabel` `0795445f…` guides declaration-owned
provider/function defining-module freeze only; no Zig code or behavior is
adopted. Bazel 9.2 and authenticated rules_cc remain exact authority.

### Configuration-field binding accepted; complete C++ semantics retry selected (2026-08-26)

Commit `fc131d7aa` adds 9 production and 59 proof lines. It exposes
`configuration_field` only to `.bzl` loading with Bazel's two required
positional-or-named strings, preserves BUILD absence and lazy reference freeze,
and routes two-positional, mixed, two-named and reverse-named calls to the same
Slug-native fail-closed diagnostic. Missing, duplicate, excess and wrong-type
forms reject. Focused proof, 249 loading units, 24 invalidation tests, 31
BUILD-loading tests, locked checks, CLI build and hygiene pass. Independent
review returned `ACCEPT`; no descriptor/schema/configured behavior or retained
type exists.

The exact dependency-free 234-line `cc/common/semantics.bzl` (`029254fd…`) now
resolves every global while retaining all function bodies lazily. Run only
`WP-4-7A-rules-cc-semantics-complete-loading-proof-r2` under 0/550/550 caps.
Prove complete bytes, both constants, private Windows label, all 30 lazy types,
all 29 captured pointer identities and every exact 43-field scalar/aggregate
shape. Invoke nothing. Clean `../zabel` `0795445f…` guides declaration-owned
aggregate, captured-function and recursive-freeze architecture only; Bazel 9.2
and authenticated rules_cc remain exact authority.

### Configuration-field named-only candidate REPLAN; dual ABI retry selected (2026-08-26)

The first binding implementation added 12 production and 58 proof lines, passed
all 249 loading, 24 invalidation and 31 BUILD-loading tests plus downstream
checks, and retained no value. Independent review nevertheless rejected it:
pinned Bazel's `@Param(named = true)` leaves `positional = true` by default, so
the Rust `#[starlark(require = named)]` parameters silently narrowed valid
two-positional and positional-then-named calls. The entire candidate was removed
and both Rust files match base hashes.

Run only `WP-4-7A-bazel-configuration-field-loading-binding-r2` under unchanged
20/80/100 caps. Both required strings accept positional or named binding; prove
two-positional, positional-then-named, two-named and reverse-named forms reach
the identical Slug-native fail-closed diagnostic. Preserve BUILD absence, lazy
freeze and every descriptor/configured deferral. Clean `../zabel` `0795445f…`
remains guidance only for binding/descriptor separation; Bazel 9.2 owns the ABI.

### Complete semantics proof REPLAN; configuration-field binding selected (2026-08-26)

The exact complete 234-line `cc/common/semantics.bzl` attempt stopped before
invocation because Starlark name resolution requires `configuration_field` in
lazy `_get_coverage_attrs` at line 80. Slug's loading globals do not expose it.
The worker made no production change and fully removed the uncommitted Rust
candidate; only the scheduling documents and required routing row are now dirty.
A test-local substitute or narrowed source would violate the packet.

Pinned Bazel 9.2 declares `configuration_field(fragment, name)` as a `.bzl`
top-level function with two required positional-or-named strings. Its valid result is a
late-bound label default after fragment/field validation. Slug does not yet own
that retained value or its configured resolver, and semantics never invokes the
binding during loading. Run only
`WP-4-7A-bazel-configuration-field-loading-binding-r2` under 20/80/100 caps: expose
the exact `.bzl` callable/ABI, preserve lazy reference/freeze, keep BUILD absence,
and fail every otherwise valid call closed before allocation. Defer descriptors,
attribute defaults and configured resolution, then retry complete semantics.

Clean `../zabel` `0795445f…` guides only the architectural split between its
`.bzl` predeclared binding and separately retained late-bound descriptor/resolver.
No Zig code, representation, algorithm or diagnostic is copied; Bazel 9.2
remains sole exact authority. No retained structure changes, so the Buck2
utility-reuse audit requires no skill/ledger row.

### Complete action names accepted; C++ semantics selected (2026-08-26)

Commit `9e312f958` adds 328 proof lines and no production. It embeds and
byte-verifies the complete dependency-free 220-line action-name producer and
exhaustively proves all 33 public constants, every 33-field `ACTION_NAMES`
mapping, all seven exported lists in exact order, and all seven final struct
fields pointer-identical to those frozen lists. Focused proof, 248 loading units,
24 invalidation tests, 31 BUILD-loading tests, locked checks, CLI build and
hygiene pass. Independent review returned `ACCEPT`.

Private `cc_common.bzl` remains in 2,295-line
`cc/private/compile/compile.bzl`. After complete Skylib paths, action names and
helper children, its first incomplete child is dependency-free 234-line
`cc/common/semantics.bzl` (SHA-256 `029254fd…`). Its eager rows are two public
Booleans, 30 lazy private functions, one private canonical `Label`, and a public
43-field struct capturing 29 functions plus exact scalar/list/dictionary data.
All shapes are admitted without invocation. The alternative toolchain-config
branch now reaches the dependency-free 622-line configuration library, so
semantics is the smaller source-ordered successor.

Run only `WP-4-7A-rules-cc-semantics-complete-loading-proof` under 0/550/550
caps. Prove complete source/hash, both constants, private label, all lazy types,
all captured pointer identities and every exact aggregate/scalar field. Invoke
nothing and add no production behavior. Defer compile, configuration library,
legacy features, toolchain config, private/public `cc_common`, generated proxy
and configured C++. Clean `../zabel` `0795445f…` guides declaration-owned
aggregate, captured-function and defining-module recursive-freeze architecture
only; Bazel 9.2 and authenticated rules_cc remain exact authority.

### Complete compilation outputs accepted; action names selected (2026-08-26)

Commit `63d4bda76` adds exactly 450 proof lines and no production. It embeds and
byte-verifies the complete 226-line compilation-output producer, reconstructs
the accepted helper/internal/LTO children, preserves all five imported pointers,
and proves the sentinel/output provider identities, private visibility, every
lazy binding type and exact source-owned empty output. Focused proof, 247 loading
units, 24 invalidation tests, 31 BUILD-loading tests, locked checks, CLI build
and hygiene pass. Independent review returned `ACCEPT`, including transitive
ownership of the captured helper-created closure without invoking it.

Private `cc_common.bzl` source order next enters 2,295-line
`cc/private/compile/compile.bzl`. Its first child, Skylib paths, is complete; its
first incomplete child is dependency-free 220-line `cc/action_names.bzl`
(SHA-256 `e52d1647…`). It eagerly defines 33 strings, one exact 33-field struct,
seven ordered lists and one exact seven-field struct using accepted evaluator
shapes. The alternative toolchain-config branch's 1,387-line
`legacy_features.bzl` also loads action names first, so this is the minimum
source-ordered successor shared by both branches.

Run only `WP-4-7A-rules-cc-action-names-complete-loading-proof` under
0/450/450 caps. Prove complete source/hash, every constant and struct mapping,
every list's exact order and the final struct's retained list identities. Add no
production or consumer behavior and invoke nothing. Defer compile, legacy
features, toolchain config, private/public `cc_common`, generated proxy and
configured C++. Clean `../zabel` `0795445f…` guides declaration-owned generic
aggregate and defining-module recursive-freeze architecture only; Bazel 9.2 and
authenticated rules_cc remain exact authority.

### Complete LTO context accepted; compilation outputs selected (2026-08-26)

Commit `974b9e981` adds 207 proof lines and no production. It embeds and
byte-verifies the complete 97-line LTO-context producer, rebuilds both accepted
children, preserves both imported pointers, and proves the two provider IDs,
three lazy function types and exact empty-context identity/dictionary. Focused
proof, 246 loading units, 24 invalidation tests, 31 BUILD-loading tests, locked
checks, CLI build and hygiene pass. Independent review accepts caps/boundaries.

All three children of 226-line
`cc/private/compile/cc_compilation_outputs.bzl` (SHA-256 `294e3da1…`) are now
complete. Its eager rows declare a private sentinel provider/instance and public
outputs provider, then source-invoke one empty-output constructor using only
accepted list freeze, depset, helper wrapper and empty LTO shapes. Every later
function body is lazy. Toolchain config remains the broader later proxy branch.

Run only
`WP-4-7A-rules-cc-private-compilation-outputs-complete-loading-proof` under
0/450/450 caps. Prove complete source/hash, five imported identities, provider
and sentinel identities/visibility, all lazy types, and the exact empty output's
lists, None fields, helper-owned closure and LTO identity. Manually invoke
nothing. Defer create/merge behavior, compile actions, private/public
`cc_common`, generated proxy, toolchain config and configured C++. Clean
`../zabel` `0795445f…` guides defining-module, captured-function and recursive
freeze ownership only; Bazel 9.2 remains exact authority.

### Complete shared-library hint accepted; LTO context selected (2026-08-26)

Commit `9b44f0352` adds 88 proof lines and no production. It embeds and
byte-verifies the complete dependency-free 56-line shared-library-hint producer
and proves the public provider's exact source/export identity and type without
invocation. Focused proof, 245 loading units, 24 invalidation tests, 31
BUILD-loading tests, locked checks, CLI build and hygiene pass. Independent
review accepts caps and boundaries.

Private `cc_common` source order next reaches
`cc/private/compile/cc_compilation_outputs.bzl`. Its helper and internal children
are complete; the first incomplete child is 97-line
`compile/lto_compilation_context.bzl` (SHA-256 `a17435cd…`). That producer loads
only the accepted children, declares two providers and three lazy public
functions, then eagerly constructs one empty LTO context. Toolchain config
remains the broader later generated-proxy child.

Run only
`WP-4-7A-rules-cc-private-lto-compilation-context-complete-loading-proof` under
0/220/220 caps. Prove complete source/hash, both imported identities, distinct
provider IDs, lazy binding types and exact empty-context provider/dictionary
shape; invoke no lazy binding. Defer compilation outputs, private/public
`cc_common`, generated proxy, toolchain config and configured C++. Clean
`../zabel` `0795445f…` guides defining-module ownership and recursive freeze
only; Bazel 9.2 remains exact authority.

### Complete launcher info accepted; shared-library hint selected (2026-08-26)

Commit `badf5844a` adds 80 proof lines and no production. It embeds and
byte-verifies the complete 31-line launcher-info producer, rebuilds the accepted
helper closure, preserves the imported wrapper pointer, and proves the exact
initialized provider identity plus private raw and constructor visibility/types
without invocation. Focused proof, 244 loading units, 24 invalidation tests, 31
BUILD-loading tests, locked checks, CLI build and hygiene pass. Independent
review accepts caps and boundaries.

Private `cc_common` source order next reaches dependency-free 56-line
`cc/private/cc_shared_library_hint_info.bzl` (SHA-256 `7d067aad…`). It has no
loads or lazy functions and eagerly declares only public
`CcSharedLibraryHintInfo` with its authenticated two-field schema. Toolchain
config remains a broader later generated-proxy child.

Run only
`WP-4-7A-rules-cc-private-cc-shared-library-hint-info-complete-loading-proof`
under 0/100/100 caps. Prove complete source/hash, dependency-free evaluation and
the callable's canonical provider identity/type/public visibility; invoke
nothing. Defer instances, private/public `cc_common`, generated proxy, toolchain
config and configured C++. Clean `../zabel` `0795445f…` guides defining-module
ownership/freeze only; Bazel 9.2 remains exact authority.

### Complete private CcInfo accepted; launcher info selected (2026-08-26)

Commit `07077e23d` adds 892 proof lines and no production. It embeds and
byte-verifies the complete 656-line private CcInfo producer, rebuilds all four
exact children with the actual Skylib mapping, preserves every imported pointer,
and proves all six provider identities, the initialized raw constructor, three
complete empty-context shapes and every lazy binding type. Focused proof, 243
loading units, 24 invalidation tests, 31 BUILD-loading tests, locked checks, CLI
build and hygiene pass. Independent review accepts caps and boundaries.

Re-audit returns to generated-proxy source order: private `cc_common.bzl` remains
the first incomplete root. Its helper, private CcInfo and `cc_internal` children
are now complete. The next child is 31-line
`cc/private/cc_launcher_info.bzl` (SHA-256 `41da5476…`), which loads only the
accepted helper, defines one lazy initializer and declares initialized
`CcLauncherInfo` plus private raw `_`. Toolchain config still reaches 1,387-line
legacy features, 220-line action names and the 622-line configuration library,
so it is not the minimum source-ordered successor.

Run only
`WP-4-7A-rules-cc-private-cc-launcher-info-complete-loading-proof` under
0/120/120 caps. Prove complete source/hash, recursive helper and imported
identity, exact provider source/export identity, private raw/initializer types
and visibility, and invoke nothing. Defer private/public `cc_common`, generated
proxy, launcher instances, toolchain config and configured C++. Clean `../zabel`
`0795445f…` guides defining-module ownership and recursive freeze only; no Zig
code, representation or behavior is copied. Bazel 9.2 remains exact authority.

### Complete extra-link library accepted; private CcInfo proof selected (2026-08-26)

Commit `30ec1de4f` embeds/hash-verifies the complete 192-line extra-link-library
producer, rebuilds exact helper/internal children, and retains both import
identities. It proves four pairwise-distinct provider callables, private names,
the exact `_EMPTY` exported provider ID and empty list, and lazy function types
without invocation. Growth is 0/316/316; 242 loading units, 24 invalidation
tests, 31 BUILD-loading tests, locked checks, CLI build and hygiene pass.
Independent review returns `ACCEPT`.

All four children of private `cc_info.bzl` are now complete. The 656-line parent
(`4424bb87…`) eagerly declares five ordinary providers, constructs compilation,
linking and debug empty contexts, then declares initialized `CcInfo` and its raw
constructor. Zero-argument depsets and the narrow Slug-native header-info bridge
are admitted; every other function body is lazy. Existing tests are only
source-shaped slices.

Run only `WP-4-7A-rules-cc-private-cc-info-complete-loading-proof`. Under
0/900/900 caps embed/hash the complete parent, rebuild all four frozen children,
prove import/provider/empty-context identities and frozen visibility/types, and
invoke no lazy helper. Stop before `cc_common` or proxy loading. Clean `../zabel`
`0795445f…` guides defining-module identity and recursive freeze only; no Zig
implementation or behavior is copied. Bazel 9.2 remains exact authority.

### Zero-argument depset accepted; exact ObjcInfo proxy child selected (2026-08-26)

Commit `498e5efc7` admits zero/no-name `depset()` as the existing empty frozen
value in shared BUILD and `.bzl` globals. It preserves the existing one-list
validation and order without adding a scratch allocation, rejects deferred
named zero-position forms, and leaves all broader depset semantics unchanged.
Focused proof, all 237 loading-library tests, 24 invalidation tests, 31
BUILD-loading tests, analysis/core checks and the CLI build pass. Independent
reviews accept 9/50/59 additions and the compatibility boundary.

Exact `cc/private/objc_info.bzl` can now evaluate all five eager `depset()`
defaults and freeze its initialized provider declaration. Run only
`WP-4-7A-rules-cc-compatibility-proxy-objc-info-loading-proof`. Embed the
complete 97-line child and exact generated proxy load/export slices; prove the
private initializer/raw bindings are functions, public ObjcInfo is a distinct
provider callable, and proxy `ObjcInfo` plus `new_objc_provider` both
pointer-alias only the public callable. Invoke nothing and keep omitted proxy
children plus complete public CcInfo deferred. Clean `../zabel` `0795445f…`
guides only defining-module ownership/reexport reachability; no Zig code,
representation or behavior is copied.

### Direct-provider proxy children accepted; zero-argument depset selected (2026-08-26)

Commit `0699dffe7` freezes exact complete `CcSharedLibraryInfo` and
`DebugPackageInfo` child modules and proves their provider-callable types plus
pointer-identical reexports through the narrowed generated-proxy slices. It
uses the actual `rules_cc` apparent-to-canonical mapping and leaves every
omitted proxy export absent. Focused proof, all 236 loading-library tests, 24
invalidation tests, 31 BUILD-loading tests, analysis/core checks and the CLI
build pass; independent review accepts 0/158/158 scope and compatibility
classification.

The next smallest child is exact 97-line `cc/private/objc_info.bzl`, but its
five `depset()` defaults execute when `_objcinfo_init` is defined. Slug's
loading callable currently requires one positional list, so exact full-module
freeze fails before the initialized provider is declared. Do not substitute a
provider-only slice.

Run only `WP-4-7A-bazel-zero-argument-depset-loading`. Accept zero arguments as
the existing empty frozen depset only when no names are supplied, preserve the
one-list constructor and reject all further breadth under 20 production/50
proof/70 total caps. Pinned Bazel
9.2's default-`None` signature and `DepsetTest.testEmptyGenericType` are exact
authority. Clean `../zabel` `0795445f…` guides only the architectural choice to
reuse the existing empty ownership shape; no Zig code, representation, cache,
order or behavior is copied. After acceptance, select exact complete ObjcInfo
and prove that proxy `ObjcInfo` and `new_objc_provider` both alias its public
callable rather than the private raw constructor.

### Public CcInfo audit selects direct-provider proxy children (2026-08-26)

Audit `242325974` authenticates the public -> generated proxy -> private CcInfo
route. Generated `symbols.bzl` eagerly loads six children, so accepted
source-shaped CcInfo declaration behavior cannot substitute for complete proxy
freeze. Private CcInfo also retains four children and eager contexts before its
provider publication.

Run only
`WP-4-7A-rules-cc-compatibility-proxy-direct-provider-children-loading-proof`.
Freeze exact full `cc_shared_library_info.bzl` (27 lines, `5b7dcd1f…`) and
`debug_package_info.bzl` (26, `b22666c6…`), then prove exact provider types and
pointer identities through a narrowed proxy harness under 0/160/160 caps.
Classify that harness as Slug-native and keep the omitted proxy children plus
complete public CcInfo route deferred. Architecture review accepts this bounded
prerequisite. Clean `../zabel` `0795445f…` guides definition/reexport
reachability only; Bazel 9.2 and authenticated rules sources remain exact.

### Exact compute-crate-name accepted; public CcInfo route audit selected (2026-08-26)

Commit `7d45bee02` adds 230 proof lines and verifies all five new crate-name
slices plus three accepted eager slices in exact source order. It proves the
one private and four public new bindings, accepted eager pointer retention and
parent identity without invocation. All 235 loading-lib, 24 invalidation and 31
BUILD-loading tests pass with analysis/core checks and the CLI build;
independent review accepts the closure and caps.

Only `transform_deps` and `transform_link_deps` remain. Both reach exact
`CcInfo` through the 18-line public rules_cc module, generated 15-line
compatibility proxy and 656-line private child. Accepted provider-initializer
commits prove the declaration abstraction but not this complete loaded route.
Run only `WP-4-7A-rules-cc-cc-info-public-route-frontier-audit`: authenticate
the recursive route, determine whether accepted exact children make one bounded
successor honest, and otherwise record `REPLAN`. No narrowed stub or code change
is authorized. Clean `../zabel` `0795445f…` guides closure reachability and
freeze ownership only; Bazel 9.2 and authenticated sources remain exact.

### Exact transform-sources export accepted; crate-name selected (2026-08-26)

Commit `4d037e48d` adds 152 proof lines and verifies exact `transform_sources`,
its private helper, the accepted Skylib paths child, actual repository mapping,
loaded identity and parent identity without invocation. All 234 loading-lib, 24
invalidation and 31 BUILD-loading tests pass with analysis/core checks and the
CLI build; independent review accepts the closure and caps.

Run only `WP-4-7A-rules-rust-utils-compute-crate-name-export-loading-proof`
under 0/240/240 caps. Freeze exact `compute_crate_name` and the four dependency
helper slices authenticated by audit `6381223ce`, reusing accepted eager
encoding slices in source order. Prove hashes, exact visibility and parent
identities and nonexecution. Defer both dependency transforms on their exact
CcInfo closure. Clean `../zabel` `0795445f…` guides reachability and freeze
ownership only; Bazel 9.2 and authenticated rules sources remain exact.

### Exact output-diagnostics export accepted; transform-sources selected (2026-08-26)

Commit `53c4d7d78` adds only 109 proof lines and verifies exact
`RustcOutputDiagnosticsInfo` and `generate_output_diagnostics` bytes, types,
and provider -> utils -> parent pointer identities without invocation. All 233
loading-lib, 24 invalidation and 31 BUILD-loading tests pass with analysis/core
checks and the CLI build; independent review accepts the closure and caps.

Run only `WP-4-7A-rules-rust-utils-transform-sources-export-loading-proof`.
Embed exact `utils.bzl:878-917` (SHA-256
`1006a8daf526ca60d494f691067d417db5ca34ef350bd6fcf901b8f1d5fd14c7`)
and helper 937-965 (SHA-256
`c5105f745ea0032b282f9de9825bac784ebd88ec55c80c2692017038357eaaaa`),
reusing accepted exact Skylib paths source `96cce438…`. Prove the actual
apparent Skylib load, private helper visibility and public parent identity under
0/180/180 caps; invoke nothing. Clean `../zabel` `0795445f…` guides
reachability and freeze ownership only; Bazel 9.2 and authenticated rules
sources remain exact authority.

### Exact can-build-metadata export accepted; diagnostics selected (2026-08-26)

Commit `cf76c0443` adds only 115 proof lines to the established external-Bzl
owner. It verifies exact `AlwaysEnableMetadataOutputGroupsInfo` and
`can_build_metadata` bytes, exact provider/function types, and provider ->
utils -> parent pointer identity while invoking nothing. All 232 loading-lib,
24 invalidation and 31 BUILD-loading tests pass with analysis/core checks and
the CLI build; independent review accepts the closure and caps.

Audit `6381223ce` already authenticates the next minimum closure. Run only
`WP-4-7A-rules-rust-utils-output-diagnostics-export-loading-proof`: exact
`utils.bzl:967-991` (SHA-256
`8535acbf356edec97a667da93592f211b9c0f34f5a9b88de6e0a83ac453f5bec`)
plus exact `providers.bzl:120-128` (SHA-256
`a066585ff0356b5baa65fb4ddcc3fe6d5644be4facd457bf83b5eb6886324086`)
under 0/120/120 caps. Prove only loaded and parent binding identity; do not
invoke the provider or function or admit diagnostic/action behavior. Clean
`../zabel` `0795445f…` guides reachability and freeze ownership only; Bazel 9.2
and authenticated rules sources remain exact authority.

### Post-private-helper audit selects can-build-metadata export (2026-08-26)

Audit `f3ddca46a` authenticates the six residual roots and their recursive
freeze dependencies. `compute_crate_name` needs 104 new helper lines plus the
accepted eager encoder; `transform_sources` needs 69 new local lines plus the
accepted exact Skylib paths child. The transform-dependency pair additionally
needs rules_rust providers and exact `CcInfo`; the latter still crosses the
generated compatibility proxy and broad private initialized-provider closure,
so no proof-only stub is permitted.

`can_build_metadata` and `generate_output_diagnostics` each need 34 new exact
source lines. Select the earlier parent import: `utils.bzl:742-765` (SHA-256
`4d57fbeaa3abeee124920697c17f08cd785655f3de64723f9e071bd2b50cb8eb`)
plus `providers.bzl:109-118` (SHA-256
`3c21b9e0c388512de065d30fe0910e8fc6db274e6643662fb1922ce47787db8b`),
reusing accepted exact `can_use_metadata_for_pipelining`.

Run only `WP-4-7A-rules-rust-utils-can-build-metadata-export-loading-proof`
under 0/120/120 caps. Freeze the exact provider declaration, then the selected
function with a narrowed actual `:providers.bzl` load and accepted helper;
prove loaded and parent pointer identities and invoke nothing. Clean `../zabel`
`0795445f…` guides only recursive loaded-binding retention; Bazel 9.2 and the
authenticated rules sources remain exact authority.

### Exact utils crate-root export accepted; loaded frontier audit selected (2026-08-26)

Commit `cdd2f68f7` freezes the second exact private-helper utils closure,
retains the helper's hidden function visibility, and proves pointer-identical
public import through the proof-only exact parent. Neither body is invoked. The
+107 proof/0 production change ends at 8,858; focused proof, 231 loading units,
24 invalidation tests, 31 BUILD-loading tests, dependent checks, rebuilt CLI
and hygiene pass. Independent review returned `ACCEPT`.

The remaining six parent imports are `can_build_metadata`,
`compute_crate_name`, `generate_output_diagnostics`, `transform_deps`,
`transform_link_deps`, and `transform_sources`. Each now reaches loaded
providers, accepted eager composites, bazel_skylib paths or the large crate-
name helper closure. Run only
`WP-4-7A-post-utils-private-helper-loaded-frontier-audit`: authenticate every
local and loaded source dependency, distinguish already accepted bindings from
missing ones, and select exactly one smallest coherent proof successor. Change
only the three scheduling documents; do not add Rust, fixtures or oracle rows.

Clean `../zabel` `0795445f…` guides only recursively reachable defining-module
and loaded-binding retention. No Zig code, representation, traversal/order
algorithm, diagnostic, identity or behavior is copied; pinned Bazel 9.2
resolver tests and authenticated rules_rust source remain sole exact authority.

### Exact utils expand-dict export accepted; crate-root export selected (2026-08-26)

Commit `216b83ac0` freezes the first dependency-bearing utils export with its
one private helper, retains the helper's hidden function visibility, and proves
the public binding is pointer-identical through the proof-only exact parent.
Neither body is invoked. The +145 proof/0 production change ends at 8,751;
focused proof, 230 loading units, 24 invalidation tests, 31 BUILD-loading tests,
dependent checks, rebuilt CLI and hygiene pass. Independent correction review
returned `ACCEPT`.

Seven dependency-bearing imports remain. The smallest closure without a loaded
provider, accepted eager composite or bazel_skylib binding is exact
`utils.bzl:788-816` public `crate_root_src` (29 lines, SHA-256
`f5a21bb9e1f694a1baec8c238bb52f4eb70f7ec25014f6d0cf71b09e2670ee41`)
plus `:818-833` private `_shortest_src_with_basename` (16 lines, SHA-256
`7157302d387837bc1d83c2aae3caed49c2cd76a074d58d9d4b6fdc3d6f5f7bdc`).

Run only `WP-4-7A-rules-rust-utils-crate-root-export-loading-proof` under
0/130/130 caps in the existing proof owner. Freeze both slices in exact source
order under the utils producer, retain the public/private visibility boundary,
and prove the public export's pointer-identical proof-only parent import with
actual `:utils.bzl` spelling. Invoke neither function. Results, diagnostics,
configured behavior, the other six dependency-bearing exports, whole-utils
freeze and parent body remain deferred.

Clean `../zabel` `0795445f…` guides only recursive defining-module helper
retention. No Zig code, representation, traversal/order algorithm, diagnostic,
identity or behavior is copied; pinned Bazel 9.2 resolver tests and the
authenticated rules_rust source remain sole exact authority.

### Exact utils leaf exports accepted; expand-dict export selected (2026-08-26)

Commit `13ebf0a14` freezes all six remaining helper-free utils functions needed
by exact `rust.bzl`, preserves their actual parent import order, and proves
pointer-identical proof-only parent bindings without invocation. The +191
proof/0 production change ends at 8,606; focused proof, 229 loading units, 24
invalidation tests, 31 BUILD-loading tests, dependent checks, rebuilt CLI and
hygiene pass. Independent review returned `ACCEPT`.

Eight dependency-bearing imports remain. Source order first reaches
`expand_dict_value_locations`. Its source-complete closure is only exact
`utils.bzl:268-313` `_expand_location_for_build_script_runner` (46 lines,
SHA-256 `73cd67a0bf9e2b370f7d287cefe1fa73efa20552a8f99f7cdb45ecf14c24d64d`)
and `:315-348` `expand_dict_value_locations` (34 lines, SHA-256
`0c8ce89317f00a453998d33aa2236824bff20eb6cdb0092dc5077604033e10bd`).
The helper references only predeclared values and standard methods; no loaded
provider, eager composite or other same-module definition enters the closure.

Run only `WP-4-7A-rules-rust-utils-expand-dict-export-loading-proof` under
0/180/180 caps in the existing proof owner. Freeze the two exact slices under
the utils producer, prove both function bindings, import only the public export
with actual `:utils.bzl` spelling in the proof-only exact parent, prove pointer
identity and invoke neither function. Results, diagnostics, configured
behavior, the other seven dependency-bearing exports, whole-utils freeze and
the parent body remain deferred.

Clean `../zabel` `0795445f…` guides only recursive defining-module helper
retention. No Zig code, representation, traversal/order algorithm, diagnostic,
identity or behavior is copied; pinned Bazel 9.2 resolver tests and the
authenticated rules_rust source remain sole exact authority.

### Exact utils find-toolchain export accepted; leaf family selected (2026-08-26)

Commit `d3cb959f6` freezes exact `utils.bzl:61-70` `find_toolchain` under
the utils producer and proves pointer-identical import through a proof-only
exact-parent module using actual `:utils.bzl` spelling. It adds +53 proof and 0
production lines, ends at 8,415, and passes focused proof, 228 loading units, 24
invalidation tests, 31 BUILD-loading tests, dependents, CLI and hygiene.

Run only `WP-4-7A-rules-rust-utils-leaf-exports-loading-proof` for the six
remaining helper-free parent imports: `determine_output_hash`, `deduplicate`,
`dedent`, `can_use_metadata_for_pipelining`, `determine_lib_name`, and
`get_edition`. Their separately authenticated exact slices total 128 lines.
Freeze them under the utils producer and prove ordered pointer-identical imports
through an exact-parent proof module under 0/250/250 caps. Invoke no function;
the eight helper/provider/path-bearing exports and parent body remain deferred.

Pinned Bazel resolver tests remain exact authority. Clean `../zabel`
`0795445f…` guides only reachable defining-module function retention; no Zig
code, representation, algorithm, diagnostic, identity or behavior is copied.

### Post-utils parent-import audit selects exact find-toolchain export (2026-08-26)

Audit `d4e264cdc` maps the fifteen exact `rust.bzl:40-57` imports across utils.
Seven are leaf functions over predeclared globals or field/string operations;
eight require same-module helpers, accepted eager composites, loaded providers
or bazel_skylib paths. The earliest parent-needed definition is
`find_toolchain` at exact utils lines 61-70, SHA-256
`75fe3e764290fcfcec78cc25d25b4d2486708dafabb112f5d1e44b8e21081be1`,
with only the admitted `Label` predeclared dependency.

Run only `WP-4-7A-rules-rust-find-toolchain-export-loading-proof` in the
existing loading test owner under 0/120/120 caps. Freeze the ten exact source
lines under the utils producer, then load only that export using actual
`:utils.bzl` spelling in a proof-only exact-parent consumer and prove frozen
pointer identity. Invoke neither function nor `Label`; do not claim configured
toolchain lookup, another export, whole utils freeze or parent source.

Pinned Bazel resolver tests remain exact authority. Clean `../zabel`
`0795445f…` guides only defining-module function reachability; no Zig code,
representation, algorithm, diagnostic, identity or behavior is copied.

### Exact utils eager values accepted; parent-import frontier audit selected (2026-08-26)

Commit `adde01290` freezes five exact rules_rust 0.73.0 `utils.bzl` slices
totaling 124 source lines under exact producer
`@@rules_rust+//rust/private:utils.bzl`. It proves the ordered six unsupported
features, false C++ kill switch, all 63 derived substitution pairs, the public
list alias, and the public encode function alias without invoking a helper.
Lines 692-740 close only `_encode_raw_string`'s compiler/freeze dependency on
`_replace_all`; all utility results and diagnostics remain deferred.

The +202 proof/0 production change ends at 8,362 lines. Focused proof, all 227
loading units, 24 invalidation tests, 31 BUILD-loading tests, direct dependents,
CLI build and hygiene pass; independent review returned `ACCEPT`.

The exact parent `rust.bzl`, 1,821 lines at SHA-256
`a645bd5db6344bd3c0997dcf73600475c0af53fb4dd025890be24b8e1e2dbfd8`,
imports fifteen functions from utils at lines 40-57, slice SHA-256
`1ad3406b7c58cc7d74e1e86991fdb6aeadbd836d32926fc54eee9583295ab500`.
Those exports and their transitive compiler/freeze dependencies are not yet
admitted. Run only the bounded docs audit
`WP-4-7A-post-utils-eager-values-parent-import-frontier-audit`; do not return to
parent source or invoke a utility until it selects an exact proof or `REPLAN`.

Clean `../zabel` `0795445f…` remains architecture guidance only for recursively
freezing reachable defining-module values. Bazel 9.2 remains sole behavior
authority; no Zig implementation or behavior is copied.

### Post-find-toolchain audit selects bounded utils eager-values proof (2026-08-26)

Exact `rust/private/utils.bzl` returns from the find-toolchain child through
already-admitted rules_cc `cc_common`, rules_cc `CcInfo` and rules_rust
providers. The authenticated 1,032-line rules_rust 0.73.0 source hashes to
`8aa49b9312d4ae5c4aed033aba65392a039a681b3ee21ca83da0f05acac28ace`.
No unsupported eager loading expression exists.

The remaining eager values are the six-string `UNSUPPORTED_FEATURES` list,
private false kill switch, 31-pair encoding tuple, derived ordered 63-pair
substitution list, its public alias and the public alias of lazy
`_encode_raw_string`. Every other top-level declaration is a lazy function.
Bazel's pinned Starlark loop/comprehension regressions establish tuple binding,
nested clause and list-result order.

Run only `WP-4-7A-rules-rust-utils-eager-values-loading-proof` in the existing
loading test file under 0/250/250 caps. Embed five exact source slices totaling
124 lines rather than the full module, hash each slice, and prove the exact
ordered list/pairs, false capture, alias identities and frozen function type.
Lines 692-740 are included solely to close `_encode_raw_string`'s lazy compiler/
freeze dependency on `_replace_all`; invoke neither function and stop before
later utils/allocator/parent behavior.

Exact compatibility covers the five exact slice bytes and ordered eager values/
aliases under the exact producer. Proof-only private projections and frozen
Rust storage are Slug-native. `_replace_all` invocation/results, full-source
freeze, utility results/diagnostics, configured toolchain/allocator semantics
and later source remain deferred.

Clean `../zabel` `0795445f…` guides only recursive freeze reachability for
composites and aliases. No Zig code, representation, owner pointer, ordering
algorithm, diagnostic, identity or behavior is copied. Bazel 9.2 remains sole
authority; the retained utility review selects no action.

### Exact rules_cc find-toolchain child accepted; utils audit selected (2026-08-26)

Commit `ee9ef5254` freezes exact rules_cc 0.2.17
`cc/find_cc_toolchain.bzl` under producer
`@@rules_cc+//cc:find_cc_toolchain.bzl` and exact cached child
`@@rules_cc+//cc/common:cc_common.bzl`. The authenticated 131-line source hashes
to `3f62d3ea99f59674f71dbc669c80dd0dc5ef14637933d727b74f0bd556334655`.
Its five source-defined names retain dict/Label/function types, while the
canonical toolchain Label and singleton `_cc_toolchain` Label/default survive a
proof-only rule consumer. No helper or implementation executes.

All 226 loading units, both 24/31 integration suites, dependent core checks,
rebuilt CLI and hygiene pass. Growth is 225 proof-only lines, ending at 8,160
below the 8,235 ceiling. Independent review caught the initially mispackaged
cached-child label; the corrected exact identity reran focused/full proof and
received `ACCEPT`.

Run only docs packet `WP-4-7A-post-find-cc-toolchain-utils-frontier-audit`.
Resume exact `rust/private/utils.bzl` after find-toolchain returns, account for
its cached rules_cc and providers children, classify all remaining eager values
and select one bounded next packet or `REPLAN`. Edit no Rust and invoke no
utility/toolchain helper.

Exact compatibility covers source/identity freeze, five source-defined export
types and eager canonical label/declaration facts. Frozen Rust representation
and the proof consumer are Slug-native. Function execution, configured/legacy
toolchain behavior, exact display text and later parent bodies remain deferred.

Clean `../zabel` `0795445f…` guided only closure/declaration-dictionary
reachability through module freeze. No Zig code, representation, owner pointer,
ordering, capture algorithm, diagnostic, identity or behavior was copied.
Bazel 9.2 remains sole authority; no retained utility or accounting changed.

### Post-paths audit selects exact rules_cc find-toolchain proof (2026-08-26)

Exact `rust/private/rust.bzl` returns from paths through already-admitted
bazel_skylib common settings, rules_cc CcInfo and rules_rust common/providers.
Its first new direct child is `rust/private/rust_allocator_libraries.bzl`, 302
lines at SHA-256
`ae4acb50ac6a1b922254a07346d97b4649810d33836f2be4824fd0b7a81e536e`.
After cached rules_cc children it enters new `rust/private/utils.bzl`, 1,032
lines at SHA-256
`8aa49b9312d4ae5c4aed033aba65392a039a681b3ee21ca83da0f05acac28ace`.

Utils first passes cached bazel_skylib paths and reaches rules_cc 0.2.17
`cc/find_cc_toolchain.bzl`, 131 lines at SHA-256
`3f62d3ea99f59674f71dbc669c80dd0dc5ef14637933d727b74f0bd556334655`.
Its only child is the admitted `cc_common` wrapper. The eager body defines the
canonical C++ toolchain type Label, one label attribute/default in
`CC_TOOLCHAIN_ATTRS`, and three lazy functions, all on admitted generic shapes.

Run only `WP-4-7A-rules-cc-find-cc-toolchain-loading-proof`, changing the one
loading test file under 0/300/300 caps. Freeze exact source under the exact
rules_cc producer/child identities and prove the source-defined name/type set,
canonical toolchain label, singleton declaration map and canonical label default through
a proof-only consumer. Invoke no toolchain helper and stop when this child
returns.

Exact compatibility covers exact-source freeze, identities, exports and eager
label/declaration facts. Frozen Rust representation and proof projection are
Slug-native. Helper execution, configured toolchain selection, exact display
text and later utils/allocator/parent bodies remain deferred.

Clean `../zabel` `0795445f…` guides only frozen reachability of exported closures
and the declaration dictionary. No Zig code, representation, owner pointer,
ordering, capture algorithm, diagnostic, identity or behavior is copied. Bazel
9.2 remains sole authority; the utility review selects no retained change.

### Exact bazel_skylib paths child accepted; parent audit selected (2026-08-26)

Commit `8440742f7` freezes exact unabridged bazel_skylib 1.8.2
`lib/paths.bzl` under producer `@@bazel_skylib+//lib:paths.bzl`. Its authenticated
SHA-256 is
`96cce43871d8228126a12ceff771351f9030b1e9d029f2185853aa6541766a83`.
The exported `paths` struct retains the exact ten source-bound members as frozen
function values. Proof sorts only names for set comparison, invokes no helper
and claims no exact field iteration order.

All 225 loading units, both 24/31 integration suites, dependent core checks,
rebuilt CLI and hygiene pass. Growth is 361 proof-only lines, ending at 7,935
below the 7,994 ceiling. Independent terminal review returned `ACCEPT`.

Run only docs packet `WP-4-7A-post-paths-rust-parent-frontier-audit`. Return to
exact `rust/private/rust.bzl` source order after paths. Account for the already-
admitted `@bazel_skylib//rules:common_settings.bzl` and all subsequent cached
children before authenticating the first newly evaluated module and selecting
its first unsupported eager loading expression or `REPLAN`. Edit no Rust and do
not enter configured semantics.

Exact compatibility covers exact-source freeze, producer identity and the ten
name-to-function bindings. Generic frozen Rust representation and sorted proof
comparison are Slug-native. Exact struct iteration order and path-helper
behavior remain deferred.

Clean `../zabel` `0795445f…` guided only closure reachability through an exported
composite after module freeze. No Zig code, representation, ordering, owner
pointer, capture algorithm, diagnostic or behavior was copied. Bazel 9.2
remains sole behavior authority, with no retained utility or accounting change.

### Post-lints audit selects exact bazel_skylib paths proof (2026-08-26)

Exact `rust/defs.bzl` next reaches `rust/private/rust.bzl` at SHA-256
`a645bd5db6344bd3c0997dcf73600475c0af53fb4dd025890be24b8e1e2dbfd8`.
Its first direct child is new bazel_skylib 1.8.2 `lib/paths.bzl`, 320 lines at
SHA-256
`96cce43871d8228126a12ceff771351f9030b1e9d029f2185853aa6541766a83`.
The child has no loads. Ten functions are declared lazily; four integer
constants and the final `paths = struct(...)` are the only other eager values.
The existing Slug parser, standard globals, struct and recursive frozen-value
owner cover every loading shape.

Run only proof packet `WP-4-7A-bazel-skylib-paths-loading-proof`, changing one
loading test file under 0/420/420 caps. Embed the exact source and prove its
hash, producer identity, exact ten-member name set and frozen function value for
each binding. Invoke no path helper and stop when the child returns.

Exact compatibility covers source freeze and exported member/function binding.
Generic frozen Rust storage and current constructor-order iteration are
Slug-native. Pinned Bazel 9.2 sorts schemaless struct keys, so field iteration
order is not exact; path helper behavior and the parent frontier remain
unsupported/deferred.

Clean `../zabel` `0795445f…` guides only closure-graph reachability from an
exported composite through module freeze. No Zig code, representation, field
ordering, owner pointer, capture algorithm, diagnostic or behavior is copied.
Bazel remains authoritative and no retained utility or accounting changes.

### Exact lints child accepted; next parent audit selected (2026-08-26)

Commit `227257a90` freezes exact unabridged rules_rust 0.73.0
`rust/private/lints.bzl` with its provider child. The authenticated source hash,
pointer-identical `LintsInfo`, rule export, ordered dictionary kinds,
nonmandatory/configurable policy and omitted declaration defaults are directly
proved. The exact source binding freezes without executing the helper or
constructing the provider.

All 224 loading units, both 24/31 integration suites, dependent core checks,
rebuilt CLI and hygiene pass at 180 proof-only additions and 7,574 final lines.
Independent terminal review returned `ACCEPT`.

Run only docs packet `WP-4-7A-post-lints-parent-frontier-audit`. Resume exact
`rust/defs.bzl` direct-load order after lints returns, account for cached
children, authenticate the next newly evaluated child and classify its first
unsupported loading expression. Edit no Rust and do not enter configured
provider/rule/action semantics.

Exact compatibility covers recursive lints source freeze and provider/export/
ordered schema identity. Existing frozen Rust storage/probes are Slug-native.
Helper/rule execution, provider construction and configured dictionaries remain
unsupported/deferred.

Clean `../zabel` `0795445f…` guided only producer-owned provider identity and
declaration-owned attribute order. No Zig code, representation, owner pointer,
capture, algorithm, diagnostic or behavior was copied. Bazel 9.2 remains sole
authority, with no retained utility or accounting change.

### Post-clippy parent audit selects exact lints proof (2026-08-26)

Exact `rust/defs.bzl` loads toolchain, clippy, common and lints in source order.
Clippy already completed common and providers, making
`rust/private/lints.bzl` the first newly evaluated child. Its SHA-256 is
`0c6dcf615bb9f43d57c4056253f89a9f1bed0b16b9e17d8eed64da85d1b05677`;
the cached provider child remains
`57a59ec9a60b9709df197333c94bac464b572af63bc78f560ce32570b6d84ac6`.

The lint implementation body and `LintsInfo(...)` construction are lazy. The
only eager declaration is a documented rule with ordered `rustc`,
`rustc_check_cfg`, `clippy`, `rustdoc` attributes of kinds StringDict,
StringListDict, StringDict and StringDict. All omit defaults and use existing
`None` declaration defaults; typed empty dictionaries arise only in the
already-accepted later invocation projection. Slug already owns these
constructors, docs and frozen schema projection. Pinned Bazel 9.2's
`StarlarkAttrModule` and rule-class tests confirm the constructor kinds and
dictionary defaults.

Run only proof packet `WP-4-7A-lints-child-loading-proof`, changing
`host_package_load_tests.rs` under 0/220/220 caps. Freeze the exact unabridged
98-line child with the accepted loaded-child helper; prove exact LintsInfo
producer identity, exact implementation source binding, rule export identity,
ordered schema kinds and omitted (`None`) declaration defaults, and helper
nonexecution. Stop when lints returns.

Exact compatibility covers recursive source freeze and producer/order/schema
identity. Existing frozen Rust storage and proof probes are Slug-native.
Configured dictionary values, rule/helper execution, provider construction and
configured action behavior remain unsupported/deferred.

Clean `../zabel` `0795445f…` guides only producer-owned imported provider
identity and declaration-owned attribute order. No Zig code, representation,
owner pointer, capture, algorithm, diagnostic or behavior is copied. Bazel 9.2
remains authoritative. This proof changes no retained utility or accounting.

### Imported frozen lint descriptors accepted; parent audit selected (2026-08-26)

Commit `db51996b9` accepts the exact imported plain `LINT_TEST_COMMON_ATTRS`
descriptors and imported frozen `platform_transition` without adding a second
retained representation. Complete `clippy.bzl:463-596` now freezes with its
lint/provider/common children. The proof discriminates ordered kinds/defaults,
producer-context canonical labels, provider/aspect identities, transition
implementation/output, test capability and Boolean settings. Rich imported
descriptors remain fail-closed.

All 223 loading units, both 24/31 integration suites, dependent core checks,
rebuilt CLI and hygiene pass within 39 production and 259 proof additions.
Independent terminal review returned `ACCEPT`.

Run only docs packet `WP-4-7A-post-clippy-parent-frontier-audit`. Resume exact
`rust/defs.bzl` direct-load order after clippy returns, account for cached
children, authenticate the next newly evaluated child and classify its first
unsupported loading expression. Edit no Rust and do not enter configured
semantics.

Exact compatibility covers imported plain descriptor validity/fields,
imported transition implementation/output and complete clippy-tail freeze.
Generic Rust wrapper reconstruction and rich-import rejection are Slug-native.
Transition execution, identity bytes and configured provider/aspect/test/
build-setting/action semantics remain unsupported/deferred.

Clean `../zabel` `0795445f…` guided only declaration producer ownership across
freeze. No Zig code, representation, pointer, capture, algorithm, diagnostic or
behavior was copied. Bazel 9.2 remains authoritative, and the existing compact
values require no new Buck2-derived utility or memory-ledger entry.

### Imported-transition correction exposes frozen common attributes (2026-08-26)

The exact tail candidate first cleared imported `platform_transition`, then
stopped at `rule(attrs=...)` because `platform` is the first plain frozen
descriptor in `LINT_TEST_COMMON_ATTRS`. Slug's rule adapter discarded the
frozen half of `AttributeDefinition::from_value`. The 9-production/248-proof
candidate was completely removed and both accepted hashes restored.

Run only `WP-4-7A-imported-frozen-attribute-transition-clippy-tail-loading`.
Project the existing frozen transition fields and only plain frozen attribute
fields into their current transient wrappers. Rich frozen attributes remain
fail-closed. Rerun the exact clippy tail and producer-identity proof under
55/260/315 caps; add no new retained representation or configured semantics.

Exact compatibility covers imported plain attribute validity/fields and
imported transition implementation/output. Rust wrapper reconstruction and the
rich frozen boundary are Slug-native. Identity bytes, evaluation/configuration
hashing and configured provider/aspect/test/build-setting/action behavior remain
unsupported/deferred.

Clean `../zabel` `0795445f…` guides producer-owned attribute/transition
publication only. No Zig owner pointer, representation, identity, capture,
algorithm or behavior is copied. Bazel 9.2 remains sole authority; existing
Arc/CompactString/default values need no new utility or memory-ledger entry.

### Clippy-tail proof exposes imported frozen-transition prerequisite (2026-08-26)

The exact recursive proof selected by `f4cfaacb3` stopped at
`clippy.bzl:502`: the label-list `cfg` receives the valid frozen
`platform_transition` exported by exact `lint_test.bzl:37-41`, but Slug's
attribute converter accepts only the transient half of the Starlark transition
value union. The rejected 246-line test candidate was removed and the file's
accepted SHA restored.

Run only `WP-4-7A-imported-frozen-transition-clippy-tail-loading`. Project the
frozen implementation/output into the existing transient generic wrapper, then
rerun the complete exact-tail producer proof. Prove the final package-schema
transition implementation is pointer-identical to the lint child's exported
implementation and retains the exact output. Change only `package.rs` and the
loading test under 20/260/280 caps. Do not add identity, registry, DICE,
transition execution or configured semantics.

Exact compatibility covers valid imported transition acceptance and retained
implementation/output. The Rust wrapper reconstruction is Slug-native.
Observable identity bytes, evaluation/configuration hashing and all configured
provider/aspect/test/build-setting/action behavior remain unsupported/deferred.

Clean `../zabel` `0795445f…` guides only producer-owned transition publication
and definition-module identity before detached capture. No Zig representation,
identity, ordinal, capture, algorithm or behavior is copied. Bazel 9.2 remains
sole authority; existing CompactString/value storage requires no utility or
memory-ledger change.

### Post-RunEnvironmentInfo clippy-tail audit accepts proof-only closure (2026-08-26)

The source-order audit authenticates complete rules_rust 0.73.0
`clippy.bzl:463-596` and its `lint_test.bzl`, `providers.bzl` and `common.bzl`
producers. No new production terminal exists: the documented test provider,
ordered output-group list, required/advertised aspect, child-owned attribute
merge, label-list provider/aspect/transition schema, test rule and two true
Boolean build settings all reuse admitted loading contracts. The two provider
constructions are confined to lazy helper bodies.

Run only proof packet `WP-4-7A-clippy-test-tail-loading-proof`. In the existing
loading test file, combine the accepted clippy prefix with exact unabridged
tail source and recursively loaded lint/provider/common exports. Prove
pointer-identical imports and every retained aspect/rule/build-setting edge;
change no production code. Caps are 0 production, 260 proof and 260 total.

Exact compatibility covers the authenticated source-order freeze and producer
identities. Existing Rust frozen/Arc ownership and fail-closed diagnostics are
Slug-native. Helper execution and configured provider matching, aspect
application, transition, test runner/actions/runfiles, build-setting values and
CLI flags remain unsupported/deferred.

Clean `../zabel` `0795445f…` guides producer-module/export-name provider
identity, declaration-owned rules and detached build-setting descriptors only.
No Zig code, representation, capture, configured behavior, algorithm or
diagnostic is copied. Bazel 9.2 remains sole behavior authority.

### RunEnvironmentInfo declaration and exact lint-test child accepted; clippy-tail audit selected (2026-08-26)

Commit `45b479e56` adds a dedicated zero-state `RunEnvironmentInfo` token only
to complete `.bzl` globals. It renders exactly as
`<function RunEnvironmentInfo>`, is distinct from the existing
`OutputGroupInfo` token and user providers, remains absent from BUILD globals,
freezes without evaluator state, and rejects every invocation before producing
a value. Constructor values, fields, equality/hash and configured
environment/test behavior remain deferred.

An exact unabridged 159-line `rust/private/lint_test.bzl` child with SHA-256
`4f4fade9218980db0296f99e5d199059c91ebebc7b9745bee18ad58c37b551c8`
now compiles and freezes. A parent using exact `clippy.bzl:19-25` loads proves
all four imports are pointer-identical to their child exports, while neither
helper nor any native-provider constructor executes. All 222 loading units, 24
invalidation tests, 31 BUILD-loading tests, locked dependent checks, rebuilt
CLI and hygiene pass. Growth is 28 production and 217 proof additions, 245
total, within caps; independent terminal review returned `ACCEPT`.

Run only docs audit `WP-4-7A-post-run-environment-info-clippy-tail-audit`.
Authenticate `clippy.bzl:463-596` and every imported provider/helper identity
in source order, then select one bounded exact loading closure or `REPLAN`.
Helper execution and configured provider/aspect/transition/test, build-setting
and action behavior remain unsupported/deferred.

Clean `../zabel` `0795445f…` guided only the distinct builtin-provider ID and
declaration-owned loading-binding architecture. No Zig code, representation,
provider value, constructor, configured lowering, diagnostic or behavior was
copied. Bazel 9.2 remains sole behavior authority.

### Post-rust_clippy source audit selects RunEnvironmentInfo global (2026-08-26)

The selected rules_rust 0.73.0 source replay reaches
`clippy.bzl:19-25`'s `//rust/private:lint_test.bzl` child before any remaining
clippy-local declaration. That defining module has no child loads. Its exact
SHA-256 is
`4f4fade9218980db0296f99e5d199059c91ebebc7b9745bee18ad58c37b551c8`.
The accepted rustfmt proof retained the exact common attribute/transition
shapes but deliberately replaced both imported helper bodies; it therefore did
not prove the real child compilation.

Starlark's module scope resolver checks every function-body name during
compilation. In the exact `lint_test_aspect_impl`, `OutputGroupInfo` and
`depset` now resolve at lines 82-100. In `lint_test_rule_impl`, `DefaultInfo`
and `depset` resolve first; Slug's next absent global is
`RunEnvironmentInfo` at line 154. The following `OutputGroupInfo` call at line
158 is already name-resolvable. No helper or provider constructor needs to run
to expose this stop.

Pinned Bazel 9.2 `StarlarkGlobalsImpl.getFixedBzlToplevels:91-107` installs
`RunEnvironmentInfo.PROVIDER` only in fixed `.bzl` globals. Its
`RunEnvironmentInfoProvider` is a distinct `BuiltinProvider`; the common
`BuiltinProvider` contract provides class-owned identity and exact
`<function RunEnvironmentInfo>` representation. The constructor's environment
map, inherited environment, provider instance and configured executable/test
behavior are later semantics, not part of declaration loading.

Run only `WP-4-7A-run-environment-info-declaration-global-loading`. Add one
dedicated zero-state fail-closed token beside the existing `OutputGroupInfo`
token, install it only in complete `.bzl` globals, and recursively load an
unabridged exact `lint_test.bzl` child through line 159. Prove BUILD absence,
exact representation, distinct native/user/OutputGroupInfo types and helper
nonexecution. Stop before returning to `clippy.bzl:463`; re-audit that tail
after the defining child is genuinely complete.

Exact compatibility covers the fixed global placement, representation and
source-order helper freeze. The distinct zero-sized Rust type and fail-closed
invocation are Slug-native. Constructor calls, equality/hash, provider values,
fields, environment semantics, `testing.TestEnvironment`, helper execution,
runfiles/actions/output groups, configured providers/aspects/transitions and
the clippy tail remain unsupported/deferred.

Clean `../zabel` `0795445f…` is architecture guidance only. Its separate
`BuiltinProviderId.run_environment_info`, native/starlark identity union,
provider-definition owner and loading binding support the same identity/phase
split. No Zig code, discriminant, layout, constructor, value, configured
lowering, diagnostic or behavior is copied. Bazel 9.2 remains sole authority.
The Buck2 utility review reuses the zero-state `Allocative` simple-value
pattern; no retained collection, interner, cache, clone path, hash owner or
Stage 9 ledger entry is needed.

### OutputGroupInfo declaration and rust_clippy accepted; tail audit selected (2026-08-26)

Commit `993ba5e4` adds a zero-state native-provider declaration token with the
exact `<function OutputGroupInfo>` representation only to `.bzl` globals. It
freezes without evaluator state and rejects every constructor call. Its Rust
type supplies only Slug-native internal separation; observable equality/hash,
provider values, fields, indexing, artifacts and configured consumers remain
deferred.

The exact clippy helper captures that token without executing. The following
`rust_clippy` rule freezes through line 461 with one `deps` label-list, two
ordered provider alternatives and the exact same frozen
`rust_clippy_aspect` producer value. All 221 loading units, 24 invalidation
tests, 31 BUILD-loading tests, locked checks, rebuilt CLI and hygiene pass;
independent terminal review returned `ACCEPT`. Growth is 28 production and 124
proof additions within caps.

The remaining source begins with documented two-field `RustClippyTestInfo`, a
two-string output-group list, lazy aspect/rule helpers, a test aspect requiring
the accepted clippy aspect, a `rust_clippy_test` declaration using imported
`LINT_TEST_COMMON_ATTRS` plus `platform_transition`, and two Boolean
build-setting rules. Most shapes resemble accepted provider, rustfmt-test and
config-bool slices, but the complete imported producer graph and first actual
terminal have not been proved.

Run only docs packet `WP-4-7A-post-rust-clippy-source-audit`. Authenticate the
remaining line 463-596 source in order, all imported lint-test identities and
the existing Slug owners. Select one bounded exact loading closure or `REPLAN`;
do not edit Rust, invoke helpers, construct OutputGroupInfo, or claim configured
test/aspect/action behavior during the audit.

Clean `../zabel` `0795445f…` remains architecture guidance only. Its separate
provider definition/value and declaration-owned attribute/aspect/rule shapes
may guide owner reuse, but no Zig code, layout, provider value, configured
capture, action or behavior is copied. Bazel 9.2 remains sole authority.

### OutputGroupInfo global audit accepts bounded loading (2026-08-26)

Commit `fc9473b1` replaces the aspect's singular label with the same immutable
typed requirement slice used by rules. One shared evaluator-aware parser
retains distinct String, Label and typed entries in source order, resolves
defining-module labels and makes mandatory state structural. The real clippy
aspect now freezes its mandatory Rust and optional C++ rows. Existing rule
semantics and duplicate rejection remain unchanged; Bazel duplicate
strictest-wins normalization is still explicitly deferred.

Focused aspect, clippy, rustfmt and rule/config-common proofs pass beside all
220 loading units, 24 invalidation tests and 31 BUILD-loading tests. Locked
analysis/core checks, rebuilt CLI, formatting and diff hygiene pass;
archive-status reports only its three known archive-only paths. The packet
adds 31 production and 90 proof lines, and independent terminal review returned
`ACCEPT`.

The next source expression is `_rust_clippy_rule_impl`. Although its body is
lazy at loading time, Starlark compilation resolves its global names. The
independently accepted proof-only `WP-4-7A-clippy-rule-loading` candidate
therefore exposed `OutputGroupInfo` as the first missing name before the
following rule declaration could freeze. Its partial test change was fully
reverted and no production code was changed.

Bazel 9.2 `StarlarkGlobalsImpl.getFixedBzlToplevels` installs
`OutputGroupInfo.STARLARK_CONSTRUCTOR` in `.bzl` globals, not fixed BUILD
globals. `OutputGroupInfoProvider` is a named native `BuiltinProvider`; its
constructor accepts named groups and converts their values to artifact nested
sets. The clippy helper only captures the provider global as a later indexing
key: no constructor, group, artifact or configured target is evaluated while
the module freezes.

Pinned `BuiltinProvider.equals` and `BuiltinProvider.Key.equals` compare the
concrete native provider class. Slug's `ProviderId` instead owns a user
provider's defining module label and exported name, and its
`AnalysisBuiltinCallable` is a generic callable rather than a provider identity.
Neither is a truthful owner for the fixed declaration token.

Run only `WP-4-7A-output-group-info-declaration-global-loading`. Add one
zero-state, evaluator-free `OutputGroupInfo` native-provider callable in the
loading provider module and install it only in complete `.bzl` globals. Its
distinct Rust Starlark value type provides only Slug-native internal separation;
observable Bazel provider equality/hashability remains deferred. Its display
matches `<function OutputGroupInfo>`, and every invocation fails closed because
the selected source only captures it in a lazy body. Then extend the exact
clippy source proof through the helper and `rust_clippy` declaration, stopping
before `RustClippyTestInfo`. Do not construct empty or named output groups or
admit fields, artifacts, configured lookup, merge or selection.

Clean `../zabel` `0795445f…` supplies guidance only. Its process-stable
`BuiltinProviderId.output_group_info` distinguishes native provider identity
from module/export-owned user providers and later configured values. Slug may
adopt that phase/identity separation in Rust, but copies no Zig code,
discriminant, layout, constructor, configured capture or behavior. Bazel 9.2
remains sole authority. The Buck2 utility review selects the zero-state
`Allocative` declaration value; no collection, interner, cache, clone path or
Stage 9 ledger update is warranted.

### Clippy aspect attributes accepted; typed aspect toolchains selected (2026-08-26)

Commit `5f8dd852` admits the exact ordered clippy map beside the rustfmt pair.
It validates concrete defining-repository defaults, omitted configurability,
file allowance, executable/exec configuration, provider/aspect/transition and
allowed-value fields before reusing `declared_attribute_schema`. The frozen
source proof retains all 11 rows in order and the mutation proof fails closed
on every named divergence. Existing rustfmt behavior is unchanged. All 219
loading units, 24 invalidation tests, 31 BUILD-loading tests and locked checks
pass; independent review returned `ACCEPT`.

The unchanged mixed source list is now the exact terminal. Bazel 9.2
`parseToolchainTypes` accepts String, Label and typed requirements for aspects,
resolves strings in the defining thread, makes String/Label entries mandatory,
retains typed false, preserves first-label order, and applies strictest-wins to
duplicates. The source has distinct labels, so duplicate normalization remains
deferred without weakening this exact source claim.

Run only `WP-4-7A-bazel-aspect-toolchain-requirements-loading-r2`. Rename
Slug's rule-named label/mandatory record into one shared declaration type,
retain its immutable Arc slice on both rule and aspect definitions, and reuse
the evaluator-aware parser for the aspect value list. Rule storage and
configured consumers stay semantically unchanged. Freeze the complete source
`rust_clippy_aspect` through line 404 and stop before its lazy helper and rule.

Clean `../zabel` `0795445f…` supplies concept-only architectural guidance: its
rule and aspect declarations share `ToolchainTypeRequirement` slices and one
evaluator-detachment path. Slug uses its own canonical label and mandatory
Boolean. The Buck2 utility review selects existing immutable `Arc<[T]>` and
`Allocative`; no interner, map, cache, clone mechanism, import or ledger row is
needed. No Zig behavior is copied, and Bazel 9.2 remains authoritative.

### Clippy aspect attribute audit; exact source loading selected (2026-08-26)

Bazel 9.2 `StarlarkRuleClassFunctions.attrObjectToAttributesList` validates
identifier spelling, converts private direct-value names from `_x` to `$x`
internally and retains dictionary insertion order. `aspect()` builds those
descriptors before later semantic owners, rejects either explicit value of
`configurable`, requires defaults for implicit attributes and rejects computed
defaults. `StarlarkAttrModule` supplies the ordinary defining-module label,
file, executable and `cfg="exec"` semantics. Focused upstream tests
`testAspectExtraDeps`, `testAspectNoDefaultValueAttribute`,
`testAspectParameterBadType`, `testAspectCannotSetConfigurableOnAttr`,
`testAttrAllowedSingleFileTypesWrongType` and `testAttrSingleFileWithList`
discriminate the selected subset. API prose is not widened into a complete
private-kind claim where the pinned implementation is broader.

Rules_rust clippy lines 317-364 declare these exact ordered private label
names: `_capture_output`, `_clippy_error_format`, `_clippy_flag`,
`_clippy_flags`, `_clippy_output_diagnostics`, `_config`, `_error_format`,
`_extra_rustc_flag`, `_incompatible_change_clippy_error_format`,
`_per_crate_rustc_flag`, `_process_wrapper`. Their concrete defaults resolve
in the defining rules_rust repository. All are nonmandatory and omit
configurability; only `_config` allows one file, and only `_process_wrapper` is
executable with the exec configuration. All ordinary file, provider, attached-
aspect, allowed-value and custom-transition fields are empty.

Slug's existing `AttributeDefinition` already captures every constructor
field. `declared_attribute_schema` detaches it into the same immutable schema
used by rules, and `AspectDefinitionGen.attributes` retains/freezes that slice.
No new representation, DICE key, analysis consumer, mapping owner or fallback
is needed. The exact source gate is validation only; it does not become a
parallel semantic owner.

Run only `WP-4-7A-clippy-aspect-attribute-loading`. Admit the exact map beside
the existing rustfmt pair, validate all names/defaults/fields, and reuse
`declared_attribute_schema`. Freeze a source-shaped declaration with its later
toolchain list omitted or reduced to the already-admitted singleton string,
preserve rustfmt coverage, mutate every significant field, and prove the
unchanged mixed toolchain list remains terminal. Stop before toolchain parsing,
complete aspect loading or configured aspect execution.

Clean `../zabel` `0795445f…` contributes concept-only guidance: its rule and
aspect declarations share `NamedAttribute` plus `AttrDefinition` retention.
That supports the existing Slug owner, not copied Zig layout, code, diagnostics
or behavior. Bazel 9.2 remains sole authority. Since immutable Arc storage,
hashing, collection choice, clone cost and accounting are unchanged, no Buck2
utility or Stage 9 ledger work applies. Independent audit returned bounded
implementation approval.

### Post-toolchain source-order correction; aspect attribute audit selected (2026-08-26)

Slug's external Bzl driver first parses and resolves every direct load, then
awaits each child serially in `AstModule::loads()` source order. It returns at
the first Need or semantic terminal. `ExternalBzlModuleEvalKey` is structural
over route and repository Bzl label, and the canonical-manifest regression
proves duplicate direct loads and warm transactions reuse completed child
values while preserving first-seen direct/reachable order.

That live contract authenticates the source route after `4aed2438`:
`rust/private/toolchain.bzl` returns through alias-only
`rust/rust_toolchain.bzl`; `rust/toolchain.bzl` finishes its source-ordered
analyzer, rustfmt, stdlib and toolchain wrappers, whose recursive children are
already complete; and the first load in `rust/defs.bzl` completes before its
second load reaches `rust/private/clippy.bzl`. The selected archive hashes in
the audit manifest all match.

Clippy's first import newly evaluates bazel_skylib 1.8.2
`lib/structs.bzl` at SHA-256
`c3fa79b9246582cb57c1bd9cbed999afbee822915d5888009bc0a197c43e9749`;
its one function body is lazy and its sole top-level `struct` uses the accepted
surface. The other six imports are completed dependencies of the toolchain,
rust-analyzer or rustfmt branches. Lines 48-74 use accepted provider and
string-list build-setting surfaces. Lines 76-309 are lazy function bodies,
and lines 311-314 are comments.

The implementation attempt selected in `7bba3a4e` added mixed aspect
toolchain retention, but its required source-shaped proof failed before that
parser ran: after evaluating every keyword value, `aspect()` calls
`aspect_attributes` first. The live owner accepts only the fixed rustfmt
`_config`/`_process_wrapper` pair and rejects clippy's 11 private label
attributes at lines 317-364. The attempted Rust/test diff was fully reverted.
This is a source-order `REPLAN`, not an accepted toolchain change.

Run only docs packet `WP-4-7A-clippy-aspect-attribute-audit`. Pin Bazel 9.2's
aspect attribute API, validation and tests; classify clippy's defaults,
`allow_single_file`, executable and exec-configuration rows; compare them to
the existing retained rule/aspect schema; and select one bounded implementation
or `REPLAN`. Stop before toolchain parsing.

Clean `../zabel` `0795445f…` confirms that rule and aspect declarations share
one named-attribute definition shape with defining-module ownership. That is
architectural guidance only. No Zig code, layout, diagnostic or behavior is
copied; Bazel 9.2 remains authoritative. No retained representation changes in
this docs audit, so the Buck2 utility skill selects no further action.

### Config-common toolchain requirement accepted; caller audit selected (2026-08-26)

Commit `4aed2438` introduces one evaluator-free `RuleToolchainRequirement`
owning canonical label and mandatory state through transient, frozen and
package rule owners. The `.bzl` config-common value exposes both fields;
existing String requirements remain mandatory; Label and typed requirements
retain defining identities and source order; duplicates reject; and optional
invocation stops before target publication. The configured consumer reads only
the label of already-gated mandatory requirements.

The complete 217 loading units, 24 invalidation tests, 31 BUILD-loading tests,
configured mandatory-toolchain regression, locked checks, rebuilt CLI and
hygiene pass at 172 production and 111 proof additions. Independent terminal
review returned `ACCEPT`. The focused non-root proof also corrected relative
rule toolchain strings to retain their defining repository.

The source-text candidate returns from `rust/private/toolchain.bzl` through the
alias-only `rust/rust_toolchain.bzl`; `rust/toolchain.bzl` then names the
rustfmt wrapper and imported aliases, and `rust/defs.bzl` text names
`rust/private/clippy.bzl` next. Recursive manifest/cache order has not yet
promoted that textual route to accepted evaluation order.

Run only docs packet `WP-4-7A-post-toolchain-source-order-audit`. Authenticate
the wrapper hashes, actual next child and cached-child order, then inspect the
reached call arguments in evaluation order and use pinned Bazel 9.2 to classify
the first unsupported surface. Do not edit Rust or assume that clippy is
reached or that later action bodies are evaluated.

Clean `../zabel` `0795445f…` guides only the typed declaration/capture ownership
available if the frontier is a rule/aspect toolchain requirement. It does not
define recursive source order or behavior, and no Zig code, layout or
diagnostic may be copied. Bazel 9.2 remains sole authority.

### Scalar-label provider predicate accepted; toolchain requirement selected (2026-08-26)

Commit `ef910068` admits omitted/empty and one exported provider in a flat
scalar-label predicate. The exported identity is detached into the existing
nested immutable provider schema; non-provider, unexported, multiple and nested
forms reject, as do nonempty repository/tag projections and constrained target
invocation. Both rules_rust provider rows freeze. All 215 loading tests and
downstream gates pass at 22 production and 88 proof additions; independent
terminal review returned `ACCEPT`.

The next and final evaluated expression in `rust/private/toolchain.bzl` is the
singleton rule requirement
`config_common.toolchain_type("@bazel_tools//tools/cpp:toolchain_type",
mandatory=False)`. Pinned Bazel 9.2 owns this as a typed label-plus-mandatory
value, accepts String or Label input, resolves a String in the defining `.bzl`
mapping, treats bare rule toolchain labels as mandatory, and preserves false in
the rule class. Duplicate labels use strictest-wins normalization, which is not
needed by this source row.

Run only `WP-4-7A-bazel-config-common-toolchain-type-loading`: add the bounded
`.bzl` namespace/value, replace bare retained rule labels with one compact
label-plus-mandatory record, and accept distinct String, Label and typed rule
requirements. Reject duplicate declarations rather than approximate Bazel's
strictest merge. Optional target invocation must fail before recording because
configured optional resolution is deferred. Aspect toolchains and every other
`config_common` member remain out of scope. Re-audit the caller after the child
finishes.

Clean `../zabel` `0795445f…` supplies architectural guidance only: its rule
declaration owns typed toolchain requirements and its later capture detaches
canonical label plus mandatory state. Slug uses its own Rust
`CanonicalLabel`/Boolean/`Arc` representation and copies no Zig code, layout,
diagnostic or configured behavior. The Buck2 utility audit selects existing
compact immutable owners; no new collection, interner, hash or ledger row is
needed. Bazel 9.2 remains sole authority.

### Scalar-label file allowance accepted; provider predicate selected (2026-08-26)

Commit `b1edbe0e` admits Boolean/`None` scalar-label file allowance, checks the
simultaneous non-None `allow_single_file` conflict before normalization, and
reuses the existing schema Boolean. True is any-file but not single-artifact;
repository and tag projections fail closed. Both selected rules_rust LLVM rows
freeze. All 214 loading tests and downstream gates pass at 10 production, 91
proof and 101 total additions; independent terminal review returned `ACCEPT`.

Source order now reaches `lto` with `providers=[RustLtoInfo]`; the hidden
allocator setting later repeats that scalar-label shape with
`BuildSettingInfo`. All other remaining attribute-map rows use admitted
constructors. Pinned Bazel 9.2 maps a flat provider list to one conjunction of
exported provider identities. The next distinct stop after both predicates is
the rule-level `config_common.toolchain_type(...)` call.

Run only `WP-4-7A-bazel-label-provider-predicate-loading`: accept omitted,
empty and the source-required single exported provider in a flat list, reuse
the existing immutable nested-`Arc` provider schema, and keep target invocation
plus repository/tag projections fail-closed. Broader predicate shapes,
configured validation and `config_common` remain deferred.

Clean `../zabel` `0795445f…` guides sharing one provider-predicate declaration
fact across scalar/list dependency attributes and detaching it before package
lowering. No Zig evaluator value, code, layout or behavior is copied. The
Buck2 audit selects the existing `Arc<[Arc<[ProviderId]>]>` and `Allocative`
owners with no utility or ledger change. Bazel 9.2 remains sole authority.

### String allowed values accepted; scalar-label file allowance selected (2026-08-26)

Commit `80425ce9` unifies integer and string allowed-value sets in one typed,
evaluator-free schema owner. String sets normalize once, explicit direct,
selectable and final concatenated candidates are enforced, ordinary defaults
remain unchecked, and repository/tag projections fail closed. Both selected
rules_rust linker constraints freeze. All 213 loading tests and downstream
gates pass within 77 production, 165 proof and 242 total additions;
independent terminal review returned `ACCEPT`.

Source order now reaches `llvm_lib` and `llvm_tools` in
`rust/private/toolchain.bzl`. Each uses scalar
`attr.label(allow_files=True)`; the intervening `llvm_profdata` uses the
accepted single-file form. Pinned Bazel 9.2 maps Boolean true to `ANY_FILE`,
false/omitted/`None` to no file predicate, rejects simultaneous non-None
`allow_files` and `allow_single_file`, and sets `SINGLE_ARTIFACT` only for the
latter. The next distinct stop is `lto` with
`providers=[RustLtoInfo]`.

Run only `WP-4-7A-bazel-label-allow-files-loading`: add the Boolean/`None`
argument to the scalar-label adapter, perform the presence conflict before
normalization, and reuse the existing declaration/frozen/package
`allow_files` Boolean. Extension predicates, file resolution and scalar-label
providers remain deferred.

Clean `../zabel` `0795445f…` guides the separate file/single-file declaration
facts and the same pre-normalization conflict boundary. No Zig code, layout or
behavior is copied. The Buck2 audit selects the existing inline Boolean and
`Allocative` owners, with no utility or ledger change. Bazel 9.2 remains sole
behavior authority.

### Integer allowed values accepted; string allowed values selected (2026-08-26)

Commit `563699ab` detaches the selected signed-32-bit integer allowed-value set
into immutable declaration, frozen-rule and package schemas. Empty sequences
remain unconstrained, order/duplicates normalize, explicit and plain-selector
candidates are checked, and ordinary defaults remain unchecked exactly on the
admitted Bazel rule path. Unsupported projections reject the constraint rather
than erase it. All 212 loading tests and downstream gates pass at 73 production
and 160 proof additions; independent terminal review returned `ACCEPT`.

Source order next reaches `linker_preference` and `linker_type` at
`rust/private/toolchain.bzl:766-772`. Both use `attr.string(values=...)` with
small string sets. Pinned Bazel 9.2 shares the nonempty `AllowedValueSet`
builder path and checks every explicit configurable candidate; its
`ConfigurableAttributesTest` also proves string concatenation is validated
after selector candidate combination. The next distinct stop is `llvm_lib`
line 781, whose label `allow_files=True` remains absent.

Run only `WP-4-7A-bazel-string-allowed-values-loading`: replace the
integer-only constraint field with one typed integer/string enum, retain sorted
deduplicated `Arc` slices through the same schemas, and reuse existing
correlated candidate expansion for string selector/concatenation enforcement.
Keep ordinary defaults unchecked and unsupported projections fail-closed.

Clean `../zabel` `0795445f…` guides the same unified declaration-owned
`allowed_values` fact and evaluator-detachment boundary. The Buck2 utility
audit selects existing `Arc`, `CompactString` and `Allocative` patterns with no
new utility import or ledger row. No Zig code, layout or behavior is copied;
Bazel 9.2 remains sole compatibility authority.

### Data-attribute docs accepted; integer allowed values selected (2026-08-26)

Commit `8d3f9b6e` accepts string/`None` documentation on the remaining int,
string-list, string-dict and string-list-dict constructors used by
`rust_toolchain`. Documentation is validated at the existing adapter and
discarded from retained semantics. Distinct docs freeze to equal rule schemas,
wrong types reject, and the source-shaped prefix reaches its first `values`
argument. All 210 loading tests, configured analysis, locked checks, rebuilt
CLI and hygiene pass at 8 production and 61 proof additions; independent
terminal review returned `ACCEPT`.

The next evaluated row at `rust/private/toolchain.bzl:727-738` is
`attr.int(values = [-1, 0, 1], default = -1)`. Pinned Bazel 9.2 defines
named-only `values` as an integer sequence, installs no predicate for an empty
sequence, and checks every possible explicitly supplied rule-instance value
against the nonempty set while leaving ordinary defaults unchecked. Its
focused `testAttrIntValues` distinguishes members from nonmembers.

Run only `WP-4-7A-bazel-int-allowed-values-loading`: normalize and detach the
integer set into the existing declaration, frozen-rule and package schema
owners, then enforce explicit/select candidates before target
recording. Preserve empty as no constraint and reject unsupported projections
instead of dropping the fact. Source order must stop at `linker_preference`
line 768, whose string allowed values remain unadmitted.

Clean `../zabel` `0795445f…` guides the declaration-owned constraint and
evaluator-detachment boundary only. The Buck2 utility audit selects the
existing immutable `Arc<[T]>` and `Allocative` schema pattern; it adds no
utility import, interner, cache or ledger row. No Zig code, layout or behavior
is copied, and Bazel 9.2 remains sole compatibility authority.

### Rust stdlib filegroup accepted; data-attribute docs selected (2026-08-26)

Commit `75709828` adds one normalized `allow_files` Boolean to the existing
attribute declaration, frozen rule and target schema owners. Omitted,
explicit `None` and false normalize to no-file; true normalizes to any-file.
The source-shaped rules_rust `rust_stdlib_filegroup` declaration freezes and
projects its mandatory, non-single-artifact `srcs` schema without admitting a
file target or running its implementation. All 209 loading units, configured
analysis, locked checks, rebuilt CLI and hygiene pass at 37 production and 84
proof additions; independent terminal review returned `ACCEPT`.

Source order next begins the larger `rust_toolchain` rule at line 664.
Allocator, binary, cargo, channel and clippy descriptors use accepted shapes.
The first missing evaluated argument is `doc` on the `debug_info`
`attr.string_dict` at line 695. Later in the same rule, string-list,
string-list-dict and int descriptors also carry string documentation, while
Slug currently accepts docs only on label, label-list, bool and string.

Run only `WP-4-7A-bazel-data-attribute-doc-loading`: validate string/`None`
documentation through the existing helper for int, string-list, string-dict
and string-list-dict, discard it from semantic identity, and prove wrong types
reject. Stop at line 727, where `attr.int(values = [-1, 0, 1])` first requires
a retained allowed-value predicate. Documentation extraction and allowed-value
enforcement remain deferred.

Clean `../zabel` `0795445f…` is architectural guidance for the same transient
validation-and-discard boundary only; no Zig code or behavior is copied. No
retained structure, collection, hashing, interning or memory accounting changes,
so the Buck2 utility audit requires no new utility or ledger row. Bazel 9.2
remains sole behavior authority.

### Cc_common wrapper accepted; label-list file allowance selected (2026-08-26)

Commit `4bdd64bf` adds only the deprecated
`do_not_use_tools_cpp_compiler_present` property to the existing public
`cc_common` loading value. Bazel's `None` result is visible directly and when
captured into the frozen exported rules_cc wrapper; it remains non-callable,
unknown fields remain absent, BUILD globals remain unchanged and no configured
C++ behavior is admitted. All 207 loading units, configured analysis, locked
checks, rebuilt CLI and hygiene pass at 4 production and 34 proof additions;
independent terminal review returned `ACCEPT`.

Recursive source order next enters rules_rust
`rust/private/toolchain.bzl`. The first evaluated declaration is
`rust_stdlib_filegroup`; its `srcs` schema calls
`attr.label_list(allow_files = True)` at line 115. All imports, the lazy
implementation and the remaining declaration arguments use accepted shapes,
but Slug's label-list constructor does not accept `allow_files`.

Pinned Bazel 9.2 `StarlarkAttrModule.setAllowedFileTypes` maps Boolean true to
`FileTypeSet.ANY_FILE`; its regression proves this is a label-list file
predicate and not `SINGLE_ARTIFACT`. Run only
`WP-4-7A-bazel-label-list-allow-files-loading`: retain a normalized Boolean
file-allowance fact in the existing declaration, frozen rule schema and target
schema, prove true/false/None identity and freeze the source-shaped stdlib
rule. Extension predicates and actual source-file target resolution remain
fail-closed. Stop before the later `rust_toolchain` declaration and
`config_common.toolchain_type`.

Clean `../zabel` `0795445f…` supplies architectural/test guidance only: its
declaration owns `allows_files` separately from `allows_single_file`, and its
source-shaped tests keep the label-list schema non-single-artifact. No Zig code,
layout, algorithm, diagnostics or behavior is copied. The Buck2 utility audit
selects one inline Boolean in existing `Allocative` schemas; there is no new
collection, allocation, hash, interner or Stage 9 ledger row. Bazel 9.2 remains
sole behavior authority.

### Empty compilation outputs accepted; cc_common compiler sentinel selected (2026-08-26)

Commit `b0cd7855` accepts exact empty-list `cc_internal.freeze` and completes
the top-level `EMPTY_COMPILATION_OUTPUTS` provider. The result is evaluator-
owned and immutable; all non-empty and general container shapes fail closed.
The source-shaped proof, configured regression, all 206 loading units, locked
checks, rebuilt CLI and hygiene pass within 15 production, 69 proof and 84
total additions. Independent terminal review returned `ACCEPT`.

The recursive source audit passes `compile.bzl` and the remaining direct
children at loading time because their semantic C++ operations stay lazy and
their evaluated declarations use admitted shapes. At
`cc/private/cc_common.bzl:735`, exported wrapper construction first reads
`_cc_common_internal.do_not_use_tools_cpp_compiler_present`. Pinned Bazel 9.2
exports that deprecated struct field as `None`; no invocation or configured
behavior is involved.

Run only `WP-4-7A-bazel-cc-common-compiler-sentinel-loading`. Add one stateless
attribute observation to the existing `.bzl` `cc_common` value, preserve BUILD
absence, and stop once rules_cc's wrapper freezes. Clean `../zabel`
`0795445f…` is architectural/test guidance for the same direct-property and
wrapper-construction boundary only; no Zig code, dispatch representation or
behavior is copied. No retained representation or Buck2 utility changes.

### Documented provider initializer accepted; empty-list freeze selected (2026-08-26)

Commit `152caa6f` accepts documented string-dictionary schemas through the
existing initialized-provider owner, completing the source-shaped `CcInfo` and
`CcLauncherInfo` declarations. The shared-library hint and LTO provider
declarations then freeze. All 205 loading units, configured analysis, locked
checks, rebuilt CLI and hygiene pass; independent terminal review returned
`ACCEPT`.

Source order next enters `cc/private/compile/cc_compilation_outputs.bzl`.
Top-level `EMPTY_COMPILATION_OUTPUTS = create_compilation_outputs_internal()`
first reaches `_cc_internal.freeze(objects)` at line 86; all ten freeze
arguments on this invocation are default empty lists. Pinned Bazel 9.2 returns
an immutable list copy. The bounded exact row can reuse the existing
starlark-rust frozen empty-list singleton while rejecting non-empty lists,
dictionaries and every other unselected shape.

Run only `WP-4-7A-bazel-empty-list-freeze-loading`. Complete and freeze the
top-level empty compilation-output provider, but do not claim general
`cc_internal.freeze`, configured C++ semantics or later source. Clean
`../zabel` `0795445f…` is architectural/test guidance for one evaluator-owned
immutable-copy boundary and source/result mutation separation only; no Zig
code, representation or behavior is copied. The Buck2 reuse audit selects the
existing `AllocList::EMPTY` and frozen heap, so no new utility or ledger row is
needed. Bazel 9.2 remains sole behavior authority.

### Empty HeaderInfo accepted; documented provider initializer selected (2026-08-26)

Commit `2ebc6fe1` accepts only no-argument empty HeaderInfo loading. The private
capability returns a fresh immutable `HeaderInfo`; four module fields are
`None`, four header fields are immutable empty lists, aliases retain occurrence
identity through freeze and distinct calls differ. The value is loading-only,
unhashable, accepts no named/non-empty inputs, and has no dependency or
configured-analysis projection. Focused proof, all 204 loading units,
configured analysis, locked checks, rebuilt CLI and hygiene pass at 77
production, 74 proof and 151 total additions. Selection review corrected the
next stop to `CcInfo`; terminal review returned `ACCEPT`.

The first absent expression is now `cc/private/cc_info.bzl:260–269`:

```starlark
CcInfo, _ = provider(
    doc = "Provider for C++ compilation and linking information.",
    fields = { ... },
    init = _create_cc_info,
)
```

Pinned Bazel 9.2 normalizes a string-to-string dictionary schema before the
same `ArgumentProcessorWithInit` and raw-constructor path used by list schemas.
The initializer still receives original arguments, returns a string-keyed
dictionary, permits omitted declared fields and rejects unknown fields. Both
constructors share the exported provider identity.

The same abstraction freezes `cc_info.bzl` and the later documented-dictionary
initialized `CcLauncherInfo`. `cc_shared_library_hint_info.bzl` then needs only
accepted direct documented providers. The LTO child declares direct documented
providers and an already-admitted dictionary-valued instance. Source order next
enters `cc_compilation_outputs.bzl`; its top-level empty row invokes
`create_compilation_outputs_internal()` and first reaches the unsupported
`_cc_internal.freeze(objects)` at line 86.

Run only `WP-4-7A-bazel-documented-provider-initializer-loading`. Generalize
only initialized schema parsing to accept a documented dictionary and feed its
normalized names into the existing callable/raw/instance owner. Do not add a
second identity or representation, retain documentation, admit initialized
values to configured analysis, or implement `cc_internal.freeze`.

Clean `../zabel` `0795445f…` is architectural guidance only. Its normalized
provider schema and one `ProviderDefinition` owner keep schema, initializer,
raw constructor, publication and export identity together. Slug follows that
owner and phase split using its existing `Arc<[CompactString]>`, ordinal
`SmallMap`, `Value`/`FrozenValue`, `Dupe` and `Allocative`; no Zig code, layout,
runtime, allocator or behavior is copied, and no Buck2 utility or ledger row is
needed. Bazel 9.2 remains sole compatibility authority.

### Provider schemas accepted; empty HeaderInfo selected (2026-08-26)

Commit `f65c9ce0` accepts non-initialized schemaless, unique string-list and
documented-map provider schemas plus optional arbitrary loading values. One
assignment-bound `ProviderId` survives freeze; schemaful rows use compact
ordinals, schemaless rows retain dynamic compact names, and only the pre-
existing complete documented-string shape enters configured analysis. Focused
proof, all 203 loading units, configured analysis, locked checks, rebuilt CLI
and hygiene pass at 173 production, 102 proof and 275 total additions.
Independent review returned `ACCEPT`.

Recursive source order now completes
`cc/private/link/create_extra_link_time_library.bzl` and resumes
`cc/private/cc_info.bzl`. Its top-level `EMPTY_COMPILATION_CONTEXT` constructs
the admitted documented provider with depsets, lists and `None`, then first
fails at line 134:

```starlark
_header_info = _cc_internal.create_header_info()
```

Pinned Bazel 9.2 `CcStarlarkInternal.createHeaderInfo` supplies eight named-
only parameters with empty defaults. The selected zero-argument row creates a
fresh immutable `HeaderInfo`: `header_module`, `pic_header_module`,
`separate_module`, and `separate_pic_module` are `None`; the four direct header
fields are immutable empty lists; dependencies are empty. Evaluation then
passes lazy functions until lines 260–269, where `CcInfo, _ = provider(...)`
uses a documented dictionary schema with an initializer. That shape is not
admitted by the accepted list-schema initializer and is the next source stop.

Run only `WP-4-7A-bazel-empty-header-info-loading`. Extend the existing opaque
`cc_internal` capability with the no-argument constructor and add one frozen,
loading-only HeaderInfo value. Preserve fresh occurrence identity and exact
empty field observations. Reject all arguments; keep non-empty HeaderInfo,
hashing, dependency DAGs, `create_header_info_with_deps`, configured provider
lowering and every other C++ method unsupported/deferred.

Clean `../zabel` `0795445f…` is architectural guidance only. Its C++ primitive
owner keeps evaluator-local HeaderInfo fields together and explicitly hands
retained ownership to later provider lowering. Slug follows that phase split
with one immutable empty-list value shared by the unobservable-equal list
fields, but copies no Zig code, layout, allocator, analysis behavior or API.
Bazel 9.2 is the sole compatibility authority. Existing starlark-rust frozen
values and `Allocative` satisfy the Buck2 utility audit without a new utility,
collection, counter, interner or Stage 9 ledger row.

### Provider initializer accepted; provider schemas selected (2026-08-26)

Commit `9c51999f` accepts the rules_cc initialized artifact-category provider.
One assignment-bound callable owns normal/raw construction and identity; normal
calls forward original arguments through the initializer, raw calls bypass it,
dictionary results are schema-checked with optional fields, and arbitrary
Starlark values freeze in compact schema slots. The initialized instance stays
separate from the configured string provider. Focused proof, all 202 loading
units, the configured regression, locked core check, rebuilt CLI and hygiene
pass. Growth is 300 production and 97 proof additions. Independent review
restored the pre-existing unbound-provider diagnostic and returned `ACCEPT`.

Recursive source order then enters rules_cc 0.2.17
`cc/private/link/create_extra_link_time_library.bzl` through
`cc/private/cc_info.bzl`. Its accepted `cc_helper_internal` and `cc_internal`
loads are followed by two schemaless provider declarations, a string-list
schema, and a documented provider whose top-level `_EMPTY` instance contains a
list. The first missing expression is line 34
`provider("ExtraLinkTimeLibraryInfo")`; accepting declarations alone would not
freeze the child because `_EMPTY` also requires arbitrary direct field values.

Run only `WP-4-7A-bazel-provider-schema-loading`. Normalize omitted/`None` as
schemaless and unique string lists as schemaful, retain arbitrary optional
loading values, reject positional and unknown schema fields, and distinguish
an empty schema from schemaless. Preserve the existing documented/full-string
configured projection; every other new instance remains loading-only. Stop
after this child freezes, before `cc_info.bzl` invokes
`cc_internal.create_header_info()` or any C++ semantic method.

Pinned Bazel 9.2 `StarlarkRuleFunctionsApi.provider`,
`StarlarkRuleClassFunctions.provider`, `StarlarkProvider.RawArgumentProcessor`,
`StarlarkInfoNoSchema`/`WithSchema`, and their declaration/schema/optional-field
tests are exact authority. Clean `../zabel` `0795445f…` guides only the
architecture: one definition owns schema kind, initializer, publication owner
and identity; schemaful values use canonical field positions while schemaless
values necessarily retain dynamic names. Slug reuses `Arc<CompactString>`,
ordinal `SmallMap` slots, `Value`/`FrozenValue`, `Dupe` and `Allocative`; it
copies no Zig code, runtime, allocator, digest or behavior.

### cc_common private bridge accepted; provider initializer selected (2026-08-26)

Commit `4d7a9bbb` installs the public `cc_common` wrapper only in complete
`.bzl` globals. `internal_DO_NOT_USE()` is zero-argument, accepts canonical
`rules_cc+` defining-call owners, rejects root and foreign owners with Bazel's
canonical-label private diagnostic, returns one frozen opaque token and leaves
BUILD plus every internal C++ method absent. Focused proof and all 201 loading
units pass; broad loading retains only its known line-2948 `@external`
diagnostic-order failure. Locked core check, rebuilt CLI and hygiene pass.
Independent review requested exact main-repository `//...` diagnostic spelling
and returned `ACCEPT` after correction.

The next recursive child `cc/private/paths.bzl` defines only the lazy
`is_path_absolute` function and freezes on the accepted evaluator. The first
absent evaluated call is then rules_cc 0.2.17
`cc/common/cc_helper_internal.bzl`'s `_ArtifactCategoryInfo` declaration:
`provider(fields = [four strings], init = _artifact_category_info_init)`. The
same file immediately constructs its fixed rows, reads their fields, forms the
artifact-name struct and freezes the instances.

Pinned Bazel 9.2 `StarlarkRuleFunctionsApi`,
`StarlarkRuleClassFunctions.provider`,
`StarlarkProvider.ArgumentProcessorWithInit`/`RawArgumentProcessor`, and the
focused `declaredProvidersWithInit`/raw-bypass/failure tests establish the
contract. A callable initializer changes `provider()` to a two-value provider
and raw-constructor pair; the normal constructor forwards original arguments,
requires a string-keyed dictionary and applies the schema; raw construction
bypasses the callback, rejects positional arguments and retains the same
provider identity. Declared schema fields remain optional.

Run only `WP-4-7A-bazel-provider-initializer-loading`. Keep the existing
configured string-only callable/instance untouched and add a loading-only
initialized family in the same provider owner. Its raw callable references the
authoritative assignment-bound provider callable, and every closure, instance
and arbitrary freezeable field value remains owned by the frozen module heap.
Do not admit initialized instances as rule-analysis results or broaden into a
C++ provider/toolchain/action method. Stop after this artifact-category child
and re-audit recursive source order.

Pinned Zabel `c7298478…` is architectural guidance only. Its provider
definition co-owns schema, initializer and export identity; raw construction
references that owner; and its rules_cc-shaped test exercises the same source
sequence. Slug adopts the ownership and phase split through existing
starlark-rust values without copying Zig code or behavior. The retained
`Value`/`FrozenValue`, `CompactString`, deterministic `SmallMap`, `Dupe` and
`Allocative` patterns already satisfy the Buck2 utility ledger.

### Config-string descriptor accepted; rules_cc private bridge selected (2026-08-26)

Commit `919ecfa5` retains `.bzl` String `flag` and `allow_multiple` as one
compact declaration fact. All four Boolean pairs load and freeze; BUILD keeps
its true/single-only constructor; and only true/single targets may record and
reach the unchanged scalar configured consumer. Non-flag and multi-value
targets fail in the small pre-recording gate. Focused proof, all 200 loading
units, locked core check, rebuilt CLI and hygiene pass. Broad integration is
30/31 only because of the declared stale `@external` diagnostic-order row.
Independent review returned `ACCEPT` within the 41/134/175 addition caps.

The authenticated rules_rust 0.73.0 source now returns to
`rust/private/toolchain.bzl` (SHA-256 `c4b613ce…`). Its next load is rules_cc
0.2.17 `cc/common/cc_common.bzl` (SHA-256 `65e91cf0…`). The lockfile fixes the
rules_cc source JSON SHA-256 at `3832f45d…`; that source selects archive
SHA-256 `283fa1cd…`. Bazel 9.2's generated compatibility proxy
`symbols.bzl` hashes to `2adedeea…` and loads
`cc/private/cc_common.bzl` (SHA-256 `5e6ab737…`). Its first child reaches
`cc/common/cc_helper_internal.bzl` (SHA-256 `793ab429…`), which first freezes
Skylib `lib/paths.bzl` (SHA-256 `96cce438…`) and then loads
`cc/private/cc_internal.bzl` (SHA-256 `8241ced5…`).

That last file's sole evaluated expression is
`cc_common.internal_DO_NOT_USE()` guarded by `hasattr`. Slug has no
`cc_common` `.bzl` global, so name resolution is the first absent surface.
Pinned Bazel 9.2 `bazel/exports.bzl`, `cc_common_bazel.bzl`,
`BazelStarlarkEnvironment` and `CcStarlarkInternal.checkPrivateApi` establish
the contract: builtins injection exports the public wrapper; its zero-argument
internal bridge checks the innermost calling module against rules_cc; any
canonical repository whose name begins `rules_cc+` satisfies that module-name
allowlist; and a foreign caller fails with `file '<label>' cannot use private
API`. BUILD receives injected rules only, not this exported `.bzl` toplevel.

Run only `WP-4-7A-bazel-cc-common-private-bridge-loading`. Put a stateless
public wrapper and stateless opaque internal token in a small loading-owned
module, install the wrapper only in the complete `.bzl` globals environment,
and make the bridge use existing defining-call provenance to admit the pinned
`rules_cc+` owner and fail every other owner. The token must freeze but expose
no fields or methods. Prove exact placement, zero-argument binding, allowed
canonical-owner call/freeze and foreign-owner diagnostic. Do not load or
implement bundled Bazel builtins, add a generic private-API framework, expose
BUILD `cc_common`, or admit any `cc_internal`/public C++ method, provider,
toolchain, action or analysis behavior. Re-audit source order after this one
child.

Pinned Zabel `c7298478…` supplies architectural guidance only. Its
`builtins_cc_primitives.zig` deliberately keeps the private native token behind
a mandatory owner capability and leaves public wrapper construction to the
builtins layer. Slug adopts that public/private separation and fail-closed
ownership rule, while its narrow public projection is explicitly Slug-native
until full builtins injection is selected. No Zig code, layout, method table,
analysis object or behavior is copied. The values are zero-sized and
`Allocative`; the Buck2 utility audit selects no collection, interner, cache or
utility import. Bazel 9.2 remains sole behavior authority.

### Universal-builtin environment accepted; complete helper retry selected (2026-08-26)

Commit `cb71a302d` centralizes the exact process-stable 30-name Bazel 9.2
universe across active loading, MODULE, REPO and core routes, activates the real
default `set`, removes REPO's stale shim and excludes `chr`/`ord` without
overlay leakage. The set proof includes non-aliasing copy and frozen mutation
rejection. Full affected regressions and independent review pass.

The required archive check then exposed one checker-only omission: its explicit
V2 app list predated `app/slug_starlark_v2`. Commit `5c3b4492f` adds exactly
that pathspec and restores the app gate without hiding the three longstanding
thoughts baselines. Retry only
`WP-4-7A-rules-cc-compilation-helper-complete-loading-proof-r2` under
0/1050/1050 caps with the identical authenticated source and five accepted
children, complete imported/eager identity and visibility, and no invocation.
Zabel is peer ownership guidance only; Bazel 9.2 and authenticated rules_cc
bytes own compatibility.

### Complete compilation helper accepted; compile-variable producer selected (2026-08-26)

Commit `3060e4d4d` freezes the exact 666-line compilation helper over its five
complete children and proves the complete imported/public/private/captured
inventory without invocation. Independent review found one child manifest row
that paired a mapped frozen module with an empty mapping; the accepted proof
retains the actual mapped defining identity and asserts all five child labels
and mappings. All required suites pass at 0 production and 871 proof additions.

Recursive source order through `compile.bzl` now reaches
`compile_action_templates.bzl`, whose sole incomplete child is the same
644-line `compile_build_variables.bzl` loaded directly by private
`cc_common.bzl`. This is therefore the smallest shared proxy/toolchain
frontier. Its additional 18-line `native_cc_common.bzl` leaf aliases the
already admitted `.bzl` predeclared wrapper; its complete producer eagerly
builds only one 25-field struct, an empty-schema provider and sentinel instance,
one source-type set, function defaults and lazy functions from accepted shapes.

Run only `WP-4-7A-rules-cc-compile-build-variables-complete-loading-proof`
under 0/1050/1050 caps. Embed/hash both complete sources, retain the three child
identities, prove exact imported/eager/public/private inventory and invoke no
callable/provider. Zabel `0795445f…` is peer guidance for defining-module-owned
global/default freezing and separation from invocation values only; no Zig
content or behavior is adopted. Bazel 9.2 and authenticated rules_cc own exact
compatibility. Re-audit action-template/compile source order after acceptance.

### Complete compile variables accepted; action templates selected (2026-08-26)

Commit `97faa6e71` freezes the exact native wrapper and 644-line compile-variable
producer over its complete children. The proof retains the wrapper's actual
predeclared pointer, exact child mappings, imports, 25-field struct, private
provider/sentinel, ordered 22-element set, 13 functions and complete visible and
private inventories. All required suites pass at 0 production and 875 net proof
additions; independent review returned `ACCEPT`.

Recursive source order now reaches complete 266-line
`compile_action_templates.bzl` (`10a43c512a85458f45a0223a7ddc7c1b56f8072872b765b1744d336ff91ec794`).
All six children are accepted. Its eager surface is exactly ten imported aliases
and five lazy functions, so no global, builtin, provider, native method or
configured semantic change is required. Run only
`WP-4-7A-rules-cc-compile-action-templates-complete-loading-proof` under
0/600/600 caps, prove complete identity/visibility and invoke nothing. Zabel is
peer defining-module/import guidance only; Bazel 9.2 and authenticated rules_cc
bytes own compatibility. Re-audit the complete `compile.bzl` parent afterward.

### Complete action templates accepted; compile parent selected (2026-08-26)

Commit `bb11a1f73` freezes all 266 authenticated action-template lines over six
complete children and proves its complete ten-import/five-function inventory
without invocation. All required suites pass at 0 production and 482 proof
additions; independent review returned `ACCEPT`.

The recursive frontier is now complete 2,295-line `compile.bzl`, SHA-256
`bec506ffc3be08fffc4842b9daac498773534db9916121648a5527fac84cabea`.
All eleven children are accepted. Its eager surface uses the existing universal
`set`, accepted provider initializer/raw-constructor shape, four ordered sets
and lazy functions; no new builtin or native method is required. Run only
`WP-4-7A-rules-cc-compile-complete-loading-proof` under 0/3000/3000 caps.
Embed/hash the complete source, retain exact child/import/eager/function
identities and invoke nothing. Zabel is peer ownership/freeze guidance only;
Bazel 9.2 and authenticated rules_cc bytes own compatibility. Re-audit private
`cc_common` and proxy/toolchain consumers after acceptance.

### Complete compile producer accepted; linkstamp child selected (2026-08-26)

Commit `d32e2602d` freezes all 2,295 authenticated compile lines over eleven
complete children. It proves the complete 25-import, four-set, initialized
provider/raw-constructor and 28-function inventories without invocation. All
required suites pass at 0 production and 2,694 proof additions; independent
review returned `ACCEPT`.

Private `cc_common.bzl` source order now reaches complete 111-line
`compile/linkstamp_compile.bzl`, SHA-256
`6f5ceb39f1b6c26b65073867f3435ec01093775edf6129d2b9421bca4c7a70bb`.
All six children are accepted. Its eager surface is exactly six imported
aliases and one public lazy function, so no builtin, provider, native method or
configured semantic change is required. Run only
`WP-4-7A-rules-cc-linkstamp-compile-complete-loading-proof` under 0/300/300
caps, retain exact source/child/import identities and invoke nothing. Zabel is
peer ownership guidance only; Bazel 9.2 and authenticated rules_cc bytes own
compatibility. Re-audit the first link-family child after acceptance.

### Complete linkstamp accepted; LTO-backends child selected (2026-08-26)

Commit `78acfe43f` freezes all 111 authenticated linkstamp lines over the six
actual complete children, including CcInfo's retained Skylib mapping. It proves
all six imported identities, the public function and exact visibility
inventories without invocation. All required suites pass at 0 production and
223 proof additions; independent review returned `ACCEPT`.

Private `cc_common.bzl` source order next reaches
`link/create_library_to_link.bzl`; its first incomplete child is complete
540-line `link/lto_backends.bzl`, SHA-256
`078bfb686e85b584745fcea2d9e5535938f9afc1a0066f80cc88aceb699f4226`.
All four children are accepted. Its eager surface is four imports, one provider
and ten lazy functions using admitted shapes. Run only
`WP-4-7A-rules-cc-lto-backends-complete-loading-proof` under 0/900/900, retain
exact source/child/import/provider/function identities and invoke nothing.
Zabel remains peer ownership guidance; Bazel 9.2 and authenticated rules_cc
bytes own compatibility.

### Complete LTO backends accepted; create-library selected (2026-08-27)

Commit `ccab93d4c` freezes all 540 authenticated LTO-backend lines over four
actual complete children. It proves every imported identity, native alias,
provider, ten functions and exact visibility inventories without invocation.
All required suites pass at 0 production and 657 proof additions; independent
review returned `ACCEPT`.

All five children of complete 291-line `link/create_library_to_link.bzl`,
SHA-256 `5f57423312f24392f106aeb5959485c4f30c54ee2d8e926a45934de51a2455d1`,
are now accepted. Its eager surface is six imports, one private warning string,
one provider and four lazy functions. Run only
`WP-4-7A-rules-cc-create-library-to-link-complete-loading-proof` under
0/600/600, retain exact identities and invoke nothing. Zabel remains peer
ownership guidance; Bazel 9.2 and authenticated rules_cc bytes own
compatibility.

### Complete create-library accepted; linker-input selected (2026-08-27)

Commit `ace75573b` freezes all 291 authenticated create-library lines over five
actual complete children. It proves exact child mappings, all six imported
identities, the warning, provider, four functions and exact visibility
inventories without invocation. All required suites pass at 0 production and
463 proof additions; independent correction review returned `ACCEPT`.

Private `cc_common.bzl` source order next reaches complete 69-line
`link/create_linker_input.bzl`, SHA-256
`e4e8a7fc9d7be8edd40a2b95e72a96710c05d5bbd610b2c1cc2f274e3672cbd1`.
Its sole child, `cc_internal.bzl`, is accepted. Its eager surface is one private
import, one private provider and one public lazy function; empty depset/list
defaults remain inside the function object. Run only
`WP-4-7A-rules-cc-create-linker-input-complete-loading-proof` under 0/300/300,
retain exact identities, invoke nothing and inspect no callable default. Zabel
remains peer ownership guidance; Bazel 9.2 and authenticated rules_cc bytes own
compatibility.

### Complete linker-input accepted; target types selected (2026-08-27)

Commit `2c1706e70` freezes all 69 authenticated linker-input lines over complete
`cc_internal.bzl`. It proves the private imported pointer and provider, public
function and exact one-public/three-all inventories without invocation or
callable-default inspection. All required suites pass at 0 production and 142
proof additions; independent review returned `ACCEPT`.

The next private `cc_common` producer reaches incomplete `cc_linking_helper.bzl`.
Recursive source order through `cpp_link_action.bzl`, `finalize_link_action.bzl`
and `collect_solib_dirs.bzl` first reaches complete 131-line
`link/target_types.bzl`, SHA-256
`12110c7dce405cd2ba4253d694502f08cc97a95bd0004444054ae8aa689da8fd`.
Its two children are accepted. Run only
`WP-4-7A-rules-cc-target-types-complete-loading-proof` under 0/500/500, prove
the complete imported/string/nested-struct/function inventory and invoke
nothing. Zabel remains peer ownership guidance; Bazel 9.2 and authenticated
rules_cc bytes own compatibility.

### Complete target types accepted; solib dirs selected (2026-08-27)

Commit `49e139212` freezes all 131 authenticated target-type lines over two
actual complete children. It proves exact named imports, strings, linking-mode
and all ten six-field target mappings, the function and exact seven-public/
seven-all inventories without invocation; struct iteration order remains
Slug-native. All required suites pass at 0 production and 283 proof additions;
independent review returned `ACCEPT`.

Recursive source order resumes at complete 479-line
`link/collect_solib_dirs.bzl`, SHA-256
`f25b0f978bce3a3cf810b36c6897a85adefce7036ec68ba53613352afa218125`.
Its three children are accepted. Run only
`WP-4-7A-rules-cc-collect-solib-dirs-complete-loading-proof` under 0/750/750,
prove the complete five-import/seven-function inventory and invoke nothing.
Zabel remains peer ownership guidance; Bazel 9.2 and authenticated rules_cc
bytes own compatibility.

### Complete solib dirs accepted; link values selected (2026-08-27)

Commit `6833c72de` freezes all 479 authenticated solib-directory lines over
three actual complete children. It proves all five imported identities, seven
function types/visibility and exact six-public/twelve-all inventories without
invocation. All required suites pass at 0 production and 599 proof additions;
independent review returned `ACCEPT`.

`finalize_link_action.bzl` source order next reaches complete 363-line
`link/create_libraries_to_link_values.bzl`, SHA-256
`7d8df512d6b0df2178a2ca9cd30cb36d1a22c96877dd8e69f49bd3cf739a3764`.
Its sole child is accepted. Run only
`WP-4-7A-rules-cc-create-libraries-to-link-values-complete-loading-proof`
under 0/650/650, prove its complete imported/struct/provider/function inventory
and invoke nothing. Zabel remains peer ownership guidance; Bazel 9.2 and
authenticated rules_cc bytes own compatibility.

### Complete link values accepted; link-build variables selected (2026-08-27)

Commit `955e2204f` freezes all 363 authenticated library-to-link-value lines
over its actual complete child. It proves all three imported identities, six
named type mappings, three private provider identities, five function
types/visibility and exact six-public/twelve-all inventories without
invocation. All required suites pass at 0 production and 498 proof additions;
independent review returned `ACCEPT`.

`finalize_link_action.bzl` source order next reaches complete 392-line
`link/link_build_variables.bzl`, SHA-256
`bdf030361c5a199f6c0fd1bbe5e3b1ce68d041141626a6b0242639b13eab33f0`.
Its helper and internal children are accepted. Run only
`WP-4-7A-rules-cc-link-build-variables-complete-loading-proof` under
0/700/700, prove its complete imported/struct/dictionary/function inventory
and invoke nothing. Zabel remains peer ownership guidance; Bazel 9.2 and
authenticated rules_cc bytes own compatibility.

### Complete link-build variables accepted; finalizer selected (2026-08-27)

Commit `3b82f098c` freezes all 392 authenticated link-build-variable lines over
two actual complete children. It proves four imported identities, all 24 named
struct mappings, all four named dictionary mappings, five function
types/visibility and exact eight-public/eleven-all inventories without
invocation. All required suites pass at 0 production and 530 proof additions;
independent review returned `ACCEPT`.

All eight children of `link/finalize_link_action.bzl`, 469 lines, SHA-256
`adc6ea3b355d0c5e5fbf1b1e9eaa7d7dd7c0c095234a0cff7fdb4fc72eb167c9`,
are accepted. Run only
`WP-4-7A-rules-cc-finalize-link-action-complete-loading-proof` under
0/800/800, prove its complete fourteen-import/six-function inventory and invoke
nothing. Zabel remains peer ownership guidance; Bazel 9.2 and authenticated
rules_cc bytes own compatibility.

### Complete link finalizer accepted; C++ link action selected (2026-08-27)

Commit `aa797d082` freezes all 469 authenticated finalizer lines over eight
actual complete children. It proves all fourteen imported identities, six
function types/visibility and exact thirteen-public/twenty-all inventories
without invocation. All required suites pass at 0 production and 678 proof
additions; independent review returned `ACCEPT`.

The first direct consumer is `link/cpp_link_action.bzl`, 273 lines, SHA-256
`0cbe9d6b0ce0f6bea5abe1d9783b79435f495ba93bdaf402ad9539513a82223f`.
All eight children are accepted. Run only
`WP-4-7A-rules-cc-cpp-link-action-complete-loading-proof` under 0/600/600,
prove its complete eleven-import/two-function inventory and invoke nothing.
Zabel remains peer ownership guidance; Bazel 9.2 and authenticated rules_cc
bytes own compatibility.

### Complete C++ link action accepted; LTO indexing action selected (2026-08-27)

Commit `8daf80a2c` freezes all 273 authenticated C++ link-action lines over eight
actual complete children. It proves all eleven imported identities, two
function types/visibility and exact ten-public/thirteen-all inventories without
invocation. All required suites pass at 0 production and 454 proof additions;
independent review returned `ACCEPT`.

The direct parent is complete `link/cc_linking_helper.bzl`. Recursive source
order reaches accepted `cpp_link_action.bzl` and `create_library_to_link.bzl`,
then first lacks `link/lto_indexing_action.bzl`, 288 lines, SHA-256
`03cb57e972bb7503d665ca56340a34fff3e6289f9c7a168ca87a427e57c66863`.
All seven of its children are accepted. Run only
`WP-4-7A-rules-cc-lto-indexing-action-complete-loading-proof` under 0/625/625,
prove its complete nine-import/two-function and nine-public/eleven-all
inventories, and invoke nothing.

Pinned Bazel 9.2 and authenticated BCR rules_cc bytes remain exact authority.
Clean `../zabel` commit `0795445f…` is concept-only peer guidance for separating
the generic evaluator from Bazel host APIs and retaining frozen values with
their defining module; copy no Zig implementation or claimed behavior.

### Complete LTO indexing action accepted; linking helper selected (2026-08-27)

Commit `99d9289da` freezes all 288 authenticated LTO-indexing-action lines over
seven actual complete children. It proves all nine imported identities, two
function types/visibility and exact nine-public/eleven-all inventories without
invocation. All required suites pass at 0 production and 420 proof additions;
independent review returned `ACCEPT`.

The direct complete parent is `link/cc_linking_helper.bzl`, 675 lines, SHA-256
`c45dd243835bd70803a7bb2e0a11167c9ea5ac912b3f02d415841879873b2a03`.
Its eight source-order children are accepted: skylib paths, helper,
`cc_internal`, compilation outputs, C++ link action, library-to-link creator,
LTO indexing action and target types. Run only
`WP-4-7A-rules-cc-linking-helper-complete-loading-proof` under 0/950/950,
prove its complete fourteen-import/eight-function and
twelve-public/twenty-two-all inventories, and invoke nothing.

Pinned Bazel 9.2 and authenticated BCR rules_cc bytes remain exact authority.
Clean `../zabel` commit `0795445f…` remains concept-only guidance for the
generic-evaluator/Bazel-host split and producer-owned frozen module lifetime;
copy no Zig implementation or claimed behavior.

### Complete linking helper accepted; linking-context producer selected (2026-08-27)

Commit `233cdf9ef` freezes all 675 authenticated C++ linking-helper lines over
eight actual complete children. It proves all fourteen imported identities,
eight function types/visibility and exact twelve-public/twenty-two-all
inventories without invocation. All required suites pass at 0 production and
862 proof additions; independent review returned `ACCEPT`.

Public `cc/private/cc_common.bzl` first loads
`link/create_linking_context_from_compilation_outputs.bzl`, 137 lines, SHA-256
`664a461564abd348111d791aa03da0207fe158620d276b6da1936f8abb23be59`,
before its other direct linking-helper consumer `link.bzl`. All five children
of the first producer are accepted: CcInfo, `cc_internal`, linking helper,
linker-input creator and target types. Run only
`WP-4-7A-rules-cc-create-linking-context-complete-loading-proof` under
0/400/400, prove its complete seven-import/one-function and
seven-public/eight-all inventories, and invoke nothing.

Pinned Bazel 9.2 and authenticated BCR rules_cc bytes remain exact authority.
Clean `../zabel` commit `0795445f…` remains concept-only guidance for the
generic-evaluator/Bazel-host split and producer-owned frozen module lifetime;
copy no Zig implementation or claimed behavior.

### Complete linking-context producer accepted; linkstamp producer selected (2026-08-27)

Commit `da0d9a5a5` freezes all 137 authenticated
`create_linking_context_from_compilation_outputs.bzl` lines over five actual
complete children. It proves all seven imported identities, one function and
exact seven-public/eight-all inventories without invocation. All required
suites pass at 0 production and 279 proof additions; independent review
returned `ACCEPT`.

Private `cc_common.bzl` source order next reaches complete 44-line
`link/create_linkstamp.bzl`, SHA-256
`8d5fc394e31c5f0eb8a84f5020f35e71f90cdbf89591e44d1c0da8a8899e6000`.
Its helper child is accepted. Its eager surface is one public imported
function, one private provider declaration and one public lazy function. Run
only `WP-4-7A-rules-cc-create-linkstamp-complete-loading-proof` under
0/250/250, retain the complete source and exact identities, and invoke nothing.

This remains generic Starlark loading/evaluation against authenticated BCR
rules_cc, not C++-specific parsing. Rules_cc owns the Starlark rule/module
logic; Bazel host primitives consumed by those modules remain a distinct Rust
host boundary. Pinned Bazel 9.2 and authenticated rules_cc bytes remain exact
authority. Clean `../zabel` commit `0795445f…` remains concept-only guidance
for the generic-evaluator/Bazel-host split and producer-owned frozen module
lifetime; copy no Zig implementation or claimed behavior.

### Complete linkstamp producer accepted; link producer selected (2026-08-27)

Commit `6959f0370` freezes all 44 authenticated `create_linkstamp.bzl` lines
over its actual helper child. It proves the imported identity, private provider,
public function and exact two-public/three-all inventories without invocation.
All required suites pass at 0 production and 119 proof additions; independent
review returned `ACCEPT`.

Private `cc_common.bzl` source order next reaches complete 197-line
`link/link.bzl`, SHA-256
`666e819dee4777d0c3d8624e18588a905046532a6668d89d5744419cbee4a0e2`.
Its four children are accepted. Its eager surface is five imported identities,
one private five-entry target-type dictionary and one public lazy function. Run
only `WP-4-7A-rules-cc-link-complete-loading-proof` under 0/450/450, retain the
complete source and exact identities/rows, and invoke nothing.

This remains generic Starlark loading/evaluation against authenticated BCR
rules_cc, not C++-specific parsing. Rules_cc owns the Starlark rule/module
logic; Bazel host primitives consumed by those modules remain a distinct Rust
host boundary. Pinned Bazel 9.2 and authenticated rules_cc bytes remain exact
authority. Clean `../zabel` commit `0795445f…` remains concept-only guidance
for the generic-evaluator/Bazel-host split and producer-owned frozen module
lifetime; copy no Zig implementation or claimed behavior.

### Canonical external package-loading adapter design (2026-08-27)

Commit `85593f300` accepts the apparent-free canonical source/load route. The
next caller audit found that loading's external subtree, external `.bzl` cycle
identity and repository package load all retain `RootRepositoryRoute`, so a
selected canonical repository without a root alias still stops before package
loading. Do not fabricate that alias or retype the root route.

The accepted design target is a loading request-address enum over the existing
full root route or workspace plus canonical repository. Canonical evaluation
computes the accepted canonical load route first, then uses the generalized
Bzlmod source/policy carrier owned by the prerequisite Stage A packet.
Cross-repository `.bzl` loads resolve through the declaring canonical mapping,
compute the child canonical load route and merge child route/effect epochs
before source/module epochs. Root constructors, cycle identity and callers
remain exact. Activate no Rust until the cross-crate design packet is reviewed;
then Stage B follows only after Bzlmod Stage A is accepted.

Bazel 9 BCR Starlark owns rules and control flow including `cc_internal`;
`cc_common` remains a generic host-builtin consumer. Zabel supplies peer
ownership/compact-carrier guidance only, while Bazel 9.2 owns behavior.

Stage A implementation preflight then exposed a fixed-value mismatch: root
source keys return `HostRepositorySourceFileValue`, canonical keys return the
built-in-capable `HostRepositorySourceObservation`, and loading/core exhaustively
consume the former outside the allowlist. No Rust changed. Design the shared
zero-copy source result and its consumer migration before either adapter stage;
retain the canonical wrappers meanwhile.

The follow-up audit narrowed that prerequisite: the shared zero-copy carrier
already exists as `HostRepositorySourceObservation`, so loading/core result
consumers do not migrate. Generalize only its owner over Root/Canonical input,
add the observed sibling, and preserve old root source-file keys unchanged.
Temporary canonical wrappers delegate through the owner and remain until the
corrected Bzlmod policy stage migrates its callers and tests. This prerequisite
does not parse or implement C++ rules: Bazel 9 BCR Starlark, including
`cc_internal`, remains the rule layer; `cc_common` exercises the generic host
ABI. Zabel informs ownership and compact-retention choices only.

Commit `9764f8a4f` accepts that prerequisite with zero-copy catalog/materialized
payloads, exact root compatibility and the observed resolution-before-file
epoch. Corrected Stage A is now active under the current manifest. It retains
one Root/Canonical source-route carrier across path/listing, REPO, ignore,
package boundary and selected BUILD source, then deletes all four temporary
canonical wrappers. Root constructors keep their accepted children and order;
canonical policy uses the shared observation owner without an apparent alias.
Stage B external subtree/`.bzl`/package loading remains read-only until this
packet is accepted. Bazel 9 BCR Starlark still owns `cc_internal`; `cc_common`
remains a generic host-ABI consumer and Zabel remains peer guidance only.

### Root-context repository load-route publication selected (2026-09-01)

The authentic rules_rust replay now proves one missing adapter above the
accepted Root/Canonical source carrier: a root BUILD direct `.bzl` load still
uses Bzlmod's selected/direct-local root route, so an imported extension-
generated apparent name stops before the canonical load route. Loading is the
natural shared owner because it already owns canonical apparent mapping,
generated definition/effect composition and external-Bzl/package consumers.

Design one admission-aware `HostRootRepositoryLoadRoute` key family over
workspace plus nonroot apparent name. It preserves ordinary versus root-BUILD
admission, returns the existing root source route when available, and only on
the exact Unknown/Unsupported polarity resolves the producer-recorded
canonical target and requires the existing canonical load route to be
Generated. Missing or mapped non-generated fallback restores the original
route error. Root direct `.bzl`, external query and exported-source build must
consume the same value; core's temporary generated-package bridge is deleted.
No apparent alias, mapping copy, generated-effect duplicate, parser/ruleset or
C++ special case is permitted.

Pinned Bazel 9.2 `ModuleExtensionResolutionTest.simpleExtension` and
`generatedReposHaveCorrectMappings` own direct-load/mapping behavior. Zabel
`0795445f...` composed repository sources and selected package-source consumer
are ownership guidance only. Independent public-DICE architecture review
returns `ACCEPT`; implementation is active only within the frozen shared-route
packet.

### Repository-rule file-admissibility category selected (2026-09-01)

Commit `75fad534c` accepts the shared root route. Its authentic replay next
loads verbatim Bazel-tools `git.bzl`, where the existing repository-rule filter
rejects `build_file = attr.label(allow_single_file = True)` after the general
attr constructor has already produced the correct compact policy.

Select
`WP-4-5-7A-repository-rule-file-admissibility-category-implementation-r2`.
Retain the existing `FileAdmissibility` value through the frozen repository
definition for all five label-bearing constructors; do not add a second parser,
target/file resolution, repository effect, private-attribute name semantics or
consumer special case. Pinned Bazel 9.2 owns behavior. Clean Zabel
`0795445f...` supplies only the declaration-schema/explicit-invocation ownership
lesson. Independent retained-representation review is required before Rust.

Independent review returns `ACCEPT`; the existing compact policy, frozen
repository-definition owner, no-resolution phase and proof/cap boundary are
accepted. Implementation is active only within the current packet.

The pre-Rust R2 correction adds only
`host_package_load_tests.rs` for two stale repository-rule `allow_files`
rejection expectations discovered after R1 review. Focused correction rereview
returns `ACCEPT`; the retained owner, no-resolution phase, production
allowlist, caps and all neighboring rejection boundaries remain unchanged.

Implementation and independent terminal review return `ACCEPT`. The existing
compact policy now participates in frozen repository-definition, repeated-call
and DICE identity across the complete admitted category; complete loading and
query validation pass. The authentic replay clears Bazel-tools `build_file`
and selects the separate generic `@rust_host_tools` label-default boundary.

### Package-context label-string category selected (2026-09-01)

Commit `95b4f0da6` accepts the repository-rule file-admissibility category and
its authentic replay selects the generic `@rust_host_tools` descriptor-default
boundary. Activate docs-only
`WP-4-5-7A-package-context-label-string-category-design-r2`, covering the
complete Bazel 9.2 package-context dependency-label string grammar across
`Label()`, all five dependency constructors/default shapes, ordinary BUILD
values, explicit module tags, explicit repository-rule values and toolchain
declarations.

One pure borrowed spelling owner in `slug_identity_v2::label` must project to
the existing `CanonicalLabel`; loading retains the defining `.bzl`, BUILD
package, calling module and extension-evaluation mapping decisions. Add no
second label representation, DICE key, I/O, target/file resolution, output,
load, transition, command-pattern, ruleset or C++ behavior. Bazel 9.2 is
authority. starlark-rust and clean Zabel `0795445f...` provide only shared-parser
and syntax/mapping-separation guidance. Independent architecture review is
required before Rust.

R1 review returned `REPLAN` without rejecting the shared owner. R2 explicitly
separates `@//` mapping from `@@//` main-repository bypass, rejects distinct raw
label-keyed dictionary keys that canonicalize alike, and inventories every
admitted `PackageRecorder` consumer: generic/symbolic-macro attrs and selector
keys, visibility/package metadata, package groups, alias/filegroup/test-suite,
config-setting and constraint/platform/toolchain declarations. It also splits
ordinary extension repository calls from innate calls. Focused R2 rereview is
required before Rust and returns `ACCEPT`; implementation may proceed only
through the shared parser and inventoried conversion paths.

Terminal R2 implementation review returns `REPLAN`. Corrected R3 keeps the
shared parser but requires ordinary extension explicit strings to use the
selected extension evaluation `.bzl` package even when their repository rule
is imported, restores pre-packet output parsing/policy at BUILD, tag and
repository-call positions, and adds the existing ordinary BUILD integration
test to the proof allowlist. Descriptor defaults still use the repository-rule
definition context, ordinary explicit strings still use the full generated
namespace, and innate explicit strings still use the repository-rule `.bzl`
package plus calling-module mapping. No compatibility class, retained owner,
DICE boundary, production allowlist or cap changes. Rust resumes only after
focused R3 rereview returns `ACCEPT`.
Focused R3 architecture rereview returns `ACCEPT`; Rust may resume only within
the corrected ordinary-base, deferred-output and BUILD-proof boundaries.

Terminal R3 implementation review returns `REPLAN`: imported repository-rule
outputs still used the extension evaluation base and dependency parser's
special-main-package handling. R4 keeps the accepted shared dependency parser
unchanged but selects the repository-rule definition package and pre-packet
parser for `Output`/`OutputList`, including definition-repository
`//conditions`/`//visibility`. Add one imported-rule output discriminator; no
other owner, compatibility class, allowlist, cap or gate changes. Focused R4
architecture rereview is required before Rust resumes.

Focused R4 architecture and terminal implementation rereviews return `ACCEPT`.
One borrowed spelling parser now projects every admitted dependency-label
consumer to the existing `CanonicalLabel` with caller-owned context; deferred
outputs retain their old routes. Full loading/query validation and fresh
rules_rust replay pass the label boundary and select the separate generic
built-in-catalog `glob()` stop in `@@bazel_tools//tools/res`.

### Repository-source glob routing selected (2026-09-01)

The next bounded packet preserves the accepted `GlobPattern`, attempt retry,
recursive traversal and final package projection. Host sources keep their
existing raw-name, symlink and observed-path segment keys. Built-in catalog
sources select a distinct traversal scope whose segment membership comes from
the existing canonical-repository directory-listing key; both external source
kinds share the existing package-boundary key. The catalog path is source-
neutral across Rust targets while non-Unix Host traversal remains deferred.

Bazel 9.2's already-accepted glob sources/tests and fixtures remain authority;
no new oracle is needed. Clean Zabel `0795445f...` supplies only the peer lesson
that one package-glob producer visits source-owned directory facts. Add no
second parser/traversal, catalog copy, materialization, DICE key, cache,
ruleset, toolchain, C++, `cc_common` or `cc_internal` branch. Independent
DICE/ownership review is required before Rust.

Initial review returns `REVISE` only on proof precision. The corrected packet
requires an integrated catalog traversal that activates and obeys the
`src/tools/launcher/util` package boundary, defines catalog-name lifting as
valid Unicode scalars U+0000..U+00FF to equal bytes, rejects both invalid name
classes plus symlink/unknown kinds before publication, and narrows exact output
claims to named catalog packages. Focused correction rereview is required.
Focused correction rereview returns `ACCEPT`; Rust may proceed only through the
corrected source-routed traversal and proof boundary.

R1 implementation ends `REPLAN` because its proof required complete
`tools/res` package publication after the globs. The catalog globs now succeed,
then the existing `toolchain()` schema rejects `toolchain_type` as a `Label`
where Slug expects `str`. R2 changes no glob owner or production route: prove
the exact raw match slices through the shared request adapter and treat the
later package/declaration publication as deferred. Focused R2 correction
review is required before Rust resumes; no toolchain or consumer special case
is authorized.

R2 passes the complete serial owner/direct-dependent gates and an authentic
rules_rust workspace replay at 228/302/530 gross production/proof/total Rust
lines. Catalog glob routing is implemented; successful `tools/res` package
publication remains deferred at the later generic `toolchain_type` Label-versus-
string schema boundary. The next packet must audit that complete builtin
parameter category rather than specialize this package or consumer.

### Native builtin label-like direct parameters selected (2026-09-02)

Commit `bf509cd8b` accepts repository-source glob routing, and its authentic
replay selects the generic direct native parameter boundary when
`toolchain_type` receives a preconstructed `Label`. Select docs-first
`WP-4-7A-native-builtin-label-like-parameter-category-design-r1`.

Pinned Bazel 9.2 `BuildType.LabelType` preserves an existing `Label` and sends
only strings through the package converter; list, nodep-list and label-keyed
dictionary shapes recurse through that same owner. Audit the complete admitted
direct parameter inventory across package metadata, package groups, alias,
test-suite, config-setting, constraint/platform and toolchain declarations.
Reuse Slug's existing `RawAttributeValue`/`RawLabelContext::Package` conversion
and sole `CanonicalLabel` result. Add no parser, mapping policy, DICE key,
retained representation, selector/configured behavior, output/path grammar,
ruleset, toolchain-selection, C++, `cc_common` or `cc_internal` special case.

The pinned generic conversion source/tests plus the accepted verbatim catalog
replay are the exact evidence basis; no new oracle fixture is selected.
Independent architecture review must accept the inventory, defining-versus-
calling-package ownership discriminator, collision/failure proof, allowlist,
caps and stops before Rust begins.

Initial review corrected the pinned Alias/ToolchainType/PackageGroup source
filenames, required canonical duplicate rejection for
`default_package_metadata`, and removed Slug's target-pattern-name filter from
ordinary attribute labels. Focused correction rereview returns `ACCEPT`; Rust
may proceed only within the corrected packet.

Implementation and the focused terminal correction rereview return `ACCEPT`.
All direct BUILD/native facade adapters now share the existing package-context
raw coercer, preserve typed defining-label identity, reject collisions and
invalid nested values before publication, and keep wildcard-like ordinary
target names separate from target-pattern APIs. The packet closes at
186/315/501 gross production/proof/total additions with complete serial
loading, Bzlmod, query, CLI-build, formatting and lifecycle gates passing.

The isolated authenticated rules_rust replay clears the prior
`toolchain_type` Label-versus-string boundary and next stops at the generic
missing predeclared `analysis_test_transition` symbol in
`@@bazel_skylib+//lib:unittest.bzl`. Audit that complete Bazel 9.2 category
docs-first. Do not add a bazel_skylib, rules_rust, toolchain or consumer branch,
and do not imply analysis-test execution semantics before they are owned.

### Analysis-test transition category audited (2026-09-02)

The complete pinned Bazel 9.2 audit separates the globally predeclared
`analysis_test_transition(settings = ...)` constructor from the BUILD-only
`testing.analysis_test` factory and from ordinary callback-backed transitions.
The constructor owns a fixed literal patch with no inputs, callback or split;
its arbitrary values and canonical outputs are semantic identity. Bazel permits
it only on attributes of `rule(analysis_test = True)` rules, whose configured
semantics additionally require a distinct configuration marker, no registered
actions, `AnalysisTestResultInfo`, nested-test rejection and the transitive
dependency cap.

Select docs-first
`WP-4-7A-analysis-test-transition-loading-declaration-design-r1`. The bounded
exact surface is BZL-only predeclaration, signature/key validation, canonical
output ordering, defining-module package/mapping context, repr/freeze,
transient attribute
descriptor retention and exact rejection when an ordinary Slug rule consumes
the descriptor. Stop before `RuleAttributeSchema`, package publication or DICE
semantic state so arbitrary literal settings are never compared by pointer or
omitted from equality. Do not reuse the regular callable transition type.

The accepted frozen-transition lifetime pattern supplies evaluator/frozen heap
ownership and immutable compact output slices. No new Stage 9 extraction row is
needed unless review changes that decision. Full analysis-test rule/configured
execution, BUILD-only `testing.analysis_test`, provider/action enforcement,
nested-test prevention and the dependency cap remain unsupported/deferred.
Independent architecture review must accept the lifetime, fail-closed boundary,
proof, allowlist, caps and stops before Rust.

Initial review corrected raw-dictionary duplicate ownership and deferred native
option existence checks. A focused source rereview confirms that the first
validation phase still requires absolute build-setting labels and only the
analysis-test native-option policy differs from ordinary transitions. It
returns `ACCEPT`; Rust may proceed only inside the frozen loading-only packet.

### Analysis-test transition loading declaration accepted (2026-09-02)

Terminal implementation rereview returns `ACCEPT`. The exact BZL-only
constructor retains arbitrary settings on its live/frozen module heap, a
canonical immutable output slice and defining-module identity. It is distinct
from callback transitions and never reaches rule schema, package, DICE or
configured state. All five dependency constructors retain it through two-hop
import; ordinary rules use Bazel's pinned rejection, and macro, subrule,
aspect, repository-rule and tag-class consumers fail closed.

The packet closes at 152/265/417 gross production/proof/total Rust additions,
with no function over 100 lines. Focused proof, all loading integrations,
596 Bzlmod units, 55 query-library units, rebuilt CLI, formatting, hygiene and
the unchanged archive baseline pass. The authentic rules_rust replay clears
the missing global and stops at
`@@bazel_skylib+//toolchains/unittest:BUILD` on the generic
`package(default_applicable_licenses = ["//:license"])` parameter.

### Applicable-license loading category audited (2026-09-02)

Pinned Bazel 9.2 `PackageArgs`, `PackageCallable`, `RepoFileGlobals`,
`RuleClass`, `AttributeProvider`, `BaseRuleClasses`, `MacroClass` and
`StarlarkRuleClassFunctions` close the category. Package input
`default_applicable_licenses` and rule input `applicable_licenses` are aliases
for the sole canonical `default_package_metadata` field and `package_metadata`
slot. Package aliases reject simultaneous use; rule aliases rewrite before
schema lookup, ignore `None`, and keep the last non-`None` value when both
spellings occur. Only the canonical name is stored and queried.

The complete trace also bounds the claim. `MacroClass` rejects unknown
keywords before shared attribute population, so symbolic macros do not admit
the alternate rule spelling. Platform, constraint-setting/value and
materializer/dependency-resolution classes lack the canonical slot. Bazel's
`rules_license` repository-name special case suppresses package metadata
defaults to avoid self-edges. Slug's dormant REPO.bazel evaluator discards all
`repo()` keyword values, and Slug currently admits repeated BUILD package
calls. Those independent categories remain unsupported/deferred rather than
being hidden inside an alias patch.

Select
`WP-4-7A-applicable-licenses-loading-alias-design-r1`. Implement only immediate
canonicalization at existing BUILD package, admitted Starlark-rule and native-
rule ingress. Reuse the existing package recorder, repository-mapped label
coercer, immutable label slice, canonical native/Starlark schema slots and
package-load DICE owner. Add no retained spelling, metadata slot, registry,
cache, key, parser, fixture, ruleset or configured consumer branch.

`RuleClassTest.testPackageMetadataAlternateName` and the accepted Stage 4
package-metadata matrix are sufficient pinned-source evidence. The active
packet owns the exact/Slug-native/deferred classifications, last-non-`None`
and macro-rejection proof, 90/230/320 line caps, one-file production allowlist
and terminal stops. Audit and architecture return `ACCEPT`; bounded Rust is
authorized without a Skylib or toolchain special case.

### Applicable-license loading aliases accepted (2026-09-02)

Terminal implementation rereview returns `ACCEPT`. Both BUILD package facades
canonicalize `default_applicable_licenses` before the existing package state;
admitted native and Starlark rules canonicalize `applicable_licenses` before
schema lookup. Only the existing immutable default slice and canonical
`package_metadata` attribute survive. Explicit empty, omitted/`None`, last-
non-`None`, duplicate, absent-schema and macro-rejection proofs pass. The typed
package binding does not retain raw keyword order, so malformed dual-spelling
diagnostic precedence is Slug-native rather than an exact claim.

The packet closes at 66 gross added and 23 removed production Rust lines, 214
proof lines and 280 gross additions total. Focused proof passes 2/2; loading
passes 512 active library units plus one ignored and all integration targets
(51/29/8/6/2/1/5/1). Bzlmod passes 596/596 and query-library passes 55/55.
Rebuilt CLI, formatting, diff, process hygiene and the unchanged archive
baseline pass.

The authenticated rules_rust replay clears the Skylib package alias and
reaches rules_cc toolchain registration row 8. The next generic stop is
`Label()` inside a `.bzl`-defined repository-rule implementation: the
repository evaluator lacks the defining BZL label/mapping context and rejects
the constructor as outside a `.bzl` module. Run docs-only
`WP-4-7A-repository-rule-label-constructor-context-audit` next. Audit the
complete Bazel 9.2 context and lifetime category; add no rules_cc, toolchain or
repository-name special case.

### Repository-rule Label runtime context audited (2026-09-02)

Pinned Bazel 9.2 `StarlarkRuleClassFunctions.label`, `BazelModuleContext` and
`RepositoryFetchFunction` establish that repository execution does not create
a new Label owner. `Label()` uses the package and mapping of the innermost
executing Starlark function. An isolated Bzlmod oracle discriminates a direct
call in `//defs:ext.bzl` as `@@//defs:direct_target` from a call inside its
imported `//helper:support.bzl` helper as `@@//helper:helper_target`.

Select
`WP-4-5-7A-repository-rule-label-constructor-context-implementation-r1`.
The authenticated repository-file-effect owner already reacquires the exact
frozen definition module and recursive manifest. Pass that manifest by borrow
to the synchronous invocation and nest the existing `BzlEvaluationContext` in
the existing invocation-only repository state. Reuse the accepted caller-
source resolver for direct functions, imported helpers, builtin aliases and
per-module mappings. Add no retained field, DICE key, label resolver, mapping,
I/O path, lock, rules_cc or toolchain branch.

Exact compatibility is the existing Label grammar/idempotence under the
innermost function's package and mapping during admitted selected repository
effects. Flat manifest lookup, runtime-extra composition, diagnostics and DICE
cutoff are Slug-native. Wider Label/repository_ctx/repository-rule surfaces,
native rules, mapping-recorder identity, materialization, lockfiles and
configured semantics remain unsupported/deferred. The active manifest owns
proof, caps and terminal stops; audit and architecture return `ACCEPT`.

### Repository-rule Label runtime context accepted (2026-09-02)

Terminal rereview returns `ACCEPT`. The repository invocation state now nests
the existing manifest-derived BZL context, and the shared projector preserves
the innermost direct/imported function's package and mapping. No retained
definition, call, certificate, manifest, key or effect shape changes.

The implementation closes at 13/150/163 production/proof/total gross Rust
additions. Seven focused repository-context tests and all loading, Bzlmod and
query gates pass. The rebuilt authentic replay clears the Label constructor and
next rejects the independent `repository_ctx.path(Label(label))` method. Stage
5 owns the docs-only path audit; no Label grammar or rules_cc branch follows
from this acceptance.

### Repository-context path audit replans to a dedicated Label-path owner (2026-09-02)

Pinned Bazel 9.2 separates Label construction from filesystem projection.
`repository_ctx.path(Label)` first requires the Label package to exist, then
returns the lexical package-root path without inspecting the target or
resolving symlinks. With the Bazel 9.2 default it does not implicitly watch the
target. Slug's existing `HostRepositoryPathKey` therefore cannot be reused: it
observes existence and performs exact symlink resolution for source reads.

The Stage 4 evaluator is synchronous while route, package and materialization
owners are DICE computations. Select docs-only
`WP-2-4-5-7A-repository-label-path-owner-design-r1`: freeze a bounded
invocation-demand/retry contract that drops the evaluator and all invocation
borrows before awaiting those owners, and return a real immutable path value
backed by their lexical materialized root. String/generated-root paths, path
filesystem methods, built-in catalog paths, symlink/template effects and all
other repository APIs remain deferred. No Rust is authorized by this audit.

### Repository Label-path evaluator bridge design accepted (2026-09-02)

Independent architecture review accepts a bounded 256-address retry bridge.
`repository_ctx.path` admits only an existing Label in this slice. A prepared
hit allocates one immutable `path` value whose equality/hash/stringification
use normalized physical path bytes; a miss returns one typed demand. The outer
repository-effect owner drops all evaluator and invocation state before DICE,
resolves the lexical path, and retries. Only terminal-attempt prints,
environment observations and file effects publish.

The address retains package/target and deliberately drops optional repository-
mapping provenance after canonical resolution. String/generated-root paths,
built-in catalog paths, path fields/methods and symlink/template effects remain
unsupported/deferred. The active packet freezes proof, caps and terminal stops;
bounded Rust may begin with no ruleset or toolchain branch.

### Repository Label-path evaluator bridge accepted (2026-09-02)

Terminal correction rereview returns `ACCEPT`. The repository evaluator now
admits only `path(Label)` through a typed unresolved-address demand and returns
an immutable, hashable `path` value on a prepared hit. Physical normalized
path bytes define Starlark equality, hash, `str` and `repr`; route namespace
remains DICE-only identity. The 256-address invocation cap, repeated-hit reuse,
multi-demand retry, final-only prints, terminal-failure prints and absence of
target observations are discriminated.

The accepted implementation changes no retained repository definition, call,
certificate, BZL manifest or effect-plan shape. Complete loading gates pass and
the rebuilt authentic rules_cc replay clears these Label-path calls before
stopping at the independent `repository_ctx.template` method. Stage 5 owns the
next docs-only audit; template behavior and ruleset special cases remain
unsupported/deferred here.
