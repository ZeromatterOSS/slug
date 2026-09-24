# Current Slug V2 Work Packet

Packet: WP-7-96-m7a-selected-rust-host-tools-archive-effects-r1
Status: ACCEPTED 2026-09-24; M7A remains partial

## Outcome and compatibility

Admit the source-selected `rust_host_tools` generated repository's six ordered,
SHA-pinned `repository_ctx.download_and_extract` effects on Linux x86_64 and
materialize their merged `.tar.xz` contents before its final `ctx.file` calls.
This is the independent generated owner demanded by pinned rules_rust 0.73;
production-root `slug_crates` Label and rules_shell `which` prerequisites are
separate stops. No root-build success is claimed by this packet.

Exact Bazel 9.2 is claimed for the six selected URL/SHA/stripPrefix calls,
ordered regular-file bytes and executable modes, and last-writer merge only
after the audited six-archive source set and discriminating Bazel oracle or
pinned-source regression. Retained structural plan identity, observation,
capture limits, cancellation, and the selected rule's legacy reproducibility
metadata return are Slug-native. Bazel 9.2 uses opaque `ctx.repo_metadata`;
Slug validates and discards the pinned 18-key fallback dictionary without
claiming Bazel metadata parity. Unsupported/deferred:
unpinned downloads, nonempty auth/netrc, product URLs other than the six
selected static.rust-lang.org objects, other formats/options,
links, special entries, unmodeled archive-path observations, malformed or
unaudited archives, and broader repository-rule use. Never read a netrc or
forward credentials. If exact extraction evidence remains incomplete, narrow
the claim and REPLAN before landing a success path.

## Source, owner and decision

Pinned rules_rust `rust/private/repositories.bzl:397-462` invokes rustc,
clippy, cargo, rustfmt, llvm-tools, rust-std in that order; rustfmt is nightly
2026-07-16, the others are Rust 1.97.1. `repository_utils.bzl:845-1009`
passes `urls`, a `known_shas.bzl` SHA-256, empty auth here, and a
`<archive-stem>/<component>` stripPrefix, leaving output and type at Bazel's
defaults. Pinned Bazel 9.2 `StarlarkBaseExternalContext.java:541-565,1070-1172`,
`TarXzFunction.java`, and `CompressedTarFunction.java:75-158` establish
result binding, tar order, prefix filtering, regular-file overwrite, mode
(`archive mode | 0400`), and mtime. Bazel's symlink/hardlink handling is not
admitted. The six selected archives were audited: 168,875,656 compressed bytes
total (largest call 81,050,916), 859,069,440 full decoded TAR bytes
(largest call 407,661,056),
858,348,847 stripped regular payload bytes, 380 physical headers/296 logical
entries, largest regular entry 199,494,192 bytes, longest path 167 bytes,
only regular files/directories
with modes 0644/0755 and no links/PAX. GNU longname headers occur in three
archives. `manifest.in` is the one cross-archive stripped path collision,
present in all six; no file/directory collision or within-archive duplicate.
A focused Bazel 9.2 local tar.xz merge oracle must cover ordering, overwrite,
prefix and executable mode without relying on a huge network run. Accepted
BCR SHA capture/strict TAR tests may be reused, but are not evidence for
xz/ordered merge. No V1/Buck2 donor is a semantic authority; reuse current
bounded transport/parser concepts, not a side cache or shell extractor.

The selected `HostSelectedRepositoryFileEffectObservationKey` remains the
DICE producer; its owner certificate, full authenticated repository input,
observed source/environment/path cells and an ordered structural effect plan
determine the value. Admit the archive method only when the authenticated
rule's defining label is `@@rules_rust+//rust/private:repositories.bzl`,
exported name is `rust_toolchain_tools_repository`, and generated name is
`rust_host_tools`. The selected DICE effect producer validates all six exact
ordered URL/SHA/stripPrefix tuples from the audited receipt below, with no
missing/extra archive calls, before publishing its result; changed tool
versions or mirrors fail closed until reaudited. Other repository rules fail
closed even for the same call shape. Local `file://` objects are a
checksum-pinned unit/Core oracle carrier: the production selected DICE owner
rejects them before publication. Extend the current
`GeneratedRepositoryFileEffectPlan`
with archive intents (ordered URLs, SHA, stripPrefix, format) interleaved with
file writes, preserving existing file-only API behavior. No captured bytes or
temp root enter DICE. Source identity for the materialized root frames every
ordered effect, including archive URL order/SHA/prefix and file path/bytes/mode,
in a domain separate from repository, Bazel and REAPI hashes. The Core
materializer alone owns verified capture and physical staging. Keep it in a
fresh temporary root, apply effects sequentially, publish only after all
captures, extraction and final files succeed; abort/cancellation drops staging.
Never hold a lock across DICE compute. Changed SHA, URL, prefix, order,
contents, source, environment or owner must invalidate/retry via the current
observation and materialization flow; A/B/A restoration must preserve identity.

Choose an in-process xz decoder and a bounded TAR reader. Reuse the existing
streamed SHA-verified temp capture and parser safety checks, parameterizing
limits only for this audited selected class; do not loosen BCR's policy.
Cap XZ decoder memory at 136 MiB: the audited maximum is 134,283,320 bytes
for rustc, llvm-tools and rust-std, just above 128 MiB.
Reject path escape, links/special nodes, unsupported modes or ambiguous
file/dir collisions before a root is published. Preserve ordered
archive-to-archive regular-file replacement and final `ctx.file` overwrite
of an archive file; repeated `ctx.file` paths remain unsupported. A generated-path
`exists`/read that would require extracted state must fail closed rather than
return a fabricated absence; this selected owner rejects unknown generated
path construction and child paths before an archive as well, preventing a
precreated handle or `hasattr` bypass. The source-selected rule must reach all six calls
and its final BUILD/WORKSPACE files; unrelated rules remain unsupported.

The source-authentic direct smoke uses root `use_repo_rule` with the pinned
`@rules_rust//rust/private:repositories.bzl` export, so it exercises the same
repository-rule implementation and selected DICE effect/materializer but has
an innate owner. The original rules_rust `rust_host_tools` module-extension
owner remains stopped earlier by unadmitted `os_dependent`/`arch_dependent`
factors. A later generated BUILD Loading stop is not archive-effect failure.

Audited pinned archive receipt (order; SHA-256; compressed/full decoded TAR
bytes; physical headers):

| Component | SHA-256 | Compressed | Decoded | Headers |
|---|---|---:|---:|---:|
| rustc | `9819d0a32d56bd339585319c80260e332779f5541fd66838ab7e016d6c814819` | 81,050,916 | 407,661,056 | 71 |
| clippy | `3441df8fb54db985f8c8a3e8356b8874a3f92cc8cca8565cfe36f1dc15935e72` | 4,935,556 | 23,790,080 | 22 |
| cargo | `e1be5f5ff7f7f80ca506fb65770b759edbdc6d303781ed71c5de8ec8a8394779` | 11,150,208 | 43,520,512 | 68 |
| rustfmt | `bb48a8124f84818dab540701952e4cf5506045000baa975b6046d54aeac87d5d` | 2,473,860 | 7,398,400 | 22 |
| llvm-tools | `3e7a1c596a42dea6bf625ec6f006ce2fbcf5d1ff892f082828eb182a5d483b95` | 39,156,716 | 207,548,416 | 51 |
| rust-std | `1c1e704ae80126b7de34f72ea2825f7fd01736dec20732faed47374b95282fba` | 30,108,400 | 169,150,976 | 146 |

## Scope, resources and stops

Base clean main at 2cf074303. Allowlist: this manifest, canonical Current
paragraph and Stage 5/9 only for accepted owner/reuse decisions;
`app/slug_bzlmod_v2/src/generated_repository_file_effect.rs` and lib export;
`app/slug_loading_v2/src/repository_rule_context.rs`,
`module_extension_repository_file_effect.rs`,
`registration_diagnostic.rs` for exhaustive typed archive-plan diagnostics,
and colocated tests;
`app/slug_core_v2/src/runtime/repository_archive_http.rs`,
`repository_archive_realize.rs` or a cohesive sibling selected-effect
extractor, `repository_io.rs`, direct tests and module registration;
`app/slug_core_v2/Cargo.toml`, Cargo.lock and Cargo.Bazel.lock for xz2.
No MODULE/lockfile-auth, Bazel vendoring, NativeLink, BuildBuddy configuration,
external credentials, generic repository downloads, cross-owner Label or
rules_shell changes. Existing Core and Loading files above 2,000 lines are
cohesive owners; put a substantial decoder in a sibling rather than expanding
their existing giant methods. REPLAN if selected source includes unsupported
entry/collision semantics, needs auth/secret access, a new semantic owner, or
requires broader URL/method options. Do not silently widen caps to accommodate
an unknown object.

Retained memory is a compact ordered archive intent plus existing file bytes
in DICE; `Arc`/compact strings/SmallMap patterns suffice, no new interner or
retained artifact cache. A verified compressed archive occupies at most one
transfer-owned temp file per sequential effect (128 MiB existing capture cap);
stream decoded bytes directly into the materialization root with explicit
audited caps: 512 MiB full decoded TAR stream per call, 256 MiB per regular
entry, 1 GiB full decoded stream across six, 1024 physical headers per call,
256-byte
paths and 32 path components. Reject duplicate stripped destinations within
one archive but permit audited cross-archive regular-file replacement.
Per-effect scratch
and staging release on success/error/cancellation; request/source arcs release
under normal DICE invalidation/eviction. No performance claim.

## Validation

Independent DESIGN review before Rust edits and FINAL review of the actual
diff. Record six SHA hashes and per-archive header counts with the audit;
compare against the source-selected objects. Add a small fixture under
Stage 1 provenance only for a demonstrated oracle gap: local `file://`
tar.xz with two ordered archives, same-path overwrite, stripPrefix filtering,
executable mode and GNU longname, plus a negative SHA case. Compare Bazel 9.2
and the Slug Core materializer output bytes/modes and SHA failure class.
The production DICE owner admits only the six source-derived tuples.
Path escape, link and
special entries receive Slug-only fail-closed tests and pinned-source
classification, not an exact diagnostic claim. Focused tests cover structural
order/identity, result return, Need/error/cancel, capture SHA/cap, xz decode
limits, path confinement, archive+file overwrites, atomic publication and
cold/warm/A/B/A reuse. Run direct Bzlmod, Loading, Core and CLI compile
dependents with pinned Rust toolchain; JSON `--no-run` and exact-selector
preflight, serial Cargo, fresh `slug_cli_v2` build before smokes, and
stale-slugd cleanup before/after. A direct selected-owner smoke must reach
the archive behavior; production-root earlier stops are recorded, not
bypassed. Run format, diff, archive and plan status checks. Normal Bazel RC
discovery may pick up BuildBuddy auth; never inspect or print `~/.bazelrc`
or netrc material.

## Acceptance receipt

Independent DESIGN and post-correction FINAL reviews returned ACCEPT. The
Bazel 9.2 local tar.xz oracle covered two ordered extracts, shared-path
last-writer bytes, final `ctx.file` overwrite, stripPrefix, executable mode,
GNU longname and wrong-SHA failure. The six pinned archives' source hashes,
sizes, modes, entry types and `manifest.in` collision were audited above.
Pinned-toolchain JSON no-run compile succeeded for Bzlmod, Loading and Core;
exact nonignored selector preflight preceded 5/5 Bzlmod, 5/5 Loading and 8/8
Core focused tests. A fresh `slug_cli_v2` build succeeded. `cargo fmt --check`,
`git diff --check`, Cargo/Bazel lock coherence and plan/archive checks passed.
No broad suite or root build is claimed. The packet ran from the 03:20 UTC
WP795 base through the 04:22 UTC final checks, about 62 minutes elapsed;
focused compile/rebuild calls were serial, with the longest JSON no-run about
50 seconds and the first CLI rebuild about 26 seconds. No speedup claim.

The authentic innate `use_repo_rule` fixture at
`/tmp/slug-wp796-direct-ZUzVoG` uses the pinned rules_rust export and Linux
x86_64 Rust 1.97.1/nightly rustfmt values. Bazel 9.2 `build //:probe`
succeeded. Fresh Slug `build //:probe`, with HOME/NETRC unset and no `slugd`,
completed archive materialization and then failed while Loading the generated
`BUILD.bazel:131`: `rust_toolchain(iso_date=None)` was rejected as a label
attribute. The original module-extension fixture separately stops before this
owner on unadmitted OS/architecture-dependent factors. These are next-packet
stops, not WP796 archive parity or root-bootstrap success. No credential file
was inspected, copied or committed.

## Immediate predecessor

WP795 at 2cf074303 accepted bounded selected absolute-host-path `exists`
with DICE-observed environment/path; production root still stops at generated
`slug_crates` Label and toolchain fixture at independent rules_shell `which`.
