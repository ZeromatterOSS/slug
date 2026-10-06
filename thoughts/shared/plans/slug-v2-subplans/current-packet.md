# Slug V2 current work packet

Packet: WP-7-223-m7a-build-info-supplied-byte-render-r1
Status: Supplied status parsing accepted; complete pure BuildInfo byte rendering selected

## Observable result and source basis

Complete the pure supplied raw-status/template bytes to independently owned header
bytes path on the existing checked BuildInfoCallbackRecipe. Select
render_supplied_bytes(&self, status_bytes: &[u8], template_bytes: &[u8]) ->
Result<Vec<u8>, &'static str>. Saved WP905 has four demanded TranslateBuildInfo rows;
accepted supplied status parsing and string rendering leave template decoding and
output byte conversion as concrete missing prerequisites.

Use immutable Bazel9.2 commit8220c6198837d5c13d53fea211cf3282aa12408a:
WorkspaceStatusAction126–141 parses Latin1 status, BuildInfoFileWriteAction99–184
selects status/callback/template, Template.java107 reads ISO8859_1 and
LocalTemplateExpansionStrategy46–51/86–93 encodes ISO8859_1 after ordered literal
passes. Source pins/full anchors: target/wp923/successor-source.md. Existing builtin
callback source/digest/callsite authentication remains unchanged. Exact byte conversion
and finite callback pipeline apply to supplied bytes. Labels/configuration/path and
structural action identity remain Slug-native; this does not authorize artifact IO,
status production, source certificates, volatile execution/cache or action execution.

## Owner and allocation contract

Reuse WorkspaceStatusArtifacts::parse_supplied_values and render_template. Promote
the existing private Latin1 decoder only to pub(crate) for the sibling checked recipe
owner; add no general public decoder/encoder, module/corpus or donor dependency.
Every input byte decodes to U+0000..U+00FF, and existing callbacks supply only selected
map values or ASCII defaults. Template replacement stays in that closed domain, so
output characters map back exactly to bytes. Preserve missing timestamp's exact error
for empty/no-placeholder templates. If source validation-before-template handling
requires factoring, use one private borrowed substitution selector shared by both
entrypoints rather than duplicating callback rules. Freeze concrete algorithm and
lifetimes in target/wp924/contract.md with three DESIGN reviews before editing.

Choose standard character-to-byte collection into owned Vec, with a checked private
Latin1 closure invariant; no unchecked truncation or arbitrary native-Unicode encoder
with guessed replacement policy. Review the parsed compact map, decoded template,
replacement intermediates and final String/Vec overlap; drop map/template/output text
at their final use. Reuse compact owners and borrow three pairs, without new retained
cache/pool/interner/key/graph/DICE/locking or global scratch interning. Capacity growth,
transient String-plus-Vec storage and ordinary allocation failures remain explicit;
no one-allocation, zero-copy, performance or whole-memory-fit claim. Existing scoped
producer interning remains intact. Avoid an independent three-pass byte engine.

Allowlist: actions/build_info_transform.rs and its existing tests.rs;
workspace_status.rs decoder visibility only. Optional small checks in existing
Analysis compile_context/provider/build_info_transform.rs may use actual retained
installed recipes and catalog templates by reference. No Loading/Core/query/identity/
source staging/builtin/corpus/CLI API or runtime edits. Root owns production/glue/
compilation/integration/scheduling/commit; independent source/ownership/evidence
DESIGN and FINAL reviews. Production estimate<100, tests<180; inspect responsibility
growth before expanding. Operations target/wp924; serial shared Cargo target.

## Evidence and validation

Reuse all accepted source/parser/renderer guards. Independent complete ASCII headers
and high-byte status values must match exact Vec bytes, including NUL/CR/empty values,
duplicate and malformed lines, valid/invalid UTF8 interpreted bytewise. All256 supplied
template bytes roundtrip without recognized placeholders. Include a byte-level
same-key/cross-boundary/later-pass discriminator and missing versus empty timestamp.
Check repeat equality, immutable inputs/recipe and output after releasing owners.
Real installed recipe calls, if added, prove retained recipe provenance with supplied
content, without claiming observed artifacts or action execution.

Resolve pinned nightly tools; owner offlineJSON no-run and default CLI check <=60s
TERM3, jobs1, serial. If Analysis checks change, use established invocation-only four-
package opt0/debug0 owning starlark_rule preparation. Batch exact nonignored preflight
before serial10s TERM3 runtimes, protected parser/renderer/status controls plus new
byte controls. Format/diff/archive/status, compact receipt and local root commit.
No network/external oracle/source16/17/18 retry. Invocation and test corrections stay
in packet; REPLAN only for observed source/ownership/byte-domain or actual artifact/
execution prerequisites beyond this pure boundary.

Original memory refusals/resource caps persist. Actual status producer/update policy,
tracked template reads/materialization/source certificates, volatile cache/execution,
general callbacks, REAPI, Windows breadth, authentic full-root/M7A and aggregate
memory fit remain open; complete supplied byte rendering is one prerequisite.

## Immediate predecessor

[WP923](wp923-supplied-workspace-status-values-2026-10-06.json) supplies exact Latin1
LF/first-space/last-value parsing beside existing workspace-status descriptors. Lazy
borrowed slices decode into compact owned pairs and transfer into the existing
immutable map, without whole-file decoded scratch or another pair Vec. Original
status/renderer guards, decoder/parser/release/composition controls, default CLI and
three FINAL reviews establish the supplied-byte leaf; artifact/runtime gates persist.
