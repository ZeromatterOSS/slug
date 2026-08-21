# Current Slug V2 Packet

Packet: `WP-6-7A-host-selected-extension-evaluation-input-requests-observation-carrier-promotion-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling/design base: `d17637fd`
Rust base: `3a68afa5`

## Goal and authority

Implement only the accepted doc-hidden Bzlmod -> loading visibility surface for
the existing evaluation-input observation key/carrier/opaque outer. Do not add
a semantic consumer or change the accepted driver, Result/epoch association,
Need/outer/error algebra, equality, validity, events, retention or lifecycle.

Write authority is exactly:

- `app/slug_bzlmod_v2/src/selected_repo_spec.rs`;
- `app/slug_bzlmod_v2/src/lib.rs`; and
- new `app/slug_bzlmod_v2/tests/evaluation_input_request_observation_api.rs`.

Every other Rust file, test, fixture, oracle, Cargo/BUILD target, API and plan is
read-only. Production is <=70 lines, colocated proof <=40, external proof <=60
and aggregate semantic authority <=170. Physical caps are 11,750 lines for
`selected_repo_spec.rs`, 425 for `lib.rs` and 60 for the new smoke. Every
changed helper/test remains below 100 lines.

## Frozen nominal surface

Promote exactly these three `#[doc(hidden)]` nominal types from
`selected_repo_spec.rs`:

- existing `HostSelectedExtensionEvaluationInputRequestsObservationKey`;
- existing `ObservedHostSelectedExtensionEvaluationInputRequests`; and
- new `HostSelectedExtensionEvaluationInputRequestsObservationError`, an
  opaque public tuple wrapper around private
  `EvaluationInputRequestsObservationError`.

Make only the observation key's `new(NormalizedAbsolutePath) -> Self`
constructor and the observed carrier's two borrowed accessors public. Spell
`result()` with the public concrete return type:
`&Arc<Result<HostSelectedExtensionEvaluationInputRequests,
HostSelectedExtensionEvaluationInputRequestsError>>`. Keep
`observations() -> &PathObservationEpoch`. Keep tuple/struct fields, the private
result alias, observation-stage enum and Requests/RootFiles/Merge error kinds
private.

Add exactly three `#[doc(hidden)]` crate-root reexports with those names. Add no
fourth type, public alias, field, constructor, error inspector or adapter key.

## Wrapper and DICE boundary

Change only the observation key's associated `Key::Value` error from private
`EvaluationInputRequestsObservationError` to the public opaque wrapper. Wrap
the private error only in the key's `Complete(Err(...))` projection. Leave the
private driver/finishers and their typed stage errors unchanged.

There is no current production unwrapping consumer. Same-module proof may keep
inspecting the private driver/finisher error algebra directly; it must not add a
public unwrap path. The later loading prepared-input owner will carry the opaque
child outer without inspecting its internals. Preserve Display/key identity,
Complete-only equality/validity, carrierless Need/outer, the exact local Result
Arc and transaction-local epoch, eventlessness, warm/cancel behavior and
retained lifetime.

## External smoke and evidence

The new external-crate smoke imports the three hidden reexports and
`NormalizedAbsolutePath`, constructs the key for `/workspace`, and asserts the
exact existing Display:
`observed-host-selected-extension-evaluation-inputs:"/workspace"`. A typed
inspection function must accept borrowed carrier/error values and call both
borrowed carrier accessors from outside the crate.

The smoke must not construct the carrier or error, compute the key, add a
semantic caller, inspect the opaque error, name private aliases/stages/kinds or
depend on mapping/root-file internals. Reuse the accepted evaluation-input
observation proof; add no oracle because no Bazel-visible behavior changes.

Run:

- `cargo test -p slug_bzlmod_v2 observed_evaluation_inputs_ --lib`;
- `cargo test -p slug_bzlmod_v2 --test evaluation_input_request_observation_api`;
- full `cargo test -p slug_bzlmod_v2`;
- direct dependent `cargo check -p slug_loading_v2`;
- `cargo fmt --all -- --check`; and
- `git diff --check`.

Do not run Cargo commands concurrently in the shared target directory.

## Compatibility and terminal

Existing evaluation-input values/errors/order/root metadata/tags and child
events remain exact Bazel 9 compatibility. The hidden observation API, opaque
typed outer and shared-Arc transaction-local epoch association are Slug-native.
Prepared/pure/instantiated/validated/root-mapping/generated/public/bootstrap
activation, M8/M7B and exact identity bytes remain deferred.

Implementation ACCEPT returns only to one docs-only prepared-input owner design
packet. STOP semantic/event/equality/retention change, public field/alias/error
inspection, a second key/adapter/type, loading/caller change, Cargo/BUILD,
fixture/oracle work, cap/proof waiver, prepared activation, milestone closure,
M8/M7B or exact identity work. REPLAN before widening. M7 remains partial and
M7A -> M8 -> M7B remains.

## Immediate predecessor

Design `d17637fd` proves the existing evaluation-input observation key, carrier
and private outer are the sole unavailable cross-crate inputs before prepared
ownership. The accepted definition-request promotion supplies an exact bounded
precedent: one key constructor, two borrowed carrier accessors, one opaque
wrapper at `Key::Value`, exactly three reexports and one external compile smoke,
with no semantic, event, retention or DICE dependency change.
