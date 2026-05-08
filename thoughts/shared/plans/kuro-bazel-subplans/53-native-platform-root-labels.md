# Plan 53: native platform root labels

## Problem

`//sdk:sdk_contents` in `../zeromatter` now reaches the zeromatter
`.bazelrc` host platform, but analysis fails while validating the native
`platform()` target `//bazel/platforms:linux-gnu-host`:

```text
`PlatformInfo` label for `platform()` rule should be a valid target label
Invalid absolute target pattern `zeromatter//bazel/platforms:linux-gnu-host`
unknown cell alias: `zeromatter`
```

Kuro's internal `TargetLabel` display prints root targets as
`<root-cell>//pkg:target`. Native `platform()` copied that display string
into `PlatformInfo.label`, then `compute_platform_configuration` reparsed
the provider label through Bazel target-pattern parsing. For the main repo,
Bazel-shaped labels are `//pkg:target`; `zeromatter//pkg:target` is a Buck
cell-prefixed rendering and is not a valid Bazel root label in this path.

## Fix

When native `platform()` constructs `PlatformInfo`, canonicalize the provider
label for root-cell targets to `//pkg:target`. Leave external repositories
unchanged. This is systemic for all native platforms and keeps the provider
label parseable by the same validation path that consumes it.

## Verification

- [ ] `cargo check -p kuro_analysis`
- [ ] `cargo build --bin kuro`
- [ ] `../zeromatter`: `kuro build //sdk:sdk_contents`
