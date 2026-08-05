# Current Slug V2 Packet

Packet: `WP-6-m2-root-cquery-starlark-label-evidence`
Milestone: M2 analysis graph with the first configuration-opaque M4 consumer
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: evidence-only retained Bazel 9.2 Starlark-label discriminator
Evidence: accepted recursive configured analysis and Bazel 9.2 fixture;
accepted configuration-identity REPLAN; reserved review selecting the
configuration-opaque Starlark expression boundary.

Do not edit Rust, tests, fixtures, generated oracle records, or harness code.
Copy the existing `recursive-custom-rule-providers-actions` workspace into an
isolated temporary root and use one retained Bazel 9.2 output base. After
confirming `/usr/bin/bazel` is exactly 9.2.0, run these commands serially:

1. `cquery //parent:parent --output=starlark --starlark:expr=str(target.label)`
2. `cquery //parent:missing --output=starlark --starlark:expr=str(target.label)`
3. `cquery //parent:parent --output=starlark --starlark:expr=str(target.label)`

Use ordinary RC discovery, but never inspect or copy `~/.bazelrc`. Capture each
exact exit code, raw stdout, relevant raw stderr, and command order. Record the
exact canonical-label bytes, prove that no configuration checksum/mnemonic is
exposed, pin the missing-target diagnostic under this formatter, and prove
successful same-server recovery. Compare the first and third stdout
byte-for-byte and record warm loaded/configured counts.

Shut down the retained Bazel server and remove the temporary workspace/output
base. Compare the successful configured label with the accepted recursive
fixture's Starlark-file output, but do not regenerate or edit that fixture. The
old standalone `cquery-provider-starlark` Bazel 9.1.1 record is orientation
only, not acceptance authority.

At `ACCEPT`, record exact evidence/provenance in the Stage 6 owner plan and
resume `WP-6-m2-root-cquery-starlark-label-boundary-design`. That design must
drive `RootConfiguredTargetAnalysisKey` directly through `NativeCommandRoot`,
with no second graph/key/evaluator call. It may support only one root literal,
explicit `--output=starlark`, and the exact
`--starlark:expr=str(target.label)` expression. Default/explicit `label`,
arbitrary expressions/files, patterns, and general configuration stay
unsupported.

Stops: no checkout asset or production change; no fixture/oracle growth; no
default/label output, alternate expression, Starlark file, external label,
pattern, transition, toolchain, provider, aquery, action, execution, REAPI, or
cycle probe; no credentials; no parallel Bazel command or second output base.
