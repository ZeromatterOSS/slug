# Plan 34: Sandboxed execution strategy

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Follows Plan 8 / Phase 16's functional Linux sandbox prototype and Plan 32's
> local-overhead work. This plan owns a production sandboxed execution strategy
> for local actions.
>
> Depends on [Plan 44](./44-workspace-layout-parity.md)'s action-layout
> invariants. Plan 34 enforces the declared-input/output contract; it does
> not choose a separate cwd, external-repo, or output-root model.

## Goal

Add a sandboxed local execution strategy that catches undeclared input reads,
prevents undeclared or overlapping writes, and gives Slug a clear network
sandboxing policy without losing the local-execution performance advantage.

The target is functionality first, not exact Bazel implementation shape:

- A rule that reads an undeclared source or generated input fails locally.
- A rule that writes outside declared outputs fails locally.
- Two actions cannot race on the same final output path.
- Network access is denied unless the action explicitly allows it.
- Linux is first tier. Windows is supported as second tier, with explicit
  capability gaps rather than silent success.

## Current Slug State

Slug already has a `slug_sandbox` crate and a `--sandbox` flag, but the current
implementation is a prototype rather than the final strategy.

Observed implementation shape:

- `app/slug_sandbox/src/lib.rs` applies Linux user and mount namespace setup via
  `std::process::Command::pre_exec`.
- The sandbox remounts the inherited filesystem read-only, bind-mounts output
  directories writable, and overlays `buck-out` with a staging tree of declared
  build artifacts.
- Source files under the workspace remain generally visible.
- Sandbox setup failures are printed to stderr and execution continues without
  isolation.
- Non-Linux platforms are a no-op.
- `app/slug_execute_impl/src/executors/local.rs` builds a `SandboxSpec` from
  action outputs and some artifact inputs, then bypasses the forkserver when a
  sandbox is present.

Those properties are enough to prove the direction, but not enough for a
sandboxed execution strategy:

- Failing open is not acceptable when the user requested a sandbox.
- `pre_exec` is the wrong place for complex filesystem setup.
- Undeclared source reads are not caught.
- Input collection does not use the full `CommandExecutionPaths` input
  directory model.
- Network sandboxing is absent.
- Persistent workers and the forkserver are bypassed, which matters for local
  performance.
- Windows has no enforcement backend.

## Background Research

### Bazel Prior Art

Bazel's public sandboxing docs define the behavior we care about: local
execution should run actions in a restricted filesystem view so undeclared input
reads fail and output interference is contained. Bazel uses several strategies:

- `processwrapper-sandbox`: builds a per-action symlink forest containing known
  inputs, runs the action there, then moves outputs out.
- `linux-sandbox`: adds Linux user, mount, PID, IPC, and network namespaces on
  top of the process-wrapper model.
- Darwin and Windows have platform-specific sandbox runners.
- `sandboxfs` was an optional FUSE-based optimization for building sandbox views,
  but it is not the obvious first choice today.

Bazel source-of-truth references to consult when matching behavior:

- `src/main/java/com/google/devtools/build/lib/sandbox/`
- `src/main/tools/linux-sandbox/linux-sandbox.cc`
- `src/test/java/com/google/devtools/build/lib/sandbox/`
- `src/test/shell/integration/sandboxing_test.sh`

Useful public resources:

- Bazel sandboxing docs: <https://bazel.build/docs/sandboxing>
- Bazel sandboxfs repository: <https://github.com/bazelbuild/sandboxfs>

Slug should borrow the behavioral contract and tests, not necessarily the exact
runner structure.

### Linux Filesystem Options

#### Landlock

Landlock is the best candidate for Slug's default Linux fast path.

Properties:

- Unprivileged and inherited by child processes.
- Enforces path-based filesystem allowlists in-kernel.
- Can restrict read and write access without a mount namespace.
- Newer ABI versions also support TCP bind/connect restrictions.
- Low setup cost compared with constructing many bind mounts.

Tradeoffs:

- It does not create a virtual execroot. Slug still needs a per-action input and
  output directory layout.
- It is one-way: once restricted, the process cannot regain access.
- Feature availability depends on kernel Landlock ABI version.
- It cannot express every namespace-style view, so a namespace backend is still
  needed for stricter isolation or older kernels.

Resources:

- Rust crate: <https://docs.rs/landlock/latest/landlock/>
- Kernel docs: <https://docs.kernel.org/userspace-api/landlock.html>

#### User and Mount Namespaces

Direct namespace setup with `nix`, `rustix`, or `libc` gives the strongest Linux
filesystem model.

Properties:

- Can expose only the sandbox execroot, selected tools, `/proc`, `/dev/null`,
  `/dev/urandom`, tmp, and declared outputs.
- Can make the rest of the filesystem read-only or invisible.
- Can pair with a network namespace for no-network actions.

Tradeoffs:

- More setup work per action.
- Kernel and distribution settings may block unprivileged user namespaces.
- Correct mount setup is subtle and should happen in a helper process, not
  `pre_exec`.

Resources:

- `nix` crate namespace APIs: <https://docs.rs/nix/latest/nix/sched/>
- `rustix` crate: <https://docs.rs/rustix/latest/rustix/>
- Bubblewrap reference implementation: <https://man.archlinux.org/man/extra/bubblewrap/bwrap.1.en>

#### Seccomp

Seccomp is useful for denying socket syscalls or hardening child processes, but
it is not a filesystem sandbox. It can be a secondary network or syscall
backend, not the primary input/output enforcement mechanism.

Resources:

- rust-vmm `seccompiler`: <https://github.com/rust-vmm/seccompiler>
- `libseccomp` bindings: <https://docs.rs/libseccomp/latest/libseccomp/>

#### FUSE and sandboxfs

FUSE can present an action-specific filesystem tree without creating thousands
of symlinks. Rust crates such as `fuser` and `fuse3` make this possible.

Tradeoffs:

- Per-I/O overhead can hurt compiler-heavy builds.
- A daemon filesystem adds lifecycle and cache-invalidation complexity.
- Bazel's `sandboxfs` is useful prior art, but not a dependency to bet the first
  production strategy on.

Resources:

- `fuser`: <https://docs.rs/fuser/latest/fuser/>
- `fuse3`: <https://docs.rs/fuse3/latest/fuse3/>

#### fanotify

fanotify permission events can observe and sometimes block file opens, but it is
not a good default sandbox substrate for Slug.

Tradeoffs:

- Permission events generally need privileges or capabilities.
- Per-open mediation is expensive.
- It is Linux-only and awkward to make deterministic across filesystems.

Use it, if at all, as a validation or diagnostics tool during rollout.

### Rust Capability Libraries

`cap-std` and related crates are useful for Slug-owned code that can be written
against capability handles. They do not constrain arbitrary compiler or linker
subprocesses that call normal OS APIs, so they are not the action sandbox.

Resources:

- `cap-std`: <https://docs.rs/cap-std/latest/cap_std/>

### Container Runtimes

OCI runtimes such as `youki` prove Rust can drive namespaces and cgroups, but a
full container runtime is too heavy for per-action build sandboxing. The useful
lesson is to isolate the unsafe OS setup in a small, auditable component.

Resources:

- `youki`: <https://github.com/containers/youki>

### Windows Options

Windows should be second tier but real: either enforce the requested sandbox or
report that the strict strategy is unavailable.

Relevant primitives:

- AppContainer / LPAC restricted tokens for filesystem and network capability
  isolation.
- ACL grants on the per-action sandbox root, output root, and required tools.
- Job Objects for process-tree lifetime and resource control. Slug already has
  Job Object process management in `app/slug_execute_local/src/win/`.

Tradeoffs:

- Windows path canonicalization, symlinks, junctions, and case-insensitivity are
  correctness hazards.
- AppContainer process creation is more invasive than Linux Landlock.
- Toolchains may require broad read access at first; those grants must be
  explicit and auditable.

Resources:

- AppContainer implementation docs: <https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer>
- Job Objects docs: <https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects>
- `rappct` crate: <https://docs.rs/rappct/latest/rappct/>

## Design Direction

Implement a small `slug-sandbox-helper` binary plus a Rust library API used by
the local executor.

The parent process should:

1. Build a complete action sandbox spec from `CommandExecutionRequest` and
   `CommandExecutionPaths`.
2. Materialize a per-action input tree containing only declared inputs.
3. Allocate private tmp, scratch, and output directories.
4. Claim final output paths before execution.
5. Spawn the helper with the spec and command.
6. After success, validate and commit declared outputs to their final artifact
   paths.

The helper should:

1. Apply the selected OS sandbox backend.
2. Enter the sandbox working directory.
3. Apply environment and network policy.
4. `exec` the real command.

This keeps unsafe and platform-specific setup out of `pre_exec`, lets sandbox
setup fail closed, and gives us one place to test backend availability.

### SandboxSpec v2

Replace the current minimal spec with a complete action execution contract:

```rust
SandboxSpec {
    action_digest,
    working_directory,
    argv,
    env,
    input_root,
    output_root,
    tmp_root,
    declared_inputs,
    declared_outputs,
    read_allowlist,
    write_allowlist,
    network_policy,
    platform_policy,
    debug_preserve_sandbox,
}
```

Important modeling requirements:

- `declared_inputs` must come from the full input directory, including action
  metadata blobs, scratch inputs, tree artifacts, and incremental remote output
  inputs.
- `declared_outputs` must use normalized project-relative artifact paths and
  output types.
- Toolchain and system reads must be explicit in `read_allowlist`. Broad
  allowlists are acceptable early if logged and measurable.
- Final `buck-out` output paths should not be writable by the action. The action
  writes private outputs; Slug commits them after validation.

### Strategy Selection

Expose strategy as an execution strategy, not just a boolean:

- `local`: existing unsandboxed local execution.
- `sandboxed`: default sandboxed local strategy when enabled.
- `sandboxed-linux-landlock`: Linux fast path.
- `sandboxed-linux-namespace`: Linux strict namespace path.
- `sandboxed-windows-appcontainer`: Windows second-tier strict path.
- `sandboxed-portable`: directory-only layout for debugging and bootstrap, not a
  strict sandbox unless paired with an OS enforcement backend.

When a strict sandboxed strategy is requested and the backend is unavailable,
the action must fail with a clear error. A separate best-effort/debug mode may
exist, but it must be named as such.

## Phases

### 34.1 Define the execution contract and fail-closed behavior (OPEN)

Deliverables:

- Replace or extend `SandboxSpec` with the v2 contract.
- Add a backend availability probe that reports Landlock ABI, namespace support,
  and Windows AppContainer availability.
- Make sandbox setup failure an action failure for strict sandbox strategies.
- Record sandbox strategy and backend in action execution events.
- Respect `no-sandbox` execution requirements by routing those actions through
  unsandboxed local execution with a visible reason.

Success criteria:

- `--sandbox` no longer silently runs unsandboxed after setup failure.
- Unsupported platforms/backends produce deterministic diagnostics.
- Action events identify the sandbox strategy used.

### 34.2 Private execroot, output claims, and atomic commit (OPEN)

Deliverables:

- Build a per-action execroot/input tree from `CommandExecutionPaths`.
- Run actions from the sandbox execroot rather than the real workspace.
- Allocate private output and tmp roots per action.
- Add an output-claim table that rejects:
  - identical output paths claimed by two running actions,
  - parent/child output overlaps,
  - output paths outside the artifact output root,
  - symlink or junction escapes after canonicalization.
- After action success, validate that only declared outputs were produced and
  atomically commit them to final artifact paths.

Success criteria:

- An undeclared source input read fails.
- An undeclared generated input read fails.
- An undeclared write outside declared outputs fails.
- Two actions with overlapping final outputs cannot execute concurrently.
- Failed actions do not leave partial final outputs.

### 34.3 Linux Landlock fast path (OPEN)

Deliverables:

- Implement a Landlock backend in the helper.
- Allow reads only from:
  - sandbox input root,
  - explicit toolchain/system read allowlist,
  - necessary runtime files such as dynamic loader, certs when allowed, and
    `/proc` entries if needed.
- Allow writes only to private output and tmp roots.
- Use Landlock network rules where supported by the kernel ABI.
- Fall back to namespace backend only when strategy policy allows it.

Success criteria:

- Landlock sandbox overhead is measured on Plan 32's local benchmark harness.
- The fast path catches the same filesystem isolation tests as namespace mode
  for normal actions.
- Kernels without sufficient Landlock support produce a clear fallback or
  failure, depending on selected strategy.

### 34.4 Linux namespace strict path (OPEN)

Deliverables:

- Move namespace setup from `pre_exec` into `slug-sandbox-helper`.
- Construct a minimal mount view:
  - sandbox execroot as the working tree,
  - declared inputs visible at action paths,
  - private output/tmp writable,
  - selected tools and system paths read-only,
  - `/proc` and `/dev` minimized.
- Add optional network namespace isolation for no-network actions.
- Preserve process-tree cleanup semantics.

Success criteria:

- Namespace mode passes the filesystem and network sandbox E2E suite.
- Unprivileged namespace unavailability is diagnosed before action execution.
- `--sandbox-debug` can preserve the sandbox tree for inspection.

### 34.5 Network policy (OPEN)

Deliverables:

- Model network policy as an action property:
  - `Deny`,
  - `Allow`,
  - optionally `LoopbackOnly` if needed by test runners.
- Map execution requirements and config flags into that policy.
- Keep repository fetching and module resolution outside ordinary action network
  access; those operations stay in repository/module computations with lockfile
  inputs.
- Implement Linux policy through network namespaces, Landlock network rules, or
  seccomp as appropriate.
- Implement Windows policy through AppContainer capabilities where possible.

Success criteria:

- A build action that opens an external TCP connection fails under default
  sandboxing.
- A network-allowed action can connect when explicitly configured.
- Repository rules that intentionally fetch are not accidentally routed through
  ordinary action sandbox policy.

### 34.6 Windows second-tier backend (OPEN)

Deliverables:

- Implement private execroot/output commit on Windows first.
- Add AppContainer or LPAC process launch for strict enforcement.
- Grant filesystem ACLs only for declared sandbox roots and required tools.
- Reuse existing Job Object process-tree management.
- Add Windows-specific canonicalization tests for case-insensitive paths,
  symlinks, junctions, and drive prefixes.

Success criteria:

- Strict Windows sandbox either enforces filesystem isolation or fails as
  unavailable. It must not silently claim success.
- Basic undeclared read and undeclared write tests pass on Windows.
- Windows gaps are documented in action diagnostics and this plan.

### 34.7 Workers, forkserver, and performance recovery (OPEN)

Sandboxing must not permanently give up Slug's local execution performance.

Deliverables:

- Decide the initial strict behavior for persistent workers:
  - disable workers under strict sandboxing, or
  - spawn per-action workers, or
  - add a worker protocol extension that gives each request a fresh filesystem
    view.
- Reintroduce forkserver-like launch performance only when the helper can still
  apply per-action restrictions safely.
- Cache reusable sandbox input trees where safe.
- Measure:
  - setup wall,
  - action launch latency,
  - peak/average local parallelism,
  - no-op overhead,
  - cleanup cost.

Success criteria:

- The first strict implementation is correct even if workers are disabled.
- A follow-up optimization path is measured before re-enabling workers.
- Sandboxed local execution overhead is explicitly tracked alongside Plan 32.

### 34.8 Tests and rollout (OPEN)

Test categories:

- Unit tests:
  - path normalization,
  - prefix checks,
  - output overlap detection,
  - symlink/junction escape detection,
  - Landlock rule generation,
  - backend availability probing.
- E2E filesystem tests:
  - undeclared source read,
  - undeclared generated input read,
  - undeclared output write,
  - output parent/child overlap,
  - tree artifact inputs and outputs,
  - sandbox debug preservation.
- E2E network tests:
  - default-denied TCP connect,
  - explicitly allowed TCP connect,
  - loopback behavior if supported.
- Parity tests:
  - port relevant Bazel sandbox Java tests and shell integration tests where
    they encode behavior rather than Java-specific internals.
- Performance tests:
  - run Plan 32 local harness with unsandboxed, Landlock, and namespace modes.

Rollout:

1. Land tests behind an explicit `--sandbox` / strategy flag.
2. Make strict Linux sandbox available but not default.
3. Make strict Linux sandbox default for CI once rules_* smoke tests pass.
4. Add Windows strict backend as second tier.
5. Revisit default network policy after repository-rule and test-runner
   exceptions are explicit.

## Open Decisions

- Should Linux default to Landlock fast path with namespace fallback, or require
  explicit selection while the strategy is new?
- What is the minimal system/toolchain read allowlist for C++, Rust, Python, and
  Java toolchains without allowing broad host leakage?
- Should `--sandbox_default_allow_network` follow Bazel's flag name exactly but
  default to deny for ordinary actions, or should Slug keep Bazel's default and
  expose stricter behavior through a separate flag?
- How should persistent workers participate in strict sandboxing without
  cross-action filesystem state leaks?
- Should macOS be added later through `sandbox-exec` profiles, a directory-only
  strategy, or left out until there is a modern supported OS primitive?

## Success Criteria

- Strict sandboxing fails closed on every supported backend.
- Linux catches undeclared source inputs, undeclared generated inputs, and
  undeclared writes.
- Final output path overlaps are rejected before they can race.
- Network denial is tested and observable.
- Windows has either enforcing strict sandbox support or clear unavailable
  diagnostics.
- Sandboxed local execution overhead is measured and tracked against the local
  overhead plan.
- The old `pre_exec` prototype is removed or quarantined as a fallback that is
  not used by strict strategies.
