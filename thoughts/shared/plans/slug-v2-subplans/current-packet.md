# Current Slug V2 Packet

Packet: `WP-4-5-6-7A-selected-registry-extension-bzl-source-observable-scope-correction-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: `04-starlark-loading-and-build-packages.md`,
`05-bzlmod-and-repository-graph.md`, and
`06-analysis-toolchains-and-actions.md`
Base: stopped implementation contract `dc85f527`

Result: correct the selected-registry extension source owner's observable and
proof boundary before any Rust implementation.

## Accepted facts

The bootstrap frontier is still the selected-registry extension definition
source association. Root
`use_extension("@rules_rust//rust:extensions.bzl", "rust")` is rejected before
loading; the root-only definition consumers cannot read it; and the existing
external Bzl evaluator rejects rules_rust's same-repository cross-package and
mapped-repository loads.

The ownership decision in `dc85f527` remains sound. Bzlmod's retained
`HostCanonicalSelectedModuleDefinition` is the natural source/repository-view
producer, and loading is the natural source-byte/recursive-evaluation owner.
The six-file authority, distinct selected source polarity, structural identity,
DICE order, event ownership, retained-state and lifecycle design require no
architecture change.

The stopped packet made one invalid observable claim. The actual rules_rust
0.73 source evaluates top-level
`repository_rule(doc = ..., implementation = ...)` before exporting its module
extension. Slug rejects nonempty repository-rule `doc`; later transitive source
also declares collection-valued repository-rule attributes that Slug rejects.
Those declaration/schema semantics were explicitly deferred. Therefore the
packet cannot both forbid them and require successful rules_rust definition
export, pure reacquisition or the full pinned source closure.

All six frozen entry hashes in `dc85f527` were verified before the stop. No
Rust file was edited and no Cargo command ran.

## Design task

Design exactly one corrected observable for the unchanged selected-source
owner. Prefer the smallest focused selected-registry module whose extension
definition and recursive `.bzl` loads use only already-admitted Starlark:

- retain the real root request -> selected canonical definition association;
- discriminate a same-selected-repository cross-package load and one declared
  mapped selected-repository load, switching to each child's producer view;
- project and authenticate the named module-extension export only for that
  bounded source;
- preserve the actual rules_rust `repository_rule(doc)` terminal as an exact
  named downstream boundary, not a success claim; and
- prove root definition loading remains unchanged.

First audit existing pinned Bazel 9.2 source/tests and accepted fixtures for a
discriminating minimal source-loading case. Reuse accepted evidence if it
separates source association/mapping from repository-rule declaration. If it
does not, select one bounded evidence prerequisite rather than authorizing an
implementation with nondiscriminating synthetic proof.

Record exact natural producer and consumer, source/load child order, structural
identity/invalidation, retained versus compute-local state, event/observation
ownership, cancellation/warm/A-B-A behavior, file authority, entry hashes,
physical/semantic caps and compatibility classes. State explicitly which
rules_rust behaviors remain downstream.

Reach exactly one terminal:

1. an independently reviewed corrected implementation design authorizing at
   most one six-file successor;
2. one uniquely smaller pinned-source/oracle evidence prerequisite; or
3. `REPLAN` if no bounded observable isolates source loading from declaration
   semantics.

## Architecture and compatibility

Pinned Bazel 9.2 remains sole behavior authority. Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` remains concept-only guidance:
Bzlmod owns selected semantic descriptor/repository view, repository code owns
immutable realization, loading consumes typed source facts, and physical paths
never repair semantic visibility. Copy no Zig code or semantics.

The intended admitted behavior is **exact** only for the bounded root-owned,
non-isolated selected-registry source request and self/mapped recursive loads.
Typed DICE carriers, event/epoch representation, Rust heap ownership and route
representation are **Slug-native**. Actual rules_rust repository-rule
declarations, collection schemas/calls, `repository_ctx` breadth, toolchains,
providers/actions/input trees, crate_universe, public activation, M8/M7B and
exact configuration/output bytes remain **unsupported/deferred**.

## Authority

This packet is docs-only. Write authority is exactly the canonical plan,
current manifest, Stages 4/5/6 and orchestration routing log, at net caps
<=40/<=180/<=160/<=220/<=30/<=30 and <=660 aggregate lines. Rust, tests,
fixtures, oracle JSON/workspaces, Cargo/BUILD, generated/vendored content and
all callers are read-only.

STOP implementation, source/schema waivers, invented `@bazel_tools` content,
Java/JVM, public breadth, repository-rule semantics, unrelated cleanup,
milestone closure, M8/M7B and exact identity bytes. `REPLAN` before widening
authority, changing the accepted producer split or selecting a second owner.
