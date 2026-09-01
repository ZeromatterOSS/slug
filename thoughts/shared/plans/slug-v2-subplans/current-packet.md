# Current Slug V2 Packet

Packet: `WP-6-7C-attribute-property-flag-category-implementation-r3`

Milestone: M7A generic Starlark/ruleset closure; complete dependency-attribute
property-flag architecture.

Status: independent architecture review and focused correction rereviews
return `ACCEPT`. Rust is authorized only within the frozen R3 boundary.

The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Replay-selected objective

Commit `e267ae86b` terminally accepted the complete module-extension metadata
construction/capture category. The authentic rules_rust 0.73 configured-query
replay clears `module_ctx.extension_metadata(reproducible=True)` and now stops
while loading rules_shell at the generic dependency-attribute property flag
`SKIP_CONSTRAINTS_OVERRIDE`.

Design the complete Bazel 9.2 `attr.*(flags=...)` declaration and retained
property category, not that flag or consumer. The design must prevent one field
or parser branch per future flag while refusing to claim unimplemented
downstream effects. C++, `cc_common`, `cc_internal`, rules_shell and rules_rust
remain replay consumers only; Bazel 9 BCR Starlark rules own their semantics.

## Bazel 9.2 category

The public `flags` keyword exists on exactly five constructors:

- `attr.label`;
- `attr.label_list`;
- `attr.string_keyed_label_dict`;
- `attr.label_keyed_string_dict`; and
- `attr.label_list_dict`.

It is named-only, defaults to `[]`, and accepts a Starlark `Sequence[str]`.
Bazel casts the complete sequence before resolving property names, so a later
non-string beats an earlier unknown name. Lists and tuples are accepted,
duplicates collapse set-like, order is not retained semantically, and an
unknown string fails as `unknown attribute flag '<name>'`.

`Attribute.PropertyFlag` contains the complete 25-name set:

- `MANDATORY`, `EXECUTABLE`, `UNDOCUMENTED`, `TAGGABLE`;
- `ORDER_INDEPENDENT`, `STRICT_LABEL_CHECKING`,
  `DIRECT_COMPILE_TIME_INPUT`, `NON_EMPTY`, `SINGLE_ARTIFACT`,
  `SILENT_RULECLASS_FILTER`, `SKIP_ANALYSIS_TIME_FILETYPE_CHECK`;
- `CHECK_ALLOWED_VALUES`, `NONCONFIGURABLE`,
  `CONFIGURABLE_ATTR_WAS_USER_SET`, `SKIP_PREREQ_VALIDATOR_CHECKS`;
- `CHECK_CONSTRAINTS_OVERRIDE`, `SKIP_CONSTRAINTS_OVERRIDE`,
  `OUTPUT_LICENSES`;
- `HAS_STARLARK_DEFINED_TRANSITION`, `HAS_ANALYSIS_TEST_TRANSITION`,
  `IS_TOOL_DEPENDENCY`, `STARLARK_DEFINED`, `SKIP_VALIDATIONS`;
- `FOR_DEPENDENCY_RESOLUTION`; and
- `FOR_DEPENDENCY_RESOLUTION_EXPLICITLY_SET`.

The raw Starlark `flags` path calls `setPropertyFlag` directly. It therefore
admits flags normally derived by another keyword and does not run the Java
builder helpers' mutual-exclusion assertions. In particular both constraint
override bits may coexist; the analysis consumer's skip-first branch wins.

The design audit records this complete effect map before choosing an
implementation cohort:

| Property | Bazel setter / principal consumer | Current Slug disposition |
|---|---|---|
| `MANDATORY` | `mandatory`; BUILD required-value check | existing `mandatory`, reconcile exactly |
| `EXECUTABLE` | `executable`; configured executable validation | existing `executable`, reconcile exactly |
| `UNDOCUMENTED` | implicit/private name; docs/publicity | owner deferred |
| `TAGGABLE` | raw/Java builder; rule tag collection | owner deferred |
| `ORDER_INDEPENDENT` | raw; list conversion normalization | existing `order_independent`, reconcile exactly |
| `STRICT_LABEL_CHECKING` | file/rule predicates; legacy dependency checking | file/provider slice exists; legacy rule-class policy deferred |
| `DIRECT_COMPILE_TIME_INPUT` | raw; `compile_one_dependency` selection | retained today; command effect deferred |
| `NON_EMPTY` | `allow_empty=False`; value validation | existing `allow_empty`, reconcile exactly |
| `SINGLE_ARTIFACT` | `allow_single_file`; configured files-to-build check | existing file policy, reconcile exactly |
| `SILENT_RULECLASS_FILTER` | raw/Java builder; prerequisite filtering | owner deferred |
| `SKIP_ANALYSIS_TIME_FILETYPE_CHECK` | raw; configured prerequisite file check | existing validator; connect exactly |
| `CHECK_ALLOWED_VALUES` | `values`; scalar allowed-value validation | raw bit on these five constructors has no predicate; retain and fail closed before a predicate consumer |
| `NONCONFIGURABLE` | `configurable=False`; selector admission | existing `configurable`, reconcile exactly |
| `CONFIGURABLE_ATTR_WAS_USER_SET` | explicit `configurable`; rule-class validation | explicitness bit absent; retain, consumer deferred |
| `SKIP_PREREQ_VALIDATOR_CHECKS` | raw/Java builder; visibility/prerequisite validator | owner deferred |
| `CHECK_CONSTRAINTS_OVERRIDE` | raw/Java builder; dependency constraint selection | owner deferred; no partial checker |
| `SKIP_CONSTRAINTS_OVERRIDE` | raw/Java builder; skip-first constraint selection | retain exactly; constraint owner deferred |
| `OUTPUT_LICENSES` | raw/Java builder; license checking | owner deferred |
| `HAS_STARLARK_DEFINED_TRANSITION` | Starlark `cfg`; rule transition summary | transition owner exists; reconcile identity, aggregate effect audit required |
| `HAS_ANALYSIS_TEST_TRANSITION` | analysis-test `cfg`; rule analysis-test path | owner deferred |
| `IS_TOOL_DEPENDENCY` | raw; constraint/instrumentation/aspect classification | owner deferred |
| `STARLARK_DEFINED` | every `attr.*` descriptor; macro/docs/inheritance | marker absent; retain, consumer audit required |
| `SKIP_VALIDATIONS` | `skip_validations=True`; transitive validation actions | owner deferred |
| `FOR_DEPENDENCY_RESOLUTION` | sibling keyword/raw; dependency-resolution rule checks | owner deferred |
| `FOR_DEPENDENCY_RESOLUTION_EXPLICITLY_SET` | sibling keyword presence/raw; rule-class consistency | explicitness absent; owner deferred |

The audit also includes the public sibling keywords that derive these bits,
especially `for_dependency_resolution` and `skip_validations`, even if their
effects remain deferred. Otherwise a raw-flags carrier would become a second
truth source when those Bazel 9 surfaces are admitted later.

## Compatibility boundary to freeze

Admit as **exact** in the first implementation successor:

- the five-constructor binding, complete-sequence type validation, all 25
  recognized spellings, unknown-name failure, duplicate collapse and set-like
  equality/invalidation;
- `for_dependency_resolution` on all five constructors and
  `skip_validations` on `label`, `label_list`, `label_keyed_string_dict` and
  `label_list_dict`, including their Bazel defaults, types and ordered property
  mutations; `string_keyed_label_dict` continues to reject
  `skip_validations` at binding;
- one compact retained property set that survives descriptor freeze, rule and
  aspect export/import, schema projection and A/B/A DICE restoration;
- reconciliation with already-admitted Slug fields so raw `MANDATORY`,
  `EXECUTABLE`, `ORDER_INDEPENDENT`, `NON_EMPTY`, `SINGLE_ARTIFACT`,
  `NONCONFIGURABLE` and `DIRECT_COMPILE_TIME_INPUT` cannot disagree with the
  schema behavior those bits already control; and
- `SKIP_ANALYSIS_TIME_FILETYPE_CHECK` at the existing configured dependency
  validator, preserving direct source-file admissibility while bypassing only
  generated/rule files-to-build checks exactly.

Admit declaration/capture, but classify the downstream effect as
**unsupported/deferred**, for flags whose Bazel consumer does not yet exist in
Slug: documentation/tag sets, silent rule-class filtering, prerequisite and
transitive validation actions, dependency constraint compatibility,
output-license checking, analysis-test transitions, tool-dependency
instrumentation/aspect propagation and dependency-resolution-only rule
validation. Every bit remains structural input; no flag may be silently
discarded. The implementation packet must include an explicit effect table and
either connect an existing exact consumer, prove the effect is presently
unobservable because its whole owner is deferred, or fail closed at the first
admitted consumer boundary.

`CHECK_CONSTRAINTS_OVERRIDE` does not authorize inventing partial dependency
constraint checking. `SKIP_CONSTRAINTS_OVERRIDE` may be retained and may pass
the current rules_shell replay because Slug has no admitted ordinary dependency
constraint checker to bypass; this is not a parity claim for the missing
checker. Exact constraint selection remains a later coherent analysis packet.

Keep Rust valid-Unicode strings, starlark-rust binder/error decoration, compact
bit storage and DICE scheduling/accounting **Slug-native**.

## Frozen architecture candidate

Replace the one-bit `AttributeFlags(u32)` API with one complete typed
`AttributePropertyFlags(u32)` owner in `slug_loading_v2::attrs`. Assign one
stable internal bit to every Bazel 9.2 property name and expose only named
insert/query operations required by current consumers plus a support/effect
classification method. Do not retain the source list, duplicate count or
order. Do not create 25 booleans, strings in frozen schemas, another flags
value in analysis, a cache, interner or DICE key.

Parse once in the common five-constructor helper after starlark-rust has fully
unpacked the sequence. Preserve the property set through the existing
`AttributeDefinition` and `AttributeSchema` path. Produce one canonical final
set by applying Bazel's fixed `buildAttribute` mutations in source order after
the raw set: keyword setters may add bits, and
`for_dependency_resolution=False` specifically removes
`FOR_DEPENDENCY_RESOLUTION` while adding
`FOR_DEPENDENCY_RESOLUTION_EXPLICITLY_SET`. Do not use a commutative union or
add parallel truth sources. Starlark keyword spelling order cannot affect the
result. A property bit remains present when its effect is projected into an
existing optimized field because Bazel attribute equality includes the final
complete property set.

Rules and aspects use the same descriptor owner and retain the same final set.
Bazel symbolic macros retain direct descriptor flags and clone the complete
`Attribute` property set during `inherit_attrs`; Slug must remove its blanket
inherited-flags rejection and carry the set in `MacroAttributeSchema`, while
keeping computed/late-bound and reserved-name restrictions unchanged.

Bazel subrules accept flagged `attr.label` and `attr.label_list` descriptors
under their existing private-name, default-required, target/exec-transition
and type restrictions, then lift the same descriptor into the owning rule or
aspect. Slug must replace its blanket nonempty-flags rejection with exact set
retention in `SubruleAttribute` and `LiftedSubruleAttribute`. Unknown/type
errors still occur during descriptor construction before subrule validation.
All recognized flags retain identity; only effects admitted by this packet run
at analysis, and deferred consumers remain classified as above. Bazel tag
classes and repository rules can consume ordinary label-bearing descriptors
with their property sets unchanged. Slug already accepts their unflagged
descriptor forms but currently rejects nonempty flags during conversion; this
packet preserves that unsupported/fail-closed flagged-conversion boundary and
must prove it without rejecting or changing the existing unflagged forms.

The implementation successor production allowlist is exactly:

- `app/slug_loading_v2/src/attrs.rs`;
- `app/slug_loading_v2/src/package.rs`;
- `app/slug_loading_v2/src/subrule.rs`;
- `app/slug_analysis_v2/src/dice.rs`; and
- `app/slug_analysis_v2/src/subrule.rs`.

Focused proof may use colocated tests plus
`app/slug_loading_v2/src/host_package_load_tests.rs`,
`app/slug_loading_v2/tests/build_file_loading.rs`,
`app/slug_loading_v2/tests/subrule_loading.rs`, and
`app/slug_analysis_v2/tests/starlark_rule.rs`. Gross caps are 650 production
Rust lines, 900 proof Rust lines and 1,550 total. Replan before adding
configuration, toolchain, instrumentation, license, query, C++, or ruleset-
specific production files.

## Source authority and peer guidance

Bazel tag `9.2.0` commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole semantic authority:

- `Attribute.java`
  `fbe208c37ad4ed88030f874fa6cd8bd5cf2f4aac63f9a01a4ff24ca499c9a6a4`;
- `StarlarkAttrModule.java`
  `388421c44c623c1c6625fd9f2b059d2a7d1e13b8d45e7c96173f24866a917967`;
- `RuleContextConstraintSemantics.java`
  `8ae242015971a1e9434a54214a520ef876a3d73712dd2131571c401253fc4090`;
- `StarlarkAttrModuleApi.java` constructor signatures; and
- `StarlarkRuleClassFunctionsTest.java`
  `e09c93616e096d639ec69b6b0c6a397a8a36bc8a95fa21b986cb5fc7f8f010aa`.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance only.
`build_rule_declaration.zig`
`f2221daad6d0ad61177d860e58faf3ade1bb249cce9789d7150f22bc18804fcd`
independently uses one complete recognized-name table, full-sequence type
precedence and compact derived properties; `build_single_host_capture.zig`
`d59939fcf55e5e039dc7c5f3bccf014036682c6deccbcb008b92fc89c95e7a75`
shows the same real `SKIP_CONSTRAINTS_OVERRIDE` consumer. Copy no Zig behavior,
allocator, evaluator, diagnostic, cache or unsupported-effect claim.

## Required design evidence and stop conditions

The 25-row table above is the required effect ledger. The implementation must
prove constructor inclusion/exclusion, complete-cast-before-name failure
order, duplicates/set equality, Bazel-ordered raw/keyword mutations (including
the false removal case), rule/aspect/macro/subrule retention and A/B/A identity.
The file-type-skip proof must discriminate direct source rejection, generated
suffix bypass and ordinary rule-output bypass. Add downstream proof only for
effects admitted exact. Add tag-class and repository-rule boundary rows proving
that their unflagged label descriptors remain accepted while flagged variants
fail closed at the existing conversion boundary.

Stop with `REPLAN` if any admitted bit needs a second semantic owner, if accepting a
flag would silently bypass an existing Slug validation, if macro/subrule
behavior cannot be bounded from pinned source, or if the live rules_shell
consumer requires dependency-constraint semantics that Slug does not own.

Independent architecture review is required before this design can become an
implementation packet.

R1 independent review returned `REPLAN`: the skip-filetype flag affects only
generated/rule outputs, final bits follow ordered mutations rather than union,
raw `CHECK_ALLOWED_VALUES` has no predicate on the five constructors, Bazel
lifts flagged subrule descriptors, and implementation needed frozen caps and
allowlists. R2 corrects exactly those points and also freezes symbolic-macro
retention. Focused R2 rereview `REPLAN`s only an incorrect claim that tag and
repository schemas exclude the descriptors; the correction records Bazel's
acceptance and preserves Slug's narrower flagged-conversion fail-closed
boundary. Focused correction rereview returns `ACCEPT`; implement only the
frozen R3 boundary.

## Immediate predecessor

`WP-6-7B-module-extension-metadata-construction-and-capture-implementation-r2`
is terminally `ACCEPTED` in `e267ae86b` at 584 production/356 proof/940 total
gross Rust lines. It reuses lockfile-v28 facts, retains root proxies, detaches
default-normalized metadata, passes owner/dependent gates and advances the
authentic replay to this generic category. Facts persistence/reuse and module
fixups remain separate lifecycle successors.
