---
id: exit_codes
title: Exit Codes
---

These exit codes are returned from a Kuro command to the shell when the command
exits.

These exit codes are Kuro's binary protocol for interacting with other software
such as shell scripts.

| Exit Code | Exit Status        | Description                                                                                                           |
| --------- | ------------------ | --------------------------------------------------------------------------------------------------------------------- |
| 0         | Success            | The command returned successfully.                                                                                    |
| 1         | UnknownFailure     | Command failed with an error that Kuro was unable to identify                                                        |
| 2         | InfraError         | Error caused by the underlying infrastructure, such as Kuro itself, the File System, etc.                            |
| 3         | UserError          | Error caused by user actions, such as wrong arguments, typos, etc.                                                    |
| 4         | DaemonIsBusy       | `--exit-when differentstate` commands only. Daemon is connected and busy with another command                         |
| 5         | DaemonPreempted    | `--preemptible` commands only. Kuro daemon preempted the command as another came in.                                 |
| 6         | Timeout            | Command execution exceeded time limit                                                                                 |
| 11        | ConnectError       | Kuro client failed to connect to Kuro daemon                                                                        |
| 32        | TestError          | Kuro Test only. Build succeeded but at least 1 test failed                                                           |
| 64        | TestNothing        | Kuro Test only. Build succeeded but no test were ran (Either no tests defined or tests are skipped)                  |
| 129-192   | SignalInterruption | The code is computed as 128 + signal number. If Kuro exited due to SIGINT(2) for example, the exit code would be 130 |
