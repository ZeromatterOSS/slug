---
description: Manage local task files - create, update, comment, and follow workflow patterns
---

# Tasks - Local Task Management

You are tasked with managing local task files in `agent/tasks/`, including creating tasks from thoughts documents, updating existing tasks, and following the team's specific workflow patterns.

## Initial Response

Respond based on the user's request:

### For general requests:
```
I can help you with local tasks. What would you like to do?
1. Create a new task from a thoughts document
2. Add a comment to a task (I'll use our conversation context)
3. Search for tasks
4. Update task status or details
```

### For specific create requests:
```
I'll help you create a local task from your thoughts document. Please provide:
1. The path to the thoughts document (or topic to search for)
2. Any specific focus or angle for the task (optional)
```

Then wait for the user's input.

## Team Workflow & Status Progression

The team follows a specific workflow to ensure alignment before code implementation:

1. **triage** → All new tasks start here for initial review
2. **spec_needed** → More detail is needed - problem to solve and solution outline necessary
3. **research_needed** → Task requires investigation before plan can be written
4. **research_in_progress** → Active research/investigation underway
5. **research_in_review** → Research findings under review (optional step)
6. **ready_for_plan** → Research complete, task needs an implementation plan
7. **plan_in_progress** → Actively writing the implementation plan
8. **plan_in_review** → Plan is written and under discussion
9. **ready_for_dev** → Plan approved, ready for implementation
10. **in_dev** → Active development
11. **code_review** → PR submitted
12. **done** → Completed
13. **backlog** → Deferred for later
14. **canceled** → Won't do

**Key principle**: Review and alignment happen at the plan stage (not PR stage) to move faster and avoid rework.

## Task File Location & Schema

Tasks are stored as markdown files in `agent/tasks/active/` with YAML frontmatter:

### File naming: `TASK-XXXX.md` where XXXX is zero-padded ID

### YAML Frontmatter Schema
```yaml
---
id: TASK-0001
title: "Task title here"
status: triage  # triage|spec_needed|research_needed|research_in_progress|research_in_review|ready_for_plan|plan_in_progress|plan_in_review|ready_for_dev|in_dev|code_review|done|backlog|canceled
priority: 3     # 1=urgent, 2=high, 3=medium, 4=low
created: 2026-01-21T10:30:00-08:00
updated: 2026-01-21T10:30:00-08:00
labels: []      # bug, hld, wui, meta
size: small     # xs, small, medium, large, xl
assignee: null
links: []       # [{title: "Doc Title", url: "path/or/url"}]
branch: null
pr_url: null
---
```

### Content Template
```markdown
# TASK-XXXX: Title

## Problem to Solve
[Description]

## Proposed Solution
[Approach]

## Acceptance Criteria
- [ ] Criterion 1

---
## Comments

### YYYY-MM-DD HH:MM - author
Comment text.
```

## ID Generation

When creating a new task:
1. Read `agent/tasks/_counter.txt` to get the next ID
2. Zero-pad to 4 digits (e.g., 1 → 0001, 42 → 0042)
3. Increment the counter and write it back to `_counter.txt`
4. Create the task file as `agent/tasks/active/TASK-XXXX.md`

## Action-Specific Instructions

### 1. Creating Tasks from Thoughts

#### Steps to follow after receiving the request:

1. **Locate and read the thoughts document:**
   - If given a path, read the document directly
   - If given a topic/keyword, search thoughts/ directory using Grep to find relevant documents
   - If multiple matches found, show list and ask user to select
   - Create a TodoWrite list to track: Read document → Analyze content → Draft task → Get user input → Create task

2. **Analyze the document content:**
   - Identify the core problem or feature being discussed
   - Extract key implementation details or technical decisions
   - Note any specific code files or areas mentioned
   - Look for action items or next steps
   - Identify what stage the idea is at (early ideation vs ready to implement)
   - Take time to think deeply about distilling the essence of this document into a clear problem statement and solution approach

3. **Check for related context (if mentioned in doc):**
   - If the document references specific code files, read relevant sections
   - If it mentions other thoughts documents, quickly check them
   - Look for any existing tasks mentioned

4. **Draft the task summary:**
   Present a draft to the user:
   ```
   ## Draft Task

   **Title**: [Clear, action-oriented title]

   **Description**:
   [2-3 sentence summary of the problem/goal]

   ## Key Details
   - [Bullet points of important details from thoughts]
   - [Technical decisions or constraints]
   - [Any specific requirements]

   ## Implementation Notes (if applicable)
   [Any specific technical approach or steps outlined]

   ## References
   - Source: `thoughts/[path/to/document.md]`
   - Related code: [any file:line references]
   - Parent task: [if applicable]

   ---
   Based on the document, this seems to be at the stage of: [ideation/planning/ready to implement]
   ```

5. **Interactive refinement:**
   Ask the user:
   - Does this summary capture the task accurately?
   - What priority? (Default: Medium/3)
   - What size? (xs/small/medium/large/xl)
   - Any additional context to add?
   - Should we include more/less implementation detail?
   - Any labels to apply? (bug, hld, wui, meta)

   Note: Task will be created in "triage" status by default.

6. **Create the task file:**
   - Read `agent/tasks/_counter.txt` for next ID
   - Create file at `agent/tasks/active/TASK-XXXX.md` with proper frontmatter and content
   - Increment and save the counter

7. **Post-creation actions:**
   - Show the created task path
   - Ask if user wants to:
     - Add a comment with additional implementation details
     - Update the original thoughts document with the task reference

## Example transformations:

### From verbose thoughts:
```
"I've been thinking about how our resumed sessions don't inherit permissions properly.
This is causing issues where users have to re-specify everything. We should probably
store all the config in the database and then pull it when resuming. Maybe we need
new columns for permission_prompt_tool and allowed_tools..."
```

### To concise task:
```
Title: Fix resumed sessions to inherit all configuration from parent

## Problem to Solve
Currently, resumed sessions only inherit Model and WorkingDir from parent sessions,
causing all other configuration to be lost. Users must re-specify permissions and
settings when resuming.

## Proposed Solution
Store all session configuration in the database and automatically inherit it when
resuming sessions, with support for explicit overrides.
```

### 2. Adding Comments to Existing Tasks

When user wants to add a comment to a task:

1. **Determine which task:**
   - Use context from the current conversation to identify the relevant task
   - If uncertain, ask user for the task ID
   - Read the task file to confirm

2. **Format comments for clarity:**
   - Keep comments concise (~10 lines) unless more detail is needed
   - Focus on the key insight or most useful information for a human reader
   - Not just what was done, but what matters about it
   - Include relevant file references with backticks

3. **Comment structure example:**
   ```markdown
   ### 2026-01-21 14:30 - claude
   Implemented retry logic in webhook handler to address rate limit issues.

   Key insight: The 429 responses were clustered during batch operations,
   so exponential backoff alone wasn't sufficient - added request queuing.

   Files updated:
   - `src/webhooks/handler.go`
   - `thoughts/shared/rate_limit_analysis.md`
   ```

4. **Add the comment:**
   - Read the task file
   - Append the comment to the Comments section
   - Update the `updated` timestamp in frontmatter
   - If adding a link, also update the `links` array in frontmatter

### 3. Searching for Tasks

When user wants to find tasks:

1. **Search by status:**
   ```bash
   for f in agent/tasks/active/TASK-*.md; do
     if grep -q "^status: STATUS_NAME" "$f"; then
       title=$(grep "^title:" "$f" | cut -d'"' -f2)
       id=$(grep "^id:" "$f" | cut -d: -f2 | tr -d ' ')
       echo "$id: $title"
     fi
   done
   ```

2. **Search by keyword in title/content:**
   ```bash
   grep -l "KEYWORD" agent/tasks/active/TASK-*.md
   ```

3. **Search by priority:**
   ```bash
   for f in agent/tasks/active/TASK-*.md; do
     if grep -q "^priority: N" "$f"; then
       # extract and display
     fi
   done
   ```

4. **Present results:**
   - Show task ID, title, status, priority
   - Include file path for easy access

### 4. Updating Task Status

When moving tasks through the workflow:

1. **Get current status:**
   - Read the task file
   - Show current status in workflow

2. **Suggest next status:**
   - triage → spec_needed (lacks detail/problem statement)
   - spec_needed → research_needed (once problem/solution outlined)
   - research_needed → research_in_progress (starting research)
   - research_in_progress → research_in_review (optional, can skip to ready_for_plan)
   - research_in_review → ready_for_plan (research approved)
   - ready_for_plan → plan_in_progress (starting to write plan)
   - plan_in_progress → plan_in_review (plan written)
   - plan_in_review → ready_for_dev (plan approved)
   - ready_for_dev → in_dev (work started)
   - in_dev → code_review (PR submitted)
   - code_review → done (merged)

3. **Update the task file:**
   - Edit the `status` field in frontmatter
   - Update the `updated` timestamp
   - Consider adding a comment explaining the status change

### 5. Archiving Tasks

When a task is completed or canceled:
1. Move from `agent/tasks/active/` to `agent/tasks/archive/`
2. The file retains the same name

## Important Notes

- Keep tasks concise but complete - aim for scannable content
- All tasks should include a clear "problem to solve" - if the user asks for a task and only gives implementation details, you MUST ask "To write a good task, please explain the problem you're trying to solve from a user perspective"
- Focus on the "what" and "why", include "how" only if well-defined
- Always preserve links to source material in the `links` array
- Don't create tasks from early-stage brainstorming unless requested
- Include code references as: `path/to/file.ext:linenum`
- Ask for clarification rather than guessing status
- Remember - you must get a "Problem to solve"!

## Comment Quality Guidelines

When creating comments, focus on extracting the **most valuable information** for a human reader:

- **Key insights over summaries**: What's the "aha" moment or critical understanding?
- **Decisions and tradeoffs**: What approach was chosen and what it enables/prevents
- **Blockers resolved**: What was preventing progress and how it was addressed
- **State changes**: What's different now and what it means for next steps
- **Surprises or discoveries**: Unexpected findings that affect the work

Avoid:
- Mechanical lists of changes without context
- Restating what's obvious from code diffs
- Generic summaries that don't add value

Remember: The goal is to help a future reader (including yourself) quickly understand what matters about this update.

## Valid Labels

- **bug**: Bug fixes
- **hld**: Related to the hld/ directory (the daemon)
- **wui**: Related to humanlayer-wui/
- **meta**: Related to hlyr commands, thoughts tool, or thoughts/ directory

Note: meta is mutually exclusive with hld/wui. Tasks can have both hld and wui, but not meta with either.
