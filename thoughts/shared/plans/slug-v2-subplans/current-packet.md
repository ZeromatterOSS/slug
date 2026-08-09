# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-isolated-observable-candidate-oracle-design`
Milestone: M3 query / Stage 4 loading evidence design
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: design the smallest complete isolated Bazel 9.2 ordinary-`attr()`
fixture that no protected Slug semantic regression loads.

## Background and boundary

The accepted 18-lane matrix covers the finite observable RuleClass ledger, but
generation disproved its proposed shared-workspace placement. Required
constructors such as `attr.string_list()` added to the existing `pkg/defs.bzl`
are parsed by the protected 29-row Slug CLI consumer before row one and exceed
Slug's currently admitted Starlark attr surface. A draft Bazel fixture passed
57-row update/replay and froze `@@ext+//leaf:label`, but was incomplete and was
discarded. No production expansion is authorized to make an oracle fixture
loadable by Slug.

## Required design

- Select the smallest isolated canonical-payload workspace and oracle fixture
  that the Bazel harness can generate/replay without being selected by any
  current Slug CLI/server semantic regression. Prove the consumer boundary from
  the actual test/fixture call graph rather than the fixture name alone.
- Inventory every required virtual file, directory, module edge, Starlark
  definition, package, target family, external source, fixture row, expected
  record, and Python/Rust payload-integrity consumer before generation. Compare
  its measured growth with the `51540963` hygiene reset and set a new bounded
  cap; do not inherit the disproved +3-file cap mechanically.
- Map every exact observable atom in the accepted Stage 4 18-lane table to a
  distinct positive and negative rule instance. No label may be positive for
  two atoms in one command. Include explicit absence/removal and nonrule
  operands, and require exact stdout to enumerate every positive once and no
  negative.
- Recheck the proposed isolated Starlark definitions against pinned Bazel 9.2
  loading source and current Slug fixture discovery. Candidate position and
  equal-candidate multiplicity remain excluded. Native-toolchain and generic-
  external rows remain loading evidence only.
- Decide whether the isolated fixture should independently reproduce
  `@@ext+//leaf:label` or treat the stopped draft token as non-accepted guidance;
  never weaken an anchored canonical-label regex.

## Files

Edit only the Stage 4 and Stage 8 owner plans. Read the accepted matrix, stopped
generation record in those plans/routing log, pinned Bazel source, oracle
harness/payload inventory, and current CLI/server fixture consumers without
edits. Add no fixture, payload, expected record, Rust, Cargo/lockfile, BUILD,
canonical-plan, manifest, routing-log, `@bazel_tools`, or generated content
during this design packet. Obtain independent oracle-design review before
authorizing isolated generation.

## Stops

Stop and `REPLAN` if no isolated fixture can stay outside protected Slug
semantic consumers, if complete unique atom pairs exceed a reviewable bounded
fixture, if any value requires configured analysis or an unbounded registry, or
if the work would activate production Starlark/query semantics, add graph/DICE
state or regex behavior, integrate JVM/Java source/bytecode/helpers, or delegate
production semantics to Bazel.
