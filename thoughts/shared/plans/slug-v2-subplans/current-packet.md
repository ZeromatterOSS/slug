# Current Slug V2 Work Packet

Packet: WP-7-139-m7a-repository-execute-gcc-probes-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Admit the default local Unix `repository_ctx.execute` calls needed by the
source-authenticated rules_cc 0.2.17 Linux GCC `local_config_cc` probe family.
An offline generated-file GCC fixture must pass, and an authentic Rust-toolchain
`//:probe` replay must advance beyond the current first `execute([cc, "-v"])`
stop. The packet implements a real ordered process bridge, not a fabricated
answer for that first call. It does not claim full `local_config_cc`, C++
toolchain, root Build or M8 closure: `path.dirname` and later independent
Loading/configuration stops remain separate.

For admitted calls, the Starlark argument/default *binding* and result-field
protocol, nonzero-exit-as-result behavior, and Latin-1 projection of captured
bytes are **exact** to pinned Bazel 9.2. Host process outputs, executable
availability, sandboxed filesystem/process/environment view, root path
spelling, request freshness, limits and diagnostics are **Slug-native**
observations. Bazel passes its repository environment to the child; Slug's
admitted GCC probes use a fixed minimal child environment. Arbitrary executable
or `CC` override, Clang/module-map branch, Windows, Label argv, non-default
`timeout`/`environment`/`quiet`/`working_directory`, commands intentionally
writing the repository or host, network access, non-UTF-8 argv/environment,
client-disconnect cancellation while a probe runs, and uncapped process
trees/output are **unsupported/deferred**. No JVM, Java
helper, Bazel semantic delegation, or general process-execution fallback enters
Slug.

## Pinned evidence and selected demand

Bazel 9.2 `StarlarkBaseExternalContext.java:1898-2075` validates String/Label/
StarlarkPath argv, defaults timeout to 600 seconds and environment to `{}`,
uses the repository root as cwd, and launches the local command. Its
`StarlarkExecutionResult.java:59-111,192-250` exposes immutable
`return_code`/`stdout`/`stderr`, captures Latin-1, and returns nonzero status
as a value; upstream `starlark_repository_test.sh:224-305,1369-1393` covers
the family. These source tests are not a proof of Slug's sandbox policy.

rules_cc 0.2.17 `cc/private/toolchain/unix_cc_configure.bzl:115-178,273-283,
337,615` and `lib_cc_configure.bzl:193-196` contain the authentic calls.
The first is `[cc, "-v"]` at line 273, after generated symlinks and
`tools/cpp/empty.cc`; later GCC probes read that generated file, send compile/
link outputs to `/dev/null`, or capture compiler streams. All selected GCC
calls use default execute options. Clang's `readlink` and module-map script
are outside this packet. `unix_cc_configure.bzl:498-507` also demands
`path.dirname` later, which this packet does not quietly claim.

A fresh ignored-RC, download-disabled Bazel 9.2 exploratory probe at
`/tmp/slug-wp839-gcc-execute-ZqF1nG` passed in 3.61 seconds: `gcc -v`
returned 0 and version stderr; preprocessing a preceding generated `empty.cc`
returned stdout `123\n`; an invalid GCC option returned 1 and nonempty
stderr. Its latter two argv forms are *not* authenticated rules_cc calls and
are not admitted by the production guard. The checked-in fixture instead uses
the exact source-derived `-fno-canonical-system-headers` compile-option probe;
the exploratory result demonstrates the semantic gap, not a compatibility
claim. The accepted WP838 authentic smoke reaches the first `execute` with
ambient PATH; its prior file effects and selected owner are the entry.

## Owner and design checkpoint

Loading's selected repository-rule invocation remains the sole semantic
producer. A new typed `ExecuteNeed` carries selected owner, repository ordinal,
call index, typed argv, effective environment, default option/cwd descriptor,
and the exact ordered file-effect prefix. The synchronous Starlark evaluator
never launches a child. On each replay it checks every prior descriptor and
prefix against an immutable prepared transcript before returning the saved
result; the first unprepared call yields a Need. Reordered/skipped/repeated
calls, conflicting Need union, unused transcript entries and changed prefix
fail closed. Existing Label/Host/which/environment observations remain owned
by their current DICE keys and are verified before a child is run.

Core's native-demand session drops the DICE transaction before satisfying the
Need. Before child launch Loading verifies the complete immutable request
environment against `RepositoryEnvironmentSnapshotKey`, and the semantic
transcript retains that raw snapshot structurally with redacted Debug output.
Core stages the ordered prefix in request-scoped scratch, maps only an
audited standalone generated-path argv token to its scratch file, runs the
guarded child, captures a bounded result, and appends it to a command-scoped
ordinal transcript. Each retry injects an immutable execution epoch containing
the command-lease nonce and transcript snapshot before committing a fresh DICE
version. Loading depends on a computed execution-observation key only after
an actual execute call. An active observation is DICE-valid so command selection
retains its exact event and demand closure; a revoked observation is transient.
The lease nonce changes between requests, the transcript changes between
retries, and acceptance or abort revokes the epoch. This forces fresh execution
on each new request, including terminal errors after execute, while
non-executing repositories keep existing reuse. The selected Complete
value retains the structural ordered request/result transcript in its Host
observation, excluding the nonce and any *explicit* scratch-root field. Raw
stdout/stderr may themselves contain the scratch path (notably GCC `-v`), so
those bytes remain structural even when that prevents A→B→A equality cutoff;
never normalize them merely for reuse. Acceptance, timeout and abort revoke
the epoch; no prior process result becomes a cross-request cache.

The runner uses a fresh Linux mount/PID/network namespace and only the
minimum read-only system/compiler mounts, private writable `/tmp`, a private
synthetic `/dev` (including `/dev/null`), and a read-only bind of the disposable staged generated root at
`/exec`. No successful repository write is possible inside the child;
denied attempts need not be detectable if the child ignores `EROFS`.
No host home, credential file,
socket or broad `/` bind is exposed. The guard is source-*derived* by argv
shape, not a hard-coded rules_cc caller allowlist: the offline fixture and
authentic local_config_cc take the same owner path. The only admitted
pre-execute prefixes are the fixture's sole ordinary `tools/cpp/empty.cc`
file, or the authentic source-ordered `cc_toolchain_config.bzl` and
`armeabi_cc_toolchain_config.bzl` symlinks followed by that file. The source
has exact `"int main() {}"` bytes, executable=true (Bazel's `ctx.file`
default), and no symlinked ancestor. The executable must be a
Which-origin path resolving to the trusted root-owned, non-writable
`/usr/bin/gcc` ELF
chain, not a string `CC` override. The full typed argv must match one
source-enumerated GCC form, with no response file, plugin/spec option,
embedded generated token, arbitrary `-B` or output destination. Require
absent `CC`/`CPLUS_INCLUDE_PATH`, default `BAZEL_CONLYOPTS`/
`BAZEL_CXXOPTS`, and system `ld` provenance for linker forms. Launch with
`env_clear` on both the launcher and child, and only `PATH=/usr/bin:/bin`,
`LC_ALL=C`, `TMPDIR=/tmp` and bwrap's fixed `PWD=/exec` inside the namespace;
raw client environment is
never inherited by the child. The raw snapshot remains DICE-owned because
source-level branches may read it. Unknown argv/source/env forms fail closed.

The finite argv grammar is the source-listed `-v`/`--version` pair;
`[source,-o,/dev/null,-Wl,--start-lib,-Wl,--end-lib,-fuse-ld={lld,gold},-v]`;
`[option,-o,/dev/null,-c,source]` with only the source-listed compiler-option
probes; `[force_flags,option,-o,/dev/null,source]` with only the source-listed
linker-option probes and `force_flags` in source order from trusted `/usr/bin`
linker discovery; and the six source-listed `[-E,-xc|-xc++,-,-v,flags]`
include scans plus their conditional `[-print-resource-dir,flags]` companion.
Here `source` is only the exact standalone stringified generated
`tools/cpp/empty.cc` token; even though the first argv is a Which-path object,
this source argv is a String. The guard rejects other options instead of
generalizing these patterns.

The launcher is an explicit, resolved local bwrap prerequisite, never selected
from a repository-controlled PATH. On this host it is Homebrew-installed under
`/home/linuxbrew/.linuxbrew/bin`; that user-owned binary is a trusted host-tool
assumption, not a proven immutable/root-owned component. An actor able to
replace same-user host tooling outside the workspace is outside this packet's
sandbox guarantee. Core validates the launcher path/type/executable bits and
rejects workspace aliasing, but does not attest its bytes. This limitation is
**Slug-native** and must not be presented as Bazel-exact isolation.

### Low-commit launcher correction after resource-blocked review

The authentic smoke reached the launcher, but `tokio::process::Command`'s
`pre_exec` closure forced Rust's `fork` route and failed with host `ENOMEM`
while committed memory exceeded the host commit limit. Keep the same guarded
argv, bwrap mounts, status-JSON protocol, limits and process-group ownership;
change only the Linux/GNU launcher transport to glibc `posix_spawn` so it can
use its low-commit `CLONE_VM|CLONE_VFORK` route. This execute slice requires
`POSIX_SPAWN_SETSID` and `posix_spawn_file_actions_addclosefrom_np` (glibc
2.34 or newer). Resolve the latter symbol at runtime and fail only this runner
closed when absent: do not make the whole Slug binary require glibc 2.34 merely
to load. Other Linux libcs/older glibc are **unsupported/deferred** for this
execute slice. No shell/helper, semantic fallback, or fork fallback is admitted.

Before constructing file actions, normalize every parent source descriptor
(both pipe write ends and the status file) to distinct CLOEXEC FDs at least 4;
daemon stdio and FD 3 may already be closed or reused. Ordered file actions
set stdin to `/dev/null`, route bounded stdout/stderr pipes, duplicate only
the isolated status file onto FD 3, then close every FD from 4 upward before
exec. The bwrap launcher receives an empty environment and
remains outside any repository-controlled PATH. `POSIX_SPAWN_SETSID` gives it
a unique process group. Immediately after successful spawn, the parent closes
all pipe write ends and normalized source copies, retaining only pipe read
ends and the isolated status reader; failed spawn closes every owned FD.
The raw launcher PID remains unreaped until both pipes
reach EOF; then a nonblocking `waitpid` loop runs within the same effective
timeout. On output-limit, timeout or future drop, an RAII owner signals the
group and ensures eventual reap independently of caller cancellation. Disarm
that owner immediately upon successful reap, before another await. Unexpected
`ECHILD` is infrastructure failure and must never trigger a signal to a
possibly reused process-group ID. An absent
status record remains a launcher/setup failure, never a compiler result.
The owned pipe buffers are command/async scratch; no new DICE key, retained
cache, transcript identity, or semantic owner is introduced.

Independent DESIGN correction review accepted this bounded transport on
2026-09-24; it did not accept implementation or authentic progression. Final
rereview must cover spawn/file-action order, inherited-FD
closure including an intentionally writable host FD, runtime absence of the
close-from symbol, a parent with 0/1/2/3
closed or reassigned, child/status FD isolation, launcher/setup versus
nonzero-child outcomes, early launcher exit
with a descendant holding output open, output/timeout/drop cleanup and PID
reuse safety. Rebuild the CLI and rerun the same authentic cquery only after
these focused gates; record host commit headroom around that one retry. If
`posix_spawn` itself fails from resources, preserve the candidate and report
the new environmental evidence rather than weakening isolation.

A read-only mount can hide a denied write if a child ignores `EROFS`,
so source guarding and the Slug-native classification remain essential.
An overlay's final upper-layer scan is insufficient: a create-then-delete
inside the child leaves no residual mutation. Missing sandbox support never
falls back to direct local execution.
The command transcript has a fixed maximum of 128 calls; exceeding it fails
before another Need is issued. The runner closes stdin, caps output and effective time (at most 30 seconds
per probe, Slug-native despite Bazel's 600-second default), kills/reaps the
PID-namespace child tree on timeout or runner-future drop, and drops scratch
on all terminals. Current synchronous command driving does not expose a
client-disconnect signal while `progress` blocks; do not claim that form of
cancellation or leave a child alive past the effective timeout. The staged
root is never a selected generated effect or a path source for publication;
captured streams may contain its path as inert bytes and remain structural.
No shared lock spans DICE or process execution.

New retained transcript/request values reuse `CompactString`, `Arc`/`Dupe`,
small deterministic maps and `Allocative`; no interner, weak hash identity,
service-global process cache or V1/Buck2 runtime import. The transcript is
DICE-retained semantic memory; child output buffers and provisional roots are
command/async scratch released at acceptance or abort. The matched Stage 9
disposition must be recorded before acceptance. This is a cold repository-rule
path, not a demonstrated hot-path optimization, so no benchmark claim.

## Proof and validation

Add one offline Stage 1 fixture `repository-rule-execute-gcc-probes` with
fixture-level bounded `PATH=/usr/bin:/bin`, a preceding generated
`tools/cpp/empty.cc`, `which("gcc")`-path argv, source-authentic `-v`/
`--version`, and the exact `unix_cc_configure.bzl:144-154,216-235,561-566`
`-fno-canonical-system-headers` compile probe writing `/dev/null`; use one
anchored query row. Its manifest cites the pinned source and fresh probe, and
records fresh ignored-RC generation, independent verification and rebuilt-Slug
replay. A prepared-transcript Loading test covers nonzero exit without
admitting a fabricated compiler flag. Do not copy the temporary Bazel output
base or credential material. The existing WP838 fixture protects `which`.

Focused tests must discriminate two identical argv calls by call index, exact
prefix/descriptor replay and conflict, result fields including nonzero status,
generated-path token mapping and rejection of embedded/foreign tokens, changed
environment/compiler between commands, fixed child environment that omits
secret/loader variables, rejected `CC` and dynamic compiler-option overrides,
same-DICE and independent-request A→B→A, terminal error transience,
abort/revocation, bounded stdout/stderr,
128-call cap, timeout/runner-drop and child reap. Sandbox tests must prove read-only staged
root enforcement (including create-then-delete attempts), host-write
confinement, network denial, unavailable-bwrap failure,
and no home/credential mount. Source-authentic smoke reports the first new
stop without claiming the whole route. Use the current Core writer/manifest
for materialization assertions; preserve WP837 symlink and WP838 which gates.

Serialize Cargo; resolve the pinned toolchain, compile JSON `--no-run`, run
exact-selector preflight and focused Loading/Bzlmod/Core tests plus direct
CLI compile. Rebuild `slug_cli_v2` before `SLUG_V2_BIN` oracle/smoke replay;
clean stale `slugd` before/after. Run format, diff, plan-status, archive and
fixture hygiene. An independent DESIGN review must accept this ownership and
sandbox contract before production edits; a FINAL review must inspect actual
diff and evidence. This is a Linux-specific capability: test fail-closed
unavailability and the actual supported host target, with Windows explicitly
deferred.

## Scope, stops and immediate predecessor

Base clean main `648cb001f`. Allowlist: this manifest/canonical Current;
`app/slug_bzlmod_v2/src/{source_preparation,generated_repository_file_effect,
repository_execute,lib}.rs` (one new typed module);
`app/slug_loading_v2/src/{repository_rule_context,
module_extension_repository_file_effect}.rs`;
`app/slug_core_v2/src/runtime/{dice,repository_io,repository_process,mod}.rs`
(one new runner module) and colocated tests; the named fixture; compact Stage 5
owner and Stage 9 disposition notes after acceptance. New boundary across
three crates and Core is a size/review trigger, not permission to widen
behavior. No action execution, cache transport, root Build, general path text,
compiler flags, source catalog, or configuration identity changes.

`REPLAN` if minimal sandbox mounts cannot run the authenticated GCC calls,
if repository writes cannot be excluded for an admitted form, if DICE request
revision/transience cannot prevent stale successful or error results, if
pre-execute effects cannot be staged without bypassing Core's owner, or if
authentic compiler flags demand a materially different process policy. WP838
at `648cb001f` remains accepted and unchanged; M7A remains partial.

## Candidate validation and resource hold (2026-09-24)

Fresh ignored-RC Bazel 9.2 fixture generation and independent verification
passed the one-row offline GCC probe. The rebuilt `slug_cli_v2` passed its
matching Slug fixture twice, including after the sandbox corrections. Pinned
JSON no-run compiles passed for Bzlmod, Loading and Core; exact-selector
preflight passed for their focused tests. The revised lease-scoped epoch test,
same-DICE fresh-nonce A→B→A Loading test and all 13 focused Core runner tests
passed, including inherited-FD isolation, create-then-delete denial,
early-exit process-group ownership, timeout and cancellation. `cargo fmt
--all -- --check`, `git diff --check` and plan status passed; no stale
`slugd` remained before or after the smokes.

The rebuilt authentic Rust-toolchain `cquery //:probe --output=label` from
`/tmp/slug-wp797-authentic-2YQmSt` reached the runner but exited 2 after
about 45 seconds with `repository execute launcher: sandbox launch failed
(os error 12)`. The host reported `vm.overcommit_memory=0`,
`Committed_AS=50,847,224 kB` above `CommitLimit=48,427,772 kB`, and an
unrelated process using about 20 GiB RSS. This is a fail-closed host resource
stop, not proof that the authenticated GCC sequence passed or that its sandbox
contract is impossible. Do not kill unrelated processes or weaken isolation.
Independent FINAL review returned **REVISE** only for this missing authentic
progression; it found no remaining FD/PGID or finite-guard code blocker. The
low-commit correction and second authentic replay below supersede this
resource hold. No M7A closure was claimed at that point.

## Accepted low-commit correction (2026-09-24)

The Linux/GNU runner now uses direct glibc `posix_spawn` with `SETSID` and a
runtime-gated close-from file action. It preserves the same guarded GCC
forms, staged read-only sandbox, fixed child environment, status protocol,
bounded output/time and DICE transcript owner. Source FDs are distinct
CLOEXEC descriptors at least 4, then ordered file actions map stdin,
stdout, stderr and status FD 3 before closing all higher child FDs. The
parent retains an unreaped raw launcher PID through pipe EOF, kills its
process group on timeout/error/drop only while ownership is verified, and
reaps independently of caller cancellation. Interrupted output reads and
blocking waits retry. Missing glibc close-from support fails only this
execute slice closed; no shell/fork semantic fallback is admitted.

Pinned Core JSON no-run passed; exact-selector preflight found 18 focused
runner tests and all 18 passed, including closed/reassigned 0–3, inherited
FD 9, status isolation, descendant-held pipe, timeout, cancellation, and
injected EINTR cases. The rebuilt CLI passed a fresh one-row Slug replay of
`repository-rule-execute-gcc-probes` at
`/tmp/slug-wp839-fixture-final-oS9Uae`; the fresh ignored-RC Bazel 9.2
generation and independent fixture verification above remain the accepted
oracle receipt. Format, diff, plan-status and archive checks passed, with no
stale `slugd` before or after smoke. Independent FINAL rereview returned
**ACCEPT** after the EINTR correction.

The rebuilt authentic Rust-toolchain `cquery //:probe --output=label` from
`/tmp/slug-wp797-authentic-2YQmSt` no longer failed with `ENOMEM`; it ran
about 60 seconds and stopped at the unchanged WP838 64-candidate
`repository_ctx.which` limit. The pinned rules_cc 0.2.17
`unix_cc_configure.bzl:272-273,332-340,376-417` source orders a GCC lookup
and two `execute([cc, "-v"])` calls before the later tool lookups that can
exhaust that limit. Thus authentic progression through the first
two GCC probes is accepted, but `--version`, compile probes, full
`local_config_cc`, and root C++ toolchain remain unproven. Pre/post host
receipts were `Committed_AS=48,296,604/48,364,088 kB` against
`CommitLimit=48,427,772 kB`; no ambient credentials or Bazel RC contents
were inspected or copied. The later `which` boundary belongs to a successor
packet; M7A remains partial.
