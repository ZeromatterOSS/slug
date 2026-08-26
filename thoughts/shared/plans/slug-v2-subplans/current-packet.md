# Current Slug V2 Packet

Packet: `WP-4-5-7A-root-package-external-bzl-load-owner-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: `04-starlark-loading-and-build-packages.md` and
`05-bzlmod-and-repository-graph.md`
Base: accepted request-owned selected-registry extension source owner

Result: design the smallest structural selected-registry route producer and
root-package external-Bzl consumer, or return `REPLAN` at one smaller missing
owner. This packet is docs-only.

## Corrected entry terminal

The prior audit entry was not command-true. An unchanged direct rules_rust
fixture first stops at the parked M8 wildcard toolchain-registration shape.
After removing only that line in a disposable copy, it stops while evaluating
the root BUILD file:

```text
external repository load is not available in the root Host loader:
@rules_rust//rust:defs.bzl
```

`rust/extensions.bzl` later declares
`repository_rule(doc = "Declare an empty repository.", ...)`, and Slug still
rejects nonempty repository-rule `doc`, but that is not the next live M7A
owner. The accepted selected-extension source owner does not make root BUILD
loads external-repository aware.

`RootPackageLoadKey` currently parses every direct load through the root-only
label resolver and invokes only root Host Bzl children. Repository-package
loading already consumes a structural `RootRepositoryRoute` and dispatches
external Bzl evaluation. `RootRepositoryRouteKey`, however, projects only
builtin and direct-local root dependencies; it returns Unsupported for a
registry-selected dependency. Loading therefore has no lawful selected route
to consume and must not reconstruct one from mappings, canonical names or
physical paths.

Pinned Bazel 9.2 remains behavioral authority. Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only:
the package-source layer owns resolved direct-load associations and the
runtime consumes immutable already-resolved modules. Copy no Zig code,
representation, scheduling, path, cache, digest or behavior.

## Design questions

1. Freeze the Stage 5 producer that resolves one root apparent repository
   through the accepted root mapping and canonical selected definition, then
   projects the existing structural selected route. Preserve builtin and
   direct-local results byte-for-byte. Decide the exact key/value visibility,
   typed error, observation and equality boundary without a second selected
   graph, global registry, fallback scan or path inference.
2. Freeze the Stage 4 consumer change for one external load discovered by
   `RootPackageLoadKey`. It must consume that route and the existing external
   Bzl owner, while root/self loads retain the current Host Bzl path. Package
   evaluation remains the package owner; the Bzl child retains source,
   recursive evaluation, manifest and event ownership.
3. Preserve deterministic source order: root-module/package predecessors,
   then each direct load's route and Bzl child in declaration order. Merge
   observed epochs left-first before semantic projection; stop immediately on
   Need, typed outer or semantic failure. Do not replay child events.
4. Define retained identity and lifecycle proof. Every selected module,
   RepoSpec/source-policy and ordered mapping input used by the route must
   participate structurally in DICE equality/invalidation. Physical roots and
   apparent/display spelling remain projections. Prove cold/warm, A/B/A,
   cancellation, source/mapping change and no unrelated command repair.
5. Select one bounded implementation successor with exact file authority,
   entry hashes, production/proof caps and command-visible evidence, or return
   `REPLAN` at one smaller visibility/observation prerequisite. Reuse accepted
   Bazel 9.2 source/oracle evidence unless a demonstrated behavior gap remains.

## Required proof boundary

The future design must cover root-local nonregression; builtin/direct-local
external loads; one selected-registry root BUILD external load; selected self
and mapped recursive loads; missing/unsupported/cycle terminals; route/source/
mapping A/B/A; Need/cancellation; event and epoch order; and warm silence. A
small disposable command replay may discriminate the selected route, but no
fixture or oracle change is authorized by this packet.

## Compatibility

- **Exact:** Bazel 9.2 root BUILD external-load resolution through the root
  repository mapping, plus all already accepted root/direct-local behavior.
- **Slug-native:** Rust/DICE route ownership, typed outer carriers, retained
  epochs/events and heap lifetime.
- **Unsupported/deferred:** implementation during this design; wildcard
  registration; repository-rule `doc`, collection schemas, invocation and
  effects; generated package breadth; toolchains/providers/actions/input
  trees; crate_universe; M8/M7B; exact configuration/output identity bytes.

## Authority and validation

Write authority is exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- this manifest
- `04-starlark-loading-and-build-packages.md`
- `05-bzlmod-and-repository-graph.md`
- `06-analysis-toolchains-and-actions.md`
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`

Use net documentation caps <=50 canonical, <=260 current, <=180 Stage 04,
<=180 Stage 05, <=240 Stage 06 and one routing row, <=910 aggregate. Validate
targeted live-source consumers, pinned Bazel/Zabel anchors, scheduling
consistency, structure and `git diff --check`. Obtain independent review for
the route/DICE ownership and before activating implementation.

STOP Rust/test/fixture/oracle/Cargo/BUILD edits, a RootPackageLoad-only design,
route reconstruction in loading, command repair, public activation,
`@bazel_tools` invention, Java/JVM, a second successor, proof/cap waiver,
milestone closure, M8/M7B or exact identity bytes. `REPLAN` before widening.
