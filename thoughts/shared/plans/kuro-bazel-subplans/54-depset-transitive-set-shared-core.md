# Plan 54: depset and transitive_set shared core

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Related: [Plan 51](./51-kurod-memory-profiling.md)

## Status: PROPOSED

## Problem

Kuro inherited Buck2's `transitive_set` and has since added a separate
Bazel-compatible `depset` implementation. These occupy the same broad design
space: a cheap-to-merge transitive DAG that can be flattened with deterministic
deduplication. They are not, however, the same public abstraction.

The current split has several costs:

- `depset` has an independent graph representation in
  `app/kuro_build_api/src/interpreter/rule_defs/depset.rs`.
- `TransitiveSet` has a separate representation and traversal machinery in
  `app/kuro_build_api/src/interpreter/rule_defs/transitive_set/`.
- The explicit bridge in
  `app/kuro_interpreter_for_build/src/interpreter/natives.rs` is lossy and
  expensive: `depset -> transitive_set` creates one tset node per direct depset
  item, and `transitive_set -> depset` materializes a flat list.
- Kuro's current depset surface includes non-Bazel behavior (`.order`,
  `.direct`, `.transitive`, `len(depset)`, and `depset | depset`) that should be
  removed or explicitly quarantined for Bazel 9 parity.
- Kuro's current depset ordering is not exact Bazel behavior. In particular,
  `topological` is treated like `postorder`, and omitted/default order is
  inferred from transitive depsets in a way Bazel 9 does not do.

This plan is also the likely structural follow-up to the remaining Plan 51
high-RSS analysis issue. After alias diagnostics, package-listing corruption,
dynamic-cell reference stability, and unbounded diagnostic payloads were ruled
out or fixed, the remaining zeromatter failure shape is genuine memory pressure
during analysis of large bzlmod/toolchain graphs. The leading suspect is large
toolchain/provider depsets being flattened or retained repeatedly instead of
remaining shared DAGs.

The goal is to merge the duplicated graph mechanics without collapsing two
public APIs that intentionally differ.

## Source of truth

### Bazel depset

Use Bazel 9 source and docs as the compatibility source of truth:

- `Depset.java`:
  <https://github.com/bazelbuild/bazel/blob/release-9.0.0/src/main/java/com/google/devtools/build/lib/collect/nestedset/Depset.java>
- `NestedSet.java`:
  <https://github.com/bazelbuild/bazel/blob/release-9.0.0/src/main/java/com/google/devtools/build/lib/collect/nestedset/NestedSet.java>
- `NestedSetBuilder.java`:
  <https://github.com/bazelbuild/bazel/blob/release-9.0.0/src/main/java/com/google/devtools/build/lib/collect/nestedset/NestedSetBuilder.java>
- `Order.java`:
  <https://github.com/bazelbuild/bazel/blob/release-9.0.0/src/main/java/com/google/devtools/build/lib/collect/nestedset/Order.java>
- Builtin API docs:
  <https://bazel.build/rules/lib/builtins/depset>
- Concept docs:
  <https://bazel.build/extending/depsets>

Important Bazel design points:

- `depset` is a Starlark wrapper over `NestedSet`.
- A nested set is an immutable ordered DAG. Direct elements are leaf
  successors, transitive depsets are non-leaf successors.
- Construction is cheap; flattening (`to_list`) is intentionally expensive and
  should be avoided in rule hot paths.
- Element type is tracked without flattening. Empty depsets have no element
  type and can merge with any element type.
- Elements are constrained by Bazel's current hashability/mutability checks and
  by same top-level Starlark type.
- `is_empty` and truthiness are O(1).
- `to_list` suppresses duplicate element values using hash/equality.
- Direct duplicates are eliminated by the nested-set builder.
- Order is selected at construction. Bazel's Starlark orders are:
  `default`, `postorder`, `preorder`, and `topological`.
- `default` is stable/unspecified deterministic order. It is compatible with
  other orders but should remain the constructed order unless Bazel source or
  tests prove otherwise.
- `topological` is not postorder. For a diamond, Bazel's docs show:
  `d = depset(["d"], transitive = [b, c], order = "topological")` flattens to
  `["d", "b", "c", "a"]`.

Black-box check against installed Bazel 9.1.0 on 2026-05-08:

- `hasattr(depset(["x"]), "order") == False`
- `hasattr(depset(["x"]), "direct") == False`
- `hasattr(depset(["x"]), "transitive") == False`
- `len(depset(["x"]))` errors with `want 'iterable or string'`
- `depset(["c"], transitive = [depset(["a"], order = "preorder")])`
  flattened to `["a", "c"]`, meaning omitted order did not inherit preorder
  traversal from the child in that observed Bazel 9.x build.

Before implementation, re-run focused probes against the exact Bazel 9 version
Kuro is targeting if there is any contradiction between docs and source. Prefer
release source over old docs when they disagree.

### Buck2 transitive_set

Use Buck2 docs and Kuro's inherited implementation for intent:

- Buck2 docs:
  <https://buck2.build/docs/rule_authors/transitive_sets/>
- Kuro docs:
  `docs/rule_authors/transitive_sets.md`
- Kuro implementation:
  `app/kuro_build_api/src/interpreter/rule_defs/transitive_set/`

Important transitive_set design points:

- `transitive_set` is nominal. Users first define a set type with
  `transitive_set(...)`, then create instances with `ctx.actions.tset`.
- Each logical tset node has zero or one value and any number of child tsets.
- Values are projected eagerly at node creation into args/json projection
  values. Reductions are also computed eagerly.
- Projection objects are cheap to create and are lazily expanded later.
- Action input discovery can keep a transitive set projection as a shared graph
  edge via `ArtifactGroup::TransitiveSetProjection`, avoiding one action input
  edge per flattened artifact.
- Tset traversal skips already visited nodes by node identity. It does not
  promise Bazel's value-level duplicate suppression.
- Tset order is selected at use site (`traverse`, `project_as_args`,
  `project_as_json`), not at set construction.
- Tsets support traversal orders beyond Bazel depset: `bfs` and `dfs`.

### Current Kuro implementation

Relevant current files:

- `app/kuro_build_api/src/interpreter/rule_defs/depset.rs`
- `app/kuro_build_api/src/interpreter/rule_defs/transitive_set/transitive_set.rs`
- `app/kuro_build_api/src/interpreter/rule_defs/transitive_set/transitive_set_iterator.rs`
- `app/kuro_build_api/src/interpreter/rule_defs/transitive_set/traversal.rs`
- `app/kuro_interpreter_for_build/src/interpreter/natives.rs`
- `app/kuro_build_api/src/interpreter/rule_defs/provider/builtin/default_info.rs`
- `app/kuro_build_api/src/interpreter/rule_defs/cmd_args/typ.rs`
- `app/kuro_action_impl/src/context/runfiles_tree.rs`
- `app/kuro_util/src/memory_checkpoint.rs`
- `scripts/memory_smoke.sh`

Current local behavior that needs attention:

- `Depset` stores `direct: Vec<FrozenValue>`, `children: Vec<FrozenValue>`,
  and `order: String`.
- `LiveDepsetGen<V>` stores `direct` and `transitive` as list values plus an
  order string.
- `DepsetWithListGen<V>` exists only to wrap `DefaultInfo.files` when values
  are not frozen.
- `collect_depset_elements` silently ignores non-depsets in some call paths.
- `to_list` dedupes by value equality, but type/hashability validation is
  incomplete.
- `length()` is implemented for depsets, which is not Bazel-compatible.
- `has_attr/get_attr` expose `direct`, `transitive`, and `order`, which are not
  Bazel-compatible Starlark members.
- `bit_or` implements `depset | depset`, which is not Bazel-compatible.
- `validate_depset_order` infers order from non-default children for a default
  parent; this does not match the observed Bazel 9.x behavior above.
- `topological` in depset collection is treated like `postorder`.

Plan 51 added `KURO_MEMORY_CHECKPOINTS`-gated depset flattening checkpoints
around current `depset.to_list()` paths:

- `depset_to_list_frozen`
- `depset_to_list_live`

These record root direct/transitive counts, collected element count before
dedupe, deduped element count, duplicate count, RSS, and max RSS. Use them to
confirm which depsets are flattening during the zeromatter repro before making
large representation changes.

## Decision

Do not make Bazel `depset` a public alias for Buck/Kuro `TransitiveSet`.

Instead, create a shared nested-DAG engine and keep separate public facades:

- `depset`: Bazel-compatible Starlark value over the shared core.
- `TransitiveSet`: Buck-compatible action/projection/reduction value over the
  shared core or over shared traversal/building primitives.

This is the best tradeoff because:

- Bazel `depset` and Buck `transitive_set` have different semantic boundaries.
- A raw alias would either expose Buck-only projection APIs on Bazel depsets or
  remove the execution-facing benefits that make tsets useful.
- Shared graph mechanics solve the real duplication while letting each facade
  enforce its own invariants.
- The shared core can fix depset topological behavior by reusing or generalizing
  the existing tset topological iterator.
- The shared core creates a path to deferred depset command-line/action-input
  expansion instead of repeated analysis-time flattening.

## Non-goals

- Do not preserve Kuro's previous prototype depset surface if it conflicts with
  Bazel 9 parity.
- Do not support Bazel 8.x or legacy depset names beyond what Bazel 9 accepts.
- Do not make tset projections/reductions part of Bazel depset.
- Do not silently coerce arbitrary `TransitiveSet` values to depsets or depsets
  to arbitrary tset definitions.
- Do not flatten as the primary implementation strategy except at explicit API
  boundaries such as `depset.to_list()`.

## Target architecture

### Layer 1: nested DAG core

Introduce an internal module, likely under
`app/kuro_build_api/src/interpreter/rule_defs/nested_set/`, with shared order,
builder, and traversal mechanics.

The exact Rust representation should be chosen after prototyping with
Starlark's `Trace`, `Freeze`, `Coerce`, and lifetime requirements. The
preferred shape is:

```rust
enum NestedSetOrder {
    Default,
    Postorder,
    Preorder,
    Topological,
}

struct NestedSetGen<V> {
    order: NestedSetOrder,
    direct: Box<[V]>,
    children: Box<[V]>, // values pointing to same facade/core node type
    approx_depth: u32,
    is_empty: bool,
    // Optional flatten cache for frozen/transitive-heavy nodes.
}
```

If a single concrete storage type is too invasive because depset and tset
children point at different Starlark wrapper types, use a shared builder and
generic traversal framework first:

- a shared `NestedSetOrder`;
- a shared direct/transitive construction algorithm;
- a shared traversal trait for "node with direct values and child nodes";
- a shared value-deduping flatten implementation for depset;
- a shared node-deduping traversal implementation for tset.

That fallback still eliminates semantic divergence while keeping the option for
physical storage unification later.

### Layer 2: Bazel depset facade

Replace `Depset`, `LiveDepsetGen`, and `DepsetWithListGen` with a single
live/frozen `DepsetGen<V>` over the nested core.

Required facade behavior:

- Starlark type name is exactly `depset`.
- Constructor signature matches Bazel 9:
  `depset(direct = None, order = "default", *, transitive = None)`.
- Positional direct argument remains accepted if Bazel 9 accepts it.
- Public Starlark methods include `to_list`.
- Do not expose `.direct`, `.transitive`, `.order`, `len`, or `|` unless a
  deliberate non-Bazel diagnostic hook is hidden from normal Bazel Starlark.
- Truthiness uses O(1) emptiness.
- `to_list` performs a traversal based on the depset's construction order and
  suppresses duplicate element values.
- Flattening returns a copy.
- Debug/introspection can expose direct/transitive internally for Rust callers,
  but not as Starlark attributes.
- `DefaultInfo.files`, runfiles, coverage, cc providers, and action APIs should
  all consume this single depset type.

Validation requirements:

- `transitive` must be a sequence of depsets.
- Direct and transitive elements must have compatible top-level Starlark type.
- Empty depsets do not constrain element type.
- Direct elements must obey Bazel 9 hashability/mutability checks. Match Bazel
  9 source and black-box tests exactly; do not implement a future Bazel TODO
  stricter than the target version.
- Direct list/dict elements must be rejected if Bazel 9 rejects them.
- Order compatibility follows Bazel 9 `Order.isCompatible`.
- Do not infer parent order from children unless source/tests prove Bazel 9
  does so. Current evidence says the parent order remains the requested order,
  including `default`.
- Enforce Bazel's nested set depth limit if Kuro has an equivalent semantics
  knob. If Kuro does not expose the flag yet, add a TODO with a focused parity
  test and do not leave unbounded pathological recursion.

### Layer 3: Buck/Kuro TransitiveSet facade

Keep `TransitiveSet` as a distinct public type.

Do not change these semantics without a separate user decision:

- users define a nominal type with `transitive_set(...)`;
- instances are created by `ctx.actions.tset`;
- each logical node has zero or one value;
- projections and reductions are tied to the nominal definition;
- projections/reductions are evaluated at node creation;
- projection values are cached per node;
- traversal order is chosen at projection/traversal use site;
- node identity, not value equality, controls deduplication;
- `bfs` and `dfs` remain tset-only orders;
- action input discovery continues to use `ArtifactGroup::TransitiveSetProjection`.

What can be made more depset-like:

- Use the same iterative traversal algorithms for preorder, postorder, and
  topological where semantics match.
- Use a shared graph-node interface so traversal logic is tested once.
- Consider shared depth accounting and cycle/pathological-depth checks.
- Consider shared flatten caching for `list(tset.traverse(...))` only if it
  does not interfere with lazy projection expansion or memory behavior.

What should not be made depset-like:

- Do not add same-Starlark-type element restrictions to tsets. Tset definitions
  and projection functions are the type boundary.
- Do not dedupe tset values by equality. Distinct nodes with equal values can
  have distinct projections/reduction context and should remain distinct unless
  they are the same visited node.
- Do not move tset order to construction time. Use-site order is a useful Buck
  divergence.
- Do not support multiple direct values per public tset node unless projections
  and reductions are redesigned. A tset node value maps to exactly one set of
  projections and one reduction input.

## Detailed migration phases

### Phase 0: parity characterization

Add focused tests before refactoring. These tests should fail against current
Kuro where behavior is wrong.

Before changing representation, run at least one Plan 51 zeromatter repro with:

```sh
KURO_MEMORY_CHECKPOINTS=1 scripts/memory_smoke.sh \
  --include-pgrep '<zeromatter kurod pgrep pattern>' \
  -- target/debug/kuro --isolation-dir <name> build //sdk:sdk_contents
```

Capture all `depset_to_list_frozen` and `depset_to_list_live` lines. This gives
the baseline for whether the shared-core work needs to prioritize lazy
command-line/action expansion, `to_list()` caching, or provider construction
dedupe. If these checkpoints do not fire near the high-RSS phase, the memory
root cause is probably adjacent provider/toolchain retention rather than
flattening itself.

Depset constructor and surface:

- `type(depset()) == "depset"`.
- `depset().to_list() == []`.
- `bool(depset()) == False`; `bool(depset(["x"])) == True`.
- `hasattr(d, "order")`, `hasattr(d, "direct")`, and
  `hasattr(d, "transitive")` are false.
- `len(depset(["x"]))` errors like Bazel.
- `depset(["x"]) | depset(["y"])` errors like Bazel.
- `depset(transitive = None)` behavior matches Bazel 9.
- `depset(direct = None)` behavior matches Bazel 9.

Depset validation:

- Transitive elements must be depsets.
- Mixed direct element types fail.
- Direct type and non-empty transitive depset type mismatch fails.
- Empty transitive depsets do not constrain type.
- List/dict direct elements fail if Bazel 9 fails.
- Unhashable but frozen values follow exact Bazel 9 behavior.

Depset order:

- Preorder simple tree.
- Postorder simple tree.
- Topological diamond:
  `d -> b -> a` and `d -> c -> a` should match Bazel's `["d", "b", "c", "a"]`
  for the documented construction.
- Default simple tree matches a Bazel 9 probe, not Kuro's current preorder
  assumption.
- Explicit non-default parent plus incompatible child order fails.
- Default parent plus non-default child stays default if Bazel source/probe says
  so.

Depset freezing/providers:

- Depsets created in one rule and read in another preserve behavior.
- Depsets exported from loaded `.bzl` files freeze and thaw correctly.
- `DefaultInfo.files.to_list()` works for frozen and live outputs.
- `runfiles.files` works through `ctx.runfiles(transitive_files = depset(...))`.

TransitiveSet regression:

- Existing `transitive_set` tests continue to pass.
- Projection input discovery still emits `ArtifactGroup::TransitiveSetProjection`.
- `project_as_args` and `project_as_json` remain lazy from action perspective.
- Tset topological, bfs, dfs examples in docs still match.

### Phase 1: introduce shared order and traversal tests

Add `NestedSetOrder` with Bazel names:

- `default`
- `postorder`
- `preorder`
- `topological`

Keep `TransitiveSetOrdering` as a separate public enum for now, but map its
common variants to shared traversal implementations. Tset-only variants remain
`bfs` and `dfs`.

Move traversal algorithms into a shared module with two dedupe strategies:

- `Dedup::NodeIdentity` for tsets.
- `Dedup::ValueHashEq` for depsets.

This phase should be behavior-preserving for tsets and should allow a depset
prototype to call the same preorder/postorder/topological traversal code.

### Phase 2: replace depset internals

Create `DepsetGen<V>` and remove the separate frozen-only `Depset` shape once
live/frozen handling is proven.

Implementation notes:

- Derive or implement `Trace`, `Freeze`, `Coerce`, `Allocative`,
  `ProvidesStaticType`, and `NoSerialize` as needed.
- Store direct elements as `Box<[V]>`, not a list `Value`, to avoid repeatedly
  reinterpreting list values.
- Store transitive children as `Box<[V]>` pointing to depset values.
- Store `NestedSetOrder` rather than `String`.
- Store element type metadata in the depset facade, not in the generic core if
  the core is also used by tsets.
- Store `is_empty` and `approx_depth` at construction.
- Optionally cache flattened frozen results behind a weak/cache mechanism if
  current memory model supports it. Do not cache live `Value<'v>` flattening
  across mutability boundaries.

Update constructors:

- Replace `make_depset_from_lists` with a builder that validates type, order,
  hashability, and depth.
- Replace direct calls to `Depset::empty()` with the new empty depset builder.
- Replace `Depset::from_frozen_values` with the same builder over frozen values.
- Remove `DepsetWithListGen`; a live depset should handle non-frozen default
  outputs directly.

Update methods:

- Keep only `to_list` in Starlark methods.
- Remove `length`.
- Remove `has_attr/get_attr` for `direct`, `transitive`, `order`.
- Remove `bit_or`.
- Keep Rust-only accessors for direct/transitive only if bridge/internal code
  still needs them.

### Phase 3: fix depset consumers

Replace ad hoc collection APIs with typed helpers:

- `depset_to_list(value, heap) -> Result<Vec<Value>>`
- `depset_to_artifact_inputs(value, heap) -> Result<Vec<Value>>`
- `depset_direct_and_transitive` only as an internal debug/bridge helper, not
  as a Starlark-visible behavior dependency.

Update call sites:

- `cmd_args.add_all` and `add_joined` in
  `app/kuro_build_api/src/interpreter/rule_defs/cmd_args/typ.rs`.
- runfiles tree synthesis in
  `app/kuro_action_impl/src/context/runfiles_tree.rs`.
- cc_common actions/providers.
- coverage_common.
- java_common.
- DefaultInfo and Runfiles in
  `app/kuro_build_api/src/interpreter/rule_defs/provider/builtin/default_info.rs`.
- any `request_value::<Depset>()` code that assumes the old concrete type.

Prefer returning errors for wrong-type values rather than silently returning an
empty collection.

### Phase 4: preserve and simplify TransitiveSet

Refactor `TransitiveSetGen<V>` to use the shared traversal/building
implementation where possible.

Possible approaches:

1. Minimal: keep current storage, replace iterator internals with shared
   traversal traits.
2. Medium: replace `children: Box<[V]>` and `node: Option<NodeGen<V>>` traversal
   access with a shared `NestedGraphNode` adapter.
3. Full: store graph structure in a reusable core and keep `TransitiveSetGen`
   metadata beside it.

Start with minimal or medium. Full physical storage unification should happen
only if it reduces complexity after the depset facade is correct.

Do not refactor projections/reductions during this phase except to adjust them
to the shared traversal adapter.

### Phase 5: bridge semantics

Revisit `native.transitive_set_from_depset` and
`native.depset_from_transitive_set`.

Desired end state:

- These helpers are unnecessary for normal Bazel compatibility.
- If kept for Kuro-specific internals, they should be explicit and documented as
  lossy where semantics cannot be preserved.
- `depset -> transitive_set` should preserve graph shape when using a built-in
  `BazelDepsetTset` definition, not materialize one node per direct element
  unless no better representation is possible.
- `transitive_set -> depset` necessarily loses projections, reductions,
  definition identity, and tset node-identity semantics. It may materialize a
  list and build a depset from values.
- Add caching only after semantic correctness, and only at stable/frozen
  boundaries where cache lifetime is clear.

If public Bazel mode should not expose these helpers, move them behind a Kuro
internal namespace or keep them only in native internals.

### Phase 6: deferred depset action expansion

After depset storage is correct, reduce analysis-time flattening:

- Teach `ctx.actions.args().add_all(depset)` and `add_joined(depset)` to carry a
  lazy depset command-line item where possible.
- Add artifact visitation for depsets of files without flattening everything
  during analysis if a shared artifact group can represent the depset.
- Consider adding an `ArtifactGroup::Depset` or similar if a depset of artifacts
  can be represented safely through execution.
- Keep `ArtifactGroup::TransitiveSetProjection` for tset projections. Do not
  force depsets through tset projections unless that preserves Bazel semantics.

This phase is performance work and should not block the semantic cleanup unless
Plan 51 checkpoints show that analysis-time depset flattening is the dominant
zeromatter RSS driver. In that case, pull this phase forward immediately after the
depset facade is semantically correct.

### Phase 7: remove obsolete code and docs

Once the new depset facade and shared traversal are stable:

- Delete `DepsetWithListGen`.
- Delete old fallback code that treats any value with `get_type() == "depset"`
  as depset-like by scraping `.direct` and `.transitive`.
- Delete Kuro-only depset tests for `.order`, `len`, and `|`, or rewrite them
  as negative parity tests.
- Update the bridge section in the parent plan, which currently describes a
  best-effort conversion strategy, to point to this shared-core plan.
- Update user docs only if `depset` behavior is documented anywhere outside
  Bazel parity docs.

## Technical tradeoffs

### Why not make depset a raw TransitiveSet alias?

Raw aliasing is simpler but wrong:

- `TransitiveSet` has nominal definitions; `depset` is anonymous/generic.
- `TransitiveSet` requires `ctx.actions.tset` and a deferred key; `depset()` is
  a plain Starlark constructor usable wherever Bazel allows it.
- `TransitiveSet` has projections/reductions; `depset` does not.
- `TransitiveSet` traversal dedupes nodes; `depset.to_list` dedupes values.
- `TransitiveSet` order is selected at use site; `depset` order is fixed at
  construction.
- `TransitiveSet` permits `bfs`/`dfs`; Bazel depset does not.

### Why not make TransitiveSet more like depset?

Some internals can be shared, but the public differences are valuable:

- Tset nominal definitions make projections and reductions type-directed.
- Eager projection/reduction validation catches errors at tset construction.
- Action input projection keys allow execution to preserve shared graph edges.
- Use-site ordering lets the same tset support different projection consumers.
- Node-deduping is the correct unit for projections; value-deduping would change
  behavior when two nodes contain equal values.

Changing these would reduce the technical value of tsets and would not improve
Bazel compatibility, because Bazel rules should use `depset` at their API
boundary.

### Why a shared nested-DAG core is better

Shared core gives most of the benefit:

- one order parser/enum for Bazel orders;
- one tested topological traversal implementation;
- one depth/emptiness accounting model;
- one place to optimize flattening and direct duplicate removal;
- one path to deferred action expansion for depsets;
- separate facades for separate invariants.

## Risks

- Starlark lifetime and GC constraints may make a single physical storage type
  awkward. Mitigation: start by sharing traversal/building traits and only move
  to physical storage unification if it reduces complexity.
- Bazel docs and source occasionally disagree around default order wording.
  Mitigation: use Bazel 9 release source plus focused black-box probes.
- Tightening depset parity will break existing Kuro prototype tests or local
  rules relying on `.order`, `len`, or `|`. This is acceptable under Bazel 9
  parity policy.
- Deferring depset expansion into action execution may require new artifact
  group plumbing. Mitigation: treat that as Phase 6 after semantic parity.
- Same-type/hashability validation can be subtle with Starlark Rust values.
  Mitigation: create a dedicated element-type helper with tests for strings,
  ints, artifacts/files, providers, structs, lists, dicts, tuples, and frozen
  values.

## Verification matrix

Unit tests:

- depset constructor validation;
- depset order traversal;
- depset topological diamonds;
- depset value deduplication;
- depset freeze/live behavior;
- tset traversal regression;
- tset projection/reduction regression.

Integration tests:

- `tests/core/analysis/test_depset_order.py`, rewritten for Bazel parity;
- `tests/core/analysis/test_runfiles.py`;
- `tests/core/analysis/test_providers.py`;
- `tests/core/transitive_sets/test_transitive_sets.py`;
- rules_cc fixtures that pass compilation/include depsets through providers;
- rules_python fixtures that pass runfiles depsets through `DefaultInfo`.

Black-box Bazel checks:

- Generate tiny temporary Bazel workspaces for every behavior where source is
  ambiguous.
- Record exact Bazel version used in the test comment or plan progress note.
- Do not accept Kuro behavior based only on old Kuro tests.

Memory checks:

- With `KURO_MEMORY_CHECKPOINTS=1`, compare `depset_to_list_*` counts before
  and after the migration.
- The number and size of large `depset.to_list()` expansions during the zeromatter
  repro should either drop materially or be explained by user-visible Starlark
  calls that Bazel would also flatten.
- If deferred depset expansion is implemented, actions should not need to
  flatten depsets of files during analysis solely to discover inputs.

Commands for implementation PRs:

- `cargo test -p kuro_build_api_tests transitive_set`
- `cargo test -p kuro_build_api_tests depset` if a focused test module exists
  or is added.
- Relevant e2e tests under `tests/core/analysis`.
- Representative rules_cc/rules_python builds that exercise provider depsets
  and runfiles.

## Proposed implementation order

1. Add parity tests and mark current failures.
2. Add shared `NestedSetOrder` and traversal module.
3. Reimplement depset on the shared machinery.
4. Update depset consumers and remove `DepsetWithListGen`.
5. Refactor tset traversal to use shared machinery while preserving public tset
   behavior.
6. Replace or retire lossy bridge helpers.
7. Add deferred depset action expansion if benchmarks show analysis-time
   flattening remains material.
8. Update parent plan and docs.

## Definition of done

- Bazel-facing `depset` matches Bazel 9 behavior for constructor, public
  surface, validation, truthiness, and `to_list` order.
- Kuro no longer has three independent depset wrapper shapes.
- Tset projection/reduction behavior is unchanged.
- Shared traversal code is used for common preorder/postorder/topological
  behavior or there is a documented reason why a specific path remains separate.
- `depset -> transitive_set` and `transitive_set -> depset` are either removed
  from public surface or documented as explicit, lossy conversions with tests.
- No action-input or runfiles path relies on silent "unknown depset-like value"
  fallbacks.
