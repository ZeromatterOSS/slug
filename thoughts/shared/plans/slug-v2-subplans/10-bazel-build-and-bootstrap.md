# Stage 10: Bazel Build and Bootstrap

## Goal

Make Bazel 9 a fast, supported way to build and test Slug through BuildBuddy,
then use the resulting binary to build Slug again until the analysis/action
graph and declared outputs reach a self-hosted fixed point.

## Ordering

This stage has two tracks with different gates:

1. The Bazel developer graph may start immediately and progress alongside
   M1-M5. It accelerates Rust builds/tests and supplies a first-party query and
   aquery corpus.
2. Slug self-hosting starts only after Stage 8 exact `aquery` and Stage 7 REAPI
   execution/materialization are accepted for the bootstrap action set.

A Bazel-built `slug` binary is not self-hosting evidence. A passing self-build
is not enough unless the stage1/stage2 action graphs and declared output
manifests reach the fixed point below.

## Source and Version Policy

- Pin the bootstrap oracle/tool to Bazel 9.2.0 at
  `8220c6198837d5c13d53fea211cf3282aa12408a` and add a root
  `.bazelversion` with `9.2.0` when implementation begins.
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
- BuildBuddy is the primary remote development/CI lane. Sibling `../actiond`
  provides a local REAPI conformance lane when a hosted service is undesirable.
  Both remain execution services behind REAPI, not Slug-core dependencies.

## Implementation Slices

### 10.1 Fresh Bazel 9 Rust Graph

- Add root `MODULE.bazel`, `.bazelversion`, Bazel build metadata, and pinned
  Rust toolchains/dependencies for the V2 workspace.
- Start with `slug_cli_v2` and its transitive V2/retained-infrastructure crates,
  then cover all active workspace members. Each source has one owning Bazel
  target and focused test target; avoid monolithic filegroup compilation.
- Preserve Bazel 9 package boundaries and visibility. Do not expose archived
  V1 paths or introduce `buck-out`-shaped outputs.
- Add a deterministic build-info input so bootstrap comparisons can normalize
  the expected compiler/version stamp without hiding semantic differences.

#### Accepted first-closure design (2026-08-05)

`WP-10-m8-bazel-developer-graph-boundary-design` inspected the live manifests
without running Cargo or Bazel and accepts this finite production boundary:

- `slug_cli_v2` reaches exactly 14 V2 packages: `slug_cli_v2`,
  `slug_commands_v2`, `slug_core_v2`, `slug_reapi_v2`, `slug_server_v2`,
  `slug_analysis_v2`, `slug_bep_v2`, `slug_build_api_v2`, `slug_bzlmod_v2`,
  `slug_events_v2`, `slug_identity_v2`, `slug_loading_v2`, `slug_query_v2`, and
  `slug_workspace_v2`. `slug_configuration_v2` is not in this closure.
- It reaches exactly 19 retained packages: `allocative`, `allocative_derive`,
  `cmp_any`, `dice`, `dice_error`, `dice_futures`, `display_container`, `dupe`,
  `dupe_derive`, `gazebo`, `gazebo_derive`, `lock_free_hashtable`,
  `lock_free_vec`, `starlark`, `starlark_derive`, `starlark_map`,
  `starlark_syntax`, `strong_hash`, and `strong_hash_derive`.
  `starlark_map` does not enable its optional `pagable` feature, so `pagable`,
  `pagable_derive`, and `static_interner` are excluded. The locked external git
  package `sorted_vector_map` remains pinned at `84a82026...`.
- The five local proc-macro targets are `allocative_derive`, `dupe_derive`,
  `gazebo_derive`, `starlark_derive`, and `strong_hash_derive`. External proc
  macros belong to the generated crate universe.

The toolchain is the repository's `nightly/2025-09-14`, with edition `2024`
on 2024 crates and edition `2021` on `slug_server_v2`. Until a later reviewed
repository configuration exists, every credential-free local command passes
`--@rules_rust//rust/toolchain/channel=nightly` explicitly because the
rules_rust channel setting defaults to stable.

Cargo stays authoritative for dependency declarations and resolution.
Implementation removes the root `Cargo.lock` ignore, validates and commits
that lock, and gives crate_universe the root manifest and lock with
`isolated = True` and `generate_build_scripts = True`. A separate checked-in
`Cargo.Bazel.lock` owns reproducible rendering, and `MODULE.bazel.lock` owns
bzlmod resolution. Manifest/toolchain changes update and review Cargo inputs
first; only an explicit `CARGO_BAZEL_REPIN=1 bazel sync --only=slug_crates`
may then update the rendering lock on rules_rust versions that support `sync`.
Bazel 9.2 has removed that command, so this repository evaluates the named
module extension with `CARGO_BAZEL_REPIN=1 bazel mod deps` instead. All affected
lock diffs are reviewed together. The generated external repository may cover
more than the CLI closure because the root manifest names the full workspace;
only the owned first-party Bazel target graph is claimed to be closure-limited.

Generated-source ownership is explicit and remains within the same closure:

- `cargo_build_script` carries the `rust_nightly` cfg emitted by the
  `allocative`, `starlark`, and `starlark_map` build scripts from the pinned
  compiler, rather than replacing it with an ambient host probe.
- A first-party build-script target runs LALRPOP for
  `starlark_syntax/src/syntax/grammar.lalrpop`; generated Rust stays in Bazel
  outputs.
- A first-party build-script target runs vendored protoc plus tonic-build for
  the five checked-in `slug_reapi_v2` protos consumed through `OUT_DIR`;
  generated Rust is neither handwritten nor checked in.

The implementation is split into three reviewable gates. Gate A owns only root
module/toolchain/lock metadata and BUILD files for the 19 retained packages;
it replaces all 19 live Buck/fbcode-shaped BUILD files in that closure,
updates the archive checker so fresh root `MODULE.bazel`/`BUILD.bazel` are no
longer mistaken for archived V1 metadata, and builds the retained `dice` and
`starlark` roots. Gate B adds BUILD files for the 14 V2 packages, the REAPI
proto generation, `slug_cli_v2` library, and
`//app/slug_cli_v2:slug`, then proves the first complete production build.
Gate C maps the CLI unit and integration tests and then every unit/integration
test owned by the transitive V2 packages. The CLI integration targets must
adapt compile-time `CARGO_BIN_EXE_slug`/`CARGO_MANIFEST_DIR` with declared
binary and fixture runfiles; they may not silently drop, rewrite, or use Cargo
as an executor.

Gates A-C use local Bazel with `--ignore_all_rc_files`; no repository or home
rc is inspected or consumed, and no BuildBuddy/cache/RBE claim is made. A later
credential-reviewed cache-only packet may add a non-secret opt-in repository
configuration; RBE remains a distinct evidence packet. Query, cquery, aquery,
self-hosting, and M2/M5/M6 semantics remain outside this developer-graph work.

Gate A is accepted. Bazel 9.2.0/rules_rust 0.73.0 now owns all 19 retained
packages, five local proc macros, three compiler-channel build scripts, and the
LALRPOP build script. The fresh bzlmod graph pins `nightly/2025-09-14`; Cargo,
crate-universe rendering, and bzlmod locks are checked in and a no-repin
`bazel mod deps` left all three hashes stable. The first build exposed the
grammar as compile-only data; declaring it as build-script runtime data fixed
the exact sandbox failure. The final credential-free retained-root Bazel build
and serial Cargo `dice`/`starlark` check passed with only existing unused-import
and gold-linker warnings. The archive checker positively recognizes the fresh
V2 root metadata. Independent Terra review returned `ACCEPT`; no rc, remote,
app target, Rust source, or generated source entered the gate.

The first Gate B attempt mapped all 14 V2 packages and reached the REAPI build
script, then stopped with `REPLAN`: each vendored protoc platform crate embeds
its compile-sandbox `CARGO_MANIFEST_DIR`, which does not exist in the later
build-script sandbox. Declaring only the selected executable as a
`cargo_build_script.tools` input is also insufficient in rules_rust 0.73.0:
that attribute supplies exec-configured location expansion but is omitted from
the runner action's tools depset. No Cargo input or lock changed in this stopped
attempt.

`WP-10-m8-bazel-cli-production-protoc-bridge` corrects only that boundary. The
module imports stable apparent names for the eight locked platform executable
repositories; a root target selects by execution OS/CPU without a default, and
an action-input-bearing data aggregate carries the raw files into the sandbox.
The REAPI build script receives the selected path through private
`SLUG_BAZEL_PROTOC`, while ordinary Cargo retains
`protoc_bin_vendored::protoc_bin_path()`. It continues to consume exactly the
five checked-in protos from the materialized `CARGO_MANIFEST_DIR`. This packet
may regenerate only the rendering and module locks before proving the complete
CLI production build.

Gate B is accepted. The 14 V2 packages now have one production library target
each, `src/main.rs` is owned only by `//app/slug_cli_v2:slug`, and the reachable
app closure is exactly those 14 packages. Bzlmod imports all eight locked
vendored-protoc repositories under stable apparent names; the exec-configured
tool selection has no unsupported-platform default, while the distinct data
aggregate supplies every raw executable to the rules_rust 0.73 runner action.
The credential-free nightly Bazel build exercised REAPI generation and built
the production binary successfully. Serial Cargo checks for `slug_reapi_v2`
and `slug_cli_v2` passed through the vendored fallback. Cargo, rendering, and
module lock hashes remained stable under a final no-repin `bazel mod deps`;
archive, formatting, diff, scope, and the 444-line handwritten net cap passed.
Independent reserved review returned `ACCEPT`. No rc, remote, test target,
generated source, Cargo input, or M2/M5/M6 surface entered the gate.

#### Gate C0 CLI test runfiles design (2026-08-05)

`WP-10-m8-bazel-cli-test-runfiles-design` inventories one library unit case,
39 cases in `tests/cli.rs`, and three in `tests/graph_output.rs`. Both
integration crates use compile-time `env!("CARGO_BIN_EXE_slug")`; rules_rust
0.73 supplies that exact variable automatically when the `slug` binary is in a
`rust_test.data` edge, preserving its platform extension and runfile-relative
short path. Both also use compile-time `env!("CARGO_MANIFEST_DIR")` to reach
repository fixtures. A literal `rustc_env` value `app/slug_cli_v2` would retain
the Cargo-relative `../../tests/...` spelling under Bazel's test runfiles root;
an absolute compile-sandbox path is rejected. Windows additionally requires a
real runfiles tree rather than manifest-only lookup, as pinned by rules_rust's
own `CARGO_MANIFEST_DIR` test boundary.

The fixture owner is not implementable under the packet's frozen constraints.
The CLI tests reference exactly these 14 existing workspaces and 163 files:

| Files | Fixture workspace |
|------:|-------------------|
| 4 | `simple-rule-action` |
| 7 | `recursive-custom-rule-providers-actions` |
| 6 | `build-file-loading` |
| 4 | `query-parser-and-sets` |
| 21 | `tests-query-expansion` |
| 30 | `query-visible-visibility` |
| 26 | `query-build-load-files-provenance` |
| 10 | `query-siblings-build-file-node` |
| 12 | `query-loading-thin-vertical` |
| 11 | `query-labels-attribute-metadata` |
| 10 | `query-executables-rule-capability` |
| 13 | `query-rdeps-and-subtree-patterns` |
| 4 | `query-path-topology` |
| 5 | `query-some-selection` |

Those files span 105 nested Bazel packages. Root `glob`/`filegroup` ownership
cannot cross them, and the explicit Bazel 9.2 source-directory alternative was
discriminated in an isolated scratch module: exporting a workspace directory
analyzed, but an action traversing it failed because the directory artifact
crossed into the first nested package. A local runfiles tree can instead expose
such a directory as one recursive symlink, but Bazel 9.2's `Artifact` source
documents that source-directory access has no declared Skyframe dependencies,
is not incrementally invalidated, and is not a supported remote input. It is
therefore not hermetic evidence. Package-local exports would edit oracle BUILD
inputs and add query-visible targets; copying/archiving, undeclared repository
reads, Cargo execution, ambient source paths, or a Windows-only runfiles flag
would violate the frozen semantics. Therefore the two integration targets
remain `REPLAN` until a deliberate fixture-ownership or test-runfiles semantic
redesign is accepted; no partial target or platform exclusion is claimed.

The library unit test is independent of every blocked env/runfile boundary.
`WP-10-m8-bazel-cli-library-unit-test-implementation` adds exactly one
`rust_test(crate = ":slug_cli_v2")` and proves it with focused Bazel and Cargo
library-test commands. After that bounded target, inventory the other 13 V2
packages separately while the CLI integration redesign remains explicit.

The CLI library unit target is accepted. It reuses the production crate through
`rust_test(crate = ":slug_cli_v2")`, is private and small, and adds no source,
dependency restatement, binary, fixture, env, data, platform, process, daemon,
or integration adapter. The focused credential-free nightly Bazel test and
serial Cargo library test each ran the one case successfully. A no-repin module
evaluation left all three lock hashes stable; formatting, archive, scope, cap,
and diff gates passed. The two integration crates remain exactly at the C0
`REPLAN` boundary.

#### Gate C1 transitive V2 test boundary (2026-08-05)

`WP-10-m8-bazel-transitive-v2-test-boundary-design` mechanically reconciles
the 13 non-CLI V2 Cargo manifests, accepted production BUILD targets, source
test attributes, and every `tests/*.rs` crate. They own 1,005 source-declared
cases: 592 unit cases and 413 integration cases in 33 standalone integration
crates. Platform `cfg` attributes make the executed count host-specific; the
inventory deliberately counts source ownership rather than claiming one
cross-platform runtime total.

| Package | Unit | Integration | Integration crates | Route |
|---------|-----:|------------:|-------------------:|-------|
| `slug_analysis_v2` | 1 | 23 | 4 | fixture-free packet |
| `slug_bep_v2` | 0 | 2 | 1 | first packet |
| `slug_build_api_v2` | 0 | 22 | 4 | first packet |
| `slug_bzlmod_v2` | 278 | 186 | 11 | core packet plus scratch-path redesign |
| `slug_commands_v2` | 0 | 16 | 1 | first packet |
| `slug_core_v2` | 141 | 13 | 1 | host-tool boundary first |
| `slug_events_v2` | 9 | 0 | 0 | fixture-free packet |
| `slug_identity_v2` | 1 | 19 | 3 | fixture-free packet |
| `slug_loading_v2` | 59 | 59 | 5 | isolated platform/scratch packet |
| `slug_query_v2` | 28 | 59 | 2 | fixture-free pair plus fixture `REPLAN` |
| `slug_reapi_v2` | 0 | 14 | 1 | generated-library packet |
| `slug_server_v2` | 34 | 0 | 0 | fixture `REPLAN` |
| `slug_workspace_v2` | 41 | 0 | 0 | fixture-free packet |
| **Total** | **592** | **413** | **33** | **1,005 cases** |

Unit ownership stays one `rust_test(crate = ...)` per library: this reuses the
accepted production source graph and preserves the library test module. Each
integration source stays a separate `rust_test`; combining files would change
Cargo's crate namespaces and diagnostics. Integration targets declare only the
direct local/external crates their source imports. Dev-only crate-universe
edges use the generated `normal_dev`/`proc_macro_dev` helpers where required;
no target receives a broad fixture, env, binary, or tool adapter by default.

The following boundaries are not interchangeable:

- `slug_query_v2/tests/loading_query.rs` owns 53 cases and traverses five of
  the nested fixture workspaces already rejected by Gate C0. The 28 library
  cases and six-case `tests/query.rs` parser crate are independent, but the
  loading integration remains whole-target `REPLAN`; filtering its cases would
  change test semantics.
- Three `slug_server_v2` unit functions traverse four Gate C0 fixture
  workspaces. Because crate-mode testing compiles all 34 unit cases together,
  the complete unit target remains `REPLAN`; no partial source or synthetic
  `CARGO_MANIFEST_DIR` target is valid.
- `slug_bzlmod_v2/tests/lockfile.rs` owns 22 cases and derives a writable path
  under `env!("CARGO_MANIFEST_DIR")/../../.codex-cargo-target`. A source/runfile
  manifest directory is not writable test scratch, so that integration waits
  for a separate exact adapter design. The other 442 Bzlmod cases need no
  fixture tree. One unit case re-executes `current_exe()` with a private child
  marker; it needs only the test executable already owned by the target.
- `slug_core_v2` library tests invoke host `git` and `tar`; the runtime
  integration only needs isolated temporary directories and platform-gated
  Unix sockets. The two targets stay separate from low-risk work until a
  hermetic supported-tool owner is frozen; ambient PATH is not evidence.
- `slug_reapi_v2/tests/reapi.rs` has 14 cases. Its NativeLink transport case is
  already ignored and is the sole consumer of `SLUG_V2_NATIVELINK_ENDPOINT`.
  The Bazel target must leave it ignored and supply no service env; the other
  13 cases reuse the accepted generated REAPI library output.
- `slug_loading_v2` synthesizes unique temporary workspaces and owns
  platform-gated symlink/non-UTF-8 tests but no checked-in fixture. It receives
  a dedicated packet so Bazel sandbox/TMPDIR behavior is validated without a
  source rewrite or serial-test assumption.

The accepted serial implementation sequence is therefore:

1. Map the six pure integration crates in `slug_bep_v2`,
   `slug_build_api_v2`, and `slug_commands_v2` (40 cases) in exactly those
   three BUILD files, with a 180-line metadata/documentation cap.
2. Map `slug_events_v2` and `slug_identity_v2` (five targets, 29 cases) in two
   BUILD files at 140 lines; then map the one 41-case `slug_workspace_v2` unit
   target in its BUILD file at 80 lines.
3. Map the fixture-free `slug_query_v2` unit and parser integration targets
   (34 cases) in its BUILD file at 120 lines, then the five `slug_analysis_v2`
   targets (24 cases) in its BUILD file at 190 lines.
4. Map the one REAPI integration target in its BUILD file at 100 lines,
   retaining its default 13/14 execution boundary and generated-source graph.
5. Map the six `slug_loading_v2` targets (118 cases) alone in its BUILD file at
   190 lines, including platform and scratch-directory validation.
6. Map the Bzlmod unit target and ten fixture-free integration crates (442
   cases) in its BUILD file at 380 lines. Design the 22-case lockfile scratch
   adapter separately before authorizing that final integration crate.
7. Design the core host-tool owner before authorizing its two targets. Keep the
   CLI integrations, query loading integration, and server unit target at the
   shared nested-fixture `REPLAN` boundary until fixture ownership or test
   semantics are deliberately redesigned.

The first packet is `WP-10-m8-bazel-simple-v2-integration-tests-implementation`.
It adds only the six private, small, standalone integration targets named by
the live source files; it adds no unit target, suite, env, data, tool, fixture,
platform restriction, process, daemon, generated input, Cargo execution, or
lock change.

### 10.2 Bazel/BuildBuddy Developer Gate

- Build and test `slug_cli_v2` with Bazel 9 using the repository's named
  BuildBuddy configuration, without inspecting home-directory auth.
- Prove remote cache reuse and, when enabled, RBE execution from structured
  BuildBuddy/BEP evidence with secrets redacted by construction.
- Add CI only after the same commands work locally with credentials supplied by
  the environment. CI must distinguish remote unavailable, cache-only, and
  execution-enabled modes rather than falling back silently.
- Keep focused Cargo checks as a cross-build-system regression until the Bazel
  graph covers every active crate/test.

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

- stage1 and stage2 normalized `query`, `cquery`, and `aquery` results match;
- stage1 and stage2 declared output path/type/mode/symlink/digest manifests
  match after only the reviewed build-info normalization;
- every stage1/stage2 action crosses REAPI with zero direct-local actions;
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

Exact config/target names are chosen by the implementation packet after
inspecting the repository-safe `.bazelrc` and Bazel graph. Record commands with
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
