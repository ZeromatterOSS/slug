---
description: Create local task and PR for experimental features after implementation
---

you're working on an experimental feature that didn't get the proper task and pr stuff set up.

assuming you just made a commit, here are the next steps:


1. get the sha of the commit you just made (if you didn't make one, read `.claude/commands/commit.md` and make one)

2. read `.claude/commands/tasks.md` - think deeply about what you just implemented, then create a local task about what you just did, and put it in 'in_dev' status - it should have ### headers for "Problem to Solve" and "Proposed Solution"
3. note the task ID (TASK-XXXX)
4. git checkout main
5. git checkout -b 'dex/TASK-XXXX-description'
6. git cherry-pick 'COMMITHASH'
7. git push -u origin 'BRANCHNAME'
8. gh pr create --fill
9. read '.claude/commands/describe_pr.md' and follow the instructions

