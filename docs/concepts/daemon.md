---
id: daemon
title: Daemon (buckd)
---

import { FbInternalOnly } from 'docusaurus-plugin-internaldocs-fb/internal';

The first time that a Kuro command is run, Kuro starts a daemon process for
the current project. For subsequent commands, Kuro checks for the running
daemon process and, if found, uses the daemon to execute the command. Using the
Kuro daemon can save significant time as it enables Buck to share cache between
Kuro invocations.

By default, there is 1 daemon per [project](./glossary.md#project) root, you can
run multiple daemons in the same project by specifying an
[isolation dir](./glossary.md#isolation-dir).

While it runs, the Buck daemon process monitors the project's file system for
changes. The Buck daemon excludes from monitoring any subtrees of the project
file system that are specified in the `[project].ignore` setting of
`.buckconfig`.

You can see detailed information about the status of the daemon by running
`kuro status`.

## Killing or disabling the Buck daemon

The Buck daemon process is killed if `kuro clean` or `kuro kill` commands are
run. Note that they won't kill the daemon associated with custom isolation dirs.
To do that, run using the `--isolation-dir` option
(`kuro --isolation-dir <dir> <command>`)

<FbInternalOnly>

The Daemon is also killed when:

- The `kuro killall` command is run.
- A new kuro version is available.

</FbInternalOnly>
