# Plan 48: Small Bazel Parity Gap Backlog

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)

## Status: PROPOSED

## Scope

Own the remaining small but real parity gaps that do not currently justify
their own standalone plan. Split any item out if implementation proves
larger than expected.

| Gap | Current behavior | Target |
|-----|------------------|--------|
| `genquery` | Touch/empty-output stub | Execute Bazel query semantics for `genquery` or produce a Bazel-shaped unsupported error if a specific feature is out of scope. |
| Build status stamping | `StampFile` stubs | Implement stable/volatile status file behavior and stamp propagation needed by Bazel-compatible rules. |
| `proto_common.compile()` | No-op / stubbed action behavior | Create real proto compilation actions or move the surface to loaded protobuf Starlark exactly as Bazel 9 expects. |
| Protoc tool path | Hardcoded | Resolve via toolchain or upstream protobuf ruleset behavior. |
| `target_platform_has_constraint()` | Host-OS shortcut | Query the configured target platform constraints. |
| `CppFragment.sysroot()` | Returns `None` | Read sysroot from the selected C++ toolchain / configured fragment. |
| `CppFragment.fdo_instrument()` | Returns `None` | Reflect FDO instrumentation configuration when present. |
| `string.elems()` | Returns opaque iterator | Match Bazel 9: return a list so list operations such as slicing work (`"abc".elems()[:]`). |
| `repository_ctx.execute()` / `module_ctx.execute()` args | Accept only list | Match Bazel 9: accept list or tuple command argument sequences. |

## Bazel Source of Truth

Each sub-item must cite upstream Bazel 9 source/tests before closure:

- `genquery`: Bazel native rule implementation and query tests.
- stamping: build-info/status-file implementation and shell tests.
- proto: protobuf repository Bazel 9 integration and any Bazel
  compatibility hooks still used by rulesets.
- platform constraints: `ctx.target_platform_has_constraint` tests and
  configured-target platform APIs.

## Verification

Each item needs:

- a focused unit or integration test;
- one fixture that fails before the change and passes after it;
- a note whether Bazel source behavior was matched exactly or whether the
  item was intentionally split to a larger follow-up.

## Progress

2026-05-08:

- ZeroMatter verification after Plan 50 and Plan 46 directory-source fixes
  reached rules_python's pip extension and failed evaluating
  `content.elems()[:]` in `parse_requirements_txt.bzl`: Slug's
  `string.elems()` returned an opaque iterator that did not support slicing.
- Checked Bazel 9.1.0 behavior directly with a tiny rule:
  `type("abc".elems())` prints `list`, and `"abc".elems()[:]` prints
  `["a", "b", "c"]`.
- Updated Slug's Starlark `string.elems()` implementation to return a list
  rather than an opaque iterator. Left `string.codepoints()` unchanged for
  now; Bazel 9.1.0 reports no `codepoints` method, so removing or hiding that
  extra surface should be handled separately if it blocks Bazel 9 parity.
- The next zeromatter run reached `ape.pe` and failed on
  `rctx.execute(("cmd.exe", "/c", "@echo.%SYSTEMROOT%"))` because Slug only
  accepted lists. Checked Bazel 9.1.0 with a tiny module extension:
  `mctx.execute(("/bin/echo", "ok"))` succeeds. Updated both
  `repository_ctx.execute()` and `module_ctx.execute()` to unpack list-or-tuple
  command arguments.
