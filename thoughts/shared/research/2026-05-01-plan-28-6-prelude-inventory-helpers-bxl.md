# Phase 28.6 Prelude Inventory — helpers + bxl + toolchains + tests + docs

(Audit by parallel agent a98b442b3392beb37. Covers utils/, bxl/, toolchains/, tests/, playground/, docs/, debugging/, validation/.)

## Inventory Table

| Path | LOC | Role | Disposition | Evidence |
|------|-----|------|-------------|----------|
| **Utilities (mostly Bazel-compatible)** | | | | |
| prelude/utils/utils.bzl | 79 | Common utility functions (value_or, flatten, map_idx, dedupe) | **integrate** | Pure Starlark helpers; Bazel parity. |
| prelude/utils/expect.bzl | 147 | Assertion/validation macros | **integrate** | expect(), expect_string(), expect_list_of_strings(); validation library. |
| prelude/utils/type_defs.bzl | 168 | Type-checking utilities | **integrate** | is_string, is_list, is_dict, is_select. |
| prelude/utils/arglike.bzl | 16 | ArgLike type alias | **integrate** | Pure type def. |
| prelude/utils/materialization_test.bzl | 30 | Test materialization helper | **extension-only** | ExternalRunnerTestInfo for materialization tests; Kuro-specific. |
| prelude/utils/selects.bzl | ~50 | select() utilities | **integrate** | Configuration selection helpers; Bazel-compatible. |
| **Test infrastructure** | | | | |
| prelude/tests/test_toolchain.bzl | 30 | Test execution toolchain (TestToolchainInfo) | **extension-only** | Kuro test infra. |
| prelude/tests/remote_test_execution_toolchain.bzl | 22 | RE config (RemoteTestExecutionToolchainInfo) | **extension-only** | Internal RE orchestration. |
| prelude/tests/re_utils.bzl | 100 | RE utilities (run_as_bundle, test env setup) | **extension-only** | Kuro-specific RE integration. |
| **Empty / placeholder directories** | | | | |
| prelude/bxl/ | 0 | BXL support (Kuro extension) | **extension-only** | Empty currently. BXL is Kuro-only; reserve namespace. |
| prelude/toolchains/ | 0 | Toolchain configuration | **extension-only** | Empty; toolchain logic scattered into decls/ and language rules (which are gone post-Phase-7). |
| prelude/playground/ | 0 | Example/demo scripts | **remove** | Empty. |
| prelude/docs/ | 0 | Documentation | **remove** | Empty. |
| prelude/debugging/ | 0 | Debug utilities | **remove** | Empty. |
| prelude/validation/ | 0 | Validation helpers | **remove** | Empty. |

## Summary

- **integrate** (helpers): `utils/utils.bzl`, `utils/expect.bzl`, `utils/type_defs.bzl`, `utils/arglike.bzl`, `utils/selects.bzl` — pure Starlark, Bazel parity.
- **extension-only**: `utils/materialization_test.bzl`, all `tests/*` files, `bxl/` namespace, `toolchains/` namespace.
- **remove**: empty placeholder directories `playground/`, `docs/`, `debugging/`, `validation/`.

## Notes

1. **utils/** is the cleanest "integrate" target. The five core files (utils, expect, type_defs, arglike, selects) are pure Starlark helpers with no Buck2-specific dependencies and obvious Bazel parity. Move to `kuro_builtins/` or a new `prelude/bazel_builtins/utils/` module.
2. **bxl/** and **toolchains/** are empty today but reserve namespaces. Keep as `extension-only`.
3. **tests/** is Kuro-specific test orchestration; move behind a `_kuro_test_*` extension boundary.
4. **playground/**, **docs/**, **debugging/**, **validation/** are empty placeholders from Buck2 days; delete.
