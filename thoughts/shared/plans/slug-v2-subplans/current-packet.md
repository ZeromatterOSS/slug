# Current Slug V2 Packet

Packet: `WP-5-m1-external-starlark-test-base-query-closure-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: design worker
Evidence: accepted dependency-free external Starlark-rule projection, prior
Bazel 9.2 external test-rule closure probe, and pinned Bazel test-base rule
attribute sources recorded in the owner plan.

Design only the smallest exact DICE-owned Rust query representation for the
Bazel 9 implicit dependency closure of one explicitly public external
`test = True` Starlark rule. Audit the source-pinned/verbatim `@bazel_tools`
test attributes and every required platforms, rules_java, rules_shell, and
coverage repository route. Decide whether the observed closure is a finite
source-proven unconfigured graph under existing route/package owners or whether
any edge requires general repository discovery or configuration and forces
`REPLAN`.

Do not edit Rust, fixtures, tools, plans beyond the reviewed design result, or
generated evidence, and do not run Bazel unless a later reviewed evidence
packet explicitly authorizes it. Do not activate the test rule, test metadata,
suite membership, analysis, actions, or execution. Port `@bazel_tools` content
verbatim only; never synthesize it.

Audit exact graph identity, ownership, source routes, equality/lifecycle,
consumer breadth, implementation feasibility, evidence sufficiency, bounded
allowlist/cap, and stop gates. Obtain independent reserved-boundary review
before scheduling any implementation. Stop with **REPLAN** if the closure is
not finite and source-proven or crosses general discovery, configuration,
toolchain resolution, analysis/actions/execution, JVM, Java bytecode, or Bazel
delegation.
