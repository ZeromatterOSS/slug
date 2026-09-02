# Current Slug V2 Packet

Packet: WP-6-7D-rule-class-restriction-category-implementation-r2

Milestone: M7A generic Starlark/ruleset closure; complete dependency-attribute
rule-class restriction architecture.

Status: R2 architecture `ACCEPTED`; Rust implementation is authorized only
within this packet. R1 independent architecture review returned REPLAN on the
existing alias-to-file actual-identity gap, configured-aspect execution being
deferred, and one inverted evidence phrase. R2 narrows only those runtime
claims; focused independent rereview returns `ACCEPT`.

The unrelated dirty
app/slug_loading_v2/src/registration_expansion_tests.rs proof remains parked
at SHA-256
36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a.
Do not edit or stage it.

## Immediate predecessor and observable result

Commit b9411cd61 terminally accepted
WP-6-7C-attribute-property-flag-category-implementation-r4 at 537 production,
371 proof and 908 total gross Rust lines. It owns the complete Bazel 9.2
25-property set and retains every property through rule, aspect, macro and
subrule projections. The authentic rules_rust 0.73 configured-query replay
clears that category and now stops while loading rules_shell at the generic
surface:

    attr.label_list(allow_rules = ["sh_library"])

This packet must make that declaration and the complete Bazel 9.2 rule-class
restriction category work without a rules_shell, rules_rust, C++,
cc_common, cc_internal or parser special case. Bazel 9 BCR Starlark rules are
consumers of the general loading and analysis architecture.

The observable result is:

- all five ordinary dependency constructors bind and retain allow_rules;
- rules, aspect declarations, symbolic macros and lifted subrules preserve the same
  restriction identity;
- configured dependency validation uses the prerequisite's effective rule
  class with Bazel's allow-rules/provider OR semantics;
- aliases to configured rule/non-rule targets are checked against their
  resolved actual rule class or non-rule status;
- source files, generated files and non-rule targets bypass rule-class and
  provider validation;
- the stable SILENT_RULECLASS_FILTER projections filter ctx.attr and subrule
  views without deleting declared/configured dependency topology; and
- unrestricted/restricted/unrestricted A/B/A requests restore the original
  result through ordinary DICE equality and dependencies.

## Learned Bazel 9.2 facts

Semantic authority is Bazel tag 9.2.0, commit
8220c6198837d5c13d53fea211cf3282aa12408a.

Public declaration:

- allow_rules exists on exactly attr.label, attr.label_list,
  attr.string_keyed_label_dict, attr.label_keyed_string_dict and
  attr.label_list_dict among admitted ordinary constructors;
- the experimental dormant-label constructors also expose it, but Slug has
  not admitted that experimental family;
- it is named-only, defaults to None, and accepts None or a Sequence[str];
- lists and tuples work; arbitrary strings are accepted; matching is
  case-sensitive;
- omitted and explicit None are the same unrestricted predicate;
- an explicit empty sequence is a distinct predicate that allows no rule
  class; and
- duplicates collapse and semantic equality is set-like and order-insensitive.

StarlarkAttrModule.createAttribute processes the default and materializer
before raw flags, mandatory/sibling properties, executable/cfg presence and
file policy. It then processes allow_rules, followed by values, providers, cfg
conversion and aspects. The packet preserves that failure order. The complete
allow_rules sequence is cast to strings before RuleClassNamePredicate.only
canonicalizes it.

Configured validation:

- RuleContext validates a rule dependency only when the prerequisite exposes
  a nonempty rule class. Source/generated files and package-group-like
  non-rules bypass both rule-class and provider predicates.
- A rule passes when its class matches the explicit allowed-class predicate OR
  it satisfies one required-provider alternative.
- With no explicit class predicate and no required providers, every rule
  passes. With providers but no class predicate, providers are required.
- An explicit empty predicate with no provider alternative rejects every rule.
- A mismatch with a provider mismatch reports both requirements.
- Aliases use the resolved actual target's rule class, not the alias wrapper
  class.
- Rule/provider validation precedes generated-file admissibility checks.

SILENT_RULECLASS_FILTER is part of the same rule-class predicate family, but
has a distinct projection:

- an unspecified predicate is a no-op;
- an explicit predicate removes mismatching rule prerequisites from analysis
  attribute views before ordinary prerequisite validation;
- file and other non-rule prerequisites have the empty rule-class string and
  are therefore removed unless the predicate explicitly contains the empty
  string;
- provider alternatives do not rescue a silent mismatch because filtering
  precedes provider validation;
- scalar mismatches project as None, list mismatches are omitted, and
  label-keyed-string dictionary entries are omitted;
- dependency/query topology still contains every declared prerequisite; and
- Bazel 9.2 crashes internally when a mismatch is silently filtered from
  string_keyed_label_dict or label_list_dict. The failures are respectively a
  null ImmutableMap value and a null Starlark-list element in
  StarlarkAttributesCollection.

Fresh disposable Bazel 9.2 oracle evidence at
/tmp/slug-allow-rules-oracle.YMQpXx proves None versus empty behavior,
rule/provider OR, combined diagnostics, alias-to-rule resolution, source and
generated-file bypass, silent scalar/list/label-keyed projections, alias-to-
file removal, filter-before-provider ordering and topology retention. It also
isolates both dictionary crash shapes. This is design evidence only; it creates
no committed fixture or oracle asset.

Relevant pinned sources and hashes:

- packages/Attribute.java:
  fbe208c37ad4ed88030f874fa6cd8bd5cf2f4aac63f9a01a4ff24ca499c9a6a4
- packages/RuleClass.java:
  33be32dc5c884d7fba2338f13f3bc4bcd0c175e3479c70fcd810474a5749b5e6
- analysis/RuleContext.java:
  0f6dcffac7286a9056d050624bd29e73cefc4138dd9dc24708dec63e147b41e2
- analysis/starlark/StarlarkAttributesCollection.java:
  9b3b300d7e9c25dceafc8a9450dd2511f9b0b83088e11421b6dc3b5086cc7442
- analysis/starlark/StarlarkAttrModule.java:
  388421c44c623c1c6625fd9f2b059d2a7d1e13b8d45e7c96173f24866a917967
- starlarkbuildapi/StarlarkAttrModuleApi.java:
  af70c851882fa049034184dbb6f6580731cfa738d79dfb8abcf61af176257670
- the focused Starlark rule-class, Attribute and BuildView tests named in the
  source audit.

## Compatibility decision

Admit as exact:

- declaration, type/failure order and retained identity on all five ordinary
  constructors;
- unrestricted versus explicit empty/only predicates, duplicate collapse and
  set-like equality/invalidation;
- propagation through rule, aspect declarations, macro inheritance and lifted
  subrules;
- ordinary rule and subrule-owned dependency validation;
- effective-class semantics for aliases whose actual remains a configured
  target, direct non-rule bypass and rule/provider OR;
- stable silent-filter projections for label, label_list and
  label_keyed_string_dict;
- validation/filtering order and preservation of configured/query topology;
  and
- repository-rule/tag-class conversion of unrestricted descriptors, while an
  explicit class restriction fails closed at the boundary that lacks a
  configured rule prerequisite owner.

Keep Slug-native:

- Rust valid-Unicode rule-class strings and starlark-rust diagnostic
  decoration;
- canonical sorted diagnostic class order rather than Java insertion-order
  details;
- compact structural semantic identity and normal DICE scheduling/accounting;
  and
- an explicit unsupported error instead of reproducing a Bazel JVM crash.

Classify unsupported/deferred:

- experimental dormant_label and dormant_label_list;
- Bazel's native allowed-rule-class warning predicate, which has no public
  Starlark declaration surface;
- legacy command-specific rule-class consumers outside configured dependency
  validation;
- explicit allow_rules in repository/tag schemas, whose configured
  prerequisite owner does not exist there; and
- configured aspect execution/validation, while aspect declarations retain
  exact restriction identity for that later category;
- alias-to-file effective-class/filter behavior. Slug's existing alias result
  requires an actual configured-target identity and cannot yet represent a
  source/null actual; repairing that broader identity gap is not part of this
  attribute packet; and
- a SILENT_RULECLASS_FILTER mismatch in string_keyed_label_dict or
  label_list_dict. Slug accepts the declaration and matching values, but fails
  closed during configured validation if a mismatch would enter Bazel's
  crashing projection.

The last boundary is deliberately narrower than rejecting either constructor:
ordinary non-silent validation remains exact, silent unrestricted predicates
remain no-ops, and silent restricted dictionaries with only matching rules
remain usable.

## Representation and natural semantic owners

Loading owns one new immutable value in slug_loading_v2::attrs:

    RuleClassAdmissibility::Any
    RuleClassAdmissibility::Only(Arc<[CompactString]>)

Only contains a sorted, deduplicated slice. Any is distinct from Only([]).
This makes equality, hashing, cloning, allocation accounting and diagnostics
canonical without retaining source order or duplicate count. Do not use five
booleans, a second provider predicate, a global registry, cache, interner or
DICE key.

AttributeDefinitionGen is the declaration producer. Existing
RuleAttributeSchemaGen, AttributeSchema, MacroAttributeSchema,
SubruleAttribute and LiftedSubruleAttribute projections retain the same value.
Repository/tag conversion accepts Any and fails closed on Only. No command-side
repair or consumer-specific copy is permitted.

ConfiguredDependencyValidation is the sole configured validation owner. It
borrows the schema restriction and property set to decide:

1. whether the prerequisite is silently filtered;
2. whether a crashing dictionary projection must fail closed;
3. whether a remaining rule satisfies class OR provider requirements; and
4. the existing file/executable checks in Bazel order.

ConfiguredNodeResult continues to own direct rule capability. An alias does
not rewrite that capability. Instead ConfiguredEdgeKind::AliasActual gains the
resolved actual prerequisite rule class as Option<CompactString>, and
ConfiguredNodeResult::prerequisite_rule_class returns:

- the AliasActual payload for aliases, including chained aliases;
- the direct RuleCapability class for ordinary rules; or
- None for direct files, configured non-rules and aliases to configured
  non-rules.

This retains the minimum semantic fact on the already-required alias child
edge. Alias construction already depends on the child configured result, so no
new DICE key, dependency, lock or alternate registry is introduced. It does
not repair or claim alias-to-file actual identity.

Every declared dependency remains a ConfiguredEdge. PreparedDependency gains a
phase-local filtered marker so ctx.attr and configured subrule projection can
consume a placeholder without dropping graph topology. Filtered dependencies
do not contribute executable provenance or analysis values.

## Request, revision and DICE behavior

The immutable attribute schema, resolved label/configuration and prerequisite
configured result completely determine validation. No command option,
environment read, filesystem read, repository mutation, lockfile fact or host
session input is added.

Schema equality includes RuleClassAdmissibility. Changing Any to Only or one
Only set to another invalidates the existing rule/schema consumers. Alias
effective-class changes arrive through the existing AliasActual child DICE
dependency. Equality cutoff restores the original result on A/B/A without a
fresh-graph bypass. Overlapping requests use DICE's existing key deduplication;
the packet adds no lock and holds no guard across a compute.

The design follows docs/developers/dice.md: semantic inputs remain in retained
schema/results and explicit child dependencies, with no mutable global state or
post-compute injection.

## Memory, asynchronous ownership and reuse

- The canonical Arc slice is DICE-retained semantic memory. It is published
  with the immutable schema/result, participates in equality and is released
  when those retained values are evicted.
- The optional alias class is DICE-retained semantic memory on the existing
  alias edge and follows that edge's publication/invalidation lifetime.
- Silent-filter booleans/placeholders are phase scratch owned by one configured
  analysis and are released at phase completion or cancellation.
- No evaluator heap is retained. No service cache, command cache, async
  transfer buffer, background task or shutdown hook is added.

The Buck2 utility-reuse review chooses concept/test only. Existing V2
CompactString, Arc slices and Allocative support are sufficient. SmallSet is
insertion-preserving and set-equal, but would make immutable schema clones
deep; a sorted Arc slice provides cheaper retained clones and deterministic
diagnostics. No V1 extraction is justified.

Zabel commit 0795445f3ab60f4e49070bdd0b94425c5610f73a is peer guidance only:

- build_rule_declaration.zig
  f2221daad6d0ad61177d860e58faf3ade1bb249cce9789d7150f22bc18804fcd
  demonstrates one dependency-attribute declaration slot and complete sequence
  validation;
- target_kind_projection.zig
  0e4e233a75b32988c200b2daf88659d985d02b1cc6303d944dc7df51246c0213
  keeps native and Starlark rule-class identity in loading-owned facts; and
- ordinary_dependency_facts.zig
  1048b681a1e575cbd9fc1be8b1a5b7765d19f0a72426297fcdfbaed0c7b1d24f
  demonstrates retained schema/invocation separation.

These are concept/test inputs, not compatibility authority. Copy no Zig type,
allocator, evaluator value, scheduler, cache, error or behavior. Zabel's
current non-None subrule restriction is specifically not adopted because
Bazel 9.2 admits that surface.

## Evidence and proof contract

Reuse existing property-flag, provider, file-policy, macro/subrule, alias and
A/B/A test scaffolding. Add no external fixture. Focused tests must prove:

- constructor inclusion/exclusion and named-only binding;
- default/flags/file/allow_rules/providers/cfg/aspects dual-invalid failure
  order;
- None, empty, duplicate and permuted-set identity;
- propagation through rule, aspect declaration, inherited macro and lifted
  subrule schemas;
- ordinary exact accept/reject cases on all five shapes;
- provider-only acceptance, class-only acceptance and combined mismatch;
- source/generated/package-group bypass;
- direct rule alias, alias chain and alias-to-configured-nonrule behavior;
- stable silent scalar/list/label-keyed projection, provider non-rescue and
  executable omission while every configured edge remains present;
- fail-closed mismatch behavior for the two Bazel-crashing dictionary shapes;
- repository/tag unrestricted acceptance and restricted failure;
- dependency/provider error precedence before existing generated-file checks;
  and
- unrestricted/restricted/unrestricted A/B/A restoration.

Upstream Java unit tests are not copied because their builder-internal identity
assertions are implementation details. Public Starlark behavior is covered by
the focused tests plus the disposable Bazel oracle above, which is stronger for
alias, provider and silent-filter interactions. No fixture.toml is applicable
because no fixture is committed.

There is no fallback or temporary bridge. Each unsupported combination fails
closed at its natural configured or conversion boundary.

## Scope, complexity and stops

Production allowlist:

- app/slug_loading_v2/src/attrs.rs
- app/slug_loading_v2/src/package.rs
- app/slug_loading_v2/src/subrule.rs
- app/slug_analysis_v2/src/configured_target.rs
- app/slug_analysis_v2/src/result.rs
- app/slug_analysis_v2/src/dice.rs
- app/slug_analysis_v2/src/subrule.rs
- app/slug_analysis_v2/src/starlark_rule.rs

Proof allowlist:

- colocated tests in the production files above
- app/slug_loading_v2/tests/build_file_loading.rs
- app/slug_loading_v2/tests/subrule_loading.rs
- app/slug_analysis_v2/tests/configured_target.rs
- app/slug_analysis_v2/tests/starlark_rule.rs

Gross caps are 850 production Rust lines, 1,100 proof Rust lines and 1,950
total. Documentation/routing updates do not count toward Rust caps.

package.rs and dice.rs exceed the 2,000-line review trigger. They remain the
cohesive owners because the changes are local constructor/schema lowering and
existing finish-analysis orchestration respectively. Creating a parallel
attribute factory or dependency-graph coordinator would split semantic
ownership. No function may grow past the 150-line trigger without a bounded
local helper extraction.

Do not edit configuration, toolchain, action, query, CLI, repository, BCR,
ruleset, C++, cc_common or cc_internal production code. Query topology is
validated through the existing configured edges, not a query-side patch.

Run focused loading/analysis tests, direct dependent crate tests, rustfmt,
reference hash checks, parked-proof integrity, archive baseline, rebuilt
slug_cli_v2 and authentic replay. Clean stale slugd processes before and after
daemon-sensitive validation.

Return REPLAN before Rust if independent review finds that:

- the actual rule class cannot be derived from the existing child result
  without a second semantic registry or DICE key;
- implementation would need to repair alias-to-file actual identity or execute
  configured aspects;
- silent filtering would require deleting configured/query edges;
- class/provider OR would create a second validation owner;
- macro/subrule propagation cannot use the shared schema representation;
- a stable Bazel dictionary projection differs from the oracle evidence;
- the two crash boundaries cannot fail closed at configured validation; or
- implementation exceeds the allowlist/caps or needs configuration, query,
  ruleset or C++ specialization.

Focused independent architecture rereview accepts this R2 contract. Implement
only the frozen boundary above.
