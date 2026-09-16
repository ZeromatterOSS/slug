# Current Slug V2 Work Packet

Packet: WP-7-10-m7a-local-feature-flags-r1
Status: local feature-attribute correction ACCEPT; configured action/build gates open

## Outcome and authority

Make the checked-in production Bazel Rust rules expose the selected local
Cargo feature names that are currently missing, including build-script
feature environment. This is exact feature-name parity for the named local
rules under the frozen Linux CLI closure. It changes no Slug semantic owner,
external crate features, generated input contents, action-family admission,
compilation, REAPI or M7A status. Other target platforms and unobserved
feature selections remain unsupported/deferred.

Freeze clean main `9bc029215`, root `Cargo.lock` SHA-256
`a19882e78b50a82ed900fce570f4a6aee778553448790eaf08e2e08a591f76d8`,
`Cargo.Bazel.lock` SHA-256
`6335564ccbb3c915d0a2fa23b8aec0d69986911e062b586e13c5ec9812c2f35e`,
`MODULE.bazel` SHA-256
`109585b7b40d274ea912d32c73f54c25cc4bef1bc5e60526e18ac9bfe48d210c`,
selected Linux Cargo analysis SHA-256
`dad20c2240aa421f5d99fe30190cbe7fa9035b4e388ad64aa30e762afaf9cd10`,
and accepted configured-root query receipt/analysis SHA-256 values
`459c41aad42c9259adc239b74271c0e7b380bb6970135e4e266ee91e13b322bc`
and `266cf64932c2ef555ca5d36bb8d87ad95a8b0fe6a641a4c41973527f0c711dde`.
That query proved all 35 selected local package paths have a non-null
configured label row; it did not inspect their feature attributes or
execute compiler actions.

The frozen Cargo analysis has six local packages with nonempty features.
`allocative` has its 19 named features, `gazebo` has
`str_pattern_extensions`, and `strong_hash` has `num-bigint,triomphe`;
their checked-in library rule lists already match. `slug_cli_v2`,
`slug_core_v2` and `starlark_map` each enable `default` in Cargo, but their
production `rust_library` and CLI `rust_binary` rules omit it. All three
`default` definitions currently expand to empty arrays; the named
`feature="default"` cfg is nevertheless a distinct compile input.
The selected `allocative` and `starlark_map` build scripts omit their
package's activated feature lists. The other three selected local build
scripts have no enabled features.

Pinned rules_rust 0.73 `rust/private/rustc.bzl` emits each `crate_features`
value as `--cfg feature="%s"` (source SHA-256
`a7712508f50e5952f3f51e33c98acbe8ba39554e9c6f6edc26f16820d7c2a9e4`).
Its `cargo/private/cargo_build_script.bzl` maps each feature to
`CARGO_FEATURE_<NAME>=1` (SHA-256
`6147df938723ec58cef646169814c85a72fa0f74080471756e3c1891d02f4630`),
and `cargo/private/cargo_build_script_wrapper.bzl` forwards the same list
to the underlying script Rust binary (SHA-256
`4db7f9fd06ea44106813e3696ae0d28469ced3178613dd1c349836edb58ee866`).
These source-owned transformations, the accepted Cargo feature snapshot,
and exact checked-in rule inputs are the discriminating source regression;
no new oracle command or test is needed for this static correction.
Independent design review `ACCEPT` confirmed the six nonempty local feature
sets, five build-script owners, CLI binary mapping, pinned source hashes and
static-only validation boundary.

## Bounded edit and validation

- In `allocative/allocative/BUILD.bazel`, reuse one exact 19-feature list
  for its existing `rust_library` and `cargo_build_script`. Preserve order,
  spelling, dependencies and generated-source declarations.
- In `app/slug_cli_v2/BUILD.bazel`, add `crate_features = ["default"]` to
  `slug_cli_v2` and `slug`; in `app/slug_core_v2/BUILD.bazel`, add it to
  `slug_core_v2`; in `starlark-rust/starlark_map/BUILD.bazel`, add it to
  `starlark_map` and its `build_script`.
- Compare every selected local package's frozen Cargo feature set to its
  checked-in production Rust rule inputs; compare all five local build
  script feature inputs to their package sets. Record any difference as a
  gap, not an inferred match. Check the pinned rules_rust source hashes,
  `git diff --check` and `python3 scripts/v2_plan_status.py`. Do not rerun
  Cargo metadata, full-root Bazel analysis, a build or a test.

The tracked allowlist is those four BUILD files, this manifest, canonical
plan status, Stage 10 and bootstrap readiness. No Cargo or Bazel lockfile
may change. Independent design review precedes the edit; independent final
review checks exact list mapping and source anchors. This packet accepts
only local feature flag inputs, not external feature equivalence or
compilation/buildability. A later selected packet must inspect actual
configured actions and resolve any remaining external feature questions.

## Result

The four BUILD files now set exact frozen Cargo feature names on all selected
production Rust rules and build scripts. A static parse compared 35 package
sets, 36 Rust rules and five build scripts with zero mismatches; the six
nonempty package sets and empty sets for the other 29 packages match exactly.
The three pinned rules_rust source hashes, `git diff --check` and plan status
passed. No Cargo metadata, Bazel command, build or test ran. Independent final
review `ACCEPT` repeated the 41-target comparison, checked the allowed diff
and frozen Cargo/Bazel locks and MODULE hashes, and retained configured
actions, generated inputs, compilation and M7A as open gates.
