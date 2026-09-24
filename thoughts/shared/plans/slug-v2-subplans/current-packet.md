# Current Slug V2 Work Packet

Packet: WP-7-94-m7a-selected-file-effect-fixture-download-input-r1
Status: accepted; independent DESIGN and FINAL review ACCEPT

## Outcome and compatibility

Restore the 13 older selected repository-file-effect tests that now fail
because their shared test transaction omits the selected-download input
installed for selected-owner execution. This is a fixture-only repair:
inject an empty, unauthorized `SelectedExtensionDownloadTransaction` for
the fixture workspace. Do not alter production preflight, selected
download authorization, generated repository effects or any admitted
runtime behavior.

No compatibility surface changes. Existing exact Bazel 9 slices stay
exact, existing Slug-native physical/configuration/action identity
divergences stay Slug-native, and generated `rust_host_tools`
`download_and_extract`/path, physical reads and generator execution
remain unsupported/deferred. This packet's test input models an
extension that does not call `module_ctx.download`; it is not an oracle
or a new parity claim.

## Evidence and semantic owner

WP793 at `3e9a895a6` independently accepted selected root and
non-generated external `module_ctx.path(Label)`; 39/39 owner tests,
three Core checks and four Bzlmod label-path tests pass. Its fresh root
smoke reaches the honest generated-Label refusal at rules_rust's
`@rust_host_tools` operand. The unchanged
`module_extension_repository_file_effect::tests::` suite instead passes
6/19, with 13 terminals reaching `selected module extension download
inputs unavailable`. The preflight is already in the accepted HEAD
`module_extension.rs`, and all selected-owner transactions in that test
module pass through `with_host_inputs_for` in
`module_extension_repository_file_effect.rs`. That helper installs the
host-input transaction, platform and environment cells but not the
selected-download transaction. An empty transaction for the exact
workspace is the correct no-download fixture state: no owner is
authorized, so the preflight returns no ready download without
relaxing the production invariant.

No additional Bazel oracle is needed for test-fixture plumbing; the
accepted WP793 regression remains the semantic evidence. The fixture
may expose further independent failures after the input is installed;
report them separately and do not claim all 13 share a cause until the
full suite passes.

## Scope, stops and validation

Base clean `main` at `3e9a895a6`. Root owns the one-file fixture edit,
serial Cargo validation, documentation receipt and commit. Test-only
allowlist: `app/slug_loading_v2/src/module_extension_repository_file_effect.rs`
under `#[cfg(test)]`; scheduling allowlist: this manifest and the
canonical Current paragraph. No production, Core/Bzlmod/DICE API,
repository-rule, MODULE/lockfile, vendor, transport or NativeLink
change. The Buck2 hot-path utility skill creates no action here.

Obtain independent DESIGN ACCEPT before editing the fixture and
independent FINAL review of the actual diff. Preflight exact
nonignored selectors for the 19 file-effect tests, run the full group,
and rerun WP793's 39 selected-owner tests to guard the shared
preflight. If failures remain, determine whether they are separate
fixture/input defects; do not broaden the packet to production
semantics without design correction. Run `cargo fmt --all -- --check`,
`git diff --check`, archive status and plan status. A CLI rebuild or
root smoke is not necessary for a test-only change. Preserve ordinary
Bazel RC discovery; never inspect/copy/print `~/.bazelrc`.

Stop and `REPLAN` if the empty transaction would authorize any owner,
change a production input path, or fail to restore the intended tests
without a bounded fixture-only explanation. Larger generated host-tools
work and public `@bazel_tools` query breadth remain separate M7A
packets.

## Immediate predecessor

WP793 at `3e9a895a6` independently accepted bounded selected
`path(Label)` and lexical dirname/get_child while explicitly refusing
generated-owner routes before DICE recursion.

## Validation receipt

Independent DESIGN ACCEPT: the empty transaction has no authorized
owners or verified downloads, and the selected preflight returns before
reading any download epoch for unauthorized owners. The fixture helper
is the shared owner for these tests. One test-only import and one
test-helper insertion use `workspace.dupe()`; no production code or
download epoch changed.

Exact nonignored preflight finds 19 file-effect selectors. The full
`module_extension_repository_file_effect::tests::` group passes 19/19,
restoring all 13 formerly red cases. The WP793 selected-owner group
passes 39/39 after the fixture repair. `cargo fmt --all -- --check`,
`git diff --check`, archive status and plan status pass. No CLI rebuild,
root smoke, NativeLink or BuildBuddy transport was needed for this
test-only packet. Independent FINAL ACCEPT of the actual one-helper diff
and scoped receipt.
