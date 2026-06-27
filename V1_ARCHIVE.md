# Slug V1 Archive

Slug V1 is preserved by Git ref, not by an in-tree source copy. The active
checkout remains the V2 clean-restart line; V1 code is reference and extraction
material only.

## Archive Record

| Field | Value |
|-------|-------|
| V1 commit | `e218054d4c796655939b968d90208b185decb352` |
| V1 tag | `slug-v1-archive` |
| V1 archive branch | `v1-archive` |
| Tag type | annotated |
| Physical archive directory | none by default |
| Dirty files intentionally excluded | none; native `git status --short --branch` was clean before archiving |

The archive tag and branch both peel to the V1 commit above. Because the tag is
annotated, use `git rev-parse slug-v1-archive^{commit}` when comparing it with
the branch commit.

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
| `dice/`, `starlark-rust/`, `remote_execution/`, `superconsole/`, `allocative/`, `gazebo/`, `shed/` | retained infrastructure candidates | Reuse only behind V2 wrappers and Bazel-shaped semantics. |
| `app/slug`, `app/slug_*` without `_v2` | V1 reference/extraction | Do not treat as the V2 default implementation. |
| `tests/core/`, `tests/e2e/`, `tests/plan31/`, `tests/plan34/` | V1 reference/extraction | Mine only when a V2 subplan names the surface. |
| `thoughts/shared/plans/slug-bazel-subplans/` | archived/reference plans | V1 bug database unless a V2 plan explicitly asks for comparison. |
| `buck2/`, `prelude/`, Buck-shaped root metadata | V1 or vendored reference | Do not expose Buck user semantics in V2. |

## Validation

Run the Stage 0 checker from the repository root:

```bash
scripts/v2_archive_status.sh
```

On Windows, run it through a POSIX shell if direct execution is unavailable:

```bash
sh scripts/v2_archive_status.sh
```
