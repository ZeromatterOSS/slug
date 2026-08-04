# Current Slug V2 Packet

Packet: `WP-5-m1-external-bzl-macro-query-oracle`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: oracle implementation worker
Evidence: independently accepted exact oracle design in the Stage 5 owner
plan, two byte-identical fresh-output-base Bazel 9.2 probes, and the protected
17-row `module-local-override` checkpoint.

Implement only the exact four-path oracle contract in the accepted owner
appendix. Read `AGENTS.md`, the orchestration skill implementation-worker
reference, the appendix, the fixture schema/runner/normalizer, and the live
fixture/expected record before editing. Check the clean worktree and preserve
all existing rows and `workspace/dep/BUILD.bazel` byte-for-byte.

Add exactly the two accepted `dep/macro` assets and append exactly the three
accepted anchored `message_shape` rows. Generate with `/usr/bin/bazel` only
after proving it reports `bazel 9.2.0`, then replay all 20 rows from a distinct
fresh run root. Preserve expected command objects 0 through 16 as JSON-deep-
equal baseline objects and keep only the three new generated records.

The exact caps are four changed paths, two new regular files, three new rows,
and +112 lines total: TOML +21 (144 total), expected JSON +86 (546 total),
BUILD +3, defs +2. Final inventory is 20 commands, seven workspace assets,
nine fixture files, 710 protected-metric lines, and 713 whole-fixture lines.
No daemon, epoch, mutation, lifecycle, missing/cycle, source attribute,
visibility, MODULE, helper, tool, Rust, Cargo, CLI, or Slug activation change
is authorized.

Run the exact generation/replay and structural checks from the appendix, the
full oracle harness unit suite, archive status, and `git diff --check`. Obtain
one independent latest-diff fixture review. Stop with **REPLAN** on any cap,
output, provenance, protected-row, tool-version, or scope mismatch rather than
widening the packet.
