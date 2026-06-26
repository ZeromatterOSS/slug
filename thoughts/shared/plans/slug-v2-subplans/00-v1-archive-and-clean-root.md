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

## Acceptance Criteria

- `git status --short` is understood before any archive action.
- V1 tag and archive branch names are recorded in this file.
- Root `README.md` and `AGENTS.md` identify the V2 plan as canonical.
- The active root does not require new agents to search through V1 source to
  understand where to begin.

## Validation

```bash
git status --short --branch
git tag --list 'slug-v1-*'
git branch --list 'v1-archive'
git diff --check -- AGENTS.md README.md thoughts/shared/plans
```

## Open Decision

Whether to physically move V1 into an in-tree `v1-archive/` directory remains a
user decision. The default is tag plus branch archive.
