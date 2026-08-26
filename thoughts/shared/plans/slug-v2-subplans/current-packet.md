# Current Slug V2 Packet

Packet: `WP-4-7A-output-group-info-global-audit`

Milestone: M7A command/ruleset bootstrap closure.

Result: authenticate the first missing fixed native-provider global reached by
the exact clippy helper, then select one bounded loading implementation or
`REPLAN`. This is a docs-only audit.

## Accepted starting point and disproven candidate

Base is `fc9473b1` (`Load clippy aspect toolchain requirements`). The complete
`rust_clippy_aspect` freezes through rules_rust 0.73.0 `clippy.bzl` line 404.
The next helper begins at line 406; the source SHA-256 is
`a778d2ddc77587ffbffc72efcdaa458a1ffae0763e500da1c876b9b567b2a686`.

The independently accepted proof-only `WP-4-7A-clippy-rule-loading` candidate
was attempted and disproved before acceptance. The exact helper body fails at
compile time with `Variable OutputGroupInfo not found`. Function laziness
prevents invocation but not global name resolution. Its partial test edit was
fully reverted; there is no Rust or proof delta to carry forward.

## Audit authorities and questions

Bazel 9.2 clean commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority. Audit:

- `StarlarkGlobalsImpl.getFixedBzlToplevels`, which installs
  `OutputGroupInfo.STARLARK_CONSTRUCTOR` directly in the fixed `.bzl`
  environment and omits it from fixed BUILD-file globals;
- `OutputGroupInfoApi` and `OutputGroupInfo.OutputGroupInfoProvider`, which
  define one callable native `BuiltinProvider` named `OutputGroupInfo`;
- `BuiltinProvider` identity/key semantics needed to distinguish this fixed
  provider from module/export-owned user providers and other native providers;
- constructor validation and focused tests only far enough to place a strict
  unsupported boundary around named output groups, depsets/artifacts and
  configured values.

Answer whether Slug's existing `AnalysisBuiltinCallable` can truthfully expose
the fixed declaration identity while construction remains unsupported, or
whether a small dedicated native-provider declaration token is required. Do
not collapse native provider identity into `ProviderId`, whose source-label
plus export-name domain is explicitly for user providers.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architecture guidance only.
Inspect its process-stable `BuiltinProviderId.output_group_info`, provider
binding and separation between declaration identity and configured value. Copy
no Zig code, numeric discriminant, enum layout, parser, constructor, configured
capture, diagnostic or behavior. Bazel 9.2 decides compatibility.

## Compatibility target for a selected implementation

- **Exact candidate:** `.bzl` name availability, omission from BUILD globals,
  callable native-provider declaration identity, distinctness from user
  providers and other fixed providers, and lazy capture by the clippy helper.
- **Slug-native candidate:** an evaluator-free Rust representation for the
  fixed declaration token, with explicit memory accounting if retained.
- **Unsupported/deferred:** constructing nonempty or empty OutputGroupInfo
  values unless separately proved necessary; named-group validation;
  depset/artifact conversion; fields, indexing, iteration and membership;
  returning the provider from a rule/aspect; configured target/aspect lookup,
  attachment, merge and output selection.

The audit must narrow these candidates rather than assume them. In particular,
callability in Bazel does not authorize configured construction in Slug.

## Allowlist and deliverable

Only these documentation files may change:

- `.codex/skills/slug-agent-orchestration/references/routing-log.md`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

The deliverable must record:

1. exact Bazel source/test anchors for placement, native identity and callable
   behavior;
2. the current Slug global/provider owners and why the chosen owner does not
   conflate user and native provider identity;
3. the precise Zabel guidance used and excluded;
4. Buck2 utility/retained-memory review if a new representation is selected;
5. one bounded implementation packet with file allowlist, base hashes, line
   caps, exact/Slug-native/deferred classifications, discriminating proof,
   serial validation and independent review, or `REPLAN`.

No Cargo, Bazel oracle, daemon or smoke command is required for this docs-only
audit. Run `git diff --check` and `scripts/v2_archive_status.sh`; only its three
known archive-only misses may remain.

STOP and `REPLAN` for any Rust/test edit; dirty authority; Java/JVM work;
configured provider/target/aspect work; output-group construction or artifact
semantics; copied Zabel content; invented Bazel behavior; an unbounded native
provider framework; another source expression; or a non-bounded implementation.

## Immediate predecessor

`fc9473b1` accepted the complete clippy aspect using one shared typed
rule/aspect requirement slice. All local gates and independent review passed.
