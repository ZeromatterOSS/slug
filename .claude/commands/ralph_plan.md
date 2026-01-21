---
description: Create implementation plan for highest priority local task ready for spec
---

## PART I - IF A TASK IS MENTIONED

0c. read the task file at `agent/tasks/active/TASK-XXXX.md`
0d. read the task and all comments to learn about past implementations and research, and any questions or concerns about them


### PART I - IF NO TASK IS MENTIONED

0.  read .claude/commands/tasks.md
0a. find the top priority items in status "ready_for_plan" using this query:

```bash
for f in agent/tasks/active/TASK-*.md; do
  if grep -q "^status: ready_for_plan" "$f"; then
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
0d. read the task and all comments to learn about past implementations and research, and any questions or concerns about them

### PART II - NEXT STEPS

think deeply

1. update the task status to "plan_in_progress" by editing the frontmatter
1a. read ./claude/commands/create_plan.md
1b. determine if the task has a linked implementation plan document based on the `links` section
1d. if the plan exists, you're done, respond with a link to the task
1e. if the research is insufficient or has unanswered questions, create a new plan document following the instructions in ./claude/commands/create_plan.md

think deeply

2. when the plan is complete, `humanlayer thoughts sync` and link the doc to the task by adding to the `links` array, then add a terse comment with the plan path (re-read .claude/commands/tasks.md if needed)
2a. update the task status to "plan_in_review"
2b. update the `updated` timestamp

think deeply, use TodoWrite to track your tasks. When searching tasks, get the top 10 items by priority but only work on ONE item - specifically the highest priority SMALL or XS sized issue.

### PART III - When you're done


Print a message for the user (replace placeholders with actual values):

```
✅ Completed implementation plan for TASK-XXXX: [task title]

Approach: [selected approach description]

The plan has been:

Created at thoughts/shared/plans/YYYY-MM-DD-TASK-XXXX-description.md
Synced to thoughts repository
Linked to the task file
Task moved to "plan_in_review" status

Implementation phases:
- Phase 1: [phase 1 description]
- Phase 2: [phase 2 description]
- Phase 3: [phase 3 description if applicable]

View the task: agent/tasks/active/TASK-XXXX.md
```

