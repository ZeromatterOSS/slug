# Plan 48: Small Bazel Parity Gap Backlog

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)

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
