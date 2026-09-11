# Current Slug V2 Work Packet

Packet: WP-7A-run-registry-policy-implementation-r2

Status: SELECTED after independent acceptance of daemon child-status diagnostics.
This is one fresh successor attempt, not a retry of the rejected R1 process.
Complete output-conflict R2 remains untouched.

## Accepted prerequisite

`start_daemon` now retains the child during readiness, checks exit before socket
connect, reports stable early/observation errors, and kills/reaps the exact owned
child on PID-write failure, observation failure or the existing deadline. It keeps
null stdio, the byte-identical timeout, successful detached behavior and all five
callers unchanged. It never removes PID/socket paths whose ownership could have
been replaced concurrently.

The one-file implementation is31 production/25 proof/56 gross additions. Exact
nightly formatting and diff checks pass. Compile-only passed in6.04s, its sole
controlled early-exit proof passed1/1 in0.05s, default CLI check passed7.60s, and
no process survived. Frozen source SHA-256 is
`a391d3464309720c1fc7fa9b0d296a26b67a5d25ca5b01a8cefe1d7677bd9b52`.
Independent terminal review returned `ACCEPT`.

## Registry propagation successor

Reapply the accepted normal request design from R1 without copying its rejected
worktree. Add one owned ordered `registry_urls: Vec<String>` to
`slug_commands_v2::run::RunRequest`, populated by existing
`bzlmod_registry_urls`. Preserve repeated order, existing empty-value errors and
`split_args`: registry text before `--` is policy, identical text after `--` is
only a program argument.

In CLI `parse_run_at_workspace`, retain registries in `run_args`; do not copy them
to temporary BuildRequest `policy_args`, which still transfers only normalized
command policy. One-shot borrows request URLs instead of `&[]`; daemon uses
`BzlmodRequestInputs::from_normalized_with_registry_urls`. The existing server
wire remains the only daemon owner. Do not alter protocol fields, RemoteConfig,
program argv, diagnostics, action/configuration identity, retained state, DICE,
source admission or fixture payloads.

Exact files and caps remain:

- `app/slug_commands_v2/src/run.rs`
- `app/slug_cli_v2/src/commands/run.rs`
- `app/slug_cli_v2/tests/cli.rs`
- `app/slug_server_v2/src/tests.rs`
- at most40 production/160 proof/200 gross additions, with per-file net caps
  80/80/50/40.

Prove ordered/missing/after-`--` parsing, workspace-override normalization and
program-argument invariance, one-shot plus daemon invalid file-registry rejection
before analysis/launch/remote execution, and ordered/default Run wire transport.
The integration proof may make one fresh R2 invocation only; if daemon startup
fails, retain the new early-status/timeout distinction and immediately `REPLAN`.

Format once, then perform one combined compile-only preparation capped55s. Freeze
all five changed source hashes, including accepted `build.rs`. Run only named
precompiled proofs serially, each<=12s and<=15s absolute. Run affected default
checks<=30s. No broad/full suite, replay, network, automatic retry, timeout
extension or post-compile edit. Any compiler error, test failure, timeout, hash
drift, survivor, remote connection, source-boundary need or cap overflow is
`REPLAN`.

Obtain independent pre-execution and terminal review. If accepted, commit/push
registry parity separately, then return to a newly reviewed all-or-nothing
authentic-fixture application of complete R2. Do not apply/copy/stage any R2
section here. Preserve `/tmp/slug-conflict-r2.XZJWwv/candidate.patch` at SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`
and `/tmp/slug-sentinel-draft.Lln8y0/candidate.patch` at SHA-256
`8eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2`.
Never inspect, print, copy or commit `~/.bazelrc` or derived credentials.
