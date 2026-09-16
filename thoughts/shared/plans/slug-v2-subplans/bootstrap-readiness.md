# Bootstrap readiness: M7A to M8

## Authority and finite production boundary

This is a planning/evidence inventory. Canonical Live Status and
[current-packet.md](./current-packet.md) alone select work. It creates no runtime
manifest, generated action graph, command bypass or source-acquisition authority.
M8 begins after the bootstrap-critical rows pass; unrelated M7B breadth does not
block it. `REPLAN` candidates remain unapplied until separately resumed.

The initial root is `//app/slug_cli_v2:slug`, with its declared runtime files.
[Stage 10's accepted production inventory](./10-bazel-build-and-bootstrap.md#accepted-production-inventory)
names all 33 first-party packages: 14 V2 and 19 retained infrastructure packages,
including five local proc-macro packages. External crates and their build scripts
are supplied by the pinned crate universe. Generated-source obligations are the
LALRPOP grammar, five REAPI protos and compiler-configuration build scripts.
The 43 tests in
[the fixed developer manifest](../../../../tests/v2_oracle/buildbuddy_cache_targets.txt)
are accepted Bazel developer-gate evidence; they are not additional Slug
bootstrap production roots or a requirement to implement Slug `test` first.

This records the known boundary at the documentation review baseline
`c5e7414d77b203a04337c980aacd0d37fd73e501`. It does not claim a fresh Bazel/Cargo
closure audit. Preserve the accepted evidence; a future selected packet must
reconcile live source/feature/toolchain/dependency deltas before calling the
inventory complete. No audit, execution or network access is performed by this
planning change.

## Selected live inventory reconciliation (2026-09-15)

Current packet `WP-7-10-m7a-production-inventory-reconcile-r1` freezes clean
main `39e3a89ac` and compares the live `//app/slug_cli_v2:slug` closure with the
accepted inventory. It authorizes one locked/offline Cargo metadata snapshot
rooted at the CLI manifest and Linux normal/build edges, plus one
network-isolated, no-fetch Bazel 9.2 batch query with streamed JSON, each under
60-second TERM/three-second KILL limits and output caps. Exact binary,
authority/output and cleanup receipts are mandatory. The result must keep
Cargo's selected package closure distinct from Bazel's unconfigured declared
superset, map paths/packages explicitly and enumerate local feature,
proc-macro, build-script, generated-source and pin deltas. It changes no code,
manifests, lockfiles, toolchains or fixed developer manifest; executes no
action/test; invokes no upstream oracle; and admits no capability from query
reachability alone.

The Cargo side completed with a 35-local/310-external Linux normal/build
closure and frozen receipt `b3886868867d6b2c6f8d1d49575fbc0f4322ab83216e1da456ae7e2f1e2c5e2`.
The sole Bazel query exited 37 before emitting any query row because the fresh
namespace left loopback down and Bazel 9.2's metrics collector crashed.
Recovery packet `WP-7-10-m7a-production-inventory-bazel-recovery-r1` permits
one identical query after asserting the namespace contains only `lo` and
bringing that device up; zero external interfaces and every prior query limit
remain fixed. Its receipt records pre/post interface names `["lo"]`, loopback
UP and `external_interfaces=0`. Cargo may not rerun, and another failure ends
this inventory path.

The recovery subsequently exited at its supervisor/interface gate before Bazel
invocation. Receipt
`bc4c11d4a939afa8b143f1bbc484d2d70a212a993111f79946f83f703209b252`
contains empty streams and exact cleanup, so live Bazel coverage remains
unknown. Preserve the successful Cargo evidence and defer partial accounting.
The clarified F3 replay subsequently reached its clean 30-second ceiling before
a typed terminal. Independent result review returned `REPLAN`; that F3 path is
closed without retry or extension. Partial M7A accounting may now resume only
from the frozen Cargo closure and existing authenticated artifacts. Bazel
declared reachability remains unknown; no M7A query or behavioral admission is
reopened.

The partial reconciliation finds 35 Cargo-selected local packages, exactly the
accepted 33 plus `slug_configuration_v2` and `slug_starlark_v2`. Static mapping
finds a BUILD target for 34; `app/slug_configuration_v2` has none despite normal
Cargo dependencies from the CLI closure. The five local proc macros and five
build scripts are enumerated in [Stage 10](./10-bazel-build-and-bootstrap.md#partial-live-cargo-inventory-reconciliation-2026-09-16).
Static key comparison also finds `slug_starlark_v2` and eight selected external
name/version keys absent from `Cargo.Bazel.lock`; see Stage 10 for the exact
list. These are proven local graph/lock gaps, not Bazel reachability or
functional admission. Keep the first readiness row open until the missing
BUILD owner, lock synchronization and fresh Bazel graph evidence are reviewed.
The accepted `WP-7-10-m7a-cargo-cache-acquisition-r1` packet staged
missing full-workspace inputs required by the Bazel lock generator. It ran no
Bazel command or test and did not close this or any other M7A row. The BUILD,
lock and fresh graph repairs require a separately reviewed successor.
The one locked fetch has populated the host cache and the sole offline
full-workspace Cargo metadata output contains all 448 locked registry keys;
the metadata receipt wrapper had a postprocessing defect documented in the
acquisition receipt. Independent final review accepted this input preparation only;
the readiness row remains open.
The `WP-7-10-m7a-bazel-graph-sync-r2` packet added the static BUILD owner and
six edges but stopped before generated-lock repair. This readiness row
still needs fresh declared/configured evidence and the other listed proofs.
The first repin stopped at Bazel's network profiler before lock generation;
the `WP-7-10-m7a-bazel-graph-sync-recovery-r1` permitted one reviewed
flag-only recovery. No readiness row closes from these static edits.
That recovery generated a candidate lock but reformatted two blank lines in
root Cargo.lock, so its exact-byte gate returned `REPLAN`. Original bytes were
restored. The selected
`WP-7-10-m7a-generated-lock-candidate-validation-r1` checks the candidate
against restored authority once, without build, test or target graph query;
the first readiness row remains open.

## Readiness matrix

“Accepted bounded” means only the stated slice has proof. “Open” or “configured
only” is not a runtime admission. The table is a finite set of exit obligations;
unobserved action families cannot be declared unnecessary by assumption.

| Obligation in the production closure | Current status / reusable evidence | Owner | Required exit evidence |
|---|---|---|---|
| Bazel builds the CLI root, its 33-package closure and generated sources | Accepted developer graph; partial live Cargo inventory is 35 local packages with two additions; `slug_configuration_v2` lacks BUILD, `slug_starlark_v2` and eight external keys are absent from `Cargo.Bazel.lock`, and Bazel reachability is unknown | Stage 10 | Add/review missing BUILD ownership, synchronize the lock, then prove live declared/configured root reachability, features, generated inputs and pin correspondence without excluding a production input |
| Authentic MODULE, registry, archive, built-in and generated repository sources | Partial. Normal Run registry parity accepted at `47163df7b`; authentic complete R2 fixture closure remains unresolved. Stage 5 owns sources and registrations | Stages 4/5 | Portable authentic inputs and ordinary source/mapping/registration demand for the selected production closure; no synthetic package, user-CAS default-test dependency or hidden Bazel semantic delegation |
| rules_rust/provider/transition/toolchain evaluation | Partial configured analysis; accepted generic declarations/providers/Args/runfiles do not prove the full rules_rust closure. Canonical M7 status names accepted owners | Stages 4/5/6 | Ordinary configured results for the production root with source-derived toolchain/provider/transition dependencies; exact named semantics and structural invalidation |
| Named/automatic execution groups | Accepted bounded with R2 at main checkpoint `71d9ce4bf`; pinned-source runtime names, target-context parsing, property precedence, action ordering, root provenance/depset shape and subrule boundary passed joint gates | Stages 4/5/6 | Preserve the accepted boundary; authentic F3 remains unaccepted and closed at its 30-second ceiling |
| `attr.label` computed-default invocation | Accepted with the atomic stack at `71d9ce4bf`; omitted invocation, typed Label/None, explicit bypass, lexical context, A/B/A and configured Exec proofs pass | Stages 4/6 | Preserve accepted ownership; F3 supplies no further configured-source admission |
| Native configurable `alias.actual` | Accepted with the atomic stack at `71d9ce4bf`; retained expression, selected branch/conditions, target/exec A/B/A and query proofs pass | Stages 4/6 | Preserve accepted ownership; F3 supplies no further configured-source admission |
| Requested-root output conflict freedom | Accepted bounded with execution groups at `71d9ce4bf`; `ValidatedActionClosure`, pre-execution rejection, positive sharing and direct consumers passed, with eight Core nonpasses attributed to unchanged main | Stage 6/Core | Preserve root-set validation before build/aquery/Run success; cold/warm A+B conflicts produce no execution or materialization even if A and B separately succeed |
| FileWrite | Accepted bounded aquery/REAPI handoff, Stage 7 canonical FileWrite projection and M5/M6 evidence. Its exact ActionKey projection remains queued | Stages 6/8/7 | Preserve accepted content/platform/protobuf/cache proof and named Slug-native token exception; land exact projection when separately selected |
| Spawn: compiler, linker, proc-macro/build-script and generated-source tool invocations | Configured common non-callback Spawn/FilesToRun expansion accepted (`bfe6f2690`, `21db5d7b8`); broader aquery/REAPI activation open; exact ActionKey deferred | Stages 6/8/7 | Source-derived invocation/tool/env/input/output semantics for observed production actions, structural invalidation, classified aquery fields and same-owner REAPI projection |
| ArgsWrite / paramfiles | Configured vector Args/param-file ownership accepted (`a01a23fe7`); execution/aquery family admission open; exact ActionKey deferred | Stages 6/8/7 | Exact encoding/content/order, declared input-tree placement and CAS bytes; typed comparator provenance for generated paths and classified ActionKey field |
| Symlink and runfiles-support action families | Typed symlink declarations, runfiles and four support actions/FilesToRun expansion are configured-only admission (`f346c209a`, `f46a009a0`, `21db5d7b8`); execution/aquery open; exact ActionKeys deferred | Stages 6/8/7 | Inventory names each required family; admit its semantic aquery/REAPI projections and outputs/runfiles behavior with explicit ActionKey classification, or prove absence from this closure |
| Ordinary source/generated/tree input transfer and output materialization | Bounded FileWrite inline input accepted; ordinary source/generated trees and broader materializer remain open | Stages 6/7 | Exact Directory topology/content/digests, declared output ownership/type/mode/symlinks, generated-output consumer/reupload and missing/corrupt data behavior; validated-root-only publication |
| Per-family graph/comparator coverage | Bounded FileWrite text/literal/deps admission accepted; production action graph comparison open | Stages 6/8/10 | Matched focused per-family graphs and selected formats, preserved accepted exact ActionKeys, explicit per-family deferred/native key allowances, typed argv/env/paramfile path correspondence and discriminating negatives |
| Shared cache core used by bootstrap | Open; `slug_reapi_v2` contains protocol/cache code and Slug graph dependencies | Stages 7/11 | Graph-independent protocol/CAS/AC boundary consumed by Slug, exact digests and bounded transfers for demanded artifacts; standalone release and direct disk interoperability are post-M8 |
| Production REAPI execution/cache proof | Slug M6 accepted only bounded FileWrite on retained Linux NativeLink; Bazel BuildBuddy developer proofs do not widen Slug admission | Stage 7 | Required production families execute with exact upload/AC/output evidence and zero direct-local build actions; selected backend/capabilities and lifecycle evidence only |
| Stage0/1/2 bootstrap fixed point | Open; developer-built binary alone is not self-hosting | Stage 10 | Stage1 binary actually launches stage2; independent miss execution and separate replay proof, graph equality and exact output manifests/digests except named build-info normalization |

## Family admission and completion procedure

1. Reconcile the accepted production inventory with the live root graph. Reuse
   existing authenticated Bazel 9.2 artifacts where sufficient; any new oracle
   invocation requires its selected packet. Record exact root/config/toolchain
   pins, action family names, producer targets and the evidence artifact/commit.
   Before freezing a successor's scope, attach a compact `Demanded by / evidence`
   record for each selected capability, including named/automatic execution-group
   policy. The matrix's unresolved execution-group row is not proof of demand.
   Reconcile existing artifacts first; request a bounded new observation only
   for a specific evidence gap. Do not use this inventory as runtime input.
2. Split only the unresolved production families/capabilities into observable
   packets, with Stage 6 semantic producer, Stage 8 query projection and Stage 7
   execution consumer linked. Source ownership and output-conflict gates precede
   activation. Bundle one behavior family's source/oracle/implementation/proof
   where the guide allows; do not require the entire Stage 7 backend or Stage 8
   formatter catalogs first.
3. Each observed family records semantic, aquery-field and REAPI compatibility
   separately. Exact ActionKey projection is not a functional admission gate.
   Preserve accepted exact projections and classify unavailable keys as
   unsupported/deferred, or define a named Slug-native token under Stage 8.
   The comparator allows only those declared family/field differences. Complete
   semantic inputs, structural invalidation, output ownership and exact REAPI
   bytes remain mandatory. Exact projection packets use the
   [Stage 6 ActionKey feasibility checkpoint](./06-analysis-toolchains-and-actions.md#per-family-actionkey-feasibility-checkpoint)
   under M9; comparison-only path correspondence cannot supply runtime keys.
4. Mark a row accepted only with a reachable discriminating proof and coverage
   of its requested production consumers. An “absent from bootstrap” result
   needs the reconciled graph evidence, not an unsupported runtime guard. New
   observed families update this finite table before M7A acceptance; hidden or
   unmodeled fields, fake sources and alternate execution paths stop admission.
5. All rows except the fixed-point row close M7A for this production closure.
   Independently review the coverage/accounting once, then run Stage 10.3 ordinary graph
   comparison and Stage 10.4 fixed-point proof. The final row closes M8, not M7A.
   `run`, `test`, BEP, unrelated public rulesets and nonrequired formats stay
   M7B; existing accepted bounded Run remains a regression.

## Evidence and implementation limits

Bazel 9.2 source/tests and Stage 1 provenance remain the exact oracle. Stage 6's
configured-action conflict contract names `OutputArtifactConflictTest` and
`Actions`; its DICE root ownership is authoritative. Stage 7 owns the canonical
FileWrite/REAPI serialization and action-domain firewall. Stage 8 owns admitted
query and per-family ActionKey projections. Stage 10 owns comparison and the
fixed-point outputs. Their historical indexes at `c5e7414d7` retain detailed
accepted contracts without repeating worker chronology here.

This matrix adds no code, DICE keys, caches, retained values, comparator or
fixtures. Runtime request/revision, memory lifetime, cancellation/join and
publication requirements remain with those owners and must appear in their
implementation packets. There is no fallback to delete because none is added.
The selected portable fixture policy and demand-gated acquisition rule are in
[configured-cli-fixture.md](./configured-cli-fixture.md). Use ordinary execution
permissions for acquisition; this inventory never authorizes reading credentials
or treating an absent catalog payload as proven runtime demand.
