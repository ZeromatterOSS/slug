# Current Slug V2 Work Packet

Packet: WP-7A-run-registry-policy-implementation-r1

Status: SELECTED after independent acceptance of the preceding docs/source
design. Complete R2 remains untouched.

## Design result

The existing representation is sufficient. Add one owned `Vec<String>` field to
`slug_commands_v2::run::RunRequest`, populated by the existing
`bzlmod_registry_urls(&parsed.flags)` helper. That helper already rejects empty
values and preserves repeated occurrence order. Derived Clone/Eq then cover the
new field without a second owner or custom identity.

`split_args` already partitions at the first literal `--`: registry flags before
it remain parsed flags, while identical text after it remains a program argument.
In CLI `parse_run_at_workspace`, the override branch must leave registry flags in
`run_args` and must not add them to `policy_args`; only the normalized Bzlmod
command policy is copied from the temporary BuildRequest. This preserves relative
module-override normalization and exact program arguments.

One-shot run must replace its literal empty registry slice with a borrow of
`request.registry_urls`. Daemon run must replace
`BzlmodRequestInputs::from_normalized` with the existing
`from_normalized_with_registry_urls`, borrowing the same field before the request
is moved. The server wire type, decoder and Core API already own ordered registry
URLs, so no protocol, server production, DICE, source-admission or identity change
is required. `RemoteConfig` intentionally ignores non-remote flags, so the parsed
registry occurrence does not alter executor configuration.

The vector lives only for the command request. One-shot borrows it synchronously
into existing normalized runtime input; daemon construction clones its strings
into the existing primitive wire owner. It is not added to program argv, remote
headers, diagnostics, action/configuration identity or retained output state.
Existing registry URL diagnostics remain unchanged and no value is logged.

## Exact implementation allowlist and caps

- `app/slug_commands_v2/src/run.rs`: import the existing helper, add/populate the
  field, and add focused parser proofs.
- `app/slug_cli_v2/src/commands/run.rs`: wire one-shot/daemon and prove the
  workspace-override partition retains registries and program arguments.
- `app/slug_cli_v2/tests/cli.rs`: add a focused invalid-file-registry proof for
  one-shot and stable-daemon run before any remote execution.
- `app/slug_server_v2/src/tests.rs`: prove a Run daemon request retains ordered
  registry wire input and omitted input remains empty.

Production additions cap40 lines, proof additions cap160, gross cap200. Per-file
net growth caps are80/80/50/40 respectively. No new file, dependency, feature,
key, cache, lock, service, parser, protocol field or fixture/payload change.

## Required proof

- Parser table: two registry occurrences retain order; missing/empty value keeps
  the existing exact InvalidFlagValue category; registry-like text after `--` is
  an unchanged program argument and does not enter `registry_urls`.
- Workspace override A/B/A: relative override normalization stays identical,
  registry order survives, and target/program arguments including `--` payload
  are byte-for-byte unchanged. No registry duplication into policy args.
- One-shot and daemon CLI run with a deliberately invalid non-local file registry
  and a syntactically valid loopback remote executor both return the existing
  registry error before analysis, launch authorization or remote execution. The
  daemon PID is cleaned by existing ownership.
- Server primitive wire: Run preserves ordered URLs, omitted field defaults to
  empty, and existing backward-compatible BuildRequest encoding is unchanged.
- Static checks prove the old `&[]`/no-registry constructor is absent only from
  run handoff, both paths use the request field, program launch sees only
  `program_args`, and no registry value enters diagnostic formatting.

## Compile and execution order

Format and perform one combined compile-only preparation of the affected Commands,
CLI library/integration and Server test targets, capped55 seconds. Do not run a
test during compilation. Freeze hashes after compile. Invoke only the precompiled
named tests serially with12-second wall limits, bounded output and15 seconds
absolute. Then run affected default checks under30 seconds. No full suite, broad
replay, automatic retry or timeout extension. Any compile error, timeout, remote
connection, daemon survivor, unrelated failure, cap overflow or second material
correction is `REPLAN`.

Obtain independent pre-execution and terminal reviews. If accepted, commit/push
this correction alone, then return to a separately frozen atomic complete-R2 plus
authentic-fixture implementation. Do not combine, apply, copy or stage any R2
section here. No credential access, network acquisition, Bazel, probe or strace.

Preserve `/tmp/slug-conflict-r2.XZJWwv/candidate.patch` at SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`
and `/tmp/slug-sentinel-draft.Lln8y0/candidate.patch` at SHA-256
`8eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2`.
Never inspect/print/copy/commit `~/.bazelrc` or derived credentials.
