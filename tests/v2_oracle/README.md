# Slug V2 Oracle Fixtures

The V2 oracle harness runs the same small Bazel-shaped workspace through
upstream Bazel and Slug V2, then compares normalized exit status, output text,
diagnostics, and selected output manifests.

Fixture directories live under `tests/v2_oracle/fixtures/<name>/` and contain:

- `workspace/`: isolated input workspace copied before each run.
- `fixture.toml`: command list, comparison mode, expected diagnostics, and
  manifest roots.
- `expected/oracle.json`: generated upstream Bazel result when available, or a
  placeholder explaining how to regenerate it.

Useful commands:

```bash
python3 tools/v2_oracle list
python3 tools/v2_oracle run --fixture empty-module-build --bazel /path/to/bazel --update-expected
SLUG_V2_BIN=target/debug/slug python3 tools/v2_oracle run --fixture version-bazel9
```

Runs write compact artifacts under `${SLUG_V2_ORACLE_ROOT:-target/v2o}`.
A failed comparison writes `comparison/failures.txt`, `actual.json`, and, when
available, `expected_vs_actual.diff` inside that fixture run directory.

The checked-in placeholders are intentional until a local Bazel 9 source build
is available. Regenerate them only with an upstream Bazel 9 binary or a local
Bazel source checkout built from the V2 plan's oracle anchors.