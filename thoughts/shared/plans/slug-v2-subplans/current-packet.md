# Current Slug V2 Packet

Packet: `WP-5-m1-external-restricted-visibility-query-typed-implementation`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: implementation worker after REPLAN
Evidence: accepted seven-row external Restricted-visibility oracle, accepted
consumer design, and the reviewed four-file prototype retained only as
correction input in the owner plan.

Change exactly these five files:

- `app/slug_identity_v2/src/label.rs`;
- `app/slug_loading_v2/src/visibility.rs`;
- `app/slug_query_v2/src/graph.rs`;
- `app/slug_query_v2/src/loading_environment.rs`; and
- `app/slug_query_v2/tests/loading_query.rs`.

Add only
`CanonicalLabel::rebind_provisional_root_repository(&CanonicalRepoName)`.
It must reject a nonroot source label and root destination, construct the new
label directly from cloned typed package and target components, and never
format, parse, resolve, or normalize a label. Clear `mapping_id` to `None`:
the prior mapping describes the provisional apparent spelling and is stale
after selected-route contextualization. Do not alter global label equality,
hashing, ordering, serialization, parsing, or repository mapping.

Inline identity tests must prove a mapped provisional-root label rebinds to
the expected canonical external label, equals the corresponding parsed
canonical label, serializes without stale `@mapping` provenance, preserves
package and target, and rejects both invalid repository directions.

Use that API for pure `RuleVisibility` projection. Keep the accepted graph
contract: validate the complete Restricted slice before target/source
synthesis; project raw declared labels, top-level `VisibilityNodep` edges, and
`PackageGroupInclude` edges separately; contextualize group contents; and
recover visible groups through the protected target's verified apparent route.

Cap the five-file diff against the pre-packet HEAD at 220 net production
lines, 600 net test lines, and 820 total.

Tests must directly discriminate raw/effective/include separation and edge
order; every rejection winning over competing source/alias synthesis; exact/
recursive positive and negative contents; separate public-only and private-
only behavior; include union/reallow/cycles; root, same-external, and actual
different-external callers; every enabled consumer/output; cold then same-
transaction warm equality/reuse; separate content, target-visibility, and
include edits; delete/recreate and A-to-B-to-A recovery; and apparent
`repo_name` remapping over the same canonical dependency/local override,
including new apparent output and stale-route rejection.

Run Cargo serially. Rebuild `slug_cli_v2`, clean `slugd` before and after, and
replay only oracle rows 1-6 in stored order. Compare exit code, normalized
stdout, and manifest exactly with the generated Bazel command objects. Bazel
server/progress stderr is tool-specific and is not Slug expected stderr;
require Slug normalized stderr to be exactly empty for every row. Row 7 is
Bazel-only and must not be invoked. Use a temporary out-of-tree six-command
comparator over the checked-in fixture/expected JSON; do not edit the fixture,
oracle tool, or schema.

Require independent identity-boundary pre-review, focused and complete
identity/loading/query tests, direct dependent checks, applicable GNU-Windows
no-run gates, formatting, archive status, exact scope/cap checks,
`git diff --check`, and independent latest-diff review.

Preserve every existing stop gate. Stop with **REPLAN** on a sixth file, cap
excess, a general/unchecked label constructor or repository setter, retained
stale mapping provenance, changed label equality/hash/order/serialization,
PackageRecorder-wide canonicalization, implicit/default/direct-pseudo
visibility, permissive missing/wrong-kind fallback, named-repository package
specifications, cross-package/repository group loading or includes, a second
repository route, dependency-filter support, discovery/enumeration, a new key/
route/owner/lock, direct filesystem observation, configuration, analysis/
actions/execution, JVM, Java bytecode, or Bazel delegation.
