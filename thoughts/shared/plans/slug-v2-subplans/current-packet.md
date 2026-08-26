# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-aspect-definition-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: Stage 4 recursive `.bzl` loading and frozen aspect declarations
Base: `a8e18278`

Result: load, bind, export, freeze and recursively import the first live
rules_rust `aspect(...)` declaration while retaining its complete admitted
semantic identity. The packet ends before an aspect is attached to an
attribute, selected from the command line, propagated or analyzed.

## Learned facts and authority

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority:

- `StarlarkRuleFunctionsApi.aspect` defines a `.bzl` global whose
  `implementation` is callable, whose fixed `attr_aspects` is an ordered
  sequence, whose `toolchains` is a sequence of requirements, and whose `doc`
  is string-or-`None`.
- `StarlarkRuleClassFunctions.aspect` requires `.bzl` initialization,
  resolves toolchain labels in the defining module context and constructs a
  `StarlarkDefinedAspect` without running its implementation.
- `StarlarkDefinedAspect.export` assigns the defining module plus the first
  top-level exported name. Imported aliases observe that already-exported
  identity rather than becoming a new aspect class.
- `StarlarkDefinedAspectsTest.simpleAspect`,
  `aspectCanBeDefinedUsingFactory` and
  `aspectCannotBeDefinedInBuildFileThread` authenticate top-level export,
  factory construction and BUILD absence. `StarlarkRuleClassFunctionsTest`
  `aspectAttrs`, `aspectDefaultAttrs`, `starTheOnlyAspectArg` and
  `invalidAttrAspectsType` authenticate ordered fixed propagation attributes
  and their validation. Broader aspect application tests exercise a deferred
  phase.

The accepted rules_rust 0.73.0 archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Slug evaluates external `.bzl` loads recursively and in source order.
`rust/defs.bzl` first reaches `rust/toolchain.bzl`, which first reaches
`rust/private/rust_analyzer.bzl`; after its already-admitted `rustc.bzl` and
`utils.bzl` children freeze, line 207 is:

```starlark
rust_analyzer_aspect = aspect(
    attr_aspects = ["srcs", "deps", "proc_macro_deps", "crate", "actual", "proto"],
    implementation = _rust_analyzer_aspect_impl,
    toolchains = [str(Label("//rust:toolchain_type"))],
    doc = "Annotates rust rules with RustAnalyzerInfo later used to build a rust-project.json",
)
```

Slug has no `aspect` global, so this is the first internal source-order stop
after all accepted String/Boolean/StringList descriptor definitions. The
later rules in that file and later `rustfmt`, `clippy` and `unpretty` aspect
forms cannot be selected first. Public query/build still expose only their
generic repository-session wrappers and are not terminal evidence.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is direct architecture guidance
only. Its `build_rule_declaration.AspectDefinition` keeps implementation,
propagation inputs and toolchain requirements in one declaration owner, while
`AspectExportIdentity` keeps producer-module identity distinct from an
importing alias. Its evaluated-package publication then exposes narrow
projections from the retained declaration. Slug follows those ownership
lessons without copying Zabel code, representation, runtime, scheduler or
behavior; exact claims remain grounded in pinned Bazel 9.2.

The Buck2 utility-reuse audit selects no import or Stage 9 ledger update. Use
Slug's existing `Arc`, `CompactString`, `CanonicalLabel`, frozen Starlark
value and module-lifetime owners. Add no collection, hash domain, interner,
side registry or clone-sensitive cache.

## Decision and non-decisions

Add `aspect` only to complete `.bzl` loading globals. Accept the exact fixed
subset needed by the first live declaration:

- callable `implementation`, positional or named as Bazel permits;
- omitted or fixed list-of-string `attr_aspects`;
- omitted `toolchains` or the live one-element list-of-string requirement,
  resolved canonically in the defining `.bzl` context; and
- omitted, string or `None` `doc`, validated but not retained.

All accepted non-implementation arguments remain named-only. The fixed
signature rejects every unadmitted Bazel aspect parameter. BUILD must not
resolve `aspect`, including through a function imported from `.bzl`.

Create one evaluator-local aspect definition and one frozen aspect definition.
Retain the implementation lifetime, ordered propagation attribute names, the
single canonical toolchain requirement, defining module label and optional
first top-level exported name. `export_as` binds the name once. An unexported
definition may freeze without an export identity, but no later consumer may
apply it. A recursive import must preserve the producer identity and semantic
fields rather than rebinding them to the importer.

Do not add aspect membership to `attr.label`/`attr.label_list`, rule aspect
parameters, command-line aspect selection, propagation, required providers,
required aspects, attributes, fragments, toolchain-aspect propagation,
configured analysis, action ownership, query/aquery presentation or aspect
implementation execution. Do not accept later rules_rust aspect call shapes.
Do not change Boolean/StringList target rejection or string-setting analysis.

## Ownership, revision and lifetime

The `aspect` call in the defining `.bzl` evaluation is the producer. Its
evaluation value owns the first export cell; the frozen definition in the
existing `BzlLoadValue` module is the sole retained semantic owner. Imported
modules borrow the frozen value through existing `FrozenBzlLifetimeEntry`
ownership. There is no command-side repair, path inference or side registry.

Existing observed source dependencies and recursive Bzl DICE keys invalidate
definition edits before freeze. No new DICE key, projection, request overlay,
revision certificate, filesystem observation or overlapping-request behavior
is added. Because application is deferred, there is no configured-aspect
equality key yet; future consumers must project every admitted retained field
from this owner and fail closed on unmodeled inputs.

The transient definition and export cell are evaluator scratch. The frozen
implementation and compact arrays are DICE-retained semantic state owned and
released with the frozen Bzl module. No command-retained state, service cache,
async transfer, cancellation hook, task, eviction or shutdown duty is added.

## Files and caps

Allowed files, with base SHA-256 and final line ceiling:

| File | Base SHA-256 | Cap |
|---|---|---:|
| `app/slug_loading_v2/src/package.rs` | `3e28fa6634c2958720a1750bcaaf858681285ed7214cd60d49019c7550980447` | 5,401 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `6fbbd2b8876f2c57056e115f7901eec2e5cc02dfaa345186f4de785578eae1d8` | 4,248 |

Production additions are <=160, proof additions <=120 and total additions
<=280. Both files exceed the authoring-guide size trigger. `package.rs`
nevertheless remains cohesive as the sole owner of loading globals and their
evaluation/frozen callable values; extracting one aspect type would split its
context, global registration and freeze contract without a second consumer.
The test file already owns the recursive external-Bzl DICE harness needed to
prove producer identity and import lifetime. No touched function may grow
past 150 lines. `REPLAN` before adding a third file or breaching a cap.

## Proof and validation

Extend focused proof that:

- an exact `rust_analyzer_aspect`-shaped declaration loads, exports and freezes
  with its six ordered attribute names, canonical toolchain label, defining
  module and exported name;
- a recursive importing module observes the same producer identity and fields;
- positional and named callable implementation plus omitted defaults work,
  while a noncallable implementation, malformed fixed lists, non-string doc
  and every unsupported parameter fail closed; an unexported nested result
  freezes without falsely acquiring producer export identity;
- `aspect` is absent from BUILD, including an imported factory call; and
- accepted String/Boolean/StringList descriptor definitions and their target
  rejection boundaries remain unchanged.

Run serially:

- `cargo fmt --check` and `git diff --check`;
- focused aspect-definition loading tests;
- full `cargo test -p slug_loading_v2`;
- `cargo check -p slug_core_v2 --locked`;
- `cargo build -p slug_cli_v2 --locked` before any `SLUG_V2_BIN` smoke;
- `scripts/v2_archive_status.sh`, preserving only its known three-path
  thoughts classification if unchanged; and
- with clean `slugd` lifecycle and fresh output roots, the existing disposable
  rules_rust query/build, recording the next internal source-order stop
  separately from unchanged public wrappers.

Pinned source/tests and the archive source shape already discriminate this
definition contract. No new oracle fixture, copied source, network mutation
or Bazel execution is authorized. Upstream application, propagation,
configured-analysis and action tests are skipped because those phases remain
unsupported; their definition/export portions are adapted into focused local
proof.

## Compatibility and STOP

- **Exact:** `.bzl` placement, callable implementation ABI, fixed ordered
  string `attr_aspects`, the live single string toolchain requirement, string/`None`
  doc validation, first top-level export identity and recursive frozen import
  for the admitted live call subset.
- **Slug-native:** Rust frozen representation, compact storage,
  valid-Unicode strings, canonical-label representation and nonrequired
  diagnostics.
- **Unsupported/deferred:** every other `aspect` parameter or dynamic
  propagation function, dependency-attribute aspect attachment, aspect
  application/selection/propagation/analysis/actions, later rules_rust call
  shapes, Boolean/StringList targets and analysis/CLI, M8/M7B and exact output
  bytes.

STOP on dirty overlap, edits outside the two-file allowlist, BUILD visibility,
an evaluator-local marker without frozen producer identity, rebinding an
imported alias, dropping or reordering semantic fields, aspect application or
execution, a side registry, behavior sourced from Zabel, source vendoring,
Java/JVM, dependency drift, fixture growth, public-success claims or any cap
breach. `REPLAN` before crossing a boundary.
