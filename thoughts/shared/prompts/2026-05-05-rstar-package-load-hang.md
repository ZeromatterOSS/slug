# Next-session prompt: investigate `crates__rstar-0.12.2-zm//lib/wirebuf` package-load hang

Use this as the starting prompt for the next session. It briefs you cold —
no shared conversation context required.

---

## What you're looking at

Building `zeromatter//sdk:sdk_contents` from
`/var/mnt/dev/zeromatter` (with the local `slug` binary at
`/var/mnt/dev/slug/slug`) hangs in DICE with the user-visible message:

```
Waiting on crates__rstar-0.12.2-zm//lib/wirebuf -- loading package file tree, and 5 other actions
```

The wait persists 30+ seconds, then the daemon connection breaks
(broken pipe on event-log flush). Reproducer:

```bash
cd /var/mnt/dev/zeromatter
/var/mnt/dev/slug/slug shutdown
rm -rf bazel-external
/var/mnt/dev/slug/slug build //sdk:sdk_contents 2>&1 | tee /tmp/sdk_hang.log
```

The hang appears after ~550 crate spokes have lazily materialized as
expected. Build progress is enormous compared to one commit ago — the
hang is a *new* blocker exposed by getting analysis to reach further.

## What's already been done

These four commits on `main` got the build past everything earlier:

- `e7949e6d` — Plan 36 phases 1-2: spoke-repo lazy materialization on
  `mctx.path(Label)` / `mctx.read(Label)` via a new sync→async DICE
  bridge (`slug_bzlmod::materialize_spoke_sync`).
- `01ce01f5` — Plan 24 §5 follow-up + Plan 36 phase 3a:
  - `legacy_exec_cfg` now mirrors `target_cfg` when bound (Bazel-shape
    "exec cfg == target cfg when no exec platforms registered"), only
    falling back to `@local_config_platform//:host` for unbound
    target_cfg. Fixes the case where `--host_platform=...gnu-host`'s
    `@llvm//constraints/libc:gnu.2.28` was being stripped from exec
    cfg, marking every rules_rs crate as
    `@platforms//:incompatible`.
  - Dynamic extension cells now carry `ExtensionRepoCellSetup` →
    `CellInstance` gets `ExternalCellOrigin::ExtensionRepo(setup)`,
    so file-ops accesses route through
    `extension_repo::get_file_ops_delegate`'s lazy DICE materialization
    path. Without this, target analysis hit `read_dir` on
    unmaterialized `bazel-external/...` and aborted.
- `7a7b0ee6` — Plans 24/36 doc updates.

Read these for context (each is self-contained):

- `thoughts/shared/plans/slug-bazel-subplans/36-extension-spoke-lazy-materialization.md`
- `thoughts/shared/plans/slug-bazel-subplans/24-exec-platform-resolution.md` §5 follow-up
- `thoughts/shared/plans/slug-bazel-subplans/13-lazy-toolchain-loading.md` (the parent
  ratchet that exposed both 36 and the rstar hang)

## What's confusing about the message

`crates__rstar-0.12.2-zm` is the canonical cell name for the patched
local rstar (zeromatter's `Cargo.toml` has `[patch.crates-io] rstar =
{ path = "vendor/rstar" }`, version "0.12.2+zm"). The materialized cell
at `bazel-external/rules_rs+crate+crates__rstar-0.12.2-zm/` contains
just `src/`, `Cargo.toml`, `README.md`, `CHANGELOG.md` — symlinks back
to `vendor/rstar/`. **It has no `lib/wirebuf` subdirectory.**

`lib/wirebuf` IS a real zeromatter workspace member at
`/var/mnt/dev/zeromatter/lib/wirebuf/` with its own
`BUILD.bazel`. But that package belongs to zeromatter's root cell, not to
`crates__rstar-0.12.2-zm`.

So the hang message's package label looks like cell-name confusion or
a misattributed wait. Don't take the label at face value.

## Questions to answer (in roughly this order)

1. **Is "loading package file tree" actually the work that's hung, or
   is it just the first of N concurrent DICE waits being printed?**
   Source: `app/slug_event_observer/src/display.rs:332`. The "and 5
   other actions" suggests there are more — find them. Expose all
   pending DICE keys.

2. **What cell-resolver entry actually maps `crates__rstar-0.12.2-zm`,
   and where in the workspace does it land?** Run
   `slug audit cell` (or query the dynamic cell registry from a
   debug build) and confirm the path is
   `bazel-external/rules_rs+crate+crates__rstar-0.12.2-zm` (not
   `vendor/rstar`, not the workspace root). Verify no other cell
   shares the same path.

3. **Is `gather_package_listing` recursing where it shouldn't?** The
   walker (`app/slug_common/src/package_listing/interpreter.rs:360`)
   walks a directory subtree for BUILD-file discovery. If the spoke's
   symlinks resolve back into zeromatter's workspace, the walker could
   re-enter the cell from the source side. Check whether the
   ExtensionRepoFileOpsDelegate's `read_dir`
   (`app/slug_external_cells/src/extension_repo.rs:157`) follows
   symlinks into zeromatter's source — it does, by design (line 198:
   `tokio::fs::metadata` not `symlink_metadata`). If `vendor/rstar/`
   somehow contains a symlink back into `bazel-external/`, the walk
   loops.

4. **Is the wait actually blocked on materialization, or on a DICE
   key cycle?** Plan 36's `materialize_spoke_sync` uses
   `block_in_place + Handle::block_on`. If the future the bridge is
   awaiting itself awaits a key whose computation is waiting on a
   sync code path holding a tokio worker, you can deadlock when the
   runtime is small. Confirm the runtime worker count and watch a
   `tokio-console` trace.

5. **Does the same hang reproduce for a non-patched crate?** Pick
   another spoke that's known to materialize (e.g. `crates__clap-4.5.60`)
   and force a query that loads a non-existent sub-package
   (`slug cquery 'crates__clap-4.5.60//does-not-exist:foo'`). If
   that hangs too, the rstar specifics don't matter and the bug is in
   sub-package loading. If it returns "no such package" cleanly, the
   bug is rstar-specific (most likely a symlink-walk loop or a cell
   path mismatch unique to local-path-override crates).

## Concrete probes you can run

```bash
# 1. Check what `external/` symlinks land where for the rstar cell.
ls -la /var/mnt/dev/zeromatter/external/crates__rstar-0.12.2-zm
ls -la /var/mnt/dev/zeromatter/external/wirebuf 2>/dev/null

# 2. Look for symlinks inside the materialized rstar that point back into
#    zeromatter's source (potential walker loop).
find /var/mnt/dev/zeromatter/bazel-external/rules_rs+crate+crates__rstar-0.12.2-zm -type l -ls

# 3. Reproduce the hang and grab a backtrace of every blocked thread.
#    The daemon log lives under buck-out/v2/log/.
cd /var/mnt/dev/zeromatter
/var/mnt/dev/slug/slug shutdown
rm -rf bazel-external
RUST_LOG=slug_common::package_listing=debug,slug_external_cells=debug \
  /var/mnt/dev/slug/slug build //sdk:sdk_contents 2>&1 | tee /tmp/sdk_hang.log &
HANG_PID=$!
sleep 90  # wait until the "Waiting on" message starts repeating
DAEMON=$(pgrep -f slug-daemon | head -1)
gdb -p "$DAEMON" -batch -ex 'thread apply all backtrace' > /tmp/daemon_bt.log 2>&1
kill $HANG_PID
```

The backtrace is the highest-signal artifact. If multiple threads are
stuck in `gather_package_listing` or `materialize_spoke_sync`, the
loop or deadlock is right there.

## Bazel-side comparison

The user has `bazel` checked out at `/var/mnt/dev/bazel`. ZeroMatter
builds successfully under bazel. Bazel's package walker behavior on
the same cell-shape (local-path-override crate with `+zm` build
metadata in version) is the ground truth. Source pointers:

- `RepositoryFunction.java` and `LocalRepositoryFunction.java` for how
  Bazel resolves and walks local-path-override repos.
- `PackageLookupFunction.java` for the package-existence query that
  corresponds to slug's "loading package file tree."

Don't read all of bazel — only consult these when the slug side gets
stuck and you need to see what the analogue actually does.

## What to deliver

- A new sub-plan in `thoughts/shared/plans/slug-bazel-subplans/`
  if the fix scope is ≥1 phase. Pick the next free number (37 as of
  this writing). If it's a one-commit fix, just update Plan 36's
  Phase 3 follow-ups list.
- Concrete fix in code. Don't paper over the symptom — find the
  loop or deadlock and break it.
- Verify with `cargo test -p slug_bzlmod --lib` (163 tests passed
  baseline) and the smaller smoke (`/var/mnt/dev/slug/examples/multi_package`
  `:gen_version_header`) before declaring victory.
- ZeroMatter `//sdk:sdk_contents` should reach **either** a clean
  build action or a different (downstream) error. If it still hangs,
  revert and re-investigate.

## What NOT to do

- Don't disable Plan 36's lazy materialization or Plan 24 §5's
  legacy_exec_cfg fix. Both are structurally correct and verified
  against simpler workspaces. The rstar hang is downstream of both
  working.
- Don't add a "skip rstar" allowlist. The bug is general; rstar is
  just the first crate to expose it.
- Don't unconditionally revert to the symlink_metadata-only behavior
  in `ExtensionRepoFileOpsDelegate::read_dir` — rules_cc's
  `llvm_configure` relies on directory-symlink following (the comment
  on lines 181-187 explains). If the walker needs containment, scope
  it to the cell's bazel-external dir or use a visited-inode set.

## Where the relevant code lives

- Spoke materialization bridge:
  `app/slug_bzlmod/src/spoke_materialization.rs`
- Spoke registration loop:
  `app/slug_external_cells/src/extension_repo.rs:488-540` (the
  Plan 36 spec hash + Setup wiring is here)
- Cell resolver dynamic-cell synthesis:
  `app/slug_core/src/cells.rs:545-650` (the Plan 36 origin
  attachment is here)
- File-ops delegate:
  `app/slug_external_cells/src/extension_repo.rs:133-280`
- Package walker:
  `app/slug_common/src/package_listing/interpreter.rs`
- legacy_exec_cfg:
  `app/slug_configured/src/execution.rs:259-340`

Good hunting.
