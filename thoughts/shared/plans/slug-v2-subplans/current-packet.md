# Current Slug V2 Packet

Packet: `WP-6-7A-module-extension-tag-attribute-schema-category-implementation-r3`

Milestone: M7A generic Starlark/ruleset closure; module-extension tag schema
conversion and invocation values.

Status: R1 architecture review returned `REPLAN`; independent R2 architecture
review returned `ACCEPT`; implementation validation selected a proof-only R3
allowlist correction for independent review.

The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Objective and compatibility boundary

Implement the complete ordinary Bazel 9.2 module-extension tag attribute-kind
category, rather than the replay's literal `auth: StringDict` row. The admitted
kind matrix is `bool`, `int`, `int_list`, `string`, `string_list`,
`string_dict`, `string_list_dict`, `label`, `label_list`,
`string_keyed_label_dict`, `label_keyed_string_dict`, `label_list_dict`,
`output`, and `output_list`. This packet also adds the missing general
`attr.int_list` descriptor and carries that kind through the already-shared
rule, macro and repository-rule attribute model so the public constructor does
not create a tag-only special case.

Admit as **exact** for Bazel 9.2's default-enabled ordinary descriptor surface
when every supplied/default label is visible:

- supplied non-`None` values are converted in MODULE call order; explicit
  `None` is skipped; then mandatory/default/visibility checks run in schema
  order and every runtime tag exposes one value per schema entry;
- signed i32 scalar and integer-list members, list or tuple input for list
  kinds, ordered Starlark dictionary conversion, all scalar/list/dictionary
  type failures, intrinsic defaults, declared defaults, `mandatory`, scalar
  `values`, unknown attributes, and failure order outside the deferred
  non-visible-label case;
- empty tag collections even when their descriptor carries
  `allow_empty = False`, because Bazel's tag conversion does not consult that
  rule-attribute policy;
- apparent label conversion in the consuming module's repository mapping,
  definition-owned visible canonical label defaults, recursive visibility
  acceptance, same-package output conversion, and duplicate canonical
  `label_keyed_string_dict` key rejection;
- public Starlark tag field spelling, including leading-underscore private
  names, schema-order field lookup and `dir`, Starlark insertion order within
  collections, immutable invocation-local list/dictionary values, Label ABI,
  and no retained evaluator borrow; and
- the same `IntegerList` kind/value in rules, symbolic macros, repository-rule
  declaration/instantiation/context projection, query candidates and explicit
  unsupported output-template diagnostics.

Keep as **Slug-native** Rust valid-Unicode strings and diagnostics, compact
retained Rust values, starlark-rust exception decoration, and DICE scheduling,
cancellation, memory accounting and eviction. Exact Java exception text,
HotSpot identity and Java UTF-16-only invalid strings are not claimed.

Keep **unsupported/deferred** experimental dormant-label descriptors,
the disabled legacy `attr.license`, computed/late-bound defaults, selectors in
MODULE values, dormant dependencies, descriptor policies not consumed by
`AttributeUtils.typeCheckAttrValues`, exact diagnostic/failure precedence for
non-visible supplied or definition-default labels, and exact documentation-only
descriptor metadata. Non-visible labels still fail closed and are never
accepted. This packet changes no MODULE parser, BCR rule body, rules_rust,
toolchain, C++, `cc_common`, `cc_internal`, provider, configured-analysis or
action semantics. Those names remain downstream discriminators only.

## Bazel 9.2 authority and oracle evidence

Bazel tag `9.2.0` commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole semantic authority.
Pinned sources are:

- `StarlarkAttrModuleApi.java`
  `af70c851882fa049034184dbb6f6580731cfa738d79dfb8abcf61af176257670`;
- `StarlarkAttrModule.java`
  `388421c44c623c1c6625fd9f2b059d2a7d1e13b8d45e7c96173f24866a917967`;
- `StarlarkRepositoryModule.java`
  `c6adf0f521e56419ec22e7980def6b27778bab4d5c5294b3556c2286f5b6bcea`;
- `TagClass.java`
  `8b6359f485d473482162bceabf0375c69b770ab1d14ab712d7cfd98e2d620571`;
- `TypeCheckedTag.java`
  `402ed1072a6364191cee513e4669d2f676f27a19168e629186e512b0f1b0c642`;
- `AttributeUtils.java`
  `d1db963ecf54b3c921112d1bfd15876c525921187ba4b6fc6cee16426b5f2c3f`;
- `Attribute.java`
  `fbe208c37ad4ed88030f874fa6cd8bd5cf2f4aac63f9a01a4ff24ca499c9a6a4`;
  and
- `ModuleExtensionResolutionTest.java`
  `d8602fd385d34ab5387cb0ef3891ef9acc0ca62cd8f67324e09fd33ea7a3e769`.

A disposable pinned-Bazel oracle exercised one tag containing all fourteen
kinds. Bazel exposed
`True`, `7`, `[1, -2, 3]`, scalar/list/nested-dictionary `Label` values,
same-package output labels, strings and ordered dictionaries. A second omitted
tag exposed intrinsic defaults
`False`, `0`, `[]`, `None`, empty label collections, `None`, `[]`, `""`, and
empty string collections/dictionaries in the same schema order. The fixture,
output base and Bazel server were removed after capture; no Java helper,
checked-in oracle or probe artifact enters Slug.

## Frozen architecture

Extend the existing shared `AttributeKind` with `IntegerList` and
`CoercedAttributeValue` with `IntegerList(Arc<[i32]>)`. Reuse that value through
ordinary rule/macro/repository projections. Do not add a parallel
module-extension-only type graph. The existing heap-independent
`NonrootAttributeValue` already owns raw list/tuple/dictionary and arbitrary
integer syntax; conversion validates i32 only at the typed schema boundary.

Retain existing allowed scalar values in the shared definition/schema
projection. `allow_empty` remains available to the shared rule-attribute
consumers that actually enforce it, but module-extension tag conversion must
ignore it. `prepare_module_extension_tag_attributes` owns the Bazel
`AttributeUtils` two-phase order: initialize schema slots, convert every
supplied non-`None` value in source order, then fill/check slots in schema
order. It recursively converts visible labels and checks visibility; an
invisible label fails closed without claiming Bazel's precise diagnostic
precedence. Collection conversion returns existing immutable `Arc` slices;
dictionary equality stays structural while source order remains available for
Starlark iteration.

Store each tag schema entry under its public Starlark name. In particular,
`_private` remains `_private` for supplied lookup, defaults, runtime
`get_attr`, and `dir`; Slug does not expose Bazel's internal `$private`
spelling because it has no separate native/public tag-field representation.

At extension invocation, materialize the already-prepared values into the
invocation module's existing starlark-rust `FrozenHeap`. Lists and dictionaries
are therefore immutable without a custom collection implementation or a
DICE-retained evaluator value. The invocation module owns and releases those
frozen values with the existing evaluator lifetime. Add no evaluator borrow to
a DICE value, new key, graph, cache, interner, global table, lock, task or
semantic side store.

Buck2/starlark-rust supplies the already-retained `Arc`, compact collection,
`Allocative`, `FrozenHeap`, `AllocList` and `AllocDict` utilities. Zabel commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance only:
`module_extension_tag_value.zig`
`4e2dfc4148b7ef12d02e48d5d59d0067832d27cac61957a313cc2270822e2741`,
`module_extension_declaration_host.zig`
`7474d1ddb37d2ffaa0006b4ce3b19df3917bb6dce055c4db87363fcf50067600`,
and `module_extension_execution_capture.zig`
`8f03505b2302f79443d3ab95f12cbca2b65eec8a417ff94e739fb9fafcd06fc0`
motivate one typed value family, schema-ordered slots and invocation-local
freezing. Copy no Zig representation, allocator, evaluator, diagnostic,
scheduler, cache, policy or behavior.

## Closed ownership, caps and stop conditions

Production allowlist:

- `app/slug_loading_v2/src/attrs.rs`;
- `app/slug_loading_v2/src/package.rs`;
- `app/slug_loading_v2/src/module_extension.rs`;
- `app/slug_loading_v2/src/module_extension_repository_instantiation.rs`;
- `app/slug_loading_v2/src/repository_rule_context.rs`; and
- `app/slug_loading_v2/src/rule_outputs.rs`.

Focused proof may use tests colocated in those files and the selected-graph
module-extension corridor in
`app/slug_loading_v2/src/host_package_load_tests.rs`. R3 additionally permits
only the existing
`module_extension_definition_loading_tests::real_prepared_inputs_preserve_raw_first_and_contextual_errors`
proof in `app/slug_loading_v2/src/bzl_module.rs` to replace its stale
string-list-schema rejection with successful preparation. No production line
or other test in that file is authorized. Do not touch the parked proof,
fixtures, generated files, another crate, parser code, C++ modules or a
consumer-specific file. Gross caps are 900 production Rust lines, 1,100 proof
Rust lines and 2,000 total; deletions and moves count. Stop with `REPLAN` if
another file, DICE key, retained evaluator value, custom Starlark collection,
second attribute value graph, cache, interner or broader parser change is
required.

## Required proof and validation

Add one table-driven fourteen-kind conversion/default/runtime projection proof
covering list and tuple inputs, ordered nested dictionaries, local and mapped
labels, outputs and immutable collection mutation failures. Add discriminating
rows for every wrong shape, i32 scalar/member overflow, unknown attributes,
explicit `None`, mandatory/default order, scalar `values`, acceptance of empty
tag collections with `allow_empty = False`, fail-closed invisible
nested/default labels without an exact-order claim, same-package outputs,
canonicalized duplicate label keys, and supplied/default/runtime/`dir`
behavior for a leading-underscore field. Prove the shared `IntegerList` value
through ordinary rule, macro, repository-rule and query/output-template
boundaries.

Run formatting and diff checks, focused package/module-extension/repository
tests, the full `slug_loading_v2` suite, direct analysis/query/core/server
checks, pinned source hashes, clean Bazel/Buck2/Zabel trees, parked-proof hash
and `scripts/v2_archive_status.sh`. Rebuild `slug_cli_v2`, clean `slugd` before
and after, then replay the authentic rules_rust fixture. It must clear
`auth: StringDict` without a rules_rust/toolchain/C++ special case; the next
genuine generic failure selects the following packet. Independent architecture
review is required before Rust and independent terminal review before
acceptance.

## R1 review and R2 correction

Independent R1 architecture review returned `REPLAN` before Rust. First,
Slug's shared `CanonicalLabel` value cannot retain a non-visible label long
enough to reproduce Bazel's later schema-ordered visibility failure, so R1's
exact nested/default visibility-order claim required a forbidden second value
graph. R2 keeps complete exact visible-label conversion and explicitly defers
only the precise non-visible diagnostic precedence while preserving fail-closed
behavior. Second, R1 incorrectly treated `allow_empty` as tag conversion
policy; pinned `AttributeUtils.typeCheckAttrValues` never consults it, so R2
requires empty tag collections to succeed. Third, Bazel indexes tag fields by
`Attribute.getPublicName()`; R2 therefore preserves `_private` rather than the
internal `$private` spelling. The fourteen-kind inventory, shared
`IntegerList`, invocation `FrozenHeap`, allowlist and caps are otherwise
unchanged.

Independent R2 architecture review returns `ACCEPT`. It confirms that exact
valid/visible conversion, fail-closed invisible labels with deferred diagnostic
precedence, tag-specific `allow_empty` behavior, public private-field names,
the shared compact `IntegerList`, invocation-local `FrozenHeap`, allowlist,
caps and proof matrix are coherent and implementation-ready. The only residual
risk is the explicitly deferred precise ordering/diagnostics for non-visible
labels.

## R3 proof-only allowlist correction

The first full `slug_loading_v2` run passed the new implementation proofs but
correctly invalidated two historical rejection assertions. The selected
`host_package_load_tests.rs` corridor already owns the scalar allowed-values
expectation. The second assertion is colocated in `bzl_module.rs` and expected
an unused `attr.string_list()` tag schema to fail during prepared-input
validation. That behavior is exactly what this packet removes, so leaving the
assertion unchanged cannot produce a green full suite without contradicting
the admitted category.

R3 changes only the proof allowlist to permit that one assertion to require a
successful prepared input. It changes no production file, compatibility
claim, value representation, lifetime, cap, test fixture or downstream
consumer. Independent review must accept this bounded correction before
terminal validation resumes.

## Immediate predecessor

Commit `cfe83834d` terminally accepts the complete recursive BUILD glob
category and advances authentic replay from external `@@platforms//host` glob
loading to this generic module-extension schema frontier.
