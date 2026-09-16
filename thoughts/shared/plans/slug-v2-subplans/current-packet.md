# Current Slug V2 Work Packet

Packet: WP-7-10-m7a-bazel-declared-root-query-r1
Status: read-only declared-root query design ACCEPT; one bounded query pending

## Outcome and authority

Obtain the first fresh Bazel declared target-dependency view of
`//app/slug_cli_v2:slug` after accepted static BUILD/lock synchronization.
This is an unconfigured `bazel query` result. It can prove that named
targets are declared and reachable in that view; it cannot prove Cargo feature
parity, configured/action reachability, compilation, execution or M7A
readiness.

Freeze clean main `0447958f5`, root `Cargo.lock` SHA-256
`a19882e78b50a82ed900fce570f4a6aee778553448790eaf08e2e08a591f76d8`,
generated `Cargo.Bazel.lock` SHA-256
`6335564ccbb3c915d0a2fa23b8aec0d69986911e062b586e13c5ec9812c2f35e`,
`MODULE.bazel.lock` SHA-256
`bcdda78dd82aab5fd7ab1796985a78322b50178dceed7669c1b20173c2634302`,
the accepted 35-local/310-external Linux Cargo analysis SHA-256
`dad20c2240aa421f5d99fe30190cbe7fa9035b4e388ad64aa30e762afaf9cd10`,
and the successful no-repin generated-lock validation receipt SHA-256
`290a3c2fce004a895b4470bd985d3618b047eb890457404daa37b182ebcbe278`.
That prior check used a fresh Bazel output base and accepted the candidate
under exact restored Cargo authority; independent final review accepted only
static BUILD/lock synchronization. The earlier inventory Bazel query and
recovery yielded zero rows.

## One bounded query

Assert a clean working tree, all frozen hashes, pinned Bazel 9.2.0 executable
SHA-256
`7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694`,
and absent `/tmp/slug-m7a-declared-query-output-base`. Use the absolute
Bazel executable at
`/home/wgray/.cache/bazelisk/downloads/sha256/7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694/bin/bazel`.
Run one query with startup flags `--batch --ignore_all_rc_files
--output_base=/tmp/slug-m7a-declared-query-output-base` and command:

```text
query --repository_disable_download --lockfile_mode=off --noexperimental_collect_system_network_usage --output=label 'deps(//app/slug_cli_v2:slug)'
```

Leave `CARGO_BAZEL_REPIN`, `REPIN` and generator URL/SHA overrides unset.
Set `CARGO_BAZEL_ISOLATED=false`, `CARGO_HOME=/home/wgray/.cargo`,
`CARGO_NET_OFFLINE=true` and put the pinned nightly-2025-09-14 Cargo
binary directory first in `PATH`. Use the restricted network environment.
A fresh output base and `--lockfile_mode=off` force current extension
evaluation; omit `--nofetch` so Bazel may materialize only cached local
repository content. `--repository_disable_download` forbids repository
downloads. This preserves the no-repin, restored Cargo authority.

The one query has a 30-second TERM/three-second KILL ceiling and 16 MiB cap
per captured stream. Record exact command/environment, fresh-base assertion,
exit/elapsed/cleanup, stream hashes and pre/post tracked-file hashes/status
in a machine-readable `/tmp` receipt. A cache miss, failure, timeout,
unexpected tracked write, invalid cleanup or truncated output stops without
automatic retry, fetch, build, repin or longer ceiling. No test is selected.

## Analysis and result gate

On clean exit, parse the label output as a set and record every line/hash.
Reject non-label stdout rows; count root-workspace packages only from labels
beginning `//`, with apparent/canonical `@` labels classified as external.
Require the root label and report whether the configuration and Starlark V2
package labels are present. Map the 35 selected local Cargo manifests to
their repository package paths and record the reached/missing path sets.
Report external repo labels separately; the unconfigured Bazel target graph
is not the selected Linux Cargo closure. Do not infer configured action
coverage from lock keys, BUILD declarations or a successful query. Compare
the six direct configuration consumer edges from tracked BUILD declarations
to the frozen Cargo normal edges; this static check does not turn them into
configured evidence.

The only tracked edits allowed are this manifest, canonical plan status,
Stage 10 and bootstrap readiness for the result. Run `git diff --check` and
`python3 scripts/v2_plan_status.py`. Independent design review precedes
the query; independent final review checks the receipt, labels, path mapping
and strict unconfigured classification. A valid zero/missing-key result is
recorded as a gap, not silently accepted as full breadth. Configured target
analysis, action execution and buildability need separately scoped evidence.
Independent design review `ACCEPT` confirmed the fresh-base/offline query
boundary and strict unconfigured interpretation. The one 30-second-capped
query is selected.
