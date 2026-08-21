# Current Slug V2 Packet

Packet: `WP-6-7A-host-canonical-selected-module-definition-observation-carrier-promotion-proof-correction-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling/design and retained-candidate base: `6204c9f2` / `a7d9ffcc`

## Goal and retained authority

Retain the current narrow three-file carrier-promotion draft and correct only
the stale source scan at the end of
`observed_canonical_selected_definition_lifecycle_cancellation_and_nonactivation`.
The scan must recognize the intentional Bzlmod crate-root exports while still
proving loading and core remain inactive. Do not change production, API shape,
reexports, external smoke, identity wrapper proof or any lifecycle assertion.

The retained draft is:

- `app/slug_bzlmod_v2/src/selected_repo_spec.rs` at +25/-11, comprising
  +19/-9 production and the frozen +6/-2 identity wrapper update;
- `app/slug_bzlmod_v2/src/lib.rs` at +6/-0; and
- new 44-line
  `app/slug_bzlmod_v2/tests/canonical_selected_definition_observation_api.rs`.

Current physical sizes are 12,538/421/44 against unchanged caps
12,645/425/70. Original authority remains <=80 production, <=40 colocated
proof, <=10 lib, <=70 external proof and <=200 aggregate semantic. The existing
identity wrapper update retains its under-200 proof authority, and every new
smoke/helper remains below 100. No cap or test-size waiver is authorized.

Only the lifecycle test's final source-scan block in `selected_repo_spec.rs` is
writable. Every other line in the three retained files and every other Rust
file, test, fixture, oracle, Cargo/BUILD target, API, caller and plan is
read-only. Freeze:

- production through line 4,677 at SHA-256
  `eec5dd83dc786a2317c011a19de68dff952c2e6821d664cf6e75da0f18240f0a`;
- all of `lib.rs` at
  `0f9e763c1685ab3e666f7b06950c0ec4b8580441f3ad913740a27867c812f23b`;
- the external smoke at
  `68fa57dc7e367ee00fca8e98e925a5bea62a63c853bcfe70551d623fc0cdc97e`;
  and
- the existing identity test including its wrapper projection/match update at
  `d22d3a4d65d0021cbfcd0bbf37cc4fe5d5a039eb8141d66d9756ec7d1e641052`.

## Frozen carrier draft

Keep exactly the three doc-hidden public nominal types: key with public
two-argument `new`, carrier with concrete borrowed selected Result-Arc and
epoch accessors, and field-private opaque public outer over private
`CanonicalSelectedModuleDefinitionObservationError::Routes`. Keep only
observed Complete Err wrapping at Key projection; Need and success are
unchanged and no unwrap exists.

Keep exactly one adjacent `#[doc(hidden)]` plus `pub use
selected_repo_spec::...;` pair in `lib.rs` for each of:

- `HostCanonicalSelectedModuleDefinitionObservationError`;
- `HostCanonicalSelectedModuleDefinitionObservationKey`; and
- `ObservedHostCanonicalSelectedModuleDefinition`.

There is no fourth selected-observation reexport. Keep the 44-line external
key/Display/associated-Value/concrete-accessor smoke byte-for-byte. Keep the
identity proof's input, wrapper projection, private Routes match, control flow
and every assertion byte-for-byte.

## Exact proof correction

Replace only the lifecycle tail's obsolete three-source absence loop. The new
semantic-neutral scan must:

1. read `lib.rs` and require exactly one occurrence of each exact two-line
   adjacent hidden reexport pair above;
2. collect selected-observation reexport lines containing either
   `HostCanonicalSelectedModuleDefinitionObservation` or
   `ObservedHostCanonicalSelectedModuleDefinition` and require exactly the
   retained three lines, in retained source order, with no extra selected-
   observation reexport; and
3. require both `slug_loading_v2/src/bzl_module.rs` and core
   `runtime/generated_repository_definition.rs` to contain none of those three
   nominal names.

Do not edit either scanned file. Preserve every tracker row, parent/warm
batchlessness assertion, upper-nonactivation assertion, held Result/carrier/
epoch A-B-A and metadata behavior, transaction epoch-subset check, Arc identity
condition, cancellation absence and recovery assertion in the lifecycle test.
Add no helper, semantic input, compute, activation or runtime injection.

## Validation, compatibility and terminal

Run serially:

- `cargo test -p slug_bzlmod_v2 observed_canonical_selected_definition_ --lib`;
- `cargo test -p slug_bzlmod_v2 --test canonical_selected_definition_observation_api`;
- protected `cargo test -p slug_bzlmod_v2 --test definition_request_observation_api`;
- protected `cargo test -p slug_bzlmod_v2 --test evaluation_input_request_observation_api`;
- full `cargo test -p slug_bzlmod_v2`;
- direct dependent `cargo check -p slug_core_v2`;
- `cargo fmt --all -- --check`; and
- `git diff --check`.

Existing selected values/errors/dispositions/full scan/order/views, DICE
equality/invalidation and lower event ownership remain exact Bazel 9
compatibility. The doc-hidden cross-crate key/carrier/opaque outer and shared-
Arc transaction-local epoch association are Slug-native. Canonical/generated
composition, root/publication/command/bootstrap activation and exact Bazel
configuration/output/ActionKey bytes remain unsupported/deferred.

ACCEPT returns only to a docs-only canonical selected/generated observation-
owner design. STOP any frozen-hash mismatch, production/API/lib/smoke/identity
edit, lifecycle change outside the final source scan, tracker/nonactivation/
retention/cancellation assertion drift, public field/alias/terminal, fourth
type/reexport, second key/carrier/adapter, core/loading edit, reverse dependency,
Cargo/BUILD, fixture/oracle, cap/proof/test-size waiver, upper activation,
milestone closure, M8/M7B or exact identity work. REPLAN before widening. M7
remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

The implementation draft from `6204c9f2` satisfies the accepted visibility
surface and smokes. Review found only that the old lifecycle denylist includes
`lib.rs`, which now intentionally owns the three hidden exports; loading and
core remain correctly absent.
