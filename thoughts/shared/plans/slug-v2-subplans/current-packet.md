# Current Slug V2 Packet

Packet: `WP-4-7A-current-rust-analyzer-toolchain-rule-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Type: docs-only compatibility, mapping and ownership audit
Base: `eda81a4d`

Result: audit the next source-order declaration at accepted rules_rust
`rust/private/rust_analyzer.bzl:404-433`. Determine the smallest exact loading
slice for the explicit apparent-self Label and retained `rule(toolchains = ...)`
requirement, or record `REPLAN`. Do not edit Rust in this packet.

## Authority and evidence

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority. Authenticate:

- `StarlarkRuleFunctionsApi.Label`, `BazelModuleContext` and
  `LabelConverter.forBzlEvaluatingThread` for explicit apparent repository
  resolution in the innermost defining `.bzl` module's repository mapping;
- `StarlarkRuleFunctionsApi.rule`, `StarlarkRuleClassFunctions.createRule` and
  `parseToolchainTypes` for toolchain input types, ordering/deduplication,
  canonical identity, mandatory policy, freeze/export and diagnostics; and
- focused Label repository-mapping and Starlark rule-toolchain tests proving a
  module's apparent self-name, imported definitions and invalid/non-visible
  repository behavior.

The accepted rules_rust 0.73.0 archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
The implementation function at lines 404-421 is retained but not executed.
Declaration evaluation first reaches
`str(Label("@rules_rust//rust/rust_analyzer:toolchain_type"))` at line 427,
inside the lines 426-428 one-element `rule(toolchains = ...)` list; the rule
declaration spans lines 423-429.

Trace Slug's shared `StarlarkLabel`, `BzlEvaluationContext`, recursive
`BzlLoadManifest`, selected external route/module mapping inputs,
`rule_toolchain_requirement`, `RuleDefinitionGen.required_toolchains`, freeze
and equality. Identify the first absent mapping fact and whether it has one
bounded producer. Do not infer that apparent `@rules_rust` means the current
canonical repository merely because the names resemble each other.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Inspect `generic_label.zig`, `generic_label_value.zig`,
`toolchain_declaration_resolution.zig` and the declaration-owner/module facts.
Reuse only the lessons that repository mapping is an explicit observed input,
the Label owns canonical identity, and toolchain requirements are a narrow
projection of the defining declaration. Do not copy Zig code, mapping rules,
storage, evaluator, DICE keys or behavior.

## Required decision

1. Establish exact apparent-self resolution, including which mapping owns
   `@rules_rust`, which defining module supplies it, and the failure for absent
   or conflicting mappings.
2. Establish exact `rule(toolchains = ...)` conversion/retention for the one
   fixed string, including producer-relative context, ordering/deduplication,
   mandatory default and export lifetime.
3. Trace the complete Slug producer-to-consumer route. Select no implementation
   unless canonical mapping is already observed and can be projected without
   a guessed alias, filesystem lookup, second label owner or analysis change.
4. If bounded, name exact Rust/test allowlists, base hashes, line/addition caps,
   proof matrix, validation commands and STOP conditions. Otherwise `REPLAN`.
5. Classify the result as exact, Slug-native or unsupported/deferred.

## Non-decisions and STOP

Do not implement, run Bazel, add fixtures, invoke either rule, access
`ctx.toolchains`, resolve/select a toolchain, configure dependencies, run
analysis/actions, advance to `rust_analyzer_detect_sysroot`, widen unrelated
Label forms/APIs, apply aspects, or claim public rules_rust success. Stop on a
missing mapping producer, unbounded Bzlmod coupling, dirty overlap, Zabel
behavior adoption, Java/JVM work or any need for source changes.

Validation is docs-only: verify archive/source hashes and lines, pinned
Bazel/Zabel anchors, live Slug mapping/retention paths, exact docs allowlist,
`git diff --check` and `scripts/v2_archive_status.sh`. Independent terminal
review must return `ACCEPT` before commit.
