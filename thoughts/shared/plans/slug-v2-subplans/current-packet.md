# Current Slug V2 Packet

Packet: `WP-4-5-7A-selected-registration-pattern-retention-owner`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Result: implement the first accepted ownership slice of the complete MODULE
registration-pattern design. Retain final raw pattern declarations for both
builtins and publish one post-extension-mapping-backed owner projection. Do not
parse wildcards through package loading or activate configured behavior.

## Learned facts and decision

Commit `0a799e522` accepts the complete shared JSON category and recursively
freezes authenticated rules_rust `rust/private/toolchain.bzl`. The first normal
`slug query '//...'` replay now fails at root `MODULE.bazel` line 21 because
Slug's root evaluator rejects `@rust_toolchains//:all` as not being a direct
label.

Bazel 9.2 does not parse registrations as labels during MODULE evaluation.
`ModuleFileGlobals` checks only absolute pattern spelling after dev-dependency
suppression and retains raw strings. `RegisteredToolchainsFunction` and
`RegisteredExecutionPlatformsFunction` later parse each selected module's
patterns with that module's canonical repository and full mapping, expand them,
deduplicate in stable order, apply different wildcard-kind filters, then
validate configured providers/settings. Patterns expanding to multiple targets
are lexical by target name; direct, `:all`, `:*`, `:all-targets` and recursive
forms belong to one category, including explicit-target/wildcard ambiguity.

The complete design is recorded in
`06-analysis-toolchains-and-actions.md`. This packet finalizes only its
declaration and selected-owner representation so later syntax and expansion
packets do not replace it.

## Required implementation

1. Add one compact validated raw registration-pattern text type used by root
   and nonroot MODULE results. It preserves exact spelling/source order and
   deliberately exposes no label accessors.
2. Use one collector for both builtins and root/nonroot evaluation. Suppress
   ignored dev rows before inspecting variadic elements; ordinary rows then
   require string values beginning with `//` or `@`. A fresh Bazel 9.2 oracle
   proves ignored relative strings and ignored integers both succeed.
3. Replace `RootModuleRegistrations` direct labels with immutable raw rows.
   Preserve DICE equality, include order, command-policy filtering and all
   existing direct registration behavior.
4. Add a public selected-registration-pattern key/value/view beside the private
   selected extension mappings. Retain that predecessor plus compact checked
   `(route_ordinal, pattern_ordinal)` arrays for each family. Pattern text comes
   from its retained route/source; owner and ordered mapping come from its final
   post-`use_repo` mapping at the same ordinal. Do not use the base route mapping:
   it cannot resolve the live generated `@rust_toolchains` apparent name.
5. Supply legacy and observed keys with existing selected-extension-mapping
   Need/error/epoch order, complete-only equality/validity, warm reuse, semantic
   A/B/A restoration, cancellation and nonactivation proof.
6. Adapt current analysis compilation by parsing only already-supported direct
   patterns. A package/recursive pattern must fail closed with an explicit
   unexpanded-registration error before loading/configured publication. Query
   may advance past MODULE evaluation, but this packet makes no expansion or
   toolchain-resolution claim.

## Architecture, compatibility and guidance

Bazel 9.2 `ModuleFileGlobals`, `InterimModule`, `TargetPattern`,
`TargetPatternUtil`, `RegisteredToolchainsFunction`,
`RegisteredExecutionPlatformsFunction` and their focused tests are exact
authority. Generate a focused Bazel 9.2 oracle only where pinned source/tests do
not discriminate retained order, ambiguity, filtering or error timing.

Zabel is peer guidance only. Its useful separation is raw registration text +
declaring canonical owner, followed by a context-aware parser, later expansion
and policy-specific filtering. Its packed storage, Zig types, diagnostics and
compatibility claims are not copied. Reuse existing Buck2-derived compact
strings, small collections, immutable slices and DICE patterns in Slug; the
design must include a utility-reuse audit and memory-accounting impact.

- **Exact:** MODULE ABI/type and dev filtering, absolute raw retention,
  selected-module/declaration order and selected canonical owner/mapping views.
- **Slug-native:** Rust retained representation, compact allocation, DICE keys,
  cancellation carriers and memory accounting.
- **Unsupported/deferred:** wildcard parsing/ambiguity, package or recursive
  expansion, deduplication/filter activation, configuration-option inputs,
  configured provider/settings validation, rule/toolchain implementation,
  actions/input trees and exact configuration/output identity.

## Allowlist and validation

Base is `3a1d19f40`. Change only:

- `app/slug_bzlmod_v2/src/interim_module.rs` (452 lines);
- `app/slug_bzlmod_v2/src/module_eval.rs` (6,672 lines);
- `app/slug_bzlmod_v2/src/selected_repo_spec.rs` (13,741 lines);
- `app/slug_bzlmod_v2/src/lib.rs` (475 lines);
- `app/slug_bzlmod_v2/src/host_module.rs` (5,349 lines, proof/adapters only);
- `app/slug_bzlmod_v2/tests/nonroot_module_eval.rs` (701 lines, raw-pattern
  type adaptation/proof only);
- `app/slug_bzlmod_v2/tests/root_module_dice.rs` (2,429 lines); and
- `app/slug_analysis_v2/src/dice.rs` (2,964 lines, direct-only adapter/proof).

Caps are 500 production, 650 proof and 1,150 total additions; deletions do not
buy budget. Each new helper/test is at most 100 lines. Add no dependency,
global state, lock, package traversal, target-pattern parser or copied mapping.
Use existing `CompactString`, `Arc`, `SmallMap`, `Dupe` and `Allocative`; record
the Buck2 utility audit and retained-size effect.

Run focused root/nonroot declaration and selected-projection tests, all
`slug_bzlmod_v2` tests, `slug_analysis_v2` tests, loading/BUILD/invalidation
suites, locked core check and locked CLI build serially. Rebuild
`slug_cli_v2`, clean `slugd`, and replay `slug query '//...'`; acceptance is
advancement beyond the MODULE registration rejection, not query success. Run
format, diff, scope/cap/helper, archive and no-lock/DICE ownership review.

STOP and `REPLAN` for route/source lifetime duplication, missing owner mapping,
ordinal overflow without typed failure, changed selected order, package loading,
wildcard activation, a new parser/traversal, configured publication, allowlist/
cap escape or a new terminal before the expected fail-closed adapter.

## Immediate predecessor

Commit `3a1d19f40` selects and the canonical subplan records the complete shared
registration-pattern architecture. This packet implements only its final raw
declaration and selected-owner representation.

## Replan record

The live source audit found that `NonrootModuleBuilder` is intentionally public
and its integration test constructs registration rows directly. A nominal
validated raw-pattern type therefore requires the corresponding mechanical test
adaptation. The original allowlist omitted
`app/slug_bzlmod_v2/tests/nonroot_module_eval.rs`; adding that proof-only file is
the smallest correction that preserves requirement 1 instead of weakening it
to an unenforced `CompactString` alias. Scope, caps, architecture, compatibility
classification and all stop conditions are otherwise unchanged.
