# Current Slug V2 Work Packet

Packet: WP-7-107-m7a-direct-label-repository-template-source-r1
Status: ACCEPTED 2026-09-24; M7A remains partial

## Outcome and compatibility

Admit a direct typed `Label` as the source argument of the already accepted
bounded `repository_ctx.template` method when it names a canonical external
repository file. The authenticated rules_kotlin 1.9.6 capabilities repository
passes private attr.label defaults and a selected private attr.label_list
element directly to three ordered template calls in `compiler.bzl:25-42`;
the rebuilt normalized-PATH authentic `slug build //:probe` must pass its
current `repository_ctx.template invocation is unsupported` stop. A later
independently typed stop is acceptable. Complete generated repository
success, Kotlin analysis/actions and M7A close are not claimed.

Exact Bazel 9.2 slice: a direct Label template source follows Bazel's
`getPath(template)` source path/package validation before file read, then
uses the existing accepted destination, default/explicit executable mode,
insertion-ordered literal Latin-1 substitution, and output byte behavior.
For the source-authentic calls, the destination is a normalized relative
string, the Label belongs to a canonical external repository, and explicit
`executable=False` is retained. The Label package must exist, but its file
need not be a declared target. Slug-native: lexical/materialized absolute
path tokens, bounded evaluator retries, canonical route observations and
structural effect-plan identity. Unsupported/deferred: root/built-in,
self-generated, raw string and non-Label path sources beyond the already
accepted `path(Label)` slice; explicit watch modes, arbitrary template
sizes/encodings, broader repository effects and configured Kotlin semantics.

## Source, owner and decision

The authenticated selected BCR rules_kotlin 1.9.6
`src/main/starlark/core/repositories/compiler.bzl:18-42` writes `WORKSPACE`,
then uses direct Labels for ordered `BUILD.bazel`, `artifacts.bzl` and
`capabilities.bzl` template effects, all nonexecutable. The first has one
`$git_repo$` substitution; the others have none. The defaults and selected
capabilities Label are at `compiler.bzl:44-88`; the authenticated Bzlmod
default compiler 1.9.23 selects the 1.9 capabilities file. All three
template sources are below Slug's existing 2 MiB limit. Pinned Bazel 9.2
`StarlarkRepositoryContext.java:258-359` calls `getPath` on destination and
source, observes the source, applies literal replacements in dict order and
writes output bytes; `StarlarkBaseExternalContext.java:2353-2379` resolves a
Label path through package lookup and external materialization, and
`RepositoryUtils.java:53-72` requires a package but not a declared target.
`StarlarkRepositoryIntegrationTest.java:126-154` and
`starlark_repository_test.sh:459-470,777-800,1076-1100` cover direct
Label sources, source edits and executable mode. A Bazel 9.2 offline
scratch oracle at `/tmp/slug-wp807-template-Pxwnp9`, run with
`--ignore_all_rc_files` and download disabled, confirms a direct
`ctx.attr.template` Label into nested destinations, insertion-ordered
forward/reverse substitutions, the Kotlin `$git_repo$` token and explicit
false/default true executable modes. A cross-repository local-path-override
case resolves `@@producer+//templates:source.tpl` and produces identical
bytes/mode in two fresh output bases. Missing source, non-string map keys or
values, invalid executable, destination and source types fail separately.
The scratch stays outside this checkout and proves no general repository
effects or watch-mode parity. Existing accepted path-wrapped template
evidence remains the control.

`app/slug_loading_v2/src/repository_rule_context.rs` is the sole intake
owner. Its template method currently requires a `RepositoryStarlarkPath`
from `ctx.path(Label)` and rejects the direct `StarlarkLabel` before any
source observation. Derive the existing owned `RepositoryLabelPathAddress`
from a direct Label as well as the admitted path wrapper. For the direct
Label, reject an address whose canonical repository equals the invocation's
`generated_repository` before any Need, preventing self-materialization
through its own file-effect key. Otherwise request `LabelPathNeed` before
`RepositorySourceNeed`, reusing the existing retry driver and the accepted
containing-package/materialization owner. Keep the current external-root
guard, substitution limits,
source-byte read, generated-file effect, call order and terminal-attempt
publication. Do not introduce a second source resolver, retained effect,
cache, DICE key, lock or fallback. The existing
`HostRepositoryLabelPathObservationKey` and canonical route/source-read keys
own package/materialization, exact file bytes, Need/error and A/B/A
invalidation; the immutable ordered `GeneratedRepositoryFileEffectPlan`
remains sole output path/content/mode identity. Each retry drops evaluator
state before DICE awaits. New address and substitution values are invocation
scratch only, bounded by existing limits; no extra retained or clone-heavy
representation is selected. The large `repository_rule_context.rs` remains
cohesive because the change is one source-intake branch beside its existing
template method, not a second effect executor.

## Evidence and validation

Extend the focused context test to discriminate direct Label path Need,
prepared path then source Need, terminal output bytes/mode and unchanged
`ctx.path(Label)` and non-Label-path controls. In the existing observed DICE
template fixture, convert one source call to direct Label while keeping one
path-wrapped control; reuse its route/materialization/source-read, warm reuse,
A/B/A byte restoration and terminal-effect assertions. Include root and
same-generated-repository direct-Label fail-closed controls, plus
unprepared/missing-source controls where the existing tests do
not already distinguish them. Reuse accepted path-wrapped Bazel/Slug
evidence; the direct-Label scratch oracle discriminates this new intake.
No Stage 1 fixture is selected unless focused proof exposes a gap. Scratch
stays outside the checkout. No `~/.bazelrc` contents, BuildBuddy credentials,
remote transport or NativeLink enter this Loading packet.

Resolve pinned Cargo/rustc from `rust-toolchain`; compile Loading library
tests with Cargo JSON `--no-run`, preflight exact nonignored selectors, and
run selected tests serially. Rebuild `slug_cli_v2` before the authentic
normalized-PATH smoke; check/clean `slugd` before and after. Run format,
archive, diff and plan-status checks and obtain independent DESIGN and FINAL
reviews. No broad suite or remote-service gate is selected.

## Scope and stops

Base clean main `139bd8c7d`. Allowlist: this manifest and canonical Current
paragraph; Stage 4 template owner contract only after acceptance;
production and focused tests in
`app/slug_loading_v2/src/repository_rule_context.rs`; one existing observed
retry/effect test and its local fixture in
`app/slug_loading_v2/src/module_extension_repository_file_effect.rs`.
Production growth under roughly 35 lines and proof under roughly 100 lines
are review triggers, not permission ceilings. REPLAN if Bazel's direct Label
does not share the admitted `getPath`/source-read semantics, existing source
owners cannot validate the authentic external Label, a new retained effect
or DICE owner is required, or the authentic stop cannot clear without a
different repository method or configured Kotlin work.

## Immediate predecessor

WP806 accepted at `139bd8c7d`: private repository-rule LabelList defaults
now retain ordered typed labels and fail closed on attached aspects. Six
focused tests and the rebuilt authentic smoke passed its schema stop and
reached this separate direct-Label template-source rejection at toolchain
registration row 55. M7A remains partial.

## Acceptance receipt

Independent DESIGN and FINAL reviews returned ACCEPT. The pinned-nightly
Loading JSON library-test compile passed. Exact preflight
verified five nonignored selectors and all five passed: direct Label path
Need, source Need and terminal bytes/mode; self-generated/root fail-closed
controls; the observed DICE route/source A/B/A test with an actual private
`ctx.attr._template` direct Label and a path-wrapped control; and protected
read/source, substitution-limit and which-path negatives. The direct
`slug_cli_v2` rebuild passed. The normalized-PATH authentic one-shot
`slug build //:probe` cleared the old `repository_ctx.template` stop and
failed independently at toolchain registration row 55 when
`@@rules_kotlin+//kotlin/internal:toolchains.bzl:353` calls unsupported
`native.package_name()` during Loading. No `slugd` remained before or after.
No remote cache, NativeLink or BuildBuddy gate was selected. Full Kotlin
repository/build behavior and M7A close remain deferred.
