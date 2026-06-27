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
- V1 tag and archive branch names are recorded in this file.
- Root `README.md` and `AGENTS.md` identify the V2 plan as canonical.
- The active root does not require new agents to search through V1 source to
  understand where to begin.

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

Fill this in during Stage 0 execution:

| Field | Value |
|-------|-------|
| V1 commit | `e218054d4c796655939b968d90208b185decb352` |
| V1 tag | `slug-v1-archive` |
| V1 archive branch | `v1-archive` |
| Physical archive directory | none by default |
| Dirty files intentionally excluded | none; native `git status --short --branch` was clean before archiving |

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
