# Current Slug V2 Work Packet

Packet: WP-7A-daemon-sandbox-serve-attribution-audit-r1

Status: SELECTED after the fresh registry successor returned `REPLAN`. The four
registry files are restored; accepted daemon diagnostics remain on main and
complete output-conflict R2 remains untouched.

## Fresh successor stop

The R2 registry candidate measured6 production/147 proof/153 gross additions
after exact nightly formatting, within all total and per-file caps. Independent
pre-execution review returned `ACCEPT`. Combined no-test compilation passed in
7.93s. Frozen hashes were:

- accepted CLI build: `a391d3464309720c1fc7fa9b0d296a26b67a5d25ca5b01a8cefe1d7677bd9b52`
- Commands run: `3af8a7d6978f3cd4c1844ecfe3097071f38c66fd64593e740b4df34d73df7144`
- CLI run: `eeafc8aa5d1b0fd209e90e2c7669b4ada131e9194682f1d4b82a794c922ef52f`
- CLI integration: `92994cc9a6f862484c4f7e93f719664e6e75c336a5d03dda070ee692d791bc34`
- Server tests: `a5e7bcbe204761b39706ff15bb3365b94380bb34f325fe8831535c866caabbab`

Commands parser proofs passed2/2 in0.00s and the CLI workspace proof passed1/1 in
0.00s. The single fresh integration proof's one-shot half passed, then the daemon
child exited before readiness with status2. The proof failed in0.10s. Per contract
the Server wire proof was not run and there was no retry, extension, correction
or later gate. All four registry files were restored exactly; `build.rs` retains
its accepted hash. No compiler, test, slugd or tracer survived and R2/fixtures
were untouched.

Bounded filesystem evidence for the exact failed output base shows a Unix socket
path length of87 bytes, below the Linux108-byte limit; the directory is mode0755,
uid/gid1000 and the PID file mode0644. No socket inode remains. This excludes the
simple path-length and directory-owner explanations but does not distinguish
argument dispatch from a sandbox-denied or other failed bind because daemon stderr
is intentionally null in normal detached startup. Cause remains unknown.

## Same-boundary direct serve audit

Run one non-test diagnostic only in the ordinary managed sandbox, not escalated.
Invoke the already-built `target/debug/slug --serve` directly with the exact
leftover failed workspace and output-base socket. Capture its foreground stderr,
use no `strace`, and enforce a2s wall deadline plus1s kill grace. Send no command
request, registry value, remote endpoint, network traffic or credential. The sole
question is whether the same binary in the same sandbox reaches normal started
stderr/socket bind or returns a concrete `daemon_serve_error`/argument error.

Before invocation verify the exact workspace/output directory and absence of the
socket. Afterward reap the process group, check no survivor, and remove only the
verified diagnostic socket. Retain bounded stdout/stderr/time logs under `/tmp`,
cap aggregate output at64KiB, and record hashes. Do not rerun any test or registry
command, compile, edit Rust, apply R2, replay, access network or inspect
`~/.bazelrc`. Any survivor, output overflow, source mutation or result that does
not distinguish started-versus-error is `REPLAN`.

After the audit, independently review the attribution. Do not revisit registry
implementation until a separate correction/validation packet is accepted.
Preserve `/tmp/slug-conflict-r2.XZJWwv/candidate.patch` at SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`
and `/tmp/slug-sentinel-draft.Lln8y0/candidate.patch` at SHA-256
`8eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2`.
Every future test remains <=12 seconds and <=15 seconds absolute.
