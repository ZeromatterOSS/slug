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

## WP-4-8-m3-executables-rule-capability: Stage 4 Gate A (2026-07-23)

Oracle gate `c8e469f5` is landed and Sol-accepted: 32 semantic rows plus eight
Bazel-only rule-class representation rows passed Terra update/clean and root
clean runs `085202-880190`, `085213-881221`, and `085303-889108`. The explicit
`test=True, executable=False` row observes accepted syntax plus `_test`
exclusion only; Bazel's pinned `createRule`/test-base source establishes that
test still implies executable capability. Stage 4 is now the next gate.

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
Stage 8 may not start until Sol accepts this equality/invalidation boundary.
