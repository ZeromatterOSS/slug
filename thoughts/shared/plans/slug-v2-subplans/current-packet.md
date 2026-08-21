# Current Slug V2 Packet

Packet: `WP-6-7A-host-canonical-selected-module-definition-observation-carrier-promotion-identity-test-layout-correction-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and retained-candidate base: `f70ede65` / `a7d9ffcc`

## Goal and exact authority

Retain the fully formatted, green three-file carrier-promotion draft. Remove
only one rustfmt-stable blank line inside
`observed_canonical_selected_definition_identity_scan_and_terminal_algebra` so
the function is below 200 physical lines. Do not change any token, semantic
input, branch, assertion, API, type, reexport, smoke or lifecycle proof.

Only `app/slug_bzlmod_v2/src/selected_repo_spec.rs` is writable, and only the
single blank line between the completed Missing assertion and
`let mut duplicate_routes = routes.clone();`. `app/slug_bzlmod_v2/src/lib.rs`
and
`app/slug_bzlmod_v2/tests/canonical_selected_definition_observation_api.rs`
are byte-frozen. Every other Rust file, test, fixture, oracle, Cargo/BUILD
target, API, caller and plan is read-only.

## Frozen and target bytes

The green formatted draft is frozen at:

- `selected_repo_spec.rs` SHA-256
  `78a202a4a72b5a49cc6b052234ff95f2813dfbd3b1a1634b9a78102dd71185f5`,
  +61/-20 and 12,565 physical;
- `lib.rs` SHA-256
  `3fdd3d81d94ce7d3618f356114505d7c30515596a3adbe0f14fb7add30c5cea0`,
  +6/-0 and 421 physical; and
- external smoke SHA-256
  `c8ee92e0c7ca1aee1dfcb1fa75e07decda0d2bf084e8baffe434a22608bc5e33`,
  47 physical.

The current identity function from signature through closing brace is exactly
200 physical lines with SHA-256
`3e4fc30b33b2d125bca9cd448915defe3e1fb17b64da4cc716269ac329b3a18d`.

Temporary-copy preflight removes only the named blank line and remains clean
under rustfmt. The exact target is:

- `selected_repo_spec.rs` SHA-256
  `70bfee696f637543ca1e830ebb780d961c0bacffc85a1940eb41bd229b5ce31e`,
  +61/-21 and 12,564 physical; and
- the identity function from signature through closing brace is 199 physical
  lines with SHA-256
  `ab3ad14719d015c5744a4c1a29aa39fb9d6f4a8793859435691247aeb4ff6901`.

Production remains +24/-14 and colocated proof becomes +37/-7. With frozen lib
and smoke, aggregate accounting is +114/-21. Preserve unchanged caps <=80
production, <=40 colocated proof, <=10 lib, <=70 external, <=200 aggregate
semantic and physical <=12,645/425/70. Every new smoke/helper remains below
100. There is no proof, test-size or formatter waiver.

## Frozen behavior and proof

The only allowed diff is deletion of the blank separator. Freeze byte-for-byte
all production and public/private carrier types, constructor/accessors,
private-inner/public-outer projection, Error/Key/Observed hidden reexport
membership/order/adjacency, external key/Display/associated-Value/accessor
smoke, and the corrected lifecycle source scan.

Within the identity test, preserve every token and the exact identity/hash/
Display, Need, outer, compute, Missing, Duplicate, BuiltinDeferred, success,
epoch, Arc, equality and validity assertions. Preserve all tracker/event/
nonactivation, held-handle/epoch, cancellation and recovery proof elsewhere.

## Validation, compatibility and terminal

Run serially:

- verify the three frozen input hashes;
- delete only the named blank line;
- `cargo fmt --all -- --check`;
- verify the exact target selected/identity hashes, accounting, physical sizes
  and three-file allowlist;
- `cargo test -p slug_bzlmod_v2 observed_canonical_selected_definition_ --lib`;
- `cargo test -p slug_bzlmod_v2 --test canonical_selected_definition_observation_api`;
- protected `cargo test -p slug_bzlmod_v2 --test definition_request_observation_api`;
- protected `cargo test -p slug_bzlmod_v2 --test evaluation_input_request_observation_api`;
- full `cargo test -p slug_bzlmod_v2`;
- direct dependent `cargo check -p slug_core_v2`; and
- `git diff --check`.

No oracle is needed because blank-line removal cannot change Bazel-visible
behavior. Existing selected values/errors/dispositions/full scan/order/views,
DICE equality/invalidation and lower event ownership remain exact Bazel 9
compatibility. The doc-hidden carrier/opaque outer/shared-Arc transaction-local
epoch is Slug-native. Canonical/generated composition, upper activation and
exact Bazel configuration/output/ActionKey bytes remain unsupported/deferred.

ACCEPT returns only to a docs-only canonical selected/generated observation-
owner design. STOP an input/target hash, accounting, physical, fmt or allowlist
mismatch; deletion or edit of any other line/token/file; semantic/API/type/
reexport/smoke/assertion/lifecycle drift; loading/core edit; Cargo/BUILD;
fixture/oracle; cap/proof/test-size/fmt waiver; upper activation; milestone
closure; M8/M7B or exact identity work. REPLAN before widening. M7 remains
partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Formatter packet `f70ede65` produces the exact fully green three-file bytes.
Final review finds only that the identity function is 200 rather than below 200
physical lines; temporary-copy deletion of one blank separator proves the
minimal rustfmt-stable correction.
