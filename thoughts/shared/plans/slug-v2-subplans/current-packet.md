# Current Slug V2 Packet

Packet: `WP-4-5-7A-repository-rule-attribute-family-architecture`

Milestone: M7A bootstrap-critical repository/ruleset closure.

Base: accepted complete direct Bazel 9.2 `tools/build_defs/repo` catalog
`3023718a0` and accepted generated-repository route/owner `f747507f6`.
Selected-context, configured-analysis, registration, and REAPI candidates remain
dirty, parked, and read-only.

## Immediate predecessor

Commit `3023718a0` imports the complete direct embedded
`tools/build_defs/repo` package: `BUILD.repo` as `BUILD` plus all eight sibling
`.bzl` files, 2,513 exact lines/96,027 bytes at mode 0644. Exact hashes,
checked-in inventory, direct listing, package-set visibility, manifest identity
`de4c7231…`, 592 Bzlmod tests, 430 loading tests, all integrations, CLI build,
two cold replays, and independent architecture/terminal reviews pass.

Both cold replays now advance beyond `utils.bzl` and stop identically while
loading `@@bazel_features+//private:globals_repo.bzl`:

```text
unsupported repository_rule attribute schema 'globals'
globals = attr.string_list_dict(mandatory = True)
```

This is unrelated to `cc_common`, `cc_internal`, C++ rules, the parser, or
Starlark `set`. It is the first live discriminator for Slug's deliberately
scalar-only repository-rule attribute bridge.

## Design question and category boundary

Freeze one generic architecture for the complete repository-rule attribute
value family already exposed by Slug's shared `attr` module, rather than adding
a `globals` or `string_list_dict` special case. The family is all thirteen
Bazel 9.2 public descriptor kinds:

```text
bool, int, string, label, output,
string_list, label_list, output_list,
string_dict, string_list_dict,
string_keyed_label_dict, label_keyed_string_dict, label_list_dict
```

The architecture must make later descriptor-policy breadth additive without
changing the retained raw-call or RepoSpec value representation. It must keep
ordinary module-extension calls and innate `use_repo_rule` calls on the same
coercion and publication semantics.

This packet is docs/evidence/design only. It authorizes no Rust or fixture
change. Its terminal output is an independently reviewed implementation packet
or `REPLAN`.

## Learned facts and authority

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is sole
semantic authority:

- `StarlarkRepositoryModule.repositoryRule` casts every `attrs` value to a
  standard `Descriptor` and passes `Descriptor.build(attrName)` directly to the
  `RepoRule` builder. It does not define a scalar-only repository schema.
- `RepoRule.instantiate` runs the common `AttributeUtils.typeCheckAttrValues`,
  preserves only explicitly supplied non-None attributes in `RepoSpec`, and
  resolves labels through the definition's repository mapping.
- `StarlarkAttrModuleApi` defines exactly the thirteen public kinds above.
- A disposable Bazel 9.2 oracle instantiated one repository rule with an
  explicit nonempty value for every kind and successfully queried its generated
  `@probe//:x`. No oracle workspace or copied output is retained.

The live Slug audit establishes:

1. `AttributeKind`, `AttributeDefinition`, and `CoercedAttributeValue` already
   represent all thirteen kinds and preserve ordered nested values.
2. `repository_rule()` nevertheless accepts only String/Boolean/Integer/Label
   definitions and scalar-compatible defaults.
3. `RepositoryRuleCallValue` captures only None/bool/i32/string/canonical label;
   ordinary extension list/dict/tuple values therefore fail before type
   coercion.
4. innate `use_repo_rule` values already retain list, tuple, dict, string-key,
   and label-key structure in `NonrootAttributeValue`, but its bridge rejects
   every collection.
5. `OverrideAttributeValue` and `OverrideAttributeKey` already provide the
   complete heap-independent recursive iterable/map/label RepoSpec carrier.
   Their `SmallMap` structural equality and the three RepoSpec hash owners are
   intentionally membership-based, while `RepoSpecPublicationIdentity`
   currently restores observable map order only for the built-in
   `http_archive`/`git_repository` `remote_patches` attribute. The retained
   carrier is complete, but its generic recursive publication projection is
   not.

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` remains
**concept/test only** guidance. Its repository declaration/execution path uses
one complete typed attribute family, including `string_list_dict`, and avoids a
per-ruleset adapter. Use that completeness and test shape as guidance only; do
not reuse Zabel code, scheduler, store, tokens, fingerprints, or semantics.

## Required architecture decision

The design must freeze:

- one heap-independent recursive raw-call carrier for None, bool, i32, string,
  canonical label, sequence, and string/canonical-label-keyed map values;
- list and tuple acceptance with one post-coercion sequence identity, ordered
  dictionary values, cycle rejection, i32 bounds, and fail-closed unsupported
  keys/values;
- one kind-directed coercer from that carrier into existing
  `OverrideAttributeValue`, including mapping-relative labels at every nested
  position and output-label behavior;
- one generic `RepoSpecPublicationIdentity` projection which keeps top-level
  attributes name-addressed but compares and hashes every nested map's key
  order and recursively projected values in insertion order; replace the
  `remote_patches` ruleset special case, preserve existing membership equality,
  and update all three RepoSpec hash owners through the shared projection;
- shared ordinary-extension and innate-call conversion, without raw Starlark
  heap values escaping evaluation;
- complete default-kind validation for the thirteen kinds while retaining
  Bazel's rule that only explicit non-None call attributes enter RepoSpec;
- exact error/order behavior for unknown, mandatory, wrong-kind, bad label,
  missing mapping, duplicate repository name, and cyclic raw values; and
- a descriptor-policy ledger distinguishing metadata supported in this packet
  from explicit `configurable`, transition/cfg, executable, allow-files,
  allow-single-file, provider, allowed-values, `remotable`, and other policy
  breadth. Unsupported policy must continue to fail closed, but the retained
  value carrier must not need redesign when it is later admitted.

Prefer the existing `OverrideAttributeValue` recursive shape or a bounded
loading-private isomorphic carrier only after reviewing ownership and stage
meaning. Do not duplicate thirteen bespoke stored variants when the recursive
carrier can preserve exact values compactly. Do not widen tag-class attributes,
configured rule attributes, root module parsing, or repository execution Host
methods merely because they reuse `attr` syntax.

## Compatibility and ownership

- **Exact target:** accepted descriptor kinds; explicit-value type coercion;
  list/tuple equivalence after coercion; dictionary order/content; nested label
  resolution; intrinsic/explicit defaults; mandatory and unknown handling;
  explicit non-None RepoSpec publication; ordinary/innate parity; and stable
  errors/order demonstrated by Bazel 9.2 source or oracle.
- **Slug-native:** Rust recursive carrier types, `Arc`/`SmallMap` layout, DICE
  key names, error enums, membership equality, and the conservative recursive
  publication-equality/hash projection that prevents an order-observable map
  change from being cut off. This projection is semantic identity, not a claim
  about Bazel Java `Dict` equality or fingerprint bytes.
- **Unsupported/deferred:** descriptor policy not admitted by the frozen ledger;
  `remotable`; additional repository_ctx Host methods; repository action or
  download breadth; other Starlark builtin categories; parser/set work; C++
  rule/action semantics; exact JVM/HotSpot state; and later bootstrap families.

Natural owners remain the loading-owned repository-rule definition/call and
instantiation pipeline plus the existing Bzlmod `RepoSpec`. No new DICE key,
global registry, side cache, command repair, physical source lookup, or fallback
is permitted. Calls are evaluator/phase scratch until copied into existing
heap-independent invocation receipts; RepoSpecs remain DICE-retained semantic
values. No retained value may borrow a Starlark heap.

## Evidence and implementation-plan deliverable

The design must cite the exact Bazel source/test/oracle rows for all thirteen
kinds and at least these discriminators:

1. empty/nonempty list and tuple inputs normalize to the same typed value;
2. ordered string and label dictionaries retain key/value association;
3. labels resolve correctly as strings and `Label()` objects at scalar, list,
   key, and nested-list positions;
4. defaults of every kind validate without being published when omitted;
5. explicit None is omitted; mandatory, wrong-kind, invalid key, cycle, large
   integer, missing mapping, and unknown attribute fail at the natural stage;
6. ordinary and innate calls publish equal RepoSpecs for equal semantics;
7. A/B/A value/mapping changes restore structural identity; and
8. the real `bazel_features` globals repository advances in two fresh replays.

The A/B/A matrix must include reordered dictionary entries with unchanged
membership for both ordinary and innate calls. It must prove the recursively
ordered publication projection changes and restores, while ordinary structural
membership equality remains unchanged. Audit `RepoSpec::eq` and every live
RepoSpec hash owner together so equality/hash contracts remain coherent.

Reuse existing synthetic extension/innate/mapping/DICE scaffolding and the real
rules_rust fixture. Add no checked-in oracle fixture unless pinned source plus a
disposable probe cannot discriminate a required behavior; if one is necessary,
freeze its provenance before implementation.

The implementation packet must name exact live blobs and hunks. Its Bzlmod
allowlist must include the retained projection owner in `module_eval.rs`, both
route/request hash owners in `canonical_repository_route.rs` and
`host_module.rs`, and the existing publication-identity proof in
`selected_repo_spec.rs`; no additional RepoSpec equality/hash owner may remain
unaudited. In particular,
`app/slug_loading_v2/src/package.rs` is already dirty with unrelated
definition-source work; any later packet must freeze its live blob, permit only
the repository-rule schema hunk, preserve the parked changes byte-for-byte, and
stage only packet-owned hunks. It must set production/test caps, complexity
decisions for `package.rs` and the 2,249-line instantiation owner, serial crate
and direct-dependent validation, two clean replays, and independent terminal
review.

## Allowlist and stops

Only these scheduling documents may change in this design packet:

- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`; and
- the orchestration routing log only if review returns `REPLAN` or a reusable
  unusual routing lesson.

Validate source anchors, disposable oracle result, dirty isolation, canonical/
manifest packet-ID consistency, `git diff --check`, and independent architecture
review. No Cargo build/test is required because Rust is unchanged.

`REPLAN` if exact semantics require a new public cross-crate value or DICE owner;
if the thirteen kinds cannot share one recursive raw carrier and kind-directed
coercer; if ordinary and innate calls require different RepoSpec semantics; if
the dirty `package.rs` work cannot be isolated; if descriptor policy cannot be
bounded without silently claiming parity; if any parser, set, C++ rule,
ruleset-specific, filesystem, materialization, JVM, or fallback path is needed;
if generic recursively ordered publication identity cannot replace the current
ruleset-specific `remote_patches` projection without widening storage; or if a
second material architecture correction is required after review.
