# Current Slug V2 Packet

Packet: `WP-6-m2-root-configured-target-command-boundary-design`
Milestone: M2 analysis graph with the first M4 cquery consumer
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: read-only reserved cross-stage command/identity design
Evidence: accepted recursive `ConfiguredTargetAnalysisKey` implementation in
`4f4599e0`; generated Bazel 9.2 `recursive-custom-rule-providers-actions`
cquery/aquery evidence in `9e6a4450`; accepted command driver/event ownership.

Do not edit Rust, tests, fixtures, oracle records, or harness code. Audit the
live `cquery` parser/placeholder, configured-target key/result, runtime command
driver, CLI/server command surfaces, and existing oracle rows. Obtain reserved
Sol review before authorizing any implementation.

The candidate observable slice is one root-repository
`TargetPattern::Single` naming an already-supported Starlark rule under the
default target configuration, with default or explicit `--output=label`.
Decide whether a retained cquery command root can compute exactly the existing
`ConfiguredTargetAnalysisKey { workspace, configured_target }`, project the
requested configured label, and reuse accepted command Need/error/event
publication without a second analysis graph or evaluator call.

The design must enumerate:

- request and configured-label identity, canonical order, equality, validity,
  and default configuration naming;
- root MODULE/package/analysis Need and failure precedence;
- whether only the requested configured target or its ordered dependency
  results are part of the command value and output;
- exact Bazel 9.2 default/explicit label text, stdout/stderr, exit status, and
  one-shot/daemon JSON or text boundary;
- cold, warm, provider edit, unrelated edit, declaration deletion/recreation,
  and event/retry behavior; and
- the exact implementation allowlist, downstream tests, platform gate, and
  formatted net caps.

Reuse `recursive-custom-rule-providers-actions` evidence if its existing cquery
rows discriminate the literal-label result. Add no oracle merely for command
wiring. If exact default configuration/output or error behavior is absent, the
only successor may be one isolated evidence packet before Rust.

Stops: no implementation; no new configuration representation, transition,
select, toolchain/platform, repository mapping, external target, pattern
breadth, query-function/formatter breadth beyond label, provider rendering,
action/aquery surface, execution, REAPI, new DICE key, second command-owned
analysis graph, direct evaluator call, filesystem discovery, CLI/server
production edit, fixture/oracle growth without a demonstrated gap, or M1
external build expansion.
