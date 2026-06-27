# V2 Oracle CLI

Run with Python from the repository root:

```bash
python3 tools/v2_oracle list
python3 tools/v2_oracle run --fixture empty-module-build --bazel /path/to/bazel --update-expected
SLUG_V2_BIN=target/debug/slug python3 tools/v2_oracle run --fixture version-bazel9
```

`SLUG_V2_BIN` points the harness at the Slug V2 binary created by
`slug_cli_v2`. The harness does not assume the V1 `app/slug` binary is the V2
implementation.