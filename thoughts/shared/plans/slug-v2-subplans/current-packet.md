# Current Slug V2 Packet

Packet: `WP-4-7A-complete-json-builtin-and-rules-rust-toolchain-loading`

Milestone: M7A command/ruleset bootstrap closure.

Result: admit Bazel's complete four-method shared `json` module by reusing and
correcting the adopted starlark-rust implementation, then retry the complete
authenticated rules_rust toolchain parent without invocation.

## Learned facts and decision

Commit `4a2022764` admits the complete shared rule/aspect target-fragment
declaration category. The 1,002-line rules_rust toolchain then stops during
compilation at `json.decode`: Bazel predeclares `json` across BUILD, `.bzl`
and cquery environments, while Slug did not install starlark-rust's existing
`LibraryExtension::Json`.

Starlark-rust already owns `decode`, `encode`, `encode_indent` and
`indent`, but its current ABI and output differ from Bazel 9.2: decode's
default is incorrectly keyword-only, indent exposes `indent_str`, dictionaries
encode in insertion order instead of lexical key order, and formatting
round-trips through serde instead of preserving JSON token spelling. Correct
the reusable library once and install it in both BUILD and `.bzl` globals.
Prove the complete method category, not only the toolchain's decode reference.

Then freeze authenticated `rust/private/toolchain.bzl` over its ten complete
real children. Prove source/hash, fourteen imported pointer identities,
`_DIGITS`, ten private functions, two rules, fragment/toolchain declarations
and exact inventories. Invoke nothing. STOP and replan at any next eager gap.

## Architecture, compatibility and guidance

Bazel 9.2 `Json.java`, `json.star`,
`StarlarkRuleClassFunctionsTest` and authenticated rules_rust bytes are exact
authority. Reuse the adopted starlark-rust JSON module rather than creating a
package-specific host. Zabel is concept/test guidance only: its shared,
context-free, pure JSON predeclared reinforces that JSON owns no package,
mapping, filesystem or DICE authority; no Zig code or diagnostics are copied.

- **Exact:** shared BUILD/`.bzl` availability; four-method inventory and
  argument ABI; supported value encoding, lexical object-key ordering, compact
  encoding, default behavior, valid decoding and token-preserving indentation;
  complete parent source graph and declarations without invocation.
- **Slug-native:** Rust valid-Unicode and serde-backed numeric/parse internals,
  with diagnostics not claimed exact where they differ from Bazel.
- **Unsupported/deferred:** invalid UTF-16 edge parity, exact parse diagnostics,
  configured toolchain/function/rule behavior and every filesystem/action read.

JSON values are evaluator scratch and frozen module globals are shared
predeclareds; JSON has no retained semantic side store, host capability or
fallback. No DICE key, request, revision, cache or async ownership changes.

## Allowlist, caps and validation

Change only `starlark-rust/starlark/src/stdlib/json.rs`,
`app/slug_loading_v2/src/package.rs` and
`app/slug_loading_v2/src/host_package_load_tests.rs`. At base `4a2022764`,
production is 6,310 lines and test authority is 33,437 lines, with a final test
ceiling of 34,837. Caps are 250 production, 1,400 proof and 1,650 total
additions; deletions do not buy budget. Each new helper/proof function remains
at most 120 lines. Embed/hash all 1,002 parent lines.

Run starlark-rust JSON tests, focused parent proof, all loading-library tests,
BUILD loading, Bzl invalidation, locked analysis/core checks and locked CLI
build. Run formatting, diff, caps/function-size and archive hygiene, then root
review for Bazel authority, full method/ABI coverage, shared pure ownership,
parent completeness, no invocation and Zabel's peer-guidance role.

STOP and `REPLAN` for another eager global/shape, unbounded JSON rewrite,
new host/I/O authority, source/hash mismatch, copied Zabel content, incomplete
method category or parent, exact invalid-diagnostic claim, allowlist/cap escape
or failing baseline.

## Immediate predecessor

Commit `4a2022764` accepts shared fragment declarations; the parent attempt
stopped without retaining its source proof when the missing JSON global was
identified.
