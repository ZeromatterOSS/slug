# Current Slug V2 Work Packet

Packet: WP-7A-run-daemon-readiness-attribution-audit-r1

Status: SELECTED after the run-registry implementation gate returned `REPLAN`.
All four Rust edits were restored; complete R2 remains untouched.

## Preceding implementation stop

The rejected candidate stayed inside its four-file allowlist and measured 6
production, 132 proof and 138 gross added lines. Its frozen source hashes were:

- Commands run: `f6737fe55ecae2e322f1d0876e819203a97f6915feb6007677b6fa4e4db35b18`
- CLI run: `41bd3d7c660001461e068955229584a76e655e341e5a7ed358564961944c7392`
- CLI integration: `df154265347fbf8b604c2217bb096012253c985c68beeeaccc99133062df8b31`
- Server tests: `6fe3ce7f65f64ae0a0fe42b7cc66b3bf9f8484f2a231aa220ff2059798a38b6a`

Direct `rustfmt` passed after both Snap formatter entry points failed before
formatting. The single combined compile-only preparation passed in 41.7 seconds.
Two Commands parser proofs passed in 0.00 seconds and the CLI workspace-partition
unit proof passed in 0.00 seconds. The CLI one-shot/daemon integration proof then
failed in 10.04 seconds: its one-shot half completed, but daemon startup reported
that its socket did not become ready within 10 seconds. The server wire proof was
not run. There was no retry, extension or test over the 15-second absolute cap.

The candidate was fully restored after the failed gate. `git diff --check` and
the source worktree are clean, and no Cargo, compiler, slug, slugd or tracer
process survived. No R2 section or fixture changed. The compile had produced a
fresh 649612568-byte `target/debug/slug` at
`2026-09-10 20:44:12.555` local time, so staleness does not explain the result;
it also does not establish why the child failed.

## Read-only attribution audit

`start_daemon` re-execs `current_exe` with `--serve`, writes its PID, discards the
child handle and redirects stdin/stdout/stderr to null. It then polls only socket
connectability for 10 seconds. It never calls `try_wait`, so an immediate child
exit, dispatch/argument error, bind error and a process that remains alive without
binding collapse into the same timeout. `serve_daemon` does dispatch `--serve`
and the server binds before constructing `Daemon`; direct Server tests construct
or call the daemon in-process and therefore do not cover CLI re-exec readiness.
Existing CLI tests prove successful launches but retain no failed-child stderr or
exit status. The failed fixture was cleaned, so no causal child output survives.

Run one diagnostic only, using the already-built binary whose launch/serve code
matches the restored source. Invoke `strace -f` on direct `slug --serve` with a
fresh short socket path and the repository as workspace. Select only process,
network and descriptor syscalls, retain bounded trace/stderr under `/tmp`, and
enforce a 5-second wall deadline with a 1-second kill grace. Send no build/run
request, registry value, remote endpoint or credential; do not inspect
`~/.bazelrc`. The only question is whether this binary reaches exec, argument
dispatch and Unix bind or exits first. Check and remove only the diagnostic
socket/temp directory after process reaping; retain the bounded logs by path and
hash if informative.

Do not rerun the failed integration test, compile, execute a command request,
start a replay, edit Rust or apply any R2 section during this packet. Any trace
growth above 2 MiB, wall time above 5 seconds, surviving process, ambiguous
binary, unexpected workspace mutation or credential-bearing access is `REPLAN`.
After the diagnostic, independently review attribution and select either a
test-only supervised launch proof or a narrowly designed `start_daemon`
child-exit diagnostic correction before revisiting registry propagation.

Preserve `/tmp/slug-conflict-r2.XZJWwv/candidate.patch` at SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`
and `/tmp/slug-sentinel-draft.Lln8y0/candidate.patch` at SHA-256
`8eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2`.
Every future test remains capped at 12 seconds, 15 seconds absolute. More than a
minute is a red flag for any iterative command.
