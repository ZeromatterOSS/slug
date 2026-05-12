# AGENTS.md

Project-wide instructions for AI agents on slug.

## Bazel version target

**Bazel 9 parity only.** No back-compat for older Bazel or slug's earlier prototype behaviour.

- Bazel 9 removes symbol (`CcInfo`, `PyInfo`, `ProtoInfo` from globals) → slug removes too. No deprecation, no shim.
- Bazel 9 changes lockfile/WORKSPACE/Starlark API → slug matches exact. Not superset, not subset.
- Bazel 9 errors on pattern (native `cc_library` without `load("@rules_cc//...")`) → slug errors same message shape.
- `@bazel_tools` content: port verbatim from upstream `src/<path>/BUILD.tools`. No invention, copy exact.

## Rationale

Prototype. No external users of slug's Starlark surface. Break any slug workspace for parity — fine. No migration guides, no deprecation flags, no compat shims unless user asks.

Cite Bazel source of truth for parity decisions:

- Symbol removal: `src/main/java/com/google/devtools/build/lib/analysis/BaseRuleClasses.java` (EmptyRule pattern) + relevant `rules-*.java` registry.
- `@bazel_tools` content: `src/main/java/.../BUILD.tools` + `embedded_tools/` layout in installed Bazel.
- Lockfile format: `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/` (version, digest encoding, repo spec schema).

## "Parity" concretely

- Bazel 9 errors → slug errors, same kind.
- Bazel 9 output path → slug output path, same (modulo `bazel-out`/`buck-out`, deliberately different).
- Bazel 9 MODULE.bazel builds → slug builds, same result.
- Bazel 9 fails → slug fails. Workarounds masking a Bazel 9 failure = bugs.

## NOT in scope

- Bazel 8.x compat. `.bazelversion=8.x` → upgrade it.
- WORKSPACE files. Removed in Bazel 9. Unsupported.
- Legacy toolchain resolution. Bzlmod-only.
