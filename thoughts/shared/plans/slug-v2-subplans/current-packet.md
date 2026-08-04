# Current Slug V2 Packet

Packet: `WP-5-m1-external-restricted-visibility-single-route-typed-implementation`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: implementation worker after reviewed route/reuse redesign
Evidence: accepted seven-row oracle, consumer and typed-projection designs,
plus the accepted single-route caller/reuse correction in the owner plan.

Change exactly these five files:

- `app/slug_identity_v2/src/label.rs`;
- `app/slug_loading_v2/src/visibility.rs`;
- `app/slug_query_v2/src/graph.rs`;
- `app/slug_query_v2/src/loading_environment.rs`; and
- `app/slug_query_v2/tests/loading_query.rs`.

Implement the previously accepted typed projection and graph contract. Add
only `CanonicalLabel::rebind_provisional_root_repository`; reject nonroot
source/root destination, clone typed package/target components, clear stale
`mapping_id`, and never format/parse/resolve/normalize. Validate the complete
Restricted slice before target/source synthesis. Keep raw declared visibility,
effective top-level `VisibilityNodep`, ordinary dependency, and existing
`PackageGroupInclude` edges separate and ordered. Contextualize group contents
and recover groups through the protected target's verified apparent route.

Directly retain every accepted consumer, rejection, contents, public/private,
include/cycle, edit/recovery, route-remap, and oracle discriminator. For the
different-external caller, add an inline `loading_environment.rs` unit test
using one real cached `@dep -> dep+` graph only: build its minimal Host epoch,
inject existing root/module policies and only the `dep+` local materialization,
then intern the real routed target plus fake same-`dep+` and synthetic-`other+`
`//viewer` consuming owners. Keep the target in the dependency root package so
neither caller reaches the package-fragment/Java shortcut. Direct `visible()`
must retain the target for the same-repository control and return no batch for
`other+`. Preserve the existing cross-repository Private and Java fragment
behavior unchanged. Do not declare,
materialize, resolve, load, or query an `other` route.

Extend only the existing `RootAnchorTracker` for warm evidence. Enable rich
activations and count public `RootQueryCommandKey` Evaluated/Reused callbacks.
For the same key and transaction, cold must add one Evaluated/no Reused and
warm must add no Evaluated/one Reused, while each compute activates one typed
root and forbidden activations remain zero. Result equality is secondary.

Cap the five-file diff from this pre-packet HEAD at 220 net production lines,
720 net test lines, and 940 total. Enforce those three caps mechanically. The
test allocation is at most 542 retained corrected-draft lines, 140 for the
single-route source-unit setup/assertions, and 30 for rich tracker/import/
assertion changes, leaving eight lines of test slack. The larger test cap is
solely for the no-hook one-route unit setup; it authorizes no new route, owner,
key, public
test hook, file, production surface, or semantic breadth.

Run Cargo serially. Require focused and complete identity/loading/query tests,
direct dependent checks, applicable GNU-Windows no-run gates, formatting,
archive/scope/cap/diff checks, and independent latest-diff review. Rebuild
`slug_cli_v2`, clean `slugd` before/after, and run only oracle rows 1-6 using a
temporary out-of-tree comparator: exact exit, normalized stdout, manifest,
and exactly empty Slug normalized stderr. Never invoke Bazel-only row 7 or edit
the fixture/tool/schema.

Preserve every earlier stop. Stop with **REPLAN** on a sixth file, cap excess,
general/unchecked identity setter, stale mapping provenance, changed global
identity semantics, implicit/default/direct-pseudo visibility, permissive
fallback, named-repository specs, cross-package/repository group loading or
includes, a second route/materialization/caller load, dependency-filter flags,
discovery, new key/owner/lock/tracker, filesystem bypass, configuration,
analysis/actions/execution, JVM, Java bytecode, or Bazel delegation.
