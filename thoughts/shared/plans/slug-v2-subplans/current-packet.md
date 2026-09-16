# Current Slug V2 Work Packet

Packet: WP-7-10-m7a-bazel-configured-root-nightly-flag-r1
Status: flag-corrected configured-root cquery design ACCEPT; one query selected

## Outcome and prior evidence

Obtain one authentic Bazel 9.2 configured dependency observation for
`//app/slug_cli_v2:slug` with the repository's registered nightly Rust
toolchain selected. This corrects only the omitted invocation setting of
`WP-7-10-m7a-bazel-configured-root-cquery-r1`. It changes no Slug semantic
owner, BUILD graph, lockfile, toolchain registration or compatibility class.
The view is diagnostic configured analysis, not a Slug behavior admission:
full configuration identity, target/exec classification, feature vectors,
generated inputs, action coverage, compilation, REAPI and M7A remain open.

The predecessor's only cquery exited 1 in 2.862 seconds with zero stdout,
clean cleanup and unchanged tracked hashes/status. Receipt SHA-256 is
`80e255dbdffa001fa48f05d78d3b10d1ba9ed3c489a4c184e95b6238b06cd4d9`;
stderr SHA-256 is
`982df292f41c8faaa362e7cea9f521a9a06a4fe080ee52f29541387c609bf3ce`.
Bazel stopped at Rust toolchain resolution. `MODULE.bazel` registers only
`nightly/2025-09-14`, the generated registered toolchains require
`@rules_rust//rust/toolchain/channel:nightly`, and pinned rules_rust defaults
the channel to stable. Existing developer-gate wrappers set
`--@rules_rust//rust/toolchain/channel=nightly`. Independent result review
confirmed this invocation attribution and the flag-only successor; the prior
failure provides no configured reachability evidence.

Freeze clean main `848ad9ed5`, root `Cargo.lock` SHA-256
`a19882e78b50a82ed900fce570f4a6aee778553448790eaf08e2e08a591f76d8`,
`Cargo.Bazel.lock` SHA-256
`6335564ccbb3c915d0a2fa23b8aec0d69986911e062b586e13c5ec9812c2f35e`,
`MODULE.bazel.lock` SHA-256
`bcdda78dd82aab5fd7ab1796985a78322b50178dceed7669c1b20173c2634302`,
selected Linux Cargo analysis SHA-256
`dad20c2240aa421f5d99fe30190cbe7fa9035b4e388ad64aa30e762afaf9cd10`,
accepted declared-root query receipt/analysis SHA-256 values
`1935a3d026a6a31566d6250ac2fc49747b1f08de04d36819984e9dd34ae89b8a`
and `f734391152ac608a0787b7443575b6fe3253294b8211494ba161d6f1545f0b13`,
and the predecessor receipt/stderr above.

## One bounded invocation correction

Assert every frozen hash, a clean working tree and absent
`/tmp/slug-m7a-configured-nightly-cquery-output-base`. Verify the pinned
Bazel 9.2.0 executable SHA-256
`7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694`.
Use that absolute executable at
`/home/wgray/.cache/bazelisk/downloads/sha256/7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694/bin/bazel`.
Run exactly one command with startup flags `--batch --ignore_all_rc_files
--output_base=/tmp/slug-m7a-configured-nightly-cquery-output-base` and command:

```text
cquery --repository_disable_download --lockfile_mode=off --noexperimental_collect_system_network_usage --@rules_rust//rust/toolchain/channel=nightly --output=label 'deps(//app/slug_cli_v2:slug)'
```

This is the predecessor's exact command with only the nightly-channel flag
added and a new output base. Leave `CARGO_BAZEL_REPIN`, `REPIN` and generator
URL/SHA overrides unset; set `CARGO_BAZEL_ISOLATED=false`,
`CARGO_HOME=/home/wgray/.cargo`, `CARGO_NET_OFFLINE=true` and prepend the
pinned nightly-2025-09-14 Cargo binary directory to `PATH`. Keep the
restricted network environment, no `--nofetch`, and
`--repository_disable_download` so only cached repository inputs may
materialize. No build, test, network fetch, repin or action runs.

The one full-root analysis has a 30-second TERM/three-second process-group
KILL ceiling and a finite pipe-drain deadline, with 16 MiB per captured
stream. This is the minimum full-root configured observation; a smaller
owner-only cquery cannot establish CLI-root analysis. Record the exact
command/environment, output hashes, exit/elapsed time, process cleanup,
pre/post lock and seven BUILD hashes/status in a machine-readable `/tmp`
receipt. On cache miss, analysis failure, timeout, truncation, unexpected
tracked write or cleanup failure, record the typed gap and stop without
another attempt or longer ceiling.

## Result gate

On success, preserve every raw label/displayed-token row. Bazel 9.2 label
output shows seven-hex configuration-checksum prefixes or `null`, not full
unique identities or target/exec classification. Parse actual rows without
collapsing across tokens and report root-workspace/external row counts,
distinct root package paths and displayed tokens, null rows, the named
configuration/Starlark labels and reachability of all 35 Cargo-selected
local package paths. Any missing path is a gap. Configured label reachability
alone does not establish features, generated inputs, actions, buildability,
Slug parity or M7A.

The only tracked edits allowed are this manifest, canonical plan status,
Stage 10 and bootstrap readiness. Run `git diff --check` and
`python3 scripts/v2_plan_status.py`. Independent design review precedes the
single invocation and independent final review checks the receipt, preserved
rows and strict interpretation. The next capability decision requires a
separate packet.
Independent design review `ACCEPT` confirmed the exact flag-only command
delta, frozen failed receipt, bounded runner and strict label-token evidence
limit. The single follow-up cquery is selected.
