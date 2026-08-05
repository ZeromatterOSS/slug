# Current Slug V2 Packet

Packet: `WP-6-m2-root-cquery-label-output-evidence`
Milestone: M2 analysis graph with the first M4 cquery consumer
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: evidence-only retained Bazel 9.2 literal cquery discriminator
Evidence: accepted recursive configured-analysis oracle/implementation;
Terra ownership/evidence audits and reserved Sol review of the missing output
contract.

Do not edit Rust, tests, fixtures, generated oracle records, or harness code.
Copy the existing `recursive-custom-rule-providers-actions` workspace into an
isolated temporary root and use one retained Bazel 9.2 output base. Run these
commands serially:

1. `cquery //parent:parent`
2. `cquery //parent:parent --output=label`
3. `cquery //parent:missing`
4. `cquery //parent:parent --output=label`

Use `/usr/bin/bazel` only after confirming 9.2.0, with ordinary RC discovery;
never inspect or copy `~/.bazelrc`. Capture each exact exit code, raw stdout,
raw relevant stderr, and command order. Explicitly record whether default and
label stdout are byte-identical, whether either includes a configuration hash
or mnemonic, the stable missing-target diagnostic shape, and successful
recovery in the same server. Do not normalize configuration identifiers in raw
evidence. No analysis-error probe is authorized.

Shut down the retained Bazel server and remove the temporary workspace/output
base. Compare successful analysis with the frozen Starlark cquery rows but do
not regenerate or edit them.

At `ACCEPT`, record exact evidence/provenance in the Stage 6 owner plan and
resume `WP-6-m2-root-configured-target-command-boundary-redesign`. That design
must retain the accepted no-new-key route: drive
`RootConfiguredTargetAnalysisKey` directly through `NativeCommandRoot`, with no
second analysis graph or evaluator call. At contradiction, record `REPLAN`.

Stops: no checkout asset or production change; no fixture/oracle growth;
no analysis-error, external label, pattern, transition, toolchain, provider,
aquery, action, execution, REAPI, or cycle probe; no credentials; no parallel
Bazel command or second output base.
