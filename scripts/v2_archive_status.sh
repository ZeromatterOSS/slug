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

root_v1_paths=$(git ls-files -- \
  .bazelignore .claude .github .vscode .watchmanconfig .envrc \
  CHANGELOG.md Cross.toml HACKING.md \
  buck_rust_binary.bzl ci.bzl defs.bzl lint_levels.bzl proto_defs.bzl \
  action_error_handler agent app_dep_graph_rules assets bazel_tools benchmarks \
  bootstrap buck2 build cfg examples explorer flake.lock flake.nix host_sharing \
  integrations prelude remote_execution rust-project.sh shim slug.bat slug.py \
  slug_builtins test.py website 2>/dev/null || true)
if [ -n "$root_v1_paths" ]; then
  fail "tracked V1-only root paths remain:"
  printf '%s\n' "$root_v1_paths"
else
  ok "no tracked V1-only root paths"
fi

if [ -n "$(git_value ls-files --error-unmatch MODULE.bazel)" ]; then
  if grep -F 'name = "slug"' MODULE.bazel >/dev/null 2>&1 &&
     grep -F 'name = "rules_rust"' MODULE.bazel >/dev/null 2>&1 &&
     grep -F 'version = "0.73.0"' MODULE.bazel >/dev/null 2>&1; then
    ok "root MODULE.bazel is fresh Slug V2 metadata"
  else
    fail "root MODULE.bazel is not recognized Slug V2 metadata"
  fi
fi

if [ -n "$(git_value ls-files --error-unmatch BUILD.bazel)" ]; then
  if grep -F 'Cargo.Bazel.lock' BUILD.bazel >/dev/null 2>&1 &&
     grep -F 'Cargo.lock' BUILD.bazel >/dev/null 2>&1; then
    ok "root BUILD.bazel owns Slug V2 dependency inputs"
  else
    fail "root BUILD.bazel is not recognized Slug V2 metadata"
  fi
fi

codex_v1_paths=$(git ls-files -- .codex \
  ':!.codex/skills/slug-buck2-utility-reuse/**' \
  ':!.codex/skills/slug-agent-orchestration/**' 2>/dev/null || true)
if [ -n "$codex_v1_paths" ]; then
  fail "tracked non-V2 .codex paths remain:"
  printf '%s\n' "$codex_v1_paths"
else
  ok "only V2 repo-local skills are tracked under .codex/"
fi

app_v1_paths=$(git ls-files -- app \
  ':!app/slug_analysis_v2/**' \
  ':!app/slug_bep_v2/**' \
  ':!app/slug_build_api_v2/**' \
  ':!app/slug_bzlmod_v2/**' \
  ':!app/slug_cli_v2/**' \
  ':!app/slug_commands_v2/**' \
  ':!app/slug_configuration_v2/**' \
  ':!app/slug_core_v2/**' \
  ':!app/slug_events_v2/**' \
  ':!app/slug_identity_v2/**' \
  ':!app/slug_loading_v2/**' \
  ':!app/slug_query_v2/**' \
  ':!app/slug_reapi_v2/**' \
  ':!app/slug_server_v2/**' \
  ':!app/slug_starlark_v2/**' \
  ':!app/slug_workspace_v2/**' 2>/dev/null || true)
if [ -n "$app_v1_paths" ]; then
  fail "tracked non-V2 app paths remain:"
  printf '%s\n' "$app_v1_paths"
else
  ok "only V2 app crates are tracked under app/"
fi

test_v1_paths=$(git ls-files -- tests \
  ':!tests/v2_oracle/**' \
  ':!tests/v2_fixture_payload/**' \
  ':!tests/v2_fixture_support/**' 2>/dev/null || true)
if [ -n "$test_v1_paths" ]; then
  fail "tracked non-oracle test paths remain:"
  printf '%s\n' "$test_v1_paths"
else
  ok "only V2 test infrastructure is tracked under tests/"
fi

tool_v1_paths=$(git ls-files -- tools \
  ':!tools/v2_oracle/**' \
  ':!tools/v2_oracle_lib/**' 2>/dev/null || true)
if [ -n "$tool_v1_paths" ]; then
  fail "tracked non-V2 tool paths remain:"
  printf '%s\n' "$tool_v1_paths"
else
  ok "only V2 oracle tools are tracked under tools/"
fi

thought_v1_paths=$(git ls-files -- thoughts \
  ':!thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md' \
  ':!thoughts/shared/plans/slug-v2-subplans/**' \
  ':!thoughts/shared/prompts/2026-06-29-slug-v2-generic-implementer.md' \
  ':!thoughts/shared/prompts/2026-07-23-slug-v2-root-orchestrator.md' \
  2>/dev/null || true)
if [ -n "$thought_v1_paths" ]; then
  fail "tracked non-V2 thoughts paths remain:"
  printf '%s\n' "$thought_v1_paths"
else
  ok "only V2 plans and prompts are tracked under thoughts/"
fi

doc_v1_paths=$(git ls-files -- docs ':!docs/developers/dice.md' 2>/dev/null || true)
if [ -n "$doc_v1_paths" ]; then
  fail "tracked non-retained docs paths remain:"
  printf '%s\n' "$doc_v1_paths"
else
  ok "only retained DICE docs are tracked under docs/"
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
