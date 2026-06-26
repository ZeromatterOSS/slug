# Stage 8: Ruleset and Command Conformance

## Goal

Prove Slug V2 works with modern Bazel 9+ rulesets and user commands after the
core loading, bzlmod, analysis, and REAPI surfaces exist.

## Scope

- rules_cc, rules_rust, rules_python, protobuf, bazel_skylib, and rules_oci
  public smoke fixtures.
- `build`, `test`, `run`, `query`, `cquery`, and `aquery` command slices.
- BEP and event output needed by common integrations.
- diagnostics and exit-code compatibility where rulesets depend on them.

## Non-Goals

- Native language-rule fallbacks removed from Bazel 9.
- Android/iOS breadth before the core public rulesets are stable.
- Private workspace-specific fixtures as the only proof for a behavior.

## Acceptance Criteria

- Each supported ruleset has at least one public fixture pinned to a modern
  Bazel-9-compatible version.
- Command conformance fixtures compare against upstream Bazel through the oracle
  harness.
- Real-world stress projects supplement, but do not replace, repo-owned focused
  fixtures.

## Validation

```bash
slug-v2-oracle run --fixture rules-cc-basic
slug-v2-oracle run --fixture rules-rust-basic
slug-v2-oracle run --fixture rules-python-basic
slug-v2-oracle run --fixture query-basic
```
