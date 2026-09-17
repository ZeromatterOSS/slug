# Current Slug V2 Work Packet

Packet: WP-7-19-m7a-external-build-script-flags-r1
Status: accepted; five exact generated-flag artifacts add no named feature cfg

## Outcome and compatibility

Classify the actual generated `_bs.flags` bytes for the `ahash 0.8.12`,
`num-traits 0.2.19`, and `rustix 1.1.4` Rustc actions reached by the Bazel
CLI root. The accepted earlier aquery found these files as Rustc inputs but
could not inspect their bytes; a 20-second probe of the `num-traits` producer
timed out before generation. WP-7-18 at `8f3ddef8b` subsequently completed
the pinned Bazel CLI-root build, materializing the artifacts. This packet
changes no Slug semantics, compatibility class, source, build rule or test.
It can close only these generated-flag byte uncertainties, not all external
feature equivalence, Cargo/Bazel pin correspondence or M7A readiness.

## Authority and bounded analysis

The pinned rules_rust 0.73 crate-universe actions and external crate sources
own the build-script output; the Bazel output tree is command scratch. Reuse
the WP-7-14 saved CLI-root `aquery` JSON only after checking its SHA-256 and
that Cargo/Bazel/module locks and the nightly toolchain did not change since
that observation. Resolve exact path-fragment artifact IDs, producer action
owners and Rustc input-depset membership for all target and exec variants.
Then inspect the five materialized output files after WP-7-18's successful
root build. Record exact lengths, SHA-256, ordered lines, target/exec equality
where both exist, and whether any line supplies a named `feature=...` cfg.
Do not infer compiler flags from a source audit alone or treat an unobserved
artifact as empty. No Bazel/Cargo command, build or test is selected.

If the saved graph fails verification, a named file is missing, or its
producer/consumer path cannot be established, stop without a feature claim
and select a new observation. An exact positive result establishes only these
five generated file bytes and their saved configured consumer edges. Keep
analysis scratch under ignored `target/wp719/`. Allowlist: this manifest,
canonical Live Status, Stage 10's external-feature inventory delta and the
first bootstrap-readiness row. Run the plan status checker and
`git diff --check`; commit and push the reviewable result.

## Artifact receipt (2026-09-16)

The saved WP-7-14 aquery JSON still hashes to
`ffc50a44648f85b464988eb5dd6c6dd152269f342121a006839d3e8c7b4c0be8`.
`git diff --quiet 757a96799..8f3ddef8b -- Cargo.lock Cargo.Bazel.lock
MODULE.bazel MODULE.bazel.lock rust-toolchain` exited 0. A 0.12-second local
parser resolved each exact artifact ID to one `CargoBuildScriptRun` producer
and one Rustc consumer through the saved input depsets, then read the
materialized files from WP-7-18's completed root build. Its full analysis
SHA-256 is
`d57bac183041db61fbdc4718a8d2ca83d05c897034ba8b0df033f551f8d025df`;
the ignored `target/wp719/` directory retains the parser and JSON.

| Crate | Configured artifacts | Bytes and SHA-256 | Ordered generated cfg names |
|---|---|---|---|
| `ahash 0.8.12` | target artifact 13238 | 38; `3166cce2d635b5c220ed87c5f9290b2a66a39607000f2344da11d00754211378` | `specialize`, `folded_multiply` |
| `num-traits 0.2.19` | target 7672, exec 17909; byte-equal | 19; `c80a44241bc21275b2dcda15f48d51a5d4b6301757b03a36cd0a0ae405b389cd` | `has_total_cmp` |
| `rustix 1.1.4` | target 6917, exec 16786; byte-equal | 154; `6fbbe78e3023f9cb7ed2866355d4fcc1f2e4dbf213bf355286202d2fe18b27a9` | `static_assertions`, `lower_upper_exp_for_non_zero`, `rustc_diagnostics`, `linux_raw_dep`, `linux_raw`, `linux_like`, `linux_kernel` |

Every line is `--cfg=<named non-feature cfg>`; none contains `feature=`.
The two target/exec pairs compare equal byte-for-byte. This resolves the
previously opaque generated-flag byte input for these named actions only.
It does not establish all external crate feature equivalence, every compiler
argument, or M7A readiness. No Bazel/Cargo command, build or test ran in this
packet, and no tracked source or lock input changed.
