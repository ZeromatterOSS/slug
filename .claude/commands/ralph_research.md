---
description: Research highest priority local task needing investigation
---

## PART I - IF A TASK IS MENTIONED

0c. read the task file at `agent/tasks/active/TASK-XXXX.md`
0d. read the task and all comments to understand what research is needed and any previous attempts

## PART I - IF NO TASK IS MENTIONED

0.  read .claude/commands/tasks.md
0a. find the top priority items in status "research_needed" using this query:

```bash
for f in agent/tasks/active/TASK-*.md; do
  if grep -q "^status: research_needed" "$f"; then
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
0d. read the task and all comments to understand what research is needed and any previous attempts

## PART II - NEXT STEPS

think deeply

1. update the task status to "research_in_progress" by editing the frontmatter
1a. read any linked documents in the `links` section to understand context
1b. if insufficient information to conduct research, add a comment asking for clarification and move back to "research_needed"

think deeply about the research needs

2. conduct the research:
2a. read .claude/commands/research_codebase.md for guidance on effective codebase research
2b. if the task comments suggest web research is needed, use WebSearch to research external solutions, APIs, or best practices
2c. search the codebase for relevant implementations and patterns
2d. examine existing similar features or related code
2e. identify technical constraints and opportunities
2f. Be unbiased - don't think too much about an ideal implementation plan, just document all related files and how the systems work today
2g. document findings in a new thoughts document: `thoughts/shared/research/YYYY-MM-DD-TASK-XXXX-description.md`
   - Format: `YYYY-MM-DD-TASK-XXXX-description.md` where:
     - YYYY-MM-DD is today's date
     - TASK-XXXX is the task number (omit if no task)
     - description is a brief kebab-case description of the research topic
   - Examples:
     - With task: `2026-01-21-TASK-0001-parent-child-tracking.md`
     - Without task: `2026-01-21-error-handling-patterns.md`

think deeply about the findings

3. synthesize research into actionable insights:
3a. summarize key findings and technical decisions
3b. identify potential implementation approaches
3c. note any risks or concerns discovered
3d. run `humanlayer thoughts sync` to save the research

4. update the task:
4a. add the research document link to the task's `links` array in frontmatter
4b. add a comment summarizing the research outcomes
4c. update the task status to "research_in_review"
4d. update the `updated` timestamp

think deeply, use TodoWrite to track your tasks. When searching tasks, get the top 10 items by priority but only work on ONE item - specifically the highest priority issue.

## PART III - When you're done

Print a message for the user (replace placeholders with actual values):

```
✅ Completed research for TASK-XXXX: [task title]

Research topic: [research topic description]

The research has been:

Created at thoughts/shared/research/YYYY-MM-DD-TASK-XXXX-description.md
Synced to thoughts repository
Linked to the task file
Task moved to "research_in_review" status

Key findings:
- [Major finding 1]
- [Major finding 2]
- [Major finding 3]

View the task: agent/tasks/active/TASK-XXXX.md
```

