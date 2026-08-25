# Stage 1: Compliance Oracle Harness

## Goal

Build a fixture harness that runs the same small workspace through upstream
Bazel and Slug V2, then compares behavior at the right level of strictness.

## Bazel Oracle Inputs

Use the local Bazel checkout as the source of truth:

- loading and packages: `PackageFunction`, `BzlLoadFunction`, and loading tests;
- bzlmod: `ModuleFileFunction`, `BazelModuleResolutionFunction`,
  `BazelLockFileFunction`, extension functions, and lockfile tests;
- analysis: `ConfiguredTargetFunction`, `AspectFunction`, platform and
  toolchain functions;
- execution: `RemoteActionContextProvider`, `RemoteSpawnStrategy`,
  `GrpcRemoteExecutor`, remote cache tests, and REAPI protos.

### Canonical Bazel 9 baseline

All new or regenerated parity evidence uses Bazel 9.2.0 at immutable commit
`8220c6198837d5c13d53fea211cf3282aa12408a`. Resolve sources from
`../bazel` with `git show 9.2.0:<path>` or a detached worktree at that commit.
Do not take the sibling checkout's current `HEAD` as the oracle: on the
2026-07-22 review it was newer than Bazel 9.

Older checked-in Bazel 9.1.1 results remain historical evidence until a packet
touches them. Any fixture used to accept M1-M8 must first be regenerated and
verified with the canonical 9.2.0 baseline. A packet may deliberately advance
the Bazel 9 baseline only through a plan update that records the new release
and immutable commit.

### Fixture provenance contract

Every new or refreshed fixture must record, in `fixture.toml` or a companion
manifest consumed by the harness:

- the Bazel release and immutable source commit;
- the exact Bazel source path and, for migrated tests, test class/method;
- translation notes for platform normalization or reduced fixture scope;
- the comparison mode for each artifact (`exact`, `message_shape`, or
  structured semantic comparison); and
- the command used to generate and then independently verify the expected
  result.

A generated JSON blob without this provenance is a probe, not acceptance
evidence. Migrate upstream test themes rather than paraphrasing them from
memory. Initial source families are:

- analysis: `RuleConfiguredTargetTest`, `StarlarkRuleContextTest`,
  `StarlarkRuleClassFunctionsTest`,
  `StarlarkRuleImplementationFunctionsTest`, `DepsetTest`, and focused
  platform/toolchain tests;
- query: `QueryParserTest`, `AbstractQueryTest`,
  `ConfiguredTargetQuerySemanticsTest`, `ProtoOutputFormatterCallbackTest`,
  `ActionGraphQueryTest`, and `src/test/shell/integration/bazel_aquery_test.sh`;
- execution: focused remote shell tests and REAPI proto identity fixtures.

### Zabel-derived fixture-theme backlog (2026-08-12)

Zabel commit `c7298478e2e56262a2f438e9c065325744c9f0fc` supplies
fixture themes and differential-harness lessons, not acceptance output. Port
each admitted theme into this harness, replace donor observations with the
canonical Bazel 9.2 source/test anchor, and preserve the `fixture.toml`
provenance and comparison-mode contract above. Record why any apparently
relevant upstream test is skipped.

Wave A is a just-in-time catalog, not one prerequisite block. Admit each
bounded oracle-only subset immediately before its semantic owner; keep the
unrelated catalog behind the active M1 request-revision vertical and split any
workspace or behavior family that would obscure error precedence.

| Theme | Required discrimination | Likely owner |
|-------|-------------------------|--------------|
| Starlark call and error order | callable lookup before/after argument evaluation, positional and named expansion, duplicate precedence, `*args`/`**kwargs`, dict/comprehension order, provider initializers, and `ctx.actions` calls | Stages 1/4/6 |
| Provider schema and identity | ordered fields, missing versus `None`, large schemas, initializer results, immutable/hashable keys, forwarding, cross-owner identity, and depset topology | Stages 1/6 |
| Action conflict/error precedence | analysis failure versus output conflict, duplicate returned providers, shareable identical actions, and execution-time argument failures | Stages 1/6 |
| Structured aquery topology | artifact, depset, param-file, command-filter, quoting, and stable normalized ID relationships across all admitted formats | Stages 1/8 |
| Toolchain negative and selection cases | zero requested toolchains must not activate broken registrations, direct-platform dedup, exec-config `config_setting`, dependency aspects, and native-test implicit inputs | Stages 1/6/8 |

Wave B begins only after its Stage 5/7 owners exist:

| Theme | Required discrimination | Likely owner |
|-------|-------------------------|--------------|
| REAPI concurrency and interoperability | concurrent Execute, input-upload coalescing, ByteStream reads, tree-artifact inputs, operation progress, cancellation, and exact evidence counts | Stages 1/7 |
| Remote repository output lifecycle | alternative recorded inputs, mutation/reversion, missing CAS data, transport retry, dependent materialization, sparse control files and `.bzl` demand | Stages 1/5/7 |
| Source symlink manifests | source path, target, executable metadata, digest identity, and materialized shape | Stages 1/3/7 |
| Real-workspace ratchet | LLVM Support/Demangle first, then broader LLVM only after every mismatch receives a focused repo-owned fixture | Stages 1/8/10 |

The donor theme index is recorded in
[zabel-adoption-roadmap.md](./zabel-adoption-roadmap.md). Do not copy its shell
scripts or checked-in output wholesale. In particular:

- run the call-order matrix through pinned Bazel and Slug's `starlark-rust`
  host so Java-versus-Rust evaluation-order differences are explicit;
- compare action conflict and duplicate-provider diagnostics at the first
  observable error, not merely eventual failure;
- normalize aquery numeric ids only while preserving their graph topology;
- drive REAPI fixtures through the configured backend and the ordinary Slug
  executor boundary rather than a test-only action path; and
- treat real workspaces as stress evidence, never replacements for focused
  acceptance fixtures.

### Fixture admission checklist

A Zabel-derived fixture is ready only when:

- [ ] the exact Bazel 9.2 source test, class/method, or shell test is named;
- [ ] the translated workspace uses public BUILD, MODULE, Starlark, command, or
      REAPI surfaces;
- [ ] the manifest records the donor theme only as an adaptation note;
- [ ] exact, message-shape, or semantic comparison preserves the
      discriminating relationship;
- [ ] expected output was generated and independently replayed from the pinned
      Bazel baseline;
- [ ] adjacent existing Slug fixtures were checked for duplication;
- [ ] any skipped upstream case records unsupported phase,
      implementation-detail assertion, obsolete behavior, or stronger
      existing coverage; and
- [ ] the packet remains oracle-only unless its current-packet manifest
      separately authorizes implementation.

## Initial Fixture Classes

- `version`, `help`, and incompatible pre-Bazel-9 configuration errors.
- empty module plus empty package loading.
- `BUILD.bazel` with `exports_files`, `filegroup`, and a small custom rule.
- `MODULE.bazel` with one registry dependency and one local override.
- one shell action with declared input/output.
- one negative diagnostic where message shape matters to rulesets.

## Implementation Slices

### 1.1 Harness Layout

- Add a small Python harness with executable `tools/v2_oracle` and library code
  under `tools/v2_oracle_lib/`.
- Use the standard-library `tomllib` parser for fixture files; do not add a
  dependency until the fixture format outgrows it.
- Store fixtures under `tests/v2_oracle/fixtures/<fixture-name>/`.
- Each fixture contains:
  - `workspace/` with `MODULE.bazel`, `BUILD.bazel`, and source files;
  - `fixture.toml` describing commands, comparison mode, expected outputs, and
    expected diagnostics;
  - `expected/oracle.json` for upstream Bazel results;
  - optional extra `expected/` files for exact manifests or normalized event
    slices.
- Each run gets isolated output roots under the artifact root:
  - Bazel: `${SLUG_V2_ORACLE_ROOT:-target/v2o}/ob/<fixture>/bazel`;
  - Slug: `${SLUG_V2_ORACLE_ROOT:-target/v2o}/ob/<fixture>/slug`.
- Failure artifacts are compact and written under
  `${SLUG_V2_ORACLE_ROOT:-target/v2o}/runs/<fixture>/`.

Initial concrete harness files present in the checkout:

- `tools/v2_oracle`
- `tools/v2_oracle_lib/{fixture.py,runner.py,normalize.py,manifest.py,compare.py}`
- `tests/v2_oracle/README.md`
- `tests/v2_oracle/test_v2_oracle.py`
- `tests/v2_oracle/fixtures/{version-bazel9,empty-module-build,exports-and-filegroup,simple-rule-action,load-invalidation,module-local-override,negative-no-workspace}/`

The next Stage 1 fixture packet must add
`tests/v2_oracle/fixtures/workspace-file-ignored/` and
`tests/v2_oracle/fixtures/missing-module-warning/`. Neither directory exists in
the current checkout. Do not run or report either fixture as validation until
the packet adds its workspace, `fixture.toml`, generated Bazel 9 oracle, and
focused harness tests.

### 1.2 Comparison Contract

For every command, record:

- argv and environment allowlist;
- exit code;
- normalized stdout/stderr;
- output file manifest with path, type, mode, symlink target, and digest;
- `MODULE.bazel.lock` digest and selected JSON fields when the fixture touches
  bzlmod;
- BEP event subset when `--build_event_json_file` is supported;
- Slug what-ran/what-uploaded evidence when execution is involved.

Do not compare full logs by default. Each fixture must declare one of:

- `exact`: byte-for-byte output or manifest match;
- `message_shape`: regex-based diagnostic shape;
- `semantic`: structured facts match while incidental text may differ.

Normalize paths, ANSI escapes, tmp directories, output bases, build ids,
volatile digests, timestamps, timing, and host-specific absolute paths before
comparison.

### 1.3 Initial Fixture List

Create these fixtures first:

| Fixture | Command | Comparison |
|---------|---------|------------|
| `version-bazel9` | `version` or `info release` | message shape includes Bazel 9 policy |
| `empty-module-build` | `build //:all` | exact success, no outputs |
| `exports-and-filegroup` | `query //pkg:all` and `build //pkg:fg` | semantic target/output manifest |
| `simple-rule-action` | `build //pkg:write_file` | exact declared output digest |
| `load-invalidation` | build, edit `.bzl`, rebuild | semantic invalidation and changed output |
| `module-local-override` | `build @dep//:target` | lockfile/repo mapping facts |
| `workspace-file-ignored` | `build //...` with `MODULE.bazel` and an otherwise-invalid legacy `WORKSPACE` | semantic success; the legacy file has no effect |
| `missing-module-warning` | `build //...` without `MODULE.bazel` | semantic warning/created-empty-module behavior observed from Bazel |

The existing `negative-no-workspace` scaffold is not an acceptance fixture. It
must be replaced or rebaselined as the two probes above when its Bazel oracle is
generated: Bazel 9 rejects legacy WORKSPACE semantics, but a missing
`MODULE.bazel` is not itself the asserted failure mode.

The two replacement probes are the next bounded fixture packet, not completed
checkpoint evidence. Generate and review their Bazel 9 results before any Slug
implementation attempts to match them.

### 1.4 Oracle Anchors

Keep the fixture mapping tied to local Bazel source:

- loading: `../bazel/src/main/java/com/google/devtools/build/lib/skyframe/PackageFunction.java`
  and `BzlLoadFunction.java`;
- bzlmod: `../bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileFunction.java`,
  `BazelModuleResolutionFunction.java`, and `BazelLockFileFunction.java`;
- analysis: `../bazel/src/main/java/com/google/devtools/build/lib/skyframe/ConfiguredTargetFunction.java`;
- query: `../bazel/src/main/java/com/google/devtools/build/lib/query2/engine/QueryParser.java`,
  `QueryEnvironment.java`, and the `query2/{cquery,aquery}` implementations;
- execution: `../bazel/src/main/java/com/google/devtools/build/lib/remote/RemoteActionContextProvider.java`
  and `GrpcRemoteExecutor.java`.

Resolve every path at the canonical 9.2.0 commit rather than assuming the
working-tree contents match that tag.

### 1.5 Generated-Oracle Policy

- A fixture is not acceptance evidence while its checked-in oracle has
  `generated: false`.
- `message_shape` is reserved for diagnostics whose text is intentionally
  normalized; build outputs, action structures, and REAPI evidence use exact
  or semantic comparison.
- The First Real Bazel Build gate requires generated Bazel results for
  `simple-rule-action`, `shell-action-reapi`, and `load-invalidation` before a
  Slug result can be called parity evidence.

### 1.6 Analysis and query artifact contract

- Analysis fixtures compare structured labels, configurations, providers,
  depsets, toolchains, transitions, and declared actions. A build's final file
  digest cannot substitute for these facts.
- `query` and `cquery` fixtures compare exit status, ordering where Bazel
  defines it, selected output format, and structured target/configuration
  identity.
- `aquery` fixtures capture Bazel's `ActionGraphContainer` through `proto` or
  `jsonproto`, normalize only unstable ids/paths, and compare command lines,
  environment, inputs, outputs and tree artifacts, dep sets, configuration
  ids, mnemonics, execution platform/properties, paramfiles, aspects, and
  toolchains when present.
- The aquery matrix covers every Bazel 9.2.0 formatter: `text`, `commands`,
  `summary`, `textproto`, `proto`, `streamed_proto`, and `jsonproto`, including
  `include_commandline`, `include_artifacts`, `include_pruned_inputs`,
  `include_param_files`, `include_file_write_contents`, `skyframe_state`, and
  invalid combinations.
- Text `aquery` output must render from the same Slug action-query IR as the
  proto formats. `stdout_contains` or a separately assembled debug view is not
  parity evidence.

### Reviewed next packet — `WP-1-oracle-file-mutations` (2026-07-22)

Work packet ID: `WP-1-oracle-file-mutations`

Owner stage and plan: Stage 1,
`thoughts/shared/plans/slug-v2-subplans/01-compliance-oracle-harness.md`.

Goal and gate link: add oracle-harness create, rename, and delete mutations and
generate the Bazel 9.2.0 `glob-directory-invalidation` evidence required before
the M1 directory/glob DICE packet can begin.

Prerequisites and current state: `3659b0f9` supplies the first unified
workspace DICE runtime; the existing `glob-package-boundaries` oracle is
generated from Bazel 9.1.1 and the harness can mutate only an existing file's
contents. `/usr/bin/bazel` reports 9.2.0 when home, system, and workspace RC
handling is left to Bazel. Bazel invocations may use the user's BuildBuddy
configuration from `~/.bazelrc`, but the harness must never inspect, copy,
record, or commit that configuration or its credentials.

Oracle-first artifact:
`tests/v2_oracle/fixtures/glob-directory-invalidation/expected/oracle.json`,
generated and independently rerun with Bazel 9.2.0 at
`8220c6198837d5c13d53fea211cf3282aa12408a`.

Reuse audit: none required because this is a Stage 1 standard-library harness
packet. The subsequent Stage 2/4 implementation has a separately approved
reuse audit; this packet grants no DICE or Starlark bridge design permission.

Exact scope:

- `tools/v2_oracle_lib/{fixture.py,runner.py,compare.py}`;
- `tests/v2_oracle/test_v2_oracle.py`;
- `tests/v2_oracle/fixtures/glob-directory-invalidation/**`; and
- this Stage 1 evidence section after validation.

Bazel subprocesses retain normal RC handling so configured BuildBuddy services
may accelerate them. Fixture artifacts record only the explicit fixture
command and normalized tool path; no RC contents, credential headers, or auth
material may enter the repository or captured evidence.

Decisions reserved for design reviewer: Sol-low review accepted explicit,
mutually exclusive `create`, `delete`, and `rename` operations with
workspace-containment checks, deterministic mutation records, missing-source
errors, and destination-collision errors. The existing edit/content forms
remain supported for current fixtures.

Implementation steps:

1. Extend the parsed fixture model with the mutation operations and consumed
   Bazel release/commit/source/translation/generation/verification provenance.
2. Apply each operation safely inside the copied workspace and cover parsing,
   execution, rejection, and credential-free evidence recording with focused
   tests.
3. Add ordered initial/create/rename/delete `query` commands over one workspace
   and output base, generate the expected result with Bazel 9.2.0, then rerun
   it without `--update-expected`.

Focused validation:

- `python3 -B -m pytest -q -p no:cacheprovider
  tests/v2_oracle/test_v2_oracle.py`;
- `python3 -B -m tools.v2_oracle run --fixture
  glob-directory-invalidation --tool bazel --bazel /usr/bin/bazel
  --update-expected`;
- the same command without `--update-expected`; and
- `git diff --check`.

Evidence and plan update: record the generated Bazel version/commit, exact
source anchors, command results, mutation sequence, and Sol post-review result
in this section before committing.

Stop conditions: stop on a Bazel version mismatch, credential or RC contents
appearing in captured artifacts, a Bazel server restart between fixture
commands, a mutation that escapes the copied workspace, or directory-tree
semantics beyond the reviewed single-path create/delete/rename contract.

Accepted evidence (2026-07-22):

- The harness now parses and executes explicit file `create`, `rename`, and
  `delete` operations while retaining the existing content and
  find/replace forms. Mutations reject workspace escapes, missing sources,
  existing destinations, and missing or symlink destination parents; they do
  not create unrecorded directories.
- `glob-directory-invalidation` records Bazel 9.2.0 and
  `8220c6198837d5c13d53fea211cf3282aa12408a` provenance plus the reviewed
  `DirectoryListingValue`, recursive glob, and package-function source
  anchors. Its generated oracle proves sorted labels across initial, create,
  rename, and delete queries in one output base.
- Generation and an independent rerun with `/usr/bin/bazel` passed. Only the
  first command restarted the Bazel server; the three post-mutation commands
  stayed warm. A credential/header/token scan of the fixture, expected result,
  and independent run artifact found no auth material.
- `py_compile`, fixture listing, a direct standard-library mutation/rejection
  exercise, and `git diff --check` passed. The focused pytest suite could not
  run because this Python environment has no `pytest` module; this is recorded
  as a validation residual rather than a passing command.
- Sol-low post-review returned `ACCEPT` after one correction removed implicit
  destination-parent creation. File-only operations and ordinary
  check/use races inside the private copied workspace remain intentional
  residuals.

### Bazel 9.2.0 glob callable contract evidence (2026-07-22)

`tests/v2_oracle/fixtures/glob-callable-contract` is the oracle-first input for
the reviewed Stage 4 prepared-listing bridge. Its rule names encode glob
results so ordinary Bazel query output captures loading semantics without
requiring Slug query or analysis.

- The generated Bazel 9.2.0 oracle at commit
  `8220c6198837d5c13d53fea211cf3282aa12408a` proves list and tuple inputs,
  explicit excludes, result membership, `exclude_directories=0` returning a
  non-package directory, nested-package exclusion, and a `.bzl` macro's
  `native.glob()` using its caller BUILD package.
- It corrects the earlier draft contract: `include` may be omitted and defaults
  to `[]`; `allow_empty` is unbound, and OSS Bazel 9.2.0 defaults
  `--incompatible_disallow_empty_glob=true`, so omission behaves as False.
  Explicit `allow_empty=True` permits both an empty include and an unmatched
  pattern.
- Separate negative packages capture omitted/default empty failure, explicit
  `allow_empty=False` per-pattern failure, and the non-boolean diagnostic
  `expected boolean for argument \`allow_empty\`, got \`5\``.
- Generation and an independent rerun with normal Bazel RC handling passed.
  Fixture discovery and `git diff --check` passed; a credential/header scan
  returned zero matches. No home-RC contents or authentication material were
  read into, copied to, or captured by the repository. The focused pytest suite
  remains unavailable because this environment has no `pytest` module. Query
  sorts the encoded rule labels independently, so this artifact does not claim
  to prove the order returned by `glob()`; Bazel source plus the focused pure
  matcher regression own that ordering evidence.

## Acceptance Criteria

- The harness records command line, exit code, stdout/stderr, output manifest,
  selected BEP/what-ran facts, and normalized diagnostics.
- Each fixture declares whether it requires exact output, message-shape
  matching, or semantic matching.
- A failed comparison produces a small artifact that an agent can use without
  re-running the whole suite.
- The initial fixture set above can run against upstream Bazel before Slug V2
  implements them, producing checked-in expected outputs or clearly documented
  generated artifacts.
- Stage 1 records action and event facts but does not assert REAPI executor
  boundary policy; Stage 7 owns zero-direct-local execution gates.
- Harness output is deterministic enough that a failed fixture can be reviewed
  with `git diff --no-index` against the previous artifact.

## Target-State Exact Test Criteria

These criteria define Stage 1 acceptance. They are not all runnable from the
current checkout: the two missing-module replacement fixtures are the next
fixture packet, and `shell-action-reapi` remains gated on the Stage 7
NativeLink-backed harness.

- `tools/v2_oracle run --fixture empty-module-build --bazel "$BAZEL_9_BIN"`
  succeeds against upstream Bazel.
- `tools/v2_oracle run --fixture simple-rule-action --update-expected` writes
  a manifest containing exactly the declared output and no undeclared source
  files.
- Editing a loaded `.bzl` file in `load-invalidation` changes the second-run
  output digest and records that the first result was not reused.
- `workspace-file-ignored` proves that legacy WORKSPACE content is not evaluated
  when `MODULE.bazel` is present; `missing-module-warning` captures Bazel's
  empty-module creation and warning behavior rather than asserting failure.
- Execution fixtures must include zero direct-local rows once Stage 7 is active.
- `python3 -m pytest -q tests/v2_oracle/test_v2_oracle.py` covers fixture
  parsing, normalization, manifest comparison, and failure artifact writing
  without requiring Slug V2 to exist.

## Checkpoint Evidence

Stage 1 scaffold checkpoint:

- Added `tools/v2_oracle` and `tools/v2_oracle_lib/` for fixture discovery,
  TOML parsing, isolated workspace runs, normalization, manifest collection,
  comparison, and compact failure artifacts.
- Added initial fixture directories for `version-bazel9`, `empty-module-build`,
  `exports-and-filegroup`, `simple-rule-action`, `load-invalidation`,
  `module-local-override`, and `negative-no-workspace`. The latter is
  superseded by the planned `workspace-file-ignored` and
  `missing-module-warning` probes; it must not be used as parity evidence.
- Local validation: `py -3 tools/v2_oracle list` and bundled-runtime
  `python -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`
  passed on Windows.
- Upstream Bazel expected generation was not updated locally: `bazel --version`
  resolved through Bazelisk and attempted to fetch `latest` from GCS, which is
  unavailable under the restricted network proxy. The checked-in
  `expected/oracle.json` files remain documented placeholders until a local
  Bazel 9 binary/source build is available.
- Stage 8 `rules_rust` probing shortened the default artifact root to
  `target/v2o` and Bazel output bases to `ob/<fixture>/<tool>` so Windows
  toolchain paths do not dominate fixture outcomes; `SLUG_V2_ORACLE_ROOT`
  still overrides the root.

Stage 1 normalization checkpoint:

- Extended oracle text normalization to collapse stale Bazel server
  `--workspace_directory` paths from previous fixture runs under both the old
  temp `slug-v2-oracle/runs/.../workspace` root and the current
  `target/v2o/runs/.../workspace` root. This keeps regenerated expected output
  from preserving unrelated prior run IDs when Bazel restarts its server.
- Validation passed: `py -3 -B -m tools.v2_oracle list`; bundled
  `python.exe -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`.

Stage 1 command environment checkpoint:

- Extended fixture command parsing with `[commands.env]` per-command overrides,
  and runner output now records `env_overrides` alongside `env_allowlist`. This
  lets oracle fixtures model Bazel client environment behavior without leaking
  the host environment into comparisons.
- Validation passed: bundled `python.exe -B -m pytest -q -p no:cacheprovider
  tests/v2_oracle/test_v2_oracle.py`; `yanked-version-env-allowlist` generated
  and compared with `BZLMOD_ALLOW_YANKED_VERSIONS` set through the command
  environment.

Stage 1 simple-rule-action oracle checkpoint:

- Generated `tests/v2_oracle/fixtures/simple-rule-action/expected/oracle.json`
  with local Bazel 9.1.1, then reran the fixture successfully without
  `--update-expected`. The generated manifest contains only
  `bazel-bin/pkg/write_file.txt` with SHA-256
  `dc5b456bbed0dafb1a5719d46d4484453b730745b12083e67b240c953e427a49`.
- Corrected this fixture from `exact` to `semantic`: Bazel server/progress
  stderr and action-count summaries vary across cold and warm runs, while the
  declared output manifest/digest is the intended exact oracle fact. The
  harness still compares that manifest exactly.
- The standard-library fixture listing passed. The focused pytest suite was not
  available in this Linux environment because `pytest` is not installed; this
  is a validation-environment gap, not a claim of test success.

## Validation

### Runnable Current-Checkout Validation

These commands reference only files and fixture names that exist now and run
with the standard-library Python environment; they do not require Bazel, Slug
V2, or the two absent fixture directories.

```bash
cd "$(git rev-parse --show-toplevel)"
python3 -B -m tools.v2_oracle list
git diff --check -- tools/v2_oracle tools/v2_oracle_lib tests/v2_oracle thoughts/shared/plans/slug-v2-subplans/01-compliance-oracle-harness.md
```

Where the test environment includes `pytest`, also run the current focused
harness suite; it does not require Bazel or Slug V2:

```bash
cd "$(git rev-parse --show-toplevel)"
python3 -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py
```

When a local Bazel 9 source binary is available, the existing oracle-first
fixture chain is generated with these exact commands. Set `BAZEL_9_BIN` to a
verified Bazel 9.2.0 binary first; do not point it at the sibling checkout's
newer `HEAD` build:

```bash
cd "$(git rev-parse --show-toplevel)"
python3 -B -m tools.v2_oracle run --tool bazel --fixture empty-module-build --bazel "$BAZEL_9_BIN" --update-expected
python3 -B -m tools.v2_oracle run --tool bazel --fixture simple-rule-action --bazel "$BAZEL_9_BIN" --update-expected
python3 -B -m tools.v2_oracle run --tool bazel --fixture load-invalidation --bazel "$BAZEL_9_BIN" --update-expected
```

### Next Fixture Packet And Target-State Validation

The next packet creates `workspace-file-ignored` and
`missing-module-warning`, cites the Bazel 9 source or observed behavior behind
each expectation, generates each checked-in `expected/oracle.json`, and adds
focused parsing/comparison tests. Once both fixture directories land, this
block becomes mandatory and replaces any use of `negative-no-workspace` as
parity evidence:

```bash
cd "$(git rev-parse --show-toplevel)"
python3 -B -m tools.v2_oracle run --tool bazel --fixture workspace-file-ignored --bazel "$BAZEL_9_BIN" --update-expected
python3 -B -m tools.v2_oracle run --tool bazel --fixture missing-module-warning --bazel "$BAZEL_9_BIN" --update-expected
python3 -B -m tools.v2_oracle run --tool bazel --fixture workspace-file-ignored --bazel "$BAZEL_9_BIN"
python3 -B -m tools.v2_oracle run --tool bazel --fixture missing-module-warning --bazel "$BAZEL_9_BIN"
python3 -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py
git diff --check -- tools/v2_oracle tools/v2_oracle_lib tests/v2_oracle thoughts/shared/plans/slug-v2-subplans/01-compliance-oracle-harness.md
```

`shell-action-reapi` is a separate target-state gate and is not runnable as a
NativeLink oracle today. After Stage 7 supplies and starts the NativeLink-backed
harness, injects its remote endpoint into the fixture command, and records the
required REAPI evidence, generate the Bazel result with:

```bash
cd "$(git rev-parse --show-toplevel)"
python3 -B -m tools.v2_oracle run --tool bazel --fixture shell-action-reapi --bazel "$BAZEL_9_BIN" --update-expected
```

That command must replace the placeholder at
`tests/v2_oracle/fixtures/shell-action-reapi/expected/oracle.json`; a result
with `generated: false` is not acceptance evidence.

## REAPI Oracle Integration Checkpoint

- 2026-07-14 Stage 1 REAPI oracle integration: the harness now starts a local
  NativeLink REAPI service for fixtures that declare `[reapi]
  remote_executor = true` and the tool is slug. Added
  `tools/v2_oracle_lib/nativelink.py` (binary discovery, config generation,
  readiness polling, teardown), a `[reapi]` fixture section, runner injection
  of `--remote_executor=<endpoint>` plus `default_exec_properties`, REAPI
  evidence extraction from slug stderr, and comparison-layer validation of
  `reapi_actions >= 1`, `direct_local_actions == 0`, and nonempty
  action/upload/materialized digest lists. The `simple-rule-action` fixture
  declares `reapi.remote_executor = true`; its checked-in Bazel 9.1.1 oracle
  (manifest digest
  `dc5b456bbed0dafb1a5719d46d4484453b730745b12083e67b240c953e427a49`) is
  compared exactly against the Slug materialized output. This satisfies the
  First Real Bazel Build gate clause 4 for `simple-rule-action`:
  `reapi_actions=1`, `direct_local_actions=0`, and the declared output digest
  matches. Validation: `python3 -B -m tools.v2_oracle run --fixture
  simple-rule-action --tool slug --slug <slug-v2-bin> --timeout 60` reported
  `status: ok`; `python3.12 -B -m pytest -q -p no:cacheprovider
  tests/v2_oracle/test_v2_oracle.py` passed 17 tests (5 new REAPI-lifecycle
  and evidence tests). The `shell-action-reapi` Bazel oracle is still a
  placeholder; it requires the same NativeLink lifecycle applied to a Bazel
  run with `--remote_executor`, which is a later packet.

- 2026-07-14 Stage 1 `shell-action-reapi` oracle landed: the fixture's Bazel
  oracle is now generated (`generated: true`) with Bazel 9.2.0 using
  `--remote_executor` against NativeLink 1.4.0. The `shell-action-reapi`
  fixture declares `[reapi] remote_executor = true` and exercises
  `ctx.actions.run_shell` through the same harness path as
  `simple-rule-action`. Slug produces the same declared output manifest
  (`probe.txt`, digest
  `ac0cb855e0243634730f146e7b14a0dbc8ed0c3271e7b6ca4974c116a87f2a28`, mode
  `0o555`, size 5) as the Bazel oracle, with `reapi_actions=1` and
  `direct_local_actions=0`. This satisfies gate clause 4 for the second
  fixture in the initial chain. The `load-invalidation` fixture (clause 5)
  remains, pending the daemon.
- 2026-07-14 Stage 1 bare-executor and platform-properties fixtures landed:
  `bare-remote-executor-reapi` and `platform-exec-properties-reapi` are now
  live oracle fixtures (both `generated: true`). The harness `[reapi]` section
  gained `default_exec_properties` (injected as
  `--remote_default_exec_properties`) and `worker_platform_properties`
  (injected into the NativeLink scheduler + worker config). The comparison
  layer validates that declared platform properties appear in slug's REAPI
  evidence `platform_properties` field. Python test count is now 20 (3 new
  platform-property tests). Four REAPI fixtures pass end-to-end:
  `simple-rule-action`, `shell-action-reapi`, `bare-remote-executor-reapi`,
  `platform-exec-properties-reapi`. The `load-invalidation` fixture (clause 5)
  remains, pending the daemon.

## Recursive Configured-Analysis Oracle Checkpoint

- 2026-07-22 commit `9e6a4450` added the generated Bazel 9.2.0
  `recursive-custom-rule-providers-actions` fixture at immutable Bazel commit
  `8220c6198837d5c13d53fea211cf3282aa12408a`. Separate `cquery
  --output=starlark` commands prove the qualified structural provider keys
  `//rules:defs.bzl%LeafInfo` and `//rules:defs.bzl%ParentInfo`, their exact
  string fields, canonical configured labels, and returned
  `DefaultInfo.files`.
- The parent declares dependencies in `[second, first]` order and its provider
  value is exactly `second,first`. `aquery deps(//parent:parent)
  --output=text --include_file_write_contents` proves three distinct
  target-owned `FileWrite` actions and exact output/content ownership without
  invoking build execution or materialization. Its assertions are independent
  of action ordering, configuration spelling, and action-key hashes.
- Generation and an independent no-update rerun both passed with
  `/usr/bin/bazel` 9.2.0. Fixture discovery, immutable provenance,
  `generated: true`, whitespace checks, and candidate credential scans passed.
  Bazel used ordinary RC discovery and was allowed to consume the user's
  `~/.bazelrc`. No agent or inspection tool read its contents, and no external
  RC or BuildBuddy credential content was copied, logged into project files,
  or committed. Sol-low reviewed the fixture and returned `ACCEPT`.
- Implementation commit `4f4599e0` consumes this oracle through one recursive
  configured-target DICE key. Focused Slug tests match the fixture's structural
  provider identities, dependency order, returned `DefaultInfo.files`, and
  per-target write-action ownership. The seven affected/downstream crates pass
  together, and exact retained-DICE activation evidence is recorded in the
  Stage 6 owner plan. This does not yet make the fixture runnable through
  Slug's cquery/aquery command surfaces; those remain consumers of the landed
  graph in later packets.

## Loading Query Oracle Checkpoint

- 2026-07-22 commit `7e8993b2` added generated Bazel 9.2.0
  `query-parser-and-sets` and `query-loading-thin-vertical` fixtures at
  immutable source commit
  `8220c6198837d5c13d53fea211cf3282aa12408a`.
- CLI evidence covers quoted literals, `let`, parentheses, set operators and
  duplicate elimination, parse/arity/unknown-function diagnostics, and the
  distinct default/auto versus full text orders. It deliberately does not
  claim internal AST or span evidence.
- Loading evidence covers literal rule/source labels, rule-only `:all` and
  recursive expansion, alias and custom-rule dependency traversal, a
  structural cycle, missing target/package exit 7 diagnostics, and a
  query-visible source label whose backing file is deliberately absent.
- Generation plus independent no-update reruns passed with `/usr/bin/bazel`
  9.2.0. Every command has stable stdout/stderr assertions; fixture discovery,
  provenance, generated markers, whitespace, and candidate credential scans
  passed. Bazel was allowed ordinary RC discovery, including the user's
  `~/.bazelrc`; no agent or inspection tool read its contents, and no external
  RC or BuildBuddy credential content was copied, logged into project files,
  or committed. Sol-low returned `ACCEPT` after requiring and reviewing the
  absent-source-node regression.
- Implementation commit `61ca25db` now passes both fixtures through the Slug
  V2 CLI. A table-driven CLI regression pins the complete accepted loading
  matrix, including the absent source, rule-only expansion, alias/custom-rule
  closure, cycle termination, set result, structural auto ordering, and exit-7
  failures. Source-cited Rust tests separately pin spans, equal-precedence
  binary sequencing, generic calls, `let`, and the 16-entry Bazel 9.2 loading
  registry. The serial six-crate suite passed 67 tests, root independently
  reran both Slug fixtures after final corrections, and Sol-low returned
  `ACCEPT`. This proves only the recorded thin vertical; the remaining
  functions, repositories, patterns, formats, cquery, and aquery stay open.

## Reverse Query and Subtree Oracle Checkpoint

- 2026-07-23 commit `5b7806d7` added the generated Bazel 9.2.0
  `query-rdeps-and-subtree-patterns` fixture at immutable source commit
  `8220c6198837d5c13d53fea211cf3282aa12408a`.
- Its 26 commands cover existing nested and non-package-prefix
  `//pkg/...` expansion, empty/missing subtree failures, unbounded and
  depth-zero/one/two `rdeps`, universe-closure exclusion, multiple roots,
  duplicate seeds, cycles, source/rule seeds, alias/custom-rule edges, empty
  results, and default/auto/full ordering.
- `same_pkg_direct_rdeps` evidence covers source inputs, duplicates, multiple
  parents, alias/custom-rule parents, cross-package exclusion, and an
  edge-specific two-package criss-cross case. Arity errors and integer
  expression operands are also pinned.
- Bazel 9.2 observed that empty and absent subtree patterns both exit 7 with
  `no targets found beneath '<prefix>'`; `rdeps` depth zero returns only an
  in-universe seed; and an integer in an expression position is parsed as the
  target literal `//:1`, then fails with exit 7 rather than a syntax/type
  error. Default/auto and full reverse traversal have distinct pinned orders.
- Generation, the worker's independent rerun, and a separate root no-update
  rerun passed with `/usr/bin/bazel` 9.2.0. Fixture discovery, immutable
  provenance, `generated: true`, all-command assertion coverage, per-file
  whitespace checks, and candidate credential scans passed. Bazel used
  ordinary RC discovery and could consume the user's `~/.bazelrc`; no agent or
  inspection tool read its contents, and no external RC or BuildBuddy
  credential content was copied, logged into project files, or committed.
  Sol-low reviewed the complete fixture and returned `ACCEPT`.
- Implementation commit `cdc5af41` now passes all 26 commands through the
  rebuilt Slug V2 CLI and retained daemon. Its focused tests also preserve the
  preceding two loading-query fixtures, exact prefix/operand-local DICE
  activation, and same-daemon edge/subtree transitions. The serial six-crate
  suite passed 71 tests, root independently reran all three Slug fixtures, and
  Sol-low returned final `ACCEPT`. M3 remains open for the residual registry,
  repositories, patterns, order modes, and formatters.

## Path Topology Oracle Checkpoint

- 2026-07-23 commit `2b73c08d` added the generated Bazel 9.2.0
  `query-path-topology` fixture at immutable source commit
  `8220c6198837d5c13d53fea211cf3282aa12408a`.
- Its 43 commands cover `allpaths` and `somepath` over linear, diamond, cycle,
  zero-length, disconnected, empty, duplicate, multiple-origin/destination,
  source-direction, alias, and custom-rule topology. Arbitrary diamond and
  multi-pair results admit only bounded complete shortest paths; arity and
  both integer-operand positions are pinned.
- Bazel preserves forward insertion order for a root-node `somepath` under
  default/AUTO output, while a top-level union containing that call returns
  lexical AUTO order. The two nested rows deliberately distinguish those
  policies; `QueryCommand.java:112-118` and
  `QueryExpression.java:110-114` supply the source boundary.
- Generation and two independent sequential no-update reruns passed with
  `/usr/bin/bazel` 9.2.0. All 43 exits and anchored output patterns,
  provenance, whitespace, and fixture-only candidate credential scans passed.
  Bazel could consume the user's external `~/.bazelrc`; no agent or inspection
  tool read its contents, and no external RC or BuildBuddy credential content
  was copied, logged into project files, or committed. Sol-low returned
  `ACCEPT`.
- Implementation commit `7d851ce9` now passes all 43 rows through the rebuilt
  Slug V2 CLI and retained daemon. It directly reuses the landed unbounded
  reverse-dependency helper for `allpaths`, adds Buck2-derived compact
  integer-index BFS/parent reconstruction for `somepath`, and applies the
  root-node AUTO exception only where the parsed AST meets output ordering.
- Focused tests pin complete activation multisets and same-daemon edge/package
  transitions without a new DICE key, cache, protocol, or filesystem seam.
  Worker and root serial validations each passed the 76-test six-crate suite,
  rebuilt the V2 CLI, and passed all four query fixtures. Formatting,
  ownership/scope, diff, and stale-daemon checks passed; Sol-low returned
  `ACCEPT` before and after broad validation. M3 remains open for the other 11
  loading functions, repository/pattern breadth, ordering modes, and
  formatters.

## Arbitrary Selection Oracle Checkpoint

- 2026-07-23 commit `e8e1d9ef` added the generated Bazel 9.2.0
  `query-some-selection` fixture at immutable source commit
  `8220c6198837d5c13d53fea211cf3282aa12408a`.
- Its 42 commands cover ordinary-query `some`: singleton and finite bounded
  arbitrary selections, equal/excess/zero/negative counts, duplicates, nested
  selection, empty/cycle/recursive/cross-package inputs, AUTO/FULL ordering,
  later operand errors, arity, and signed Java-`int` boundaries.
- The same fixture pins the shared integer seam for `deps` and `rdeps`.
  `2147483647` is accepted; `2147483648` is rejected before lookup; quoted
  negative/minimum depths succeed with empty output; and expression-position
  `2147483648` remains the target literal `//:2147483648`.
- Four ordinary-query stop probes prove that an early valid member does not
  mask a later missing target/package and that the failing command emits empty
  stdout. This permits eager V2 operand materialization without importing Sky
  Query cancellation or a streaming result protocol.
- Final generation and two independent sequential no-update reruns passed
  with `/usr/bin/bazel` 9.2.0. All 42 command records, exits, anchored output
  patterns, generated metadata, provenance, whitespace, and fixture-only
  candidate credential scans passed. Bazel could consume the user's external
  `~/.bazelrc`; no agent or tool read its contents, and no external RC or
  BuildBuddy credential content entered the repository. Sol-low returned
  `ACCEPT`.
- Implementation commit `b25c8aff` closes the fixture's Slug parity gate:
  worker and root independently passed the serial six-crate 82-test suite and
  all five query fixtures, 10+12+26+43+42 = 133/133 rows. The worker runs end
  `030821`, `030825`, `030829`, `030833`, and `030837`; the independent root
  runs are parser `031045-559795`, loading `031045-559816`, rdeps
  `031045-559841`, path `031045-559894`, and some `031045-559794`.
- The landed shared signed-`i32` seam covers `some` count and `deps`/`rdeps`
  depth without narrowing expression-position target literals. The retained
  daemon transition regressions and all fixture-only credential constraints
  remained clean; no new key/cache/protocol/filesystem/lock boundary entered
  the harness or command path.

## Siblings BUILD-file-node Oracle Checkpoint

- Commit `8c28877b` lands the generated 40-command Bazel 9.2.0
  `query-siblings-build-file-node` fixture at immutable source commit
  `8220c6198837d5c13d53fea211cf3282aa12408a`.
- It pins actual active basenames for modern `BUILD.bazel`, fallback `BUILD`,
  root, and dual-file priority; matching exported active BUILD files appear
  once. It covers rule/source/alias/custom/BUILD operands, same/multiple
  packages, implemented set compositions, default/AUTO/FULL ordering,
  zero-edge `deps`/`rdeps` behavior, empty/arity/syntax/missing diagnostics,
  and no partial stdout after a later operand error.
- Authoritative generation
  `target/v2o/runs/query-siblings-build-file-node/20260723-033048-572448-bazel`,
  worker clean no-update
  `target/v2o/runs/query-siblings-build-file-node/20260723-033115-575225-bazel`,
  and root independent no-update
  `target/v2o/runs/query-siblings-build-file-node/20260723-033329-578427-bazel`
  passed all 40/40 records. Schema/generated/tool/provenance, anchored
  assertions, whitespace/diff, and fixture-only hygiene were clean. Bazel may
  consume external `~/.bazelrc` only by invocation; no agent read its contents
  and no RC/BuildBuddy credential material entered the repository.
- The first 35-row draft was not landed: root caught the wrong
  `PackageProvider` source path and absent root/fallback/dual/syntax coverage;
  all were corrected before `8c28877b`. Sol-low final review returned
  `ACCEPT`. At that oracle checkpoint, implementation, exact DICE/daemon
  transitions, and Slug comparison remained pending.

### Siblings BUILD-file-node landed evidence (2026-07-23)

- The fixture chain is base `8c28877b`, attribute correction `20f88c05`, and
  43-row FULL-provenance oracle `1a3dec16`; implementation is `d19a9b29`. The
  attribute-corrected update/no-update/root Bazel runs `034446-589899`,
  `034516-592708`, and `034623-595736` passed. FULL-provenance discovery,
  anchored update, clean no-update, and root runs `035638-609525`,
  `035734-612675`, `035759-615627`, and `035853-619234` passed.
- The provenance rows prove that direct literal `siblings` and its graphless
  union wrapper have the same FULL order, while `siblings(deps(...))` retains
  the recorded dependency-evaluation edge and differs. External RC could be
  consumed only by Bazel invocation; no RC contents or credentials were
  accessed.
- Rebuilt Slug passed the 91/91 serial six-crate gate and six fixtures/176 rows:
  worker `040407-626548`, `040411-626572`, `040414-626601`,
  `040418-626692`, `040423-626782`, `040427-626870`; independent root
  `040534-628098`, `040540-628123`, `040546-628189`, `040549-628247`,
  `040554-628339`, `040558-628428`.

### Build/load provenance oracle checkpoint (2026-07-23)

`8f6f02b3` established the base 58-command fixture; `e8014b25` corrects it to
the 64-command Bazel 9.2 `query-build-load-files-provenance` fixture with a
singleton fake-target topology. Update `051423-694832`, Terra clean
`051521-700085`, and root clean `051644-705470` passed; Sol-low final review
was `ACCEPT`. Its anchors are `BuildFilesFunction`, `LoadFilesFunction`,
`AbstractBlazeQueryEnvironment#transitiveLoadFiles`, `FakeLoadTarget`,
`BlazeQueryEnvironment#getTransitiveLoadFilesHelper`,
`BlazeTargetAccessor#getPackage`, `TargetKeyExtractor`,
`BinaryOperatorExpression#evalPlus/#evalMinus/#evalIntersect`, `QueryUtil`'s
label-key set, and `SiblingsFunction`, all at
`8220c6198837d5c13d53fea211cf3282aa12408a`.

This is an implementation prerequisite, not Slug parity evidence: nine
ordinary functions remain deferred. It establishes transitive/companion,
fake-provenance/set, failure, and factored FULL observations with
`--output=graph --graph:factored`: intersection keeps the left representative,
label-equal `except` is symmetric, and union preserves distinct callback
batches to `siblings`.

### Labels metadata oracle landed (2026-07-23)

`8dfae99c` generated 31 Bazel rows. Exact command names are
`scalar_explicit`, `label_list_explicit_cross_package`,
`label_list_omitted_default`, `label_list_explicit_empty`,
`existing_non_label_attribute`, `absent_attribute`,
`implicit_attribute_dollar_spelling`, `implicit_attribute_wrong_spelling`,
`selector_all_branches_default_no_keys`, `selector_concatenation_deduplicates`,
`output_scalar_generated_target`, `output_scalar_generated_label_kind`,
`output_list_generated_targets`, `output_list_generated_label_kind`,
`second_output_producer_generated_targets`, `outputs_reach_generating_rule`,
`outputs_and_generating_rule_graph`, `outputs_and_generating_rule_deps`,
`distinct_output_generators_deps`, `distinct_output_generators_graph`,
`string_keyed_label_dict_values`, `label_keyed_string_dict_keys`,
`label_list_dict_values`, `source_alias_and_build_operands`,
`duplicate_and_cross_package_composition`, `union_composition`, `default_order`,
`auto_order`, `full_order`, `missing_referenced_target`,
`mandatory_attribute_package_error`. Seven public default label attrs are
covered; dormant attrs excluded. 29 rows are future CLI; two `label_kind` rows
are Bazel-only generated-file representation evidence pending
`QueryNodeKind::GeneratedFile`. Worker/root runs and all fixture checks passed;
pytest unavailable; Sol `ACCEPT`.

Gate A implementation `1b7c179c` is accepted with no query activation. It
retains ordered immutable `Allocative` seven-label-kind-plus-String metadata,
defaults/configurability/provenance/selectors, canonical generated owner,
outputs outside ordinary deps, semantic equality, same-DICE
`BzlModuleEval`→`PackageLoad`→consumer/observer tracking, and a preactivation
guard. Root passed fmt/diff, loading 35/query 39/analysis 11; Sol corrected six
blockers then `ACCEPT`. Stage 8 remains 29 CLI plus two generated-kind rows.

`f3e8ad48` supplies the fixture prerequisite: native `config_setting` retains
sorted compact `values` as a load-only zero-edge `config_setting rule`, with
semantic reorder/change tests and fail-closed unsupported attrs. It does not
evaluate configuration. Sol `ACCEPT`; define/flag/constraint/common attrs and
matching remain deferred. The Stage 8 29+2 boundary is unchanged.

`8fec2696` activates only `labels`. Exact acceptance is 29 non-label-kind CLI
rows including two graph stdout rows; two label-kind rows stay Bazel-only
formatter constraints. Same-DICE/daemon transitions pass (loading 37/query
42/CLI 21 [1+17+3]/server 15/analysis 11, fmt/diff); Sol `ACCEPT`. Six ordinary
functions/M3 remain open; no 31/31 claim or credential exposure.

### Executable rule-capability oracle landed (2026-07-23)

`c8e469f5` adds the generated Bazel 9.2
`query-executables-rule-capability` fixture at immutable upstream commit
`8220c6198837d5c13d53fea211cf3282aa12408a`. Its 40 commands comprise 32
semantic `executables()` rows and eight representation-only
`--output=label_kind` rows. The semantic matrix covers executable/nonexecutable
Starlark rules, exported `_test` exclusion, target-name nonclassification,
native/non-rule negatives, set/let/nesting, exact order and graph behavior,
and failure/no-partial-output contracts. The representation rows pin exact
exported Starlark names plus `filegroup`, `alias`, and `config_setting`; they
are excluded from Stage 8 Slug formatter acceptance.

The accepted `rule(test=True, executable=False)` probe yields an empty query
because its exported class is a test. It does not observe a false executable
capability: pinned `StarlarkRuleClassFunctions#createRule` and
`getTestBaseRule` establish that test forces executable capability
independently. Terra update `085202-880190`, Terra clean `085213-881221`, and
root clean `085303-889108` passed; Sol-low final review returned `ACCEPT`.
Native `genrule` and `test_suite` remain separate substrate gates. Bazel used
ordinary external RC discovery only; no agent inspected or persisted
`~/.bazelrc` or BuildBuddy credentials.

Stage 8 activation `69565a29` accepts all 32 semantic fixture rows through the
Slug CLI while deliberately excluding the eight `label_kind` formatter rows.
The retained-daemon transition matrix and focused DICE equality/reuse checks
also pass. Root validation covered 45 query tests and 50 downstream
CLI/commands/server tests; Sol-low returned final `ACCEPT`. Five ordinary
functions remain deferred, so this is not full M3 acceptance.

### Serialized Slug packet validation accepted (2026-07-23)

Commit `0618a007` adds `scripts/v2_packet_validate.py` as root-owned
orchestration around the existing `tools.v2_oracle` comparison path. It
requires an explicit ordered fixture list, rejects unknown, duplicate, or
ungenerated oracle evidence before building, takes one nonblocking repository
lock, builds `slug_cli_v2` once with one Cargo job, and invokes the existing
Slug oracle CLI sequentially with an explicit binary and unique artifact root.
It does not discover a default suite, update expected output, invoke Bazel, or
duplicate fixture parsing/comparison semantics.

Daemon evidence stays fail-closed. The wrapper never probes and skips Unix
sockets; it preflights the exact socket pathname, treats fixture failure as
failure, and rejects leftover socket/PID markers without adding a kill or
protocol path. The first integration run preserved a real 153-byte socket-path
failure and stale marker; the single correction moved artifacts to a
collision-safe short root and added the pathname guard. A second run passed
`query-parser-and-sets` and all four same-daemon
`glob-directory-invalidation` commands with no marker or process left.

Validation passed 10 standard-library wrapper tests, help/bytecode/diff checks,
three CLI `output_base_` daemon-reuse tests, and eight server
`retained_daemon_` tests. The broader Python harness pytest suite could not run
because `/usr/bin/python3` has no `pytest` module; this remains an explicit
environment residual, not a pass or a reason to weaken the daemon lanes.
Sol-low returned final `ACCEPT`.

### Visibility oracle and activation accepted (2026-07-23)

Oracle commit `a376e30e` pins 39 Bazel 9.2 rows at immutable upstream commit
`8220c6198837d5c13d53fea211cf3282aa12408a`: 25 exact `visible()` commands,
12 accepted non-`visible()` Stage 4 rows, and two Bazel-only flag-structure
rows. Generation and two clean runs passed; all stable fields of the prior 36
records remained unchanged and all 27 pinned source anchors resolved. The
three final discriminators prove cross-package top/include traversal,
real-first real/fake same-label streamed input, and label-keyed predicate
materialization retaining the first fake caller's consuming package.
Independent evidence review returned `ACCEPT`.

Stage 8 implementation `76025ede` passes all 25 semantic rows exactly both
one-shot and through one retained daemon, while preserving the 12-row Stage 4
gate. Focused evidence covers universal and empty predicates, source/generated/
BUILD/package-group/fake kinds, local-negative groups, cycles, wrong-kind and
missing-target behavior, one-way Java visibility, no filtering topology,
ordered singleton callback deliveries, and exact same-DICE cross-package
reuse/invalidation/recovery. The two flag-structure rows remain Bazel-only.
This activates the thirteenth default function; three Java `Pattern`-dependent
functions and the rest of M3 remain open.

### Bzlmod runtime-input oracle accepted (2026-07-23)

Commit `911f16f2` refreshes six Stage 5 fixtures at Bazel 9.2.0 and immutable
upstream commit `8220c6198837d5c13d53fea211cf3282aa12408a`.
`module-include-change-invalidation` covers edit, delete failure, and recreate
recovery in one output base; `module-root-dev-dependency-visibility` covers
default/ignore/default policy; `lockfile-mode-update-refresh` and
`lockfile-version-error` pin visible lockfile creation, preservation, version
28, and exact error behavior; `yanked-version-command-env-union` proves union
of flag and environment policy; and `repo-mapping-canonical-names` pins root,
dependency, multi-version, and extension-generated mapping identities without
claiming materialization.

Generation and two independent clean replay sets passed all six fixtures.
Pinned source anchors resolve, normalized output contains no host paths, the
visible lockfile manifest digest is
`38731963ff6d7df650a7355090c4388b7218e064bc75f839531902dc92f98023`,
diff/archive/credential checks passed, and independent final review returned
`ACCEPT`. Hidden output-base lockfile ownership, network, fetch,
materialization, and module-extension replay remain explicitly outside this
oracle checkpoint.

### Fixture-growth hygiene baseline (2026-07-24)

The first bounded hygiene review establishes accepted-tree baseline
`3afc1c5a` for `tests/v2_oracle/fixtures`: 1,231 regular files and 27,626
newline-counted lines. It reviewed the five accepted oracle packets
`183970d9`, `51bfc915`, `908c7c62`, `8824135a`, and `eeea40a6`; their retained
fixtures respectively contain 92/1,454, 54/776, 52/398, 50/441, and 14/704
files/lines.

Repeated fixture-local platform and registry subtrees remain necessary for
immutable provenance, isolated local overrides, hermetic topology, and
per-row failure isolation. The pending raw-attribute fixture's copied
six-file/50-line platforms module is explicitly required by its owner packet.
No unused or nondiscriminating module, registry, mutation, manifest field,
expected field, or negative assertion was established, so the pruning
allowlist and affected replay set are empty. The next checkpoint starts from
this baseline and counts accepted oracle packets after `eeea40a6`.

### Fixture-growth hygiene checkpoint (2026-07-25)

The second bounded review compared tracked archives at baseline `3afc1c5a`
and accepted tree `42e38bc3`. The fixture tree grew from 1,231 regular files,
zero symlinks, and 27,626 newline-counted regular-file lines to 1,272 regular
files, ten symlinks, and 31,208 lines: 41 regular files plus ten symlinks, or
51 entries, and 3,582 lines.

The five accepted packets were raw attributes `cffc39b0` (+15 regular files,
+1,183 lines), include composition `203cdaac` (+12, +438), discovery
boundaries `12bb70a1` (+1, +419), package policy `60c24045` (+1 regular file,
+1 symlink, +750 lines), and repository path state `42e38bc3` (+12 regular
files, +9 symlinks, +792 lines). The earlier 3,583-line rollup counted the
package-policy symlink blob as a text line; regular-file newline accounting
corrects it to 3,582.

All 57 reviewed rows remain discriminating: 12 raw-attribute, six include,
eight discovery, 15 package-policy, and 16 path-state rows. Every retained
asset, symlink, mutation, manifest field, expected record, and negative
assertion contributes to a distinct boundary. The only exact repeated
substantive subtree is the raw-attribute fixture's six-file/50-line
`platforms` module copied from `nonroot-module-consumers`; it remains reserved
for fixture-local override closure, hermetic replay, and immutable provenance.
No post-baseline registry subtree was copied, and the remaining apparent
scaffolding overlap is fixture-specific package topology or label/action
identity.

Two independent inventories, root tracked-archive synthesis, and a fresh
terminal review returned `ACCEPT`. The pruning allowlist and affected replay
set are both `none`. The next fixture-growth checkpoint starts from accepted
tree `42e38bc3` and counts later accepted oracle packets only.

### Fixture-growth hygiene checkpoint (2026-07-25, third review)

The five-packet review compared tracked archives at baseline `42e38bc3` and
accepted oracle tree `f01ebd33`. The fixture tree grew from 1,272 regular
files, ten symlinks, and 31,208 newline-counted regular-file lines to 1,284
regular files, 14 symlinks, and 33,789 lines: 12 regular files, four symlinks,
and 2,581 lines.

The accepted packet deltas were root-patch `9fa4fbde` (+3 regular, +1 symlink,
+104 lines), Local lifecycle `dcc19327` (+2 regular, +3 symlinks, +282),
root-MODULE include events `699c3a8e` (+1 regular, +224), terminal
event/execution `7f6c71c9` (no entries, +321), and root main-package-policy
`f01ebd33` (+6 regular, +1,650). The four affected final fixtures retain 71
rows, 56 more than the baseline.

Every retained row, asset, mutation, manifest field, expected record, and
negative assertion remains discriminating. The review found one stale-output
hole in the Local regular-file row; `c039c347` retains its contract-mandated
wrong-kind transition while requiring Bazel's observed refetch warning, so a
cached preceding Missing result cannot pass. Its focused 16-row replay passed.
No substantive subtree was copied. Repeated one-line unfetched registry
metadata and package markers remain required at fixture-local paths; the
relative lifecycle symlinks, nested include, existing terminal topology, and
two-fragment root-policy topology each encode distinct behavior.

Root tracked-archive synthesis and three independent corrected terminal
reviews returned `ACCEPT`. The pruning allowlist and further affected replay
set are both `none`. The next checkpoint starts from accepted tree `c039c347`
and counts later accepted oracle packets only.

### Fixture-growth hygiene checkpoint (2026-07-26, fourth review)

The mandatory five-packet review compared tracked archives at baseline
`c039c347` and accepted oracle tree `22de3631`. The fixture tree grew from
1,284 regular files, 14 symlinks, and 33,789 newline-counted regular-file
lines to 1,303 regular files, 16 symlinks, and 36,985 lines: 19 regular
files, two symlinks, and 3,196 lines.

The accepted packet deltas were exact Bazel v28 schema `eb8c2d23`
(+12 regular files, +1,311 lines), Host visible-lockfile `d20f6557` (+1,
+500), Host RegistryFunction `204ee408` (no entries, +507), Host
registry-file vendor `dd57518e` (+4, +496), and local registry-directory
transport `22de3631` (+2 regular files, +2 symlinks, +382). Their five
affected fixtures retain 61 rows, 40 more than the baseline: 15 v28-schema,
nine lockfile-Off, twelve registry-command, fourteen yanked/vendor, and
eleven nonroot/directory rows.

Per fixture, `bazel-lockfile-v28-schema` grew by 12 regular files, zero
symlinks, and 1,311 lines; `lockfile-mode-off` by 1/0/500;
`registry-command-transport` by 0/0/254;
`registry-yanked-lockfile-mode` by 4/0/749; and
`nonroot-interim-module-graph` by 2/2/382. These fixture deltas, rather than
the packet split above, are the pruning inventory and sum to the exact
19/2/3,196 tree growth.

Every retained row, asset, mutation, manifest field, expected record, and
negative assertion remains discriminating. The sole default-BCR row is the
unflagged original-root/default-registry control; the expunge-only row creates
the synchronous cold server/output-base boundary consumed by its following
Off replay. The repeated portless vendor hierarchy is Bazel's exact
`VendorManager` projection, and its four assets distinguish hit, fatal,
missing, wrong-kind, restoration, and misleading Refresh selection. The two
directory children intentionally share sentinel contents so their distinct
names, rather than child bytes, prove the 80-byte listing. Fixture-local v28
registry files and invalid-UTF8 input remain necessary for hermetic
provenance, typed failure isolation, and exact replay. No exact repeated
multi-file addition is safely shareable.

Tracked-archive synthesis, added-entry/hash/use inventory, all 61 evidence
signatures, and source/parity, implementation/evidence, plus
architecture/fixture-hygiene reviews returned `ACCEPT`. The pruning allowlist
and affected replay set are both `none`. The next checkpoint starts from
accepted tree `22de3631` and counts later accepted oracle packets only.

### Post-checkpoint oracle packet 1 (2026-07-26)

The accepted local registry-directory collation/charset oracle adds exactly
three regular files, three relative symlinks, and 332 newline-counted lines
to `nonroot-interim-module-graph`. That fixture is now 57/5/1,112 and the
full tracked fixture tree is 1,306 regular files, 19 symlinks, and 37,317
lines. Its fourteen retained rows include the new ROOT/ISO-8859-1 listing,
found-empty directory, and ordinary restoration sequence; generation and two
distinct-root replays passed.

This is packet one after accepted baseline `22de3631`. Growth remains below
both the roughly 100-file and 10,000-line review triggers, so the checkpoint
baseline is unchanged and no pruning review is due.

### Fixture-growth hygiene checkpoint (2026-07-27, fifth review)

The mandatory five-packet review compared tracked archives at baseline
`22de3631`, implementation tree `03684d84`, and accepted pruned tree
`e2cc891d`. The fixture tree grew from 1,303 regular files, 16 symlinks, and
36,985 newline-counted regular-file lines to 1,314 regular files, 24
symlinks, and 39,304 lines: 11 regular files, eight symlinks, and 2,319
lines.

The accepted packet endpoint deltas were local directory collation/charset
`d262052d` (+3 regular files, +3 symlinks, +332 lines), host-JVM
startup/reuse `c67dc3a5` (+1/+0/+733), Host-dirent glob semantics
`0a4aa0af` (+4/+6/+484), Bazel-internal string bytes `98b8b0e1`
(+6/+0/+519), and Linux raw-name pattern-lazy glob `03684d84`
(+7/+0/+261). The terminal hygiene correction `e2cc891d` then removed ten
nondiscriminating regular files, one redundant symlink, and ten lines.

Per affected final fixture, `nonroot-interim-module-graph` is 49 regular
files, five symlinks, 1,836 lines, and 22 rows, a baseline delta of
-5/+3/+1,056; `glob-directory-invalidation` is 9/5/666 and 13 rows, a
delta of +3/+5/+483; `starlark-internal-string-bytes` is 6/0/519 and eight
rows; and `glob-raw-name-pattern-lazy` is 7/0/261 and four rows. The four
fixtures retain 47 rows, 32 more than the baseline.

Every retained row, asset, mutation, manifest root and expected field remains
discriminating. The nonroot rows separately prove directory collation,
found-empty and ordinary restoration, ordered/source-sensitive JVM startup
diagnostics, equal-multiset reuse, occurrence restart, and default
restoration. The POSIX glob rows require both absent observations, all kind
transitions, matched-cycle failure and recovery, and their exact relative
assets. The string rows independently prove static/dynamic carriers, five
escape failures, and the byte column. The Linux raw rows isolate byte order,
literal/octal/dynamic wildcard matching, exact `?` rejection, deletion, and
restoration without copying the POSIX topology.

Added-blob, reachability, and repeated-subtree inventory removed the exact
pruning allowlist: nine unused `BUILD.bazel` markers beneath registry modules
`apple_support`, `platforms`, `protobuf`, `rules_cc`, `rules_java`,
`rules_license`, `rules_python`, `rules_shell`, and `zlib`; the unaddressed
POSIX fixture root `BUILD.bazel`; and redundant unmatched dangling link
`pkg/links/unrelated-dangling.bin`. Required registry extension payloads,
nested package markers, directory-transport sentinels, the matched dangling
link, and unmatched cycle remain discriminating and fixture-local. The
affected replay set is exactly `nonroot-interim-module-graph` and
`glob-directory-invalidation`; both full post-prune replays passed from fresh
absolute roots with expected JSON unchanged. The string and raw fixtures also
replayed during the checkpoint, and exact-output fixtures retained empty
manifests.

Tracked-archive synthesis, row/field/mutation/asset inventory, duplicate-blob
inspection, focused and full harness validation, and source/evidence plus
architecture/fixture-hygiene reviews returned `ACCEPT`. The next checkpoint
starts from accepted tree `e2cc891d` and counts later accepted oracle packets
only.

### Post-checkpoint oracle packet 1 (2026-07-27)

Commit `9f42c3e5` extends only `glob-callable-contract` with a simple
terminal-segment matcher package. The fixture grew from 17 regular files,
zero symlinks, 228 newline-counted lines, and four rows to 20/0/306 and five
rows: +3 regular files, +0 symlinks, +78 lines, and +1 row. The whole fixture
tree is now 1,317 regular files, 24 symlinks, and 39,382 lines, the same
+3/+0/+78 delta from checkpoint `e2cc891d`.

The added BUILD and two assets are all discriminating: exact query labels
separately expose bare-star hidden membership, ordinary suffix hidden
exclusion, explicit-dot inclusion, and multiple non-adjacent nonempty star
spans. Pinned generation, two distinct-root callable replays, both protected
glob-fixture replays, 97 harness tests, and structural/cleanup guards passed.
This is packet one after the checkpoint and remains below every fixture-growth
review trigger.

### Fixture-growth hygiene checkpoint (2026-07-27, sixth review)

The mandatory five-packet review compared tracked archives at baseline
`e2cc891d` and accepted tree `8d84d336`. The fixture tree grew from 1,314
regular files, 24 symlinks, 39,304 newline-counted regular-file lines, and 796
command rows to 1,325/24/39,632 and 803 rows: +11 regular files, zero symlinks,
+328 lines, and +7 rows.

The accepted packet deltas were terminal-segment glob matching `9f42c3e5`
(+3 regular files, +78 lines, +1 row), package-boundary projection
`85ba4975` (+5/-15/-1), pure Host glob traversal `5abff72e` (+3/+52/+1),
query package output `c2ba9298` (no entries, +103/+3), and explicit query
label output `8d84d336` (no entries, +110/+3). The final affected fixtures are
`glob-callable-contract` at 20/0/306 and five rows,
`glob-package-boundaries` at 18/0/140 and two rows,
`query-loading-thin-vertical` at 14/0/519 and 15 rows, and
`query-path-topology` at 6/0/1,478 and 46 rows.

Every retained row and asset remains discriminating. Segment matching isolates
bare-star hidden membership, suffix hidden exclusion, explicit-dot inclusion,
and non-adjacent nonempty star spans. The boundary fixture deliberately
replaced its stale two-row topology with the exact six-state projection, and
the traversal row separately proves multi-pattern, zero-depth `**`,
file/directory, and duplicate behavior. The package formatter rows distinguish
root projection, dependency projection, and loaded-file packages; the label
rows distinguish auto, explicit full output, and the exact invalid-`text`
diagnostic.

No added nonempty blob is duplicated across fixtures, and no repeated
substantive subtree, unused mutation, manifest field, expected field, or
negative assertion is safely removable. Tracked-archive inventory, per-commit
and per-fixture attribution, and added-blob inspection therefore set both the
pruning allowlist and affected replay set to `none`. The next checkpoint starts
from accepted tree `8d84d336` and counts later accepted oracle packets only.

### Accepted M1 in-flight loading/source-lock design (2026-08-13)

The accepted design selects one Bazel-only fixture,
`loading-inflight-source-lock`. It adapts the public package-loading FIFO and
same-output-base client-lock theme at pinned Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`; it does not claim that Bazel
executes two same-output-base commands concurrently or that its first
in-flight result defines Slug's final-validation policy.

Pinned authorities are:

- `src/test/shell/integration/client_test.sh:465-495`,
  `test_noblock_for_lock_reuse_server`: a command consumes one package and
  then blocks reading a demanded FIFO `BUILD`; a same-output-base
  `--noblock_for_lock` client exits 9;
- `client_test.sh:286-393`, `test_multiple_commands_same_output_base`:
  same-output-base clients execute sequentially;
- `src/main/cpp/blaze.cc:96-128,286-323` and
  `src/main/cpp/startup_options.cc:73,122`: the output-base lock is exclusive,
  blocking is the default, and the negated startup option exits immediately;
- `src/test/java/com/google/devtools/build/lib/skyframe/PackageFunctionTest.java:896-938`,
  `testTransitiveStarlarkDepsStoredInPackage`: a transitive `.bzl` source is
  a package dependency and changes the next explicitly invalidated result; and
- `src/test/java/com/google/devtools/build/lib/skyframe/LocalDiffAwarenessIntegrationTest.java:104-113,270-291`:
  serial host-change observation is eventual and its test retries.

The design deliberately skips
`ModuleExtensionResolutionTest.labels_readInModuleExtension`: it proves
`ctx.read` output but no FIFO or in-flight mutation and would widen the packet
into module-extension/repository materialization. It also skips
`EditDuringBuildTest`, whose edited action input has an explicitly undefined
first result and belongs to execution rather than the M1 semantic spine.

#### Fixture and five-record timeline

The workspace contains `MODULE.bazel`; `a/BUILD.bazel`;
`a/defs.bzl`; two source sentinels `a/before.txt` and
`a/after.txt`; and `b/gate.txt` only to retain the gate-package directory.
The loaded `defs.bzl` exports the complete `srcs` list for
`//a:root`: V1 names `before.txt` and `//b:b`, while V2 names
`after.txt` and the same gate target. It also prints a version marker.
There is initially no `b/BUILD.bazel`.

The runner creates contained mode-0600 FIFO `b/BUILD.bazel`, starts the
ordinary primary `query deps(//a:root)` command, and owns a writer thread
whose successful blocking `open(O_WRONLY)` proves Bazel reached that package.
Because the `//b:b` edge comes from the already evaluated V1 `defs.bzl`,
this is a causal ordering gate: V1 was demanded and the request is still in
loading. The runner then changes the one `.bzl` sentinel from V1 to V2,
runs the adjacent ordinary `info` contender with
`--noblock_for_lock` against the same output base, writes the fixed
`b/BUILD.bazel` filegroup through the FIFO, collects both clients, replaces
the consumed FIFO with the identical regular file, and continues serially.

The result contains exactly five command records in declaration order:

1. `inflight_v1_loading`: exit 0, one V1 load marker, V1 dependency output,
   no V2 marker or dependency;
2. `same_output_base_noblock`: exit 9 and the normalized public
   lock-contention diagnostic;
3. `post_mutation_v2`: exit 0, one V2 marker and V2 dependency output;
4. `warm_v2_no_replay`: exit 0, the same V2 dependencies and no version
   marker; and
5. `restored_v1`: the existing inverse text mutation restores V1, exit 0,
   one V1 marker and V1 dependencies.

All rows capture one Bazel server epoch. Query outputs and marker
presence/absence use anchored message-shape expressions; exit codes, command
order, group evidence, mutations, and epochs remain literal generated fields.
The lock diagnostic is message-shape normalized for its PID/path material.
The first in-flight V1 terminal is accepted only if generation plus two
fresh-root replays are identical. It is pinned-version observation, not an
upstream guarantee and not a Slug parity requirement; any V2 result,
mixed marker/dependency result, or replay variation returns `REPLAN`.

#### One narrow harness schema and ownership

Permit at most one optional fixture table:

```toml
[concurrent_command_group]
primary = "inflight_v1_loading"
contender = "same_output_base_noblock"
gate_path = "b/BUILD.bazel"
gate_content = "filegroup(name = \"b\", srcs = [\"gate.txt\"], visibility = [\"//visibility:public\"])\n"
mutations = [{ path = "a/defs.bzl", find = "V1_SENTINEL", replace = "V2_SENTINEL" }]
```

The two names reference distinct adjacent ordinary `[[commands]]`; their
declared positions are also their output-record positions. The table reuses
`Mutation` parsing and admits exactly one text mutation, one contained absent
FIFO path, and one fixed UTF-8 release body. It is POSIX-only, Bazel-only,
cannot occur under a manifest root, and cannot coexist with command-local
mutations on its two owned rows. It is not a general scheduler.

`runner.py` owns both `Popen` objects, the FIFO descriptor, writer thread,
one absolute group deadline, stdout/stderr pipes, process groups, gate
replacement, and all cleanup. Early primary exit, gate-open timeout, contender
timeout/success, bad exit, mutation/release failure, incomplete collection, or
a live descendant fails the fixture. One `finally` path signals the writer,
opens a nonblocking cleanup reader when needed, closes descriptors, terminates
then kills and waits for uncollected process groups, joins the writer, unlinks
the FIFO, and chains cleanup failure behind the primary error. No sleep or
polling decides readiness.

The future implementation allowlist is exactly:

- `tools/v2_oracle_lib/fixture.py`,
  `tools/v2_oracle_lib/runner.py`, and
  `tests/v2_oracle/test_v2_oracle.py`;
- one new `tests/v2_oracle/fixtures/loading-inflight-source-lock/` directory
  containing `fixture.toml`, the six workspace files named above, and
  generated `expected/oracle.json`; and
- canonical/current/Stage 1/Stage 2 owner ledgers.

Caps are three harness files, one fixture, seven authored fixture files plus
one generated oracle, five records, 430 net production-harness lines, 380 net
harness-test lines, 150 authored fixture lines, 500 generated-oracle lines,
260 net ledger lines, and 1,850 total net lines. The single allowed correction
is consumed by this cap-only increase after the strict parser plus complete
owned-process cleanup proved the original 260/280 budgets underfit.

Exact compatibility is the pinned Bazel serial package-change relationship,
same-output-base serialization, exit 9, and selected diagnostic/output
relationships. The first in-flight result is Bazel-only recorded evidence.
Slug's overlapping requests, immutable overlays, barriers, final reobservation,
retry, certificate/revision identity, and no-mixed-epoch publication are
Slug-native; the later Rust proof emits no Bazel records. Bazel client
serialization neither requires nor permits Slug's global command lease.

`REPLAN` on a noncausal gate, FIFO rejection, polling/sleeps, unstable first
terminal, a general scheduler, arbitrary fixture executable, second group or
schema, module-extension/repository execution, failure to terminate and reap
both clients, cap excess, or any need for Rust/public-command changes.

### Accepted M1 in-flight loading/source-lock implementation (2026-08-13)

Commit `2ffad088` implements the bounded design above. The strict optional
group parser rejects unknown keys, duplicate/nonadjacent command ownership,
command-local mutations/diagnostics, absent epoch capture, non-exit-9
contenders, manifest-root gate overlap, and a mutation that aliases the FIFO
gate. The POSIX runner uses one mode-0600 FIFO, blocking writer-open readiness,
two owned process groups, one absolute deadline, regular-file replacement, and
guarded terminate/kill/reap/join/unlink cleanup. An already-exited primary is
still passed through collection on failure.

The fixture contains the seven authorized authored files and one generated
oracle. Its five rows retain one server epoch and the exact sequence V1,
exit-9 lock contender, V2, marker-free warm V2, restored V1. Pinned Bazel
9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a` generated the
oracle and two independent fresh-root no-update replays at
`/tmp/slug-m1-oracle-replay-a` and `/tmp/slug-m1-oracle-replay-b` matched
it. A third fresh-root replay after the final cleanup corrections also passed.
These temporary paths identify local evidence only and are not repository
inputs.

Focused harness validation passed 19 tests. The full harness module passed 119
tests; its only three failures are inherited stale expectations that predate
this packet: the accepted extra `load-invalidation.restored_message_v1` row
and two `simple-rule-action` tests that still construct one record after that
fixture grew to three. They are outside this packet's semantic and file scope.

Final net accounting is 325 production-harness lines, 323 harness-test lines,
95 authored fixture lines, 169 generated-oracle lines, and 913 implementation
lines total, all within the corrected caps. Local pinned-source anchors,
`git diff --check`, process cleanup, schema/lifecycle review, and two
independent evidence reviews passed.

This evidence accepts only Bazel's serialized client boundary, exit 9,
diagnostic/output relationships, and the stable pinned V1 observation. It does
not accept V1 as Slug final-publication behavior. The active successor is the
docs-only request-revision/source-certificate design; no further oracle subset
is required before its first private root-host vertical.

### M7A exec-group action-owner evidence accepted (2026-08-19)

The generated Bazel 9.2 `exec-groups-action-platform` record now contains six
cleanly replayed non-summary aquery rows for two actions of one configured
owner. The default action omits the Starlark `exec_group` argument and retains
`default_platform`; the named compile action selects `compile_a` cold/warm.

A same-platform exec-property A/B/A edit changes and restores only the compile
action's opaque ActionKey. Ordered compatible-platform A/B/A mutation moves
only that action to `compile_b` and restores its prior platform/token; the
default action stays byte-for-byte stable in every expected row. The evidence
does not claim the property map is serialized or that ActionKey bytes are exact
Slug requirements.

The five fixture files total 364 physical lines: 156 authored and 208 generated.
Generation and clean replay pass with Bazel 9.2.0; schema/list discovery, fixed
message-shape assertions, provenance, cleanup and diff hygiene pass. No harness,
Slug implementation, other fixture or expected record changed.

Run next only the docs-only
`WP-6-7A-immutable-configured-action-owner-context-design`.

### Fixture-growth hygiene checkpoint (2026-08-25, corrected payload expansion)

Status: `ACCEPT` for the mandatory review from recorded reset `51540963` to
last fixture-tree commit `3ac0a85b`. This closes the overdue threshold before
the selected-registry source oracle adds any file.

Fresh tracked-archive synthesis corrects the prior reset rollup. At
`51540963`, `tests/v2_oracle/fixtures` contains 1,189 regular files, 24
symlinks and 41,449 newline bytes; the canonical payload expands to 163 files
and 984 newline bytes. The exact logical baseline is therefore 1,352 regular
files, 24 symlinks, 42,433 newline-counted lines and 864 command rows—not the
previously published 1,361/24/42,520/864. That earlier total was nine files and
87 lines high; no tracked asset is removed by this accounting correction.

At `3ac0a85b`, the physical fixture tree is 1,244/24/46,109 and the payload
expands to 168 files/1,424 lines. The corrected logical endpoint is therefore
1,412 regular files, 24 symlinks, 47,533 lines and 959 rows: exact growth of
+60 regular files, zero links, +5,100 lines and +95 rows. Raw fixture plus
payload Git scope is 78 touched files and +5,218/-111 lines. Including the five
changed harness/support paths gives 83 and +6,090/-134: 77 fixture paths at
+4,762/-110 plus payload and harness/support at +1,328/-24. Payload container
and harness representation lines are reviewed but are not double-counted as
expanded fixture files.

Thirteen row-bearing packets account for the 95 rows: attr candidates
`4ea8f6c7` (+18); cquery filter/subset/executable/delegation
`31f75b38`/`13d0b1c0`/`28d3fad4`/`f3471f17` (+4/+12/+12/+6);
toolchain topology `5ce69c92` (+7); FileWrite aquery/build/run
`e10ae7df`/`f55c5e77`/`3e24a00d` (+4/+3/+3); root extension usages
`af46bc00` (+5); in-flight loading `2ffad088` (+5); exec-group action owner
`8eecf172` (+5); and rules_rust owner `b7390392` (+11). Corrections
`29e43ce1`, `f9347ff4` and `6fd78a21` retain row counts; `3ac0a85b` changes
only fail-closed tool-specific assertion dispatch.

The fifteen affected final fixtures are `query-attr-observable-candidates`,
`cquery-filter-label`, `cquery-some-root-selection`,
`cquery-executables-rule-capability`, `toolchain-resolution-first-platform`,
`cquery-delegation-topology`, `filewrite-aquery-root-order`,
`load-invalidation`, `simple-rule-action`, `run-basic`,
`root-extension-usage-semantics`, `loading-inflight-source-lock`,
`exec-groups-action-platform`, `rules-rust-073-toolchain-owner` and
`module-extension-use-repo`.

Every retained row remains discriminating within its recorded owner: attr
schema candidates; configured label/subset/executable/toolchain/delegation
topology; FileWrite owner/order/build/run/invalidation; root extension usage;
serialized in-flight loading; action platform/ActionKey sensitivity;
rules_rust bootstrap ownership; and generated-repository success. The last two
do not witness the newly selected source-only observable: rules_rust crosses
deferred declarations and `module-extension-use-repo` is generated-repository
evidence.

Final-blob inspection found no cross-fixture duplicate nonempty file touched by
the interval and no identical fixture TOML or expected JSON. All 60 new logical
paths are nonempty; the only sub-20-byte files are the four demanded
source-lock/delegation markers. Exact duplicate-blob groups remain 115 while
members fall 553 to 551; exact multi-file subtree groups/roots remain 78/229;
no added or changed asset joins either set. Command mutations grow 324 to 347
plus one concurrent-group mutation; manifest roots remain 117; expected
manifest entries grow 210 to 213; expected command records grow 847 to 943.
The +96 expected records versus +95 rows is the previously empty one-row
exec-group expectation generated beside its five new rows. Row, mutation,
manifest, expected-field and asset reachability review found no removable
nondiscriminating entry. The pruning allowlist and affected replay set are both
`none`; accepted generation and packet-local replays remain their behavioral
evidence.

Independent history and architecture/hygiene audits returned `ACCEPT`. The new
fixture-growth reset is `3ac0a85b`. The selected-registry source oracle in
current is packet one and remains below +100 files/+10,000 lines; review again
before packet six or an earlier size trigger.
