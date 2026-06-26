# Stage 3: Bazel Identity and Layout

## Goal

Define Bazel-shaped identity and path types before package loading and analysis
depend on them.

## Scope

- canonical and apparent repository names;
- labels and package identifiers;
- target patterns;
- execroot and output-root layout;
- external repository paths;
- runfiles and output path string behavior;
- stable serialization for DICE keys and lockfile-facing values.

## V1 Lessons

V1 accumulated too much Buck cell and output-root residue. V2 should not start
with a compatibility exception for `buck-out`; output and exec paths should be
Bazel-shaped unless a Slug extension explicitly documents otherwise.

## Acceptance Criteria

- Round-trip tests for canonical labels, apparent labels, repo mappings, and
  target patterns pass against Bazel oracle fixtures.
- Output path tests cover generated files, tree artifacts, source artifacts,
  external repos, and runfiles.
- Identity structs make illegal states difficult: do not pass raw strings for
  hot graph identifiers.

## Validation

```bash
cargo test -p slug_identity_v2
slug-v2-oracle run --fixture labels-and-output-paths
```
