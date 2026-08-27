# Current Slug V2 Packet

Packet: `WP-4-6-7A-complete-module-registration-pattern-category-design`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Result: freeze one complete architecture for MODULE
`register_toolchains` and `register_execution_platforms` target patterns before
any more Rust. The design must cover declaration, selected-module ownership,
parsing, expansion, kind filtering and configured validation as distinct
layers shared by both builtins.

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

Do not patch only `:all`, store a wildcard as a fake label, or expand patterns
inside MODULE evaluation. That would lose owner mapping, selected-module order,
ambiguity and the shared expansion boundary and would force churn when the
second builtin or recursive patterns arrive.

## Required design

Record one reviewed design in
`thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
and update the canonical/current manifests. It must specify:

1. A compact immutable declaration row retaining raw absolute pattern text and
   its declaring selected-module owner. Owner context must be sufficient to
   recover the canonical repository and complete repository mapping; physical
   repository paths are not semantic identity.
2. One parsed target-pattern vocabulary shared with ordinary query/build
   patterns while preserving wildcard spelling and absolute-target ambiguity.
   Parsing must not perform package lookup.
3. One DICE-owned expansion projection used by both registration families.
   It loads package targets through existing loading owners, preserves selected
   module and declaration order, gives each multi-target expansion Bazel lexical
   order, deduplicates stably and participates structurally in invalidation.
4. Separate policies after expansion: wildcard toolchain patterns select
   toolchain-rule candidates; wildcard execution-platform patterns select
   platform candidates; explicit targets survive loading filters and fail later
   if their configured provider is wrong. Target-setting checks remain a
   configured-analysis concern.
5. Root and selected nonroot registrations, dev-dependency suppression,
   apparent repository mapping, direct/package/recursive patterns, empty and
   duplicate expansions, invalid patterns, missing packages/targets and
   A/B/A/cancellation/warm behavior. Both builtins must be handled together.
6. A bounded implementation sequence with exact allowlists, line/function
   caps, oracle/pinned-source evidence and one first packet. Slices may follow
   ownership boundaries, but no slice may invent a representation that the
   complete category later replaces.

Configuration-option registrations may feed the same future expansion owner,
but are not silently claimed by this MODULE category. Rule implementations,
toolchain resolution, action creation, input trees and bootstrap execution stay
downstream.

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

- **Exact:** MODULE ABI and dev filtering; selected-module/declaration order;
  owner-relative mapping; accepted pattern vocabulary; lexical expansion,
  stable deduplication and family-specific filtering/error timing admitted by
  the eventual implementation.
- **Slug-native:** Rust retained representation, compact allocation, DICE keys,
  cancellation carriers and memory accounting.
- **Unsupported/deferred:** configuration-option registration inputs unless
  explicitly selected, configured provider/settings validation not already
  owned, rule/toolchain implementation behavior, actions/input trees, exact
  configuration/output identity and invalid diagnostics not oracle-proved.

## Allowlist and validation

This is docs/design only. Change only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`; and
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`.

Run no Rust mutation and make no live network or credential access. Read the
live checkout, Bazel 9.2 authority and the relevant Zabel peer implementation.
Validate plan/current agreement, exact source citations, one bounded successor,
`git diff --check`, scope and archive hygiene. Root review must reject an
`:all` special case, direct-label storage, root-only semantic claims, merged
declaration/expansion/configured phases, a parallel target-pattern parser,
package loading outside DICE, or copied Zabel authority.

STOP and `REPLAN` if the complete category cannot be bounded, owner mapping is
unavailable, existing target-pattern/loading owners cannot be reused safely, a
new lock would cross DICE compute, exact ordering/filter behavior lacks source
or oracle evidence, or the design would enter downstream toolchain/action work.

## Immediate predecessor

Commit `0a799e522` accepts the shared JSON category and complete authenticated
rules_rust toolchain parent without invocation. The ordinary bootstrap replay
now exposes wildcard registration representation as the next real boundary.
