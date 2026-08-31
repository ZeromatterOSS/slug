# Current Slug V2 Packet

Packet: `WP-6-7A-runfiles-value-and-default-info-implementation-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 standard providers.

Status: Terminal implementation `ACCEPT` after independent design `ACCEPT`
under the category architecture accepted in commit `8911a99f2` and typed
provider-core implementation accepted in commit `8e7234b82`. A bounded cap
`REPLAN` raised production allowance without widening semantics or file scope;
terminal review then required explicit-stack SymlinkEntry DAG traversal,
alias-partition-aware publication comparison, and genuine shared-diamond
proofs. All corrections passed rereview. Base `8e7234b82`. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked;
do not edit or stage it.

## Result and boundary

Implement successor 2 of the accepted category: one typed immutable Starlark
`runfiles` value, `ctx.runfiles`, `merge`/`merge_all`, and all five
`DefaultInfo` constructor parameters (`files`, `runfiles`, `data_runfiles`,
`default_runfiles`, `executable`). Normalize raw constructor choices into the
single retained `DefaultInfo`/`RetainedRunfiles`/`FilesToRunProvider` model
before publication.

This packet constructs no `RunfilesSupport`, tree or manifest Artifact and
registers no support action. Executable providers remain incomplete and direct
or associated action use continues to fail before publication. Collection
flags, private conflict bypass, physical materialization, Spawn expansion,
execution, and ruleset-specific behavior remain deferred.

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and the source/test hashes frozen in `8911a99f2` are sole semantic authority.
`BazelRuleClassProvider.java` SHA-256
`a7de1ba5a700468ead269865f2563378ea0851d3430844ee6491591e52fd3d91`
pins Bazel 9's build-wide runfiles prefix to `_main`.
Authenticated rules_cc 0.2.17 consumers only prove generic reachability.
Zabel `0795445f...` remains peer phase-ownership and compact-layout guidance;
copy no behavior, code, layout, or claim.

## Compatibility

**Exact:** public `ctx.runfiles` parameter names, defaults and admitted outer
types; File and compatible-order depset topology; dictionary and SymlinkEntry
depset topology for normalized relative paths; public runfiles fields and
method names; empty/unique-nonempty merge identity; merge order; conflict
policy propagation; DefaultInfo five-parameter binding and legacy/stateful
mutual exclusion; executable insertion into legacy/default runfiles; data
runfiles separation; explicit-files override and predeclared-output fallback;
typed public fields; the private build-wide `_main` repository prefix; and
structural publication equality/invalidation.

**Slug-native:** Rust valid-Unicode paths, compact retained layout, and
structural DICE identity. The exact `_main` prefix is retained and compared but
has no public field or physical-path effect in this packet.

**Unsupported/deferred:** `collect_data=True`, `collect_default=True`,
`skip_conflict_checking=True`; unknown, absolute, empty or up-level symlink
paths; overlapping-link diagnostics at manifest creation; directories/tree
Artifacts; support/tree/manifest construction; runfiles manifest bytes;
repository mapping bytes; Windows mode; action expansion/execution/aquery;
aspects; and every C++ or other rule-family special case.

## Frozen implementation

Keep the final retained model introduced by `8e7234b82`. Replace the remaining
legacy string-backed `Runfiles`/`RunfilesBuilder` owner rather than retaining a
compatibility representation. `DefaultInfo.default_runfiles` and
`data_runfiles` become `RetainedRunfiles`.

`RunfilesSymlink` gains a private occurrence token so dense-depset leaf
deduplication matches Bazel's identity-sensitive `SymlinkEntry` behavior; its
publication comparator ignores that temporary occurrence and compares path and
Artifact while preserving dense successor topology. All retained types remain
`Allocative`; `Arc`, `CompactString`, `SmallMap`, dense `Depset`,
`AnalysisDepset`, and `Dupe` are reused. Add no interner, global cache, second
graph, or evaluator-owned retained value.

`ctx.runfiles` is a root rule-context method with the Bazel 9.2 signature:

```text
files = []
transitive_files = None
collect_data = False
collect_default = False
symlinks = {}
root_symlinks = {}
skip_conflict_checking = False
```

Direct Files become direct runfiles leaves; `transitive_files` remains a
transitive child and must have compatible default/compile order. Symlink
dictionaries become direct identity-distinct entries. Symlink depsets are
imported iteratively/topology-preservingly with an explicit-stack memoized DAG
traversal into the typed dense depset; do not
flatten them. Any nonempty explicit symlink source raises conflict policy from
Warn to Error. Validate admitted paths before retaining them. True collection
or bypass flags fail in the method before a runfiles value is returned.

One dedicated nonconstructible `StarlarkRunfiles` retains
`RetainedRunfiles`. Its fields are typed depsets: `files`, `symlinks`,
`root_symlinks`, and `empty_filenames`. Symlink leaves are dedicated
`SymlinkEntry` values exposing `path` and `target_file`. `merge` and
`merge_all` return the existing operand when Bazel does for empty or exactly
one nonempty value; otherwise they compose transitive dense nodes in order.
Runfiles objects use heap occurrence identity, separate from retained
publication equality.

The loading-owned raw `StarlarkDefaultInfo` stores all five unevaluated values
only until the implementation returns. The analysis lowering pass validates
and computes:

- omitted `files`: predeclared regular outputs plus executable;
- explicit `files`: exactly that depset, even when it omits executable;
- no runfiles arguments: legacy empty runfiles;
- legacy `runfiles`: reject either stateful argument and insert executable,
  publishing the same effective value as default and data runfiles;
- stateful arguments: missing sides become empty; insert executable only into
  default runfiles for executable/test rules; and
- files-to-run: effective files plus executable, still incomplete whenever an
  executable exists because no support tree is owned.

Materialized configured targets expose the same dedicated runfiles values.
No parser change, DICE key, path lookup, rule-family dispatch, or action schema
is allowed.

## Allowlist, caps, and proof

Production:

- `app/slug_build_api_v2/src/{providers/mod.rs,runfiles.rs,lib.rs}`;
- `app/slug_build_api_v2/src/analysis_value.rs` only for the shared
  bidirectional runfiles-depset publication alias map required by terminal
  review;
- `app/slug_loading_v2/src/provider.rs`; and
- `app/slug_analysis_v2/src/{analysis_value.rs,starlark_rule.rs}`; and
- `app/slug_core_v2/src/runtime/dice.rs` only for the existing bounded run-view
  adapter from legacy string runfiles to typed Artifact runfiles.

Proof:

- `app/slug_build_api_v2/tests/{providers.rs,ctx.rs}`;
- `app/slug_loading_v2/src/provider.rs` colocated binding proof; and
- `app/slug_analysis_v2/tests/starlark_rule.rs`.

Plans may update this manifest, canonical, Stage 6, and Stage 9. Add no crate,
production file, dependency, Artifact/action kind, DICE key, or execution
branch. The bounded cap correction freezes both accounting forms: at most 850
net / 1,050 gross added production Rust, 300 net / 400 gross proof Rust, and
1,150 net / 1,450 gross total Rust. It admits only the dedicated Starlark
wrappers, extracted five-field normalization, and explicit-stack DAG adapters
already required by this packet; it admits no new behavior or file. No
touched production file may newly cross 2,000 lines. New helpers may not cross
150 lines. The pre-existing oversized `evaluate_loaded_rule` may receive only a
bounded call-site replacement with no net growth; all five-field validation
and normalization must be extracted into new helpers below 150 lines. `REPLAN`
before cap excess, support/action implementation,
collection traversal, string compatibility fields, flattened topology, global
state, or scope/ruleset widening.

Focused proof must show:

1. binding defaults, outer-type errors and every deferred flag;
2. direct/transitive files, dictionary/depset symlinks, all four public fields,
   admitted path rejection and conflict-policy propagation;
3. empty, binary and many-way merge order, identity shortcuts, deep-safe dense
   composition, and identity-distinct duplicate SymlinkEntry leaves;
4. all legal/illegal DefaultInfo argument combinations and effective
   default/data runfiles;
5. implicit and explicit executable/default-file topology, predeclared-output
   fallback, typed materialization, and incomplete guard preservation; and
6. provider/runfiles publication equality plus warm A/B/A restoration for a
   changed retained runfiles input.

Run serial focused and full `slug_build_api_v2`, `slug_loading_v2`, and
`slug_analysis_v2` suites plus `cargo check -p slug_core_v2`. Finish with fmt,
metadata, archive status, diff check, caps, physical sizes, independent
terminal review, and parked-file SHA-256 verification.

## Terminal acceptance

Independent terminal review returned `ACCEPT`. The final packet is 837 net /
1,001 gross production lines and 255 net / 286 gross proof lines, for 1,092 net
/ 1,287 gross total. The build-API proof discriminates shared versus
separate-equal grandchildren and cross-field alias partitions. The Starlark
proof imports and rematerializes a genuine depth-3,500 shared diamond with
3,500 distinct SymlinkEntry occurrences. Full serial build-API, loading, and
analysis suites pass, as does the core check. Successor 3 owns support Artifact
construction and atomic support-action registration; Spawn expansion remains
successor 4.
