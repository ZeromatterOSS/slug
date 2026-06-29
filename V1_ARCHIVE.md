# Slug V1 Archive

Slug V1 is preserved by Git ref, not by an in-tree source copy. This file
records the archive policy and current verification status.

As of the 2026-06-29 Stage 0 remediation, the local `slug-v1-archive`
annotated tag and `v1-archive` branch both peel to
`e218054d4c796655939b968d90208b185decb352`.

## Archive Record

| Field | Value |
|-------|-------|
| V1 commit | `e218054d4c796655939b968d90208b185decb352` |
| V1 tag | `slug-v1-archive` |
| V1 archive branch | `v1-archive` |
| Tag type | annotated |
| Physical archive directory | none by default |
| Dirty files intentionally excluded | archive action excluded active V2 remediation docs and prompt only; later root-metadata cleanup changed `Cargo.toml`; no V1 implementation files |
| Current verification status | verified locally on 2026-06-29; checker exits 0 with active V2 doc changes |

The archive tag and branch must both peel to the V1 commit above. Because the
tag is annotated, use `git rev-parse slug-v1-archive^{commit}` when comparing it
with the branch commit.

Current remediation rule: if `scripts/v2_archive_status.sh` reports missing or
mismatched archive refs in another checkout, repair the refs before any
clean-root V2 implementation continues.

## 2026-06-29 Dirty-State Triage

The archive refs were created while the `codex/slugv2` worktree had active V2
remediation documentation changes only:

- `V1_ARCHIVE.md`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/00-v1-archive-and-clean-root.md`
- `thoughts/shared/plans/slug-v2-subplans/09-v1-extraction-ledger.md`
- `thoughts/shared/prompts/2026-06-29-slug-v2-generic-implementer.md`

No V1 implementation files were dirty, and the archive refs were created at the
recorded V1 commit rather than at the mixed-root `codex/slugv2` head.

## Inspecting V1

```bash
git show --stat slug-v1-archive
git switch v1-archive
git switch main
```

To inspect a single V1 file without leaving the active V2 branch:

```bash
git show slug-v1-archive:path/to/file
```

## Policy

- Do not create a root `v1-archive/` directory unless the user explicitly asks
  for a physical archive.
- Do not move large V1 source trees into the active V2 root without a separate
  approval and matching codegraph/build exclusions.
- Treat V1 implementation code and old plans as extraction/reference material.
  New work starts from
  `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md` and the V2
  subplans under `thoughts/shared/plans/slug-v2-subplans/`.
- Every V1 extraction must be recorded in
  `thoughts/shared/plans/slug-v2-subplans/09-v1-extraction-ledger.md` with the
  V2 owner stage, oracle evidence, import mode, and validation.

## V2 Root Layout Proposal

This table records the Stage 0 ownership policy before any root cleanup or V1
source movement.

| Path | V2 status | Notes |
|------|-----------|-------|
| `AGENTS.md`, `README.md`, `V1_ARCHIVE.md` | active V2 orientation | Root pointers for new agents. |
| `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md` | canonical V2 plan | First roadmap entry for new work. |
| `thoughts/shared/plans/slug-v2-subplans/` | active V2 plans | Stage owners and validation policy. |
| `tools/v2_oracle*`, `tests/v2_oracle/` | active V2 scaffold | Stage 1 harness home. |
| `app/slug_*_v2/` | active V2 scaffold | Stage 2+ Rust crates. |
| `dice/`, `starlark-rust/`, `superconsole/`, `allocative/`, `gazebo/`, `shed/` | active retained infrastructure | Cargo workspace members after the 2026-06-29 root-metadata cleanup. |
| `remote_execution/` | retained source candidate | Not an active Cargo workspace member until Stage 7 wraps or ports it without V1 `slug_*` crate dependencies. |
| `app/slug`, `app/slug_*` without `_v2` | V1 reference/extraction | Do not treat as the V2 default implementation. |
| `tests/core/`, `tests/e2e/`, `tests/plan31/`, `tests/plan34/` | V1 reference/extraction | Mine only when a V2 subplan names the surface. |
| `thoughts/shared/plans/slug-bazel-subplans/` | archived/reference plans | V1 bug database unless a V2 plan explicitly asks for comparison. |
| `buck2/`, `prelude/`, Buck-shaped root metadata | V1 or vendored reference | Do not expose Buck user semantics in V2. |

The 2026-06-29 root-metadata cleanup removed V1-only app crates, integrations,
`remote_execution`, and `host_sharing` from active Cargo workspace metadata.
The current `codex/slugv2` branch still tracks V1 source/test paths beside V2
crates, so treat those paths as reference material until a follow-up clean-root
slice physically removes or excludes them.

Validation for the metadata cleanup:

- `cargo metadata --no-deps --format-version 1`
- `CARGO_TARGET_DIR=.codex-cargo-target cargo check -p slug_cli_v2 -p slug_core_v2 -p slug_commands_v2 -p slug_identity_v2 -p slug_query_v2 -p slug_build_api_v2 -p slug_analysis_v2 -p slug_loading_v2 -p slug_bzlmod_v2 -p slug_reapi_v2 -p slug_bep_v2`
- `CARGO_TARGET_DIR=.codex-cargo-target cargo test -p slug_cli_v2 -p slug_core_v2 -p slug_commands_v2 -p slug_identity_v2 -p slug_query_v2 -p slug_build_api_v2 -p slug_analysis_v2 -p slug_loading_v2 -p slug_bzlmod_v2 -p slug_reapi_v2 -p slug_bep_v2`
- `python3 -B -m tools.v2_oracle list`

## Validation

Run the Stage 0 checker from the repository root:

```bash
scripts/v2_archive_status.sh
```

On Windows, run it through a POSIX shell if direct execution is unavailable:

```bash
sh scripts/v2_archive_status.sh
```
