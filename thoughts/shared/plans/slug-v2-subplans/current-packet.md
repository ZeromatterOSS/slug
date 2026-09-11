# Current Slug V2 Work Packet

Packet: WP-7A-run-registry-policy-design-r1

Status: SELECTED, DOCS/SOURCE DESIGN ONLY. Complete R2 remains preserved and
must not be restored, copied, staged or executed.

## Audit result and concrete stop

WP-5-7A-complete-r2-authentic-fixture-policy-audit-r1 returns `REPLAN` at the
`run` request boundary. The read-only audit inspected the preserved R2 and its
validation plus bounded current CLI request, fixture, mirror and accepted-recipe
owners. It ran no compiler, test, CLI, daemon, Bazel, probe, strace, network,
cache scan, acquisition or replay and did not read credentials.

The repeated `--registry` flag is the natural request-policy input. Existing
Build, Query, Cquery and Aquery requests retain it in `registry_urls`; their
one-shot and daemon paths pass that value to Bzlmod. R2's direct
`sentinel_outputs` and subprocess calls omit the flag, but adding it everywhere
does not fix `run`: `slug_commands_v2::run::RunRequest` has no `registry_urls`,
one-shot `commands/run.rs` explicitly passes `&[]`, and daemon mode constructs
`BzlmodRequestInputs::from_normalized` without registries. Complete R2 cannot
receive one authentic policy on every entry path without a production change.

The fake fixture inventory is nevertheless exact and remains frozen. In R2's
new `configured_action_conflicts` module, remove the root MODULE's
`local_path_override` and three writes under `.slug_test_builtin/platforms`:
`MODULE.bazel`, `host/BUILD.bazel`, and `host/constraints.bzl`. Retain the root
`//:platform`, toolchain declarations, BUILD and defs; those are conflict
semantics, not fake external source bodies.

## Goal

Freeze the smallest normal command-policy correction that gives `run` the same
explicit registry semantics already owned by build/query/cquery/aquery, before
any Rust edit or R2 restoration. Cover direct parsing, workspace override-policy
normalization, one-shot Core evaluation, daemon request construction, program
argument separation and command diagnostics. This is command parity, not an R2
test-only bypass.

## Authorized work

- Read at most eight initial source/test/doc owners and at most eight directly
  referenced owners, excerpts below2MiB. Start with Commands `run.rs`/common
  parsing, CLI `commands/run.rs`, BuildRequest parity, server Bzlmod inputs and
  their closest tests.
- Trace repeated/ordered `--registry` occurrences before and after `--`, the
  `parse_run_request_at_workspace` override branch, one-shot and daemon handoff,
  and existing redaction/error behavior. Preserve program arguments verbatim.
- Decide whether `RunRequest.registry_urls: Vec<String>` plus the existing
  `bzlmod_registry_urls` parser and `from_normalized_with_registry_urls` daemon
  constructor is the single sufficient representation. No second policy owner.
- Freeze exact production/test files, structural/equality/lifetime behavior,
  failure precedence, proof cases, line caps and compile-first bounded gates for
  a separately reviewed implementation packet.
- Update only this manifest, canonical Live Status, Stage5 and `~/PROGRESS.md`.
  Obtain independent terminal review before selecting Rust implementation.

## Stops and prohibitions

No Rust, fixture, payload or driver edits. No Cargo/compiler/test/CLI/daemon/
Bazel/probe/strace/network/cache scan/acquisition/replay. Do not apply, copy,
reconstruct or partially stage R2. Do not add a test-only flag parser, mutate
semantic identity, alter selected graph/source admission, or combine the run
correction with R2 before its own design and implementation are accepted.

Every future test invocation is capped at12 seconds with15 seconds absolute;
compile-only preparation may be separately bounded below60 seconds. No full
suite, automatic retry, timeout extension or broad replay. Any design needing a
second retained policy, changed argument partition, credential handling or broad
server protocol change is `REPLAN`.

After an independently accepted run-policy implementation, return to one atomic
complete-R2 restoration plus authentic fixture correction. That later fixture
must use the already verified byte-authentic local registry/file-mirror recipe,
give one exact registry argument to direct Core and every CLI subprocess, and
stop at any missing payload, external-network attempt or new source boundary.
Transport policy must not enter configured/action/output-conflict identity.

Preserve `/tmp/slug-conflict-r2.XZJWwv/candidate.patch` at SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`
and its adjacent validation evidence. Preserve
`/tmp/slug-sentinel-draft.Lln8y0/candidate.patch` at SHA-256
`8eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2`.
Never inspect/print/copy/commit `~/.bazelrc` or derived credentials.
