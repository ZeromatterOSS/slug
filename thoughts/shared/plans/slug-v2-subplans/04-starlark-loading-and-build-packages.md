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
