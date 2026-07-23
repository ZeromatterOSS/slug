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
