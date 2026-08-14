# Current Slug V2 Packet

Packet: `WP-2A-m1-root-module-include-progress-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: give the legacy Host root-MODULE producer a finite typed terminal for
direct or indirect active-ancestry include recurrence without collapsing
accepted repeated acyclic occurrences. This packet changes no frontier or
public caller.

## Accepted predecessor and pinned source decision

Commit `8a555daa` records the root-module frontier `REPLAN`: every finite child
frontier is representable, but `HostRootModuleFileKey` can continually refill
its include horizon and never publish a DICE value.

The exact Bazel 9.2.0 source at
`src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileFunction.java`
is discriminating. Its `State` retains a BFS `horizon` and a raw-label keyed
compiled-file map. `execNonRegistryModuleFile` loops while the horizon is
nonempty; `advanceHorizon` compiles every occurrence, overwrites the raw-label
map entry, and appends every compiled child's include statements. It has no
visited, ancestry, recurrence, or nonprogress terminal. Matching
`ModuleFileFunctionTest.testRootModule_include_good` and
`src/test/py/bazel/bzlmod/bazel_module_test.py::{testInclude,
testNonRegistryOverrideModuleInclude}` prove only finite acyclic nesting; the
matching Java/Python bzlmod tests contain no include-cycle case.

Therefore direct and indirect recurrence have no exact Bazel terminal, message,
or source location to copy. A timeout-only oracle would merely reconfirm the
source-proven lack of a result and would not select a finite contract, so this
packet adds no fixture. Bazel nontermination remains unsupported; the finite
Slug safeguard below is explicitly Slug-native. Existing acyclic include
behavior remains an exact regression/non-widening invariant.

## Frozen implementation contract

1. `HostRootModuleFileKey` is the sole semantic and progress owner. Add one
   private `HostRootModuleFileError::IncludeCycle { raw_label, location,
   logical_path }`. The fields identify the back-edge occurrence only; do not
   retain a chain, evaluator, event batch, transaction, child value, or second
   source collection in the completed DICE result.
2. Recurrence identity is the selected normalized logical Host path returned by
   package preflight. Raw spellings that select the same path recur only when
   that path is already on the current occurrence's ancestry. Repeated siblings
   and repeated aliases on distinct ancestry branches remain distinct validated
   and evaluated occurrences with their existing repeated events.
3. Root policy, root bytes, bootstrap Need, and root validation stay first.
   Each horizon still parses/preflights all labels, computes the same grouped
   Host-file batch, unions the same Needs, and selects semantic failures in
   source order. Only after the current occurrence has a complete Present file
   and successful source validation, compare its logical path with its active
   ancestry. A match returns `IncludeCycle` at that include call; otherwise add
   the path to the ancestry inherited by its children and continue unchanged.
4. Represent ancestry as command-local parent-linked immutable `Arc` nodes,
   rooted at the logical root `MODULE.bazel` path. Use constant-time `Dupe`
   pointer clones for sibling inheritance and a linear parent walk for a
   membership check. Do not add a global visited set, DICE key/store, interner,
   retained Arc slice, arbitrary depth limit, or direct filesystem read.
5. A cycle is a normal complete semantic error. Complete pre-evaluation errors
   retain the existing empty parent event-batch behavior when capture is
   enabled. Need/cancellation publishes no parent terminal or parent completed
   batch; parent ancestry and horizon scratch drop, while completed child DICE
   observations remain dependency-owned cache state.
6. Existing complete-only DICE equality/validity includes the new typed error
   structurally. Warm recurrence reuse, cycle-to-acyclic recovery, and A/B/A
   restoration must follow ordinary dependency invalidation without retained
   progress state.

Existing admitted serial acyclic root MODULE/include parsing, validation,
diagnostics, source-order errors, grouped Need behavior, evaluation, event
order, repeated occurrences, and Host observation values remain exact.
Selected-logical-path ancestry, the finite `IncludeCycle` terminal, and its
diagnostic shape are Slug-native. Root-frontier aggregation/sealing remains for
the immediate successor. Lockfile/registry, package source, BUILD/.bzl/glob,
loading/core/public activation, routed/materialized repositories,
overlap/final validation, and exact Bazel identity bytes remain
unsupported/deferred.

## Proof and validation

Add focused colocated proof for:

- a direct self-include and an indirect A -> B -> A recurrence, including an
  alias-spelled back edge, with the exact typed back-edge fields;
- the existing repeated sibling/alias case still validating and evaluating
  every occurrence in the admitted event order;
- grouped Need and earlier source-order semantic failures retaining precedence
  over any later recurrence candidate;
- a complete cycle error owning an empty parent event batch, while Need owns no
  parent completed batch;
- warm equality, cycle/acyclic recovery, and A/B/A restoration; and
- source inspection that every await precedes only drop-safe command scratch
  and that no legacy dependency or public output owner changes.

Run the focused Host root-module tests, the full `slug_bzlmod_v2` suite, direct
`slug_loading_v2` and `slug_core_v2` checks, `cargo fmt --all -- --check`,
strict Clippy with inherited-baseline disposition, the V2 archive checker, and
`git diff --check`. Do not run Cargo commands concurrently in one target
directory.

Because `host_module.rs` is already 2,919 physical lines, require independent
pre- and post-implementation cohesion/AI-cleanup review. Keep the recurrence
owner adjacent to the existing Host root-module key unless review proves a
real separable responsibility; a split is not authorized implicitly.

## Authority and caps

Write only:

- `app/slug_bzlmod_v2/src/host_module.rs`; and
- at completion only,
  `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`,
  `slug-v2-subplans/current-packet.md`, and
  `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Read only the active packet and owner section, the plan-authoring guide,
`docs/developers/dice.md`, the Buck2 utility-reuse skill and matching Stage 9
Arc/`Dupe`/`Allocative` row, local Bzlmod
`src/{host_module,host_include,module_eval,host_file,host_package,lib}.rs`,
loading `src/bzl_module.rs`, their manifests and directly referenced focused
tests, and the exact Bazel 9.2.0 source/tests named above.

Rust growth is capped at 130 net production lines, 240 in-module test lines,
370 total net lines, and 3,289 physical lines in `host_module.rs`, with no cap
correction. Completion ledgers are capped at 180 net lines.

## STOP / REPLAN

STOP on every other Rust file; another key, cache, graph, store, interner,
evaluator, retained frontier, public API/output, Cargo/dependency, oracle,
fixture, loading/core caller, lockfile/registry/package-source/BUILD/.bzl/glob,
routed/materialized repository, watcher, JVM, or unrelated cleanup change.

REPLAN if selected logical-path identity is unavailable after the existing
preflight, recurrence cannot be detected after validation without changing
acyclic error/Need/event order, finite progress requires state outside the
command-local Host producer, the typed error changes a legacy public surface
beyond the declared Slug-native cycle case, another file is required, or any
cap/ceiling is exceeded.

## Immediate successor

On acceptance, resume docs-only `WP-2A-m1-root-module-frontier-design` using the
finite legacy terminal. Do not combine observed-frontier implementation with
this behavior correction.
