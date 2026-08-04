# Current Slug V2 Packet

Packet: `WP-5-m1-external-repository-config-setting-query`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted direct local-override external source-file, native
filegroup, and direct native alias queries; accepted root-package
`config_setting` graph projection and Bazel 9.2 `label_kind` evidence;
complete route hashing, native materialization/path retries, and end-to-end
no-legacy guards
Validation tier: one-file private query projection plus focused public
query/core and exact Bazel/Slug oracle rows

Implement only the external native `config_setting` query projection in
`app/slug_query_v2/src/graph.rs`. Reuse the ordinary `LoadedPackage`,
`PackageTargetKind::ConfigSetting`, accepted external graph value/key, retained
native capability, and existing query/render owners. Project one
`QueryNodeKind::Rule("config_setting rule")` with no ordinary edge and no
query-visible attribute, matching the accepted root unconfigured graph. The
retained `values` remain loading metadata; this packet does not interpret
configuration or activate analysis.

Canonical semantic identity and route-specific apparent output remain
unchanged. Accept only public/private visibility without a dependency label,
and project no query-visible visibility attribute. The observable surface is
the direct literal and existing `--output=label_kind` formatter. Forward
`deps` is the node itself and may be proven in focused Rust evidence without
adding a third fixture row.

Production allowlist: `app/slug_query_v2/src/graph.rs`. Tests may change only
`app/slug_query_v2/src/graph.rs` and
`app/slug_core_v2/src/runtime/dice.rs`. Oracle changes are limited to the
existing `module-local-override` fixture TOML, `workspace/dep/BUILD.bazel`, and
expected JSON; add no asset. Do not alter Cargo metadata, public APIs, DICE
keys, repository routes, loading/source owners, CLI/server adapters, protocol,
formatters, analysis, actions, execution, or another fixture.

Extend the fixture with
`config_setting(name = "is_k8", values = {"cpu": "k8"})` and exactly two
Bazel/Slug commands: the literal `@dep//:is_k8` and
`query --output=label_kind @dep//:is_k8`. Protect all existing normalized
fixture semantics. Focused Rust evidence must prove exact rule kind and
capability, no edges or attributes, canonical identity, apparent output,
self-only forward `deps`, public/private visibility acceptance, nontrivial
visibility rejection, BUILD-name collision, lifecycle reuse after unchanged
source files, and all accepted filegroup/alias behavior and stop gates.

Keep `test_suite`, `package_group`, generated files, and Starlark rules
unsupported in the external projector. Stop and `REPLAN` on a need for
configuration matching, select resolution, analysis, another retained
representation or key, new package/repository discovery, source observation,
external patterns or functions, external loads/globs, registry transport,
repository rules/extensions, build/execution, JVM, Java bytecode, or Bazel
delegation.

Finish with serial focused query/core tests, the full query suite, quiet
direct-dependent checks, the required `slug_cli_v2` rebuild before Slug oracle
replay, GNU-Windows query/core no-run linkage, formatting, `git diff --check`,
archive/scope/no-Cargo guards, fixture generation plus distinct-root replay,
and one independent terminal implementation review.
