# Current Slug V2 Work Packet

Packet: WP-7-10-m7a-bazel-configured-root-offline-r1
Status: scoped offline configured-root cquery design ACCEPT; one query selected

## Outcome and authority

Obtain one authentic Bazel 9.2 configured dependency view of the full
`//app/slug_cli_v2:slug` root after the reviewed graph/lock sync and exact
Linux x86_64 Rust toolchain cache preparation. This is read-only oracle
analysis, with no Slug semantic owner or compatibility-class change.
Configured label reachability may inform the first M7A readiness row, but
does not admit Cargo feature parity, generated inputs, action families,
compilation, REAPI, self-hosting or M7A. Other target/exec platform
toolchains remain unsupported/deferred by the cache packet.

The earlier unconfigured query produced 18,846 valid labels and all 35
Cargo-selected local package paths; its receipt SHA-256 is
`1935a3d026a6a31566d6250ac2fc49747b1f08de04d36819984e9dd34ae89b8a`.
The first configured cquery omitted the nightly setting and emitted zero
rows; receipt SHA-256 is
`80e255dbdffa001fa48f05d78d3b10d1ba9ed3c489a4c184e95b6238b06cd4d9`.
The flag-corrected cquery selected the nightly toolchain but stopped at an
uncached Rust compiler archive; receipt SHA-256 is
`9f305306861c887bffc9ca6dbde4c1930cf8733cfa91cbe97b41c3b55364ab5c`.
No configured-root coverage was inferred from either failure.

Accepted checkpoint `54a206095` pins official SHA-256s for the six Linux
x86_64 nightly-2025-09-14 `.tar.xz` components and the same-date rustfmt
version in `MODULE.bazel`. The sidecar-verified release manifest SHA-256 is
`2f3d96c78c69647d0dc209aae553150793ec10988fa1011767c3990915173e01`.
The exact six-file acquisition receipt SHA-256 is
`c84576af16f7c46bac05eb878d052b2196278473e32a280db9e2219b072162e2`;
the download-disabled selected toolchain repository fetch receipt SHA-256 is
`1a47242a21971bbd5b63ae5d21f49f9a7a079d916643e8b02e187ef4ab7e8b10`.
That fetch exited 0 with a materialized Linux x86_64 tools repo and
unchanged tracked files. It did not run configured analysis.

Freeze clean main `54a206095`, pinned Bazel executable SHA-256
`7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694`,
root `Cargo.lock` SHA-256
`a19882e78b50a82ed900fce570f4a6aee778553448790eaf08e2e08a591f76d8`,
`Cargo.Bazel.lock` SHA-256
`6335564ccbb3c915d0a2fa23b8aec0d69986911e062b586e13c5ec9812c2f35e`,
`MODULE.bazel` SHA-256
`109585b7b40d274ea912d32c73f54c25cc4bef1bc5e60526e18ac9bfe48d210c`,
`MODULE.bazel.lock` SHA-256
`bcdda78dd82aab5fd7ab1796985a78322b50178dceed7669c1b20173c2634302`,
selected Linux Cargo analysis SHA-256
`dad20c2240aa421f5d99fe30190cbe7fa9035b4e388ad64aa30e762afaf9cd10`,
the prior query/acquisition/fetch receipts and every verified archive hash.

## One necessary configured-root query

Assert all frozen hashes, a clean tree and an absent
`/tmp/slug-m7a-configured-offline-cquery-output-base`. Use the absolute
pinned Bazel executable at
`/home/wgray/.cache/bazelisk/downloads/sha256/7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694/bin/bazel`.
Run exactly one invocation with startup flags `--batch --ignore_all_rc_files
--output_base=/tmp/slug-m7a-configured-offline-cquery-output-base` and command:

```text
cquery --distdir=/tmp/slug-m7a-rust-nightly-2025-09-14-distdir --repository_disable_download --lockfile_mode=error --noexperimental_collect_system_network_usage --@rules_rust//rust/toolchain/channel=nightly --output=label 'deps(//app/slug_cli_v2:slug)'
```

The verified six-file distdir is the only extra archive source; Bazel's
checksum-keyed local repository cache may also satisfy the pinned requests.
Do not infer which one Bazel used without explicit evidence. Leave
`CARGO_BAZEL_REPIN`, `REPIN` and generator URL/SHA overrides unset; set
`CARGO_BAZEL_ISOLATED=false`, `CARGO_HOME=/home/wgray/.cargo`,
`CARGO_NET_OFFLINE=true` and prepend the pinned nightly Cargo directory to
`PATH`. Keep the restricted network environment. No fetch with network,
repin, build, action or test is selected.

Full-root configured analysis is needed to test the production root;
owner-only cquery cannot answer this graph question. Cap the one command at
30 seconds TERM/three-second process-group KILL with a finite drain deadline
and 16 MiB per captured stream. Record exact command/environment,
output hashes, exit/elapsed time, cleanup, pre/post lock, MODULE and seven
BUILD hashes/status in a machine-readable `/tmp` receipt. A cache miss,
analysis failure, timeout, truncated output, unexpected tracked write or
cleanup failure is a typed gap and stops without retry or deadline increase.

## Result and stop

On success, preserve complete raw label/displayed-token rows. Bazel 9.2
`cquery --output=label` displays seven-hex configuration checksum prefixes
or `null`; these are not full, guaranteed-unique identities and do not
classify target versus exec. Parse actual rows without collapsing tokens;
report root-workspace and external row counts, distinct root package paths,
displayed tokens and null rows. Compare the named configuration/Starlark
labels and all 35 selected local Cargo package paths, recording missing
paths as gaps. A successful query still does not establish features,
generated inputs, actions, buildability, Slug parity or M7A.

The only tracked edits allowed are this manifest, canonical plan status,
Stage 10 and bootstrap readiness for the result. Run `git diff --check` and
`python3 scripts/v2_plan_status.py`. Independent design review precedes
the query; independent final review checks the receipt, parsing and strict
scope. No follow-on command is selected by this packet.
Independent design review `ACCEPT` confirmed the full-root command, strict
label-token interpretation and a preflight that requires exactly six
regular, hash-verified distdir archives while freezing both prior failures.
The one offline configured-root cquery is selected.
