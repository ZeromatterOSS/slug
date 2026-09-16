# Stage 10: Bazel Build and Bootstrap

## Goal

Make Bazel 9 a fast, supported way to build and test Slug through BuildBuddy,
then use the resulting binary to build Slug again until the analysis/action
graph and declared outputs reach a self-hosted fixed point.

## Ordering

This stage has two tracks with different gates:

1. The Bazel developer graph is accepted and may continue to accelerate Rust
   builds/tests and supply a first-party query/aquery corpus.
2. Stage 10.3 analysis and Stage 10.4 self-hosting begin immediately after M7A
   accepts the bootstrap action closure: its repository sources,
   rules_rust/provider/toolchain semantics, action kinds and input trees,
   normalized aquery, and Stage 7 REAPI execution/materialization.

The bounded M2, M5, and M6 slices are accepted prerequisites but do not alone
cover the bootstrap action set. Conversely, M8 does not wait for M7B run/test/
BEP, unrelated public-ruleset, or command-format breadth. M9 exact Bazel
configuration/output identity and unadmitted exact ActionKey bytes are not
bootstrap prerequisites. Per-family compatibility classification and the typed
comparison contract below are mandatory. [Bootstrap readiness](./bootstrap-readiness.md) ties
these prerequisites to the finite production inventory and records open gates.

A Bazel-built `slug` binary is not self-hosting evidence. A passing self-build
is not enough unless the stage1/stage2 action graphs and declared output
manifests reach the fixed point below.

The first product milestone is Linux self-hosting through the ordinary REAPI
boundary, with pinned platform/toolchain inputs and the selected local actiond
or hosted executor. Stage 7 establishes the reusable cache core while implementing
bootstrap execution. Standalone library packaging, direct Bazel disk-cache
interoperability, language bindings and mixed-language repository breadth are
post-bootstrap work under [Stage 11](./11-bazel-compatible-cache-library.md)
and Stage 8. Reconcile extracted crate/proto dependencies in the production
inventory before the fixed-point gate.

## Source and Version Policy

- Pin the bootstrap oracle/tool to Bazel 9.2.0 at
  `8220c6198837d5c13d53fea211cf3282aa12408a` and retain the root
  `.bazelversion` pin `9.2.0`.
- Use bzlmod and `rules_rust` 0.73.0 from the BCR archive with integrity
  `sha256-LQyLlnthnVcXvoIQ9SokxapiTjIpo43EBxcS2x3VIvI=`. Its registry
  presubmit includes Bazel 9.x. No WORKSPACE file, legacy repository rules, or
  native language-rule fallback is allowed.
- Keep Cargo as a supported development path while the Bazel graph matures.
  `Cargo.lock` and the Bazel Rust dependency graph must have an explicit,
  reviewed synchronization policy rather than drifting silently.
- Treat archived V1 root Bazel/Buck metadata as source-inventory reference
  only. Build a fresh Bazel 9 graph; do not revive Buck-shaped ownership or
  generated V1 targets.

## BuildBuddy and Credentials

- Use the workspace `.bazelrc` for checked-in, non-secret BuildBuddy/RBE/cache
  configuration. Preserve any live untracked `.bazelrc` as user state until a
  scoped packet reviews only the repository-safe options with the user.
- Authentication remains in `~/.bazelrc` or injected CI secrets. Agents and
  tests must never open, print, copy, snapshot, or commit `~/.bazelrc`.
  Invoking Bazel normally may allow the Bazel client to consume its rc files;
  logs/evidence must not echo credentials or expanded headers.
- Do not put tokens, remote headers, certificates, or credential-helper output
  into plans, fixture expected files, command lines recorded in evidence, or
  the repository `.bazelrc`.
- BuildBuddy is the primary remote developer lane; CI is not admitted. Sibling `../actiond`
  provides a local REAPI conformance lane when a hosted service is undesirable.
  Both remain execution services behind REAPI, not Slug-core dependencies.

## Implementation Slices

### 10.1 Accepted Bazel 9 Rust graph and coverage policy

- Preserve root `MODULE.bazel`, `.bazelversion`, Bazel build metadata, and
  pinned Rust toolchains/dependencies for the V2 workspace.
- The accepted boundary is `slug_cli_v2` and its transitive V2/retained
  infrastructure crates; broader workspace coverage is separately admitted. Each
  source has one owning Bazel target and focused test target; avoid monolithic filegroup compilation.
- Preserve Bazel 9 package boundaries and visibility. Do not expose archived
  V1 paths or introduce `buck-out`-shaped outputs.
- Require a deterministic build-info input so bootstrap comparisons can normalize
  the expected compiler/version stamp without hiding semantic differences.

#### Accepted production inventory

The root production target is `//app/slug_cli_v2:slug`, declared in
[app/slug_cli_v2/BUILD.bazel](../../../../app/slug_cli_v2/BUILD.bazel). Its
accepted first closure contains these 33 first-party packages:

- 14 V2 packages: `slug_cli_v2`, `slug_commands_v2`, `slug_core_v2`,
  `slug_reapi_v2`, `slug_server_v2`, `slug_analysis_v2`, `slug_bep_v2`,
  `slug_build_api_v2`, `slug_bzlmod_v2`, `slug_events_v2`, `slug_identity_v2`,
  `slug_loading_v2`, `slug_query_v2`, and `slug_workspace_v2`.
- 19 retained packages: `allocative`, `allocative_derive`, `cmp_any`, `dice`,
  `dice_error`, `dice_futures`, `display_container`, `dupe`, `dupe_derive`,
  `gazebo`, `gazebo_derive`, `lock_free_hashtable`, `lock_free_vec`, `starlark`,
  `starlark_derive`, `starlark_map`, `starlark_syntax`, `strong_hash`, and
  `strong_hash_derive`.

The five local proc-macro targets are the five listed `_derive` packages.
`slug_configuration_v2`, `pagable`, `pagable_derive`, and `static_interner` were
outside this accepted closure; external crates/proc macros belong to the pinned
crate universe. This is the accepted inventory, not a fresh audit of every
current Cargo change. Before bootstrap admission, reconcile live Cargo/Bazel
coverage against it and record any delta in the readiness matrix.

Cargo remains authoritative for dependency declarations/resolution. Review
`Cargo.lock`, `Cargo.Bazel.lock`, `MODULE.bazel.lock`, toolchain and manifest
changes together. The accepted toolchain is `nightly/2025-09-14`; retain explicit
nightly channel selection where needed. Crate-universe rendering uses the root
manifest/lock, isolation, and generated build scripts. Bazel 9.2 removed `sync`;
a separately authorized repin uses `CARGO_BAZEL_REPIN=1 bazel mod deps`.

The selected live-inventory packet at main `39e3a89ac` permits only one
locked/offline Cargo metadata snapshot rooted at the CLI manifest and Linux
normal/build edges, plus one no-fetch/download-disabled Bazel 9.2 streamed-JSON
batch query in a network namespace. Each has 60-second TERM/three-second KILL
limits, output caps and a complete command/output/cleanup receipt. Cargo's
selected package closure and Bazel's unconfigured declared superset remain
separate with an explicit path mapping. The packet records exact deltas before
any M7A family packet is selected; it does not build, test, repin, execute
actions or change the fixed developer manifest.

The Cargo inventory view completed. The Bazel query emitted no row and exited
37 because loopback remained down inside its fresh namespace, triggering a
Bazel 9.2 metrics-collector null dereference. A separately reviewed recovery
may run the identical query once after asserting the namespace exposes only
`lo` and bringing that loopback device up. This does not add an external
interface, relax no-fetch/download controls or authorize another Cargo
snapshot. The receipt must persist pre/post interface names `["lo"]`, loopback
UP and `external_interfaces=0`.

The recovery later exited at its supervisor/interface gate before Bazel
invocation, with empty output and exact cleanup recorded in receipt
`bc4c11d4a939afa8b143f1bbc484d2d70a212a993111f79946f83f703209b252`.
No current Bazel reachability result exists. The user-clarified F3 packet later
reached its clean 30-second ceiling and closed without acceptance. Resume only
partial accounting from the preserved Cargo inventory and existing
authenticated artifacts; no Stage 10 query, build or behavioral admission is
authorized.

#### Partial live Cargo inventory reconciliation (2026-09-16)

The frozen locked/offline Linux normal/build Cargo receipt
`b3886868867d6b2c6f8d1d49575fbc0f4322ab83216e1da456ae7e2f1e2c5e2`
contains 345 selected packages and 957 selected edges: 35 first-party and 310
external packages. Its analysis SHA-256 is
`dad20c2240aa421f5d99fe30190cbe7fa9035b4e388ad64aa30e762afaf9cd10`.
Set comparison against the accepted 33-package list above finds exactly two
additions, `slug_configuration_v2` and `slug_starlark_v2`, and no removal. This
is a Cargo-selected delta, not a Bazel configured or declared-reachability
result.

The explicit path mapping for all 35 local packages is: every `slug_*_v2`
package maps to `app/<package>` (16 packages); `allocative` and
`allocative_derive` map to `allocative/<package>`; `dice`, `dice_error` and
`dice_futures` map to `dice/<package>`; `cmp_any`, `display_container`, `dupe`,
`dupe_derive`, `gazebo`, `gazebo_derive`, `strong_hash` and
`strong_hash_derive` map to `gazebo/<package>`; `lock_free_hashtable` and
`lock_free_vec` map to `shed/<package>`; and `starlark`, `starlark_derive`,
`starlark_map` and `starlark_syntax` map to `starlark-rust/<package>`.
Thirty-four directories currently contain a `BUILD.bazel` with a Rust target
named for the package. `app/slug_configuration_v2` has no BUILD file or target;
its absence is a static graph gap, not evidence about a completed Bazel query.
The new `//app/slug_starlark_v2:slug_starlark_v2` target is statically named by
the Core, loading and bzlmod BUILD files. `slug_configuration_v2` is a normal
Cargo dependency of analysis, build API, commands, Core, loading and server
packages, so its missing Bazel package is material to the selected CLI
closure. The generated `Cargo.Bazel.lock` contains its path package, but a lock
entry does not create the missing BUILD target.

The five selected local proc macros remain `allocative_derive`, `dupe_derive`,
`gazebo_derive`, `starlark_derive` and `strong_hash_derive`. The five selected
build scripts are `allocative`, `slug_reapi_v2`, `starlark`, `starlark_map` and
`starlark_syntax`. Their checked-in BUILD files still declare script owners:
three `rust_nightly` probes, the five checked-in REAPI/protobuf inputs with
vendored protoc, and the Starlark syntax LALRPOP grammar. The Cargo receipt
records enabled local features. Only six packages have nonempty feature sets:
`slug_cli_v2`, `slug_core_v2` and `starlark_map` have `default`; `gazebo` has
`str_pattern_extensions`; `strong_hash` has `num-bigint,triomphe`; and
`allocative` has the 19-feature set in the frozen analysis. The other 29 have
no enabled features in this selected resolution. The accepted 33-package
inventory did not freeze feature vectors, so this cannot establish a feature
delta against that baseline.

The nine authority-file SHA-256 values in the frozen receipt still match the
current files, including `.bazelversion`, `rust-toolchain`, both Cargo locks,
`MODULE.bazel` and its lock. This proves no authority-file change since that
Cargo snapshot; it does not prove historical pin parity with the original
33-package developer gate. Static key comparison finds only 34 of the 35
selected local packages in `Cargo.Bazel.lock`'s `workspace_members`:
`slug_starlark_v2 0.1.0` is absent despite its BUILD target. Of 310 selected
external package name/version keys, 302 appear in its `crates` map; the eight
absent keys are `adler2 2.0.1`, `crc32fast 1.5.1`, `filetime 0.2.29`,
`flate2 1.1.9`, `miniz_oxide 0.8.9`, `simd-adler32 0.3.10`, `tar 0.4.46`
and `xattr 1.6.1`. These are missing pinned package entries, not a proven
Bazel configured dependency set or a license to invent lock contents.

The live Bazel query and its recovery produced zero rows. Bazel declared
reachability, generated action coverage, package source/checksum equality and
Cargo-to-Bazel feature equivalence remain unknown. Fixing the static
configuration BUILD gap, synchronizing the missing lock entries under a
separate reviewed packet and obtaining Bazel evidence are prerequisites to
closing M7A coverage; this accounting admits no action family, exact key,
REAPI behavior or self-hosting.

The first graph-sync design was rejected before mutation: rules_rust repin
needs full-workspace Cargo inputs, while the host cache then lacked 54 of 448 locked
registry archive/source pairs. A locked/offline full-workspace metadata
preflight failed on `anstyle-wincon 3.0.11` in 0.15 seconds. The selected
`WP-7-10-m7a-cargo-cache-acquisition-r1` stages exact lock inputs only, under
one bounded locked fetch and a short offline metadata gate. Its frozen gap
inventory SHA-256 is
`5c8294dcbebbefea8602f891c28ee2545ff701d572084ae236049fdd0eba3189`.
The missing configuration BUILD owner, six normal consumer edges, generated
Bazel lock synchronization, fresh graph evidence and M7A readiness remain
unaccepted and require a separate reviewed packet.
The single locked fetch and checksum check now found all 448 registry archives
and source directories present, with unchanged `Cargo.lock`; the sole offline
full-workspace metadata output includes exactly those 448 registry keys.
Fetch receipt SHA-256 is
`861309ac82fcf02d8d8385b25622757ea0f4c04ffd4e568122532db0031f31b6`.
Its metadata wrapper failed only while processing a local package with null
source after Cargo completed; recovered output/cleanup receipt SHA-256 is
`f5163a1827114cdc71acc9c325369db93f23a3cbc807cf821203e5486fbb5bea`.
Independent final review `ACCEPT` confirmed the recovered metadata evidence
without a rerun. The graph and M7A gates remain open.
The `WP-7-10-m7a-bazel-graph-sync-r2` packet used that accepted
offline cache to add the missing configuration BUILD owner and six normal
first-party edges, then attempted one bounded, offline crate-universe repin.
The generated lock was not changed; live Bazel reachability, owner
compilation and M7A readiness remain separate gates.
The first repin stopped in 1.098 seconds at Bazel 9.2.0's system-network
profiler null dereference, before generated-lock mutation. Receipt SHA-256
`86995d8a436d74b7db448d5fdbb3aa652a30c7f3ab50477036a3f6dca37b1051`.
The selected `WP-7-10-m7a-bazel-graph-sync-recovery-r1` changes only Bazel's
profiler flag to disable system-network collection for one bounded offline
repin; independent design review `ACCEPT` confirmed the flag-only correction.
Static BUILD edits remain uncommitted
until generated lock and final review.
The flag-only recovery command exited 0 in 6.722 seconds but added two blank
lines to root `Cargo.lock`'s unused-patch section. Receipt SHA-256 is
`2322f3a8481e3e7393ca0d81f0572153f979a0a5cf2e8d4c44da73aff4806cfb`.
Exact root-lock bytes were restored and parsed TOML remained equal, yet the
packet's byte-hash gate rejects the invocation. Generated-lock candidate
analysis SHA-256
`19b365fff8230f71021edf660901769d8a940744879dc2f5d6ac5486b1325997`
finds all 35/310 selected keys and all 448 registry checksums with nine
additive crate keys and no removals. The selected
`WP-7-10-m7a-generated-lock-candidate-validation-r1` permits one read-only,
fresh-output-base, no-repin lock-coherence check before final static-sync
review. Bazel graph and M7A evidence remain open.
The sole no-repin, fresh-output-base validation exited 0 in 2.171 seconds;
all tracked authority/BUILD hashes and status stayed unchanged. Receipt
SHA-256 is
`290a3c2fce004a895b4470bd985d3618b047eb890457404daa37b182ebcbe278`.
Together with parsed Cargo-lock equality and all 448 registry checksums,
this supports the generated-lock candidate for independent final review,
without admitting target reachability or M7A behavior.
Independent final review `ACCEPT` found exact 497/497 Cargo-lock package keys,
448 matching registry checksums and seven BUILD files aligned with Cargo.
Static BUILD/lock synchronization is accepted. The next packet must obtain
fresh target graph evidence; this result proves no compilation or M7A
behavior.
The selected `WP-7-10-m7a-bazel-declared-root-query-r1` permits one fresh
offline/download-disabled target dependency query under a 30-second ceiling.
It reports selected local path reachability and any gap; query labels are an
unconfigured declared view and cannot close configured/action or M7A gates.
The one query exited 0 in 2.952 seconds and yielded 18,846 valid distinct
labels. All 35 selected local package paths, the configuration/Starlark
targets and all eight previously missing selected external repositories are
present in this unconfigured declared closure. Command receipt SHA-256 is
`1935a3d026a6a31566d6250ac2fc49747b1f08de04d36819984e9dd34ae89b8a`;
parsed analysis SHA-256 is
`f734391152ac608a0787b7443575b6fe3253294b8211494ba161d6f1545f0b13`.
Independent final review `ACCEPT` confirmed the saved label set and strict
unconfigured classification. Configured/action and buildability evidence is
still absent.
The selected `WP-7-10-m7a-bazel-configured-root-cquery-r1` obtains one
full-root configured dependency view with cached offline inputs and a
30-second ceiling. It preserves only displayed seven-character configuration
tokens, not full identities or target/exec classification, and records any
analysis failure; no build, action, test or M7A behavior is selected.
That one cquery exited 1 in 2.862 seconds with no configured rows because
the registered nightly Rust toolchains did not match the default stable
channel setting. Receipt SHA-256 is
`80e255dbdffa001fa48f05d78d3b10d1ba9ed3c489a4c184e95b6238b06cd4d9`;
tracked inputs stayed unchanged. Configured reachability remains open; the
existing developer-gate invocation supplies the missing
`--@rules_rust//rust/toolchain/channel=nightly` flag.
The selected `WP-7-10-m7a-bazel-configured-root-nightly-flag-r1` corrects
only that invocation setting for one fresh, offline, 30-second-capped
full-root cquery. It leaves all configured/action/buildability gates open
pending a successful result and separate inspection.
That sole corrected cquery selected the nightly toolchain but exited 1 before
configured rows because the pinned Linux x86_64 compiler archive was not
available to the download-disabled repository rule. Receipt SHA-256 is
`9f305306861c887bffc9ca6dbde4c1930cf8733cfa91cbe97b41c3b55364ab5c`;
tracked inputs stayed unchanged. The selected toolchain pin/cache packet uses
the official dated release manifest to pin six Linux x86_64 archives and
prepare offline repository materialization, without retrying cquery or
claiming buildability.
The six Linux x86_64 archive hashes and matching rustfmt date are now in
`MODULE.bazel`. Offline module resolution changed no lockfile; all six
official archives passed the manifest checksum gate, and one corrected
download-disabled Bazel fetch materialized the selected toolchain-tools repo
in 11.147 seconds with tracked inputs unchanged. Acquisition and fetch
receipt SHA-256 values are
`c84576af16f7c46bac05eb878d052b2196278473e32a280db9e2219b072162e2`
and `1a47242a21971bbd5b63ae5d21f49f9a7a079d916643e8b02e187ef4ab7e8b10`.
This proves Linux x86_64 toolchain input provenance/materialization only;
configured CLI analysis and buildability are still open.
Independent final review `ACCEPT` confirmed exact official archive hashes,
the MODULE delta and clean selected-repository materialization; the bytes may
have come from the supplied distdir or Bazel's checksum-keyed cache.
The selected `WP-7-10-m7a-bazel-configured-root-offline-r1` now obtains one
full-root configured cquery using the pinned nightly toolchain, verified
archives and download-disabled Bazel. It cannot infer features, generated
inputs, action execution or buildability from label rows alone.
The one offline cquery exited 0 in 5.337 seconds with 17,685 valid
configured label/token rows and unchanged tracked inputs. All 35 selected
local package paths have non-null rows; the CLI, configuration and Starlark
V2 targets appear. Receipt and parsed analysis SHA-256 values are
`459c41aad42c9259adc239b74271c0e7b380bb6970135e4e266ee91e13b322bc`
and `266cf64932c2ef555ca5d36bb8d87ad95a8b0fe6a641a4c41973527f0c711dde`.
This is configured label reachability only; full configuration identity,
features, generated inputs, actions and buildability still need proof.
Independent final review `ACCEPT` independently confirmed every row and
the 35 Cargo manifest-parent paths, without widening the configured-label
claim to action or buildability evidence.
The selected `WP-7-10-m7a-local-feature-flags-r1` corrects only selected local
Rust rule and build-script feature inputs against the frozen Cargo snapshot.
Its discriminating check is exact attribute comparison plus pinned rules_rust
source transformation; no new Bazel analysis, build or test is selected.
External feature parity, generated inputs, actions and buildability remain
open.
The four BUILD edits passed a frozen Cargo snapshot comparison for all 35
local packages, 36 production Rust rules and five build scripts, with no
feature-set mismatch. Pinned rules_rust source hashes and diff/plan checks
passed; no Cargo/Bazel command, build or test ran. Independent final review
`ACCEPT` confirmed the mapping and unchanged frozen locks/MODULE. This is
local attribute parity only; action and buildability gates remain open.
The selected `WP-7-10-m7a-external-tokio-feature-probe-r1` classifies one
Cargo metadata versus platform-select feature ambiguity for Tokio. It allows
one offline, capped unit-graph observation of the Linux CLI without build
actions. No external feature parity or compilation claim is selected.
The one unit-graph observation exited 0 in 0.506 seconds without compiling.
The CLI root reaches one normal Linux Tokio library unit, whose 22 features
match the lock's Linux selection and omit metadata-only `windows-sys`.
Receipt SHA-256 `6a83706f5289c18d00619ddb9e4df96c0feddb7f0d5304bb3ade2b5502955696`
and independent final review `ACCEPT` preserve this one-unit claim only.
The selected `WP-7-10-m7a-external-unit-feature-accounting-r1` compares all
root-reachable external Cargo compilation units in that saved graph with
generated lock feature lists, separating host and Linux target units. It
runs no new Cargo/Bazel command and cannot prove configured Bazel flags.

Generated sources remain declared build outputs: build scripts supply
`rust_nightly` configuration for allocative/starlark/starlark_map; LALRPOP
processes starlark_syntax's grammar; vendored protoc/tonic-build generate Rust
from slug_reapi_v2's five checked-in protos. Do not replace these with ambient
host probes, handwritten generated Rust, or Cargo execution delegation.

### 10.2 Accepted Bazel/BuildBuddy developer gate

Gates A–C accepted the 33-package production boundary and transitive V2 tests.
The fixed developer manifest is
[buildbuddy_cache_targets.txt](../../../../tests/v2_oracle/buildbuddy_cache_targets.txt):
one production label and 43 test labels at acceptance. Preserve the fixture
ownership/runfiles adaptations and the strict manifest-aware gate drivers.
Current changes to source/test coverage need a new coverage audit; the historical
counts are not evidence for unreviewed additions.

The accepted full-cache proof has 1,006 local sandbox misses in prime and 1,006
remote-cache hits in replay, identical eligible action digest multisets, and
43 passed tests with zero/43 remotely cached test results. The separate full-RBE
proof has 1,006 remote SpawnExec records, 43 passed tests, no cache hits/local
fallback, and clean lifecycle. These are sufficient accepted developer evidence,
not Slug execution or self-hosting evidence; routine remote reruns are unnecessary.

The supported local no-argument gates run manually and serially:

```text
python3 tools/v2_oracle/buildbuddy_cache_gate.py
python3 tools/v2_oracle/buildbuddy_rbe_gate.py
```

Cache and RBE retain separate CLI invocations, lifecycle namespaces and closed
`PROVED_CACHE_ONLY` / `PROVED_RBE` records. Preserve strict singleton BEP and
all-SpawnExec accounting, immutable manifest/profile bindings, private bounded
no-follow/replacement-aware artifacts, identity-safe shutdown/removal and cleanup
suppression. No reconstructed combined command, relaxed parser, automatic live
retry, exposed raw credential-bearing evidence, or direct-local RBE fallback is
admitted. The baseline history below records exact command vectors, schema,
shared-parser identities, caps and accepted implementation/proof anchors.

CI was explicitly declined. No provider, workflow, triggers, credentials,
permissions, runner, concurrency or cost policy is selected; reopening CI needs
a new explicit user request. This decision parks only the developer-gate slice.
M8 still requires M7A readiness and the Stage 10.3/10.4 proofs below.

### 10.3 Slug-as-Bazel Analysis Gate

- Use the Slug repository itself as a Stage 1 oracle workspace.
- Bazel 9 and Slug evaluate the same `MODULE.bazel`/BUILD graph. Compare target
  patterns, configured targets, providers needed by rules_rust, toolchains,
  transitions, and normalized `aquery` output before executing a self-build.
- The comparison must use the same Stage 4/6/8 graphs as ordinary commands;
  bootstrap-specific analysis shortcuts, precomputed action manifests, and
  Cargo delegation are forbidden.

### 10.4 Self-Hosted Fixed Point

Define the stages precisely:

- stage0: Bazel 9.2.0 builds the Slug binary and its declared runtime files;
- stage1: stage0 Slug, invoked through the Bazel-compatible command surface,
  builds the same Slug targets through REAPI; and
- stage2: stage1 Slug repeats that build from an isolated output base.

Acceptance requires:

- stage1 and stage2 `query`, `cquery`, and `aquery` results match under the
  typed comparison contract below;
- stage1 and stage2 declared output path/type/mode/symlink/digest manifests
  match after only the reviewed build-info normalization;
- every stage1/stage2 build action crosses REAPI with zero direct-local actions;
  record the exact stage1 executable used to launch stage2. Isolated output
  bases alone are not a cache-miss proof: establish independent miss execution
  for the fixed-point runs with a reviewed cache-read/namespace policy, then
  prove warm replay separately without salting semantic inputs or digests;
- clearing local outputs while retaining remote cache produces explainable
  action-cache hits, and clearing the relevant cache produces explainable
  re-execution; and
- stage1/stage2 do not invoke Cargo or Bazel as a hidden executor.

Stage0 and stage1 need not be byte-identical if the compiler/toolchain embeds a
known stage identity. Any normalization must be named, minimal, and tested;
stage1 and stage2 are the required fixed point.

### 10.5 Complex-Project Stress

After the focused bootstrap gate, use a populated sibling `../llvm-project` as
an optional loading/analysis/query/aquery stress corpus. Convert every defect
into a small repository-owned Bazel 9 oracle before fixing it. The sibling was
not a valid checkout in the 2026-07-22 review and is not a prerequisite.

## Exact Test Criteria

- Bazel 9.2.0 builds and runs the focused Rust unit/integration tests for the
  CLI plus its transitive V2 crates.
- A second identical Bazel/BuildBuddy invocation records remote cache reuse
  without leaking credentials into BEP or checked-in evidence.
- Bazel and Slug `query`/`cquery` results for the bootstrap target closure match
  at the accepted Stage 8 formats.
- Bazel and Slug normalized `aquery` `ActionGraphContainer` results match for
  the initial bootstrap closure before self-hosted execution starts.
- Isolated stage1 and stage2 runs satisfy every fixed-point condition above.
- A negative test proves the bootstrap driver fails if stage1 delegates to
  Cargo/Bazel or emits a direct-local action.

## Acceptance Criteria

- Bazel plus BuildBuddy is a documented, tested fast development path for
  building and testing Slug without repository-stored credentials.
- The Bazel graph covers all source and tests required by the bootstrapped
  binary and stays synchronized with the active Cargo workspace.
- Slug analyzes its own Bazel 9 graph without a bootstrap-only semantic path.
- A Bazel-built Slug reaches the stage1/stage2 self-hosted fixed point through
  REAPI.
- Sandboxing inside Slug remains out of scope; backend isolation is recorded as
  backend evidence only.

## Validation Shape

The initial production label is `//app/slug_cli_v2:slug`. The implementation
packet freezes exact config and closure labels after reviewing the repository-safe
`.bazelrc`, Bazel graph and bootstrap readiness matrix. Record commands with
credentials and expanded headers omitted. The final validation bundle must
contain:

```text
Bazel 9 release and immutable commit
rules_rust/toolchain/dependency pins
Bazel build and test target results
BuildBuddy cache/RBE structured evidence with secrets redacted
Bazel-versus-Slug query/cquery/aquery comparison artifacts
stage0/stage1/stage2 binary identities
stage1/stage2 action and output manifest comparison
REAPI evidence proving direct_local_actions=0
Cargo-versus-Bazel active workspace coverage audit
```

## Bootstrap comparison contract

This is a future comparator contract, not an implemented comparator or new
semantic identity. Stage 10 compares ordinary Stage 4/6/8 graph results and
Stage 7 execution evidence. Structural configuration, display/path tokens,
Bazel checksum, per-family ActionKey and REAPI/CAS digests remain distinct.
No JVM, Java bytecode/helper or Bazel/Java semantic delegation may enter Slug.

### Bazel-versus-Slug graph comparison

Configuration IDs and admitted configured-output root segments may differ.
Build a command-local, graph-scoped one-to-one correspondence from matched
structural owner/configuration/artifact relationships, not from display hashes
or coincidentally equal text. Preserve null/target/exec distinctions, equality,
change/restoration, ordering, multiplicity and graph topology. Unmatched,
ambiguous or colliding mappings fail closed.

The same typed generated-path correspondence must be applied consistently to
artifact fields and proven generated-path occurrences within action argv,
environment values and paramfile contents. Each admitted action family's
comparison schema identifies those slots, their path/argument encoding, segment
boundaries and producer provenance before normalization. Preserve literal
prefixes/suffixes, option spelling, quoting/escaping, separators, argument/env
ordering, duplicate/missing fields and all other bytes. Paramfile comparison
uses the declared paramfile producer/encoding and verifies the actual uploaded
bytes separately. Never use a regex, global text replacement, substring match,
or basename heuristic to infer that arbitrary content is a generated path.
An unmodeled occurrence is an unsupported comparison and blocks that family;
it is not silently normalized or declared semantically unequal.

Each family has an explicit ActionKey classification in bootstrap readiness.
Preserve every accepted exact projection byte. For Slug-native or
unsupported/deferred ActionKey fields, the comparator schema names the precise
family/field allowance and reports it in comparison evidence; undeclared
omissions or unclassified fields fail closed. These allowances apply only to ActionKey
inspection, never to argv, environment, inputs, outputs or other action facts.
Exact projection work uses the
[Stage 6 feasibility checkpoint](./06-analysis-toolchains-and-actions.md#per-family-actionkey-feasibility-checkpoint).
Comparison-only path correspondence cannot produce runtime fingerprint bytes.
M9 owns unadmitted exact projections and exact configuration/output identity.

Do not normalize mnemonics, owner labels, selected platforms/properties,
non-path argv/env/paramfile bytes, logical FileWrite content, artifact relative
names, input/output topology, failures or ordering. Do not normalize REAPI/CAS
digests: verify their exact protobuf/content derivation for each system's actual
graph, reporting intentional projection differences separately. Bazel-versus-
Slug digest equality is not implied by graph correspondence or an exact
ActionKey; cache interoperability requires its own evidence.

### Stage1-versus-stage2 fixed point

The cross-system path/token allowances above do not relax the self-hosted
fixed point. Stage1/stage2 declared output paths, types, modes, symlink targets,
contents and digests must match. Compare their graph/action identities and
arguments using the same Slug identity scheme; do not remap unrelated differences
until they disappear. Physical isolated output-base locations may be recorded as
an execution envelope, never scrubbed from declared output content or action
inputs. Only a reviewed, named deterministic build-info normalization is allowed;
it identifies exact fields/outputs, reason and positive/negative tests. If no
normalization is named, comparison is byte exact. Record raw manifests/digests
as well as any admitted normalized comparison, and verify every raw CAS digest.

### Required comparator evidence

Before Stage 10.3 implementation, freeze the schema and reuse accepted Bazel
9.2 `ActionGraphContainer`/action-family source and oracle fixtures. Prove a
consistent configured-root change matches in artifact fields, typed argv/env
slots and encoded paramfile paths, while near-match literals, logical content,
changed suffixes, quoting, argument order, different owner/configuration edges,
missing artifacts and accepted exact ActionKey changes remain discriminating.
Prove that only declared family/ActionKey allowances match, and that unknown
families, an exact field becoming absent, and allowances applied to semantic
fields fail. Report deferred fields without claiming whole-record byte parity.
Prove malformed or ambiguous mappings fail closed, and stage1/stage2 path,
mode, symlink, content and digest changes fail outside the named build-info rule.

Comparator mappings and decoded paramfiles are command-local comparison scratch;
they create no DICE key/cache, retained semantic graph or execution fallback.
Runtime ownership, invalidation and output publication stay with the ordinary
producers. Create one cohesive comparator implementation packet with code/fixture scope,
validation and growth estimates when that readiness row is selected. Resolve
remaining schema decisions within its review; a separate design-only handoff
is needed only for a new shared boundary under the orchestration skill.

## Historical evidence index

Completed Stage 10 chronology is preserved at Git baseline
`c5e7414d77b203a04337c980aacd0d37fd73e501` (short `c5e7414d7`) in this same path:
`git show c5e7414d7:thoughts/shared/plans/slug-v2-subplans/10-bazel-build-and-bootstrap.md`.
Ranges below refer to that immutable version. Historical “next” instructions,
M2/M5/M6 waits, CI proposals and broad ActionKey deferral are superseded by
current canonical scheduling and the contracts above; acceptance evidence stays
reachable and is not rerun merely for compaction.

| Baseline lines | Historical anchor / reusable evidence |
|---|---|
| 76–195 | “Accepted first-closure design”; inventory, pins, generated-source/dependency ownership, Gates A/B |
| 196–896 | “Gate C0 CLI test runfiles design”; C1 transitive tests and canonical fixture payload migration |
| 897–1065 | “Bazel/BuildBuddy Developer Gate”; repository-safe cloud policy and initial stop |
| 1066–2327 | “BuildBuddy cache-evidence design”; exact command vectors, strict parser, schema/privacy/lifecycle contracts and diagnostics |
| 2328–2484 | “Full-gate driver reconciliation design”; immutable manifest, shared APIs, full-cache/RBE drivers and accepted transported proofs |
| 2485–2542 | “BuildBuddy CI admission decision”; no-CI acceptance, developer-gate records and superseded diagnostic-next paragraphs |
| 2543–2644 | Original bootstrap/fixed-point gate and prior identity comparison; use the corrected contract above |
