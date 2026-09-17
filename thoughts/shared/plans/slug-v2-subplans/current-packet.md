# Current Slug V2 Work Packet

Packet: WP-7-16-m7a-reapi-adapter-bazel-compile-r1
Status: accepted partial observation; cache leaf Rustc built, adapter blocked by embedded Bazel inputs

## Outcome and compatibility

Build the existing `//app/slug_reapi_v2:slug_reapi_v2` Bazel library target.
Its declared direct dependency on `//app/slug_reapi_cache_v2` makes one
successful build cover both first-party Rustc actions after WP-7-15's
generated-proto materialization. This is buildability evidence only: it changes
no Slug semantic owner or compatibility class and cannot prove the CLI root,
runtime REAPI behavior, or M7A readiness.

WP-7-15 at `03455689d` built the cache leaf's proto build script in 31.36
seconds and inspected all six nonempty generated Rust modules. The library
Rustc action and its direct adapter consumer still need a live compile.

## Bounded build and decision

The checked-in leaf/adapter Rust sources, BUILD rules, Cargo/Bazel locks,
`MODULE.bazel`, pinned rules_rust 0.73 and nightly 2025-09-14 toolchain own
this target. Bazel's output is command scratch, never a Slug DICE fact or a
semantic compatibility oracle. Preserve the four input locks and first-party
sources exactly; no implementation edit or test is selected here.

From clean main, run one `bazel --ignore_all_rc_files --batch build` of this
one target with `--@rules_rust//rust/toolchain/channel=nightly` and
`--lockfile_mode=error`. Bound compilation at 60 seconds with TERM/short KILL
cleanup and a 128 MiB output-file cap. Record elapsed time, exit status,
stdout/stderr digests, Bazel action/target result and tracked status. If it
succeeds, inspect the target's output and the named cache-leaf dependency
output. A timeout or compiler error is not success; record the precise
failure, do not repeat an unchanged command, and select a corrective successor.
This is a compilation gate, not a test run. The prior WP-7-14 graph already
proves the declared consumer edge; this packet checks execution of Rustc.

Validation is the build receipt, source/lock status, plan status checker and
`git diff --check`. Allowlist: this manifest, canonical Live Status, Stage 10's
live inventory delta, and bootstrap-readiness's first row. Commit and push a
reviewable result. Do not broaden to the CLI root or tests in this packet.

## Build receipt (2026-09-16)

With only packet documentation modified from main `03455689d`, the sole build
used a 128 MiB file cap and this command:

```text
/usr/bin/time -f 'elapsed_seconds=%e\nexit_status=%x' -o target/wp716/time.txt timeout -k 3s 60s bazel --ignore_all_rc_files --batch build --lockfile_mode=error --@rules_rust//rust/toolchain/channel=nightly //app/slug_reapi_v2:slug_reapi_v2 > target/wp716/stdout.txt 2> target/wp716/stderr.txt
```

It exited 1 in 42.34 seconds, before the 60-second cap. Bazel reported 961
processes (216 internal, 745 linux-sandbox) and 395 action-cache hits. The new
cache leaf Rustc output exists at
`bazel-bin/app/slug_reapi_cache_v2/libslug_reapi_cache_v2-1303892456.rlib`;
the adapter output does not. The first failure is the separate
`//app/slug_bzlmod_v2:slug_bzlmod_v2` Rustc action: 48 `include_bytes!` paths
under `builtin/bazel_tools/` are absent from its compile sandbox. All 49
pinned catalog files exist in the checkout; the sole file visible to Rustc is
`MODULE.bazel`. The other 48 fall below nine nested upstream `BUILD` package
boundaries, which the parent `compile_data = glob(["builtin/**"])` cannot
cross. A 1.1-second read-only Bazel visibility query for a nested source also
failed while loading its verbatim upstream BUILD because it refers to labels
relative to the upstream Bazel repository, not this checkout.

Stdout is empty (SHA-256
`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`);
the 28,406-byte stderr SHA-256 is
`aa958f442a9cdcb5b4d2ae096865eae548597eb355bf00865d2412dfaee3f948`.
The ignored `target/wp716/` directory retains the streams and timing. No
tracked source, BUILD, module or lock input changed and no test ran. This
proves the leaf library Rustc action, but not the adapter; the next packet
must supply all 49 verbatim catalog files as declared Bazel compile inputs
without changing their bytes or weakening the exact catalog assertion.
