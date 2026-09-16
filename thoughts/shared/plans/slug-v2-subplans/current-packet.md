# Current Slug V2 Work Packet

Packet: WP-7-10-m7a-bazel-graph-sync-r2
Status: independent design review ACCEPT; metadata mutation pending

## Outcome and authority

Repair the Bazel developer graph metadata for the frozen Linux CLI Cargo
closure: add the missing `slug_configuration_v2` library owner and its six
normal first-party incoming edges, then regenerate `Cargo.Bazel.lock` from
unchanged Cargo authority. This packet does not build or query Bazel targets
and admits no declared/configured reachability, action behavior, exact
ActionKey or M7A milestone. Those need separately reviewed live evidence.

Freeze clean main `401b6d953`, root `Cargo.lock` SHA-256
`a19882e78b50a82ed900fce570f4a6aee778553448790eaf08e2e08a591f76d8`,
the accepted Cargo Linux-closure analysis SHA-256
`dad20c2240aa421f5d99fe30190cbe7fa9035b4e388ad64aa30e762afaf9cd10`,
and accepted locked-cache fetch/metadata receipt SHA-256 values
`861309ac82fcf02d8d8385b25622757ea0f4c04ffd4e568122532db0031f31b6`
and `f5163a1827114cdc71acc9c325369db93f23a3cbc807cf821203e5486fbb5bea`.
The latter verification found all 448 registry archives and source directories,
their individual Cargo-lock checksums and full-workspace offline metadata.
The selected CLI Cargo closure has 35 local/310 external packages. Its static
Bazel lock has only 34/35 local and 302/310 selected external name/version
keys. The missing local lock member is `slug_starlark_v2 0.1.0`; Stage 10
lists all eight absent external keys. `slug_configuration_v2` is in the
lock but has no BUILD owner, and six normal local consumers omit its Bazel
edge. The prior Bazel query and recovery yielded zero rows.

## Mutation boundary

Create `app/slug_configuration_v2/BUILD.bazel` with one public
`rust_library(name = "slug_configuration_v2")`, a source glob, crate-universe
aliases/edition, normal external dependencies, proc-macro dependencies if
provided by crate-universe, and four explicit local dependencies:
`//allocative/allocative`, `//app/slug_identity_v2`, `//gazebo/dupe` and
`//gazebo/strong_hash`. Add `//app/slug_configuration_v2` only to the
`rust_library` dependency lists in `slug_analysis_v2`,
`slug_build_api_v2`, `slug_commands_v2`, `slug_core_v2`,
`slug_loading_v2` and `slug_server_v2`. The REAPI configuration dependency
is dev-only in the frozen Cargo resolution and is outside the production
correction. No test target is added.

The tracked-file allowlist is the new configuration BUILD file, those six
existing BUILD files, generated `Cargo.Bazel.lock` and `MODULE.bazel.lock`
only if Bazel emits it, plus this manifest, canonical status, Stage 10 and
bootstrap readiness. Do not edit any Rust source, Cargo manifest/lock,
`MODULE.bazel`, toolchain pin, fixture, target manifest, REAPI code, DICE
value or fallback. Handwritten BUILD growth should stay under 50 lines;
inspect generated locks structurally instead of applying a line cap.

## Generation and verification

Before Bazel, verify frozen hashes, static target syntax/labels and the six
normal Cargo edges. Confirm the pinned Bazel 9.2.0 executable SHA-256
`7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694`
and rules_rust 0.73.0 default `x86_64-unknown-linux-musl` cargo-bazel
generator cached at SHA-256
`482b82fc521cf3bbc5b4b3946198c0f273608ee63f1691c54c2e7efc3bece656`.
Leave generator URL/SHA override variables unset. The module's
`crate.from_cargo(isolated = True)` uses an empty generated Cargo home;
only for repin, set `CARGO_BAZEL_ISOLATED=false` and
`CARGO_HOME=/home/wgray/.cargo` to use the now-verified host cache.

Run exactly one `CARGO_BAZEL_REPIN=1` generation using that absolute Bazel
binary, startup flags `--batch --ignore_all_rc_files` and command
`mod deps --repository_disable_download --lockfile_mode=update`.
Set `CARGO_NET_OFFLINE=true` and keep the restricted network environment.
The cached rules_rust source maps repin mode `1` to
`cargo update --workspace`; therefore compare root `Cargo.lock` SHA
afterward and reject a change. The one-time generator has a 120-second
TERM/three-second KILL ceiling and 16 MiB per-stream cap. Record exact
command/environment, output hashes, elapsed time, exit, cleanup and
pre/post tracked-file status in a `/tmp` receipt. This is metadata
generation, not a test. A missing downloader input, timeout, unexpected
mutation, generator failure or changed Cargo authority stops without a blind
retry, alternate generator or network fetch.

After a clean generation, parse the generated lock and compare selected
Cargo package name/version keys: require all 35 local and 310 external keys,
including the nine previously absent keys. Review before/after versions,
sources, checksums, features and any `MODULE.bazel.lock` change; reject
unexplained drift or new authority. Static key coverage does not prove Bazel
reachability or package content parity. Run `git diff --check` and
`python3 scripts/v2_plan_status.py`. Do not run Bazel query/build, Cargo
build/test, F3, developer gates or BuildBuddy replay in this packet. The
user's test guidance favors the smallest valid checks; no test is needed
for this metadata-only correction.

Independent design review precedes mutation, and independent final review
checks generated diffs and receipts. Success closes only static graph/lock
synchronization. Select a separate focused Bazel evidence packet afterward;
it must obtain fresh generated repository content after repin while downloads
stay disabled, since `--nofetch` alone can leave stale repository content.

Independent design review `ACCEPT` confirmed the exact four local owner deps,
six normal consumer edges, cached default generator and offline host Cargo
inputs. Retain exact pre-run `Cargo.lock` bytes for restoration/reporting if
the generator changes that forbidden authority; no retry is authorized.
