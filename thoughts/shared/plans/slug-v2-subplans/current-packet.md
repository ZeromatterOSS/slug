# Current Slug V2 Work Packet

Packet: WP-7-106-m7a-private-repository-label-list-default-r1
Status: ACCEPTED 2026-09-24; M7A remains partial

## Outcome and compatibility

Admit the source-authentic rules_kotlin 1.9.6 repository-rule declaration
`_capabilities_templates: attr.label_list(default = _CAPABILITIES_TEMPLATES.values())`
in authenticated BCR `compiler.bzl:76-79`. Its eight ordered label defaults must be
retained in the frozen repository-rule definition, visible through
`repository_ctx.attr._capabilities_templates`, and distinct in structural
definition identity when changed. The rebuilt normalized-PATH authentic
`slug build //:probe` must pass its current unsupported-schema stop; a later
independently typed stop is acceptable. Full repository execution/effects, Kotlin
analysis/actions, target success and M7A close are not claimed.

Exact Bazel 9.2 slice: private `attr.label_list` declaration with ordinary
typed label defaults from a sequence, including `dict.values()`; default
elements resolve in the defining `.bzl` package and keep
sequence order; the public Starlark `_name` exposes defaults and concrete
overrides through `repository_ctx.attr`. Malformed element types, malformed
labels and a raw dictionary instead of a sequence fail at declaration.
Slug-native: canonical-label structural tokens, repository-rule projection
and source-fingerprint/DICE equality. Unsupported/deferred: Bazel's native
`$name` private representation, cross-repository caller mapping, attached
aspects and other lossy/deferred repository schemas, full repository effects
and configured Kotlin semantics.
An attached aspect on the newly admitted private list must fail closed, not
be silently discarded. Bazel's acceptance of such a declaration is known;
Slug does not claim parity for its aspect semantics.

## Source, owner and decision

The authenticated selected BCR rules_kotlin source
`src/main/starlark/core/repositories/compiler.bzl:56-88` declares eight
ordered Kotlin capabilities templates and passes
`ctx.attr._capabilities_templates` to `_get_capability_template` before
`repository_ctx.template`. Pinned Bazel 9.2
`StarlarkRepositoryModule.java:80-91` builds the repository descriptor;
`StarlarkAttrModule.java:375-378` converts the label-list default with the
defining `.bzl` label converter (`LabelConverter.java:35-44`).
`AttributeValueSource.java:23-71` maps private `_x` to native `$x`;
`RepoDefinition.java:42-60` exposes the declared default under `_x` in
`repository_ctx.attr`, while `RepoRule.java:107-115` keeps explicit call
overrides separately. A Bazel 9.2 offline scratch oracle at
`/tmp/slug-wp806-label-list-default`, with fresh output base and
`--ignore_all_rc_files`, observes ordered private defaults from
`dict.values()`, a tuple and a list of Labels, including `:local` resolution
in the defining `.bzl` package. The scratch does not distinguish a different
caller repository mapping. Separate malformed defaults reject a raw
dict, an integer element and an invalid label. A separate aspect control
confirms Bazel accepts a private label-list aspect declaration; this packet
explicitly defers that semantics. These checks select bounded
declaration/default behavior, not arbitrary repository execution.

`app/slug_loading_v2/src/package.rs` is the declaration gate owner: its
existing `repository_rule_default_matches` already accepts a typed
`CoercedAttributeValue::LabelList`, while the private-kind allowlist alone
rejects it. Admit `AttributeKind::LabelList` there, with an explicit
`attached_aspect.is_some()` rejection for this newly admitted private kind:
`RepositoryRuleAttribute` does not retain that field. Keep all other schema
guards intact. The existing source-backed label coercion, ordered
`RepositoryRuleAttribute.default`, frozen
`RepositoryRuleDefinitionProjection`, call record, instantiation validation
and `repository_ctx.attr` allocation own the resulting values. Do not add a
second default map, identity, cache, DICE key, lock or bypass. The existing
Loading source/recursive-manifest observations own invalidation and A/B/A
restoration; the default stays in the immutable projection, not a call-site
override-only `RepoSpec`. `package.rs` is large but this is one local guard
in its established declaration owner; a split would fragment the bounded
schema check without removing a second responsibility.

## Evidence and validation

Update the obsolete private-list rejection in the repository-rule definition
matrix, preserving private-bool and unrelated schema negatives. Add a
source-shaped frozen declaration/call test with an ordered eight-label
`dict.values()` default from an external defining `.bzl` context, exact
ordered canonical-label assertions, concrete override, default A/B/A identity and a
changed-order/changed-element distinction. Add a private-list
`repository_ctx.attr` test for default and override order and typed Labels;
retain existing public-list and private string/label protections. Add a
negative attached-aspect descriptor and malformed default checks. Reuse the
offline scratch oracle; no Stage 1 fixture is selected unless a focused
regression demonstrates a missing discriminator. Scratch artifacts stay
outside this checkout. No `~/.bazelrc` contents, BuildBuddy credentials,
remote transport or NativeLink enter this Loading packet.

Resolve pinned Cargo/rustc from `rust-toolchain`; compile Loading library
tests with Cargo JSON `--no-run`, preflight exact nonignored selectors, and
run selected tests serially. Rebuild `slug_cli_v2` before the authentic
normalized-PATH smoke; check/clean `slugd` before and after. Run format,
archive, diff and plan-status checks, and obtain independent DESIGN and
FINAL reviews. A downstream direct CLI build covers this public Loading
behavior; no broad suite or remote-service gate is selected.

## Scope and stops

Base clean main `6e3abc2ee`. Allowlist: this manifest, canonical Current
paragraph, Stage 4 repository-rule owner contract after acceptance;
production `app/slug_loading_v2/src/package.rs`; focused tests in
`app/slug_loading_v2/src/module_extension_repository_rule.rs` and
`app/slug_loading_v2/src/repository_rule_context.rs`. Production growth
under roughly 15 lines and proof under roughly 150 lines are review
triggers, not permission ceilings. REPLAN if Bazel 9.2 does not admit the
authentic private list, the existing label coercion/default owner loses
order or source-relative mapping, exact behavior requires a new retained
owner, or the authentic stop cannot clear without unrelated repository or
configured semantics.

## Immediate predecessor

WP805 accepted at `6e3abc2ee`: exported ordinary and initialized provider
constructors now serve as dict keys. Five focused tests and the rebuilt
authentic smoke passed the old provider hashability stop and reached this
separate rules_kotlin repository-rule schema error at toolchain registration
row 55. M7A remains partial.

## Acceptance receipt

Independent DESIGN and FINAL reviews returned ACCEPT. The pinned-nightly
Loading JSON library-test compile passed. Exact preflight
verified six nonignored selectors and all six passed: the source-shaped
eight-label frozen definition/call and context default/override tests, the
updated declaration/negative matrix, and three protected private/public
repository attribute tests. The malformed dict, integer-element, bad-label
and aspect-bearing private-list negatives all failed as intended. The direct
`slug_cli_v2` rebuild passed. The normalized-PATH authentic one-shot
`slug build //:probe` cleared the old `_capabilities_templates` schema stop
and failed independently at registration row 55 on
`repository_ctx.template invocation is unsupported` in the selected
rules_kotlin generated repository. No `slugd` remained before or after.
Two focused test owners add about 184 lines, above the estimated 150-line
review trigger because the source-authentic eight-label order, canonical
source resolution, A/B/A distinction and context projection require separate
direct assertions; the production guard remains six changed lines. Full
repository effects, configured Kotlin and M7A close remain deferred. No
remote cache, NativeLink or BuildBuddy gate was selected.
