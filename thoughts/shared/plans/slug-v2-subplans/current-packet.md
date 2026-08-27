# Current Slug V2 Packet

Packet: `WP-4-7A-rule-fragments-family-and-rules-rust-toolchain-complete-loading`

Milestone: M7A command/ruleset bootstrap closure.

Result: admit the complete shared rule/aspect target-fragment declaration
category, then freeze authenticated complete rules_rust
`rust/private/toolchain.bzl` over its ten complete real children without
invocation.

## Learned facts and decision

Commit `1070b0cf5` freezes all 181 authenticated Bazel Skylib
`rules/common_settings.bzl` lines. Every direct child of rules_rust
`rust/private/toolchain.bzl` is now complete. The 1,002-line parent has SHA-256
`c4b613cee96540a94fbdf4fbdca7b8dc4ef6d3082024c4d3636afc2e9c4d468e`;
its first remaining eager gap is `rust_toolchain = rule(fragments = ["cpp"],
...)`.

Bazel 9.2 `StarlarkRuleFunctionsApi` admits a sequence of strings for rule and
aspect `fragments`. `StarlarkRuleClassFunctions` casts the complete sequence,
and `ConfigurationFragmentPolicy.Builder` stores names in `LinkedHashSet`,
ignoring duplicates while retaining first-seen order. Use one parser and one
immutable retained representation for both declaration kinds. Prove absent,
empty list/tuple, multiple names, duplicates and wrong container/element
types. Keep declared names distinct from active fragments and typed fragment
producers.

Run only this packet. After the shared category is admitted, freeze the exact
parent at its canonical owner over all ten authenticated child modules. Prove
the fourteen imported pointer identities, `_DIGITS`, ten private functions,
`rust_stdlib_filegroup`, `rust_toolchain`, exact rule declarations and
sixteen-public/twenty-seven-all inventories. Invoke nothing.

## Generic architecture, authorities and compatibility

This is generic BCR Starlark declaration loading. `cc_common` and the
rules_rust toolchain are integration cases, not Rust implementations of C++ or
Rust rules. Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
and authenticated rules_rust bytes are sole exact authority.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
concept/test guidance only. Its separation of declared names, active fragments
and typed producers informs the ownership boundary; no Zig code,
representation, algorithm, cache, configured behavior or diagnostic is copied,
and Zabel is not compatibility authority.

- **Exact:** rule/aspect target-fragment accepted shapes, type rejection and
  first-seen duplicate normalization; complete parent source/hash;
  owner/path/mappings; ten real child loads and fourteen pointer identities;
  eager declarations and exact inventories; complete freeze without
  invocation.
- **Slug-native:** starlark-rust parse/evaluate/freeze; `CompactString`,
  `SmallSet` and immutable `Arc` ownership; structural DICE equality.
- **Unsupported/deferred:** host fragments, rule inheritance/subrules,
  configured fragment availability and `ctx.fragments`, typed fragment
  producers, every parent function/rule invocation and configured toolchain
  semantics.

The natural producers are transient/frozen rule and aspect definitions. Frozen
rules publish the immutable fragment slice through `StarlarkRuleImplementation`;
it participates in loaded-target equality and invalidation. Frozen aspects keep
the same representation. Nonempty fragments fail closed at target invocation
until configured fragment semantics are admitted. Values never borrow the
evaluator. This is DICE-retained semantic memory; existing `Arc`,
`CompactString`, `SmallSet`, `Dupe` and `Allocative` patterns are reused,
with no cache or async lifetime and no new Buck2-derived utility.

## Allowlist, caps and proof

Change only `app/slug_loading_v2/src/package.rs` and
`app/slug_loading_v2/src/host_package_load_tests.rs`. Scheduling documents may
change only at selection and terminal acceptance.

At base `1070b0cf5`, production is 6,275 lines, SHA-256 `81586c94…`; test
authority is 33,330 lines, SHA-256 `9f0c94dc…`, with a final ceiling of 35,030
lines. Each new proof/helper function remains at most 120 physical lines.
Caps are 120 production, 1,700 proof and 1,820 total additions; deletions do
not buy budget. Embed/hash all 1,002 parent lines. Evaluate at
`@@rules_rust+//rust/private:toolchain.bzl`, path
`/rules_rust/rust/private/toolchain.bzl`, with authenticated mappings and ten
complete children. Prove every eager declaration and inventory; invoke
nothing.

Run focused loading tests, all `slug_loading_v2` library tests, BUILD loading,
Bzl invalidation, locked analysis/core compile checks and locked CLI build.
Run formatting, diff, caps/function-size, source hashes and archive hygiene,
then root review for authority, category completeness, retained ownership,
equality/invalidation, fail-closed invocation, recursive-source completeness,
no-invocation scope, generic architecture and Zabel's peer-guidance role.

STOP and `REPLAN` for another unadmitted eager shape, source/hash mismatch,
configured fragment behavior, borrowed evaluator value, incomplete category or
parent, unpinned source, copied Zabel content, new utility, dirty authority,
allowlist escape, cap/function violation or failing baseline. Stop after the
complete parent and re-audit the source-ordered bootstrap frontier.

## Immediate predecessor

Commit `1070b0cf5` accepts only complete Bazel Skylib common-settings
declaration loading without invocation.
