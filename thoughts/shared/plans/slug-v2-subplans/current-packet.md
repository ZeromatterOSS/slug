# Current Slug V2 Packet

Packet: `WP-6-7A-rule-level-starlark-transition-execution-architecture-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 configured-target
identity, transition application and delegation.

Status: corrected R2 architecture accepted; implementation active. R1 review
returned `REVISE` only because it incorrectly admitted a scalar `Label` as a
`platforms` output; Bazel accepts scalar string or a sequence of strings/Labels
and rejects scalar `Label`. Focused R2 rereview returned `ACCEPT` with no
remaining findings. Commit `493be79b6` terminally accepted provider-constrained
dependencies and the rebuilt authentic rules_rust 0.73 replay now stops at the
independent rule-level Starlark transition-execution gate for `@@//pkg:probe`.

The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Observable result and compatibility classification

Implement one generic rule self-transition lifecycle before configured
attribute, dependency, toolchain, fragment or rule-implementation analysis.
The transition engine covers every already-admitted Starlark build-setting
value and Bazel's target-platform native-option category. The BCR rules_rust
transition is one discriminator, never an implementation branch.

Exact behavior within the admitted surface:

- a Starlark rule carrying `rule(cfg = transition(...))` applies that patch
  transition to its own incoming target configuration. The transition receives
  a `settings` dictionary keyed by each declared spelling and an `attr` struct
  containing raw configured rule attributes, with label-bearing values exposed
  as `Label` values rather than configured dependency/provider objects;
- direct Starlark build settings in any visible repository may be inputs or
  outputs. Integer, Boolean, string, repeatable/nonrepeatable string-list and
  string-set declarations use their existing typed default/effective-value,
  unpacking, normalization and default-elision owners. Multiple inputs and
  outputs are admitted in one transition, and every declaration is loaded
  through DICE before synchronous evaluation;
- `//command_line_option:platforms` may be an input or output. Its input is the
  effective target-platform label list, including the Host fallback when the
  retained option list is empty. Output accepts Bazel's scalar string or a
  sequence of strings/Labels; scalar `Label` is rejected as an invalid native
  option value. Accepted spellings resolve in the transition definition's Bzl/
  repository context, normalize Bazel's list option to its first member, and
  treat empty or the effective Host platform as the existing Host-fallback
  semantic configuration. A changed effective selection is DICE-validated as
  an actual platform before delegation. Selected platform declarations in this
  packet have default empty `flags`, `parents`, `required_settings` and other
  already-unsupported platform policy fields;
- the implementation may return a patch dictionary, a string-keyed dictionary
  of patch dictionaries, a sequence of patch dictionaries, `None`, an empty
  dictionary or an empty sequence. The latter three forms are identity. Every
  nonempty patch must return every declared output and no undeclared output. A
  one-entry split-shaped return is accepted; rule transitions reject more than
  one returned configuration with Bazel's single-configuration
  rule-transition boundary;
- values are validated against their declared setting before publication.
  Invalid/missing/non-build-setting targets, bad native option/value forms,
  missing or extra outputs, evaluation failures and split rule results fail
  before any configured dependency, toolchain or implementation work;
- transition `attr` preparation uses the pre-transition configuration. A
  selector that is invalid or whose condition reads a setting the transition
  may output is omitted from the struct, preserving Bazel's informative
  missing-attribute behavior. Other selectors resolve under the incoming
  configuration. The final rule analysis resolves all attributes again under
  the transitioned configuration;
- the first application is classified as identity when structural
  configuration is unchanged. A changed result is applied a second time with
  the same transition and captured `attr` object but a fresh `settings`
  dictionary projected from the first result. Equal first and second results
  are idempotent; unequal results are non-idempotent. Identity analyzes the
  requested key. Idempotent results delegate to the transitioned key with rule
  application still enabled. Non-idempotent results delegate to the
  transitioned key with rule application disabled, so the transition executes
  exactly once and cannot create an unbounded key chain;
- the apply/skip state participates in configured-target/DICE equality and
  hashing but not user-facing label/configuration formatting. The delegated
  configured result owns its final structural configuration, dependencies,
  toolchain selection, fragments, actions and providers. Two incoming
  configurations that converge share the same lawful final key; and
- rule transition output is the parent for all ordinary attribute transitions:
  the rule transition is applied first, final rule attributes/selects are
  resolved under it, and each dependency transition starts from that final
  rule configuration. The attribute transition receives fresh settings plus
  that final raw configured attr struct and may use the same complete admitted
  setting/platform category; its currently admitted dependency edge still
  requires zero or one result. Same-DICE A/B/A changes to transition
  implementation, transition attributes, setting defaults or declarations,
  and configuration inputs recompute and exact restoration cuts off.

Slug-native behavior:

- semantic configuration equality remains Slug's complete Rust structural
  identity. The apply/skip bit is semantic configured-analysis control, while
  display/path tokens remain separate projections. Slug does not reproduce
  Bazel's Java object identity, SkyKey interning, checksum, `-ST-` fragment or
  `bazel-out` bytes; and
- Host-fallback and explicit-Host target-platform forms may canonicalize to one
  Slug semantic configuration. Diagnostics preserve target, transition source,
  declared setting and value shape without claiming byte-identical Java event
  decoration.

Unsupported/deferred behavior:

- native options other than `//command_line_option:platforms`, native flag
  aliases, `--define`, platform mappings and nonempty platform `flags` or
  inheritance/policy remain fail-closed. Each later native option family must
  project and update the existing sole typed `OptionRecord` vector; no opaque
  native dictionary or parallel configuration store is permitted;
- aliased build settings, aliases whose actual uses `select()`, project-boundary
  enforcement and exact Bazel output-directory affected-option bytes remain
  later complete categories. Direct canonical build settings in root and
  external repositories are admitted here;
- multi-configuration attribute transitions remain unsupported. The shared
  evaluator may represent zero/one/many returned patches, but the existing
  configured dependency producer continues to admit only one final child
  configuration until split-edge identity, ordering, query topology and
  provider materialization move together;
- transition `print`/event replay, analysis-test transitions, exec transitions,
  native trimming/composed rule transitions, configuration fragment trimming,
  flag-setting aliases and platform flag expansion remain separate categories;
- configured aspect execution remains fail-closed. No `CcInfo`, `rust_common`,
  `cc_common`, `cc_internal`, C++/Rust rule body, parser grammar, `set`, action,
  query formatter or execution fallback is added. Bazel 9 BCR Starlark owns all
  rule and transition bodies.

## Bazel 9.2 authority and evidence

Bazel tag `9.2.0` commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole semantic authority.
Pinned source SHA-256 values are:

- `RuleTransitionApplier.java`:
  `077a37ebf2f339f530ae97bfd405ede4852a7a922dd1d54c260bb316c017eebd`;
- `TargetAndConfigurationProducer.java`:
  `0321b8858ed23282237947183040d6ec7a9dc83f776aaa1619cc4d3b71fe0722`;
- `StarlarkRuleTransitionProvider.java`:
  `98f6904646e5f789dd79d81dfce09687fa09a5161fe5bfd2772b08b78e45f7ae`;
- `StarlarkAttributeTransitionProvider.java`:
  `669bc7600309ac0553375ae1ff4104efc801f8d19a4b4b673c6d243eeffdc54f`;
- `StarlarkDefinedConfigTransition.java`:
  `427a16020eb158943b4073981b1f0701b75ebd85d28816f3a3b6415afcb9a22b`;
- `FunctionTransitionUtil.java`:
  `f07a15da5366085a9ba9d8054628f0e244d7022611180c63299e79c2a5cb7447`;
- `ConfiguredTargetKey.java`:
  `a679d99c0195fe16c247b9702e349a32c72f5710757b78f75f82ab54e035ae28`;
- `PlatformOptions.java`:
  `631330a01e28bff914f79d4ad63898f333853756c012407376313474fde3f26e`;
- `StarlarkRuleTransitionProviderTest.java`:
  `9b7e78408513f0d989d76fb84bed45093333dbb0d066f737b01e49035e4ae3bb`;
- `StarlarkAttrTransitionProviderTest.java`:
  `607f6f12b6fbc343a3a423f7ab99f25eef40dfb05e1b1abaf948245b6baca7d7`;
  and
- `ConfigStringSetTest.java`:
  `0aeba90ac56ea1687268d2f9eefa1b4264e58aeafb8bf653634522ff60539ad0`.

`StarlarkRuleTransitionProvider` proves raw/configured attribute projection,
selector-output omission and patch-only rule transitions.
`StarlarkAttributeTransitionProvider` proves that dependency transitions use
the owning rule's final configured raw attrs and are intrinsically split.
`StarlarkDefinedConfigTransition` proves all accepted return shapes, declared-
spelling output validation and canonicalization. `FunctionTransitionUtil`
proves declared settings, typed application and native list conversion.
`RuleTransitionApplier` and
`TargetAndConfigurationProducer` prove ordering, double application,
identity/idempotent/non-idempotent classification and delegation.
`ConfiguredTargetKey` proves the apply bit belongs in equality/hash while its
false form is a rare subtype optimization. `PlatformOptions` proves first-label
normalization and Host fallback. Reuse the named Bazel tests for bad return,
input/output settings, configurable attrs, Label-shaped attrs, build-setting
types/default elision, platform list conversion, no-op forms, attribute-only
invalidation and set-normalized configuration convergence.

Authentic consumer evidence is rules_rust 0.73 `rust/private/rust.bzl`, SHA-256
`a645bd5db6344bd3c0997dcf73600475c0af53fb4dd025890be24b8e1e2dbfd8`.
Its static/shared/binary/test rules share an input/output `platforms` self-
transition and `_resolve_platform(settings, attr)`. It is replay evidence only,
never compatibility authority.

## Learned Slug facts and architecture decision

Loading already retains one frozen regular `TransitionDefinition` on the final
`StarlarkRuleImplementation`; setting canonicalization and rule attachment are
terminally accepted. Extend that sole definition with the transition call's
immutable `BzlModuleIdentity` and shared source-identity table. This context is
required for `Label()` calls and returned relative/apparent platform strings,
and participates in equality. Do not retain source text or add a callable
registry.

Analysis currently rejects every incoming transition before configured work.
Its narrow attribute-transition function independently executes only an
input-free, one-root-build-setting patch. Extract one synchronous
`starlark_transition` evaluator used by both rule and attribute callers. It
accepts DICE-prepared typed setting rows and raw resolved attributes, allocates
the declared-key settings dictionary and transition attr struct, evaluates the
frozen function in its Bzl source context, validates generic return/output
shape and produces typed phase-scratch patches. It does not load packages,
mutate DICE, retain evaluator values or publish configuration.

DICE remains the sole asynchronous owner. A preparation helper loads direct
build-setting declarations and prepares selector facts plus the immutable
transition `attr` object. Each application projects fresh effective setting
inputs from its own source configuration, invokes the synchronous evaluator,
converts every typed output, atomically constructs a candidate
`ConfigurationKey`, and validates any changed effective target platform through
the existing configured-platform DICE owner before publication. Thus the
idempotency check reuses declarations and attrs but observes the first result's
settings. No lock is held across a DICE computation. The existing attribute-
transition producer uses the same helper but retains its one-result boundary.

Add one `should_apply_rule_transition` bit to `ConfiguredTargetKey`; `new`
defaults true and a crate-private final constructor sets false. Include the bit
in Eq/Ord/Hash/Allocative and DICE keys, omit it from `stable_serialize`, and
prove that its only production false construction follows a detected
non-idempotent rule transition. In the configured Starlark-rule driver, prepare
and apply before final configured attributes. Identity continues locally;
changed results run the same prepared patch a second time, then recurse through
the ordinary observed configured-result function using the idempotency-selected
key. The delegated value, not the pre-transition shell, is returned.

Extend `SlugConfiguration` only with borrowed/functional target-platform
transition projections over its existing private `OptionRecord` vector.
Canonical labels are converted before publication; clone the vector only on a
semantic change and retain its existing `Arc` otherwise. Future native-setting
families extend this projection seam and the same evaluator row rather than
adding maps, strings or per-transition configuration overlays.

Retained cost is the definition-source/source-table `Arc`s already shared by
the owning module and one compact key bit (expected to fit existing padding).
Every settings/attrs/output dictionary, declaration vector and second-apply
result is phase scratch. Keep `CompactString`, `Arc` slices, `SmallMap`/
`SmallSet`, canonical labels, normalized `StarlarkOptions`, `Dupe` and
`Allocative`. Add no `HashMap`/`HashSet`, interner, global cache, source-text
identity or flattened option dictionary. Stage 9 needs no new utility-adoption
row.

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` remains peer
guidance only. Its typed `RuleConfigurationTransition`, explicit
`should_apply_rule_transition` owner bit, separate pre/final configuration
services and retained failure provenance support the same ownership split.
Slug adopts no Zig code, IDs, allocator, DICE engine, scheduler, cache, limits,
tests or behavior.

## Implementation boundary, proofs and stops

Production allowlist:

- `app/slug_loading_v2/src/attrs.rs`, `package.rs`, `provider.rs` and
  `starlark_label.rs` for the sole source-aware transition definition and a
  lightweight transition evaluation context;
- `app/slug_configuration_v2/src/native/configuration.rs` and the smallest
  existing export module for borrowed/functional target-platform projection;
- `app/slug_analysis_v2/src/key.rs`, `configured_attribute.rs`, `dice.rs`,
  `lib.rs` and one new `starlark_transition.rs` for the key bit, selector-output
  predicate, DICE preparation/delegation and shared synchronous evaluator.

Proof allowlist:

- focused existing loading and configuration unit/integration owners for
  transition-source retention and platform projection;
- `app/slug_analysis_v2/tests/starlark_rule.rs` plus the smallest internal
  transition/key unit owner;
- existing attribute-transition tests only where sharing the evaluator changes
  their proof surface;
- `tests/v2_oracle/fixtures/rule-level-starlark-transition-execution/` with
  fixed `fixture.toml`, `workspace/{MODULE.bazel,BUILD.bazel,defs.bzl}` and
  `expected/oracle.json`; and
- the existing `tools.v2_oracle` harness without harness-code changes.

The fixture uses `comparison = "message_shape"`, Bazel 9.2 commit provenance,
the pinned sources above and generation command
`python3 -B -m tools.v2_oracle run --fixture
rule-level-starlark-transition-execution --tool bazel --bazel /usr/bin/bazel
--update-expected`. Fixed rows prove settings plus raw attrs; identity,
idempotent and non-idempotent outcomes; selector re-resolution after the rule
transition; direct typed Starlark inputs/multiple outputs; effective/default
elision; empty and explicit target-platform results; composition with one
attribute transition; missing/extra/bad outputs; selector-output omission; and
split-rule rejection. Platform targets have empty flags/policy.

The touched `package.rs` and `dice.rs` exceed the size trigger. Their edits are
limited to transfer and orchestration; evaluation belongs in the new module and
platform mutation stays with configuration. Add no helper over 140 lines and
no more than 20 lines to an already oversized function. Caps are 760 net /
1,050 gross production Rust lines, 900 net / 1,300 gross proof Rust lines and
2,350 total gross Rust lines.

Focused proofs cover:

1. transition definition source/mapping retention through live, frozen,
   imported, final rule and attribute ownership, including equality and A/B/A;
2. `ConfiguredTargetKey` apply-bit equality/hash/size, unchanged display,
   default-true construction and non-idempotent-only false construction;
3. settings dictionaries for every admitted Starlark value plus effective
   `platforms`, declared spelling, multiple input/output ordering and default
   elision;
4. raw attr values for all admitted kinds, Label rather than Target shape,
   ordinary selector resolution, output-reading selector omission and final
   post-transition re-resolution;
5. patch-dict, one-entry dict-of-dicts and one-entry sequence returns, all
   identity forms, complete output validation, wrong type,
   missing/non-setting declaration, evaluation failure and multi-result split
   rejection before configured work;
6. identity local completion, fresh second-application settings, idempotent
   true-bit convergence and non-idempotent false-bit single application,
   including two incoming configs converging on one final key;
7. rule-before-attribute-transition composition, attribute-transition access
   to final settings/raw attrs, and final configuration use by dependencies,
   toolchains, fragments, implementation, providers and action owner identity;
8. target-platform Host fallback, explicit Host equivalence, alternate/empty,
   scalar string, string/Label sequences, scalar-Label rejection, first-member
   normalization, invalid label, missing/non-platform target and nondefault-
   platform-policy failure;
9. same-DICE implementation, attribute, setting-default and input A/B/A with
   exact restoration cutoff and no warm replay; and
10. unchanged unsupported gates for other native options, platform flags,
    build-setting aliases, split attribute edges and configured aspects.

Run focused tests first, then serial complete
`cargo test -p slug_configuration_v2`, `cargo test -p slug_loading_v2`,
`cargo test -p slug_analysis_v2`, `cargo test -p slug_query_v2`, the generated
and verified Bazel fixture, `cargo fmt --all -- --check`, metadata, diff,
archive-status and source/hash gates. Rebuild `slug_cli_v2`, clean stale `slugd`,
and replay a fresh authenticated rules_rust 0.73 workspace. The replay must
clear the rule-transition gate without parser, ruleset or C++ special cases and
stop at the next honest independent boundary. Clean `slugd` again.

Stop and `REPLAN` before Rust if independent review rejects the key/control
identity, source-context owner, selector-output rule, target-platform boundary,
shared evaluator, proof discriminators, allowlist or caps. During
implementation stop on any need for a second configuration store, opaque
native option map, unbounded delegation, held lock across DICE, platform flag
guess, alias bypass, split-edge flattening, rules_rust/`cc_common` branch,
parser change, source-text identity, harness mutation or out-of-allowlist
production edit.
