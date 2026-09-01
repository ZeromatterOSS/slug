# Current Slug V2 Packet

Packet: `WP-6-7A-bzlmod-declaration-signature-parity-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 Bzlmod declaration
builtin call binding.

Status: independent public-ABI review returned `ACCEPT`; implementation is
active. Commit `21db5d7b8` terminally accepts the complete FilesToRun Spawn
expansion and is this packet's base. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked;
do not edit or stage it.

## Observable result and stop boundary

Make the admitted call signatures of the complete Bzlmod `.bzl` declaration
builtin category match Bazel 9.2:

1. `repository_rule(implementation, ...)` keeps its already-correct mandatory
   positional-or-named first parameter;
2. `module_extension(implementation, ...)` admits its mandatory first
   parameter positionally or by name; and
3. `tag_class(attrs, ...)` admits its optional first parameter positionally or
   by name, with omission retaining the empty-dictionary default.

Every later parameter remains named-only. Duplicate positional-plus-named
binding, missing mandatory implementation, excess positional arguments and
existing type/semantic validation continue to fail before publication. Named
forms remain byte-for-byte behaviorally unchanged.

After focused proof, rebuild the V2 CLI and run two daemon-clean
`cquery //app/slug_cli_v2:slug` replays. Both must pass the authentic rules_cc
0.2.17 `compatibility_proxy = module_extension(_compat_proxy_impl)` declaration
and stop identically at the next unsupported boundary, or succeed. The observed
next boundary selects the successor; do not expand this packet to consume it.

This is generic Starlark Host-ABI binding. It adds no parser or evaluator
language construct, rule implementation, repository/module-extension effect,
provider, DICE key, configured analysis, action, execution, `cc_common`,
`cc_internal`, rules_cc or C++ special case. Bazel 9 BCR Starlark continues to
own rule bodies; `cc_common` is only a downstream consumer.

## Learned facts and authenticated evidence

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole
semantic authority. Pinned sources and SHA-256 values are:

- `RepositoryModuleApi.java`
  `1bb286ec5fe4667c4328081b3ca002e22fbcfb1af8f4ba5d06581a20151ddd8f`;
- `Param.java`
  `3014de7bc7fb2bb40b8f1e8f0ec648bd923eb5777963c9d21111e0dfcae28104`;
- `RunfilesRepoMappingManifestTest.java`
  `8df1c7f6cc4558fe35405f43e7130ffc4f0588f41e75f18709adf520146545df`;
  and
- selected rules_cc 0.2.17 `cc/extensions.bzl`
  `a190a467ac48329a76e1a9ccab1fea53519af4bb2202e22346b23fc24dcf9872`.

`RepositoryModuleApi` marks the first parameter of `repository_rule` and
`module_extension` as `named = true` without disabling positional binding;
`tag_class.attrs` has the same declaration. `Param.positional()` defaults to
true. All following parameters explicitly set `positional = false`.
`RunfilesRepoMappingManifestTest` and rules_cc independently exercise the
single positional `module_extension` spelling.

A disposable Bazel 9.2 oracle composed all three positional first-parameter
forms in one extension: `repository_rule(_repo_impl)`,
`tag_class({"value": attr.string()})`, and
`module_extension(_extension_impl, tag_classes = {...})`. Querying the
generated repository returned `@generated//:ok`. The current rebuilt Slug
binary instead reaches rules_cc and returns the discriminating error
`Missing named-only parameter implementation` at `cc/extensions.bzl:190`.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance,
not a source of truth. Its
`src/starlark_host/engine/module_extension_execution_capture.zig`
(`8f03505b2302f79443d3ab95f12cbca2b65eec8a417ff94e739fb9fafcd06fc0`)
keeps call binding at the declaration Host boundary and already proves the
rules_cc positional `module_extension` spelling. Its `tag_class` binder rejects
all positional arguments, contrary to pinned Bazel's API declaration, so that
behavior is explicitly not adopted. Copy no Zig code, binding tables, errors,
representation, scheduler, cache or compatibility claim.

## Compatibility classification

**Exact:** first-parameter positional-or-named acceptance for
`repository_rule`, `module_extension`, and `tag_class`; omission/default rules;
named-only status for later parameters; duplicate, missing and excess argument
rejection; and unchanged existing semantic validation after binding.

**Slug-native:** Rust/starlark-rust diagnostic wording where the accepted tests
do not claim exact Bazel text, Rust Unicode, compact frozen declaration layout
and existing structural publication identity.

**Unsupported/deferred:** `repository_rule.remotable` behind Bazel's
experimental flag; unadmitted repository/module-context operations; physical
repository effects beyond accepted capabilities; later BCR loading/configured
analysis failures; exact Java exception text; and all action/execution breadth.
The packet may not silently widen any of these surfaces.

## Frozen architecture

Keep `package_globals` as the sole declaration-builtin registration owner and
starlark-rust's generated parameter schema as the sole binder. Correct only the
two mismatched first-parameter annotations: remove the named-only requirement
from `module_extension.implementation` and `tag_class.attrs`. Do not add a
manual `*args/**kwargs` adapter, signature table, wrapper builtin, call-site
rewrite, source inspection or rules_cc branch. The existing Rust function
arguments continue to feed the same validation and frozen declaration owners.

Category proof must cover all three builtins together so later consumers do not
reopen this signature decision. Assert positional, named and omitted-where-
optional forms; reject positional-plus-named duplicates and a second
positional argument; and prove positional/named results retain the same
declaration content. No retained representation, hashing, compact collection,
interning, clone, memory-accounting or DICE ownership changes.

## Allowlist, caps, validation and stops

Production allowlist:

- `app/slug_loading_v2/src/package.rs`.

Proof allowlist:

- the inline `module_extension_definition_tests` module in
  `app/slug_loading_v2/src/package.rs`;
- `app/slug_loading_v2/src/host_package_load_tests.rs`; or
- one existing focused `app/slug_loading_v2` test module if it already owns
  declaration-builtin call signatures.

Scheduling/status edits may touch this manifest, canonical Live Status,
Stage 6 and Stage 9. No routing-log row is needed unless review changes the
route.

Caps are 10 net / 10 gross production Rust lines, 120 net / 150 gross proof
Rust lines, and 130 net / 160 gross total Rust lines. Validate serially with
the focused signature tests, full `slug_loading_v2`, direct `slug_query_v2` and
`slug_analysis_v2` dependents, rebuilt `slug_cli_v2`, two daemon-clean real
bootstrap replays, `cargo fmt --all -- --check`, Cargo metadata,
`scripts/v2_archive_status.sh`, `git diff --check`, cap accounting and parked
file SHA-256 verification. Clean stale `slugd` processes before and after the
replays.

`REPLAN` before changing parser/evaluator language semantics, starlark-rust,
manual argument binding, a retained declaration representation, DICE/loading
ownership, repository effects, a rule/provider/action owner, exact diagnostic
claims, an unsupported parameter, any ruleset/C++ branch, or a cap. Independent
public-ABI review is required before Rust; independent terminal review is
required before acceptance and commit.

## Immediate predecessor

Commit `21db5d7b8` implements the independently accepted complete FilesToRun
Spawn expansion at `+378/-226` production and `+279/-25` proof Rust lines. It
passes full owner/downstream, alias, root/subrule, warm A/B/A, format, metadata,
archive and hygiene gates. Its later execution/REAPI importer remains deferred
and must consume FilesToRun roots from both invocation and tool domains without
flattening or losing alias topology.
