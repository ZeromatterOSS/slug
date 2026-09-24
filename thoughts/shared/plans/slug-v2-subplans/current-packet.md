# Current Slug V2 Work Packet

Packet: WP-7-142-m7a-repository-path-dirname-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Advance the authentic Rust-toolchain `cquery //:probe --output=label` past
the first post-WP841 stop, `Object of type path has no attribute dirname`
at pinned rules_cc `unix_cc_configure.bzl:500`, then report the first new
stop. Admit only lexical `dirname` on a selected
`repository_ctx.which` executable path or an already observed-present
absolute Host path, returning a path-valued parent (root returns `None`).
Do not claim arbitrary repository paths, linker success, generated
toolchain bytes, root C++ analysis or M7A closure.

The Bazel 9.2 Unix lexical parent spelling, path-valued result, root
`None`, equality of admitted parent paths, and plain valid-Unicode Host
path text are **exact**. The parent
object reuses Slug's Host path identity and its already-known existence;
Rust Host observations, generated-repository display/path tokens and
bounded selected-owner execution remain **Slug-native**. `dirname` on a
missing Host path, Label-source path or generated-repository path, other
new path methods and Windows path behavior are **unsupported/deferred**.
Prior WP838/840 `which` lookup and WP839/841 GCC execute/effect slices
stay exact within their accepted boundaries. Do not widen them to make
this packet pass.

## Source basis and next boundary

Pinned Bazel 9.2
`StarlarkPath.java:127-136` returns a new path from
`Path.getParentDirectory()`; pinned `Path.java:139-151` returns no
parent for root. This is lexical and does not stat, resolve symlinks or
read contents. `StarlarkBaseExternalContext.java:1545-1575` passes an
existing path object through `repository_ctx.path`, and lines
2304-2352 author `which`. Pinned rules_cc 0.2.17
`unix_cc_configure.bzl:493-523` first stringifies
`cc_path.dirname` at line 500 for `-B`. If neither lld nor gold is
found, lines 505-507 compare `ld_path.dirname` to
`cc_path.dirname` and may stringify the former. `cc_path` comes from
`which`; the fallback `ld_path` is a path from a discovered Host
string. No recursive dirname, `get_child`, `exists` query or new file
effect is demanded before the following linker-option execute.

Slug's `RepositoryStarlarkPath` at
`repository_rule_context.rs:513-602` currently exposes only `exists`
for Generated/Host and `get_child` for Generated. Its `Which` value
comes from an observed executable; `repository_ctx.path(which_value)`
returns the same object at lines 912-927. The selected Which resolution
and present Host-path observation already own ancestor state through
DICE (`module_extension_repository_file_effect.rs:1221-1330`).
The current Host text arm at `repository_rule_context.rs:556-557`
uses `NormalizedAbsolutePath::Display`, which writes debug-quoted path
text (`path_observation.rs:221-224`). Correct only that Host text arm to
plain valid-Unicode Unix spelling; do not change the general path
Display, hash or equality implementation.
The first authentic error is an absent attribute, not evidence that a
later linker-option argv or toolchain action already succeeded.

## Owner, policy and memory

Keep the selected repository-rule invocation and existing Which/Host
observations as semantic owners. For a `Which` path proved executable
or a Host path whose observed `exists` is true, compute its normalized
lexical parent and expose it as the existing
`RepositoryStarlarkPath::Host { path: parent, exists: true }`; successful
observation of the child implies parent existence. At root return
`None`. A missing Host child does not imply its parent exists or is
missing: do not manufacture a parent snapshot or return a false
`exists` value. Reject that branch, and reject Label/Generated origins
instead of laundering their template/symlink or generated-path
provenance into a Host object. `dirname` must not turn a derived parent
into a `Which` executable or a Label source.

Reuse current path hash/equality and Allocative shape; correct Host
text only at the existing path-value seam. The new parent is evaluator
scratch only. The observed child path and PATH/raw
environment cells already structurally invalidate a derived parent
across request and DICE replay. No DICE key, lock, filesystem probe,
new retained variant/collection, byte-hash identity, interner,
materializer or Core policy is admitted. Keep the change at the
repository-path attribute seam; the large owner file is a size-review
trigger, not permission to split semantic ownership.

## Proof, validation and scope

Add one independent query row to the existing
`repository-rule-which-normalized-path` fixture. Its rule uses fixed
`PATH=/usr/../bin` and asserts
`str(ctx.path(ctx.which("sh")).dirname) == "/bin"`, that parent equals
`ctx.path("/bin")` as a path value, that direct
`str(ctx.path("/bin")) == "/bin"` has no debug quotes, that
observed-present Host `str(ctx.path("/bin").dirname) == "/"`,
that `str(ctx.path("/")) == "/"`, and that
`ctx.path("/").dirname == None`. The old two query rows and expected
results remain unchanged. This is a small offline discriminator for
the authentic Which→path→dirname chain and parent comparison; it does
not require GCC or a remote service. Regenerate and independently
verify Bazel 9.2 expected results in fresh ignored-RC roots without
reading credentials; replay all three rows with a rebuilt Slug CLI.
Do not add copied upstream assets, arbitrary path operations or a
nondiscriminating assertion.

Focused Loading tests must cover Which and observed-present Host
lexical parent text, path type, equality, root `None`, implied
`exists=true`, and fail-closed missing Host/Label/Generated origins.
Reuse existing selected PATH/raw-observation and Host-path A→B→A DICE
tests unless a demonstrated gap requires a new same-DICE discriminator.
The authentic smoke must clear the old `dirname` error and reach a
source-attributed first new stop; a resource failure or earlier-owner
error is diagnostic only, not acceptance. Record host resource state
and avoid an identical retry.

Resolve the pinned toolchain; compile Loading with JSON `--no-run`,
preflight exact selected tests, run them, then rebuild
`slug_cli_v2` before offline Slug and authentic replay. Serialize
Cargo. Clean stale `slugd` before/after daemon-sensitive checks.
Run format, diff, plan-status, archive and fixture hygiene. Independent
DESIGN review must accept the implied-parent-existence and provenance
boundary before production edits; FINAL review checks the actual
diff/evidence. Update compact Stage 5/9 owner notes only after
acceptance.

Allowlist: this manifest and canonical Current;
`app/slug_loading_v2/src/repository_rule_context.rs` and colocated
tests, with `module_extension_repository_file_effect.rs` only if a
demonstrated selected-DICE gap requires a focused test;
`tests/v2_oracle/fixtures/repository-rule-which-normalized-path/{fixture.toml,
expected/oracle.json,workspace/MODULE.bazel,workspace/repo.bzl}`;
compact Stage 5/9 notes after acceptance. No general path Display or
representation, Bzlmod key, Core linker grammar/process, repository IO,
action, cache transport or other fixture change.

`REPLAN` if Which/present-Host observations do not actually imply
lexical-parent existence and invalidation, if authentic source requires
dirname on a missing/Label/Generated origin before the next stop, if
parent equality requires a new identity domain or retained variant, or
if admission needs a new owner or filesystem observation. A later
unadmitted GCC argv/effect is the next packet's stop, not a reason to
silently widen this one.

## Acceptance receipt

Independent DESIGN review ACCEPT followed correction of the Host text
contract. Pinned nightly toolchain resolved. Loading JSON `--no-run`
passed (10.7 s); exact preflight verified six nonignored new/protected
tests, and all 6/6 passed. `cargo build -p slug_cli_v2` passed after
the Loading production edit (8.31 s). Fresh ignored-RC Bazel 9.2
generation and independent verification passed all three rows
(10.2/4.5 s), with the original two expected command objects unchanged.
The rebuilt Slug CLI passed all three rows. The non-instrumented
authentic cquery cleared the former `path.dirname` error, exited 2 and
stopped at Core's generated GCC prefix guard. A temporary redacted
diagnostic replay identified selected `local_config_cc` execute call
index 14 with seven effects and only 4,145 total file bytes; the last
effect is `builtin_include_directory_paths`, not an over-cap template.
Pinned `lib_cc_configure.bzl:282-310` authors that file at
`unix_cc_configure.bzl:627` before later `BUILD.tpl` substitution
probes. The precise rejected argv was not captured and is an inference
from source order. The diagnostic was removed with no Core diff; the
production CLI was rebuilt and the three-row Slug fixture passed again.
No `slugd` remained. Independent FINAL review ACCEPT found the seventh
effect source-attributed while leaving the rejected argv unproved.
Packet elapsed was roughly 24 minutes from WP841 acceptance through
FINAL review. No compiler-option or toolchain closure is claimed.

## Immediate predecessor

WP841 at `962dbd2ed` accepted the bounded three-template
six-effect prefix, selected pre-render Homebrew-presence gate, fresh
two-row Bazel/Slug evidence and independent FINAL review. Authentic
cquery advanced beyond the former Core prefix rejection to the
`path.dirname` error at `unix_cc_configure.bzl:500`; M7A remains
partial. The complete WP841 contract and receipt remain in that
commit's `current-packet.md`.
