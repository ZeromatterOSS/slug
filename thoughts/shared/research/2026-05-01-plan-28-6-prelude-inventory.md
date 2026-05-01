# Phase 28.6 Prelude Inventory — synthesis

Cross-cutting summary of the four sub-inventories:

- [`2026-05-01-plan-28-6-prelude-inventory-root-user-decls.md`](./2026-05-01-plan-28-6-prelude-inventory-root-user-decls.md) — root `*.bzl` (33 files), `user/`, `decls/`.
- [`2026-05-01-plan-28-6-prelude-inventory-helpers-bxl.md`](./2026-05-01-plan-28-6-prelude-inventory-helpers-bxl.md) — `utils/`, `bxl/`, `toolchains/`, `tests/`, `playground/`, `docs/`, `debugging/`, `validation/`.
- [`2026-05-01-plan-28-6-prelude-inventory-configurations.md`](./2026-05-01-plan-28-6-prelude-inventory-configurations.md) — `configurations/`, `platforms/`, `transitions/`, `cfg/`, `cpu/`, `os/`, `os_lookup/`, `build_mode/`.
- [`2026-05-01-plan-28-6-prelude-inventory-misc-rust.md`](./2026-05-01-plan-28-6-prelude-inventory-misc-rust.md) — remaining language/build subtrees + Rust loaders + `__kuro_builtins__`.

## Top-level findings

**Phase 7d already deleted ~124k lines of Buck2 language rules** (android, apple, cxx, java, kotlin, python, rust, etc.). What remains is a relatively thin layer: ~33 root `.bzl` files, `decls/` rule-spec records, `utils/` helpers, `configurations/` platform machinery, and a handful of Meta-internal stubs.

**Three classes of work in Phase 28.6:**

1. **Drop dead weight** — Meta-internal predicates (`is_full_meta_repo`, `cache_mode`, `genrule_local_labels`, etc.), Apple platform stubs (`package_runs_on_*`), tombstones (`attributes.bzl`), and empty placeholder dirs (`playground/`, `docs/`, `debugging/`, `validation/`).
2. **Integrate Bazel-shaped rule impls into the bundled cell** — alias, filegroup, genrule, sh_*, test_suite, http_file, remote_file, http_archive, zip_file, etc.
3. **Cut the prelude-driven BUILD-global path** — remove `prelude/native.bzl`, `prelude/prelude.bzl`, `prelude_path.rs`, `extra_globals_from_prelude_for_buck_files`. Final state: `@kuro_builtins//:exports.bzl` is the *only* source of BUILD/.bzl globals.

## Removal-order graph

```
                                     [Phase 28.6 final state]
                                    delete prelude/native.bzl
                                    delete prelude/prelude.bzl
                                    delete prelude_path.rs
                                    delete extra_globals_from_prelude_for_buck_files
                                              ↑
                                              | requires
                                              ↑
                            [exported_native covers all surviving names]
                                              ↑
                                              | requires
                                              ↑
        +-----------------------+   +-----------------------+
        | rule impls migrated   |   | helpers exported via  |
        | to bundled cell       |   | exports.bzl           |
        +-----------------------+   +-----------------------+
                  ↑                             ↑
                  | requires                    | requires
                  ↑                             ↑
        +-----------------------+   +-----------------------+
        | meta-internal deps    |   | utils/, paths.bzl,    |
        | (cache_mode etc.)     |   | artifacts.bzl, etc.   |
        | inlined or deleted    |   | moved unchanged       |
        +-----------------------+   +-----------------------+
```

## Tier-1: safe to delete immediately (no callers / dead since Phase 7d)

These can be deleted in a single PR with no other prerequisites:

| Path | Why safe |
|------|----------|
| prelude/attributes.bzl | Pure tombstone; says "moved to decls/". |
| prelude/artifact_tset.bzl | Apple-specific tags; no Bazel parity, language rules deleted in Phase 7d. |
| prelude/attrs_validators.bzl | Meta-internal validation framework; no callers in Bazel-shape paths. |
| prelude/local_only.bzl | Imports cxx/python toolchain modules deleted in Phase 7d; orphan. |
| prelude/package_runs_on_*.bzl | Apple platform stubs (8 variants); use `@platforms` constraints instead. |
| prelude/user/all.bzl | Buck2 user-overlay; Bazel uses explicit `load()`. |
| prelude/playground/, prelude/docs/, prelude/debugging/, prelude/validation/ | Empty placeholders. |
| prelude/abi/, prelude/dist/ (if empty), prelude/error_handler/, prelude/ide_integrations/, prelude/oss/, prelude/runtime/, prelude/test/ (if empty), prelude/third-party/, prelude/tools/, prelude/unix/, prelude/windows/, prelude/xplugins/ | Empty post-Phase-7d. |
| prelude/cpu/, prelude/os/, prelude/build_mode/ | No `.bzl` files; replaced by `@platforms` and `@local_config_platform`. |
| prelude/cfg/modifier/ | Buck2-only modifier system; no Bazel parity. |
| prelude/os_lookup/ | Buck2 platform detection; only callers are also being removed. |

## Tier-2: integrate (move into bundled cell)

These are Bazel-shaped and should land as Starlark exports in `kuro_builtins/exports.bzl` or as bundled-cell rule registrations:

**Helpers:**
- `prelude/utils/utils.bzl`, `expect.bzl`, `type_defs.bzl`, `arglike.bzl`, `selects.bzl`
- `prelude/paths.bzl` (Skylib paths)
- `prelude/asserts.bzl` (could also be `extension-only` — depends on whether Bazel `asserts` parity is wanted)

**Providers:**
- `prelude/artifacts.bzl::ArtifactGroupInfo`
- `prelude/configurations/rules.bzl` + `util.bzl` (config_setting, constraint_setting/value, platform, configuration_alias)

**Rule impls:**
- `prelude/alias.bzl`, `command_alias.bzl`, `export_file.bzl`, `filegroup.bzl`, `genrule.bzl`, `http_file.bzl`, `remote_file.bzl`, `sh_binary.bzl`, `sh_library.bzl`, `sh_test.bzl`, `test_suite.bzl`, `none.bzl`
- `prelude/http_archive/http_archive.bzl` + `extract_archive.bzl` + `unarchive.bzl`
- `prelude/zip_file/zip_file.bzl`
- `prelude/git/git_fetch.bzl`
- `prelude/user/write_file.bzl`

## Tier-3: temporary shims (delete with conditions)

| Path | Deletion condition |
|------|--------------------|
| prelude/native.bzl | All surviving native names listed in `kuro_builtins/exports.bzl::exported_native`. |
| prelude/prelude.bzl | bazel_builtins_autoload fully replaces the prelude entry point. |
| prelude/rules.bzl + rules_impl.bzl | All rule impls migrated to bundled cell registry. |
| prelude/decls/common.bzl, core_rules.bzl, shell_rules.bzl | Rule specs become Rust-native metadata or move to bundled cell. |
| prelude/genrule_local_labels.bzl, genrule_prefer_local_labels.bzl, cache_mode.bzl, genrule_toolchain.bzl, is_full_meta_repo.bzl | Inlined into genrule.bzl during integrate step, then deleted. |
| prelude/platforms/defs.bzl | Callers migrated to `@local_config_platform`. |
| prelude/transitions/constraint_overrides.bzl | Callers migrated to standard Bazel transitions. |

## Tier-4: extension-only (keep but isolate)

These survive but should move behind a `_kuro_*` naming or `@kuro_internal` cell so they're not visible to user BUILD/.bzl files:

- `prelude/bxl/` (entire subtree) — BXL is Kuro-only.
- `prelude/asserts.bzl` (test-only, if not chosen as `integrate`).
- `prelude/is_buck2.bzl`, `is_kuro_internal.bzl` — feature flags.
- `prelude/dist/dist_info.bzl` — runtime artifact tracking.
- `prelude/tests/*` — TestToolchainInfo, RemoteTestExecutionToolchainInfo, re_utils.
- `prelude/test/inject_test_run_info.bzl` — test injection helper.
- `prelude/utils/materialization_test.bzl` — Kuro-specific test helper.
- `prelude/decls/` non-core helpers — test_common, genrule_common, remote_common, re_test_common, toolchains_common.
- `prelude/user/rule_spec.bzl` — RuleRegistrationSpec record.
- `prelude/http_archive/exec_deps.bzl`, `prelude/zip_file/zip_file_toolchain.bzl` — internal toolchain slots.

## Rust loader cleanup

- `app/kuro_interpreter/src/prelude_path.rs` — remove after `prelude/prelude.bzl` deletion.
- `app/kuro_interpreter/src/file_loader.rs::extra_globals_from_prelude_for_buck_files` — remove after `prelude/native.bzl` deletion.
- `__kuro_builtins__` namespace in `globals.rs::base_globals` — restrict (rename `__kuro_internal__` or remove `register_all_natives` from the namespace) once internal tests stop reading from it. Tests in `kuro_interpreter_for_build_tests::interpreter` use `__kuro_builtins__.json.encode(...)` etc. — migrate those references first.
- `interpreter_for_dir.rs::create_env` lines 391-404 (prelude scrape + extra_globals call) — delete after both files above are gone.

## Suggested PR sequencing (≈4 PRs)

### PR 1 — drop dead weight (Tier 1)
Delete all the placeholder dirs + Meta-internal predicates + tombstones in one shot. ~10-15 files removed. No behavior change beyond removing dead code. Should be a clean diff.

### PR 2 — move helpers and providers into bundled cell
- `paths.bzl` → exported via `kuro_builtins`. Bazel users gain `paths.basename` etc.
- `utils/utils.bzl`, `utils/expect.bzl`, `utils/type_defs.bzl`, `utils/arglike.bzl`, `utils/selects.bzl` → exported similarly.
- `artifacts.bzl::ArtifactGroupInfo` → exported provider.
- `configurations/rules.bzl` → bundled cell native rule analysis (or kept as Starlark, exported via `exported_native`).

Test guard: a new `tests/core/analysis/test_native_rules.py::test_28_6_paths_helpers_visible` proves the helpers are reachable without `load("@prelude//paths.bzl")`.

### PR 3 — migrate rule impls one at a time
Each rule (alias, filegroup, sh_*, genrule, etc.) migrates as its own commit with its own acceptance test. The order is bottom-up:
1. Simple rules first: `alias`, `none`, `test_suite`, `sh_library`, `export_file` (low LOC, no deps).
2. Medium: `filegroup`, `sh_binary`, `sh_test`, `command_alias`, `http_file`, `remote_file`, `write_file`.
3. Complex: `genrule` (after Meta-internal deps inlined or deleted).
4. Archives: `http_archive`, `zip_file`, `git_fetch`.

After each rule migrates, its `prelude/<rule>.bzl` is deleted and `rules_impl.bzl` updated to drop the load.

### PR 4 — cut the prelude-driven BUILD-global pipeline
Once Tier 1-3 done and `exported_native` covers all needed names:
1. Delete `prelude/native.bzl`.
2. Delete `prelude/prelude.bzl`.
3. Delete `prelude/rules.bzl` + `rules_impl.bzl` (if all rule impls migrated).
4. Delete `prelude/decls/{common,core_rules,shell_rules}.bzl` (if rule specs no longer needed in Starlark).
5. Delete `app/kuro_interpreter/src/prelude_path.rs`.
6. Delete `extra_globals_from_prelude_for_buck_files` in `file_loader.rs`.
7. Strip the prelude-scrape block at `interpreter_for_dir.rs:391-404`.
8. Rename `__kuro_builtins__` namespace (after migrating tests).

After PR 4, BUILD-global construction is purely `@kuro_builtins//:exports.bzl` driven, and Phase 28.6 is complete.

## Acceptance criteria (Phase 28.6)

Per the plan doc:

- ✅ Ordinary `kuro build` does not need to evaluate `prelude/prelude.bzl` to construct BUILD globals.
- ✅ `prelude/native.bzl`, if still present, is a temporary shim with a deletion condition and no unique symbol ownership. → After PR 4, both files are gone.
- ✅ Every remaining file under `prelude/` has an owner: `bazel_builtins`, `bxl`, `test fixture`, or `delete`.
- ✅ `@prelude//...` loads in user BUILD/.bzl files are either unsupported with a clear Kuro/Bazel-compatibility error or explicitly documented as Kuro extension APIs.
- ✅ No Buck2 language/toolchain prelude directories are reachable from Bazel-compatible BUILD loading.

## Out-of-scope (later phases)

- Performance benchmarking of the new injection path vs the old prelude scrape.
- Deprecating `__kuro_builtins__` namespace fully (test migration is its own task).
- Documenting the user-facing migration story (a single load shim that maps `@prelude//paths.bzl` → `@kuro_builtins//paths.bzl` for backwards compat, if desired).
