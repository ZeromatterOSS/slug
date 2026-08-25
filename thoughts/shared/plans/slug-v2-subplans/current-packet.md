# Current Slug V2 Packet

Packet: `WP-6-7A-generated-package-load-bridge-proof-cap-correction-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design base: independently accepted proof/cap correction over retained candidate
from `4d83a829` plus Bazel 9.2 evidence `6fd78a21`

Result: complete and prove the retained generated-package bridge without
widening its semantics or public surface.

## Exact authority and freeze

Write exactly:

- `app/slug_bzlmod_v2/src/host_module.rs`;
- `app/slug_core_v2/src/runtime/generated_repository_definition.rs`;
- `app/slug_core_v2/src/runtime/generated_package_route.rs`;
- `app/slug_core_v2/src/runtime/mod.rs`;
- `app/slug_core_v2/src/runtime/dice.rs`;
- `app/slug_core_v2/src/runtime/root_apparent_repository_definition.rs`; and
- `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`.

Before editing, require the retained candidate SHA-256 values
`185ec768…`, `8166e0c8…`, `27e6ee70…`, `204fd751…`, and `49f937c8…` in the file
order above through `dice.rs`. Require clean test-only baselines
`root_apparent_repository_definition.rs` `3b2e046a…` and
`build_command_tests.rs` `501375e7…`. The accepted fixture at `6fd78a21` is
read/execute-only and must remain byte-identical.

The retained accounting from `4d83a829` is exact: generated route production
lines 1..425 contribute 425; host classifier 14; mapping disposition 21;
runtime registration 1; and dice glue net 70, totaling +531 production. The
host classifier proof contributes 28 and new-module proof lines 426..591
contribute 166; mapping proof replacements are net zero, totaling +194 proof
and +725 aggregate.

## Bounded correction

Preserve every retained semantic line except one extraction in `dice.rs`: move
the already-inline observed bridge-outer translation into one private,
production-used `external_build_generated_route_outer` pure finisher. It takes
the apparent repository, opaque bridge outer and completed public-route prefix;
it returns `external_build_complete` with typed GeneratedRoute::Compute and the
unchanged prefix. It may retain no carrier or epoch and add no key, event, lock,
cache or task.

In the existing `#[cfg(test)]` root-definition module, change only
`CompositionTracker` and `local_materialized_transaction` to
`pub(in crate::runtime)` and generalize that helper to take an explicit
`RepositoryMaterializationSuccess` instead of hardcoding Local. Existing
callers pass Local; the archive proof passes Immutable, matching its request
kind and avoiding `SuccessKindMismatch`. Do not expose fields, production
kinds, constructors or materialization state. Add exactly two command proofs
in `build_command_tests.rs`:

1. a real archive-override dependency whose public route is Unsupported but
   whose canonical apparent mapping and selected-nonregistry definition succeed
   after a lawful Immutable materialization epoch through the generalized
   existing helper; drive the actual
   external exported-source branch in Legacy and Observed modes and require the
   exact original route error text/typed RepositoryRoute terminal, with no
   GeneratedRoute replacement; and
2. call the production-used opaque-outer finisher with a nonempty completed
   public-route prefix; require typed GeneratedRoute::Compute, pointer-stable
   prefix content, no source certificate and no bridge carrier/epoch.

Reuse existing transaction/materialization and epoch helpers. Fabricate no
semantic DICE value, private error kind or malformed observation.

## Caps and utility boundary

Against `4d83a829`, caps are <=540 production, <=370 proof and <=900 aggregate
net additions. Physical ceilings are host 4,825, dice 11,720, generated
definition 4,010, runtime mod 340, new route 720, root definition 1,740 and
build command tests 3,950. Add no `rustfmt::skip`.

The retained key/carrier already uses V2-owned compact identity types,
`Arc<Result<...>>`, `Dupe`, `PathObservationEpoch` and `Allocative`. Add no
`String`/`Vec` retained field, collection, interner, hash wrapper, cache or new
utility import. No Stage 9 ledger row is needed because representation and
utility ownership remain unchanged.

## Proof and validation

The existing generated-route, classifier, mapping-disposition, identity,
complete equality/validity, left-first epoch, cancellation/recovery, warm
silence and upper-family nonactivation proofs remain mandatory. Preserve exact
direct-local, builtin, unknown and ordinary unsupported diagnostics.

Validate serially on Ubuntu 24.04 WSL:

1. focused classifier, generated-package-route and the two named command tests;
2. protected external build suites, then full `slug_bzlmod_v2`, full
   `slug_core_v2` with only the recorded byte-identical query diagnostic
   baseline, and separate runtime with only the recorded
   `PathObservationEpochKey` baseline;
3. clean stale `slugd`, `cargo build -p slug_cli_v2`, run the unchanged
   `module-extension-use-repo` fixture with `SLUG_V2_BIN=target/debug/slug` and
   clean `slugd` again;
4. direct build/cquery/query command checks, `cargo fmt --all -- --check`,
   `scripts/v2_archive_status.sh` with only its exact three accepted non-V2
   thoughts-path residuals, exact SHA/allowlist/accounting/physical/no-skip/
   utility scans, fixture byte identity, credential scan and `git diff --check`.

## Compatibility and stops

Legacy route/build semantics and diagnostics remain exact. The bridge keys,
opaque classifiers, observed epoch association and opaque-outer translation
remain Slug-native. Public/query generated-repository publication, other
platforms and exact configuration/output identity remain unsupported/deferred.

STOP fixture/docs/harness/Cargo/BUILD edits, retained production drift outside
the pure finisher extraction, public private-kind exposure, diagnostic string
classification in production, fabricated DICE state, another test hook, cap or
baseline breach, an eighth Rust file, query/public activation, milestone
closure, M8/M7B or exact identity work. REPLAN before widening. After terminal
review, commit all accepted Rust together with the plan outcome. M7 remains
partial and M7A -> M8 -> M7B remains.
