# Current Slug V2 Packet

Packet: WP-4-5-7A-repository-rule-label-constructor-context-implementation-r1

Milestone: M7A bootstrap-critical loading/repository execution closure. Make
the existing `.bzl` `Label()` constructor see the exact defining-function
module context while an admitted repository-rule implementation executes.

Status: implementation terminally `ACCEPTED`. The packet composes two existing
evaluation-time owners and adds no retained repository-rule, BZL-manifest or
DICE state.

Immediate predecessor
`WP-4-7A-applicable-licenses-loading-alias-design-r1` is terminally accepted in
`9c6b8bb0b`. Its authenticated rules_rust 0.73.0 replay clears the Skylib
package alias and reaches rules_cc toolchain registration row 8. Execution of
`@@rules_cc+//cc/private/toolchain:lib_cc_configure.bzl` stops when the
repository-rule implementation calls `Label(label)` because Slug installs only
its repository-effect state as evaluator extra data.

## Learned facts and research basis

Pinned Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` owns the category:

- `StarlarkRuleFunctionsApi.Label` documents that a string resolves in the
  package containing the calling `.bzl` source file and that an existing Label
  is returned unchanged.
- `StarlarkRuleClassFunctions.label` deliberately stack-inspects the innermost
  executing Starlark function, obtains that function's `BazelModuleContext`,
  and calls `Label.parseWithPackageContext` with the module package and
  repository mapping. Its source comment rejects binding resolution to the
  module that exported or aliased the shared builtin.
- `BazelModuleContext` stores one immutable label and repository mapping on
  each loaded Starlark module. `ofInnermostBzlOrFail` selects the module of the
  innermost executing Starlark function rather than the repository being
  generated or the outer evaluator.
- `RepositoryFetchFunction.fetch` invokes the retained repository-rule
  callable directly with `Starlark.positionalOnlyCall`. It does not replace
  the callable's defining module, so the ordinary Label stack rule applies
  during repository fetching.

An isolated Bzlmod oracle run with pinned `bazel 9.2.0` defined a repository
rule in `//defs:ext.bzl` and called an imported helper from
`//helper:support.bzl` during repository generation. The generated evidence was
exactly:

```text
direct=@@//defs:direct_target
helper=@@//helper:helper_target
```

This discriminates the defining function from both the generated repository
and the outer repository-rule implementation. The checked-in recursive-BZL
Label regression already proves the same direct-builtin-alias versus imported-
function ownership rule and distinct module mappings during module loading.

Slug already reacquires the defining `FrozenBzlModule` before every selected
repository file effect. Its `BzlLoadManifest` contains the root and complete
reachable source-name-to-`BzlModuleIdentity` closure; every identity includes
the canonical package and already-selected repository mapping. Authentication
then compares the reacquired exported repository-rule projection with the
certificate call record before releasing its implementation callable.

`BzlEvaluationContext::from_manifest` and `source_identity_for_call` already
implement the accepted caller-source selection, imported-helper ownership,
missing/ambiguous-source rejection and mapping lookup used by ordinary `.bzl`
evaluation, macros and transitions. The gap is solely that
`invoke_repository_rule` puts a different runtime-only invocation state in
`Evaluator.extra`, and `BzlEvaluationContext::from_evaluator` cannot project a
BZL context from it.

## Decision and compatibility classification

Implement as **exact** within the admitted Bazel 9.2 repository-file-effect
slice:

1. A string passed to the existing `Label()` global from a repository-rule
   implementation resolves with the package and repository mapping of the
   innermost executing `.bzl` function.
2. A direct call in the implementation uses the implementation module. A call
   inside an imported helper uses the helper's own module, including its own
   package and mapping. Aliasing the shared builtin does not change this rule.
3. Every label spelling already admitted by Slug's `.bzl` constructor keeps
   its existing grammar and canonical result. Passing an existing Label stays
   idempotent.
4. Root and selected canonical external definition-module routes use the same
   authenticated behavior. Missing or ambiguous caller provenance fails
   closed before an effect plan is published.

Keep **Slug-native** the flat recursive-manifest representation, native-call
source accessor, Rust/starlark-rust evaluator-extra composition, diagnostics,
source spans, effect-plan representation, collision-safe canonical repository
identity and DICE equality/cutoff behavior.

Keep **unsupported/deferred**:

- new Label string grammar, fields or methods, BUILD aliases and calls outside
  an executing `.bzl` function;
- repository-rule declaration or attribute breadth not already admitted;
- repository_ctx methods, host operations and return-value forms outside the
  existing file/getenv subset;
- non-Starlark/native repository-rule implementations, built-in native
  repository rules and repository execution routes not selected by the
  existing authenticated module-extension/innate-repository owner;
- Bazel's Java `RepoMappingRecorder` object/event identity, evaluator or
  HotSpot state, incidental diagnostic bytes and exact generated-repository
  filesystem layout;
- materialization, lockfile update, remote repository execution, configured
  analysis/actions and any rules_cc, toolchain or repository-name special
  case.

This packet supplies context to an already-admitted pure constructor. It does
not make labels filesystem inputs and does not authorize a new DICE compute
from inside Starlark execution.

## Natural owner, identity and revision behavior

`HostSelectedRepositoryFileEffectKey` remains the only semantic owner. It
already depends on the selected owner certificate, definition source route,
recursive frozen module, host inputs and platform, and it authenticates the
reacquired definition before invocation. Pass that same module's manifest by
borrow to `invoke_repository_rule`; never reconstruct an identity from the
call record's defining label or the generated repository name.

Extend the existing runtime-only repository invocation state with one
`BzlEvaluationContext::from_manifest` value. Keep the effect builder, dynamic
environment names, invocation error and BZL context in this single evaluator
extra object. Let `BzlEvaluationContext::from_evaluator` project the nested
context just as it already does for transition and macro runtime wrappers.
Repository_ctx methods continue to project the same outer invocation state.

The manifest is already DICE-retained and compared as part of the reacquired
`FrozenBzlModule`; the effect key already observes changes to source bytes,
recursive loads, packages and mappings. The new context is evaluation scratch:
it clones existing Arc-backed mapping/manifest identities for the duration of
one synchronous invocation and is dropped before publication. Cancellation,
retry, invalidation, eviction, shutdown and A/B/A restoration do not change.
No lock is introduced or held across a DICE computation.

Do not add a field to `RepositoryRuleDefinitionProjection`,
`RepositoryRuleCallRecord`, `RepositoryRuleInvocationInput`, the selected owner
certificate, `FrozenBzlModule`, any DICE key or any published effect. Do not add
a map, interner, cache, registry, process global, I/O lookup or second label
identity.

The Buck2 utility-reuse audit selects the existing `Arc`-backed
`BzlLoadManifest`, compact `BzlModuleIdentity` values, immutable repository
mappings, `CompactString`, `SmallMap`, `Dupe` and `Allocative` owners. Because
the change adds only invocation-scoped composition and changes no retained
representation, Stage 9 gains no extraction row. The DICE ownership audit also
accepts the existing key and dependency set unchanged.

## Evidence and proof

Reuse the pinned-source implementation, the isolated Bazel 9.2 discriminator
above and the accepted recursive-BZL Label/mapping regression. Add focused Rust
proof that:

- repository invocation writes canonical results for a direct Label call and
  a call inside an imported helper from a different package;
- direct and helper modules can carry different apparent-to-canonical mappings
  and each call selects its own mapping;
- a direct builtin alias resolves against the calling function rather than the
  builtin exporter;
- an existing Label is unchanged;
- missing and ambiguous source provenance reject without publishing an effect;
- ordinary repository_ctx file/getenv behavior and its invocation-state access
  remain unchanged; and
- the rebuilt authentic rules_rust replay clears the rules_cc Label stop and
  selects only the next independent generic boundary.

The imported-helper test is the integration discriminator. A source-shape
assertion alone is insufficient. No checked-in Bazel fixture is needed because
the isolated oracle is complete for caller-package selection and the existing
accepted mapping evidence covers the unchanged resolver.

## Allowlist, caps and complexity

Production Rust may change only:

- `app/slug_loading_v2/src/provider.rs`;
- `app/slug_loading_v2/src/repository_rule_context.rs`; and
- `app/slug_loading_v2/src/module_extension_repository_file_effect.rs`.

Proof Rust may change only:

- the `#[cfg(test)]` modules in those same files.

Scheduling records may change only the canonical plan, Stage 4 owner, Stage 5
owner and this manifest. Do not change repository-rule definition/call
projections, BZL manifest representation, DICE keys, oracle fixtures,
Bazel/rules_cc sources, Cargo metadata or vendored starlark-rust.

Caps are 55 gross added production Rust lines, 150 proof lines and 205 total.
No new function may exceed 80 lines and no existing function may grow by more
than 15 lines.

All three production files exceed the 2,000-line trigger. They remain the
cohesive owners because this is one narrow handoff between the existing BZL
context projector, repository invocation runtime state and authenticated
effect caller. Do not move unrelated declarations or create another context
module. Keep the imported-helper proof in one focused test and reuse existing
test helpers where possible. No benchmark is required because retained state
and the configured hot path do not change.

## Validation and stops

Run serially:

- focused direct/imported repository Label context, mapping, idempotence and
  fail-closed tests;
- `cargo test -p slug_loading_v2 --lib -q` and every loading integration test;
- `cargo test -p slug_bzlmod_v2 --lib -q`;
- `cargo test -p slug_query_v2 --lib -q`;
- `cargo build -p slug_cli_v2 -q` before authentic replay;
- authentic rules_rust configured-query replay with stale `slugd` cleanup
  before and after; and
- `cargo fmt --check`, `git diff --check`, archive checker and parked-proof
  verification.

Return `REPLAN` before or during Rust if:

- the defining identity or mapping is reconstructed from only the repository-
  rule projection, call site, generated repository or current workspace;
- any retained definition, call, certificate, manifest, key or effect shape
  changes;
- caller selection differs from the innermost executing Starlark function or
  an imported helper cannot retain its own module mapping;
- repository execution performs I/O, a DICE compute or a lock acquisition to
  resolve Label;
- a second evaluator-extra channel, source-name parser, label resolver, map,
  cache, registry, interner, process global or special-case branch appears;
- wider repository_ctx, repository-rule, Label, materialization, lockfile,
  configured or ruleset behavior becomes necessary;
- a new oracle fixture is necessary without a docs-first provenance decision;
  or
- the file allowlist, growth caps or bounded large-file decision is exceeded.

Audit and architecture result: `ACCEPT`. The natural owner has the complete
manifest at invocation time, the existing caller-aware resolver matches the
Bazel stack rule, and runtime-only context composition is bounded. Rust may
begin under this contract.

## Accepted implementation outcome

Terminal rereview returns `ACCEPT`. The authenticated frozen definition
module's manifest is borrowed into repository execution, and the single
invocation-only state now exposes its nested `BzlEvaluationContext` to the
shared caller-aware projector. Direct implementation calls, builtin aliases,
Label idempotence and imported helpers select their own package and mapping.
Missing or ambiguous source provenance fails before effect publication.

The candidate closes at 13 gross added production Rust lines, 150 proof lines
and 163 total, with the focused discriminator at the 80-line cap. All seven
repository-context tests pass. Loading passes 513 active library units plus one
ignored and every integration target (51/29/8/6/2/1/5/1). Bzlmod passes
596/596 and query-library passes 55/55. CLI rebuild, formatting, diff, process
hygiene and archive-baseline checks pass.

The rebuilt authenticated rules_rust 0.73.0 replay clears the repository-rule
`Label()` stop. It advances on the same
`@@rules_cc+//cc/private/toolchain:lib_cc_configure.bzl:38` expression to the
independent missing `repository_ctx.path(Label(label))` method and reports that
`repository_ctx` has no attribute `path`. Select docs-only
`WP-5-7A-repository-context-path-audit` next. Audit Bazel 9.2 Label/path value,
generated-repository root, existence and host-observation semantics before
Rust; add no rules_cc, toolchain or label-spelling special case.
