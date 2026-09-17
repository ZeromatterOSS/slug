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
The sole no-repin validation passed under a fresh Bazel output base with
unchanged authority and BUILD hashes; independent final review of static
graph/lock synchronization accepted it. No target reachability or M7A row
closes from that validation alone.
The accepted `WP-7-10-m7a-bazel-declared-root-query-r1` obtained one fresh
unconfigured root target view. Even complete declared path reachability does
not close this row without configured/action and behavioral evidence.
The fresh query yielded all 35 selected local package paths and the named
configuration/Starlark targets in its unconfigured declared view; independent
result review `ACCEPT` confirmed the labels. This does not yet prove configured
action reachability or close the first readiness row.
The selected `WP-7-10-m7a-bazel-configured-root-cquery-r1` checks the full
CLI root's configured label/token view once. Its seven-character tokens do
not prove full configuration identity or target/exec classification; even
successful cquery rows need separate
feature, generated-input, action and compilation proof for this row.
The sole configured cquery stopped at Rust toolchain resolution with zero
configured rows. Receipt
`80e255dbdffa001fa48f05d78d3b10d1ba9ed3c489a4c184e95b6238b06cd4d9`
records a clean 2.862-second failure and unchanged tracked inputs. The
invocation omitted the nightly channel setting required by the registered
toolchains; this row remains open.
The selected flag-only successor permits one new full-root cquery with the
registered nightly channel. No row closes until its configured output, and
later feature/generated-input/action/compilation evidence, are reviewed.
That sole corrected cquery reached nightly toolchain selection but stopped at
an uncached compiler archive with downloads disabled. Receipt
`9f305306861c887bffc9ca6dbde4c1930cf8733cfa91cbe97b41c3b55364ab5c`
records zero configured rows, clean cleanup and unchanged tracked inputs.
The selected successor pins and stages exact Linux x86_64 toolchain archives;
it does not close this readiness row or run another cquery.
The toolchain packet pinned all six official Linux x86_64 hashes, left the
module lock and other frozen inputs unchanged, and locally materialized the
nightly tools repo with downloads disabled. Acquisition receipt
`c84576af16f7c46bac05eb878d052b2196278473e32a280db9e2219b072162e2`
and fetch receipt
`1a47242a21971bbd5b63ae5d21f49f9a7a079d916643e8b02e187ef4ab7e8b10`
prove input preparation only. The first readiness row still needs configured
root, feature, generated-input, action and compilation evidence.
The selected successor permits one offline configured-root cquery from the
prepared toolchain cache. This remains observation only; the first readiness
row still requires later feature, generated-input, action and compilation
proof even if all selected package paths appear.
The one offline cquery yielded 17,685 valid configured label/token rows in
5.337 seconds with unchanged tracked inputs. All 35 selected local package
paths have non-null rows, but seven-character display tokens do not prove
full configuration identity or the other required row evidence. Receipt
`459c41aad42c9259adc239b74271c0e7b380bb6970135e4e266ee91e13b322bc`
and parsed analysis
`266cf64932c2ef555ca5d36bb8d87ad95a8b0fe6a641a4c41973527f0c711dde`
advance configured label reachability only; the first readiness row stays
open.
Independent final review `ACCEPT` confirmed the 35 non-null selected paths
against frozen Cargo manifests. This closes only the configured label/path
observation slice, not the first readiness row.
The selected local feature-flag packet compares all 35 selected local Cargo
package feature sets with checked-in Rust rules and five build-script feature
inputs. It scopes a four-BUILD correction for missing `default` names and two
build-script lists. This does not close external features, generated inputs,
configured actions, compilation or the first readiness row.
The four BUILD edits passed the frozen 35-package/41-target local feature-set
comparison with zero mismatches, including all five build scripts. Independent
final review `ACCEPT` confirmed unchanged frozen locks/MODULE and the narrow
static claim. No Cargo/Bazel command, build or test ran. The first readiness
row remains open for external features, generated inputs, configured actions
and compilation.
The selected Tokio feature probe checks whether Cargo metadata's
`windows-sys` feature is present in the actual Linux CLI Tokio unit. Its
single offline unit-graph command performs no compilation and cannot close
the external feature, action, build or first-readiness-row gates alone.
The sole unit-graph command exited 0 in 0.506 seconds without compiling.
The one root-reachable normal Linux Tokio library unit matches the Bazel
lock's 22 Linux features and omits metadata-only `windows-sys`; independent
final review `ACCEPT` confirmed the raw row and clean receipt. This resolves
only Tokio's feature-list ambiguity; the first readiness row stays open.
The selected saved-graph successor accounts for all external Cargo unit IDs
and host/target feature lists against the generated Bazel lock. It introduces
no new build or test; structural list differences require later configured
action evidence before this readiness row can close.
The saved CLI graph has 308 external package IDs; frozen metadata's
`getrandom 0.3.4` and `libm 0.2.16` have no CLI unit. Among 344 normal
external units, 322 feature sets equal the lock's Linux list and 22 are
narrower, including five Linux target units. Independent final review
`ACCEPT` recomputed the analysis at SHA-256
`42c2aeb3e1e0624ef705fcab21e05d57037151913ef25fdc0e65c493194a1f7f`.
This does not establish configured Bazel action flags or close the first row.
The selected successor inspects one filtered CLI-root `aquery` for the five
Linux target crates with wider generated-lock feature lists. Exact action
owners and feature flags may classify those five differences but cannot
close other host units, compilation or the first readiness row.
The sole aquery found five exact non-tool Rustc actions whose direct
feature flags equal the wider lock lists. Independent final review `REPLAN`
because three actions consume generated `_bs.flags` absent from analysis
output. Receipt SHA-256 is
`040e2e4c49298f445a6be66310f250587c56a476ad05ff360c0173f658eb2606`.
No action ran; complete configured compiler arguments and the first
readiness row remain open.
The selected source-only successor audits whether the three unresolved
`_bs.flags` producers can emit additional named `feature=...` cfgs. It
cannot infer actual generated values or close compilation/readiness gates.
The pinned source audit rules out additional named feature cfgs from
`ahash` and `rustix` build-script output paths, but `num-traits`'s inherited
probe stdout remains opaque. Independent final review `ACCEPT` confirmed
source analysis SHA-256
`0befd1d6b4187984859ee3a07769a50ff314f092217d8b1d303f5bc14251bb17`.
No action ran and the first readiness row remains open.
The selected `num-traits` action-environment query narrows the remaining
inherited probe-child stdout question to configured wrapper variables.
Generated flag bytes, execution and the first readiness row stay open.
The sole query exited 0 in 4.154 seconds and found the exact non-tool
`num-traits` build-script action plus a separate tool action. No wrapper key
appears in the selected action's aquery-visible fixed environment; inherited
wrapper state and generated flag bytes remain unknown. Receipt SHA-256 is
`ffba1990df9f5a6051cf180859f562a949f632d9949851abd0461570e12696dd`;
analysis SHA-256 is
`b3e727f268ad66c7ae9c6ba7cd90c067d5ee041a5d69054bc29f2f3375240ae4`.
Independent result review `ACCEPT` confirmed this narrow bound. No build or
test action ran and the first readiness row remains open.
The selected successor builds only the generated `num-traits` build-script
alias under a 20-second bound to inspect exact `_bs.flags` bytes. A successful
observation can classify that script's feature cfg output; it cannot prove
the rest of the compiler graph or close this row.
The sole build stopped at its 20-second bound before producing `_bs.flags`;
the 211/246 action progress counter does not prove the requested run action
executed. Receipt SHA-256 is
`5b998a1de1942c4d54d4d1131be659b07467371729df6e042b7e466ccc3f0d74`.
Independent result review `REPLAN` confirmed clean cleanup and unchanged
tracked/frozen inputs. The packet has no retry; generated flags, compilation
and the first readiness row remain open.
The selected saved-artifact successor checks whether the 22 narrower CLI
Cargo feature sets correspond to workspace-wide Cargo metadata and pinned
crate_universe resolution scope. It cannot establish generated flag contents,
compilation or the first readiness row.
The static comparison found 343/344 normal external unit rows with Linux
lock features equal to workspace metadata. Target Tokio alone omits
metadata-only `windows-sys` on Linux; all 22 CLI-narrower unit rows equal
metadata at the lock feature set. Independent final review `ACCEPT` confirmed
analysis SHA-256
`2d30fd2b80942d2be57e558dca86730f7d897f05d2aaa481cc1b5c048eedb90e`.
This reconciles saved feature-list scope only; configured flags, generated
flags, compilation and the first readiness row remain open.
The selected first-party generated-input action query checks the CLI root's
LALRPOP and REAPI proto build-script outputs as Rustc inputs. It is
analysis-only and cannot prove generated bytes, compilation or this row.
The sole 6.267-second query found both first-party `build_script.out_dir`
tree artifacts in their corresponding library Rustc input closures by exact
artifact ID. The grammar, five protos and eight vendored protoc files are
producer inputs, with Linux x86_64 protoc selected in REAPI's environment.
Independent final review `ACCEPT` confirmed receipt SHA-256
`f30b6ba9450cd26b36fe8bdcfd6e6bc529fb36d8bc6226b5459ac1389c3deb6e`
and analysis SHA-256
`f77396e8ea9f7fdf59d05bdd18425ed6bc42c29559bc1b60e3ea3a2379021836`.
Generated bytes, compiler success and this readiness row remain open.

WP-7-13 adds a live dependency and generated-input owner:
`slug_reapi_cache_v2` now owns the pinned upstream REAPI/Google import closure
and vendored-protoc build script, and `slug_reapi_v2` consumes its protocol/CAS/AC
API. The accepted five-proto `slug_reapi_v2` action-query receipt above is
historical evidence, not proof of this new producer edge. WP-7-14's single
CLI-root aquery found new cache-leaf tree artifact 3518 in the leaf Rustc input
depset chain 615 → 616 → 617, all 15 proto inputs and Linux x86_64 protoc,
with the old adapter producer absent; it also retained the LALRPOP edge.
Bazel ran zero actions. Its `--nofetch` warning leaves the external crate
repository's freshness unproved. This closes the new first-party declaration
observation only; generated bytes, compilation, complete live coverage and
the first readiness row remain open.

## Readiness matrix

The accepted Spawn parameter-file source audit found that the
CLI rules_rust Rustc and cargo build-script paths use virtual
`Args.use_param_file` inputs. Aquery `--include_param_files` covers a
different, artifact-backed action path, so it cannot establish those virtual
bytes. WP-7-20/21/22 accept source-pinned regular-crate-root, four regular-File
dirname and two nonempty crate-provider sites, with real `construct_arguments`
configured Spawn, metadata/alias selection, parameter bytes and same-DICE
A/B/A proofs. WP-7-23 adds forced replacement argv/virtual bytes and atomic
REAPI input-tree composition with output/input collision checks. Other builder
paths, native-link callbacks, conditional spilling and resolved-input execution
staging remain open. The Spawn and Args/param-file rows therefore remain open;
these configured/component slices do not prove Rustc execution.

“Accepted bounded” means only the stated slice has proof. “Open” or “configured
only” is not a runtime admission. The table is a finite set of exit obligations;
unobserved action families cannot be declared unnecessary by assumption.

| Obligation in the production closure | Current status / reusable evidence | Owner | Required exit evidence |
|---|---|---|---|
| Bazel builds the CLI root, its selected Cargo closure and generated sources | The named `//app/slug_cli_v2:slug` production root builds under pinned Bazel 9.2/nightly rules_rust, with nonempty LALRPOP and cache-leaf proto trees. Earlier static/configured/local-feature and exact embedded-input slices remain accepted; five observed `ahash`/`num-traits`/`rustix` generated flag artifacts add no named feature cfg. Live Cargo/Bazel closure, other external feature/pin/action correspondence and Slug behavioral admission remain open | Stage 10 | Reconcile the live cache-leaf closure and prove remaining feature, pin and action correspondence without excluding a production input; preserve the successful named CLI-root build |
| Authentic MODULE, registry, archive, built-in and generated repository sources | Partial. Normal Run registry parity accepted at `47163df7b`; authentic complete R2 fixture closure remains unresolved. Stage 5 owns sources and registrations | Stages 4/5 | Portable authentic inputs and ordinary source/mapping/registration demand for the selected production closure; no synthetic package, user-CAS default-test dependency or hidden Bazel semantic delegation |
| rules_rust/provider/transition/toolchain evaluation | Partial configured analysis; accepted generic declarations/providers/Args/runfiles do not prove the full rules_rust closure. Canonical M7 status names accepted owners | Stages 4/5/6 | Ordinary configured results for the production root with source-derived toolchain/provider/transition dependencies; exact named semantics and structural invalidation |
| Named/automatic execution groups | Accepted bounded with R2 at main checkpoint `71d9ce4bf`; pinned-source runtime names, target-context parsing, property precedence, action ordering, root provenance/depset shape and subrule boundary passed joint gates | Stages 4/5/6 | Preserve the accepted boundary; authentic F3 remains unaccepted and closed at its 30-second ceiling |
| `attr.label` computed-default invocation | Accepted with the atomic stack at `71d9ce4bf`; omitted invocation, typed Label/None, explicit bypass, lexical context, A/B/A and configured Exec proofs pass | Stages 4/6 | Preserve accepted ownership; F3 supplies no further configured-source admission |
| Native configurable `alias.actual` | Accepted with the atomic stack at `71d9ce4bf`; retained expression, selected branch/conditions, target/exec A/B/A and query proofs pass | Stages 4/6 | Preserve accepted ownership; F3 supplies no further configured-source admission |
| Requested-root output conflict freedom | Accepted bounded with execution groups at `71d9ce4bf`; `ValidatedActionClosure`, pre-execution rejection, positive sharing and direct consumers passed, with eight Core nonpasses attributed to unchanged main | Stage 6/Core | Preserve root-set validation before build/aquery/Run success; cold/warm A+B conflicts produce no execution or materialization even if A and B separately succeed |
| FileWrite | Accepted bounded aquery/REAPI handoff, Stage 7 canonical FileWrite projection and M5/M6 evidence. Its exact ActionKey projection remains queued | Stages 6/8/7 | Preserve accepted content/platform/protobuf/cache proof and named Slug-native token exception; land exact projection when separately selected |
| Spawn: compiler, linker, proc-macro/build-script and generated-source tool invocations | Configured common non-callback Spawn/FilesToRun expansion accepted (`bfe6f2690`, `21db5d7b8`), plus source-pinned root/dirname/crate-provider callback publication (WP-7-20/21/22); broader aquery/REAPI activation open; exact ActionKey deferred | Stages 6/8/7 | Source-derived invocation/tool/env/input/output semantics for observed production actions, structural invalidation, classified aquery fields and same-owner REAPI projection |
| ArgsWrite / paramfiles | Configured vector Args/param-file ownership accepted (`a01a23fe7`); forced virtual argv/bytes and atomic input-tree composition added in WP-7-23; execution/aquery family admission open; exact ActionKey deferred | Stages 6/8/7 | Exact encoding/content/order, declared input-tree placement and CAS bytes; typed comparator provenance for generated paths and classified ActionKey field |
| Symlink and runfiles-support action families | Typed symlink declarations, runfiles and four support actions/FilesToRun expansion are configured-only admission (`f346c209a`, `f46a009a0`, `21db5d7b8`); execution/aquery open; exact ActionKeys deferred | Stages 6/8/7 | Inventory names each required family; admit its semantic aquery/REAPI projections and outputs/runfiles behavior with explicit ActionKey classification, or prove absence from this closure |
| Ordinary source/generated/tree input transfer and output materialization | Bounded FileWrite inline input accepted; ordinary source/generated trees and broader materializer remain open | Stages 6/7 | Exact Directory topology/content/digests, declared output ownership/type/mode/symlinks, generated-output consumer/reupload and missing/corrupt data behavior; validated-root-only publication |
| Per-family graph/comparator coverage | Bounded FileWrite text/literal/deps admission accepted; production action graph comparison open | Stages 6/8/10 | Matched focused per-family graphs and selected formats, preserved accepted exact ActionKeys, explicit per-family deferred/native key allowances, typed argv/env/paramfile path correspondence and discriminating negatives |
| Shared cache core used by bootstrap | Bounded FileWrite leaf accepted in WP-7-13; broader bootstrap transfer coverage open | Stages 7/11 | Extend the graph-independent protocol/CAS/AC boundary only as demanded by observed production artifacts; standalone release and direct disk interoperability are post-M8 |
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
