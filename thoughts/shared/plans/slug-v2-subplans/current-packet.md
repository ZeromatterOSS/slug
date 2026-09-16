# Current Slug V2 Work Packet

Packet: WP-7-10-m7a-five-target-rustc-aquery-r1
Status: design ACCEPT; one focused offline aquery pending

## Question and frozen inputs

Determine whether the five Linux target-unit feature differences found in
the saved Cargo-versus-lock accounting actually appear in Bazel's configured
Rustc commands for the CLI root. The generated lock and checked-out crate
BUILD files are wider than the selected Cargo units for `ahash 0.8.12`,
`lalrpop-util 0.19.12`, `num-traits 0.2.19`, `relative-path 1.9.3` and
`rustix 1.1.4`. Those lists are not yet action evidence. This packet only
observes their actions; it does not compile, edit features, repin or admit
whole-closure parity or M7A.

Freeze clean main `00499bfe9`, `Cargo.lock` SHA-256
`a19882e78b50a82ed900fce570f4a6aee778553448790eaf08e2e08a591f76d8`,
`Cargo.Bazel.lock` SHA-256
`6335564ccbb3c915d0a2fa23b8aec0d69986911e062b586e13c5ec9812c2f35e`,
`MODULE.bazel` SHA-256
`109585b7b40d274ea912d32c73f54c25cc4bef1bc5e60526e18ac9bfe48d210c`,
and accepted external unit feature analysis SHA-256
`42c2aeb3e1e0624ef705fcab21e05d57037151913ef25fdc0e65c493194a1f7f`.
The accepted full-root cquery receipt SHA-256
`459c41aad42c9259adc239b74271c0e7b380bb6970135e4e266ee91e13b322bc`
contains configured label rows for these five external crate targets, but its
short display tokens do not prove action identity. Query the CLI root itself.

## One bounded analysis-only command

Use pinned Bazel 9.2.0 in batch mode, with the existing offline output base,
verified six-file nightly distdir, nightly Rust toolchain channel,
`--repository_disable_download`, `--lockfile_mode=error` and the established
network-profiler disable flag. Run one `aquery` of
`mnemonic("Rustc.*", outputs(".*(ahash|lalrpop.util|num.traits|relative.path|rustix).*", deps(//app/slug_cli_v2:slug)))`
with `--output=jsonproto --include_param_files`. [Bazel's aquery reference](https://bazel.build/query/aquery)
documents post-analysis action inspection without execution and the
parameter-file content option. The output
filter limits saved actions, while `deps(root)` retains the root's configured
closure. Check the five exact owner labels in the returned target table;
extra regex matches do not count as evidence for a named crate.
The exact Bazel binary SHA-256 is
`7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694`;
reuse output base `/tmp/slug-m7a-configured-offline-cquery-output-base`.
The five required canonical owners from the accepted root cquery are
`@@rules_rust++crate+slug_crates__ahash-0.8.12//:ahash`,
`@@rules_rust++crate+slug_crates__lalrpop-util-0.19.12//:lalrpop_util`,
`@@rules_rust++crate+slug_crates__num-traits-0.2.19//:num_traits`,
`@@rules_rust++crate+slug_crates__relative-path-1.9.3//:relative_path`
and `@@rules_rust++crate+slug_crates__rustix-1.1.4//:rustix`.
Normalize only the leading Bazel `@`/`@@` spelling if jsonproto differs;
repo and target names must match exactly. Alias and build-script owners do
not substitute for these five.

A supervisor caps wall time at 15 seconds and stdout/stderr at 16 MiB each,
starts a separate process group, then applies TERM and a two-second KILL
cleanup on a cap. Save raw output and a receipt with exact command, exit,
elapsed time, stop reason, stream hashes/bytes, output-base status, tracked
before/after status and frozen hashes. One failed, capped, truncated or
unparseable attempt stops this packet; no retry or broader query.

On exit 0, require a Rustc action and complete arguments/param
file content for each of the five exact configured owners. `Rustc` and
`RustcMetadata` actions for one owner are acceptable only when their feature
sets agree; otherwise the owner is ambiguous. Record configuration IDs and
parse only each
`--cfg feature=...` set and compare with the corresponding Cargo Linux unit
and generated lock list. Record extra/missing features and configuration/owner
IDs, without claiming compilation or full graph parity. If any owner is
absent, has ambiguous actions or hides feature arguments, report
that as a gap and stop. No BUILD or lock edit is authorized by this packet.

The tracked allowlist is this manifest, canonical plan status, Stage 10 and
bootstrap readiness. Independent design review precedes the one command;
independent final review checks raw action ownership and flags. Other host
units, generated inputs, execution, compilation and M7A remain open.
Independent design review initially required exact canonical owners,
configuration IDs and agreement between Rustc/RustcMetadata feature flags,
plus pinned binary/output-base paths. The corrected one-command design was
independently re-reviewed `ACCEPT`.
