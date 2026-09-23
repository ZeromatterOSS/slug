# Current Slug V2 Work Packet

Packet: WP-7-85-m7a-linux-runshell-transport-restoration-r1
Status: accepted; independent final ACCEPT

## Outcome and compatibility

Restore the previously accepted `shell-action-reapi` Linux remote-execution
slice through the ordinary selected Build path. On clean main `8fd6e0601`,
the existing Slug fixture starts local NativeLink 1.7.1 but fails before
transport: Core's source staging rejects a typed `run_shell` invocation as
not artifact-backed. A separate fresh Bazel 9.2 `aquery` of that fixture
shows `/bin/bash -c 'printf reapi > $1' '' <output>`; pinned
`StarlarkActionFactory.java:627-650`, `ShellCommand.java:20-47`,
`ShToolchain.java:35-58`, `ShellConfiguration.java:76-88`, and
`ConstraintConstants.java:27-94` establish the ordered argv and selected
execution-platform shell choice. The checked-in Bazel 9.2 fixture expects a
5-byte `reapi` output with SHA-256
`ac0cb855e0243634730f146e7b14a0dbc8ed0c3271e7b6ca4974c116a87f2a28`.
Its earlier accepted Stage 7 receipt recorded one REAPI action and zero
direct-local actions.

Exact: the short string-command `ctx.actions.run_shell` shell argv/padding
and source/generated input ownership when the selected execution platform is
Linux, the process-latched `BAZEL_SH` is absent, and explicit
`--shell_executable` is absent. Existing source/content/CAS digests stay
exact. Slug-native: structural configuration/action identity and output-root
tokens; REAPI Action digest is exact only for Slug's actual wire graph.
Unsupported/deferred: non-Linux or unknown selected-platform shell paths,
nonempty `BAZEL_SH`, explicit `--shell_executable`, command-list form,
Bazel's generated helper script for commands over 64,000 Java UTF-16 units,
unowned string executable paths, unrelated `rules_java` repository probing,
and exact Bazel ActionKey/output-identity bytes. Do not label the whole Spawn
family exact from this fixture.

## Decision and owners

`ProcessHostOwner` latches `BAZEL_SH` on non-Windows hosts as well as
Windows, without opening a new process cache. The existing
`ActionEnvironmentHost` field and `SlugConfiguration` canonical structural
bytes retain its absence/presence and invalidate configured analysis. An
explicit value fails closed for this Linux-default shell packet, including if
it happens to equal `/bin/bash`; do not read ambient environment in Analysis
or REAPI. Existing configured action-environment policy continues to reject
explicit `--shell_executable`.

The selected execution-group `ConfiguredActionOwnerContext` supplies the
resolved platform constraints. Reuse the pinned `@platforms//os:os` versus
Host fallback policy already implemented for `exec_os` in
`starlark_rule/cc_context.rs`; admit only a proven Linux result. A selected
unknown/non-Linux OS must fail before publishing a configured shell action.
The DICE-owned configured action retains the chosen `/bin/bash` shell path
alongside command and `$0` padding in the existing
`RetainedSpawnInvocation::Shell`; its publication equality includes that
path. This path is a platform-provided executable, not an invented source
artifact or CAS input. Do not add a DICE key, interner, side cache, lock, or
untracked Host read. Reuse `CompactString`, `Allocative`, existing shared
action/context handles and the Stage 9 retained-action utility disposition;
the extra field is paid only by shell actions, with no performance claim.

Core's shared declared-artifact traversal must continue collecting every
declared input and tool, and additionally admit the guarded typed Shell without
adding an executable artifact. Keep arbitrary `SpawnExecutable::Path`,
runfiles/pruning/generated-source limits and source certificate/Need/error
behavior fail-closed. The same traversal feeds source-only staging and
generated-prerequisite closure, so both must preserve exact dependency
relationships. Shared command-line expansion must emit the retained
`/bin/bash`, `-c`, command, optional empty `$0`, then authored arguments,
with virtual parameter-file bytes and replacement argv from one expansion.
Both REAPI source-only and action-chain projections use that expansion;
they must not synthesize a shell blob or bypass fresh-frontier validation.
The selected command and full source/Host/configuration identity participate
in DICE publication; command result sets and RPC buffers remain request-local.

## Scope and stops

Base `8fd6e0601`; review branch
`codex/wp785-linux-runshell-review`. Root owns scheduling, integration, serial Cargo/CLI,
NativeLink oracle, receipt and commit. Production allowlist:
`app/slug_configuration_v2/src/native/{host,configuration}.rs`,
`app/slug_core_v2/src/runtime/{process_host,source_staging,action_chain_execution,configured_output}.rs`,
`app/slug_build_api_v2/src/actions/spec.rs`,
`app/slug_analysis_v2/src/{files_to_run_spawn,starlark_rule,result}.rs`,
`app/slug_analysis_v2/src/starlark_rule/cc_context.rs`, and
`app/slug_reapi_v2/src/{source_spawn,source_staging,action_chain}.rs` only
if defensive projection changes are required. Tests may change only alongside
those owners in existing focused test modules/files. Amend this allowlist after
review if a concrete direct consumer requires it. The ordinary requested-action
consumer required preparing its configured-output marker directory before
source-frontier observation: otherwise its own first claim changes the
workspace root `Lstat` and fails the pre-Execute freshness gate. Do not edit fixture expected
bytes, Bazel sources, MODULE/lockfiles, generic executable/OS parsing, action
transport policy, or unrelated BuildBuddy configuration. Independent design
and final review are mandatory for the retained invocation and execution
boundary. Keep concurrent writers disjoint; serialize Cargo. The touched
configuration, action-result, and Starlark-rule files are already large, but
each edit stays at its existing producer/attachment boundary; extract a
helper only if the shell policy otherwise duplicates the existing exec-OS
classification. No central action dispatcher or second identity is
introduced.

Stop and REPLAN if selected exec-group constraints are unavailable at the
configured action attachment, the process-latched `BAZEL_SH` fact cannot
structurally reach configuration equality, the fixture requires an unmodeled
helper script, or restoring Shell would bypass another action-family guard.
Do not pass by changing fixture outputs, weakening negative assertions,
falling back to a local process, or fabricating an executable source.

## Acceptance and validation

The existing `shell-action-reapi` fixture must reach successful Slug remote
execution with a newly rebuilt `slug_cli_v2` and fresh supervised NativeLink
1.7.1: one remote action, zero direct-local actions, exact verified output
digest/size/mode under the Slug-native configured root, and no leftover
`slugd` or NativeLink process. The raw Bazel-vs-Slug fixture comparison is
expected to report a manifest difference because Bazel's `bazel-bin/pkg`
surface is deferred and Slug publishes under `bazel-out/slugcfg-v2-*/bin/pkg`;
do not widen output identity just to make that comparison green. Add focused discriminators for
no-argument versus argument padding, source and generated inputs, changed
command/path publication equality, absent/present/absent `BAZEL_SH`
configuration identity, explicit-shell-option and non-Linux/unknown
platform refusal, long command refusal, unowned path rejection and unchanged
source-certificate A/B/A. Do not require a new oracle fixture: the existing
Bazel-generated fixture plus pinned source covers this bounded restoration.
The direct FileWrite real-transport regression is already verified on this
base with one selected ignored test and NativeLink 1.7.1; preserve it.

Resolve the pinned nightly tools before Rust validation. Use offline/locked
Cargo JSON `--no-run`, exact nonignored test preflight and counted execution
for changed owners and direct dependents; supervise any selected ignored
NativeLink test separately. Rebuild `slug_cli_v2` before binary comparison.
Run changed Rust formatting, plan/archive status, `git diff --check` and
independent final review. Clean stale `slugd` before and after the fixture.
Never inspect, print, copy or commit `~/.bazelrc` or BuildBuddy authentication.
An estimated compile phase over 30 seconds is preparation, not a test-runtime
ceiling; keep each behavior test minimal.

## Immediate predecessor and diagnostic receipt

WP784-r2 at `8fd6e0601` independently accepted authentic
`@bazel_tools//tools/cpp` nonvisible alias loading and source-derived
rule-toolchain Query edges with its three Bazel 9.2 outcomes. This packet
does not reopen its Loading/Query contract. On 2026-09-23 the installed
`../nativelink/target/release/nativelink` reported version 1.7.1. The
direct `slug_reapi_v2` ignored FileWrite test passed 1/1 against a supervised
fresh service, exercising CAS/Execute, output integrity and AC hit. The
`shell-action-reapi` fixture failed before transport with
`source staging requires an artifact-backed executable`; NativeLink started
and stopped cleanly. `simple-rule-action` failed separately during an
unrelated `rules_java++toolchains+local_jdk` repository `which` limit
before transport and is not a pass or a shell regression. The first
relative-path fixture invocation was an invocation error, corrected with an
absolute Slug binary path. No source files were changed by those diagnostics.

## Implementation receipt (pending final review)

The guarded retained Shell now carries `/bin/bash` in publication equality;
shared argv expansion emits `shell,-c,command[,empty $0],arguments`. Analysis
requires a selected Linux execution platform, absent process-latched
`BAZEL_SH`, absent explicit shell option and a short string command. Core
admits that typed invocation while still rejecting unowned executable paths
and generated sources in source-only staging. REAPI uses the shared argv and
has a defensive shell-path check. The ordinary requested-action and selected
chain entry points prepare the configured-output marker directory before
source-frontier observation; a diagnostic first showed that its first claim
otherwise changed the workspace-root `Lstat` at the pre-Execute check.

Offline/locked compile and exact test preflight succeeded for configuration,
Build API, Analysis, Core and REAPI. Counted selected tests passed: 2
configuration, 3 Build API, 4 Analysis, 10 Core (including marker stability,
requested publication, source-change retries and Host latching), and 2 REAPI.
The newly rebuilt CLI exited zero in a fresh `shell-action-reapi` run against
supervised NativeLink 1.7.1. Evidence reported one REAPI action, zero
direct-local actions, one AC miss, and one materialized
`ac0cb855e0243634730f146e7b14a0dbc8ed0c3271e7b6ca4974c116a87f2a28/5`.
The published `bazel-out/slugcfg-v2-*/bin/pkg/probe.txt` was independently
SHA-256 checked, 5 bytes and mode `0555`; no `slugd` or NativeLink process
remained. The raw fixture command reports a **manifest-only mismatch** because
its Bazel root is `bazel-bin/pkg` and the Slug-native output-root domain is
explicitly deferred. Fixture expected bytes were not edited. Following
independent review, the exact ignored FileWrite test was recompiled and
rerun against a new supervised NativeLink 1.7.1 instance from current
sources: 1/1 passed in 0.21s, and no service remained. This is direct
REAPI transport/AC evidence; the Shell CLI and focused Core tests cover the
changed requested-action marker preparation.
Independent final review returned ACCEPT with that evidence classification
and the raw manifest-only difference retained as Slug-native/deferred.
