# Phase 28.6 Prelude Inventory — configurations + platforms + transitions

| Path | Files | Purpose | Disposition | Evidence |
|------|-------|---------|-------------|----------|
| `prelude/platforms/` | `defs.bzl` | `execution_platform` rule + `host_configuration()` helpers; reads from `prelude//cpu` and `prelude//os` hardcoded targets. Uses `host_info()` (Rust). | **temporary-shim** | Bazel-mode now generates `@local_config_platform//:host` from `app/slug_external_cells_bundled/build.rs`; deletion condition: switch all callers to `@local_config_platform`. |
| `prelude/os_lookup/` | `defs.bzl` | `os_lookup` rule with `Os` enum + custom provider; imports `core_rules.bzl` (TargetCpuType). | **remove** | Buck2-internal platform detection; no Bazel parity. Only callers are `prelude/cfg/modifier/name.bzl` (also being removed). OSS uses `@platforms` cell. |
| `prelude/cfg/modifier/` | `types.bzl`, `name.bzl` | `ConditionalModifierInfo` + `cfg_name()` for configuration hashing; modifier records for package/target/CLI/buckconfig locations. `NAMED_CONSTRAINT_SETTINGS` dict maps Meta-internal labels. | **remove** | Pure Buck2 machinery: ModifierLocation records, buckconfig-backed modifiers, pre/post-platform ordering. Bazel uses simple platforms with no hierarchical modifiers. Hardcodes `ovr_config//*`, `fbcode//*`. Not exported by `slug_builtins`. |
| `prelude/configurations/` | `rules.bzl`, `util.bzl` | Native rule impls: `config_setting`, `constraint_setting`, `constraint_value`, `constraint`, `platform`, `configuration_alias`. Helpers for merging `ConfigurationInfo`. | **integrate** | Core Bazel-compatible platform/constraint machinery. Bazel 9 parity. Move to `prelude/bazel_builtins/` with citations. |
| `prelude/transitions/` | `constraint_overrides.bzl` | `_impl()`/`_apply()`/`_resolve()` Buck2 transition logic; reads `read_root_config("slug", "platforms", ...)`. | **temporary-shim** | Heavyweight Buck2 transition system with `cfg_name()` labeling. Bazel has simpler transitions API. Deletion condition: when rules_python and integration callers switch to standard Bazel transitions. Currently imported by `prelude/rules_impl.bzl`. |
| `prelude/cpu/` | (empty / BUILD-only) | Placeholder for `prelude//cpu:arm64`, `:x86_64`, etc. | **remove** | No `.bzl` files. Bazel's `@platforms//cpu:*` provides equivalent. Only `prelude/platforms/defs.bzl` references the targets. |
| `prelude/os/` | (empty / BUILD-only) | Placeholder for `prelude//os:linux`, `:macos`, `:windows`. | **remove** | No `.bzl` files. Use `@platforms//os:*`. |
| `prelude/build_mode/` | (empty / BUILD-only) | Placeholder for build-mode constraints. | **remove** | No `.bzl` files. Bazel uses rules_cc's `--compilation_mode`. |

## Summary

- **integrate** (1): `prelude/configurations/rules.bzl`, `util.bzl` — foundational Bazel platform/constraint machinery. Move to `prelude/bazel_builtins/`.
- **temporary-shim** (2): `prelude/platforms/defs.bzl` (delete when all callers use `@local_config_platform`); `prelude/transitions/constraint_overrides.bzl` (delete when callers switch to standard Bazel transitions).
- **remove** (5): `prelude/cfg/modifier/`, `prelude/os_lookup/`, `prelude/cpu/`, `prelude/os/`, `prelude/build_mode/`.

## Cross-file dependencies

- `prelude/rules_impl.bzl:12` imports `prelude//configurations:rules.bzl` → keep configurations/.
- `prelude/transitions/constraint_overrides.bzl:9,28` imports `prelude//cfg/modifier:*` → remove modifier/ once `constraint_overrides` is replaced.
- `prelude/platforms/defs.bzl:45,58` hardcodes `"prelude//cpu:*"`, `"prelude//os:*"` → those subtrees can be removed once `platforms/defs.bzl` is replaced by `@local_config_platform`.
- `prelude/cfg/modifier/name.bzl:19-32` hardcodes Meta-internal constraint labels → remove with the file.
