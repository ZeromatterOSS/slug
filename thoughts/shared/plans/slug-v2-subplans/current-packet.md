# Current Slug V2 Work Packet

Packet: WP-7-10-m7a-rust-toolchain-pin-and-cache-r1
Status: exact Linux x86_64 toolchain pin/cache result ACCEPT; configured root open

## Outcome and evidence

Pin and stage the six official 2025-09-14 nightly archives required by the
registered Linux x86_64 Rust toolchain, then materialize that single Bazel
toolchain-tools repository from verified local archives. This is exact
toolchain-content provenance for this host triple, not a Slug semantic
change. Other target/exec triples, configured-root reachability, generated
inputs, compilation, actions and M7A remain unsupported/deferred by this
packet. No cquery, build or test is selected.

The accepted static BUILD/lock and unconfigured root-query evidence remains
at main `05233f540`. The sole flag-corrected configured cquery exited 1 in
2.933 seconds with zero stdout, clean cleanup and identical pre/post tracked
hashes/status. Receipt SHA-256 is
`9f305306861c887bffc9ca6dbde4c1930cf8733cfa91cbe97b41c3b55364ab5c`;
stderr SHA-256 is
`c6fcc183bade96a355484028c4e7cc557ff34a4f4450deacf457d40daa514f0d`.
Its registered nightly toolchain selected, but the Linux x86_64 tools
repository called `ctx.download_and_extract` for an unavailable compiler
archive while downloads were disabled. Independent final review confirmed a
typed cache miss, not configured graph evidence or a BUILD/lock defect.

Pinned rules_rust 0.73 `rust/extensions.bzl` accepts `sha256s` on
`rust.toolchain`; `rust/private/repository_utils.bzl` uses each matching
archive hash for `ctx.download_and_extract`. Its built-in hash table has no
2025-09-14 entry. The extension's default `rustfmt_version` resolves to
`nightly/2026-07-16`, so set it explicitly to `nightly/2025-09-14` alongside
the already selected compiler date. Do not add a local compiler override or
infer that an installed rustup tree is a verified Bazel archive.

The official dated manifest is
`https://static.rust-lang.org/dist/2025-09-14/channel-rust-nightly.toml`.
Its official `.sha256` sidecar matches manifest SHA-256
`2f3d96c78c69647d0dc209aae553150793ec10988fa1011767c3990915173e01`.
The parsed six-component Linux x86_64 map is saved at
`/tmp/slug-m7a-rust-nightly-2025-09-14-x86_64-sha-map.json`, SHA-256
`31fd7e74c0d9fed2744f2cadcf3a1ae6961e081255ac8f8516750f4ea0edf8cb`:

| Dated `.tar.xz` archive basename | Official SHA-256 |
|---|---|
| `cargo-nightly-x86_64-unknown-linux-gnu` | `cbc76e93946fb4658b692f9d71dbd4861f942b5b2bf6e34b072ba708d00e89e8` |
| `clippy-nightly-x86_64-unknown-linux-gnu` | `98bb5375ef6648d6e59fe8558e12df854e73653db2b6c7ee8891ffa0336f7622` |
| `llvm-tools-nightly-x86_64-unknown-linux-gnu` | `7302ec9a0447d5e21d918428b629a1d76ff934a562660e19b275fad707cf83dc` |
| `rust-std-nightly-x86_64-unknown-linux-gnu` | `9a0a34cc7a11eb74f408ab7a5eafd063bcce3726c9bb89351ed7f2811fa35b1a` |
| `rustc-nightly-x86_64-unknown-linux-gnu` | `0aa143cf401827874af7114f6466c0c87d4e31dfd6a2355c50db2e0c71bd45c7` |
| `rustfmt-nightly-x86_64-unknown-linux-gnu` | `d9206e55ee07ae3ba9de7190142a8e422a72f300726fa8e094a6300c7128aa82` |

Each `sha256s` key has prefix `2025-09-14/` and suffix `.tar.xz`; each URL
uses the official manifest `xz_url`. The table is the exact allowlist, not
permission to download a full toolchain matrix.

## Bounded work

1. Freeze clean main and root `Cargo.lock` SHA-256
   `a19882e78b50a82ed900fce570f4a6aee778553448790eaf08e2e08a591f76d8`,
   `Cargo.Bazel.lock` SHA-256
   `6335564ccbb3c915d0a2fa23b8aec0d69986911e062b586e13c5ec9812c2f35e`,
   `MODULE.bazel.lock` SHA-256
   `bcdda78dd82aab5fd7ab1796985a78322b50178dceed7669c1b20173c2634302`,
   official manifest/map hashes and the failed-query receipt. Add only the
   explicit rustfmt date and six `sha256s` entries to `rust.toolchain` in
   `MODULE.bazel`.
2. Run one pinned Bazel 9.2 `mod deps` from a fresh output base, with
   `--repository_disable_download --noexperimental_collect_system_network_usage
   --lockfile_mode=update`, no repin and offline Cargo. Cap it at 30 seconds
   TERM/three-second KILL with finite pipe drain and 16 MiB streams. Accept
   `MODULE.bazel.lock` changes only if parsed diff belongs to the rust
   toolchain extension and frozen Cargo/Cargo.Bazel bytes and seven BUILD
   hashes remain exact. Any other write/failure stops; preserve the receipt.
3. Fetch only the six exact official `.tar.xz` URLs into a new `/tmp`
   distribution directory using a single supervised acquisition under a
   90-second absolute ceiling. Verify every complete file against the
   manifest SHA before admitting it; a missing, partial or mismatched file
   stops and never enters the verified directory. Record URL, byte count,
   expected/actual digest, elapsed time and cleanup in a `/tmp` receipt.
4. Run one pinned Bazel `fetch
   --repo=@@rules_rust++rust+rust_linux_x86_64__x86_64-unknown-linux-gnu__nightly_tools`
   from a fresh output base with `--distdir` pointing only to those verified
   files, `--repository_disable_download`, `--lockfile_mode=error`, the
   profiler workaround and offline Cargo. Cap repository materialization
   at 60 seconds TERM/three-second KILL and 16 MiB streams. Verify the
   selected repository exists and record exact pre/post lock, BUILD and
   status hashes. A cache miss or any unexpected mutation stops; do not
   retry with network-enabled Bazel or a longer ceiling.

The only tracked edits allowed are `MODULE.bazel`, its reviewed lockfile
update, this manifest, canonical status, Stage 10 and bootstrap readiness.
No action/test, full dependency fetch, cquery or BuildBuddy replay is
selected. `git diff --check` and `python3 scripts/v2_plan_status.py` are
required. Independent design review precedes edits/acquisition and
independent final review checks pinned mapping, exact archive digests,
lockfile delta, receipts and strict scope. Only a separately selected
successor may attempt offline configured-root analysis.
Independent design review `ACCEPT` verified the official six-archive hash
mapping, rules_rust component and rustfmt-date behavior, and Bazel's
checksum-keyed distdir lookup before its download-disabled stop. The bounded
pin/cache work is selected.

## Observed result

`MODULE.bazel` now pins the six reviewed SHA-256s and the matching
`nightly/2025-09-14` rustfmt version. A static comparison exactly matched
all six entries to the sidecar-verified official manifest. The sole offline
`bazel mod deps` exited 0 in 2.068 seconds; `MODULE.bazel.lock`, both Cargo
locks, seven BUILD files and Git status stayed unchanged. Its receipt SHA-256
is `b55177b30806613e15c7827154c2cd6615dd5554d50038df1961b8311bbdd60c`.

The one supervised official acquisition completed in 1.627 seconds,
downloading 171,520,576 bytes across exactly six `.tar.xz` files. Every
archive SHA-256 matched the frozen manifest map before the verified distdir
was admitted. Acquisition receipt SHA-256 is
`c84576af16f7c46bac05eb878d052b2196278473e32a280db9e2219b072162e2`.
The initial Bazel fetch launcher rejected `--distdir` as a startup option
before making an output base or repository request (exit 2, 0.022 seconds);
its receipt SHA-256 is
`65e69361f3592520c062f6e3edb362f852a86541b49f13ebf5eb65d6bcd20547`.
The invocation-only correction placed that option after `fetch`. Its sole
repository materialization exited 0 in 11.147 seconds with the selected
Linux x86_64 tools repository present and generated `iso_date = "2025-09-14"`.
Download-disabled Bazel had the verified distdir and its checksum-keyed cache
available. All tracked hashes
and status remained exact; recovery receipt SHA-256 is
`1a47242a21971bbd5b63ae5d21f49f9a7a079d916643e8b02e187ef4ab7e8b10`.
No cquery, build, test, Slug behavior or M7A gate ran or closed.
Independent final review `ACCEPT` rehashed all six official archives,
confirmed the exact MODULE delta, both fetch receipts and the materialized
repository, and retained the strict configured-root and M7A boundaries.
