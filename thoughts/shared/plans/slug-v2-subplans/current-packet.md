# Current Slug V2 Packet

Packet: `WP-5-m1-loading-host-package-key-input-ownership`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted Host package-loading key input and ownership design; pinned
Bazel 9.2 package/BzlLoad source; existing Host root-module, package lookup,
special-capable file, raw-string parser, glob traversal/adapter/attempt, and
legacy cycle-detector owners
Validation tier: public cross-crate DICE identity, lifecycle, and event owner

Implementation files:

- `app/slug_bzlmod_v2/src/host_package.rs`
- `app/slug_bzlmod_v2/src/lib.rs`
- `app/slug_loading_v2/src/bzl_module.rs`
- `app/slug_loading_v2/src/cycle_detector.rs`
- new `app/slug_loading_v2/src/host_package_load_tests.rs`

Terminal scheduling updates may also change this manifest, the owner plan, and
canonical Live Status.

Result: implement the accepted public bzlmod `RootPackageBzlTarget` and
`RootPackageSourceKey` projection plus dormant private
`HostBzlModuleEvalKey`/`HostPackageLoadKey`. Preserve exact package/BUILD/
special-byte selection, nested-package checks, root-only load labels,
Bazel-internal parsing, typed Need/errors, complete-only equality/validity,
local event batches, legacy-versus-Host cycle isolation, same-DICE lifecycle,
and the accepted transactional glob-attempt boundary.

Add no dependency, fixture, command/query/analysis/core caller, public loading
export, legacy key/value/diagnostic change, external repository or
materialization breadth, direct IO, injected semantic value, fresh graph,
blocking/lock-across-compute path, JVM, Java bytecode, or Bazel delegation.
Stop on a sixth implementation file, a private bzlmod-owner exposure, arbitrary
invalid-UTF-8 source parsing, legacy behavior change, or required activation.

Validate focused source/key/cycle/lifecycle/event tests, full
`slug_bzlmod_v2` and `slug_loading_v2`, one direct `slug_core_v2` compile
dependent, GNU-Windows no-run linkage, formatting, diff/archive status, and
exact scope/public/Cargo/dependency/caller/legacy/IO/lock/blocking/JVM guards.
Obtain one terminal independent implementation review.
