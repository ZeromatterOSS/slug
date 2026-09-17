# Current Slug V2 Work Packet

Packet: WP-7-18-m7a-cli-root-bazel-build-r1
Status: accepted; pinned Bazel CLI root builds

## Outcome and compatibility

Run one real Bazel 9.2 build of the production root
`//app/slug_cli_v2:slug` after the accepted cache-leaf generation, direct
REAPI adapter compile, and verbatim `bazel_tools` compile-input fix. A success
would prove only this developer-built binary under the pinned source and
toolchain; it does not prove Slug self-hosting, runtime compatibility, REAPI
execution, production feature parity, or M7A readiness. This packet changes
no Slug semantics or compatibility class.

WP-7-17 at `54c1eb905` compiled the direct adapter target and its cache-leaf
dependency while preserving all 49 pinned `bazel_tools` bytes. Earlier
CLI-root cquery/aquery established configured labels and generated-input
declarations but ran no compiler action. The full-root build is the next
distinct Stage 10 gate.

## Bounded observation and stops

The checked-in Cargo/Bazel/module locks, first-party BUILD files, pinned
rules_rust 0.73 and nightly 2025-09-14 toolchain own the build. Bazel output
is command scratch, not a Slug DICE fact or semantic oracle. From clean main,
run only `bazel --ignore_all_rc_files --batch build` of this root with
`--lockfile_mode=error` and
`--@rules_rust//rust/toolchain/channel=nightly`. No BuildBuddy RC or
credentials are used. Bound the compile at 60 seconds with TERM/short KILL
cleanup and a 128 MiB output-file cap; record command, elapsed time, exit
status, stdout/stderr digest, action result, output and tracked-input status.
This is compile preparation, not a test. No tests or oracle fixture runs are
selected, consistent with keeping tests as small as possible.

On success, inspect the binary target output and keep the M7A readiness row
open pending feature/action correspondence and Slug's own bootstrap behavior.
On compiler, source, lock or timeout failure, record the first actionable
cause and stop this packet; do not repeat the unchanged command or infer that
the source or target is absent. A correction must have its own bounded
successor. This packet edits only this manifest, canonical Live Status, Stage
10's live inventory delta and the first bootstrap-readiness row. Preserve all
source and lock inputs. Run `python3 scripts/v2_plan_status.py` and
`git diff --check`; commit and push a complete observation checkpoint.

## Build receipt (2026-09-16)

From source/lock baseline main `54c1eb905`, with only packet documentation
modified, the sole build command was:

```text
ulimit -f 131072
/usr/bin/time -f 'elapsed_seconds=%e\nexit_status=%x' -o target/wp718/time.txt timeout -k 3s 60s bazel --ignore_all_rc_files --batch build --lockfile_mode=error --@rules_rust//rust/toolchain/channel=nightly //app/slug_cli_v2:slug > target/wp718/stdout.txt 2> target/wp718/stderr.txt
```

It exited 0 in 10.69 seconds. Bazel analyzed 18,787 configured targets and
reported 1,370 actions in this build graph, with 1,360 action-cache hits and
ten processes (five internal, five linux-sandbox). The binary is a 129,251,216-
byte Linux x86-64 ELF at `bazel-bin/app/slug_cli_v2/slug`, SHA-256
`fd398374150c724e8da499139b76cc4c6e397b6d2bcb335f9a4f8fd93691f86a`.
The LALRPOP producer tree contains nonempty `syntax/grammar.rs` (618,176
bytes, SHA-256
`b8f262d5c57e6348de5de7d5ea09fab6714b6e4f01543adfceed3092a0c7b096`)
and its report; the cache leaf retains six nonempty generated proto modules.
Stdout is empty; the 1,348-byte stderr SHA-256 is
`80125d2b6d35dee06966f1c7aeca1ef664d8f3f59ebcd035c4309b3acf25bfb5`.
The ignored `target/wp718/` scratch holds the streams and timing. No test,
runtime smoke, source or lock change ran. This proves the named Bazel build
under its current configuration. Feature/pin correspondence, Slug semantic
action coverage, runtime execution and M7A remain open.
