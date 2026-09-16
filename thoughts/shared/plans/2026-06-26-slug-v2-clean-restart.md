# Slug V2 Clean-Restart Implementation Plan

## Canonical Status

This plan owns milestone priority and acceptance state. The
[current packet](./slug-v2-subplans/current-packet.md) owns the one active
contract; stage plans own subsystem invariants and future acceptance gates.
The orchestration skill owns execution/recovery, and the
[authoring guide](./slug-v2-plan-authoring-guide.md) owns packet readiness.
Historical evidence is indexed in [plan-history.md](./plan-history.md) and
never selects work. This compaction changes no accepted compatibility surface.

## Live Status

| Milestone | Status | Accepted boundary / remaining gate |
|---|---|---|
| M0: archive and baseline | accepted | V1 archive refs and clean-root baseline in `9897e940`; keep archive identities |
| M1: one semantic spine | accepted | shared daemon DICE, immutable requests/observations/source certificates, atomic publication, overlapping isolation and warm reuse; historical acceptance inventory in plan-history |
| M2: analysis graph | accepted, Slug-native identity | recursive configured graph; exact configuration/output bytes remain M9 |
| M3: query | accepted | 16 default functions and admitted text formats, including `attr()` through `ed38f82a`; Sky Query/non-text breadth deferred |
| M4: cquery | accepted | shared configured graph, structural/null identity, admitted topology/formats, error and daemon lifecycle behavior |
| M5: aquery | accepted, bounded FileWrite | owner-complete configured graph and admitted literal/deps output; per-family ActionKey fields have independent exact/Slug-native/deferred classifications |
| M6: execution/caching | accepted, bounded FileWrite | sole resolved semantic input, canonical REAPI SHA-256, selected platform and daemon restoration; zero direct-local actions |
| M7A: bootstrap-critical breadth | partial; current milestone | authentic configured fixture, combined selected-request/conflict acceptance, execution groups and only the remaining bootstrap closure capabilities |
| M8: bootstrap | blocked on M7A | Bazel developer graph and BuildBuddy cache/RBE gates accepted; CI not admitted; Stage 10.3/10.4 self-hosting remains unproved |
| Cache library release | deferred until M8 | Stage 11 standalone Rust consumer, remote AC/CAS and Bazel disk-cache interoperability; bootstrap establishes the shared core |
| M7B: remaining breadth | deferred until M8; mixed-language repository follows cache library release | run/test/BEP, unrelated rulesets, extra formats and product extensions not demanded by bootstrap |
| M9: exact identity/inspection bytes | deferred until functional bootstrap | retain four-domain evidence `f00e99db`; exact configuration/output bytes and unadmitted exact ActionKey projections |

### Current packet

Packet: WP-7-10-m7a-external-tokio-feature-probe-r1
Status: design ACCEPT; one bounded Cargo unit-graph observation pending

Independent final atomic review ACCEPTS checkpoint `71d9ce4bf`. Main was
fast-forwarded from `4824a0861`, integrating selected-request identity,
configured-action conflict validation, named/automatic execution groups and
their reviewed prerequisites as one boundary. All required gates are complete;
the eight main Core failures remain baseline debt, and authentic F3 is the next
separate post-activation packet.

The accepted packet's sole 60-second harness preparation stopped with exit 124,
315 compiler artifacts, two messages, zero errors and no executable or
`build-finished`; receipt
`4400c7f81af35a53a6ad40ed322062117193608bc3a595ab93490fca17c6ec34`
records clean cleanup and zero F3 invocations. The narrow successor freezes
source checkpoint `12d96a4d4` and authorizes one identical compile-only
continuation against the warmed target under a 180-second ceiling. A stop or
invalid Cargo receipt replans without retry. Only preparation success permits
the still-unused single F3 invocation under the accepted 12/15-second limits
and exact persistent summary, supervisor, PID and observer gates. No source,
fixture or accepted-gate change/rerun is allowed.

That accepted continuation also stopped at its ceiling: exit 124 after 360
compiler artifacts, 52 build-script rows and 17 messages, with zero errors,
malformed rows, executables or `build-finished`. Receipt
`5de997cd2c5176ab1fcfe0678f7f33b543b876c22948c975de9b37b27e9a9597`
records clean cleanup and zero F3 invocations. The final recovery permits one
identical compile-only continuation against the warmed target under a
360-second ceiling. A preparation defect ends this evidence path without retry;
only strict success exposes the still-unused single F3 proof.

The final preparation succeeded within the authorized 360-second ceiling with
one harness and a valid receipt. The sole F3 proof then exited 2 after 12.146 seconds: selector and
cleanup gates passed, but the run hit its 12-second deadline with raw status 9,
zero executed/passed tests and no publication/sentinel while the observer
remained enabled at `RootCompute`/`Entry`. Proof receipt
`918ffa9c556b6a8cd242dc847303d47dedce2feff235a9db7e353239b3bd54d9`
is trustworthy timeout evidence but semantically nonselecting. Independent
result review returns REPLAN; F3 is unaccepted and may not rerun.

The selected docs-first successor audits whether finite test-only partitions
can preserve the exact authentic fixture, public evaluation/return paths,
terminal projection, publication and sentinel assertions within the unchanged
12/15-second limits. It permits inspection and documentation only. No deadline
increase, F3/diagnostic replay, production/fixture change, warm-cache
substitution, smaller-target acceptance, payload acquisition or semantic
inference from the last observer sample is allowed. If the public result is
indivisible, terminate the F3 evidence path as resource-blocked and select only
independent M7A production-inventory reconciliation.

Source inspection finds the exact public result indivisible. The public call
creates a private `WorkspaceRuntime` and synchronously owns its DICE/native
demand/repository retry loop through terminal selection and final acceptance;
no continuation or checkpoint escapes. Projection and publication consume the
returned accepted terminal/events afterward, so they cannot partition the
timed evaluation. Any earlier split needs private stages or retained state, and
separate calls repeat the whole evaluation; warm state, smaller targets and
synthetic stages violate the frozen contract. Record F3 resource-blocked,
terminate this evidence path and select only independent M7A
production-inventory reconciliation. No code or test changed and no command ran.

Independent result review ACCEPTS that boundary audit and records F3
resource-blocked with no rerun or semantic implementation. The selected M7A
successor freezes clean main `39e3a89ac` and reconciles the live
`//app/slug_cli_v2:slug` production closure against the accepted 33-package
inventory. It permits one locked/offline Cargo metadata snapshot and one Bazel
9.2 streamed-JSON dependency query in a network namespace with fetch/download
disabled, each under 60-second TERM/three-second KILL limits and output caps.
Cargo is rooted at the CLI manifest and filtered to Linux normal/build edges;
Bazel is pinned by absolute executable/SHA and remains an unconfigured declared
superset. Complete receipts record exact manifest/lock/toolchain hashes and
every local package, feature, proc-macro, build-script and generated-source
delta plus their explicit path mapping. It runs no build, test, action, oracle,
repin, payload acquisition or F3 replay and admits no behavior from reachability
alone.

The user's clarification supersedes the plan's treatment of 12/15 seconds as
an exact acceptance gate: 12 seconds is a general guideline, and tests should
use the smallest valid semantic scope. The accepted boundary audit already
proves the one authentic ignored F3 test is indivisible without private-stage,
warm-state or smaller-target substitution, so it remains the smallest valid
scope and is no longer resource-blocked solely by the 12-second timeout.

The selected recovery changes only test supervision. It adds a validated
1--30-second `prove --deadline-seconds` option, retains 12 as the default, and
threads the chosen value through the portable driver with nested `N`, `N+3`
and `N+5` bounds, including portable CPU=`N+3`; the final external bound is
`N+8`. Persistent driver/Python/final receipts must agree on every value.
Implementation is limited to the fixture entry point, portable driver and
focused fixture tests under 100/70 tooling and 100/80 test gross/physical caps.
Boundary/forwarding tests, the existing three fixture tests, Python/Bash checks
and supervisor self-check precede independent implementation/necessity review.
The frozen harness may then run once at a
30-second internal ceiling/38-second external ceiling. It remains ignored and
opt-in, outside routine suites. The exact selector, fixture, public evaluation,
observer and persistent success gates remain unchanged. No Cargo compile,
production Rust change, diagnostic replay, automatic deadline extension or
semantic inference is allowed.

The bounded-supervision implementation now passes syntax, compilation, diff
and all ten focused checks; the focused suite completes in 0.39 seconds.
Independent implementation and necessity review returned `EXECUTE`. One
`N=30` replay is selected under the frozen 38-second external ceiling, with no
retry or extension.

The sole proof reached its clean 30-second wall deadline at
`RootCompute`/`Entry` (6/1), before test completion, publication or sentinel.
Final receipt SHA-256 is
`43038196236b120fdf6c254f39ac6f369c0ec8497f227c3c0665c42171f5f888`.
Independent result review returned `REPLAN`: F3 is unaccepted and
resource-blocked at its indivisible boundary under the user's maximum. Close
this path without retry or extension and resume only partial M7A accounting
from frozen Cargo and authenticated artifacts, with Bazel reachability unknown.

That partial accounting now identifies exactly two local Cargo additions to
the accepted 33-package inventory: `slug_configuration_v2` and
`slug_starlark_v2`. The former is a normal dependency in the selected Cargo
closure but has no BUILD file; the latter has a static BUILD target. This
establishes a concrete Bazel ownership gap, not live Bazel reachability or
behavioral admission. The first independent result review verified the six
normal incoming configuration edges, source/path mapping and frozen receipts.
A subsequent static lock-key comparison found `slug_starlark_v2` and eight
selected external package versions absent from `Cargo.Bazel.lock`.
Independent correction rereview `ACCEPT` confirmed the 34/35 local and 302/310
external lock-key intersections. M7A remains open.

An offline graph-sync draft was rejected at design review before any BUILD or
Bazel lock change. Its intended repin invokes full-workspace Cargo resolution,
but the host cache then lacked archive/source pairs for 54 of the 448 registry
packages in the frozen `Cargo.lock`; 16 archives exist in Bazel's downloader
cache and 38 needed acquisition. One read-only, full-workspace locked/offline
metadata preflight failed in 0.15 seconds on missing `anstyle-wincon 3.0.11`
(stderr SHA-256
`529cdff554a4b7edbf576fef4ff48ef9d4ec8e8f68e22e61c57dae9826c30dde`).
The selected docs-first acquisition packet freezes the exact gap inventory
SHA-256 `5c8294dcbebbefea8602f891c28ee2545ff701d572084ae236049fdd0eba3189`.
After independent design review, it permits one bounded `cargo fetch --locked`
for all platforms and one short locked/offline metadata verification. It runs
no BUILD change, Bazel repin/query/build or test. Graph synchronization and
fresh Bazel reachability require a separate reviewed successor; M7A remains
open.

Independent design review accepted the acquisition packet. The one locked
fetch completed in 3.117 seconds with clean cleanup, unchanged tree/lock and
all 448 locked registry archives checksum-verified; receipt SHA-256
`861309ac82fcf02d8d8385b25622757ea0f4c04ffd4e568122532db0031f31b6`.
The one offline full-workspace metadata command produced all 448 registry
keys and complete sources in under 0.224 seconds. Its receipt wrapper failed
afterward on a local package's null `source`, so the saved command output was
verified read-only and the wrapper defect recorded in recovered receipt SHA-256
`f5163a1827114cdc71acc9c325369db93f23a3cbc807cf821203e5486fbb5bea`.
Independent final review `ACCEPT` confirmed the recovered metadata evidence
and cleanup without a rerun. This closes only the locked-input prerequisite;
no Bazel command or test followed.

The graph-sync packet added the missing configuration BUILD owner and six
normal consumer edges, then attempted one offline Bazel crate-universe repin
against the verified host Cargo cache. It scoped generated-lock coverage
and drift without a target query, build or test. Fresh Bazel reachability and
M7A behavior remain separate gates.
Independent design review `ACCEPT` confirmed this narrow boundary and the
cached generator; preserve exact pre-run Cargo-lock bytes for recovery if
repin violates its allowlist.
The static BUILD owner and six normal edges now match frozen Cargo analysis.
The sole initial repin exited 37 in 1.098 seconds before lock generation at
Bazel 9.2.0's system-network profiler null dereference. Receipt SHA-256
`86995d8a436d74b7db448d5fdbb3aa652a30c7f3ab50477036a3f6dca37b1051`
records clean cleanup, unchanged Cargo/other locks and identical pre/post
tracked status. Pinned Bazel help exposes a flag to disable only that
collector. The reviewed recovery kept offline/download controls and permitted
one otherwise identical bounded repin with
`--noexperimental_collect_system_network_usage`; no build, query or test is
selected.
Independent design review `ACCEPT` confirmed the flag disables the crashing
collector without changing repository or Cargo resolution, and verified the
unchanged lock/status receipts. One modified attempt was selected.
That single recovery command exited 0 in 6.722 seconds but inserted two blank
lines in root `Cargo.lock`'s unused-patch section, violating its exact byte
hash gate. Receipt SHA-256 is
`2322f3a8481e3e7393ca0d81f0572153f979a0a5cf2e8d4c44da73aff4806cfb`.
The original Cargo-lock bytes were restored; the changed copy parses to the
same TOML. The generated Bazel-lock candidate reaches 35/35 selected local
and 310/310 selected external keys, adds exactly nine missing crate keys and
matches all 448 registry checksums, with no version removal or existing
external drift. Candidate analysis SHA-256 is
`19b365fff8230f71021edf660901769d8a940744879dc2f5d6ac5486b1325997`.
Independent result review returned `REPLAN` under the explicit hash gate.
The selected successor permits one fresh-output-base, no-repin, offline Bazel
lock-coherence validation against restored authority; it still runs no target
query, build or test and admits no M7A behavior.
Independent design review `ACCEPT` confirmed that the fresh base and
`--lockfile_mode=off` combination forces extension evaluation, while
rules_rust's no-repin path checks its generated lock. One 30-second-capped
validation is selected.
The sole no-repin validation exited 0 in 2.171 seconds with clean cleanup,
fresh rules_rust extension evaluation and identical pre/post hashes for
Cargo.lock, Cargo.Bazel.lock, MODULE.bazel.lock and all seven BUILD files.
Receipt SHA-256 is
`290a3c2fce004a895b4470bd985d3618b047eb890457404daa37b182ebcbe278`.
The candidate's parsed Cargo-lock equality and all 448 registry checksums
remain separate gates for independent final review. No build, test or Bazel
target query ran; M7A remains open.
Independent final review `ACCEPT` verified the generated lock has all 497
Cargo-lock package name/version keys, no extras, all 448 registry checksums,
and the seven BUILD edits match normal Cargo edges. This closes only static
BUILD/lock synchronization; live target graph, compilation and M7A gates
remain open.
The selected successor obtains one fresh, bounded, download-disabled Bazel
declared dependency view of `//app/slug_cli_v2:slug`. It compares selected
local package paths and records gaps without treating an unconfigured query
as configured/action, compilation or M7A evidence. No build or test is
selected.
Independent design review `ACCEPT` confirmed the single fresh-base query and
label parsing boundary; the result must distinguish root `//` labels from
external `@` labels.
The sole pinned root dependency query exited 0 in 2.952 seconds with clean
cleanup, no repository download and unchanged tracked hashes/status. Command
receipt SHA-256 is
`1935a3d026a6a31566d6250ac2fc49747b1f08de04d36819984e9dd34ae89b8a`.
Its 18,846 valid distinct labels include all 35 selected local package paths,
the configuration and Starlark targets, and repositories for all eight
formerly absent selected external lock keys. Parsed analysis receipt SHA-256
is `f734391152ac608a0787b7443575b6fe3253294b8211494ba161d6f1545f0b13`.
Independent final review `ACCEPT` confirmed the saved labels, path mapping
and clean receipt. This is only unconfigured declared reachability; configured
target/action, generated-input, buildability and M7A gates remain open.
The selected successor permits one fresh-output-base, download-disabled
`bazel cquery deps(//app/slug_cli_v2:slug)` under a 30-second ceiling. Its
label output preserves each displayed seven-character configuration token
to identify root analysis coverage or a concrete failure; full configuration
identity and target/exec classification remain open. No build or test runs, and a
successful cquery alone cannot admit actions, generated inputs or M7A.
Independent design review `ACCEPT` confirmed that the saved output will be
interpreted only as displayed label/token rows and that the runner has bounded
process-group and pipe cleanup.
The sole cquery instead exited 1 in 2.862 seconds at Rust toolchain resolution
before emitting any configured row. Its receipt SHA-256 is
`80e255dbdffa001fa48f05d78d3b10d1ba9ed3c489a4c184e95b6238b06cd4d9`;
all frozen tracked hashes and Git status stayed unchanged. The invocation
omitted the nightly channel setting required by the registered Rust toolchain,
so configured-root reachability remains open.
The selected successor freezes that failed receipt and permits one otherwise
identical fresh-output-base cquery with the existing developer-gate
`--@rules_rust//rust/toolchain/channel=nightly` setting. Its 30-second
ceiling, offline/download-disabled boundary and no-test/no-build scope remain.
Independent design review `ACCEPT` confirmed that this changes only the
nightly invocation flag and preserves the one-attempt limits.
The one corrected cquery exited 1 in 2.933 seconds with zero configured rows:
nightly toolchain selection succeeded, then the Linux x86_64 toolchain tools
repository needed an uncached `rustc` archive and downloads were disabled.
Receipt SHA-256 is
`9f305306861c887bffc9ca6dbde4c1930cf8733cfa91cbe97b41c3b55364ab5c`;
all tracked hashes/status and cleanup stayed clean. Independent result review
confirmed this typed cache-miss boundary, not a BUILD/lock defect. The Rust
release manifest for 2025-09-14 supplies exact archive SHA-256 values; the
selected successor pins the required Linux x86_64 toolchain components and
stages only those verified archives before any further offline cquery.
Independent design review `ACCEPT` confirmed the six official hashes, the
2025-09-14 rustfmt correction and the local distdir materialization path.
The six pin entries in `MODULE.bazel` match the sidecar-verified manifest.
Offline `bazel mod deps` exited 0 in 2.068 seconds without changing
`MODULE.bazel.lock` or other frozen inputs. All six official archives matched
their SHA-256 values after a 1.627-second, 171,520,576-byte acquisition;
receipt SHA-256 is
`c84576af16f7c46bac05eb878d052b2196278473e32a280db9e2219b072162e2`.
The corrected download-disabled Bazel repository fetch exited 0 in 11.147
seconds from a fresh output base, materialized the dated Linux x86_64
toolchain tools repo and left tracked hashes/status unchanged; receipt SHA-256
is `1a47242a21971bbd5b63ae5d21f49f9a7a079d916643e8b02e187ef4ab7e8b10`.
Its preceding launcher-only option-placement failure is preserved at receipt
`65e69361f3592520c062f6e3edb362f852a86541b49f13ebf5eb65d6bcd20547`.
This is verified input preparation only; configured-root analysis remains open.
Independent final review `ACCEPT` confirmed the six archive digests,
MODULE-only change and clean materialization receipt; it does not infer
whether Bazel read each archive from distdir or its checksum-keyed cache.
The selected successor permits one fresh-output-base, 30-second-capped,
download-disabled `cquery deps(//app/slug_cli_v2:slug)` with the nightly
channel and verified six-file distdir. It records configured label/token
rows or an exact analysis failure, with no build/test or M7A admission.
Independent design review `ACCEPT` confirmed the exact six-file preflight,
offline one-command limit and narrow label-token evidence claim.
The sole offline cquery exited 0 in 5.337 seconds with 17,685 valid distinct
label/token rows, including all 35 selected local package paths with non-null
rows and the named CLI/configuration/Starlark targets. Receipt SHA-256 is
`459c41aad42c9259adc239b74271c0e7b380bb6970135e4e266ee91e13b322bc`;
parsed analysis SHA-256 is
`266cf64932c2ef555ca5d36bb8d87ad95a8b0fe6a641a4c41973527f0c711dde`.
All frozen tracked hashes/status stayed unchanged. Displayed short tokens
cannot establish full configuration identity, action reachability,
generated inputs, compilation or M7A readiness.
Independent final review `ACCEPT` re-parsed all 17,685 saved rows and
matched all 35 non-null root paths directly to frozen Cargo manifests.
Configured label/path reachability is accepted; the first readiness row
remains open.

The selected successor compares the frozen 35-package Cargo feature snapshot
with checked-in local Rust and build-script rules. It scopes only missing
`default` flags and two build-script feature lists in four BUILD files, using
pinned rules_rust source to verify flag and environment propagation. Static
source/attribute comparison is the packet's regression check; no new Bazel
query, build or test is selected. External feature equivalence, generated
inputs, configured actions, compilation and M7A remain open.
Independent design review `ACCEPT` confirmed the exact local feature mapping,
five build-script owners and pinned rules_rust propagation; the bounded edit
may proceed with static-only validation.
The four BUILD edits now match the frozen Cargo feature sets for all 35 local
packages, 36 production Rust rules and five build scripts. The static parse,
three pinned rules_rust hashes, diff check and plan status passed without a
Cargo/Bazel command, build or test. Independent final review `ACCEPT` repeated
the exact 41-target check and frozen-input/allowlist audit. This closes only
local feature-attribute parity; configured actions, generated inputs,
compilation and M7A remain open.
The selected successor isolates the only remaining external feature-list
ambiguity in a static Cargo/Bazel lock comparison: Tokio's `windows-sys`
appears in frozen Cargo metadata but only the Bazel lock's Windows select.
One offline, 10-second-capped Cargo unit-graph command may inspect the actual
Linux CLI Tokio feature set without compiling. It admits no external parity,
configured action, build or M7A claim by itself.
Independent design review `ACCEPT` followed a correction to select only
`--bin slug`, bound each output stream and process group, and classify only
Tokio's normal Linux target-library unit.

The M7A Cargo inventory receipt remains frozen. Its Bazel recovery later exited
at the interface gate before Bazel invocation; receipt
`bc4c11d4a939afa8b143f1bbc484d2d70a212a993111f79946f83f703209b252`
records no query output and exact cleanup. That partial inventory work is
preserved and deferred while the clarified F3 boundary is resolved.

The Cargo snapshot completed with a 35-local/310-external Linux normal/build
closure; receipt
`b3886868867d6b2c6f8d1d49575fbc0f4322ab83216e1da456ae7e2f1e2c5e2`
is frozen. The sole Bazel query emitted no row and exited 37 because its fresh
network namespace left loopback down, triggering a Bazel 9.2 metrics-collector
null dereference; receipt
`0c4df4f51470fb1b98ae60f606c2cac1bf354480646eb16050e05fcec2b6660f`
records exact cleanup and no reachability evidence. The Bazel-only recovery may
invoke the identical bounded query once after asserting the namespace contains
only `lo` and bringing that loopback device up. External interfaces remain
zero; its receipt must record pre/post interface names `["lo"]`, loopback UP
and `external_interfaces=0`. No Cargo rerun or other scope change is allowed; a
second failure ends the inventory evidence path.

#### Historical gate progression

The reconciled selected-request/configured-conflict and execution-group stack
remains unaccepted at local commit `1b09dbfa1`. Its frozen Core binary passed
313 of 330 active selectors in the approved host environment, produced eight
non-timeout assertion results and hit the exact 12-second deadline in nine
over-broad public build/cquery selectors. The complete receipt and binary are
hashed in the current packet. A timeout cannot be waived or retried.

The first proof-only partition is preserved unaccepted at `678ff2e7c`: 25 of
34 exact selectors pass and nine still hit the exact deadline, with zero
assertion failures. Freeze those 25 bodies. The independently reviewed second
partition replaces only the nine timed bodies with 53 fresh-fixture selectors,
each executing at most three real public commands. Production and expectations
remain frozen, and neither earlier timed nor passing names may run again. The
final binary `af7fa9c80829856b544d03cfb668df61d83592208044f8342f70ee3fd643396b`
passed all 53 once under the 12/15-second limits. Atomic receipt
`35c469ecd891903220ebde8bb7a13d16edf691155d6f9acfdb22c7e4ae6d3bef`
records zero failures and zero timeouts; independent final review ACCEPT
confirmed preserved expressions, result classes, assertions, edit order,
frozen bodies and caps. Resume the eight unchanged-main assertion comparisons
and remaining direct consumer gates without rerunning either completed
partition. Only complete atomic acceptance may merge to `main` and push `main`
to the authorized ZeromatterOSS remote.

Final predecessor review returned REPLAN solely because the eight candidate
Core exit-101 rows still lack unchanged-main attribution. Freeze feature
checkpoint `055b6fe18`, its 53/53 Core proof, passing direct REAPI/server
receipts and compile linkage. Six 60-second main preparations ran no tests and
produced no binary. The narrow successor allows one same-command 180-second
compile-only preparation at clean main `4824a0861`; on success it may preflight
and run only the eight frozen comparator selectors once under unchanged
12/15-second test limits. Any preparation stop, missing selector, main pass,
timeout or different failure class replans. No source change, completed proof
rerun, full sweep, CLI conflict, F3, merge or push is allowed.

The one authorized 180-second preparation then exited 124 while compiling the
final `slug_core_v2` crate. Receipt
`232d6ce07b1cfaefaf8ddfde359d3c728751e26281c780534fd3f930ec5684c7`
contains 366 compiler artifacts, 51 build-script rows, 17 messages, zero errors,
zero executables, no `build-finished` and zero tests. The final r2 recovery
allows one identical continuation with the warmed target under a 360-second
compile-only ceiling. Success still requires complete JSON, one successful
build-finished row and one hashed Core executable before only the eight frozen
comparators may run once at 12/15 seconds. Another preparation stop ends this
path as an external resource blocker; no further retry, source change, merge or
push is allowed.

The final continuation completed in 81 seconds. Compile receipt
`97ac6aa2b4bacb80c30107631b62cd0c8610eeebcd4a6b0e1c94a86c56378978`
has zero errors, one successful build-finished row and one main Core binary,
SHA-256 `31b941d3ec87a59a6cae2d7c399bd2ee6fb0182cd5284c54c35d957580fb149e`.
All eight exact main comparators then returned exit 101 without timeout or
forced kill in receipt
`7f87d741e0ad8edaa08b9d98f13b4488f478c1a8afa1704264e5651a73dca68c`.
Comparison receipt
`9819e9487d30d089ffa6b59900a26c1ea5f10990c698b6047242162e86ca2eb6`
matches every selector, timing class, panic/assertion family and material
diagnostic to the frozen candidate row. The eight nonpasses are baseline facts;
the complete atomic stack now awaits independent final review.

Independent final atomic review ACCEPTS checkpoint `71d9ce4bf`. Main was
fast-forwarded from `4824a0861` to that checkpoint, integrating selected-request
identity, configured-action conflict validation, named/automatic execution
groups and their reviewed prerequisites as one boundary. All focused, Core,
direct-consumer, compile and baseline-attribution gates are complete. The eight
main Core failures remain recorded baseline debt. Authentic F3 is the next
separate post-activation packet and was not run or accepted here.

Normal run registry propagation is accepted at `47163df7b`, typed registration
error identity at `a06f3ddfc`, bounded observer at `64c7d2475`, and source
observation diagnostics at `ac6140f41`. Their old implementation checkpoints
are historical, not pending work. The partition change preserves the sole full
accepted epoch and exact values; direct-epoch DICE callers retain their existing
lookup path.

The reconciled selected-request/output-conflict R2 candidate remains unaccepted
at local commit `f3c90ea46` on
`integration/selected-request-output-conflict-r2`, based on `2b3fedf76`. The
older source remains at `27e9e9c0c` on `review/output-conflict-r2`, based on
`97dffd5d4`. Preservation is not integration. Do not activate either candidate
partially or integrate `review-evidence/`.

### Ready and blocked queue

| Order | Result | State / dependency |
|---|---|---|
| 1 | Configured production graph coverage | configured 35/35 local path and local feature-attribute evidence accepted; separately review external features, configured actions and generated inputs |
| 2 | Bazel root compilation and action coverage | select focused build/action evidence only after configured graph accounting; current packet cannot admit M7A behavior |
| 3 | Remaining M7A action/input-tree/REAPI capabilities and shared cache core | select demanded rows in [bootstrap readiness](./slug-v2-subplans/bootstrap-readiness.md); Stage 11 owns the library boundary |
| 4 | Stage 10.3 graph comparison, then 10.4 fixed point | blocked on finite M7A closure; use reviewed typed comparison contract |
| 5 | Standalone remote/disk cache library | blocked on M8; [Stage 11](./slug-v2-subplans/11-bazel-compatible-cache-library.md) owns release gates |
| 6 | M7B mixed-language/command breadth, then M9 exact identity/inspection | separate functional ruleset and exact projection admission |

The user treats 12 seconds as a guideline, not an exact test rule. Keep tests
as small as the semantic boundary allows; anything over a few seconds should
run infrequently, and a test over roughly 30 seconds needs serious necessity
review. The one opt-in 30-second F3 proof was independently reviewed, ran once
and is closed without success. Compile/preparation remains separate and must
be split or checked in smaller units when necessary. An unattributed timeout
is not a semantic failure.
Cumulative path-epoch fanout remains a separate measured concern: the recorded
retry 116 recomputed 109 path keys and checked 163 module/192 registry keys.
It is not evidence of a semantic loop or authority for checkout-wide replay.

## Non-Negotiables

- Bazel 9 only. No pre-Bazel-9 behavior, no WORKSPACE support, and no legacy
  toolchain-resolution compatibility.
- Bazel source and Bazel tests are the compliance oracle. A parity claim needs
  a local Bazel source citation or an oracle fixture result.
- DICE owns semantic build state. Do not hide semantic discovery inside
  synchronous Starlark-visible APIs.
- REAPI is the execution boundary. BuildBuddy is the primary scaled remote
  development/CI lane; sibling `../actiond` is the preferred hermetic local
  conformance backend; NativeLink remains a useful regression backend. All sit
  behind the same REAPI boundary.
- Bazel invocations may use ordinary RC discovery and consume the user's
  `~/.bazelrc` for BuildBuddy authentication. Agents and inspection tools must
  never read or copy its contents, and credentials or derived secret material
  must never enter this checkout, logs intended for commit, or Git history.
- Slug-local sandbox implementation is deferred until after analysis, admitted
  `aquery` semantics, remote execution, and cache correctness. Backend isolation supplied
  by BuildBuddy or actiond does not count as a Slug sandbox implementation.
- Progress is demonstrated by a vertical Bazel-shaped build, not by independent
  identity, parser, DICE-shaped, action, or REAPI data models. A wrapper trait
  or stable-serialization helper is scaffolding until the owner fixture drives
  it through the real runtime boundary.
- V2 output layout targets Bazel-shaped paths. Any deliberate Slug-specific
  divergence must be explicitly documented as an extension, not assumed.
- V1 plans and code are evidence and extraction sources, not the V2 source of
  truth.
- New packets and replans follow
  [slug-v2-plan-authoring-guide.md](./slug-v2-plan-authoring-guide.md): name
  learned facts, decisions and non-decisions, exact/Slug-native/deferred
  classification, natural producer ownership, request/revision behavior,
  memory lifetime, upstream tests, fallback deletion, scope, and stops.

## Current Milestone Overlay

Stages are ownership boundaries, not execution order. Preserve
**M7A → M8 → cache library release → M7B → M9**. Linux self-hosting is the first
product milestone. Establish the graph-independent cache core within demanded
M7A execution work; standalone packaging, direct Bazel disk-cache compatibility,
C/Python bindings and mixed-language breadth are post-bootstrap work.
Exact Bazel ActionKeys do not gate functional action admission or self-hosting.
Preserve accepted exact fields, complete semantic identity/invalidation and
exact REAPI/CAS digests; classify each unadmitted inspection field explicitly.
The finite bootstrap readiness matrix owns the
capability-to-target/evidence mapping. Stage 10 comparison uses ordinary graph
owners; no bootstrap-only path, precomputed runtime graph or hidden Cargo/Bazel
executor is admitted.

## Stage Map

| Stage | Owner Plan | Parallelism | Checkpoint |
|-------|------------|-------------|------------|
| 0 | [00-v1-archive-and-clean-root.md](./slug-v2-subplans/00-v1-archive-and-clean-root.md) | Serial | V1 is tagged/branched, V2 root docs and metadata are active, archive policy is clear. |
| 1 | [01-compliance-oracle-harness.md](./slug-v2-subplans/01-compliance-oracle-harness.md) | Parallel | A fixture runner compares Java Bazel and Slug V2 for exit status, outputs, events, and selected diagnostics. |
| 2 | [02-rust-skeleton-and-runtime-substrate.md](./slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md) | Parallel | Minimal Rust CLI/server skeleton uses actual Buck2 runtime crates without exposing Buck semantics. |
| 3 | [03-bazel-identity-and-layout.md](./slug-v2-subplans/03-bazel-identity-and-layout.md) | Parallel after Stage 2 starts | Labels, repositories, packages, target ids, execroot, and output paths are Bazel-shaped. |
| 4 | [04-starlark-loading-and-build-packages.md](./slug-v2-subplans/04-starlark-loading-and-build-packages.md) | Parallel after Stage 3 basics | `BUILD.bazel` and `.bzl` loading work for small packages with Bazel globals. |
| 5 | [05-bzlmod-and-repository-graph.md](./slug-v2-subplans/05-bzlmod-and-repository-graph.md) | Parallel after Stage 3 basics | Starlark-evaluated `MODULE.bazel`, registry, repo mapping, extensions, repo specs, and lockfile policy are DICE-owned. |
| 6 | [06-analysis-toolchains-and-actions.md](./slug-v2-subplans/06-analysis-toolchains-and-actions.md) | Parallel after Stages 4/5 | Configured-target analysis, toolchains, providers, depsets, and action declarations pass focused oracle fixtures. |
| 7 | [07-reapi-native-execution.md](./slug-v2-subplans/07-reapi-native-execution.md) | Parallel with synthetic actions, then after Stage 6 | Shell and ruleset actions execute through REAPI with upload, AC, materialization, and zero direct-local proof. |
| 8 | [08-ruleset-and-command-conformance.md](./slug-v2-subplans/08-ruleset-and-command-conformance.md) | Query after loading/analysis; execution commands after admitted aquery semantics | `query`, `cquery`, and classified `aquery` comparisons pass before the corresponding ruleset, run, test, and BEP breadth. |
| 9 | [09-v1-extraction-ledger.md](./slug-v2-subplans/09-v1-extraction-ledger.md) | Continuous | Every V1 or Buck2-derived extraction has an owner, oracle proof, and cleanup decision. |
| 10 | [10-bazel-build-and-bootstrap.md](./slug-v2-subplans/10-bazel-build-and-bootstrap.md) | Bazel developer graph may start now; self-hosting follows classified aquery comparison and execution | Bazel 9 builds/tests Slug through BuildBuddy, then Slug reaches a stage1/stage2 self-build fixed point. |
| 11 | [11-bazel-compatible-cache-library.md](./slug-v2-subplans/11-bazel-compatible-cache-library.md) | Core boundary with M7A execution; standalone release after M8 | Slug consumes a graph-independent cache core; external Rust applications share remote and Bazel disk caches. |

## Supporting contracts

- [DICE ownership](../../../docs/developers/dice.md) owns runtime invariants.
- [Stage 9](./slug-v2-subplans/09-v1-extraction-ledger.md) owns retained reuse
  decisions; V1 is archival reference only.
- [Adoption roadmap](./slug-v2-subplans/zabel-adoption-roadmap.md) supplies
  bounded future work, not an alternate scheduler or Bazel compatibility oracle.
- [Plan history](./plan-history.md) preserves original acceptance inventories,
  source audits, and rejected experiments at immutable Git revisions.

Future branding consideration: Rubin (Red Rubin basil) remains a product
choice outside implementation milestones.

The legacy activation audit's corrected 166/217/85 production, proof and
scratch preparation passed exact Core proof and independent preexecution review.
Exactly one frozen same-selector replay is authorized; no retry is allowed.

That sole replay is accepted. At a coherent 220/220/219 handoff cutoff it
observed 2,488 legacy-delivered evaluated `PathObservationKey` callbacks, all
with an immediate exact shard dependency; reused and no-direct cells were zero.
This establishes neither identity nor invalidation causality, necessity, cost or
avoidability. Stop this aggregate diagnostic chain. Any continuation requires a
separately reviewed typed invalidation-provenance design.
The proposed successor uses DICE's existing typed invalidation-path API inside
the path key and retains only a fixed-size source category. It is docs-first and
cannot implement or replay until independent design acceptance.
