# Current Slug V2 Packet

Packet: `WP-5-7A-selected-bcr-archive-transform-implementation-r2`

Milestone: M7A category 6 registered-toolchain repository prerequisite.

Base: accepted ordered source/RepoSpec identity `01f2802f0`, independently
accepted architecture `2a7c9436e`, accepted exec-configured loading
implementation `831e574e6`, accepted canonical repository Host capabilities
`26a68d61c`, and parked proof-only registration base `20ad71ffa`. The passing
four-row proof draft and retained selected-context R2 candidate remain dirty
and are read-only during implementation.

## Why this implementation is active

The ordered producer/publication-identity successor is terminally accepted in
`01f2802f0`. It retains source JSON patch/overlay order, keeps ordinary map
equality unchanged, and makes exact `http_archive`/`git_repository`
`remote_patches` order structural through both route families and the actual
materialization-request DICE cutoff.

The real command/REAPI dependent now clears launcher row 1 and stops at row 2
while loading `@@bazel_tools//tools/test:BUILD`'s ordinary
`@rules_shell//shell:sh_binary.bzl` dependency. The selected BCR RepoSpec for
`rules_shell@0.6.1` lawfully omits `type`, supplies
`strip_prefix = "rules_shell-0.6.1"`, one authenticated remote patch and
`remote_patch_strip = 1`. Its authenticated tar also uses 0664 regular-file,
0775 executable-file and 0775 directory modes. Slug currently requires a
ten-key shape including `type`, empty prefix/patch/overlay maps, patch strip
zero and only 0644/0755 entries.

This is a generic selected-BCR archive transform boundary, not a rules_shell,
registration, parser, Starlark builtin, `cc_common`, action or REAPI defect.
The active packet implements the already frozen complete category owner.

R1 implemented that owner and passed every hermetic archive row, but the first
fresh-root real REAPI replay exposed one incorrect source fact: the
authenticated rules_shell archive begins with a global PAX header before its
regular-file/directory entries. Physical block 0 has typeflag `g`; its sole
well-formed record is the semantic-inert `comment` key carrying the release
commit. Bazel accepts it through its generic tar reader, while R1 rejects every
PAX header. Independent terminal review therefore returned `REPLAN`. R2 adds
only the bounded generic metadata category required by that evidence.

## Learned facts and authority

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole semantic authority:

- `IndexRegistry.ArchiveSourceJson`, `createArchiveRepoSpec()` and
  `ArchiveRepoSpecBuilder` omit `type` when `archive_type` is absent and retain
  registry patch/overlay rows;
- `IndexRegistryTest.testGetArchiveRepoSpec` and
  `testArchiveWithExplicitType` discriminate absent versus explicit archive
  type, strip prefix, patches, overlays and remote MODULE projection;
- `DecompressorValue`, `CompressedTarFunction` and `StripPrefixedPath` infer
  `.tar.gz`/`.tgz`, consume PAX through `TarArchiveInputStream`, discard paths
  outside a prefix, fail when the prefix is not found and make extracted
  regular files user-readable; `CompressedTarFunction` does not inspect PAX
  `comment` metadata when projecting filesystem output;
- `http.bzl::_http_archive_impl` and `utils.bzl::{download_remote_files,patch}`
  extract first, place authenticated overlays, apply authenticated remote
  patches in map order, then replace `MODULE.bazel` with the authenticated
  registry copy; and
- `PatchUtil` plus `PatchUtilTest` are authority for native patch behavior.

The Bazel 9.2 registry specification states that `patches` are applied in
source JSON order. Slug's `SourceJson` currently converts both `patches` and
`overlay` through `BTreeMap`, silently sorting that semantic order before the
retained RepoSpec. The producer must correct this before realization consumes
the maps. Buck2-derived `SmallMap` deliberately compares maps by membership,
not iteration order, so insertion-order retention alone also cannot make a
reordered patch program structurally distinct at DICE cutoffs. The exact BCR
`rules_shell/0.6.1/source.json`, patch, MODULE file and release archive are the
real downstream evidence; no semantic stub is lawful.

The same SHA-256-authenticated release stream starts with a 512-byte global
PAX header and one 512-byte payload block. The payload is exactly one POSIX PAX
record, `52 comment=e071f45e209f3e154210faed3d0e60c29aef3b4a\n`;
the first archive directory begins at physical block 2. This is source evidence
for a generic inert-metadata parser, not permission to compare the comment
value or repository name.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is concept/test guidance only. Its `registry_source_json.zig` keeps patch rows
ordered and scratch-owned; `archive_source_realization.zig` keeps verified
downloads, transforms and inventory in one private physical phase before
publication; and `patch.zig` isolates patch parsing. Slug adopts those owner
separations, not Zabel's Zig code, scheduler, caches, layouts or compatibility
claims.

## Compatibility classification

- **Exact:** absent/explicit tar-gzip type selection for the admitted source;
  source-order retained patch/overlay projection; HTTPS/SHA-256 SRI
  authentication; normalized `strip_prefix`; extraction of the admitted
  regular-file/directory tar subset plus well-formed global PAX records whose
  only key is semantic-inert `comment`; regular-file read/execute bits;
  overlay before ordered patch before registry-MODULE replacement; final
  `rules_shell@0.6.1` bytes required by loading; and the successful four-row
  registration continuation.
- **Slug-native:** Rust valid-Unicode path restriction, bounded transport/tar/
  patch ceilings, strict path-containment diagnostics, directory metadata and
  timestamps, collision-safe transform/source association, sequential capture
  scheduling, and TempDir/cancellation publication mechanics.
- **Unsupported/deferred:** archive formats other than tar.gz/tgz; local PAX,
  malformed global PAX, every global PAX key other than `comment`, symlink,
  hardlink, device, sparse and non-Unicode archive entries; patch
  creation/deletion/rename/mode-only/binary/fuzzy-context and malformed-UTF-8
  shapes; local patches, patch commands, auth/netrc, user `files` symlink
  overlays and broader `http_archive` attributes. Every unadmitted shape fails
  closed rather than being skipped or approximated.

BCR Starlark continues to own all rule/macro control flow, including
`cc_internal`. `cc_common` remains only a generic Host/provider ABI consumer.
This packet adds no Rust rule implementation and is unrelated to `set`.

## Frozen architecture

### Producer projection

`slug_bzlmod_v2::selected_repo_spec` remains the sole selected-registry
`source.json` -> RepoSpec producer. Parse patch and overlay objects into
source-order scratch rows without enabling global `serde_json/preserve_order`.
Project those rows into the existing insertion-ordered nested `SmallMap`s. The
scratch vectors die after RepoSpec projection; core must never guess order from
sorted keys.

Insertion order is physical representation, not equality. Freeze one separate
`RepoSpec` publication-identity domain for repository attributes whose order
changes repository semantics. Its first admitted member is `remote_patches`
for the exact Bazel `http_archive` and `git_repository` rule identities. Normal
RepoSpec equality continues to compare every attribute and nested map by
ordinary membership, then additionally compares the ordered key sequence for
that admitted semantic map. Because JSON objects have unique keys and ordinary
equality already compares each key/value pair, the added key sequence is a
collision-safe structural identity for the patch program; no digest stands in
for equality. RepoSpec hashing owners add the same ordered key sequence after
their existing order-insensitive complete-content hash.

This changes neither `SmallMap` nor Starlark dict equality. It also adds no
second retained side table: the publication identity is a typed view over the
producer-retained RepoSpec map. Every existing selected-spec, selected-route,
root/canonical capability and `RepositoryMaterializationRequest` equality
boundary already carries RepoSpec, so the strengthened RepoSpec identity flows
through all of them without a new parallel route field. The implementation
must prove that no route projects only the membership identity and that the
actual materialization-request boundary observes reordered same-entry patches
as A != B and restored A == A.

### Plan and realization

`slug_core_v2::runtime::repository_archive` owns a phase-scratch
`SelectedBcrArchive` plan. It accepts the exact nine mandatory BCR fields plus
optional `type`, represents format with an extensible enum whose only admitted
variant is tar-gzip, and retains normalized prefix, source-order patch rows,
overlay rows, patch strip and registry MODULE input. An absent type is inferred
only when every admitted source URL selects tar.gz/tgz; incompatible or
ambiguous mirrors fail closed.

Each patch retains one HTTPS URL and SHA-256 SRI. Each overlay retains its safe
relative destination, nonempty HTTPS mirror list and SHA-256 SRI. Map key sets
must match, duplicate/unsafe paths fail before I/O, and patch strip is bounded
and nonnegative. No path or semantic field is recovered from a canonical
repository name.

The existing direct HTTP/1 lifecycle owner captures archive, overlay, patch
and MODULE payloads into verified transfer-owned temporary files with
subject-specific ceilings. Realization creates one private TempDir and performs
this exact phase order:

1. stream-extract tar-gzip while applying the normalized strip prefix;
2. place verified overlays with overwrite semantics and executable mode;
3. apply verified remote patches in retained source order and the shared strip;
4. replace `MODULE.bazel` with the verified registry copy; and
5. return the private root only after the session remains active.

The tar reader consumes any number allowed by the existing physical-header
limit of global PAX payloads, each capped at 64 KiB. Each payload must be
UTF-8, NUL-free,
newline-terminated POSIX length-framed records, and every record key must be
exactly `comment`. Record lengths are checked against consumed bytes, and PAX
payload reads use the same decompressed-byte and cancellation owner. Local PAX,
malformed framing, or a semantic/unknown key fails before publication. The
comment value is neither retained nor compared: the verified archive digest
already authenticates every header/payload byte, while Bazel filesystem output
does not depend on this extra metadata.

Patch parsing lives in a new private `repository_archive_patch.rs`, not in the
already multi-responsibility archive/HTTP files. The admitted parser supports
UTF-8 LF unified-diff modifications of existing regular files, multiple files
and hunks, safe equal old/new paths after stripping and exact source context.
Every other PatchUtil shape above is an explicit unsupported terminal. This
keeps the owner extensible without claiming unimplemented Bazel breadth.

The source association advances to a new domain and length-frames format,
prefix, archive digest, ordered patch digests, patch strip, ordered overlay
destination/digests and registry MODULE digest. Mirror URLs do not change root
semantics when their verified bytes are identical, but remain in the complete
RepoSpec/materialization request identity. No lock crosses I/O or DICE
compute. Any capture, transform, cancellation or validation failure drops all
captures and the unpublished private root.

## Immediate predecessor accepted

Commit `01f2802f0` accepts the ordered producer, narrow RepoSpec publication
identity, both complete hash owners and route/request/DICE A/B/A proof under
161 production, 344 proof and 505 aggregate additions. Ordinary
`SmallMap`/Starlark equality and global serde ordering remain unchanged.

## R1 terminal stop and active R2 implementation

R1's focused archive suite passed 31 rows with one disposable artifact row
ignored; its full core run passed 297 rows with the same three independently
reproduced dirty route/event failures and one ignore. Producer/request identity
and the parked four-row proof passed. The real fresh-root REAPI dependent then
advanced into authentic rules_shell realization and failed only at the leading
global PAX header. Independent terminal review returned `REPLAN`: freeze the
generic inert global-comment category before resuming and do not add a
rules_shell byte special case.

Implement `WP-5-7A-selected-bcr-archive-transform-implementation-r2` against
these clean baseline blobs and the complete R1 candidate:

- `app/slug_core_v2/src/runtime/mod.rs`
  `de71925e40b73800c6b589526bddbad90c2e4c2e`;
- `app/slug_core_v2/src/runtime/repository_archive.rs`
  `179ec1e59375959f3f6bcb06cb2787af2db94530`;
- `app/slug_core_v2/src/runtime/repository_archive_http.rs`
  `bf310f3ccb75d80174849b9738bcc1192dc3f436`;
- `app/slug_core_v2/src/runtime/repository_archive_realize.rs`
  `d1d1d1400a7208faccf49586c7b36b780ddbb00e`;
- new `app/slug_core_v2/src/runtime/repository_archive_patch.rs`, which must be
  absent at activation;
- `app/slug_core_v2/src/runtime/tests/repository_archive_tests.rs`
  `fc42cc3ed6cb179b88a7054942231d76b63ddd8d`;
- `app/slug_core_v2/src/runtime/tests/repository_archive_http_tests.rs`
  `c158b34323320396a22b11f5c7873448293c8bb5`;
- `app/slug_core_v2/src/runtime/tests/repository_archive_realize_tests.rs`
  `cbc1a6b51ea1d5dc15c7efb6ce24f2799214b1b8`; and
- new `app/slug_core_v2/src/runtime/tests/repository_archive_patch_tests.rs`,
  which must be absent at activation.

Cap additions at 900 production, 1,100 proof and 2,000 aggregate Rust lines.
No Cargo/dependency, bzlmod, loading, analysis, registration, command, REAPI,
fixture, selected-context or other file may change. Existing production files
are under 2,000 lines but mix plan/transport/extraction concerns; the new patch
module is the bounded split. `selected_repo_spec.rs` exceeds the guide trigger,
but successor 1 remains cohesive because it changes only its private parser ->
RepoSpec projection and adds no new key or public surface.

Proof must discriminate nine/ten-field plans, inferred/explicit tgz,
ambiguous/unsupported formats, safe/unsafe prefix and overlay paths, ordered
patches, SRI and field-shape failures; exact prefix discard/not-found behavior;
0664/0775 rules_shell-shaped modes; overlay-before-patch-before-MODULE bytes;
multiple patches/files/hunks; unsupported patch shapes; transform-aware source
association; cancellation between every phase; and capture/private-root
cleanup on each failure. It must additionally prove well-formed global
`comment` records are consumed without output changes; multiple bounded
records; malformed length/framing/UTF-8/NUL rejection; local PAX rejection;
semantic and unknown global-key rejection; and cancellation during PAX payload
consumption. Use generated hermetic tar/capture inputs plus the
exact small rules_shell source/patch bytes already authenticated above; add no
new oracle fixture.

Run focused archive plan/HTTP/realization/patch tests, full serial
`slug_core_v2`, the selected source/materialization direct dependents, the
parked registration-row test, and the previously red REAPI test twice from
fresh test roots. Then run `cargo fmt --all`, `git diff --check`, scope/blob/
cap/dirty-isolation audits and `scripts/v2_archive_status.sh`. Rebuild
`slug_cli_v2` first if an oracle invokes `SLUG_V2_BIN`; clean stale `slugd`
before and after daemon-sensitive tests.

## Design stops and successor

The first independent design review returned `REPLAN` because insertion order
was not structural identity. The corrected narrow RepoSpec publication domain,
complete carrier trace, request-boundary A/B/A proof and archive lifecycle then
received independent architecture/identity/lifecycle `ACCEPT`; the identity
successor is now accepted. This implementation changes only the frozen
nine-file core allowlist.

`REPLAN` for a rules_shell/BCR comment-value special case; accepting local,
malformed, semantic or unknown PAX; a patch skip because the
registry MODULE later overwrites the file; lost source order; format inference
from content or repository name; unverified transform input; identity omitting
a semantic transform; external `patch`/shell/JVM execution; global cache or
background task; partial root publication; owner/Cargo/file/cap widening; or a
real rules_shell tar/patch shape outside the admitted subset.

Implement only the frozen active R2 allowlist.
After this packet receives terminal `ACCEPT`, resume the unchanged proof-only
four-registration-row closure. Only after that proof and its command/
REAPI dependents pass may the retained selected-context R2 candidate return to
terminal review, followed by M8 bootstrap work.
