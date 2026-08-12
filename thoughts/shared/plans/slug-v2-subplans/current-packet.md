# Current Slug V2 Packet

Packet: `WP-5-host-command-module-override-owner-design`
Milestone: cross-stage M7 prerequisite design
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: freeze one normalized command-module-override semantic input from
command parsing through DICE, without activating discovery or a graph.

## REPLAN predecessor

`WP-5-host-selected-module-graph-owner-design` ended `REPLAN` after live
source audit and independent reserved-architecture review. A sole Host selected
graph cannot yet preserve Bazel 9.2 override precedence:
`BzlmodCommandPolicyKey` carries only yanked-version and dev-dependency
policy, `RootModuleOverrides` carries only root-MODULE declarations, and
`HostDiscoveredModuleKey` therefore cannot distinguish an explicit command
override from the default built-in/nonregistry sentinel. Reusing either map or
legacy `ResolvedGraph` would corrupt structural equality and built-in bypass.

The accepted discovered-module leaf in `6b2967c7` remains callerless and
unchanged. No discovery recursion, MVS, mapping, package/loading, or consumer is
authorized until this missing command input has an accepted owner.

## Active design contract

Design only one normalized command-module-override input spanning the existing
CLI/server request-policy path and DICE injection. Audit live Rust ownership and
pinned Bazel 9.2 primary source; do not implement it.

The design must freeze:

1. Bazel 9.2's exact flag name and grammar, repetition/duplicate behavior,
   option-source precedence, module-name validation, path validation,
   workspace-relative/absolute interpretation, normalization timing, and
   terminal diagnostic order;
2. one retained normalized value separate from `RootModuleOverrides`, with
   exact ordered/effective override identity and every semantic path input;
3. the narrow command parser/request/server/runtime/DICE ownership route,
   including whether the existing generic normalized flag wire can carry the
   value without schema change and which process owns workspace-relative
   normalization;
4. precedence between command overrides and root MODULE declarations, plus the
   explicit `bazel_tools` override that must bypass the default built-in key;
5. equality/validity/invalidation rules that retain semantic normalized paths
   but do not observe repository contents, materialize a RepoSpec, inspect the
   filesystem, or enter discovery;
6. exact typed success/error behavior and Slug-native DICE/type/diagnostic
   framing; and
7. one bounded implementation successor with an explicit file allowlist,
   separate production/test caps, parser/server/runtime and real-DICE proof,
   downstream checks, structural scans, and terminal stops.

Required proof for the future implementation must discriminate absent/present
A/B/A, path changes, spelling equivalence only where Bazel normalizes it,
ordered repetitions/duplicates/errors, root-declaration versus command
precedence, explicit `bazel_tools` bypass, cold/warm reuse, and one-shot/
stable-daemon request isolation. The design must fail closed rather than infer
semantics not pinned by source or oracle.

Compatibility may be exact only for the admitted Bazel 9.2 command-override
grammar, precedence, and path semantics. Slug-native surfaces remain
DICE/type/diagnostic names, Rust path representation, compact storage, and
non-Bazel identity bytes. RepoSpec synthesis, filesystem observation,
materialization, discovery/MVS, selected graph, canonical mappings, extensions/
registrations/flags, lockfile products, package/Bzl loading, configured
toolchains, Test, execution/results/BEP/coverage, Windows, JVM/Java, and exact
Bazel identity bytes remain unsupported/deferred.

## Scope, caps, and proof

This design packet may edit only:

- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`;
  and
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`.

Cap formatted net document growth at 300 lines in the manifest, 160 lines in
the owner plan, 40 lines in canonical status, and 500 total. Read-only ownership
audit may inspect existing CLI command parsing, command/request policy,
server/runtime injection, and bzlmod DICE source. Add no Rust, Cargo/BUILD
metadata, wire/schema change, fixture, generated evidence, dependency, public
surface, or production representation.

Required evidence is pinned Bazel 9.2 source or a minimal accepted oracle for
every exact grammar/path/precedence claim, a targeted live-source route and
identity audit, diff/scope/cap checks, and one independent reserved-architecture
review. Freeze the successor allowlist/caps/stops before independent
`ACCEPT`.

## Terminal stops

Return `REPLAN` on unresolved flag grammar or precedence, filesystem or
materialization ownership in this input, public-wire ambiguity, command
normalization that lacks a bounded owner, discovery/graph implementation, Rust
edit, fourth file, cap excess, or independent-review blocker.
