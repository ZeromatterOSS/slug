# Current Slug V2 Work Packet

Packet: WP-7-108-m7a-native-package-name-build-context-r1
Status: ACCEPTED 2026-09-24; M7A remains partial

## Outcome and compatibility

Admit zero-argument `native.package_name()` in an ordinary loaded `.bzl`
macro called during BUILD-package evaluation. The
authenticated rules_kotlin 1.9.6
`@@rules_kotlin+//kotlin/internal:BUILD.bazel:15-18` invokes
`kt_configure_toolchains()` from `toolchains.bzl`; its first body check at
`:347-354` must receive `kotlin/internal`, without a repository prefix.
The rebuilt normalized-PATH authentic `slug build //:probe` must pass the
current missing-method stop; a later independently typed stop is acceptable.
Subsequent Kotlin toolchain declarations, analysis/actions, target success
and M7A close are not claimed.

Exact Bazel 9.2 slice: `native.package_name()` in a loaded `.bzl` macro
returns the calling BUILD package path, not the defining `.bzl` package,
for root and external callers; root returns the empty string. A direct
BUILD-file `native.package_name()` call fails closed, since Bazel does not
bind `native` in BUILD globals. Slug-native: the
existing `PackageRecorder`/`PackageIdentifier` Rust representation and
repository package DICE/source-observation equality. Unsupported/deferred:
top-level `.bzl`, module-extension and repository-rule invocations without
a BUILD recorder; bare-global `package_name()` exposure, Slug's existing
broader BUILD `native` namespace, broader `native` introspection, exact
invalid-context diagnostic text and downstream Kotlin
semantics. Invalid non-package calls must fail closed without fabricating a
package from source labels.

## Source, owner and decision

Pinned Bazel 9.2 `StarlarkNativeModule.java:616-620` obtains the current
`TargetDefinitionContext` and returns its package fragment as a string;
`StarlarkNativeModuleApi.java:229-239` specifies the caller BUILD package
for a loaded `.bzl` function. `PackageFactory.java:518-544` supplies the
BUILD context, `MacroClass.java:412-429` preserves the caller context for
macros, and `TargetDefinitionContext.java:161-195,224-249` rejects calls
without it. `StarlarkRepositoryIntegrationTest.java:235-250` checks an
external `//p` package. A Bazel 9.2 offline scratch oracle at
`/tmp/slug-wp808-package-name-Q2wlze`, run with `--ignore_all_rc_files`
and downloads disabled in two fresh output bases, observes a root-defined
`.bzl` macro returning caller package `alpha`, `beta/nested` and `nested`
from a generated external BUILD. Direct BUILD `native.package_name()` fails
because `native` is unbound; top-level `.bzl` and rule-implementation calls
fail phase validation. Bare-global BUILD `package_name()` succeeds but is
outside this packet. A separate fresh output base proves a root BUILD
calling that macro receives the empty string, while direct BUILD `native`
remains unbound.
The scratch stays outside the checkout. The authenticated Kotlin check is
the selected source closure, not broad toolchain parity.

`app/slug_loading_v2/src/package.rs` owns both the `native_methods` table
and `PackageRecorder::from_evaluator`. The latter already resolves ordinary
BUILD evaluation and `MacroEvaluationContext` to the caller's recorder;
its `package` is initialized from the package path in the existing package
identifier. Add only a zero-argument `native_methods::package_name` that
requires a Starlark caller-function frame whose source is a member of the
recorder's loaded `.bzl` manifest, then returns that recorder's package
string. Reuse the existing `label_source_identity_for_call` source
authentication without using its label as the package result. This gate
rejects both module-scope direct BUILD and a Slug BUILD-defined `def`
calling the broader existing `native` namespace; neither is an exact Bazel
admission. Do not use the defining `.bzl` label,
repository name, a global mutable current package, a direct filesystem read
or a new DICE key. The existing root/repository package-load keys, route and
source observations own change, invalidation, A/B/A restoration and errors;
the value is evaluator scratch allocated from an already retained compact
package path. `NativeModule` and `BzlmodNativeModule` share the methods
table, but outside BUILD/macro execution there is no recorder and the
method must fail closed. No new retained value, interner, lock, cache or
clone-heavy representation is selected. The large `package.rs` remains the
cohesive owner because this is one method beside existing package-context
`native.package_relative_label`, not a new subsystem.

## Evidence and validation

Add a focused external-package Loading test adjacent to the existing
caller-package `native.package_relative_label` macro regression: define the
function in a different `.bzl` package, call it from external
`kotlin/internal` and another consumer BUILD, and assert returned package
text has no repository/definition prefix. Cover a root BUILD calling the
macro and receiving `""`, zero-argument arity, direct-BUILD `native` call
rejection at module scope and in a BUILD-defined function, and invalid
top-level `.bzl`/extension context. Reuse the existing observed source and
route A/B/A harness; add a new fixture only for a demonstrated discriminator
gap. A protected `package_relative_label` test should still pass. No
`~/.bazelrc` contents, BuildBuddy credentials, remote transport or
NativeLink enter this Loading packet.

Resolve pinned Cargo/rustc from `rust-toolchain`; compile Loading library
tests with Cargo JSON `--no-run`, preflight exact nonignored selectors and
run selected tests serially. Rebuild `slug_cli_v2` before the authentic
normalized-PATH smoke; check/clean `slugd` before and after. Run format,
archive, diff and plan-status checks and obtain independent DESIGN and FINAL
reviews. No broad suite or remote-service gate is selected.

## Scope and stops

Base clean main `7e6920923`. Allowlist: this manifest and canonical Current
paragraph; Stage 4 native/package owner contract after acceptance;
production `app/slug_loading_v2/src/package.rs`; focused tests in
`app/slug_loading_v2/src/host_package_load_tests.rs` and, only for a smaller
wrong-context direct unit, the existing `package.rs` test module.
Production growth under roughly 15 lines and proof under roughly 130 lines
are review triggers, not permission ceilings. REPLAN if Bazel returns the
defining `.bzl` package in the selected call, the current recorder cannot
identify the caller BUILD package, exact behavior requires a new retained
context or DICE owner, or the authentic stop cannot clear without unrelated
native/toolchain semantics.

## Immediate predecessor

WP807 accepted at `7e6920923`: direct canonical-external Label sources now
reuse the bounded repository template path/source/effect owners, with a
self-generated-source guard. Five focused tests and the rebuilt authentic
smoke passed its template stop and reached this separate rules_kotlin
`native.package_name()` Loading error at registration row 55. M7A remains
partial.

## Acceptance receipt

Independent corrected DESIGN and FINAL reviews returned ACCEPT. The pinned
nightly Loading JSON library-test compile passed. Exact preflight verified
five nonignored selectors; all five passed serially: canonical-external
root, `kotlin/internal` and other caller packages against a root `.bzl`
definition; source A/B/A restoration; direct BUILD module/function,
arity and non-package fail-closed cases; and two protected package-context
regressions. The direct `slug_cli_v2` rebuild passed. The normalized-PATH
authentic one-shot `slug build //:probe` cleared the old
`native.package_name()` stop and failed independently at rules_kotlin
`toolchains.bzl:320` with `frozen select values are not valid BUILD attribute
values`. No `slugd` remained before or after. Format, archive, diff and
plan-status checks passed. No remote cache, NativeLink or BuildBuddy gate
was selected. Symbolic macros, broader direct-BUILD `native` exposure,
configured Kotlin behavior and M7A close remain deferred.
