# Slug V2 current work packet

Packet: WP-7-222-m7a-supplied-workspace-status-values-r1
Status: Finite BuildInfo rendering accepted; supplied-byte status parser selected

## Observable result and source basis

Add a pure supplied-byte workspace-status parser beside the existing BuildAPI
workspace-status owner. It returns independently owned CanonicalStringMap values
that can feed the accepted checked recipe renderer. Saved WP905 has four demanded
TranslateBuildInfo rows; raw status parsing is a named remaining prerequisite in
bootstrap-readiness.md. This packet removes only that pure parsing prerequisite.

Pinned Bazel9.2 commit8220c6198837d5c13d53fea211cf3282aa12408a,
WorkspaceStatusAction.java126–141 parseValues, reads Latin1, splits only LF,
splits each line at its first ASCII space, ignores lines without that separator,
and retains the last value for duplicate keys. Empty keys/values are permitted;
extra spaces, CR and NUL are retained, without validation or trimming. Source
SHA2562e05526a947d4643a5e9b5cc45ee5c938ecd6fc133941e87a02147a2953a9db8
and bounded discovery at target/wp922/successor-source.md anchor the algorithm.
Every byte maps to U+0000..U+00FF; even valid UTF8 bytes decode individually.
Exact key/value parsing and Latin1 mapping apply to supplied bytes. Deterministic
map ordering and existing structural identity remain Slug-native; this is not
filesystem observation, status production or original header output-byte parity.

## Owner, allocation and scope

Select WorkspaceStatusArtifacts::parse_supplied_values(bytes: &[u8]) ->
CanonicalStringMap as a role-independent associated pure function. A method with
no descriptor or workspace access cannot assert observation authority. Keep full
workspace/role/constant-metadata identity and every execution guard unchanged.
No DICE keys, observations, source certificates, retained cache/interner/graph,
locking, filesystem, clock, user/host lookup or status command is introduced.

Split borrowed byte slices before decoding. Reuse existing compact strings and
CanonicalStringMap::from_pairs, whose exact sorted keys and last-wins insertion
are already implemented. Stream pairs into that constructor, avoiding an extra
pair Vec or whole-file decoded String. Decode each surviving segment directly
into its final CompactString: ASCII from borrowed str after ASCII check, high
bytes via byte-to-char conversion into one owned compact buffer, then transfer
storage to the map. No from_utf8_lossy or UTF8 interpretation of high bytes.
Review peak temporary buffers, duplicate replacement release, inline versus heap
storage and returned-map ownership before implementation. Unique input scratch
must not be globally interned; existing scoped producer interners remain intact.
No one-allocation, speedup or whole-workload memory-fit claim follows from this leaf.

Allowlist: app/slug_build_api_v2/src/workspace_status.rs and its existing
tests.rs. A small composition assertion stays in that test owner using existing
checked recipes and supplied ASCII template; no new corpus or fixture harness.
Production estimate<80 lines, new tests<180 lines; inspect coherent growth.
Root owns API/implementation/compile/integration/commit. Three independent
source, ownership/allocation and evidence DESIGN reviews precede edits, then
FINAL reviews use actual tests. Operations: target/wp923. Reuse orchestration
and utility skills; no donor copy/new utility dependency or Stage9 import.

## Discriminators and validation

Protect all three existing status identity/metadata/projection/nonUnicode/runfiles
controls. New full-map checks cover missing space/empty bytes, empty key/value,
first-space versus subsequent spaces, LF/CRLF/no finalLF, duplicate valid/malformed
lines and exact last value. Check the private decoder against an independent all256-byte Latin1 vector,
and distinguish parser LF/space boundaries from retained NUL/high bytes, invalidUTF8
and validUTF8 bytewise decoding. Assert
input immutability, repeat equality and map use after input release. Compose
supplied parsing with existing Info/Version rendering for full independently
expected strings and missing versus present-empty timestamp, without promoting
supplied bytes into authentic artifact or template observations.

Resolve pinned nightly tools; offlineJSON owner no-run preparation <=60s TERM3,
jobs1 shared target serial. Compile default CLI for public production change.
Fresh exact nonignored preflight precedes serial10s TERM3 owner runtime including
protected status/renderer controls; no broad suites or daemon-sensitive fixtures.
Format, diff/archive/status checks, compact receipt and root local commit.
Invocation corrections stay in this packet. REPLAN only if exact parser semantics
require an unmodeled owner/retained representation or artifact/execution authority.

Raw status production, Latin1 template decode/output encoding, tracked template
reads/materialization, stable/volatile update/cache policy, arbitrary callback
execution, REAPI, Windows breadth, authentic source/full-root/M7A and aggregate
memory fit remain open. Source16/17/18 attempts and prior memory/resource caps persist.

## Immediate predecessor

[WP922](wp922-build-info-recipe-render-2026-10-06.json) adds render_template to the
existing checked recipe: three ordered borrowed pairs, literal replacement and
independent output with exact defaults/timestamp error. Five BuildAPI and three
actual installed-rule Analysis controls pass, default CLI compiles and three FINAL
reviews accept. Matching passes may transiently hold old/new buffers; no retained
storage or pool is added. Supplied Unicode rendering keeps artifact/runtime gates open.
