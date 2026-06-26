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

## Acceptance Criteria

- The harness records command line, exit code, stdout/stderr, output manifest,
  selected BEP/what-ran facts, and normalized diagnostics.
- Each fixture declares whether it requires exact output, message-shape
  matching, or semantic matching.
- A failed comparison produces a small artifact that an agent can use without
  re-running the whole suite.

## Validation

```bash
slug-v2-oracle run --fixture smoke-empty-package
slug-v2-oracle run --fixture shell-action-reapi
```

The command names are placeholders until Stage 2 creates the binary layout.
