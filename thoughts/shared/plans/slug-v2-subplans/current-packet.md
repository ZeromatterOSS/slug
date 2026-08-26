# Current Slug V2 Packet

Packet: `WP-4-7A-post-run-environment-info-clippy-tail-audit`

Milestone: M7A command/ruleset bootstrap closure.

Result: replay the now-complete lint-test child return into rules_rust
`rust/private/clippy.bzl`, authenticate every remaining evaluated declaration
through line 596, identify the first unsupported expression or prove the tail
freezes, and select one bounded implementation or `REPLAN`. This is docs-only.

## Accepted starting point and audit horizon

Base is `45b479e56` (`Load RunEnvironmentInfo declaration global`). Exact
rules_rust 0.73.0 `rust/private/lint_test.bzl` now compiles and freezes through
line 159 with all four `clippy.bzl:19-25` imports pointer-identical to their
defining child exports. Neither helper nor any native-provider constructor
executes.

The authenticated source hashes remain:

- `lint_test.bzl`:
  `4f4fade9218980db0296f99e5d199059c91ebebc7b9745bee18ad58c37b551c8`;
- `clippy.bzl`:
  `a778d2ddc77587ffbffc72efcdaa458a1ffae0763e500da1c876b9b567b2a686`.

After the child returns, audit `clippy.bzl:463-596` strictly in source order:

- `RustClippyTestInfo`, a documented two-field provider;
- `_CLIPPY_OUTPUT_GROUPS`, a two-string immutable list;
- two lazy helpers using the four authenticated child exports;
- `_rust_clippy_test_aspect`, requiring the accepted clippy aspect and
  advertising `RustClippyTestInfo`;
- `rust_clippy_test`, merging the child-owned common attrs with one
  provider/aspect/transition-bearing `targets` label list;
- `capture_clippy_output` and `clippy_output_diagnostics`, each using a Boolean
  build-setting descriptor and an imported provider in its lazy helper.

Stop at the first unsupported evaluated expression. Similarity to the accepted
rustfmt/config/provider shapes is evidence to inspect, not proof of closure.

## Authorities and required audit

Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` remains sole behavior authority.
Read the locally available object explicitly with `git show` without changing
the clean `../bazel` checkout. Reuse already-pinned provider, aspect, rule,
attribute, transition, dictionary and build-setting evidence only after
showing each live expression uses the same contract and producer identity.

Inspect live Slug owners and accepted proofs for:

- documented provider schema and defining-module provider identity;
- immutable top-level list freezing;
- aspect `requires`/`provides`, export identity and complete required-aspect
  closure;
- exact `dict(base, **overlay)` ordering and child-owned base values;
- label-list provider alternatives, attached aspect and custom transition;
- test-rule capability and both `config.bool(flag = True)` definitions;
- every imported provider/helper identity used by lazy function bodies;
- fail-closed BUILD target invocation before configured metadata is dropped.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architecture guidance only.
Consult its single declaration-owned provider/attribute/aspect/rule definitions
and detached consumer projections only to decide reuse and phase boundaries.
Copy no Zig code, representation, provider value, configured capture,
transition, action, diagnostic or behavior. Bazel 9.2 decides compatibility.

## Compatibility questions

Classify any selected closure explicitly:

- **Exact candidate:** source-order loading/freeze of declarations whose Bazel
  contract and producer/import identities are fully authenticated.
- **Slug-native candidate:** existing Rust frozen values, Arc ownership and
  admitted fail-closed invocation boundaries.
- **Unsupported/deferred:** helper/rule/aspect execution; constructed
  `OutputGroupInfo` or `RunEnvironmentInfo`; configured provider matching,
  aspect application, transition, test runner/actions/runfiles; build-setting
  configured values; and any expression after the proven stop.

## Allowlist and deliverable

Only these documentation files may change:

- `.codex/skills/slug-agent-orchestration/references/routing-log.md`;
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`.

The deliverable must record exact selected-source lines and producer identities,
the first unsupported evaluated expression or proof of tail closure, Bazel
anchors and existing Slug owners, the precise Zabel guidance used/excluded, a
Buck2 retained-memory review if representation changes are proposed, and one
bounded implementation packet with hashes/caps/proof/serial validation/review
or `REPLAN`.

No Cargo, daemon, oracle or smoke run is required. Run `git diff --check` and
`scripts/v2_archive_status.sh`; only its three known archive-only misses may
remain.

STOP and `REPLAN` for Rust/test edits; dirty authority; helper execution;
constructed native provider values; configured provider/aspect/transition/test
or action semantics; Java/JVM work; copied Zabel content; skipped source order;
an unauthenticated imported module; invented parity; or an unbounded packet.

## Immediate predecessor

`45b479e56` accepted the distinct fixed `.bzl` `RunEnvironmentInfo` token and
exact unabridged lint-test child freeze with full validation and terminal review.
