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
l11_a003;timeout;^moderate$;//attr:l11_a003_yes,test,size=medium,timeout=moderate;//attr:l11_a003_no,test,size=short,timeout=short;yes/no;-
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
l12_a003;$allowlist_function_transition;^@@bazel_tools//tools/allowlists:function_transition_allowlist$;//attr:l12_a003_yes,normal,identity_transition,outputs=[//attr:base_string_setting],allowlist=@@bazel_tools//tools/allowlists:function_transition_allowlist;//attr:l12_a003_no,normal,no_transition,allowlist=absent;yes/no;//attr:base_string_setting,@@bazel_tools//tools/allowlists:function_transition_allowlist
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
`3352106d79edef976c998b5423b2ee6686c7c5bda9540d27b66fe6e61566faf2`
before transcribing any row.

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
