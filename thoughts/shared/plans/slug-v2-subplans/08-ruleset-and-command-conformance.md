# Stage 8: Ruleset and Command Conformance

## Goal

First prove Slug V2 exposes Bazel 9's loading, configured-target, and action
graphs through `query`, `cquery`, and `aquery`. Prove modern rulesets and
execution-oriented commands on those same graphs using per-field compatibility
classification and discriminating semantic action comparisons.

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

## Current status and command dependency order

M3/M4 and bounded FileWrite M5 are accepted. Preserve those slices and use
[bootstrap-readiness.md](./bootstrap-readiness.md) to select M7A breadth.
The semantic dependency order for a newly admitted surface is:

1. `query` over the Stage 4/5 unconfigured loading graph;
2. `cquery` over the accepted Stage 6 configured-target DICE keys;
3. `aquery` over the exact actions retained by those analysis results;
4. only then broaden `build`, `run`, `test`, BEP, public ruleset execution, and
   cache behavior.

`aquery` is the formal Stage 6-to-Stage 7 handoff. The selected
family's `ActionGraphContainer` comparison must satisfy its exact/Slug-native
classification against Bazel 9.2.0 before that family executes. All seven
formatters and unrelated action families are later breadth, not a renewed M5
or bootstrap-wide gate.

### Current M3 status

Live Status in the canonical plan owns scheduling. M3 is accepted with all 16
Bazel 9.2 default query functions and the admitted output formats. `attr`,
`filter`, and `kind` use the reviewed Slug-native `regex` 1.13.1 contract:
compile once, search the exact function-specific candidate strings without
implicit anchoring, enforce bounded parser/NFA/DFA and input limits, and fail
closed with Slug-owned diagnostics.

This is intentionally valid-Unicode compatibility, not Java `Pattern`
emulation. Java-only constructs, lone UTF-16 surrogates, exact Java diagnostic
wording, and UTF-16 error-offset parity are unsupported. Sky Query-only
functions, external-repository breadth, and non-text formats remain later
breadth rather than M3 gates. Historical Java-compatibility feasibility notes
below are evidence only and do not reopen the accepted contract.

### Query engine reuse policy

The admitted query engine already uses extracted generic machinery. Preserve
that boundary when extending it; audit these donors for a demonstrated gap:

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
implementation. Historical seven-function parser and placeholder descriptions
below are not the current M3 boundary.

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

### 8.2A Bazel ActionKey command projection

For every action family whose Stage 6 exact ActionKey projection is accepted,
`aquery` emits that exact Bazel 9.2 ActionKey. The command consumes the
immutable configured-action row and owner context; it must not derive a key
from formatter text, Slug's structural action identity, or the REAPI Action
digest.

Functional action admission and exact ActionKey admission are separate gates.
Each bootstrap family requires complete semantic input/owner facts, structural
invalidation, validated requested-root output ownership, a discriminating action
graph comparison, and canonical Stage 7 REAPI projection. Exact ActionKey output
is not an execution or bootstrap prerequisite.

Record every family's ActionKey field as exact, Slug-native, or
unsupported/deferred in bootstrap readiness. FileWrite has an admitted
Slug-native token. A new Slug-native token requires an explicit field/domain
contract; an unsupported projection must have a documented omission or targeted
unsupported-format diagnostic. A command requesting exact keys fails explicitly
when a selected family's projection is unavailable. No zero, guessed, or opaque
value may be presented as an exact Bazel ActionKey. Unadmitted exact projections
and their missing fingerprint producers are M9 work.
Bootstrap comparison preserves every accepted exact projection byte; it cannot
blanket-normalize ActionKey fields. Exact ActionKey output is a parity and
inspection surface, not evidence that Bazel local-cache or remote-cache entries
are interchangeable.

### 8.3 Diagnostics and Compatibility Gates

- Version checks through `native.bazel_version` and `bazel_features` must report
  Bazel 9.
- Removed native language rules must fail in the same shape as Bazel 9.
- Unsupported flags should be classified as parse, ignored-compatible, or
  planned, never silently accepted as behavior.
- Output paths in command output should be Bazel-shaped, not V1 `buck-out`.

### 8.3A M7A, M8, and M7B scheduling

The command order above is a semantic dependency order, not a requirement to
finish every Stage 8 surface before bootstrap. Divide the remaining work at the
observable bootstrap closure:

- **M7A** admits only the repository sources, rules_rust/provider/toolchain
  semantics, action/input-tree/aquery shapes, `build` behavior, and REAPI
  execution/materialization required to analyze and build Slug through the
  ordinary graph;
- **M8** starts Stage 10.3 analysis and Stage 10.4 fixed-point bootstrap as soon
  as that closure passes its focused Bazel 9.2 and REAPI fixtures; and
- **M7B** resumes `run`, `test`, BEP, unrelated public rulesets, query/action
  formats, and command breadth not required by the accepted bootstrap closure.

After M8, the standalone remote/disk cache library milestone in
[Stage 11](./11-bazel-compatible-cache-library.md) precedes mixed-language
repository breadth. C/C++ and Python work enters M7A only when demanded by
Slug's own production closure. Exact ActionKey inspection remains independent
of these functional ruleset gates.

Do not delay M8 for M7B completeness, and do not hide an M7A semantic gap in a
bootstrap-only command, precomputed graph, Cargo/Bazel delegation, or local
execution fallback.

### 8.4 Stress and Regression Policy

- Public real-world projects are stress evidence only.
- Every discovered bug gets a focused repo-owned oracle fixture before broad
  smoke status is upgraded.
- Private or organization-specific target labels must not enter persistent
  tests or plans.
- `../llvm-project` is an optional complex-project stress corpus after it has a
  valid checkout and the focused gates pass. It is not acceptance evidence and
  was incomplete during the 2026-07-22 review.

### 8.5 Slug-native explain and watch extensions

These are post-conformance Slug extensions, not Bazel parity gates. They may be
scheduled only after the exact underlying query/configuration/action/execution
facts are accepted and their Stage 2/6/7 prerequisites are present.

#### Explain and provenance output

Add a machine-readable core with a concise text renderer that can answer:

- which configuration-affecting inputs and transitions produced a configured
  target;
- which default or named exec group, execution platform, toolchains, and merged
  properties own an action;
- which source certificate and repository/lockfile observations justify reuse;
- whether a node was reused, recomputed, cache-hit, executed, or rematerialized,
  and which changed dependency caused that choice; and
- which semantic identity, display/path projection, Bazel checksum/ActionKey
  projection, and REAPI/CAS digest domain a displayed token belongs to.

The command reads retained producer facts and request-local trace observations;
it does not install a second semantic graph, replay configuration or toolchain
resolution, infer ownership from paths, or make instrumentation counters the
correctness source. Diagnostic format and additional provenance are
**Slug-native**. Any embedded Bazel-compatible labels/configuration/action
facts retain their own exact or deferred classification.

Progress and explain share producer observations only through a bounded
presentation-neutral vocabulary. Neither is a DICE dependency or action-key
input, and sanitized output must not expose remote headers, credentials, full
private environment, or unrequested file contents.

#### Dependency-driven watch

Introduce `query --watch`, `cquery --watch`, and `build --watch` in that order:

1. the initial iteration opens an ordinary immutable request overlay and emits
   exactly the ordinary command result;
2. its accepted source certificate becomes the watched dependency set;
3. filesystem/backend notifications identify candidates only; overflow or
   uncertainty forces conservative reobservation, not guessed invalidation;
4. debounce coalesces presentation work but does not weaken exact final
   validation;
5. each iteration uses fresh command/output memory over the retained semantic
   DICE graph;
6. a changed input cancels an in-flight iteration only under an explicit
   policy, then joins its tasks and retries from a new revision;
7. stdout and diagnostics are published only after source-certificate and
   provisional-value validation; and
8. watched build outputs use Stage 7 atomic accepted/provisional generations
   and repair damaged requested outputs.

A watch packet must prove create/edit/delete/recreate, rename, directory/glob,
transitive `.bzl`, MODULE/lockfile/repository mapping, relevant versus
irrelevant environment, concurrent foreground request, event overflow,
cancellation, failure/recovery, bounded retained memory, and clean shutdown.
The watch dependency set follows actual producer observations rather than a
workspace-wide recursive scan.

Do not reproduce Zabel's custom event backend or large watch scheduler. Prefer
the smallest cross-platform notification abstraction whose events merely
wake the existing Buck2-DICE request/revision validation path.

### 8.6 Real-workspace and measured-performance ratchet

After focused public fixtures pass, promote real-workspace coverage gradually:
LLVM Support/Demangle first, a broader LLVM slice next, and full workspaces only
when their command/ruleset surface is admitted. A real-workspace mismatch must
first become a focused repo-owned fixture. Stress success never upgrades an
unsupported compatibility surface by itself.

For demonstrated hot paths, use the plan-authoring performance discipline:
exact output/RPC invariants, alternating control/candidate runs, predetermined
thresholds, instructions, cycles, wall time, and RSS. Keep a compact ledger of
accepted and rejected experiments. Do not replace `starlark-rust` with Zabel's
runtime; use its call-order fixture themes and measurement method to optimize
the retained Rust engine or Slug Host boundaries only when profiles identify
them.

This section does not change the active M7 packet, make watch a Bazel-exact
claim, or authorize a command implementation without a separate packet.

## Exact Test Criteria

This is the eventual Stage 8 catalog. A packet runs its selected and protected
fixtures; M7A does not require unrelated rulesets or every formatter here.

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
- A structural regression proves `aquery` and Stage 7 derive their distinct
  query and REAPI protobuf projections from the same retained action and owner
  context. Check provenance and each domain's exact fields/bytes/digests; no
  protobuf-schema equality or digest substitution is implied.
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
- Each selected new execution family follows query/configured-analysis/aquery
  admission over the shared graph, with exact fields and named Slug-native
  exceptions preserved. Existing bounded M3/M4/M5 gates stay accepted.
- Admitted commands do not return `planned_placeholder`; any retained placeholder
  belongs only to an unsupported, explicitly diagnosed surface.

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

## Retained command contracts

- Query and cquery consume the ordinary loading/configured DICE graph and retain
  their accepted format, ordering, request, failure and invalidation boundaries.
  Canonical Live Status records the current accepted slices.
- Bounded FileWrite aquery reads the same configured action closure as build;
  literal-owner ordering/framing and aspect-free `deps()` membership are admitted.
  Configuration/path/order and the named FileWrite token exception remain
  explicitly Slug-native where recorded. Broader action/formats require their
  own admission; no formatter-owned action reconstruction is permitted.
- Accepted POSIX FileWrite `run` builds through the resolved FileWrite REAPI
  view, then returns a request-local launch plan. Only the CLI launches the
  program, with local program arguments and inherited streams. Launch is command
  behavior, not a direct-local build action. Both server and client validate the
  owner-derived executable path/type/mode; the daemon never launches the program.
  The existing workspace cwd and inherited environment minus `JAVA_RUNFILES`,
  `RUNFILES_DIR`, `RUNFILES_MANIFEST_FILE`, `RUNFILES_MANIFEST_ONLY`, and
  `TEST_SRCDIR` remain Slug-native. No environment values or program arguments
  enter the bounded daemon wire. Additional runfiles, test rules, other producers,
  and broader Run options remain guarded until separately admitted.
- `test` is not Run plus command-owned status. Configured analysis must own the
  TestProvider/TestRunnerAction and its executable, runfiles, tools, environment,
  timeout/shard/run and declared result inputs. Result analysis and reporting own
  their separate status/exit/BEP projections. Reuse pinned Bazel
  `RuleConfiguredTargetBuilder`, `TestActionBuilder`, `TestRunnerAction`, result
  analyzer and command tests; no client-launch substitute or synthetic built-in
  source closure is admitted. Test breadth remains M7B unless a specific
  bootstrap production action proves an earlier dependency.

## Historical evidence index

Completed chronology is preserved at Git baseline
`c5e7414d77b203a04337c980aacd0d37fd73e501` (short `c5e7414d7`), in this same
path. Retrieve only the relevant range with
`git show c5e7414d7:thoughts/shared/plans/slug-v2-subplans/08-ruleset-and-command-conformance.md`.
The following are line ranges in that immutable version; quoted headings are
search anchors. Its old “current”, “next”, caps and waits are historical, not
scheduling instructions. The current contracts above and canonical Live Status
resolve later compatibility and scheduling changes.

| Baseline lines | Historical anchor / reusable evidence |
|---|---|
| 364–535 | “Command Surface Substrate”; Bzlmod/lockfile bridges and fixture start |
| 536–2244 | “loading-query-thin-vertical”; query parser, traversal, metadata and provenance acceptance |
| 2245–3795 | Java Pattern/attr feasibility and rejected routes; use the current Rust-Unicode contract rather than reopening them |
| 3796–4362 | “FileWrite aquery command/root design”; framing/order, deps membership and platform resolution contracts/proofs |
| 4363–4620 | “Executable FileWrite `run` handoff design”; precise provider/action guards, path validation, daemon wire and implementation evidence |
| 4621–4826 | “FileWrite Test handoff replan”; TestRunner ownership and authentic built-in source prerequisites; old Stage 5 waits are superseded |

### Public Ruleset Fixture Start

This retained anchor serves Stage 9's extraction-ledger reference. The exact
fixture-introduction provenance, validation and residual scope remain at
`c5e7414d7:thoughts/shared/plans/slug-v2-subplans/08-ruleset-and-command-conformance.md`,
lines 475–535, under the same heading. This historical fixture start is not
current public-ruleset acceptance or scheduling authority.
