# Current Slug V2 Packet

Packet: `WP-4-5-7A-repository-rule-attribute-family-implementation-r2`

Milestone: M7A bootstrap-critical repository/ruleset closure.

Base: accepted architecture `a286f0b04`, accepted complete direct Bazel 9.2
`tools/build_defs/repo` catalog `3023718a0`, and accepted generated-repository
route/owner `f747507f6`. Selected-context, configured-analysis, registration,
and REAPI candidates remain dirty, parked, and read-only.

## Observable result

Implement the reviewed complete repository-rule attribute value category. Both
ordinary module-extension repository calls and innate `use_repo_rule` calls
must accept and identically publish all thirteen Bazel 9.2 public kinds:

```text
bool, int, string, label, output,
string_list, label_list, output_list,
string_dict, string_list_dict,
string_keyed_label_dict, label_keyed_string_dict, label_list_dict
```

Two fresh real rules_rust replays must advance beyond the current
`bazel_features` rejection at
`globals = attr.string_list_dict(mandatory = True)` and stop identically at the
next authentic unsupported boundary or succeed. That successor is evidence,
not authorization to widen this packet.

## Accepted architecture and authority

Commit `a286f0b04` records independent R2 `ACCEPT`. Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole semantic authority:

- `StarlarkRepositoryModule.repositoryRule` accepts standard `Descriptor`
  values and adds `Descriptor.build(attrName)` without a scalar-only schema.
- `ModuleExtensionEvalStarlarkThreadContext.deepCloneAttrValue` immediately
  copies None/bool/int/string/Label, ordered dictionaries, and iterable values
  from the evaluator call.
- `RepoRule.instantiate` delegates all definitions and calls to
  `AttributeUtils.typeCheckAttrValues`, then publishes only explicitly supplied
  non-None nonlegacy attributes in invocation order.
- `AttributeUtils.typeCheckAttrValues` converts by descriptor kind, validates
  mandatory/default values, and recursively rejects nonvisible labels.
- `InnateRunnableExtension` calls the same `RepoRule.instantiate` path with its
  owner mapping and definition-package label converter.
- `StarlarkAttrModuleApi`/`BuildType` define exactly the thirteen kinds above;
  `BuildType.OutputType` additionally requires output labels to remain in the
  converter's base package.
- A disposable Bazel 9.2 oracle successfully instantiated one repository rule
  with explicit nonempty values of every kind. No oracle workspace is retained.

Relevant upstream tests are
`ModuleExtensionResolutionTest`'s complex `string_list_dict` rows and
`starlark_repository_test.sh`'s typed/default label rows. They do not cover the
whole category, so the accepted disposable all-thirteen probe plus pinned
source remains the stronger category evidence. Add no checked-in oracle
fixture.

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
**concept/test only** guidance: retain one complete typed family and avoid a
per-ruleset adapter. Copy no Zabel code, scheduler, store, fingerprint, token,
or semantic claim.

The Buck2-derived utility review keeps the existing `Arc`, `CompactString`,
`SmallMap`, `SmallSet`, `Allocative`, and Starlark value-identity utilities.
No new collection, interner, cache, strong hash, or dependency is admitted.
The Stage 9 retained-utility row remains unchanged because this packet adapts
an existing V2-owned carrier rather than importing a donor utility.

## Implementation decision

1. Extend loading-private `RepositoryRuleCallValue` with one normalized
   sequence variant and one ordered recursive map variant whose keys are
   strings or canonical labels. Lists and tuples deep-copy to the same sequence
   identity. Copy ordinary calls immediately, reject active-container cycles,
   reject integers outside i32 and unsupported values/keys, and retain no
   evaluator value or identity.
2. Convert innate `NonrootAttributeValue` lists, tuples, dictionaries, strings,
   labels and scalar values into that same call carrier. Deferred-invalid
   float/builtin/proxy/cycle tokens continue to fail closed.
3. Replace the scalar `convert_supplied` match with one kind-directed recursive
   coercer into the existing `OverrideAttributeValue`. Resolve raw strings
   through the definition/owner base and final mapping at every scalar, list,
   dictionary key and nested-list label position; preserve authenticated
   canonical Label objects and validate visibility. Enforce the definition
   package for output/output-list values.
4. Admit all thirteen kinds in `repository_rule()` while continuing to reject
   explicit configurable policy, transitions/cfg, executable, allow-files,
   allow-single-file, providers and allowed-values. Add a repository-only
   complete default-kind predicate; do not widen tag-class validation.
5. Validate every omitted explicit default by kind and recursive label/output
   visibility, but retain Bazel's rule that defaults do not enter `RepoSpec`.
   Explicit None remains omitted and therefore does not satisfy mandatory.
6. Replace the `http_archive`/`git_repository` `remote_patches` publication
   exception with one borrowed generic projection. Top-level attributes remain
   name-addressed and membership-equal; every nested map compares and hashes
   its ordered key/value sequence recursively, including maps beneath
   sequences. `RepoSpec::eq` continues to combine structural membership
   equality with this projection. The two production RepoSpec hash owners
   already call the projection and must remain unchanged unless a compile or
   equality-contract proof demonstrates a necessary correction.

## Compatibility and non-decisions

- **Exact:** the thirteen admitted descriptor kinds; list/tuple normalization;
  ordered sequence/dictionary values; nested label/output coercion; intrinsic
  and explicit default validation; mandatory/unknown/wrong-kind/bad-label/
  missing-mapping behavior; explicit non-None publication; and ordinary/innate
  value parity.
- **Slug-native:** Rust carrier names, `Arc`/`SmallMap` layout, error enums and
  the conservative recursive publication equality/hash projection. The latter
  prevents DICE cutoff of an order-observable repository input and is not a
  claim about Java `Dict` equality or Bazel fingerprint bytes.
- **Unsupported/deferred:** explicit descriptor policies named above;
  `remotable`; additional `repository_ctx` methods; repository action/download
  breadth; parser or `set` work; other Starlark builtin categories; `cc_common`,
  `cc_internal`, rules_cc or C++ rule/action semantics; JVM/HotSpot state; and
  the next replay boundary.

No new public cross-crate value, DICE key, global registry, side cache, command
repair, filesystem read, physical materialization, fallback, parser branch, or
ruleset special case. BCR Starlark remains the rule owner; `cc_common` remains
only a future generic Host/provider ABI consumer.

## Ownership, request/revision, and memory

The frozen loading definition owns schema metadata. The invocation evaluator
owns mutable Starlark values only until it copies a call into the existing
heap-independent receipt. The loading instantiation owner combines that receipt
with the authenticated definition label and final repository mapping and
publishes the existing `RepoSpec`. Ordinary and innate calls converge before
coercion; no command-local adapter survives publication.

`RepoSpec` remains DICE-retained semantic state. Per `docs/developers/dice.md`,
every order-observable nested map participates in equality cutoff and in both
route/request hashes. A/B/A changes must restore equality and hash identity.
No lock, task, async transfer, cache, eviction, shutdown, or cancellation owner
changes. All new scratch allocations die with evaluation/instantiation; no
retained value borrows a Starlark heap or request scratch. Overlapping requests
observe immutable receipts/specs through existing DICE dependencies.

## Proof matrix

Focused tests must prove:

1. ordinary call capture deep-copies None/scalars/Label, list and tuple to one
   sequence form, ordered nested dictionaries, and rejects cycles, large ints,
   invalid keys and callable/unsupported values without a partial record;
2. definitions admit every kind and correct explicit/intrinsic default while
   every deferred descriptor policy still fails closed;
3. explicit empty/nonempty values of all thirteen kinds produce exact
   `OverrideAttributeValue` shapes and preserve order;
4. string and Label-object labels resolve at scalar, list, map-key, map-value,
   nested-list, output and output-list positions, with missing mapping,
   invisible canonical labels and cross-package outputs rejected;
5. defaults of all thirteen kinds validate but remain unpublished; explicit
   None is omitted; mandatory, unknown and wrong-kind failures retain natural
   ordering;
6. ordinary and innate calls with equal semantics publish equal RepoSpecs;
   their list/tuple spellings normalize equally and their invalid raw forms fail
   through the same coercion boundary;
7. reordered nested maps with unchanged membership are structurally equal but
   have distinct generic publication identity and both production route hashes;
   A/B/A restores identity for ordinary and innate inputs; reordering only
   top-level attributes preserves publication equality and both production
   route hashes; and
8. existing selected-BCR `remote_patches` order proofs remain green without a
   built-in/rule-name special case.

Reuse colocated unit/DICE scaffolding and the real rules_rust workspace. No new
fixture, copied registry subtree, manifest, or expected output is admitted.

## Exact allowlist and dirty isolation

Base `a286f0b04`; exact live blobs before implementation:

- `app/slug_loading_v2/src/module_extension_repository_rule.rs`
  `af918a5f55a5735d2727e8b98211b45fc17718d8`;
- `app/slug_loading_v2/src/module_extension_repository_instantiation.rs`
  `0b64abfed3e451015035d94289fa6c426dc0f498`;
- `app/slug_loading_v2/src/module_extension_innate_repository.rs`
  `a04272161682f86b36c8e46cc13662d52b7410aa`;
- `app/slug_loading_v2/src/package.rs` live
  `46669562b2326c16fb720394e1e9b56328730f27`, whose HEAD blob is
  `b48b51d7360c75fd4415564d70cf651e760933a3`;
- `app/slug_bzlmod_v2/src/module_eval.rs`
  `31b1ccb368123481db0f1eef9d795e04d633db36`;
- `app/slug_bzlmod_v2/src/selected_repo_spec.rs`
  `aa047c0236fda9f853b7806f7aeaf75b9408ad2a`.

`canonical_repository_route.rs` blob
`42458a059436e9920948263314ddc03b5406e084` and `host_module.rs` blob
`b849106d9028d1a5f384cd11e9cbe17ffe8aca25` are read-only audited hash
consumers. Change them only after `REPLAN`.

The pre-existing 28-line `package.rs` definition-source diff occupies hunks
near lines 690-6125 but not the repository-rule schema hunk near 5613. Preserve
those bytes exactly. At close, stage only the packet-owned repository-rule hunk
from `package.rs`; never stage the whole file. Every other dirty analysis,
loading, core and REAPI file is parked and excluded.

Scheduling documents may change only to record terminal acceptance, `REPLAN`,
or the next authentic packet.

## Caps, complexity, validation, and stops

Cap net Rust production growth at 550 lines, net Rust test growth at 900 lines,
and total at 1,450 lines. No new file, crate, dependency, unsafe code, key,
fixture, background task, lock, cache, fallback, or public compatibility shim.

`package.rs` (6,959 live lines) remains the cohesive loading-global definition
owner; its only packet hunk is a schema-kind/default predicate and extracting a
new module would split declaration policy from its producer. The 2,249-line
instantiation owner remains cohesive because all added code is one recursive
attribute coercer beside its existing mapping/default logic. No touched
function may exceed 150 lines; split local conversion helpers if needed.
`module_eval.rs`, `host_module.rs`, and `selected_repo_spec.rs` are also large,
but this packet touches only the existing compact RepoSpec identity function
and colocated proofs; semantic, presentation, persistence and transport owners
do not move.

Validate serially:

1. focused loading capture/schema/coercion/innate tests and focused Bzlmod
   publication-identity/hash tests;
2. `cargo test -p slug_bzlmod_v2` then `cargo test -p slug_loading_v2`;
3. `cargo build -p slug_cli_v2` before any `SLUG_V2_BIN` replay;
4. clean stale `slugd`, run two copied-workspace/fresh-output-root real
   rules_rust cqueries, and clean stale `slugd` afterward;
5. `cargo fmt --all -- --check`, `git diff --check`, exact allowlist/cap and
   package dirty-byte isolation checks; and
6. `scripts/v2_archive_status.sh` plus independent terminal review.

`REPLAN` before widening if a new public cross-crate carrier or DICE owner is
required; ordinary and innate calls cannot share one coercer; output semantics
require a generated-repository context absent from the authenticated inputs;
generic recursively ordered publication identity cannot replace the special
case; a policy must be silently admitted; the dirty package hunk cannot be
isolated; a parser, set, ruleset, filesystem, materialization, JVM or fallback
path is needed; a read-only hash consumer must change for more than mechanical
equality coherence; caps are exceeded; or one focused implementation correction
does not resolve terminal review.
