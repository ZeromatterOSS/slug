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
REAPI input-tree composition with output/input collision checks. WP-7-24 adds
observed per-module source provenance needed to authenticate imported helpers.
WP-7-25 uses it for native-library directory and default direct/indirect link
callbacks, with real Linux builder/parameter-byte proofs. WP-7-26 runs public
static/PIC rules_cc constructors into those recipes and retains full CcInfo with
empty native HeaderInfo identity. Other builder paths,
full Cc toolchains/operations, conditional spilling and resolved-input execution
staging remain open. The Spawn and Args/param-file rows therefore remain open;
these configured/component slices do not prove Rustc execution.

“Accepted bounded” means only the stated slice has proof. “Open” or “configured
only” is not a runtime admission. The table is a finite set of exit obligations;
unobserved action families cannot be declared unnecessary by assumption.

| Obligation in the production closure | Current status / reusable evidence | Owner | Required exit evidence |
|---|---|---|---|
| Bazel builds the CLI root, its selected Cargo closure and generated sources | The named `//app/slug_cli_v2:slug` production root builds under pinned Bazel 9.2/nightly rules_rust, with nonempty LALRPOP and cache-leaf proto trees. Current [WP910 compilation](wp910-current-cli-root-build-2026-10-06.json) succeeds in58.721s after current36-local/316-external metadata and direct configured feature/producer reconciliation; [WP911 generated cfg bytes](wp911-generated-argument-cfg-2026-10-06.json) are observed for all201 selected files with no added named features; [WP930](wp930-effective-compiler-arguments-2026-10-06.json) reconciles all513 qualified literal argument streams and1167 ordered shared-vector bindings under its memory caps. Historical runtime argv/environment, producer freshness and authentic Slug execution remain open. [WP931](wp931-generated-compiler-env-files-2026-10-06.json) observes all446 demanded generated env files/190700B and466 ordered bindings to411 saved consumers under its caps; file/OUT_DIR subsets are classified while inherited environment and producer freshness remain unknown. The current packet selects normal retained public Rustc producer readiness through the existing9.2/0.73 probe fixture; no source retry/acquisition is selected. [WP932](wp932-offline-source-guard-2026-10-06.json) accepts the scoped local-only HTTP prerequisite with8 focused gates/current CLI check and33,136KiB conservative Core unit peak; exact local module/import/generated tool readiness remains next, with no actual source/root invocation or full memory-fit claim. [WP933](wp933-finite-source-availability-2026-10-06.json) verifies8 references/206598B and records5 rules_rust+ symlink refusals with zero external body bytes,11 controls and18,968KiB observed child peak. [WP934](wp934-explicit-root-alias-2026-10-06.json) accepts4 controls and one187B absolute alias spelling with zero target/body reads,17,984KiB observed child peak. [WP935](wp935-explicit-local-source-2026-10-06.json) observes all13 rows/325671B with five upstream files119073B,three old source hashes matching,11 reused controls and22,848KiB peak RSS. [WP936](wp936-direct-local-imports-2026-10-06.json) observes16 directlocal files/5092301B at31,180KiB peakRSS,11reusedcontrols;14partialfact tables plusrustc nodeguard/4.75MBknown_shas hashonly preserve unknown imports/initialization. [WP937](wp937-native-source-memory-2026-10-06.json) accepts two complete native parser profiles at23,932/44,236KiB peaks with12/0 native loads,3FD+3protected tests and separate Loading/Core preparation; original source-owner retirement and the required parser copy are accounted. Representative parses fit the existing envelope. [WP938](wp938-normal-offline-source-prerequisite-2026-10-06.json) observes one normal offline public-probe command at63,960KiB/0.211s with16MiB image/scratch budgets, first scope NetworkDisabled/1registryattempt/zero admitted bytes and outer observed_terminal;3protected+1observer tests/Core+CLI preparation and scope retirement pass. Exact denied request/mismatch subtype were not recorded. [WP939](wp939-borrowed-registry-attribution-2026-10-06.json) accepts borrowed exact credited-slab rows and six mismatch labels, nine ordinary passes (1.748 s; 37,320 KiB peak), Core preparation (36.746 s), CLI check (7.373 s) and two FINAL reviews; pointer/domain/order/partial-history/panic recovery/final credit and Weak retirement pass with no new retained storage. [WP940](wp940-exact-offline-registry-prerequisite-2026-10-06.json) observes one newly armed offline command with a sole exact BCR request ending in modules/platforms/1.1.0/MODULE.bazel, first NetworkDisabled and separate Demand mismatch. Fourteen executed tests pass; native 0.218 s/65,860 KiB, recorder credit 279,143 bytes/source peak 344,679 bytes, fixed 16 KiB scratch and credit/recorder/scope Weak release pass under unchanged caps. No source bytes acquired. [WP941](wp941-platforms-local-module-readiness-2026-10-06.json) proves evaluated fixture declaration/forward-request correspondence and observes the exact local platforms MODULE416B (0.052s/18,572KiB) with partial literal1.1.0 facts; seven executed checks (one new test case), corrected preparation and Core check pass. [WP942](wp942-native-local-module-readiness-2026-10-06.json) accepts genuine native426B root MODULE loading/evaluation at0.226s/66,816KiB with source/recorder/scope Weak and credit retirement, five ordinary gates, one native observer and Core/CLI preparation. Normal mapping preserves a classified offline registry transport prerequisite; the first66B diagnostic request names bazel_features1.50.0 but is not sole-offline qualified or associated with a selected typed leaf. Next ground that association or exact local-source availability before another source operation. Platforms evaluation/mapping certification, generated providers, public Rustc and whole-fit remain open; no acquisition/spent-source/full-root retry is selected. Earlier static/configured/local-feature and exact embedded-input slices remain accepted; five observed `ahash`/`num-traits`/`rustix` generated flag artifacts add no named feature cfg. Live Cargo/Bazel closure, other external feature/pin/action correspondence and Slug behavioral admission remain open | Stage 10 | Reconcile the live cache-leaf closure and prove remaining feature, pin and action correspondence without excluding a production input; preserve the successful named CLI-root build |
| Authentic MODULE, registry, archive, built-in and generated repository sources | Partial. Normal Run registry parity accepted at `47163df7b`; authentic complete R2 fixture closure remains unresolved. Stage 5 owns sources and registrations | Stages 4/5 | Portable authentic inputs and ordinary source/mapping/registration demand for the selected production closure; no synthetic package, user-CAS default-test dependency or hidden Bazel semantic delegation |
| rules_rust/provider/transition/toolchain evaluation | Partial configured analysis; WP-7-39/40 retain complete output groups, shared depsets, authoritative file providers and alias projection. WP-7-41 adds ordinary requested-artifact metadata with per-request/group membership, structural deduplication and tracked source identity. WP-7-42 through WP-7-46 add requested prerequisite scheduling and public CLI/daemon Build execution/publication for admitted FileWrite/Spawn forests; accepted generic declarations/providers/Args/runfiles still do not prove the full rules_rust closure. Canonical M7 status names accepted owners | Stages 4/5/6 | Ordinary configured results for the production root with source-derived toolchain/provider/transition dependencies; exact named semantics and structural invalidation |
| Named/automatic execution groups | Accepted bounded with R2 at main checkpoint `71d9ce4bf`; pinned-source runtime names, target-context parsing, property precedence, action ordering, root provenance/depset shape and subrule boundary passed joint gates | Stages 4/5/6 | Preserve the accepted boundary; authentic F3 remains unaccepted and closed at its 30-second ceiling |
| `attr.label` computed-default invocation | Accepted with the atomic stack at `71d9ce4bf`; omitted invocation, typed Label/None, explicit bypass, lexical context, A/B/A and configured Exec proofs pass | Stages 4/6 | Preserve accepted ownership; F3 supplies no further configured-source admission |
| Native configurable `alias.actual` | Accepted with the atomic stack at `71d9ce4bf`; retained expression, selected branch/conditions, target/exec A/B/A and query proofs pass | Stages 4/6 | Preserve accepted ownership; F3 supplies no further configured-source admission |
| Requested-root output conflict freedom | Accepted bounded with execution groups at `71d9ce4bf`; `ValidatedActionClosure`, pre-execution rejection, positive sharing and direct consumers passed, with eight Core nonpasses attributed to unchanged main | Stage 6/Core | Preserve root-set validation before build/aquery/Run success; cold/warm A+B conflicts produce no execution or materialization even if A and B separately succeed |
| FileWrite | Accepted bounded aquery/REAPI handoff, Stage 7 canonical FileWrite projection and M5/M6 evidence. [WP919](wp919-executable-file-write-aquery-2026-10-06.json) admits either executable bit; [WP920](wp920-external-file-write-aquery-2026-10-06.json) adds guarded external owners in validated dependency closures. Standalone external rule requests remain outside the source-only BuildCommandRootKey route. Its exact ActionKey projection remains queued | Stages 6/8/7 | Preserve accepted content/platform/protobuf/cache proof and named Slug-native token exception; land exact projection when separately selected |
| Spawn: compiler, linker, proc-macro/build-script and generated-source tool invocations | Configured common non-callback Spawn/FilesToRun expansion accepted (`bfe6f2690`, `21db5d7b8`), plus source-pinned root/dirname/crate-provider callback publication (WP-7-20/21/22) and native-library default link mappings (WP-7-25); bounded source/generated File/Directory Spawn execution and public requested Build accepted through WP-7-46; ordinary typed Spawn text aquery and mixed FileWrite/Spawn consumer accepted by [WP912](wp912-spawn-aquery-2026-10-06.json), with retained unspilled Args, exact artifact membership, fixed env and native config/path domains; authentic compiler/linker/proc-macro/build-script closure and other query families remain open; exact ActionKey and original-primary-input progress deferred | Stages 6/8/7 | Source-derived invocation/tool/env/input/output semantics for observed production actions, structural invalidation, classified aquery fields and same-owner REAPI projection |
| ArgsWrite / paramfiles | Configured vector Args/param-file ownership accepted (`a01a23fe7`); forced virtual argv/bytes and atomic input-tree composition added in WP-7-23; forced param-file CAS input bytes are exercised by WP-7-46 requested Build; broader family/aquery admission remains open; exact ActionKey deferred | Stages 6/8/7 | Exact encoding/content/order, declared input-tree placement and CAS bytes; typed comparator provenance for generated paths and classified ActionKey field |
| Symlink and runfiles-support action families | Typed symlink declarations, runfiles and four support actions/FilesToRun expansion are configured-only admission (`f346c209a`, `f46a009a0`, `21db5d7b8`); WP-7-47 adds a pure artifact-owned runfiles layout/constituent projection; WP-7-48 adds observed native Host-source/configured-output target resolution and deterministic source/repository mapping bytes; WP-7-49 adds selected native materialization-generation leases with held-result/runtime-shutdown lifetime proof; WP-7-50 admits ordinary requested Build support execution, durable backing and confined coupled tree/MANIFEST publication with real REAPI and CLI lifetime proof; WP-7-51 adds exact retained FilesToRun tool-input binding and logical remote runfiles expansion, including generated Directory artifacts; WP-7-52 admits regular-File artifact-target aliases with verified remote bytes and durable physical links; [WP913](wp913-symlink-aquery-2026-10-06.json) admits regular Artifact/absolute SymlinkAction query fields; [WP914](wp914-symlink-equivalence-2026-10-06.json) admits default ordinary Artifact sharing and exact duplicate producer planning over retained closure; [WP915](wp915-runfiles-aquery-2026-10-06.json) admits the four default support query families with class-specific fields and borrowed owners; actual six typed producer pairs, Solib query and full-root comparison remain open; exact ActionKeys deferred | Stages 6/8/7 | Inventory names each required family; admit its semantic aquery/REAPI projections and outputs/runfiles behavior with explicit ActionKey classification, or prove absence from this closure |
| Ordinary source/generated/tree input transfer and output materialization | Bounded FileWrite/source Spawn and verified regular-file output trees accepted (WP-7-31/34/35). WP-7-36/37 add owner-preserving prerequisites and verified generated File/Directory consumer execution within one native request. WP-7-38 adds selected-output publication after full-frontier validation on Linux GNU, with verified bytes, 0555 file/tree modes, per-artifact replacement and confined cleanup. WP-7-42/43 add the shared requested-artifact forest, complete source staging and native execution with zero-action completion. WP-7-44 accepts selected-artifact publication across producers/configurations on Linux GNU, including real multi-producer REAPI proof. WP-7-45 preserves typed build-error terminals and diagnostics without execution effects, including retained-runtime analysis/conflict recovery through DICE-attested transient roots. WP-7-46 activates one-shot and retained-daemon Execute Build with exact selected File/Tree publication, zero-action success, typed errors, complete remote policy and accepted-attempt cache evidence. WP-7-50 adds complete backing and typed runfiles-tree publication with persistent materialized sources. WP-7-52 adds regular-File artifact-target aliases; unresolved Symlink outputs remain open | Stages 6/7 | Exact Directory topology/content/digests, declared output ownership/type/mode/symlinks, generated-output consumer/reupload and missing/corrupt data behavior; validated-root-only publication |
| Per-family graph/comparator coverage | Bounded FileWrite plus ordinary typed Spawn text/literal/deps admission accepted ([WP912](wp912-spawn-aquery-2026-10-06.json)); ordinary Symlink and four default runfiles-support text query are also admitted ([WP913](wp913-symlink-aquery-2026-10-06.json), [WP915](wp915-runfiles-aquery-2026-10-06.json)); production action graph comparison open | Stages 6/8/10 | Matched focused per-family graphs and selected formats, preserved accepted exact ActionKeys, explicit per-family deferred/native key allowances, typed argv/env/paramfile path correspondence and discriminating negatives |
| Shared cache core used by bootstrap | Bounded FileWrite leaf accepted in WP-7-13; broader bootstrap transfer coverage open | Stages 7/11 | Extend the graph-independent protocol/CAS/AC boundary only as demanded by observed production artifacts; standalone release and direct disk interoperability are post-M8 |
| Production REAPI execution/cache proof | Bounded mixed FileWrite/generated File/Directory Spawn execution through public CLI and retained-daemon Build accepted in WP-7-46 on fresh Linux NativeLink backends: three reachable actions out of four declared, cold/warm/A→B→A cache counts, selected bytes/modes/replacement, zero direct-local actions and retained diagnostic behavior. Authentic production compiler/toolchain closure remains open; Bazel BuildBuddy developer proofs do not widen Slug admission | Stage 7 | Required production families execute with exact upload/AC/output evidence and zero direct-local build actions; selected backend/capabilities and lifecycle evidence only |
| Stage0/1/2 bootstrap fixed point | Open; developer-built binary alone is not self-hosting | Stage 10 | Stage1 binary actually launches stage2; independent miss execution and separate replay proof, graph equality and exact output manifests/digests except named build-info normalization |

Default binary runfiles support is admitted for ordinary Build through WP-7-50. The production
`//app/slug_cli_v2:slug` is a `rust_binary`; its hidden RunfilesTree can now use the existing
retained support families, artifact-owned layout, deterministic manifests, observed native
source authority, persistent materialized backing and confined tree/MANIFEST publication.
A tiny binary proves hidden generated backing, materialized source modes, A/warm-A/B/restored-A,
MANIFEST-only requests and actual post-CLI-process usability. This closes that family gap;
it does not prove the entire production root. Continue reconciling the finite production
closure's remaining demanded action surfaces against the inventory before claiming M7A or M8.
WP-7-51 supplies FilesToRun tool inputs using certified source files and verified generated
File/Directory results, with no tool-only publication. Saved WP714 declarations and pinned
rules_rust 0.73 wrapper/build-script invocations establish this demand. The same evidence
names bootstrap Symlink/ExecutableSymlink outputs and Cargo's _runfiles_map callback.
WP-7-52 admits the demanded regular-File artifact-target alias family: nested aliases retain
source/CAS/local provenance and executable checks, remote consumers get verified target
bytes, and requested physical links retain complete durable backing after CLI exit.
The two saved SolibSymlink actions remain a separate native family; Cargo's _runfiles_map
callback is admitted by WP-7-53 as an authenticated structural recipe with retained captures
and forced parameter bytes, including the required unexpanded Directory argument. Full
Cargo rule/toolchain execution remains open. Virtual Args paramfiles do not prove standalone
ArgsWrite demand.
WP755's [durable Solib observation](./solib-demand.json) identifies two declarations
for rules_rust's C++ allocator library, in target and exec configurations. Authenticated
cc_library linking uses dynamic_library_symlink with both flags false on a generated
library input. The [native inventory](./solib-construction-inventory.md) now selects
that constructor and structural action boundary. WP756 implements the False/False
generated-File leaf, distinct typed action, bin-relative declaration and structural
local/cross-owner sharing. Input repository path participates in native mangling;
Slug-native configuration/path projections omit Bazel ST history. Different configured
inputs at the same output fail closed. Native execution/REAPI/aquery/ActionKey remain
unsupported; neither the helper/binder nor sink proof establishes full cc_library.
Default nonsibling Bazel bin roots are shared. The observation
ran zero build actions and proves neither downstream use nor full C++ toolchain support.
WP757's [input-filtered observation](./solib-consumption.json) returns the same
two producer controls and no direct declared/scheduling consumers of their outputs.
This is not an execution/default-output/runfiles proof. Keep Solib execution gated.
The native C++ feature model/configuration prerequisite now has complete typed
feature/action/flag/environment/tool/pattern retention and native selection, with
provider publication preserving payload and wrapper alias identity (WP758).
The implementation and pinned-source regressions are in BuildAPI/Loading
cc_features modules; Analysis tests cover publication and heap/GC lifetime.
WP759 adds structural repository/package/rule/global feature composition, typed
Cpp/coverage projections, both exec feature-option mappings and default selected
execution-platform exec_os. The observed canonical-external analysis proof invokes
the unchanged creator, legacy generator and public configure_features with a real
rule ctx; capability/FDO envelopes are explicit boundary inputs, not production
toolchain/FDO construction. Canonical OS identity is exact; Host fallback remains
structural Rust Host. Root-origin local-override cross-package imports and legacy
apparent @platforms normalization remain separately deferred. The model still
rejects Windows/unknown execution OS, NUL paths and cyclic flag-group metadata.
WP760 adds native Variables and tool/action-enabled/requirements/argv/environment
queries. The unchanged compile-variable builder now runs after the creator,
legacy generator and public configure_features on a real ctx and source File;
provider publication and same-DICE changes/restoration preserve complete variables,
including unused payload and stale dotted-cache snapshots. This is the admitted
NOOP-mapper/no-execution-metadata slice, not full toolchain/FDO construction.
WP761 adds structural fission/strip/linkopts projections, including omitfp-derived
flags and exact selected exec resets/host-linkopt mapping. The unchanged link
builder and complete public cc_common export now feed the unchanged Rust
get_linker_and_args with real ctx/native feature configuration. Authored toolchain
envelopes remain explicit: this proves action-name/tool/argv/environment selection
and Rust-preferred filtering, not production toolchain construction or execution.
WP762 extends native HeaderInfo beyond the empty sentinel: owned artifacts,
module slots, normalized header lists and ordinary/exported dependency graphs
survive publication. The unchanged create_compilation_context now runs with real
Files and configured producer/consumer providers; hidden ordinary-edge changes
and visible exported-header changes participate in same-DICE restoration.
This supplies compile.bzl's mandatory context prerequisite, not a compile action,
native scanner or full toolchain rule proof. WP763 additionally publishes all
fields from the unchanged CcToolchainInfo constructor, with real Cpp/Variables/
features/CcInfo and source-authenticated PIC/runtime callable recipes. Configured
consumers pass that same provider to unchanged configure_features and Rust linker
queries. This replaces the authored envelope at the constructor boundary only;
inert FDO/tool inputs are explicit, not full toolchain production. Generic functions,
unmodeled captures and unauthenticated recipes remain rejected. Cpp occurrence
identity is explicitly Slug-native over full structural configuration.
WP764 passes that genuine published toolchain to unchanged get_copts and
dotd_files_enabled, with structural compile flags, exact host-to-exec mapping,
File.extension and typed empty per-file options. The default dotd-on policy and
feature suppression branches are covered; this is a configuration/input prerequisite,
not actual bootstrap compile argv, discovery or execution evidence. CppCompile
still requires distinct mandatory/spawn/cache-only inputs and discovery lifecycle.
WP765 adds original-context recovery with root/subrule lifetime checks and
toolchain-owned artifact naming through the unchanged public wrapper. Native
provider schema classification survives publication, including same-key/field
source changes. Naming admits observed non-Windows Host only. WP766 supplies
coherent derived roots through File/ctx, Args, actions, REAPI and confined physical
publication, with real configured root-aware helper callers and three fresh
NativeLink transport/publication/cache-restoration gates. Equal relative names in
target/exec configurations coexist without double prefixes. Output/configuration
tokens remain Slug-native; no getter-only prefix or empty derived root is admitted.
WP767 adds the demanded regular-source compile-output prefix/declaration pair,
with genuine configuration, normalized package containment, distinct naming label
and active owner, and main/external target/exec publication/restoration. Existing
toolchain naming remains independently protected; this is not full public compile()
traversal or compiler execution. Other context/tree/action natives remain open.
WP768 proves unchanged compilation-context initialization for main/external
target/exec owners and header-only public compile composition. Fixed nonsibling layout and
genuine File-kind queries add no new retained identity. Virtual headers, modules
and tree templates remain outside this proof.
WP769 adds regular C/C++ construction-only CppCompile registration with object/dotd,
separate input roles and traced end-of-analysis recipe publication. The unchanged
public caller uses an inert authored toolchain; this does not establish production
allocator compiler arguments or execution. WP770 adds pure typed command/environment
expansion from published recipes, including fresh argv versus original environment
cache scopes and explicit client-environment resolution. Complete command tests
cover inert public C/C++ PIC/main/external/target/exec recipes and same-DICE
restoration; they do not establish production allocator commands.
[WP921](wp921-public-compile-query-2026-10-06.json) connects the unchanged public caller
through actual main/external Target/selected Exec closures to the accepted typed query.
Fourteen recipes preserve optimized Exec PIC then NOPIC, fixed PATH/env, borrowed input
roles and roots, ordered full records and source/configuration A/B/A release. Its 71
Core checks use qualified69+1+1 union plus three Analysis controls; production and retained
storage are unchanged. Actual allocator flags, discovery/execution and whole memory
fit remain open. WP771 supplies
regular-file input sets and pure supplied-dotd reconciliation with explicit source
resolution and action-configuration inclusion policy. Synthetic dependency bytes
over real published recipes are not compiler-output evidence. Core discovery
ownership, verified dotd acquisition and cache replay still gate execution. WP772
adds pure preparation with source-defined include-path checks and original-cache
compiler options; this does not establish actual allocator flags or full production
toolchain construction. WP773 replaces authored CcToolchainInfo construction with
the unchanged rules_cc provider helper and authentic no-profile FDO subrule,
publishing into separate public compile consumers across main/external and
fastbuild/opt/default exec. It does not yet prove full cc_toolchain attribute/
allowlist/TemplateVariableInfo construction, runtime Solib, modules, profile
actions or allocator execution. WP774 supplies genuine ctx.file/ctx.files from
configured dependency DefaultInfo, consumed by that same helper/compile proof.
WP775 adds the genuine native package-group allowlist and preserves its hidden
membership through that helper's publication. WP776 supplies genuine native
TemplateVariableInfo from unchanged make-variable helper results, preserving
ordered values and occurrence aliases across configured dependencies. This does
not admit ctx.var expansion or the full toolchain envelope. The full rule's
implicit cc_build_info produces both stamped and redacted groups
regardless --stamp; status transforms and embedded helper dependencies cannot be
replaced by empty providers to claim construction. WP777 now supplies genuine
workspace-global status File descriptors through ctx.info_file/version_file,
with workspace/role identity and constant-metadata policy. WP777 itself does not
produce status bytes or register/execute build-info transforms.
WP778 accepts unchanged installed C++ build-info construction, which
registers volatile/stable typed transforms and a redacted Symlink,
publishes bare OutputGroupInfo in native group order, and feeds that real provider
into the existing authentic toolchain-helper/public-compile proof. Finite recipes
authenticate the complete pinned callback source and callsite; no callbacks run
in analysis. Full structural action identity includes workspace/status, template,
configuration/root and orthogonal output metadata. This removes a construction
prerequisite. [WP922](wp922-build-info-recipe-render-2026-10-06.json) now renders the
checked finite recipes from supplied borrowed status/template strings, preserving
ordered literal replacement, defaults and missing timestamp errors. Real installed
main/external Target/Exec recipes produce complete supplied ASCII headers.
[WP923](wp923-supplied-workspace-status-values-2026-10-06.json) supplies pure status-byte
parsing with exact Latin1/LF/first-space/last-value semantics and owned compact maps.
Parsed supplied content composes with both checked string recipes, without observing
status artifacts. [WP924](wp924-build-info-supplied-byte-render-2026-10-06.json) now
completes supplied status/template byte rendering with exact Latin1 decode/encode
and shared finite callback/literal order, including all256-byte no-match roundtrip.
[WP925](wp925-build-info-catalog-template-preparation-2026-10-06.json) prepares tracked
installed template content from the actual validated configured closure, sharing the
original immutable catalog owner and retaining full source frontier/certificate.
Actual main/external Target/Exec selection, full configuration restoration, native
current-source refresh/revocation and held-owner release pass.
[WP926](wp926-native-workspace-status-inputs-2026-10-06.json) captures genuine bounded
native default status inputs once per request and produces sorted UTF8 bytes inside
DICE. Both complete catalog-backed headers under different configurations share one
producer. Current capture/source association, same-sample retry/new-request revocation,
real DICE cancellation/abort recovery and held/final owner release pass. USER/kernel
identity and fixed English UTC are qualified native domains; SOURCE_DATE_EPOCH presence
and nondefault policies refuse. Twenty-three unique Core controls (18 preserved protected
plus5 current), default CLI and three FINAL reviews accept this content prerequisite.
[WP927](wp927-native-workspace-status-publication-2026-10-06.json) accepts private native
source-certified publication of both workspace-global status files. Genuine prepared
namespace/full bytes/configuration sharing, equal stable inode/mtime/ctime preservation,
equal volatile rewrite, actual source retry/no early output, typed partial failure,
preflight refusal/capture association and normal stage/held-owner release pass. Linux
GNU readable regular single-link/no-symlink destinations and 0444 atomic replacement
are qualified native policy. Thirty-nine distinct Core controls (33 preserved protected
plus6 current), default CLI and three FINAL reviews pass; this is not one fresh39 batch.
Unclaimed scratch on identity-read/keep/unlink IO failure, concurrent filesystem race
and partial revision-before-publication qualifications remain. Volatile cache/invalidation,
BuildInfo execution/input-image integration, module tools and full cc_toolchain remain
unproved; bounded buffers/Arc sharing do not establish whole-workload fit. The unrelated legacy bare frontier fixture requires an
uncached registry dependency and remains unproved offline; its guard was not changed.
[WP928](wp928-native-workspace-status-file-inputs-2026-10-06.json) accepts actual
physical status-input consumption within the same certified native publication
callback. Original metadata/root/producer bindings, both complete role headers under
covered configured frontiers, equal stable/new volatile receipts, historical/current
refusal, binding/phase-error grant exhaustion, nine physical mutation families,
source retry/abort/partial/read-error recovery and four-FD/Weak final release pass.
All44 current controls (39 protected+5 new), default CLI and three FINAL reviews
accept this private GNU/Linux domain. Capture-time IO/finite render-error propagation
is source-only; no executed-negative claim.8236 stack bytes and shared original
owners add no status-body/catalog/whole-string copy or global pool; original metadata
header retention and full-workload-fit limits remain explicit. Header output-file
publication was not admitted by WP928.
[WP929](wp929-native-build-info-header-publication-2026-10-06.json) now accepts the
private genuine-current native Info/Version header write at the original configured
output. Complete physical bytes/paths/Ordinary versus Constant metadata/0444,
configuration/source A/B/A, original one-use phase grants, real retry/abort/13mutation
faults including actual rename EACCES, held-cell six-FD retirement and final Weak
release pass in one current55-test batch, with default CLI and three FINAL reviews.
The result shares the actual consumed header, using existing original Arc/interner
owners and borrowed body/path scratch. Checked2MiB admission covers the pinned
ordered recipe's1,405,044-byte conservative bound; pure worst-marker output is
1,400,909bytes. Original precomputed/renderer overlap and9,260-byte conservative
stack scratch remain accounted, with no whole-workload memory-fit claim.
Status/revision may already change before header failure; conditional Drop unlink
can leave owned scratch under a readonly parent. Generic action chains/cache/REAPI,
other IO faults, external writers and Windows remain qualified or deferred.
Saved WP905 artifact edges give all four TranslateBuildInfo outputs zero consumers;
this closes a declared capability, not a demonstrated default CLI dependency.
Further header/C++ execution needs concrete requested-root demand.
Generic execution and raw projection/materialization paths for these descriptors/transforms
remain closed; bounded producer evidence does not establish whole-workload memory fit.
Native filegroup forwarding supplies the nested configured srcs/FilesToRun
prerequisite used by the four implicit tool dependencies, preserving real file,
runfiles and output-group providers and selected target/exec ownership. It does
not install module binaries or load the complete tools/cpp package. WP780 adds
native label_flag; WP781 installs the exact Bazel 9.2 tools/cpp BUILD and sibling
source entries through the built-in catalog, making both cpp package markers
available without claiming package Loading. Authentic Loading still requires
the genuine compatibility-proxy/rules/protobuf load closure and ordinary
repository-package Starlark deps; binary catalog admission also needs explicit
release-platform provenance. None may be replaced with an authored tool/provider
envelope.
Generic command/aquery/ActionKey, discovery-aware input selection/cache hits, execution-time tree/
path mapping and full cc_library remain deferred. Variables reject cyclic/foreign publication;
raw set/range publication and unmodeled builtin structures remain outside
the admitted boundary. See the current packet for exact/native/deferred classes.
The existing full Rustc argument proof's None C++ fields still do not establish
those remaining gates; the nonempty C++ linker-helper proof is independently bounded.
Run remains deferred and F3 remains closed.

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
