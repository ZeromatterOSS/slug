# Current Slug V2 Packet

Packet: `WP-6-7A-host-pure-module-extension-invocations-observation-proof-correction-2-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `c344d2ed`; Rust candidate base: `f76bab3a`; candidate is uncommitted

## Goal and retained authority

Make one final proof-only correction to the green one-file pure-invocation
observation candidate. Reuse the prepared owner's committed private-join proof
and prove only the pure owner's positive real dependency edges. Keep every
production byte and all accepted semantic/result/error/order, event, retention,
lifecycle, cancellation and nonactivation proof unchanged.

Write authority is solely
`app/slug_loading_v2/src/module_extension.rs`, and only content at or below the
first `#[cfg(test)]` may change. The retained candidate is +301/-171 production
and +597/-127 proof against `f76bab3a`, with 2,229 physical lines. Its lines
1-894 are byte-frozen production with SHA-256
`bdee0efe2873997c4a90429cb0a6912cd809f77fb6e0f2657688817d8ae6b738`.
Production accounting must remain exactly +301/-171. Proof caps are
<=650 additions, <=150 deletions; aggregate caps are <=951 additions, <=321
deletions and <=2,285 physical lines. Keep exactly three observed-parent tests,
at most six test helpers and every changed helper/test below 200 lines. Every
other file, fixture, oracle, Cargo/BUILD target, API, export and caller is
read-only.

## Frozen production and corrected Label contract

Freeze the private Legacy/Observed owner, prepared-first and ordered Host-Bzl
preflight, all-preflight-before-invocation execution, Result-Arc/epoch carrier,
Prepared/HostBzl/Merge outer algebra, first-terminal behavior, pure print-batch
predicate, repository-rule receipts, compact retention and upper
nonactivation exactly as implemented. Add no production helper, branch,
adapter, task, lock, cache or side state.

`HostPureModuleExtensionInvocationError::Label` is an unreachable defensive
legacy branch for valid prepared inputs. The source chain is exact:

- loaded-definition preparation parses the request's target with
  `RootPackageBzlTarget::parse` at `bzl_module.rs:2709-2731` before producing a
  loaded definition;
- prepared inputs join the raw evaluation input to that exact loaded request
  at `bzl_module.rs:2934-2964` before producing a prepared input;
- pure preflight defensively repeats the identical parse at
  `module_extension.rs:342`; and
- `HostSelectedExtensionEvaluationInput` is constructed only at
  `selected_repo_spec.rs:3887-3893`, its fields are private at 3752-3758, and
  external consumers can only borrow its parts.

Remove the synthetic Label terminal from
`observed_pure_identity_finisher_and_prefix_algebra`. Replace it with a
bounded static scan that pins the private producer, loaded-definition parse,
prepared join and pure defensive reparse. Retain the lawful real assertions
that the prepared input equals its public raw request and its target parses.
Reuse rather than duplicate the committed `bzl_module.rs` prepared proof: its
real-order test owns the exact observed raw then observed loaded-definitions
dependency vector, and its same-module production/test proof owns the private
loaded-request equality join. Do not type-access or clone that private request
from pure, add an accessor, edit `bzl_module.rs`, use a runtime malformed request,
visibility hook, Label prepared-injection or lower malformed epoch. Keep all
accepted key/finisher/prefix algebra proof.

## Missing proof obligations

Keep the test names and the accepted proof in all three tests. Amend
`observed_pure_real_order_terminals_events_and_parity` to prove:

- both later-preflight Bzl and drift failures occur after an earlier successful
  preflight but before invocation, and legacy publishes no invocation print;
- the real observed pure parent dependency row is exactly observed prepared,
  then the first and second observed Host-Bzl children in input order;
- prepared failure has exact Legacy/Observed semantic Result parity and neither
  path publishes a pure parent batch;
- every activation row collected from a warm transaction—prepared, Host-Bzl
  and pure parent, evaluated/reused as applicable—is batchless; and
- all existing child-before-parent order, observed child-only load batches,
  empty Complete batches, terminal prefixes, first-failure suppression,
  invocation/non-None behavior and Legacy/Observed parity remain proven.

Amend `observed_pure_lifecycle_cancellation_and_nonactivation` to hold the
pure, prepared and Host-Bzl Result+epoch carriers for every row and prove:

- root-tag A -> B -> A changes prepared and pure semantics, keeps Host-Bzl
  semantics stable and restores all three A projections;
- implementation-source A -> B -> A changes Host-Bzl, prepared and pure
  semantics and restores all three A projections;
- the metadata-only row keeps all three semantic Results equal to A while each
  carrier has a different transaction-local epoch;
- each historical carrier remains validated only against its own
  transaction's global epoch; and
- poll-drop publishes no parent, recovery computes all three carriers, each
  recovery epoch matches its own global epoch, and the prepared and Host-Bzl
  epoch maps are subsets of the recovered pure epoch.

Preserve historical handles, exact cached-Reused pointer requirements,
semantic/projection comparisons across transactions, family separation and
exact upper/`slug-command:` nonactivation. Every warm row is batchless; no
child or parent replay is permitted.

## Validation, compatibility and terminal

Reuse pinned Bazel 9.2 `RegularRunnableExtension.load` and
`SingleExtensionEvalFunction` evidence plus accepted prepared/Host-Bzl/pure
tests; add no oracle. Run serially:

- focused `observed_pure_` tests;
- protected `real_repository_rule_`, `observed_prepared_` and `observed_bzl_`;
- full `cargo test -p slug_loading_v2`;
- direct dependent `cargo check -p slug_core_v2`;
- `cargo fmt --all -- --check`;
- verify lines 1-894 still hash to
  `bdee0efe2873997c4a90429cb0a6912cd809f77fb6e0f2657688817d8ae6b738`,
  production remains +301/-171 and all proof/physical caps hold; and
- `git diff --check`.

Existing pure values/errors/order/evaluator ABI/repository-rule receipts and
event behavior remain exact Bazel 9 compatibility. The private key, carrier,
typed outer and Result-Arc/epoch association remain Slug-native.
Instantiation, validation, generated/public/root-mapping/bootstrap activation
and exact Bazel configuration/output/ActionKey bytes remain
unsupported/deferred.

Implementation ACCEPT returns only to a docs-only instantiation frontier
audit. REPLAN on any production-byte or production-accounting change,
private loaded-request typed access or accessor, `bzl_module.rs` edit,
runtime/prepared Label injection, deletion of accepted terminal/event/
lifecycle/family proof, proof or physical cap widening, second file/key/
adapter/owner, visibility/export/caller change, semantic/event/retention drift,
retained Starlark heap/callable, lock or manual task across DICE, upper
activation, fixture/oracle work, milestone closure, M8/M7B or exact identity
work. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

The first correction removed the false runtime Label terminal and completed
the missing event/lifecycle proof. Its remaining miss was assigning the pure
test ownership of a private prepared-layer join. The final retry reuses that
committed proof and adds only the pure parent's real ordered dependency row.
