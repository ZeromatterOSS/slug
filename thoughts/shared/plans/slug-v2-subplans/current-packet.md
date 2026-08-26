# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-rule-doc-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: Stage 4 `package_globals::rule` adapter and retained rule definition
Base: `a81b5823`

Result: accept Bazel's named string/`None` `doc` argument on the existing
`rule` global, preserve the current semantic rule schema/capability and prove
documented definitions freeze through recursive `.bzl` loading. Do not add a
documentation extractor or widen rule analysis.

## Learned facts and authority

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority:

- `StarlarkRuleFunctionsApi.rule` declares named-only `doc` as `string | None`
  with `None` default, independently of `implementation`, `attrs`,
  `build_setting`, `toolchains` and the other rule parameters.
- `StarlarkRuleClassFunctions.createRule` converts a present string, trims it
  with `Starlark.trimDocString`, and stores it through
  `RuleClass.Builder.setStarlarkDocumentation`; a non-string fails argument
  conversion and `None` stores nothing.
- `StarlarkRuleClassFunctionsTest.testRuleDoc` authenticates short, multiline
  trimmed and omitted documentation. `RuleInfoExtractor` is the separate
  documentation consumer; loaded rule invocation and configured analysis do
  not read this prose.

The accepted rules_rust 0.73.0 archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
After the now-accepted provider declarations load, `rust/private/rustc.bzl`
recursively loads `rust/private/lto.bzl`. That module first creates documented
`RustLtoInfo`, then its line-40 `rust_lto_flag = rule(...)` supplies a string
`doc`, one implementation, and the already admitted
`config.string(flag = True)` build setting. Fresh query and build reach this
declaration; their public wrappers remain `query_error` exit 7 and
`build_runtime_error` exit 2 at the repository-session boundary.

Slug's `package_globals::rule` already converts the admitted parameters into
one `RuleDefinition`: implementation, toolchain requirements, complete
attribute schema, executable/test/build-setting bits, and export-time rule
class. Freeze retains only those build-semantic facts and its `RuleCapability`.
No admitted Slug command extracts rule documentation.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architecture guidance only.
Its complete retained semantics owner and narrow consumer projections support
keeping one call-shape adapter and projecting only build-semantic rule facts
into the frozen definition. Copy no Zabel code, representation, fingerprint,
scheduler or behavior; Bazel remains rule authority.

## Decision and non-decisions

In `package.rs`, add named `doc: Option<Value<'v>>` to
`package_globals::rule`. Validate a present value as explicit Starlark `None`
or a string, consume it at that adapter, and construct the existing
`RuleDefinition` unchanged.

Do not retain documentation, add an accessor or add a metadata registry. This
packet admits build/query loading, where docs neither alter invocation nor
configured analysis. Bazel trimming, retention and Stardoc extraction remain
unsupported/deferred; a future documentation command must `REPLAN` and retain
rule plus attribute docs from their declaration owner.

Do not change `RuleDefinitionGen`, `FrozenRuleDefinition`, `RuleCapability`,
attribute schemas, rule invocation/analysis, provider behavior, globals
placement, BUILD/MODULE/REPO behavior, DICE keys, source observations, events
or error translation. Do not admit any other missing `rule` parameter.

## Ownership, revision and lifetime

`package_globals::rule` remains the complete call-shape adapter and the frozen
rule definition remains the sole owner of admitted semantic schema/capability.
Existing source observation invalidates a doc edit before module evaluation;
because prose is not an admitted build fact, the frozen semantic projection
may remain equal after a prose-only edit.

No request input, revision certificate, overlapping-request behavior,
publication or equality rule changes. No memory is added: globals remain
evaluation-local and the frozen rule retains only its existing DICE-owned
semantic facts. Cancellation, evaluator lifetime and module ownership remain
unchanged. No fallback, cache, task or dependency is added.

## Files and caps

Allowed files, with base SHA-256 and final line ceiling:

| File | Base SHA-256 | Cap |
|---|---|---:|
| `app/slug_loading_v2/src/package.rs` | `93f04926e7bda7e2d6d12bdb6eaa7e628a0e0dde4a2001f4f2fa8c714afe1c87` | 5,125 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `3e4c944731ad50cccea263426262b6ec10665a0bfe2a329f0b966a671026d0ea` | 3,930 |

Production additions are <=5, proof additions <=65 and total additions <=70.
Both files exceed the authoring-guide size trigger, but the production change
is one parameter at the existing sole global adapter and the proof belongs in
the existing recursive external-Bzl harness. Splitting unrelated loading or
test orchestration would widen scope without separating a responsibility.

## Proof and validation

Add focused recursive external-Bzl proofs that rule definitions with string
and `None` docs bind, export and freeze as rule values while retaining the live
build-setting shape. Add a non-string `doc` rejection through the same
evaluator boundary. Do not expose documentation merely to inspect it.

Run:

- `cargo fmt --check` and `git diff --check`;
- the focused external-Bzl rule-doc tests;
- full `cargo test -p slug_loading_v2`;
- `cargo check -p slug_core_v2 --locked`;
- `cargo build -p slug_cli_v2 --locked`;
- with clean `slugd` lifecycle and fresh output roots, the existing disposable
  rules_rust query and build, recording the next common internal/public
  terminal.

Pinned source/tests already discriminate the call contract, so no new Bazel
fixture or copied archive is authorized.

## Compatibility and STOP

- **Exact:** named string/`None` `doc` acceptance, non-string rejection, and
  unchanged rule binding/export/freeze semantics for the live build-setting
  declaration route.
- **Slug-native:** Rust storage, valid-Unicode strings, internal error
  representation and nonrequired diagnostic wording.
- **Unsupported/deferred:** Bazel doc trimming/storage and Stardoc extraction,
  attribute-documentation access, every other missing rule parameter, broader
  provider/rule analysis, later rules_rust toolchains/actions, M8/M7B and exact
  output bytes.

STOP on dirty overlap, edits outside the two-file allowlist, documentation
retention/accessors/side stores, rule schema/capability changes, environment
widening, analysis changes, source vendoring, Java/JVM, dependency drift,
public documentation claims or scope above the caps. `REPLAN` before crossing
a boundary.
