# Bazel 9 Parity Source Anchors

Read only the entry relevant to the current packet.

- Removed globals such as `CcInfo`, `PyInfo`, and `ProtoInfo`:
  `src/main/java/com/google/devtools/build/lib/analysis/BaseRuleClasses.java`
  (`EmptyRule`) and the relevant `rules-*.java` registry.
- `@bazel_tools`: the upstream `src/main/java/.../BUILD.tools` source and
  installed `embedded_tools/` layout. Port content verbatim.
- Bzlmod lockfile version, digest, and repository-spec schema:
  `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/`.
- Rule availability and load errors: the Bazel 9 rule registry. For example,
  native `cc_library` without `load("@rules_cc//...")` must fail as Bazel 9
  does.

Bazel 9 removes WORKSPACE support and legacy toolchain resolution. Do not add
compatibility for them or for Bazel 8.
