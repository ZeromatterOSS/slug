# Current Slug V2 Packet

Packet: `WP-6-7A-loaded-module-extension-definitions-real-order-event-proof-repair-retry-3`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `ab4db01f`
Accepted predecessor: `99c23033`

## Goal and authority

Repair only the remaining exact event proof and size-cap miss for the retained
loaded-definition observation candidate. Retry 2 completed lawful key-family
classification, separate legacy/observed parity and the nine terminal rows,
but its 213-line test exceeded the hard limit and its batch assertions remained
nondiscriminating. Its sole correction stopped because the existing two-helper
authority could not fit an extracted exact-event proof. Add one dedicated
test-only helper; do not reopen any accepted semantic or independent slice.

Write exactly the `#[cfg(test)] module_extension_definition_loading_tests`
module in `app/slug_loading_v2/src/bzl_module.rs`. Production, the accepted
finisher algebra test and the lifecycle/cancellation test are frozen. Every
other file, fixture, oracle, Cargo/BUILD target, caller and plan is read-only
until terminal rollover.

The retained candidate is 7,824 physical lines and `+1,056/-114` versus
`0a8e1220`. Final caps for this slice are <=`+1,250/-250` and <=8,000 physical.
Replace only `observed_loaded_real_order_terminals_events_and_parity` plus at
most the two already directly used test helpers, one new dedicated test-only
event assertion/helper and the already test-only tracker records; do not append
a fourth parent test. Every changed helper/test remains below 200.

## Frozen decisions

Production semantics remain independently accepted: matching families,
request-compute/Host-Bzl-invariant asymmetry, one request-value clone and child
Arc drop, left-first Complete epoch merge, first terminal, child-only event
batches, compact retention and Complete-only equality/validity.

The identity/finisher test now lawfully proves equal duplicate left-Arc,
valid-epoch conflict -> carrierless Merge and typed child conflict/operation
mismatch -> carrierless Bzl without malformed epoch construction. The opaque
request outer reuses accepted Bzlmod proof and parent forwarding source/
dependency evidence. Do not reopen it.

Lifecycle, parent cancellation/recovery and held-handle A -> B -> A remain the
next serial proof slice. Do not edit or weaken their current test here.

## Three-request fixture and exact parity

Use one real MODULE request list of exactly three distinct ordered root Bzl
labels. Reuse the existing in-memory `ext.bzl`, `other.bzl` and `child.bzl`
sources; if needed, change only test-helper inputs so all three export valid
module-extension definitions. Do not add an external fixture or test hook.

For a successful fresh transaction compute the legacy key and observed key in
separate matching-family transactions over identical injected state. Assert:

- exact semantic Result equality, including request aggregate;
- exact three-definition source order;
- exact manifest and frozen projection equality for each position;
- legacy transactions activate only legacy request/Bzl families and observed
  transactions only observed request/Bzl families; and
- the observed cumulative epoch contains the exact request prefix plus each
  reached root/recursive Host-Bzl prefix, with per-demand Result Arcs forwarded.

## Terminal-position matrix

Drive one decisive request at first, middle and last position by reordering the
same three valid requests. Cover each reachable terminal family separately:

- Bzl semantic failure uses a print-then-fail source;
- export failure requests a missing name from a successfully evaluated module;
  and
- wrong kind requests an exported non-extension value.

For all nine rows assert the exact existing `Request` error variant and decisive
request context, cumulative prefix through the decisive Complete Bzl child,
exact observed root-Bzl activation label order, and zero activation/export work
for every later request. Do not accept generic Need/Request polarity. Label
parse and request outer remain frozen finisher/lower invariants, not fixture
rows.

## Exact event ownership

Use separate tracker stores: all-key dependency rows from `key_activated` and
real kind/batch rows from `key_activated_rich`. Never synthesize an activation
kind or treat both callbacks as two activations.

Prove in isolated transactions:

- fresh success: exactly the three reached Host-Bzl child keys own nonempty
  evaluated batches in request order, with exact print event variants/texts
  `A`, `B`, `C`; loaded parent and request child have no batch;
- direct warm parent: no Host-Bzl child activation/batch is emitted;
- changed parent with one unchanged reached Bzl child: that child is Reused
  with `batch: None` and is not reevaluated; and
- print-then-fail: the decisive semantic-failure Host-Bzl child alone retains
  its exact nonempty print batch; use silent successful predecessors so other
  reached children are batchless/empty as semantically appropriate, while the
  loaded parent and request child remain batchless.

The dependency rows must show the loaded observed parent depends on the
observed request child and exactly the reached observed Host-Bzl roots in
source order. Use exact key-family prefixes, never substring vocabulary.

Classify the actual injected lower policy keys by tracker-side
`DynKey::downcast_ref::<RootModuleCommandPolicyKey>()` and
`DynKey::downcast_ref::<RootModuleEnvironmentPolicyKey>()`, or equivalently by
their exact `root-module-command-policy:*` and
`root-module-environment-policy:*` Displays. Allow and require those families
where reached. Also allow the exact lower Display prefixes
`root-module-lockfile-mode:*` and `visible-lockfile:*` (plus a separately
reached `host-visible-lockfile:*` family). `BzlmodCommandPolicyKey` and
`BzlmodEnvironmentPolicyKey` are policy values, not DICE keys: never require
them as activation/dependency rows. Exclude the reverse legacy families
`host-selected-extension-definition-load-requests:*`,
`host-bzl-module:*` and `host-loaded-module-extension-definitions:*`. Exclude
the exact upper/public families `host-prepared-module-extension-inputs:*`,
`host-pure-module-extension-invocations:*`,
`host-instantiated-module-extension-repositories:*`,
`host-validated-module-extension-repositories:*`,
`host-root-repository-mapping:*`,
`host-canonical-selected-module-definition:*`,
`host-generated-repository-definition:*` and `slug-command:*`. Do not reject a
key merely because its Display contains `command`, `public` or another word.

## Compatibility and validation

Exact remains current loaded-definition Result/errors/order/manifests/
projections and child events. The private observed association remains
Slug-native. Upper evaluation/public/bootstrap, M8/M7B and exact identity bytes
remain deferred. No oracle is needed; reuse pinned Bazel 9.2 loading evidence.

Run serially:

1. `CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2
   module_extension_definition_loading_tests::observed_loaded_real_order_terminals_events_and_parity --quiet`;
2. protected `module_extension_definition_loading_tests::observed_bzl_` tests;
3. `CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 --quiet`;
4. `CARGO_BUILD_JOBS=1 cargo check -p slug_core_v2 --quiet`;
5. `cargo fmt --all -- --check`; and
6. `git diff --check`.

## Terminal and stops

ACCEPT freezes this real-order/event proof and activates only the docs packet
for the final lifecycle/cancellation/nonactivation repair. It does not accept or
commit the production candidate yet.

STOP and `REPLAN` a production, identity/finisher or lifecycle-test edit; a
second Rust file/key/owner; fake key/hook/external fixture; parent event batch;
tracker row conflation; generic terminal assertion; cap waiver; upper
activation; milestone closure; M8/M7B or exact identity work. M7 remains partial
and M7A -> M8 -> M7B remains.

## Immediate predecessor

Retry 2 scheduled by `23e4a3a8` completed lawful RootModule policy
classification, separate legacy/observed Result/request/manifest/projection
parity, family separation, warm silence, Reused/None and nine positional
Request/prefix/suppression rows. Full validation passed at `+1,056/-114` and
7,824 physical. Terminal review rejected its 213-line test and `batch.is_some()`
checks: predecessor prints made every reached child nonempty in semantic-failure
rows, rather than proving isolated decisive ownership. The sole correction made
no edit because exact helper extraction exceeded the two-helper authority, so
the orchestration second-correction rule requires this bounded REPLAN.
