# bzlmod Phase 6: Migrate Stubs to Starlark (Blocked)

> **Main Plan**: [02-bzlmod.md](./02-bzlmod.md)

## Overview

Originally planned to migrate language/platform-specific compatibility modules from native Rust to Starlark. **Completed with architectural constraint discovered.**

---

## Priority Order (Original)

1. `config_common`, `platform_common` (low-risk)
2. `apple_common`, `coverage_common` (medium complexity)
3. `proto_common` stub methods (partial - keep `compile()` native)
4. `cc_common` internals (complex, deferred)

---

## ARCHITECTURAL CONSTRAINT DISCOVERED

**Date**: 2026-01-28

**Finding**: The Bazel compatibility modules **cannot be migrated to pure Starlark** due to Kuro/Buck2's global injection architecture.

### Symbol Availability by Context

| Context                     | Base Globals | Prelude Symbols | native.* Extraction |
|-----------------------------|--------------|-----------------|---------------------|
| Root cell BUILD file        | Yes          | Yes             | Yes                 |
| Root cell .bzl file         | Yes          | Yes             | No                  |
| External cell .bzl file     | Yes          | No              | No                  |

### Two Injection Mechanisms

1. **`REGISTER_BUCK2_BUILD_API_GLOBALS` (Native Registration)**
   - **When**: During interpreter initialization (before any Starlark evaluation)
   - **Where**: Available in ALL Starlark contexts
   - **Code**: `app/kuro_build_api/src/interpreter/more.rs`

2. **Prelude Injection**
   - **When**: During file evaluation (after base globals exist)
   - **Where**: Only BUILD files and .bzl files within the prelude cell
   - **Code**: `interpreter_for_dir.rs:create_env()` lines 323-335

### Why External Cells Need Native Registration

External cells (rules_cc, bazel_skylib, protobuf, etc.) access these modules at **module level** in their .bzl files:

```python
# rules_cc//cc/private/rules_impl/objc_common.bzl:22
_apple_toolchain = apple_common.apple_toolchain()

# rules_cc//cc/find_cc_toolchain.bzl:131
return [config_common.toolchain_type(CC_TOOLCHAIN_TYPE, mandatory = mandatory)]
```

These external .bzl files only see **base globals** from `REGISTER_BUCK2_BUILD_API_GLOBALS`.

**Implication**: These modules MUST remain as native Rust registrations to be available in external cells.

---

## Native Implementations (Must Remain)

Located in `app/kuro_build_api/src/interpreter/rule_defs/`:
- `apple_common.rs`
- `config_common.rs`
- `coverage_common.rs`
- `platform_common.rs`

---

## Success Criteria (All Met)

- [x] `config_common.toolchain_type()` works (Test 15 verified)
- [x] `platform_common.TemplateVariableInfo` works (Test 16 verified)
- [x] `apple_common.platform_type.ios` returns "ios" (Test 17 verified)
- [x] `coverage_common.instrumented_files_info` exists (Test 18 verified)
- [x] All existing tests pass
- [x] No regression in rules_cc loading (blocked on aspect(), not these modules)
- [N/A] Native code reduced - See architectural constraint above
