# Current Slug V2 Packet

Packet: `WP-5-m1-external-restricted-visibility-query-implementation`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: implementation worker
Evidence: accepted seven-row external Restricted-visibility oracle, complete
consumer design, root visibility/NODEP oracle, standalone external package-
group projection, and external query package identity recorded in the owner
plan.

Change exactly these four files:

- `app/slug_loading_v2/src/visibility.rs`;
- `app/slug_query_v2/src/graph.rs`;
- `app/slug_query_v2/src/loading_environment.rs`; and
- `app/slug_query_v2/tests/loading_query.rs`.

Add pure provisional-root-to-selected-repository projection for
`RuleVisibility` and `PackageGroupContents`, failing on retained nonroot
inputs. Before the external graph target loop/source synthesis, admit exactly
one explicitly Restricted native `filegroup` with at least one direct same-
package already-loaded native group and no direct package pseudo-specs. Reject
Restricted defaults, a second Restricted target, another protected kind, and
named-repo/cross-package/missing/alias/wrong-kind groups.

Project raw declared top-level labels as one explicit route-aware
`visibility` attribute; project effective top-level labels separately as
`VisibilityNodep` edges before ordinary filegroup edges; retain includes only
as existing `PackageGroupInclude` edges; contextualize group contents to the
selected canonical repository. Make `visible()` recover each canonical group
through the target's verified apparent route. Reuse existing DICE owners.

Cap the change at 190 net production lines, 300 net test lines, and 490 total.
Cover raw/effective/include separation, every rejection before source
synthesis, exact/recursive positive and negative contents, public/private,
include union/reallow/cycles, root/same-/different-external callers, every
enabled consumer/output in the owner design, and cold/warm/edit/delete/
recreate/route-remap lifecycle behavior. Run Cargo serially. Rebuild
`slug_cli_v2`, clean `slugd` before/after, and replay only the first six oracle
rows against Slug; row seven remains Bazel-only. Obtain independent latest-
diff review.

Do not edit fixtures, Cargo metadata, tools, schemas, plans beyond the
terminal reviewed result, or another Rust file. Do not run Bazel. Stop with
**REPLAN** on any extra path/line need, PackageRecorder-wide canonicalization,
implicit/default/direct-pseudo visibility, permissive missing/wrong-kind
fallback, direct named-repository specs, cross-package/repository group
loading/includes, dependency-filter flag support, discovery/enumeration, a new
key/route/owner, direct filesystem observation, configuration, analysis/
actions/execution, JVM, Java bytecode, or Bazel delegation.
