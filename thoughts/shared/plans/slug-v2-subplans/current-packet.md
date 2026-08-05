# Current Slug V2 Packet

Packet: `WP-6-m2-positive-first-compatible-toolchain-oracle`
Milestone: M2 successful toolchain/platform selection
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: rewrite one dormant scaffold as an exact Bazel 9.2 positive oracle
Predecessor: accepted internal string build-setting transition implementation
`dfc1705e`.

Rewrite exactly the six files under
`tests/v2_oracle/fixtures/toolchain-resolution-first-platform/`:
`fixture.toml`, generated `expected/oracle.json`, and workspace
`MODULE.bazel`, `BUILD.bazel`, `defs.bzl`, and new `cquery_format.bzl`. The
existing fixture is ungenerated action-based scaffolding and is not protected
evidence. Do not edit Rust, Cargo, the harness, another fixture, commands, or
plans in the evidence worker.

Model exactly:

1. one fixture-local constraint setting with mutually exclusive `first` and
   `second` values;
2. two execution platforms carrying those values, registered in explicit
   order while the host platform matches neither;
3. one mandatory toolchain type and two registered toolchains, each compatible
   with exactly one execution platform;
4. two Starlark toolchain implementations returning
   `platform_common.ToolchainInfo(marker = <attribute value>)`; and
5. one probe rule declaring that toolchain type and returning only
   `ProbeInfo(marker = ctx.toolchains["//:demo_type"].marker)`.

The fixture-local cquery Starlark formatter reads only the exact
`//:defs.bzl%ProbeInfo` key and emits
`label=@@//:probe provider=ProbeInfo marker=<value>`. It must not enumerate
providers or print configuration, platform, toolchain label, action, or path
identity.

Record exactly six successful commands against one retained Bazel 9.2 server:

1. the initial execution-platform registration order selects `first`;
2. an unchanged warm replay is byte-identical `first`;
3. mutating only `register_execution_platforms` order selects `second`;
4. restoring that order selects `first`;
5. mutating only the first toolchain implementation marker in `BUILD.bazel`
   yields `edited-first`; and
6. restoring the marker yields `first` again.

Every row uses `cquery //:probe --output=starlark` with the fixture formatter,
exits zero, and has exact stdout/stderr. Every row must retain zero actions and
no output or manifest observation.

Pin Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
sources for `ModuleFileGlobals#registerExecutionPlatforms`, registered
execution platforms and toolchains functions,
`PlatformKeys#findExecutionPlatformKeys`, single/multi-toolchain constraint
resolution and candidate selection, `ResolvedToolchainContext#load`, Starlark
rule/toolchain context exposure and indexing, `ToolchainInfo`, and the cquery
Starlark formatter. Generate and no-update replay with `/usr/bin/bazel` 9.2;
run fixture list, JSON, inventory/cap, provenance, credential-pattern,
archive, and diff checks and obtain independent fixture review.

Caps are exactly six regular files, zero links, 220 authored non-generated
lines, 500 total generated lines, and six commands. Rewriting this named
dormant scaffold is the bounded hygiene action for the demonstrated Stage 6
gap; add no aggregate fixture breadth.

Stop if any successful output exposes a configuration token, configured path,
platform/toolchain label, action key, mnemonic, action, output, or manifest; if
selection order cannot be proved solely by the semantic provider marker; or if
the packet needs host fallback, aliases, optional or multiple toolchain types,
target constraints, exec groups, execution properties, missing-toolchain
diagnostics, command-line registration, external repositories, public Slug
cquery, native option identity, Rust, execution, aquery, REAPI, another graph,
direct filesystem discovery, or process-global registration state.

After acceptance, design the exact root registration, native declaration,
real DICE resolution, and prepared `ctx.toolchains` ownership before
authorizing Rust.
