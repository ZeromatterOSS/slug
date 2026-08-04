# Current Slug V2 Packet

Packet: `WP-5-m1-external-repository-test-suite-query`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted corrected suite-only external test-suite design, live Bazel
9.2 direct-override omitted/explicit-empty/parent/cycle probes, pinned Bazel
membership and tests-function source, accepted root test-suite loading/query
representation, and accepted external source/filegroup/alias/config-setting
query spine
Validation tier: one-file private query projection plus focused public
query/core and exact Bazel/Slug oracle rows

Implement only the accepted same-package suite-only external native
`test_suite` projection in `app/slug_query_v2/src/graph.rs`. Reuse
`PackageTargetKind::TestSuite`, `TestSuiteMembership`, native capability and
test metadata, the accepted external graph/key, and existing query/render
consumers. Before generic source synthesis, require every nonempty explicit
member to resolve in the loaded target batch to a same-package native
`test_suite`. Accept omitted, explicit-empty, parent-to-suite chains, and
suite-only cycles; do not add acyclicity logic.

Project `test_suite rule`, retained non-executable suite capability, sorted
tags/manual metadata with no size, total `tests` and `$implicit_tests`
attributes, and ordinary edges. `tests` preserves the loader's canonical
stored order and `membership.tests_explicit()`; `$implicit_tests` is explicit
true and empty for this accepted no-test-rule slice. Ordinary edges use first
occurrence across `tests` then implicit members. Remap accepted members to
canonical external identity with the selected apparent route spelling. Do not
synthesize a suite member or observe a source path. Project no query-visible
visibility attribute and retain existing public/private visibility policy.

Production allowlist: `app/slug_query_v2/src/graph.rs`. Tests may change only
that file and `app/slug_core_v2/src/runtime/dice.rs`. Oracle changes are
limited to the existing `module-local-override` fixture TOML,
`workspace/dep/BUILD.bazel`, and expected JSON; add no asset or fixture. Do not
alter Cargo metadata, public APIs, DICE keys, repository routes,
loading/source owners, CLI/server adapters, protocol, formatters,
configuration, analysis, actions, execution, or another fixture.

Extend the existing fixture with omitted, explicit-empty, parent-to-empty,
and two cyclic suites. Add exactly five Bazel/Slug commands for the parent:
literal, `--output=label_kind`, `labels(tests, ...)`, `deps(...)`, and
`tests(...)`. Exact live evidence is parent literal, `test_suite rule`, empty
leaf from `labels`, leaf then parent from `deps`, and empty successful
`tests()`. Protect all existing normalized fixture semantics.

Focused structural/public evidence must additionally prove omitted versus
explicit-empty provenance, total empty `$implicit_tests`, member remapping,
attribute and edge order/explicitness, capability, tags/manual metadata,
canonical/apparent labels, empty/self closures, parent closure, finite cycle
`deps` and empty `tests()`, accepted visibility, and no source synthesis.
Preserve all accepted external source/filegroup/alias/config-setting behavior,
lifecycle reuse, publication, and stop gates.

Stop non-suite, unresolved, cross-package, and named-repository members;
external test rules or nonempty implicit collection; nontrivial visibility;
and other unsupported rule kinds. Stop and `REPLAN` on a need for external
test-rule loading/metadata/discovery, source synthesis, a new target kind/key
or owner, another package/repository route, loads/globs/patterns,
configuration/select, analysis, build/execution, JVM, Java bytecode, or Bazel
delegation.

Finish with serial focused query/core tests, the full query suite, the
unchanged root test-suite projection test, quiet direct-dependent checks, the
required `slug_cli_v2` rebuild before Slug oracle replay, GNU-Windows
query/core no-run linkage, formatting, `git diff --check`, archive/scope/
no-Cargo guards, fixture generation plus distinct-root replay, and one
independent terminal implementation review.
