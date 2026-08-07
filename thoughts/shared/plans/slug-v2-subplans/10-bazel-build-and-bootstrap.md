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
- `slug_bzlmod_v2/tests/lockfile.rs` owns 11 cases and derives a writable path
  under `env!("CARGO_MANIFEST_DIR")/../../.codex-cargo-target`. A source/runfile
  manifest directory is not writable test scratch, so that integration waits
  for a separate exact adapter design. The other 453 Bzlmod cases need no
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
6. Map the Bzlmod unit target and ten fixture-free integration crates (453
   cases) in its BUILD file at 380 lines. Design the 11-case lockfile scratch
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

The simple V2 integration packet is accepted. Exactly six source-owned private
`rust_test` targets cover the BEP, four build-API, and commands integration
crates; their direct local dependency lists match the source imports without
restating transitive production edges. Credential-free nightly Bazel passed
all six targets, and serial Cargo passed all 40 cases. No-repin module
evaluation left the Cargo, rendering, and module lock hashes stable. Archive,
scope, cap, and diff gates passed, and independent latest-diff review returned
`ACCEPT`. No Rust, Cargo input, lock, unit target, suite, env, data, tool,
fixture, platform, process, daemon, generated input, or remote surface changed.
The next packet maps only the fixture-free events and identity tests.

The events/identity packet is accepted. Two crate-mode unit targets and three
source-owned identity integration targets pass credential-free nightly Bazel
5/5 and serial Cargo 29/29. The integrations depend only on the identity
library, and the unit targets reuse the accepted production crates. All three
lock hashes remain stable after no-repin module evaluation; archive, scope,
43-net/140-line cap, and diff gates pass. Independent review returned `ACCEPT`
with no adapter or dependency correction. Next map only the workspace library
unit target and its declared dev-only Tokio edge.

The workspace unit packet is accepted. One private crate-mode target inherits
the production library and adds only generated `normal_dev` aliases/deps for
the declared Tokio dev edge. Bazel passes the target; serial Cargo passes all
39 Linux-active cases while the two Windows-only cases remain source-owned and
cfg-excluded. The three locks remain stable, and archive, scope, +9-net/80-line,
diff, and independent review gates pass. Next map only the query library unit
target and fixture-free six-case parser integration.

The first query packet stops at `REPLAN`. The standalone parser target passes,
but the crate-mode target runs 28 cases and deterministically fails
`external_restricted_visible_uses_canonical_fake_caller_without_a_second_route`
with `QueryErrorKind::PreparationRestart` under both Bazel (27/28) and Cargo
`--lib` (27/28). This is the same source-owned failure previously reproduced at
clean `f9f3c3d8`; BUILD metadata cannot repair it. Do not publish a red target,
filter the case, or edit Rust under Gate C. Defer the whole unit target until a
separate semantic repair makes clean Cargo green. The independently passing
`tests/query.rs` crate advances as
`WP-10-m8-bazel-query-parser-test-implementation`, still leaving all 53
loading-query fixture cases at `REPLAN`.

The parser-only successor is accepted. Its one private standalone target owns
only `tests/query.rs`, depends only on the production query library, and passes
Bazel plus serial Cargo 6/6. No-repin module evaluation leaves all three lock
hashes stable; archive, scope, 30-net/80-line, diff, and independent review
gates pass. The 28-case library target and 53-case loading integration remain
absent at their distinct semantic and fixture `REPLAN` boundaries. Next map the
one analysis unit target and four source-owned analysis integrations.

The analysis packet is accepted. Five private targets pass credential-free
nightly Bazel 5/5 and serial Cargo 24/24. Each integration owns one source and
only its direct imports; the async targets use the declared Tokio dev edge.
All three locks remain stable, and archive, scope, 81-net/190-line, diff, and
independent review gates pass. Next map only the REAPI integration crate while
leaving its NativeLink service case ignored by default.

The REAPI integration packet is accepted. Its first compile exposed one exact
metadata omission: the ignored `#[tokio::test]` function is still compiled, so
the standalone crate needs the direct Tokio label in addition to REAPI,
build-API, and `prost`. The bounded correction adds only that label; no broad
dependency helper, endpoint, service, env, data, tool, or generated-source
owner enters the target. Credential-free nightly Bazel passes, and serial Cargo
reports 13 passed/one ignored with `SLUG_V2_NATIVELINK_ENDPOINT` explicitly
unset. All three lock hashes remain stable; archive, scope, cap, diff, and
focused correction review gates pass at 30 net lines against the 100-line cap.
Next map the six loading targets while
preserving their platform cfgs and synthetic scratch-workspace behavior.

The loading packet is accepted. One crate-mode unit target and five
source-owned integration targets pass credential-free nightly Bazel and serial
Cargo with the same 117 Linux-active cases; the remaining source-declared case
is Windows-only and stays cfg-owned. Exact direct dependency labels cover the
test sources without a broad helper. The source-defined unique temporary
workspaces work inside Bazel's sandbox without path rewrites, serialization,
fixtures, env, data, tools, processes, or platform exclusions. All three lock
hashes remain stable; archive, scope, cap, diff, and independent platform review
gates pass at 93 net lines against the 190-line cap. Next attempt the Bzlmod
unit target and ten fixture-free integrations, leaving the writable
manifest-relative lockfile integration deferred.

The first Bzlmod packet stops at `REPLAN`. The crate-mode target reproduces the
known clean-baseline
`records_exact_proxy_tag_and_innate_call_spans` failure identically under Bazel
and Cargo: 277/278 pass, with actual span 2:22–22 versus expected 2:9–39. Its
private `current_exe()` child test passes, so no subprocess or sandbox adapter
is missing. Publishing a red target or filtering one unit case is invalid; the
whole unit target waits for a separate semantic repair. Execution also
corrects the design inventory: `lockfile.rs` has 11 cases, not 22—the earlier
mechanical command counted that file once explicitly and once through its
glob. Bzlmod still owns 278 unit plus 186 integration cases, and the project
total remains 1,005; the correct split is 278 unit, 175 across the ten
fixture-free integrations, and 11 lockfile cases.

The ten-integration successor is accepted. Each private target owns one named
source and exact direct dependencies; all ten pass credential-free nightly
Bazel and one serial Cargo command, 175/175. No unit or lockfile target, env,
data, tool, fixture, platform exclusion, process, daemon, or writable-path
adapter enters the BUILD graph. All three lock hashes remain stable; archive,
scope, 141-net/380-line, diff, and independent review gates pass. Next design
only the exact writable scratch owner for the remaining 11 lockfile cases.

The metadata-only lockfile scratch design stops at `REPLAN`. Only
`lockfile_atomic_apply_writes_only_write_plans_and_errors_never_write` calls the
shared scratch helper, but all 11 cases compile into one integration target.
rules_rust 0.73 sets rustc's `CARGO_MANIFEST_DIR` after target `rustc_env` to
`${pwd}/app/slug_bzlmod_v2`; `env!` therefore embeds the compile-action sandbox,
which is stale in the later TestRunner sandbox. Runtime env cannot change that
literal. `rustc_env_files` can embed only static or compile-action values;
relative values write beneath the runfiles CWD, while `${pwd}` remains stale.
Data/runfile writes and platform-specific wrappers are rejected, and
`rust_test` has no per-target working-directory attribute. Windows
manifest-only runfiles make those workarounds weaker, not stronger.

The smallest successor deliberately changes only the test helper's scratch
selection. At runtime it prefers `TEST_TMPDIR`, then appends the existing
`.codex-cargo-target/slug_bzlmod_v2_tests/<name>-<pid>` suffix; when Bazel's
standard test variable is absent, Cargo retains the current
`CARGO_MANIFEST_DIR/../..` root byte-for-byte. Pre-cleanup with ignored removal
errors, `create_dir_all`, PID/name isolation, lockfile writes, and no
post-cleanup remain unchanged. One private `lockfile_test` target needs only
the production Bzlmod library. No target env, data, tool, runner, fixture,
Cargo input, or production source changes.

The lockfile semantic-adapter successor is accepted. The helper now chooses
Bazel's runtime `TEST_TMPDIR` while preserving the exact prior
`CARGO_MANIFEST_DIR/../..` Cargo fallback and the existing suffix, pre-clean,
creation, write, and no-post-cleanup lifecycle. One private target passes all
11 cases under credential-free nightly Bazel, and serial Cargo passes the same
11 cases through the fallback; the GNU-Windows test binary also compiles.
Formatting, no-repin module evaluation, archive, scope, 14-net/100-line cap,
credential-pattern, diff, and independent review gates pass, and all three
lock hashes remain stable. Gate C1 now has 36 accepted Bazel targets covering
458 source cases and 454 default-active Linux Cargo cases. Next design only the
hermetic host-tool owner for the core unit/runtime test split; do not map either
target until that design is accepted.

The core host-tool owner design stops at `REPLAN` for the 141-case crate-mode
unit target. Four `repository_io.rs` cases execute bare `git` and/or `tar`, and
the production paths also resolve those names through ambient command lookup.
The current module graph has no pinned, immutable, platform-selected Git or tar
executable supplier. rules_rust test `data` plus `env` can pass a declared
`$(rootpath)` file, but cannot turn that file path into the directory required
by `PATH`; Windows manifest-only runfiles make a directory shim invalid. Do not
inherit host `PATH`, discover home/system tools, add wrappers, write runfiles,
exclude Windows, or publish the core unit target. A future successor requires a
separately reviewed Linux/macOS/Windows supplier and a consuming API that takes
declared executable paths directly.

The audit also proves the standalone `tests/runtime.rs` crate is independent of
that boundary. Its 13 source cases use only unique temporary workspaces and
source-owned platform cfgs; it has no checked-in fixture, external process,
service env, daemon, or host tool. Therefore
`WP-10-m8-bazel-core-runtime-test-implementation` may add only that private
standalone target with direct core, Bzlmod, identity, loading, query, and
`tempfile` dependencies. It must preserve 13 Unix-active/12 non-Unix-active
source behavior and add no unit target, env, data, tool, runner, fixture,
platform exclusion, source adapter, Cargo input, or lock change.

The core runtime successor is accepted. Its one private standalone target has
the exact five first-party and `tempfile` dependencies, while the 141-case
crate-mode target remains absent at the host-tool `REPLAN` boundary.
Credential-free nightly Bazel and serial Cargo each pass all 13 Unix-active
runtime cases; the GNU-Windows test binary compiles with the source-owned
non-Unix cfg reduction. No-repin module evaluation leaves all three lock hashes
stable, and archive, scope, 15-net/100-line cap, credential-pattern, diff,
process-cleanup, and independent review gates pass. Gate C1 now has 37 accepted
Bazel targets covering 471 source cases and 467 default-active Linux Cargo
cases. Next design only the clean-baseline Bzlmod proxy-span semantic repair
that blocks its 278-case crate-mode target; do not change Rust in the design.

The Bzlmod proxy-span design proves no production repair is valid. At pinned
Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, `Eval` sets the
caller location to `CallExpression.getLparenLocation()`, `Location` is a
1-based UTF-16 point rather than a range, and `ModuleFileGlobals` stores that
point for extension proxies, tags, and both values created by an innate repo
rule call. starlark-rust already carries the parsed `(` as a zero-width call
site, and `nonroot_span` already combines it with the current logical include
file. The correct Slug half-open spans are therefore `2:22–22` for
`use_extension`, `3:10–10` for `proxy.tag`, and `5:5–5` for both the innate
proxy and tag. The existing full-call expectations are stale.

`WP-10-m8-bazel-bzlmod-caller-location-expectation-correction` changes only
those four assertions in the existing exact-location unit test. It must not
change `nonroot_span`, any retained carrier/equality/finalization path, AST or
source retention, include identity, DICE/source preparation, or BUILD
metadata. Focused and full Cargo library tests must pass 278/278 before a later
packet maps the independently blocked crate-mode Bazel target.

The caller-location expectation correction is accepted. Exactly four test
literals now encode Bazel's opening-parenthesis points; no producer, retained
value, equality, logical file, AST, DICE, BUILD, Cargo, or lock input changed.
The focused case passes and the full Bzlmod library is clean at 278/278 for the
first time in this Gate C sequence. Formatting, archive, exact four-line scope,
cap, credential-pattern, stable-lock, diff, process-cleanup, and independent
review gates pass. Next map only the private crate-mode Bzlmod target with its
declared Tokio dev edge; do not combine any other target or source repair.

The Bzlmod crate-mode target is accepted. One private target reuses the
production crate and adds only generated `normal_dev` aliases/deps for the
declared Tokio dev edge. Credential-free nightly Bazel and serial Cargo both
pass all 278 library cases, including the source-owned `current_exe()` child;
the GNU-Windows test binary compiles. No-repin module evaluation leaves all
three lock hashes stable, and archive, scope, +9-net/100-line cap,
credential-pattern, diff, process-cleanup, and independent review gates pass.
Together with the ten fixture-free integrations and lockfile target, all 464
Bzlmod cases now have green Bazel owners. Gate C1 has 38 accepted targets
covering 749 source cases and 745 default-active Linux Cargo cases. Next design
only the query library's clean-baseline `PreparationRestart` failure before
mapping its 28-case crate-mode target.

The query restart design proves production and the accepted one-route caller
contract are already exact. The private test epoch omits only Host
`Lstat(/workspace/dep/.bazelignore) = Missing`. External package lookup
correctly resolves the routed ignore file before loading the package, so that
absent observation becomes a typed Need; the direct environment correctly
turns it into its private restart sentinel, but this unit test has no outer
root-query retry owner. Normal integration epochs already supply the missing
observation. The fake same-`dep+` and synthetic `other+` callers still share
one real route and preserve the accepted Bazel 9.2 canonical-package
visibility discriminator.

`WP-10-m8-bazel-query-unit-ignore-observation-correction` adds only the missing
external `.bazelignore` path to the test's existing missing-Lstat list. It must
not change an expectation, production query/loading/Bzlmod code, a DICE key or
retry owner, route/materialization state, or any other fixture input. Focused
and full Cargo library tests must pass 28/28 before a later packet maps the
crate-mode Bazel target.

The query observation-fixture correction is accepted. One test-only path now
declares the routed dependency's absent `.bazelignore`; production Need/restart
ownership, DICE identity, one-route materialization, fake callers, and
visibility expectations remain unchanged. The focused case passes and the full
query library is clean at 28/28. Formatting, archive, exact one-line scope,
cap, credential-pattern, stable-lock, diff, process-cleanup, and independent
review gates pass. Next map only the private query crate-mode target with its
declared Tokio dev edge; keep the 53-case loading-query fixture target absent.

The query crate-mode target is accepted. One private target reuses the
production crate and adds only generated `normal_dev` aliases/deps for the
declared Tokio dev edge. Credential-free nightly Bazel and serial Cargo both
pass all 28 library cases; the GNU-Windows test binary compiles. No-repin
module evaluation leaves all three lock hashes stable, and archive, scope,
+9-net/100-line cap, credential-pattern, diff, process-cleanup, and independent
review gates pass. Gate C1 now has 39 accepted targets covering 777 source
cases and 773 default-active Linux Cargo cases. All low-risk targets and the
two bounded clean-baseline corrections are exhausted. Next redesign only the
shared nested-fixture owner for the 42 CLI integration, 53 query-loading, and
34 server-unit cases while the 141-case core unit target remains independently
blocked on pinned cross-platform Git/tar owners.

#### Gate C1 nested-fixture ownership redesign (2026-08-05)

`WP-10-m8-bazel-nested-fixture-ownership-redesign` reconciles the complete
shared boundary. The two CLI integration crates own 42 cases and reference all
14 workspaces/163 files; loading-query owns 53 cases but only seven functions
reference five workspaces/53 files; the server unit crate owns 34 inseparable
cases but only three functions reference four workspaces/37 files. The union
has 112 directories, 105 package directories, 107 `BUILD`/`BUILD.bazel` files,
and only regular non-executable files: every directory is mode 0755, every
file is mode 0644, every path is ASCII and Windows-safe, every content is valid
UTF-8, and there are no symlinks or case-folded path collisions. Consumers do
not mutate these sources; their writes are confined to separate temporary
workspaces. Extra package files or targets remain forbidden because recursive,
siblings, buildfiles/loadfiles, generated-file, and exact label-kind outputs
observe the complete workspace graph.

No Bazel 9.2 repository-rule owner satisfies the frozen no-follow incremental
contract. `watch_tree` computes a recursive directory digest by following a
symlink to a directory before Starlark can reject it. Replacing it with watched
`readdir` plus watched file reads also fails: the recorded directory marker
hashes sorted names but not entry types, and the file marker reduces both a
directory and a symlink-to-directory to `DIR`. Replacing an existing directory
with a same-shaped directory symlink can therefore leave every marker stable,
skip repository reevaluation, and bypass a `realpath` check. Adding `watch` on
each child does not close that discriminator.

The repository-rule proposal is terminal `REPLAN`, including its otherwise
valid length-framed byte transport and compile-time embedding consumer. Direct
source directories, package-local exports, runtime runfiles, ambient traversal
tools, and Windows exclusion remain rejected. The only unexhausted bounded
mechanism is the immutable checked-in snapshot explicitly reserved by Gate C0:
a deterministic cross-platform no-follow generator, ordered directory/file
manifest and hashes, mandatory source-to-snapshot drift check, create-new
scratch extraction, and compile-time declared-input consumer. That duplicate
is not accepted by this packet; it needs its own design and independent review
before any payload, helper, BUILD, Cargo, lock, or application change.

The next packet is `WP-10-m8-bazel-nested-fixture-snapshot-design`. It may only
freeze the generator/manifest/hash format, how drift enforcement cannot be
silently skipped, the 14/112/163 source inventory and no-follow lifecycle, the
compile-time embedding/extraction API, Windows and remote contracts, packet
split, and exact caps. If no bounded drift enforcement exists without Cargo
execution from Bazel, ambient tools, or CI assumptions, it must return terminal
`REPLAN` rather than weaken the fixture semantics.

The snapshot design is also terminal `REPLAN`. A checked-in payload is a
declared Bazel input, but Bazel cannot mandatorily prove its no-follow source
provenance under the frozen fixture graph. Recursive checking has the same
repository-watch hole; individual file labels miss additions and empty
directories; package-local globs/exports change the queried graphs; and a
manual Rust/Cargo check, `bazel sync`, Git-index check, or future CI rule is
skippable or ambient. The payload can therefore diverge silently in an ordinary
supported Bazel developer invocation.

Making the payload the sole canonical fixture representation would remove the
duplicate, but it is a different ownership migration: the Python oracle
harness currently `copytree`s each `fixture/workspace`, Cargo consumers use
those paths directly, and deleting 14 source trees changes the fixture corpus.
That possibility is neither accepted nor implemented here. The next packet is
`WP-10-m8-bazel-canonical-fixture-payload-migration-design`; it must audit every
Python/Rust/plan consumer, define the reversible provenance transition and
byte-exact fresh Bazel 9 replay, and either freeze one bounded canonical format
used by oracle, Cargo, and Bazel together or return terminal `REPLAN`.

#### Gate C1 canonical fixture payload migration (2026-08-05)

`WP-10-m8-bazel-canonical-fixture-payload-migration-design` establishes a sole
canonical payload candidate, but returns `REPLAN` before implementation. The fixed set
is the 14 workspaces already listed by Gate C0: 112 directories including the
roots, 163 regular files, 24,939 bytes, and 992 logical lines. Direct consumers
are exactly `app/slug_cli_v2/tests/{cli,graph_output}.rs`,
`app/slug_query_v2/tests/loading_query.rs`, and
`app/slug_server_v2/src/tests.rs`; they cover 42 CLI, 53 loading-query, and 34
inseparable server cases. The Python loader currently assigns
`fixture/workspace`, and the runner's `shutil.copytree` is the only live Python
read of those trees. The 14 TOMLs contain 403 commands matching 403 expected
rows, zero mutations, zero workspace-URI templates, and zero HTTP registries.
Twelve TOMLs retain 153 Bazel 9.2 provenance anchors; the two without a
provenance table must not receive fabricated upstream provenance.

The only canonical byte source after migration is
`tests/v2_fixture_payload/fixtures.payload`. Its frozen grammar is:

```text
slug-fixture-payload-v1\n
D\t0755\tPATH\n
F\t0644\tLEN\tSHA256\tPATH\n<exactly LEN raw bytes>\n
E\t112\t163\t24939\n
```

`PATH` is its literal 7-bit ASCII byte sequence and must match
`(?:[A-Za-z0-9._-]+/)*[A-Za-z0-9._-]+`; there is no escaping. Each component
must be nonempty, neither `.` nor `..`, must not end in dot, and must not be a
case-insensitive Windows device basename. Thus tab, CR, LF, NUL, space, colon,
backslash, non-ASCII, and every other byte outside that alphabet are rejected.
All 275 directory/file records are globally sorted by these path bytes, with
the fixture name retained as the first component; the fixture root itself is
its directory record. The parser also rejects noncanonical order, duplicates
and ASCII-case-fold collisions, absolute paths, unknown type/mode,
count/length/hash mismatch, and trailing bytes. File bodies are length-framed
raw bytes, so their UTF-8 validity is irrelevant. Directory records retain
empty directories; links and other file types have no encoding. Repeated bytes
at distinct paths are not deduplicated because paths are observable inputs.

Python and Rust use the same named conformance matrix. Canonical vectors cover
an empty directory, empty file, no-final-newline file, and a binary body with
`00 09 0a ff`; malformed vectors cover every header/footer/type/mode/count/
length/hash/trailing-data failure plus out-of-order, duplicate, case-collision,
absolute, dot, dot-dot, trailing-dot, device, backslash, tab, newline, NUL, and
non-ASCII paths. Each parser must accept/reject the exact same byte vectors.

The initial encoded payload is exactly 50,103 bytes/1,424 lines with SHA-256
`d4a5a0f05866908934725209649897fc7b3cf1dfc3f91aad2f5a9d7725bb5566`.
For a per-workspace projection, start with the same header, copy in full-payload
order only records whose path is exactly `NAME` or starts `NAME/` without
stripping that prefix, then append `E\tD_NAME\tF_NAME\tBYTES_NAME\n` using that
workspace's counts. SHA-256 covers that complete byte sequence. Implementations
must derive and verify all 14 projections from the full payload. Their hashes
are:

| Workspace | SHA-256 |
|-----------|---------|
| `simple-rule-action` | `3b8a1425ef7ea5b92de2f363465e5d52d92ce25c2b1818450bffc9098277f5fb` |
| `recursive-custom-rule-providers-actions` | `56584525959da70efe9fa64ef5acd862cb70fdf19ed5466d4c2d8f7a8d900c0f` |
| `build-file-loading` | `a54763ef1ff899547f4620bc2c3ec912d9c1cdca1d30714a2e43fcfc851f9cbf` |
| `query-parser-and-sets` | `0be99e30892443f9262e9618dd38c4a89522e107e0176f122a7a8cf4162542d6` |
| `tests-query-expansion` | `8b9ee022d4736bc58d3adc1adb67b6a1e6de5569950a475b6cc6c03cb70ffdee` |
| `query-visible-visibility` | `5bed82ad5b929c8d5f64dcfb2bb800ffdfa3fa13126ba22d02438bd5fe12cb9a` |
| `query-build-load-files-provenance` | `85a2e8fdedbe19e46f4b11a9e6e008d44b290d56c672775e018938eacaec9f7a` |
| `query-siblings-build-file-node` | `c2c102d891f2095f07878eae45fa0d3ad75bff269b32a213b7f4b2826d63b2b9` |
| `query-loading-thin-vertical` | `74e8d13fceff7c8868431a3e57653f70d7dea73bc3d203c7000090d66fceb330` |
| `query-labels-attribute-metadata` | `6ee33fce813b0ea9f286fea78dfde2a8e98389afdb01647c1e6d4892fed6ff5d` |
| `query-executables-rule-capability` | `7d320eb69086edf9ca85ca512d65b7259baabc7ac35fa7077c011536a57af227` |
| `query-rdeps-and-subtree-patterns` | `c4f5d3970fd6a3c8e04ebe277e12072311ef87dfccd372303d86dc1515260110` |
| `query-path-topology` | `50e86ad2c6528567aa9b106cd487e024f562b34020b84174a96e1012d24b52be` |
| `query-some-selection` | `9c0422b184f725508bd598d6b554f635a0f6ceeb507ac79c2bc59d2a3b1bc121` |

A repository-owned standard-library Python tool is the sole packer and the
Python parser/extractor. The one-time migration pack is POSIX-only: it walks
only the fixed roots, never descends a link, compares entry identity around
reads, uses directory-relative opens plus `O_NOFOLLOW`, and writes atomically.
Extraction validates the complete payload before writing, accepts only a
freshly create-new root, creates parents and files without reusing an existing
component, and applies exact 0755/0644 modes on POSIX plus ordinary directory/
regular-file semantics on Windows. The immutable payload cannot encode a link.
Hostile concurrent replacement of a newly created extraction component is not
claimed; native Windows adversarial reparse-race safety is explicitly outside
this developer-test helper. Before source deletion the POSIX migration lane
must reproduce every ordered path/type/mode/byte manifest and frozen hash.

Each selected TOML gains an explicit payload workspace selector and initial
tree hash; existing provenance tables remain byte-for-byte unchanged. Other
fixtures keep the directory-backed loader path. The runner extracts the
selected workspace, then applies the existing generic template expansion and
mutation lifecycle so this representation does not fork oracle semantics.
The README and validator describe and accept both representations.

The candidate Rust boundary uses one shared standard-library test-only source
module, not a production crate: the helper label belongs in each `rust_test`'s
`srcs`, the payload only in `compile_data`, and the payload must be embedded at
compile time under both Cargo and Bazel. The first correction proposed passing
`$(execpath ...fixtures.payload)` through `rustc_env` to `include_bytes!`, but
that path is execroot-relative while the macro resolves a relative argument
from the helper source file. It therefore does not locate the declared input
from the proposed layout. This is the second material contract correction, so
no Rust embedding mechanism, implementation cap, payload creation, consumer
switch, target activation, or source deletion is accepted by this design.

If compile-input evidence later permits replanning, the retained candidate
requires one exceptional atomic commit: any duplicate may exist only in the
uncommitted checkout while packing and extraction are proved; the same future
commit would add every owner/consumer/target, delete all 163 source files last,
and remain reversible with one `git revert`. These are unaccepted future
requirements, not current implementation authority.

The retained future validation candidate is the Python parser/extractor/oracle
and packet-validator suites; 42+53+34 Cargo cases; four Bazel targets;
GNU-Windows branch compilation; per-target compile/runfiles checks; and all
14/403 Bazel 9.2 commands in fresh and distinct replay roots against unchanged
expected JSON. It would also require exact archive/deletion/payload accounting,
stable locks, and independent destructive/platform latest-diff review. No part
of that validation currently authorizes payload, consumer, target, or deletion
work.

`WP-10-m8-bazel-fixture-payload-compile-input-evidence` resolves that REPLAN.
An isolated probe used the final sibling layout and the helper literal
`include_bytes!("../../v2_fixture_payload/fixtures.payload")`. Standalone Cargo
and credential-free Bazel 9.2/rules_rust 0.73 each ran the same one-case test.
The Bazel target put the helper label in `srcs` and payload in `compile_data`;
`aquery` listed consumer, helper, and payload on the Rust compile action, while
the runfiles tree and manifest contained neither helper nor payload. The
literal is resolved from the helper source under both build systems, needs no
cfg/env/absolute sandbox path, and has no platform branch. All temporary Rust,
BUILD, Cargo, lock, payload, and target files were removed; archive and Git
state returned clean.

The atomic implementation may now use that exact mechanism and the retained
format/migration requirements. `WP-10-m8-bazel-canonical-fixture-payload-
migration-implementation` owns the sole commit under the measured 300 Python,
380 Rust, 330 test, and 190 metadata/consumer/docs allocations: at most 1,200
handwritten additions, 38 non-generated touched files, 3,900 total changed
lines, and 1,650 final net lines. Any different compile path or ownership,
partial commit, weakened replay/platform contract, or cap overflow is REPLAN.

The first atomic implementation attempt stopped before deletion and left no
diff. Its preflight passed the migrated graph-output 5/5 and loading-query
55/55 suites, but reproduced three clean `9344ea0d` baselines: the CLI
broken-Bzl row still lacks Bazel's pinned `compilation of module ... failed`
fragment, while two server-only `loadfiles` scratch tests name absolute `.bzl`
labels in directories without BUILD package markers. Clean focused reruns
failed identically after the complete migration draft was removed.

The server failures are invalid test setup, not query behavior: add empty
`BUILD.bazel` files for `shared`, `root`, `leaf`, and `alternate` in one
four-line test-only packet, preserving all query/lifecycle assertions. Run
`WP-10-m8-bazel-server-loadfiles-package-fixture-correction` first. Then design
the separate Host-Bzl diagnostic parity repair around the pinned fixture and
`HostBzlModuleEvalKey`; only after both prerequisites are green may the atomic
payload migration restart unchanged.

The server fixture correction is accepted. Exactly four empty `BUILD.bazel`
markers make `shared`, `root`, `leaf`, and `alternate` real scratch packages;
the two focused tests and all 34 server library cases pass with every `.bzl`
byte, query, output, invalidation count, lifecycle, and assertion unchanged.
Independent review accepted the four-line test-only diff. Next run only
`WP-10-m8-host-bzl-parse-diagnostic-parity-design`; no payload migration work
resumes until that separate Bazel-authoritative diagnostic is green.

The Host-Bzl diagnostic design accepts one private presentation repair.
`HostBzlModuleEvalKey` already produces a typed `HostBzlModuleError::Parse`;
root-package loading preserves it under the raw load context, and query/CLI/
daemon layers serialize that error unchanged. Only its display arm omits the
legacy loader's pinned `compilation of module 'pkg/file.bzl' failed` summary.
The label already retains validated package and direct-file target fields, so
the logical slash path needs no parsing, filesystem access, new carrier, key,
event, equality, or wrapper.

`WP-10-m8-host-bzl-parse-diagnostic-parity-implementation` may add the logical
path formatter and exact summary in `bzl_module.rs`, then tighten the existing
malformed-`a.bzl` Host lifecycle assertion. A direct private display assertion
must also prove that root-package `:a.bzl` renders `a.bzl`, not `/a.bzl`. The
accepted CLI/oracle row remains unchanged and supplies the public regression.
Caps are 20 production/12 test/92 total net lines; external, BUILD, missing,
cycle, evaluation, freeze, query, CLI, fixture, and DICE behavior remain frozen.

The implementation adds only a validated `package/target` path formatter and
the missing Parse display suffix, plus root and non-root assertions. Both
focused cases, all 58 loading tests, all 53 loading-query tests, all 34 server
tests, and the loading/query GNU-Windows check pass. A direct CLI replay now
matches the accepted broken-Bzl exit and summary. The unchanged 57-row test
continues past that repaired row and then reproduces its clean pre-existing
`bzl_cycle_failure` unavailable-root-DICE-node terminal; the full CLI suite is
therefore 38/39. The user explicitly deferred cyclic dependencies, and no cycle
branch or assertion changed.

Because the implementation packet falsely required that whole CLI target to
be green, independent review required a second material contract correction;
its terminal scheduling outcome is `REPLAN`, while the exact two-file Host-Bzl
repair remains an accepted prerequisite. Next design only
`WP-10-m8-bazel-canonical-fixture-payload-cycle-baseline-design`: preserve the
cycle row and oracle unchanged, and freeze an honest before/after negative gate
for the atomic migration before any payload, consumer, target, or deletion work.

That design is accepted without a wrapper or test change. The 57-row Rust test
runs all 49 successes, then missing and repaired broken-Bzl failures, and stops
at the user-deferred cycle mismatch before five later failure rows. The existing
oracle runner supplies the complementary payload-sensitive gate because it runs
all 64 fixture commands independently. Two distinct directory-backed Slug
replays each produced exactly one comparison failure, `bzl_cycle_failure`, and
all other commands passed. Only numeric engine/node fields varied; after
canonicalizing the complete `DiceNodeId` fragment to
`DiceNodeId { engine: <id>, node: <id> }`, the ordered name/argv/exit/normalized
stdout/stderr/manifest/mutations records serialized by compact sorted-key
Python JSON with no trailing newline have SHA-256
`eb217429572083716f41e133cb68c67e8ee3237d2524c65c21d7a5f472709cb4`.

The atomic migration is rescheduled unchanged except for truthful target
acceptance. Three Bazel targets must pass. The CLI target must build, then fail
only as 38/39 source cases (40/41 including helper conformance tests), paired
with the exact 64-command single-mismatch gate; Bazel exit 3 alone is never
accepted. No wrapper, fifth target, skip, ignore, filter, split, manual tag,
cycle/oracle/assertion change, new path, or cap increase is authorized.

The atomic migration is accepted. One 50,103-byte canonical payload owns all
14 workspaces, its Python/Rust consumers verify the frozen hashes before fresh
extraction, and the old 163 files are deleted in the same reversible commit.
Cargo/Bazel graph, loading-query, and server targets pass; CLI is exactly 40/41
at only the unchanged cycle row. Two absolute Bazel 9.2 roots each pass 14
fixtures/403 commands against unchanged evidence. Windows compile, declared
inputs, runfiles, reconstruction, locks, archive, caps, cleanup, and independent
review pass. Gate C1 has 43 targets/906 cases; core unit remains blocked on
declared cross-platform Git/tar owners. Next design only
`WP-10-m8-bazel-buildbuddy-developer-gate-design` before cache/RBE work.

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

#### Developer-gate design stop (2026-08-05)

`WP-10-m8-bazel-buildbuddy-developer-gate-design` returns `REPLAN` before any
configuration or remote invocation. The live root has no tracked or untracked
`.bazelrc`, named BuildBuddy profile, remote cache/executor/BES endpoint,
instance name, or remote execution platform. The sixteen tracked `.bazelrc`
files are isolated oracle-fixture inputs and define no reusable developer
profile. Home authentication is intentionally unavailable to the audit.
Consequently the repository does not establish whether the intended service is
BuildBuddy Cloud or an organization/self-hosted deployment, whether cache and
executor endpoints coincide, or which executor OS/CPU/container is compatible
with the pinned toolchain. Inventing any of those values would violate the
fail-closed packet contract.

The target boundary itself is closed and is not the blocker. The checkout has
44 live `rust_test` targets. Gate C1 contributes 43 targets and 906 source
cases: 42 green targets plus the expected-red CLI fixture target. The previously
accepted Gate C0 CLI unit target is the forty-third green target, so the full
remote boundary is exactly 43 green targets plus one separate expected-red
`//app/slug_cli_v2:cli_fixture_test`. That target must stay 40/41 at only
`bzl_cycle_failure`, paired with the independent 14-fixture/403-command
single-mismatch oracle. The blocked 141-case core unit target is absent from
both sets.

Pinned Bazel 9.2 source does close the later evidence vocabulary:

- The pinned
  [`SpawnExec`](https://github.com/bazelbuild/bazel/blob/8220c6198837d5c13d53fea211cf3282aa12408a/src/main/protobuf/spawn.proto#L100-L192)
  `runner` distinguishes `remote` from `remote cache hit`, while
  `cache_hit`, `status`, `exit_code`, `remotable`, `remote_cacheable`,
  `target_label`, `mnemonic`, and the action digest supply the per-spawn
  discriminator. Persistent local action-cache hits are not logged, so prime
  and replay must use distinct fresh output bases.
- The pinned
  [BEP schema](https://github.com/bazelbuild/bazel/blob/8220c6198837d5c13d53fea211cf3282aa12408a/src/main/java/com/google/devtools/build/lib/buildeventstream/proto/build_event_stream.proto#L819-L900)
  supplies `BuildFinished`, target/test completion events, and
  `BuildMetrics.ActionSummary.runner_count` corroborate exit, target coverage,
  and aggregate runner counts. They do not replace per-spawn evidence.
- A cache-only candidate would prime locally with cache reads disabled and
  synchronous uploads, then replay from a second output base with remote reads
  enabled. An RBE candidate would use a third output base, remote reads
  disabled, remote strategy forced, and local fallback disabled. The exact
  commands remain unaccepted until the repository profiles and platform are
  selected.
- Raw BEP and execution logs are sensitive intermediates: BEP carries parsed
  command lines, options, workspace/host/user data, and file URIs, while the
  execution log carries action arguments and environments. A later driver must
  create them under a mode-0700 temporary directory outside the checkout,
  never echo or upload them as review evidence, extract only a closed
  allowlisted record, and delete them on every exit. Profiles, elapsed text,
  invocation URLs, effective RC expansion, BuildBuddy UI exports, and terminal
  process totals are not acceptance evidence.

The eventual classifier must fail closed. `PROVED_CACHE_ONLY` requires a
successful fresh/replay pair with matching selected action digests, remote
cache hits on replay, and no remote executor runner. `PROVED_RBE` requires
successful selected spawns with runner `remote`, empty status, zero exit, no
cache hit on the forced-fresh run, and no local fallback. Bazel remote exit
codes may classify `REMOTE_UNAVAILABLE`; ordinary analysis/build/test failures
remain `TARGET_FAILURE`. Missing terminal events, unknown runners, mixed
fallback, absent selected actions, or incomplete raw logs are
`EVIDENCE_INCOMPLETE`, never success.

The smallest successor is
`WP-10-m8-bazel-buildbuddy-repository-config-decision`. It is a docs-only,
explicitly user-reviewed choice of:

1. opt-in cache-only and execution-enabled profile names and whether either may
   become a default;
2. exact non-secret BES/cache/executor endpoints and optional instance name;
3. the exact supported RBE OS/CPU and required platform/container properties,
   including an immutable image identifier when applicable; and
4. the 43-green/one-expected-red command matrix plus private raw-artifact and
   sanitized-record locations.

The user must either approve the hosted BuildBuddy connection/platform values
from official service documentation or supply a sanitized organization/on-prem
connection snippet. The successor adds no `.bazelrc`, CI, remote invocation,
evidence, code, lock, or credential. After that decision, implementation splits
serially into cache-only repository configuration and live evidence first, then
RBE configuration and evidence with an independently reviewed platform map.
Only those implementation designs may set file allowlists and line caps; CI and
self-hosting remain later work.

#### Repository-safe BuildBuddy Cloud decision (2026-08-06)

`WP-10-m8-bazel-buildbuddy-repository-config-decision` is accepted by explicit
user approval. The repository targets hosted BuildBuddy Cloud. Authentication
remains only in the user's `~/.bazelrc`; the root configuration must contain no
header, token, certificate, credential helper, import of a home path, or derived
credential value.

The five approved service options are exact:

```text
build --bes_results_url=https://app.buildbuddy.io/invocation/
build --bes_backend=grpcs://remote.buildbuddy.io
common --remote_cache=grpcs://remote.buildbuddy.io
common --remote_timeout=10m
common --remote_executor=grpcs://remote.buildbuddy.io
```

Remote cache is the ordinary developer default, but remote execution is
opt-in. The root file therefore also sets
`common --spawn_strategy=worker,sandboxed,local`; without that line Bazel 9.2's
default `remote,worker,sandboxed,local` order would make the common executor
remote-capable on ordinary builds. Named `build:` profiles are
`buildbuddy-cache` and `buildbuddy-rbe`; `test` inherits them through the build
command hierarchy. The cache profile clears the common executor with
`--remote_executor=` and waits for cache uploads with
`--remote_cache_async=false`. The RBE profile forces
`--spawn_strategy=remote`, disables local fallback, and selects the managed
BuildBuddy pool with `OSFamily=linux`, `Arch=amd64`, and
`use-self-hosted-executors=false` remote default execution properties.

No remote instance name or custom container image is selected. Linux/amd64 is
the initial RBE platform; other developer hosts use cache-only mode until an
independently reviewed compatible executor exists. The 43-green/one-expected-
red command boundary, blocked core unit, deferred cycle, private raw-log rules,
and fail-closed evidence classifier remain unchanged.

The next packet is
`WP-10-m8-bazel-buildbuddy-repository-config-implementation`. It may add only
the 13 non-comment option lines above to root `.bazelrc` and update the owner
plan plus scheduling documents: at most 13 configuration lines, 80 authored
documentation lines, four files, and 150 total changed lines. It validates the
two profiles with Bazel 9.2 while explicitly disabling system, workspace, and
home RC discovery and overriding every remote/BES endpoint to empty after the
selected profile. It must not inspect effective home options, contact a remote
service, add evidence/CI/code/BUILD/MODULE/lock changes, or weaken credential,
target, cycle, core, and platform boundaries. Live cache evidence and live RBE
evidence remain separate successor packets in that order.

The repository-config implementation is accepted. Root `.bazelrc` contains
exactly the five user-approved BuildBuddy Cloud options plus the eight accepted
mode options: ordinary builds use remote cache with local
`worker,sandboxed,local` execution, `buildbuddy-cache` explicitly clears the
executor and waits for uploads, and `buildbuddy-rbe` forces remote-only managed
Linux/amd64 execution with no fallback. There is no header, import, instance,
custom image, credential, or unrelated option.

Bazel 9.2 loaded only this explicit root RC with system, workspace, and home RC
discovery disabled. Both named profiles parsed and analyzed
`//app/slug_cli_v2:slug` with `--nobuild`, zero actions, and final empty
cache/executor/BES endpoints; the reported expansion contained only the root
file and showed the cache executor clearing plus the exact RBE strategy and
three scheduler properties. No remote service or home authentication was
accessed. Archive, exact-line, credential-pattern, scope, cap, and diff checks
pass.

Next design only `WP-10-m8-bazel-buildbuddy-cache-evidence-design`. It must
freeze the smallest secret-safe prime/replay driver, exact 43-green target
manifest, disposable raw BEP/execution-log lifecycle, allowlisted sanitized
record, cache-only discriminators, failure classes, review split, and caps
before any authenticated invocation. RBE configuration remains present but no
RBE claim or command enters that packet.

#### BuildBuddy cache-evidence design (2026-08-06)

The design is accepted without an authenticated invocation. The canonical
manifest is `tests/v2_oracle/buildbuddy_cache_targets.txt`. Its first line is
the literal `slug-buildbuddy-targets-v1`; every remaining line is
`kind<TAB>canonical-label`, with exactly
`build<TAB>//app/slug_cli_v2:slug` followed by the 43 green `rust_test` labels
sorted bytewise and prefixed by `test<TAB>`. Its exact 45-line, final-newline SHA-256 is
`3a717cb4b0a1f5cab06d336e69d2382861a9c21af9a1502ea20c54b990adf6d5`.
It includes the accepted 13-case `//app/slug_core_v2:runtime_test`; the absent,
blocked 141-case core crate-mode unit is not that target. The only live target
excluded is the separately accepted expected-red
`//app/slug_cli_v2:cli_fixture_test` cycle gate.

One stdlib-only Python driver runs one `bazel test` command per phase over the
build label and all 43 test labels. Prime and replay use distinct never-before-
existing output bases beneath one mode-0700 temporary directory outside the
checkout, the same unprinted random action/test-environment nonce, Bazel 9.2.0,
a clean Linux x86_64 checkout, the exact manifest hash, and root `.bazelrc`
SHA-256 `e72f4223b6cfffbc96de018849e306ff9cbfdf4ca50248d8fee229a80dc4c805`.
The command tail disables BES and RBE, clears disk cache, forces local
strategies, fixes one unsharded run per test, emits local BEP JSON plus an
execution-log JSON sequence, and publishes all action events. Prime disables
remote reads, enables local-result upload, and waits synchronously; replay
enables reads and disables upload. Both disable local fallback. No elapsed-time
or terminal-text inference is evidence.

After ordinary RC discovery and `--config=buildbuddy-cache`, both invocations
append these exact command options before the manifest labels:

```text
--remote_cache=grpcs://remote.buildbuddy.io
--remote_instance_name=
--remote_executor=
--bes_backend=
--bes_results_url=
--disk_cache=
--spawn_strategy=worker,sandboxed,local
--test_strategy=local
--cache_test_results=yes
--runs_per_test=1
--test_sharding_strategy=disabled
--noremote_local_fallback
--build_event_publish_all_actions
--build_event_json_file=<private-phase-path>
--execution_log_json_file=<private-phase-path>
--action_env=SLUG_BUILDBUDDY_CACHE_GATE_NONCE=<shared-nonce>
--test_env=SLUG_BUILDBUDDY_CACHE_GATE_NONCE=<shared-nonce>
```

Prime then appends `--noremote_accept_cached`,
`--remote_upload_local_results`, and `--noremote_cache_async`; replay appends
`--remote_accept_cached`, `--noremote_upload_local_results`, and
`--noremote_cache_async`. The only accepted prime runner spellings are
`local`, `worker`, and `linux-sandbox`; every other spelling is unknown and
fail-closed. Command construction and error handling must never render the
shared nonce or the full argv.

The driver sets umask 077 before creating raw BEP, execution-log, and captured
stdout/stderr files. Raw data can contain RC-expanded headers, command lines,
environment, paths, host/user data, URIs, invocation IDs, and action inputs, so
it never reaches stdout, an exception, a repository path, or the sanitized
record. A streaming JSON decoder accepts pretty-printed JSON sequences. A
`finally` path shuts down both private Bazel servers with all RC files ignored
and recursively removes the private directory; failure to parse, sanitize,
shutdown, or clean is fail-closed. Abrupt process death remains the documented
mode-0700 temporary-file residual, not accepted evidence.

The driver emits one compact JSON object built from a new allowlisted value,
never by redacting raw objects. Its closed top-level fields are
`schema_version`, `classification`, `mode`, `bazel_version`, `host_platform`,
`git_head`, `git_clean`, `manifest_sha256`, `bazelrc_sha256`, `target_counts`,
`prime`, and `replay`. Each phase contains only the Bazel process exit code;
BuildFinished exit-code name/code; selected build-success, passed-test,
test-run, and remotely-cached-test counts; local persistent-action-cache hit
count; and an eligible-spawn summary. That summary contains count, a canonical
digest-multiset SHA-256, cache-hit/status/exit failure counts, and counts for
`local`, `remote_cache_hit`, `disk_cache_hit`, `remote_execution`, and
`unknown`. Labels are represented only by manifest index while parsing and are
not emitted. Arguments, environment, mnemonic, raw runner spelling, individual
digests, paths, timing, host/user names, UUIDs, endpoints, headers, options,
commands, and free-form status/error text are forbidden output fields.
Stdout contains only that compact object plus one newline, stderr is empty,
and every exception is converted to an allowlisted failure object without a
traceback or raw exception text; a caller may redirect only stdout to a
review-controlled sanitized-record path.

`PROVED_CACHE_ONLY` requires both phases and every selected target to complete
successfully; all 43 tests pass exactly once; prime has zero persistent local
action-cache hits and zero remotely cached tests; and replay reports all 43
tests remotely cached. Every prime spawn that is both cacheable and remotely
cacheable has a digest, runner exactly `local`, `worker`, or `linux-sandbox`,
no cache hit, empty status, and exit zero; every other prime runner maps to
`unknown`. Replay has the identical eligible digest multiset and every
eligible entry is exactly `runner="remote cache hit"`, `cache_hit=true`, empty
status, and exit zero. No eligible replay entry may be local, disk-cache,
remote-execution, or unknown. Non-cacheable or non-remotely-cacheable spawns may
run locally but are counted outside the claim. Fresh output bases close Bazel
9.2's omission of persistent action-cache hits from SpawnExec; BEP target/test
events corroborate completion, while SpawnExec is authoritative for cache
reuse.

Failures classify without raw detail as `CONFIG_DRIFT`, `REMOTE_UNAVAILABLE`,
`TARGET_FAILURE`, `CACHE_MISS_OR_MIXED_REPLAY`, `EVIDENCE_INCOMPLETE`, or
`SANITIZER_REJECTED`. Unknown runners, malformed or missing terminal data,
mixed/local eligible replay, digest mismatch, any persistent-action-cache hit,
or incomplete cleanup can never prove the gate. The evidence covers only
Linux x86_64 cache reuse; RBE, other platforms, CI, and the cycle/core
boundaries remain separate.

Next implementation only
`WP-10-m8-bazel-buildbuddy-cache-evidence-implementation`. It may add
`tools/v2_oracle_lib/buildbuddy_cache.py` (at most 480 lines),
`tools/v2_oracle/buildbuddy_cache_gate.py` (40), the 45-line manifest, and
`tests/v2_oracle/test_buildbuddy_cache_gate.py` (520), plus at most 120 owner
and scheduling lines: seven files and 1,300 changed lines total. Tests use only
synthetic raw bytes and mocked subprocesses and cover manifest/config/platform
drift, JSON-sequence parsing, every admitted prime runner plus near-miss unknown
runners, every failure class, target/test coverage,
digest multiset mismatch, output allowlisting, command hardening, distinct
output bases with one shared nonce, and raw cleanup on every exit. The packet
may run offline tests but must not invoke BuildBuddy or consume authentication.
A later evidence-only packet runs the frozen driver once; any live contract
defect returns `REPLAN` rather than changing code beside authenticated evidence.

The cache-evidence implementation is accepted. The exact 45-line manifest
selects `slug` plus all 43 green tests, including the 13-case core runtime test,
and has the frozen `3a717c…f6d5` SHA-256. The stdlib driver binds the approved
BuildBuddy cache endpoint and empty instance after RC expansion, clears BES,
executor, and disk cache, uses one private fresh output base per phase, and
records only the closed sanitized schema. Bazel/version, root RC hash, clean
Git head, Linux x86_64, manifest, per-target completion/run/cache counts,
persistent action-cache hits, eligible action digests, exact runners, and
cleanup all fail closed.

Twenty-two synthetic/mocked tests cover both command phases, every admitted
prime runner and near miss, JSON sequences and scalar types, per-label 0/2
run/cache masking, digest multiplicity/mismatch, target/BEP failures, remote
abort, configuration/platform drift, private file modes, outside-checkout
roots, raw-output suppression, both shutdown failures, recursive cleanup, and
the sanitized key allowlist. Unit tests, Python compilation, the exact manifest
hash, archive, caps, diff, a dirty-tree no-network `CONFIG_DRIFT` CLI smoke, and
independent review pass. No authenticated command or home RC read occurred.

Next evidence only `WP-10-m8-bazel-buildbuddy-cache-live-evidence`. From the
clean implementation commit it runs the frozen driver once with ordinary RC
discovery, permitting Bazel alone to consume home-owned authentication. Review
only the compact stdout object. Success requires `PROVED_CACHE_ONLY`; any other
classification, unexpected stderr, retained private path, or required code/
configuration repair returns `REPLAN`. The packet may update only owner and
scheduling documentation with the sanitized result: three files and 100 lines.
It must not inspect home configuration, retain raw artifacts, rerun to chase a
failure, add code/config/CI, invoke RBE, or change targets, cycle/core, or
platform boundaries.

The first live-evidence packet returns `REPLAN` before temporary-root creation
or any remote/authenticated phase. Its sole sanitized result was
`CONFIG_DRIFT`. Offline reproduction proved the local defect: with Bazelisk,
`bazel --ignore_all_rc_files --version` treats `--version` as an unknown startup
option and exits 2, while `bazel --ignore_all_rc_files version` succeeds without
RC discovery and reports one `Build label: 9.2.0` line. Manifest/root-RC hashes,
Git cleanliness, and Linux x86_64 remained exact. No BuildBuddy request or raw
artifact occurred, and the packet is not rerun.

Next repair only
`WP-10-m8-bazel-buildbuddy-cache-evidence-preflight-version-repair`. It changes
only `tools/v2_oracle_lib/buildbuddy_cache.py` and
`tests/v2_oracle/test_buildbuddy_cache_gate.py`, at most 35 changed lines. The
preflight argv becomes exactly `bazel --ignore_all_rc_files version`; parsing
accepts exactly one `Build label: 9.2.0` line within Bazelisk's multiline output
and rejects a missing, duplicate, or different build label. Tests pin the argv
and all four outputs. No Bazel test/build, ordinary RC discovery, authentication,
remote call, driver/schema/classifier/config/manifest, or documentation change
is authorized. After offline review, a new evidence packet—not this failed
packet—may make one fresh live attempt.

The preflight repair is accepted in its exact two-file/30-line scope. The
driver now runs the RC-disabled `version` command and accepts exactly one
`Build label: 9.2.0` line from Bazelisk's multiline output. Wrong, missing,
duplicate, undecodable, and failed version output remain `CONFIG_DRIFT`.
Twenty-two offline tests, Python compilation, exact live RC-disabled version
output, caps, diff, and independent review pass; no other driver behavior and
no remote or home-RC boundary changed.

Next evidence only `WP-10-m8-bazel-buildbuddy-cache-live-evidence-retry`. From
the clean repair commit it makes one new driver invocation under the previously
frozen live-evidence contract. This is not a continuation or rerun of the
preflight-stopped packet: it is a fresh reviewed packet after the committed
repair. Accept only `PROVED_CACHE_ONLY`; otherwise return `REPLAN` without a
second attempt or code/config repair. Only a successful sanitized record may
update owner and scheduling documentation, at most 100 lines in three files.

The fresh live-evidence retry returns `REPLAN`. Its sanitized record binds clean
head `bfa95056…`, Bazel 9.2.0, Linux x86_64, and the exact manifest/root-RC
hashes, but both phases finish with process/BuildFinished exit 2, no completed
target or test, no action-cache hit, and zero eligible spawns; classification is
`TARGET_FAILURE`. Stderr remained empty and current private-root cleanup passed.
Three older mode-0700 directories left by synthetic development runs before the
cleanup tests were finalized were resolved by exact path and permanently
deleted without reading their contents; no matching directory remains.

An RC-disabled Bazel 9.2 help audit recognizes every frozen flag. Two
credential-free `canonicalize-flags --for_command=test` probes also accept the
final option tail and the explicit root-only `buildbuddy-cache` profile when
labels are omitted. They perform no build/test or remote request. Thus no
checked-in argv defect is demonstrated. Exit 2 with zero events points to the
effective normal-RC environment, but the sanitizer intentionally maps unknown
exit names to `OTHER`, and inspecting or expanding home RC would violate the
credential boundary. No third live attempt is authorized.

Next decision only `WP-10-m8-bazel-buildbuddy-cache-home-auth-rc-decision`.
The user must privately reduce or confirm `~/.bazelrc` to the authentication
option needed by the checked-in configuration, preferably
`common --remote_header=x-buildbuddy-api-key=<secret>` (a `build` scope also
reaches `test`), with no stale endpoint, profile, instance, executor, strategy,
or unsupported option. The secret line/value must not be pasted, inspected,
logged, or committed; only a token-free confirmation is recorded. No agent
command, repository change, or live retry occurs before that confirmation.
After it, a separately reviewed packet may authorize one fresh attempt.

The home-auth decision is accepted by the user's explicit token-free
confirmation. No home path, line, token, derived value, or effective option was
inspected or recorded. The confirmation closes only the prior normal-RC drift
boundary; it does not itself prove connectivity or cache behavior.

Next evidence only
`WP-10-m8-bazel-buildbuddy-cache-live-evidence-after-home-auth`. From the clean
decision commit it runs the frozen driver exactly once. This is a fresh packet
after the user-owned external-state change, not a retry inside either failed
packet. Accept only the compact `PROVED_CACHE_ONLY` record with exit 0 and empty
stderr. Any other classification, retained private path, or required repair is
`REPLAN` with no second attempt. Only owner and scheduling documentation may
record a successful sanitized result, at most 100 lines in three files; code,
configuration, raw artifacts, RBE, CI, and target/platform boundaries remain
unchanged.

The post-confirmation evidence packet returns `REPLAN`. Its compact record
binds clean head `2c3370dc…`, Bazel 9.2.0, Linux x86_64, and the frozen manifest
and root-RC hashes. Both prime and replay terminate with process and
BuildFinished exit 2/name `OTHER`, zero completed builds/tests, zero eligible
spawns, and zero cache hits; classification remains `TARGET_FAILURE`. Stderr is
empty, private-root cleanup passes, and no second attempt occurred. Repeating
the same opaque live invocation cannot distinguish a command-line failure from
a target failure.

Next implementation only
`WP-10-m8-bazel-buildbuddy-command-failure-diagnostic-implementation`. Bazel
9.2 source commit `8220c619…` establishes `BuildFinished.failureDetail` as the
structured failure owner and marks the relevant command/options, remote
configuration, execution configuration, and build-configuration enum codes as
exit 2. Extend the sanitizer from that structured object only: admit the fixed
BuildFinished name `COMMAND_LINE_ERROR`, emit a fixed per-phase
`command_failure_class`, and classify matching exit-2 phases as
`COMMAND_LINE_FAILURE` ahead of ordinary target failure. The closed classes
are `NONE`, `COMMAND_OPTIONS_PARSE`, `COMMAND_STARLARK_OPTIONS_PARSE`,
`COMMAND_ARGUMENTS_NOT_RECOGNIZED`, `COMMAND_INVOCATION_POLICY`,
`REMOTE_OPTIONS_CONFIGURATION`, `REMOTE_EXECUTION_CONFIGURATION`,
`EXECUTION_OPTIONS_CONFIGURATION`, `EXECUTION_LOG_CONFIGURATION`,
`BUILD_CONFIGURATION`, and `UNKNOWN_COMMAND_LINE_ERROR`. Each non-unknown
class requires one exact category/code pair from an explicit allowlist;
missing, multiple, malformed, or unrecognized data fails closed to the unknown
class.

The implementation may change only
`tools/v2_oracle_lib/buildbuddy_cache.py` (at most 90 changed lines) and
`tests/v2_oracle/test_buildbuddy_cache_gate.py` (at most 180 changed lines).
Tests must cover every admitted pair and malformed/unknown input, prove that
free-form `failureDetail.message`, stderr, credentials, nonces, paths, and
arbitrary enums cannot enter the compact record, and preserve all existing
success/remote/target/cache behavior. No raw output may be parsed or read for
diagnosis. The packet is offline only: no Bazel build/test, ordinary RC
discovery, home inspection, remote call, configuration/manifest change, or
live retry. After offline review, a separate evidence packet may make exactly
one fresh diagnostic invocation.

The structured diagnostic implementation is accepted in its exact two-file,
74-changed-line scope. It allowlists all 33 pinned Bazel 9.2 exit-2 pairs,
admits only fixed `COMMAND_LINE_ERROR`, emits only the closed per-phase class,
and gives `COMMAND_LINE_FAILURE` precedence over ordinary target failure while
preserving remote precedence. Unknown or malformed detail fails closed without
retaining any raw key or value. Twenty-four offline tests, Python compilation,
caps, diff, and independent review pass; no Bazel, home-RC, or remote command
ran.

Next evidence only
`WP-10-m8-bazel-buildbuddy-command-failure-diagnostic-evidence`. From clean
implementation commit `b66c0bc3…`, run the frozen driver exactly once with
ordinary RC discovery and let only Bazel consume home-owned authentication.
Review only the compact stdout object, process status, and empty stderr.
`PROVED_CACHE_ONLY` closes the cache proof. `COMMAND_LINE_FAILURE` is accepted
only as a secret-safe diagnosis and must report the fixed class for each phase;
it does not prove cache behavior and returns `REPLAN` for a later bounded
repair/decision. Any other classification, schema surprise, retained private
path, or required repair also returns `REPLAN`. Do not make a second attempt,
inspect home configuration or raw artifacts, change code/config/CI/targets, or
invoke RBE. Only owner and scheduling documentation may record the sanitized
result, at most 100 changed lines in three files.

The structured evidence packet returns `REPLAN` after its single authorized
invocation. Its compact record binds clean head `0a8f9730…`, Bazel 9.2.0,
Linux x86_64, and the frozen manifest/root-RC hashes. Prime and replay both
report process and BuildFinished exit 2/name `COMMAND_LINE_ERROR`, fixed class
`UNKNOWN_COMMAND_LINE_ERROR`, zero completed targets/tests, zero eligible
spawns, and zero cache hits. The driver exits 1 with no stderr; private-root,
stale-daemon, and Git cleanup checks pass. No raw artifact or home RC was read,
and no second attempt occurred. This proves a pre-target command-line boundary
but neither identifies its structured category nor proves cache behavior.

Next design only
`WP-10-m8-bazel-buildbuddy-unknown-command-diagnostic-design`. Audit pinned
Bazel 9.2 `FailureDetail` exit-2 categories and the current fail-closed parser
to choose the smallest fixed classification expansion that can distinguish the
unknown result without exposing the human message, raw category/code/enum,
paths, options, credentials, nonces, or stderr. Freeze exact allowlists,
malformed-input behavior, tests, file/line caps, and a separately reviewed live
evidence boundary. The packet may change only owner and scheduling
documentation, at most 120 lines in three files. It must not change code or
configuration, inspect home/raw data, run Bazel, contact BuildBuddy, or
authorize another live attempt.

The unknown-command diagnostic design is accepted. Pinned Bazel 9.2 source
contains 131 exit-2 category/code pairs across 33 of the 64
`FailureDetail.oneof category` fields; the first sanitizer mapped only 33.
Category-only output is insufficient because even the omitted `command` pairs
distinguish command discovery, workspace context, and output-tree context.

Next implementation only
`WP-10-m8-bazel-buildbuddy-complete-command-diagnostic-implementation`. Add an
internal ordered `B92_EXIT2_SOURCE_PAIRS` table for all 131 pinned pairs. Its
canonical bytes start with
`slug-bazel-9.2-failure-detail-exit2-v1\n`, followed by one
`lowerCamelCategory\tENUM_IDENTIFIER\n` line per pair in oneof-field order and
then enum source-declaration order. The exact SHA-256 is
`cbc5777ca02212ba3a5d20847c469eb221bd29b3c217162e6be39c5f5bf86d57`.
The existing 33 pairs retain semantic classes; each remaining pair emits only
its unique fixed source ordinal `B92_EXIT2_CLASS_NNN`, never a copied key/code.

Also distinguish fixed structural results `MISSING_FAILURE_DETAIL`,
`MALFORMED_FAILURE_DETAIL`, `UNSUPPORTED_GENERAL_FAILURE_DETAIL`, and
`UNRECOGNIZED_B9_2_EXIT2_DETAIL` using the exhaustive 64-key oneof set.
Optional string `message` is ignored; require one known category and exactly a
string `code`. Extra/multiple/unknown general or category data fails closed and
no raw value is emitted. Tests pin the 131 count/hash, 98 unique opaque IDs,
unchanged semantic classes, every pair, all structural cases, malicious
message/key/value/path/header/nonce/stderr suppression, no stderr read, and all
existing classification/lifecycle behavior. Change only
`tools/v2_oracle_lib/buildbuddy_cache.py` (150 changed lines) and
`tests/v2_oracle/test_buildbuddy_cache_gate.py` (180), at most 330 total.
Offline tests, compilation, caps, diff, and independent privacy/schema review
are required. Do not run Bazel, inspect home/raw data, contact BuildBuddy, or
make a live attempt; a later separately frozen evidence packet owns one run.

The complete command diagnostic is accepted in its exact two-file,
151-changed-line scope. Its internal table reproduces the frozen 131-pair
canonical hash and exhaustive 64-key set; all 98 new opaque ordinals are
unique, the original 33 semantic classes are unchanged, and fixed structural
outcomes distinguish missing, malformed, unsupported-general, and unrecognized
detail. Twenty-four offline tests, Python compilation, caps, diff, and an
independent pinned-source privacy/schema reconstruction pass. The diagnostic
never reads private stderr, and no Bazel, home-RC, or remote command ran.

Next evidence only
`WP-10-m8-bazel-buildbuddy-complete-command-diagnostic-evidence`. From clean
implementation commit `fcc754a2…`, run the unchanged frozen driver exactly
once with ordinary RC discovery and let only Bazel consume home-owned
authentication. Review only compact stdout, process status, and empty stderr.
`PROVED_CACHE_ONLY` closes the cache proof. A `COMMAND_LINE_FAILURE` is accepted
only as a diagnosis when prime and replay report the same fixed non-`NONE`
class; record that class and return `REPLAN` for the corresponding bounded
repair/decision. Any other classification, differing class, schema surprise,
retained private path, or required repair is also `REPLAN`. Do not retry,
inspect home/raw data, change code/config/CI/targets, or invoke RBE. Only owner
and scheduling docs may record the sanitized result, at most 100 changed lines
in three files.

The complete diagnostic evidence packet returns `REPLAN` after exactly one
invocation. Its compact record binds clean head `7f573984…`, Bazel 9.2.0,
Linux x86_64, and the frozen hashes. Both phases report process and
BuildFinished exit 2/name `COMMAND_LINE_ERROR`, the same fixed
`MISSING_FAILURE_DETAIL` class, zero completed targets/tests, zero eligible
spawns, and zero cache hits. Exit is 1, stderr is empty, and private-root,
daemon, and Git cleanup pass. No raw/home data was read and no retry occurred.
Bazel's pinned BEP contract permits failed BuildFinished events to omit
`failure_detail`; therefore neither more structured pairs nor another identical
run can identify this error.

Next decision only
`WP-10-m8-bazel-buildbuddy-command-stderr-user-decision`. The user may run the
token-free minimal reproduction from the current-packet manifest and privately
inspect its terminal error. They must not paste the raw stream or any header,
token, path, or value. Report only either `minimal succeeds` or a token-free
paraphrase naming the offending option/failure kind. No agent reads stderr or
home configuration, no repository change occurs before the response, and no
cache claim is made. The response determines whether the next bounded packet
repairs a checked-in command option or records an external home/service
boundary.

The user decision is accepted from a token-free terminal diagnostic. The
minimal cache-profile invocation parses ordinary/home RC state and reaches
analysis, where `//app/slug_cli_v2:slug` fails because no registered
`@@rules_rust+//rust:toolchain_type` matches. No header, token, invocation URL,
or home-RC content was provided or recorded, and this result makes no cache or
connectivity claim.

The repository already registers `@rust_toolchains//:all` from the sole
`nightly/2025-09-14` extension tag. The accepted Stage 10 boundary requires
local commands to pass `--@rules_rust//rust/toolchain/channel=nightly` because
rules_rust defaults the channel selector to stable. The frozen cache driver
omitted that selector.

Next repair only `WP-10-m8-bazel-buildbuddy-nightly-channel-repair`. Add the
exact channel flag once to `command()` immediately after
`--config=buildbuddy-cache`, and pin its spelling, uniqueness, and order in the
existing command test. Change only
`tools/v2_oracle_lib/buildbuddy_cache.py` (at most 5 changed lines) and
`tests/v2_oracle/test_buildbuddy_cache_gate.py` (at most 15), at most 20 total.
Run only the focused offline unit test, Python compilation, cap/diff checks,
and independent REPLAN review. Do not change MODULE/lock/config/manifest/
targets, run Bazel, inspect home RC, contact BuildBuddy, or make a live retry.
Afterward, prefer the user's authentication-free sibling `../actiond` lane to
validate the Bazel build through REAPI before returning to hosted cache proof.

The nightly-channel repair is accepted in commit `7f58f3bc…`. The driver now
passes the exact selector once after its cache profile. Twenty-four focused
offline tests, Python compilation, six changed lines, diff checks, and
independent REPLAN review pass; MODULE, RC, locks, manifest, and targets are
unchanged, and no Bazel or remote command ran.

The user chooses sibling `../actiond` for the first authentication-free REAPI
build proof. The live sibling is clean at `8a42c3d4…`; Linux x86_64 has writable
KVM and vhost-vsock devices, but no built worker binary. This newer candidate
does not replace Stage 7's formal `ca39423b…` pin or count as backend
acceptance. It is only the Stage 10 developer-build smoke.

Next evidence only `WP-10-m8-actiond-local-reapi-build-evidence`. Create one
private mode-0700 top root and require loopback port 8980 to be unbound. In the
clean sibling, use private output base/symlink state and workspace RC with
system/home RC disabled to build `-c opt`
`//cmd/linux-actiond:linux-actiond_linux_x86_64`, explicitly clearing BES,
remote executor/cache, and disk cache and using no remote config. Allow ordinary
Bzlmod downloads, but no release download, commit switch, or fallback if this
source build fails. Resolve exactly one canonical executable through same-base
`cquery` plus `info execution_root`, bounded beneath the private output base.

Start that binary in its own process group on `127.0.0.1:8980` with private
state, 8192 MiB CAS image, 4096 MiB memory, four CPUs, and 180-second startup
timeout. Require a live PID and the exact VM gRPC-bridge listening event. Then,
from a clean Slug scheduling commit that retains `7f58f3bc…` as its accepted
code ancestor, and a separate fresh output base, run exactly one
`//app/slug_cli_v2:slug` build with system/home RC disabled; explicit nightly;
actiond as loopback executor/cache; empty instance/BES/results/disk/downloader;
remote-only spawn/genrule, no local fallback, no accepted cached actions, no
local-result upload or cache compression, top-level output download, 900-second
remote timeout, four jobs, and exact `libc=glibc2.39` plus `requires-bash=`
properties. Keep BEP and execution JSON private.

Accept only process/BEP/target success, one materialized executable, a nonempty
SpawnExec sequence, and every spawn exactly runner `remote`, cache miss, empty
status, and exit zero. Mixed/local/worker/sandbox/cache-hit/unknown execution
fails closed. On every exit, shut down both private Bazel servers, terminate and
reap the worker process group with bounded TERM/KILL, verify the port closes,
delete all private roots/logs, and recheck both repositories clean. Record only
the fixed summary, exact Slug run HEAD, and candidate actiond commit. Do not use
home RC, BuildBuddy,
RBE profiles, persistent state, or raw artifacts; this is local REAPI build
evidence, not BuildBuddy cache proof. After the run only owner/canonical/current
docs may record the result, at most 120 changed lines.

The source-worker packet returns `REPLAN` before worker startup or any Slug
build. Bazel 9.1.0 successfully analyzed the clean `8a42c3d4…` sibling target
and began 5,886 local actions, but after 4,416 processes its Linux sandbox
failed copying the generated kernel `arch` input because the destination
already existed; process exit was 36. No home RC, BES, remote executor/cache,
BuildBuddy, or release fallback was used. The private Bazel server shut down.
Generated LLVM/musl outputs were read-only, so the first exact-root delete
failed; making only that private root owner-writable then deleting it closed
cleanup. Both repositories are clean, port 8980 is closed, no worker/slugd
remains, and the packet is not retried.

Actiond's own README recommends releases for users. GitHub's immutable latest
full release is `v0.0.6`, which the clean sibling tag resolves to commit
`4bdf3e8899ead4eafad54943a18063e6ff0a2637`. Its
`linux-actiond_linux_x86_64` asset is exactly 15,905,480 bytes with SHA-256
`006dc798d4363596fe8ab997606fc93766a0cc427c2d005cf4fc1765fa4c2052`.
The 479-byte `SHA256.txt` asset has SHA-256
`639b31e99c2d9236b43e18ab03f6368625c346cab364386f8487ab6dea3a649a`
and must contain that exact binary digest/name pair.

Next evidence only `WP-10-m8-actiond-release-local-reapi-build-evidence`.
Create one private 0700 root, require clean checkouts and unbound port 8980,
download only the two exact versioned `v0.0.6` assets from GitHub, and verify
manifest size/digest/content plus binary size/digest before setting mode 0500.
No latest redirect, source-build retry, alternate mirror, commit switch, or
fallback is allowed. Start the verified binary with the already accepted
private-state/process-group/resource/readiness contract. Run the same single
fresh-output-base, no-system/home-RC, explicit-nightly, actiond remote-only
Slug binary build and the same private BEP/execution-log classifier.

Acceptance and claims are unchanged: process/BEP/target/output success,
nonempty SpawnExec, every spawn remote/uncached/empty-status/exit-zero, and only
Stage 10 local REAPI developer evidence. Always stop/reap the worker, shut down
the private Bazel server, verify port closure, make only the exact private root
owner-writable if needed, delete it, and recheck both repositories. Any
download/verification/start/build/evidence/cleanup failure is `REPLAN` with no
retry. Afterward only owner/canonical/current docs may record the fixed result,
at most 120 changed lines.

The verified-release packet returns `REPLAN` after its single permitted Slug
build. Clean Slug run HEAD `827c6fcc…` used clean sibling `8a42c3d4…`; the
`v0.0.6` tag remained `4bdf3e88…`. Both immutable release assets passed their
frozen size, SHA-256, and manifest-row checks before the binary became
executable. The KVM VM reached the exact loopback bridge-ready event and Bazel
9.2.0 analyzed `//app/slug_cli_v2:slug` with the registered nightly toolchain.

The sole SpawnExec was an uncached `remote` spawn, proving loopback REAPI
transport and remote scheduling, but its process setup returned exit 127 with
fixed actiond diagnostic `child setup failed: execve SUCCESS`. BEP ended
`BUILD_FAILURE`/`NON_ZERO_EXIT`, the target did not complete, and no executable
materialized. Pinned source places that diagnostic after sandbox setup when
`execve` returns; this identifies an actiond execution-launch boundary, but the
discarded private action data and no-retry rule leave its exact cause unproven.
It is not successful Slug compilation, cache reuse, Stage 7 backend acceptance,
or a BuildBuddy claim. The private Bazel server and VM stopped, port 8980
closed, the exact private root was made owner-writable and deleted, and both
repositories plus the no-`slugd` check remained clean.

Next evidence only
`WP-10-m8-bazel-buildbuddy-cache-live-evidence-after-nightly-repair`. Run the
frozen sanitized cache driver exactly once from the clean scheduling commit.
Only Bazel may consume ordinary workspace/home RC discovery; agents may review
only compact stdout, process status, and empty stderr. Accept only
`PROVED_CACHE_ONLY` with the already frozen 43-test/build, fresh prime/replay,
digest, runner, cache, and cleanup invariants. Any other classification,
stderr, retained private path, schema surprise, service/target/cache failure,
or required repair is `REPLAN` without a second attempt. Do not inspect home
configuration or raw artifacts, invoke RBE, or change code/config/CI/targets.
Only owner/canonical/current documentation may record the result, at most 100
lines in three files.

That packet returns `REPLAN` after its single frozen driver invocation from
clean head `df7fe87d…`. The compact record binds Bazel 9.2.0, Linux x86_64,
the exact manifest/root-RC hashes, and the clean head. Prime and replay both
report process and BuildFinished exit 2/name `COMMAND_LINE_ERROR`, fixed class
`MISSING_FAILURE_DETAIL`, zero completed targets/tests, zero eligible spawns,
and zero cache hits. Classification is `COMMAND_LINE_FAILURE`, stderr is empty,
all private roots are absent, Git and daemon checks pass, and no retry occurred.
This proves only a second pre-target command-line boundary; it proves neither
analysis/execution nor BuildBuddy connectivity/cache reuse.

The repaired driver contains the nightly selector exactly once in its pinned
location, while Bazel's failed BuildFinished again omits the only structured
failure detail. No checked-in command repair follows from the sanitized record,
and another identical driver invocation cannot add evidence. A static review
also found that the implementation currently requires prime eligible runners
to be exactly `local`, while its accepted design admits `local`, `worker`, and
`linux-sandbox`; this latent classifier drift was not exercised and did not
cause the pre-spawn failure, but must be separately resolved before a later
live cache retry.

Next decision only
`WP-10-m8-bazel-buildbuddy-command-stderr-user-decision-after-nightly-repair`.
The user may privately run the frozen token-free minimal reproduction with the
nightly selector and report only `minimal succeeds` or a token-free paraphrase
naming the offending option/failure kind. No raw output, header/token, path,
value, RC content, URL, or log may be pasted. No agent runs Bazel, reads stderr
or home configuration, contacts BuildBuddy, changes the repository, or makes a
cache/RBE claim before the response. RBE remains behind cache evidence.

The user decision is accepted: the exact minimal build succeeds with ordinary
RC discovery and the nightly selector. The user explicitly authorizes agents
to invoke Bazel in the normal environment so Bazel can consume the auth-only
`~/.bazelrc`; its contents and token remain unread, unprinted, uncopied, and
uncommitted. This closes the authentication/RC and single-target analysis/build
boundary, but it is not structured cache or RBE evidence. The known prime-
runner classifier drift remains a separate required repair before another
formal cache-gate attempt.

Next evidence only
`WP-10-m8-bazel-buildbuddy-rbe-developer-build-after-user-minimal-success`.
From the clean
decision commit, create one private mode-0700 root and run exactly one fresh-
output-base Bazel build of `//app/slug_cli_v2:slug` with ordinary RC discovery,
`--config=buildbuddy-rbe`, explicit nightly, no accepted cached result or local-
result upload, top-level output download, and private BEP/execution/terminal
files. The checked-in repository RC configuration supplies the hosted
endpoints; the `buildbuddy-rbe` profile supplies remote-only strategy/no-
fallback and managed Linux/amd64 properties. Inherit the process environment
unchanged; do not set, print, expand, copy, inspect, or otherwise touch `HOME`
or home RC. Only Bazel consumes the user-owned authentication.

Accept only process exit zero and exactly one executable materialized beneath
the private output base. Never display or inspect the terminal/BEP/execution
files. Any build, output, retained-root, Git, daemon, or cleanup failure is
`REPLAN` without retry. Always shut down the private Bazel server, delete raw/
private state, and recheck Git and `slugd`. This proves only that one fresh,
remote-only BuildBuddy-profile Stage 10 developer build completed; without
parsing structured logs it does not prove per-spawn RBE, cache reuse, the
43-test gate, CI, Stage 7 backend acceptance, or self-hosting. Afterward only
owner/canonical/current docs may record the fixed result, at most 120 lines in
three files.

The direct BuildBuddy-profile packet is accepted from clean head `1d931816…`.
Its one ordinary-RC invocation inherited the environment unchanged, exited
zero, and materialized exactly one executable regular Slug binary beneath the
fresh private output base. The private Bazel server shut down with exit zero;
the exact private root was deleted; Git remained clean and no `slugd` existed.
No terminal, BEP, execution-log, home-RC, header, or token content was read.
This proves one fresh remote-only BuildBuddy-profile Stage 10 developer build,
not structured per-spawn RBE, cache reuse, the 43-test gate, CI, Stage 7
acceptance, or self-hosting.

The earlier recorded prime-runner classifier drift is withdrawn. `_runner()`
normalizes exact `local`, `worker`, and `linux-sandbox` spellings to the same
`local` class, which prime `spawn_summary()` accepts. The focused test already
pins that normalization; a later test-only strengthening may assert its zero
status-failure consequence, but there is no production repair or live-gate
blocker at this boundary.

Next evidence only
`WP-10-m8-bazel-buildbuddy-cache-prime-command-vector-isolation`. Run exactly
one private ordinary-RC `bazel test` with the frozen cache driver's prime option
vector unchanged but only `//app/slug_cli_v2:slug` as its label. Inherit the
environment; only Bazel may consume home auth. Keep mode-0600 terminal/BEP/
execution files unread and private, then delete them. Exit zero proves the
option vector parses and isolates the prior opaque failure to the full target
set/combined test boundary. Exit two records only `PRIME_VECTOR_EXIT_2` and
stops; any other result is `REPLAN`. Do not retry, bisect options, change code/
config, or make cache/RBE/test claims. Always shut down the private server,
delete the exact root, and recheck Git and `slugd`. Afterward only owner/
canonical/current docs may record the fixed result, at most 120 lines.

The one-label prime-vector packet returns `REPLAN` from clean head
`6b66faa8…`. The exact frozen prime option vector with only
`//app/slug_cli_v2:slug` exits two in 1.3 seconds, so the 43-label target set is
not causal and the checked-in vector remains the bounded failure surface. No
raw terminal/BEP/execution or home-RC content was read, no retry or bisection
occurred, and private-server shutdown, exact-root deletion, Git, and no-`slugd`
checks pass. The sole retained result is `PRIME_VECTOR_EXIT_2`; it makes no
cache, RBE, test, or option-cause claim.

Next evidence only
`WP-10-m8-bazel-buildbuddy-cache-prime-root-only-nobuild-diagnosis`. Run the
one-label prime-vector no-remote variant exactly once with system/home/workspace
RC disabled, ambient `BAZELRC` unset, the tracked root RC selected explicitly,
and final empty service/cache overrides plus `--nofetch --nobuild`. This command
cannot consume home authentication, contact a configured BuildBuddy service,
fetch repositories, or execute actions, so its private stderr may be inspected
transiently only to identify a public checked-in flag/combination; never paste,
retain, or commit it. Exit two plus an attributable public non-remote identifier
is `ROOT_ONLY_NONREMOTE_DIAGNOSED`; exit zero is
`ROOT_ONLY_NONREMOTE_ACCEPTED`; an unattributable exit two is
`ROOT_ONLY_UNEXPLAINED`; any other result or cleanup failure is `REPLAN`. Do not
retry, bisect, change code/config, or claim cache/RBE behavior. Always shut down
the private server, delete the exact root, and recheck Git and `slugd`. Afterward
only owner/canonical/current docs may record the fixed result, at most 120 lines.

The root-only no-remote packet returns `REPLAN` from clean head `cc79bac0…`.
Its one credential-free `--nofetch --nobuild` variant exits one in 0.8 seconds,
outside the accepted zero/two outcomes. Raw terminal/BEP/execution content
remained unread; shutdown, exact-root deletion, Git, and no-`slugd` checks pass.
The fixed result `REPLAN_EXIT_1` proves neither a checked-in option cause nor a
normal-RC/auth interaction, and no retry or interpretation is allowed.
Next implementation only
`WP-10-m8-bazel-buildbuddy-prime-stderr-sanitizer-implementation`. Add a
dedicated stdlib diagnostic library/CLI and focused tests without changing the
existing cache gate. The runner must reuse the frozen one-label prime argv,
ordinary RC discovery, private mode-0700/0600 lifecycle, unread stdout/BEP/
execution data, RC-disabled shutdown, and exact-root cleanup. Only on process
exit two may its pure sanitizer read bounded strict-UTF-8 stderr bytes. It may
map only the five bare public spellings at the intersection of pinned Bazel
9.2's unrecognized-option grammar and the frozen vector:
`--noremote_local_fallback`, `--build_event_publish_all_actions`,
`--noremote_accept_cached`, `--remote_upload_local_results`, and
`--noremote_cache_async`. Require the entire payload to match the one-line
`ERROR: <flag> :: Unrecognized option: <flag>` shape apart from fixed terminal
whitespace. Malformed/oversized input, every other prime flag, extra bytes or
lines, zero/multiple/unknown flags, paths, URLs, headers, tokens, nonces,
arbitrary values, or unexpected shape yield only an opaque fixed class. No
source substring, hash, exception, path, or raw value may enter stdout/stderr,
persistence, or Git.
Change only `tools/v2_oracle_lib/buildbuddy_prime_diagnostic.py` (240 lines),
`tools/v2_oracle/buildbuddy_prime_diagnostic.py` (40), and
`tests/v2_oracle/test_buildbuddy_prime_diagnostic.py` (320): at most 600 lines.
Tests use mocks/synthetic bytes and cover all five mappings, every other prime
flag as rejection, all rejection classes, malicious secret/path/value
suppression, frozen argv reuse, mode and cleanup failures, empty CLI stderr,
and the closed JSON schema. Run only focused offline tests, Python compilation,
caps/diff checks, and independent review; no Bazel, ordinary/home RC, remote
call, or live evidence. A later packet owns one sanitized normal-RC invocation.

The sanitizer implementation is accepted in commit `ec8ec2d7…`: 423 lines in
the exact three-file scope. It reuses the frozen one-label prime argv, generates
one private nonce, reads only regular no-follow bounded stderr on exit two,
recognizes only the five whole-payload pinned-source shapes, normalizes every
record/CLI return through the closed schema, and owns RC-disabled shutdown plus
descriptor-anchored exact-root cleanup. Eight focused synthetic/mocked tests,
Python compilation, caps, diff checks, and independent privacy/lifecycle review
pass, including malicious return, FIFO/symlink stderr, read-only cleanup, and
root/nested-directory swap regressions. No Bazel, normal/home RC, or remote
command ran.

Next evidence only
`WP-10-m8-bazel-buildbuddy-prime-normal-rc-sanitized-stderr-evidence`. From the
clean implementation commit, run
`python3 tools/v2_oracle/buildbuddy_prime_diagnostic.py` exactly once with the
inherited environment. Only Bazel may consume ordinary/home RC. Review only the
single compact JSON object, CLI status, and empty CLI stderr; never inspect home
RC or private/raw artifacts. Accept a diagnosis only for exit-zero CLI result,
classification `NORMAL_RC_PRIME_DIAGNOSED`, and one of the five frozen fixed
IDs, then schedule one bounded checked-in option repair. Any other class/status,
schema surprise, retained root, Git/daemon drift, or cleanup failure is
`REPLAN` without retry, bisection, code/config change, or cache/RBE claim. Only
owner/canonical/current docs may record the fixed result, at most 100 lines.

The single sanitized invocation from clean head `d0f4d572…` returns `REPLAN`.
Its CLI exits one with empty stderr and the sole closed record
`SANITIZER_REJECTED`/`NONE`; all diagnostic/wrapper roots are absent, Git is
clean, and no `slugd` remains. No raw/home data was read and no retry occurred.
Because no public option identifier was emitted, no checked-in repair is
justified. The old 43-test driver remains unchanged as a later expansion target.

Next implementation only
`WP-10-m8-bazel-buildbuddy-build-cache-vertical-implementation`. Add a separate
stdlib build-only cache driver/CLI/tests. Each phase uses a fresh output base and
the user-confirmed minimal `bazel build --config=buildbuddy-cache` command with
nightly, empty executor/BES/results/disk cache, no local fallback, one shared
fresh 64-hex action-env nonce, private BEP/execution logs, and only
`//app/slug_cli_v2:slug`. Deliberately omit the failed vector's explicit cache/
instance, spawn/test strategy, all test flags, publish-all-actions, cache-read/
upload/async overrides, and test nonce; repository RC/profile/defaults own
those already-proven settings.

Accept `PROVED_BUILD_CACHE` only when both process/BEP/target results succeed;
each fresh base contains exactly one executable regular target output matching
`*/bin/app/slug_cli_v2/slug`; eligible prime runners are exactly `local`,
`worker`, or `linux-sandbox` and all are cache misses; replay spawns are remote-
cache hits; nonempty digest multisets
match; status/exit and persistent-action-cache failures are zero; and private
cleanup/Git/no-`slugd` pass. Add only
`tools/v2_oracle_lib/buildbuddy_build_cache.py` (250 lines),
`tools/v2_oracle/buildbuddy_build_cache_gate.py` (40), and
`tests/v2_oracle/test_buildbuddy_build_cache_gate.py` (360): 650 lines maximum.
Offline mocked tests, compilation, caps/diff, and independent privacy/lifecycle
review only; no Bazel/home/remote invocation. A separate packet owns one live
pair. The claim remains one build label; structured RBE and then the full
43-test expansion follow without redefining the final Stage 10 gate.

The build-only cache vertical is accepted in commit `e902610c…`: 412 lines in
the exact three-file scope. Ten focused synthetic/mocked tests, Python
compilation, caps, diff checks, and independent privacy/lifecycle review pass.
The frozen driver constructs the successful minimal ordinary-RC build shape,
shares only a fresh unprinted nonce across distinct output bases, emits only a
deep-normalized fixed schema, requires exactly one terminal BEP event and one
regular executable per phase, reads precreated evidence through retained
no-follow descriptors, and keeps root/phase identities through output checks,
RC-disabled shutdown, and descriptor-safe cleanup. Hostile nested output,
evidence symlink/hardlink replacement, and phase-directory swap regressions
fail closed. No Bazel, home RC, or remote service was invoked by that packet.

Next evidence only
`WP-10-m8-bazel-buildbuddy-build-cache-vertical-live-evidence`. From its clean
scheduling commit, run
`python3 tools/v2_oracle/buildbuddy_build_cache_gate.py` exactly once with the
inherited environment. Only Bazel may consume ordinary/home RC. Review only the
single compact JSON object, empty CLI stderr, and CLI status. Accept only exit
zero, schema version one, fixed `buildbuddy-build-cache-only` mode, and
`PROVED_BUILD_CACHE`. Any other class/status, schema surprise, retained root,
Git/daemon drift, or cleanup failure is `REPLAN` without retry, bisection, or
code/config change. Never inspect home RC, terminal/BEP/execution artifacts,
BuildBuddy UI data, or invocation URLs. This proves at most one build-label
cache vertical; structured build-only RBE proof and then expansion to all 43
accepted test targets remain mandatory successors.

The single live cache pair from clean head `1a4ca52c…` returns `REPLAN`. The
frozen CLI exits one with empty stderr and the sole fixed classification
`EVIDENCE_INCOMPLETE`; its closed prime/replay summaries remain conservative
and empty. The exact temporary root is absent afterward, Git remains clean,
and no `slugd` exists. No terminal/BEP/execution or home-RC content was read,
and no retry, cause inference, cache claim, or RBE claim is allowed.

Next implementation only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-artifact-probe-implementation`.
Add a separate stdlib library/CLI and tests that reuse the accepted cache
command exactly for one mocked prime but never read any output or evidence
contents. After process return it may expose only fixed `ZERO|NONZERO` process
and `PRIVATE_REGULAR|NOT_PRIVATE_REGULAR` BEP/execution metadata classes, using
precreated identities and no-follow checks. Its deep-normalized closed schema
also contains only schema version one, fixed
`buildbuddy-build-cache-prime-artifact-probe` mode, and
`PROBE_RECORDED|SANITIZER_REJECTED`; rejection uses conservative fixed values.
Reuse clean-Git/no-`slugd`, RC-disabled shutdown, retained-root identity, and
exact cleanup. Add only the library (220 lines), CLI (35), and test (260): 515
lines maximum. Mocked tests cover all eight combinations, malicious schema,
symlink/hardlink/replacement, no-read enforcement, command reuse, cleanup and
secret suppression. Offline tests/compilation/caps/diff and independent review
only; do not invoke Bazel/home RC/remote service or edit the accepted gate.
A separate packet owns one live probe; structured cache/RBE and the 43-test
expansion remain required.

The metadata-only artifact probe is accepted in commit `83262a2c…`: 261 lines
in the exact three-file scope. Seven offline mocked tests, Python compilation,
caps/diff checks, and independent privacy/lifecycle review pass. The probe
reuses the exact build-only prime command, never reads terminal or artifact
contents, and emits only fixed process and nonempty retained-private-identity
classes. Deep CLI normalization, all eight combinations, malicious schema,
evidence symlink/hardlink/replacement, phase/root swaps, no-read enforcement,
RC-disabled shutdown, original-inode cleanup, and secret suppression are pinned.
No Bazel, normal/home RC, or remote service was invoked by that packet.

Next evidence only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-artifact-probe-live-evidence`.
From its clean scheduling commit, run
`python3 tools/v2_oracle/buildbuddy_build_cache_artifact_probe.py` exactly once
with the inherited environment; only Bazel consumes ordinary/home RC. Review
only CLI status, empty stderr, and the normalized fixed-schema record. Accept
only exit zero and `PROBE_RECORDED`; do not retry, read any private contents,
modify code/config, or claim cache/RBE. Route `NONZERO` plus any unusable
artifact to a user-owned token-free environment decision; `NONZERO` plus two
usable artifacts to a separate strict failure-detail sanitizer design; `ZERO`
plus any unusable artifact to `REPLAN`; and `ZERO` plus both usable artifacts
to a strictly allowlisted parser-discriminator design. Any lifecycle/schema
failure is `REPLAN`. Structured RBE and the 43-test expansion remain required.

The one-shot metadata probe from clean head `bb1f169d…` is accepted with exit
zero, empty stderr, and fixed record `ZERO` / `PRIVATE_REGULAR` BEP /
`NOT_PRIVATE_REGULAR` execution. The private root is absent afterward, Git is
clean, and no `slugd` exists. No artifact/home-RC content was read and no retry
or cache/RBE claim occurred. This isolates the prior failure to the requested
execution-artifact ownership contract after a successful prime process.

Pinned Bazel 9.2 source at `8220c619…` resolves that contract:
`ExpandedSpawnLogContext.java` lines 106-130 makes JSON require conversion,
deletes a preexisting output, and writes through a temporary path; lines
291-316 create the converted final output and may emit zero records.
`ExecutionOptions.java` lines 420-436 defines JSON entries as executed spawns.
Therefore retained inode identity is invalid for this artifact, while an empty
final regular file remains source-permitted.

Next implementation only
`WP-10-m8-bazel-buildbuddy-prime-execution-artifact-contract-implementation`.
Add separate stdlib library/CLI/tests (190/35/270 lines; 495 total) that reuse
the frozen prime command and classify only process `ZERO|NONZERO` plus final
execution metadata `ANCHORED_PRIVATE_NONEMPTY|ANCHORED_PRIVATE_EMPTY|
NOT_ANCHORED_PRIVATE`. Through the retained private phase descriptor, accept a
replaced or retained direct-child file only when regular, mode 0600, and single-
link; never open/read/hash content or expose size/inode/path. Deep-normalize the
fixed schema and reuse exact cleanup/shutdown/Git/daemon guards. Mock all
replacement/retention/empty/link/mode/directory/swap and hostile-output cases.
Offline tests/compilation/caps/diff and independent review only; do not invoke
Bazel/home RC/remote service or edit accepted files. One later packet owns one
live probe; structured cache/RBE and 43-test expansion remain mandatory.

The execution-artifact replacement contract is accepted in commit
`520b45f6…`: 272 lines in the exact three-file scope. Eight mocked offline
tests, Python compilation, caps/diff checks, and independent privacy/lifecycle
review pass. It accepts Bazel's pinned regular-file replacement only under
retained root/phase/output identities and mode-0600/single-link constraints,
distinguishes empty/nonempty by metadata, never opens content, deep-normalizes
exact built-in schema types, anchors shutdown, and deletes original plus
replacement reserved roots. Prime/shutdown output/root swaps, link/mode/
directory attacks, subclass smuggling, cleanup failure, and no-leak lifecycle
are pinned. No Bazel/home RC/remote service ran.

Next evidence only
`WP-10-m8-bazel-buildbuddy-prime-execution-artifact-contract-live-evidence`.
From its clean scheduling commit, invoke
`python3 tools/v2_oracle/buildbuddy_build_cache_execution_artifact_probe.py`
exactly once with inherited environment. Only Bazel consumes ordinary/home RC;
review only CLI status, empty stderr, and the fixed record. Accept only exit
zero and `PROBE_RECORDED`; never retry or inspect private contents. Route
`ZERO+ANCHORED_PRIVATE_NONEMPTY` to a separate strict parser-discriminator
design; `ZERO+ANCHORED_PRIVATE_EMPTY` to a source-consistent no-record stop;
`ZERO+NOT_ANCHORED_PRIVATE` to `REPLAN`; anchored `NONZERO` to a failure-detail
sanitizer design; and other `NONZERO` to a user-owned token-free environment
decision. This makes no cache/RBE claim; those and 43-test expansion remain.

The one-shot replacement-aware probe from clean head `e25d02f2…` is accepted:
exit zero, empty stderr, and fixed `ZERO+ANCHORED_PRIVATE_NONEMPTY`. Its exact
private root is absent afterward, Git remains clean, and no `slugd` exists. No
artifact/home-RC content was inspected and no retry occurred. This proves a
usable private JSON execution log exists under Bazel's pinned replacement
semantics, not cache behavior.

Next repair only
`WP-10-m8-bazel-buildbuddy-build-cache-execution-replacement-repair`. Change
only `tools/v2_oracle_lib/buildbuddy_build_cache.py` and its focused test: at
most 150 production/220 test/370 total net lines. Preserve the CLI, schema,
argv, classifications, and exact-inode BEP rule. For execution only, open the
final direct child relative to the retained phase FD with no-follow; accept a
retained/replaced regular mode-0600 single-link file, read through that FD,
recheck its dirent identity, and use the existing strict JSON/spawn parser.
Precreate/retain each output-directory FD and require root/phase/output identity
around parsing, output inspection, and shutdown. Cleanup both original and
replacement reserved roots without following links. Offline tests pin retained/
replaced/empty, links/mode/directory, BEP rejection, all path swaps, unchanged
public contracts, cleanup, and suppression. Focused tests/compile/caps/diff and
independent review only; no Bazel/home RC/network/live data. A second material
repair or public-contract relaxation is `REPLAN`. One later packet owns one
unchanged gate invocation; RBE and 43-test expansion remain successors.

The replacement-aware cache-gate repair is accepted in commit `72f45081…`:
production +97/test +93 net lines in the exact two-file scope. Fourteen focused
offline tests, Python compilation, caps/scope/diff checks, and independent
privacy/lifecycle review pass. The public CLI/schema/argv/classes and BEP
exact-inode rule remain unchanged. Execution alone admits Bazel's pinned final
regular replacement through a no-follow phase-relative FD, revalidates exact
0600/single-link/dirent state after reading, and uses the existing strict
parser. Root/phase/output identities surround parsing, materialization, and
shutdown; cleanup removes original/replacement reserved roots. Replacement,
empty/link/mode/directory, mid-read mutation, setup-FD failure, swap, subclass,
cleanup, suppression, and no-leak cases are pinned. No Bazel/home RC ran.

Next evidence only
`WP-10-m8-bazel-buildbuddy-build-cache-repaired-live-evidence`. From its clean
scheduling commit, invoke
`python3 tools/v2_oracle/buildbuddy_build_cache_gate.py` exactly once with the
inherited environment. Only Bazel consumes ordinary/home RC. Review only CLI
status, empty stderr, and the fixed compact record. Accept only exit zero and
`PROVED_BUILD_CACHE`; any other class/status/schema/lifecycle result is
`REPLAN` without retry, artifact inspection, or code/config change. This proves
at most one build-label cache vertical; structured build-only RBE, 43-test
expansion, and the rest of Stage 10 remain required.

The one repaired live gate from clean head `9c537944…` returns `REPLAN`: exit
one, empty stderr, fixed `EVIDENCE_INCOMPLETE`, and conservative empty phase
summaries. The private root is absent afterward, Git is clean, and no `slugd`
exists. No private/home-RC content was read and no retry or cause inference is
allowed. Prior probes already prove prime process zero plus retained nonempty
BEP and replacement-aware nonempty execution, so a parser-stage discriminator
is the next bounded boundary.

Next implementation only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-stage-probe-implementation`. Add a
separate stdlib library/CLI/tests (260/35/320 lines; 615 total) that reuse the
exact one-prime command, descriptor readers/anchors/output checks, parsers,
shutdown, cleanup, Git, and daemon guards. Emit only schema version, fixed mode,
`STAGE_RECORDED|SANITIZER_REJECTED`, process `ZERO|NONZERO`, and one fixed stage:
`NOT_RECORDED`, `PRECHECK_REJECTED`, `SETUP_REJECTED`, `PROCESS_NONZERO`,
`POST_RUN_ANCHOR_REJECTED`, `BEP_DESCRIPTOR_REJECTED`, `BEP_PHASE_REJECTED`,
`EXECUTION_DESCRIPTOR_REJECTED`, `EXECUTION_SPAWN_REJECTED`, `OUTPUT_REJECTED`,
`POST_PARSE_ANCHOR_REJECTED`, `PRIME_SEMANTICS_REJECTED`, or `PRIME_READY`.
Never emit counts, values, paths, sizes, hashes, labels, runners, or exceptions.
Mock every stage, ownership/swap/lifecycle attack, schema subclass, suppression,
empty CLI stderr, exact argv, and unchanged gate. Offline tests/compile/caps/
diff and independent review only; no Bazel/home RC/network or gate edit. One
later packet owns one probe; cache/RBE and 43-test expansion remain required.

The fixed-enum one-prime stage probe is accepted in `a5eb73d7…` at 114 library,
17 CLI, 126 test, and 257 total lines. Its six focused tests and the 35-test
related regression set pass; independent review accepts its outcome check,
canonical stage/process matrix, strict BEP/replacement-aware execution readers,
frozen-gate assertion, and fail-closed lifecycle.

Next evidence only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-stage-probe-live-evidence`. From a
clean scheduling commit, invoke the frozen CLI exactly once with the inherited
environment and ordinary Bazel RC discovery. Bazel alone may consume the
private home RC. Review only CLI status, empty stderr, and its fixed compact
JSON; never inspect or retain home-RC, terminal, BEP, execution, invocation, or
remote-service data. Do not retry or modify code/config. Record the fixed stage
and follow its predeclared route; cache/RBE and 43-test expansion remain open.

The one CLI issue from clean `ba003ec3…` is control-plane `REPLAN`: after the
terminal tool yielded, its caller discarded the live session identifier, so no
exit, stderr, or fixed JSON is recoverable. No result is inferred. The private
root existed only while the command ran and is absent afterward; Git is clean
and no `slugd` exists. Home RC, terminal/BEP/execution, invocation, and service
data were unavailable and uninspected. This says nothing about Bazel,
credentials, BuildBuddy, cache state, or the frozen probe.

Next evidence only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-stage-probe-transported-live-evidence`.
One in-memory wrapper may start the frozen CLI exactly once with unchanged
environment, no shell child, and anonymous private stdout/stderr files; it
checks stderr by size, bounds stdout to 2 KiB, validates exact normalized JSON,
and emits only a fixed `DELIVERED|REJECTED` envelope. The terminal caller must
retain and poll any returned session ID. Session loss, invalid envelope, raw
output, cleanup/Git/daemon drift, or `REJECTED` stops at `REPLAN` without reissue.

The transported invocation from clean `e017d16d…` is accepted as delivery:
outer zero, `DELIVERED`, child zero, empty child stderr, and fixed
`BEP_PHASE_REJECTED`. Its private root is absent afterward, Git is clean, and no
`slugd` exists. No raw/home/artifact/service data was inspected. This localizes
only the prime BEP parser boundary and proves no cache or RBE behavior.

Next implementation only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-bep-stage-probe-implementation`.
Add separate stdlib library/CLI/tests at 230/35/300 and 565 total lines. Reuse
the exact one-prime command, strict BEP descriptor, anchors, shutdown, cleanup,
Git, and daemon guards; never read execution content or traverse outputs. Emit
only fixed stream/event/terminal/counter/BEP-ready stages, with exact schema and
process pairing. Offline tests and independent review only; no Bazel, home RC,
network, gate/stage-probe/config edit, or live artifact.

The BEP-only fixed-stage probe is accepted in `6b176661…` at 127 library, 17
CLI, 117 test, and 261 total lines. Its five focused and 40 related tests pass;
independent review accepts exact `phase_record` evaluation/first-failure order,
semantic-value acceptance, terminal/counter separation, privacy, and lifecycle.

Next evidence only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-bep-stage-probe-transported-live-evidence`.
Use the accepted anonymous bounded transport and retained-session polling to
invoke the frozen BEP CLI exactly once with inherited environment. Review only
the fixed envelope, empty child stderr, and normalized fixed-stage record. No
retry, raw/home/artifact/service access, code/config edit, or cache/RBE claim.

The transported BEP invocation from clean `b4ffdf58…` delivers outer/child zero,
empty child stderr, and fixed `BEP_COUNTER_REJECTED`; cleanup, Git, and no-`slugd`
are green. Pinned Bazel 9.2 `JsonFormatFileTransport` uses protobuf 33.4 JSON
without default-value inclusion. Both touched fields are `int32`: nonzero values
are JSON numbers, while successful `BuildFinished.ExitCode.code == 0` is omitted.
`ActionCacheStatistics.hits` already defaults an absent field correctly; no live
artifact was needed or inspected.

Next repair only `WP-10-m8-bazel-buildbuddy-bep-zero-default-repair`. In the
gate and BEP probe only, default absent exit `code` to integer zero before the
unchanged strict counter validator. Update their focused tests plus only the
prime-stage test's frozen gate digest, with source-faithful omitted-zero fixtures
and explicit null/bool/string/negative rejections. Five files, at most 12
production/68 test/80 total net lines;
offline validation and independent review only. No Bazel/home RC/network/live
artifact/config/schema/command/lifecycle change.

The omitted-zero repair is accepted in `6758d97f…`: both parser sites default
only absent exit `code` to integer zero before unchanged strict validation.
Source-faithful fixtures and explicit invalid-value regressions pass 40 related
tests; independent review accepts the five-file, net-zero-production change.

Next evidence only
`WP-10-m8-bazel-buildbuddy-bep-zero-default-transported-live-evidence`. Invoke
the repaired BEP-stage CLI exactly once through the accepted anonymous bounded
transport with retained-session polling and inherited environment. Review only
the fixed envelope, empty child stderr, and normalized stage. No retry,
raw/home/artifact/service access, code/config edit, or cache/RBE claim.

The repaired transported BEP invocation from clean `91a0b8fb…` delivers
outer/child zero, empty child stderr, and fixed `BEP_READY`; its private root is
absent afterward, Git is clean, and no `slugd` exists. No raw/home/artifact/
service data was inspected. This proves only the BEP portion of prime parsing.

Next implementation only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-execution-stage-probe-implementation`.
Add separate stdlib library/CLI/tests at 180/30/220 and 430 total lines. Reuse
the exact command, replacement-aware execution reader, anchors, shutdown,
cleanup, Git, and daemon guards. Preserve lazy first-failure order and emit only
fixed descriptor/stream/spawn/execution-ready stages. Never read BEP or traverse
output. Offline tests and independent review only; no Bazel/home RC/network/
live artifact or existing-file edit.

The combined output/prime-semantics probe is accepted in `1eb2d324…` at 114
library, 17 CLI, 130 test, and 261 total lines. Its six focused and 51 related
tests pass; independent review accepts output-first short-circuiting, structural
prime-predicate reuse, complete predicate branches, private descriptor/read
attacks, shutdown/cleanup, schema, and lifecycle.

Next evidence only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-output-semantics-probe-transported-live-evidence`.
Invoke the frozen CLI exactly once through the accepted anonymous bounded
transport with retained-session polling and inherited environment. Review only
the fixed envelope, empty child stderr, and normalized stage. No retry, raw/
home/artifact/service access, code/config edit, or cache/RBE claim.

That transported invocation delivers outer/child zero, empty child stderr, and
fixed `PRIME_SEMANTICS_REJECTED`, but returns lifecycle `REPLAN`: one matching
temporary root remains for more than 30 seconds after return. No matching probe
or Bazel-output-base process or `slugd` remains; the root is removed with the
accepted no-follow helper without reading its contents, and a later audit is
clean. Because the external observation is not identity-bound, quarantine the
semantic stage as routing input rather than accepted live evidence. No raw,
home, artifact, invocation, or service data was inspected.

Next implementation only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-lifecycle-guard-implementation`.
Add separate stdlib library/CLI/tests at 220/30/300 and 550 total lines, without
editing existing probe code. Require an empty reserved-root namespace before
starting, invoke the frozen output-semantics CLI once with inherited environment
and anonymous bounded transport, then recheck the namespace, Git, and daemon
state. Only `LIFECYCLE_CLEAN` may expose the normalized child stage. A single
new residue is removed through the existing no-follow helper but suppresses the
stage; all other lifecycle failures fail closed. Offline tests and independent
review only; no Bazel, network, home RC, live artifact, or service access.

The lifecycle guard is accepted in `a0cdaa13…` at 183 library, 25 CLI, 213
test, and 421 total lines. Thirteen focused and 64 related tests pass along with
compile, scope, line, and diff gates. Independent correction review accepts
exact schema coupling, canonical anonymous child transport, independent Git and
daemon checks, nofollow root identity through cleanup, replacement preservation,
and suppression of every non-clean child stage.

Next evidence only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-lifecycle-guard-transported-live-evidence`.
From a clean reserved-root namespace, invoke the frozen guard CLI exactly once
through the accepted anonymous bounded outer transport with retained-session
polling and inherited environment. Review only the fixed transport envelope,
empty guard stderr, normalized lifecycle, and normalized nested stage when the
lifecycle is clean. No retry, raw/home/artifact/service access, code/config edit,
or cache/RBE claim.

The single guarded invocation from clean `13b0700f…` delivers outer/guard zero,
empty guard stderr, `LIFECYCLE_CLEAN`, and nested fixed
`PRIME_SEMANTICS_REJECTED`. Afterward there are zero matching output-base
processes, reserved roots, or `slugd`, and Git is clean. No raw, home, artifact,
invocation, or service data was inspected. This accepts lifecycle and the fixed
semantic route only; it makes no cache or RBE claim.

Next implementation only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-semantic-branch-discriminator-implementation`.
Within the existing shared prime predicate, output-semantics probe, and its
tests, replace the coarse semantic rejection with one fixed ordered branch per
existing predicate input. Preserve all pre-semantic stages and lifecycle code,
reuse one shared helper, and emit no values, counts, paths, labels, hashes, or
runner spellings. At most three existing files, 535 final lines, and +125 net
lines. Offline tests and independent public-enum review only; no Bazel, network,
home RC, live artifact, service, CLI, guard, parser, gate, config, or fixture edit.

The semantic branch discriminator is accepted in `70b3c84a…` at 134 shared
predicate, 114 output probe, 187 test, 435 final, and +77 net lines. Nine
focused and 67 related tests pass with compile, scope, cap, and diff gates.
Independent correction review accepts exact first-failure coverage, parsed
non-success BEP routes, shared `_ready` equivalence, ZERO-only fixed stages, and
unchanged pre-semantic and lifecycle behavior.

Next evidence only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-semantic-branch-discriminator-transported-live-evidence`.
Invoke the unchanged lifecycle-guard CLI exactly once through the accepted
anonymous bounded outer transport with retained-session polling and inherited
environment. Review only the fixed transport envelope, empty guard stderr,
`LIFECYCLE_CLEAN`, and its normalized nested semantic stage. No retry, raw/
home/artifact/service access, code/config edit, or cache/RBE claim.

The single guarded invocation from clean `7ab0b0a7…` delivers outer/guard zero,
empty guard stderr, `LIFECYCLE_CLEAN`, and nested fixed
`PRIME_CACHE_EXPECTATION_REJECTED`. Afterward there are zero matching
output-base processes, reserved roots, or `slugd`, and Git is clean. No raw,
home, artifact, invocation, or service data was inspected. This proves only
that the prime execution log records at least one cache-state mismatch.

Pinned Bazel 9.2 source closes the repair direction. `SpawnExec.cache_hit` is a
proto3 boolean; `ExpandedSpawnLogContext` always sets it, and the execution-log
JSON printer uses `alwaysPrintFieldsWithNoPresence`, so false remains explicit.
`RemoteOptions` defaults cache reads on and `RemoteExecutionService` uses that
option in its read policy. The one-label command omitted the already-designed
prime override, so parser defaulting would weaken exact evidence rather than
repair the command.

Next repair only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-disable-cache-reads-repair`. Add
exactly `--noremote_accept_cached` to the one-prime command and pin its position,
strict cache-field matrix, and source-faithful prime readiness. Update only four
shared-source digest assertions. Six files, at most four production and 50 test
changed lines. Do not change parsers, schema, stages, lifecycle, RC/config,
fixtures, or any other option. Offline tests and independent source review only;
no Bazel, network, home RC, live artifact, or service access.

The cache-read repair is accepted in `89b848b9…`: one production argument and
18 test changed lines across the exact six files. Three focused and 67 related
tests pass with compile, scope, cap, digest, and diff gates. Independent pinned-
source review accepts one positioned `--noremote_accept_cached`, strict unchanged
cache-hit parsing, the complete prime/replay field matrix, and no other command
drift.

Next evidence only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-disable-cache-reads-transported-live-evidence`.
Invoke the unchanged lifecycle-guard CLI exactly once through the accepted
anonymous bounded outer transport with retained-session polling and inherited
environment. Review only the fixed transport envelope, empty guard stderr,
`LIFECYCLE_CLEAN`, and its nested semantic stage. No retry, raw/home/artifact/
service access, code/config edit, or cache/RBE claim.

The single guarded invocation from clean `dae86d3d…` delivers outer/guard zero,
empty guard stderr, `LIFECYCLE_CLEAN`, and nested fixed `PRIME_READY`. Afterward
there are zero matching output-base processes, reserved roots, or `slugd`, and
Git is clean. No raw, home, artifact, invocation, or service data was inspected.
This proves the isolated prime build only; replay and cache reuse remain open.

The paired gate cannot yet run: its phase loop calls one phase-agnostic command,
so the newly correct prime-negative cache-read flag would also disable replay
reads. Pinned Bazel 9.2 makes `remote_accept_cached` the read-policy selector;
upload and async flags do not control reads and remain outside the minimal
one-label vector.

Next repair only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-replay-read-policy-repair`.
Require an exact `prime|replay` phase in the shared command API, selecting only
`--noremote_accept_cached|--remote_accept_cached` at the existing argument slot.
Update every prime caller explicitly and make the paired gate pass its loop
phase. Fourteen files, at most 30 production and 70 test changed lines. No
compatibility wrapper, default phase, upload/async option, parser, schema,
classifier, lifecycle, config, fixture, or CLI change. Offline tests and
independent source review only; no Bazel, network, home RC, artifact, or service.

The phase-explicit read-policy repair is accepted in `7bb65291…`: 20 production
and 27 test changed lines across the exact 14 files. Fifty-four focused and 67
related tests pass with compile, scope, cap, digest, and diff gates. Independent
review accepts the required exact phase API, unchanged prime vectors, opposite
single-slot read policies, paired order, shared nonce, distinct output bases,
and no upload/async/parser/schema/lifecycle/config drift.

Next evidence only
`WP-10-m8-bazel-buildbuddy-build-cache-phase-explicit-transported-live-evidence`.
Invoke the paired build-cache CLI exactly once through an anonymous bounded outer
transport with retained-session polling and inherited environment. Review only
the fixed transport envelope, empty CLI stderr, and normalized compact gate
record. No retry, raw/home/artifact/service access, code/config edit, broad gate,
or RBE claim.

The single paired invocation from clean `d9f14877…` delivers outer/child zero,
empty child stderr, and fixed `PROVED_BUILD_CACHE`. Both phases have process,
BuildFinished, target, and output counts one, zero persistent action-cache and
spawn errors, and the same 877-action digest multiset. Prime records 877
`linux-sandbox` cache misses; replay records 877 remote-cache hits and no local,
worker, sandbox, or other eligible runners. Afterward there are zero matching
output-base processes, reserved roots, or `slugd`, and Git is clean. No raw,
home, artifact, invocation, or service data was inspected. This proves only the
one build-label cache vertical.

Next implementation only
`WP-10-m8-bazel-buildbuddy-build-rbe-vertical-implementation`. Add a separate
stdlib RBE library/CLI/tests at 260/20/300 and 580 total lines. Use the exact-
hash checked-in managed Linux/amd64 profile, one fresh output base, disabled
cache reads/local-result uploads, top-level output materialization, private BEP/
execution logs, and all-SpawnExec structured classification. Reuse hardened
parsing/filesystem primitives without changing the accepted cache gate. Offline
tests and independent schema/privacy/lifecycle review only; no Bazel, network,
home RC, artifact, service, config, target, or fixture access/change.

The structured RBE driver is accepted in `e48213bb…` at 213 library, 17 CLI,
190 test, and 420 total lines. Six focused and 91 related tests pass with compile,
scope, cap, and diff gates. Independent review accepts the exact inherited-RC
command, strict all-SpawnExec schema/classification, bounded no-follow pinned-
file reads, private evidence/output anchoring, cleanup suppression, and canonical
CLI behavior. Pinned Bazel 9.2 source confirms the expanded execution-log JSON
printer includes no-presence scalar defaults, so successful SpawnExec status and
exit fields are explicitly empty and zero.

Next evidence only
`WP-10-m8-bazel-buildbuddy-build-rbe-vertical-transported-live-evidence`.
Invoke the frozen CLI exactly once through the accepted anonymous bounded outer
transport with a 4 KiB stdout cap, retained-session polling, and inherited
environment. Accept only outer/child zero, empty stderr, `DELIVERED`, exact
normalized `PROVED_BUILD_RBE`, and clean root/process/daemon/Git lifecycle. No
retry, raw/home/artifact/UI/service access, code/config edit, or 43-target claim.

The single invocation from clean `475c6f17…` delivers outer/child zero, empty
child stderr, and fixed `PROVED_BUILD_RBE`. Process, BuildFinished, target, and
output counts are one; all 877 SpawnExec records have valid digests and runner
`remote`, with zero cache hits, field errors, local/worker/sandbox/other runners,
or persistent action-cache hits. Three stale reserved roots were removed through
the identity-bound no-follow helper before invocation without reading contents.
Afterward there are zero matching roots, output-base processes, or `slugd`, and
Git is clean. No raw/home/artifact/UI/service data was inspected. This proves
only the one build-label managed-RBE vertical.

Next design only
`WP-10-m8-bazel-buildbuddy-full-gate-driver-reconciliation-design`. Preserve the
frozen one-build/43-green-test manifest and reconcile the older full-cache driver
with the accepted one-label command, parser, closed-schema, anchored-artifact,
and cleanup semantics. Define separate full-manifest cache and RBE implementations
and serialized evidence packets without widening either one-label proof. Do not
run the older driver unchanged. No code/config/manifest/test, Bazel, network,
home RC, artifact, or service access in the design.

The execution-only fixed-stage probe is accepted in `9b5c1180…` at 107 library,
17 CLI, 101 test, and 225 total lines. Its five focused and 45 related tests
pass; independent review accepts private dual-artifact setup, replacement-aware
execution reads, lazy first-failure order, no BEP/output access, and lifecycle.

Next evidence only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-execution-stage-probe-transported-live-evidence`.
Invoke the frozen CLI exactly once through the accepted anonymous transport and
retained-session polling with inherited environment. Review only the fixed
envelope, empty child stderr, and normalized stage. No retry, raw/home/artifact/
service access, code/config edit, or cache/RBE claim.

The transported execution invocation from clean `3235a511…` delivers
outer/child zero, empty child stderr, and fixed `EXECUTION_READY`; its private
root is absent afterward, Git is clean, and no `slugd` exists. No raw/home/
artifact/service data was inspected. Together with `BEP_READY`, this closes both
private parser boundaries but not output or aggregate prime semantics.

Next implementation only
`WP-10-m8-bazel-buildbuddy-build-cache-prime-output-semantics-probe-implementation`.
Add separate stdlib library/CLI/tests at 190/30/260 and 480 total lines. One
prime checks anchored output materialization before any BEP/execution read, then
reuses the accepted readers/parsers and opaque prime predicate. Emit only fixed
output/read/parse/semantics/ready stages. Offline tests and independent review
only; no Bazel/home RC/network/live artifact or existing-file edit.

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
