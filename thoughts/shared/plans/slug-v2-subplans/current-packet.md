# Current Slug V2 Packet

Packet: `WP-0-4-5-slug-starlark-archive-whitelist-correction`

Milestone: M0 archive/baseline health required by the accepted shared Stage 4/5
Starlark owner.

Result: teach the clean-root archive checker that the newly accepted
`app/slug_starlark_v2/**` crate is a V2 app path. Change one shell pathspec and
no Rust, semantics or compatibility behavior.

## Learned facts and decision

Commit `cb71a302d` correctly integrated the accepted exact universal Starlark
owner, but its 16-path implementation allowlist did not include
`scripts/v2_archive_status.sh`. The checker therefore reports the three new
crate files as non-V2 even though the canonical plan, Cargo workspace and
accepted packet identify them as V2. The checker's three longstanding
non-V2-thoughts rows remain unchanged baseline exceptions.

Do not hide this failure inside the proof-only compilation-helper retry. Run
only `WP-0-4-5-slug-starlark-archive-whitelist-correction`, restore the archive
gate to its prior baseline, then select the unchanged complete-helper retry.

## Authorities, ownership and compatibility

The accepted `cb71a302d` crate paths and the clean-root checker's existing
explicit V2-app pathspec list are the sole authority for this maintenance
change. Bazel and Starlark behavior are untouched.

- **Exact:** the checker admits every tracked file beneath the accepted
  `app/slug_starlark_v2/**` crate and continues rejecting unlisted app paths.
- **Slug-native:** the repository-maintenance pathspec and checker wording.
- **Unsupported/deferred:** no compatibility surface changes; the three known
  thoughts-path baseline failures are not part of this packet.

Zabel is irrelevant to this checker correction. It remains peer architectural
guidance for the accepted universe design, not authority or source content.

## Allowlist, caps and proof

Change only `scripts/v2_archive_status.sh`. At base `cb71a302d` it is 220 lines,
SHA-256 `0f79ad9fe2deeb4d2b92f9397c217b9e2eab6ee4fe981fd8cc78c16c7c3f3ad3`,
with a 223-line ceiling. Caps are 0 production, 3 maintenance and 3 total added
lines; deletions do not buy budget.

Add only the exclusion pathspec `:!app/slug_starlark_v2/**` beside the other V2
app crates. Run `scripts/v2_archive_status.sh` and prove that the app-path gate
returns `OK` while only the unchanged three thoughts rows fail. Run shell syntax,
formatting/diff and clean-scope checks. No Cargo build is required because no
Rust or build metadata changes.

STOP and `REPLAN` for any Rust/build/doc change after this scheduling commit,
checker weakening, wildcard broader than the one accepted crate, suppression of
the thoughts baseline, copied Zabel content, allowlist escape or cap violation.

After acceptance, select only
`WP-4-7A-rules-cc-compilation-helper-complete-loading-proof-r2` with its
previously reviewed 0/1050/1050 proof boundary.

## Immediate predecessor

Commit `cb71a302d` accepts the exact shared universal environment and creates
`app/slug_starlark_v2`; it does not update the archive checker's explicit V2
app allowlist.
