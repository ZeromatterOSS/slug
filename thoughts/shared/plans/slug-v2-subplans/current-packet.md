# Current Slug V2 Packet

Packet: `WP-6-7A-repository-declaration-documentation-category-implementation-r1`

Milestone: M7A generic Starlark/ruleset closure; loading declaration metadata.

Status: implementation active after independent design `ACCEPT` for
`WP-6-7A-repository-declaration-documentation-category-design-r1`.

The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Observable result and compatibility classification

Freeze the complete declaration-documentation boundary needed to admit Bazel
9.2 `repository_rule(doc = ...)` during ordinary `.bzl` evaluation. The
authentic rules_rust/platforms replay is only a consumer discriminator. No
repository, platforms, ruleset, `cc_common`, `cc_internal`, parser or rule-body
special case is allowed.

Exact behavior within the admitted loading/build surface:

- `repository_rule` binds named `doc` with default/explicit `None` equivalence,
  accepts a Starlark string, and rejects every other value kind at call binding;
- a valid doc string does not change repository-rule invocation, declared
  attributes, local/configure/environ policy, repository effects, selected
  module graph, package loading, configured analysis or action identity;
- the complete already-exposed inventory is the thirteen `attr` constructors
  (`label`, `label_list`, `string_keyed_label_dict`,
  `label_keyed_string_dict`, `bool`, `int`, `label_list_dict`, `output`,
  `output_list`, `string`, `string_list`, `string_dict`, and
  `string_list_dict`), plus `rule`, `aspect`, `provider`, symbolic `macro`,
  `module_extension`, `tag_class`, and `repository_rule`. Existing members
  retain their accepted typed binding and ownership. Within this exact Slug
  inventory, `repository_rule` is the sole declaration-time rejection of a
  valid non-`None` doc argument; and
- omitted, explicit `None`, empty, whitespace/multiline and ordinary strings
  all reach the same repository build semantics. Source changes still
  invalidate through existing module-source identity.

Slug-native behavior:

- for the currently admitted loading/build graph, repository documentation is
  validated and discarded after the call. It does not enter repository-rule
  projection equality, DICE identity, marker inputs or effect identity. The
  source module remains the structural invalidation owner; and
- Rust valid-Unicode string handling retains the project-wide admitted string
  divergence. No Java object, UTF-16 layout or doc-string storage identity is
  reproduced.

Unsupported/deferred behavior:

- `native.starlark_doc_extract`, `ModuleInfo`/Stardoc protobuf output, retained
  documentation text, doc-comment association, trimming/output formatting and
  documentation query surfaces remain one later complete extraction category;
- the experimental `repository_rule(remotable = ...)`, its semantics flag,
  implicit `exec_properties`, remote repository execution and any associated
  action/REAPI model remain fail-closed as a separate category; and
- Bazel 9.2 `materializer_rule`, including its `doc` argument and complete
  dynamic dependency semantics, is not exposed by Slug and remains one
  explicitly deferred builtin category; and
- no other declaration metadata, repository behavior, parser builtin, ruleset
  API, configured analysis or action family is widened.

This classification is intentionally narrower than claiming exact
documentation extraction. It is exact for Bazel 9.2 call acceptance and build
semantics on the admitted surface, Slug-native for nonsemantic metadata
retention, and explicit about the unavailable extractor output.

## Authority and evidence

Bazel tag `9.2.0` commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
Pinned SHA-256 values are:

- `RepositoryModuleApi.java`:
  `1bb286ec5fe4667c4328081b3ca002e22fbcfb1af8f4ba5d06581a20151ddd8f`;
- `StarlarkRuleFunctionsApi.java`:
  `be73dbda0b5a3e8285a05bb732a0a01441f99e8d20dc29b83759ef972c0392ea`;
- `StarlarkRepositoryModule.java`:
  `c6adf0f521e56419ec22e7980def6b27778bab4d5c5294b3556c2286f5b6bcea`;
- `ModuleInfoExtractor.java`:
  `b17a46782eab739066ef593d90cefe8a0f7f15fa6aeaee8ccc828e573de0bebf`;
- `StarlarkDocExtractTest.java`:
  `f5779cf3f7a90350b8e422e3e67f0f5ef397465e981ce99c406befb2745c50bb`;
  and
- `ModuleInfoExtractorTest.java`:
  `0e165cbac7dfadfba0f24732e2f5da0d73a539b64951c027021710041e5fff82`.

The repository API fixes the exact public parameter list and string/None type.
`StarlarkRuleFunctionsApi` proves that `materializer_rule(doc = ...)` belongs to
a distinct Bazel builtin that Slug does not expose. The
implementation trims and retains the string in Bazel's `RepoRule`, while
`ModuleInfoExtractor` is the consumer that publishes it. These sources prove
both that build evaluation accepts the metadata and that claiming extraction
without retaining it would be false. The implementation successor therefore
admits only the loading/build slice and leaves structured extraction deferred.

Existing Slug source proves that rule/aspect documentation is already typed and
nonsemantic for current build evaluation, symbolic macros retain documentation,
provider documentation remains owned by starlark-rust, and module-extension and
tag-class docs are already typed. The successor must add a table-driven audit
test so this remains a category decision rather than a one-call workaround.

The authentic replay after default-exec configured dependencies reaches
`@platforms//host:extension.bzl` and fails only because its repository rule
passes `doc`; `local`, `configure`, and `environ` are already accepted and
retained. Replay proves priority and boundedness, not semantics.

## Learned Slug facts and architecture decision

`package_globals::repository_rule` currently binds `doc` as `Option<Value>` and
explicitly rejects every non-`None` value. Its frozen repository definition and
projection already own implementation source, attributes, local/configure and
the canonical environment-name set. Repository instantiation and effect paths
consume only those semantic fields.

Change the binding to the same typed optional-string shape already used by
module-extension/tag-class declaration functions, then discard the value in the
call body. Add no documentation field to `RepositoryRuleDefinition`, frozen
definition, projection, module selector, repository request, marker, effect or
DICE key. This makes wrong-type rejection a shared starlark-rust binding fact
and prevents documentation from perturbing build identity.

The category audit records, rather than rewrites, the exact inventory above.
Any additional valid sibling rejection, retained public metadata, extractor
consumer, repository projection or cross-crate owner is `REPLAN` and requires
an independently reviewed broader design.

Buck2/starlark-rust supplies the existing typed parameter conversion and frozen
module lifetime; add no utility. Zabel commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance only: its
`starlark_doc_extract` inputs are owned separately from repository execution
facts, supporting the separation of documentation output from semantic
repository identity. Copy no Zig code, layout, IDs, extraction behavior,
scheduler, cache, limits or compatibility claim.

## Bounded implementation successor

Implement only
`WP-6-7A-repository-declaration-documentation-category-implementation-r1`.

Allowed production file:

- `app/slug_loading_v2/src/package.rs`, limited to the repository-rule `doc`
  binding and removal of the current rejection.

`package.rs` is currently 9,580 lines. The existing `package_globals`
`starlark_module` remains the cohesive owner because the public callable
signature and the adjacent rejection are the entire change; extracting a
wrapper would split one generated binding for a two-line conversion. Forbid
growth or extraction outside that exact function hunk.

Allowed proof files:

- `app/slug_loading_v2/src/module_extension_repository_rule.rs` for the
  repository-rule call/equality/error matrix;
- Stage 6/current/canonical status documents at acceptance.

Cap Rust production at 12 gross lines, proof at 90 gross lines and total at 102.
No new file, struct field, collection, clone, allocation, cache, interner, task,
lock, DICE key, extractor model, fixture or command surface is allowed. A
compiler-required production consumer outside `package.rs`, any valid sibling
rejection, or any remote-execution coupling is `REPLAN`.

Required proof matrix:

- omitted and explicit `None` compare equal to the existing repository-rule
  projection and invocation behavior;
- empty, ordinary and multiline/whitespace strings load and invoke identically;
- integer, bool, list, dict and callable docs fail at typed call binding;
- doc variants compare equal in final repository-rule projection and captured
  invocation behavior. The unchanged complete-suite regression
  `repository_context_attributes_restore_warm_effects_for_ordinary_and_innate_owners`
  remains the downstream materialization/effect-identity guard; and
- the audit explicitly records every already-exposed doc-bearing builtin above,
  confirms no additional build-loading reject in that inventory, and records
  `materializer_rule` as deferred rather than silently excluding it.

Validation is the focused loading tests, complete `slug_loading_v2`, direct
loading-query dependents if compilation requires them, `cargo fmt --all --
--check`, `git diff --check`, source hashes, archive/allowlist/cap/forbidden-
surface gates, rebuilt `slug_cli_v2`, clean `slugd`, authentic replay, and
independent terminal implementation review.

## Immediate predecessor

Commit `20bbe8661` terminally accepts
`WP-6-7A-default-exec-configured-label-dependency-implementation-r2`. It covers
all five label-bearing constructors and default
execution configuration generically; it added no parser, ruleset, `cc_common`,
`cc_internal` or C++/Rust rule special case. Its replay exposes this independent
declaration-metadata frontier.
