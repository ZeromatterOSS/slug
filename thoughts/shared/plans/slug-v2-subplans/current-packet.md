# Current Slug V2 Packet

Packet: `WP-5-builtin-bazel-tools-module-injection-design`
Milestone: cross-stage M7 prerequisite design
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: freeze Bazel 9.2's hidden built-in module graph and contextual
repository mappings before any built-in package consumer dispatch.

## Active design contract

The repository/source owner is accepted, but the reviewed test-tools closure
packet ends `REPLAN`. Pinned Bazel shows that `bazel_tools` is an injected
module outside the user-visible `mod graph`; its verbatim MODULE has ordinary
dependencies, module extensions/use_repo names, repository rules, and ordered
toolchain registrations. Slug currently evaluates and resolves only the user
root graph. A two-name rules_shell/platforms map, fabricated RepoSpecs, or a
root dependency would not preserve Bazel ownership or structural identity.

Audit pinned Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` primary source for built-in
module injection, module-file selection, resolution, repo mappings, extension
generated names, override/lockfile interaction, and registration order.
Correlate that source with the exact checked-in embedded MODULE bytes and the
observed `bazel_tools`/rules_shell/platforms mappings. Audit Slug's module
evaluation, resolution, lockfile, route, mapping-digest, and loading-anchor
owners.

Freeze one compact DICE-owned representation and dependency direction that
reuses the existing resolution graph rather than creating a parallel graph.
Every built-in direct dependency, MVS-selected transitive input,
extension/use_repo name, registration, snapshot/content identity, registry and
lockfile policy, and root graph input that can change the combined result must
participate structurally. Preserve root file Need/error ordering and never hold
a lock across a compute. If full injection is not bounded, record `REPLAN`
without pruning the embedded module.

## Compatibility

Exact: verbatim embedded MODULE bytes, dependency and registration order,
Bazel 9.2 MVS/module-repository relationships, apparent-to-canonical mappings,
registry content hashes, and root/built-in precedence. Slug-native: DICE type
names, compact representation, diagnostics, manifest framing, and non-Bazel
identity bytes. Unsupported/deferred: catalog expansion, package/BUILD/Bzl
dispatch, configured external toolchain resolution, TestProvider/TestRunner,
execution/results/BEP/coverage, Host scanning, Windows, JVM/Java, and exact
Bazel identity bytes.

## Scope, proof, and stops

This design packet may edit only:

- `thoughts/shared/plans/slug-v2-subplans/current-packet.md` and
  `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`
  and `05-bzlmod-checkpoint-evidence-3.md`;
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`,
  `06-analysis-toolchains-and-actions.md`, and
  `08-ruleset-and-command-conformance.md` for dependency bookkeeping; and
- `thoughts/shared/plans/slug-v2-subplans/09-v1-extraction-ledger.md` only if
  the final reuse decision changes an existing row.

Cap bookkeeping at 260 net lines and add no file. No Rust, Cargo/BUILD metadata,
source asset, fixture/oracle record, DICE key, registry/materializer action,
package/Bzl/configured-analysis behavior, command/Test/TestRunner, REAPI/BEP,
JVM/Java, Windows branch, second graph, process-global state, Host observation,
or runtime source selection is authorized.

Require pinned primary-source anchors and observed mapping evidence; a complete
input/output/Need/error/equality/invalidation contract; accepted and rejected
reuse decisions; exact/Slug-native/deferred classification; an explicit
successor allowlist/caps/tests/stops; source/structure, credential, archive
active-layout, and diff checks; and independent review. One bounded correction
is allowed; a second material miss is `REPLAN`. At `ACCEPT`, schedule only
the reviewed successor.
