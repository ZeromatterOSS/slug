# Current Slug V2 Packet

Packet: `WP-4-5-7A-effective-repository-host-input-architecture-r5`

Milestone: M7A category 6 generated-repository prerequisite.

Base: declaration-metadata commit `10e6f1a8b`, retaining the dirty selected-
context R2 candidate unchanged. The metadata implementation is terminally
`ACCEPT`; this packet changes documentation only.

## Observable result

Freeze one command/session repository Host-input model and the exact successor
packet sequence. The design must give generic BCR `repository_rule`
implementations Bazel-compatible OS/environment observations without ambient
daemon reads, whole-map DICE dependencies, evaluator-lifetime semantic state,
or a ruleset-specific `local_config_winsdk` shortcut.

No Rust, Bazel asset, fixture, generated repository, registration result,
selected context, configured analysis, action, REAPI value, or daemon protocol
changes in this packet.

R1 review is `REPLAN`: the first draft did not retain observed Host identity in
effect equality, let earlier successor tests invalidate later frozen blobs,
overclassified Rust OS/architecture spelling as exact, and described an
impossible fail-closed policy for bazelrc inputs Slug does not discover. R2
retains only the corrected documentation: complete observed-input equality,
dedicated nonoverlapping successor proof files, exact current blobs, a
Slug-native Host-value classification, and an explicitly no-rc exact command
surface. No R1 Rust exists.

R2 review is also `REPLAN`: merge-walking only present/prior names cannot
compute a cold dynamically requested absent name because an `InjectedKey` has
no default compute path. R3 adds one compact accepted/restored injected-name
frontier plus a typed environment `Need` on the existing source-preparation
retry carrier. Unknown declared/dynamic names stage no result, extend the
attempt frontier with an explicitly injected snapshot value (including
`None`), and rerun before dependency validation or publication. No R1/R2 Rust
exists.

R3 review is `REPLAN`: rolling back only the transaction frontier cannot
invalidate a completed effect cached against a rejected attempt's extra
`None`/`Some` cell, because transaction data is not a DICE dependency. R4 makes
authorization part of each per-name injected value itself:
`Unauthorized | Observed(Option<Arc<str>>)`. Restoration changes rejected-only
cells to `Unauthorized`, invalidating any cached effect without a whole-frontier
dependency. No R1/R2/R3 Rust exists.

R4 review is `REPLAN`: loading must compute the per-name key but cannot depend
on a key type owned by `slug_core_v2`, which already depends on loading. R5
makes the key/value one doc-hidden shared ABI in lower-level
`slug_bzlmod_v2`; core remains the sole production injector/lifecycle owner and
loading is the sole generic semantic consumer. It also splits proof lawfully:
packet 1 drives private core restoration, while packet 2 directly injects the
shared key in test code to prove real-effect cache invalidation. No
R1/R2/R3/R4 Rust exists.

## Authority and learned facts

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole semantic authority.

- `CommandEnvironment#getEffectiveRepositoryEnvironment` starts from the full
  client environment by default. Direct `--repo_env` occurrences then apply in
  order: `NAME=VALUE` sets, `NAME` inherits from the original client
  environment when present, and `=NAME` unsets. The Bazel workspace token is
  expanded in set values. Bazel 9 defaults ignore `--action_env` here and do
  not enable strict repository environment.
- `RepoEnvironmentFunction` deliberately owns one Skyframe key per variable.
  `getEnvironmentView` sorts declared names and preserves absent separately
  from an empty value; using the whole environment as one key would invalidate
  every repository on every unrelated change.
- `RepositoryFetchFunction` requests every `repository_rule(environ=...)` name
  before evaluating the implementation. `repository_ctx.getenv` adds a dynamic
  recorded input. `repository_ctx.os.environ` exposes the full effective map
  but map access itself does not add dependencies.
- `StarlarkOS` exposes lowercased Java OS name and architecture. `DigestWriter`
  includes OS/architecture plus the sorted declared environment view in the
  repository marker input, independently of whether the implementation reads
  those values.
- `external_integration_test.sh` proves declared changes and dynamic `getenv`
  changes rerun a repository, unrelated changes do not, `--repo_env` wins, and
  stable build/query/build commands do not rerun without a relevant change.
- Slug already has a service-owned `ProcessHostOwner`, a serialized native-
  demand command lease, immutable accepted/restored request bundles, a
  per-attempt `UserComputationData` boundary, and one injection path. It also
  has canonical external `.bzl` loading and a staged repository file-effect
  leaf. Those owners are extended; none is duplicated.

`docs/developers/dice.md` supplies the local ownership/deadlock contract.
Buck2-derived DICE `dice/dice_tests/src/linear_recompute.rs`,
`dice/dice/src/impls/tests/user_data.rs`, and
`dice/dice/src/transaction_update.rs` are the implementation evidence for
equality cutoff, per-transaction data and injected `changed_to` updates. The
successor tests adapt those owners rather than assuming transaction data itself
records a dependency.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is **concept/test guidance only**. Its useful separation is provider-supplied
OS values, a full environment view, dynamic `getenv` observation, and effect
callbacks beneath an outer owner that stages and discards failed invocations.
Slug does not copy Zig code/layout, its scheduler, stores, or compatibility
claims. Bazel evidence remains required for every exact behavior.

The Buck2-derived utility review selects `CompactString` for names,
`Arc<str>` for retained values, an immutable sorted `Arc` slice for each
snapshot, `Allocative`, and cheap `Dupe`/Arc clones. A global interner, mutable
environment cache, whole-map DICE key, or new lock is rejected. R4 additionally
retains one sorted immutable Arc slice of names whose per-name cells have been
explicitly injected; values never move into that frontier.

No new oracle fixture is needed: the accepted pinned Bazel integration tests
and authenticated BCR winsdk source are discriminating evidence. Strict/action
environment upstream variants are skipped only because those flag modes are
explicitly unsupported in this slice. There is no temporary fallback or
deletion ledger.

## Frozen ownership and behavior

### Immutable request projection

`slug_bzlmod_v2`, already below both core and loading, owns the doc-hidden
shared semantic ABI:

- `RepositoryEnvironmentEntry { name: CompactString, value: Arc<str> }`;
- sorted, unique `RepositoryEnvironmentSnapshot(Arc<[... ]>)` containing the
  complete effective client environment;
- `RepositoryEnvironmentCell::Unauthorized |
  Observed(Option<Arc<str>>)` as the value of one injected variable key;
- `RepositoryEnvironmentCellKey { workspace, name }` with `InjectedKey`
  equality on that complete cell value;
- sorted, unique `RepositoryEnvironmentNameFrontier(Arc<[CompactString]>)`
  authenticating the per-name cells explicitly injected for an accepted or
  in-flight command; and
- `RepositoryPlatform { os_name, arch }`, both compact lowercased strings, plus
  `RepositoryPlatformKey { workspace }` with complete injected equality.

Core is the sole production constructor/injector and lifetime owner of both key
families. Loading is the sole generic semantic consumer and may only construct
keys and compute their values. No upper crate is re-exported downward, and no
provider trait/dynamic side channel is introduced. Test code may inject these
public/doc-hidden keys directly to isolate equality and invalidation behavior.

The same module owns
`NeedRepositoryEnvironmentNames { workspace: NormalizedAbsolutePath, names }`
for the existing `SourcePreparationNeeds` retry carrier. `names` is a sorted
unique immutable Arc slice; the value never contains environment values or a
whole snapshot. Need union rejects different workspaces, and core progress
rejects a workspace other than its runtime owner.

The command parser owns an ordered list of direct `--repo_env=VALUE`
operations.
The CLI captures `std::env::vars_os()` exactly once after successful parsing,
rejects a non-Unicode name or value, and applies those operations against the
unchanged original client snapshot. Set/unset order is observable; inherit
always consults the original snapshot, not an earlier overlay result; when the
name was absent originally, inherit is a no-op and leaves any prior overlay in
place. The daemon wire carries the already-normalized sorted snapshot and rejects
duplicates or out-of-order entries. The daemon never substitutes its own
environment. Active build, query, cquery, aquery and run paths share this
projection; the placeholder test parser retains the same option category for
its future execution path.

Only the Bazel 9 default policy and direct `--repo_env=VALUE` argv occurrences
are admitted. Direct strict-repository-environment or legacy action-environment
interaction flags are rejected. Slug's admitted command surface does not read
or discover bazelrc files, so rc-supplied repository options are explicitly
outside this exact slice rather than silently treated as inputs. Space-separated
`--repo_env VALUE` spelling is likewise unsupported/deferred in this packet.

Environment values are transport/semantic inputs, never content for Slug-
generated parser/capture/wire/request/argv diagnostics. `RepositoryEnvironmentEntry`
and snapshot/cell/wire/request `Debug` output must use custom value-redacted
implementations (or expose no `Debug`); parsing/capture errors name the flag or
entry position without reproducing its value; and every CLI/server argv echo
replaces a `--repo_env=...` payload with a fixed redaction before formatting.
The local daemon socket still carries actual normalized values, but neither
endpoint logs them through a Slug-generated path. User-authored Starlark
`print()`/`fail()` output is intentionally excluded: as in Bazel, a repository
implementation may deliberately reveal a value it reads, and Slug must not
rewrite that user output.

### DICE and request lifetime

The runtime derives `RepositoryPlatform` from its existing process-latched
`ProcessHostOwner` once per workspace runtime and fails closed for an unknown
mapping. It does not reread process environment.

One injected key `(workspace, variable name)` owns
`RepositoryEnvironmentCell::Unauthorized | Observed(Option<Arc<str>>)`. In the
observed state, absent `None` is distinct from `Some("")`; `Unauthorized` is an
internal lifecycle state and is never a Starlark value. One injected
`(workspace)` platform key owns the OS pair. The command-retained full snapshot
participates in the existing
native-demand request input bundle and accepted/restored snapshot, but is not
a DICE dependency as a whole. The accepted native-demand snapshot separately
retains the injected-name frontier. A new command begins with the union of its
present names and the prior accepted frontier. Every attempt injects that
frontier from the desired full snapshot, using `Observed(None)` for absent
names and `Observed(Some(value))` for present names, then installs both exact
values in `UserComputationData`.

An environment-name `Need` must add at least one previously unknown name to the
in-flight frontier or fail with environment non-progress. The next attempt
injects each new cell from the unchanged command snapshot, including cold
`Observed(None)`, before computation. Successful acceptance retains the
expanded frontier. Rejection/cancellation restores every prior-frontier cell as
`Observed(prior value)` and changes every rejected-current-only cell to
`Unauthorized`; it also restores the prior frontier and full snapshot in
transaction data. Because completed effects depend on each observed cell,
that authorization-state change invalidates any result computed by the
rejected attempt, for both extra present and absent names. A physical
`Unauthorized` cell may remain in DICE until workspace shutdown, but cannot
yield an environment value or authorize publication; a later command must
demand and change it back to `Observed` first. Every transaction-data
construction site receives its desired snapshot and frontier explicitly.
Passive computations get explicit empty/default carriers and cannot consult
ambient state.

Slug's existing Busy result before allocation remains the explicit overlapping
command behavior. No mutex is held across a DICE compute, evaluator call,
effect, await, acceptance, restoration or publication. DICE equality cutoff
applies per variable and platform. Snapshot Arcs are command-retained and then
accepted-session-retained; injected values are DICE-retained semantic memory;
the accepted frontier is session-retained semantic authorization; and the
invocation's dynamic-name recorder and evaluator values are phase scratch. The
frontier grows only when an evaluated repository rule names a variable and has
the same lifetime as its per-name DICE keys; there is no separate eviction
policy. Shutdown drops the workspace runtime, frontier and injected values.

### Generic repository evaluation

Before invocation, the repository-effect key compares every name in the
authenticated definition projection's `environ` set with the transaction's
injected frontier. If any declared name is unknown—including a cold absent
name—it returns one typed environment `Need` containing all such names, without
constructing a context or invoking Starlark. On the successful retry it first
computes the platform key and every declared per-name key in sorted order; only
an `Observed(value)` matching the transaction snapshot is admissible.
`Unauthorized` or frontier/cell/snapshot disagreement fails closed; only then
may it invoke the implementation. The context receives:

- `repository_ctx.os.name` and `.arch` from the platform observation;
- `repository_ctx.os.environ` from the immutable full request snapshot;
- `repository_ctx.getenv(name, default=None)` from that same snapshot while
  recording the dynamic name; and
- the existing staged `repository_ctx.file` capability.

After synchronous Starlark evaluation, the outer async DICE owner compares all
recorded dynamic names with the same frontier. If any is unknown, it discards
the staged plan and returns one typed environment `Need` containing every new
name. The command owner extends the frontier, injects snapshot `Some`/`None`,
and retries the complete authenticated effect. On the successful retry, the
outer owner computes every recorded dynamic per-name key and verifies its value
is `Observed` and matches the same request snapshot before publication. Any
`Unauthorized` state, definition,
projection, Host-input, dependency, invocation or effect error discards the
staged plan. There is no shared evaluator scratch or side table.

Discovery is bounded by monotone progress: a Host retry is lawful only when it
adds a new unique name, and all unknown names from one invocation are batched.
Injected-cell availability is not visible to Starlark, so a deterministic
replay over the unchanged snapshot records the same dynamic names. Declared-
absent discovery requires at most one pre-invocation retry per newly reached
definition; dynamic discovery requires at most one post-invocation retry per
newly reached invocation. Repeated equal Needs fail as internal non-progress.

The completed effect value retains a compact sorted Host-observation projection
containing platform plus every declared and dynamically recorded name/value,
with absence explicit. That projection participates in complete DICE equality
alongside the plan, so an admitted Host input never disappears behind equality
cutoff merely because it generated identical file bytes. The unobserved
remainder of the full environment is deliberately not retained in effect
identity.

An unrelated environment change updates the command snapshot and injected key
but does not invalidate an effect with no dependency on that name. If another
owned input later recomputes the effect, it sees the new full `os.environ`, as
Bazel does. Declared names remain dependencies even when an early Starlark
branch never reads them.

The defining label routes through the existing Root/Canonical external `.bzl`
source owner. Canonical definitions use the existing canonical load-route and
module-evaluation keys, then reauthenticate the exact exported callable and
projection already retained by instantiation. No root-path fallback scan,
ruleset name check, or second loader is allowed.

The real pinned BCR `winsdk_configure.bzl` therefore follows its ordinary
non-Windows branch: observe OS, stage empty `BUILD`, and stage exact
`toolchains.bzl` with `register_local_rc_exe_toolchains(): pass`. An actual
Windows host fails closed before publication because path/SDK discovery is not
admitted. No `local_config_winsdk` special case is permitted.

## Compatibility classification

- **Exact:** on the admitted non-Windows Host, Bazel 9 default full-client base
  on Slug's no-rc command surface;
  direct `--repo_env=VALUE`
  set/inherit/unset, occurrence order and workspace-token expansion; sorted
  declared views; absent versus empty; declared and dynamic per-name
  invalidation; unrelated-name non-invalidation; the Starlark access shape and
  environment/`getenv` values/default behavior; canonical definition
  authentication; non-Windows winsdk bytes; and the four eventual BCR
  registration rows and order.
- **Slug-native:** valid-Unicode Rust environment strings; existing Rust Host
  OS/architecture observation and value spelling exposed through the exact
  Starlark field shape; internal per-name authorization/frontier/retry state;
  structural DICE identity/equality; command Busy overlap; and Slug's staged
  file-effect identity.
- **Unsupported/deferred:** strict repository environment; action-environment
  interaction; bazelrc discovery and rc-supplied repository options until that
  command source is owned; space-separated `--repo_env VALUE` spelling;
  non-Unicode environment entries; Windows SDK/path/executable discovery;
  Windows repository environment name/path edge behavior and repository
  execution generally; other repository-context capabilities; `local`/
  `configure` scheduling policy; exact Bazel marker-fingerprint bytes; and
  selected-context closure until the proof packet passes.

BCR Starlark continues to own all rules and control flow, including
`cc_internal`. `cc_common` is a generic Host/provider ABI client. This is not a
`set` project or a C++ parser: shared builtin categories remain on the adopted
Rust/Buck2 Starlark substrate.

## Frozen successor packets

### 1. Effective Host-input implementation

Activate
`WP-4-5-7A-effective-repository-host-input-implementation` first. It adds the
shared immutable types/parser/capture/wire/runtime injection plus the typed
environment-Need/frontier progress substrate; repository evaluation emits no
such Need yet and generated output does not change.

Existing-file allowlist and current blobs:

| Area | Paths and blobs |
|---|---|
| shared ABI | `app/slug_bzlmod_v2/src/lib.rs` `bc00bdfddc4587fb3c3e38c646cca0b6d1d460c8`; `app/slug_bzlmod_v2/src/source_preparation.rs` `c3aa654c072bf5698de321cdf1e100e3795f4921` |
| commands | `app/slug_commands_v2/src/{lib.rs,common.rs,build.rs,query.rs,aquery.rs,cquery.rs,run.rs,test.rs}` respectively `18e48b45229aabcfbfd30dedab84a7204728caad`, `0c35a9d5bfc66bdd54e3699c9b8493c0682f1596`, `18d50f636fe1e3798a0a57fb5eb3f85e28119c8c`, `14bfb969bca859067c784ac1747e014a56f6179c`, `2793496e8a56ccf39639dcfab81272404136e3d0`, `daae4d8b214eb386ad15fd6c18dcda46e088b690`, `7c848f746fa379fa8e276565f49a9bb84173f058`, `7603af1a4e858cb25223afa7cc4ee171e2463071`; `app/slug_commands_v2/tests/commands.rs` `d0e6609f5729fe6824f161c2c4f3e1cd9457b77f` |
| CLI | `app/slug_cli_v2/src/commands/{mod.rs,build.rs,query.rs,aquery.rs,cquery.rs,run.rs,test.rs}` respectively `d73d297e5d8c9917fae8dda9bd979119695348ba`, `965dea4b9201ca15e41fd108fd6301f19886d71a`, `31450435ae660dae7ef977358659ddd70adc2a50`, `c11ff97cc525008b95758d2fb15b6a9972ecdfe5`, `167629bec6a41fbfebc40f8508201e09577b3d1b`, `7f6b280e7c302cd4568388ea8121431601392826`, `0da04a49994505fea771ba1fe7e675521b5090cd`; `app/slug_cli_v2/tests/cli.rs` `50998ba0ad57c1a7886eb20b6490b5a0228368f5` |
| daemon | `app/slug_server_v2/src/{server.rs,lib.rs,tests.rs}` respectively `b22cf412449dde3c7cb4e075838e244bd3852cbc`, `c220f8fad487abdd5314e3b566377ad4da698b9b`, `82965bad6922fa76839b58baeab51184fc8e0f02` |
| runtime | `app/slug_core_v2/src/runtime/{process_host.rs,dice.rs,mod.rs}` respectively `666fd708e84f21b47ab9ef402d187f349d3c0cdf`, live retained-candidate blob `85562562f9f2d9e03f0d866f9194ba70fc05b7ec`, `bdf4af4a8ae422fc315bb0a0dd9a6077727b3016` |

New files are exactly
`app/slug_bzlmod_v2/src/repository_host_input.rs`,
`app/slug_cli_v2/src/commands/repository_environment.rs`, and
`app/slug_core_v2/src/runtime/repository_host_input.rs`; the only new separate
proof file is
`app/slug_core_v2/src/runtime/tests/repository_host_input_tests.rs`, included
under the existing `dice.rs` test module so it can exercise private session
restoration without widening production visibility. No Cargo manifest or
lockfile change is authorized. Maximum additions are 1,750 production Rust,
1,900 proof Rust and 3,650 aggregate Rust lines. `dice.rs` exceeds 2,000 lines
but receives only thin request-bundle, frontier/progress, user-data, injection
and test-include calls; shared key/value/equality logic belongs in the new
bzlmod module and injection/lifecycle logic in the new runtime module. The
17,000-line `source_preparation.rs` receives only the new typed
Needs field/constructor/accessor/union and colocated carrier tests; it gains no
Host value, evaluator or command owner. The exact dirty base must be preserved
and only packet deltas may be staged/committed.

Proof must discriminate set/inherit/unset order, original-client inheritance,
workspace expansion, malformed values, absent/empty, Unicode rejection,
canonical sorted wire rejection, daemon-environment non-use, all active
command paths, sentinel-value absence from system-generated Debug/errors/argv
terminal output while deliberate Starlark print/fail remains unchanged,
per-name equality cutoff, unrelated-name non-invalidation,
deletion injection, cold/warm/A-B-A, rejected-attempt restoration, cancellation
restoration including frontier rollback and prior transaction-data recovery,
typed Need union/order, at-least-one-name progress, repeated-Need non-progress,
cold absent injection, `Unauthorized`/observed equality transitions, wrong-
workspace Need/union rejection, accepted-frontier warm reuse, rejected/
cancelled extra-`None` and extra-`Some` authorization rollback, foreign/expired
user data, platform-key equality, the lower-crate ABI with core-only production
injection, Busy overlap, retained-size accounting and Arc value sharing without
duplicated long strings. No hot-path benchmark is required absent evidence that
bounded once-per-command environment normalization is a demonstrated hot path.
Run serial focused tests, then
commands, CLI, server, core and loading dependents, rebuild
`slug_cli_v2` before binary smokes, format, diff/scope/cap/archive gates.

### 2. Canonical repository Host capabilities

After packet 1 is accepted, activate
`WP-4-5-7A-canonical-repository-rule-host-capability-implementation`. Its
existing-file allowlist is:

- `app/slug_loading_v2/src/module_extension_repository_file_effect.rs`, blob
  `7396e2a80e2079be695f860af8b077d415bd7c3c`;
- `app/slug_loading_v2/src/bzl_module.rs`, blob
  `8309f65c379a12e66fcd53eccfc49cd9f53cb889`;
- `app/slug_loading_v2/src/lib.rs`, blob
  `9e4d4ec028ca3ec7ea95ff88298cb85943f7945a`.

The only new production file is
`app/slug_loading_v2/src/repository_rule_context.rs`; the only new separate
proof file is
`app/slug_loading_v2/tests/repository_rule_host_capabilities.rs`. Maximum
additions are 650 production Rust, 900 proof Rust and 1,550 aggregate Rust. The
10,000-line `bzl_module.rs` may only expose/reuse the existing canonical key
route; it gains no new loader or semantic owner. The effect file delegates the
context value/capabilities to the new module and remains the async DICE owner.

Proof must cover root and canonical defining labels, projection mismatch,
declared-present direct dependency, declared-cold-absent Need before any
invocation and successful retry, sorted view and absent/empty, os name and
architecture, full `os.environ`, dynamic `getenv` with/without default, cold
dynamic-absent Need with staged-plan discard and authenticated retry, warm no-
retry, missing→present→missing A/B/A, dynamic dependency replay, unrelated-name
cache retention, staged-plan discard on every failure/cancellation, retained
observed-input equality even when file bytes are unchanged, and a completed
rejected/cancelled effect that cannot warm-reuse after both extra-absent and
extra-present cells restore to `Unauthorized`. The latter uses direct shared-
key injection only in loading test code; packet 1 separately proves the real
core session performs those transitions. Also prove unchanged file path/mode
semantics and exact non-Windows pinned winsdk bytes. A forced Windows platform
must fail before publication. Run full serial loading plus direct core/Bzlmod
dependents and all scope/format/archive gates.

### 3. Registration proof only

After packet 2 is accepted, activate
`WP-4-5-7A-registered-toolchain-generated-repository-proof`. It may change no
production Rust. Its exact proof allowlist is:

- `app/slug_loading_v2/src/registration_expansion_tests.rs`, blob
  `ce333ab6c6f4e79210ec216d710429e3cd9a575d`;
- `app/slug_loading_v2/tests/build_file_loading.rs`, retained live blob
  `7b1e2a98a54b8fa49ce4bda3c32c6d819f0771c4`;
- `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`, retained live
  blob `c37f74b275baaed29e10e7fc717ecca8f1ff675c`; and
- `app/slug_reapi_v2/tests/reapi.rs`, retained live blob
  `dd4f59cdf2bb4a8e00c5493aa09d17663f0d92ff`.

Maximum additions: 900 proof Rust lines. Fixtures must be exact pinned BCR
sources/assets already authenticated by the catalog and may not introduce a
semantic stub. Prove all four source registration rows in declaration order,
the exact empty non-Windows row 3, no demanded `UnsupportedCatalog`, and
unchanged selected custom implementation, `ctx.toolchains`, configuration and
REAPI output. Only after terminal `ACCEPT` may the retained selected-context R2
candidate return to review.

## Stops and review

`REPLAN` for a whole-map dependency, process-environment read outside CLI
capture, daemon-side ambient fallback, hidden evaluator semantic state, lock
across compute/await, unowned historical snapshot, new loader, ruleset special
case, fabricated repository/path Need for an environment name, unknown-name
publication without a typed retry, repeated Need without non-progress failure,
foreign-workspace Need acceptance, rejected-only observed authorization after
restoration, whole-frontier DICE dependency, Windows realization, exact marker-
byte claim, change outside the frozen
allowlists/caps, or inability to isolate packet deltas from the dirty retained
candidate.

This architecture requires independent terminal review before packet 1 is
activated. Review must explicitly check DICE dependency/equality ownership,
retry/restoration and overlapping-command lifetime, Buck2-derived retained
representation, Bazel source/test authority, Zabel's peer-only role, canonical
loader reuse, failure-before-publication, and the three successor manifests.
