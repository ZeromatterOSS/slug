# Current Slug V2 Packet

Packet: `WP-5-m1-loading-host-glob-transactional-attempt-owner`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted `Host glob transactional package-evaluation design`,
private traversal `18f4b2db`, and loading adapter `cb892552`
Validation tier: private/local Rust

Allowed files:

- `app/slug_loading_v2/src/host_glob/mod.rs`
- `app/slug_loading_v2/src/host_glob/adapter.rs`
- `app/slug_loading_v2/src/package.rs`
- `app/slug_loading_v2/src/bzl_module.rs`
- new `app/slug_loading_v2/src/host_package_attempt_tests.rs`
- terminal canonical, manifest, and exceptional routing updates

Result: add the reviewed private, dormant abort/await/restart owner. Across
attempts retain only a compact prepared map keyed by one shared raw pattern
plus operation. Every attempt reparses immutable source and owns a fresh
module, recorder, targets, used-glob state, and print capture. Missing prepared
work exits through typed `Pending`; prepared traversal/non-UTF8 failures exit
through typed `Terminal`; outer control is recognized only through the
attempt-local slot, never error text.

Drop evaluator state and all targets before any adapter await. Complete work
enters the prepared map and retries; `Need` returns unchanged; typed input
failure retains only its saved print prefix. Pending attempts publish no
events. Terminal errors retain their exact typed payload and print prefix but
never call `finish`; final success alone publishes package targets. Preserve
include-then-exclude source order, both operations, per-include/all-excluded
`allow_empty` diagnostics, leading-`@` disambiguation, sorting, deduplication,
and exact UTF-8 conversion or typed rejection.

Add no public export, dependency, DICE key, legacy `PackageLoadKey` change,
production caller, fixture, parser change, direct IO, lock, blocking, fresh
graph, speculative value, JVM code, external repository, SUBPACKAGES,
native-Windows, or broader grammar. Tests must cover one/multiple/repeated
requests, loaded macros, operations and composition, payload-bearing
traversal/input/non-UTF8 errors, `Need`, pending discard, terminal/final event
and target ownership, same-graph reuse/restoration, and zero production
callers. Run focused attempt/adapter/traversal tests, full loading crate,
formatting, diff/archive, and exact scope/public/key/dependency/IO/lock/caller
guards. Stop on a sixth file or downstream propagation requirement.
