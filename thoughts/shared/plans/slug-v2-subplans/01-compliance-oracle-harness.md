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
| `version-bazel9` | `version` or `info release` | message shape includes Bazel 9+ policy |
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

- loading: `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/skyframe/PackageFunction.java`
  and `BzlLoadFunction.java`;
- bzlmod: `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileFunction.java`,
  `BazelModuleResolutionFunction.java`, and `BazelLockFileFunction.java`;
- analysis: `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/skyframe/ConfiguredTargetFunction.java`;
- execution: `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/remote/RemoteActionContextProvider.java`
  and `GrpcRemoteExecutor.java`.

### 1.5 Generated-Oracle Policy

- A fixture is not acceptance evidence while its checked-in oracle has
  `generated: false`.
- `message_shape` is reserved for diagnostics whose text is intentionally
  normalized; build outputs, action structures, and REAPI evidence use exact
  or semantic comparison.
- The First Real Bazel Build gate requires generated Bazel results for
  `simple-rule-action`, `shell-action-reapi`, and `load-invalidation` before a
  Slug result can be called parity evidence.

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

- `tools/v2_oracle run --fixture empty-module-build --bazel /var/mnt/dev/bazel/bazel-bin/src/bazel-dev`
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
cd /var/mnt/dev/slug
python3 -B -m tools.v2_oracle list
git diff --check -- tools/v2_oracle tools/v2_oracle_lib tests/v2_oracle thoughts/shared/plans/slug-v2-subplans/01-compliance-oracle-harness.md
```

Where the test environment includes `pytest`, also run the current focused
harness suite; it does not require Bazel or Slug V2:

```bash
cd /var/mnt/dev/slug
python3 -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py
```

When a local Bazel 9 source binary is available, the existing oracle-first
fixture chain is generated with these exact commands:

```bash
cd /var/mnt/dev/slug
python3 -B -m tools.v2_oracle run --tool bazel --fixture empty-module-build --bazel /var/mnt/dev/bazel/bazel-bin/src/bazel-dev --update-expected
python3 -B -m tools.v2_oracle run --tool bazel --fixture simple-rule-action --bazel /var/mnt/dev/bazel/bazel-bin/src/bazel-dev --update-expected
python3 -B -m tools.v2_oracle run --tool bazel --fixture load-invalidation --bazel /var/mnt/dev/bazel/bazel-bin/src/bazel-dev --update-expected
```

### Next Fixture Packet And Target-State Validation

The next packet creates `workspace-file-ignored` and
`missing-module-warning`, cites the Bazel 9 source or observed behavior behind
each expectation, generates each checked-in `expected/oracle.json`, and adds
focused parsing/comparison tests. Once both fixture directories land, this
block becomes mandatory and replaces any use of `negative-no-workspace` as
parity evidence:

```bash
cd /var/mnt/dev/slug
python3 -B -m tools.v2_oracle run --tool bazel --fixture workspace-file-ignored --bazel /var/mnt/dev/bazel/bazel-bin/src/bazel-dev --update-expected
python3 -B -m tools.v2_oracle run --tool bazel --fixture missing-module-warning --bazel /var/mnt/dev/bazel/bazel-bin/src/bazel-dev --update-expected
python3 -B -m tools.v2_oracle run --tool bazel --fixture workspace-file-ignored --bazel /var/mnt/dev/bazel/bazel-bin/src/bazel-dev
python3 -B -m tools.v2_oracle run --tool bazel --fixture missing-module-warning --bazel /var/mnt/dev/bazel/bazel-bin/src/bazel-dev
python3 -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py
git diff --check -- tools/v2_oracle tools/v2_oracle_lib tests/v2_oracle thoughts/shared/plans/slug-v2-subplans/01-compliance-oracle-harness.md
```

`shell-action-reapi` is a separate target-state gate and is not runnable as a
NativeLink oracle today. After Stage 7 supplies and starts the NativeLink-backed
harness, injects its remote endpoint into the fixture command, and records the
required REAPI evidence, generate the Bazel result with:

```bash
cd /var/mnt/dev/slug
python3 -B -m tools.v2_oracle run --tool bazel --fixture shell-action-reapi --bazel /var/mnt/dev/bazel/bazel-bin/src/bazel-dev --update-expected
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
