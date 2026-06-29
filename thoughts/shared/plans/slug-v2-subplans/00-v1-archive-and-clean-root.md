# Stage 0: V1 Archive and Clean V2 Root

## Goal

Preserve the current Slug V1 implementation, then make the repository root
clearly point at the V2 clean restart without making V1 code the active default
for new work.

## Scope

- Create an immutable V1 preservation point.
- Add root-level V2 orientation.
- Keep old plans and source available for extraction.
- Prevent V1 code from polluting V2 discovery and build metadata.

## Recommended Archive Shape

1. Commit or explicitly preserve any existing dirty V1 work.
2. Create a tag such as `slug-v1-archive`.
3. Create an archive branch such as `v1-archive`.
4. Keep a small root document that explains how to inspect V1.
5. Restart active `main` around V2 docs, metadata, and skeleton code.

Do not move the whole V1 tree into `v1-archive/` unless tags/branches are not
acceptable. If a physical archive is required, it must be read-only reference
material and excluded from active build metadata and codegraph indexing.

Concrete supporting files for this stage:

- `V1_ARCHIVE.md`: exact V1 commit, tag, archive branch, checkout commands, and
  explicit no-physical-archive-by-default policy.
- `scripts/v2_archive_status.sh`: read-only checker for dirty state, archive
  refs, V2 root pointers, and accidental `v1-archive/` directory presence.

## Acceptance Criteria

- `git status --short` is understood before any archive action.
- V1 tag and archive branch names are recorded in this file and exist in Git.
- `scripts/v2_archive_status.sh` exits 0 with a clean worktree.
- Root `README.md` and `AGENTS.md` identify the V2 plan as canonical.
- The active root does not require new agents to search through V1 source to
  understand where to begin.
- Active V2 build/workspace metadata does not include V1-only crates, tests, or
  Buck-shaped user surfaces unless a V2 subplan records them as retained
  infrastructure.
- Existing mixed-root work on `codex/slugv2` is classified before reuse:
  discard, cherry-pick as V2-only, port with an oracle, or keep as
  reference-only.

## Implementation Slices

### 0.1 Dirty-State Triage

- Run `git status --short --branch` and `git diff --name-status`.
- For each dirty file, classify it as one of:
  - already-owned V1 work that must be committed before the archive;
  - disposable generated state;
  - unrelated concurrent-agent work that must be left alone.
- Do not archive with unknown dirty files. If dirty files are unrelated and the
  owner is unavailable, create a temporary preservation branch from the current
  worktree before proceeding.
- Record the triage result in `V1_ARCHIVE.md` before creating the tag.

### 0.2 V1 Preservation Point

- Create a commit for any V1 preservation work the user wants included.
- Record the exact commit selected as V1 in this file under `Archive Record`.
- Create an annotated tag:

```bash
git tag -a slug-v1-archive <v1-commit> -m "Archive Slug V1"
```

- Create an archive branch without checking it out:

```bash
git branch v1-archive <v1-commit>
```

### 0.3 Active V2 Root Policy

- Keep only small root pointers to V1. Do not move the full V1 tree in-tree by
  default.
- If the user insists on physical `v1-archive/`, add ignores and codegraph
  exclusions in the same change before new V2 code is indexed.
- Update root metadata so `AGENTS.md`, `README.md`, and the top-level plan all
  point at the V2 plan first.

### 0.4 First V2 Skeleton Gate

- Create a V2 root layout proposal before deleting V1 root files.
- The proposal must say which existing directories are retained as vendored
  infrastructure (`dice`, `starlark-rust`, selected `remote_execution`) and
  which are V1 archive-only.
- Add the retained/archive-only directory table to `V1_ARCHIVE.md` so agents do
  not infer ownership from old paths.
- Do not remove V1 code until the tag and branch validation below pass.

### 0.R 2026-06-29 Mixed-Root Remediation

The `codex/slugv2` branch has useful V2 scaffolding, but it does not satisfy
Stage 0. The 2026-06-29 review found missing local archive refs and V2 crates
layered into the still-active V1 Cargo workspace.

Before any further V2 trunk work:

1. Run `scripts/v2_archive_status.sh` and treat a missing tag or branch as a
   blocker, not a warning.
2. Re-select the V1 archive commit from the live checkout. If the intended
   commit remains `e218054d4c796655939b968d90208b185decb352`, create the
   annotated `slug-v1-archive` tag and `v1-archive` branch there. If not, update
   this plan and `V1_ARCHIVE.md` before creating refs.
3. Create a separate clean-root worktree or branch for V2 remediation. Do not
   keep iterating on the mixed-root tree as if it were the clean trunk.
4. Remove V1-only workspace membership and build metadata from the active V2
   root. Keep only root orientation docs, V2 plans, Stage 1 oracle harnesses,
   V2 crates, and explicitly retained infrastructure.
5. Treat current `codex/slugv2` commits as a patch queue. Reapply them in stage
   order only after the owner subplan names the oracle and validation command.
6. Update Stage 9 for any V1 or mixed-root code imported into the clean root.
7. Re-run the archive checker, the touched stage tests, and `git diff --check`
   before declaring Stage 0 complete.

Execution update on 2026-06-29: the local `slug-v1-archive` annotated tag and
`v1-archive` branch were created at
`e218054d4c796655939b968d90208b185decb352`, and
`scripts/v2_archive_status.sh` now verifies the refs. Stage 0 is still not
complete because the active branch still tracks V1 source/test paths.

Root-metadata update on 2026-06-29: `Cargo.toml` now keeps only V2 app crates
as active `app/slug_*` workspace members and `slug_*` workspace dependencies.
The active retained infrastructure members are `allocative`, `dice`,
`gazebo`, `pagable`, `shed`, `starlark-rust`, and `superconsole`.
`remote_execution` remains a Stage 7 source candidate, but it is not an active
workspace member because `remote_execution/oss/re_grpc*` still depends on V1
`slug_data`, `slug_protoc_dev`, `slug_re_configuration`, and `slug_util`.
Follow-up clean-root work should remove or exclude V1 source/test paths and
Buck-shaped metadata without treating the mixed-root `codex/slugv2` commits as
already accepted V2 trunk.

## Exact Test Criteria

- `git rev-parse slug-v1-archive^{commit}` equals the commit recorded in
  `Archive Record` because the archive tag is annotated.
- `git rev-parse v1-archive` equals the same commit.
- `test "$(git rev-parse slug-v1-archive^{commit})" = "$(git rev-parse v1-archive)"`
  passes.
- `V1_ARCHIVE.md` and `Archive Record` agree on commit and ref names.
- `git status --short` after archiving shows only intentionally active V2 work.
- `git ls-tree -d HEAD v1-archive` is empty unless the user explicitly chose a
  physical archive.
- `rg -n "2026-06-26-slug-v2-clean-restart" AGENTS.md README.md thoughts/shared/plans`
  finds the canonical V2 entrypoints.
- If a physical archive exists, codegraph/indexing config excludes it and the
  active build does not traverse it.

## Archive Record

This record was verified in the local `codex/slugv2` checkout on 2026-06-29.
The archive refs exist, but the branch remains a mixed-root tree until the
clean-root remediation lands.

| Field | Value |
|-------|-------|
| V1 commit | `e218054d4c796655939b968d90208b185decb352` verified locally on 2026-06-29 |
| V1 tag | `slug-v1-archive` created as an annotated tag on 2026-06-29 |
| V1 archive branch | `v1-archive` created on 2026-06-29 |
| Physical archive directory | none by default |
| Dirty files intentionally excluded | archive action excluded active V2 remediation docs and prompt only; later root-metadata cleanup changed `Cargo.toml`; no V1 implementation files |
| Current verification status | archive refs and Cargo root metadata verified; physical V1 source/test cleanup remains pending |

## Validation

```bash
git status --short --branch
git diff --name-status
git rev-parse slug-v1-archive^{commit}
git rev-parse v1-archive
test "$(git rev-parse slug-v1-archive^{commit})" = "$(git rev-parse v1-archive)"
git tag --list 'slug-v1-*'
git branch --list 'v1-archive'
git ls-tree -d HEAD v1-archive
scripts/v2_archive_status.sh
git diff --check -- AGENTS.md README.md V1_ARCHIVE.md scripts/v2_archive_status.sh thoughts/shared/plans
rg -n "2026-06-26-slug-v2-clean-restart" AGENTS.md README.md thoughts/shared/plans
```

## Open Decision

Whether to physically move V1 into an in-tree `v1-archive/` directory remains a
user decision. The default is tag plus branch archive.
