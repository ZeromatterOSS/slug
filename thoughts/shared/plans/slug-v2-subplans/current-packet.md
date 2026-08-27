# Current Slug V2 Packet

Packet: `WP-4-5-7A-loading-root-subtree-package-owner-extraction`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Result: move the existing root-workspace subtree package-set DICE owner from
the query crate into loading, and make query consume that same owner with no
observable behavior change. Do not add external traversal, target-pattern
expansion or registration activation.

## Learned facts and decision

Commit `e9947e8ba` completes the shared absolute package and recursive target-
pattern syntax. Registration expansion now needs one package-enumeration
primitive that is not query-owned.

Today `slug_query_v2::graph` owns `RootSubtreePackageSetKey`, its observed key,
result, traversal and marker probes. The computation depends only on loading,
bzlmod and Host path/package-boundary inputs. Query adds no semantic input; it
only converts the loading terminal to `QueryError`, merges the observed epoch
into its request and loads each discovered package. The accepted
`typed_recursive_query_unions_package_roots_and_replays_package_lifecycle`
regression already discriminates Need propagation, multiple package roots,
root precedence, ignore/package-policy changes and create/edit/delete/restore.

Bazel 9.2 `RecursivePkgFunction`, `RecursiveDirectoryTraversalFunction` and
`RecursivePkgFunctionTest` treat recursive package discovery as a reusable
Skyframe loading primitive below target-pattern consumers. The natural Slug
owner is therefore `slug_loading_v2`, not query and not the future registration
projection.

## Required implementation

1. Add one cohesive loading module for the existing root subtree package-set
   result, terminal error, legacy/observed DICE keys and traversal helpers.
   Preserve semantic key inputs/equality as normalized workspace plus package
   prefix; the Rust key type necessarily moves to its natural crate owner.
2. Move the existing computation mechanically. Preserve package-root order,
   package-marker and ignored-directory policy, non-UTF-8 handling, lexical
   sort/dedup, observed-outer before accumulated Need before terminal-error
   precedence,
   observation merging, complete-only equality/validity and display text.
3. Give loading a loading-owned terminal type. Query converts it to the same
   `QueryError` text only at the consumer boundary and continues to merge the
   observed epoch before loading packages.
4. Remove the moved owner and now-unused Host traversal imports from
   `slug_query_v2::graph`; import the loading-owned keys/value in the loading
   query environment. The legacy non-root `SubtreePackageSetKey` remains query-
   local and unchanged.
5. Preserve the accepted recursive-query lifecycle regression byte-for-byte
   where practical. Add only small loading owner/API regressions needed to
   distinguish key construction, display, result/error access and complete-
   only behavior; do not copy its large query harness.
6. Compare the moved production body and dependency inventory against the
   predecessor so this packet proves ownership extraction, not a semantic
   rewrite.

## Architecture, compatibility and guidance

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`,
`RecursivePkgFunction.java`, `RecursiveDirectoryTraversalFunction.java` and
`RecursivePkgFunctionTest.java` are pinned semantic guidance. No new oracle is
needed because the accepted recursive-query lifecycle is more discriminating
for this ownership-only change and public output must remain identical.

Zabel is peer guidance only. Its `load/session_recursive_package_discovery.zig`
keeps recursive package discovery behind a natural loading producer, while
`query/main_workspace_recursive_deps_command.zig` and toolchain consumers
demand that producer without owning a second traversal. Slug adopts that
ownership lesson, not Zabel's Zig types, session store, allocation model,
diagnostics or behavior authority.

This is general Starlark/loading architecture. Bazel 9 BCR Starlark remains the
source of rule definitions including `cc_internal`; neither `cc_common` nor a
ruleset owns this traversal.

- **Exact:** the already-admitted root recursive package set, package-root and
  marker/ignore behavior, lexical result, query output, observed-outer before
  accumulated Need before terminal-error precedence and observed lifecycle.
- **Slug-native:** Rust module/API shape, DICE key/value layout, error wrapper
  and observation carrier.
- **Unsupported/deferred:** selected-external subtree ownership, repository
  mapping, target-pattern conflict lookup or expansion, registration filters,
  configured provider/settings validation, rule implementations and actions.

The result remains DICE-retained semantic state as one `Arc` slice of compact
package strings plus the existing observed epoch. Moving its module does not
change publication, equality cutoff, invalidation, cancellation or release.
No command scratch is retained and no lock may span a DICE compute. Both
touched query production files exceed 2,000 lines; extracting this cohesive
owner reduces mixed responsibility rather than adding another concern there.

## Allowlist and validation

Base is `e9947e8ba`. Change only:

- new `app/slug_loading_v2/src/root_subtree_package_set.rs`;
- `app/slug_loading_v2/src/lib.rs` (module and narrow exports only);
- `app/slug_query_v2/src/graph.rs` (remove the moved owner/imports only); and
- `app/slug_query_v2/src/loading_environment.rs` (loading-owned imports and
  terminal conversion only).

The first exact extraction showed that the 583 removed query lines require 688
production and 19 proof additions after the loading-owned terminal, exports
and boundary adaptation are counted. The original 620-line production cap was
a planning miss, not a scope or behavior change. Corrected caps are 740
production, 100 proof and 840 total additions; deletions do not buy budget.
Each genuinely new helper/test is at most 100 lines; mechanically moved
predecessor helpers retain their reviewed shape. Add no dependency, new
DICE key semantics, external route/source input, traversal branch, package
load, pattern parser, expansion, mapping, registration activation, interner,
global state or manual lock.

Run all `slug_loading_v2` tests, the named recursive-query lifecycle regression,
all `slug_query_v2` tests, locked query/core checks and locked CLI build
serially. Run format, diff, scope/cap/helper, archive, moved-body, dependency,
DICE/no-lock and utility/retained-size audits. Public cross-crate review must
confirm that query is only a consumer and that no second traversal remains.

STOP and `REPLAN` for changed result/error/Need ordering, a new filesystem or
fresh-graph bypass, query-specific policy in loading, an external repository
owner, mapping or expansion, copied lifecycle harness, new retained utility,
allowlist/cap escape or any accepted recursive-query result change.

## Immediate predecessor

Commit `e9947e8ba` accepts `WP-4-5-7A-registration-target-pattern-syntax` at
147 production and 445 proof lines. It preserves suffix spelling and wildcard
ambiguity while newly represented all-target forms fail closed before loading.
This packet implements only bounded registration-architecture sequence step
3a; the selected-external subtree owner remains the next separate slice.
