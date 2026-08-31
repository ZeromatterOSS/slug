# Current Slug V2 Packet

Packet: `WP-6-7A-dense-retained-depset-action-import-implementation-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 retained-depset gate.

Status: Terminal implementation rereview `ACCEPT`. This packet is complete.

Base: `c702cafb8`, which records architecture `ACCEPT` for the dense retained
depset/action-import design. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked;
do not edit or stage it.

## Observable result

Replace the current retained per-node `Arc` depset scaffold with one compact,
immutable, Bazel-specialized dense store. Preserve Bazel order, validation,
leaf equality, node occurrence identity, local alias topology, diamonds and
cross-owner sharing. Add a typed File/action-input import view over that same
retained topology so an action consumer can collect authenticated
`AnalysisArtifact` inputs without first publishing a flat Starlark list.

This is the prerequisite named by the Stage 6 and extraction-ledger gates
before a broad ruleset consumer uses nonempty transitive depsets. Authentic
rules_cc 0.2.17 is the discriminator: its FDO subrule forwards `all_files`
through `tools=[all_files]`. It is not a `cc_common`, `cc_internal`, parser,
`set`, action-builtin, rules_cc, aquery, executor, REAPI, or ActionKey packet.
The next packet, after acceptance of this representation, is the generic
non-callback `Args`/`run`/artifact-symlink builtin category.

## Authority and learned facts

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole
semantic authority. Its depset/nested-set tests and source determine public
construction validation, order compatibility, traversal, duplicate handling,
depth and error behavior. Existing `depset-orders-and-rejections` oracle
evidence remains authoritative where it discriminates the same behavior.

starlark-rust from Buck2 remains Slug's parser, binder, evaluator, heap, `set`
implementation and public `depset` call surface. This packet changes only
Slug-owned retained analysis values behind that facade. It adds no parser,
alternate binder, custom `set`, or Buck2 `transitive_set` API.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer design and
optimization guidance, not semantic authority or a source of truth. Its dense
retained rows, distinct generic/File paths, external producer references,
late materialization and direct action import motivate the architecture. Copy
no Zig code, names, layout, diagnostics, tests, fingerprints or behavior.

Buck2-derived utility guidance governs the Rust substrate: preserve cheap
`Arc`/`Dupe` ownership, compact immutable slices, `Allocative` accounting and
`FxHashMap`/`FxHashSet` phase scratch. Add no global interner or retained
scratch cache.

## Compatibility boundary

**Exact:** all four Bazel depset orders; default/order compatibility; empty and
singleton behavior; construction normalization and validation precedence;
duplicate direct leaves; equal leaves in distinct nodes; repeated child
aliases; diamonds; multi-child depth and the configured maximum-depth failure;
stack-safe traversal at the supported limit; topological alias result
`[a, b, c, b] -> [a, c, b]`; cold and repeated `to_list()`; provider
freeze/materialization; cross-owner forwarding; structural publication
equality that distinguishes alias partitions; and the ordered unique File
sequence observed by an action-input consumer.

**Slug-native:** Rust store and index layout, allocation boundaries, builder
scratch structures, structural-equality implementation, lifecycle mechanics,
measurement counters, invocation/evaluator-local caches, and direct typed
File/action-input import without a public flattened intermediate.

**Unsupported/deferred:** the internal bytes or fingerprints of Bazel's Java
`NestedSet`; exact Bazel configuration/output identity; Bazel ActionKey; REAPI
digests; aquery and execution for new action kinds; directory/tree-artifact
input expansion; `Args.map_each`, callbacks, param files and command-line
fingerprints; public Buck2 transitive sets, projections, reductions, BFS/DFS or
implicit coercion; and the generic action builtins selected as the successor.

## Retained architecture

### One immutable store and checked handles

Use one `Arc<DenseDepsetStore<T, M>>` per constructed local graph. The store
owns compact immutable slices for nodes, ordered successor entries, leaves,
external depset handles, external canonical-row handles and per-node metadata.
A public retained `Depset<T, M>` handle is a cheap duplicate of that store plus
a checked `u32` root node id. Index conversion is checked at construction;
overflow fails closed. No semantic operation relies on pointer addresses or
allocation order.

Each node contains one canonical **successor-row reference**: either a local
range or an external store plus range. Construction resolves an external-row
chain to its underlying owner, so dereference wrappers do not grow traversal
chains. This preserves Bazel's order-only single-child wrapper and the existing
`shares_successors_with` fact without copying the row or inserting another
semantic successor node. Every entry in the referenced ordered row is tagged
exactly one of:

- `Leaf(leaf_index)`, indexing the row owner's compact leaf slice;
- `Local(node_id)`, indexing another node in the row-owning store; or
- `External(external_index)`, indexing a compact side table of retained depset
  handles.

This single tagged index row is required: separate leaf and edge ranges must
not lose the declaration-time interleaving used by Bazel traversal. Leaf,
local-node and external indexes in a row are interpreted against that row's
owning store. Node metadata is stored by local node id. A handle's occurrence
identity is `(store, root)` for topology/lifecycle operations only; semantic or
publication equality never uses pointer identity as its answer. Arc-bearing
external references live once in side tables rather than widening every row
entry.

`shares_successors_with` means the two handles resolve to the same canonical
row owner and range, including across an order-only wrapper store. Structural
equality remains a separate operation. Local aliases point to one local node
id; external aliases retain one external handle. Equal leaf values share
traversal equality but do not collapse distinct node occurrences or alias
partitions. External row and successor references may point only to an already
frozen store, so construction cannot form retained cycles.

### Phase-scratch construction and lowering

A phase-scratch builder memoizes each source node occurrence into one local id,
emits deterministic ordered successor rows, and freezes all vectors to compact
immutable slices once. Construction is iterative where depth can be
user-shaped. It does not recursively copy transitive children. Dependency- or
provider-owned depsets enter as `External` handles rather than being cloned
into the new store. An order-only dereference wrapper retains the child's
canonical external row reference rather than copying its entries.

Keep the existing starlark-rust `StarlarkDepset` facade. When analysis lowers a
locally constructed Starlark DAG, lower the whole local occurrence graph once
into one `AnalysisDepset` dense store. A transitive depset already materialized
by a dependency is an external handle. Retain no evaluator `Value`, heap,
evaluator pointer or call token in `AnalysisDepset`, actions, DICE keys or
configured results.

Construction validation stays at the existing public boundary and preserves
Bazel's error precedence. The dense builder receives only already-validated,
typed order/element facts; representation work must not move a failure from
construction to consumption.

### Traversal, equality and caches

Traverse iteratively using `FxHashSet`/`FxHashMap` scratch for visited node
occurrences and leaf equality. Preserve the existing four-order algorithms and
topological alias semantics. A consumer chooses no order: it receives the
declared Bazel order retained by the root. Repeated consumption may use only a
bounded invocation- or evaluator-local projection cache. No flattened vector,
visited set or fingerprint is stored in the retained graph, a DICE key or
process-global state.

`AnalysisDepset` occurrence equality remains distinct from structural
publication equality. Publication equality recursively compares metadata,
ordered tagged rows, leaf values and external subgraphs while memoizing node
pairs. It must preserve the alias partition: two occurrences referencing the
same child are not equal to two separately constructed equal children merely
because their flattened lists match. The comparison is stack-safe and handles
diamonds without exponential revisits.

### Typed File/action-input view

Add a narrow retained File-set view backed by the same dense core. It
authenticates each leaf as an `AnalysisArtifact` and feeds ordered unique
artifacts directly into a synthetic action-input sink through topology-aware
traversal. It exposes neither raw store indexes nor a public flattened vector.
Generic `Depset<T, M>` remains the semantic owner; the File view is a typed
consumer adapter, not a second retained representation or cache.

The seam may touch `actions/spec.rs` only to define the typed input-set import
boundary. This packet stops at a synthetic sink and does not register or
execute `run`, `symlink`, or another new action kind. The following generic
action-builtin packet consumes this seam for both `inputs` and `tools`, then
extends it category-wise rather than adding a C++-specific branch.

## Evidence contract

1. Preserve every existing generic depset, analysis-value and provider test
   with the same observable meaning; reuse existing Bazel 9.2 oracle evidence.
2. Prove one local store for a multi-node construction, same-store local alias
   reuse, cheap external-store sharing without recursive copying, and
   cross-store order-only wrappers sharing the same canonical successor row.
   The direct evaluator-lowering proof must start with one locally constructed
   Starlark diamond plus one already-retained dependency child and show one
   local store, preserved local aliases, an external retained handle, and no
   evaluator value reachable from the retained result.
3. Prove occurrence identity and publication equality separately, including
   equal lists with different alias partitions and equal distinct leaves.
4. Prove all orders, mixed/default compatibility, empties, duplicates,
   diamonds, the topological `[a, b, c, b] -> [a, c, b]` result, exact
   construction errors, multi-child depth and supported-limit stack safety.
5. Prove Starlark provider materialization and direct typed action-input import
   produce the exact ordered unique File sequence without calling a public
   `to_list()` seam.
6. Prove dropping every root releases local leaves and external owners; no
   evaluator value or scratch cache survives the owning phase.
7. Add `Allocative` coverage and deterministic test counters. Against a
   test-only legacy-Arc control on chain, diamond and authentic-shaped
   rules_cc/rules_rust fan-in graphs, retained bytes and allocation objects
   must decrease. Construction, cold traversal and warm traversal operation
   counts must each stay within 10% of the control. Wall-clock timing is
   informative only and never a flaky acceptance gate.

The legacy DAG exists only as a test measurement control. It is not a
production fallback, feature flag or compatibility path. Failure to meet a
measurement threshold is `REPLAN`, not permission to retain both production
representations.

## Writable allowlist and caps

Production:

- `app/slug_build_api_v2/src/depset.rs`
- `app/slug_build_api_v2/src/analysis_value.rs`
- `app/slug_build_api_v2/src/actions/mod.rs`
- `app/slug_build_api_v2/src/actions/spec.rs` only for the typed File-set seam
- `app/slug_build_api_v2/src/lib.rs`
- `app/slug_loading_v2/src/provider.rs`
- `app/slug_analysis_v2/src/analysis_value.rs` only for the whole-local-DAG
  lowering owner; base blob `251853de3c9319901e33b54d1d6034bb4e4c9477`,
  1,009 physical lines, at most +220 production/+180 inline proof and 1,409
  physical lines after the packet

Proof:

- existing depset, analysis-value, action and provider test modules in those
  crates;
- inline proof in the admitted `slug_analysis_v2/src/analysis_value.rs` lowering
  owner for the local-diamond/external-child/no-evaluator-retention facts; and
- a test-only legacy control colocated with the depset tests.

Plans:

- the canonical plan;
- Stage 6;
- the Stage 9 extraction ledger; and
- this manifest.

Caps are 1,250 production additions, 1,000 proof additions and 2,250 aggregate
additions, excluding plan text. Add no dependency, DICE key, global cache,
lock, interner, parser or production fallback. If `analysis_value.rs` would
exceed 2,000 physical lines in `slug_build_api_v2`, split one cohesive leaf
within the allowlist and return to architecture review before implementation.
The separate `slug_analysis_v2` lowering owner is governed by its explicit
1,409-line cap above.

## Validation

Run serially:

- focused depset, analysis-value, action and provider tests;
- `cargo test -p slug_build_api_v2`;
- `cargo test -p slug_loading_v2`;
- `cargo test -p slug_analysis_v2`;
- the V2 archive/clean-root checker;
- `cargo fmt --check` and `git diff --check`; and
- staged-only allowlist, cap and unrelated-dirty-file audits.

Add fresh Bazel oracle evidence only for a demonstrated semantic gap. Do not
rerun an already discriminating fixture solely because the representation
changed.

## Review gate

The independent reserved-representation reviewer must answer:

- Does one dense store plus local/external canonical row references and tagged
  ordered rows preserve direct/transitive interleaving, order-only row sharing,
  local aliases, cross-owner sharing and occurrence identity?
- Are semantic/publication equality, leaf deduplication, topology identity and
  flattening kept as distinct domains?
- Can Starlark lowering complete without retaining evaluator objects or
  recursively copying external graphs?
- Is the typed File/action-input adapter genuinely backed by the same retained
  owner and free of an implicit flattened ABI?
- Are lifecycle, `Allocative`, deterministic measurement and cache bounds
  sufficient to reject a representation regression?
- Can the next complete Args/run/symlink category reuse the seam without a
  rules_cc, `cc_common`, `cc_internal`, parser or builtin-specific branch?

Only `ACCEPT` activates Rust work. `REPLAN` is mandatory if implementation
would retain evaluator state, add global/unbounded caches, recursively copy
children, make flattened vectors the action ABI, use pointer identity as
semantic equality, change public error timing/precedence, permit unchecked
indexes, keep a production fallback, add a DICE key, or miss a measurement
threshold.

## Architecture review evidence

The first independent review returned `REPLAN` because the writable allowlist
omitted `slug_analysis_v2/src/analysis_value.rs`, the sole owner capable of
lowering a whole evaluator-local DAG. The correction admitted that exact base
blob with bounded production/proof/physical caps and added a discriminating
local-diamond plus retained-external-child proof.

The independent rereview returned `ACCEPT`. It confirmed that checked local
indexes plus external depset/canonical-row side tables form an acyclic retained
ownership graph; dereference chains resolve to one frozen row owner; local
aliases, cross-owner lifetime and no-copy lowering remain expressible; and the
typed File/action-input adapter adds neither a flattened ABI nor a second
semantic representation. No compatibility or packet scope widened.

## Implementation evidence

The candidate implements checked `u32` node/range/leaf/external indexes, one
Arc-owned immutable dense store, compact tagged successor rows, deduplicated
external depset/canonical-row side tables and canonical cross-store row sharing.
Generic structural equality remains stack-safe and distinct from
`AnalysisDepset` occurrence equality and alias-partition-preserving publication
equality. Existing one-pass `to_list()` traversal remains the cold/warm list
projection; direct action-input streaming uses topology scratch and no leaf
vector. Its topological path keys selected entries by parent node occurrence
plus row offset, including distinct wrappers that share one canonical row.

`AnalysisValueLowerer` now preflights the complete enclosing supported value
and lowers every reachable evaluator-local depset DAG into one store, while
already-retained dependency children remain external handles. The focused
diamond proof shows root/left/right/shared local store identity, shared-child
occurrence identity, one external occurrence and correct values after the
`FrozenHeap` is dropped. A separate child-before-parent tuple proof prevents a
previously memoized local child from being misclassified as external.
`RetainedArtifactInputs` validates the File element category and streams exact
ordered unique `AnalysisArtifact` leaves into a synthetic sink without
creating an `ActionSpec` or flattened action ABI.

The first terminal implementation review returned `REPLAN`: the original
candidate measured only retained shape, allocated a temporary successor
vector while traversing, could classify a child observed before its enclosing
parent as external, and lacked a supported-depth publication-equality proof.
The correction iterates dense rows directly in reverse, counts the actual
construction/cold/warm paths, preflights enclosing values, and makes
publication equality iterative with a 3,500-level regression.

The correction rereview found one remaining nondiscriminating lifecycle test:
its same-order, zero-direct parent reused the child and bypassed both external
side tables. The final proof separately forces an external-successor entry and
an order-only external-row wrapper, verifies retention after the original
child drops, and verifies release after each forwarding owner drops. Terminal
rereview then returned `ACCEPT`; no compatibility class or scope widened.

Deterministic test-only legacy-Arc controls record these 64-bit measurements:

- four-node diamond: dense 4 versus legacy 8 allocation objects, 368 versus
  472 estimated retained bytes; construction 13 versus 13 operations; cold
  and warm traversal each 19 versus 19 operations;
- 64-child authenticated ruleset-shaped fan-in: dense 4 versus legacy 130
  allocation objects, 6,248 versus 8,768 estimated retained bytes;
  construction 257 versus 257 operations; cold and warm traversal each 386
  versus 386 operations; and
- 256-node chain: dense 4 versus legacy 512 allocation objects, 16,488 versus
  28,672 estimated retained bytes; construction 768 versus 768 operations;
  cold and warm traversal each 1,026 versus 1,026 operations.

The fan-in shape is pinned beside the test to the authenticated rules_cc FDO
source SHA-256 `91b7b46c515b4773d5a241e699027212f679ab93160cc79218bd687eac51d5b7`
and rules_rust 0.73.0 archive SHA-256
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Wall-clock results are deliberately not acceptance evidence.

All existing Bazel-order tests retain their meaning. Added proof covers the
exact topological `[a, b, c, b] -> [a, c, b]` alias result, distinct nodes
sharing a canonical row during streaming, and separate lifecycle paths for an
external-successor side-table owner and an order-only external-row wrapper;
each retains its tracked leaf after the source handle drops and releases it
after the forwarding owner drops. Dense/local/external topology, action
artifact type rejection and direct import are also covered. `Allocative`
covers every retained store, node, external-row and public stats type.

Serial correction validation passes `slug_build_api_v2` (49 tests),
`slug_loading_v2` (456 passed/1 realized-source test ignored plus every
integration suite), and `slug_analysis_v2` (102 tests), including the three
dense measurement controls, the 3,500-level publication-equality proof and the
child-before-parent lowering proof. `cargo fmt --check`, working/staged
`git diff --check`, and the archive checker's archive/root invariants pass. The
archive checker has a known pre-existing failure on three tracked V2
authoring/evidence paths that its stale allowlist does not admit; all three are
present at base `c702cafb8`, and this packet does not edit the checker.
Classified packet additions are 1,159 production and 693 proof (1,852
aggregate), within the 1,250/1,000/2,250 caps.
`slug_analysis_v2/src/analysis_value.rs` is 1,263 physical lines versus its
1,409 cap, with 114 production and 167 inline-proof additions versus its
+220/+180 subcaps. `slug_build_api_v2/src/analysis_value.rs` is 1,450 lines
versus its 2,000 split gate. The unrelated registration proof remains
unstaged.
