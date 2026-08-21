# Current Slug V2 Packet

Packet: `WP-6-7A-host-canonical-selected-module-definition-observation-carrier-promotion-formatting-correction-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and retained-candidate base: `890b7862` / `a7d9ffcc`

## Goal and exact authority

Retain the complete semantic carrier-promotion draft and corrected lifecycle
source scan. Apply only the exact semantic-neutral `cargo fmt` layout
preflighted on temporary copies of the three carrier files. Do not change any
behavior, API, type, reexport membership/order, source-scan expectation or
assertion.

Write authority is exactly:

- `app/slug_bzlmod_v2/src/selected_repo_spec.rs`;
- `app/slug_bzlmod_v2/src/lib.rs`; and
- `app/slug_bzlmod_v2/tests/canonical_selected_definition_observation_api.rs`.

Every other Rust file, test, fixture, oracle, Cargo/BUILD target, API, caller and
plan is read-only. Formatting must touch only the exact ranges below. If
`cargo fmt` would change any other byte or file, STOP and REPLAN; there is no
format waiver.

## Retained and post-format baselines

The pre-format semantic draft is frozen at full-file SHA-256:

- `selected_repo_spec.rs`:
  `ae0162835c21f20a0e8be2b33bb1476e7b853e42c18fad36aa55a1267106caf5`;
- `lib.rs`:
  `0f9e763c1685ab3e666f7b06950c0ec4b8580441f3ad913740a27867c812f23b`;
- external smoke:
  `68fa57dc7e367ee00fca8e98e925a5bea62a63c853bcfe70551d623fc0cdc97e`.

It accounts as selected +56/-13, lib +6/-0 and a new 44-line smoke, at
12,567/421/44 physical. The exact temporary-copy rustfmt preflight produces:

- `selected_repo_spec.rs` +61/-20, 12,565 physical, SHA-256
  `78a202a4a72b5a49cc6b052234ff95f2813dfbd3b1a1634b9a78102dd71185f5`;
- `lib.rs` +6/-0, 421 physical, SHA-256
  `3fdd3d81d94ce7d3618f356114505d7c30515596a3adbe0f14fb7add30c5cea0`;
- external smoke 47 lines, SHA-256
  `c8ee92e0c7ca1aee1dfcb1fa75e07decda0d2bf084e8baffe434a22608bc5e33`.

Post-format selected accounting is +24/-14 production and +37/-6 colocated
proof; aggregate draft accounting is +114/-20. Preserve the original caps
<=80 production, <=40 colocated proof, <=10 lib, <=70 external, <=200 aggregate
semantic and physical <=12,645/425/70. The identity proof retains its
under-200 proof authority and every new smoke/helper remains below 100.

## Exact allowed formatting

Allow only these rustfmt transformations:

1. In `lib.rs`, relocate the existing hidden error and key reexport pairs to
   rustfmt's lexical position after the legacy selected key, and relocate the
   existing hidden observed-carrier pair to the `Observed...` group. Preserve
   each adjacent `#[doc(hidden)]`/`pub use` pair and the selected-observation
   order Error, Key, Observed. Membership remains exactly those three.
2. In the observed selected Key projection, indent the closure and closing
   parenthesis under `.map(`. Tokens, closure body, Result mapping and
   `.map_err(HostCanonicalSelectedModuleDefinitionObservationError)` are
   unchanged.
3. In the identity/scan/terminal proof, collapse only the wrapper projection
   arm to rustfmt's expression layout. The carrierless panic, public-wrapper/
   private-Routes match and every other input/control/assertion remain
   unchanged.
4. In the external smoke, use rustfmt's two-line function-pointer binding and
   multiline concrete selected `Result` payload. Imports, key construction,
   exact Display, associated Value, accessor types and assertions are
   unchanged.

The corrected lifecycle scan is byte-frozen: it still requires exactly one
adjacent hidden lib reexport pair for Error/Key/Observed, exactly those three
selected-observation reexport lines in that order, and none of those names in
loading `bzl_module.rs` or core `generated_repository_definition.rs`. Freeze
all production/API/types, private-inner/public-outer projection, tracker/event/
nonactivation, held-handle/epoch, cancellation/recovery and smoke assertions.

## Validation, compatibility and terminal

Run serially, after applying the exact formatting:

- `cargo fmt --all -- --check`;
- `cargo test -p slug_bzlmod_v2 observed_canonical_selected_definition_ --lib`;
- `cargo test -p slug_bzlmod_v2 --test canonical_selected_definition_observation_api`;
- protected `cargo test -p slug_bzlmod_v2 --test definition_request_observation_api`;
- protected `cargo test -p slug_bzlmod_v2 --test evaluation_input_request_observation_api`;
- full `cargo test -p slug_bzlmod_v2`;
- direct dependent `cargo check -p slug_core_v2`; and
- `git diff --check`.

Before testing, require the three exact post-format hashes, accounting, physical
sizes, file allowlist and Error/Key/Observed export order above. No oracle is
needed because formatting cannot change Bazel-visible behavior.

Existing selected values/errors/dispositions/full scan/order/views, DICE
equality/invalidation and lower event ownership remain exact Bazel 9
compatibility. The doc-hidden cross-crate key/carrier/opaque outer and shared-
Arc transaction-local epoch association are Slug-native. Canonical/generated
composition, root/publication/command/bootstrap activation and exact Bazel
configuration/output/ActionKey bytes remain unsupported/deferred.

ACCEPT returns only to a docs-only canonical selected/generated observation-
owner design. STOP a pre/post hash or accounting mismatch, any non-rustfmt or
out-of-range change, reexport membership/order/adjacency drift, behavior/API/
type/token/assertion change, lifecycle scan change, fourth reexport, downstream
loading/core edit, Cargo/BUILD, fixture/oracle, cap/proof/test-size/fmt waiver,
upper activation, milestone closure, M8/M7B or exact identity work. REPLAN
before widening. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Proof-correction packet `890b7862` retains the sound semantic draft and fixes
the visibility source scan. Its serial gate now fails only because the four
listed rustfmt layouts were not applied; temporary-copy rustfmt proves the
bounded final bytes above.
