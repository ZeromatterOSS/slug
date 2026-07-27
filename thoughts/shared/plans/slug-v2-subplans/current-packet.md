# Current Slug V2 Packet

Packet: `WP-5-m1-loading-host-glob-loading-adapter`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted `glob-package-boundaries` traversal oracle `5abff72e` and
private Host traversal owner `18f4b2db`; exact owner handoff under
`Pure Host glob traversal owner`
Validation tier: private/local Rust

Allowed files:

- `app/slug_loading_v2/src/host_glob/mod.rs`
- `app/slug_loading_v2/src/host_glob/traversal.rs`
- new `app/slug_loading_v2/src/host_glob/adapter.rs`
- new `app/slug_loading_v2/src/host_glob/adapter_tests.rs`
- terminal owner, canonical, manifest, and exceptional routing updates

Result: add one private async loading adapter that accepts a normalized
workspace, selected logical package root, `PackagePath`, one complete raw-byte
pattern, and FILES or FILES_AND_DIRS operation. It must use the existing
checked pattern and traversal-key constructors, compute the existing traversal
through the caller's `DiceComputations`, and project a complete success to the
same sorted/deduplicated shared raw paths without UTF-8 conversion or copying
path bytes. Forward `Need` and typed traversal failures unchanged. Pattern and
key-construction failures remain distinct pre-compute results.

The adapter adds no DICE key, cache, interner, lock, event, direct filesystem
IO, dependency, public export, or production caller. It does not add
include/exclude composition, `allow_empty`, callable diagnostics or sorting,
BUILD/`.bzl` acquisition, parser/evaluator activation or transaction
ownership, external repositories, SUBPACKAGES, native-Windows behavior, or
lone-surrogate claims.

Focused tests must prove pre-compute pattern/key rejection without traversal
activation, exact one-pattern operation projection, raw non-UTF8 byte
preservation, sorted/deduplicated shared-path identity, unchanged Need and
typed-error propagation, caller-owned same-graph invalidation/restoration, and
zero production callers. Run focused adapter and traversal tests, one direct
loading-crate suite, formatting, `git diff --check`, archive status, and exact
scope/caller/dependency/IO/lock/public-surface guards. Stop on a fifth
implementation file, a new key or retained cache, any production/callable
activation, or any need to change accepted traversal semantics.
