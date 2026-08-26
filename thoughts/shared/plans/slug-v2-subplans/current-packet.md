# Current Slug V2 Packet

Packet: `WP-4-7A-clippy-aspect-attribute-audit`

Milestone: M7A command/ruleset bootstrap closure.

Result: authenticate Bazel's private aspect-attribute contract for clippy's
11-label map and select one bounded implementation or `REPLAN`. This packet is
docs-only.

## Corrected starting point

Base `7bba3a4e` selected mixed aspect toolchain requirements after correctly
authenticating the recursive route into `rust/private/clippy.bzl`. Its required
source-shaped proof exposed an earlier unsupported operation before any Rust
was accepted.

All `rust_clippy_aspect` keyword values evaluate, including the mixed toolchain
list. Inside Slug's `aspect()` body, however, `aspect_attributes` runs before
`aspect_toolchain_requirement`. It accepts only the fixed rustfmt `_config` and
`_process_wrapper` pair, so clippy's 11-entry private label map at lines
317-364 is the first unsupported surface. The failed implementation and proof
diff was fully reverted; the worktree returned to the committed docs state.

The toolchain list at lines 370-373 remains a later candidate. Do not implement
or retain it in this audit.

## Fixed sources and authorities

Selected rules_rust 0.73.0:

- `rust/private/clippy.bzl`, SHA-256 `a778d2ddc77587ffbffc72efcdaa458a1ffae0763e500da1c876b9b567b2a686`;
- `rust/defs.bzl`, SHA-256 `5b71e4344a6c6ee04ade488c741784479f392b71d42f2102eedc5e4993654512`.

Selected bazel_skylib 1.8.2 `lib/structs.bzl` is SHA-256
`c3fa79b9246582cb57c1bd9cbed999afbee822915d5888009bc0a197c43e9749`.

Behavior authority is clean Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`. Inspect the aspect API,
`StarlarkRuleClassFunctions.aspect`, its attribute construction/validation and
focused tests. Authenticate private-name/default requirements, allowed
attribute kinds, configurability, `cfg = "exec"`, `executable`, file allowance,
defining-module label conversion, order and rejection behavior.

Architectural guidance is clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a`. Its `AspectDefinition.attrs` and
shared named-attribute declaration shape may guide natural ownership and
evaluator detachment. Zabel defines no compatibility behavior; copy no Zig
code, layout, diagnostics or evaluator algorithm.

## Audit obligations

- Classify each clippy row at lines 317-364 by name, kind, default identity,
  file allowance, executable bit and configuration transition.
- Establish the exact Bazel 9.2 contract and focused regression for private
  aspect attributes, including invalid public/implicit/defaultless cases.
- Compare the source subset with `AttributeDefinition`,
  `RuleAttributeSchemaGen`, `declared_attribute_schema` and the current fixed
  `aspect_attributes` owner. Identify what can be shared without changing
  ordinary rule behavior or configured analysis.
- Preserve source order: the selected implementation may stop only after the
  complete attribute map is retained and must leave provider, fragment,
  toolchain and complete-aspect claims at their already admitted/later bounds.
- Classify the prospective change as exact, Slug-native or
  unsupported/deferred. If bounded, write one implementation packet with an
  exact source stop, two-file-or-smaller allowlist, base hashes, line/addition
  caps, discriminating proofs, serial validation and STOP triggers.
- State exactly how Zabel informed ownership. Read the Buck2 utility skill only
  if the selected implementation changes retained representation, collection,
  hashing, interning, clone cost or memory accounting.

## Allowlist and caps

Only these plan files may change from base `7bba3a4e`:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `.codex/skills/slug-agent-orchestration/references/routing-log.md` (one
  correction row only).

No Rust, tests, lockfiles, sources, DICE keys, repository data, oracle fixtures
or generated evidence may change. The correction/audit addition cap is 220
lines across the canonical plan and Stage 4 subplan; this manifest is capped at
180 lines and the routing addition at one row. Use read-only checks only.

## Review and STOP

Independent review must verify the failed proof evidence, corrected first
unsupported operation, fixed hashes, docs-only boundary, Bazel authority,
Zabel guidance-only role and selected follow-up.

STOP and `REPLAN` for a dirty source/authority checkout; unresolved attribute
semantics; a required configured-aspect or analysis consumer; an unbounded
general attribute redesign; Java/JVM work; copied Zabel behavior; a Rust/test/
source edit; toolchain parsing; a claim beyond the attribute map; or cap
violation.
