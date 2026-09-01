# Current Slug V2 Packet

Packet: `WP-6-7A-attribute-doc-parameter-category-parity-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 public attribute-
constructor signature breadth.

Status: implementation terminally `ACCEPTED`; commit pending. The initial
review required a positional-string rejection row for the claimed named-only
surface; the focused correction added that row across all thirteen constructors
and rereview returned `ACCEPT`. Base commit
`18b2549bd` terminally accepts Bzlmod declaration selection and repository-rule
producer identity. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked;
do not edit or stage it.

## Observable result and complete category boundary

Close Bazel 9.2's documentation-parameter category for every attribute value
representation Slug already owns. All thirteen existing constructors—`bool`,
`int`, `string`, `label`, `string_list`, `label_list`,
`string_keyed_label_dict`, `label_keyed_string_dict`, `label_list_dict`,
`output`, `output_list`, `string_dict`, and `string_list_dict`—accept `doc` as a
named-only string or `None`, reject any other value at declaration evaluation,
and produce the same retained attribute definition whether `doc` is omitted,
`None`, or any valid string.

Eight constructors already meet that contract. Add the shared existing
`discard_attribute_doc` call and static generated-binder parameter only to
`string_keyed_label_dict`, `label_keyed_string_dict`, `label_list_dict`,
`output`, and `output_list`. Prove the whole thirteen-constructor matrix,
including positional-string rejection, so the next caller cannot expose another
member of the same category. Rebuild the V2
CLI and replay `cquery //app/slug_cli_v2:slug`; it must pass authentic
rules_rust's `attr.label_keyed_string_dict(doc=...)` call and stop at the next
unsupported boundary or succeed. Do not consume that next boundary here.

This is a public builtin call-signature and declaration-validation packet, not
parser grammar or a rule implementation. It adds no Starlark value kind,
attribute value representation, schema field, configurable-analysis behavior,
provider/aspect/file policy, action, DICE key, repository effect, `cc_common`,
`cc_internal`, rules_rust, rules_cc, or C++ branch. Bazel 9 BCR Starlark remains
the rule-body owner.

## Full constructor-family architecture

The Bazel 9.2 public `attr` API is deliberately partitioned by owned semantic
effect rather than patched caller by caller:

1. this packet closes validation-only `doc` for all thirteen representations
   Slug already retains;
2. the collection-core successor owns `int_list` plus `allow_empty` across all
   list/dictionary/output-list constructors because both require retained value
   or declaration policy and configured-value validation;
3. label-dependency successors group `allow_files`/`allow_single_file`,
   `providers`, `cfg`, `aspects`, and the still-deferred dependency controls by
   their existing loading/analysis owners, never by a ruleset caller; and
4. undocumented experimental `dormant_label*` stays deferred, while
   `attr.license` remains absent exactly under Bazel 9.2's default
   `--incompatible_no_attr_license` behavior. Adding it would incorrectly flip
   BCR `hasattr(attr, "license")` fallbacks.

Keep starlark-rust's generated static function signatures as the sole binder.
Use shared validation/build helpers for common parameter semantics; do not add
a dynamic signature table, manual keyword dispatcher, AST scan, source rewrite,
or second retained attribute schema. A later parameter moves only with its
natural semantic owner and complete constructor family. This gives the full
surface one architecture without forcing unrelated retained semantics into a
validation-only packet.

## Learned facts and authenticated evidence

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole
semantic authority. Pinned sources are:

- `StarlarkAttrModuleApi.java`, SHA-256
  `af70c851882fa049034184dbb6f6580731cfa738d79dfb8abcf61af176257670`;
- `StarlarkAttrModule.java`, SHA-256
  `388421c44c623c1c6625fd9f2b059d2a7d1e13b8d45e7c96173f24866a917967`;
  and
- `StarlarkRuleClassFunctionsTest.java`, SHA-256
  `e09c93616e096d639ec69b6b0c6a397a8a36bc8a95fa21b986cb5fc7f8f010aa`.

`StarlarkAttrModuleApi` declares `doc` named-only, defaulting to `None`, with
only string and `NoneType` accepted on every constructor in this packet.
`StarlarkRuleClassFunctionsTest#testAttrDoc` proves omitted and string forms
across the ordinary family, while `#testAttrDocValueBadType` proves declaration-
time type rejection. The API additionally proves the same contract for
`string_keyed_label_dict`, which that table omits. Bazel retains normalized
documentation internally; no admitted Slug query, analysis, action, execution,
or documentation-export surface reads it.

The selected rules_rust source used by the real replay has SHA-256
`a645bd5db6344bd3c0997dcf73600475c0af53fb4dd025890be24b8e1e2dbfd8`.
At `rust/private/rust.bzl:709`, `_COMMON_ATTRS.aliases` calls
`attr.label_keyed_string_dict(doc=dedent(...))`. The two accepted predecessor
replays stop at the generated binder before any rule implementation or
`cc_common` call.

Existing Slug proof already establishes that valid documentation on `int`,
`string_list`, `string_dict`, and `string_list_dict` is type-checked and does
not change retained schema identity. Extend that exact invariant across the
complete existing family; no new fixture or external oracle is needed because
the pinned source regression and authentic replay discriminate the gap.

## Compatibility classification

**Exact:** named-only binding of `doc`; omission, `None`, and string acceptance;
non-string rejection during declaration evaluation; unchanged build/query/
analysis semantics for valid documentation; and absence of `attr.license` under
default Bazel 9.2 semantics.

**Slug-native:** Rust/starlark-rust diagnostic wording, Rust Unicode, and
discarding valid documentation after validation because documentation export
is not an admitted surface.

**Unsupported/deferred:** documentation extraction/formatting; `int_list`;
`allow_empty`; undocumented `dormant_label*`; materializers; dormant dependency
resolution; unowned `flags`, `skip_validations`, and legacy `allow_rules`
semantics; richer provider/aspect/file/transition behavior beyond already
accepted slices; and every later bootstrap failure. No existing exact slice is
widened by accepting a keyword whose semantic effect Slug does not own.

## Natural owner, request behavior, and lifetime

`attr_methods` in `package.rs` is the sole builtin binder and declaration
producer. Its shared `discard_attribute_doc` helper owns the accepted type
check. Valid documentation is evaluator-call scratch and is released after the
constructor returns. `AttributeDefinition`, `AttributeSchema`, loaded packages,
configured targets, actions, and DICE keys remain byte-for-byte structurally
unaffected by documentation spelling.

There is no new retained memory, cache, registry, interner, task, lock,
publication path, invalidation edge, fallback, or historical Host observation.
Existing source/module/package keys already invalidate when source bytes change;
their equality cutoff correctly ignores documentation after re-evaluation
because no admitted semantic consumer observes it. Overlapping requests,
cancellation, eviction, and shutdown retain their existing evaluator and DICE
lifecycles.

## Buck2 and Zabel guidance

Buck2-derived starlark-rust remains the parser, evaluator, and generated binder.
This packet uses its static `#[starlark_module]` signature mechanism unchanged;
there is no Buck2 utility or retained-representation change to audit.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance,
not truth. Its `build_invocation_capture.zig` groups `allow_empty`, provider,
file, transition, and validation policy on a shared captured declaration, and
its tests cover the full collection and label-dictionary families. Adopt only
the category-partitioning lesson for the successor sequence. Copy no Zig code,
evaluator/binder behavior, allocator, representation, cache, diagnostics, or
compatibility claim. Bazel 9.2 remains authoritative for every accepted call.

## Allowlist, caps, validation, and stops

Production and inline proof allowlist:

- `app/slug_loading_v2/src/package.rs`.

Scheduling/status edits may replace this manifest and update canonical Live
Status and Stage 6. Do not touch `attrs.rs`, analysis, starlark-rust, Cargo,
fixtures, rulesets, `cc_common`, or the parked registration proof. The production
cap is 20 net / 30 gross Rust lines, proof cap 90 net / 110 gross, and total cap
140 gross Rust lines.

`package.rs` exceeds the physical-size trigger but remains cohesive for this
five-signature change: it is already the sole generated-binder and shared
validation owner, and splitting five parameters would create a second builtin
registration boundary. No touched function exceeds 150 lines; no new central
builtin abstraction or dynamic dispatch table is admitted.

Validate serially with one table-driven thirteen-constructor documentation
test, including omitted/`None`/two distinct named strings, invalid non-string
values, and rejection of one positional string on every constructor; the
complete `slug_loading_v2` library suite; one direct
`slug_analysis_v2` compile/test dependent; rebuilt `slug_cli_v2`; a daemon-clean
real bootstrap replay; `cargo fmt --all -- --check`; Cargo metadata;
`scripts/v2_archive_status.sh`; `git diff --check`; cap accounting; and parked-
file SHA-256 verification.

`REPLAN` before retaining documentation, adding a value kind or schema field,
accepting another constructor parameter, changing parser/evaluator semantics,
adding a DICE key or cache, touching analysis/action/repository/ruleset/C++
owners, exposing `license`/experimental constructors, changing an existing
semantic projection, or exceeding a cap. Independent plan review must confirm
the family partition before Rust. Terminal review is required before commit.

## Implementation candidate evidence

The candidate adds exactly five named-only `doc` parameters and five calls to
the existing validator. It adds no field, value kind, collection, side table,
or downstream branch. The table-driven proof covers all thirteen existing
constructors: omitted, `None`, and two distinct named strings yield equal
snapshots of every retained `AttributeDefinition` field; integer and list docs
fail; and a positional string fails for every constructor.

The focused proof passes. The complete `slug_loading_v2` library is 463 passed,
0 failed, and 1 expected ignored authenticated-source test. Complete
`slug_analysis_v2` unit, integration, and doc tests pass. `slug_cli_v2` rebuild,
formatting, Cargo metadata, and diff checks pass. The archive checker reports
only its three known retained thoughts paths. Production accounting is 10 net /
10 gross lines; proof is 89 net / 89 gross; total Rust is 99 gross. The parked
registration proof remains unchanged at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`.

The daemon-clean authentic replay passes rules_rust's prior
`label_keyed_string_dict(doc=...)` call and stops at the separately planned
label-dependency-control family: `rust/private/rust.bzl:865` calls
`attr.label_list(allow_files=[".rs"], flags=["DIRECT_COMPILE_TIME_INPUT"])`,
and Slug rejects the unsupported `flags` keyword. No rule implementation,
configured analysis, `cc_common`, or C++ owner is reached.

Independent terminal review returned `ACCEPT`: the diff contains only the five
named-only parameters and shared-validator calls, the complete proof matrix
matches the contract, no retained/schema/DICE state changes, caps are exact,
and `flags` is correctly left to the separate label-dependency-control family.

## Immediate predecessor

Commit `18b2549bd` terminally accepts exact Bzlmod assigned-global selection,
private/public alias/reexport behavior, and repository-rule first-producer
identity. Two real replays pass the rules_cc private repository-rule boundary
and stop identically at rules_rust
`attr.label_keyed_string_dict(doc=...)`, selecting this successor.
