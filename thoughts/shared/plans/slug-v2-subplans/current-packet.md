# Current Slug V2 Work Packet

Packet: WP-7-92-m7a-selected-extension-root-label-watch-r1
Status: accepted; independent design and final review ACCEPT

## Outcome and compatibility

Advance the authentic root `cquery` through rules_rust 0.73.0's selected
`module_ctx.watch(cfg.cargo_lockfile)` at `crate_universe/extensions.bzl:1153`,
where `cfg.cargo_lockfile` is the root Label `//:Cargo.lock`. A watch is a
real tracked file dependency, never a no-op. Report the next honest root
terminal; this packet does not admit path(Label), reading watched bytes,
generator execution, configured-root analysis or M7A closure.

Exact for the admitted root Label regular/missing-file slice: explicit
`watch(Label)` returns None, content change or existence transition
invalidates the selected owner, a warm owner reuses only while its file
observation remains valid, and duplicate watches are idempotent. Bazel
also accepts directories/symlinks and String/path operands, but these
are unsupported/deferred here and must fail closed. Slug-native: the
Rust path-observation frontier and its potentially conservative metadata
invalidation; valid-Unicode physical path requirement; a cap of eight
distinct root Labels per selected invocation. Do not claim exact
Skyframe fingerprint bytes, directory state, or full watch surface.

## Evidence and semantic owner

WP791 at `c52b3b691` passed both selected tag-list additions. Fresh
root `cquery` stops at `extensions.bzl:1153` on missing `watch`;
root `MODULE.bazel` supplies `//:Cargo.lock`, a regular file. The
source next watches `//:Cargo.toml` in `cfg.manifests`, then later
uses `module_ctx.path(Label)` for its lockfiles, but only a new smoke
establishes the next actual terminal.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`:
`StarlarkBaseExternalContext.java:1548-1577,1633-1709,1725-1755`
resolves the Label, rejects extension workdir/outside-workspace watches,
and records `RepoRecordedInput.File`; `RepositoryUtils.java:54-73`
uses package lookup/selected package root; `RepoRecordedInput.java:407-484`
distinguishes file digest, directory and ENOENT; upstream
`starlark_repository_test.sh:2352-2399,2682-2826` covers workdir
refusal and file/missing/directory/symlink changes. The pinned sources
and authentic rules_rust call are existing discrimination evidence;
add a Bazel oracle only for a demonstrated gap.

Slug already has `HostRepositoryLabelPathObservationKey::new_root` for
package lookup, selected package root and policy provenance,
`ResolvedPathObservationKey` for the no-symlink regular/missing kind
gate, and `PathFileDigestObservationKey` for tracked file digest or
missing status. Compose these in order, not a lexical workspace join:
compute a tracked `ResolvedPathObservationKey` on the resolved watched
logical path and reject any entry in `ResolvedPath::symlinks()` before
accepting either Present or Missing from the digest key. Merge the
resolution observation frontier as well as the label-path and digest
frontiers into the selected owner. A missing target is allowed if its
package exists, but absent/deleted package or selected package root
outside the workspace fails closed.
Reject observed directory/nonregular paths or any resolved symlink
before digest observation until their Bazel semantics are modeled.
The selected owner's DICE key must depend on
the observations, and its observed carrier must merge all three frontiers.
No new DICE key, process-global cache, or physical-file side store.

The synchronous Starlark `watch` method validates a selected context
and root `StarlarkLabel`, records at most eight distinct labels in
evaluation-local scratch, and returns None without host I/O. After
evaluation, including when a later Starlark expression fails, drain
the scratch without holding its lock across any `ctx.compute`; resolve
each label and compute file digest observations asynchronously. A
transient observation returns typed path/label Need and discards the
attempt's prints, repository calls and terminal. Only after all
watches complete may the owner merge frontiers and publish events,
calls or a stable error. A selected download Need may take priority;
the subsequent replay must still observe every called watch. Uncalled
prefetched labels must not enter the selected closure. A successfully
used download is marked before any later watch failure. A changed
label/watch set or content invalidates the owner; deselection prunes
its observations through the existing selected-demand closure and
aborted attempts restore the accepted frontier.

## Scope, stops and validation

Base clean `main` at `c52b3b691`. Root owns implementation, serial
Cargo, fresh CLI build/smoke, reviews and commit. Production/test
allowlist: `app/slug_loading_v2/src/module_extension.rs` and directly
colocated tests; scheduling allowlist: this manifest and the canonical
Current paragraph. If the existing typed Need/frontier or selected
closure cannot own the watch without Core/Bzlmod/DICE API changes,
return for design correction before editing them. No generated repo,
MODULE/lockfile, vendor, action/REAPI or remote transport change.
No new retained hot-path representation is planned; the Buck2 utility
skill should not trigger for bounded evaluation-local scratch alone.

Obtain independent design ACCEPT before Rust edits and independent final
review of the actual diff. Focused tests must exercise the real selected
external `.bzl` route with a root Label: cold label-path/digest Need,
zero partial events/calls, replay, warm reuse, same-DICE content A→B→A,
missing→present, duplicate watch, post-watch Starlark error dependency,
and changed watch set. Reuse unchanged Core path-demand, selected-closure,
and abort/deselection tests as compositional evidence, recording that
watch-specific snapshot restoration is not directly end-to-end tested
in this Loading-only packet. Prove refusal of String, scratch path,
external Label, directory/symlink and over-cap
requests before publication. Test direct and ancestor symlinks to both
regular and dangling targets, with no accepted watch state. Use small
deterministic local files or injected path epochs; do not mutate the
root Cargo files. Preflight exact nonignored selectors, run them and
direct dependents, then
rebuild `slug_cli_v2` before fresh authentic root smoke. Clean stale
`slugd` before and after. Run `cargo fmt --all -- --check`,
`git diff --check`, archive status and plan status. Preserve Bazel RC
discovery; never inspect, copy or print `~/.bazelrc` auth contents.

Stop and `REPLAN` if the authentic operand is not a root Label, package
lookup/selected package-root provenance cannot be retained, a real file
observation cannot invalidate warm DICE reuse, or scratch publication
cannot be delayed until all watches finish. Keep path(Label) and
generator execution for a successor packet. Diagnose compiler,
fixture and observation replay failures within this bounded class.

## Immediate predecessor

WP791 at `c52b3b691` independently accepted native immutable tag
views and advanced the real root to the selected watch call.

## Validation receipt

Independent DESIGN ACCEPT after the tracked-resolution/symlink-chain
correction. Loading-only implementation records bounded root Labels in
evaluation-local scratch, then composes observed package/path, no-symlink
resolution and file-digest frontiers before effects. The used-download
key is marked before observing watches, preserving WP790 closure on a
later stable watch error. No Core, Bzlmod or DICE API change.

Exact nonignored preflight verifies 30 Loading selectors and three Core
selectors on the final test executables. Fresh nightly
`cargo test -p slug_loading_v2 --lib --offline --locked` focused
`module_extension::tests::`: 30/30 pass. This includes the real
selected external `.bzl` Label route, cold Need/no partial effects,
warm replay, content A→B→A, missing→present, changed tag/watch set,
post-watch Starlark error, String/scratch/external/cap refusal, direct
and ancestor regular/dangling symlink refusal, directory refusal and
download-then-failing-watch used-key retention. Reused unchanged Core
selectors `runtime::dice::tests::native_demand_session_drives_real_dice_repository_then_path_then_complete`,
`synthetic_compute_cancellation_restores_before_explicit_abort_and_reopens`
and `native_selected_download_need_accept_warm_revoke_and_abort` each
pass 1/1. These are compositional path/selected-snapshot/abort receipts,
not a watch-specific end-to-end deselection claim.

Fresh `cargo build -p slug_cli_v2 --offline --locked -q` passes. Root
`target/debug/slug cquery '//app/slug_cli_v2:slug' --noimplicit_deps
--lockfile_mode=off --output=label` exits 2 after passing both root
Cargo watches and stops at rules_rust 0.73.0
`crate_universe/extensions.bzl:877,1159` on `module_ctx.path(host_tools_repo)`:
the operand is a Label while the admitted `path` slice expects `str`.
No configured-root or M7A closure claim. No `slugd` before/after.
`cargo fmt --all -- --check`, `git diff --check`, archive status and
plan status pass. Independent FINAL ACCEPT; the residual watch-specific
snapshot-restoration coverage gap remains a compositional, not an
end-to-end, claim.
