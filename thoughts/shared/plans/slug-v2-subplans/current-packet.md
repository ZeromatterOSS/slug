# Current Slug V2 Packet

Packet: `WP-4-7A-rust-analyzer-toolchain-rule-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Type: docs-only compatibility and ownership audit
Base: `84ddb6a3`

Result: audit the first source-order declaration after the accepted fixed
aspect and bounded Label vertical:
`rust/private/rust_analyzer.bzl:359-402`. Select the smallest exact loading
subset for `rust_analyzer_toolchain = rule(...)` or record `REPLAN`. Do not
edit Rust or implement behavior in this packet.

## Authority and evidence

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority. Authenticate the relevant `attr.label`,
`attr.string`, `rule`, attribute-descriptor and rule-class/export sources plus
focused tests. Record named-only arguments, defaults, validation order,
definition-relative label semantics, freeze/export identity and whether docs
are retained or discarded.

The accepted rules_rust 0.73.0 archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
The declaration has four label attributes using combinations of `doc`,
`cfg = "exec"`, `executable = True`, `allow_single_file = True` and
`mandatory = True`, then two string attributes with `doc` and defaults. Audit
the complete declaration before selecting a slice; do not silently omit a
field that changes dependency or analysis semantics.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Inspect its retained ordinary-dependency attribute schema, executable and
single-file policy, declaration-owner relation, and executable-module
identity split. Reuse only the single-owner/thin-projection lesson. Do not
copy Zig code, layouts, parser, evaluator, storage or behavior.

## Required audit

1. Trace the live Slug call binding and retained/frozen schema for every field
   in lines 359-402. Identify the first unsupported argument and every
   downstream semantic field needed to retain the declaration truthfully.
2. Establish exact Bazel 9.2 behavior for `doc`, string `cfg = "exec"`,
   executable labels, single-file labels, mandatory labels and string
   defaults, including invalid combinations and export identity.
3. Decide whether the complete fixed declaration fits one bounded loading
   packet. Name exact production/test allowlists, base hashes, line/addition
   caps, proof matrix, validation commands and STOP conditions. `REPLAN`
   before widening into target invocation, dependency resolution or analysis.
4. Classify the selected surface as exact, Slug-native or
   unsupported/deferred. Bazel 9.2 behavior is exact; Rust representation and
   nonrequired diagnostics may be Slug-native.

## Non-decisions and STOP

Do not implement, run Bazel, add fixtures, edit dependencies, invoke the rule,
resolve executable prerequisites, perform configured analysis, add actions,
advance to later `current_rust_analyzer_toolchain`/sysroot declarations, widen
Label mapping/APIs, apply aspects, or claim public rules_rust success. Stop on
an unbounded schema/analysis coupling, missing pinned authority, dirty overlap,
Zabel behavior adoption, Java/JVM work or any need for a source change.

Validation is docs-only: verify the accepted archive/source lines and hashes,
Slug/Zabel/Bazel source anchors, exact docs allowlist, `git diff --check` and
`scripts/v2_archive_status.sh`. Independent terminal review must return
`ACCEPT` before commit.
