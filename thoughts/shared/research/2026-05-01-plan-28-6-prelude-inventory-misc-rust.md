# Phase 28.6 Prelude Inventory — misc subtrees + Rust loaders

## Prelude subtrees

| Path | LOC | Role | Disposition | Evidence |
|------|-----|------|-------------|----------|
| prelude/abi/ | 0 | Buck2 ABI metadata (deleted Phase 7) | **remove** | Empty / non-existent post-Phase-7 cleanup; no Bazel equivalent. |
| prelude/dist/ | varies | DistInfo provider for runtime artifact tracking | **extension-only** | Lines 9-50 define `DistInfo` provider. Used by Buck2-era runtime tracking; mark @internal or move into a Kuro extension module. |
| prelude/error_handler/ | 0 | Buck2 error event handler | **remove** | Empty / non-existent post-Phase-7 cleanup; Bazel uses stdout/stderr. |
| prelude/git/git_fetch.bzl | ~70 | Fetch Git repositories via `ctx.actions` | **integrate** | Bazel-compatible generic action pattern. Used by `prelude/rules_impl.bzl:29`. Action-based, language-neutral. |
| prelude/http_archive/http_archive.bzl | ~150 | Download + extract archives via `ctx.actions.download_file` | **integrate** | Bazel-compatible generic rule. Imported in `prelude/rules_impl.bzl:30`. Move to bundled cell. |
| prelude/http_archive/extract_archive.bzl | ~100 | Archive extraction impl (untar/unzip) | **integrate** | Bazel-compatible. |
| prelude/http_archive/exec_deps.bzl | ~50 | Tool dependencies for archive extraction | **extension-only** | Internal toolchain slots for unzip/untar. |
| prelude/http_archive/unarchive.bzl | ~100 | Unarchive action implementation | **integrate** | Core archive extraction; pure action wiring. |
| prelude/ide_integrations/ | 0 | Buck2 IDE support plugins (deleted Phase 7) | **remove** | Empty / non-existent. Bazel has separate LSP integration. |
| prelude/oss/ | 0 | Buck2 OSS-specific overrides (deleted Phase 7) | **remove** | Empty. Kuro uses `is_open_source()` Rust flag instead. |
| prelude/runtime/ | 0 | Buck2 runtime library for test runners | **remove** | Empty post-Phase-7. |
| prelude/test/inject_test_run_info.bzl | ~50 | Test execution injection (ExternalRunnerTestInfo setup) | **extension-only** | Used by `sh_test.bzl` etc. Move with test infra into bundled cell test module. |
| prelude/third-party/ | 0 | Buck2 third-party manifest model | **remove** | Empty. Bazel uses bzlmod. |
| prelude/tools/ | 0 | Buck2 tool definitions | **remove** | Empty. |
| prelude/unix/ | 0 | Buck2 Unix-specific helpers | **remove** | Empty post-Phase-7. |
| prelude/windows/ | 0 | Buck2 Windows-specific helpers | **remove** | Empty post-Phase-7. |
| prelude/xplugins/ | 0 | Buck2 plugin system | **remove** | Empty. No Bazel equivalent. |
| prelude/zip_file/zip_file.bzl | ~100 | Create/extract ZIP archives | **integrate** | Bazel-compatible action. Imported in `prelude/rules_impl.bzl:32`. |
| prelude/zip_file/zip_file_toolchain.bzl | ~50 | Zip toolchain provider | **extension-only** | Internal toolchain slot for zip tool. |

## Rust loader notes

### `app/kuro_interpreter/src/prelude_path.rs`

**51 lines.** Wrapper around `ImportPath` for `@prelude//:prelude.bzl`.

**Callers:**
- `app/kuro_interpreter_for_build/src/interpreter/interpreter_for_dir.rs:572-590` — `prelude_import()` method.
- `interpreter_for_dir.rs:628` — pushes path into `implicit_imports` for BUILD/.bzl loads.

**Disposition:** **temporary-shim** → **remove**.

**Deletion condition:** After Phase 28.6 final step (when `prelude/native.bzl` and `prelude/prelude.bzl` are deleted), the prelude no longer needs to be implicitly imported. Delete the `prelude_path()` call site and the entire module.

---

### `app/kuro_interpreter/src/file_loader.rs::extra_globals_from_prelude_for_buck_files`

**Lines 116-134.** Returns `(name, FrozenValue)` pairs by scraping the `native` struct from a loaded prelude module.

**Callers:** `interpreter_for_dir.rs:400-402` only — injects prelude's `native` members into BUCK files.

**Disposition:** **remove** after Phase 28.5/28.6 completion.

**Deletion condition:** When `prelude/native.bzl` is deleted and `exported_native` is the source of truth for BUCK-only globals, delete this method and its single call site. The `prelude_env` dance in `interpreter_for_dir.rs:392-397` can also be removed, simplifying `create_env()`.

---

### `__kuro_builtins__` namespace

**Registered in:** `app/kuro_interpreter_for_build/src/interpreter/globals.rs:173-175` via `global_env.namespace("__kuro_builtins__", |x| { register_all_natives(x); })`.

**Members:** All Rust-native rules, providers, and functions registered via `register_all_natives` (which itself calls into `REGISTER_BUCK2_*` chains).

**Current usage:**
- `prelude/native.bzl:21` — sole caller. Scrapes `__kuro_builtins__` via `__struct_to_dict()`, wraps in struct, exports as `native`.
- Tests in `app/kuro_interpreter_for_build_tests/src/interpreter.rs` use `__kuro_builtins__.json.encode(...)` etc. as a stable internal namespace for testing.

**Disposition:** **restrict** → eventually **remove**.

**Rationale:** Once Phase 28.5 moves all Bazel-shaped names to `exported_native`, the only Starlark caller of `__kuro_builtins__` is `prelude/native.bzl`, which is deleted in Phase 28.6. Tests can be migrated to read names directly from their owning module.

**Deletion condition:** After (a) `prelude/native.bzl` deletion, (b) test migration off `__kuro_builtins__.X`, the namespace becomes dead code. It can either be deleted or renamed to `__kuro_internal__` to indicate non-public status while internal code transitions.

## Summary

### Tier 1: delete now (no blockers)
Empty-or-stub directories: `abi/`, `error_handler/`, `ide_integrations/`, `oss/`, `runtime/`, `third-party/`, `tools/`, `unix/`, `windows/`, `xplugins/`.

### Tier 2: integrate into bundled cell
- `git/git_fetch.bzl`
- `http_archive/http_archive.bzl` + `extract_archive.bzl` + `unarchive.bzl`
- `zip_file/zip_file.bzl`

### Tier 3: extension-only
- `dist/dist_info.bzl`, `http_archive/exec_deps.bzl`, `zip_file/zip_file_toolchain.bzl`, `test/inject_test_run_info.bzl`

### Tier 4: Rust loader cleanup (final step)
- `prelude_path.rs` — remove after `prelude/prelude.bzl` deletion.
- `file_loader.rs::extra_globals_from_prelude_for_buck_files` — remove after `prelude/native.bzl` deletion.
- `__kuro_builtins__` namespace — restrict, then remove after test migration.
