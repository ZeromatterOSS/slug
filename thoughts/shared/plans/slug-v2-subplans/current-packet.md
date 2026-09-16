# Current Slug V2 Work Packet

Packet: WP-7-10-m7a-workspace-feature-scope-reconcile-r1
Status: design ACCEPT; saved-artifact and pinned-source reconciliation pending

## Outcome and boundary

Classify the relationship between 22 CLI Cargo external normal units with
fewer features and the generated Bazel lock's Linux list, using saved
accepted artifacts and pinned rules_rust source only. This is a structural
source-provenance result,
not an exact configured Bazel action or Slug behavior claim. The immediately
prior packet stopped its one build-script target at 20 seconds with no
`num-traits` `_bs.flags` output; no build retry is selected.

Freeze clean main `28c6c74f7`, `Cargo.lock` SHA-256
`a19882e78b50a82ed900fce570f4a6aee778553448790eaf08e2e08a591f76d8`,
`Cargo.Bazel.lock` SHA-256
`6335564ccbb3c915d0a2fa23b8aec0d69986911e062b586e13c5ec9812c2f35e`,
`MODULE.bazel` SHA-256
`109585b7b40d274ea912d32c73f54c25cc4bef1bc5e60526e18ac9bfe48d210c`,
accepted CLI unit graph SHA-256
`edd4d806c72dccd3dd8589b20bb9ad897fef3a99a01b6a91692ef8ec3a01f7fd`,
accepted unit/lock analysis SHA-256
`42c2aeb3e1e0624ef705fcab21e05d57037151913ef25fdc0e65c493194a1f7f`,
and recovered offline Cargo metadata stdout SHA-256
`9f5141d31263857533f43646c2695411c951d2350e59fe1d68392eda10786017`.
The metadata receipt records `cargo metadata --locked --offline` and an
unchanged lock; the unit graph receipt records one frozen CLI `--bin slug`
Linux target graph without build actions.

`MODULE.bazel` gives crate_universe the root `Cargo.toml`, whose workspace
contains 48 members. Pinned rules_rust
`crate_universe/src/metadata/cargo_tree_resolver.rs` SHA-256
`446e9fe5217d065ec5d1cffba0bb5133353f685e895f193b293e9b3c506e670b`
invokes `cargo tree --workspace --edges normal,build,dev --target <triple>`
for feature resolution. `crate_context.rs` SHA-256
`526128841401537447a7c03f509aaef140c15186dc602f46c01482849955ee0f`
places the resolver feature sets in generated crate attributes. This proves
the generator's feature-resolution scope is broader than a single CLI binary;
it does not by itself establish every exact enabled feature.

## Static comparison

Use one local parser on the frozen accepted unit/lock analysis and the raw
Cargo metadata stdout. Assert unique package IDs in metadata, every one of
the 344 normal external units maps to one metadata node, the accepted
analysis still has 322 exact unit/lock sets and 22 narrower sets, and no
unit-only features. Compare each selected lock Linux list against that
package's workspace metadata `resolve.nodes.features`, with target/host
units kept separate. Name every exception, and separately check that each
of the 22 lock-only feature sets is present in metadata. Inspect the pinned
source commands and root manifest path rather than inferring generator
behavior solely from matching lists. Save a compact JSON analysis with
input hashes, counts, exception names and claim limits in `/tmp`.

Do not run Cargo, Bazel, a build script, compiler or test. Stop on hash,
cardinality, mapping or source mismatch; do not edit generated locks or
features. If the lists match broadly, describe this as workspace-scope
consistency, not proof that metadata directly generated lock bytes. If a
platform-select feature is absent from Linux lock but present in metadata,
keep that difference explicit. Complete Rustc arguments, generated flags,
compilation and M7A remain open.

The tracked allowlist is this manifest, canonical plan status, Stage 10 and
bootstrap readiness. Independent design review precedes the computation;
independent final review recomputes the source/metadata mapping and checks
that the conclusion does not claim external feature parity beyond the saved
lists.
Independent design review `ACCEPT` confirmed the frozen 344-unit input and
pinned workspace-wide `cargo tree` source. The final result must describe
workspace-scope consistency without claiming exact metadata-to-lock causality,
and keep Tokio's Linux selection separate from the 22 narrower CLI units.
