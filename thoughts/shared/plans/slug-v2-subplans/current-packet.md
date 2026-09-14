# Current Slug V2 Work Packet

Packet: WP-4-6-7A-r2-execution-group-combined-r1
Status: ready; Phase A contract freeze selected after independent order review REVISE

## Result and owner

Preserve the reconciled selected-toolchain request/configured-action conflict R2
candidate as an explicitly unaccepted base, freeze the complete named and
automatic rule execution-group contract against that base, implement the group
runtime on top of it, and accept the two owners only as one combined checkpoint.
Then replay the unchanged authentic F3 configured-source proof.

R2 commit `f3c90ea46` is local on
`integration/selected-request-output-conflict-r2`, based on current main
`2b3fedf76`. It contains only the reconciled 19-file app candidate plus the
12-second CLI deadline correction and positive producer-to-REAPI proof. It is a
preservation commit, not accepted behavior, and must not reach `main` alone.
The older preserved source is `27e9e9c0c` based on `97dffd5d4`; exclude all
`review-evidence/` content.

The configured-target-owned group collection is the semantic owner. Loaded rule
declarations produce detached requirements, execution constraints and automatic
policy. Configured analysis resolves every group under the owner's target
configuration and retains group identity, selected execution platform,
configured toolchain providers and merged properties. That one immutable
collection supplies named dependency transitions, `ctx.exec_groups`, action
routing and automatic group inference. R2 supplies selected-platform request
identity and root-set action-closure validation before success, RPC or
materialization.

Exact: the pinned Bazel named/automatic declaration, resolution, transition,
provider, property and action-routing behavior enumerated in Stage 6, plus R2's
selected request and FileWrite equality/conflict contract. Slug-native: DICE
keys, structural configured owners, closure ordering and error storage.
Deferred: configured aspects, built-in test-runner groups unless current demand
proves them, unrelated action-family equivalence, computed defaults, complete
C++/Java behavior, exact configuration/output/ActionKey bytes and execution.

## Why the order changed

The first reconciliation pass completed Analysis, Build API, REAPI and compile
consumers, and accounted for every Core library test. Its required real CLI
one-shot, stable-daemon and positive handoff tests enter builtin `bazel_tools`
transitive registration before their command assertions. They cannot complete
before the retained rules_java/rules_cc named/automatic group declarations are
supported. Minimal local-module stubs merely advanced to the first required
autoload and were reverted; they are no acceptance evidence.

Independent correction review returned `REVISE`: landing groups alone on current
main would consume the still-unaccepted R2 selected-platform identity and
validated closure. The safe boundary is therefore R2 plus complete groups on one
branch, with joint gates and atomic integration. Matching resource timeouts are
open gates, not semantic passes.

## Phase A: freeze the implementation contract

Do not edit runtime code until Phase A is complete and independently reviewed.
Against `f3c90ea46`:

1. Reconcile production-closure demand for named and automatic groups under
   `//app/slug_cli_v2:slug`. Preserve the authenticated F3 chain
   rules_java `toolchains/BUILD:138` -> rules_cc `cc_library.bzl:19`, but do not
   claim broader bootstrap membership without the current Bazel/Cargo closure.
2. Trace the live declaration, configured-target, dependency-transition,
   provider, property and action-factory call paths. Decide the cohesive owner
   and any necessary extension of the existing structural toolchain-resolution
   key. Do not activate `toolchains/exec_groups.rs` as a detached registry.
3. Freeze exact Rust/test files, direct consumers, executable proof selectors,
   upstream adaptations, production/proof growth estimates and review caps.
   Give concrete split/cohesion decisions for large `package.rs` and `dice.rs`.
4. Freeze guard replacement: the loading guard remains until the complete
   collection supplies resolution, transitions, providers, properties and
   action routing. Unknown names fail; no default fallback is allowed.
5. Obtain independent terminal design review. Update this manifest with the
   reviewed Phase B allowlist and caps before the first runtime edit.

Primary source authority is Bazel commit
`8220c6198837d5c13d53fea211cf3282aa12408a` in `/home/wgray/bazel`, using the
Stage 6 source/test matrix. Read only that pinned object when working-tree HEAD
differs. Buck2/DICE sources are ownership and memory guidance only.

## Required Phase B semantics

The reviewed freeze must preserve all of these obligations:

- named requirements and execution constraints resolve independently, including
  constraint-only groups, under the owner's target configuration;
- named dependency edges use the named group's selected execution platform and
  reject unknown names;
- public `ctx.exec_groups` exposes resolved named groups, excludes the default
  group and distinguishes absent collection from an empty toolchain map;
- effective property precedence is platform-default < platform-group <
  target-default < target-group, including the target-default-over-platform-group
  discriminator;
- actions validate the public group then bind that group's owner/platform;
  automatic actions also validate explicit toolchain/group agreement;
- automatic policy comes from the retained native option plus rule attribute
  precedence, derives a distinct structural group per toolchain type and does
  not collapse public named identity;
- equal resolution inputs may share computation but never collapse group/action
  identity;
- the complete immutable collection publishes only after every group resolution,
  child and final source-certificate validation; Need/error/cancellation exposes
  no partial provider or action state;
- no second registry/cache, CLI reconstruction, evaluator-owned retained map,
  lock across DICE awaits, output suffix, fallback or broader action equivalence;
- R2 output-conflict validation remains before command success, RPC and
  materialization, while cquery remains independent.

Required invalidation proof covers same-DICE source/configuration/platform and
policy A/B/A; requirements, constraints, toolchain payload and properties;
absent/edit/delete/recreate; unavailable history; overlapping policies;
unchanged-input Arc cutoff; Need/error/cancellation; and default-only
nonregression. Views and joins are phase scratch; declarations and configured
collections are DICE-retained structural memory with Allocative coverage.

## Joint validation and acceptance

Use the pinned nightly `nightly-2025-09-14-x86_64-unknown-linux-gnu`. Every
compile/preparation command has a 60-second ceiling. Every test command has a
12-second deadline and 15-second absolute outer ceiling. Run Cargo serially per
target directory and preflight every exact nonignored selector with
`scripts/v2_test_preflight.py` after its final rebuild.

Joint acceptance requires:

- all reviewed declaration and configured group tests, including named,
  constraint-only, automatic-policy, transition, provider, property-precedence,
  action-routing and negative controls;
- all R2 selected-request/raw-platform/cutoff/cancellation/A-B-A tests and all
  configured-action conflict/sharing tests;
- complete `slug_analysis_v2` and `slug_build_api_v2` suites;
- every `slug_core_v2` library test in bounded exact partitions, with each
  candidate failure compared by exact selector to unchanged current main;
- the CLI drain control and separately bounded real one-shot, stable-daemon and
  positive completed-closure -> one `FileWriteReapiPlan` tests;
- complete direct `slug_reapi_v2` tests, the direct server REAPI consumer, and
  query/server/CLI compile dependents;
- unchanged F3 with its observer and cleanup receipt after complete group
  activation;
- pinned formatting, `git diff --check`, scope/growth checks,
  `python3 scripts/v2_plan_status.py` and independent final invariant review.

The reconciliation receipt is reusable only for untouched code: Analysis
150/150, Build API 79/79, direct REAPI 23 active with one ignored, direct server
REAPI consumer pass, and query/server/CLI checks pass. Core preflighted 329
nonignored tests with one ignored; 309 passed, 13 assertions failed identically
on current main, and seven exact commands timed out at 12 seconds on both lines.
Those seven remain resource gates and must be diagnosed or replaced by bounded
discriminating proof. Group edits invalidate every affected receipt.

Commit intermediate preservation/design checkpoints on the isolated branch.
Do not push R2 or group behavior to `main` until every joint gate passes.
Integration must be atomic, followed immediately by the unchanged F3 receipt.
Return `REPLAN` if Phase A cannot freeze one configured-target owner, demand is
unresolved, required semantics exceed a bounded reviewed packet, or the combined
runtime cannot preserve landed behavior.

## Immediate predecessor

Commit `2b3fedf76` recorded the now-superseded R2-before-groups order after F3
reached `target invocation for named execution-group semantics is unsupported`
in 9.98 seconds with valid observer and cleanup evidence. The reconciled trial
proved that R2's real command consumers traverse that same builtin registration
boundary. Independent review therefore requires a combined R2-based group stack
and joint acceptance.
