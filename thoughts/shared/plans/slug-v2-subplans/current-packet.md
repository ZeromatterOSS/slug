# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-struct-builtin-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: Stage 4 Bazel loading globals and retained Starlark `struct` value
Base: `54d28477`

Result: docs-only. Audit the Bazel `.bzl` `struct` builtin now exposed by the
accepted keyword-only dialect, determine whether retained starlark-rust already
implements the exact required value/call surface, and select one bounded shared
globals owner or `REPLAN`. Do not edit Rust in this packet.

## Authority and live terminal

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority. Start with `StarlarkGlobalsImpl`,
`StructProvider`, `StarlarkRuleClassFunctionsTest` struct rows and the Starlark
evaluator tests they depend on. Run a fresh disposable oracle only if pinned
source and existing regressions do not discriminate a required edge.

Fresh query and build against the accepted rules_rust 0.73.0 root both pass
the former `_support(*, ...)` boundary and converge at
`rust/platform/triple.bzl:28`:

```starlark
return struct(
```

Slug reports `Variable struct not found` from recursive external-Bzl
evaluation. Preserve the accepted repository source, materialization, route,
parse and call slices; do not revisit them in this audit.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Inspect it with `git show`, especially
`session_analysis_starlark_semantics.zig` and
`injected_repository_starlark_semantics.zig`: they retain one complete typed
semantics value and project it to all relevant consumers. Use that guidance
when choosing Slug's globals owner, but copy no Zabel code, representation,
fingerprint, scheduler or behavior. Bazel remains builtin/value authority.

## Required read-only audit

1. Trace the exact rules_rust `struct(...)` call and every subsequent struct
   operation needed before the next honest command terminal. Distinguish the
   minimal immediate slice from broader provider/toolchain use.
2. Authenticate Bazel's environment placement. Inventory fixed `.bzl`, BUILD,
   MODULE, REPO, cquery and other relevant environments in pinned
   `StarlarkGlobalsImpl`; do not infer that a builtin belongs everywhere.
3. Audit `StructProvider` and pinned tests for named-only construction, field
   access and absence, immutability, equality, hashing, representation,
   addition, JSON and error behavior. Classify only rows required by the live
   route or needed to prevent an incorrect parity claim.
4. Audit retained starlark-rust `Struct`, `FrozenStruct`, `register_struct`,
   `LibraryExtension::StructType`, argument handling, freeze/equality/hash and
   tests. Determine whether reuse is exact for the admitted slice or needs a
   bounded Rust-native wrapper; do not expose a private engine registration
   ad hoc at multiple call sites.
5. Inventory every `loading_globals()` consumer and any preliminary or legacy
   environment on the live command path. Decide the smallest owner that gives
   all Bazel `.bzl` evaluators the same complete globals while preserving
   BUILD/MODULE/repository distinctions and existing `Print` policy.
6. Define the implementation/test file set, base hashes, line caps and
   validation. Prefer focused retained-value tests plus the existing recursive
   external-Bzl route and fresh rules_rust query/build. Do not vendor rules_rust
   or the downloaded archive.

## Compatibility candidates

- **Exact candidate:** Bazel 9.2 `.bzl` availability plus the required
  named-only construction, frozen field access and value behavior exercised by
  the live rules_rust closure.
- **Slug-native:** Rust storage/layout, valid-Unicode strings, internal error
  representation and nonrequired diagnostic wording.
- **Unsupported/deferred:** blanket BUILD/MODULE/REPO exposure, unrelated
  starlark-rust library extensions, unexercised struct breadth, later
  rules_rust providers/toolchains/actions, M8/M7B and exact output bytes.

## Documentation authority and STOP

This packet may change only the canonical plan, this manifest and Stage 4.
Caps are <=45 canonical, <=190 current and <=230 Stage additions, <=465
aggregate. Record inspected source symbols, live operations, environment
matrix, exact/Slug-native/deferred classification, implementation file hashes
and caps, tests and exactly one successor; obtain independent review before
activating Rust.

STOP on dirty overlap, a need to invent a new struct representation, exposure
outside Bazel's authenticated environments, switching to all starlark-rust
extensions, per-evaluator symbol reconstruction, parser/dialect changes,
source vendoring, Java/JVM, dependency drift or scope above the caps. `REPLAN`
instead of widening.
