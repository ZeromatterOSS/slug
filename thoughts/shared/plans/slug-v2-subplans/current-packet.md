# Current Slug V2 Work Packet

Packet: WP-7-10-m7a-bazel-configured-root-cquery-r1
Status: scoped configured-root observation design ACCEPT; one query selected

## Outcome and authority

Obtain one authentic Bazel 9.2 configured target-dependency view of
`//app/slug_cli_v2:slug` after accepted static BUILD/lock and unconfigured
declared-root evidence. This is read-only analysis: no Rust compilation,
action execution, cache proof or test. The `--output=label` result can identify
labels with displayed seven-hex-character configuration checksum prefixes,
`null` rows or a concrete analysis failure. Those prefixes are not full or
guaranteed-unique configuration identities and do not classify target versus
exec configurations. This result does not admit Slug provider/transition semantics, generated input
content, action-family coverage, REAPI execution, exact ActionKeys,
self-hosting or M7A.

Freeze clean main `8258a1a00`, root `Cargo.lock` SHA-256
`a19882e78b50a82ed900fce570f4a6aee778553448790eaf08e2e08a591f76d8`,
`Cargo.Bazel.lock` SHA-256
`6335564ccbb3c915d0a2fa23b8aec0d69986911e062b586e13c5ec9812c2f35e`,
`MODULE.bazel.lock` SHA-256
`bcdda78dd82aab5fd7ab1796985a78322b50178dceed7669c1b20173c2634302`,
the selected Linux Cargo analysis SHA-256
`dad20c2240aa421f5d99fe30190cbe7fa9035b4e388ad64aa30e762afaf9cd10`,
and accepted declared-root query receipt/analysis SHA-256 values
`1935a3d026a6a31566d6250ac2fc49747b1f08de04d36819984e9dd34ae89b8a`
and `f734391152ac608a0787b7443575b6fe3253294b8211494ba161d6f1545f0b13`.
The declared query produced 18,846 distinct labels and reached all 35
selected local package paths plus eight formerly missing external crate
repositories. It did not run configured analysis.

## One necessary configured analysis

Assert every frozen hash, a clean working tree and absent
`/tmp/slug-m7a-configured-cquery-output-base`. Use the absolute pinned
Bazel 9.2.0 executable at
`/home/wgray/.cache/bazelisk/downloads/sha256/7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694/bin/bazel`,
after verifying its SHA-256
`7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694`.
Run exactly one command with startup flags `--batch --ignore_all_rc_files
--output_base=/tmp/slug-m7a-configured-cquery-output-base` and command:

```text
cquery --repository_disable_download --lockfile_mode=off --noexperimental_collect_system_network_usage --output=label 'deps(//app/slug_cli_v2:slug)'
```

Leave `CARGO_BAZEL_REPIN`, `REPIN` and generator URL/SHA overrides unset.
Set `CARGO_BAZEL_ISOLATED=false`, `CARGO_HOME=/home/wgray/.cargo`,
`CARGO_NET_OFFLINE=true` and prepend the pinned nightly-2025-09-14 Cargo
binary directory to `PATH`. Keep the restricted network environment.
The fresh output base and `--lockfile_mode=off` avoid stale extension state;
omit `--nofetch` so cached local repository content may materialize while
`--repository_disable_download` prevents repository downloads.

This full-root configured analysis is required to learn whether the
developer graph can be analyzed; a smaller owner-only cquery cannot answer
that production-root question. It is one analysis command, not a test, with
a 30-second TERM/three-second KILL ceiling and 16 MiB per-stream cap.
Record command/environment, fresh-base assertion, output hashes, exit,
elapsed time, cleanup and pre/post tracked-file hashes/status in a
machine-readable `/tmp` receipt. A cache miss, analysis error, timeout,
truncation, unexpected write or cleanup failure stops without retry,
deadline increase, build, repin or network fetch.

## Result and stop

On a clean result, preserve complete label/displayed-token rows and report
the number of root-workspace versus external configured rows, distinct
root-workspace package paths, distinct displayed seven-character tokens and
explicit `null` rows. Compare the 35 selected local package paths and named
configuration/Starlark targets without collapsing rows across displayed
tokens. Parse labels according to the actual cquery output, rejecting
malformed rows and keeping root `//` labels separate from external `@`
labels. Any missing selected path or analysis failure is a recorded gap,
not a license to infer absent actions or invent a fallback. A successful
`cquery` does not prove full configuration identity, target/exec
classification, feature vectors, generated inputs, actions or
buildability; those require separately reviewed inspection/comparison.

The only tracked edits allowed are this manifest, canonical plan status,
Stage 10 and bootstrap readiness for result receipts. Run `git diff --check`
and `python3 scripts/v2_plan_status.py`. Independent design review
precedes this one invocation; independent final review checks the
configured-output parsing, preserved displayed tokens and strict scope before
accepting this observation. No build, test, F3, developer-gate sweep or
BuildBuddy replay is selected.
Independent design review `ACCEPT` confirmed the output-token limits and the
runner's monotonic 30-second TERM, three-second process-group KILL and bounded
pipe drain. The single configured-root cquery is selected.
