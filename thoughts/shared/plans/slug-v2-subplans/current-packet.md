# Current Slug V2 Packet

Packet: WP-5-7A-repository-context-which-audit

Milestone: M7A bootstrap-critical loading/repository execution closure. Audit
the generic Bazel 9.2 `repository_ctx.which` boundary reached after the accepted
bounded template composition.

Status: ready for one bounded docs-only audit. No Rust implementation is
authorized by this packet.

## Accepted predecessor

`WP-4-5-7A-repository-context-template-implementation-r1` returns `ACCEPT`
after one correction rereview. It admits the exact bounded normalized-string
destination plus canonical external `path(Label)` source shape, sequential
insertion-ordered Latin-1 replacement, default/explicit executable mode and
default auto-watch through existing routed source observations.

The accepted implementation adds no DICE key, source/materialization owner,
physical-path read, retained source buffer or generated-effect representation.
It closes at 371 production and 484 proof Rust additions, 855 total, within its
400/500/900 caps. Full loading and query suites pass; the rebuilt authentic
rules_rust replay contains no `repository_ctx.template` stop and terminates at
the independent `repository_ctx.which("bash")` call in authentic rules_shell.

## Audit objective

Pin the complete Bazel 9.2 `repository_ctx.which(program)` semantics and the
authentic rules_shell call shape before selecting any implementation. Determine:

1. accepted argument/result types, PATH lookup order, executable/file checks,
   platform spelling and `None` behavior;
2. environment, filesystem, symlink and path observations required for exact
   invalidation, including missing-to-present and A/B/A restoration;
3. whether existing repository Host-input, path-observation and source-route
   owners compose without a new DICE key or direct filesystem access;
4. how a synchronous repository invocation can request lookup only after its
   evaluator, heap, builder, captures and borrows have been dropped; and
5. the smallest consumer-independent exact slice that advances replay without
   a rules_shell, shell-name, platform or host special case.

Classify every proposed behavior as **exact**, **Slug-native**, or
**unsupported/deferred**. Bazel 9 is the sole exact compatibility reference.

## Required evidence

- Pinned Bazel 9.2 implementation and focused tests for `which` and its path
  lookup helper.
- An isolated Bazel 9.2 oracle covering found/missing, PATH order, executable
  and wrong-kind candidates, symlinks, empty/relative/absolute program forms,
  and platform differences relevant to the bounded slice.
- Source inspection of the authentic rules_shell 0.6.1 call and any adjacent
  generic consumer needed to distinguish the minimum shape.
- A live Slug ownership trace from repository invocation through Host
  environment and path observations, with DICE equality/invalidation domains
  named explicitly.
- A measured allowlist, gross production/proof caps, function-complexity caps,
  validation matrix and terminal `REPLAN` conditions if implementation is
  selected.

## Frozen stops

The audit must return `REPLAN` rather than authorizing Rust if exact lookup
requires an unowned process environment, direct filesystem reads, an evaluator
or lock across DICE, platform-specific widening beyond a bounded slice, or a
rules_shell/toolchain special case. It must not implement `which`, execute,
download, patch, read, watch, symlink or any other repository API.

Scheduling documentation may change only the canonical plan, Stages 4 and 5,
and this manifest. Audit result is pending.
