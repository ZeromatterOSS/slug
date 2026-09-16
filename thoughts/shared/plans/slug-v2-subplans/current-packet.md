# Current Slug V2 Work Packet

Packet: WP-7-10-m7a-external-unit-feature-accounting-r1
Status: saved-graph external unit accounting ACCEPT; configured actions open

## Outcome and authority

Reconcile the existing selected Linux CLI Cargo unit graph against the
frozen metadata inventory and generated Bazel lock at external package,
platform-unit and feature-list granularity. The prior Tokio packet accepted
one root-reachable library-unit match; it did not classify the other units.
This packet is read-only accounting, not an edit to crate features, a Bazel
action admission, compilation or M7A readiness.

Freeze clean main `917c8dcfa`, `Cargo.lock` SHA-256
`a19882e78b50a82ed900fce570f4a6aee778553448790eaf08e2e08a591f76d8`,
`Cargo.Bazel.lock` SHA-256
`6335564ccbb3c915d0a2fa23b8aec0d69986911e062b586e13c5ec9812c2f35e`,
`MODULE.bazel` SHA-256
`109585b7b40d274ea912d32c73f54c25cc4bef1bc5e60526e18ac9bfe48d210c`,
selected Cargo metadata analysis SHA-256
`dad20c2240aa421f5d99fe30190cbe7fa9035b4e388ad64aa30e762afaf9cd10`,
and accepted Cargo unit-graph stdout SHA-256
`edd4d806c72dccd3dd8589b20bb9ad897fef3a99a01b6a91692ef8ec3a01f7fd`.
Its receipt SHA-256 is
`6a83706f5289c18d00619ddb9e4df96c0feddb7f0d5304bb3ade2b5502955696`.
No new Cargo/Bazel invocation, network, build or test is selected.

## Bounded comparison

1. Re-parse the saved graph, verify one `slug` Linux binary root and traverse
   dependency indices. Compare its unique package IDs with the 35 local and
   310 external IDs in the frozen metadata inventory. Record missing and
   extra IDs explicitly; never infer that metadata reachability is an
   executable compilation unit.
2. For every root-reachable external `mode=build` library/proc-macro unit,
   exclude custom-build scripts and classify `platform` as target Linux or
   host. Map exact Cargo package ID/name/version to `Cargo.Bazel.lock` key.
   Compare that unit's feature set with the generated lock's `common` union
   its `x86_64-unknown-linux-gnu` select. Record each unit index, platform,
   equal/different result and features on each side, preserving duplicate
   package units separately. Flag any unexpected platform, target kind,
   missing lock key or non-unique package mapping as an unresolved error.
3. Save a deterministic JSON analysis under `/tmp` with source hashes,
   counts, missing IDs and every mismatch. Independently review its counts
   and at least all mismatches against raw graph/lock input before recording
   a plan result.

This is a structural lock-versus-Cargo-unit comparison. A generated lock
feature list is not evidence of an actual configured Bazel compiler action.
Different host/target Cargo units may legitimately carry distinct features;
comparison with one lock list cannot alone authorize a feature edit. The
tracked allowlist is this manifest, canonical plan status, Stage 10 and
bootstrap readiness. No Cargo or Bazel lockfile may change. Independent
design review precedes the comparison; independent final review limits any
claim to saved graph and lock data. External action feature flags, generated
inputs, compilation and the first M7A readiness row stay open.
Independent design review `ACCEPT` confirmed the one-root/491-unit graph,
343 graph package IDs, 344 eligible external compilation units and unique
lock keys. The reviewer confirmed that the comparison remains structural
even when a unit's feature set is narrower than the lock list.

## Result

Deterministic analysis SHA-256
`42c2aeb3e1e0624ef705fcab21e05d57037151913ef25fdc0e65c493194a1f7f`
records all 344 eligible external unit rows and 22 exact mismatches. The one
Linux `slug` root reaches all 491 saved graph units and 343 package IDs:
35 local and 308 external. Frozen metadata listed 310 external IDs; only
`getrandom 0.3.4` and `libm 0.2.16` lack CLI units. There is no graph-only
package ID. Of 219 Linux target and 125 host external build units, 322 have
the same feature set as the generated lock's Linux list. All 22 differences
are lock-only additions: five target units (`ahash`, `lalrpop-util`,
`num-traits`, `relative-path`, `rustix`) and 17 host units. No unit-only
feature appears. Independent final review `ACCEPT` recomputed every unit and
mismatch row against raw frozen inputs and confirmed unchanged tracked state.
No new Cargo/Bazel command, build or test ran. The generated lock list has
not been proven to be the configured Bazel action flags; the discrepancies
must be classified at that boundary before any parity or feature edit.
