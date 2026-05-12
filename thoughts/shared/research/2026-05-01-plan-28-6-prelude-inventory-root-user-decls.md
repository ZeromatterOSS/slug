# Phase 28.6 Prelude Inventory — root + user + decls

(Audit by parallel agent a49732e18fad65c96. Covers 33 root `*.bzl` files plus `user/` and `decls/` subtrees.)

## Inventory Table

| Path | LOC | Role | Disposition | Evidence |
|------|-----|------|-------------|----------|
| **BUILD-global pipeline (the core Phase 28.6 work)** | | | | |
| prelude/native.bzl | 23 | Scrape `__slug_builtins__` into `native` struct for BUCK globals | **temporary-shim** | `native = struct()` via `__struct_to_dict(__slug_builtins__)`. Deletion condition: all surviving native members in `exported_native` of `slug_builtins/exports.bzl`. After Phase 28.5, `interpreter_for_dir.rs::create_env` injects `exported_native` directly. |
| prelude/prelude.bzl | 13 | Re-export native + serve as BUILD-global entry point | **temporary-shim** | Loads native.bzl, re-exports `native = _native`. Delete after `prelude/native.bzl` is gone and bazel_builtins_autoload covers BUILD-global injection. |
| prelude/rules.bzl | 50 | Orchestrate rule loading; expose implemented_rules for BUILD dispatch | **temporary-shim** | Loads rules_impl.bzl. Delete after native_rule_analysis.rs covers all rule types or after Phase 28.4 wrapper system fully replaces. |
| prelude/rules_impl.bzl | 94 | Aggregate Bazel-compatible rule impls (alias, genrule, filegroup, etc.) | **integrate** | Loads individual rule impls; aggregates into categorized_rule_decl_records. Move aggregation to bundled cell rule registry. |
| **Rule implementations (move to bundled cell)** | | | | |
| prelude/alias.bzl | 29 | `alias()` and `configured_alias()` rule impls | **integrate** | Pure Starlark, Bazel 9 parity. |
| prelude/command_alias.bzl | 100+ | Wrapper script generator (OS-specific logic) | **integrate** | Bazel-compatible (used by rules_cc/rules_python for tools). |
| prelude/export_file.bzl | 29 | `export_file()` rule (copy/reference modes) | **integrate** | Bazel builtin. |
| prelude/export_exe.bzl | 20+ | Executable artifact export helper | **needs-review** | If Bazel-compatible, integrate; else remove. |
| prelude/filegroup.bzl | 62 | `filegroup()` rule (symlinked/copied dir) | **integrate** | Bazel builtin. Loads artifacts.bzl. |
| prelude/genrule.bzl | 456 | `genrule()` rule (shell action, env substitution, caching) | **integrate** | Bazel 9 parity. Heavy but clean; will lose Meta-specific deps after cleanup. |
| prelude/http_file.bzl | 50+ | `http_file()` rule + shared download helper | **integrate** | Bazel builtin. |
| prelude/remote_file.bzl | 40+ | `remote_file()` rule (mvn: URI + http_file_shared) | **integrate** | Bazel-compatible. |
| prelude/sh_binary.bzl | 100+ | `sh_binary()` rule (script generation, runfiles) | **integrate** | Bazel builtin. |
| prelude/sh_library.bzl | 25 | `sh_library()` rule (collect srcs + transitive deps) | **integrate** | Bazel builtin. |
| prelude/sh_test.bzl | 40+ | `sh_test()` rule | **integrate** | Bazel builtin. |
| prelude/test_suite.bzl | 20 | `test_suite()` rule | **integrate** | Bazel builtin. |
| prelude/none.bzl | 39 | `none()` constraint-satisfier stub | **integrate** | Bazel builtin (never-satisfied config for negation). |
| **Helper/provider modules** | | | | |
| prelude/artifacts.bzl | 50+ | ArtifactGroupInfo provider + ArtifactOutputs/ArtifactExt records | **integrate** | Used by filegroup, genrule, sh_binary. Move provider to bundled cell. |
| prelude/paths.bzl | 100+ | Skylib path manipulation (basename, dirname, join, etc.) | **integrate** | Skylib-licensed; Bazel 9 parity. Export top-level. |
| prelude/asserts.bzl | 30+ | Test assertion struct (equals/true/false) | **extension-only** | Test-fixture helper. Rename `_slug_asserts` or move to test-only. |
| **Meta-internal — delete in Phase 28.6** | | | | |
| prelude/artifact_tset.bzl | 30+ | ArtifactInfo + Apple/language tags (swiftmodule, objc_modulemap, etc.) | **remove** | Apple-specific, no Bazel parity. Move tags to rules_apple. |
| prelude/attributes.bzl | 2 | Redirect tombstone (specs moved to decls/) | **remove** | Pure tombstone. |
| prelude/attrs_validators.bzl | 30+ | AttrsValidatorsInfo provider + validation spec collection | **remove** | Meta-internal validation. No Bazel parity. |
| prelude/cache_mode.bzl | 1 | CacheModeInfo provider | **remove** | Meta-internal cache policy. Used only by genrule.bzl conditional. |
| prelude/genrule_local_labels.bzl | 254 | `_GENRULE_LOCAL_LABELS` set (dwp, postprocess_bolt, uses_Eden, etc.) | **remove** | Meta-internal labels. Inline check or delete. |
| prelude/genrule_prefer_local_labels.bzl | 23 | `_GENRULE_PREFER_LOCAL_LABELS` set (large_copy) | **remove** | Same as above. |
| prelude/genrule_toolchain.bzl | 14 | GenruleToolchainInfo (zip_scrubber field) | **remove** | Move zip_scrubber logic into Rust action layer. |
| prelude/is_full_meta_repo.bzl | 10 | `read_root_config("slug", "is_full_meta_repo")` predicate | **remove** | Meta-only. Delete after callers (genrule.bzl, rules_impl.bzl) drop checks. |
| prelude/local_only.bzl | 60 | Cxx/Python toolchain-dependent execution preference | **remove** | Imports cxx/python toolchain modules (deleted in Phase 7d). Dead code. |
| prelude/package_runs_on_*.bzl (8 variants: appletvos, iossim, iphoneos, maccatalyst, macos, visionos, visionsim, watchos) | ~5 each | Apple platform constraint stubs | **remove** | Use `@platforms` constraint_setting/constraint_value. |
| prelude/is_buck2.bzl | 13 | `is_slug()` predicate | **extension-only** | Feature flag. `_slug_*` prefix; move to test/BXL only. |
| prelude/is_buck2_internal.bzl | — | (file does not exist in current codebase) | **n/a** | Verify and delete if present. |
| prelude/is_slug_internal.bzl | 9 | `yes = True` constant | **extension-only** | Move with is_buck2.bzl or delete together. |
| **prelude/user/ — Buck2 user-overlay** | | | | |
| prelude/user/all.bzl | 30 | Buck2 user customization overlay (load write_file + extract_archive) | **remove** | Bazel uses explicit module loads. Move write_file/extract_archive to bundled cell if Bazel-shaped, else delete. |
| prelude/user/rule_spec.bzl | 18 | RuleRegistrationSpec record | **extension-only** | Internal rule definition format. |
| prelude/user/write_file.bzl | 100 | write_file rule | **integrate** | Bazel-compatible file generation rule. |
| **prelude/decls/ — rule spec records** | | | | |
| prelude/decls/common.bzl | 100+ | prelude_rule record + enum constants (Abi*, Cxx*, etc.) | **temporary-shim** | Many enums are language-specific (deleted in Phase 7d). Shrink and eventually delete when rule specs become Rust-native metadata. |
| prelude/decls/core_rules.bzl | 200+ | Core rule decl specs (alias, command_alias, export_file, filegroup, genrule, etc.) | **temporary-shim** | Loaded by rules_impl.bzl. Delete after native_rule_analysis covers all rule types. |
| prelude/decls/shell_rules.bzl | 200+ | Shell rule decl specs (sh_binary, sh_library, sh_test) | **temporary-shim** | Same condition as core_rules. |
| prelude/decls/test_common.bzl | 20 | Test attribute helpers (_test_toolchain) | **extension-only** | Internal attr provider. |
| prelude/decls/genrule_common.bzl | 100 | Genrule attribute specs | **extension-only** | Internal attr generation. |
| prelude/decls/remote_common.bzl | 100 | Remote-fetch attr specs (http_file, git_fetch, remote_file) | **extension-only** | Shared attrs. |
| prelude/decls/re_test_common.bzl | 100 | RE test attrs | **extension-only** | RE opt-in attributes for tests. |
| prelude/decls/toolchains_common.bzl | 100 | Toolchain loading helpers | **extension-only** | Generic toolchain access patterns. |
| prelude/decls/(other language-specific files, if any) | — | Likely deleted in Phase 7d | **remove** | Verify via `find prelude/decls -name '*.bzl'`; delete any language-specific stragglers. |

## Removal-order graph (most important blockers)

1. **prelude/genrule.bzl deletion** requires first deleting/inlining: `cache_mode.bzl`, `genrule_local_labels.bzl`, `genrule_prefer_local_labels.bzl`, `genrule_toolchain.bzl`, `is_full_meta_repo.bzl`.
2. **prelude/rules_impl.bzl deletion** requires moving every rule impl to bundled cell.
3. **prelude/rules.bzl deletion** requires rules_impl.bzl deleted + native_rule_analysis covering all rule types.
4. **prelude/native.bzl deletion** requires Phase 28.5 covering all surviving native names in `exported_native`.
5. **prelude/prelude.bzl deletion** requires bazel_builtins_autoload fully replacing the prelude entry point.
6. **prelude/decls/{common,core_rules,shell_rules}.bzl deletion** requires all rule specs becoming Rust-native metadata.

## Suggested execution plan

### Tier 1: delete now (no blockers)
- `prelude/attributes.bzl` (tombstone)
- `prelude/artifact_tset.bzl`
- `prelude/attrs_validators.bzl`
- `prelude/local_only.bzl`
- `prelude/package_runs_on_*.bzl` (any that exist)
- `prelude/user/all.bzl`
- `prelude/genrule_local_labels.bzl` + `genrule_prefer_local_labels.bzl` + `cache_mode.bzl` + `genrule_toolchain.bzl` (after inlining what genrule.bzl actually needs)
- `prelude/is_full_meta_repo.bzl` (inline checks at remaining callers)

### Tier 2: integrate to bundled cell (Bazel-shaped rule impls)
- alias, command_alias, export_file, filegroup, genrule, http_file, remote_file, sh_binary, sh_library, sh_test, test_suite, none, write_file
- artifacts.bzl (ArtifactGroupInfo provider)
- paths.bzl (Skylib paths module)

### Tier 3: temporary shims, delete with conditions
- native.bzl (Phase 28.5 completion)
- prelude.bzl (after native.bzl)
- rules.bzl + rules_impl.bzl (after rule impls migrated)
- decls/{common,core_rules,shell_rules}.bzl (after rule specs become Rust-native)

### Tier 4: extension-only — keep but isolate
- asserts.bzl, is_buck2.bzl, is_slug_internal.bzl (rename `_slug_*`, move to test/BXL boundary)
- decls/* helpers (test_common, genrule_common, remote_common, re_test_common, toolchains_common)
- user/rule_spec.bzl
