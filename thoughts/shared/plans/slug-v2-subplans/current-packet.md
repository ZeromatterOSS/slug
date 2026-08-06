# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-canonical-fixture-payload-cycle-baseline-design`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: an accepted correction to the atomic fixture-payload migration contract
which preserves the user-approved cyclic-Bzl deferral without weakening its
oracle row or conflating that baseline with payload behavior.

## Goal

Correct only the payload migration's validation and acceptance boundary now
that the broken-Bzl prerequisite is green and the 57-row CLI test reaches the
separate pre-existing cyclic-Bzl failure which the user explicitly deferred.

## Required design

Trace the clean committed CLI target, the accepted broken-Bzl direct replay,
the next `bzl_cycle_failure` row, and the proposed payload-backed target. Freeze
an exact before/after validation which proves the migration preserves every
non-cycle row and reproduces the same cycle terminal without treating a red
target as payload failure or claiming it is green.

Keep the existing 57-row test body, cyclic assertion, fixture command, and
expected JSON unchanged. Do not skip, ignore, filter out, split away, or mark
the cycle row successful. Define the exact Cargo and Bazel target expectations,
the direct row replay, and any target-level negative gate needed for the one
atomic migration. Preserve the accepted payload bytes, grammar, helper-relative
compile input, four consumers, 163 deletions, hashes, and all other validation.

## Allowed paths

- canonical plan, Stage 10 owner, this manifest, and August routing history

## Required validation

Record exact clean-HEAD and proposed-migration command/test boundaries, expected
statuses, terminal text, and unchanged assertions/evidence. Reuse the accepted
Bazel 9.2 rows; add no oracle. Obtain independent latest-text review. Run
structure, scope, cap, credential-pattern, archive, and `git diff --check` gates.

## Stop conditions

Stop with REPLAN on Rust/Python/fixture/payload/helper/consumer/target/deletion
work, changed test or expected JSON, skipped/ignored/filtered/split cycle row,
cycle success claim or diagnostic repair, weakened negative gate, new oracle,
dependency/BUILD/Cargo/lock, platform exclusion, execution/cache, self-hosting,
Java/JVM, Bazel 8, WORKSPACE, rc, CI, or credentials.

## Diff budget

- At most 180 net documentation lines. No Rust, Python, fixture, oracle, BUILD,
  Cargo, lock, generated, payload, consumer, target, CI, deletion, or unrelated
  change.
