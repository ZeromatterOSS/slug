# Current Slug V2 Packet

Packet: `WP-6-7A-cross-module-attribute-descriptor-identity-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 declaration and
configured-attribute breadth.

Status: independent design pre-review returned `ACCEPT`; implement only the
frozen allowlists, caps, proofs, and stops below.
Base commit `300b724e7` terminally accepts the complete eleven-member C++
configuration-field catalog. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Observable result and compatibility classification

Preserve Bazel `attr.*()` descriptor semantics when a descriptor crosses one
or more `.bzl` module boundaries before an already-admitted declaration
consumer receives it. Direct loads, re-exports, lists/dictionaries, dictionary
union, and lookup preserve the descriptor; none of those ordinary Starlark
container operations may erase its typed identity or declaration owner.

Exact admitted behavior:

- an imported descriptor retains its kind, mandatory/configurable state,
  file-admissibility, flags, allowed values, coerced default, late-bound or
  computed-default state, executable/exec transition, provider predicate,
  attached aspect, and user transition exactly as the same descriptor used in
  its defining module;
- declaration-owned labels and repository mappings remain those captured when
  `attr.*()` ran. Importing, re-exporting, dictionary union, or consuming the
  descriptor never reparses or rebases a default in the consumer module;
- provider identities remain their defining provider identities, attached
  aspects remain the original exported aspect values, and user transitions
  retain the original implementation and output identity;
- every already-admitted descriptor consumer accepts frozen descriptors
  through one conversion seam and then applies its existing consumer-specific
  restrictions. `rule()` receives the complete admitted descriptor shape;
  macro, fixed-aspect, repository-rule, tag-class, and subrule boundaries do
  not gain schema forms they otherwise reject; and
- invalid non-descriptor values retain the existing consumer-specific error.

The Rust representation and diagnostics remain Slug-native implementation
details. Exact Bazel configuration checksum/output-path bytes remain M9.

Unsupported/deferred behavior:

- provider-constrained target invocation remains the next independent generic
  loading/analysis category. Preserving the predicate in a frozen rule schema
  does not bypass or implement its configured target validation;
- dependency-aspect application and user-transition execution retain their
  existing typed admitted/deferred boundaries. This packet preserves their
  declaration identity only;
- descriptor kinds, arguments, macro inheritance, fixed-aspect shapes,
  repository-rule schemas, tag-class schemas, and subrule schemas that Slug
  already rejects remain rejected after import; and
- no parser grammar, `set`, rule body, `cc_common`, `cc_internal`, C++ rule,
  action, execution, or ruleset special case is added. Bazel 9 BCR Starlark
  remains the owner of every rule body.

## Bazel 9.2 authority and evidence

Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole semantic
authority. Pinned source SHA-256 values are:

- `StarlarkAttrModule.java`:
  `388421c44c623c1c6625fd9f2b059d2a7d1e13b8d45e7c96173f24866a917967`;
- `StarlarkRuleClassFunctions.java`:
  `a1f706cfbbc67aa3cd2521df2091dd5ed9af96eb4568049f8eee966d06c622f7`;
- `StarlarkRuleClassFunctionsTest.java`:
  `e09c93616e096d639ec69b6b0c6a397a8a36bc8a95fa21b986cb5fc7f8f010aa`;
- rules_rust 0.73.0 `rust/private/rust.bzl`:
  `a645bd5db6344bd3c0997dcf73600475c0af53fb4dd025890be24b8e1e2dbfd8`;
  and
- rules_rust 0.73.0 `rust/private/rust_allocator_libraries.bzl`:
  `ae4acb50ac6a1b922254a07346d97b4649810d33836f2be4824fd0b7a81e536e`.

`StarlarkAttrModule.Descriptor` owns one immutable `AttributeFactory`, has
value equality, and builds the named rule attribute without consulting the
consumer module. `StarlarkRuleClassFunctions.attrObjectToAttributesList`
casts every dictionary value directly to that descriptor type and then builds
it; there is no local-module test or reconstruction path. The pinned unit tests
separately exercise label-default conversion, provider predicates, attached
aspects, and user transitions.

A fresh Bazel 9.2 oracle defines a producer descriptor dictionary containing a
relative label default, provider predicate, dependency aspect, user
transition, and transition allowlist. A bridge module loads and re-exports it
through `{}` dictionary union; a consumer loads the bridge and unions it with a
local descriptor before `rule()`. This command succeeds:

`bazel --nosystem_rc --noworkspace_rc query 'deps(//:subject)' --output=label`

Its exact labels are `//:subject`, `//producer:owned`, and
`@bazel_tools//tools/allowlists/function_transition_allowlist:function_transition_allowlist`.
The producer default, not a consumer-relative target, proves declaration
ownership. Oracle source SHA-256 values are
`5c1cb53c9b9d3a37b5c8dc88f6802f8d2b06d90e89b59566d82d6fbc2931e979`
(`MODULE.bazel`),
`11043ddf75d77118d74ce598602a5dc8a3c8638c6dc3b26e44d7eb9b9f9a1b81`
(root `BUILD.bazel`),
`bdc292aec37d1327aa2cfea6d50150e7b732af0e7db7f7113ae791928414e959`
(`bridge.bzl`),
`0aa62aafe72d15c53283d1f77ae94121a10bfee5dce2ef94e2369bf6f69dec08`
(`consumer.bzl`),
`d176fcc7e02ca7195fd6c568443c0dd524cf9bd605a3fefff618492d04855f9d`
(producer `BUILD.bazel`),
`3a08ee94976ebefb75ed3f7c255fdb27c27ff23bc2fb430ac483ba43232f3f6d`
(`providers.bzl`), and
`f17232fdd47bf2eae6f405b63a56e92955514cffaf62fead5d94cb4815eda6ab`
(`attrs.bzl`). The fixture is ephemeral exact/message-shape evidence; focused
Rust regressions encode the semantic discriminators rather than importing the
fixture.

The authentic rules_rust consumer imports
`RUSTC_ALLOCATOR_LIBRARIES_ATTRS["allocator_libraries"]`, whose descriptor
carries `providers = [AllocatorLibrariesInfo]`, and merges it into the
`rust_common` rule attributes. Rebuilt Slug currently rejects that value with
`rule attribute 'allocator_libraries' must use attr.*()` before any rule body,
`cc_common`, `cc_internal`, or C++ semantic call.

## Learned Slug facts and architecture decision

`AttributeDefinitionGen<Value>` and
`AttributeDefinitionGen<FrozenValue>` already contain the complete descriptor
shape. starlark-rust's generated `from_value` returns either form. Slug's
current `rule_attribute_definition_from_value` clones live descriptors but
reconstructs a frozen descriptor only when providers, attached aspect, and
transition are all absent. That deliberate partial conversion is the sole
cause of the authentic failure; dictionary union preserves the frozen value.

Replace that partial helper with one complete, consumer-neutral owned
projection:

- live descriptors clone as today;
- frozen descriptors clone Rust-owned scalar/compact/`Arc` facts and project
  only embedded `FrozenValue` fields to `Value` with `to_value()`;
- frozen transitions project their original implementation pointer and clone
  their compact output; and
- all declaration consumers in `package.rs` use this seam before applying
  their existing restrictions. `subrule_attribute_from_value` uses the same
  projection and keeps all of its current checks.

Do not add an enum wrapping live/frozen schemas, dynamic registry, raw string
pair, descriptor interner, source replay, repr parser, side table, cache, or
second retained schema. The transient projection exists only while evaluating
the consuming module; freezing the resulting rule/aspect stores the existing
`FrozenRuleAttributeSchema` and references the original frozen values.

## Lifetime, memory, incremental ownership, and peer guidance

The defining module's frozen heap owns embedded aspect/transition values. In
Slug's vendored starlark-rust, `Module::load_symbol` calls
`OwnedFrozenValue::owned_value` with the importing frozen heap, which adds the
owner heap as a reference before producing a lifetime-bound `Value`; module
freeze carries that reference. Pinned local SHA-256 values are
`2aa3a01e226649e78a0fa874b20dc97d9f2aa8aa2ac31084de34fdb0b0c64e45`
for `environment/modules.rs` and
`704c26dee697ca2c453b0a10314d5b071d8274b4303c75b0f04ab80ba2e110c5`
for `values/layout/heap/heap_type.rs`. Slug additionally retains each frozen
direct/transitive loaded module in `FrozenBzlModule` as lifetime-only state
excluded from semantic equality. The consuming module therefore never holds
an unowned evaluator pointer.

The projection is evaluation scratch. Existing `CompactString`, immutable
`Arc<[T]>`, `FrozenValue`, `Allocative`, and final frozen-schema storage remain
unchanged. Clones of defaults and provider predicates retain their current
shared inner allocations; `FrozenValue::to_value()` is a pointer projection.
No DICE key, equality, invalidation, lock, request overlay, cache, global
interner, async task, or shutdown path changes. Existing source digest/load
manifest ownership publishes and invalidates the frozen result.

Buck2/starlark-rust is leaf/runtime guidance: its current
`environment/modules.rs` (SHA-256
`a6793c4c4891edb94eb511719d0b9ea3b18d52f59e67ccd87837739e18f7c13f`)
retains the same referenced-heap contract, while Buck2's Starlark attribute
wrapper carries one owned semantic attribute instead of reparsing source.
Pinned `starlark_attribute.rs` SHA-256 is
`9dfc1197309fda8cb653cc0961ab5393792c37cdbd400700db0e2ca78dc4df0b`.
No Buck2 code is copied.

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is concept/test
guidance only. Its declaration capture keeps descriptor values and their
publication owner together; its imported-attribute test proves producer-owned
label defaults through dictionary union. Relevant file SHA-256 values are
`f2221daad6d0ad61177d860e58faf3ade1bb249cce9789d7150f22bc18804fcd`
(`build_rule_declaration.zig`) and
`4e3bff2cc636a52c26e64346ff4271490d1a7a0cf59917bc46d8578bc7f404d1`
(`build_invocation_capture.zig`). Zabel is neither semantic authority nor a
copy source.

## Implementation boundary, complexity, caps, and proofs

Production allowlist:

- `app/slug_loading_v2/src/package.rs`.

Proof allowlist:

- `app/slug_loading_v2/src/host_package_load_tests.rs`.

Plan/status allowlist is this manifest, the canonical plan, and the Stage 9
ledger. Proposed cap is 100 net / 160 gross production Rust lines, 300 net /
450 gross proof Rust lines, and 610 total gross. No new file or semantic helper
over 100 lines is expected.

`package.rs` is 9,074 lines and mixes multiple loading declarations, so the
complexity trigger applies. The descriptor types, every current consumer, and
their freeze implementations are already colocated there; one bounded private
projection is more cohesive than exposing private evaluator types through a
new module. This packet must not add unrelated declaration policy. The 36,277-
line test module already owns exact recursive `.bzl` freeze/import fixtures;
new proof should replace its explicit rich-import rejection and reuse existing
helpers rather than add a second harness.

Focused proofs must cover:

1. local and frozen descriptors project the same complete field set;
2. direct and two-hop re-export plus dictionary union preserve provider
   identity, attached-aspect pointer, transition implementation/output, and
   declaration-owned defaults without deep-copying frozen values;
3. already-admitted simple descriptors remain importable by rule, macro,
   fixed-aspect, repository-rule, tag-class, and subrule consumers, while each
   consumer's unsupported schemas remain rejected;
4. non-descriptor errors and local rich descriptors are unchanged;
5. source A/B/A through the existing loaded-module route changes and restores
   the retained schema without a fresh-graph bypass;
6. the authentic rules_rust 0.73.0 cquery clears the imported descriptor and
   records the next generic frontier before any rule-body or C++ special case;
   and
7. retained modules and schema values remain `Allocative`, lifetime-owned, and
   free of new `String`/`Vec`, hash map/set, cache, interner, or repeated deep
   clone storage.

Run focused host-package tests, then complete `slug_loading_v2` and
`slug_analysis_v2` suites serially. Rebuild `slug_cli_v2` before authentic
replay; clean `slugd` before and after. Run formatting, metadata, archive,
diff, cap, pinned-source, clean-Zabel, and parked-SHA gates. Obtain independent
terminal implementation review before commit.

`REPLAN` before adding `unsafe`, a source/repr fallback, a second descriptor or
schema owner, a dynamic registry, DICE state, a caller/source side table,
deep-copying evaluator graphs, weakening consumer-specific validation,
implementing provider invocation/aspect/transition semantics, touching
starlark-rust or a ruleset, changing loaded-module equality, or exceeding a
cap.

Independent design pre-review returned `ACCEPT`. It verified the Bazel and
two-hop oracle contract, complete field projection, vendored starlark-rust and
Slug transitive heap ownership, unchanged consumer restrictions, scratch and
retained memory classification, A/B/A proof, allowlists, caps, peer-guidance
classification, and replay stop.

## Immediate predecessor

Commit `300b724e7` terminally accepts
`WP-6-7A-cpp-configuration-field-catalog-completion-r1`. It closes Bazel 9.2's
eleven-member C++ field catalog through existing typed owners and advances the
authentic replay to this imported descriptor boundary.
