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
- The proposal must say which existing directories are retained as infrastructure
  (`dice`, `starlark-rust`, selected V2-owned REAPI wrappers) and which are V1
  archive-only.
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
   root. Keep only root orientation docs, repo-local V2 skills, V2 plans,
   Stage 1 oracle harnesses, V2 crates, and explicitly retained
   infrastructure.
5. Treat current `codex/slugv2` commits as a patch queue. Reapply them in stage
   order only after the owner subplan names the oracle and validation command.
6. Update Stage 9 for any V1 or mixed-root code imported into the clean root.
7. Re-run the archive checker, the touched stage tests, and `git diff --check`
   before declaring Stage 0 complete.

Execution update on 2026-06-29: the local `slug-v1-archive` annotated tag and
`v1-archive` branch were created at
`e218054d4c796655939b968d90208b185decb352`, and
`scripts/v2_archive_status.sh` now verifies the refs.

Root-metadata update on 2026-06-29: `Cargo.toml` now keeps only V2 app crates
as active `app/slug_*` workspace members and `slug_*` workspace dependencies.
The active retained infrastructure members are `allocative`, `dice`, `gazebo`,
`pagable`, `shed`, `starlark-rust`, and `superconsole`.

Clean-root update on 2026-06-29: branch
`codex/slugv2-clean-root-remediation` physically removes tracked V1-only
source/test paths, old docs, old V1 plans, root Bazel/Buck metadata, old CI,
wrappers, shims, examples, website, and the unwrapped `remote_execution` source
candidate from the active root. Future Stage 7 work must re-import REAPI client
code through `app/slug_reapi_v2` or another V2 wrapper only after
`07-reapi-native-execution.md` and Stage 9 name the oracle and validation
command. V1 source remains available through `slug-v1-archive` and
`v1-archive`; the mixed-root `codex/slugv2` branch remains a prototype patch
queue, not an accepted trunk.

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

This record was reverified in the live `main` checkout on 2026-07-23.

| Field | Value |
|-------|-------|
| V1 commit | `e218054d4c796655939b968d90208b185decb352` verified locally on 2026-06-29 |
| V1 tag | `slug-v1-archive` is an annotated tag resolving to the V1 commit; reverified in the live checkout on 2026-07-23 |
| V1 archive branch | `v1-archive` resolves directly to the V1 commit; restored and reverified in the live checkout on 2026-07-23 |
| Physical archive directory | none by default |
| Dirty files intentionally excluded | archive action excluded active V2 remediation docs and prompt only; later root-metadata cleanup changed `Cargo.toml`; no V1 implementation files |
| Current verification status | both refs match; exact V2 allowlists are current; required-clean archive checker passes; Stage 0 accepted |

## 2026-07-22 Live-Checkout Recheck

The historical 2026-06-29 evidence above describes the remediation checkout at
that time. It is not current-state authority. In the live `main` checkout:

- annotated tag `slug-v1-archive^{commit}` resolves to
  `e218054d4c796655939b968d90208b185decb352`;
- local branch `v1-archive` is missing;
- `scripts/v2_archive_status.sh` exits 1 for that missing branch and because
  its V2 app allowlist has not been updated for the tracked
  `app/slug_server_v2` crate; and
- the untracked workspace `.bazelrc` is active user state and must be preserved.
  It is not evidence of a clean or dirty V1 archive and must not be confused
  with `~/.bazelrc`, which agents must never read or commit.

At that checkpoint, M0 acceptance required creating the missing archive branch
at the recorded V1 commit after a normal read-only ref check, updating the
checker to recognize owned V2 crates without weakening its V1 exclusions, and
rerunning the validation below. The following acceptance entry supersedes that
live-state description.

## 2026-07-23 Baseline Repair Acceptance

The bounded `WP-0-baseline-repair` restored local `v1-archive` at
`e218054d4c796655939b968d90208b185decb352` only after confirming that no local,
remote, or worktree-owned archive branch existed and that the annotated tag
peeled to the same recorded commit. No existing ref was moved or overwritten.

Commit `9897e940` updates only three exact checker pathspecs for active V2-owned
surfaces: `app/slug_server_v2/**`,
`.codex/skills/slug-agent-orchestration/**`, and the current root-orchestrator
prompt. It does not broaden any sibling, V1 root, test, tool, docs, or
physical-archive exclusion. The checker passes with a clean worktree and
retains negative coverage for a mismatched branch and a missing tag. Sol-low
returned final `ACCEPT`; M0 is accepted.

## 2026-06-29 Clean-Root Evidence

Branch: `codex/slugv2-clean-root-remediation`.

- `scripts/v2_archive_status.sh` exits 0. It verifies the archive refs, no
  physical `v1-archive/`, no tracked V1-only root paths, only V2 app crates
  under `app/`, only V2 oracle tests/tools, only V2 plans/prompt, and only
  retained DICE docs under `docs/`.
- `git ls-files -- app` with exclusions for every `app/slug_*_v2/` crate emits
  no V1 app paths.
- `git ls-files -- tests ':!tests/v2_oracle/**'`,
  `git ls-files -- tools ':!tools/v2_oracle/**' ':!tools/v2_oracle_lib/**'`,
  `git ls-files -- thoughts` with V2 plan/prompt exclusions, and
  `git ls-files -- docs ':!docs/developers/dice.md'` emit no paths.
- `scripts/v2_archive_status.sh` allows only
  `.codex/skills/slug-buck2-utility-reuse/**` under `.codex/`.
- `cargo metadata --no-deps --format-version 1` succeeds and reports the V2
  app crates plus retained infrastructure as workspace members.
- Focused `cargo check` passes for all V2 app crates with
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1`.
- The matching `cargo test` command passes for the same V2 crate set.
- `python3 -B -m tools.v2_oracle list` lists the Stage 1 fixtures.
- `python -m pytest -q tests/v2_oracle/test_v2_oracle.py` passes with 12 tests.
  The local `python3` is Python 3.14 and does not have `pytest` installed.
- `cargo tree -p slug_cli_v2` shows only `slug_commands_v2`, `slug_core_v2`,
  and their V2 dependencies.
- The grep for `buck`, `BUCK`, `TARGETS`, `CellResolver`, and `buck-out` across
  `app/slug_cli_v2` and `app/slug_core_v2` emits no matches.
- The grep for `remote_execution`, `buck2`, `prelude`, `host_sharing`,
  `slug_data`, `slug_util`, `slug_execute`, `slug_server`, and `slug_client`
  across `Cargo.toml` and V2 crate manifests emits no matches.
- `git diff --check` passes for root orientation/config files,
  `scripts/v2_archive_status.sh`, and `thoughts/shared/plans`.
- `git diff --cached --check` passes for the staged deletions.

Residual risk: upstream Bazel oracle regeneration was not rerun in this Linux
checkout. That remains owned by the individual V2 stage plans that require
Bazel-side expected output refreshes.

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
