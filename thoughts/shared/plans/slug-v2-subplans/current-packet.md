# Current Slug V2 Work Packet

Packet: WP-7-117-m7a-label-list-empty-aspects-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Admit `attr.label_list(aspects=[])` as the same zero-attached-aspect
declaration as an omitted `aspects` argument. The rebuilt authentic `slug
build //:probe` in `/tmp/slug-wp797-authentic-2YQmSt` must clear the
`only one required aspect is supported` error at
`@@rules_python+//python/private:attr_builders.bzl:89`; any later stop is a
separate packet. The prior WP816 receipt called this a multiple-required-
aspect stop, but authenticated source shows its builder supplies an **empty**
list. The cardinality error alone did not identify that value.

Exact Bazel 9.2 slice: omitted/default-empty and explicit empty list attach
zero aspects on `attr.label_list`. Existing singleton exported-aspect handling
remains unchanged. Slug-native: diagnostics and structural rule/schema source
identity. Unsupported/deferred: two or more attached aspects, duplicate and
ordered aspect propagation, configured aspect traversal, broader attr family
aspects, and exact Bazel output/configuration identity. Direct `aspects=None`,
nonsequence and wrong-element values remain errors. Bazel accepts distinct
two-aspect lists, but this packet does not claim them.

## Source, owner and decision

Pinned Bazel 9.2 HEAD `8220c6198837d5c13d53fea211cf3282aa12408a`:
`StarlarkAttrModuleApi.java:704-718` declares `attr.label_list` aspects as a
sequence defaulting to `[]`; `StarlarkAttrModule.java:498-510` adds one builder
aspect per element, so zero elements add none. Offline Bazel 9.2 scratch
`/tmp/slug-wp817-aspects-Ejq9Vl` (ignored RCs/downloads disabled) accepts
empty, one, and distinct two-aspect declarations through query/build;
explicit direct `None`, a string element and a duplicate are rejected.
Authenticated rules_python 1.7.0 `builders_util.bzl:86-90` and
`python/private/attr_builders.bzl:74-78,842-89` normalize omitted builder
aspects to `[]` and forward them. The selected `data` and `deps`
`LabelList` constructors in `python/private/attributes.bzl:155-165,205-209`
do not specify aspects, making the actual forwarded value empty; no
multiple-aspect inference comes from the authentic stop.

Slug `app/slug_loading_v2/src/package.rs:4906-4924` currently parses the
optional `aspects` list with a one-element slice pattern; that branch rejects
both `[]` and 2+. Return `None` for an empty list, retain the one exported
aspect validation, and leave 2+ rejected. The existing
`AttributeDefinitionGen.attached_aspect: Option<V>` and frozen
`RuleAttributeSchemaGen.attached_aspect: Option<V>` already represent the
correct empty case. No retained representation, DICE key, cache, lock,
Analysis or Query change. The existing loaded Bzl source/manifest owner and
frozen rule schema continue to own equality, invalidation, export/replay,
Need/error and A/B/A restoration; no command-side normalization or filesystem
probe is introduced. Evaluator list values are phase scratch and freeze only
the admitted optional aspect fact.

## Evidence and validation

Move explicit `attr.label_list(aspects=[])` out of the negative group in the
existing focused dependency-schema test, assert success alongside omitted
aspects, and first demonstrate that this selected regression fails on the
current production branch. Preserve protected singleton/exported aspect and
rule-invocation rejection tests. Keep direct `None`, wrong element/nonlist,
unexported singleton, multiple distinct and duplicate cases rejected; do
not weaken the existing 2+ controls. Reuse the offline oracle; no Stage 1
fixture growth absent a demonstrated gap.

Resolve the pinned-nightly Cargo toolchain, compile the selected Loading lib
test binary with JSON `--no-run`, exact nonignored selector preflight, and
run focused/protected tests serially. Direct Core/CLI dependent compile and
`cargo build -p slug_cli_v2` precede one normalized-`PATH` authentic smoke.
Check/clean `slugd` before/after; run Rust format, diff, archive and plan
status checks. Independent DESIGN review precedes implementation and FINAL
review checks actual diff/evidence. No broad suite or remote transport; do
not inspect or alter BuildBuddy authentication.

## Scope and stops

Base clean main `0f832b895`. Allowlist: this manifest, canonical Current
paragraph and the WP816 diagnostic wording in Stage 4; after acceptance a
compact Stage 4 owner note; production
`app/slug_loading_v2/src/package.rs` only; focused tests in
`app/slug_loading_v2/src/host_package_load_tests.rs`. Rough growth 5
production/30 proof lines is a review trigger, not a license to broaden the
public aspect model. `package.rs` is already large, but the one branch is
the existing semantic owner; no new abstraction is warranted. REPLAN if
empty list is not Bazel-equivalent to omission, if Slug's optional schema
cannot preserve the fact, or if the selected authentic path supplies a
nonempty list. A later error is not authority to change another attr kind.

## Acceptance receipt

Independent DESIGN and FINAL reviewers accepted the bounded exact slice. The
selected dependency-schema regression first failed on the unmodified branch
with `attr.label_list(aspects=[])` and `only one required aspect is supported`.
After the three-line parser guard, it asserts that the frozen rule schema has
no attached aspect; it and three protected Loading tests pass (4/4).
Pinned-nightly Loading JSON `--no-run` compiled, Core/CLI `cargo check` and
the rebuilt CLI succeeded. No `slugd` was present before or after validation.
The normalized-PATH authentic build clears the selected label-list error and
stops later at a separate `label_keyed_string_dict(aspects=[])` unexpected-
keyword error in rules_python's `attr_builders.bzl:758/89`. That attr-family
surface is not admitted here. Rust formatting, diff, plan and archive checks
pass. No remote transport or BuildBuddy authentication was used.

## Immediate predecessor

WP816 accepted at `0f832b895`: the source-guarded 26-field `py_internal`
namespace clears the prior undefined-global stop. Its aspect-cardinality
diagnosis is corrected above; helper invocation remains deferred.
