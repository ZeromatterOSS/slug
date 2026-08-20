# Current Slug V2 Packet

Packet: `WP-6-7A-loaded-module-extension-definitions-observation-proof-correction-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `0a8e1220`
Accepted predecessor: `99c23033`

## Goal and authority

Close the parent-specific proof for the uncommitted loaded-definition
observation candidate after the prior implementation packet reached terminal
`REPLAN`. Preserve the independently accepted production driver and replace
the nondiscriminating proof. Activate no caller or upper extension owner.

Write exactly `app/slug_loading_v2/src/bzl_module.rs`. Every other file is
read-only, including Bzlmod, loading callers/modules, Cargo/BUILD metadata,
fixtures, oracles and planning documents until terminal rollover.

The retained candidate is 7,334 physical lines and `+559/-107` versus
`0a8e1220`. Correction authority is <=`+540/-220`, net <=320 from that
candidate; final caps are <=`+1,099/-327` versus `0a8e1220` and <=7,654
physical lines. Production is frozen except a production-called pure finisher
extraction: prefer net zero, touch at most 110 production lines and preserve
the shared driver below 150. Permit at most four new test helpers and three
parent tests; every helper/test remains below 200.

## REPLAN basis and accepted candidate

The prior Terra-high implementation and correction pass compile and pass the
focused and full loading suites plus the direct core check. Independent Sol
review accepts its production semantics:

- matching Legacy/Observed request and Host-Bzl families;
- request DICE failure -> existing `RequestsCompute` versus Host-Bzl
  `host_dice_invariant` asymmetry;
- one successful request-value clone followed by child Result-Arc drop;
- left-first Complete Bzl epoch merge before Bzl/export/downcast semantics;
- first-terminal return, child-owned Bzl batches and no parent batch;
- bounded typed-outer retention; and
- Complete-only parent equality/validity.

The proof remained material incomplete after its sole correction. Its union
test exercised `union_host_observations` rather than the parent boundary, and
its omnibus test did not discriminate stage/context/carrierlessness, terminal
positions/order, event ownership/reuse, held-handle lifecycles, parent
cancellation, exact legacy parity or upper nonactivation. The orchestration
second-correction rule therefore requires this new packet; it does not admit a
production semantic correction.

## Lawful proof seam

An ordinary consistent snapshot cannot produce a genuine parent Bzl outer,
merge conflict or operation mismatch: equal demands share the snapshot result
Arc and epoch construction rejects a mismatched demand/result pair. A loaded
definition label-parse failure is likewise unreachable from the valid Bzlmod
request constructor.

If needed, extract only production-used pure request/Bzl finishers from the
accepted driver. They consume the real child outcome/carrier/epoch and existing
request context, are called by production, and own the same merge/semantic
projection now inline. They may not add a test-only branch, fake/synthetic DICE
key, hook, alternate owner or semantic value. Use them to prove unreachable
boundary algebra; real first/middle/last proof applies to reachable Bzl
semantic/export/wrong-kind terminals and later-child suppression.

## Required proof decomposition

Replace the two current added tests rather than accumulating another omnibus.
Add exactly three bounded parent specifications.

### Identity and finisher algebra

`observed_loaded_identity_and_finisher_algebra` proves:

- distinct key equality/hash/Display;
- Complete-only equality/validity for semantic carrier, request outer and Bzl
  outer, with Need invalid and self-unequal;
- request outer is carrierless;
- empty/request/prior/current prefix association at the production finisher;
- equal duplicate retains the left/request-side Arc;
- conflict and operation mismatch return exact parent
  `Request { requests, request, stage: Merge, error }` with no carrier; and
- a child outer returns exact `stage: Bzl` request context with no carrier.

The operation-mismatch discriminator must enter the production finisher as the
real typed child frontier failure; do not merely assert that epoch construction
rejects malformed input.

### Real order, terminals, events and parity

`observed_loaded_real_order_terminals_events_and_parity` uses three ordered
requests and proves:

- exact legacy/observed semantic Result, request aggregate, definition order,
  manifest and projection equality;
- matching request/Bzl family exclusion and exact observed Bzl activation
  order;
- reachable first/middle/last Bzl semantic, export and wrong-kind terminals
  retain the decisive prefix and suppress every later Bzl/export operation;
- the label parser invariant is recorded through the pure production finisher,
  not an impossible fixture;
- fresh evaluated Host-Bzl children alone own nonempty batches in request
  order, while parent/request activations have no batch;
- direct warm parent reuse is silent;
- changed-parent/unchanged-child activation is `Reused` with `batch: None`;
  and
- a semantic Bzl failure still leaves its decisive child batch at the child.

Extend the existing tracker only as test proof. Record key Display,
`ActivationKind`, optional batch and dependency rows for the loaded parent,
request child and Host-Bzl children. Do not introduce production event state or
weaken existing lower assertions.

### Lifecycle, cancellation and nonactivation

`observed_loaded_lifecycle_cancellation_and_nonactivation` proves:

- independent request, each decisive Bzl source/recursive load and pure export
  A -> B -> A transitions while holding prior parent Result/epoch and child
  handles;
- old handles remain valid, unaffected child Result/epoch Arcs remain shared,
  and parent equality changes/restores exactly;
- poll-drop before parent publication records no parent value, activation or
  batch, followed by same-DICE recovery;
- activation dependencies contain only the observed request/Host-Bzl lineage;
  and
- a production-slice plus activation assertion excludes legacy siblings and
  prepared, pure, instantiated, validated, root-mapping, generated,
  public/command keys.

Reuse accepted lower request identity/Need/cancellation/no-upper proof and the
existing `observed_bzl_*` recursive source/child/cycle/frontier/cancellation
tests. Parent prefix composition, typed outer mapping, family selection,
terminal stop order, event ownership, parent held handles/cancellation and
upper nonactivation must be asserted here rather than inferred.

## Compatibility, retention and validation

Exact remains existing loaded-definition values, semantic errors, request
order, manifests, projections and child Bzl events. Slug-native remains the
private observed key/carrier/typed outer/cumulative epoch and Display token.
Prepared/pure/instantiated/validated evaluation, root mapping, generated
repositories, public/bootstrap activation, M8/M7B and exact identity bytes stay
deferred.

The retained value remains exactly one local semantic Result Arc plus one
cumulative compact epoch. The typed outer retains only the accepted opaque
request error, or completed request aggregate plus decisive request/stage/
frontier error. Add no child carrier, evaluator/module heap, event vector,
cache/interner/store/lock/task/revision/certificate or second owner. No lock
spans DICE compute.

Run serially:

1. the exact three `module_extension_definition_loading_tests::observed_loaded`
   successor tests;
2. protected `module_extension_definition_loading_tests::observed_bzl_` tests;
3. `CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 --quiet`;
4. `CARGO_BUILD_JOBS=1 cargo check -p slug_core_v2 --quiet`;
5. `cargo fmt --all -- --check`; and
6. `git diff --check`.

## Terminal and stops

ACCEPT commits the retained candidate plus corrected proof, records exact
accounting/validation and activates only the docs-only prepared/evaluation
observation frontier.

STOP and `REPLAN` a second file/key/owner; a semantic driver/key/carrier/error/
event/retention correction; a test hook or synthetic DICE key; more than one
production-called pure finisher family; prepared/pure/instantiated/validated/
root-mapping/generated/public activation; a parent batch; non-Complete parent
equality/validity; parent carrier on typed outer; wider caps; milestone closure;
M8/M7B or exact identity-byte work. M7 remains partial and M7A -> M8 -> M7B
remains.

## Immediate predecessor

Design commit `0a8e1220` activated the first implementation attempt over
accepted carrier `99c23033`. Its uncommitted candidate is retained read-only
except for the proof seam above. The first review and focused correction
rereview returned `REVISE`; the latter triggered this formal `REPLAN` because
multiple parent-specific proof gaps remained.
