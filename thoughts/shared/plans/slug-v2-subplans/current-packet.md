# Current Slug V2 Packet

Packet: `WP-8-m5-filewrite-aquery-zero-toolchain-platform-design`
Milestone: M5 expansion prerequisite
Owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: freeze a bounded execution-platform strategy for actions owned by
zero-toolchain configured targets or record `REPLAN`.

## Design question

Determine how a configured action owned by a rule with no toolchain requirement
receives a structural execution platform. The immediate discriminator is an
action-bearing selected toolchain implementation reached by
`aquery deps(//:root)`: Bazel 9.2 emits its action with the selected execution
platform, while Slug retains the action but currently rejects its resolved
FileWrite view because no selected toolchain platform is attached.

Inspect action registration, configured action views, toolchain preparation and
candidate-platform topology, configuration identity/invalidation, the retained
build action closure, and FileWrite semantic identity. Decide whether the
already retained candidate execution-platform facts can select one exact
default platform without adding a toolchain requirement, recursive toolchain
selection, action reconstruction, or new DICE state. Keep toolchain selection
identity distinct from action execution-platform identity.

## Read-only scope

Compare pinned Bazel 9.2 action execution-platform assignment for zero-required
toolchain rules with Slug's Rust-native topology. Cover an ordinary zero-
toolchain rule, an action-bearing selected toolchain implementation, target and
transitioned configurations, multiple registered/candidate platforms,
constraints, incompatibility/no-platform errors, and A/B/A platform edits.

Classify platform selection and identity as exact, Slug-native, or deferred.
Select at most one bounded implementation/evidence successor with explicit
allowlist, caps, integrity failures, and direct-literal regression protection.
If the retained facts cannot support the choice without wider platform
resolution or new semantic state, record `REPLAN`.

## Validation and stops

This packet is design-only. Run source/structure/diff checks and require
independent design review. Cap bookkeeping at 180 lines. Add no Rust, tests,
fixture/expected evidence, Bazel execution, aquery expression activation,
command/wire field, toolchain recursion, action reconstruction/execution,
contents, new DICE key/state, retained identity representation, exact Bazel
identity bytes, JVM/Java artifact, REAPI reuse, or CI. One material correction
maximum; a second is `REPLAN`.
