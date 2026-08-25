# Current Slug V2 Packet

Packet: `WP-6-7A-generated-package-load-bridge-implementation-retry`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design base: corrected design accepted 2026-08-25 / Rust `4d83a829`

Result: complete one private core bridge so the external exported-source build
branch loads packages from extension-generated repositories while preserving
all existing route diagnostics byte-for-byte.

## Active implementation contract

Change only:

1. `host_module.rs`: one doc-hidden public predicate on
   `RootRepositoryRouteError` matching exactly Unknown/Unsupported. Keep the
   kind and field private.
2. `generated_repository_definition.rs`: one opaque `pub(super)` three-way
   Missing/ContextMismatch/Other disposition on
   `HostCanonicalRepositoryApparentMappingError`. Expose no predecessor or
   private inner kind.
3. new private `runtime/generated_package_route.rs`: own
   `GeneratedPackageRouteKey` and its Observed sibling, both rejecting root and
   displaying `generated-package-route:{workspace}:@{apparent}` with the
   observed prefix. Share one mapping-then-definition driver. Require the
   already-visible definition kind Generated and its RepoSpec plus validated
   LocalUnsupported policy; construct only with `for_generated_repo_spec`.
4. `runtime/mod.rs`: register only the private sibling module.
5. `dice.rs`: retain only `BuildCommandErrorKind::GeneratedRoute`, build-branch
   fallback/translation glue and the command-level discriminating proof.

Remove the dirty `root_apparent_repository_route.rs` helper completely.

The public route child remains first. Only Unknown/Unsupported invoke the
bridge. Generated success continues through unchanged package loading. Mapping
Missing and any successful non-Generated definition are fallback-neutral
Missing and restore the exact request-local public-route error. Only
ContextMismatch/Definition/Compute become GeneratedRoute.

Allowed files are exactly:

- `app/slug_bzlmod_v2/src/host_module.rs`;
- `app/slug_core_v2/src/runtime/generated_repository_definition.rs`;
- `app/slug_core_v2/src/runtime/generated_package_route.rs` (new);
- `app/slug_core_v2/src/runtime/mod.rs`; and
- `app/slug_core_v2/src/runtime/dice.rs`.

Freeze against `4d83a829`: <=480 production, <=420 proof and <=900 aggregate
net additions. Physical ceilings are host module 4,825, dice 11,720, generated
definition 4,010, runtime mod 340 and new module 720, from baselines
4,783/11,550/3,964/331/new. Add no `rustfmt::skip`, Cargo/BUILD or fixture.

## Ownership and compatibility

The bridge pair remains DICE-owned by workspace plus apparent repository.
Legacy/Observed share one driver. Need and observed outer are immediate and
carrierless; Complete retains one local Result Arc and, for Observed, only the
left-first union of mapping then definition epochs. Equality is complete-only;
the parent is eventless and adds no lock, cache, task or publication state.

Lower observed mapping/definition outers stay opaque and carrierless. At the
build boundary, translate a bridge outer to GeneratedRoute::Compute, retaining
only the completed public-route prefix and fabricating no bridge carrier or
epoch. After Need, command re-entry reacquires the public-route error through
DICE. The bridge parent remains eventless and lock/cache/task-free.

Legacy route/build semantics and existing diagnostic bytes are exact.
Doc-hidden classification, private bridge identity and observed epoch
association are Slug-native. Query/public publication, Windows/macOS and exact
identity bytes remain unsupported/deferred.

The fallback exists because the public Bzlmod route cannot represent an
extension-generated repository. Delete it when the later M7A public generated-
repository routing/publication owner consumes the accepted chain without
dependency inversion. Generated-success and direct-local/builtin/unknown/
unsupported regressions, including mapped non-Generated Unsupported and an
observed-outer/prefix case, prevent expansion into a general repair path.

## Proof and stops

The successor must prove key/root rejection and Display; mapping Missing,
ContextMismatch and Other translation; definition outer/semantic pass-through;
left-first epoch union, Need/outer carrierlessness, complete equality/validity,
cancellation/recovery and warm silence; Generated success; and byte-identical
direct-local, builtin, unknown and unsupported behavior. The unsupported proof
must include a successfully mapped non-Generated definition restoring the
exact original error. An observed-outer proof must preserve the completed
public-route prefix, produce typed GeneratedRoute::Compute and retain no bridge
carrier/epoch. Validate the doc-hidden cross-crate predicate in Bzlmod and
through core as its direct dependent. Reuse accepted Bazel 9.2 evidence; add no
fixture unless a demonstrated generated-build discriminator remains absent.

Serial validation on Ubuntu 24.04 WSL is: focused classifier/key/fallback
proofs; protected external-build suites; full Bzlmod; full core with only the
accepted query diagnostic baseline; separate runtime with only the accepted
PathObservationEpochKey baseline; direct commands check; formatting; exact
allowlist/accounting/physical/cap/no-skip checks; and diff hygiene.

STOP a sixth implementation file, retained route-error state, public private-
kind/field exposure, diagnostic string matching, a third key family, query/
public activation, new events/retention/locks, fixture growth without a
demonstrated gap, cap/format/baseline waiver, milestone closure, M8/M7B or exact
identity work. REPLAN before widening. After ACCEPT return to the Stage 6 owner
plan for scheduling. M7 remains partial and M7A -> M8 -> M7B remains.
