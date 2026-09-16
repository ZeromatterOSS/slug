# Current Slug V2 Work Packet

Packet: WP-7-10-m7a-three-build-script-feature-cfg-source-r1
Status: source audit ACCEPT; ahash/rustix named-feature paths bounded, num-traits open

## Question and authority

Determine whether the three unresolved generated `_bs.flags` inputs in the
accepted five-target Rustc aquery can add *named* `feature="..."` cfg values.
The prior packet was `REPLAN`: its five direct Rustc feature sets were
accepted, but `_bs.flags` contents were unavailable without executing build
scripts. This successor audits exact pinned sources only. It does not run a
build script, Bazel command, Cargo command or test, and cannot prove all
runtime flag bytes or compilation.

Freeze clean main `15945268a`, `Cargo.lock` SHA-256
`a19882e78b50a82ed900fce570f4a6aee778553448790eaf08e2e08a591f76d8`,
`Cargo.Bazel.lock` SHA-256
`6335564ccbb3c915d0a2fa23b8aec0d69986911e062b586e13c5ec9812c2f35e`,
`MODULE.bazel` SHA-256
`109585b7b40d274ea912d32c73f54c25cc4bef1bc5e60526e18ac9bfe48d210c`,
and accepted five-action analysis SHA-256
`0f0aaea140a1fda7b15e876cf91c5b6f651f95de06d0468311df4905d37e091f`.
The three actual Bazel external-repository `build.rs` inputs match the
cached Cargo source bytes: ahash 0.8.12 SHA-256
`d7dd5428c78b80bb3c99068561641ec661f0f94defbda17f85b443e358ab6396`,
num-traits 0.2.19 SHA-256
`d3969209fc1c9d201c66ed11820d0b328600d75b3971f8ceebeab04900bc0587`,
and rustix 1.1.4 SHA-256
`74cb32e64aa6fe99c2496a425b016e22f4e43c438a8237966b8acae04a98eaf9`.
The pinned rules_rust build-script runner `lib.rs` SHA-256 is
`54e90953ecd2016994b08fc51330b75cf9cdaf155c27163229f7bb506070ce4b`.
It maps emitted `cargo:rustc-cfg=...` to `--cfg=...` in `_bs.flags`, including
`feature=...` if a script emits it.

## Bounded source audit

- Read each entire actual Bazel `build.rs` and check for `include!`, modules,
  subprocess stdout forwarding, `rustc-flags` or arbitrary `rustc-cfg`
  construction. Enumerate every reachable build-script output path and its
  possible `rustc-cfg` values, without assuming which environment branches
  execute. The `version_check 0.9.5` helper used by ahash is pinned at
  `01bb86088ba281d511ae002aa939bb30b747f47ace5ea13a46de554a3117806e`;
  the `autocfg 1.5.1` helper used by num-traits is pinned at
  `772630d0e09d06f343fd72284c8330eacde87d2a4ee22f70af62a0e2119fbf05`.
  Include all compiled helper modules: `version_check` `channel.rs`
  `bb3eae79aaf224591f1e9dddedcabbea3fc01bc6c06eafedcd87d7b770a35aca`,
  `version.rs` `dba18a25983ec6e37b952f4cdc5219c9e5abba2c3a76cef87465e1fba6f8ac89`,
  `date.rs` `09580a0a2008fad2ccbc43fb42a88f42221b98b01692702022a296dc9c86bf37`;
  `autocfg` `rustc.rs`
  `a8a213ddb64a05c1a1af933bcb331a98879e942b167c33d8f94f9f60ebb14e29`,
  `version.rs` `4f7d23b36f01c7be1871be86c038d6cb4689e145d67c82d3793690e9aa05b133`,
  and `error.rs`
  `fd8ff67c64f7cd1b9f81325a81de4baa34c39d6ae298bdb33f9829cc91acac39`.
  Both libraries' `tests` modules are behind `cfg(test)` and excluded from
  these build-script dependencies. Inspect the compiled helper source
  closure, called implementations and subprocess output capture for any
  output side effects.
- Classify only whether any of these pinned source paths can emit a named
  `rustc-cfg=feature=...` or `rustc-flags` containing such a cfg. If no,
  combine that source-owned bound with the prior five direct action sets to
  close the **named feature cfg set** for those five actions. Leave actual
  generated cfg values, complete argument bytes, build-script execution and
  compilation open. If a path is dynamic or opaque, record it as a gap.
- Save a concise deterministic source-audit receipt under `/tmp` with all
  hashes, output paths and source anchors. Independent final review checks
  every emit path and that the claim does not expand beyond named features.

The tracked allowlist is this manifest, canonical plan status, Stage 10 and
bootstrap readiness. No source, BUILD or lockfile edit is authorized.
Independent design review precedes the audit; independent result review
precedes acceptance. Other host-unit flags, generated input bytes, action
execution, compilation and M7A remain open.
Independent design review first requested pins for all six compiled helper
modules omitted from the initial draft. The corrected complete source
closure was independently re-reviewed `ACCEPT`.

## Result

Deterministic source analysis SHA-256
`0befd1d6b4187984859ee3a07769a50ff314f092217d8b1d303f5bc14251bb17`
records all 12 pinned file hashes, actual Bazel/Cargo build-script byte
matches and output paths. `ahash` can emit bare `specialize` and
`folded_multiply` cfgs; its `version_check` child stdout is captured rather
than forwarded. `rustix` emits only fixed bare cfg names and redirects its
probe child's stdout/stderr to null. Neither source path can add a named
`feature=...` cfg to `_bs.flags` under these pinned inputs.
`num-traits` explicitly calls `autocfg` with `has_total_cmp`, but its probe
child inherits stdout and may use environment-selected rustc wrappers.
That inherited output is not bounded by this source audit, so additional
`cargo:rustc-cfg=feature=...` text cannot be ruled out. Independent final
review `ACCEPT` rehashed and inspected every source path and retained this
partial conclusion. No build script, Cargo/Bazel command or test ran. The
five-action named-feature closure, actual generated bytes, compilation and
M7A remain open pending a narrow num-traits environment/output observation.
