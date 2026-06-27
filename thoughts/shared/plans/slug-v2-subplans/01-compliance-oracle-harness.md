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
- Each run gets isolated output roots:
  - Bazel: `/tmp/slug-v2-oracle/<fixture>/bazel-output-base`;
  - Slug: `/tmp/slug-v2-oracle/<fixture>/slug-output-base`.
- Failure artifacts are compact and written under
  `/tmp/slug-v2-oracle/runs/<fixture>/`.

Initial concrete files:

- `tools/v2_oracle`
- `tools/v2_oracle_lib/{fixture.py,runner.py,normalize.py,manifest.py,compare.py}`
- `tests/v2_oracle/README.md`
- `tests/v2_oracle/test_v2_oracle.py`
- `tests/v2_oracle/fixtures/{version-bazel9,empty-module-build,exports-and-filegroup,simple-rule-action,load-invalidation,module-local-override,negative-no-workspace}/`

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
| `negative-no-workspace` | `build //...` with WORKSPACE-only root | message shape failure |

### 1.4 Oracle Anchors

Keep the fixture mapping tied to local Bazel source:

- loading: `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/skyframe/PackageFunction.java`
  and `BzlLoadFunction.java`;
- bzlmod: `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileFunction.java`,
  `BazelModuleResolutionFunction.java`, and `BazelLockFileFunction.java`;
- analysis: `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/skyframe/ConfiguredTargetFunction.java`;
- execution: `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/remote/RemoteActionContextProvider.java`
  and `GrpcRemoteExecutor.java`.

## Acceptance Criteria

- The harness records command line, exit code, stdout/stderr, output manifest,
  selected BEP/what-ran facts, and normalized diagnostics.
- Each fixture declares whether it requires exact output, message-shape
  matching, or semantic matching.
- A failed comparison produces a small artifact that an agent can use without
  re-running the whole suite.
- The first seven fixtures above can run against upstream Bazel before Slug V2
  implements them, producing checked-in expected outputs or clearly documented
  generated artifacts.
- Stage 1 records action and event facts but does not assert REAPI executor
  boundary policy; Stage 7 owns zero-direct-local execution gates.
- Harness output is deterministic enough that a failed fixture can be reviewed
  with `git diff --no-index` against the previous artifact.

## Exact Test Criteria

- `tools/v2_oracle run --fixture empty-module-build --bazel /var/mnt/dev/bazel/bazel-bin/src/bazel-dev`
  succeeds against upstream Bazel.
- `tools/v2_oracle run --fixture simple-rule-action --update-expected` writes
  a manifest containing exactly the declared output and no undeclared source
  files.
- Editing a loaded `.bzl` file in `load-invalidation` changes the second-run
  output digest and records that the first result was not reused.
- `negative-no-workspace` fails under both tools once Slug V2 exists, with a
  diagnostic that mentions missing `MODULE.bazel` or Bazel-9-only workspace
  policy.
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
  `module-local-override`, and `negative-no-workspace`.
- Local validation: `py -3 tools/v2_oracle list` and bundled-runtime
  `python -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`
  passed on Windows.
- Upstream Bazel expected generation was not updated locally: `bazel --version`
  resolved through Bazelisk and attempted to fetch `latest` from GCS, which is
  unavailable under the restricted network proxy. The checked-in
  `expected/oracle.json` files remain documented placeholders until a local
  Bazel 9 binary/source build is available.
## Validation

```bash
cd /var/mnt/dev/bazel && bazel build //src:bazel-dev
cd /var/mnt/dev/slug
python3 tools/v2_oracle list
python3 tools/v2_oracle run --fixture empty-module-build --bazel /var/mnt/dev/bazel/bazel-bin/src/bazel-dev --update-expected
python3 tools/v2_oracle run --fixture simple-rule-action --bazel /var/mnt/dev/bazel/bazel-bin/src/bazel-dev --update-expected
python3 tools/v2_oracle run --fixture load-invalidation --bazel /var/mnt/dev/bazel/bazel-bin/src/bazel-dev --update-expected
python3 tools/v2_oracle run --fixture negative-no-workspace --bazel /var/mnt/dev/bazel/bazel-bin/src/bazel-dev
python3 -m pytest -q tests/v2_oracle/test_v2_oracle.py
git diff --check -- tools/v2_oracle tools/v2_oracle_lib tests/v2_oracle thoughts/shared/plans/slug-v2-subplans/01-compliance-oracle-harness.md
```

The command names are placeholders until Stage 2 creates the binary layout.
