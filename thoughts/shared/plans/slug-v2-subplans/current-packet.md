# Current Slug V2 Packet

Packet: `WP-4-5-7A-effective-repository-host-input-implementation`

Milestone: M7A category 6 generated-repository prerequisite.

Base: accepted architecture commit `3dbd937a4`, retaining the dirty selected-
context R2 candidate unchanged. Architecture R5 is independently `ACCEPT`.

## Observable result

Every active Slug command carries one immutable effective repository
environment from the CLI through one-shot/daemon request ownership into DICE.
The runtime injects a lower-shared per-name environment cell and Host platform
key, preserves absent/empty and authorization, retains/restores the exact
accepted name frontier, and can progress typed cold-name Needs monotonically.

This packet does not make a repository-rule evaluator request those keys, add a
repository-context capability, generate a repository, realize winsdk, expand a
new registration, change selected configured analysis, or publish an action.

## Authority and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole semantic authority. `CommandEnvironment` and
`Converters.EnvVarsConverter` define default effective-environment and direct
`--repo_env=VALUE` order; `RepoEnvironmentFunction` establishes one dependency
per variable and absent distinct from empty. Pinned integration tests supply
the later invalidation scenarios.

`docs/developers/dice.md`, Buck2-derived
`dice/dice_tests/src/linear_recompute.rs`,
`dice/dice/src/impls/tests/user_data.rs`, and
`dice/dice/src/transaction_update.rs` govern equality cutoff,
per-transaction data, injection and the no-lock-across-compute rule.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is concept/test guidance only: immutable provider-supplied environment/OS data
stays separate from later evaluator capabilities and effects. No Zig code,
layout, scheduler, store or compatibility result is copied.

- **Exact:** on the admitted non-Windows, no-rc Slug command surface, capture
  the full Unicode client environment once; apply direct
  `--repo_env=NAME=VALUE`, `--repo_env=NAME`, and `--repo_env==NAME` operations
  in occurrence order, with original-client inheritance, absent/empty,
  `%bazel_workspace%` expansion, sorted canonical transport and per-name DICE
  equality/invalidation shape.
- **Slug-native:** valid-Unicode Rust inputs; Rust Host OS/architecture value
  spelling; structural DICE identity; internal name frontier,
  `Unauthorized | Observed(Some/None)` lifecycle, typed retry, and existing
  pre-allocation Busy overlap.
- **Unsupported/deferred:** bazelrc discovery/options, strict repository
  environment, legacy action-environment interaction, space-separated
  `--repo_env VALUE`, non-Unicode entries, Windows repository environment/path
  behavior and execution, repository-context/effects, exact Bazel marker bytes,
  and selected-context closure.

BCR Starlark continues to own every rule and control-flow decision including
`cc_internal`; `cc_common` is only a generic Host/provider ABI client. This is
not a `set` or C++ parsing implementation.

## Natural owners and implementation

### Lower shared ABI

Add `app/slug_bzlmod_v2/src/repository_host_input.rs` and doc-hidden exports.
It owns compact immutable:

- `RepositoryEnvironmentEntry { CompactString, Arc<str> }`;
- a sorted unique full `RepositoryEnvironmentSnapshot(Arc<[Entry]>)`;
- `RepositoryEnvironmentCell::Unauthorized |
  Observed(Option<Arc<str>>)`, with absent distinct from empty;
- `RepositoryEnvironmentCellKey { workspace, name }` as an `InjectedKey` with
  complete cell equality;
- sorted unique `RepositoryEnvironmentNameFrontier(Arc<[CompactString]>)`;
- `RepositoryPlatform { os_name, arch }` and its workspace `InjectedKey`; and
- workspace-qualified sorted `NeedRepositoryEnvironmentNames`.

Constructors validate sorted/unique canonical inputs rather than silently
normalizing an untrusted daemon wire. Values use `Arc<str>` so the full snapshot
and injected observed cells share long strings. Names use `CompactString`;
retained collections use immutable Arc slices and `Allocative`/cheap clones.
No interner, whole-map key, mutable cache, global registry or new lock.

Extend the existing `SourcePreparationNeeds` owner with an optional environment
Need, constructors/accessor and union. Union merges/sorts unique names only for
the same workspace and rejects conflicting workspaces. Needs contain no values
and remain transient/non-equal at DICE boundaries.

Core is the sole production injector/lifecycle owner of the key families.
Loading becomes their sole generic consumer only in the next packet. Test code
may inject shared keys directly. Entry/snapshot/cell custom `Debug` must redact
values or be unavailable.

### Command parsing and CLI capture

Add an ordered `RepositoryEnvironmentOverride` category to every build, query,
cquery, aquery, run and test request. Parse only direct
`--repo_env=VALUE` spelling. Split the converter payload at its first `=`:
empty and `=` fail; leading `=` is unset; no `=` is inherit; otherwise set,
including an empty value. Preserve occurrences and their order. Existing direct
strict/action-environment flags remain rejected.

After successful parsing, the CLI captures `std::env::vars_os()` exactly once.
Reject a non-Unicode name/value without reproducing it. Sort the original
snapshot; then apply overrides in order. Inherit always consults the original
snapshot: present overwrites the current result, absent is a no-op and leaves a
prior overlay intact. Set expands every `%bazel_workspace%` occurrence using
the admitted non-Windows absolute workspace string; unset removes. All active
one-shot and daemon lanes use the same helper. The placeholder test command
retains the parsed category but performs no Host capture or execution.

Every Slug-generated CLI/server argv echo redacts a repo-env payload. Parse,
capture, wire and request errors/Debug never contain environment values; use an
entry position or flag name. User-authored Starlark output is outside this
packet and will not be rewritten later.

### Daemon transport

Add a stable primitive repository-environment wire projection to build, query,
cquery, aquery and run requests. The CLI sends the already-normalized complete
snapshot. Server normalization rejects duplicate or out-of-order names before
core invocation and never reads `std::env` as a fallback. Default/legacy test
construction yields an explicit empty snapshot, with no stability shim owed.
Wire/request Debug is value-redacted.

### Runtime injection, lifetime and retry

Extend `ProcessHostOwner` with one bounded repository-platform projection from
its existing process-latched OS/CPU observations, lowercased for the Starlark
field shape. Unknown mappings fail closed; there is no environment read.

`repository_host_input.rs` in core owns only production injection and lifecycle
helpers. Each command request retains its full snapshot. The accepted native-
demand snapshot separately retains its name frontier. A command starts with the
sorted union of its present names and prior accepted frontier. Each attempt:

1. installs the exact snapshot and frontier in `UserComputationData`;
2. injects every frontier cell as `Observed(snapshot Option)` and the platform;
3. runs through the existing request-revision transaction and effect tracker.

Environment progress validates the Need workspace, requires at least one name
outside the in-flight frontier, extends it monotonically, and returns a distinct
progress kind. The retry injects newly known cold absence as `Observed(None)`.
A repeated/equal Need is environment internal non-progress. No repository/path
Need is fabricated.

Acceptance retains the expanded frontier and snapshot. Rejection/cancellation
restores prior snapshot/frontier in both injection and transaction data,
restores prior cells as `Observed(prior Option)`, and changes every rejected-
current-only cell—present or absent—to `Unauthorized`. This state change must
invalidate any dependent completed synthetic value. A physical unauthorized
key may remain until workspace shutdown but yields no semantic value. Passive
transactions install explicit empty/default carriers. Foreign/expired attempt
data is rejected through existing ownership checks.

The current Busy decision remains before request allocation. No mutex is held
across a DICE compute, await, retry, acceptance, restoration or publication.
Snapshots are command-retained then accepted-session-retained; frontiers are
accepted semantic authorization with the same lifetime as injected DICE keys;
per-command mutable union state is request scratch. No eviction is added; all
drop with the workspace runtime.

## Exact allowlist and blobs

Only these existing files may change:

| Area | Paths and current/base blobs |
|---|---|
| shared ABI | `app/slug_bzlmod_v2/src/lib.rs` `bc00bdfddc4587fb3c3e38c646cca0b6d1d460c8`; `app/slug_bzlmod_v2/src/source_preparation.rs` `c3aa654c072bf5698de321cdf1e100e3795f4921` |
| commands | `app/slug_commands_v2/src/{lib.rs,common.rs,build.rs,query.rs,aquery.rs,cquery.rs,run.rs,test.rs}` respectively `18e48b45229aabcfbfd30dedab84a7204728caad`, `0c35a9d5bfc66bdd54e3699c9b8493c0682f1596`, `18d50f636fe1e3798a0a57fb5eb3f85e28119c8c`, `14bfb969bca859067c784ac1747e014a56f6179c`, `2793496e8a56ccf39639dcfab81272404136e3d0`, `daae4d8b214eb386ad15fd6c18dcda46e088b690`, `7c848f746fa379fa8e276565f49a9bb84173f058`, `7603af1a4e858cb25223afa7cc4ee171e2463071`; `app/slug_commands_v2/tests/commands.rs` `d0e6609f5729fe6824f161c2c4f3e1cd9457b77f` |
| CLI | `app/slug_cli_v2/src/commands/{mod.rs,build.rs,query.rs,aquery.rs,cquery.rs,run.rs,test.rs}` respectively `d73d297e5d8c9917fae8dda9bd979119695348ba`, `965dea4b9201ca15e41fd108fd6301f19886d71a`, `31450435ae660dae7ef977358659ddd70adc2a50`, `c11ff97cc525008b95758d2fb15b6a9972ecdfe5`, `167629bec6a41fbfebc40f8508201e09577b3d1b`, `7f6b280e7c302cd4568388ea8121431601392826`, `0da04a49994505fea771ba1fe7e675521b5090cd`; `app/slug_cli_v2/tests/cli.rs` `50998ba0ad57c1a7886eb20b6490b5a0228368f5` |
| daemon | `app/slug_server_v2/src/{server.rs,lib.rs,tests.rs}` respectively `b22cf412449dde3c7cb4e075838e244bd3852cbc`, `c220f8fad487abdd5314e3b566377ad4da698b9b`, `82965bad6922fa76839b58baeab51184fc8e0f02` |
| runtime | `app/slug_core_v2/src/runtime/{process_host.rs,dice.rs,mod.rs}` respectively `666fd708e84f21b47ab9ef402d187f349d3c0cdf`, retained live blob `85562562f9f2d9e03f0d866f9194ba70fc05b7ec`, `bdf4af4a8ae422fc315bb0a0dd9a6077727b3016` |

New production files are exactly:

- `app/slug_bzlmod_v2/src/repository_host_input.rs`;
- `app/slug_cli_v2/src/commands/repository_environment.rs`; and
- `app/slug_core_v2/src/runtime/repository_host_input.rs`.

The only new separate proof file is
`app/slug_core_v2/src/runtime/tests/repository_host_input_tests.rs`, included
under the private `dice.rs` test module. No Cargo manifest, lockfile, loading,
analysis, selected-context, fixture, asset or REAPI file may change.

Maximum additions: 1,750 production Rust, 1,900 proof Rust and 3,650 aggregate
Rust lines; deletions do not create budget. `dice.rs` exceeds 12,000 lines but
receives only thin bundle/frontier/progress/user-data/injection/test-include
calls. New lifecycle logic belongs in its cohesive runtime module. The
17,000-line `source_preparation.rs` receives only the typed Needs field,
constructor/accessor/union and colocated carrier tests. No other owner may be
added. Preserve the exact dirty base and stage/commit only this packet's delta.

## Discriminating proof and validation

Proof must cover:

- set/inherit/unset occurrence order, original-present and original-absent
  inherit, empty set, malformed empty/`=`, first-`=` splitting and workspace
  expansion;
- sorted snapshot/wire normalization, duplicate/out-of-order rejection,
  absent/empty, Unicode rejection, and all active one-shot/daemon paths;
- a sentinel secret absent from every system-generated Debug/error/argv output;
- cell/platform key equality, `Unauthorized` versus both observed states, Arc
  value sharing and retained-size accounting;
- Need sort/dedup/union, wrong-workspace conflict, progress workspace rejection,
  cold absent injection and repeated-Need non-progress;
- cold/warm/A-B-A per-name equality, unrelated-name non-invalidation and
  deletion injection;
- accepted-frontier warm reuse; rejected and cancelled extra-`Some` and extra-
  `None` rollback to unauthorized; invalidation of a completed synthetic
  dependent; prior snapshot/frontier transaction-data recovery;
- foreign/expired attempt data and pre-allocation Busy overlap; and
- server process environment cannot substitute for the client snapshot.

Use pure supplied environment iterators for unit tests and spawned-process
environment isolation where end-to-end capture is required; do not mutate the
test process environment concurrently.

Run Cargo serially: focused new ABI/Need/command/CLI/server/runtime tests, then
full `slug_bzlmod_v2`, `slug_commands_v2`, `slug_cli_v2`, `slug_server_v2` and
`slug_core_v2`, followed by one direct full `slug_loading_v2` compile/test
dependent. Rebuild `slug_cli_v2` before `SLUG_V2_BIN` one-shot/daemon smokes;
clean stale `slugd` before and after. Run `cargo fmt --all`, `git diff --check`,
blob/scope/cap/retained-diff audit and `scripts/v2_archive_status.sh`. The three
known archive thoughts-path complaints remain baseline only.

## Stops and successor

`REPLAN` for an ambient read outside CLI capture; daemon fallback; raw value in
a Slug-generated diagnostic; whole-map/frontier DICE dependency; key ownership
outside lower Bzlmod; production injection outside core; loading/evaluator
change; cold name without typed Need; Need without new-name progress;
unauthorized semantic value; rejected-only observed authorization after
restoration; lock across compute/await; foreign workspace acceptance; Windows/
rc behavior; new Cargo dependency; change outside allowlist/caps; or inability
to isolate the packet from the retained selected-context candidate.

After terminal acceptance, activate only
`WP-4-5-7A-canonical-repository-rule-host-capability-implementation` under the
architecture commit's frozen loading allowlist and 650/900/1,550 caps. The
proof-only registration packet remains later.
