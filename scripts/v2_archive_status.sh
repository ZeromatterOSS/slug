#!/usr/bin/env sh
set -u

archive_tag=${V1_ARCHIVE_TAG:-slug-v1-archive}
archive_branch=${V1_ARCHIVE_BRANCH:-v1-archive}
v1_record=${V1_ARCHIVE_RECORD:-V1_ARCHIVE.md}
canonical_plan="thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md"
status=0

ok() {
  printf 'OK: %s\n' "$1"
}

info() {
  printf 'INFO: %s\n' "$1"
}

fail() {
  printf 'FAIL: %s\n' "$1"
  status=1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

git_value() {
  git "$@" 2>/dev/null || true
}

require_command git

tag_commit=$(git_value rev-parse --verify "${archive_tag}^{commit}")
branch_commit=$(git_value rev-parse --verify "${archive_branch}^{commit}")

if [ -n "$tag_commit" ]; then
  ok "${archive_tag} resolves to ${tag_commit}"
else
  fail "missing archive tag ${archive_tag}"
fi

if [ -n "$branch_commit" ]; then
  ok "${archive_branch} resolves to ${branch_commit}"
else
  fail "missing archive branch ${archive_branch}"
fi

if [ -n "$tag_commit" ] && [ -n "$branch_commit" ]; then
  if [ "$tag_commit" = "$branch_commit" ]; then
    ok "archive tag and branch point at the same commit"
  else
    fail "archive tag ${tag_commit} and branch ${branch_commit} differ"
  fi
fi

if [ -f "$v1_record" ]; then
  ok "${v1_record} exists"
  if [ -n "$tag_commit" ] && grep -F "$tag_commit" "$v1_record" >/dev/null 2>&1; then
    ok "${v1_record} records V1 commit"
  else
    fail "${v1_record} does not record ${tag_commit:-the archive commit}"
  fi
  if grep -F "$archive_tag" "$v1_record" >/dev/null 2>&1 &&
     grep -F "$archive_branch" "$v1_record" >/dev/null 2>&1; then
    ok "${v1_record} records archive ref names"
  else
    fail "${v1_record} does not record archive ref names"
  fi
else
  fail "missing ${v1_record}"
fi

if [ -d v1-archive ]; then
  fail "physical v1-archive/ directory exists; default policy is tag plus branch"
else
  ok "no physical v1-archive/ directory"
fi

tree_entry=$(git_value ls-tree -d HEAD v1-archive)
if [ -n "$tree_entry" ]; then
  fail "HEAD contains a tracked v1-archive directory"
else
  ok "HEAD does not track a v1-archive directory"
fi

if command -v rg >/dev/null 2>&1; then
  plan_matches=$(rg -n "$canonical_plan" AGENTS.md README.md thoughts/shared/plans 2>/dev/null || true)
else
  plan_matches=$(grep -R -n "$canonical_plan" AGENTS.md README.md thoughts/shared/plans 2>/dev/null || true)
fi

if [ -n "$plan_matches" ]; then
  ok "canonical V2 plan is referenced from root/plans"
else
  fail "canonical V2 plan reference not found"
fi

dirty=$(git status --short --untracked-files=all 2>/dev/null || true)
if [ -n "$dirty" ]; then
  info "working tree has active changes:"
  printf '%s\n' "$dirty"
  if [ "${V2_ARCHIVE_STATUS_REQUIRE_CLEAN:-0}" = "1" ]; then
    fail "working tree is dirty"
  fi
else
  ok "working tree is clean"
fi

exit "$status"
