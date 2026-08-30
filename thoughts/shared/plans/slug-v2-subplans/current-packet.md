# Current Slug V2 Packet

Packet: `WP-4-5-7A-symbolic-macro-namespace-analysis-enforcement`

Milestone: M7A bootstrap-critical generic Starlark/ruleset closure.

Base: terminally accepted symbolic-macro/provider implementation `e34cfdc7a`,
accepted category architecture `368ef9296`, and the live configured-analysis
owner. All unrelated dirty analysis, core, loading, and REAPI work is parked
and read-only.

## Observable result

Enforce the already-retained symbolic-macro namespace violation only when the
named target enters configured analysis. Package loading and ordinary target
enumeration continue to succeed; a compliant sibling configures normally; an
illegally named target fails before rule implementation, dependency, provider,
or action publication with the Bazel 9.2 diagnostic shape.

This closes the one explicitly scheduled analysis hunk from `368ef9296` and
`e34cfdc7a`. The next category packet is `subrule` declaration architecture
and implementation; it remains inactive here.

## Authority and compatibility

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is sole semantic
authority. The direct source anchors are
`ConfiguredTargetFactory.java:257-267`, `Package.java:319-331`, and
`TargetRecorder.java:47-50`; `SymbolicMacroTest` method
`assertPackageLoadsButGetConfiguredTargetFailsMacroNamingCheck` plus the
`macroNameRules_*` test family supplies the public behavior.

**Exact:** loading succeeds and retains the target; configured analysis rejects
only a target whose retained origin names a namespace violation; compliant
`name`, `name_*`, `name-*`, and `name.*` targets remain admitted; the error
identifies the canonical target and declaring macro and includes Bazel's naming
rule text.

**Slug-native:** the existing `AnalysisError::Message` carrier, Rust/DICE error
publication, and structural semantic identity.

**Unsupported/deferred:** `--allow_analysis_failures` conversion to
`AnalysisFailureInfo`, finalizer macros, lazy package pieces, subrules,
configured aspects, provider instances, parser/set work, and C++-specific
semantics. The upstream analysis-failure test is skipped because that provider
surface is not admitted; its ordinary failure path is covered here.

## Ownership and implementation contract

`LoadedPackage::macro_target_origins` already owns the sparse retained fact and
participates in package equality/invalidation. `RootAnalysisDriver::compute`
is the natural consumer: after its existing DICE package observation and exact
target lookup, inspect `LoadedPackage::macro_origin(target)` before the current
configured-shape gate and before any semantic result is published.

Add no loading field, DICE key, cache, side map, command repair, evaluator
state, async task, or fallback. No new retained memory exists. Request,
revision, cancellation, and overlapping-session behavior remain those of the
existing package-input and root-analysis keys; an A/B/A package edit restores
through their current structural equality boundary.

The diagnostic is:

`Target <label> declared in symbolic macro '<macro>' violates macro naming rules and cannot be built. Name must be the same as the macro's name, or the macro's name followed by '_' (recommended), '-', or '.', and a non-empty string.`

Do not re-run namespace string classification in analysis. Consume only the
retained loading fact, so defining-package identity and nested macro ownership
cannot drift between phases.

## Frozen proof matrix

- package loading retains both compliant and violating targets without error;
- configured analysis admits a compliant sibling and rejects the violating
  target with the exact message shape before rule implementation or actions;
- direct BUILD targets and targets with no macro origin remain unchanged;
- the same DICE graph observes violating A, compliant B, then restored A and
  reproduces rejection/success/rejection without stale publication; and
- loading, analysis, cquery/build direct dependents, formatting, diff, archive,
  and staged-hunk isolation gates pass.

Reuse the accepted local temp-workspace scaffolding; add no oracle fixture or
network input. Bazel's Java-only package-piece and analysis-failure-provider
tests are not copied because they assert unadmitted implementation/provider
surfaces; the public configured-target observation above is stronger for this
packet.

## File allowlist, caps, and dirty isolation

Only these files may change:

- `app/slug_analysis_v2/src/dice.rs`, only the post-package target-admission
  helper/call site;
- `app/slug_analysis_v2/tests/configured_target.rs`, only focused configured
  namespace and A/B/A proof;
- this manifest, the canonical plan, and Stage 4 plan for scheduling/acceptance.

Caps are 40 production, 120 proof, and 160 aggregate added/deleted lines.
`dice.rs` is large, but this fact belongs at its existing target-admission
boundary; splitting it would create a second owner for one bounded check.

At activation, unrelated worktree baselines are read-only:

- `dice.rs`: HEAD blob `e31aae7f06d6de497ee7a7bd9e1968d6548be540`,
  worktree blob `cf684bdc152bd9c5833130c52190f3466595b338`;
- `configured_target.rs`: HEAD blob
  `675ba67e2f114f310aedb176ebdc91cfc1bd471a`, worktree blob
  `d0449a945d9647975f8141d5f6beec59518f7758`.

Re-audit both hashes before editing and stage only packet-owned hunks. Stop and
`REPLAN` if either baseline moves, the natural call site is materially changed,
or clean hunk isolation is impossible.

## Validation and stop conditions

Run focused namespace tests, full `slug_analysis_v2` tests, the loading suite
only if loading behavior changes (it must not), affected cquery/build tests,
`cargo fmt --all -- --check`, `cargo build -p slug_cli_v2`,
`scripts/v2_archive_status.sh`, `git diff --check`, cap accounting, and exact
staged-hunk audit. Multiple fresh BCR replays are unnecessary: this packet has
no repository/materialization or non-hermetic behavior.

Stop and `REPLAN` for a new semantic owner/key, loading change, analysis-failure
provider support, rule-implementation execution before the check, an edit
outside the allowlist, cap overflow, or overlap with the parked dirty hunks.

## Immediate predecessor

Commit `e34cfdc7a` implements non-finalizer symbolic macros and the
nonconstructible `PackageSpecificationInfo` key. Full loading and analysis-lib
validation passed; two fresh authenticated rules_rust 0.73.0 replays cleared
`macro` and both stopped at missing `subrule`. Independent terminal rereview
returned `ACCEPT`; the exact 28-line parked `package.rs` definition-source hunk
remained outside the commit.

Clean Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a` remains concept and
optimization guidance only. It supplies no behavior, diagnostics, or parity
authority for this packet.
