# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-provider-doc-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: Stage 4 `.bzl` loading globals and retained user-provider callable
Base: `1a527089`

Result: docs-only. Audit Bazel 9.2's `provider(doc=..., fields=...)`
declaration surface exposed by the accepted rules_rust load, distinguish
loading-time callable construction/export from provider instances and analysis,
and select one bounded implementation or `REPLAN`. Do not edit Rust.

## Authority and live terminal

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority. Inspect the provider builtin implementation,
callable/provider value types and focused source tests for the `doc` and
`fields` contract. Use a fresh disposable oracle only if pinned source and
existing regressions do not discriminate an edge required by the live route.

Commit `1a527089` is accepted through exact `.bzl` `struct` placement plus the
live named construction, field-read and frozen recursive-export slice. Fresh
query and build against rules_rust 0.73.0 now converge at
`rust/private/providers.bzl:17`:

```starlark
CrateInfo = provider(doc = ..., fields = {...})
```

Slug reaches its retained `provider` builtin and reports `doc` as an extra
named parameter. Preserve the accepted source, routing, parsing, globals and
struct slices; this audit must not revisit them.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architecture guidance only.
Inspect it with `git show`, especially
`session_analysis_starlark_semantics.zig` and
`injected_repository_starlark_semantics.zig`: they retain one complete typed
semantic/global value and project it to the relevant consumers. Use that
guidance when selecting the Slug owner, but copy no Zabel code, representation,
fingerprint, scheduler or behavior. Bazel remains provider authority.

## Required read-only audit

1. Trace Bazel 9.2's provider declaration builtin and focused tests for the
   exact signature, defaults, validation, return shape, export/naming lifecycle
   and diagnostic behavior of `doc` and `fields`. Record other parameters but
   do not admit them without a live requirement.
2. Trace the immediate rules_rust provider declarations from
   `rust/private/providers.bzl`, their recursive export/load path and every
   callable/instance operation required before the next honest query/build
   terminal. Separate declaration-only work from configured analysis.
3. Audit Slug's `provider` in `package.rs`, `UserProviderCallable`, freeze and
   module-export behavior, descriptor/schema ownership, provider construction
   and existing tests. Determine whether accepting `doc` can be exact and
   observationally inert for the live declaration slice or whether semantics
   must be retained.
4. Inventory every globals consumer only as needed to confirm the accepted
   `.bzl` owner remains complete. Do not reconstruct provider symbols or
   metadata at evaluation call sites and do not widen BUILD/MODULE/REPO.
5. Classify declaration callable identity, exported naming, documentation,
   field schemas, instance construction/access, equality/hash, formatting and
   analysis integration separately. Unexercised or unowned behavior fails
   closed.
6. Define the smallest implementation/test file set, base hashes, line caps
   and validation. Prefer focused provider declaration/freeze proofs, the
   recursive external-Bzl route and fresh rules_rust query/build. Do not vendor
   rules_rust or a downloaded archive.

## Compatibility candidates

- **Exact candidate:** Bazel 9.2 acceptance and validation of the live
  `provider(doc=..., fields=...)` declaration, callable freeze/export/naming,
  and only the provider instance operations proven necessary before the next
  terminal.
- **Slug-native:** Rust storage/layout, valid-Unicode strings, internal error
  representation and nonrequired diagnostic wording.
- **Unsupported/deferred:** unauthenticated provider parameters, documentation
  introspection not exercised by the live route, broader provider-instance and
  configured-analysis semantics, struct breadth, toolchains/actions, M8/M7B
  and exact output bytes.

## Documentation authority and STOP

This packet may change only the canonical plan, this manifest and Stage 4.
Base SHA-256 values are:

| File | Base SHA-256 |
|---|---|
| `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md` | `af020bae98cd2a68821960da5c28b6276428309183fde254147be43299ecb037` |
| `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md` | `f8f936392c07ad956f15076d830a37a6ffc3a46fcac5ab3f279961ab4a90c202` |
| `thoughts/shared/plans/slug-v2-subplans/current-packet.md` | `bc844be9267d73753703476019ab13011517b18932056bbfec78312250d4e4a5` |

Caps are <=45 canonical, <=190 current and <=230 Stage additions, <=465
aggregate. Record inspected symbols, live operations, owner/lifecycle analysis,
exact/Slug-native/deferred classification, implementation file hashes/caps,
tests and exactly one successor; obtain independent review before activating
Rust.

STOP on dirty overlap, accepting `doc` without authenticating its semantics,
discarding observable provider metadata, mixing declaration and analysis
ownership, exposure outside authenticated environments, per-evaluator symbol
reconstruction, source vendoring, Java/JVM, dependency drift or scope above
the caps. `REPLAN` instead of widening.
