# Stage 2: Rust Skeleton and Runtime Substrate

## Goal

Create the minimal Slug V2 Rust binary/server skeleton while reusing Buck2
runtime crates for DICE, starlark-rust, events, and REAPI clients without
exposing Buck user semantics.

## Scope

- CLI entrypoint with `version`, `help`, `build`, `query`, `test`, and `run`
  command placeholders.
- Server or daemon boundary only where it helps DICE and warm-state validation.
- Buck2 crate reuse policy and wrapper crates.
- Basic diagnostics, event logging, and test fixture wiring.

## Reuse Policy

Reuse infrastructure:

- `dice`
- `starlark-rust`
- remote execution client/materializer pieces
- event and superconsole infrastructure where it does not leak Buck concepts

Do not expose or depend on:

- Buck cells as the semantic repository model;
- BUCK/TARGETS file discovery;
- Buck target-pattern semantics;
- Buck executor configuration as a user-facing compatibility layer.

## Acceptance Criteria

- `slug version` reports Bazel-9-compatible identity policy.
- `slug help` is V2-specific and does not advertise V1/Buck behavior.
- The oracle harness can invoke the V2 binary.
- The codegraph sees V2 crates as separate from any archived V1 code.

## Validation

```bash
cargo check -p slug
cargo test -p slug_cli_v2
git diff --check
```

Package names are placeholders until the root reset chooses the final crate
layout.
