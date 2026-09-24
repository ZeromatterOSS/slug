# Current Slug V2 Work Packet

Packet: WP-7-95-m7a-selected-repository-host-path-exists-r1
Status: FINAL ACCEPT for bounded path/environment behavior; root archive reachability deferred

## Outcome and compatibility

Admit repository_ctx.path(absolute_string).exists for a selected generated
repository rule, using the repository host-input snapshot and DICE-observed
host path state. The immediate production consumer is rules_rust 0.73
rust_host_tools: before any archive call, its auth helper uses HOME to probe
HOME/.netrc. In this environment that path is absent (existence only was
checked; no content or credential was read). This packet owns the path and
environment semantics needed by that helper, but does not claim the production
root has reached its archive effect: fresh CLI smokes stop at independent
generated `slug_crates` Label and rules_shell `which` boundaries first.

Exact Bazel 9.2 behavior is claimed only for normalized absolute paths
whose resolution is missing without a symlink traversal or terminates in an
ordinary file/directory
(including a symlink chain to one): the Boolean exists result matches.
Dangling symlink and cycle/error cases fail closed as unsupported until a
pinned-source regression admits their exact behavior. Tracked invalidation is Slug-native and
stricter than Bazel's un-watched path.exists observation. Relative generated
paths and accepted Label/which path behavior stay unchanged. Reading a
present host file, netrc parsing/auth forwarding, other host path methods,
archive download/extraction and generated module_ctx.path(Label) remain
unsupported/deferred. Never suppress HOME or fabricate an absent netrc.

## Evidence, owner and decision

Pinned Bazel 9.2 StarlarkBaseExternalContext.java:1544-1580 accepts a
string path; StarlarkPath.java:159-170 returns its existence. Vendored
@bazel_tools//tools/build_defs/repo/utils.bzl:491-508 obtains HOME from
ctx.os.environ and probes ctx.path(HOME + "/.netrc").exists before reading
that file. Pinned rules_rust repository_utils.bzl:1005-1042 calls this
helper when auth is empty, before its download_and_extract calls. Its own
MODULE.bazel:55-61 selects host_tools version 1.97.1; root MODULE's
nightly toolchain pins are a different owner. Current
repository_rule_context.rs:462-476,721-732 rejects all absolute strings
as generated-relative paths, so root archive work is not yet reachable.

The selected repository-file-effect observation key remains the semantic
producer. Its owner certificate, full repository-environment snapshot,
a normalized absolute path, a tracked ResolvedPathObservationKey result,
and merged PathObservationEpoch determine this Boolean. The current eager
ctx.os.environ dict records no DICE dependency; rules_rust reads NETRC
membership and HOME through it. Add a workspace-scoped injected
RepositoryEnvironmentSnapshotKey sharing the immutable request Arc. Record
whether invocation accesses ctx.os.environ, then compute and verify this
whole-snapshot key before publishing that result. Structural equality
covers changed values, added/removed names and absent-name lookups; direct
ctx.getenv retains its narrower existing cell owner. Invocation prepared
values are retry scratch, not a second semantic cache. No ambient stat or
global registry may bypass the observed path frontier. Full path identity,
present/missing state and environment projection participate in
invalidation/restoration; observation Need and error remain typed.
Existing which-candidate host-path preparation is a reuse candidate, but
its executable classification is not the exists semantic owner. Do not
hold a lock across DICE compute.

## Scope and stops

Base clean main at 38dec0791. Production allowlist:
app/slug_bzlmod_v2/src/repository_host_input.rs and its lib.rs export;
app/slug_core_v2/src/runtime/repository_host_input.rs plus direct request
injection tests; app/slug_loading_v2/src/repository_rule_context.rs and
app/slug_loading_v2/src/module_extension_repository_file_effect.rs.
Direct colocated tests may change in those files. One focused Stage 1
fixture may be added only if pinned-source regression cannot discriminate
absolute/relative/existence behavior. Scheduling allowlist is this
manifest and the canonical Current paragraph; update Stage 5/9 only for
an accepted ownership decision. No Core transport, archive, MODULE,
lockfile, NativeLink, Bazel vendoring, auth file or BuildBuddy change.

Accept normalized absolute host paths only for this selected path.exists
surface. Preserve repository-relative generated path behavior. Reject
non-Unicode/unnormalized paths and unsupported path methods without
accessing them. Cap distinct prepared host paths per invocation. A present
netrc may return exists=True, but the subsequent read must fail closed
without leaking content or credentials; this packet does not authorize
netrc parsing. If the actual selected request cannot model the host
environment/path source, or the work requires a new secret-bearing input
owner, REPLAN before widening scope.

New memory is a compact path/state entry in invocation scratch, existing
DICE-retained path observations and one Arc clone of the already-captured
environment snapshot in the injected key. That key can retain environment
values longer than the request; redact its Debug and never print values,
including error paths. It releases with normal DICE invalidation/eviction;
invocation scratch releases on success, Need, error or cancellation. No
archive bytes, new interner or cache. Existing Arc/Dupe/Allocative and
SmallMap patterns suffice; no Buck2/V1 donor import or performance claim.
The large selected-effect module stays the existing cohesive DICE owner;
split only if the new path branch would duplicate its observation driver.

## Validation

Independent DESIGN review before Rust edits and FINAL review of actual
diff. Pin the source behavior above in regression tests. Prove absent,
present, file/directory and symlink-to-present; classify dangling symlink
and cycle with a pinned-source regression or explicit unsupported terminal;
reject unnormalized host operands; preserve
generated-relative and Label paths. Prove HOME A/B/A and absent→present→
absent same-DICE replay with distinct observed generations, missing/present
NETRC and HOME snapshot changes, Need propagation, no ambient stat and no
secret bytes/events. Attempt selected host-tools invocation with a fresh CLI
binary and record the first honest stop. Reaching its archive refusal is a
successor integration gate if an earlier independent boundary stops the smoke;
do not claim generated repository completion.

Use pinned Rust toolchain, JSON --no-run exact-selector preflight, serial
Cargo and named direct dependents. Rebuild slug_cli_v2 before smoke; clean
stale slugd before/after. Run cargo fmt --all -- --check, git diff --check,
archive status and plan status. Ordinary Bazel RC discovery may use the
user's BuildBuddy auth; never inspect, print or copy ~/.bazelrc. Do not
inspect any netrc contents. A root smoke that stops elsewhere is evidence
to diagnose, not permission to alter another semantic boundary here.

Validation receipt: pinned JSON `--no-run` and exact-selector preflight passed
for the Bzlmod snapshot key, Core host-input lifecycle, and five new Loading
selectors. Bzlmod 1/1, Core host-input 4/4, Loading selected-owner 59/59 and
direct path 1/1 passed. A fresh `cargo build -p slug_cli_v2` passed. The
production root `slug build //app/slug_cli_v2:slug` stopped at generated
`slug_crates` `module_ctx.path(Label)`; the rules-rust toolchain fixture build
stopped at rules_shell local_config_shell `repository_ctx.which` Unix limits.
Neither invoked the host-tools archive, so root reachability remains deferred.
Independent FINAL review ACCEPTED this narrowed claim after the focused gates,
format, diff, archive and plan checks passed.

## Immediate predecessor

WP794 at 38dec0791 repaired only selected file-effect test input: 19/19
file-effect and 39/39 selected-owner tests pass; production is unchanged.
