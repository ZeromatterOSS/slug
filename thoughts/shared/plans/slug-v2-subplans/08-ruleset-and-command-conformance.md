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

### Current M3 status

Live Status in the canonical plan owns scheduling. The root-repository query
command is user-visible with default/explicit `label`, graph, `label_kind`, and
`package` output and 13 of Bazel 9.2's 16 default functions. Output-specific
kind completion loads only otherwise unresolved selected kinds; standard label
and graph dependencies and ordering stay unchanged. Focused cross-package
failure/edit/recovery proves that boundary.

`attr`, `filter`, and `kind` remain blocked on an exact Java-compatible
`Pattern` substrate. External repositories, pattern breadth, and other output
formats also remain. The old tests-metadata Gate A and subsequent `tests()`,
`labels()`, `executables()`, and `visible()` packets are accepted history, not
live instructions. When M3 resumes, select one bounded user-visible gap and
reuse accepted Bazel 9.2 evidence.

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

`8fec2696` activates exactly `labels`; six ordinary functions remained deferred
at acceptance. Its 29 rows (two complete graph stdout rows included) are exact;
the two then-formatter-deferred GeneratedFile constraints are now activated by
`WP-8-m3-query-label-kind-output`. Package-load QueryError alone gets Bazel
framing; syntax/unrelated eval diagnostics remain unchanged.
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
then-formatter-deferred `label_kind` rows are now activated by
`WP-8-m3-query-label-kind-output`. DICE and retained-daemon evidence covers
capability, exported-class, target-name, formatting, and
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

## Bundled Java `Pattern` query-functions contract replanned (2026-07-27)

`WP-8-m3-query-java-pattern-functions-contract` ends in **REPLAN**. The
requested all-three implementation packet is not one bounded abstraction, and
no exact Rust matching substrate is currently accepted. No Rust, Cargo,
dependency, fixture, generated oracle, query registry, evaluator, graph,
loading, glob, or routing-log file changed in this contract audit.

Bazel 9.2
`query2/engine/{RegexFilterExpression,FilterFunction,KindFunction,AttrFunction}.java`
compiles arbitrary user patterns with `java.util.regex.Pattern` and calls
`Matcher.find`. `filter` supplies the printed label and `kind` supplies the
target-kind string. `attr` is a separate representation problem:
`TargetUtils.getAttrAsString` visits every configurable value and performs
type-specific formatting, including boolean/tristate integer compatibility,
ordinary object/list formatting, fully qualified labels, and null
suppression.

The current V2 `QueryNode` already retains exact label and target-kind strings,
so `filter` and `kind` need no loading representation change. Its
`QueryAttribute`, however, retains only name, dependency labels, and explicit
provenance. That is sufficient for accepted `labels()` behavior but cannot
represent Bazel's string, string-list, dict, boolean/tristate, output, null, or
all-selector-branch `attr()` match universe. Extending it is a distinct
Stage 4/public-cross-crate identity and equality packet, not a regex-function
implementation detail.

The immutable `java_regex` 0.1.0 candidate remains rejected. Its 5,421-line
Rust engine is useful reference material, but the pinned source still lowers
lone `\uD800`/`\uDC00` pattern units with
`char::from_u32(...).unwrap_or('\0')`; oracle `5e78abc1` proves that changes a
boolean `find` result. It also copies subjects into scalar vectors, allocates
match/group state, reports scalar rather than Java UTF-16 diagnostic
positions, and converts fixed step/depth exhaustion to ordinary non-match.
Rust `regex`, `fancy-regex`, PCRE, and Onig remain dialect/behavior
substitutions. Executing JVM bytecode, embedding a JVM, or delegating
production behavior to Bazel/Java is forbidden by `AGENTS.md` and is not an
implementation option.

A future Pattern substrate proposal must therefore supply an independently
qualified Rust-owned UTF-16 compile/boolean-find boundary for arbitrary Java
patterns, exact syntax diagnostics, no lone-surrogate aliasing, and explicit
resource/error semantics. It may then enable `filter` and `kind` together.
`attr` follows only after a separate source-backed Stage 4 attribute-string
projection owns every value alternative and participates in package/query
equality. Do not describe a common-pattern happy-path oracle as proof of either
full engine or `attr` parity.

Next M3 work should select a bounded user-visible query gap that does not
depend on Java `Pattern`. Prefer one existing-graph output mode with an exact
Bazel 9.2 oracle and one-shot/retained-daemon coverage, keeping the accepted
13-function registry unchanged.

Independent terminal review returned `ACCEPT`: the all-three bundle is
source-invalid, the filter/kind versus attr split is exact, and no already
bounded Rust path was overlooked.

The next packet is
`WP-8-m3-query-package-output`, one logical oracle-plus-implementation packet.
Extend only `query-loading-thin-vertical/{fixture.toml,expected/oracle.json}`
with three `--output=package` rows proving main-root empty spelling,
lexicographic package sorting/deduplication across repeated rule/source
results, nested packages, dependency results, and a fake load-file target.
Protect every existing row and workspace asset; this is oracle packet four
after checkpoint `e2cc891d`, so no growth review is due.

Implementation may change only
`slug_commands_v2/src/{common,query}.rs`,
`slug_commands_v2/tests/commands.rs`,
`slug_query_v2/src/output.rs`,
`slug_query_v2/tests/loading_query.rs`,
`slug_cli_v2/src/commands/query.rs`,
`slug_cli_v2/tests/cli.rs`,
and `slug_server_v2/src/{lib,tests}.rs`. Add an explicit `Package` output
format and render from the already-selected labels without DICE re-entry:
derive each package identifier, strip leading `//` only for the main
repository, sort lexicographically, deduplicate, and emit one line each.
`--order_output` must not change package order. One-shot and retained-daemon
outputs must be exact.

Do not change Cargo/dependencies, query functions/registry, parser grammar,
graph identity/equality, loading metadata, evaluator traversal, target-pattern
breadth, cquery/aquery behavior, JVM/regex code, other fixtures, or workspace
assets. Validate one pinned Bazel 9.2 generation of the three exact rows, exact
Slug replay of changed and protected fixture rows, focused
command/query/CLI/server tests, the direct four-crate suite, formatting, diff,
archive, and daemon cleanup. Stop if formatting needs new graph state,
external-repository loading, a DICE key, or any behavior beyond Bazel 9.2
`PackageOutputFormatter`.

The package-output packet is accepted. Three generated Bazel rows, two
fresh-root Bazel replays, and exact Slug replay prove root-package blank
spelling, sorting/deduplication, dependency projection, and fake load-file
packages. The direct four-crate suite passed 136 tests; formatting, diff,
archive, daemon cleanup, exact allowlist, and preservation of all 12 prior
rows passed. Independent review returned `ACCEPT`; external repositories remain
outside this root-repository slice.

## Rank output contract replanned (2026-07-27)

Bazel 9.2 `MinrankOutputFormatter` walks the strong-component graph by
insertion-ordered successor layers for auto output; `MaxrankOutputFormatter`
also preserves component iteration order within equal ranks. A one-rule native
probe gave `rank_start` both a direct edge to `linear_end` and the longer
`linear_start -> linear_mid -> linear_end` path. Bazel minrank auto printed
`linear_start` before `linear_end`, matching source dependency order, while
full order printed `linear_end` first by label. Cycles correctly shared rank
zero and placed their exit at rank one.

V2 `SelectedQueryGraph` sorts every successor list by selected-node index in
`loading_environment.rs`, so the original dependency order required by exact
auto rank output is no longer retained. Recovering it requires a deliberate
retained-graph or output-completion boundary, outside the formatter-only
packet. The six-row draft and one-rule probe asset were removed; no Rust,
fixture, dependency, graph, DICE, or Stage 9 change remains.

`WP-8-m3-query-label-output` is next: accept Bazel's explicit `--output=label`
as the existing default label renderer and reject the prototype-only
`--output=text` with Bazel 9's invalid-format diagnostic.

## Explicit label output accepted (2026-07-27)

Three Bazel 9.2 rows prove explicit label equivalence with default output under
auto/full order and reject `--output=text` with exit 2 and the exact
12-formatter valid-value list. Pinned generation
`20260727-120133-118527-bazel`, exact Bazel replay
`20260727-120219-121350-bazel`, and exact Slug replay
`20260727-120504-126550-slug` passed while preserving all 43 prior path rows.
The direct command/CLI/server suites passed 75 tests; formatting, archive,
diff, daemon cleanup, and exact nine-file scope passed. The internal default
text discriminator remains request-local for aquery/default routing and is not
a public query output format.

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

## Visibility representation audit and corrected oracle design accepted (2026-07-23)

Pinned Bazel 9.2 source shows that `visible(PREDICATE, INPUT)` is not an
accessor-only packet. It evaluates the predicate once and retains an input only
when it is visible to every predicate target; an empty predicate is vacuously
true. Visibility always includes the same package plus the one-way matching
`javatests/X` caller to private `java/X` target. Package-group contents are
recursive OR alternatives with cycle suppression; negatives override
positives only inside one group, so a separate direct or included positive can
re-allow a package.

The prerequisite is a Stage 4 typed visibility/package-group representation.
Bazel distinguishes raw declared visibility from effective package defaults.
`labels(visibility, rule)` projects the stored raw rule attribute: explicit
loadable group labels project, omitted visibility stays empty even when a
package default applies, and explicit direct `__pkg__`/`__subpackages__`
values fail target lookup as non-loadable pseudo-labels. Ordinary `deps`
instead projects effective loadable group labels, including inherited groups.
Those rule edges are NODEP; package-group includes are separate structural
edges. Generated files inherit their rule; real source and BUILD targets use
real visibility; package groups and fake query `.bzl` load targets are public.

The design review first misclassified inherited `labels(visibility, ...)` by
following the mapper's general visit-attribute special case. The executable
oracle exposed the actual `LabelsFunction` → `TargetAccessor` →
`BlazeTargetAccessor` → `getReachableLabels` path and corrected the contract.
Sol also required include-cycle termination and independent `__pkg__` and
`__subpackages__` lookup failures. Commit `3ecfbfce` now accepts 34 commands:
32 future Slug rows plus two Bazel-only `--noimplicit_deps`/
`--nonodep_deps` structure rows. Worker and root clean Bazel runs passed; final
source/evidence review returned `ACCEPT`.

The first Stage 4 design review returned `REPLAN`. V2 exposes `config_setting`,
and pinned Bazel 9 defaults make an omitted visibility effectively public while
honoring explicit restrictions. The current packet appends two oracle rows for
that producer before design re-review. It also replaces proposed edge buckets
with one ordered tagged slice matching `LabelVisitationUtils` visitation order.

This correction does not implement or activate `visible()`. External
repositories/mapping, symbolic macros, `bind`, alternate visibility flags,
keep-going, configured query, formatters, and production Rust remain excluded
until the corrected design is accepted.

## Config-setting visibility evidence and Stage 4 design accepted (2026-07-23)

Commit `a11b43da` extends `query-visible-visibility` to 36 Bazel commands:
34 future Slug gates followed by the same two Bazel-only flag-structure rows.
The new cases prove that an omitted `config_setting.visibility` remains public
under Bazel 9's default policy despite a private package default, while an
explicit package-group restriction is honored. Worker generation/clean runs
and root clean run `20260723-160559-1242065-bazel` passed; prior normalized
records were preserved and Sol returned `ACCEPT`.

The corrected Stage 4 design also passed review. It stores typed raw/effective
visibility, direct package contents, unresolved group/include labels, producer
provenance, and one ordered tagged edge slice. Loading and graph construction
do not recursively resolve groups: missing/wrong-kind references and cycles
remain topology for Stage 8's future request-local accessor. The exact Stage 4
gate is 12 non-`visible()` rows; Stage 8 owns the remaining 22 future-Slug
`visible()` rows and command activation. No new DICE key, V1 visibility
registry, repository mapping, formatter, or alternate flag support enters the
representation packet.

## Stage 4 typed visibility graph accepted (2026-07-23)

Commit `f9ae7337` lands the loading/query prerequisite. It retains typed
effective and raw visibility, package-group direct contents and unresolved
includes, exact producer provenance, a distinct query node kind, and one
ordered tagged edge slice. Missing visibility/include destinations never
synthesize source nodes; ordinary destinations still do. The implementation
adds no recursive group evaluation, DICE key, registry, formatter, or command
activation.

The exact 12 non-`visible()` rows pass through a counted CLI table. Root also
passed all 48 loading, 56 query, and 26 CLI/graph tests, rebuilt the CLI, and
passed formatting, diff, and archive checks. Sol required one focused
correction for `native.config_setting` in loaded macros, then returned final
`ACCEPT`.

The first Stage 8 design audit returned `REPLAN` only for missing executable
discriminators. Append three Bazel rows before activation: cross-package
top-level plus included-group resolution, real-first real/fake same-label input
identity, and label-keyed predicate materialization over two same-label fake
callers with different consuming packages. The corrected fixture count is 39:
25 `visible()` rows, 12 accepted Stage 4 rows, and two final Bazel-only rows.

The executable correction shows that the activation seam must use the current
printed-label `eval_all` for the once-evaluated predicate, retain the
once-evaluated input's callback batches, and pass
`TargetSet` plus `Set` to the request-local accessor. The accessor uses the
retained fake representative's consuming-package ownership, emits singleton
passing deliveries, resolves groups without recording query topology, and
reuses existing DICE package graphs. Production work remains blocked until the
three-row oracle is generated, independently verified, and accepted.

Commit `a376e30e` accepts that oracle prerequisite. Generation plus two clean
Bazel runs passed, the prior 36 normalized records are unchanged, all 27
source anchors resolve, and independent evidence review returned `ACCEPT`.

The observed same-label fake-caller result corrected the initial design audit:
predicate callers are materialized by label through `TargetKeyExtractor`, so
the generic function must use the existing `eval_all` and pass
`TargetSet` plus streamed input `Set`. Re-review only this corrected activation
design before editing query production code.

Independent re-review returned `ACCEPT` for the corrected Stage 8 activation
design. The implementation packet owns only `expr`, generic dispatch, the
request-local loading accessor, focused query/loading tests, the exact 25-row
CLI gate, and daemon/lifecycle evidence. Predicate callers materialize by
label, inputs stay streamed, passing candidates become singleton deliveries,
and visibility lookup records no query topology.

The lifecycle gate must remove and recreate the included `package_group`
definition while leaving its BUILD package present, then assert the pinned
top-root-wrapped missing-target diagnostic and recovery. Missing-package
semantics remain unclaimed.

Implementation commit `76025ede` activates `visible()` as the thirteenth of
the 16 Bazel default loading-query functions. The once-evaluated predicate is
materialized by printed label, input callback batches remain streamed, each
passing candidate is emitted as a singleton delivery, fake callers retain the
first representative's consuming-package ownership, and fake inputs are
public. Real visibility uses non-recording existing-key lookups and fully
resolves every restricted group root with fresh cycle state, include-first
source order, local negatives, wrong-kind ignore, and top-root-wrapped missing
target errors.

All 25 exact `visible()` rows pass one-shot and through one retained daemon;
the prior 12 non-`visible()` visibility rows remain green. Same-DICE tests prove
formatting-only reuse of leaf/target/top/viewer, semantic reevaluation only of
the changed leaf graph, included-group definition delete/recreate recovery,
no filtering topology, and ordered singleton delivery shape. Root passed all
61 query tests and 28 CLI/graph tests after rebuilding `slug_cli_v2`, plus
formatting, diff, archive, and stale-daemon checks. Independent final review
used one focused evidence correction, then returned `ACCEPT`. The remaining
three functions depend on an exact Java-compatible `Pattern` substrate and
remain deferred.

## Rust-native regex compatibility reset (2026-08-08)

The exact Java `Pattern` requirement above is historical. Explicit user
direction permanently forbids JVM/Java-bytecode integration and admits a named
Slug-native valid-Unicode Rust regex surface without Java lone-surrogate,
dialect-edge, renderer, or diagnostic parity. Existing exact query functions
remain exact; no prior Java-regex rejection becomes an implementation claim.

M3 remains parked while M2 establishes complete semantic configuration identity.
A later reviewed packet may select the Rust regex crate and freeze supported
syntax, compile/match behavior, diagnostics, resource limits, and fail-closed
handling before enabling `filter` and `kind`. `attr` still requires its separate
typed attribute-string representation, and rank/external/pattern/formatter
representation gaps remain unchanged. No Java helper, standalone Java probe,
bytecode, embedded/launched JVM, or production Bazel delegation is permitted.

M2 structural configuration identity and the bounded M4 cquery projection are
now accepted. Run next only `WP-8-m3-rust-native-regex-contract-design`, a
documentation/source-audit packet. Select the existing locked Rust regex
substrate and freeze one explicit Slug-native valid-Unicode syntax, compile,
search, diagnostic, resource-limit, and fail-closed contract for `filter` and
`kind`. Preserve the exact Bazel matcher inputs and compile-once/find call
shape around the named dialect divergence. Obtain independent public-boundary
review before scheduling Rust. `attr`, query graph/identity changes, DICE regex
keys, cquery/aquery breadth, Cargo edits, UTF-16/lone-surrogate emulation, and
all JVM/Java artifacts, helpers, execution, or delegation remain excluded.

## Rust-native `filter`/`kind` regex contract (2026-08-09)

`WP-8-m3-rust-native-regex-contract-design` selects the workspace `regex`
crate, not `fancy-regex`. `Cargo.lock` currently selects `regex` 1.13.1 with
crates.io checksum
`f020237b6c8eed93db2e2cb53c00c60a8e1bc73da7d073199a1180401450218d`;
the ordinary workspace dependency enables its default `std`, `perf`, and full
`unicode` feature families. The only locked `fancy-regex` is unrelated
transitive version 0.11.0, while the unused workspace declaration names 0.16.0.
More importantly, `fancy-regex` adds a backtracking engine and runtime error
surface for constructs this contract deliberately does not admit. The selected
finite-automata substrate guarantees worst-case `O(m * n)` single-search time
and provides explicit NFA compile and lazy-DFA cache bounds.

This is a named Slug-native compatibility surface. It is the string syntax of
locked `regex` 1.13.1 over Rust `&str`, with the builder settings below:
literals and escapes; concatenation and alternation; greedy/lazy repetition;
numbered, named, and non-capturing groups; Unicode/ASCII character classes and
class set operations; `^`, `$`, `\A`, `\z`, and word-boundary assertions; and
inline `i`, `m`, `s`, `R`, `U`, `u`, and `x` flags. Look-around,
backreferences, Java character-class intersections, Java escape rules, and all
other constructs rejected by that parser are unsupported syntax. Patterns and
candidate strings contain only valid UTF-8/Unicode scalar values. Slug neither
constructs nor emulates lone UTF-16 surrogates, and makes no Java dialect,
Unicode-table, case-folding, match-offset, or diagnostic-parity claim.

The implementation must construct one `regex::Regex` before evaluating the
operand and then reuse it for every candidate. Quoted query `WORD` parsing
continues to remove only the surrounding quote characters; it performs no
second regex-escape pass. Matching is unanchored search through
`Regex::find(...).is_some()`, equivalent to Bazel's surrounding `Matcher.find`
call shape; anchors have effect only when the user writes them. Captures are
never requested. The operand is evaluated exactly once after successful
compile, and every existing nonempty callback delivery is filtered in place,
preserving candidate IDs, order, provenance, and delivery boundaries and
dropping only empty deliveries.

The four limits are public Slug constants, explicitly set on every builder so
dependency-default changes cannot silently alter the contract:

- at most 4,096 UTF-8 bytes in the pattern before parsing;
- a 1,048,576-byte approximate compiled-NFA limit through `size_limit`;
- a 1,048,576-byte lazy-DFA transition-cache capacity through
  `dfa_size_limit`; and
- a parser nesting limit of 128 through `nest_limit`.

The builder also sets every semantic default explicitly:
`case_insensitive(false)`, `multi_line(false)`,
`dot_matches_new_line(false)`, `crlf(false)`, `line_terminator(b'\n')`,
`swap_greed(false)`, `ignore_whitespace(false)`, `unicode(true)`, and
`octal(false)`. Inline pattern flags may locally change only the settings that
the admitted syntax exposes.

The first limit bounds the parser AST/HIR allocation that the upstream nesting
limit does not bound. The NFA limit bounds counted-repetition expansion and
the `m` term in search. A full lazy-DFA cache resets and ultimately falls back
to another finite-automata engine; it is not a failed or false match. The crate
exposes no ordinary per-search resource error from `find`, so there is no
timeout or resource-exhaustion-as-nonmatch path. A dependency panic, allocator
failure, cancellation, or process termination remains infrastructure failure
and must never be caught and rewritten as nonmatch.

Every admitted pattern either compiles before operand evaluation or returns a
syntax `QueryError` with exit code 2 and no query stdout. Diagnostics are
entirely Slug-owned and deliberately do not expose dependency prose:

- length rejection is exactly
  `Slug regex resource limit exceeded: pattern is longer than 4096 bytes`;
- `regex::Error::CompiledTooBig(_)` is exactly
  `Slug regex resource limit exceeded: compiled program is larger than 1048576 bytes`;
  and
- `regex::Error::Syntax(_)` or any future non-exhaustive build-error variant is
  exactly `invalid Slug regex: unsupported or malformed syntax`.

Nesting-limit rejection is an invalid-syntax case because the public `regex`
error type does not separately classify it. All compile failures are
fail-closed errors, never an empty set and never a nonmatch.

The candidate representation remains the already accepted one. `filter`
matches `QueryCandidate::printed_label().output_label()`: root labels such as
`//pkg:t` and apparent external labels such as `@repo//pkg:t`, including fake
load-file candidates. `kind` matches exactly the existing selected-node kind
projection: `source file` for BUILD/source/fake load-file candidates,
`generated file`, `package group`, or `<retained rule_class> rule`. It may
demand-load a real candidate only through the existing package-graph path used
by `label_kind`; this can request an ordinary typed query restart but records
no new evaluation edge. It must not infer kind from label spelling or from
configuration/analysis state.

The bounded implementation successor is
`WP-8-m3-rust-native-filter-kind-implementation`. It may add only
`regex.workspace = true` to `app/slug_query_v2/Cargo.toml` and the consequent
single `"regex"` direct-dependency edge in `slug_query_v2`'s existing
`Cargo.lock` package entry (every selected package/version/checksum must remain
unchanged), activate `filter` and `kind` in `expr.rs`,
add their shared compile-once generic dispatch in `generic.rs`, add the two
delivery-preserving loading-environment accessors in
`loading_environment.rs`, and add focused core/loading and existing-fixture
CLI/daemon tests. No command or server production wire changes are required.

The acceptance matrix must discriminate substring versus `^`/`$` anchoring;
literal and escaped punctuation; Unicode scalar literals, `\p{...}` classes,
case folding, and inline flags; apparent external and fake labels; all retained
kind strings; invalid look-around/backreference syntax; 4,096 versus 4,097
UTF-8 bytes; nesting and compiled-NFA rejection; no operand evaluation or
stdout after compile failure; repeated use of one compiled pattern; streamed
delivery order/provenance; AUTO/FULL/default and `label_kind` presentation; and
one-shot versus retained-daemon equivalence. Common-dialect label/kind behavior
must cite immutable Bazel 9.2 source commit `8220c619...` for compile-before-
evaluation, one compile, printed-label/target-kind inputs, find semantics, and
filtered callback delivery. Rust-dialect/resource rows use the locked crate
source and focused Slug tests, not a Java oracle.

Stop and `REPLAN` on a `Cargo.lock` change beyond that one mechanical local
dependency edge, any selected package/version/checksum change, a need for
backtracking or unsupported syntax, silent compile/search failure as nonmatch, regex state in
DICE/package/query identity, a new query edge or candidate representation,
`attr`, cquery/aquery/configuration breadth, Java parity claims, UTF-16 or
surrogate handling, or any JVM, Java source/bytecode/helper, or Bazel
delegation. Exact Java `Pattern` behavior remains permanently out of scope;
`attr` remains deferred behind its typed attribute-string projection.

Independent Sol-low public-boundary review returned `ACCEPT` without a
correction. The contract packet is complete; schedule only
`WP-8-m3-rust-native-filter-kind-implementation` next.

The implementation preflight exposed one bounded packaging correction: Cargo
must record a newly direct dependency even when its selected crate is already
locked transitively. The correction above admits only the one local package
edge and keeps `Cargo.Bazel.lock` plus every external selection/checksum
byte-identical. No regex behavior, ownership, resource, or query boundary
changed; obtain one focused independent correction rereview before accepting
the implementation.

The first implementation packet ends in `REPLAN`. Production, query, CLI, and
lockfile validation was green, but the terminal Sol review found that rendered
output plus the existing generic delivery helper test did not directly prove
regex filtering across multiple nonempty callback batches with distinct full
candidate provenance. This is the second material correction after the
accepted Cargo lock-edge correction, so it may not be folded into the same
packet.

Retry only as `WP-8-m3-rust-native-filter-kind-implementation-retry`. Preserve
the reviewed implementation draft as uncommitted retry input and add one
focused loading-environment discriminator that invokes the regex `filter` and
`kind` paths over at least two nonempty batches, includes distinct fake
consuming owners, drops selected candidates, and asserts surviving candidate
IDs, within-batch order, owners/provenance, nonempty batch boundaries, and
empty-batch removal. No production behavior, file scope, dependency, contract,
or acceptance gate may otherwise change. Rerun the frozen validation and
obtain one fresh latest-diff review.

The retry is accepted. `filter` and `kind` now share the locked `regex` 1.13.1
builder and exact Slug-owned limit/diagnostic contract, compile once before
operand evaluation, use find/search semantics, and filter the accepted
candidate batches without changing graph or DICE identity. Label matching uses
apparent printed output labels, including external and fake load candidates;
kind matching uses only the retained source/generated/package-group/rule-class
projection. The only lockfile change is the reviewed local direct-dependency
edge.

The retry-only test directly proves three streamed fake-candidate deliveries,
two distinct consuming owners, partial first/third filtering, complete middle
delivery removal, stable IDs/order/provenance, and `kind` all-preserve/all-drop
behavior. Root validation passed 31 query library tests, 56 loading tests, six
parser tests, two focused one-shot/retained-daemon CLI tests, the
`slug_cli_v2` build, formatting/diff checks, and stale-daemon cleanup. Fresh
Sol-low latest-diff review returned `ACCEPT`. M3 now implements 15 of Bazel's
16 default loading-query functions; `attr` remains deferred behind its typed
attribute-string representation.

Run next only `WP-4-8-m3-attr-typed-attribute-string-design`, a Stage 4/Stage 8
documentation and pinned-source audit. It must close the complete currently
admitted coerced-value formatting, selector/default/provenance, equality, and
invalidation boundary before any representation Rust or `attr` activation.

That design packet ends in `REPLAN`. Bazel matches every non-null whole typed
candidate after selector combination and preserves observable candidate order
and multiplicity. V2 has already detached selector-default position, normalized
some ordered values, omitted native/universal attributes, and reduced
`QueryAttribute` to labels plus explicitness. No exact Stage 8 formatter can
recover those facts. Leaf strings are source-closed; the remaining evidence gap
is ordering/correlation and complete native-rule inventory. Run next only the
focused `WP-4-8-m3-attr-candidate-order-oracle-design`; `attr` remains deferred
and the accepted Rust-native regex boundary is unchanged.

The candidate-order oracle design corrects that conclusion and ends in
`REPLAN` before fixture generation. Bazel's internal candidate order and
multiplicity are not observable through ordinary `attr()`: the generic regex
filter performs a pure existential search and emits only the selected target.
No successful query row can distinguish default-first from default-last,
candidate reordering, or duplicate-candidate suppression. Those facts must not
drive Slug representation merely because they occur inside Bazel machinery.

Observable whole-value semantics remain strict. Equal selector key sets
correlate while different—even overlapping—sets form typed cross-products;
string/list concatenation precedes formatting; order and duplicates inside one
list or map candidate remain matchable; null candidates disappear; and label
leaves use canonical rather than apparent external repository names. The
universal `name` attribute and every loadable native rule also prevent a
Starlark-only activation. In particular, the current graph rejects native
toolchain target variants, so the successor must expose that prerequisite
rather than claiming the final default function from current `QueryAttribute`.

Run next only `WP-4-8-m3-attr-observable-candidate-oracle-design`, a Stage 4/
Stage 8 documentation and fixture-design packet. It must design exact paired
membership/nonmembership rows for every observable combination, formatting,
default, implicit, canonical-label, and native-inventory boundary; explicitly
exclude candidate position and equal-candidate multiplicity from the contract;
and select the smallest fixture shape under the existing growth checkpoint.
No `attr` activation, graph broadening, representation Rust, query-time
loading, DICE identity, regex change, or JVM/Java artifact is authorized.

That oracle design reaches `REPLAN` before selecting a fixture. The proposed
retained-field matrix does not cover Bazel's actual accessor: `attr()` looks up
the full `RuleClass` schema. Current native and Starlark targets therefore have
observable inherited and hidden values that `QueryAttribute` does not carry,
including boolean `0`/`1`, empty typed defaults, class-specific `[manual]` and
test-only overrides, computed/package defaults, late-bound loading defaults,
macro `generator_*` provenance, and Starlark-test `@bazel_tools` labels.

This is not configured-query breadth and cannot be dismissed as an unsupported
BUILD argument. The target already exists in the admitted ordinary-query graph,
and an arbitrary attr name can select it from its default or automatically
populated value. A schema limited to V2-accepted call arguments would therefore
make the sixteenth default function only partially compatible while reporting
it as exact.

Run next only `WP-4-8-m3-attr-total-ruleclass-schema-source-ledger-design`, a
Stage 4/Stage 8 pinned-source documentation packet. Close the finite schema,
typed loading-value source, removals/overrides, null behavior, normalization,
canonical-label, and macro-provenance ledger before returning to fixture
design. The later oracle must derive a minimal discriminator for every ledger
equivalence class and class-specific exception, not one row per redundant
empty default. Candidate position and equal-candidate multiplicity remain
unobservable and excluded. No fixture generation, graph broadening,
representation, query activation, DICE work, regex change, JVM/Java artifact,
or production Bazel delegation is authorized.

The total RuleClass ledger is now source-closed at pinned Bazel 9.2 commit
`8220c619...`. Ordinary `attr()` consumes loading RuleClass values, including
package-computed, macro-derived, automatic, and late-bound declaration
fallbacks; it does not require configured analysis. Configuration-resolved
action listeners, flag aliases, coverage overrides, and run-under values are
not ordinary-query inputs.

The closed Starlark families have 22 built-ins for normal rules, 25 for
executables, 39 for tests, and 24 for root string build settings, plus user
attrs and the conditional Starlark-transition allowlist attr. The nine native
final counts are filegroup 23, alias 17, config_setting 21, test_suite 21,
constraint_setting 16, constraint_value 15, platform 23, toolchain_type 17, and
toolchain 21. Stage 4 owns their exact schema algebra, defaults, spellings,
removals, overrides, renderers, and pinned source anchors.

Stage 8 must not mistake V2-retained fields for this surface. Loading currently
drops most common/default/native values; `QueryAttribute` keeps only labels and
explicitness; universal `name` is absent; and native toolchain variants are
rejected before graph projection. Total activation therefore needs a typed
loading projection and a separately reviewed native-toolchain graph
prerequisite.

The future oracle families are scalar/BOOLEAN/integer/license rendering;
empty/null behavior; ordered versus order-independent list/map interiors;
every admitted dict orientation; selector correlation and cross-product; typed
concatenation; implicit query names; canonical main/external/`@@bazel_tools`
labels; package and macro defaults; test computed/fixed/automatic values; and
each native inheritance/removal exception. Candidate position and multiplicity
of equal whole candidates remain excluded because existential `attr()` cannot
expose them.

Run next only
`WP-4-8-m3-attr-observable-candidate-oracle-design-retry`. It remains a
documentation and fixture-design packet: choose the smallest fixture shape and
map paired positive/negative rows to every ledger class before fixture
generation or representation work. No graph broadening, query activation,
Rust, Cargo, DICE, regex change, JVM, Java artifact, or production Bazel
delegation is authorized.

The oracle-design retry selects an 18-command extension of
`query-labels-attribute-metadata`, for 57 Bazel rows total. Each new command
unions uniquely labeled positive/negative atomic `attr()` clauses, anchors the
whole-value regex, and expects every positive label exactly once with no
negative label. This retains the existing root/module, definitions, selector,
dictionary, package-label, filegroup/alias, canonical-payload, and harness
scaffolding while preventing union deduplication from masking a missing case.

The 18 lanes cover universal rule-only `name`; scalar/integer/BOOLEAN/license;
empty versus null; ordered/OI list and map interiors; all three user dictionary
orientations; main/generic-external/`@@bazel_tools` labels; equal-key selector
correlation and distinct-key cross-product; typed string/list concatenation;
package and macro defaults; all four Starlark families; test computed/fixed/
late-bound/automatic values; the transition allowlist; the native baseline;
and every class-specific addition/removal across all nine native classes.
Candidate order/default position/equal-candidate multiplicity remain excluded.

The same canonical payload workspace gains only `attr/BUILD.bazel` and a local
external module's `MODULE.bazel` plus `leaf/BUILD.bazel`; root `MODULE.bazel`
and `pkg/defs.bzl` are extended. The generation packet may update only the
fixture TOML/expected JSON, canonical payload, and the Python/Rust derived
payload count/hash/projection assertions. The latter are test-integrity
constants only. All 39 protected rows, 29 accepted Slug CLI rows, and two
generated-kind CLI/server rows must remain semantically unchanged.

The due fixture-hygiene review closes at tree `51540963`: the payload-expanded
corpus is 1,361 regular files, 24 links, 42,520 lines, and 864 rows, with no
removable nondiscriminating asset or stale pre-payload workspace. The successor
is packet one from that reset and is capped at +3 virtual files/+0 links/+18
rows/+1,000 lines; review again before packet six or the ordinary size trigger.

Run next only `WP-4-8-m3-attr-observable-candidate-oracle-generation` after
the fixture-hygiene checkpoint is closed. It adds Bazel-only evidence, not
Slug activation. Native-toolchain graph projection and generic-external graph
consumption through a new path remain separately reviewed prerequisites; later
production work must reuse existing external-loading owners. Stop on
fixture/command/line-cap growth, protected-row drift, weakened canonical
regexes, configured analysis, representation, production Rust, graph/DICE
changes, JVM/Java artifacts, or Bazel delegation.

Independent Sol design review returned `ACCEPT`; the residual generation risk
is dense fixture transcription, bounded by exact canonical-token freezing,
replay of all 39 protected rows, and the stated growth caps.

Generation returned `REPLAN` without retained changes. A draft passed one
Bazel update and one clean replay for 57 rows and froze `@@ext+//leaf:label`,
but completeness review found omitted atoms and positive-label reuse. Its
focused correction then proved the shared fixture is architecturally invalid:
required constructors such as `attr.string_list()` in `pkg/defs.bzl` are loaded
by the protected 29-row Slug CLI consumer before row one, outside Slug's
currently admitted Starlark attr surface. Production expansion is forbidden in
this evidence packet and matrix weakening is not acceptable, so all five draft
files were restored to `6c9a529e`.

Run next only `WP-4-8-m3-attr-isolated-observable-candidate-oracle-design` to
inventory a separate Bazel-only payload workspace/fixture, prove it is absent
from protected Slug CLI/server consumers, and assign a distinct positive and
negative instance to every accepted atom before generation. No fixture,
payload, oracle, production Rust, Cargo, graph/DICE, JVM/Java artifact, or Bazel
delegation is authorized by that design packet.

The isolated retry selects a new payload-backed fixture,
`query-attr-observable-candidates`, with no Rust projection or CLI/server case.
Fixture discovery/listing and the aggregate metadata test see its records, but
oracle execution is explicit by name and every Slug `FixtureWorkspace` remains
limited to the existing static projection allowlist. This proves that current
Slug processes cannot materialize or parse the new definitions.

Its minimal closure is five directories and five virtual files: root
`MODULE.bazel`, `attr/defs.bzl`/`attr/BUILD.bazel`, and local `ext` module/leaf
BUILD files. The base string setting and BUILD-source nonrule live in `attr`;
the root directory is not a Bazel package. `attr` also owns positive package
defaults, macro location, Starlark/native targets, generated output, and
nonrule controls; the local module independently reproduces the generic
external label. No source leaf, registry, lockfile, action, copied
`@bazel_tools`, mutation, configured analysis, or toolchain resolution is
needed.

The accepted 18 lanes become exactly 165 globally unique positive/negative
pairs, 330 probe instances, and approximately 20 support targets. Pair counts
by lane are 13/7/5/4/3/3/3/6/11/12/16/3/23/5/10/16/15/10. No positive label is
shared by two atoms in one command; exact stdout lists every `_yes` once and no
`_no`. Helper macros may compact ordinary declarations, but direct versus
legacy-macro generator provenance remains isolated.

Generation may add only the new fixture TOML/expected, its five-file canonical
payload projection, Python derived global/projection integrity, and Rust global
SHA plus the 275-to-285 entry count. It must not add a Rust projection. The cap
from `51540963` is +7 payload-expanded regular files, +5 directories, zero
links, 18 rows, and 2,400 logical lines; perform another hygiene review before
any subsequent fixture packet. Update plus clean replay must independently
freeze `@@ext+//leaf:label`, retain all fourteen existing projections, and pass
payload metadata/integrity plus the protected 29-row CLI and two generated-kind
CLI/server cases. All 18 rows, lane 9 macro/direct provenance, and lane 12's
transition-allowlist positive must pass. This remains Bazel-only loading
evidence with the permanent Rust-native/no-JVM boundary.

Independent Sol review removed a redundant root `BUILD.bazel`; corrected
rereview accepted the five-file/five-directory, `(285, 117)`, +7-file closure
and left only bounded generation risk. Run next only
`WP-4-8-m3-attr-isolated-observable-candidate-oracle-generation`.

Generation preflight corrected the pair arithmetic before any Bazel run: null
and nonrule negative operands are not standalone pairs, while every named
config-setting/toolchain removal is. The corrected 165-pair vector above is the
packet's sole material contract correction; a second material correction is
`REPLAN`.

That second contradiction occurred before Bazel ran: Bazel's deprecation
computed default always reads the package default and explicit Starlark `None`
does not suppress it. One `//attr` package cannot prove both lane 9's positive
package-derived deprecation and lane 2's same-schema null control. The entire
incomplete fixture draft was removed. Run next only
`WP-4-8-m3-attr-two-package-observable-candidate-oracle-design` to freeze the
smallest isolated positive-default/baseline package layout, remap the corrected
165 atoms, and recalculate caps. No fixture, payload, production Rust, Cargo,
graph/DICE, JVM/Java artifact, or Bazel delegation is authorized.

The two-package design retains five files by using
`modules/ext/leaf/BUILD.bazel` as the baseline package. It canonically loads the
public main definition with `@@//attr:defs.bzl`, keeps
`filegroup(name="label")`, and adds only the same-schema null-deprecation
control. Lane 2 moves its negative operand to
`@@ext+//leaf:l02_a007_no`; its positive and all other 164 pairs are unchanged.
This exactly contrasts package-derived deprecation with a null package default
without a removal class, explicit `None`, or sixth source.

The corrected 165-pair vector, 18 rows, five files/five directories, `(285,
117)` payload totals, +7-file/+5-directory/zero-link/+2,400-line caps, absent
Rust projection, and protected validations remain. A sixth plain package is
redundant. Independent review must accept canonical-main load visibility and
mapping before generation retry; all Rust-native/no-JVM and loading-only
toolchain/external boundaries remain.

Independent Sol review returned `ACCEPT` for the canonical-main load,
package-local null default, unchanged 165-pair ledger, five-file arithmetic,
and isolation. Run next only
`WP-4-8-m3-attr-two-package-observable-candidate-oracle-generation`.

Generation preflight returned `REPLAN` before writes or Bazel because the
accepted plan has no complete stable-ID atom map: only `l02_a007` freezes its
exact attr/regex/yes/no binding. Run next only
`WP-4-8-m3-attr-atomic-discriminator-manifest-design` to freeze all 165 IDs,
schemas, values/absences, regexes, expected labels, and support dependencies in
Stage 4 before another generation attempt. No fixture, payload, Rust, Cargo,
graph/DICE, JVM/Java artifact, or Bazel delegation is authorized.

## `attr` atomic discriminator manifest summary (2026-08-09)

The Stage 4 owner now freezes the complete manifest, rather than leaving ID
assignment to fixture generation. Its canonical record stream contains 165
UTF-8/LF records with vector
`13/7/5/4/3/3/3/6/11/12/16/3/23/5/10/16/15/10`; every record has a unique
stable ID and one distinct positive/negative pair, except the explicitly named
two-package external negatives for `l02_a007`, `l09_a004`, and `l13_a004`.
Those are respectively two same-schema normal rules and one native filegroup,
all in the existing `@@ext+//leaf` package loaded through
`@@//attr:defs.bzl`; they retain null package deprecation defaults without a
sixth source. The nine negative-only controls are outside both the count and
checksum scope.

The checksum scope is exactly the semicolon-delimited records between the
Stage 4 `attr-manifest-records:start` and `:end` markers, joined with LF and
terminated by one LF. SHA-256 is
`99b772e6a8a19540ad379792fe5db7c8683d50d6e8af282ba55766585242300d`.
Generation must reproduce this count, vector, and digest before transcribing
any fixture row. The lane-5 source support token is `//attr:BUILD.bazel`, the
isolated five-file layout's exported source nonrule; it supersedes the stale
`//pkg:source.txt` prose without changing the dictionary semantics.

Correction-only independent rereview returned `ACCEPT`: all five construction
and discrimination blockers are closed, no JVM/bytecode/configured-analysis or
production Bazel delegation entered the design, and residual risk is limited
to faithful generation. Run next only
`WP-4-8-m3-attr-two-package-observable-candidate-oracle-generation`, bound to
the count, vector, and digest above.

That generation preflight returned `REPLAN` before writes or Bazel. The record
digest proves semantic-ID stability, but its shorthand is not an executable
source manifest: complete rule definitions, selector dictionaries, support
declarations, macro bodies/locations, native declarations, and exact five-file
bytes remain underdetermined. Generation may not infer them.

Run next only `WP-4-8-m3-attr-five-source-template-oracle-design`. Freeze exact
five-file bodies and hashes plus literal 18 argv/stdout records, prove a
bijective ID/declaration/command mapping, and validate the templates from two
temporary Bazel 9.2 roots. Retain no fixture/payload/generated source and add no
production Rust, configured analysis, graph/DICE/regex state, JVM/Java
artifact, or production Bazel delegation.

Disposable source-template synthesis then returned `REPLAN` before checkout
writes. The five bodies loaded, but Bazel 9.2 rendered `l12_a003`'s transition
allowlist as
`@@bazel_tools//tools/allowlists/function_transition_allowlist:function_transition_allowlist`;
the frozen shorter anchored regex returned empty. This is a one-row semantic
manifest correction, not a source-template choice. All temporary roots and
output bases were removed.

Run next only `WP-4-8-m3-attr-transition-allowlist-manifest-correction`. Correct
that exact regex/value/support label, recompute and review the 165-row digest,
then resume `WP-4-8-m3-attr-five-source-template-oracle-design`. Add no fixture,
payload, source template, Rust, Cargo, configured analysis, JVM/Java artifact,
or production Bazel delegation during the correction.

The correction changes only `l12_a003`'s regex/rendered/support label to
`@@bazel_tools//tools/allowlists/function_transition_allowlist:function_transition_allowlist`.
The observed anchored query selects the positive target and the superseded
shorter anchored query selects nothing. Count 165 and vector
`13/7/5/4/3/3/3/6/11/12/16/3/23/5/10/16/15/10` are unchanged; the corrected
LF record stream SHA-256 is
`99b772e6a8a19540ad379792fe5db7c8683d50d6e8af282ba55766585242300d`.
Independent latest-diff review returned `ACCEPT` for the one-row correction and
unchanged architecture. Resume only
`WP-4-8-m3-attr-five-source-template-oracle-design`, using the corrected digest
as its immutable semantic preflight.

The complete source-template diff was not accepted. Although all five bodies
loaded and the 18 primary lanes passed twice, independent review found hidden
source-contract mismatches. During the one focused correction Bazel 9.2 proved
`l11_a003_no` cannot use frozen `size="short"`: the package fails with
`size 'short' is not a valid size` and computed `timeout 'illegal' is not a
valid timeout`. Computed timeout `short` requires valid size `small`, so the
semantic row must change before source-template repair.

Run next only `WP-4-8-m3-attr-test-timeout-manifest-correction`. Correct that
one negative size and digest, then retry the five-source design while retaining
the four remaining review obligations: paired lane-1 supports, package-derived
notice licenses, lane-13 `legacy_macro` provenance, and suite/manual tag
closure. The unaccepted docs diff and every temporary artifact were removed;
no fixture, payload, source template, code, JVM artifact, configured analysis,
or production Bazel delegation remains.

The correction changes only `l11_a003_no` to valid `size=small`; Bazel derives
its `timeout=short`, while the unchanged positive's `size=medium` derives
`timeout=moderate`. Count/vector/IDs are unchanged and the corrected LF stream
SHA-256 is
`99b772e6a8a19540ad379792fe5db7c8683d50d6e8af282ba55766585242300d`.
Independent review returned `ACCEPT` for the one-row timeout correction. Run
next only `WP-4-8-m3-attr-five-source-template-oracle-design-retry`, applying
all four retained source-template corrections and hidden focused probes before
the full-diff rereview.

The source-template retry then returned `REPLAN` before docs edits. Bazel 9.2
accepted package `licenses(["notice"])` in the required metadata layout, but
ordinary `attr("licenses","^\\[notice\\]$",//attr:x)` returned empty. Explicit
target licenses pass but do not satisfy the manifest's package-derived claim.
All disposable material was removed.

Run next only `WP-4-8-m3-attr-license-default-source-evidence`. Freeze pinned
source and a minimal package/default/explicit matrix, then select the smallest
finite source construction or manifest correction. Add no template, fixture,
payload, code, JVM artifact, configured analysis, or production delegation.

Pinned-source and Bazel 9.2 matrix evidence returns `ACCEPT` for the exact
construction: BUILD-only `licenses(["notice"])` supplies `[notice]` to native
filegroups even beside `default_package_metadata`; it does not add a Starlark
rule schema attr and does not replace config_setting's `[none]`. The six finite
package-derived filegroup operands are `l02_a005_yes`, `l02_a006_no`,
`l09_a005_yes`, `l13_a017_yes`, `l14_a003_yes`, and `l15_a002_no`; remove only
their explicit notice arguments in the next source-template retry. Manifest
rows, count/vector, and SHA `99b772e6a8a19540ad379792fe5db7c8683d50d6e8af282ba55766585242300d`
stay unchanged. Retain the four other reviewed source obligations and the
Rust-native/no-JVM, loading-only boundary. Run next only
`WP-4-8-m3-attr-five-source-template-oracle-design-retry-2`.

Independent review returned `ACCEPT`: BUILD `licenses()` remains active despite
the Starlark `attr.license` disable flag, native package-license injection is
separate from metadata, and the six-filegroup construction preserves the
165-record SHA. The retry-2 packet owns full two-root hidden-probe review.

Retry-2 is `REPLAN`, not accepted. Its five bodies, semantic constructions,
and corrected 165-record SHA/vector remain viable, but its first review used
the allowed correction for 450-line accounting, nine-control separation,
distinct sibling `workspace`/`out` scratch layout, and pending wording. The
correction rereview found two literal-argv defects: double-backslashed
tag/feature OI controls and a generator probe whose `BUILD\\.bazel` spelling
did not match the primary's `BUILD\.bazel`. The full candidate text was
discarded; no fixture/code/JVM/configured/toolchain work remains. Run next only
`WP-4-8-m3-attr-five-source-template-oracle-design-retry-3` with executable
exact argv before documentation, two independent scratch parents, all 18 lanes
twice, all nine controls, focused probes, and terminal review.

## `attr` five-source template retry-3 terminal REPLAN (2026-08-09)

Retry-3 made no Stage 4 or Stage 8 edits, created no temporary root, and ran no
Bazel command. The exact deleted five-body representation and its unaccepted
hash anchors are unrecoverable from `HEAD`, reachable log history, and the
unreachable-object audit. Manual recovery would introduce the forbidden second
representation, so this is terminal `REPLAN`, not an evidence failure.

Run next only
`WP-4-8-m3-attr-five-source-executable-reconstruction-design`, design-only.
It reconstructs fresh five LF bodies from the accepted 165-row semantic
manifest and reviewed construction obligations; fresh hashes are expected and
the old unaccepted hashes are not requirements. One disposable
machine-readable/executable representation must own bodies, 18 primary argv,
nine literal-empty controls, focused probes, execution in two independent
`mktemp -d` sibling `workspace`/`out` roots, and mechanical candidate/pending
Stage 4/Stage 8 rendering. Its exact OI and generator regex bytes are
`^\[z, a, z\]$` and `^attr/BUILD\.bazel:[0-9]+:[0-9]+$`. Stop on a manifest
semantic change or need beyond five files/two packages. No fixture, code,
configured analysis, toolchain, JVM/Java, or production-Bazel work is allowed.

## `attr` five-source executable reconstruction terminal REPLAN (2026-08-09)

The sole disposable reconstruction reached two operational Bazel 9.2 roots but
failed strict primary ownership: `l05_a003_yes`, `l16_a007_yes`,
`l16_a013_yes`, and `l17_a012_yes` were absent, while `l13_a011_no` and
`l13_a017_no` were selected. It rendered no candidate. Its script, JSON,
scratch/output roots, and processes were removed; the checkout is clean at
`2f83f90b`.

The explicit native `licenses=[none]` negative for `l13_a017` and the three
explicit-empty filegroup `package_metadata=[]` positives are source-synthesis
omissions. Whether `l05_a003`'s `label_list_dict` rendering and
`l13_a011`'s alias `:action_listener` fallback instead require a manifest
correction remains unresolved.

Run next only `WP-4-8-m3-attr-six-ownership-mismatch-evidence`, design-only:
at most five focused Bazel 9.2 constructions and pinned source as needed decide
one exact correction and affected rows. No full reconstruction, fixture, code,
configured analysis, toolchain, JVM/Java, or production-Bazel work is allowed.

## `attr` six-ownership-mismatch focused evidence accepted (2026-08-09)

Four minimal constructions in one disposable Bazel 9.2 workspace resolve the
six disputed primary rows. Each exact query selected only its positive:

```text
attr("label_list_dict", "^\{a=\[//attr:leaf\], z=\[//attr:BUILD\.bazel, //attr:leaf\]\}$", (//attr:l05_a003_yes + //attr:l05_a003_no)) → //attr:l05_a003_yes
attr(":action_listener", "^\[\]$", (//attr:l13_a011_yes + //attr:l13_a011_no)) → //attr:l13_a011_yes
attr("licenses", "^\[notice\]$", (//attr:l13_a017_yes + //attr:l13_a017_no)) → //attr:l13_a017_yes
attr("package_metadata", "^\[\]$", (//attr:l16_a007_yes + //attr:l16_a007_no)) → //attr:l16_a007_yes
attr("package_metadata", "^\[\]$", (//attr:l16_a013_yes + //attr:l16_a013_no)) → //attr:l16_a013_yes
attr("package_metadata", "^\[\]$", (//attr:l17_a012_yes + //attr:l17_a012_no)) → //attr:l17_a012_yes
```

The body construction is exact: a normal `attr.string_list_dict()` for lane 5;
native filegroup versus alias `actual = ":leaf"` for lane 13 action-listener;
inherited notice versus native `licenses = ["none"]`; and explicit empty
filegroup metadata versus absent constraint-setting, constraint-value, and
platform schemas. The single correction decision is **no manifest correction**:
all six rows, the 165-row vector, and SHA-256
`99b772e6a8a19540ad379792fe5db7c8683d50d6e8af282ba55766585242300d` remain
unchanged. The prior mismatch was source synthesis/argv construction, not
ordinary-query semantics. Temporary material is removed. Independent terminal
review returned `ACCEPT`; run next only
`WP-4-8-m3-attr-five-source-executable-reconstruction-retry`, with the four
accepted construction corrections, one executable representation, two sibling
`mktemp` roots, all 18 lanes twice, nine controls, focused probes, mechanical
pending rendering, and the retained Rust-native/no-JVM/code/configured/toolchain
boundary.

## `attr` five-source executable reconstruction retry terminal REPLAN (2026-08-09)

The allowed correction made all prior source constructions replay, but
correction rereview found a second material source-contract miss: the generated
file must be produced by `output_rule(name = "l01_generated_owner",
nullable_output = "l01_generated_nonrule")`; the unaccepted candidate used
`l01_generated_nonrule_owner`, which its empty control did not distinguish.
The candidate and all temporary material were removed. No fixture, payload,
Rust, JVM/Java, configured-analysis, or toolchain work remains.

Run next only `WP-4-8-m3-attr-five-source-executable-reconstruction-retry-2`.
Preserve the immutable 165-row semantics, five files/two packages, all accepted
source/load and six-row fixes, original nine controls/probes, two-root replay,
and mechanical pending rendering; assert the exact producer declaration and
producer identity before replay. A further material issue is terminal.

## `attr` five-source executable reconstruction retry-2 terminal REPLAN (2026-08-09)

Static ownership and producer assertions passed, but the first Bazel 9.2 package
load rejected the emitter's invented `attr.label(..., allow_none = True)`
keyword. No primary query or candidate rendering occurred. The emitter and all
temporary process/root/output material were removed at clean `1b1f5936`; no
fixture, payload, Rust, JVM/Java, configured-analysis, or toolchain work
remains.

Run next only `WP-4-8-m3-attr-five-source-executable-reconstruction-retry-3`.
Retain all prior source/load, generated-owner, control/probe, two-root, and
mechanical-rendering obligations. Assert no `allow_none` source bytes and use
`attr.label(default = None, allow_single_file = True)` or an accepted
keyword-free equivalent; pass a disposable package-load preflight before full
replay. No correction budget is available.

## FileWrite aquery command/root design (2026-08-11)

`WP-8-m5-filewrite-aquery-command-root-design` is **ACCEPT**. The existing
`QueryExpression` parser, build-command DICE root/action closure, resolved
FileWrite semantic view, and accepted per-action formatter are sufficient for
one public vertical slice. No new query parser, action graph, DICE key, or
retained representation is needed.

### Admitted request

The first command accepts one expression whose parsed AST is a direct
`TargetLiteral`; parentheses may lower away as they already do in the shared
parser. The literal must parse as one main-repository
`TargetPattern::Single`. Package-all, recursive, external, set, binary,
`let`, variable, function, integer, empty, and multiple positional roots fail
closed.

Default output and exactly `--output=text` are admitted. The existing
`--output_base` daemon selector and normalized bzlmod
`--allow_yanked_versions`, `--[no]ignore_dev_dependency`,
`--lockfile_mode`, and `--registry` inputs remain admitted transport
inputs. Passthrough, compilation/root-setting flags, include flags,
`--noshow_progress`, every other output, and all other flags are deferred and
rejected rather than silently ignored. CLI request validation parses the shared
AST and target once; the daemon independently repeats semantic validation at
the untrusted wire boundary.

### Evaluation and output

Both one-shot and daemon routes call the existing typed build command with the
single validated target and normalized bzlmod inputs. They consume that
accepted terminal's retained `BuildCommandEvaluation`; neither route invokes
loading/query analysis separately or reconstructs actions. The formatter
container calls `resolved_file_write_semantic_views()` and admits exactly one
resolved view in the complete action closure. Zero, multiple, non-Write,
non-`FileWrite`, executable, named-exec-group, external-owner,
ordinary-no-toolchain, unsafe-output, and unresolved platform/constraint
shapes fail closed through the existing producer/formatter boundaries.

For the sole action, stdout is the accepted per-action block followed by
exactly two LF bytes, matching Bazel 9.2's one-block text container. There is no
multi-action ordering claim because multiple resolved actions are rejected.
Exit is zero and stderr is empty in both runtime modes. The block's header,
indentation, field order, labels, punctuation, empty inputs, boolean, and
container framing are exact Bazel-shaped text. Configuration/output-root and
`SlugActionToken` remain the already accepted explicit Slug-native
projections. Empty progress stderr and all error diagnostics are Slug-native,
not Bazel diagnostic-parity claims.

### Wire and errors

Add one public daemon request variant carrying raw `expression` plus the
existing normalized bzlmod primitive bundle. Output is not a wire field because
text is the only admitted format; `output_base` remains a local CLI transport
choice. Reuse `DaemonResponse`. The server validates the raw expression before
calling the retained daemon runtime. Parse failures use existing command-parse
JSON and exit 2. Wire validation uses `aquery_request_error`; evaluation or
formatting uses `aquery_runtime_error`, runtime mode, escaped message, and
daemon invalidation count. Existing build errors keep their exit classification
but are relabeled for the aquery surface.

### Implementation and proof handoff

Run next
`WP-8-m5-filewrite-aquery-command-root-implementation`. The allowlist is:

- `app/slug_query_v2/src/{expr.rs,lib.rs}` for one shared literal validator;
- `app/slug_commands_v2/src/aquery.rs` and its command tests;
- `app/slug_core_v2/src/runtime/{file_write_aquery_text.rs,dice.rs,mod.rs}`;
- `app/slug_cli_v2/src/commands/aquery.rs` and focused CLI tests;
- `app/slug_server_v2/src/{lib.rs,server.rs,tests.rs}`; and
- bundled Stage 8/current/canonical scheduling bookkeeping.

Reuse `action-query-identity-evidence` and the accepted formatter golden
without oracle rerun or fixture growth. Prove parser negatives; default and
explicit text equivalence; exact two-LF output; one-shot/daemon equality;
wire validation; bzlmod forwarding; and retained-daemon content A/B/A token
change/restoration with zero source-bypass or fresh-graph path. Clean stale
`slugd` before and after daemon-sensitive tests. Run focused owner tests,
direct-dependent compile checks, formatting, archive, and diff checks. Require
independent final review because the public wire changes.

Caps are 425 production / 400 tests / 825 total Rust net lines plus bundled
bookkeeping. Add no multi-action container/order, nonliteral expression,
external root, non-text format, file-write contents, compilation/root-setting
flag, execution, new DICE state, action reconstruction, parser/vendor,
retained identity, REAPI reuse, exact Bazel checksum/ActionKey bytes, JVM/Java,
oracle growth, or CI. A second material contract or implementation correction
is `REPLAN`.

Independent design review returned `ACCEPT`: exactness is bounded to the
one-action text shape and framing, both CLI and raw-wire expression boundaries
are validated, and the retained DICE action closure is reused. Residual risk is
implementation fidelity for preserving build terminal exit classifications
while relabeling errors for aquery.
## FileWrite aquery command/root implementation accepted (2026-08-11)

`WP-8-m5-filewrite-aquery-command-root-implementation` is **ACCEPT**. The V2
`aquery` command now admits one shared-parser direct literal main-repository
target with default or explicit `--output=text`. One-shot and daemon routes
reuse the typed build command and retained `BuildCommandEvaluation` action
closure; the public daemon request carries only the raw expression and
normalized request-local bzlmod inputs, and the server independently
revalidates that expression.

The container admits exactly one resolved FileWrite semantic view and emits the
accepted per-action block followed by exactly two LF bytes. Every wider
root/action/flag/output shape remains fail-closed. Bazel-shaped header, field
order, punctuation, indentation, labels, empty inputs, boolean, and one-block
framing are exact for the admitted Bazel 9.2 surface. Configuration/output-root
and `SlugActionToken` fields, empty progress stderr, runtime observation
counts, and diagnostics remain explicitly Slug-native. Existing build terminal
errors retain their exit classifications while the surface relabels them as
aquery diagnostics.

Focused parser, formatter/container, raw-wire, request-local bzlmod, and
one-shot/retained-daemon lifecycle tests pass. The lifecycle proves one-shot
and daemon equality, content A/B token change, B/A full output and token
restoration, and stable daemon PID. The five affected crate graph checks,
rustfmt, and diff checks pass. Rust additions are 401 production, 223 tests,
and 624 total, within the 425/400/825 caps. Archive layout checks pass; the
known repository-baseline absence of the V1 archive tag, branch, and recorded
commit remains unchanged. No oracle rerun or fixture growth occurred.

Independent final review returned `ACCEPT`: both request boundaries are
validated, the public wire and errors are bounded, retained DICE/action
semantics are reused, terminal classification and identity domains are
preserved, and no scope, security, or error-handling correction is required.

Run next only
`WP-8-m5-filewrite-aquery-multi-action-order-evidence-design`, design-only.
Freeze pinned Bazel 9.2 text-container ordering ownership and the smallest
declaration/dependency/diamond discriminator matrix before selecting any
multi-action successor. Add no Rust, fixture, expected oracle, Bazel execution,
DICE state, action reconstruction, query/function/output breadth, execution,
JVM/Java, REAPI, or CI.

## FileWrite aquery root-local multi-action order design (2026-08-11)

`WP-8-m5-filewrite-aquery-multi-action-order-evidence-design` is **ACCEPT**. It
narrows the
multi-action successor to the ordering Bazel 9.2 actually owns. It does not
promote Slug's retained closure traversal order into an exact aquery claim.

### Pinned-source decision

At Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`,
`ActionGraphQueryEnvironment#getTargetsMatchingPattern` transforms a direct
literal target-pattern result into configured values for that same label; it
does not add dependency targets. `ActionGraphTextOutputFormatterCallback`
`processOutput` iterates those configured values and each
`RuleConfiguredTargetValue#getActions()` list without sorting, while
`writeText` appends one final LF after the action fields. The callback therefore
owns declaration-list order and one blank line after every action for a single
configured target. `RuleConfiguredTargetValue` retains the configured target's
immutable action list without copying or sorting it.

The broader `deps()` path is different. Aquery's
`createThreadSafeMutableSet()` uses
`QueryUtil.ThreadSafeMutableKeyExtractorBackedSetImpl`, whose iterator is
`ConcurrentHashMap.values().iterator()`. Bazel source supplies no cross-target
iteration-order contract. The query set deduplicates one configured target in
a diamond by `ActionLookupKey`, but Bazel's documented shared-action behavior
may still print equivalent actions owned by distinct configured targets.
Repeated oracle output cannot turn that concurrent-map iteration into an exact
ordering contract.

Slug's retained `BuildCommandEvaluation::action_closure` is intentionally a
roots-first breadth-first configured-target closure with dependency-order
frontiers and diamond deduplication. That order remains correct Slug build
state, but it is **not** Bazel direct-literal aquery order. Cross-owner
dependency-before/after-root order is unsupported/deferred until a later
`deps()` expression packet selects an explicit compatibility strategy.

### Admitted successor

Keep the public request and wire unchanged: one direct main-repository literal,
default or `--output=text`, plus the accepted transport flags. For its sole
analyzed requested target, admit one or more supported FileWrite actions in the
retained per-owner declaration order. Resolve every root action's platform and
constraints against the existing complete action closure, but do not emit
actions owned by dependency, platform, constraint, toolchain, alias, or
generated-file nodes. Dependency actions remain analyzed build state and are
ignored for direct-literal output exactly as Bazel ignores dependency targets.

The container formats each admitted root action with the accepted formatter
and appends exactly two LF bytes after each block. It performs no label, output
path, token, or key sort. Zero root actions, multiple requested analyses, any
unsupported root action, duplicate/missing semantic platform nodes, or any
existing FileWrite guard failure remains a closed Slug-native runtime error.
The accepted one-action output is byte-for-byte unchanged.

Action block shape, per-target declaration order, direct-literal dependency
exclusion, and two-LF-per-block framing are exact Bazel 9.2 behavior for this
slice. Configuration/output-root and `SlugActionToken` bytes, progress silence,
invalidation counts, and diagnostics remain Slug-native. Multi-owner order,
`deps()` activation, shared actions across distinct owners, aspects, multiple
configurations/roots, other action kinds, contents, output formats, and exact
Bazel checksum/ActionKey bytes remain unsupported/deferred.

### Discriminating evidence and proof

Add one self-contained Bazel 9.2 fixture with a root rule that declares
`z-root.txt` before `a-root.txt`, plus left/right dependency owners sharing one
diamond leaf and each declaring a distinct FileWrite. The fixture must use the
accepted explicit execution-platform/marker-toolchain construction.

Run direct-literal text rows in one retained Bazel server for declaration order
A (`z`, `a`), edited order B (`a`, `z`), and restored A. Anchored stdout
patterns over normalized stdout must prove exactly two root blocks, declaration
rather than lexical path order, and absence of every dependency output. Bazel's
pinned `writeText` source and the generated raw `stdout` evidence record own the
two-LF framing fact because the harness intentionally normalizes trailing
whitespace; focused Slug byte assertions own its regression. Add one
oracle-only `deps(//:root)` row whose assertions prove all four owners are
present and the shared owner occurs exactly once, while deliberately
making no cross-owner block-order claim. This row is boundary evidence only and
does not activate `deps()` in Slug.

Core proof must construct or evaluate a root with two actions and action-bearing
diamond dependencies, then prove the root-only semantic views preserve `z,a`,
exclude dependency actions, and still resolve platform facts through the full
closure. CLI proof must show default/explicit text and one-shot/daemon equality,
two-block framing, dependency exclusion, retained-daemon A/B/A order
change/restoration, stable identity restoration, and stable daemon PID. Preserve
underlying build terminal exit classifications and the existing raw-wire and
bzlmod tests.

### Implementation handoff

On design acceptance, run next only
`WP-8-m5-filewrite-aquery-root-local-order-oracle-implementation`. The allowlist
is:

- one new `tests/v2_oracle/fixtures/filewrite-aquery-root-order/` fixture with
  only `fixture.toml`, `MODULE.bazel`, `BUILD.bazel`, `defs.bzl`, and generated
  `expected/oracle.json`;
- `app/slug_core_v2/src/runtime/{dice.rs,file_write_aquery_text.rs}`;
- focused `app/slug_cli_v2/tests/cli.rs` coverage; and
- bundled Stage 8/current/canonical scheduling bookkeeping.

Caps are 70 production / 220 tests / 290 total Rust net lines, five fixture
files / 350 fixture text lines, plus bookkeeping. Rebuild no Slug binary for
the Bazel-only oracle. Validate the new fixture with pinned Bazel 9.2, protected
one-action evidence, focused core/CLI tests, direct compile dependents, retained
daemon lifecycle, rustfmt, archive, and diff checks. Require independent final
review because root action ownership and lifecycle behavior change.

Add no command/wire fields, parser or query-function breadth, cross-owner
ordering, action reconstruction, new DICE key/state, execution, file contents,
other action kinds/formats, retained identity representation, exact Bazel
identity bytes, JVM/Java artifact, REAPI reuse, or CI. A second material
contract or implementation correction is `REPLAN`.

Independent design review returned `ACCEPT`: direct-literal root ownership and
declaration-list order are source-backed, concurrent-map cross-owner order is
correctly deferred, and the A/B/A plus diamond boundary evidence, caps,
negative surface, and retained-daemon proof are sufficient.
## FileWrite aquery root-local order implementation accepted (2026-08-11)

`WP-8-m5-filewrite-aquery-root-local-order-oracle-implementation` is
**ACCEPT**. A direct main-repository literal now emits every supported
FileWrite action owned by its sole requested analyzed target in retained
declaration order. The selector reads that requested analysis separately from
the build action closure; dependency and semantic-support actions remain
unemitted, while each root action still resolves its selected platform and
constraint facts through the complete closure. Zero or multiple requested
analyses, zero root actions, unsupported root actions, and every existing
semantic-integrity failure remain closed runtime errors.

The text container appends the accepted formatter block and two LF bytes for
each root action without sorting. The prior one-action output is byte-for-byte
unchanged. Per-root declaration order, literal dependency exclusion, block
shape, and framing are exact Bazel 9.2 behavior for this slice.
Configuration/output-root and `SlugActionToken` bytes, progress silence,
invalidation counts, and diagnostics remain Slug-native. Cross-owner order,
`deps()` activation, aspects, multiple roots/configurations, other action
kinds/formats, contents, and exact Bazel checksum/ActionKey bytes remain
unsupported.

The new five-file `filewrite-aquery-root-order` fixture passes on pinned Bazel
9.2 and records retained-server A/B/A declaration order, exactly two literal
root blocks, dependency exclusion, and order-agnostic diamond owner
membership. The protected `action-query-identity-evidence` fixture also
passes. Focused core and CLI tests prove root-only/full-closure selection,
default/explicit and one-shot/daemon equality, per-block framing, dependency
exclusion, A/B/A token/order restoration, and stable daemon PID. Direct
`slug_commands_v2`, `slug_server_v2`, and `slug_cli_v2` compile checks,
rustfmt, and diff checks pass with no stale `slugd`.

Rust growth is 12 production, 73 tests, and 85 total net lines, within the
70/220/290 caps. The fixture is exactly five files and 255 text lines, within
the 350-line cap. Archive layout checks pass; the known checkout-baseline
absence of the V1 archive tag, branch, and recorded commit remains unchanged.


Independent final review returned `ACCEPT`: sole-root ownership and retained
declaration order are preserved while semantic support resolves through the
complete closure; the A/B/A and diamond evidence discriminates the claimed
exact surface, and cross-owner `deps()` semantics remain correctly deferred.
Run next only `WP-8-m5-filewrite-aquery-deps-owner-set-design`, design-only.
Determine whether the existing query AST, configured action closure, and the
accepted order-agnostic Bazel 9.2 diamond evidence can support an exact
`deps()` FileWrite owner set with an explicitly Slug-native deterministic
cross-owner order. Freeze query membership, semantic-support filtering,
deduplication, order classification, errors, allowlist, caps, and lifecycle
proof before selecting one successor. Add no Rust, fixture/expected evidence,
Bazel execution, command/wire fields, action reconstruction, DICE state,
execution, other action kinds/formats, identity-byte work, JVM/Java, REAPI, or
CI.

## FileWrite aquery deps owner-set design accepted (2026-08-11)

`WP-8-m5-filewrite-aquery-deps-owner-set-design` is **ACCEPT**. Activate only
an unbounded top-level `deps(<one direct main-repository literal>)`. Depth,
wrappers, nesting, external repositories, package/all-target patterns, and
multiple roots remain unsupported. The existing direct-literal behavior must
remain byte-for-byte unchanged. The CLI and daemon independently parse the raw
expression into a shared typed scope (`literal` or `deps`) and the public daemon
wire remains raw expression plus normalized bzlmod inputs; no scope field is
added to the wire.

Both scopes build the same sole requested root through the accepted typed build
DICE path. Literal scope selects only the requested analysis. `deps()` scope
selects every action-bearing configured owner in the retained build action
closure, deduplicated by configured-target identity. This is the exact Bazel
9.2 FileWrite owner set for the admitted aspect-free graph: configured-query
`deps()` includes its seed, ordinary and transitioned configured dependencies,
aliases/generated producers, implicit dependencies, and resolved toolchain
configured targets under the default settings. Source, visibility, platform,
constraint, and other semantic-support nodes are actionless and therefore emit
nothing; an action-bearing selected toolchain implementation is query-visible
and must emit. No new retained state or semantic-support filter is required.

Within each owner, retained action declaration order, existing block shape, and
two-LF framing remain exact. Cross-owner order is explicitly Slug-native:
roots-first breadth-first retained closure order, with no Bazel parity claim.
Slug configuration/output-root and `SlugActionToken` bytes, progress silence,
invalidation counts, and diagnostics remain Slug-native. Aspect-bearing graphs,
shared equivalent actions owned by distinct configured owners, depth-bounded or
composed expressions, multiple requested roots/configurations, other action
kinds/formats, contents, and exact Bazel checksum/ActionKey bytes remain
unsupported/deferred. Any action-bearing owner with a non-FileWrite action
fails the entire request closed; actionless owners are ignored.

Run next only
`WP-8-m5-filewrite-aquery-deps-owner-set-oracle-implementation`. Reuse or
introduce one shared aquery expression-scope parser in `slug_query_v2`; both
command parsing and daemon boundary validation must call it. Preserve the raw
public wire. Add a closure-wide resolved FileWrite selector beside the accepted
root-only selector, sharing platform/constraint resolution and the formatter.
Do not reconstruct actions or add a DICE key.

Extend only the existing five-file `filewrite-aquery-root-order` fixture. Keep
the two-action literal rows as the dependency-exclusion regression and expand
the order-agnostic `deps()` row to discriminate an action-bearing selected
toolchain implementation, alias/generated producer ownership, a configured
transition owner, and the existing ordinary diamond with the shared owner
exactly once. The fixture owns raw stdout including two-LF framing; assert
membership and per-owner declaration order without asserting cross-owner
order. Add focused parser/command/server negatives, core closure selection and
mixed-action failure tests, and CLI default/explicit plus one-shot/daemon
equality. Retained-daemon A/B/A must remove and restore one dependency edge,
prove exact owner membership/token restoration and stable PID, and retain the
direct-literal A/B/A order proof.

The implementation allowlist is:

- `app/slug_query_v2/src/{expr.rs,lib.rs}` and existing query tests;
- `app/slug_commands_v2/src/aquery.rs` and existing command tests;
- `app/slug_core_v2/src/runtime/{dice.rs,file_write_aquery_text.rs,mod.rs}`;
- `app/slug_cli_v2/src/commands/aquery.rs` and existing CLI tests;
- `app/slug_server_v2/src/{lib.rs,server.rs,tests.rs}`;
- the existing five files under
  `tests/v2_oracle/fixtures/filewrite-aquery-root-order/`; and
- canonical/current-packet/Stage 8 bookkeeping.

Cap Rust growth at 250 production, 320 tests, and 570 total net lines. Keep the
fixture at five files and cap fixture text at 420 lines; cap bookkeeping at 170
lines. Validate the expanded fixture with pinned Bazel 9.2 and the protected
direct-literal/identity evidence, focused query/commands/core/server/CLI tests,
direct compile dependents, rebuilt `slug_cli_v2`, retained-daemon lifecycle,
rustfmt, archive, and diff checks. Require independent final review because a
public expression shape and closure-wide action ownership are activated.

Add no depth/wrapper/general query activation, command/wire field, aspect
state, new DICE key/state, action reconstruction/execution/contents, other
action kind/format, retained identity representation, exact Bazel identity
bytes, JVM/Java artifact, REAPI reuse, or CI. One material correction maximum;
a second is `REPLAN`.

Independent design review returned `ACCEPT`: the bounded expression shape,
configured-owner membership, actionless-support treatment, explicit
Slug-native cross-owner order, fail-closed mixed/aspect boundaries, fixture
discriminators, caps, and daemon A/B/A proof are source-backed and sufficient.

## FileWrite aquery deps owner-set implementation replanned (2026-08-11)

`WP-8-m5-filewrite-aquery-deps-owner-set-oracle-implementation` is
**REPLAN**. The shared expression-scope, closure selector, command/daemon
plumbing, and ordinary diamond lifecycle implementation compiled and passed
focused tests, and the expanded pinned Bazel 9.2 fixture proved root,
transition, alias/generated, diamond, and action-bearing selected-toolchain
membership. Those uncommitted changes were discarded after the required
boundary failed.

The first Slug fixture run showed that strict selected-toolchain validation
rejected an implementation with actions/non-empty built-in `DefaultInfo`.
The packet's one permitted material correction relaxed only that postguard
while retaining exact topology, built-in `DefaultInfo` plus `ToolchainInfo`,
and diagnostic constraints; its focused analysis test passed. The next Slug
run reached a second independent gap: the retained action on that
zero-toolchain owner has no selected execution platform, so the exact
FileWrite semantic view fails with `configured FileWrite action requires a
selected toolchain platform`.

Bazel 9.2 emits that action with the selected execution platform. Assigning
Slug's platform now would require a second material contract change—default
execution-platform selection for zero-requirement owners or recursive
toolchain selection by toolchain implementations. The packet forbids a second
correction, action reconstruction, and silent scope narrowing, so no partial
Rust, test, fixture, or expected-evidence changes are retained.

Run next only
`WP-8-m5-filewrite-aquery-zero-toolchain-platform-design`, design-only.
Determine whether retained candidate-platform topology can give every
zero-toolchain configured action, including an action-bearing selected
toolchain implementation, a structural execution platform without toolchain
recursion, reconstruction, or new DICE state. Freeze exact/Slug-native
selection, configuration/identity participation, constraints and failures,
A/B/A evidence, allowlist, and caps before choosing one successor.

## Zero-toolchain action execution-platform design accepted (2026-08-11)

`WP-8-m5-filewrite-aquery-zero-toolchain-platform-design` is **ACCEPT** for
one bounded prerequisite. Bazel 9.2's
`ToolchainResolutionFunction#findExecutionPlatformForToolchains` selects an
execution platform even when the requested toolchain-type set is empty.
Slug does not retain candidate topology for an ordinary zero-toolchain rule,
so that broader surface remains unsupported. A selected root toolchain
implementation is different: `root_rule_execution_platforms` already retains
the parent's ordered structural candidate configured keys on that
zero-requirement owner, and the strict selected-topology postguard proves the
candidate sequence exactly with selection `None`.

Admit only an action-bearing selected toolchain implementation whose retained
topology has exactly one candidate. Its configured FileWrite action view derives
that sole configured platform key. This is exact for the one-candidate Bazel
9.2 slice and adds no selection ambiguity. It does not fabricate a
`ToolchainSelection`: toolchain type/declaration/implementation identity stays
absent on the implementation owner, while action execution-platform identity is
derived separately. Candidate topology already participates in configured-node
equality, closure edges, configuration identity, invalidation, and platform/
constraint semantic resolution, so no field, key, or reconstruction is added.

Zero candidates, multiple candidates, missing topology, ordinary
zero-toolchain action owners, external registrations, and mismatched
platform/configuration facts remain fail-closed or unsupported. Multiple
candidates are deferred because Bazel's suitability/filter/stable tie behavior
is wider than Slug's admitted platform model. Target and transitioned
configurations remain structurally distinct; shared equivalent actions owned by
distinct configured owners stay deferred.

Run next only
`WP-8-m5-filewrite-aquery-deps-owner-set-platform-oracle-implementation`.
Implement the previously accepted shared expression scope, raw-wire
revalidation, closure-wide selector, and formatter plumbing. In
`ConfiguredNodeResult::configured_file_write_actions`, prefer the existing
selected platform; otherwise accept only the sole candidate of a retained
selection-free topology. Relax selected-toolchain implementation validation to
permit retained actions, declared outputs, and non-empty built-in
`DefaultInfo`, while preserving exact topology, exactly built-in
`DefaultInfo` plus `ToolchainInfo`, and no diagnostics.

Extend only the existing five-file root-order fixture. Use one registered
platform, one action-bearing selected implementation, an ordinary diamond,
alias/generated producer, and transition owner. Give the transitioned action
an actionless second toolchain so it has a platform without creating the
deferred equivalent-action/distinct-configuration case. Keep direct literal
output at exactly two root blocks and assert deps owner membership/framing
without cross-owner order.

The implementation allowlist is:

- `app/slug_analysis_v2/src/{dice.rs,result.rs}` and existing analysis tests;
- `app/slug_query_v2/src/{expr.rs,lib.rs}` and existing query tests;
- `app/slug_commands_v2/src/aquery.rs` and existing command tests;
- `app/slug_core_v2/src/runtime/{dice.rs,file_write_aquery_text.rs,mod.rs}`;
- `app/slug_cli_v2/src/commands/aquery.rs` and existing CLI tests;
- `app/slug_server_v2/src/{lib.rs,server.rs,tests.rs}`;
- the existing five files under
  `tests/v2_oracle/fixtures/filewrite-aquery-root-order/`; and
- canonical/current-packet/Stage 8 bookkeeping.

Cap Rust growth at 280 production, 380 tests, and 660 total net lines. Keep the
fixture at five files and 420 text lines; cap bookkeeping at 180 lines. Require
expanded pinned Bazel 9.2 plus protected literal evidence, focused analysis/
query/commands/core/server/CLI tests, direct dependents, rebuilt CLI,
stable-daemon A/B/A, rustfmt/archive/diff checks, and independent final review.

Add no ordinary zero-toolchain action support, zero/multiple-candidate choice,
depth/wrapper/general query breadth, command/wire field, recursive toolchain
selection, new DICE state, action reconstruction/execution/contents, other
aquery action kind/format, identity representation, exact Bazel identity bytes,
JVM/Java artifact, REAPI reuse, or CI. One material correction maximum; a
second is `REPLAN`.

Independent design review returned `ACCEPT`: the sole-candidate derivation
uses already retained configuration-bearing topology, keeps toolchain selection
and action platform identity separate, fails closed on ambiguous/broader
zero-toolchain shapes, and gives a bounded successor with discriminating
oracle and lifecycle proof.
## FileWrite aquery deps owner-set implementation accepted (2026-08-11)

`WP-8-m5-filewrite-aquery-deps-owner-set-platform-oracle-implementation`
is **ACCEPT**. A shared parsed scope admits only a direct main-repository
literal or unary unbounded top-level `deps(literal)`; the CLI and daemon each
reparse the raw expression, and the public daemon wire remains unchanged.
Literal output still selects only the sole requested analysis and preserves its
retained declaration order plus two-LF framing. `deps()` selects action-bearing
owners from the retained roots-first breadth-first action closure. Configured
owner membership and per-owner action order/framing are exact for the admitted
aspect-free Bazel 9.2 slice; cross-owner BFS order is explicitly Slug-native.

Configured FileWrite actions prefer an existing toolchain selection. A
selection-free owner derives an action execution platform only from exactly one
retained candidate configured key. No `ToolchainSelection`, DICE key, field,
or reconstructed action is created. The selected-implementation guard now
allows retained actions, declared outputs, and nonempty built-in `DefaultInfo`
while still requiring exact candidate topology, exactly built-in `DefaultInfo`
plus `ToolchainInfo`, and no diagnostics. Zero/multiple/missing candidates,
ordinary zero-toolchain action owners, mixed actions, aspects, external
registrations, shared equivalent configured owners, wrappers/depth, other
formats/action kinds, and Bazel identity bytes remain fail-closed or deferred.

The five-file pinned Bazel 9.2 fixture is 343 lines and discriminates two
root-local nonlexical actions, an ordinary diamond, action-bearing selected
toolchain implementation, alias target, generated-file producer, and
transitioned owner without asserting cross-owner order. Refresh and protected
replay both passed. Focused analysis/query/commands/core/server tests, full
analysis/commands/query/server suites, rebuilt `slug_cli_v2`, and the
one-shot/daemon stable-PID dependency-edge A/B/A lifecycle passed. The
lifecycle includes the action-bearing selected implementation and exact output/
SlugActionToken restoration.

Rust growth is 258 net lines total (129 in production-path files and 129 in
test-path files, with the mixed core file conservatively counted as
production), inside all packet caps. `cargo check`, rustfmt, scope/credential,
fixture, and diff checks passed. The archive checker passed every active V2
layout boundary and reported only the established missing V1 tag/branch/
recorded-commit baseline. Broad integration runs also reproduced two unrelated
baselines: the CLI loadfiles fixture's unavailable root DICE node and the core
external-visibility diagnostic wording assertion. Packet-specific paths do not
reach either failure.

Independent final review returned `ACCEPT`: exact/Slug-native/deferred claims,
identity separation, retained topology, closure platform/constraint resolution,
fail-closed boundaries, fixture discrimination, lifecycle proof, and caps are
truthful. This closes M5 for the bounded FileWrite surface; broader aquery
breadth remains later work rather than an execution-gate parity claim.

Run next only `WP-7-m6-filewrite-reapi-action-handoff-design`, design-only.
Freeze one semantic FileWrite action object shared by aquery and execution and
the exact REAPI `Command`/input-root/`Action` identity boundary. Do not add
production state, execution, backend calls, cache/materialization, or protocol
breadth until that retained ownership and digest-domain design is independently
accepted.
## Executable FileWrite `run` handoff design (2026-08-11)

`WP-8-m7-filewrite-run-handoff-design` freezes the first `run` slice over the
accepted configured analysis and FileWrite REAPI executor. Bazel 9.2
`RunCommand` owns the build on the server, validates one successful executable
target, then returns an `ExecRequest` for the client to launch with its terminal
and environment. Slug preserves that boundary with a smaller, explicitly
Slug-native daemon wire; neither daemon nor build executor launches the program.

### Admitted semantic view

Add a request-local `ResolvedRunSemanticView<'a>`, constructed only by
`BuildCommandEvaluation`. It borrows the sole requested configured analysis,
its single built-in `DefaultInfo`, the sole existing
`ResolvedFileWriteSemanticView<'a>` selected through the closure-wide
platform/constraint resolver, and their shared normalized executable artifact.

Admit only a configured rule whose retained `RuleCapability` is executable and
not a test, with no analysis diagnostic. `DefaultInfo.executable` and
`files_to_run.executable` must equal the same nonempty normalized artifact.
Both manifest fields must be absent. `files`, `default_runfiles.files`, and
`data_runfiles.files` must each flatten to exactly that artifact; both symlink
maps and both empty-file depsets must be empty. No separate
`FilesToRunProvider`, `RunEnvironmentInfo`, or user provider may alter it.

The complete action closure must contain exactly one action-bearing owner and
one action: the existing resolved FileWrite view. Its sole output must equal the
executable artifact and `is_executable` must be true. All accepted FileWrite
platform, constraint, namespace, property, digest, and mixed-action guards
remain. The view exposes borrowed accessors only: no DICE key, retained field,
reconstruction, filesystem search, convenience link, or second action model.

### Build and launch ownership

One-shot and daemon paths call one shared run evaluation routine. It constructs
the run view, derives the accepted `FileWriteReapiPlan`, and calls only
`execute_file_write`; the raw FileWrite executor remains rejecting. Successful
REAPI execution must materialize the declared output at its owner-derived
configured output path before launch.

Validate that the normalized artifact joined under the configured output root
cannot escape, that no component or final entry is a symlink, and that the
final entry is a regular file with a Unix execute bit. Derive the absolute path
from that checked join; do not scan `bazel-bin`, synthesize a mirror, or trust
an arbitrary executor path. The CLI repeats the final regular-file/execute-bit
check immediately before spawn. Content digest and executable mode remain owned
by the FileWrite plan/materialization boundary.

Program launch is command behavior, never a build action. Use a direct Rust
process with `RunRequest.program_args` in order and inherited stdin/stdout/
stderr; do not buffer or serialize program streams. Preserve a normal numeric
program exit status. Map a POSIX signal Slug-natively to `128 + signal`.
Launch/inspection failure after build is a stable Slug-native command error and
exit 1. Analysis, REAPI, cache, and materialization failures preserve their
accepted build terminal classifications.

Launch from the workspace root with the client's inherited environment after
clearing exactly `JAVA_RUNFILES`, `RUNFILES_DIR`, `RUNFILES_MANIFEST_FILE`,
`RUNFILES_MANIFEST_ONLY`, and `TEST_SRCDIR`. Do not set Bazel `BUILD_*`
variables. This cwd/environment policy is Slug-native and valid only because
additional runfiles are rejected.

### Bounded daemon wire

Extend the tagged request with `DaemonRequest::Run(BuildRequest)`. It carries
the normalized target/configuration/bzlmod/executor inputs used by build and no
program arguments or client environment. The daemon independently validates
one run target and all semantic guards, builds/materializes, and creates no
user-program child.

Add a defaulted, absent-when-none `run_launch_plan` field to
`DaemonResponse` and this public payload:

```text
RunLaunchPlan {
    executable_path: String,
    working_directory: String,
    environment_to_clear: Vec<String>,
}
```

`Some(plan)` is the only launch authorization and is legal exactly when a Run
request finishes build, materialization, and server path/mode validation with
exit zero. All other responses, including Build/Query/Aquery/Cquery, have no
plan. The client rejects a missing successful-Run plan, any plan on nonzero Run,
a relative executable/cwd, an unexpected clear variable, or a plan for another
kind. It appends locally parsed arguments; they never cross the wire. The plan
contains no complete environment, value, secret, program argument, content,
digest, or arbitrary executor metadata.

One-shot constructs the same plan locally from the same view/checks and feeds
the same client launcher. No plan is retained. This bounded public-wire addition
requires serialization, cross-command absence, malformed-plan, and client tests
plus independent final review.

### Compatibility and failure boundary

Exact for the admitted Bazel 9.2 slice are one-target executable relationship
checks, already accepted FileWrite/REAPI semantics, arguments after `--`,
noninteractive program stdout/stderr bytes, and normal numeric exits.
Slug-native are output/configuration path bytes, launch wire, workspace cwd,
inherited-minus-clear environment, process mechanism, diagnostics/evidence,
and signal mapping.

Deferred are additional runfiles, manifests, symlinks/empty files,
`RunEnvironmentInfo`, exact `BUILD_*`, target-supplied binary arguments,
`run_in_cwd`, `run_under`, `script_path`, tests/coverage, interactive
terminal equivalence, multiple targets/actions, other executable producers or
action kinds, Windows, and exact Bazel identity bytes. Missing/conflicting
providers/artifacts, non-executable output, extra action owner, mixed action,
ambiguous platform, absent remote executor, materialization mismatch, symlink,
path escape, or missing authorization fails closed before spawn.

### Implementation packet and proof

The successor may edit only `app/slug_commands_v2/src/run.rs` and existing
command tests; `app/slug_cli_v2/src/commands/run.rs` and existing CLI tests;
`app/slug_core_v2/src/runtime/{dice.rs,mod.rs}` and existing focused core
tests; `app/slug_reapi_v2/src/{executor.rs,lib.rs}` and existing focused REAPI
tests; `app/slug_server_v2/src/{lib.rs,reapi.rs,server.rs,tests.rs}`; all six
existing files under `tests/v2_oracle/fixtures/run-basic/`; and canonical/
current/Stage 8 bookkeeping. Add no DICE state, local build executor, runfiles
tree/materializer, fixture, dependency, JVM artifact, other action kind, or
broad run flag. Cap Rust growth at 270 production, 340 tests, 610 total;
fixture growth at 150 and bookkeeping at 120 lines. One material correction is
allowed; a second is `REPLAN`.

Extend `run-basic` because its pinned 9.2 baseline proves success but not args,
stderr/nonzero exit, selected properties, or invalidation. Keep one executable
FileWrite topology and add one registered execution platform. Required proof:

- core positives and fail-closed provider/runfiles/action/path negatives;
- exact FileWrite REAPI bytes/digests/mode/properties, one remote and zero
  direct-local build actions, and no launch cache/action event;
- focused one-shot argument order, inherited stdout/stderr, numeric nonzero
  exit, launch failure, and fixed environment clearing;
- wire round trips proving args/env values absent, cross-command plans absent,
  and malformed authorization rejected; and
- pinned Bazel 9.2 refresh/replay plus stable-PID Slug daemon A/B/A script
  content mutation, discriminating owner path, output, action/cache evidence,
  and exact restoration.

The checked-in `run-basic` expectation was refreshed and replayed with pinned
Bazel 9.2; it replaces stale Bazel 9.1 Windows provenance. Independent design
review returned `ACCEPT` after one bounded bookkeeping correction. The final
explicit allowlist covers all six existing fixture files and each permitted
Rust module/test surface. The reviewer found the borrowed semantic-view
ownership, resolved FileWrite executor reuse, daemon/client launch boundary,
no-secret wire, path/mode guards, compatibility classification, exit
classification, and discriminating evidence plan sound. Implementation is now
replanned before acceptance.

### Run oracle endpoint injection replan (2026-08-11)

The implementation reached `run-basic` replay after focused Rust tests and a
passing pinned Bazel 9.2 refresh. Slug stopped before evaluation because
`_slug_reapi_argv` starts from an explicit build-only verb guard, so the
already-started NativeLink endpoint and default execution properties never
reach a Run command. This is oracle scaffolding, not production behavior.

The implementation allowlist excluded `tools/v2_oracle_lib/runner.py` and its
single correction had already admitted the Starlark executable-write boolean.
The second independent scope miss therefore ends that packet `REPLAN`; its
unaccepted production diff is retained in the worktree while the design
boundary is corrected. Run next only
`WP-8-m7-filewrite-run-oracle-endpoint-design`: freeze an exact Build/Run-only
injector boundary and focused negative regression before implementation resumes.

Design review required one bounded correction: Run flags belong immediately
before the first standalone `--`, not after program arguments. The corrected
successor also applies the existing successful-remote evidence requirement to
Run while leaving Build append order and Query/Aquery/Cquery untouched; it
permits only runner/comparison helpers and existing focused tests.

Correction-only rereview returned `ACCEPT`. The oracle amendment is bounded to
Build/Run endpoint placement and the existing evidence schema; no production,
wire, lifecycle, fixture-schema, or other-verb behavior changes. Run next only
`WP-8-m7-filewrite-run-handoff-implementation-retry`, retaining the existing
production diff and adding the reviewed runner/comparison regressions.

### Run fixture admission replan (2026-08-11)

The endpoint-injection retry passed seven focused harness cases and correctly
placed Run flags before `--`. Its sole correction completed the explicit
toolchain marker leaf. The subsequent Slug replay reached rule analysis and
then rejected the old fixture's independent
`ctx.configuration.host_path_separator` read, which is outside the admitted
POSIX Starlark context.

That second scope miss ends the retry `REPLAN` without accepting or committing
the retained production/harness diff. Run next only
`WP-8-m7-filewrite-run-fixture-admission-design`: audit all four workspace
sources and freeze one POSIX-only source reduction before another retry.

Count-correction rereview returned `ACCEPT`. The full four-file workspace audit
found no other unsupported construct beyond the cross-platform host-context
branch. Run next only
`WP-8-m7-filewrite-run-handoff-implementation-retry-2`, retaining the reviewed
production/harness diff and applying only the accepted POSIX fixture reduction.

### Executable FileWrite Run implementation evidence (2026-08-11)

`WP-8-m7-filewrite-run-handoff-implementation-retry-2` activates only the
reviewed POSIX one-target shape. `BuildCommandEvaluation` owns one borrowed
`ResolvedRunSemanticView`; execution still derives only
`FileWriteReapiPlan::from_resolved` and `execute_file_write`. The daemon
builds, validates, and returns a bounded plan; only the client launches with
locally retained arguments and inherited streams/environment minus the fixed
five names. No DICE key/state, second action model, raw/direct-local executor,
runfiles materializer, daemon child launch, or environment value entered the
wire.

The retry used its one correction to rename the fixture's content attribute to
the already admitted `ctx.attr.marker`; pinned Bazel 9.2 output remained
identical. The final POSIX fixture has one explicit marker-leaf toolchain,
selected `container-image=run:selected` platform, executable FileWrite, and
A/B/failing-B/A rows. Nondiscriminating generated capture fields were pruned
from the message-shape expectation; its fixture patterns still own every
argument/stdout/stderr/exit byte.

Pinned Bazel 9.2 refresh/replay passed at
`20260811-220914-230144-bazel` and `20260811-222448-238556-bazel`. Rebuilt
Slug replay passed at `20260811-222855-241623-slug`; the monitored replay
`20260811-222109-238219-slug` observed exactly daemon PID `238261` across
all four rows and cleaned its socket/PID. Each row reports one REAPI action,
zero direct-local actions, selected properties, and executable mode `0555`.
Invalidations are `0/1/0/1`; action digests are A/B/B/A with exact A
restoration. A direct no-output-base execution independently passed ordered
args, environment clearing, owner materialization, one-shot evidence, and exit
zero. The focused NativeLink bytes/digest/mode test passed.

Focused/full results: commands 20/20, analysis 39/39, server 50/50, REAPI
5+14 passing with its one endpoint-required test also run and passing, CLI Run
2/2, oracle endpoint 7/7, and direct compile/format checks pass. Core is
173/174 only on the unchanged external-visibility wording baseline; CLI is
50/51 only on the unchanged unavailable-root loadfiles baseline. The full
Python harness is 101/104 on three unrelated stale fixture-test baselines.
Archive active-layout checks pass; only the known absent V1 refs/record and
canonical-reference checker baseline fail.

Cleanup removed a no-op metadata binding and duplicated request reconstruction,
centralized cross-command plan rejection, and added output-root symlink/type
validation. Final growth is 606 production and 217 test Rust lines, 823 total;
harness growth is 7 production and 47 test lines; fixture net growth is 56.
Allowlist, credential grep, formatting, and diff checks pass. Compatibility
remains exact for the admitted target/action/args/stream/numeric-exit relation,
Slug-native for paths/wire/cwd/environment/process/signal envelope, and deferred
for every broader Run surface listed by the accepted design.

Independent final implementation review returned `ACCEPT`. It verified the
borrowed request-local semantic view, resolved-only FileWrite executor path,
provider/action/runfiles guards, owner-root and symlink/mode checks,
client-only process ownership, bounded no-secret daemon wire, exact build
failure classifications, endpoint evidence, lifecycle proof, caps, and
allowlist. The POSIX executable FileWrite Run vertical is accepted.

Run next only `WP-8-m7-filewrite-test-handoff-design`: audit pinned Bazel
9.2 test ownership and freeze the first bounded FileWrite Test vertical.
### FileWrite Test handoff replan (2026-08-11)

`WP-8-m7-filewrite-test-handoff-design` ends `REPLAN` before fixture or
production edits. Pinned Bazel 9.2 does not implement Test as Run plus
command-owned status. `RuleConfiguredTargetBuilder.initializeTestProvider`
requires runfiles support and constructs `TestProvider` parameters from the
executable, test attributes/tools, execution requirements, and optional
`RunEnvironmentInfo`. It then creates a distinct non-shareable
`TestRunnerAction`, not a client launch of the rule's FileWrite action.

The TestRunner action consumes the executable, runfiles tree, setup/XML tools,
and test environment. It owns test log, XML, cache-status, stderr,
undeclared-output, timeout, shard/run, coverage, and infrastructure-failure
state. Result analysis and Test command reporting separately own aggregate
status and terminal exit. The existing stale `test-basic` manifest already
exposes these synthetic result artifacts; no extra oracle row is needed to
prove that Run equivalence is false.

Slug loading does retain test capability, size/timeout/flaky/shard/local/args,
and the implicit Bazel test-tool labels. The configured result retains only
the rule-declared FileWrite and built-in providers, however: there is no
TestProvider, TestRunner action, declared test result set, action identity, or
result analyzer. The accepted Run view deliberately rejects test rules and
additional runfiles. Reusing it would silently omit exact observable action,
runfiles, cache, timeout/environment, log/XML, and result semantics.

Compatibility remains exact only for the already accepted rule-declared
FileWrite producer. Slug-native configured/action/path identity remains
separate. All Test execution/result semantics remain unsupported/deferred
until structurally modeled and must fail closed; no client/local shortcut or
BEP claim is admitted.

Run next only `WP-8-m7-test-runner-semantic-design`. Audit pinned Bazel
`RuleConfiguredTargetBuilder`, `TestActionBuilder`, `TestRunnerAction`,
strategies, result analyzer, Test command, and BEP ownership. Freeze one compact
retained test-action model derived during configured analysis, with structural
DICE equality and separated Slug/Bazel/REAPI/result identity domains. Inspect
the Stage 9 Buck2/V1 action representation candidates, prefer existing compact
Arc slices/small deterministic collections/Allocative, and add no parallel
graph or command-owned reconstruction.
### Embedded test-tools closure REPLAN (2026-08-11)

The design-only closure audit ends `REPLAN`. Pinned Bazel 9.2
`tools/test/BUILD` is not an isolated five-file package: it loads
rules_shell 0.6.1, selects through `src/conditions` into platforms 1.0.0,
defines default test toolchains/config settings/filegroups/coverage aliases,
and is registered by the embedded module. Slug has no built-in repository
source owner and routes only root direct local overrides.

A pruned BUILD, synthetic package, content-free labels, or host Bazel install
scan would violate exact verbatim content and structural identity. No fixture,
content, or Rust changed. Run next only
`WP-4-5-builtin-bazel-tools-repository-owner-design`; it owns immutable
canonical routing/source bytes only, not package evaluation or Test semantics.

### Built-in source-kind prerequisite REPLAN (2026-08-11)

The immutable built-in owner implementation is not accepted. Before returning
to embedded test-tools closure, Stage 5 must freeze whether the partial catalog
uses a file-only wrong-kind terminal or a general expected-kind key. No route,
catalog, package, or Test activation from the failed attempt is retained.

### Immutable bazel_tools source owner accepted (2026-08-12)

The Stage 5 prerequisite now owns the canonical route and reviewed seven-file
partial source catalog with no Host fallback or package activation. This does
not yet make `@@bazel_tools//tools/test` loadable and admits no TestProvider,
TestRunner, runfiles, execution, result, or BEP semantics.

Run next only `WP-4-6-8-bazel-tools-test-closure-design`: audit the complete
pinned Bazel 9.2 repository/source/package/config/toolchain closure, freeze its
DICE ownership and exact catalog expansion, and schedule no production
representation before independent design acceptance.

### Embedded test-tools closure design REPLAN (2026-08-12)

Pinned Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` disproves a bounded
route-to-package implementation. The exact load-time evidence is:

- `buildfiles(@bazel_tools//tools/test:all)` returns
  `@bazel_tools//tools/test:{BUILD,default_test_toolchain.bzl}`,
  `@@rules_shell+//shell:{BUILD,sh_binary.bzl}`, and
  `@@rules_shell+//shell/private:{BUILD,sh_binary.bzl,sh_executable.bzl}`;
- `loadfiles` returns the four Bzl files from that set;
- `:all` contains 20 rules: eight filegroups, one sh_binary, two aliases,
  three toolchains, one toolchain type, one empty-toolchain rule, one bool
  build setting, and three config settings; and
- the package declares ten source labels. The Linux embedded archive contains
  eight: the accepted catalog already owns five, while
  `collect_cc_coverage.sh` (SHA-256 `431ced84...cc552`, 9482 bytes),
  `collect_coverage.sh` (`e1df052d...106fb9`, 9960 bytes), and
  `extensions.bzl` (`ab2c246f...bdf7e`, 1610 bytes) remain unowned; all
  three have archive mode 755. Windows-only `tw.exe` and `xml.exe` are
  query-visible source labels but absent from the Linux archive and remain
  deferred rather than becoming invented missing-file claims.

The embedded MODULE maps `rules_shell -> rules_shell+`,
`platforms -> platforms`, the remote-coverage extension repo, and its other
ordinary/generated dependencies. `rules_shell` 0.6.1 in turn maps
bazel_features, bazel_skylib, platforms, its generated local shell, and
bazel_tools. Pinned `mod graph` displays only the user root: this injected
built-in module and its registrations are deliberately outside the ordinary
root graph, so a root dependency or two-name mapping would not be equivalent.

The live Slug audit found four independent owner gaps. `RootModuleGraph`
contains only user-root evaluation/resolution; `RootRepositoryRouteKey`
accepts only root apparent names. Repository package/source keys enter the
direct-local Host pipeline, while external Bzl resolution rejects every
repository-qualified load. `PackageRecorder` carries no canonical repository
or mapping, rejects `@repo` labels, and emits `@@//` labels even for external
packages. Finally, Stage 6 accepts only root registrations and root toolchain
types. The package's aliases, config settings, executable/dependency-bearing
Starlark rule, contextual platforms labels, and external registrations cannot
be activated by deleting one guard.

Therefore `WP-4-6-8-bazel-tools-test-closure-design` ends `REPLAN`.
Pruning the embedded MODULE to rules_shell/platforms, fabricating RepoSpecs or
repository mappings, scanning the Host Bazel install, or widening package/Test
semantics in one packet would violate structural identity and the reviewed
caps. The fixture remains unchanged; its stale Bazel 9.1 Windows manifest is
still evidence only for the later Test-result surface.

Run next only `WP-5-builtin-bazel-tools-module-injection-design`. It must
freeze the complete hidden built-in MODULE evaluation/resolution, contextual
repository mappings, extension-generated names, registration ownership, DICE
equality/invalidation, and root Need/error order before any catalog expansion,
package/Bzl dispatch, configured toolchain, TestRunner, execution, or BEP work.


### Built-in MODULE injection design REPLAN (2026-08-12)

Full injection is not bounded: Bazel adds the built-in to every module and
derives contextual mappings, extension unique names, and registrations only
after ordinary discovery/MVS selection. Slug has no corresponding Host selected
graph, so a root merge or guessed dependency subset is rejected. Stage 8 waits
while Stage 5 implements only the complete callerless embedded MODULE value;
that leaf authorizes no package, Test, toolchain, command, execution, or BEP
consumer.

### Built-in MODULE value prerequisite accepted (2026-08-12)

Stage 5 now retains the complete callerless embedded MODULE value in
`3bc745de`, with exact source/semantic content and no selected-graph or
consumer edge. Stage 8 remains parked. Run only the Stage 5
`WP-5-builtin-bazel-tools-selected-graph-owner-design` prerequisite; no
catalog expansion, package/Bzl dispatch, configured toolchain, TestRunner,
execution, result, coverage, or BEP behavior is authorized here.

### Selected-graph prerequisite narrowed (2026-08-12)

Stage 5's selected-graph design ends `REPLAN` at the missing uniform
per-module discovery/evaluation value. Stage 8 stays parked while only
`WP-5-host-discovered-module-owner-design` freezes the embedded/registry
leaf. No nonregistry, graph/MVS, mapping, catalog, package/Bzl, toolchain,
TestRunner, execution, result, coverage, or BEP behavior is authorized.

The discovered-module design is accepted for one callerless embedded/registry
leaf. Stage 8 continues to wait while
`WP-5-host-discovered-module-owner-implementation` lands no package,
toolchain, Test, execution, result, coverage, or BEP consumer.

The embedded/registry Host discovered-module leaf is accepted in `e7e4a772`.
Stage 8 remains parked while
`WP-5-host-nonregistry-discovered-module-owner-design` audits general
nonregistry source identity. No package/Bzl, toolchain, Test, execution,
result, coverage, or BEP behavior is authorized.

The general nonregistry audit ends `REPLAN` at route-bound include closure
preparation. Stage 8 remains parked while only
`WP-5-host-nonregistry-module-closure-design` audits that Stage 5 owner; no
package/Bzl, toolchain, Test, execution, result, coverage, or BEP behavior.

The nonregistry closure audit ends `REPLAN` at route-bound package policy and
marker lookup. Stage 8 remains parked while only
`WP-5-host-nonregistry-package-preflight-design` audits that Stage 5 owner
and its preselection deleted-package boundary. No package/Bzl evaluation,
toolchain, Test, execution, result, coverage, or BEP behavior.

The nonregistry package-preflight design is accepted. Stage 8 waits while only
`WP-5-host-nonregistry-package-preflight-implementation` lands crate-private
REPO/ignore/marker classification with no MODULE, package-loading, toolchain,
Test, execution, result, coverage, or BEP consumer.

Stage 8 remains waiting during
`WP-5-host-nonregistry-package-preflight-cap-replan`. The cap correction does
not activate a package, graph, command, Test, or execution consumer; it only
preserves the accepted route-independent prerequisite ownership and schedules
the same implementation with truthful measured bounds.

Stage 8 continues waiting while
`WP-5-host-nonregistry-package-preflight-implementation-r2` completes only the
corrected-bounds Stage 5 prerequisite. No consumer activation is authorized.
