# Current Slug V2 Packet

Packet: `WP-6-7A-loaded-module-extension-definitions-lifecycle-cancellation-proof-repair-retry`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `f36e5586`
Accepted predecessor: `99c23033`

## Goal and authority

Finish only the held-handle lifecycle, cancellation/recovery and upper-
nonactivation proof for the retained loaded-definition observation candidate.
The first lifecycle packet reached `REPLAN` because it inferred inner-Arc
identity from semantic DICE equality across snapshot replacement and separate
epoch computes. Correct that proof law. The exact real-order/parity/terminal/
event slice, production and identity/finisher algebra remain frozen.

Write only the `#[cfg(test)] module_extension_definition_loading_tests` module
in `app/slug_loading_v2/src/bzl_module.rs`. Replace
`observed_loaded_lifecycle_cancellation_and_nonactivation` plus at most three
directly used lifecycle helpers and existing tracker records. Do not add a
fourth parent test. Every changed helper/test remains below 200 lines. Rustfmt-
only layout in the test module is allowed; assertions, values and control flow
outside this lifecycle slice remain unchanged. Every other file, fixture,
oracle, Cargo/BUILD target, caller and plan is read-only until rollover.

The retained unaccepted lifecycle candidate is 8,181 physical lines and
`+1,410/-111` versus `0a8e1220`; it compiles and preserves production but its
restored identity assertions intentionally reproduce the stop. Final caps stay
<=`+1,800/-350` and <=8,450 physical. The shared driver stays 130 lines and
production remains byte-for-byte unchanged.

## Frozen decisions

Preserve the accepted matching-family driver, request-compute/Host-Bzl-
invariant error asymmetry, one request-value clone and child Arc drop,
left-first Complete epoch merge, first terminal, child-only event batches,
compact retention and Complete-only equality/validity. Preserve the accepted
finisher algebra and exact three-request legacy parity, nine terminal rows,
prefix/suppression, RootModule policy family boundary, fresh/warm/reused/failure
events and exact upper/legacy key-family denylist.

## Independent held-handle lifecycles

Use normal in-memory fixtures and real observed keys, with no hook or fake key.
Before each transition retain independent handles to every relevant layer:

- the loaded parent Result Arc and cumulative `PathObservationEpoch`;
- the observed request child's Result Arc and request epoch;
- each decisive `HostBzlModuleObservationKey` Result Arc and epoch; and
- the loaded manifest plus frozen exported-definition projection selected by
  the parent.

On one same-DICE fixture drive four independent A -> B -> A axes. Isolate one
axis at a time and restore it before the next:

1. request-only: change request order or selected exported name while source
   inputs stay identical;
2. direct Bzl source: change one root source while an unrelated reached root is
   identical;
3. recursive load: change only a child loaded by one root while an unrelated
   reached root is identical; and
4. pure export: switch between two valid extension exports from one unchanged
   evaluated module, without changing its source or observation epoch.

For every axis assert parent equality changes at B and restores at A and all
held old Results/epochs/projections remain valid and unchanged. Compare request
and Host-Bzl carrier Results and epochs semantically across separate computes:
snapshot replacement may recompute an equal key with fresh inner Arcs. Require
exact Result and demand/result-map equality for unaffected children, while the
affected direct/recursive Bzl carrier changes semantically. Pure export keeps
the Bzl Result/epoch semantically equal while the parent projection changes.

Require inner Result-Arc identity only for an exact child key/label whose rich
activation row proves `ActivationKind::Reused` and whose returned cached key
value actually retains the Arc. If the row is Evaluated, semantic equality is
the contract. The accepted finisher test remains the sole unconditional proof
that an exact incoming child epoch Arc and equal left-first duplicate are
forwarded. Reuse lower recursive-Bzl proofs for lower behavior, but assert the
parent semantic lifecycle and any conditional Reused sharing locally.

## Cancellation and exact recovery

In a fresh same-DICE transaction poll the loaded observed parent once to
Pending, then drop the future. Prove separately that no parent value, parent
activation or parent/child event batch was published and that tracker stores
contain no completed dependency/rich activation row from the cancelled attempt.

Recompute the same key and identical inputs in that DICE. Require a Complete
valid carrier and compare against a clean identical fixture for exact semantic
Result, request aggregate, manifests/projections and cumulative epoch
demand/result map. Against an independently computed recovery-transaction
`PathObservationEpochKey`, require the exact same demand set and semantic
`PathObservationResult` for every entry, with no extra or missing demands. Do
not require cross-compute Arc identity: DICE may publish equal epochs with fresh
inner Arcs even without another input replacement. A subsequent warm compute
must be semantically equal and silent; pointer reuse is required only under the
same exact cached/Reused condition above.

## Production-slice and all-key nonactivation

Add no production state or event tracker. Scan the production source slice
from the loaded legacy/observed owner through its driver/finisher and prove it
does not name, construct or compute any of these exact upper key types:
`HostPreparedModuleExtensionInputsKey`,
`HostPureModuleExtensionInvocationsKey`,
`HostInstantiatedModuleExtensionRepositoriesKey`,
`HostValidatedModuleExtensionRepositoriesKey`,
`HostRootRepositoryMappingKey`,
`HostCanonicalSelectedModuleDefinitionKey` and
`HostGeneratedRepositoryDefinitionKey`. Prove command/public nonactivation by
the exact all-key `slug-command:` and named upper prefixes below; do not invent
an unnamed source token or use substring vocabulary.

Across all-key activation and dependency rows require the parent has exactly
the observed request child followed by its reached observed Host-Bzl roots.
Allow the lawful lower RootModule command/environment/lockfile and visible-
lockfile families already accepted by the event slice. Exclude exact reverse
legacy prefixes `host-selected-extension-definition-load-requests:`,
`host-bzl-module:` and `host-loaded-module-extension-definitions:`. Exclude
exact upper/public prefixes `host-prepared-module-extension-inputs:`,
`host-pure-module-extension-invocations:`,
`host-instantiated-module-extension-repositories:`,
`host-validated-module-extension-repositories:`,
`host-root-repository-mapping:`,
`host-canonical-selected-module-definition:`,
`host-generated-repository-definition:` and `slug-command:`. Use exact
prefix/type predicates, never substring vocabulary.

## Compatibility and validation

Exact remains loaded-definition Result/errors/order/manifests/projections and
child events. The private Result/epoch association remains Slug-native. Upper
evaluation/public/bootstrap, M8/M7B and exact identity bytes remain deferred.
No oracle is needed; reuse pinned Bazel 9.2 loading evidence.

Run serially:

1. focused `observed_loaded_lifecycle_cancellation_and_nonactivation`;
2. the three accepted `observed_loaded_` parent tests;
3. protected `module_extension_definition_loading_tests::observed_bzl_` tests;
4. `CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 --quiet`;
5. `CARGO_BUILD_JOBS=1 cargo check -p slug_core_v2 --quiet`;
6. `cargo fmt --all -- --check`; and
7. `git diff --check`.

## Terminal and stops

ACCEPT performs final semantic acceptance of the loaded-definition observed
production candidate, commits the single Rust file, and rolls the docs-only
frontier to the prepared/evaluation observation successor selected by Stage 6.

STOP and `REPLAN` any production or accepted-test semantic edit; a second Rust
file/key/owner; fake key/hook/external fixture; lock held across DICE; parent
event batch; retention/event drift; generic lifecycle/cancellation assertion;
cap waiver; upper activation; milestone closure; M8/M7B or exact identity work.
M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

The lifecycle packet scheduled by `2d9dbf95` produced a compiling 8,181-line
four-axis candidate and exact nonactivation scan, then reproduced two contract
errors. Source-only snapshot replacement recomputes the semantically unchanged
request carrier with a new inner Result Arc; pure export can likewise recompute
unchanged Host-Bzl carriers. Cancellation recovery also returns a semantically
equal Host `/` Lstat observation with a fresh Arc relative to a separately
computed global epoch. Reserved review confirmed semantic DICE equality does
not promise cross-compute inner-Arc identity. Production is unchanged; the
retry corrects only these proof assertions and retains all exact semantic,
cancellation, nonactivation and held-old immutability obligations.
