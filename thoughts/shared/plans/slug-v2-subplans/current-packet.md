# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-label-global-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: Stage 4 recursive `.bzl` loading and evaluator-visible label values
Base: `840d28e7`

Result: produce a docs-only, source-authenticated implementation decision for
the smallest exact Bazel `Label(...)` slice needed by the accepted rules_rust
`rust_analyzer_aspect` declaration. Do not edit Rust or claim that the live
declaration loads during this packet.

## Learned facts to authenticate

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority. Inspect:

- `StarlarkRuleFunctionsApi.Label` for `.bzl` placement, the one positional
  string-or-Label input and constructor contract;
- `BazelModuleContext.ofInnermostBzlOrFail` and
  `StarlarkRuleClassFunctions.label` for the innermost executing Starlark
  function's defining-module context, package-relative parsing and
  repository-mapping use;
- `cmdline.Label` for canonical identity, `str`, `repr`, equality/hash and the
  exposed property/method boundary; and
- `StarlarkRuleClassFunctionsTest.testLabel`, `testLabelIdempotence`,
  `testLabelSameInstance`, `testLabelNameAndPackage` plus
  `StarlarkIntegrationTest.testLabelConstructorFailsInBuildFile` for focused
  observable discrimination.

The accepted rules_rust 0.73.0 archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Its first source-order use is exactly
`str(Label("//rust:toolchain_type"))` inside
`rust/private/rust_analyzer.bzl:210`; record which Label behaviors that
expression needs and stop before later aspect declarations or rule calls.

Inspect the live Slug checkout rather than inventing a second identity domain:

- `slug_identity_v2::CanonicalLabel` and its repository/package/target
  ownership;
- `BzlEvaluationContext`, including whether its source label is enough for the
  admitted top-level same-repository form, why its current outer-module
  installation is insufficient for imported functions, and where typed frame
  provenance plus a future complete repository mapping would belong;
- the existing module-extension `InvocationLabel` Starlark wrapper and all of
  its consumers, deciding whether reuse, rename/move or avoidance preserves a
  natural shared owner; and
- recursive external-Bzl evaluation/freeze plus BUILD calls through a loaded
  alias, so the builtin reads the current evaluator rather than its exporter.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is concept/test guidance only.
Inspect `generic_label.zig` and `generic_label_value.zig`: their useful
architectural lessons are canonical identity retained by the Label value and
a shared builtin resolved against the executing function's defining module
context rather than the outer evaluator or builtin exporter. Do not copy
Zabel code, mapping observers, runtime,
scheduler, side stores or behavior. Bazel 9.2 remains authoritative.

The Buck2 utility-reuse audit is mandatory because a Starlark Label is retained
identity and the repository already has a label wrapper. Prefer the existing
`CanonicalLabel`, compact strings and Starlark frozen/value owners. Record a
concrete reuse/split decision and whether any new interning or cache is
necessary; default to none.

## Questions and decision output

The audit must answer:

1. Can exact compatibility be bounded to `.bzl` construction from the live
   repository-less absolute string, canonical `str`, and BUILD rejection while
   every apparent/canonical external spelling, relative spelling, Label input,
   property/method and attribute conversion remains fail-closed?
2. Does `CanonicalLabel` already supply the required immutable equality/hash
   identity, and which display projection makes `str(Label(...))` acceptable
   to the fixed aspect toolchain adapter?
3. How will a shared/re-exported builtin consult the currently executing
   Starlark function's typed defining-module provenance, and why can no outer
   evaluator, direct alias, importer or BUILD call supply the wrong context?
4. Should the existing `InvocationLabel` move to a small shared loading-owned
   module, be generalized in place, or be avoided? Name every consumer and
   avoid duplicate wrappers over the same canonical identity.
5. Which exact Rust/test files, line and addition caps, validation commands,
   STOP conditions and residual unsupported surface make the successor
   implementation packet independently executable?

If a bounded exact implementation exists, replace this manifest with that
implementation packet and align the canonical and Stage 4 plans. Otherwise
record `REPLAN` or an unsupported boundary. Classify every result as exact,
Slug-native or unsupported/deferred.

The proof matrix must distinguish (a) the live top-level call, (b) a directly
re-exported `Label` alias, including BUILD rejection, and (c) an imported
function whose body calls `Label`, which Bazel resolves in that function's
defining `.bzl` module. Identify an existing typed function-frame source or
`REPLAN`; the outer `Evaluator.extra` module label is not sufficient evidence.

## Ownership, revision and lifetime audit

Name the call-time producer, evaluator context and retained Label value. Any
admitted canonical repository/package/target bytes must be owned by the value
and participate in Starlark equality/hash; display is a projection, not
identity. Repository mappings remain tracked semantic inputs and may not be
guessed from paths or repaired command-side.

State evaluator-scratch versus frozen-module lifetime, DICE invalidation,
overlapping-request behavior and release. The audit may add no key, cache,
registry, task, I/O, observation or async ownership.

## Files, proof and STOP

Only these docs may change:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`.

The final audit selection may add at most 100 lines to the canonical plan and
120 lines to Stage 4; keep this manifest at or below 180 lines. Run
`git diff --check`, plan-alignment search and `scripts/v2_archive_status.sh`.
Require independent terminal review before commit.

STOP on Rust or fixture edits, behavior sourced from Zabel, Java/JVM, network
mutation, a duplicate semantic Label owner, guessed repository mapping,
captured exporter context, BUILD visibility, aspect application, a public
success claim, outer-evaluator context substituted for function provenance or
any cap breach. `REPLAN` rather than widening the audit.
