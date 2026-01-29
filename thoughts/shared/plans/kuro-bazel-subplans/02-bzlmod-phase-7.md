# bzlmod Phase 7: Proto Support (Future)

> **Main Plan**: [02-bzlmod.md](./02-bzlmod.md)

## Overview

**Current Status**: Native `ProtoInfo` is deprecated in Bazel 8+. The approach taken in the current implementation may be incorrect.

---

## What Bazel Actually Does (Research Complete)

Per [Bazel 8.0 release notes](https://blog.bazel.build/2024/12/09/bazel-8-release.html):

- Native `*_proto_library` rules have been **moved to the protobuf repository**
- `ProtoInfo` should come from `@protobuf//bazel/common:proto_info.bzl`, not as a native builtin
- Bazel provides `--incompatible_autoload_externally` flag to automatically load rules from their repositories

---

## Current Implementation (Incorrect)

The current implementation in `app/kuro_build_api/src/interpreter/rule_defs/proto_common.rs` creates:

- `ProtoInfo` as a native builtin provider type
- `proto_common_do_not_use` module with stub methods

This mirrors the **deprecated** Bazel 7.x behavior, not Bazel 8+/9.0.

---

## Why rules_cc Still Needs It

rules_cc 0.2.16 depends on protobuf 27.0, and the load chain hits:

```
@rules_cc//cc:defs.bzl
  -> @protobuf//bazel/private/native.bzl:3
       NativeProtoInfo = ProtoInfo  <- Expects builtin ProtoInfo
```

**protobuf 27.0** still assumes `ProtoInfo` exists as a native builtin for backward compatibility during the transition period.

---

## Recommended Approach

**Option A: Bazel-compatible stub (Recommended for now)**

- Set `ProtoInfo = None` (matches how Bazel sets `CcInfo`, `DebugPackageInfo`, etc. to `Starlark.NONE`)
- Let protobuf rules fail gracefully or provide their own implementation
- Test if this breaks rules_cc loading

**Option B: Keep transitional stub**

- Keep current stub but document it as transitional
- Plan to remove once protobuf rules fully migrate

**Option C: Remove entirely**

- Remove `proto_common.rs` and let errors surface
- May break rules_cc until protobuf rules can fully load

---

## Files to Modify

- `app/kuro_build_api/src/interpreter/rule_defs/proto_common.rs` - Evaluate approach
- `app/kuro_build_api/src/interpreter/more.rs` - Update registration if needed

---

## Success Criteria

- [ ] Determine correct approach (Option A, B, or C)
- [ ] Test rules_cc loading with chosen approach
- [ ] Document decision and rationale
- [ ] `@rules_cc//cc:defs.bzl` loads completely (if possible with proto stub)
