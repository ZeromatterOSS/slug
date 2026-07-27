# Current Slug V2 Packet

Packet: `WP-5-m1-loading-typed-propagation`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted root loading typed-propagation design; accepted dormant
Host package/`.bzl` keys and public bzlmod preparation envelope
Validation tier: public cross-crate API/representation

Implementation files:

- `app/slug_loading_v2/src/bzl_module.rs`
- `app/slug_loading_v2/src/lib.rs`
- `app/slug_loading_v2/src/host_package_load_tests.rs`

Terminal scheduling updates may also change this manifest, the owner plan, and
canonical Live Status.

Result: rename the accepted private `HostPackageLoadKey` to public
`RootPackageLoadKey`, expose its terminal error as an opaque
`RootPackageLoadError`, and root-export loading aliases for the shared bzlmod
preparation outcome and Need. Preserve the exact compute body, key identity,
value, equality/validity, events, diagnostics, and display prefix.

Add no Cargo/dependency change, wrapper key/value/allocation, nested private
error export, query/analysis/core/CLI/server caller, external repository or
directory-discovery migration, fixture, oracle, materialization, runtime
driver, legacy change, JVM, Java bytecode, or Bazel delegation. Stop on a
fourth implementation file or any change to the accepted compute semantics.

Validate focused root-export/key/equality/Need/event/lifecycle tests, direct
`slug_query_v2` compile coverage, GNU-Windows no-run linkage, formatting,
`git diff --check`, archive status, and exact scope/export/no-caller/Cargo/
dependency/legacy/IO/blocking/JVM guards. Obtain one terminal independent
implementation review.
