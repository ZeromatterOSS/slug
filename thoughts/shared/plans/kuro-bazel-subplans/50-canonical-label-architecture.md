# Plan 50: Canonical Label Architecture

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Related: [Plan 15](./15-bazel-9-parity.md),
> [Plan 36](./36-extension-spoke-lazy-materialization.md),
> [Plan 37](./37-canonical-cell-prefix-routing.md),
> [Plan 38](./38-spoke-registration-without-lockfile.md),
> [Plan 44](./44-workspace-layout-parity.md)

## Status: IN PROGRESS

## Goal

Make canonical label handling a single, typed source of truth.

Kuro has repeatedly hit bugs where `@repo`, `@@repo`, `//pkg:target`,
`:target`, extension repo canonical names, and filesystem paths are
distinguished by ad hoc string checks. This plan moves label
canonicalization toward Bazel's model:

- parse labels once;
- resolve apparent repository names through a repository mapping;
- store canonical repository identity internally;
- choose display/storage forms only at API boundaries;
- use one label-to-filesystem resolver for `repository_ctx`,
  `module_ctx`, lockfile-seeded repo specs, and repository rule attrs.

## Bazel Source Of Truth

Bazel separates parse shape, apparent-name mapping, and canonical
storage:

- `src/main/java/com/google/devtools/build/lib/cmdline/LabelParser.java`
  parses label parts and records whether the repository part used
  canonical `@@repo` syntax.
- `src/main/java/com/google/devtools/build/lib/cmdline/Label.java`
  resolves apparent `@repo` through `RepositoryMapping`, while
  `@@repo` bypasses apparent-name mapping.
- `src/main/java/com/google/devtools/build/lib/cmdline/RepositoryMapping.java`
  maps apparent repository names to canonical `RepositoryName`s in a
  specific context repository.
- `src/main/java/com/google/devtools/build/lib/cmdline/PackageIdentifier.java`
  stores the canonical repository name as part of package identity.
- `src/main/java/com/google/devtools/build/lib/rules/repository/RepoRecordedInput.java`
  records repository mappings and canonical label/path forms for
  repository-rule inputs.

## Current Kuro State

Kuro already has a partial foundation:

- `app/kuro_bzlmod/src/repo_mapping.rs` defines `CanonicalRepoName`,
  `CanonicalLabel`, and `BzlmodRepoMapping::canonicalize_label`.
- `app/kuro_bzlmod/src/lib.rs` exports `CanonicalLabel`.

But canonicalization is not actually centralized:

- `app/kuro_bzlmod/src/globals.rs` separately canonicalizes MODULE.bazel
  label values.
- `app/kuro_bzlmod/src/pending_repo_cells.rs` separately canonicalizes
  lockfile-seeded repo spec labels.
- `app/kuro_interpreter_for_build/src/repository_rule.rs` canonicalizes
  repo-rule attr labels using call-stack filename heuristics.
- `app/kuro_interpreter_for_build/src/repository_ctx.rs` parses labels
  to paths with `trim_start_matches('@')` and `bazel-external` scans.
- `app/kuro_interpreter_for_build/src/module_ctx/context.rs` has an
  independent label-to-filesystem resolver.
- `app/kuro_bzlmod/src/repository_executor.rs` has another
  `build_file` label resolver.
- `app/kuro_common/src/legacy_configs/cells.rs` and
  `app/kuro_interpreter_for_build/src/starlark_repo_rule_executor_impl.rs`
  duplicate conversions between `TagValue`, `RepoAttrValue`, and
  repository context attr values.

One confusing mismatch: `CanonicalLabel::to_storage_string()` currently
emits a single-`@` form even though other code treats `@@` as the
canonical marker. This may be intentional legacy storage, but the type
API should make that distinction explicit.

## Design

### 1. Typed Parsed Label Forms

Add a single parser API in a shared crate reachable by bzlmod and the
interpreter, likely `kuro_bzlmod` unless dependency direction requires a
smaller label crate.

The parser should distinguish:

- canonical absolute label: `@@repo//pkg:target`;
- apparent absolute label: `@repo//pkg:target`;
- current-repo absolute label: `//pkg:target`;
- package-relative label: `:target`;
- command-line style relative target where supported by Bazel;
- plain path/string, when the calling API does not accept labels.

Do not use `trim_start_matches('@')` for semantic parsing. The count of
leading `@` is meaningful.

### 2. Canonical Label Type

Make canonical labels strongly typed across repository rule attrs and
lockfile/repo-spec data.

Target shape:

```rust
pub struct CanonicalLabel {
    repo: CanonicalRepoName,
    package: String,
    target: String,
}
```

Provide explicit renderers:

- `to_unambiguous_string()` -> `@@repo//pkg:target`;
- `to_bazel_canonical_string()` if Bazel-compatible display differs;
- `to_legacy_storage_string()` for temporary compatibility with current
  single-`@` storage;
- `to_path_fragment()` only for code that has already resolved the repo
  to a filesystem root.

Avoid an implicit `Display` if the desired output form is ambiguous.

### 3. Contextual Conversion Functions

Expose conversion entry points that require context:

- `canonicalize_with_repo_context(raw, current_repo, repo_mapping)`;
- `canonicalize_with_package_context(raw, current_repo, package, repo_mapping)`;
- `canonicalize_module_label(raw, module_repo, package, repo_mapping)`;
- `canonicalize_repo_rule_attr(raw, declaring_repo, declaring_package, repo_mapping)`.

These mirror Bazel's `Label.parseWithRepoContext` and
`Label.parseWithPackageContext`.

### 4. Repo Attr Storage

Change repository spec label-bearing data from raw strings to typed
canonical labels.

Candidate path:

1. Add `RepoAttrValue::CanonicalLabel(CanonicalLabel)` or replace
   `RepoAttrValue::Label(String)` once serialization is ready.
2. Serialize typed labels in an explicit canonical form.
3. Make lockfile loading canonicalize once, during decode.
4. Make repository rule execution pass typed labels into
   `repository_ctx.attr`.

Plain strings must remain plain strings. Do not infer that every string
beginning with `@`, `//`, or `:` is a label unless the Bazel API says the
attribute is label-typed or a specific repository_ctx method accepts
string labels.

### 5. Single Label-To-Filesystem Resolver

Create one resolver for external-loading contexts:

```rust
resolve_label_path(
    label: CanonicalLabel,
    project_root: &Path,
    repo_registry: &RepoPathRegistry,
) -> Result<PathBuf>
```

The resolver should:

- resolve canonical repo name to the registered module or extension repo
  path;
- trigger lazy materialization for extension spokes when needed;
- return paths without requiring the target file to exist, matching
  `repository_ctx.path(Label(...))`;
- handle main repo labels without collapsing `@//` and `@@//`;
- avoid normal-operation scanning of `bazel-external`.

Directory scanning may remain only as a guarded fallback with tracing,
for diagnosing stale state or pre-registry transition gaps.

### 6. Apply The Resolver Everywhere

Migrate these callsites to the shared parser/resolver:

- `repository_ctx.path`, `read`, `template`, `symlink`, `patch`;
- `module_ctx.path`, `read`, `which`, and related helpers;
- `repository_executor` `build_file` resolution;
- lockfile-seeded repo specs in `pending_repo_cells`;
- repo-rule attr conversion in `repository_rule`;
- toolchain and execution-platform label canonicalization in
  `legacy_configs/cells`.

## Implementation Order

1. [x] Add typed parsed-label and canonical-label APIs with tests for
   Bazel label forms.
2. [x] Change `CanonicalLabel` renderers so `@@` canonical form and
   legacy single-`@` storage form are explicit.
3. [x] Refactor `BzlmodRepoMapping::canonicalize_label` to use the new
   parser.
4. [x] Migrate MODULE.bazel tag conversion and lockfile repo-spec
   conversion.
5. [x] Migrate repository rule attr conversion.
6. [x] Introduce the shared label-to-filesystem resolver.
7. [x] Migrate `repository_ctx`, then `module_ctx`, then
   `repository_executor`.
8. [x] Remove or quarantine ad hoc `bazel-external` scanning and
   `trim_start_matches('@')` parsing.

## Progress

2026-05-08:

- Added `canonicalize_label_with_package_context` in
  `app/kuro_bzlmod/src/repo_mapping.rs`.
- Made `CanonicalLabel` expose package and target separately.
- Added explicit `to_unambiguous_string()` for Bazel `@@repo//pkg:target`
  rendering and explicit legacy single-`@` storage renderers.
- Added repo-mapping tests for `@@repo`, `@repo`, `@repo` shorthand,
  `@repo//:`, `//pkg`, `:target`, apparent repo mapping, and legacy
  lockfile `repo//pkg:target` shape.
- Migrated lockfile repo-spec canonicalization, MODULE.bazel relative
  label canonicalization, repository-rule attr canonicalization,
  `repository_ctx::resolve_label_to_path`,
  `module_ctx::resolve_label_to_filesystem_path`, and
  `repository_executor` `build_file` resolution onto the shared parser.
- Removed broad `trim_start_matches('@')` parsing from the scoped
  canonicalization paths. The remaining extension-id and placeholder
  target-label parsing now strip `@@`/`@` explicitly instead of collapsing
  arbitrary leading `@` runs.
- Added `app/kuro_interpreter_for_build/src/label_filesystem.rs` as the
  shared interpreter-side label-to-filesystem resolver used by both
  `repository_ctx` and `module_ctx`.
- Quarantined `bazel-external` directory scanning inside that resolver as a
  traced fallback after cell-path and dynamic extension-cell registry lookup.
- Replaced the remaining `repository_executor::resolve_build_file_label`
  semantic parser with the shared canonical label parser. Its normal path is
  now exact (`bazel-external/<canonical_repo>/<package>/<target>` or the main
  repo root for `//pkg:target`), with legacy `bazel-external` directory
  scanning isolated in a small traced fallback helper.
- Routed the interpreter `Label()` global through the shared canonical label
  parser. When a full module repo mapping is not available at that callsite,
  apparent repositories are resolved through the active cell alias resolver so
  `@repo` shorthand stores the canonical repo name and `@@repo` remains
  unremapped.
- Updated `use_repo_rule()` precomputed repo cells to use Bazel 9 canonical
  names (`+rule+repo` for root-module calls and
  `module+rule+repo` for dependency-module calls) while preserving the
  invocation `name` as the apparent alias.
- Registered bzlmod apparent-to-canonical aliases in the dynamic cell registry
  so `Label()` can canonicalize `@repo` in `.bzl` evaluations that do not have
  a `BuildContext`.
- Moved toolchain repo-name extraction in
  `app/kuro_common/src/legacy_configs/cells.rs` onto
  `canonicalize_label_with_package_context`. The adjacent `bazel-external`
  scan is documented as diagnostic/materialization bookkeeping, not label
  resolution.
- Search classification after item 8:
  - no remaining `trim_start_matches('@')` semantic label parsing;
  - no remaining `replace("//", "/")` label path fallback;
  - remaining `split_once("//")` / `find("//")` hits are the central parser,
    extension-id or `.bzl` import-path compatibility parsing, and placeholder
    unknown-cell label parsing;
  - remaining `bazel-external` scans are quarantined in
    `LabelFilesystemResolver`, the repository-executor fallback helper,
    external-cell materialization/copy code, symlink cleanup, or diagnostic
    materialization checks.

## Acceptance Criteria

- No semantic label parser uses `trim_start_matches('@')`.
- `@repo//pkg:target` and `@@repo//pkg:target` are different parsed
  forms until repository mapping is explicitly applied.
- Internal package/target identity stores canonical repository names.
- `RepoAttrValue` label data is typed or serialized from a typed
  canonical label, not inferred later from raw strings.
- `repository_ctx` and `module_ctx` use the same label-to-filesystem
  resolver.
- `build_file`, `patch`, `template`, `read`, and `path` handle Label
  objects and Bazel-accepted string labels consistently.
- Normal label resolution does not depend on scanning directory names in
  `bazel-external`.

## Verification

Add unit tests for:

- `@repo`, `@@repo`, `@repo//pkg`, `@@repo//pkg`, `@//pkg`,
  `@@//pkg`, `//pkg`, and `:target`;
- apparent repo mapping and unknown apparent repo behavior;
- root module labels versus external module labels;
- extension repo labels with canonical names containing `+`;
- lockfile repo specs with nested label values in strings, lists, and
  dicts;
- repository_ctx methods accepting both `Label` objects and Bazel-allowed
  string label arguments.

Add an integration check against the zeromatter workspace:

```bash
/var/mnt/dev/kuro/target/debug/kuro \
  --isolation-dir verify-canonical-label-architecture \
  build //sdk:sdk_contents
```

This plan is complete when canonicalization bugs stop being fixed by
per-repository special cases and the next failure is explainable by a
non-canonicalization subsystem.
