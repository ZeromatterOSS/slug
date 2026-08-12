# Current Slug V2 Packet

Packet: `WP-7-m6-filewrite-reapi-action-handoff-implementation`
Milestone: M6 implementation
Owner: `slug-v2-subplans/07-reapi-native-execution.md`
Result: make the accepted configured FileWrite semantic view the sole
FileWrite-to-REAPI handoff and prove its exact CAS/protobuf identity.

## Scope

Implement only the reviewed 2026-08-11 FileWrite Action IR-to-REAPI design.
Keep `ConfiguredNodeResult.actions: Arc<[ActionSpec]>` as the sole DICE-owned
action declaration. Add one request-local `FileWriteReapiPlan` in
`slug_reapi_v2` that is constructed from `ResolvedFileWriteSemanticView`,
the request's remote-default property map, and no other semantic source.

The plan must own the inline content blob at
`__slug_filewrite__/content`, canonical Merkle Directory blobs, a fixed
positional `sh`/`cp`/`chmod` Command, selected/default platform properties,
and encoded Action identity. `Action.timeout` is absent.
`--remote_timeout` remains transport/RPC policy and must not change Command or
Action bytes or digests.

Migrate CLI and daemon FileWrite builds to
`resolved_file_write_semantic_views_in_closure()`. Derive output-root placement
from each resolved action owner. Report platform properties from the actual
plan/result, not by echoing request defaults. After migration, the raw
`execute_action(&ActionSpec)` path must reject FileWrite; it may remain only
for the already-admitted non-FileWrite RunShell regression. A closure mixing
the bounded FileWrite path with other action kinds fails closed in this packet.

## Compatibility boundary

- **Exact:** Rust valid-Unicode FileWrite content bytes including embedded NUL;
  declared one-file output and executable bit on the admitted POSIX worker;
  canonical REAPI Directory, Command, and Action protobuf bytes and SHA-256
  digests for Slug's actual graph; selected-platform versus all-or-nothing
  remote-default property choice; absent Action timeout; and A/B/A restoration.
- **Slug-native:** reserved inline-input namespace, fixed POSIX worker recipe,
  Slug action/configuration display bytes, output-root placement, traversal
  order, diagnostics, and evidence formatting.
- **Unsupported/deferred:** exact Bazel configuration/output/ActionKey bytes,
  non-Unicode/Java string edges, action-semantic timeout, WriteJson/Run/Spawn
  migration, mixed FileWrite/non-FileWrite closures, paramfiles, tree outputs,
  ordinary source/generated input trees, ordinary zero-toolchain FileWrite
  owners, non-POSIX workers, and broader backend/cache/materializer work.

Missing or ambiguous platform closure, malformed topology, an unmodeled
FileWrite field, a first output segment equal to `__slug_filewrite__`, or a
raw FileWrite executor call fails closed.

## Evidence

Add direct Rust regressions for:

- quote, newline, and embedded-NUL content surviving only in the inline blob;
- executable and non-executable fixed recipes;
- reserved namespace and malformed shape rejection;
- canonical Directory, Command, and Action fields plus SHA-256 digests;
- nonempty selected-platform properties winning as a whole over conflicting
  defaults, and an empty selected property map admitting the complete defaults;
- content/output/executable/platform A/B/A identity restoration; and
- distinct `--remote_timeout` values leaving encoded Action bytes and digest
  unchanged.

Add one focused five-file-or-smaller FileWrite REAPI fixture with a real
selected execution platform. It must run A/B/A content edits through one-shot
and stable-PID daemon builds against the retained NativeLink harness, compare
exact output manifests/content with Bazel 9.2, prove selected platform
properties in evidence, and retain `direct_local_actions = 0`. Preserve the
existing simple FileWrite and RunShell REAPI regressions; converting the simple
payload fixture to a local selected-platform fixture is allowed if required by
the fail-closed platform boundary.

No new public CLI/daemon/protocol wire is allowed. Reuse the accepted Bazel 9.2
FileWrite/aquery evidence and official REAPI protobuf serialization contract;
do not add redundant oracle rows.

## Allowlist and caps

Edit only:

- `Cargo.lock` if the existing workspace dependency edge requires refresh;
- `app/slug_reapi_v2/Cargo.toml` and
  `app/slug_reapi_v2/src/{command,input_tree,executor,evidence,lib}.rs`;
- `app/slug_cli_v2/src/commands/build.rs` and its existing focused tests;
- `app/slug_server_v2/src/reapi.rs` and its existing focused tests;
- `tests/v2_oracle/fixtures/simple-rule-action/**` only if converting the
  retained FileWrite regression to the selected-platform boundary;
- one new `tests/v2_oracle/fixtures/filewrite-reapi-handoff/**` fixture with at
  most five files; and
- Stage 7, this manifest, and the canonical V2 plan for acceptance bookkeeping.

Do not edit retained analysis/DICE representations, Stage 9, generated REAPI
protocol, unrelated fixtures, or workspace-wide dependency declarations. Cap
formatted Rust growth at 460 production plus 300 test lines, fixture growth at
220 lines, documentation growth after this manifest at 90 lines, and total new
files at five. One new dependency edge from `slug_reapi_v2` to
`slug_core_v2` is allowed; no new external crate.

## Validation and review

Run formatter/checks and focused tests for `slug_reapi_v2`, `slug_cli_v2`,
`slug_server_v2`, and `slug_core_v2`. Rebuild `slug_cli_v2` before Slug
oracle runs. Clean stale `slugd` before and after daemon/REAPI validation.
Replay the focused fixture with pinned Bazel 9.2 and Slug/NativeLink, then run
the retained simple FileWrite, RunShell, platform-property, and daemon
regressions. Run the affected full command/server/core suites serially and
classify any unrelated baseline failures precisely. Check allowlist/caps,
credentials, archive boundary, `git diff --check`, and a clean post-test daemon
state.

Require one independent Sol implementation review because this packet changes
action identity, executor ownership, a crate dependency edge, and closes the
bounded M6 milestone. The reviewer must verify the accepted design literally,
serialized-protobuf digest construction, raw FileWrite rejection, exact
all-or-nothing properties, transport-timeout invariance, no new retained/DICE
state, no public-wire change, and truthful compatibility claims. One bounded
correction is allowed; a second material correction is `REPLAN`.

At `ACCEPT`, mark only bounded M6 FileWrite handoff accepted and schedule the
next canonical M7 design packet. At `REPLAN`, record the missing prerequisite
and schedule only its design packet.
