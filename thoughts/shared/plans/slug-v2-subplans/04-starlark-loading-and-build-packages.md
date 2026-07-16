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
- glob expansion with watched directory inputs.

Each key records the file digest or watched directory state that invalidates it.

Initial concrete files:

- `app/slug_loading_v2/Cargo.toml`
- `app/slug_loading_v2/src/{keys.rs,file_discovery.rs,load_label.rs,bzl_module.rs,package.rs,glob.rs,lib.rs}`
- `app/slug_loading_v2/src/globals/{mod.rs,native.rs,package.rs,attr.rs,rule.rs}`
- `app/slug_loading_v2/tests/{build_file_loading.rs,bzl_invalidation.rs,glob_boundaries.rs,native_removed_rules.rs}`

The first DICE keys are `BzlParseKey`, `LoadLabelResolutionKey`,
`BzlModuleEvalKey`, `PackageLoadKey`, and `GlobExpansionKey`. Use
`slug_identity_v2` labels and repo mappings only; `CellPath`,
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
