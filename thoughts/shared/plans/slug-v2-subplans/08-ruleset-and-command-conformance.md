# Stage 8: Ruleset and Command Conformance

## Goal

Prove Slug V2 works with modern Bazel 9+ rulesets and user commands after the
core loading, bzlmod, analysis, and REAPI surfaces exist.

## Scope

- rules_cc, rules_rust, rules_python, protobuf, bazel_skylib, and rules_oci
  public smoke fixtures.
- `build`, `test`, `run`, `query`, `cquery`, and `aquery` command slices.
- BEP and event output needed by common integrations.
- diagnostics and exit-code compatibility where rulesets depend on them.

## Non-Goals

- Native language-rule fallbacks removed from Bazel 9.
- Android/iOS breadth before the core public rulesets are stable.
- Private workspace-specific fixtures as the only proof for a behavior.

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

1. `build` with target patterns and output reporting.
2. `query` for `deps`, `rdeps`, `kind`, `attr`, `filter`, `buildfiles`, and
   `tests`.
3. `run` with executable target and runfiles.
4. `test` with test result reporting and exit code semantics.
5. `cquery` and `aquery` after configured-target and action IR are stable.
6. BEP JSON subset for build/test integrations.

Initial modules:

- `app/slug_commands_v2/src/{build.rs,run.rs,test.rs,query.rs,cquery.rs,aquery.rs}`
- `app/slug_query_v2`
- `app/slug_bep_v2`

Compare exit code, normalized stdout/stderr, output manifest, selected BEP
events, query output, cquery provider output, and aquery action shape. Missing
Stage 6 or Stage 7 semantics must stay expected-failing with explicit owner
backreferences; Stage 8 should not add local workarounds for analysis or
execution gaps.

### 8.3 Diagnostics and Compatibility Gates

- Version checks through `native.bazel_version` and `bazel_features` must report
  Bazel 9+.
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
- `query-basic` compares text and JSON output for a small graph against Bazel.
- `cquery-provider-starlark` compares configured provider output.
- `aquery-action-shape` compares stable action text/JSON fields.
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

## Validation

```bash
cargo test -p slug_commands_v2 -p slug_query_v2 -p slug_bep_v2
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

Validation run:

```bash
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

- Public ruleset fixtures (`rules_cc`, `rules_rust`, `rules_python`, protobuf,
  bazel_skylib, rules_oci) are still pending fixture creation and dependency
  pinning.
- Slug-side oracle runs for the new command fixtures are pending command runner
  wiring to loading, analysis, REAPI execution, runfiles, and BEP emission.

### Public Ruleset Fixture Start

- Added Bazel 9 oracle fixtures for `rules-cc-basic`, `bazel-skylib-basic`,
  `rules-python-basic`, `protobuf-basic`, and `rules-rust-basic`.
- `rules-cc-basic` covers `cc_library`, `cc_binary`, and `cc_test` with
  `rules_cc` loaded from `@rules_cc//cc:defs.bzl`; it was expanded from the
  Plan34 `rules_cc` fixture theme and updated to BCR-resolved module versions
  observed under Bazel 9.1.1 (`rules_cc` 0.2.17, `bazel_features` 1.42.1,
  `bazel_skylib` 1.8.2, `platforms` 1.0.0, `zlib` 1.3.1.bcr.5).
- `bazel-skylib-basic` covers the `copy_file` rule from `bazel_skylib` 1.8.2.
- `rules-python-basic` covers `py_library`, `py_binary`, and `py_test` with
  stable `rules_python` 2.0.3 and an explicit Python 3.12 toolchain extension.
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
- Bazel 9.1.1 expected oracle output was generated for all five fixtures.
  These fixtures currently use message-shape comparison where platform-specific
  output manifests or Python runtime runfiles would otherwise make expectations
  noisy. Upgrade to output/runfiles comparison once the oracle manifest layer is
  platform-aware.
- Checked BCR metadata for the remaining public ruleset probe: rules_oci 2.3.0.
  Defer that fixture to a focused follow-up slice because its image/package
  rule APIs need a separate oracle probe.

Validation run:

```bash
py -3 -B -m tools.v2_oracle list
py -3 -B -m tools.v2_oracle run --fixture rules-cc-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
py -3 -B -m tools.v2_oracle run --fixture bazel-skylib-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
py -3 -B -m tools.v2_oracle run --fixture rules-python-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
py -3 -B -m tools.v2_oracle run --fixture protobuf-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
py -3 -B -m tools.v2_oracle run --fixture rules-rust-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe
```
