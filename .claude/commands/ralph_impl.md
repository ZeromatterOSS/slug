---
description: Implement highest priority small local task with worktree setup
model: sonnet
---

## PART I - IF A TASK IS MENTIONED

0c. read the task file at `agent/tasks/active/TASK-XXXX.md`
0d. read the task and all comments to understand the implementation plan and any concerns

## PART I - IF NO TASK IS MENTIONED

0.  read .claude/commands/tasks.md
0a. find the top priority items in status "ready_for_dev" using this query:

```bash
for f in agent/tasks/active/TASK-*.md; do
  if grep -q "^status: ready_for_dev" "$f"; then
    priority=$(grep "^priority:" "$f" | cut -d: -f2 | tr -d ' ')
    size=$(grep "^size:" "$f" | cut -d: -f2 | tr -d ' ')
    id=$(grep "^id:" "$f" | cut -d: -f2 | tr -d ' ')
    title=$(grep "^title:" "$f" | cut -d'"' -f2)
    echo "$priority|$size|$id|$title|$f"
  fi
done | sort -t'|' -k1,1n | head -10
```

0b. select the highest priority SMALL or XS issue from the list (if no SMALL or XS issues exist, EXIT IMMEDIATELY and inform the user)
0c. read the selected task file at `agent/tasks/active/TASK-XXXX.md`
0d. read the task and all comments to understand the implementation plan and any concerns

## PART II - NEXT STEPS

think deeply

1. update the task status to "in_dev" by editing the frontmatter
1a. identify the linked implementation plan document from the `links` section
1b. if no plan exists, move the task back to "ready_for_plan" and EXIT with an explanation

think deeply about the implementation

2. set up worktree for implementation:
2a. read `hack/create_worktree.sh` and create a new worktree with the task branch name: `./hack/create_worktree.sh TASK-XXXX BRANCH_NAME`
2b. launch implementation session: `humanlayer-nightly launch --model opus --dangerously-skip-permissions --dangerously-skip-permissions-timeout 15m --title "implement TASK-XXXX" -w ~/wt/humanlayer/TASK-XXXX "/implement_plan and when you are done implementing and all tests pass, read ./claude/commands/commit.md and create a commit, then read ./claude/commands/describe_pr.md and create a PR, then add a comment to the task file at agent/tasks/active/TASK-XXXX.md with the PR link"`

think deeply, use TodoWrite to track your tasks. When searching tasks, get the top 10 items by priority but only work on ONE item - specifically the highest priority SMALL or XS sized issue.

