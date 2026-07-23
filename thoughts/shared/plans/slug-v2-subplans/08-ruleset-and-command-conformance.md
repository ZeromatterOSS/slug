# Stage 8: Ruleset and Command Conformance

## Goal

First prove Slug V2 exposes Bazel 9's loading, configured-target, and action
graphs through `query`, `cquery`, and `aquery`. After exact action-query parity,
prove modern rulesets and execution-oriented commands on those same graphs.

## Scope

- rules_cc, rules_rust, rules_python, protobuf, bazel_skylib, and rules_oci
  public smoke fixtures.
- `build`, `test`, `run`, `query`, `cquery`, and `aquery` command slices.
- complete Bazel 9 query grammar, function registries, target-pattern behavior,
  graph traversal, ordering, diagnostics, and command-specific output formats.
- BEP and event output needed by common integrations.
- diagnostics and exit-code compatibility where rulesets depend on them.

## Non-Goals

- Native language-rule fallbacks removed from Bazel 9.
- Android/iOS breadth before the core public rulesets are stable.
- Private workspace-specific fixtures as the only proof for a behavior.
- A separately invented Slug query language or a command-owned mock analysis
  graph.

## Current Priority: Query Command Gates

Implement new command work in this order:

1. `query` over the Stage 4/5 unconfigured loading graph;
2. `cquery` over the accepted Stage 6 configured-target DICE keys;
3. `aquery` over the exact actions retained by those analysis results;
4. only then broaden `build`, `run`, `test`, BEP, public ruleset execution, and
   cache behavior.

`aquery` is the formal Stage 6-to-Stage 7 handoff. The normalized
`ActionGraphContainer` for the gate matrix must match Bazel 9.2.0 before actual
execution/cache breadth becomes the project priority.

### Live packet: `WP-8-m3-tests-loading-metadata-gate-a`

Implement the Sol-accepted loading metadata and unconfigured-graph projection.
Cover typed inherited attrs, invariant-safe suite membership/provenance,
implicit finalization, capability/scalar metadata, label attributes, ordinary
edges, semantic equality, and lifecycle invalidation. Keep `tests()` inactive;
strict request plumbing and activation are later packets. Live Status in the
canonical plan owns scheduling.

### Query engine reuse policy

Do not grow the current `slug_query_v2` subset parser into a second query
engine without first extracting the mature generic machinery. Audit and prefer:

- Buck2 commit `088c75c7e36805df99c3de29062baa95db700b8b`:
  `../buck2/app/buck2_query_parser/src/lib.rs`,
  `../buck2/app/buck2_query/src/query/{environment.rs,graph.rs,traversal.rs}`,
  and `../buck2/app/buck2_query_impls/src/{uquery,cquery,aquery}/`;
- V1 archive crates `app/slug_query_parser`, `app/slug_query`,
  `app/slug_query_impls`, and `app/slug_cmd_query_server`; and
- V1 Bazel-compatibility test themes under
  `tests/core/query/test_bazel_compat_query.py`.

Reuse parser spans, generic evaluation, graph traversal, deterministic sets,
and separated uquery/cquery/aquery environments where they remain
Bazel-neutral. Replace Buck literals, cells, target patterns, functions,
attributes, configurations, actions, diagnostics, and printers with Bazel 9
semantics. Stage 9 records an explicit port/reference/reject decision before
implementation. The current seven-function parser and command placeholders are
scaffolding and may be removed.

## Implementation Slices

### 8.1 Public Ruleset Matrix

Pin one public fixture per ruleset:

| Ruleset | Minimum fixture | Required proof |
|---------|-----------------|----------------|
| rules_cc | `cc_library`, `cc_binary`, `cc_test` | toolchain, compile, link, run/test |
| rules_rust | `rust_library`, `rust_binary`, cargo build script when available | paramfiles, runfiles, toolchain |
| rules_python | `py_library`, `py_binary`, `py_test` | runfiles, imports, version switching |
| protobuf | `proto_library`, language-specific proto rule where practical | `ProtoInfo`, protoc action |
| bazel_skylib | common macros used by rulesets | loading and providers |
| rules_oci | minimal image or package flow | actions, tree artifacts, runfiles |

Each fixture must use modern Bazel-9-compatible versions and bzlmod.
Where local Bazel pins provide a useful baseline, start from
`src/MODULE.tools` versions such as `bazel_skylib`, `rules_cc`,
`rules_python`, and `protobuf`; add `rules_rust` and `rules_oci` as
Slug-owned locked fixtures with Bazel oracle output committed by the Stage 1
harness.

Initial fixture names:

- `rules-cc-basic`
- `rules-cc-run-env`
- `rules-cc-test-env-inherit`
- `rules-python-basic`
- `rules-python-runfiles`
- `rules-rust-basic`
- `protobuf-basic`
- `bazel-skylib-basic`
- `rules-oci-basic-no-daemon`

### 8.2 Command Surface

Implement command slices in this order:

1. `query`: full Bazel expression grammar, target patterns, set operations, and
   the Bazel 9 function registry including `allpaths`, `attr`, `buildfiles`,
   `deps`, `executables`, `filter`, `kind`, `labels`, `loadfiles`, `rdeps`,
   `same_pkg_direct_rdeps`, `siblings`, `some`, `somepath`, `tests`, and
   `visible`. Add Sky Query-only functions such as `allrdeps` and
   `rbuildfiles` only with their Sky Query universe semantics.
2. `cquery`: reuse the same evaluator over configured nodes, adding Bazel's
   configuration-aware functions/options, transitions, provider/Starlark
   output, and ambiguity/error behavior.
3. `aquery`: reuse the evaluator over Stage 6 actions, adding the Bazel action
   filters (`inputs`, `outputs`, `mnemonic`) and emitting `text`, `commands`,
   `summary`, `textproto`, `proto`, `streamed_proto`, and `jsonproto` from one
   IR. Match the Bazel 9.2.0 include-commandline/artifact/pruned-input/
   param-file/file-write flags and `skyframe_state` restrictions.
4. `build` with target patterns and output reporting, then `run` with executable
   target/runfiles and `test` with test results/exit semantics.
5. BEP JSON for accepted build/test integrations.

Initial modules:

- `app/slug_commands_v2/src/{build.rs,run.rs,test.rs,query.rs,cquery.rs,aquery.rs}`
- `app/slug_query_v2`
- `app/slug_bep_v2`

For each query command, derive the supported output-format matrix from Bazel
9.2.0 options/source and cover every accepted format plus invalid combinations.
Compare exit code, normalized stdout/stderr, output manifest, selected BEP
events, query output, cquery provider output, and aquery action graph. Missing
Stage 6 or Stage 7 semantics must stay expected-failing with explicit owner
backreferences; Stage 8 should not add local workarounds for analysis or
execution gaps.

### 8.3 Diagnostics and Compatibility Gates

- Version checks through `native.bazel_version` and `bazel_features` must report
  Bazel 9.
- Removed native language rules must fail in the same shape as Bazel 9.
- Unsupported flags should be classified as parse, ignored-compatible, or
  planned, never silently accepted as behavior.
- Output paths in command output should be Bazel-shaped, not V1 `buck-out`.

### 8.4 Stress and Regression Policy

- Public real-world projects are stress evidence only.
- Every discovered bug gets a focused repo-owned oracle fixture before broad
  smoke status is upgraded.
- Private or organization-specific target labels must not enter persistent
  tests or plans.
- `../llvm-project` is an optional complex-project stress corpus after it has a
  valid checkout and the focused gates pass. It is not acceptance evidence and
  was incomplete during the 2026-07-22 review.

## Exact Test Criteria

- `rules-cc-basic` builds and tests a public `cc_test`; compile/link actions
  run through REAPI when Stage 7 is enabled.
- `rules-cc-run-env` and `rules-cc-test-env-inherit` compare run/test
  environment behavior and test logs.
- `rules-rust-basic` builds and runs a `rust_binary`; paramfile and runfiles
  fixtures pass if cargo build scripts are in scope.
- `rules-python-basic` runs `py_test` and proves Starlark implementation path is
  selected rather than removed native fallback.
- `rules-python-runfiles` compares runfiles discovery and import behavior.
- `protobuf-basic` proves `hasattr(native, "proto_library")` behavior selects
  Bazel-9-compatible Starlark path and produces `ProtoInfo`.
- `bazel-skylib-basic` loads common macros used by public rulesets.
- `rules-oci-basic-no-daemon` builds a minimal image/package flow without
  relying on a background daemon.
- `query-parser-and-sets` ports Bazel `QueryParserTest` themes for precedence,
  parentheses, quoting, variables, set literals/operators, function arity,
  spans, and syntax diagnostics.
- `query-functions-and-patterns` ports focused `AbstractQueryTest` themes for
  the complete Bazel 9 function registry, target patterns, keep-going behavior,
  ordering, and command output formats.
- `query-basic` compares text and structured output for a small graph against
  Bazel and proves the command uses the loaded DICE graph rather than a fixture
  graph.
- `cquery-provider-starlark` compares configured identity, transitions,
  provider/Starlark output, and diagnostics using the Stage 6 graph.
- `aquery-action-shape` and an expanded action matrix compare normalized
  `ActionGraphContainer` facts plus all seven Bazel 9.2.0 formatter renderings:
  argv, environment, inputs, outputs/tree artifacts, dep sets, configurations,
  mnemonic, execution platform/properties, paramfiles, aspects, and toolchains
  where applicable.
- A structural regression proves `aquery` and Stage 7 receive the same action
  object/digest projection rather than independently rebuilding actions.
- `run-basic` executes a binary with runfiles and compares stdout/stderr.
- `test-basic` reports pass/fail and returns Bazel-compatible exit codes.
- `bep-minimal-build-test` emits configured target, action completed, test, and
  build finished events with stable ids.

## Acceptance Criteria

- Each supported ruleset has at least one public fixture pinned to a modern
  Bazel-9-compatible version.
- Command conformance fixtures compare against upstream Bazel through the oracle
  harness.
- Real-world stress projects supplement, but do not replace, repo-owned focused
  fixtures.
- `query`, then `cquery`, then exact `aquery` are accepted in that order over
  the shared loading/analysis graph before new execution-oriented command
  breadth is scheduled.
- The current subset parser and `planned_placeholder` command results have been
  replaced or deliberately retained only for unsupported, explicitly diagnosed
  cases.

## Validation

```bash
cargo test -p slug_commands_v2 -p slug_query_v2 -p slug_bep_v2
slug-v2-oracle run --fixture query-parser-and-sets
slug-v2-oracle run --fixture query-functions-and-patterns
slug-v2-oracle run --fixture rules-cc-basic --compare exit,outputs,bep,aquery
slug-v2-oracle run --fixture rules-cc-run-env --compare exit,stdout,stderr
slug-v2-oracle run --fixture rules-cc-test-env-inherit --compare exit,testlog,bep
slug-v2-oracle run --fixture rules-rust-basic --compare exit,outputs,bep
slug-v2-oracle run --fixture rules-python-basic --compare exit,outputs,runfiles,bep
slug-v2-oracle run --fixture rules-python-runfiles --compare exit,stdout,stderr,runfiles
slug-v2-oracle run --fixture protobuf-basic --compare exit,outputs,cquery
slug-v2-oracle run --fixture bazel-skylib-basic --compare exit,outputs
slug-v2-oracle run --fixture rules-oci-basic-no-daemon --compare exit,outputs
slug-v2-oracle run --fixture query-basic
slug-v2-oracle run --fixture cquery-provider-starlark --compare stdout,stderr
slug-v2-oracle run --fixture aquery-action-shape --compare stdout,stderr
slug-v2-oracle run --fixture run-basic
slug-v2-oracle run --fixture test-basic
slug-v2-oracle run --fixture bep-minimal-build-test --compare bep
```

## Checkpoint Evidence

### Command Surface Substrate

- Added `slug_commands_v2`, `slug_query_v2`, and `slug_bep_v2` as Stage 8
  substrates. The command crate parses `build`, `run`, `test`, `query`,
  `cquery`, and `aquery` request shapes, classifies flags as parse-only,
  ignored-compatible, or planned, and returns stage-owned placeholder errors
  rather than inventing analysis or execution workarounds.
- Wired the top-level V2 CLI help and dispatch for `cquery` and `aquery` to
  structured planned placeholders, keeping the command surface visible without
  claiming configured-target or action-graph evaluation yet.
- Connected the top-level V2 CLI command modules to the `slug_commands_v2`
  parsers before placeholder emission. The CLI now reports structured
  `command_parse_error` diagnostics and preserves argv in
  `planned_placeholder` output while real evaluation remains owned by later
  loading, analysis, and REAPI slices.
- Added a query expression parser for `deps`, `rdeps`, `kind`, `attr`,
  `filter`, `buildfiles`, and `tests`. Bazel 9's JSON query output spelling is
  `streamed_jsonproto`; the fixture and parser tests use that spelling.
- Added a BEP JSON-lines subset with stable IDs for build started, configured
  target, action completed, test result, and build finished events.
- Added command conformance fixtures `query-basic`, `run-basic`, `test-basic`,
  `cquery-provider-starlark`, `aquery-action-shape`, and
  `bep-minimal-build-test`. The fixtures use `MODULE.bazel` and Starlark-defined
  rules instead of removed native shell-language rules.
- Generated Bazel 9.1.1 expected oracle output for all six command-surface
  fixtures. `bep-minimal-build-test` currently compares message shape because
  raw BEP file digests are intentionally not stable between Bazel runs; add a
  BEP-aware comparator before upgrading it to event-field comparison.
- `run-basic` proves target execution and stdout comparison. Passthrough arg
  preservation is pinned in `slug_commands_v2` unit tests; full runfiles and
  platform-specific argv parity remain follow-up work once the command runner is
  connected to Stage 7 materialization.

2026-07-22 qualification: this section records parser/CLI scaffolding and
Bazel-side fixture generation, not a query implementation. `query`, `cquery`,
and `aquery` currently return planned placeholders because the package,
configured-target, and action graphs are not wired. The current subset parser
lacks Bazel's complete grammar/functions and should be replaced or refactored
through the reuse policy above before adding more ad hoc cases.

Validation run:

```bash
cargo fmt -p slug_cli_v2 -p slug_commands_v2 -p slug_query_v2 -p slug_core_v2
CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_cli_v2 -p slug_commands_v2 -p slug_query_v2 -p slug_core_v2
cargo fmt -p slug_commands_v2 -p slug_query_v2 -p slug_bep_v2
CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_commands_v2 -p slug_query_v2 -p slug_bep_v2
py -3 -B -m tools.v2_oracle list
py -3 -B -m tools.v2_oracle run --fixture query-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
py -3 -B -m tools.v2_oracle run --fixture cquery-provider-starlark --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
py -3 -B -m tools.v2_oracle run --fixture aquery-action-shape --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
py -3 -B -m tools.v2_oracle run --fixture run-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
py -3 -B -m tools.v2_oracle run --fixture test-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
py -3 -B -m tools.v2_oracle run --fixture bep-minimal-build-test --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
python -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py
rg -n "buck-out|BUCK|Buck2|CellResolver|direct-local" app/slug_commands_v2 app/slug_query_v2 app/slug_bep_v2 tests/v2_oracle/fixtures/{query-basic,run-basic,test-basic,aquery-action-shape,cquery-provider-starlark,bep-minimal-build-test}
```

Skipped from the full Stage 8 validation matrix in this checkpoint:

- Slug-side oracle runs for the command and public-ruleset fixtures are pending
  command runner wiring to loading, analysis, REAPI execution, runfiles, and
  BEP emission.
- Full `rules_oci` image execution is still a follow-up because upstream
  Bazel/rules_oci fails on this Windows host before the daemon boundary; keep
  the landed `rules-oci-basic-no-daemon` fixture as action-graph evidence until
  a Linux-backed oracle or upstream wrapper fix is available.

### Command Bzlmod Policy Bridge

- Added command-surface extraction for `--allow_yanked_versions`,
  `--ignore_dev_dependency`, and `--noignore_dev_dependency` in
  `slug_commands_v2`. Build, run, test, query, cquery, and aquery request
  parsing now carries the Stage 5 `BzlmodCommandPolicyKey` before placeholder
  execution, so later command-runner wiring can feed bzlmod graph keys without
  process-global command state.
- The new parser path classifies these flags as parse-only, preserves the
  original argv for placeholder diagnostics, and reports structured parse
  errors for invalid yanked-version allowlists or boolean values.

Validation run:

```bash
cargo fmt -p slug_commands_v2 -p slug_cli_v2
CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_commands_v2
CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_cli_v2
USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-root-dev-dependency-visibility --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120
USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-registration-dev-dependency --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120
python -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py
```

### Command Lockfile Mode Bridge

- Added command-surface extraction for `--lockfile_mode` in
  `slug_commands_v2`. Build, run, test, query, cquery, and aquery request
  parsing now carries the Stage 5 `LockfileMode`, defaulting to `update` and
  rejecting invalid values with Bazel-shaped diagnostics while actual lockfile
  read/write behavior remains owned by the bzlmod graph and lockfile planner.

Validation run:

```bash
cargo fmt -p slug_commands_v2 -p slug_cli_v2
CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_commands_v2
CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_cli_v2
USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-flag-validation --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120
USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-update-refresh --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120
```

### Public Ruleset Fixture Start

- Added Bazel 9 oracle fixtures for `rules-cc-basic`, `rules-cc-run-env`,
  `rules-cc-test-env-inherit`, `bazel-skylib-basic`, `rules-python-basic`,
  `rules-python-runfiles`, `protobuf-basic`, `rules-rust-basic`, and
  `rules-oci-basic-no-daemon`.
- `rules-cc-basic` covers `cc_library`, `cc_binary`, and `cc_test` with
  `rules_cc` loaded from `@rules_cc//cc:defs.bzl`; it was expanded from the
  Plan34 `rules_cc` fixture theme and updated to BCR-resolved module versions
  observed under Bazel 9.1.1 (`rules_cc` 0.2.17, `bazel_features` 1.42.1,
  `bazel_skylib` 1.8.2, `platforms` 1.0.0, `zlib` 1.3.1.bcr.5).
- `rules-cc-run-env` covers Bazel-provided `bazel run` workspace and runfiles
  environment markers for a `cc_binary`.
- `rules-cc-test-env-inherit` covers `cc_test` test-log output for stable
  `TEST_TMPDIR`, `TEST_SRCDIR`, and `TEST_WORKSPACE` markers.
- `bazel-skylib-basic` covers the `copy_file` rule from `bazel_skylib` 1.8.2.
- `rules-python-basic` covers `py_library`, `py_binary`, and `py_test` with
  stable `rules_python` 2.0.3 and an explicit Python 3.12 toolchain extension.
- `rules-python-runfiles` covers `py_binary` imports plus data lookup through
  Bazel runfiles manifest/directory environment under the same rules_python
  2.0.3 and Python 3.12 toolchain path.
- `protobuf-basic` covers `proto_library` loaded from
  `@protobuf//bazel:proto_library.bzl` in protobuf 35.1. Its cquery output
  contains the Starlark `ProtoInfo` provider from
  `@protobuf//bazel/private:proto_info.bzl`, and its aquery output contains
  the `GenProtoDescriptorSet` action plus prebuilt `protoc` invocation.
- `rules-rust-basic` covers `rust_library`, `rust_binary`, and `rust_test`
  using the `rules_rust` 0.71.1 bzlmod extension with an explicit Rust 1.96.0
  toolchain. Its aquery summary records the `Rustc`, `RunfilesTree`, and
  `SymlinkTree` actions without committing the full platform-specific sysroot
  input list.
- `rules-oci-basic-no-daemon` covers the `rules_oci` 2.3.0 bzlmod load path
  and records daemonless `OCIImage` plus `OCITarball` action graph shape via
  aquery. Full image execution remains pending on a Linux-backed oracle because
  upstream Bazel/rules_oci fails on this Windows host before the daemon boundary
  when the generated shell wrapper loses JSON quoting.
- Bazel 9.1.1 expected oracle output was generated for all nine fixtures.
  These fixtures currently use message-shape comparison where platform-specific
  output manifests or Python runtime runfiles would otherwise make expectations
  noisy. Upgrade to output/runfiles comparison once the oracle manifest layer is
  platform-aware.
- Checked BCR metadata and resolved Starlark APIs for rules_oci 2.3.0. The
  action-graph fixture is landed; the full no-daemon image/package build proof
  remains a focused follow-up that needs a Linux-backed oracle or an upstream
  Windows wrapper fix.

Validation run:

```bash
py -3 -B -m tools.v2_oracle list
py -3 -B -m tools.v2_oracle run --fixture rules-cc-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
py -3 -B -m tools.v2_oracle run --fixture rules-cc-run-env --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
py -3 -B -m tools.v2_oracle run --fixture rules-cc-test-env-inherit --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
py -3 -B -m tools.v2_oracle run --fixture bazel-skylib-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
py -3 -B -m tools.v2_oracle run --fixture rules-python-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
py -3 -B -m tools.v2_oracle run --fixture rules-python-runfiles --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
py -3 -B -m tools.v2_oracle run --fixture protobuf-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
py -3 -B -m tools.v2_oracle run --fixture rules-rust-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
py -3 -B -m tools.v2_oracle run --fixture rules-oci-basic-no-daemon --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
```

### Reviewed next packet — `WP-8-m3-loading-query-thin-vertical` (2026-07-22)

Work packet ID: `WP-8-m3-loading-query-thin-vertical`

Owner stage and plan: Stage 8,
`thoughts/shared/plans/slug-v2-subplans/08-ruleset-and-command-conformance.md`.

Goal and gate link: land the first honest M3 integration vertical by replacing
the ad hoc seven-function parser with reusable generic query machinery and
driving it through a DICE-owned root-repository loading graph from the real
`query` CLI. This packet is not “full query”; M3 remains open until the complete
Bazel 9 registry, patterns, ordering, diagnostics, and formatter matrix pass.

Prerequisites and current state:

- implementation commit `4f4599e0` and evidence commit `34959d5e` provide the
  retained workspace transaction, `PackageLoadKey`, directory observations,
  and configured analysis graph; unconfigured query must consume loading, not
  configured analysis;
- `slug_query_v2::expr` currently hard-codes seven function names, rejects
  functions during parsing, and lacks spans, `let`, parentheses, `set`,
  integers, and binary operators;
- `QueryRequest` and the CLI return a planned placeholder;
- `LoadedPackage` is a declaration record, not a complete unconfigured query
  graph: filegroup/alias edges are raw strings and implicit source-file nodes
  are absent; and
- `query-basic` is a stale Bazel 9.1.1 Windows capture that requests
  `streamed_jsonproto`. It is not Bazel-9.2 M3 acceptance evidence.

Oracle-first artifacts:

1. Create or refresh `query-parser-and-sets` with Bazel 9.2.0 CLI commands that
   make quoting, `let` binding, parentheses, set/operator precedence,
   implemented-function arity, syntax failures, unknown functions, duplicate
   elimination, and accepted text ordering externally observable.
2. Create `query-loading-thin-vertical` with root packages containing
   filegroups, an alias, exported and implicit source-file labels, a custom
   rule with `deps`, nested dependency edges, and a package subtree. Cover
   literal labels, `//pkg:all`, `//...`, alias traversal, `deps`, accepted set
   operators, missing packages/targets, and cycles.
3. Generate and independently rerun both with `/usr/bin/bazel` 9.2.0 at
   immutable source commit
   `8220c6198837d5c13d53fea211cf3282aa12408a`. Use ordinary Bazel RC
   discovery so the user's external BuildBuddy configuration may accelerate
   the commands; never read, copy, log, or commit `~/.bazelrc` or credentials.
4. Bazel CLI does not expose its parsed AST or source-span structure. Port
   precise parser-shape/span cases as Rust unit tests with citations to
   `QueryParser.java` and `QueryParserTest.java`; do not describe those unit
   tests as generated oracle output. Known-but-deferred Bazel functions get
   source-derived registry/diagnostic tests and a residual entry, not a
   falsely passing Bazel-versus-Slug comparison.

Reuse audit and approved decisions:

- port parser spans, generic expression AST, `set`, operator grammar, and
  parser mechanics from Buck2 commit
  `088c75c7e36805df99c3de29062baa95db700b8b`
  `app/buck2_query_parser/src/{lib.rs,span.rs,spanned.rs,multi_query.rs}`;
  replace the error surface and reconcile grammar/precedence with Bazel 9.2;
- selectively port the generic environment, target set, graph, and traversal
  machinery from
  `app/buck2_query/src/query/{environment.rs,graph.rs,traversal.rs}` and its
  `syntax/simple` evaluator. Preserve generic evaluation and compact,
  deterministic traversal shapes; replace Buck attributes, files, functions,
  and printers;
- adapt only the DICE literal pre-resolution/environment separation lesson
  from `app/buck2_query_impls/src/uquery/{environment.rs,evaluator.rs}`;
  reject cells, cell resolvers, Buck labels/patterns, target graphs, package
  semantics, registries, diagnostics, and output rendering;
- inspect V1 `e218054d4c796655939b968d90208b185decb352`
  `app/{slug_query_parser,slug_query,slug_query_impls,slug_cmd_query_server}`
  as same-lineage reference material. Extract scenarios from
  `tests/core/query/test_bazel_compat_query.py`, then regenerate all
  expectations with Bazel 9.2; do not import V1 server/process context,
  cells, labels, configured/action nodes, or printers; and
- reuse `SmallMap`, `SmallSet`, `SortedMap` or an explicitly justified
  Buck2-derived fast-hash traversal set, immutable shared slices, `Dupe`, and
  `Allocative`. Do not introduce a string-heavy
  `HashMap<String, Vec<String>>` graph.

Sol-low approved the combined vertical after rejecting both a disconnected
parser-only scaffold and an environment built around the current invented AST.
It required the structural graph, exact transaction boundary, observable
oracle split, complete event multisets, and explicit thin-vertical wording.

Reviewed architecture and exact scope:

1. Replace `app/slug_query_v2/src/expr.rs` with a generic spanned parser,
   registry-driven evaluator, compact target set, and traversal substrate.
   Parsing accepts generic calls. A V2-owned complete Bazel 9.2 loading-query
   registry then distinguishes unknown functions, known-but-deferred
   functions, and this packet's implemented functions while validating arity
   and argument kinds before evaluation.
2. Implement this packet's expression slice only: target literals,
   `let name = expression in expression`, parentheses, `set(...)`, `deps`, and
   Bazel-confirmed `union`/`+`, `except`/`-`, and `intersect`/`^`. A known
   deferred function fails explicitly; it is never silently accepted.
3. Add a demand-driven
   `UnconfiguredPackageGraphKey { workspace, package }`. It consumes exactly
   one `PackageLoadKey` and produces structural canonical-label nodes for that
   package. Literal, package-all, and dependency traversal compute package
   graph keys only as needed. Add a separate
   `RootPackageSetKey { workspace }` that recursively consumes
   `WorkspaceDirectoryKey` only for `//...` and future universe-wide
   operations, returning compact package identities before those packages are
   loaded. No monolithic workspace graph, query key, or evaluator reads/scans
   the filesystem.
4. Nodes distinguish rule-like targets from source files and retain
   query-visible kind, build-file identity, and normalized outgoing edges.
   Custom-rule `deps` use the existing canonical ordered labels; alias keeps
   its own node and one normalized `actual` edge; filegroup normalizes each
   `srcs` entry; file paths create explicit source-file nodes with no outgoing
   edges. Graph storage uses compact V2-owned values. A request-local compact
   visited/result set is traversal state, never a competing semantic cache.
5. Evaluate the graph inside the same committed `WorkspaceRuntime`
   transaction that injected file/directory observations. The CLI must not
   create a second DICE instance, cache, fixture graph, or filesystem view.
   Extend `slug_server_v2` only as required for this ownership: replace the
   build-only wire shape with tagged `DaemonRequest::{Build, Query}` and one
   common response envelope. Preserve all Build fields/behavior. Query sends
   the raw expression and accepted order mode; authoritative parsing,
   registry validation, evaluation, and diagnostics occur in the retained
   daemon/runtime, not the CLI. `Daemon::query` reuses the existing observation
   adapter and `WorkspaceRuntime`.
6. Wire text `query` only. Accept default/`--order_output=auto` and `full` only
   when the oracle establishes their exact behavior. Reject `deps`, `no`,
   structured formats, unsupported flag combinations, and deferred functions
   until separately implemented and oracle-pinned. Leave `cquery` and
   `aquery` placeholders untouched.

Target-pattern behavior is provisional until oracle generation:

- `//pkg:target` resolves one declared rule/alias or addressable source-file
  node and reports Bazel-shaped missing package/target failures;
- `//pkg:all` expands rule-like targets only and is not silently treated as
  `:*`;
- `//...` expands rule-like targets recursively in root packages; dependency
  traversal may then reach source-file nodes;
- `deps(x)` includes `x` and its transitive closure; alias remains visible and
  traverses its `actual`; and
- accepted set operations use Bazel precedence/associativity and deterministic
  duplicate elimination.

If Bazel 9.2 disagrees about `:all`, `//...`, implicit/explicit source nodes,
alias traversal, diagnostics, cycles, or ordering, stop and revise this packet
before Rust implementation.

Implementation steps:

1. Generate and independently rerun the two Bazel 9.2 fixtures; replace or
   explicitly supersede stale `query-basic` evidence.
2. Port the generic parser/span/set/evaluator/traversal substrate and add
   source-cited parser/registry tests.
3. Add the demand-driven DICE package-graph key, recursive-only package-set
   key, structural nodes, normalized edges, and focused ownership/pattern
   tests.
4. Add the retained-transaction query entry point, tagged daemon request,
   command evaluation, and text renderer; remove the query placeholder only
   for the accepted matrix. Add protocol regressions proving existing build
   requests are unchanged and `--output_base` queries reuse daemon state. A
   fresh-runtime convenience path may call the identical runtime entry point,
   but it is not same-daemon evidence.
5. Add exact `ActivationTracker` multiset regressions: initial evaluation;
   zero activation on an identical revision; no package-graph evaluation for
   an unrelated package BUILD edit during a literal `deps()` query (the
   retained global observation revision may honestly emit `Reused` validation
   callbacks, as in the accepted Stage 6 evidence); deliberate package-set
   validation and affected package evaluation for `//...`; BUILD
   target/edge edit; package create/delete/recreate via directory inputs; and
   dependency changes reflected by `deps` without restarting the runtime.
6. Run the affected/downstream suite serially and obtain Sol-low
   post-validation review before recording completion evidence.

Focused validation:

```bash
CARGO_TARGET_DIR=/tmp/slug-m3-query-target CARGO_BUILD_JOBS=1 cargo test \
  -p slug_query_v2 -p slug_loading_v2 -p slug_core_v2 \
  -p slug_commands_v2 -p slug_server_v2 -p slug_cli_v2
python3 -B -m tools.v2_oracle run --fixture query-parser-and-sets \
  --tool bazel --bazel /usr/bin/bazel
python3 -B -m tools.v2_oracle run --fixture query-loading-thin-vertical \
  --tool bazel --bazel /usr/bin/bazel
cargo fmt --all -- --check
git diff --check
```

Also grep for direct filesystem access in query keys/evaluation, extra DICE or
runtime creation, command-local graphs, configured/action query imports,
default hash collections/string-heavy graph state, blocking bridges, and
locks across DICE work.

The daemon protocol extension is deliberately not a general command bus:
exclude cquery/aquery variants, protocol negotiation, streaming, execution,
and unrelated request metadata.

Evidence and plan update: land oracle evidence first. After implementation
acceptance, record exact commits, Bazel provenance, externally observed
semantics, AST-test source citations, activation multisets, reuse decisions,
validation, and residual functions/formats here, in Stage 1, Stage 9, and the
orchestration routing log.

Stop conditions: dirty overlap; non-generated or non-9.2 oracle evidence; any
oracle disagreement named above; direct filesystem/package scanning in a
query key; inability to evaluate in the retained committed transaction; a need
for external repositories, repository mapping, arbitrary Starlark attributes,
Sky Query universe behavior, configured identities/transitions/providers,
action nodes/formatters, or execution; or pressure to label this packet “full
query.” Stop at the first deferred function or format rather than inventing
semantics.

#### Oracle evidence landed (2026-07-22)

Oracle commit `7e8993b2` added generated Bazel 9.2.0
`query-parser-and-sets` and `query-loading-thin-vertical` fixtures at immutable
source commit `8220c6198837d5c13d53fea211cf3282aa12408a`.

Externally observed parser/expression facts:

- quoted literals, `let`, parentheses, `set`, word and symbolic set operators,
  and duplicate elimination execute through the Bazel CLI;
- wrong `deps` arity, syntax failure, and unknown functions exit 2 with stable
  distinct diagnostics;
- default and `--order_output=auto` render
  `bin, data.txt, lib`, while `--order_output=full` renders dependency order
  `bin, lib, data.txt`; and
- the fixture claims only CLI-visible results/errors. AST shape and spans
  remain source-cited Rust unit-test obligations.

Externally observed loading-graph facts:

- `//lib:all` and `//...` include rule-like targets but exclude source files;
- explicit, implicit, and attribute-created source labels resolve literally.
  `//lib:missing_input.txt` has no backing file, but its `filegroup.srcs`
  reference creates a query-addressable node;
- aliases remain result nodes and `deps(alias)` follows the actual edge;
  custom-rule `attr.label_list` edges cross packages and reach source nodes;
- a filegroup cycle is accepted structurally and terminates with both nodes;
  and
- missing target/package queries exit 7 with their distinct Bazel diagnostics.

Generation and independent no-update reruns passed for both fixtures using
`/usr/bin/bazel` 9.2.0. Fixture discovery, immutable provenance,
`generated: true`, exact stdout/stderr pattern coverage, whitespace checks, and
candidate credential scans passed. Bazel was allowed ordinary RC discovery,
including the user's `~/.bazelrc`; no agent or inspection tool read its
contents, and no external RC or credential content was copied, logged into
project files, or committed. Sol-low requested the deliberately absent
source-label case, then returned `ACCEPT` after regeneration and an independent
root rerun.

#### Implementation evidence landed (2026-07-23)

Implementation commit `61ca25db` completes
`WP-8-m3-loading-query-thin-vertical`. It remains a thin vertical, not full
loading-query parity or M3 completion.

Reuse and ownership:

- `slug_query_v2::parser` ports Buck2's borrowed `Span`, `nom` combinators,
  `spanned()` locations, generic-call parsing, and non-recursive
  `BinaryOpSequence` from
  `app/buck2_query_parser/src/{lib.rs,span.rs,spanned.rs}`. Bazel `let`,
  Bazel-owned diagnostics, and compact owned lowering are the V2 deltas.
- The evaluator ports the `QueryFunctions`/`QueryFunction::invoke` and typed
  required/optional argument seams from Buck2
  `query/syntax/simple/{functions.rs,functions/helpers.rs,eval/evaluator.rs}`.
  The complete 16-function Bazel 9.2 loading registry is V2-owned; only
  `deps` is callable and the other 15 functions fail explicitly.
- Depth-limited traversal adapts Buck2
  `query/{traversal.rs,graph/visited.rs}`. It retains ordered compact visited
  state but resolves serially because the loading environment owns mutable
  `DiceComputations`.
- `UnconfiguredPackageGraphKey` consumes one `PackageLoadKey`;
  `RootPackageSetKey` consumes recursive `WorkspaceDirectoryKey` values only
  for `//...`. Query keys/evaluation perform no direct filesystem reads and
  create no second DICE graph, runtime, semantic cache, or command-local
  graph. Nodes and traversal use canonical labels, `SmallMap`, `SmallSet`,
  `Arc` slices, `CompactString`, `Dupe`, and `Allocative`.

Integrated behavior:

- Root-repository literals, rule-only `:all`/`//...`, explicit, implicit, and
  absent attribute-created sources, aliases, custom-rule label-list edges,
  cycles, `let`, `set`, the accepted binary operators, and depth-limited
  `deps` match the two Bazel 9.2 oracle fixtures. Auto/default output sorts
  canonical labels structurally before rendering; full output retains
  traversal order.
- CLI policy admits only text output, `auto`/`full` order, and output-base
  routing. Every other flag or missing value fails instead of being silently
  discarded. `cquery` and `aquery` remain unchanged placeholders.
- The wire protocol is limited to tagged `Build`/`Query` requests and one
  response envelope. Query sends only the raw expression and order;
  authoritative parsing/evaluation remains in the retained
  `WorkspaceRuntime` transaction. Build fields and response behavior have
  round-trip regressions, and an output-base CLI regression proves the daemon
  PID survives a BUILD dependency edit.

Exact query-key activation evidence:

- initial `deps(//app:bin)`: `app Evaluated`, `lib Evaluated`;
- identical revision: no events;
- unrelated BUILD edit: `app Reused`, `lib Reused`, with no package-graph
  evaluation;
- first `//...`: `unrelated Evaluated`, `RootPackageSet Evaluated`;
- recursive package create/recreate: `app Reused`, `dynamic Evaluated`,
  `lib Reused`, `unrelated Reused`, `RootPackageSet Evaluated`;
- recursive package delete: `app Reused`, `lib Reused`,
  `unrelated Reused`, `RootPackageSet Evaluated`; and
- affected BUILD changes and literal missing/create/delete/recreate transitions
  evaluate only the requested affected package graph.

Validation:

- the serial six-crate suite passed 67 tests under
  `CARGO_TARGET_DIR=/tmp/slug-m3-query-target CARGO_BUILD_JOBS=1`;
- `cargo build -p slug_cli_v2`, `cargo fmt --all -- --check`, and
  `git diff --check` passed;
- both `query-parser-and-sets` and `query-loading-thin-vertical` passed through
  `tools.v2_oracle --tool slug` against the rebuilt V2 binary; and
- root reran the full suite and both Slug oracle fixtures after the final
  corrections. Sol-low's final post-implementation review returned `ACCEPT`.

Residual scope remains explicit: external repositories and repository mapping,
the other 15 loading functions, broader target patterns and Sky Query,
non-text formats and remaining ordering modes, configured/action environments,
and `cquery`/`aquery` are not implemented by this packet.

### Reviewed next packet — `WP-8-m3-reverse-deps-subtree-patterns` (2026-07-23)

Work packet ID: `WP-8-m3-reverse-deps-subtree-patterns`

Owner stage and plan: Stage 8,
`thoughts/shared/plans/slug-v2-subplans/08-ruleset-and-command-conformance.md`.

Goal and gate link: extend the landed loading-query graph with one coherent
reverse-dependency vertical: root-repository subtree patterns `//pkg/...`,
`rdeps(universe, from[, depth])`, and
`same_pkg_direct_rdeps(expression)`. This packet remains text-only with
`--order_output=auto|full`; it does not complete `query`.

Prerequisites and current state:

- oracle `7e8993b2` and implementation `61ca25db` provide the generated Bazel
  9.2 loading-query baseline, generic callable registry, compact structural
  nodes, demand-driven package graph, retained DICE transaction, and root
  `//...`;
- `TargetPattern::Recursive` already represents `//pkg/...`, but loading query
  currently rejects every recursive pattern except root `//...`;
- the Bazel 9.2 registry already describes `rdeps` and
  `same_pkg_direct_rdeps` with the correct argument shapes, but both are
  explicitly deferred; and
- the implementation has only forward dependency lookup. The new reverse
  graph is request-local derived traversal state over immutable DICE-owned
  package nodes, not a new semantic cache or DICE key.

Oracle-first artifact:

1. Add `tests/v2_oracle/fixtures/query-rdeps-and-subtree-patterns/` with root
   packages under an existing package prefix, a non-package prefix, nested
   packages, a missing/empty subtree, packages outside the subtree, aliases,
   custom label-list rules, source labels, multiple parents, multiple universe
   roots, duplicate seeds, and a cycle.
2. Generate and independently rerun the fixture with `/usr/bin/bazel` 9.2.0
   and cite immutable Bazel source commit
   `8220c6198837d5c13d53fea211cf3282aa12408a`. Bazel may use ordinary RC
   discovery and the user's external BuildBuddy configuration; no agent or
   inspection tool may read, copy, print, log, or commit `~/.bazelrc` or any
   credential.
3. Pin exact exit/stdout/stderr behavior for:
   - `//pkg/...` under an existing package subtree, a non-package prefix with
     descendant packages, nested-package inclusion, rule-only expansion, and
     an empty or missing subtree;
   - unbounded `rdeps`, depth zero, one, and greater depth, a seed outside the
     universe closure, multiple universe roots, duplicate seeds, cycles,
     rule/source seeds, aliases, custom-rule edges, and an empty result;
   - `same_pkg_direct_rdeps` with a source input, duplicate input, several
     direct parents, cross-package parent exclusion, alias/custom-rule parents,
     and the Bazel criss-cross case where inputs from two packages must not
     admit a parent through the other package's input; and
   - default, `auto`, and `full` ordering plus wrong arity/type diagnostics for
     both functions.
4. Do not prescribe traversal output from memory. Treat Bazel's generated
   default/auto/full output as the authority, especially for cycles, multiple
   roots, and reverse traversal.

Reuse audit and approved decisions:

- directly adapt Buck2 commit
  `088c75c7e36805df99c3de29062baa95db700b8b`
  `app/buck2_query/src/query/graph/graph.rs`,
  `query/environment.rs`, and
  `query/syntax/simple/functions/deps.rs` for stable forward-graph
  construction, reversal, bounded traversal, and generic function invocation;
- preserve the existing serial DICE lookup adaptation where mutable
  `DiceComputations` prevents Buck2's concurrent lookup. Do not invent a
  string-keyed reverse adjacency or add a monolithic reverse-graph DICE cache;
- use V1
  `e218054d4c796655939b968d90208b185decb352`
  `tests/core/query/test_bazel_compat_query.py` only as scenario inventory.
  V1 cells, labels, graph/server context, printers, and expected output remain
  rejected; and
- Bazel 9.2 source anchors are
  `query2/engine/{RdepsFunction,SamePkgDirectRdepsFunction}.java`,
  `cmdline/TargetPattern.java`, `skyframe/TargetPatternFunction.java`, and
  focused `AbstractQueryTest` themes.

Required implementation:

1. Replace the root-only enumeration identity with
   `SubtreePackageSetKey { workspace, prefix }`. It starts at
   `workspace/prefix` and consumes only descendant `WorkspaceDirectoryKey`
   values. Root `//...` is the empty-prefix specialization. Computing the
   root package set and filtering it for `//pkg/...` is forbidden because it
   creates a false whole-workspace dependency.
2. Resolve root-repository `TargetPattern::Recursive` through that key and
   preserve Bazel's rule-only pattern expansion and exact missing-subtree
   behavior. External repository patterns remain explicit deferred errors.
3. Implement `rdeps` through the existing generic callable registry. Evaluate
   the universe expression, build the forward transitive closure of those
   roots, reverse that graph request-locally, exclude seeds outside that
   closure, and traverse from the remaining seeds with Bazel's oracle-pinned
   depth/cycle/order behavior.
4. Implement `same_pkg_direct_rdeps` without workspace enumeration. Evaluate
   and group exact input labels by package, compute only those packages'
   `UnconfiguredPackageGraphKey`s, and scan direct edges in each package. A
   candidate qualifies only when it shares the package of the specific input
   edge it matches. Do not expose or implement `siblings` as a workaround.
5. Transition only these two registry entries to implemented. Add narrow
   parser/registry, evaluator, loading-query, command, daemon, and CLI tests
   without changing the existing build protocol or the `cquery`/`aquery`
   placeholders.

DICE acceptance evidence must assert complete relevant activation multisets:

- identical and unrelated package edits validate/reuse but do not evaluate
  package graphs for an unchanged subtree query;
- edits outside the subtree do not evaluate `SubtreePackageSetKey`;
- package create/delete/recreate inside the subtree evaluates the subtree key,
  while the same transitions outside remain irrelevant;
- `rdeps` invalidates when a universe member's edge changes and when its
  forward closure gains or loses a package; and
- `same_pkg_direct_rdeps` evaluates only operand packages and ignores edits in
  cross-package reverse dependents.

Focused validation:

```bash
CARGO_TARGET_DIR=/tmp/slug-m3-rdeps-target CARGO_BUILD_JOBS=1 cargo test \
  -p slug_query_v2 -p slug_loading_v2 -p slug_core_v2 \
  -p slug_commands_v2 -p slug_server_v2 -p slug_cli_v2
cargo build -p slug_cli_v2
python3 -B -m tools.v2_oracle run \
  --fixture query-rdeps-and-subtree-patterns --tool bazel \
  --bazel /usr/bin/bazel
python3 -B -m tools.v2_oracle run \
  --fixture query-rdeps-and-subtree-patterns --tool slug
cargo fmt --all -- --check
git diff --check
```

Also inspect the accepted diff for direct filesystem access in query
keys/evaluation, extra DICE/runtime/cache creation, root-package-set filtering,
string-keyed graph state, default hash collections or avoidable string churn,
configured/action imports, locks across DICE work, and silent command-option
acceptance.

Evidence and completion boundary: land and review generated oracle evidence
before Rust changes. Require Sol-low approval of the Buck2 port shape before
implementation and a complete post-validation review before recording
implementation evidence. Record exact commits, Bazel provenance, output/depth
semantics, activation multisets, residuals, and routing observations here, in
Stage 1, Stage 9, and the orchestration log.

Sol-low accepted this revised architecture after requiring the prefix-local
enumeration key, forward-universe closure, direct Buck2 graph/reversal port,
edge-specific criss-cross filtering, and complete invalidation matrix.

Stop conditions: dirty overlap; non-generated or non-9.2 oracle data; any need
for configured/generated/toolchain edges, external repository mapping, Sky
Query universe flags, arbitrary retained Starlark attributes, a persistent
reverse-graph cache, or whole-workspace enumeration for a subtree/package-local
operation; inability to execute through the retained DICE transaction; or an
oracle ordering/diagnostic mismatch that cannot be represented without
expanding this packet. Defer `siblings`, `kind`, `filter`, `attr`, `labels`,
`buildfiles`, `loadfiles`, `some`, path functions, tests/visibility functions,
non-text formats, remaining order modes, `cquery`, and `aquery`.

#### Oracle evidence landed (2026-07-23)

Oracle commit `5b7806d7` adds the generated Bazel 9.2.0
`query-rdeps-and-subtree-patterns` fixture at immutable source commit
`8220c6198837d5c13d53fea211cf3282aa12408a`.

The 26-command matrix establishes:

- existing, nested, and non-package-prefix subtree expansion includes rules
  but not sources; empty and absent prefixes both exit 7 with
  `no targets found beneath '<prefix>'`;
- `rdeps` is restricted to the forward universe closure, excludes seeds
  outside it, includes an in-universe seed at depth zero, adds direct reverse
  parents at depth one, handles greater depth and cycles, eliminates duplicate
  seeds, and follows source/rule/alias/custom edges;
- default and `auto` sort the focused reverse result as
  `custom_parent, leaf, via_alias`, while `full` renders
  `custom_parent, via_alias, leaf`;
- `same_pkg_direct_rdeps` returns only direct parents whose matching edge and
  operand share a package, including the two-package criss-cross exclusion;
  and
- too many/few arguments exit 2, while an integer in an expression position
  is parsed as literal target `//:1` and exits 7 for the missing target.

Generation and two independent no-update reruns passed with `/usr/bin/bazel`
9.2.0. Discovery, provenance, generated/assertion coverage, whitespace, and
candidate credential checks passed. Bazel used ordinary RC discovery and was
allowed to consume the user's external BuildBuddy configuration; its contents
were not inspected or copied. Sol-low reviewed the complete oracle and
returned `ACCEPT`.

Implementation and DICE activation evidence remain pending. Do not mark this
packet or M3 complete from the oracle alone.

#### Implementation evidence landed (2026-07-23)

Implementation commit `cdc5af41` completes
`WP-8-m3-reverse-deps-subtree-patterns`. It does not complete M3 or loading
query.

Reuse and graph ownership:

- `ResolvedGraph` directly adapts Buck2 commit
  `088c75c7e36805df99c3de29062baa95db700b8b`
  `query/graph/graph.rs` stable DFS remapping, integer-indexed edges,
  `reverse`, breadth-limited retention, and DFS postorder. It uses structural
  `QueryLabel`, `SmallMap`, and `SmallSet`; only dependency lookup is
  serialized because V2 owns mutable `DiceComputations`.
- `RdepsFunction` follows Buck2 `query/environment.rs` and
  `syntax/simple/functions/deps.rs`: the generic registry evaluates the
  universe and seed expressions, constructs the forward universe closure,
  reverses it request-locally, filters out-of-universe seeds, and applies the
  optional depth. No persistent reverse key/cache, string-keyed adjacency,
  second DICE/runtime, or central name dispatch was added.
- `SubtreePackageSetKey { workspace, prefix }` starts exactly at
  `workspace/prefix` and consumes descendant `WorkspaceDirectoryKey`s. Root
  `//...` uses the empty prefix; non-root patterns never compute/filter a
  whole-workspace package set.
- `same_pkg_direct_rdeps` groups exact operand labels by package, loads only
  those `UnconfiguredPackageGraphKey`s, and matches the full dependency label
  inside that package. Cross-package and criss-cross-only parents are
  excluded without implementing `siblings`.

Integrated behavior:

- root-repository `//pkg/...`, `rdeps`, and
  `same_pkg_direct_rdeps` match all 26 generated oracle rows through the real
  CLI/daemon boundary, including missing/empty subtree exit 7, source and rule
  seeds, aliases/custom rules, duplicate seeds, cycles, universe exclusion,
  depth zero/one/two, distinct full ordering, arity errors, and integer
  expression operands resolved as `//:1`;
- only those two registry entries moved from deferred to implemented. Build
  requests, query protocol fields, cquery/aquery placeholders, and command flag
  policy remain unchanged; and
- a retained-daemon regression observes an `rdeps` edge loss and subtree
  package creation without restarting the runtime.

Exact activation evidence:

- initial `//tree/...`: `tree/base Evaluated`,
  `SubtreePackageSet(tree) Evaluated`; identical revision: no events;
- unrelated BUILD edit: `tree/base Reused`, no subtree event;
- package create/delete/recreate outside the prefix: `tree/base Reused`,
  `SubtreePackageSet(tree) Reused`, never evaluated;
- create/recreate inside the prefix: `tree/base Reused`,
  `tree/dynamic Evaluated`, `SubtreePackageSet(tree) Evaluated`; delete:
  `tree/base Reused`, `SubtreePackageSet(tree) Evaluated`;
- initial `rdeps(//app:top, //leaf:item)`: `app Evaluated`,
  `leaf Evaluated`; redirecting the universe edge evaluates `app` and the
  newly demanded `other`, reuses `leaf`, and removes the result; restoring the
  edge evaluates `app`, reuses `leaf`, and restores the result; and
- initial `same_pkg_direct_rdeps(//left:source.txt)` evaluates only `left`;
  editing a cross-package reverse dependent reuses only `left` and leaves the
  result unchanged.

Validation:

- the serial six-crate suite passed 71 tests under
  `CARGO_TARGET_DIR=/tmp/slug-m3-rdeps-target CARGO_BUILD_JOBS=1`;
- `slug_cli_v2` rebuilt successfully, and root independently passed
  `query-parser-and-sets`, `query-loading-thin-vertical`, and
  `query-rdeps-and-subtree-patterns` through the absolute rebuilt V2 binary;
- formatting, diff, direct-filesystem/runtime/cache/lock/default-collection/
  configured-action ownership scans, and stale-daemon checks passed; and
- Sol-low accepted both the early source-reuse audit and the complete
  post-validation diff.

Residual scope: external repositories and mapping, the other 13 loading
functions, broader target patterns and Sky Query, non-text formatters and
remaining ordering modes, configured/action environments, `cquery`, and
`aquery` remain open.

### Reviewed next packet — `WP-8-m3-path-topology` (2026-07-23)

Work packet ID: `WP-8-m3-path-topology`

Owner stage and plan: Stage 8,
`thoughts/shared/plans/slug-v2-subplans/08-ruleset-and-command-conformance.md`.

Goal and gate link: implement only `allpaths(from, to)` and
`somepath(from, to)` over the landed loading-query graph. Both functions share
one request-local forward topology: `allpaths` projects every node on any
valid route, while `somepath` reconstructs one shortest valid route. This
packet remains root-repository, text-only, and `auto|full`; it does not
complete M3.

Why this vertical:

- `allpaths` is exactly the unbounded reverse-dependency operation over the
  forward closure of `from`, so it reuses the accepted `rdeps` substrate;
- `somepath` adds a compact integer-index BFS/parent map to the same
  `ResolvedGraph`, without new semantic graph state;
- `some` is deliberately arbitrary and has separate empty-set semantics;
- `siblings` requires Bazel's `//pkg:BUILD` pseudo-target, which V2 does not
  represent;
- `filter` is isolated label-regex work; and
- `kind` requires real rule-class/target-kind metadata rather than the current
  generic custom-rule kind.

Oracle-first artifact:

1. Add `tests/v2_oracle/fixtures/query-path-topology/` with a unique linear
   chain, branching/merge diamond, cycle, disconnected target, duplicate
   operands, multiple origins/destinations, source-file endpoints, an alias,
   and a custom label-list rule.
2. Generate and independently rerun with `/usr/bin/bazel` 9.2.0 and cite
   immutable Bazel commit
   `8220c6198837d5c13d53fea211cf3282aa12408a`. Bazel may use ordinary RC
   discovery and the user's external BuildBuddy configuration; agents and
   inspection tools must never read, copy, print, log, or commit
   `~/.bazelrc` or credentials.
3. Cover:
   - `allpaths` for a unique linear path, full diamond, cycle, zero-length
     path, no path, empty `from`, empty `to`, multiple `from` roots, multiple
     `to` roots, an endpoint outside the forward closure, duplicates,
     rule-to-source and source-to-rule direction, alias, and custom-rule edges;
   - `somepath` for the same topology classes, with a unique shortest path
     where exact path/order is asserted;
   - a multi-pair ambiguous case where only one root/endpoint pair is
     reachable, and a genuinely ambiguous multi-pair case checked through
     bounded complete-path alternatives rather than root precedence;
   - default, `auto`, and `full` output for `allpaths` and unique-path
     `somepath`, plus a `somepath` wrapped in a top-level set operation whose
     insertion order differs from lexical order; and
   - too few/too many arguments for both functions plus integer expression
     operands in both positions across the pair. Integers retain the existing
     `//:1` missing-target exit-7 behavior; they are not type errors.
4. Bazel defines `somepath` branch choice as arbitrary.
   `testSomePathOperatorOrdering` accepts either diamond branch. The fixture
   may use a regex bounded to exactly the two complete valid branches, and
   Slug tests may accept the same two alternatives. Reject mixed branches,
   missing endpoints, extra nodes, or claims that one branch/root wins.
5. Do not include generated/output-file path cases. Their reverse generating
   edge belongs to the Stage 6/loading-representation boundary and is an
   explicit residual, not a source-file substitute.

Bazel 9.2 source anchors:

- `query2/engine/{AllPathsFunction,SomePathFunction}.java`;
- `query2/query/BlazeQueryEnvironment.java` and graph shortest-path support
  used by `getNodesOnPath`; and
- `runtime/commands/QueryCommand.java:112-118` plus
  `query2/engine/QueryExpression.java:110-114`, where Bazel disables AUTO
  lexicographic aggregation only for a root expression that is directly
  `somepath`; and
- `src/test/java/com/google/devtools/build/lib/query2/testutil/AbstractQueryTest.java`
  `testSomePathOperator`, `testSomePathOperatorOrdering`, and
  `testAllPathsOperator`. Deliberately omit
  `testPathOperatorsWithOutputFile`.

Reuse audit and approved decisions:

- for `allpaths`, call the landed
  `reverse_dependencies(environment, from, to, None)` directly. This matches
  Buck2 commit `088c75c7e36805df99c3de29062baa95db700b8b`
  `query/environment.rs::allpaths` delegating to unbounded `rdeps` and Bazel's
  forward-closure/reverse-closure intersection. Do not add another traversal,
  reverse adjacency, DICE key, or cache;
- for `somepath`, directly adapt Buck2
  `app/buck2_query/src/query/graph/async_bfs.rs` parent-map/path
  reconstruction and
  `query/syntax/simple/functions/deps.rs::invoke_somepath`. Extend the landed
  `ResolvedGraph` with one integer-index BFS shortest-path method over its
  existing compact adjacency. Build the forward closure once; do not add a
  parallel environment-level dependency walk;
- preserve stable structural `QueryLabel`, `SmallMap`, `SmallSet`, and
  request-local integer graph state. Preserve the accepted serial
  dependency-lookup adaptation required by mutable `DiceComputations`; and
- use V1
  `e218054d4c796655939b968d90208b185decb352`
  `tests/core/query/test_bazel_compat_query.py` only as scenario inventory.
  Reject V1 cells, graph/server context, labels, algorithms, printers, and
  expected output.

Required implementation:

1. Transition only `allpaths` and `somepath` from deferred to implemented in
   the generic callable registry. Each function owns typed two-expression
   invocation; evaluator dispatch stays generic.
2. `allpaths` evaluates both arguments and calls the existing unbounded
   reverse-dependency helper with `from` as universe roots and `to` as reverse
   seeds. Endpoints outside the forward closure are excluded; zero-length
   intersections are retained; cycles terminate.
3. `somepath` evaluates both arguments, builds one stable forward
   `ResolvedGraph` from all origins, then runs integer-index BFS with a parent
   map. Return one shortest path for one reachable pair, a one-node path when
   an origin is also a destination, or empty success when none exists.
   Multiple-root/endpoint choice remains unspecified.
4. Add only Bazel's top-level-`somepath` AUTO exception at the point where the
   parsed AST and `QueryOrder` already meet: `evaluate_loading_query` sorts
   labels iff `order == Auto` and the parsed root node is not directly the
   `somepath` function. Parentheses that lower to that root remain top-level;
   binary union/intersect/except and `let` wrappers are not top-level and keep
   ordinary AUTO sorting. `Full` always retains evaluator/path insertion
   order. Do not put this decision inside `SomePathFunction`,
   `ResolvedGraph`, the CLI, or the daemon protocol.
5. Reuse the exact existing `QueryEnvironment::dependencies`/DICE transaction.
   Add no new order mode, QueryNode/loading representation, DICE key, protocol,
   runtime, cache, filesystem, lock, configured/action import, other function,
   target pattern, or formatter.
6. Add focused AST/output tests for direct and parenthesized top-level
   recognition; binary and `let` non-top-level wrappers; direct
   default/explicit-auto/full forward path order; nested default/auto lexical
   order; and unchanged `allpaths` AUTO sorting. Add a full CLI fixture
   regression including
   bounded diamond alternatives, and retained-daemon edge/package transition
   coverage. Preserve all three preceding query fixtures.

DICE acceptance must compare complete relevant activation multisets:

- initial, identical, and unrelated-edit requests, distinguishing honest
  `Reused` validation from evaluation;
- adding/removing/restoring a reachable branch evaluates its owning source
  graph and only newly demanded/lost closure packages;
- removing/restoring an intermediate edge makes the path empty or removes the
  affected `allpaths` branch, then restores it;
- edits in packages outside every `from` closure never broaden traversal;
- a `to` literal outside the closure is evaluated as an operand package but
  never enters the graph/result;
- literal-only cases activate no `SubtreePackageSetKey`; and
- the retained daemon observes edge loss/regain and reachable package
  gain/loss without restart.

Focused validation:

```bash
CARGO_TARGET_DIR=/tmp/slug-m3-path-target CARGO_BUILD_JOBS=1 cargo test \
  -p slug_query_v2 -p slug_loading_v2 -p slug_core_v2 \
  -p slug_commands_v2 -p slug_server_v2 -p slug_cli_v2
CARGO_TARGET_DIR=/tmp/slug-m3-path-target CARGO_BUILD_JOBS=1 \
  cargo build -p slug_cli_v2
python3 -B -m tools.v2_oracle run \
  --fixture query-path-topology --tool bazel --bazel /usr/bin/bazel
python3 -B -m tools.v2_oracle run \
  --fixture query-path-topology --tool slug --slug <absolute-rebuilt-v2-slug>
cargo fmt --all -- --check
git diff --check
```

Also rerun the three preceding Slug query fixtures and inspect for duplicate
graph discovery, direct filesystem access, extra DICE/runtime/cache creation,
string adjacency/default hash collections, locks across DICE, configured or
action imports, unrelated registry changes, and build/cquery/aquery
protocol/placeholder drift. Reject any sorting change broader than the parsed
root-node `somepath` exception.

Evidence and completion boundary: land and review the generated oracle before
Rust changes. Require Sol-low approval of the shared graph/parent-map port
before broad validation and a complete post-validation review before recording
implementation evidence. Update Stage 1, Stage 8, Stage 9, and the routing log
with exact commits, alternatives, activation events, validation, and residuals.

Sol-low accepted this revised architecture after requiring direct unbounded
reverse-dependency reuse, one shared `ResolvedGraph` parent-map BFS, honest
bounded alternatives for arbitrary paths, the complete endpoint/order/error
matrix, and exact DICE demand evidence.

The generated oracle then exposed Bazel's source-backed top-level ordering
exception. Sol-low accepted the fixture but required the narrow
`evaluate_loading_query` AST seam and nested-expression regressions recorded
above before implementation.

#### Oracle evidence landed (2026-07-23)

Oracle commit `2b73c08d` lands the generated 43-command Bazel 9.2.0
`query-path-topology` fixture at immutable source commit
`8220c6198837d5c13d53fea211cf3282aa12408a`.

The fixture proves the reviewed topology, endpoint, ordering, diagnostic, and
bounded-arbitrary-path matrix. In particular, direct root-node `somepath`
preserves `linear_start, linear_mid, linear_end` for default and explicit
AUTO output, while a top-level union containing the same call sorts
`disconnected, linear_end, linear_mid, linear_start`. The latter two rows
would fail a broader function-local or graph-local ordering exception.

Generation
`target/v2o/runs/query-path-topology/20260723-013430-473612-bazel`, the
worker's clean no-update rerun
`target/v2o/runs/query-path-topology/20260723-013510-476303-bazel`, and root's
independent clean no-update rerun
`target/v2o/runs/query-path-topology/20260723-013603-478981-bazel` all passed
sequentially. Root matched all 43 exits and configured anchored stdout/stderr
patterns, checked immutable provenance and whitespace, and found no candidate
credential material in fixture files. Bazel could consume the user's external
`~/.bazelrc`; no agent or inspection tool read its contents, and no external
RC or BuildBuddy credential content entered the repository. Sol-low returned
final `ACCEPT`.

The oracle-first gate closed before Rust edits. The implementation evidence
below is separate; the oracle alone did not complete the packet.

#### Implementation evidence landed (2026-07-23)

Implementation commit `7d851ce9` completes `WP-8-m3-path-topology`. It does
not complete M3 or loading query.

Reuse and ownership:

- `AllPathsFunction` evaluates its typed operands and directly calls the
  landed `reverse_dependencies(environment, from, to, None)` helper. No second
  dependency walk, reverse adjacency, DICE key, or cache was added.
- `SomePathFunction` builds the existing stable forward `ResolvedGraph` once.
  Its `shortest_path` method directly adapts Buck2 commit
  `088c75c7e36805df99c3de29062baa95db700b8b`
  `query/graph/async_bfs.rs`: a multi-source `VecDeque`, `SmallSet<u32>`
  visited state, and `Vec<Option<u32>>` parent map reconstruct one complete
  shortest root-to-destination path. V2 omits Buck2's concurrent lookup queue
  only because mutable `DiceComputations` has already resolved the compact
  graph serially.
- Structural `QueryLabel`, `SmallMap`, `SmallSet`, and request-local integer
  graph storage remain authoritative. No string-keyed adjacency, default hash
  collection, new runtime/cache/key, direct filesystem read, or lock across
  DICE was introduced.
- `QueryExpression::is_top_level_somepath` recognizes only the parsed root
  function node. `evaluate_loading_query` suppresses AUTO sorting only for
  that predicate; parentheses lower away, binary and `let` wrappers retain
  ordinary lexical AUTO sorting, and `Full` remains insertion ordered.

Integrated behavior:

- only `allpaths` and `somepath` moved from deferred to implemented in the
  generic 16-entry callable registry;
- all 35 success and eight failure rows in `query-path-topology` pass through
  the real CLI/daemon boundary, including exactly bounded complete diamond and
  multi-pair paths, direct-versus-nested ordering, topology/endpoint cases,
  arity failures, and integer operands resolved as `//:1`; and
- build requests, the query protocol, prior three query fixtures,
  `cquery`/`aquery` placeholders, target patterns, formatters, configured and
  action state remain unchanged.

Exact DICE evidence:

- initial `allpaths(//origin:top, //dest:end)` evaluates `origin`, `mid`, and
  `dest`; an identical request in the same revision emits no activation
  callbacks, while an unrelated edit validates all three as `Reused`;
- removing/restoring `mid -> dest` evaluates `mid` and reuses `origin` and
  `dest`, with the path disappearing and returning;
- adding a demanded branch evaluates `origin` and new `branch` while reusing
  `mid`/`dest`; removing it evaluates only `origin` and omits `branch`;
  restoring it evaluates `origin` and reuses `branch`, `mid`, and `dest`;
- an outside-package edit only validates the four demanded packages as
  `Reused`; a literal destination outside the forward closure evaluates its
  operand package but never enters the graph/result; and
- literal-only path queries activate no `SubtreePackageSetKey`. The retained
  daemon independently observes edge loss/regain and reachable package
  create/delete/recreate without restart.

Validation:

- both the Terra-high worker and root independently passed the serial
  six-crate suite with 76 tests and rebuilt `slug_cli_v2` at
  `/tmp/slug-m3-path-target/debug/slug`;
- root's four sequential Slug runs passed at
  `query-parser-and-sets/20260723-015838-505418-slug`,
  `query-loading-thin-vertical/20260723-015842-505479-slug`,
  `query-rdeps-and-subtree-patterns/20260723-015846-505509-slug`, and
  `query-path-topology/20260723-015849-505567-slug`;
- formatting, diff, ownership/reuse, forbidden-scope, and stale-daemon checks
  passed; and
- Sol-low accepted both the early stable-diff graph/ownership audit and the
  complete post-validation evidence.

Stop conditions: an unstable diamond oracle that cannot be bounded to complete
valid alternatives; behavior contradicting generated Bazel 9.2; any need for
generated/output nodes, attrs/visibility/tests/executable/load/build/configured
or action state, external repositories, Sky Query flags, filters, non-text
formats, a duplicated dependency traversal, persistent graph/reverse cache,
new DICE/runtime, direct filesystem read, lock across DICE, protocol expansion,
sorting behavior broader than the exact top-level `somepath` exception, or
changes to build/cquery/aquery behavior. Defer `some`, `siblings`, `filter`,
`kind`, `attr`, `labels`, `buildfiles`, `loadfiles`, `tests`, `visible`,
`executables`, remaining patterns/order modes/formats, `cquery`, and `aquery`.

### Reviewed next packet — `WP-8-m3-some-selection` (2026-07-23)

Work packet ID: `WP-8-m3-some-selection`

Owner stage and plan: Stage 8,
`thoughts/shared/plans/slug-v2-subplans/08-ruleset-and-command-conformance.md`.

Goal and gate link: implement only `some(expr[, count])` for ordinary,
root-repository loading query, plus the signed-`i32` integer-argument
correction required to parse its optional count with Bazel 9 semantics. This
packet moves one loading function from deferred to implemented and leaves M3
open with ten functions.

Why this vertical and not `filter`:

- `some` consumes the existing eager `TargetSet`, preserves its structural
  deduplication, and selects a bounded arbitrary subset without another graph
  or target representation;
- ordinary `bazel query` uses `AbstractBlazeQueryEnvironment`, whose
  `gracefullyCancel()` is a no-op. Its operand therefore continues evaluating
  after `SomeFunction` has collected enough nodes. V2's eager materialization
  is valid only for this non-Sky slice;
- Sky Query can cancel streaming evaluation, but V2 has no universe-scope or
  Sky Query surface and this packet must not add one; and
- `filter` remains deferred because Bazel uses `java.util.regex.Pattern`.
  Buck2's `fancy-regex` implementation is useful structural precedent but is
  neither the same accepted language nor the same failure contract. A
  knowingly restricted regex subset would violate Bazel 9 parity.

Oracle-first artifact:

1. Add `tests/v2_oracle/fixtures/query-some-selection/` with three deliberately
   non-lexical root targets, duplicates, a cycle, two operand packages, a
   recursive subtree with dynamic candidates, alias/source/custom-rule
   candidates where useful, and no generated/output nodes.
2. Generate and independently rerun with `/usr/bin/bazel` 9.2.0 at immutable
   commit `8220c6198837d5c13d53fea211cf3282aa12408a`. Bazel may use ordinary RC
   discovery and the user's external BuildBuddy configuration; agents and
   tools must never read, copy, print, log, or commit `~/.bazelrc` or
   credentials.
3. Cover:
   - singleton input with omitted count under default, explicit `auto`, and
     `full`;
   - a multi-target input with omitted count, count two of three, count equal
     to size, count greater than size, duplicate operands, nested `some`, an
     empty set, cycle closure, and recursive input;
   - zero and quoted negative counts over nonempty and empty operands;
     unquoted negative syntax; accepted `2147483647` and `'-2147483648'`;
     rejected `2147483648`, `'-2147483649'`, and `2_147_483_647`; a
     noninteger word; and too few/too many arguments;
   - expression-position integers, including `2147483648`, remaining target
     literals rather than being globally narrowed to `i32`;
   - valid early candidates followed by a missing target and a missing package
     in union and `set` operands. Record any partial stdout, exit, and error:
     ordinary query must not silently mask the later failure;
   - an invalid/out-of-range count paired with a missing operand, proving
     integer parsing fails before target evaluation; and
   - quoted-negative and `i32` boundary probes for the already implemented
     optional integer positions of `deps` and `rdeps`, because the shared
     correction must not leave those functions with unsigned or divergent
     depth behavior.
4. Bazel defines selection as arbitrary. For one-of-many and two-of-three
   results, assertions may admit only finite complete alternatives: one actual
   member, or two distinct actual members. AUTO alternatives must be lexical;
   FULL alternatives may enumerate complete retained insertion orders. Do not
   claim a winning label, callback batch, or root.
5. Every command needs anchored stdout. Failures require empty or explicitly
   observed partial stdout plus stable stderr and exit assertions. Nonempty
   partial stdout on an ordinary-query failure is a stop condition because
   V2's current result boundary cannot return output and an error together.
   Exact output is appropriate for singleton and all-selected rows; arbitrary
   subsets use bounded alternatives.

Bazel 9.2 source anchors:

- `query2/engine/SomeFunction.java` and
  `QueryEnvironment.java#EvaluateExpression`;
- `query2/common/AbstractBlazeQueryEnvironment.java`
  `AbstractBlazeQueryEvaluateExpressionImpl`,
  `query2/SkyQueryEnvironment.java`
  `SkyQueryEvaluateExpressionImpl`, and
  `query2/QueryEnvironmentFactory.java#canUseSkyQuery`;
- `query2/engine/{QueryParser,Lexer}.java` for context-sensitive signed Java
  integer parsing;
- `runtime/commands/QueryCommand.java` and
  `query2/query/output/QueryOutputUtils.java` for normal rendering; and
- `AbstractQueryTest` `testSomeOperator_noCountParameter`,
  `testSomeOperator_countParameterNotEqualActualCount`,
  `testSomeOperator_nestedSomeTest`, and
  `testUnconditionalQueryException`.

Reuse audit and approved decisions:

- Buck2 commit `088c75c7e36805df99c3de29062baa95db700b8b`
  has no default `some` function. Reuse only the compact ordered-set lesson
  from `query/syntax/simple/eval/set.rs`; retain V2's `TargetSet<SmallSet<_>>`
  and do not import Buck query semantics;
- V1 commit `e218054d4c796655939b968d90208b185decb352`
  has no reusable `some` scenario or implementation. Do not invent one from
  the V1 query engine;
- keep the generic AST's expression-position integer representation. At the
  existing function-spec validation/typed-argument seam, accept an integer
  token or WORD only for an expected integer slot, parse it with signed
  Java-`int` bounds, and produce Bazel's
  `expected an integer literal: '<raw>'` error before evaluation; and
- carry signed `i32` through the shared optional integer argument used by
  `deps`, `rdeps`, and `some`. Do not cast negative values to unsigned. Preserve
  the oracle-observed depth result and demand behavior of `deps`/`rdeps`.

Required implementation:

1. Transition only `some` from deferred to implemented in the 16-entry
   callable registry. Its generic typed invocation evaluates its operand once,
   uses the existing unique insertion order, retains at most the requested
   count, and returns `argument set is empty` when no node was selected.
   Omitted count is one.
2. Add only the bounded signed-integer conversion described above. Quoted
   `'-1'` is a valid integer argument; bare `-1` remains a binary-minus syntax
   failure. Values outside Java `i32` fail before operand lookup.
   Expression-position integers remain target expressions, including values
   above `i32::MAX`.
3. Update shared depth handling only as required for the generated
   `deps`/`rdeps` signed-boundary oracle. Preserve normal operand/universe
   evaluation and existing traversal ownership; add no traversal, graph,
   cache, or key.
4. Preserve ordinary output policy. Root `some` has no top-level ordering
   exception: default/AUTO lexically sort whatever subset was selected. FULL
   is not `some` insertion order: it uses the shared deterministic
   topological renderer, matching Bazel's `AbstractUnorderedFormatter` and
   `Digraph` ordering path. The later siblings provenance packet established
   that its graph must contain recorded evaluation edges, not synthesized
   semantic edges.
5. Add focused parser/registry/integer tests, evaluator selection/error/order
   tests, complete CLI oracle coverage, exact DICE activation multisets, and a
   retained-daemon candidate create/rename/delete/recreate regression.
   Preserve all four preceding query fixtures and build/cquery/aquery
   behavior.

DICE acceptance must compare complete relevant activation multisets:

- initial, identical, and unrelated-edit requests, distinguishing no
  activation callback from `Reused` validation and `Evaluated` computation;
- two-package operands remain fully demanded even when the first package
  supplies enough selectable nodes;
- recursive operands activate only the existing `SubtreePackageSetKey` and
  demanded package graphs before selection;
- zero and negative counts evaluate the valid operand before returning the
  empty-selection error, while invalid/out-of-range counts activate no operand
  keys;
- cycle traversal terminates before selection;
- creating, renaming, deleting, and recreating candidates through BUILD
  target-name edits evaluates only the operand package and updates an exact
  all-selected result when `count >= available`; and
- no new DICE key, graph state, reverse cache, streaming/cancellation runtime,
  direct filesystem read, protocol, or lock is introduced.

Focused validation:

```bash
CARGO_TARGET_DIR=/tmp/slug-m3-some-target CARGO_BUILD_JOBS=1 cargo test \
  -p slug_query_v2 -p slug_loading_v2 -p slug_core_v2 \
  -p slug_commands_v2 -p slug_server_v2 -p slug_cli_v2
CARGO_TARGET_DIR=/tmp/slug-m3-some-target CARGO_BUILD_JOBS=1 \
  cargo build -p slug_cli_v2
python3 -B -m tools.v2_oracle run \
  --fixture query-some-selection --tool bazel --bazel /usr/bin/bazel
python3 -B -m tools.v2_oracle run \
  --fixture query-some-selection --tool slug --slug <absolute-rebuilt-v2-slug>
cargo fmt --all -- --check
git diff --check
```

Also rerun the four preceding Slug query fixtures and inspect for parser
rewrites, unsigned casts, arbitrary-result overclaims, duplicate evaluator
state, direct filesystem access, new DICE/runtime/cache/protocol/locks,
unrelated registry changes, and build/cquery/aquery drift.

Evidence and completion boundary: land and review the generated Bazel oracle
before Rust edits. Require Sol-low approval of the signed-integer and arbitrary
selection seams before broad validation, then a complete post-validation
review. Update Stage 1, Stage 8, Stage 9, and the routing log with exact
commits, bounded alternatives, activation events, validation, and residuals.

Sol-low accepted the architecture after rejecting `filter`'s non-parity regex
substrate and requiring valid Java max/min boundaries, a hard stop on failing
partial stdout, and complete candidate create/rename/delete/recreate evidence.

#### Oracle evidence landed (2026-07-23)

Oracle commit `e8e1d9ef` lands the generated 42-command Bazel 9.2.0
`query-some-selection` fixture at immutable source commit
`8220c6198837d5c13d53fea211cf3282aa12408a`.

The matrix establishes:

- singleton, arbitrary one-of-three, arbitrary two-of-three, equal/excess
  count, duplicate, nested, empty, cycle, recursive, and cross-package
  selection behavior with only complete finite alternatives;
- default/AUTO lexical rendering and distinct FULL insertion rendering without
  claiming a winning arbitrary member or callback order;
- zero, quoted negative, and quoted minimum counts select nothing and fail
  `argument set is empty`; bare negative is syntax; Java max is accepted;
  positive/negative overflow, separators, and nonintegers fail before operand
  lookup;
- expression-position `2147483648` remains the target literal
  `//:2147483648`; and
- `deps`/`rdeps` accept signed maximum depth, return empty success for quoted
  negative/minimum depth, and reject overflow before lookup.

All four early-candidate/later-missing target/package union/set probes exit 7
with empty stdout. Ordinary Blaze query therefore exposes neither masked
errors nor failing partial output for this matrix. Cross-package success rows
make complete operand demand observable.

Final authoritative generation
`target/v2o/runs/query-some-selection/20260723-022513-519324-bazel`, the
worker's clean no-update rerun
`target/v2o/runs/query-some-selection/20260723-022556-521952-bazel`, and root's
independent clean no-update rerun
`target/v2o/runs/query-some-selection/20260723-022658-524651-bazel` all passed
sequentially. Root validated all 42 names/argv/exits and configured anchored
stdout/stderr patterns, schema/generated/tool metadata, immutable provenance,
whitespace, and fixture-only candidate credential terms. Bazel could consume
the user's external `~/.bazelrc`; no agent or tool read its contents, and no
external RC or BuildBuddy credential material entered the repository.
Sol-low returned final `ACCEPT`.

The oracle-first gate is closed. Implementation evidence follows; do not mark
M3 complete from this one function packet.

#### Implementation evidence landed (2026-07-23)

Implementation commit `b25c8aff` lands `SomeFunction` in the existing generic
registry. It evaluates the complete operand once, chooses up to the requested
members in `TargetSet<SmallSet<_>>` insertion order, and retains the existing
empty-selection diagnostic. That insertion choice is deliberately distinct
from rendering: default/AUTO lexically order the selected result, while FULL
deterministically topologically renders it. At this checkpoint the renderer
synthesized the semantic selected-induced graph; implementation `d19a9b29`
later replaced that approximation with the final selected portion of the
request-local recorded evaluation graph, preserving every `some` row. The
first Slug gate failed `equal_count_full` because the old packet claim that
FULL preserved insertion was wrong; Bazel 9.2's
`query2/query/output/AbstractUnorderedFormatter.java` and
`graph/Digraph.java` establish the shared renderer boundary. The
corrected gate also required the UTF-8-safe three-token bare-negative message
`syntax error at '- 1 )'`, rather than a byte-offset-only parser error.

The typed optional-integer seam now parses signed Java `i32` values for
`some`, `deps`, and `rdeps`: max is accepted, quoted negative/minimum depth
returns empty success for the traversal functions, and overflow is rejected
before operand demand. Expression-position integers remain target literals.
No DICE key, cache, runtime, protocol, filesystem access, or lock was added;
the patch retains Buck2-derived `SmallMap`, `SmallSet`, and `u32` graph
indices.

Exact DICE regressions record: a two-package `some` evaluates both packages
even after the first supplies a candidate; zero count evaluates its valid
operand, invalid count activates none; negative `deps` evaluates its root and
negative `rdeps` evaluates the existing universe closure; recursive `some`
evaluates its two packages plus `SubtreePackageSet`; and the retained candidate
transition is initial `cand Evaluated`, identical no events, unrelated edit
`cand Reused`, then candidate create/rename/delete/recreate each `cand
Evaluated` with the exact all-selected result updated.

Worker validation passed the serial six-crate suite, 82/82 tests, and the five
Slug fixtures `query-parser-and-sets`, `query-loading-thin-vertical`,
`query-rdeps-and-subtree-patterns`, `query-path-topology`, and
`query-some-selection` for 10+12+26+43+42 = 133/133 rows at run directories
ending `030821`, `030825`, `030829`, `030833`, and `030837`. Root independently
repeated 82/82 and those fixture gates at parser `031045-559795`, loading
`031045-559816`, rdeps `031045-559841`, path `031045-559894`, and some
`031045-559794`. Formatting, diff, scope/reuse, and stale-daemon checks were
clean. `filter` remains deferred pending exact Java `Pattern` parity.

Stop conditions: normal Bazel 9.2 query masks a later operand failure after an
early selection, or exits nonzero with nonempty partial stdout; signed integer
parity requires a broad/context-sensitive parser rewrite rather than the
bounded typed-argument seam; negative depth behavior for `deps`/`rdeps` cannot
be preserved; arbitrary results cannot be bounded to complete finite
alternatives; or implementation requires Sky Query/universe scope,
streaming/cancellation protocol, generated/output nodes, metadata, attrs,
loads, visibility, tests, executables, external repositories, new
graph/DICE/cache/runtime state, direct filesystem access, locks, or
build/cquery/aquery changes. Defer `filter` pending a Java-regex compatibility
substrate. If this packet stops, audit `siblings` plus BUILD pseudo-node
representation as the next foundation.

### Reviewed next packet — `WP-8-m3-siblings-build-file-node` (2026-07-23)

Work packet ID: `WP-8-m3-siblings-build-file-node`

Goal and gate link: implement only ordinary root-repository `siblings(EXPR)`
and the package BUILD-file node it necessarily exposes. This is not
`buildfiles`/`loadfiles`, `kind`, regex filtering, generated/output-file,
external-repository, configured, or action-query work. It moves only
`siblings` from deferred after an oracle and implementation land; at packet
review time M3 had ten deferred functions and landing it would leave nine.

Representation decision: extend the existing `UnconfiguredPackageGraph` with
exactly one zero-edge, non-rule `QueryNodeKind::BuildFile` node. Its label is
derived from `loaded.build_file.file_name()`, not normalized: a package loaded
from `BUILD.bazel` gets `//pkg:BUILD.bazel`, a package loaded from `BUILD` gets
`//pkg:BUILD`, and root follows the same rule. The wrong basename is an
ordinary missing-target error. If `exports_files` already recorded the actual
loaded BUILD basename as an `ExportedFile`, coalesce that entry into the one
zero-edge `BuildFile` node; visibility remains deferred. Any rule, alias, or
custom-target collision is an invariant error, never a silent overwrite. The
node must not leak into current `:all`/recursive rule filters or dependency
traversal; direct `deps(actual_BUILD_label)` returns only that node.

Implementation boundary:

1. Add only `QueryEnvironment::siblings` and its generic registry function.
   It evaluates its operand once, deduplicates package identities with compact
   `SmallSet`/`SmallMap`, computes the existing package graph once per package,
   and unions every graph node.
2. Retain the existing `UnconfiguredPackageGraphKey`, `PackageLoadKey`,
   `TargetSet`, `SmallMap`, `SmallSet`, and mutable-DICE serial ownership. Add
   no DICE key, graph/cache/runtime/protocol/filesystem seam, or lock.
3. Buck2 commit `088c75c7e36805df99c3de29062baa95db700b8b` contributes only
   compact deterministic collection lessons; it has no Bazel `siblings`
   semantic port. V1 commit `e218054d4c796655939b968d90208b185decb352` is
   rejected: its query surface did not implement siblings. Do not import V1
   process/server, Buck labels/cells, or pseudo-file conventions.

Bazel 9.2 source anchors:
`src/main/java/com/google/devtools/build/lib/query2/engine/SiblingsFunction.java`;
`src/main/java/com/google/devtools/build/lib/query2/engine/QueryEnvironment.java`
for the target accessor and `getSiblingTargetsInPackage` boundary;
`src/main/java/com/google/devtools/build/lib/pkgcache/PackageProvider.java`
lines 147-153 for package target-map membership;
`src/main/java/com/google/devtools/build/lib/packages/Package.java` lines
858-862, 1036, and 1462-1474 for BUILD target membership and actual discovered
basename;
`src/test/java/com/google/devtools/build/lib/packages/PackageFactoryTest.java`
line 943 and `AbstractQueryTest.java` line 1123 for an exported active BUILD
file; and
`src/test/java/com/google/devtools/build/lib/query2/testutil/AbstractQueryTest.java`
`testSiblings_simple`, `testSiblings_duplicatePackages`,
`testSiblings_samePackageRdeps`, `testSiblings_matchesTargetNamedAll`, and
`testSiblings_withBuildfiles`. The latter two `kind`/`buildfiles` themes are
source evidence for pseudo-node interaction only, not Slug acceptance rows.

Oracle-first fixture: `query-siblings-build-file-node`, generated and
independently no-update verified with Bazel 9.2 at immutable commit
`8220c6198837d5c13d53fea211cf3282aa12408a`. Bazel may use external RC/
BuildBuddy configuration by invocation only; no agent/tool reads, prints,
copies, or records `~/.bazelrc` or credentials.

Required oracle matrix:

- actual and wrong BUILD labels for `BUILD.bazel`, `BUILD`, and root, including
  matching `exports_files(["BUILD.bazel"])` and `exports_files(["BUILD"])`
  packages that still expose exactly one directly resolvable BUILD node;
- rule, attribute-created source, alias, custom-rule, and actual BUILD-file
  operands; duplicate operands and multiple packages;
- only implemented compositions: union/set/intersection/difference, `deps`
  on an actual BUILD node, and existing `rdeps`/`same_pkg_direct_rdeps` where
  the oracle proves the zero-edge/non-rule boundary;
- exact default/AUTO/FULL output for acyclic package membership plus a local
  chain/cycle, with FULL behavior pinned by Bazel rather than inferred from
  semantic dependency edges;
- empty input, arity/syntax, wrong-basename, missing target/package, and
  no partial stdout on errors; and
- exact DICE activation multisets for initial/identical/unrelated requests,
  target create/rename/delete/recreate, BUILD content edit,
  `BUILD.bazel`/`BUILD` rename in both directions, dual-file priority/fallback,
  and package deletion/recreation.

Stop rather than broaden if Bazel requires a fake `.bzl` target/transitive-load
projection, generated/output node, whole-workspace package scan, external
repository, a new cache/key/protocol/filesystem/lock boundary, changed current
rule-only pattern semantics, metadata/attribute/visibility/test/executable
semantics, regex matching, or configured/action state. `buildfiles` and
`loadfiles` remain the subsequent load-provenance packet.

#### Oracle evidence landed (2026-07-23)

Oracle commit `8c28877b` lands the generated 40-command Bazel 9.2.0
`query-siblings-build-file-node` fixture at immutable source commit
`8220c6198837d5c13d53fea211cf3282aa12408a`. It establishes actual
BUILD.bazel/BUILD/root labels, dual-file BUILD.bazel priority, one active
exported BUILD target per matching package, all reviewed sibling operand
kinds, complete supported compositions, exact default/AUTO/FULL order,
zero-edge behavior, diagnostics, and empty stdout for later-operand failures.
It explicitly excludes
buildfiles/loadfiles/fake `.bzl`, kind/Java regex, generated/output, external,
configured, and action semantics.

The earlier 35-row draft was never landed. Root found the wrong
`PackageProvider` anchor and missing root, fallback, dual, and malformed-syntax
rows; the final fixture corrected all of them before commit. Authoritative
generation `20260723-033048-572448-bazel`, worker no-update
`20260723-033115-575225-bazel`, and root independent no-update
`20260723-033329-578427-bazel` passed 40/40. Schema/generated/tool/provenance,
anchoring, whitespace/diff, and fixture-only hygiene passed; external RC may
be consumed only by Bazel invocation and was never inspected. Sol-low returned
final `ACCEPT`.

#### Implementation evidence landed (2026-07-23)

Fixture base `8c28877b`, attribute correction `20f88c05`, FULL-provenance
oracle `1a3dec16` (which expands the fixture to 43 rows), and implementation
`d19a9b29` close this packet.
`QueryNodeKind::BuildFile` represents only the actual active loaded basename,
coalesces the matching exported BUILD target, and is zero-edge/non-rule.
`siblings` evaluates its operand once and deduplicates packages. Its
request-local evaluation-edge graph uses `u32`/`Vec`/`SmallMap` following
Bazel `BlazeQueryEnvironment` and the Buck2 graph pattern; FULL renders only
recorded evaluation edges and performs no render-time DICE read. Exact
retained-DICE and daemon create/edit/delete/basename transitions passed. No
key/cache/protocol/filesystem/lock/global boundary was added.

The attribute-corrected Bazel update/no-update/root runs `034446-589899`,
`034516-592708`, and `034623-595736` passed. FULL-provenance discovery,
anchored update, clean no-update, and root runs `035638-609525`,
`035734-612675`, `035759-615627`, and `035853-619234` passed. The provenance
fixture proves direct literal and graphless union-wrapped `siblings` order are
equal, while `siblings(deps(...))` preserves the dependency-evaluation edge
and differs. External RC could be consumed only by Bazel invocation; no RC
contents or credentials were accessed.

The worker Slug gate passed 91/91 and six fixtures passed 176/176 at
`040407-626548`, `040411-626572`, `040414-626601`, `040418-626692`,
`040423-626782`, and `040427-626870`; root independently repeated them at
`040534-628098`, `040540-628123`, `040546-628189`, `040549-628247`,
`040554-628339`, and `040558-628428`. At that checkpoint M3 had nine deferred
functions; Gate B now leaves seven.
`filter` remains deferred pending exact Java `Pattern` parity.

### Reviewed next packet: `WP-4-8-m3-build-load-files`

Status: Gate A and Gate B are accepted. Gate A, B1 query core, and B1.5 landed
in `791e26b2`, `ba457999`, and `d25bc8c0`; the diagnostic and cycle
prerequisites landed in `4428df22` and `237e7cac`. B2 landed in `cb514747`,
accepting the seven graph rows and the complete 64-row fixture.

The parent packet has two acceptance gates and one oracle-first artifact:

1. Gate A, owned with Stage 4, is `load-provenance-fake-target-substrate`.
   Stage 4 establishes compact DICE-owned load provenance; Stage 8 establishes
   request-local fake-target identity and consumer ownership. The function
   registry remains deferred.
2. Gate B activates exactly `buildfiles(EXPR)` and `loadfiles(EXPR)` after A
   receives Sol acceptance. B1 made that core activation without admitting any
   other function; B1.5 and B2 complete the downstream evidence and graph
   presentation boundary.
3. Before either gate, create one combined generated Bazel 9.2 fixture. It is
   the proof for both the substrate and eventual command activation.

Bazel source anchors are
`src/main/java/com/google/devtools/build/lib/query2/engine/{BuildFilesFunction,LoadFilesFunction}.java`,
`query2/common/AbstractBlazeQueryEnvironment.java#transitiveLoadFiles`, and
`query2/compat/FakeLoadTarget.java`. Concrete ownership/identity anchors are
`query2/query/BlazeQueryEnvironment.java#getTransitiveLoadFilesHelper`,
`query2/query/BlazeTargetAccessor.java#getPackage`, and
`query2/common/AbstractBlazeQueryEnvironment.java#TargetKeyExtractor`;
upstream query regression themes are in
`src/test/java/com/google/devtools/build/lib/query2/testutil/AbstractQueryTest.java`.
The V1 reference is limited to
`slug-v1-archive:app/slug_query_impls/src/uquery/environment.rs`
(`allbuildfiles`/`get_transitive_loads`); Buck identity, cells, and path
semantics are rejected. Buck2 references are only the generic compact graph,
environment separation, and deterministic collection patterns under
`../buck2/app/buck2_query/`.

Required combined oracle matrix:

- active primary/fallback/root basename, dual-file priority, direct/shared/
  transitive `.bzl` loads, label-first deduplication, and failing `.bzl`
  cycles;
- `buildfiles` adds the selected package's active BUILD, every transitive load
  label, and every load-label package's active BUILD companion; `loadfiles`
  emits only the transitive load labels;
- fake `.bzl` and companion BUILD labels print normally while preserving
  consuming-package provenance for `siblings`; set membership is by printed
  label, intersection retains its left representative, equal-label `except`
  removes symmetrically, and union delivers distinct operand callback batches
  to `siblings`;
- broken syntax or a broken `load()` in a loaded label's containing-package
  BUILD retains Bazel's companion basename without assuming that package's
  `PackageLoad` succeeded; missing selected loads and `.bzl` cycles are
  failure rows;
- empty, duplicates, multiple packages, set operators, `siblings`, zero-edge
  `deps` on function-produced fake nodes, AUTO and FULL provenance, plus
  missing/malformed/unsupported errors with empty stdout; and
- same-daemon load-leaf edit, load-edge create/delete/recreate, BUILD↔BUILD.bazel
  replacement, and exact DICE evaluation/reuse events.

Implementation contract: use existing `BzlParseKey`, `BzlModuleEvalKey`, load
label resolution, `PackageLoadKey`, package listing, and injected workspace
observations. The immutable manifest holds canonical root label/path, direct
children, and a transitive fingerprint in compact `Arc` slices; `LoadedPackage`
exposes its BUILD roots/reachable closure and retains matching `FrozenModule`
lifetimes separately. Companion basename lookup is parse-independent and
must not load the companion package. A new key/cache/lock requires Sol
pre-review. `LoadedPackage` equality includes the direct roots and transitive
manifest identity/fingerprint so load-edge and leaf-content changes invalidate
package/query state even when target declarations are unchanged; retained
`FrozenModule` pointer/lifetime identity remains excluded.

Use request-local fake-node/provenance state only; do not rewrite global
`QueryLabel` identity. It must preserve enough `(printed label, consuming
package, real/fake)` information to retain the left intersection representative
and explicit union callback batches while applying symmetric label removal for
`except`; do not encode asymmetric `Eq` or operator semantics. Do not assume
request-global first-owner semantics. Fake nodes never enter package graphs, `:all`,
recursive patterns, or dependency edges; they are zero-edge, so `deps(fake)`
returns itself. The projection is otherwise graphless apart from real
operand-evaluation edges, and FULL must never synthesize edges.

Hard stops: external mapping, silent `.scl` omission, direct filesystem
discovery, whole-workspace scan, global identity rewrite, unreviewed DICE key,
or treating a `.bzl` cycle as success. After B, seven ordinary functions
remain deferred; regex and rule-metadata dependent functions remain blocked.

#### Oracle evidence landed (2026-07-23)

`8f6f02b3` landed the base 58-command fixture; `e8014b25` corrects it to the
shared 64-command Bazel 9.2 `query-build-load-files-provenance` fixture with a
singleton fake-target topology. Update `051423-694832`, Terra clean
`051521-700085`, and root clean `051644-705470` passed; Sol-low final review
was `ACCEPT`. It covers selected active BUILD/transitive loads/active
companions for `buildfiles`, loads-only `loadfiles`,
fallback/dual/diamond/multi-package/empty/idempotent/deps/failure cases,
broken companions without package loading, and factored FULL with
`--output=graph --graph:factored`.

`BinaryOperatorExpression#evalPlus/#evalMinus/#evalIntersect`, `QueryUtil`'s
`TargetKeyExtractor` label-key set, and `SiblingsFunction` require Gate B to
retain `(printed label, consuming package, real/fake)` with left intersections,
symmetric equal-label `except`, and explicit union callback batches. The older
fake-left survivor was unmatched transitive `two.bzl`, not asymmetric
real/fake semantics. Fake nodes are zero-edge; direct FULL
`buildfiles` omits the selected real package BUILD unless another graph
observer materializes it, while `deps(buildfiles(...))` includes result nodes.
Gate A, B1, and B1.5 now realize every non-graph row. Exactly `buildfiles` and
`loadfiles` are active and seven ordinary functions remain deferred. The seven
graph-output rows alone still block Gate B acceptance.

#### Gate A Stage 4 half evidence (2026-07-23)

`b0670e33` accepts the Stage 4 manifest/lifetime/companion-helper half.
`791e26b2` now accepts the Stage 8 fake-target algebra, without moving either
function registry entry; Gate B and all nine ordinary functions remain
deferred. `LoadedPackage` now has
semantic direct-root/reachable/fingerprint equality with aligned retained
frozen-module lifetimes; the helper is DICE-observation-only and
parse-independent. Root passed 27 loading, 11 analysis, and 22 query
integrations; Sol-low final `ACCEPT` required the symlink, validation,
alignment, lifecycle/non-over-invalidation, and memory-accounting corrections.

#### Gate A Stage 8 provenance algebra landed (2026-07-23)

Commit `791e26b2` adds crate-private
`app/slug_query_v2/src/provenance.rs` and its one-line module declaration.
Its checked-`u32` `Vec`/`SmallMap` arena records full symmetric real/fake
identity without an `Arc` per candidate. Each callback delivery is one nonempty
`Arc`-ID batch with a label-first representative; union preserves batches,
`eval_all`/intersection/`except` materialize labels, intersection retains the
LHS representative, and equal-label `except` is symmetric. `siblings` scans
all batches for consuming-package ownership, while delayed output
label-deduplicates. Fake `evaluation_graph_label` is `None`, but fake labels
remain printable and zero-edge for later evaluation/graph work.

The module is intentionally disconnected: no evaluator, graph, registry,
DICE, or function activation changed. Gate B and all nine ordinary functions
therefore remain deferred. Worker and root independently passed
`CARGO_BUILD_JOBS=1 cargo test -p slug_query_v2`: 32 tests total (10 new
provenance, 16 loading-query, 6 parser/registry). Sol-low final review returned
`ACCEPT` with no rework.

#### Gate B B1 query core landed (2026-07-23)

Commit `ba457999` activates only `buildfiles` and `loadfiles` inside
`slug_query_v2`. The crate-private generic evaluator now carries an associated
`E::Set`; loading queries bind it to request-local candidate IDs in the Gate A
arena and preserve callback batches across variables, set literals, operators,
and function results. A dedicated `eval_set_arg` keeps that associated-set
path separate from scalar argument conversion. Unused public evaluator
reexports were removed.

The loading adapter keys `seenPackages` by the candidate's printed package,
then uses its retained owner package for `PackageLoad` and transitive-load
visitation. `.bzl` label uniqueness and final output-label uniqueness use
separate sets. Companion discovery is DICE-only and receives the absolute
workspace package path. Fake candidates return no dependencies; `siblings`
consults every preserved batch; and FULL output chooses the first representative
per printed label before projecting only recorded real edges. No new DICE key,
global `QueryLabel` identity, filesystem boundary, protocol, or other crate
entered B1.

The Terra-high worker and root independently passed
`CARGO_BUILD_JOBS=1 cargo test -p slug_query_v2`: 34 tests (10 unit, 18 loading,
6 registry/parser). Root additionally passed, serially,
`cargo test -p slug_commands_v2 -p slug_server_v2 -p slug_cli_v2
--no-fail-fast`: 11 command, 12 server, and 14 CLI unit/integration tests
(13 integration plus 1 unit), with zero doc tests. Sol-low final review returned
`ACCEPT`; its two live corrections were already incorporated, so no
post-final-review rework was required. Root also removed one transient
candidate-package `String` allocation before the final tests.

#### B1.5 diagnostics and recursive-load prerequisite landed (2026-07-23)

Commit `4428df22` matches Bazel's missing-load
`cannot load '<label>': no such file` form and malformed-module compilation
summary while preserving the underlying parse diagnostic. Commit `237e7cac`
then adapts Buck2's lazy cycle-detector design as a request-scoped DICE
`UserCycleDetector` installed only on loading-capable transactions. It records
only `BzlModuleEvalKey` edges and returns a typed acyclic path plus cycle;
loading renders the Bazel BUILD-origin, multi-node, and `[self-edge]` diagrams.
An always-invalid poison dependency prevents a detected cycle from becoming a
reusable success/failure value, so repairing the edge recovers in the same
DICE instance.

Focused loading evidence covers missing and malformed loads, two-node and
self cycles, an acyclic BUILD-to-cycle prefix, a shared-leaf diamond that must
not be classified as a cycle, timeout-bounded release of recursive waits, and
same-DICE repair. Sol-low blocked the first cycle shape because it discarded
the path leading to the cycle; the typed path-plus-cycle correction landed
before final `ACCEPT`.

#### B1.5 exhaustive text and retained-daemon evidence landed (2026-07-23)

Commit `d25bc8c0` adds one exact CLI matrix for all 57 non-graph rows in
`query-build-load-files-provenance`, checking raw stdout, empty success stderr,
failure exit codes, diagnostics, and empty failure stdout. The full CLI suite
passed 14 integration plus 1 unit test.

Two retained-daemon regressions cover load-leaf edits; direct and transitive
load-edge switch/delete/recreate; BUILD-to-BUILD.bazel companion priority; and
the fact that companion changes invalidate `buildfiles` without changing
`loadfiles`. Exact invalidated-file counts are asserted. The server suite
passed 14 tests, and Sol-low's exact-set final review returned `ACCEPT`.

#### Gate B B2 graph output landed (2026-07-23)

Commit `cb514747` completes Gate B and accepts all 64
`query-build-load-files-provenance` rows. The evaluator retains one compact
request-local selected graph in `QueryOutput`; both one-shot CLI and
retained-daemon paths format that same value without reevaluation, DICE reads,
or global state. Old daemon clients deserialize to text output with factoring
enabled.

The command surface accepts Bazel's omitted, bare, explicit boolean, and
negated `--graph:factored` forms, rejects unsupported graph limits, and keeps
the default 512-node label limit. The formatter implements factored and
unfactored output, exact predecessor/successor equivalence, quotient-edge
deduplication, minimal always-quoted DOT labels, and Bazel's sorted DFS
postorder reversal. With sorting enabled, factored class IDs are ranks of the
lexicographical member-label sequence comparator; joined rendered labels are
never used as the comparator. A focused `//a:a\\n//z:z` versus `//a:a0`
regression protects that distinction.

Root passed formatting, four focused formatter tests, the seven exact graph
oracle rows, explicit unfactored coverage, and the serialized four-crate
suite: 12 command, 14 query unit, 18 loading-query, 6 parser/registry, 15
server, 14 existing CLI integration, 2 graph integration, and 1 CLI unit
tests. Sol-low's focused correction review returned `ACCEPT`. Exactly
`buildfiles` and `loadfiles` are active; seven ordinary loading-query
functions remain deferred.

## WP-4-8-m3-labels-metadata-foundation: Stage 8 Gate B (2026-07-23)

After Gate A acceptance activate only `labels(attr, expr)`. Bazel authority:
`LabelsFunction.java`, `BlazeTargetAccessor#getPrerequisites`, and
`AggregatingAttributeMapper#getReachableLabels`; evaluate the operand once,
ignore non-rules, return named reachable labels, and retain normal query-set
dedup. Absent/non-label is empty; explicit, implicit `$`, default, and every
`select()` branch are required. `QueryNode` owns a compact immutable attribute
projection separate from `dependencies`; never infer an attr by filtering the
aggregate edge list. Cross-package/source prerequisites resolve through the
existing demand-loaded `UnconfiguredPackageGraphKey` path. Output/output-list
values resolve to the Stage 4 generated-file nodes required by Bazel's
`labels(outs, ...)` rows; their kind, owner, and edges follow the oracle rather
than inference. The projection and generated nodes participate in
`QueryNode`/`UnconfiguredPackageGraph` semantic equality.

Reuse the immutable oracle and add exact CLI nesting/set/composition,
AUTO/FULL order/dedup, missing-prerequisite diagnostics, and retained-daemon
mutation coverage. Own query
`{expr,evaluator,graph}.rs` and focused command/server tests. Do not activate
`attr` or `filter`: both await exact Java Pattern; fancy-regex/Rust regex and
finite evidence are rejected. No DICE state, filesystem, configured analysis,
visibility, executable, or test-suite scope. Generated-file scope is limited to
the exact output/output-list representation required for `labels`. Hard-stop
rather than activate when a known unconfigured reachable-label form is
unsupported. Same-daemon schema/value/select/default/output edits must activate
the demand-loaded package graph, while semantically equal and non-semantic edits
reuse it.

Oracle `8dfae99c` has 31 Bazel rows. The Stage 8 CLI gate is exactly the 29
non-`label_kind` Stage 1 names; never claim Slug 31/31. Two Bazel label-kind
rows constrain generated-file representation only and require focused
`QueryNodeKind::GeneratedFile` assertions. Preserve output→own-generator,
select-key exclusion, valid dedup, and fail-fast contracts.

Stage 4 Gate A `1b7c179c` is accepted without function activation. Its ordered
immutable metadata, generated owner identity, equality, same-DICE tracker, and
preactivation guard are the loading substrate. Stage 8 `labels` is next:
exactly 29 CLI rows plus two focused `QueryNodeKind::GeneratedFile` assertions;
never claim Slug 31/31 before its formatter boundary.

Prerequisite `f3e8ad48` is accepted: fixture-native `config_setting` values
now load as sorted compact zero-edge `config_setting rule` metadata, without
configuration evaluation; unsupported attrs fail closed. Sol `ACCEPT`.
Define/flag/constraint/common attrs and matching remain deferred. Resume
Stage 8 labels at the unchanged 29 CLI plus two generated-kind boundary.

`8fec2696` activates exactly `labels`; six ordinary functions remain deferred.
29 rows (two complete graph stdout rows included) are exact; two label-kind rows
remain formatter-deferred GeneratedFile constraints. Package-load QueryError
alone gets Bazel framing; syntax/unrelated eval diagnostics remain unchanged.
Same-DICE semantic/reuse and retained-daemon schema/value/select/default/output
transitions pass. Root validation: loading 37, query 42, CLI 21
(1 unit/17 CLI/3 graph), server 15, analysis 11, plus fmt/diff. Sol corrected
error classification, exact graph evidence, and generated-only ordering before
final `ACCEPT`; M3 remains open and this is never a 31/31 claim.

## WP-4-8-m3-executables-rule-capability: Stage 8 Gate B (2026-07-23)

Oracle gate `c8e469f5` is landed and Sol-accepted. It has 32 semantic rows plus
eight Bazel-only `label_kind` rows pinning rule-class identity; those eight are
not Slug formatter acceptance. Stage 4 Gate A `c86fc656` is also landed and
Sol-accepted: immutable exported-name/executable capability, borrowed
allocation-free projection, and focused DICE equality/invalidation passed
without query activation. Gate B `executables(EXPR)` is accepted in
`69565a29`. Authority is
`ExecutablesFunction.java`, `BlazeTargetAccessor`, and `TargetUtils` at
`8220c619…`: filter the once-evaluated operand by retained per-target
`Rule.isExecutable()` / `$is_executable` and a rule-class name not ending
`_test`. An executable test is excluded. The class is the exported `.bzl`
rule name, never the BUILD target name or implementation identity.

Stage 8 projects `RuleCapability { rule_class: CompactString, executable:
bool }` from the demand-loaded graph and adds exactly one function registry and
evaluator path. It creates no query edges and retains existing set identity,
order, AUTO/FULL/default formatting, and graph presentation. It must reject no
known current-loadable target kind: source/BUILD/generated targets are
non-rules; supported native filegroup/alias/config_setting are exact
non-executable classes and alias never inherits. `test_suite` is out of scope
while absent from globals. Native `genrule` executable positives/negatives are
a separate oracle/substrate gate, not an inferred subset. Both capability
fields participate in `QueryNode` and `UnconfiguredPackageGraphKey`
equality/invalidation at this projection boundary.

The oracle matrix includes executable and non-executable Starlark rules;
export validation of test/non-test `_test` suffixes, test-implies-executable,
and executable `_test` exclusion; native/non-rule negatives; nested/set/let
compositions; exact order and graph rows; syntax/arity/no-partial-output
diagnostics. Activation evidence must additionally cover retained-daemon
false→true executable, false→true test, export rename, target rename crossing
`_test` without class change, formatting-only reuse, and delete/recreate.
Hard-stop on any need for a DICE key, direct filesystem/query-time Starlark,
global classifier, configured analysis/providers, Java regex, visibility, or
tests expansion.

#### Gate B accepted (2026-07-23)

Commit `69565a29` evaluates the operand once and filters each existing delivery
with the retained `executable && !rule_class.ends_with("_test")` capability.
It preserves candidate IDs, order, and nonempty delivery boundaries, skips
fake/non-rule candidates, and creates no query edges or new DICE ownership
surface. Exact acceptance is all 32 semantic oracle rows; the eight
`label_kind` rows remain formatter-deferred. DICE and retained-daemon evidence
covers capability, exported-class, target-name, formatting, and
delete/recreate transitions. Root validation passed 45 query tests, 50
downstream CLI/commands/server tests, formatting/diff checks, and a clean CLI
build; Sol-low returned final `ACCEPT`. Five ordinary functions remain
deferred and M3 itself remains open.

## WP-8 query evaluator module extraction accepted (2026-07-23)

Commit `65c6c54f` completes the oracle-neutral evaluator ownership split.
`evaluator.rs` is now the public loading-query facade and preserves both
`slug_query_v2::evaluator::{QueryOrder, QueryOutput,
evaluate_loading_query}` and the existing crate-root reexports.
Crate-private modules now separately own:

- result values and exact DOT formatting in `output.rs`;
- generic expression evaluation, typed argument handling, registry dispatch,
  and all eleven implemented functions in `generic.rs`;
- request-local compact traversal and resolved-graph state in `traversal.rs`;
  and
- the retained-transaction DICE loading environment, candidate/provenance
  projection, and FULL text ordering in `loading_environment.rs`.

The moved bodies are unchanged apart from the minimum crate-private
cross-module visibility. The split retains `SmallMap`, `SmallSet`,
`CompactString`, `Arc`, checked-`u32` graph indexes, DICE compute placement,
candidate identity, traversal order, and output behavior. It adds no Cargo
dependency, DICE key, public API, fixture, or Stage 9 extraction row: this is
reorganization of the already accepted Buck2-shaped V2 substrate.

Worker and root validation passed the 45-test query crate suite. Root also
passed the serial affected suite with 95 tests across query, commands, server,
CLI, and graph output; the serialized wrapper passed all seven fully accepted
query fixtures; the archive checker passed; and no daemon/socket marker
remained. Sol-low approved the boundary before implementation and returned
final `ACCEPT` after full-diff review.

Five ordinary query functions remain deferred. The completed Java `Pattern`
feasibility and rejected `java_regex` qualification are recorded below; the
completed residual ranking after them selects `tests` before `visible`.

## Java `Pattern` feasibility audit accepted (2026-07-23)

Bazel 9.2.0 `RegexFilterExpression` compiles `java.util.regex.Pattern` once
and applies `Matcher.find` to each candidate string for `filter`, regex-based
`kind`, and `attr`; the installed Bazel runtime embeds OpenJDK 25.0.2. Rust
`regex`, `fancy-regex`, PCRE, and Onig are rejected as non-Java dialect
substitutions. Buck2 and V1 provide useful query-evaluator structure but no
exact matching substrate.

`java_regex` 0.1.0 at upstream commit
`ed518dc23dacbe1a88d7cb3f26f0cfe31cc91393` is the sole qualification
candidate found, not an accepted dependency. Its immutable published crate
identity/checksum, license files, and MSRV remain unverified; its current
boolean-search route copies every subject into `Vec<char>` and allocates match,
group, and map state; error positions use scalar-character rather than Java
UTF-16 indexes; and its unpaired-surrogate escape fallback may change boolean
results by mapping to NUL.

The next discrete packet is oracle/read-only qualification only. It must pin
the published source and independently compare exact OpenJDK 25.0.2/Bazel
9.2.0 compile acceptance, failure diagnostics, `Matcher.find`, Java-only
constructs, Unicode/UTF-16, supplementary characters, unpaired surrogates,
NUL, and bounded resource behavior. It must also measure the existing
subject-copy/allocation path and design an allocation-free boolean-find
boundary. Any mismatch, unverifiable immutable source, uncontrolled resource
behavior, or unacceptable allocation boundary rejects the candidate.

This qualification adds no production dependency or lockfile entry,
registry/evaluator/query-graph activation, DICE state, or representation
substrate. `filter`, `attr`, and regex-based `kind` remain deferred. Successful
regex qualification would unblock only their shared matching substrate;
`attr` still requires exact stringified/configurable attribute projection and
`kind` still requires exact target-kind representation. Terra-medium produced
the source audit; Sol-low required this bounded qualification direction before
any implementation authorization.

## `java_regex` 0.1.0 qualification rejected (2026-07-23)

The published crate tarball and crates.io index both pin SHA-256
`1f3b3ff81a66205722b636dae12fc5cb2e77147569e8968f38a1d73b2b05fbe6`.
Its packaged VCS metadata names
`ed518dc23dacbe1a88d7cb3f26f0cfe31cc91393`; selected source, documentation,
and both license files match that commit. The package declares
`MIT OR Apache-2.0`, Rust 1.78, and four normal Unicode dependencies. This
resolves supply-chain identity but does not authorize a dependency.

Commit `5e78abc1` checks in the `java-pattern-utf16` Bazel oracle, which
constructs all values from UTF-16 units. ASCII pattern `\uD800` does not find
NUL but does find an
unpaired-high-surrogate Java `String`. The portable fixture command uses
`remotejdk_25` and generated the same two rows on 25.0.1; root ran the
Bazel-compiled oracle jar with Bazel 9.2.0's embedded OpenJDK 25.0.2 and
independently observed the identical rows. In contrast, the published Rust
parser lowers an unpaired surrogate escape through
`char::from_u32(...).unwrap_or('\0')`; its `find("\0")` returned true, while
Rust `&str` cannot represent the Java surrogate subject. This boolean mismatch
triggers the packet's stop-on-any-mismatch gate.

The existing API also measured seven allocations for that discriminator and
fourteen for a basic successful find. Source inspection confirms per-subject
`Vec<char>` copying plus match/group/map construction, while fixed
5,000,000-step and 500-depth limits return ordinary non-match. Sol-low accepted
immediate rejection: continuing a broad corpus cannot rehabilitate 0.1.0.
No crate, Cargo/lockfile change, query activation, DICE state, or candidate
comparison probe entered the repository.

`filter`, `attr`, and regex-based `kind` remain deferred. A V2-owned UTF-16
engine is an unapproved future architecture proposal, not the next packet. The
completed read-only ranking is recorded below.

## `tests` / `visible` feasibility ranking accepted (2026-07-23)

Bazel 9.2.0 `TestsFunction` is the smaller truthful residual query vertical.
It partitions direct inputs by rule class (`*_test`, exact `test_suite`, or
other), recursively expands explicit suite members, adds same-package
`$implicit_tests` for suites without explicit members, deduplicates and
terminates cycles, and never emits suites or non-tests. Suite required/bare/`+`
and excluded/`-` tags filter against each test's tags plus size; suite tag
`manual` is not a filter, while implicit membership excludes manual tests.
Missing members retain the `couldn't expand 'tests' attribute...` prefix.
Non-test members are dropped by default and produce
`INVALID_LABEL_IN_TEST_SUITE` under `--strict_test_suite`.

V2 can reuse `RuleCapability.rule_class` for the `_test` predicate and its
retained typed Starlark attribute values. It cannot activate the function yet:
there is no native `test_suite`, scalar query projection for tags/size, implicit
test membership, or strict-setting plumbing. The next packet therefore adds
only `tests-query-expansion`, a Bazel oracle spanning direct and implicit tests,
explicit/nested/cross-package suites, cycles/deduplication, tags/size/manual,
default/strict invalid members, missing members, and ordinary/full output.
Activation is a hard stop until a later reviewed representation packet stores
these facts as immutable package/query semantics and proves same-daemon
create/edit/delete invalidation. Executability is not a test predicate.

`visible` remains second. Although `LoadedPackage` retains package default
visibility, explicit rule visibility is currently validated then discarded.
There is no canonical per-target visibility, native `package_group`, package
specification/include/exclude graph, cross-package lookup, or query visibility
accessor. A public/private-only slice would be a false parity claim because
Bazel also requires same-package access, `__pkg__`, `__subpackages__`,
recursive package-group alternatives with exclusions, and asymmetric
`//javatests/X` access to private `//java/X`.

Buck2 contributes only its generic evaluator, target-set, deduplication, and
compact-collection patterns; its test query semantics are not Bazel suite
semantics. V1's generic test accessor and stored suite-label shape are
reference-only because they omit Bazel filtering, implicit, and strict
behavior. Reject V1 visibility semantics and its process-global package-group
registry. Terra-medium produced the read-only source/reuse audit, root checked
the pinned Bazel sources and current V2 representation, and Sol-low returned
`ACCEPT` for the oracle-only next packet.

## `tests(EXPR)` expansion oracle accepted (2026-07-23)

Commit `8212afd6` checks in the 16-command Bazel 9.2.0
`tests-query-expansion` fixture. Exact-set rows distinguish direct tests from
non-tests; implicit same-package membership from explicit, nested, and
cross-package suites; `manual` exclusion; cycles and deduplication; bare/`+`
required, `-` excluded, ignored suite `manual`, and size filters; default and
strict invalid-member policy; an absent target in an existing package; and
ordinary versus full output. Multi-result assertions require each exact label
once while permitting only callback-order permutations.

Root inspected the pinned `TestsFunction`,
`TestSuiteImplicitTestsAccumulator`, `TargetUtils`, `QueryOptions`, and
`AbstractQueryTest` anchors, corrected the missing-target discriminator, and
independently passed all 16 Bazel commands. No Slug loading/query
representation, DICE state, native rule, function activation, or V1/Buck2
implementation entered the packet. The next packet is the reviewed immutable
representation boundary, not `tests` activation.

## `tests` representation review replanned (2026-07-23)

The first representation proposal correctly kept rule capability separate,
used existing package/query DICE ownership, derived implicit membership after
full package loading, made suite members ordinary graph edges, and kept strict
mode request-local. Root required one source correction for suite dependencies
and inherited `tags`/`size`.

Sol-low then found three remaining material invariants. Bazel
`BuildType#convertFromBuildLangType` naturally sorts order-independent lists
while preserving duplicates; common `tags=[]` belongs to every Starlark rule,
not only tests; and independent `manual`, explicit-member, and implicit-member
fields could encode contradictory state. Because this was the packet's second
material correction, orchestration closed it as `REPLAN`. No implementation
or worktree change entered. The replacement packet is limited to an
invariant-safe loading metadata design; strict plumbing and activation remain
separate.

## `tests` loading-metadata replan stopped on attribute provenance (2026-07-23)

The narrower design resolved canonical order with duplicate preservation,
common/test inherited attrs, one explicit-or-implicit membership source,
derived `manual`, native suite graph edges, exact suite capability, and suite
tag projection without adding a DICE key or lock. Sol then identified a
remaining parity contradiction: Bazel uses omitted and explicit-empty `tests`
equally for implicit membership, but retains
`isAttributeValueExplicitlySpecified` for build/proto formatter semantics.
Collapsing both into equal final package state would lose observable
provenance.

The packet closed `REPLAN` before implementation. The next packet adds only a
Bazel oracle discriminator for this explicitness boundary. A later design must
store explicitness orthogonally to one membership source, make package equality
distinguish it, and still derive identical implicit membership for both forms.
Strict plumbing and `tests` activation remain separate.

## `test_suite` attribute provenance oracle accepted (2026-07-23)

Commit `fd4c5da0` extends `tests-query-expansion` from 16 to 23 Bazel 9.2.0
commands. Omitted and explicit-empty suites each expand to the same test,
expose the same `$implicit_tests` label set, and expose an empty public `tests`
label set. Exact `--output=build` rows distinguish them: omitted output has no
`tests` stanza, while explicit empty prints `tests = []`; both print the same
`_implicit_tests` member.

The build-format rows are representation-only and do not claim Slug formatter
acceptance. Root inspected the `AttributeProvider`, `Rule`,
`BuildOutputFormatter`, and query-function sources and independently passed all
23 commands. No loading/query code, DICE state, strict policy, or function
activation changed. The next design must retain this explicitness
orthogonally to invariant-safe membership.

## `tests` loading metadata design accepted (2026-07-23)

The accepted design retains naturally sorted order-independent lists without
deduplicating stored values; inherited `tags=[]` on every Starlark rule and
test-only `size="medium"`; test tags/size/manual derived from retained typed
values; and one native suite membership enum. Nonempty explicit members occupy
one variant. Omitted and explicit-empty suites occupy the implicit variant with
an orthogonal explicitness bit, so they share finalized members but remain
unequal for formatter provenance.

The unconfigured graph derives capability, test/suite scalar metadata,
distinct `tests`/`$implicit_tests` label attributes, explicitness, and
deduplicated ordinary edges in one match over finished package state. Existing
package and graph DICE keys own equality and invalidation; no lock, discovery,
global state, or fresh graph is needed. V1/Buck test semantics remain rejected;
only compact collection and Arc-slice patterns are reused. Terra-medium
audited, root resolved provenance against `fd4c5da0`, and Sol-low returned
`ACCEPT`. Gate A may now implement loading/graph metadata only.

## `tests` loading metadata Gate A implementation replanned (2026-07-23)

A bounded seven-file implementation added the accepted suite/test loading
shape and passed the focused loading, same-DICE invalidation, and query graph
suites. Root review found that generated `$implicit_tests` had initially been
projected as non-explicit even though Bazel `AttributeProvider` deliberately
sets it explicit for query output; the one permitted correction fixed that
case and added all three suite-membership assertions.

The independent full-diff review then found a second material representation
gap. `QueryAttribute.explicit` was introduced as general semantic state, but
native `filegroup.srcs` was hard-coded explicit. Bazel distinguishes omitted
`srcs` from explicit `srcs=[]`, while the current loading target collapses
them. Extending a total explicitness field without resolving every existing
producer would therefore encode false formatter/equality semantics.

The orchestration correction budget was exhausted, so the packet closed
`REPLAN`. All seven implementation files were restored and no Rust change was
committed. The replacement packet is design-only: audit all current query
attribute producers and choose a provenance representation that is exact for
filegroup, alias, Starlark attrs, suite `tests`, and generated
`$implicit_tests`. Strict request plumbing and `tests()` activation remain
separate.

## Total query-attribute explicitness design accepted (2026-07-23)

The replacement design gives every projected `QueryAttribute` one total
`explicit` boolean with exactly Bazel
`Rule.isAttributeValueExplicitlySpecified(attribute)` semantics. It is not
inferred from label presence. Native `filegroup` loading must retain whether
`srcs` was supplied; omitted is false, while explicit empty and nonempty are
true. Mandatory native alias `actual` is true. Retained Starlark
`AttributeProvenance::Explicit` is true and `Default`/`Implicit` are false.
Future suite `tests` retains its input bit orthogonally to exclusive
membership, and materialized `$implicit_tests` is true.

The exact bit participates in loaded-package and unconfigured-graph equality
without changing empty dependency edges or adding a DICE key. Attribute label
ordering and multiplicity remain separate from ordinary-edge deduplication.
The field is future formatter input, not formatter acceptance.

Terra-medium audited every current producer against pinned `FilegroupRule`,
`Alias`, `RuleOrMacroInstance`, `AttributeProvider`, `BuildOutputFormatter`,
and `ProtoOutputFormatter`; root verified the sources; Sol-low returned
`ACCEPT`. Before implementation retry, add only the missing filegroup
omitted-versus-explicit-empty build-output discriminator to
`query-labels-attribute-metadata`.

## Native filegroup attribute provenance oracle accepted (2026-07-23)

Commit `e1d3f910` extends `query-labels-attribute-metadata` from 31 to 33
Bazel 9.2 commands. Two exact `--output=build` rows select sibling native
filegroups: omitted `srcs` produces no stanza, while explicit `srcs=[]` prints
`srcs = []`. The rows anchor the complete rule body, source location, and
instantiation stack.

The evidence is representation-only and does not accept a Slug build/proto
formatter. Worker generation and clean verification passed; root independently
passed all 33 commands in run
`20260723-120148-1028764-bazel`; Sol-low returned `ACCEPT`. Gate A may now be
retried with total exact explicitness across native, Starlark, suite, and
generated query attributes.

## Second loading metadata Gate A implementation replanned (2026-07-23)

The fresh eight-file attempt incorporated the total explicitness design and
the filegroup oracle, strengthened loading/query lifecycle and downstream
coverage, and passed root's focused loading 13, invalidation 22, and query 27
tests. Its one integration correction updated an older exhaustive filegroup
test pattern for the new provenance field; both full loading and query test
targets then compiled.

The independent final review found a second material Bazel boundary. Native
`test_suite.tests` used V2's existing `dependency_label`, which rejects bare
package-relative labels. Bazel accepts `tests = ["a.txt"]`;
`AbstractQueryTest#testTestSuiteWithFile` proves ordinary `deps` retains the
source edge while `tests()` excludes it by default and strict mode diagnoses
it. The same V2 helper currently makes Starlark label attrs reject a common
Bazel spelling.

The packet exhausted its correction budget and closed `REPLAN`. All eight Rust
and test files were restored; no implementation was committed. The replacement
packet is design-only and must settle shared versus native-only bare-label
coercion, source-node projection, errors, and oracle scope before Gate A is
retried. Strict plumbing and function activation remain separate.

## Package-context loading label-normalization design accepted (2026-07-23)

A suite-only bare-label converter is rejected. Bazel uses one package-context
label conversion model: bare `name` and `dir/name` are target names in the
base package; `:name` is equivalent; root-absolute labels retain their package;
and repository spellings require mapping that remains out of scope.

The accepted foundation uses one crate-private converter for all loading-time
dependency labels. Explicit BUILD/Starlark values and native rules use the
instantiated target package. Starlark attribute defaults must instead
canonicalize at rule-definition time against the defining `.bzl` package,
matching `StarlarkAttrModule` and `LabelConverter.forBzlEvaluatingThread`.
Native filegroup and alias storage also canonicalizes so equivalent spellings
have equal loaded-package values. Outputs wrap the same conversion with their
same-target-package ownership check and remain generated nodes, not source
edges.

No new DICE key, lock, repository mapping, filesystem discovery, or public
identity API is needed. If current identity validation cannot reproduce a
required invalid-label class, implementation stops for a separate identity
packet rather than inventing local grammar. Terra-medium audited the pinned
`LabelParser`, `Label`, `LabelConverter`, `BuildType`,
`StarlarkAttrModule`, and `AbstractQueryTest` sources; root verified the key
paths; Sol-low returned `ACCEPT`.

Before implementation, extend the labels fixture for explicit bare/slash
values, source edges, defining-`.bzl` default ownership, and invalid relative
package syntax; extend the tests fixture for native suite bare/slash members
and their ordinary dependency edges.

## Package-context loading label oracles accepted (2026-07-23)

Commit `3621b3e7` extends `query-labels-attribute-metadata` from 33 to 37
Bazel 9.2 commands and `tests-query-expansion` from 23 to 25. The labels rows
pin canonical explicit bare/slash/colon/root forms, implicit same-package
source edges without physical files, defining-`.bzl` ownership for a
cross-package rule default, and the invalid relative `pkg:target` diagnostic.
The tests rows pin native suite bare/slash `labels(tests, ...)` and ordinary
`deps` source edges without activating `tests()`.

Worker generation and clean verification passed both fixtures. Root
independently passed labels run `20260723-123634-1062591-bazel` and tests run
`20260723-123704-1065110-bazel`. Root required one correction removing
`exports_files` declarations so the local sources were genuinely implicit;
the corrected evidence remained green. Sol-low returned `ACCEPT`.

The next packet implements the shared loading label-normalization foundation
only. Native suite construction and all other test metadata remain deferred
until that foundation is accepted.

## Loading label-normalization implementation stopped on core identity (2026-07-23)

The foundation worker reached its explicit stop before any edit.
`CanonicalLabel::parse` splits at the first colon, while
`TargetName::parse` does not reject another colon. Constructing a canonical
same-package label from invalid relative `pkg:target` therefore succeeds, and
rejecting only that loading spelling would invent a partial label grammar.

The worktree remained clean and no tests were needed. Stage 3 now owns a
central target-name validation review and implementation. Package-context
loading normalization, native suite metadata, strict policy, and `tests()`
activation remain blocked behind that accepted identity boundary.

The Stage 3 review accepted exact validation and trailing `/.` normalization
inside `TargetName::parse` only. Package-path grammar and raw label-part
classification do not move into this identity packet. After its focused tests
and downstream consumers pass, the shared loading converter resumes and owns
Bazel's package-context rejection of non-absolute `pkg:target`.

Commit `22313daa` is accepted with all 10 identity tests green and loading/query
test targets compiled. The package-context loading converter is current again;
native suite metadata, strict policy, and `tests()` activation remain deferred.

## Package-context loading label foundation accepted (2026-07-23)

Commit `40ac1cd2` adds one V2-owned package-context converter, canonical native
filegroup/alias storage, defining-`.bzl` default ownership, same-package output
ownership, and query projection that preserves attribute multiplicity while
deduplicating ordinary edges separately. Same-DICE spelling equality,
definition edits, deletion/recreation, and implicit source-node behavior are
covered without a new key or lock.

Root's source pass used the one correction for `LabelParser`'s reserved
triple-dot package forms. Sol accepted the corrected packet. Root passed all 43
loading and 48 query tests, rebuilt `slug_cli_v2`, and confirmed formatting and
diff checks. Native suite metadata is now unblocked; strict policy and
`tests()` activation remain later packets.

The current packet retries loading/query metadata Gate A with the already
accepted invariant-safe suite membership and total query-attribute explicitness
designs. It must not activate the function or formatter surfaces.

## Third loading metadata Gate A implementation replanned (2026-07-23)

The nine-file retry implemented the accepted metadata, total explicitness, and
package-context suite-member designs. Root corrected an in-flight misreading
of Guava `Ordering.natural()` as numeric-aware sorting. Focused and full
validation then passed 47 loading and 50 query tests without activating
`tests()` or strict policy.

Sol's permitted correction found that `-+tag` must exclude the literal `+tag`,
not `tag`; the corrected source-derived regression passed. Final review then
found a second material ordering mismatch: Rust string ordering is not Java
UTF-16 `String.compareTo` for supplementary Unicode. This affects retained
tags, suite labels, package equality, and query projection.

The correction budget was exhausted, so the packet closed `REPLAN`; all nine
files were restored exactly to `HEAD` and no Rust code was retained. The
replacement packet is design/oracle-only: establish exact Bazel string and
label comparators, including BMP/supplementary and duplicate discriminators,
before Gate A retries. Strict policy, function activation, and formatters
remain deferred.

## Order-independent value audit replanned to broader oracle (2026-07-23)

Pinned source and Bazel 9.2 executable evidence disproved the preceding
UTF-16 premise for BUILD values. Bazel's hidden UTF-8-byte-string mode is on
by default, and the parser maps each source byte to a Java character before
`Ordering.natural()` runs. Valid literal order is therefore UTF-8 byte order:
ASCII, U+E000, then U+10000. Rust string ordering already matches, and
duplicate string tags remain present.

Labels sort structurally by canonical repository, package path, then target
name; rendered-label sorting is observably wrong because `//a:b/c` precedes
`//a/b:a`. An allocation-free identity-owned comparator may ignore V2
`mapping_id` without changing global `Ord`, `Eq`, or `Hash`.

The same audit exposed a real prerequisite. `RuleClass` rejects duplicate
canonical labels in every direct `LABEL_LIST` after conversion, so `member`
and `:member` collide. Current V2 tests instead preserve native filegroup and
Starlark label-list duplicates. Sol accepted the comparator boundary but
returned `REPLAN` because a suite-only duplicate oracle would not cover those
already exposed surfaces.

The replacement packet is oracle-only. Extend the labels fixture with exact
native-filegroup and direct/unconditional Starlark-label-list duplicate
errors, and the tests fixture with suite duplicate rejection plus successful
string and structural-label ordering. Configurable selector duplicate
semantics, malformed input bytes, loading fixes, Gate A metadata, strict
policy, function activation, and formatters remain separate.

## Ordering and duplicate-label oracle accepted (2026-07-23)

Commit `57192df9` extends `query-labels-attribute-metadata` from 37 to 39
Bazel commands and `tests-query-expansion` from 25 to 29. Exact rows now pin
native filegroup, direct unconditional Starlark label-list, and native suite
duplicate errors after equivalent spellings canonicalize to one label.
Successful build-output rows pin duplicate-preserving UTF-8 byte order for
Starlark and native tags, structural explicit-suite label order, and implicit
suite member order.

Generation and clean verification passed both fixtures; root's final runs
were labels `20260723-135203-1126830-bazel` and tests
`20260723-135233-1129462-bazel`. Root corrected an initial illegal explicit
`configurable=False` Starlark declaration to a default schema with a direct
unconditional value, and retained only append-only expected records rather
than regenerated UUID/path/graph noise. Sol returned `ACCEPT`.

The next prerequisite adds the allocation-free structural canonical-label
comparison and corrects direct native/Starlark label-list duplicate behavior.
Selectors, malformed bytes, suite implementation, strict policy, function
activation, and formatters remain deferred.

## Structural label and direct duplicate prerequisite accepted (2026-07-23)

Commit `5bbc4604` adds `CanonicalLabel::bazel_natural_cmp` over borrowed
canonical repository, package, and target strings while leaving global
mapping-sensitive equality, hashing, and derived order unchanged. Loading now
uses a Buck-derived compact set of borrowed identity tuples to reject the
first repeated canonical label in native filegroup and direct Starlark
label-list values, including materialized defaults.

The six-file patch preserves all nonduplicate list order, formats root
diagnostics as Bazel `//pkg:target`, and does not descend into selectors,
concatenations, dictionaries, or outputs. Focused identity, loading,
same-DICE invalidation, and query suites passed 70 tests; formatting, archive
integrity, and diff checks passed. Sol accepted both semantics and hot-path
utility reuse without correction.

Gate A may now reuse the comparison and duplicate helper for native suite
members. String tags use native Rust byte ordering with duplicates retained.
Strict policy, function activation, formatters, selector duplicate semantics,
and malformed bytes remain separate.

## `tests` loading/query metadata Gate A accepted (2026-07-23)

Commit `7abcbdce` adds native `test_suite`, invariant-safe explicit or implicit
membership with orthogonal input provenance, typed inherited Starlark
`tags`/test `size`, exact test-versus-suite capability, per-suite implicit
filtering, and total query-attribute explicitness. Package-context suite labels
reject canonical duplicates before structural sorting; tag strings retain
duplicates in UTF-8 byte order. The unconfigured graph projects distinct
`tests` and `$implicit_tests` attributes, scalar test metadata, and deduplicated
ordinary edges without collapsing stored label values.

Focused loading 16, bzl invalidation 21, glob invalidation 2, and query 29 tests
passed independently. Full `slug_loading_v2` and `slug_query_v2` suites passed,
`slug_cli_v2` rebuilt, and formatting, archive, and diff checks were clean.
Same-DICE evidence covers semantic reorder reuse, metadata changes, duplicate
error and recovery, omitted/explicit-empty transitions, deletion, and
recreation. Sol-low returned `ACCEPT` without correction and confirmed that
`tests()` activation, strict policy, formatters, repository behavior, and
V1/Buck semantics did not enter Gate A.

The next packet is design-only: review request-local
`--strict_test_suite` ownership and the bounded evaluator/diagnostic seam for
activating the already accepted 29-command `tests(EXPR)` oracle.

## `tests()` activation design replanned to oracle discriminators (2026-07-23)

Pinned-source and V2 auditing confirmed that Gate A can express every
non-formatter row: 18 current `tests()` rows plus six existing
labels/deps/loading rows. Five `--output=build` rows remain Bazel-only
representation evidence; two additionally require source location and
instantiation-stack state that V2 does not retain.

The proposed activation keeps `--strict_test_suite` request-local, outside all
package/graph/DICE identity; uses the generic evaluator with accessor-shaped
loading primitives; evaluates the operand once; and recursively expands suites
with separate compact test/suite uniquifiers. Source requires filtering before
test uniqueness, suite-local rather than inherited nested filters, and literal
`-+tag` handling. Sol-low returned `REPLAN` because the 29-command oracle does
not yet discriminate those three choices.

No Rust or fixture change was made during the audit. The replacement packet is
oracle-only and appends three successful-query rows before the activation
design is reviewed again. Error work is narrowed: lookup retains crate-private
missing-target versus package-loading detail, the function adds exact
suite/attribute or strict text, and no unused public/general failure-code API
is introduced.

## Source-critical `tests()` oracle discriminators accepted (2026-07-23)

Commit `1edb2775` extends `tests-query-expansion` from 29 to 32 Bazel 9.2.0
commands. One isolated package now proves that parent filters do not propagate
to nested suites, an excluded direct route does not consume global test
uniqueness before a valid nested route, and `-+tag` excludes literal `+tag`
rather than `tag`. Each wrong algorithm yields an empty or different
single-label result.

Worker generation and clean verification passed, and root independently passed
run `20260723-144341-1175813-bazel`. All original 29 command definitions and
expected records remain unchanged; Sol-low returned `ACCEPT`. The corrected
activation design may now be re-reviewed. Its eventual Slug gate is 27
non-build rows; five exact build-format rows remain Bazel-only evidence.

## Corrected strict-suite and `tests()` activation design accepted (2026-07-23)

The re-reviewed implementation boundary adds one copyable request-local
`QueryPolicy`, ordinary-query-only Bazel boolean parsing, and a serde-defaulted
daemon field. Policy flows by value through the authoritative one-shot and
retained-daemon paths into the loading environment and never enters package,
graph, DICE key/equality, or user-data identity.

A generic iterative `TestsFunction` evaluates once, materializes by label, and
uses separate compact test/suite uniquifiers. Loading supplies only
accessor-shaped classification, metadata, and named suite-attribute resolution
while recording ordinary evaluation edges. Filtering precedes test uniqueness;
nested suites use their own filters; strict applies only to explicit non-test
members; implicit members accept only tests. Lookup retains crate-private
missing-target versus package-loading cause, while the generic function owns
the exact prefix and strict text. No public/general error-code API is added.

Sol-low returned `ACCEPT` after `1edb2775`. Implementation is limited to the
named command, CLI, server, core-runtime, query, and matching test files. The
acceptance gate is 21 function rows through one-shot and daemon paths plus the
exact 27 non-build fixture set, request-toggle reuse evidence, full owning and
downstream tests, CLI rebuild, and format/archive/diff checks.

## Request-local strict-suite and `tests()` activation accepted (2026-07-23)

Commit `3a8ae78a` activates `tests(EXPR)` through one copyable default-false
request policy, ordinary-query-only positive/negative boolean parsing, a
serde-compatible daemon field, and policy-aware one-shot, retained-runtime,
and evaluator paths. The policy remains outside package, graph, DICE key,
equality, and user-data identity.

The generic iterative implementation evaluates its operand once, drops fake
and top-level non-test candidates without loading lookup, uses separate
compact test/suite uniquifiers, filters before test uniqueness, applies
suite-local nested filters and literal `-+tag`, diagnoses only explicit
non-test members in strict mode, accepts only tests from `$implicit_tests`,
terminates cycles, and emits one delivery. Named suite-attribute access records
ordinary query edges. Crate-private target-missing versus package-loading
classification preserves exact function-owned missing-member prefixes without
adding a public diagnostic taxonomy.

The exact 27-row one-shot gate passed: all 21 `tests()` rows plus six existing
non-build labels/deps/loading rows, with all five build-format rows excluded.
All 21 function rows also passed through one retained daemon. Unchanged
false/true/false strict toggles reported zero invalidated files and reused the
same unconfigured package graph. Root independently passed 13 command, 55
query, 10 core-runtime, 16 server, 21 CLI, and three graph-output tests, rebuilt
`slug_cli_v2`, and passed formatting, archive, and diff checks. Sol-low returned
final `ACCEPT` without correction.

M3 now implements 12 of Bazel's 16 default loading-query functions. The next
packet is read-only: audit pinned Bazel visibility representation and current
V2 loading/query ownership, then design the smallest truthful oracle for
`visible(PREDICATE, INPUT)`. No production visibility implementation or
activation is authorized by this acceptance.
