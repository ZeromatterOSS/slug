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
