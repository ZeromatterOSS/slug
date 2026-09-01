# Current Slug V2 Packet

Packet: `WP-6-7A-rule-level-transition-attachment-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 transition consumer
breadth.

Status: terminal implementation rereview `ACCEPT`. R1 independent design
pre-review returned `REVISE` for one binding-normalization gap. The accepted R2
contract normalizes omitted and explicit `None` for both `build_setting` and
`cfg`. Initial terminal implementation review found one diagnostic-ordering
miss; the focused correction validates `build_setting` before applying the
valid-build-setting/`cfg` conflict, adds the dual-invalid discriminator, and
passed focused rereview. Base commit `37f5959c1` terminally accepts complete
regular-transition declaration-setting identity. Authentic rebuilt rules_rust
0.73.0 replay clears `transition()` construction and generic
`rule(cfg = transition)` attachment, then stops at `rule(outputs = ...)` in
`rust/private/rustdoc.bzl:319-436`.

The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Observable result and compatibility classification

Implement the complete admitted Bazel 9.2 regular Starlark rule-transition
**attachment** category instead of adding a rules_rust-only keyword. A generic
`rule()` may attach an omitted/`None` configuration or a regular object created
by `transition()`. The declaration, frozen module, imported/re-exported rule
definition, and every invoked target retain one shared semantic projection of
the transition's callable and complete input/output setting identity.

Exact admitted behavior:

- `cfg` is a named-only `rule()` parameter whose default and explicit `None`
  mean no incoming transition. Existing `build_setting` binding is corrected
  at the same seam so its omitted and explicit `None` forms are also
  equivalent;
- a live or imported frozen regular `transition()` value is accepted. Any
  other non-`None` value fails with
  `` `cfg` must be set to a transition object initialized by the transition()
  function. ``;
- a valid non-`None` `build_setting` and non-`None` `cfg` fail before `cfg`
  type conversion with
  `Build setting rules cannot use the \`cfg\` param to apply transitions to
  themselves.`. The API's typed build-setting binding rejects an invalid
  non-`None` descriptor before that conflict;
- live/frozen rule definitions, direct and transitive imports/re-exports,
  target invocation, final `StarlarkRuleImplementation` equality, package
  equality, and same-DICE restoration preserve the transition callable plus
  the already-canonical complete input/output setting slices;
- the hidden `$allowlist_function_transition` label attribute is generated
  when either an incoming regular rule transition or any user attribute
  transition exists. It retains the existing exact tools-repository label,
  implicit provenance, configured dependency identity, and schema order; and
- rules with no Starlark transition retain no generated function-transition
  allowlist attribute. Attribute-only transition behavior remains unchanged.

Slug-native behavior:

- the final Rust rule/package equality projection and load fingerprint are the
  semantic identity and invalidation mechanism. They do not claim Bazel Java
  object addresses, serialization bytes, configuration checksums, or output
  path bytes;
- analysis reports an explicit Slug unsupported diagnostic for any attached
  incoming regular transition before selector resolution, configured child
  lookup, toolchain resolution, or rule implementation invocation.

Unsupported/deferred behavior:

- executing an incoming rule transition, including input/native-option reads,
  the implementation `attr` struct, patch/split returns, output validation,
  allowlist enforcement, configuration mutation, and child/parent
  configuration identity, remains a later complete execution category;
- `config.none()`, `config.target()`, exec/native transition factories,
  composed transitions, `analysis_test_transition`, transition materializers,
  and rule extension/parent transition composition remain separately typed
  categories. This packet never accepts them as ordinary regular transitions;
- analysis-time allowlist membership checks remain deferred. Retaining the
  exact hidden dependency is not an enforcement-parity claim; and
- no parser grammar, `set`, rules_rust rule body, ruleset special case,
  provider/aspect execution, `cc_common`, `cc_internal`, C++ rule, action, BCR,
  command, or repository behavior is added. Bazel 9 BCR Starlark continues to
  own all rule bodies.

## Bazel 9.2 authority and accepted evidence

Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a` is clean and is the
sole semantic authority. Pinned source SHA-256 values are:

- `StarlarkRuleFunctionsApi.java`:
  `be73dbda0b5a3e8285a05bb732a0a01441f99e8d20dc29b83759ef972c0392ea`;
- `StarlarkRuleClassFunctions.java`:
  `a1f706cfbbc67aa3cd2521df2091dd5ed9af96eb4568049f8eee966d06c622f7`;
  and
- `StarlarkRuleTransitionProviderTest.java`:
  `9b7e78408513f0d989d76fb84bed45093333dbb0d066f737b01e49035e4ae3bb`.

The API declares `cfg` named-only with default `None`. The implementation
checks the build-setting conflict before `convertConfig`, converts `None` to
identity and `StarlarkDefinedConfigTransition` to a rule-transition provider,
then visits the resulting factory when deciding whether to add the function-
transition allowlist. `testBuildSettingCannotTransition`, `testBadCfgInput`,
and `testTransitionIsCheckedAgainstDefaultAllowlist` establish the selected
public failures and hidden dependency. The broader provider test covers
execution, native options, split returns, composition and parent behavior;
those tests are skipped here because they exercise the explicitly deferred
configured-analysis category.

No new permanent or ephemeral Bazel fixture is justified. The pinned API,
implementation, and public upstream tests discriminate every exact attachment
behavior, while the existing authentic rules_rust replay supplies the
cross-workspace imported regular-transition consumer.

## Learned Slug facts and architecture decision

Slug's accepted `TransitionDefinitionGen<Value/FrozenValue>` already owns the
callable and immutable `Arc<[TransitionSetting]>` inputs/outputs. Final
`attrs::TransitionDefinition` owns the same frozen callable and shared slices;
attribute transitions already lower through this path. `rule()` currently has
no `cfg` parameter, and its live/frozen/final rule owners do not retain an
incoming transition. The final builtin schema already adds the exact hidden
allowlist dependency for attribute transitions. Analysis obtains the final
`StarlarkRuleImplementation` before selector, dependency and toolchain work,
which is the earliest consumer-neutral fail-closed boundary. The current
untyped `build_setting: Option<Value>` path incorrectly treats explicit `None`
as a supplied invalid descriptor; Bazel's API makes omitted and explicit
`None` equivalent before the rule-body conflict.

Normalize both optional raw arguments by removing explicit `None`, validate
the build-setting descriptor, then apply the build-setting/`cfg` conflict
before converting `cfg`. Add one reusable
live-or-frozen transition projection helper and use it for both attribute and
rule consumers. Retain `Option<TransitionDefinitionGen<V>>`
in live/frozen rule definitions and lower it once at target invocation to
`Option<attrs::TransitionDefinition>` in `StarlarkRuleImplementation`. Extend
structural equality and expose a read-only accessor. The rule declaration is
the producer, frozen module ownership carries imported lifetime, package
loading is the retained semantic owner, and existing package/load-fingerprint
equality supplies DICE invalidation. Analysis checks the retained option
immediately after obtaining the Starlark implementation and returns before
any configured semantic work.

Do not create a rule-transition enum, execution schema, source-text copy,
repository-mapping copy, ordinal registry, side table, global interner, cache,
new DICE key, configuration placeholder, or fallback identity. Do not reparse
settings already canonicalized by `transition()`.

## Lifetime, memory, incremental ownership, and peer guidance

The live evaluator owns the transient callable. Freezing retains the callable
in its module heap and clones only existing immutable input/output `Arc`
slices. Invoked targets retain the frozen callable plus those same slices in
the existing final transition representation. `StarlarkRuleImplementation`
derives `Allocative`; equality includes the transition's complete structural
setting identity. The package result and its source/load fingerprint own
publication, equality cutoff and invalidation. No command scratch, evaluator
borrow, async task, cache, eviction, cancellation, shutdown, request overlay,
filesystem observation, or overlapping-request behavior changes.

Clean Buck2 commit `088c75c7e36805df99c3de29062baa95db700b8b` and
`app/buck2_transition/src/transition/starlark.rs` SHA-256
`ad2e47beeba7fbd54ba77d6a518da78b99b63a36bae7db86e9ca620559e19b76`
are concept/runtime guidance only. Buck2 keeps a single frozen callable,
`Arc` identity and `Allocative`, but its provider/platform/refs/attrs model is
not Bazel's rule-setting transition API and no code is copied.

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` and
`build_rule_declaration.zig` SHA-256
`f2221daad6d0ad61177d860e58faf3ade1bb249cce9789d7150f22bc18804fcd`
are peer architecture guidance only. Its producer-owned typed transition,
shared rule/attribute consumer identity, and either-consumer allowlist test
support the ownership decision. Slug does not adopt Zabel's allocator,
ordinals, stage vector, `config.none`, composition, parent, or semantic
conclusions.

## Implementation boundary, caps, and proofs

Production allowlist:

- `app/slug_loading_v2/src/package.rs`; and
- `app/slug_analysis_v2/src/dice.rs` only for the pre-work unsupported check.

Proof allowlist:

- `app/slug_loading_v2/tests/build_file_loading.rs`;
- `app/slug_loading_v2/src/host_package_load_tests.rs`; and
- `app/slug_analysis_v2/tests/starlark_rule.rs`.

Plan/status allowlist is this manifest, the canonical plan, and the Stage 9
ledger. Proposed caps are 140 net / 220 gross production Rust lines, 260 net /
400 gross proof Rust lines, and 620 total gross Rust lines. No function over
120 lines is expected.

`package.rs` and `dice.rs` exceed the plan-authoring complexity trigger. They
remain the cohesive owners because this packet adds only binding and immutable
field transfer at the existing rule declaration/freeze/invocation seam, plus
one return immediately after the existing analysis lookup. All setting
semantics remain in the accepted `transition.rs`; splitting either general
owner would broaden the packet.

Focused proofs must cover:

1. omitted and explicit `None` independently for `build_setting` and `cfg`,
   `build_setting = None` plus a named regular transition reaching ordinary
   attachment, positional rejection, invalid scalar/object diagnostics, and a
   real build-setting descriptor plus even an invalid non-`None` `cfg`
   producing the build-setting conflict before cfg conversion, while invalid
   build-setting plus invalid cfg fails at build-setting binding first;
2. live and imported frozen transitions through direct import, transitive
   re-export and dictionary-independent rule export, retaining callable pointer,
   canonical labels, original spellings and sorted complete slices;
3. final target equality distinguishes different incoming transitions,
   treats canonical reorder-equivalent declarations consistently with the
   accepted setting identity, and restores A/B/A through the existing package
   load lifecycle;
4. hidden allowlist schema/value/dependency identity for incoming-only,
   attribute-only, both sharing one transition object, and neither; there must
   be exactly one generated hidden attribute when both consumers exist;
5. analysis failure before selector package lookup, configured dependency,
   toolchain resolution, transition callable, or rule implementation for an
   attached rule transition, while an otherwise identical `cfg = None` rule
   preserves existing analysis; and
6. rebuilt authentic rules_rust 0.73.0 replay clears generic rule-level `cfg`
   declaration binding and stops at the next independently demonstrated
   generic frontier, never at parser, `set`, `cc_common`, `cc_internal`, or a
   ruleset-specific branch.

Validation is serial: focused loading tests, complete `slug_loading_v2`,
focused and complete `slug_analysis_v2`, `cargo fmt --check`, Cargo metadata,
`git diff --check`, archive status, pinned source/hash and clean Buck2/Zabel
checks, parked-file hash, `cargo build -p slug_cli_v2`, stale-`slugd` cleanup,
and authentic replay. Independent design pre-review and terminal
implementation review are required because the packet extends retained public
cross-crate identity.

There is no fallback. `REPLAN` before implementation if exact attachment
requires transition execution, a new DICE owner/key, evaluator-borrowed
retained state, source reconstruction, a second transition representation,
rule-parent/composition support, native transition widening, or general rule
signature work outside the selected category. `REPLAN` during implementation
if the early analysis stop cannot precede all selector/dependency/toolchain
work, if package equality cannot carry the complete transition structurally,
if the production cap is exceeded, or if replay contradicts pinned Bazel 9.2
evidence.

Residual risk is explicit: complete attachment identity prevents loading-side
representation churn, but configured rule-transition execution remains a
later category selected only when replay reaches an instantiated consumer.

The accepted candidate is 55 net / 77 gross production Rust lines and 251 net / 257
gross proof Rust lines. Focused binding/identity/import/ordering/A-B-A and
pre-configured-work tests pass. Complete `slug_loading_v2` passes 560 tests
with one ignored; complete `slug_analysis_v2` passes 122. Formatting, Cargo
metadata, diff hygiene, pinned-source hashes, clean Bazel/Buck2/Zabel, parked-
file hash, rebuilt `slug_cli_v2`, and no-stale-`slugd` gates pass. The archive
checker retains only its three documented thought-path failures. Initial
terminal implementation review returned `REVISE` for the dual-invalid
diagnostic ordering; the focused correction test, complete loading suite and
independent terminal rereview pass.

Authentic rules_rust 0.73.0 cquery clears `rule(cfg = transition)` and advances
through the rest of `rust/private/rust.bzl`. It stops at the next independent
generic declaration frontier, `rule(outputs = {"rust_doc_zip":
"%{name}.zip"})` in `rust/private/rustdoc.bzl:319-436`, before any rule body,
transition execution, `cc_common`, `cc_internal`, or C++ special handling.

## Immediate predecessor

Commit `37f5959c1` terminally accepted
`WP-6-7A-transition-declaration-setting-identity-r1`: one canonical
definition-repository setting record plus shared immutable input/output slices
serves live/frozen/imported declarations and the admitted narrow attribute
execution path. Complete loading passed 559 tests with one ignored, analysis
passed 121, independent review returned `ACCEPT`, and authentic replay advanced
to the generic rule-level attachment selected here.
