# Current Slug V2 Packet

Packet: `WP-6-7A-provider-constrained-dependency-implementation-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 4 retained attribute
schema and Stage 6 configured dependency validation.

Status: terminal implementation rereview returns `ACCEPT`. R1 architecture
review returned `REVISE`: alias-to-file lacked a lawful actual-kind owner and
the permanent oracle had no path/provenance allowlist. Corrected R2 explicitly
defers Slug's broader alias-to-file configured-identity gap and freezes a
bounded five-constructor fixture; focused rereview returned `ACCEPT`. The
implementation's first terminal review required complete dependency-label/DNF
diagnostics, a multi-provider conjunction discriminator and a generating-rule
failure distinct from the generated-file exemption. The corrected proofs and
production path satisfy all three findings. The rebuilt authentic rules_rust
0.73 replay clears `rust/private/rust.bzl`'s provider-constrained `link_deps`
and stops at the independent rule-level Starlark transition-execution frontier.

The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Observable result and compatibility classification

Implement one generic architecture that admits provider-constrained target
invocation and validates the configured provider collection for every label
edge. The category covers exactly the five Bazel 9.2 constructors exposing
`providers`: `attr.label`, `attr.label_list`,
`attr.string_keyed_label_dict`, `attr.label_keyed_string_dict`, and
`attr.label_list_dict`.

Exact behavior:

- all five constructors bind the named-only `providers` parameter with Bazel's
  empty default. A flat sequence is one conjunction; a nested sequence is a
  disjunction of conjunctions. Provider order and duplicates are semantic
  sets. An empty outer sequence or any empty conjunction means no restriction;
- builtin and exported user providers share the existing `ProviderIdentity`.
  Unexported providers, nonproviders and mixed flat/nested shapes fail during
  declaration evaluation. The same canonical DNF survives descriptor freeze,
  rule freeze, target invocation and package publication;
- target invocation accepts a rule carrying a nonempty provider predicate.
  Every configured rule dependency reached through the constrained attribute
  is checked after dependency analysis and before the owner implementation
  runs. It succeeds when every provider in any one conjunction is present in
  its configured provider collection; otherwise analysis fails on that
  attribute and dependency. Source files and generated output-file targets
  that pass the independent file-admissibility policy are not rejected for
  missing providers;
- scalar, sequence and all three dictionary label orientations validate every
  contained dependency. Empty values have no dependency to validate. Select
  resolution and every already-admitted target/Exec/Starlark transition
  continue through the existing configured-dependency producer. Aliases to
  configured rules use the already-forwarded provider collection and are
  checked against it; and
- provider-policy changes participate in package equality and DICE
  invalidation. Same-DICE A/B/A must recompute on a changed predicate and cut
  off on exact restoration.

Slug-native behavior:

- the retained policy is the existing canonical
  `Arc<[Arc<[ProviderIdentity]>]>`, shared from the frozen descriptor into the
  package-owned `AttributeSchema` and cloned by `Arc` into phase-scratch edge
  validation. Slug does not reproduce Java collection/object identity or
  serialization bytes; and
- diagnostics retain the attribute, dependency label and missing predicate
  structure, but Rust formatting is not claimed byte-for-byte identical to
  Bazel's Java `StringUtil.joinEnglishList` decoration.

Unsupported/deferred behavior:

- `aspects` attachment remains fail-closed on all label-bearing constructors.
  Required-aspect expansion, direct duplicate/reverse-required validation,
  aspect parameters, applied-aspect DICE identity and configured aspect
  execution remain one later complete category. Removing the provider gate
  must not remove the independent aspect gate;
- `allow_rules`, `skip_validations`, materializers, dormant dependencies,
  `for_dependency_resolution` and their option gates remain separate complete
  attribute-policy categories. No implicit bypass is inferred from them;
- aliases to source or generated output files remain unsupported/deferred with
  Slug's broader alias-to-file configured-identity gap. The current alias
  result retains `ConfiguredNodeKind::Alias` and only a configured-target
  actual identity, so this packet neither guesses file status from an empty
  provider set nor adds a partial actual-node carrier. The Bazel oracle records
  the required future exemption;
- configured provider publication stays within the already accepted rule and
  standard-provider surfaces. This packet does not add `CcInfo`,
  `rust_common`, `cc_common`, `cc_internal`, rules_rust behavior, a native/C++
  rule, parser grammar, `set`, action, query projection or execution fallback.
  Bazel 9 BCR Starlark remains the rule-body producer.

## Bazel 9.2 authority and evidence

Bazel tag `9.2.0` commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole semantic authority.
Pinned source SHA-256 values are:

- `StarlarkAttrModuleApi.java`:
  `af70c851882fa049034184dbb6f6580731cfa738d79dfb8abcf61af176257670`;
- `StarlarkAttrModule.java`:
  `388421c44c623c1c6625fd9f2b059d2a7d1e13b8d45e7c96173f24866a917967`;
- `RequiredProviders.java`:
  `b6032c80271686c9ba1ac1f8d05c8b187c524e11e7b0831e433f408c7c40e5d3`;
- `RuleContext.java`:
  `0f6dcffac7286a9056d050624bd29e73cefc4138dd9dc24708dec63e147b41e2`;
- `StarlarkRuleClassFunctionsTest.java`:
  `e09c93616e096d639ec69b6b0c6a397a8a36bc8a95fa21b986cb5fc7f8f010aa`;
  and
- `StarlarkRuleContextTest.java`:
  `d195e5d49aae52a92bd3abebfc8de7942aacb252b522cea315985d41277f082d`.

The public API proves the exact five-constructor surface. `StarlarkAttrModule`
proves flat/nested conversion and empty-conjunction collapse.
`RequiredProviders` proves any-of/all-of satisfaction and missing-provider
projection. `RuleContext.checkRuleDependencyMandatoryProviders` proves that
validation uses the configured target's actual provider collection during
analysis. Reuse `StarlarkRuleContextTest`'s flat/nested label-list and
label-keyed/list-dict success/failure cases. A focused Bazel 9.2 workspace now
proves that custom-provider constraints reject a plain rule but accept source
and generated output files, while `DefaultInfo` accepts those same files and a
custom-provider alias forwards its rule provider collection. Preserve that
theme in the permanent `provider-constrained-dependencies` fixture described
below; alias-to-file remains oracle-only unsupported evidence. Do not copy an
unrelated rules_rust workspace into the fixture.

Authentic consumer evidence is rules_rust 0.73 `rust/private/rust.bzl`, SHA-256
`a645bd5db6344bd3c0997dcf73600475c0af53fb4dd025890be24b8e1e2dbfd8`.
It is replay evidence only, never compatibility authority.

## Learned Slug facts and architecture decision

Slug already owns the complete canonical provider DNF on live/frozen
`RuleAttributeSchema`, actual configured provider publication, and
`ConfiguredDependencyValidation.required_providers`. The validator already
implements any-alternative/all-members matching for late-bound and subrule
dependencies, but its `DefaultInfo`-only file special case is narrower than
Bazel: provider policy is not consulted for any file prerequisite that passes
file admissibility. The other artificial gaps are that three dictionary
constructors do not bind `providers`, target invocation rejects every nonempty
predicate, the final package `AttributeSchema` drops the DNF, and ordinary
dependency rows therefore pass an empty predicate to the existing validator.

Add the shared DNF to `AttributeSchema`, including its derived structural
equality, borrowed accessor and builder. During target invocation, clone the
already-canonical `Arc` from `RuleAttributeSchema` into that final schema.
Bind all three missing constructor parameters through the existing
`declaration_required_providers` parser. In `root_declared_dependency_keys`,
clone the final schema predicate into the existing
`ConfiguredDependencyValidation`. Correct that shared validator to bypass the
provider predicate for source/generated file nodes and otherwise evaluate the
actual configured provider collection; make no second validator or side
table. Narrow `reject_deferred_attribute_invocation` to attached aspects only.

The final package and configured-analysis DICE owners already observe
`StarlarkRuleImplementation` structural equality, which includes its schema.
No key, lock, filesystem read, request overlay, repository mapping, command
option, cache or global registry changes. Overlapping requests continue to use
ordinary DICE dependency recording and immutable package results.

Retained memory is one extra shallow `Arc` field per published attribute
schema; empty predicates share an empty slice and dependency-edge copies are
phase scratch. Keep `CompactString`, immutable `Arc` slices, canonical
`ProviderIdentity`, `Allocative`, and existing small deterministic parsing
scratch. Add no `HashMap`/`HashSet`, interner, flattened provider bitmap or
source-text identity. This is reuse of already-adopted Buck2 utility patterns,
not a new Buck2/V1 import, so Stage 9 needs no new adoption row.

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` remains peer
guidance only. Its typed provider-class and immutable declaration/application
separation support keeping provider policy on the attribute schema and actual
provider membership on configured results. Slug adopts no Zig IDs, allocator,
scheduler, graph, limits, tests or behavior.

## Implementation boundary, proofs and stops

Production allowlist:

- `app/slug_loading_v2/src/attrs.rs` for final typed schema ownership;
- `app/slug_loading_v2/src/package.rs` for complete constructor binding,
  transfer at target invocation and the independent aspect-only fail-closed
  gate; and
- `app/slug_analysis_v2/src/dice.rs` for the ordinary configured-dependency
  handoff to the existing validator; and
- `app/slug_analysis_v2/src/subrule.rs` for the shared exact rule-versus-file
  provider-validation branch.

Proof allowlist:

- `app/slug_loading_v2/src/host_package_load_tests.rs` and existing loading
  integration/invalidation tests; and
- `app/slug_analysis_v2/tests/starlark_rule.rs` or the smallest existing
  configured-target test owner for configured provider validation;
- `app/slug_analysis_v2/tests/subrule.rs` only to update the existing shared
  validator regression from the prior DefaultInfo-only file rule to Bazel's
  direct-file exemption;
- `tests/v2_oracle/fixtures/provider-constrained-dependencies/fixture.toml`,
  `workspace/{MODULE.bazel,BUILD.bazel,defs.bzl}` and
  `expected/oracle.json`; and
- the existing `tools.v2_oracle` harness without harness-code changes.

The fixture uses `comparison = "message_shape"`, Bazel 9.2 commit provenance,
the pinned sources above and the generation command
`python3 -B -m tools.v2_oracle run --fixture provider-constrained-dependencies
--tool bazel --bazel /usr/bin/bazel --update-expected`. Its bounded rows are:
all-five-constructor success; flat/nested and builtin/user success; missing
rule provider failure; source and generated-output file exemption; rule-alias
success/failure; empty-conjunction success; and per-entry dictionary failure.
Invalid mixed/nonprovider/unexported declaration shapes remain pinned-source
Rust regressions: one immutable three-file workspace cannot selectively
evaluate multiple invalid top-level declarations, and mutating the fixture
would contradict this packet's fixed-workspace proof boundary. Alias-to-file
remains unsupported/deferred and therefore is not a Slug replay row. No
mutation, repository download, action execution or copied ruleset is needed.

The touched `package.rs` and `dice.rs` files exceed the size trigger, but each
change remains a small extension of its existing declaration/invocation and
configured-edge producer. Extracting only these lines would split ownership
and widen private interfaces. Add no helper over 140 lines and no more than 20
lines to an already oversized function. Caps are 180 net / 300 gross
production Rust lines, 360 net / 520 gross proof Rust lines and 820 total gross
Rust lines.

Focused proofs cover:

1. the exact five-constructor named-only ABI, flat/nested/builtin/user/empty,
   canonical reorder/duplicate identity and all invalid shapes;
2. live/frozen/imported/final schema retention and package equality, including
   same-DICE A/B/A on a policy-only change;
3. successful and failing configured dependencies for one conjunction and
   alternatives, actual returned versus merely advertised providers, rule
   aliases, direct source files, generated output files and generating rules;
4. scalar/list and all three dictionary orientations, proving every contained
   label receives the same predicate and empty values are neutral;
5. explicit separation from aspect attachment: provider-only invocation
   succeeds while every still-unadmitted aspect-bearing form remains
   fail-closed; and
6. rebuilt fresh rules_rust replay clears `link_deps` provider policy and
   records the next independent generic frontier.

Validation is serial: focused oracle and Rust tests, complete loading and
analysis suites, one loading-query dependent, `cargo fmt --check`, Cargo
metadata, `git diff --check`, archive status, pinned hashes, clean Bazel/Buck2/
Zabel, parked-file hash, `cargo build -p slug_cli_v2`, stale-`slugd` cleanup
and fresh authentic replay. Corrected R2 architecture pre-review is accepted;
independent terminal cross-crate retained-representation review is required.

There is no fallback. `REPLAN` before Rust if exact satisfaction requires a
second provider identity, provider policy cannot live in package schema
equality, ordinary and hidden dependencies need divergent validators, actual
provider publication is unavailable at the existing validation point, or the
complete five-constructor family requires an admitted aspect/materializer/
dependency-resolution bypass, or the authentic frontier requires the deferred
alias-to-file category. `REPLAN` during implementation if any
constructor drops policy, a policy-only change fails to invalidate, validation
runs before the dependency's configured providers exist, caps are exceeded,
or authentic replay contradicts the pinned category.

## Immediate predecessor

Commit `096653548` terminally accepts
`WP-6-7A-generic-aspect-declaration-implementation-r1`: complete generic
aspect declarations retain every admitted input, pass terminal review and
full validation, and advance the authentic replay to provider-constrained
ordinary dependency invocation.

## Terminal result

One canonical provider DNF now survives descriptor, frozen rule, final package
schema and ordinary configured-edge ownership across exactly the five Bazel
9.2 constructors. Direct files bypass provider policy after independent file
admissibility; configured rules and rule aliases validate actual providers
through the shared ordinary/hidden validator. The permanent Bazel 9.2 oracle,
focused Rust proofs, complete loading/analysis/query suites, metadata, format,
diff, archive, pinned-source, clean-reference and parked-file gates pass. The
production delta is 42 net / 66 gross Rust lines, the proof delta is 211 net /
239 gross, and total gross is 305, all below the frozen caps. Independent
terminal rereview returns `ACCEPT`. A fresh rebuilt one-shot rules_rust replay
reaches `rule-level Starlark transition execution is not supported for
@@//pkg:probe`, proving `link_deps` is cleared without widening this packet into
transition execution.
