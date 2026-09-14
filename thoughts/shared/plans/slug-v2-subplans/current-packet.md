# Current Slug V2 Work Packet

Packet: WP-6-7A-selected-request-output-conflict-r2-reconcile-r1
Status: ready; independent prerequisite-order design review ACCEPT

## Result and owner

Reconcile the preserved selected-toolchain request and configured-action
output-conflict candidate onto current `main`, close its missing evidence, and
accept both owners atomically if every gate passes. The selected request keeps
the parent's configuration plus a separate selected execution-platform
preference. The root-set action-closure producer rejects incompatible outputs
before execution or materialization and shares only source-established equal
FileWrites through one validated execution view.

This combined checkpoint now precedes shared named/automatic execution groups.
The authentic F3 proof confirms that rules_java toolchains selects rules_cc
`cc_library`, whose retained declaration requires named `cpp_link` and
`_use_auto_exec_groups=True`. F3 is not a semantic input to selected-request or
root-set conflict correctness; replay it only after complete group activation.

Exact: the admitted selected implementation configuration/preference behavior,
FileWrite sharing/conflict behavior and pre-execution consumer failures recorded
in Stage 6. Slug-native: DICE keys, structural identities, closure ordering and
error storage. Deferred: group activation and guard removal, other action-family
equivalence, computed defaults, C++/Java behavior, configured aspects and exact
configuration/output/ActionKey bytes.

## Recovery and cohesion

Create an isolated worktree/branch from the scheduling checkpoint on `main`.
Apply only the app diff from `97dffd5d4..27e9e9c0c`; do not integrate
`review-evidence/` or its embedded patch. The app-only binary diff applies cleanly
at packet selection. Five files changed on both lines but have no textual merge
conflict:

- `app/slug_analysis_v2/src/dice.rs`;
- `app/slug_cli_v2/src/commands/build.rs`;
- `app/slug_cli_v2/tests/cli.rs`;
- `app/slug_core_v2/src/runtime/dice.rs`;
- `app/slug_core_v2/src/runtime/mod.rs`.

Review those five semantically against landed registration diagnostics, daemon
cleanup, observer, archive/file capture and path-shard work. Preserve current
behavior from both lines. The selected-request change is conceptually cohesive,
but this candidate's platform facts, owner identity, FileWrite identity,
validated closure, execution projection and command consumers form one proof
boundary. Splitting it now would activate an unvalidated collision path and
invalidate its evidence. The CLI drain helper is proof infrastructure for those
consumer gates, not a separate product checkpoint.

## Exact app allowlist and limits

Production and proof changes are limited to these 19 candidate files:

- `app/slug_analysis_v2/src/analysis_value.rs`;
- `app/slug_analysis_v2/src/dice.rs`;
- `app/slug_analysis_v2/src/key.rs`;
- `app/slug_analysis_v2/src/result.rs`;
- `app/slug_analysis_v2/src/starlark_rule.rs`;
- `app/slug_analysis_v2/tests/configured_target.rs`;
- `app/slug_analysis_v2/tests/starlark_rule.rs`;
- `app/slug_build_api_v2/src/analysis_value.rs`;
- `app/slug_build_api_v2/tests/analysis_value.rs`;
- `app/slug_cli_v2/src/commands/build.rs`;
- `app/slug_cli_v2/tests/cli.rs`;
- `app/slug_core_v2/src/runtime/configured_action_closure.rs`;
- `app/slug_core_v2/src/runtime/dice.rs`;
- `app/slug_core_v2/src/runtime/file_write_identity.rs`;
- `app/slug_core_v2/src/runtime/mod.rs`;
- `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`;
- `app/slug_core_v2/src/runtime/tests/configured_action_conflicts_tests.rs`;
- `app/slug_reapi_v2/tests/reapi.rs`;
- `app/slug_server_v2/src/reapi.rs`.

Acceptance bookkeeping may update only this manifest, canonical status, the
configured-fixture ledger, Stage 6's current owner/order record and bootstrap
readiness. The preserved baseline is 757 production / 2,103 proof / 2,860 total
gross Rust additions. Review at 900 production, 2,400 proof or 3,300 aggregate;
these are scope triggers, not permission requests.

The Stage 6 selected-request and configured-action closure contracts remain the
complete representation, equality, publication, memory/lifetime and consumer
authority. Add no key family, side registry/cache, command-side conflict scan,
output suffix, fallback, lock across DICE awaits or broader action executor.

## Required corrections and discriminators

Preserve all existing focused candidate tests, then close these recorded gaps:

1. Run the final dotted-property guard and raw-platform/message cutoff matrix,
   including same-DICE A/B/A, unchanged Arc cutoff, cancellation/Need/error,
   overlapping requests and both toolchain/no-toolchain constructor paths.
2. Account for every Core library test in bounded partitions. For every candidate
   failure, run the exact selector on unchanged current `main`. The two failures
   previously reproduced on `97dffd5d4` are inherited only if current main and
   the reconciled candidate still fail identically. Every other old full-Core
   failure is unattributed and blocks acceptance until compared or corrected.
3. Run the existing one-shot and stable-daemon conflict tests against rebuilt
   executables. They must prove build/run/aquery exit 2, deterministic path and
   owners, zero transport calls, unchanged outputs, repeated daemon failure,
   restoration, owner-complete aquery and independent successful cquery. Change
   their internal command deadline to 12 seconds; retain 15 seconds only as the
   absolute outer kill ceiling.
4. Add the missing positive common-boundary proof. Obtain the producer-validated
   execution view from a real completed closure, lower it through
   `FileWriteReapiPlan`, and prove two equal owner actions retain both semantic
   and aquery owners while producing one REAPI plan. The existing standalone
   owner-identity test is only a control.
5. Rebuild and run direct REAPI/server consumers. Preserve current registration,
   repository, observer, archive, path-observation and daemon behavior in the
   five overlapping files.

Failure must precede RPC/materialization and retain typed command errors. Cquery
remains independent. Need, analysis failure and cancellation precedence remain
unchanged; no partial validated closure or command success may publish.

## Validation and acceptance

Resolve the pinned toolchain with `rustup which --toolchain
nightly-2025-09-14-x86_64-unknown-linux-gnu` for cargo, rustc, rustdoc and
rustfmt. Compile each affected test target separately with `--no-run
--message-format=json`; each preparation operation has a 60-second ceiling.
Never run concurrent Cargo commands against one target directory.

Compile serially from the candidate worktree, substituting the cargo path
returned by the pinned `rustup which` command:

```sh
timeout 60s cargo test -p slug_analysis_v2 --tests --no-run --message-format=json
timeout 60s cargo test -p slug_build_api_v2 --tests --no-run --message-format=json
timeout 60s cargo test -p slug_core_v2 --lib --no-run --message-format=json
timeout 60s cargo test -p slug_cli_v2 --test cli --no-run --message-format=json
timeout 60s cargo test -p slug_reapi_v2 --tests --no-run --message-format=json
timeout 60s cargo test -p slug_server_v2 --tests --no-run --message-format=json
timeout 60s cargo check -p slug_query_v2 -p slug_server_v2 -p slug_cli_v2
```

Before execution, preflight exact nonignored selectors with
`scripts/v2_test_preflight.py`; rebuilding invalidates that receipt. Run tests
with a 12-second deadline and 15-second absolute ceiling. Required accounting:

- all selected-toolchain request tests plus existing selected-toolchain,
  root-toolchain, default-exec and rule-transition controls;
- all configured-action conflict tests, including the positive common handoff;
- complete `slug_analysis_v2` and `slug_build_api_v2` suites;
- every `slug_core_v2` library test in bounded exact batches, with current-main
  comparison for each failure;
- the CLI drain control and the one-shot and stable-daemon consumer tests,
  separately, with Unix-socket/loopback capability checked in their environment;
- complete direct `slug_reapi_v2` tests and server/query/core compile dependents.

For each produced executable, use the corresponding concrete forms below; exact
names and batch membership enter the receipt:

```sh
python3 scripts/v2_test_preflight.py <executable> --exact <test> [<test> ...]
timeout --kill-after=3s 12s <executable> --exact <test>
timeout --kill-after=3s 12s <executable> <preflighted-filter>
```

Use the existing current-main worktree as the unchanged comparator and keep the
candidate worktree isolated. Record executable, features, exact selected/pass/
fail/ignored counts, elapsed time and attributed failure text. Run pinned
`cargo fmt --all -- --check`, `git diff --check`, scope/growth checks and
`python3 scripts/v2_plan_status.py`. Obtain independent final invariant review
of the reconciled diff and evidence before integration. Commit and push `main`
only after acceptance; preservation or scheduling commits do not accept R2.

Return `REPLAN` only if reconciliation requires a new semantic owner/key family,
broadens action-family equivalence, cannot preserve landed behavior, or the
positive common handoff contradicts the validated-closure design. Correct
ordinary invocation, selector, compile, test or in-scope implementation failures
within this packet. Do not run F3 or remove the execution-group invocation guard.

## Immediate predecessor

The bounded package-attempt projection passes all 561 active loading tests. Its
authentic F3 receipt selected one test and reached the named group guard in 9.98
seconds with valid observer and cleanup evidence. Independent scheduling review
ACCEPTS the order: combined R2 from non-F3 gates, complete shared group runtime,
then F3. It also confirms the preserved app patch applies cleanly and that the
combined candidate must remain atomic.
