# Current Slug V2 Packet

Packet: `WP-4-7A-post-rustfmt-source-order-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: docs-only recursive selected-source and declaration-surface audit
Base: `88304c2f`

Result: replay the accepted selected rules_rust root's recursive manifest after
`rustfmt_test`, identify the first newly evaluated unsupported expression, and
select one bounded implementation packet or `REPLAN`. Make no Rust, fixture,
oracle, command behavior or source-materialization change.

## Accepted starting point

Commit `88304c2f` freezes the exact fixed `rustfmt_test.targets` label-list
dependency declaration. Its ordered `CrateInfo` and `TestCrateInfo` provider
alternatives, complete `_rustfmt_test_aspect`, and existing
`platform_transition` remain distinct facts in the declaration-owned frozen
rule schema. Every producer retains its defining module and first export.

Target invocation fails before the ordinary configured loading projection can
discard provider or aspect metadata. Aspect application/propagation,
transition evaluation, provider matching, configured dependency analysis and
actions remain unsupported. The implementation and all aspect bodies stay
lazy during module loading.

Focused proof, all 196 loading unit tests, unaffected loading integrations,
locked core check, rebuilt CLI, formatting and diff gates pass. The sole broad
integration failure is the already-recorded stale `@external` diagnostic
expectation. Final additions are 66 production and 175 proof lines, within the
packet caps. Independent correction review returned `ACCEPT`.

## Recursive source-order candidate

Use the accepted rules_rust 0.73.0 archive, not a guessed public declaration.
The preliminary replay is:

1. `rust/private/rustfmt.bzl:244-279` is the already-parsed documentation tail
   of the accepted `rustfmt_test` rule.
2. Lines 281-309 and 336-348 are lazy implementation bodies. Lines 311-334 and
   350-356 declare `rustfmt_toolchain` and `current_rustfmt_toolchain` using
   label schemas, docs and canonical toolchain strings that appear already
   admitted. The audit must prove that against current Slug.
3. Evaluation returns to `rust/toolchain.bzl`. Its rust-analyzer wrapper at
   lines 15-18 is alias-only. Lines 19-22 enter
   `rust/rust_stdlib_filegroup.bzl`, which loads
   `rust/private/toolchain.bzl`.
4. That private module first loads mapped
   `@bazel_skylib//rules:common_settings.bzl`. The selected external route is
   expected to own this child; the audit must prove the exact selected module,
   repository mapping, source identity and prior-child completion.
5. In the candidate selected `common_settings.bzl`, provider and string
   attribute declarations through the first rule call are already supported or
   lazy. The first apparent absent expression is `config.int(flag = True)` at
   line 71, followed by `config.int()` at line 81.

Do not assume this candidate is authoritative until the actual selected graph,
source bytes and live loading surface agree. Generic top-level repository
session wrappers are not discriminating source-order evidence.

## Required authorities

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
The accepted rules_rust archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Use only pinned source objects and selected-source provenance already admitted
by the repository graph. Do not add a network oracle or fixture.

For the integer candidate, inspect at minimum:

- pinned `StarlarkConfigApi.intSetting`, including named-only `flag` with
  default `False`;
- pinned `StarlarkConfig.intSetting`, `BuildSetting.create` and rule-class
  build-setting schema construction for the INTEGER type and flag bit;
- pinned tests covering integer descriptor construction, equality, default
  coercion and invalid rule defaults; and
- current Slug `ConfigModule`, `BuildSettingKind`, rule definition/freeze,
  declared attribute schema and target-invocation rejection.

Pinned Zabel commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Read its `src/starlark_host/engine/build_rule_declaration.zig` at that commit,
not the live checkout head. Its evaluator-free `BuildSettingDefinition` owns
`BuildSettingKind.int` beside boolean, string and list kinds. Use this only to
evaluate whether Slug should extend its existing declaration owner. Do not
copy Zig code, representation, diagnostics, evaluator behavior, configured
capture, cache or analysis algorithms; Zabel supplies no behavior authority.

## Audit questions

Answer all before selecting implementation:

1. Does the selected recursive manifest actually complete both remaining
   rustfmt rules and the alias wrapper before entering the mapped skylib child?
2. Which exact selected skylib module/version/source contains the reached
   `common_settings.bzl`, and does the existing producer mapping select it
   without a new route, key or source owner?
3. Is `config.int(flag = True)` the first evaluated missing capability after
   all preceding declarations, rather than an earlier load, export, provider,
   attribute, rule or freeze failure?
4. What exact API shapes are required by the adjacent `int_flag` and
   `int_setting` declarations: named `True`, omitted/default `False`, explicit
   `False`, positional rejection, or a smaller source-order subset?
5. What immutable Bazel descriptor identity includes the INTEGER kind and flag
   bit, and how does the enclosing rule derive its mandatory default schema?
6. Can Slug reuse its current build-setting definition/freeze owner and reject
   target invocation before recording, with no integer configured consumer?
7. What invalid inputs discriminate integer defaults from bool/string/list,
   and what later expression becomes the next frontier after the selected
   bounded implementation?

## Compatibility and ownership gates

Classify every selected behavior as **exact**, **Slug-native**, or
**unsupported/deferred**. A likely bounded slice may include exact `.bzl`-only
integer descriptor construction, the source-required flag forms, retained
INTEGER kind/flag identity, recursive freeze and integer default schema.
Rust enum/layout choices, diagnostics and fail-closed invocation remain
Slug-native.

At minimum, BUILD exposure, integer command-line parsing, transition values,
configured build-setting evaluation, analysis/provider returns, later
`attr.label_list(allow_files = True)`, toolchain implementations/actions,
M8/M7B and exact configuration/output identity remain unsupported unless this
audit proves a smaller prerequisite requires otherwise.

Prefer the existing `Root*BuildSetting`, `BuildSettingKind`, frozen rule
definition, compact strings and schema projection. No side registry, raw
evaluator-value retention, identity reconstruction, new interner, collection,
hash family, DICE key, repository mapping, source observer, I/O, lock, async
task or command result is admitted. If an integer descriptor cannot remain in
that existing owner, return `REPLAN`.

## Docs-only allowlist and validation

Only these files may change from base `88304c2f`:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

Validate the pinned Bazel and Zabel commit objects, accepted rules_rust archive
digest, exact selected skylib source identity, recursive source order, current
Slug signatures, packet-ID agreement, document structure, `git diff --check`
and exact file allowlist. This audit requires independent review before commit.

Any selected implementation packet must state exact file hashes, physical and
addition caps, touched-function limits, focused/full serial validation,
discriminating freeze/equality/default/invocation proofs, archive hygiene and a
terminal independent implementation review.

## STOP / `REPLAN`

STOP and `REPLAN` if the candidate source is not the selected graph input; an
earlier recursive child fails; success requires a new repository route/source
owner; integer semantics cannot remain declaration-owned; the packet would
need BUILD exposure, configured evaluation, transition/CLI behavior, analysis
or actions; a new DICE key, mapping, I/O path, cache, registry, interner or hash
domain is required; Java/JVM work would enter Slug; Zabel code or behavior
would be adopted; source or authority is unpinned; or a bounded proof/line-cap
contract cannot be written. Do not edit Rust during this audit.
