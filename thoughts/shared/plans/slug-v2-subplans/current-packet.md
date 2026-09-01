# Current Slug V2 Packet

Packet: `WP-6-7A-attribute-flags-direct-compile-input-category-parity-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 public attribute-
constructor and retained rule-attribute policy breadth.

Status: terminally accepted implementation; commit pending. Initial design review
required separating retained schema equality from source-provenance DICE
invalidation; the focused correction and matching Stage 9 wording returned
`ACCEPT`. Terminal implementation review also returned `ACCEPT` after the
package-lowering seam proof was added. Base commit `1e583e9d0` terminally accepts documentation-parameter
parity across all thirteen attribute representations Slug already owns. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked;
do not edit or stage it.

## Observable result and complete category boundary

Close the first retained member of Bazel 9.2's deprecated `attr.flags`
property family without patching rules_rust or pretending that all property
flags share one downstream semantic owner. All five constructors that expose
the named-only parameter—`label`, `label_list`, `string_keyed_label_dict`,
`label_keyed_string_dict`, and `label_list_dict`—accept omitted, empty,
list/tuple, repeated, and reordered `DIRECT_COMPILE_TIME_INPUT` spellings.
They retain one normalized property bit in the ordinary rule attribute
schema; input order and duplicates do not survive. Unknown strings, non-string
elements, non-sequences, positional binding, and `flags` on every other
constructor fail before schema publication.

The compact owner is deliberately capable of the full 25-member Bazel 9.2
property set, but this packet enables only `DIRECT_COMPILE_TIME_INPUT`.
Recognized names whose loading or analysis effects Slug does not yet own fail
closed with an unsupported diagnostic. This is narrower than Bazel's accepted
string domain and is classified accordingly; silently retaining or ignoring
those names would falsely activate configurability, validation, transition,
tool-dependency, ordering, constraint, license, or dependency-resolution
behavior. Successors add bits to the same owner only with their complete
constructor and consumer category, so no new schema/container architecture is
needed.

Rebuild `slug_cli_v2` and replay `cquery //app/slug_cli_v2:slug`. It must pass
authentic rules_rust's `attr.label_list(allow_files=[".rs"],
flags=["DIRECT_COMPILE_TIME_INPUT"])` declaration and stop at the next
unsupported boundary or succeed. Do not consume that boundary here.

This is a generated-binder, validation, and retained loading-schema packet.
It is not parser grammar, a `set` implementation, a rule implementation,
`compile_one_dependency`, query formatting, configured-edge reclassification,
an action, or a C++ builtin. Bazel 9 BCR Starlark remains the rule-body owner;
`cc_common` and `cc_internal` remain ordinary downstream consumers.

## Learned facts and authenticated evidence

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole
semantic authority. Pinned source SHA-256 values are:

- `StarlarkAttrModuleApi.java`:
  `af70c851882fa049034184dbb6f6580731cfa738d79dfb8abcf61af176257670`;
- `StarlarkAttrModule.java`:
  `388421c44c623c1c6625fd9f2b059d2a7d1e13b8d45e7c96173f24866a917967`;
- `Attribute.java`:
  `fbe208c37ad4ed88030f874fa6cd8bd5cf2f4aac63f9a01a4ff24ca499c9a6a4`;
- `BuildType.java`:
  `3064c09abcb9f38829c03c16ed1fb2799a40ebbca2ea3904a68808e158d325f8`;
- `CompileOneDependencyTransformer.java`:
  `9646f571c5da74fb9c8d1f117f2956c31501f7122d7877a187ad2fc5b54692b9`;
- `StarlarkRuleClassFunctionsTest.java`:
  `e09c93616e096d639ec69b6b0c6a397a8a36bc8a95fa21b986cb5fc7f8f010aa`;
  and
- `CompileOneDependencyTransformerTest.java`:
  `af2575630db3a4cbed34fa98fc26048468c9cf9dc6f9caa528fcfb5fe2437378`.

`StarlarkAttrModuleApi` declares `flags` named-only with default `[]` on
exactly the five constructors above. `StarlarkAttrModule.createAttributeFactory`
casts the input to a sequence of strings and visits each spelling.
`Attribute.Builder.setPropertyFlag(String)` resolves exact, case-sensitive
`PropertyFlag` enum names and stores them in an `EnumSet`; duplicates collapse,
order is not retained, and unknown names fail as `unknown attribute flag`.
Property flags participate in immutable attribute equality and hashing.

The complete pinned enum contains 25 names: `MANDATORY`, `EXECUTABLE`,
`UNDOCUMENTED`, `TAGGABLE`, `ORDER_INDEPENDENT`, `STRICT_LABEL_CHECKING`,
`DIRECT_COMPILE_TIME_INPUT`, `NON_EMPTY`, `SINGLE_ARTIFACT`,
`SILENT_RULECLASS_FILTER`, `SKIP_ANALYSIS_TIME_FILETYPE_CHECK`,
`CHECK_ALLOWED_VALUES`, `NONCONFIGURABLE`,
`CONFIGURABLE_ATTR_WAS_USER_SET`, `SKIP_PREREQ_VALIDATOR_CHECKS`,
`CHECK_CONSTRAINTS_OVERRIDE`, `SKIP_CONSTRAINTS_OVERRIDE`, `OUTPUT_LICENSES`,
`HAS_STARLARK_DEFINED_TRANSITION`, `HAS_ANALYSIS_TEST_TRANSITION`,
`IS_TOOL_DEPENDENCY`, `STARLARK_DEFINED`, `SKIP_VALIDATIONS`,
`FOR_DEPENDENCY_RESOLUTION`, and
`FOR_DEPENDENCY_RESOLUTION_EXPLICITLY_SET`. Bazel accepts every exact name
through the deprecated public parameter, even where a flag is ineffective for
the constructor's type. The packet keeps this registry centralized for
diagnostic classification but activates no unowned effect.

`DIRECT_COMPILE_TIME_INPUT` is consumed only by Bazel's
`compile_one_dependency` selection: that command reads flagged `LABEL_LIST`
attributes, accumulates labels in sorted order, and ignores scalar/dictionary
forms. Slug has no admitted `--compile_one_dependency` command or internal
consumer. Retaining the bit is therefore exact for loading identity; its
command effect remains unsupported rather than being approximated in normal
configured dependency collection.

Pinned `StarlarkRuleClassFunctionsTest#unknownRuleAttributeFlags_forbidden`
proves unknown-name rejection, and
`CompileOneDependencyTransformerTest` uses a Starlark
`attr.label_list(flags=["DIRECT_COMPILE_TIME_INPUT"])`. The selected
rules_rust source used by the real replay has SHA-256
`a645bd5db6344bd3c0997dcf73600475c0af53fb4dd025890be24b8e1e2dbfd8`;
at `rust/private/rust.bzl:865` it makes the same authentic declaration. No new
oracle fixture is needed because pinned source plus the real replay
discriminates this gap.

## Compatibility classification

**Exact:** the five-constructor named-only surface for omitted/empty and
`DIRECT_COMPILE_TIME_INPUT`; list and tuple sequence acceptance; exact
case-sensitive spelling; duplicate collapse and order-insensitive retained
identity; non-sequence/non-string/unknown rejection; no partial publication;
ordinary rule schema retention; and unchanged build/query/analysis
behavior while `compile_one_dependency` is absent.

**Slug-native:** Rust/starlark-rust diagnostic wording; the internal bit
assignment; and a compact V2-owned `u32` newtype rather than Java `EnumSet`.

**Unsupported/deferred:** the other 24 recognized property names and all of
their effects; `compile_one_dependency`; property-flag documentation or schema
introspection; accepting meaningful flags on Slug's currently fixed-schema
aspects, macros, subrules, repository rules, or module-extension tag schemas
where Slug would otherwise discard them; and every later bootstrap failure.
Those consumers fail closed if a non-empty retained flag set reaches a
conversion that cannot preserve it.

## Natural owner, identity, and lifetime

`attr_methods` in `package.rs` remains the sole generated binder. One shared
`unpack_attribute_flags` helper validates a list or tuple, resolves the
centralized Bazel-name registry, rejects recognized-but-unowned names, and
returns `AttributeFlags`. The parser input is evaluator scratch and is released
after construction.

`AttributeFlags` lives beside `AttributeSchema` in `attrs.rs` as a private
`u32` newtype deriving `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Default`,
and `Allocative`. It exposes only a derived read-only schema accessor, initially
`direct_compile_time_input()`. Do not retain `String`, `CompactString`,
`Vec`, `Arc` slice, map, set, enum allocation, or a side table. A 32-bit word
covers all 25 pinned flags while preserving compact clone and memory accounting.

The bit flows through `AttributeDefinitionGen`, `RuleAttributeSchemaGen`,
freeze, ordinary rule lowering, and final immutable `AttributeSchema`.
Existing package/rule/configured-target structural equality and DICE
invalidation already own those values; add no DICE key, hash, cache, interner,
registry, task, lock, or publication path. Built-in attributes use the empty
set. Query value/provenance and ordinary dependency topology remain unchanged.

Fixed-schema aspect, macro, subrule, repository-rule, and tag-class conversions
must not silently drop non-empty flags. Reject at each conversion boundary and
leave that consumer explicitly deferred. This packet does not widen those
surfaces merely because they share an `attr.*` descriptor.

Overlapping requests, cancellation, eviction, and shutdown retain their
existing evaluator/package/DICE lifecycle. Source edits already invalidate the
owning loaded module and package through source provenance. Normalized flag
equality makes the retained schema identical for omission/empty and for
duplicate/reordered spellings, while the source digest still invalidates any
`.bzl` byte edit. The admitted bit alone distinguishes the semantic schema.

## Full family architecture and successor order

Property names move by semantic effect, not one caller at a time:

1. this packet installs the compact owner and admits
   `DIRECT_COMPILE_TIME_INPUT` across all five constructors;
2. collection policy owns `ORDER_INDEPENDENT` and `NON_EMPTY` together with
   public `int_list` and `allow_empty`, because all affect value normalization
   or empty-value validation;
3. existing dedicated parameters adjudicate precedence with matching flags
   such as `MANDATORY`, `EXECUTABLE`, `NONCONFIGURABLE`, `SINGLE_ARTIFACT`, and
   `CHECK_ALLOWED_VALUES` before those bits can be admitted;
4. label dependency policy owns strict/file/rule filters, providers, tool
   classification, prerequisite/constraint checks, validation propagation,
   `cfg`, aspects, and dependency-resolution bits with their analysis
   consumers; and
5. documentation/license/internal transition bookkeeping remains absent or
   deferred unless an admitted Bazel surface observes it.

The real bootstrap replay chooses which already-planned category is scheduled
next. No caller-specific branch, dynamic signature table, manual keyword
dispatcher, AST scan, source rewrite, or second retained schema is allowed.

## Buck2 and Zabel guidance

Buck2-derived starlark-rust remains the evaluator and generated binder. The
matching Stage 9 attribute-coercion row was reviewed. Existing V2 scalar schema
storage plus `allocative::Allocative` is sufficient; importing a map, set,
interner, thin slice, or V1/Buck2 attribute representation would cost more than
the single word it replaces. Record this as intentional utility reuse through
existing scalar/Allocative patterns, not as a missed import.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance,
not truth. Its shared captured-declaration policy supports placing normalized
flags beside other retained attribute policy, while its compact representations
reinforce avoiding raw strings. Copy no Zig code, enum numbering, parser,
allocator, cache, diagnostic, or semantic claim. Bazel 9.2 alone decides names
and effects.

## Allowlist, caps, validation, and stops

Production and inline proof allowlist:

- `app/slug_loading_v2/src/attrs.rs`;
- `app/slug_loading_v2/src/package.rs`;
- proof only: `app/slug_loading_v2/tests/build_file_loading.rs`.

Scheduling/status edits may replace this manifest and update canonical Live
Status, Stage 6, and the matching Stage 9 ledger row. Do not touch analysis,
query, commands, starlark-rust, Cargo, fixtures, rulesets, `cc_common`,
`cc_internal`, rules_rust, rules_cc, or the parked registration proof. The
production cap is 140 net / 190 gross Rust lines, proof cap 210 net / 260 gross,
and total cap 450 gross Rust lines. The focused terminal-review correction
adds only the final package-lowering seam proof and does not widen production.

`package.rs` exceeds the physical-size trigger but remains cohesive because it
is already the sole binder, descriptor, rule-schema freeze, and consumer-
validation owner. `attrs.rs` is the sole immutable loading schema. Splitting a
one-word policy into a new module would add another ownership boundary. No
touched function may exceed 150 lines; a shared helper replaces repeated
validation.

Validate serially with focused tests proving all five constructors; omission,
empty list/tuple, duplicate and reordered normalization; non-sequence,
non-string, unknown, recognized-but-unsupported, positional, and forbidden-
constructor rejection; retained schema A/B/A identity plus exact-source
restoration without claiming a cross-source DICE cutoff; empty built-ins; and
fail-closed aspect/macro/subrule/repository/tag consumers. Then run the complete `slug_loading_v2`
library suite, complete `slug_analysis_v2` direct dependent, rebuild
`slug_cli_v2`, perform one daemon-clean real replay, run `cargo fmt --all --
--check`, Cargo metadata, `scripts/v2_archive_status.sh`, `git diff --check`,
cap accounting, and parked-file SHA-256 verification.

`REPLAN` before accepting another property name, adding a consumer or command,
changing configured dependency/action/query semantics, adding a dynamic
collection or retained raw spelling, adding a DICE key/cache/interner, touching
a ruleset/C++/starlark-rust owner, silently discarding flags in another schema,
changing an existing semantic projection, or exceeding a cap. Independent
design review is required before Rust because this adds retained semantic
state; terminal implementation review is required before commit.

## Implementation candidate evidence

The candidate adds one four-byte `AttributeFlags` value to the existing
descriptor, frozen rule schema, and immutable loading schema. One generated-
binder type plus one shared resolver serves all five constructors. The resolver
names all 25 pinned Bazel properties, admits only
`DIRECT_COMPILE_TIME_INPUT`, collapses duplicates into one bit, distinguishes
recognized-but-unsupported names from unknown names, and retains no input
string or collection. Fixed-schema aspect, explicit and inherited macro,
subrule, repository-rule, and tag-class controls succeed without flags and
reject the otherwise-identical flagged declaration.

The focused constructor/consumer matrix, four-byte schema test, and real
package-lowering schema assertion pass. The
complete `slug_loading_v2` library is 465 passed, 0 failed, and 1 expected
ignored authenticated-source test. Complete `slug_analysis_v2` unit,
integration, and doc tests pass (114 tests plus empty doc tests). The rebuilt
`slug_cli_v2`, formatting, Cargo metadata, and diff checks pass. The archive
checker reports only its three known retained thoughts paths. Production
accounting is 114 net / 136 gross Rust lines; proof is 183 net / 183 gross;
total Rust is 319 gross. The parked registration proof remains unchanged at
SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`.

The daemon-clean authentic replay accepts the previously unsupported `flags`
keyword and reaches the next field-policy boundary in the same rules_rust
declaration. At `rust/private/rust.bzl:873`,
`allow_files = [".rs"]` reaches Slug's current Boolean-only validator and fails
`allow_files must be a bool or None`; runtime mode is one-shot and no daemon
remains. The extension-sequence form belongs to the separately planned
label-file-admissibility category. No rule implementation, configured
analysis, `compile_one_dependency`, `cc_common`, or C++ owner is reached.

Independent terminal review returned `ACCEPT`: the final proof loads and
instantiates a real rule, reads flagged and unflagged attributes from the
published `StarlarkRuleImplementation` schema, and fails if the production
`.with_flags(declaration.flags)` lowering is removed. The allowlist and
114/183/319 cap accounting are consistent.

## Immediate predecessor

Commit `1e583e9d0` terminally accepts
`WP-6-7A-attribute-doc-parameter-category-parity-r1`. Five missing named-only
parameters use the existing validator, and one table proves the complete
thirteen-constructor documentation category without retained state. The
daemon-clean replay passes rules_rust's earlier `doc` declaration and exposes
this packet's `DIRECT_COMPILE_TIME_INPUT` boundary.
