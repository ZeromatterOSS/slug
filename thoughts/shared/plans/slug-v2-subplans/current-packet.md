# Current Slug V2 Packet

Packet: `WP-2A-m1-root-host-request-revision`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: implement the accepted smallest Rust-native root-host
request-revision/source-certificate vertical.

The source-consumer cutover `53152727`, post-cutover audit `47090561`, and
focused oracle implementation `2ffad088` are fixed predecessors. The
reviewed design is accepted in `94324880`. This packet implements only its
private one-file Host vertical; it cannot activate public commands, loading,
Bzlmod, repositories, or materialized paths.

## Accepted prerequisite

`loading-inflight-source-lock` records five Bazel 9.2 rows in one server
epoch: unmixed V1 primary, exit-9 same-output-base contender, V2 after mutation,
marker-free warm V2, and restored V1. Generation, two independent fresh-root
replays, and one post-correction replay match the checked-in normalized oracle.
The implementation passed 19 focused harness tests; the full module passed 119
tests with only its three pre-existing stale fixture-cardinality assertions.
Production harness, harness tests, authored fixture, and generated oracle closed
at 325/430, 323/380, 95/150, and 169/500 net lines. The FIFO proves causal
ordering only; Bazel's V1 terminal and serialized clients do not define Slug
request concurrency or final validation.

## Live ownership and DICE findings

`WorkspaceRuntime` owns the sole retained `Arc<Dice>`. Its production
updater/commit sites are the native attempt injection, legacy observation
adapters, restoration, and selected-snapshot publication in
`app/slug_core_v2/src/runtime/dice.rs`; a separate test-only acceptance seam
is not a competing production commit. The design must enumerate the live
production sites again immediately before implementation and route every site
that can race the admitted family through one revision owner.

Neither existing session owner is the future architecture:

- `NativeDemandSessionOwner::acquire` returns `Busy` whenever one command is
  open and retains `AcceptedNativeDemandSnapshot`, so it cannot prove
  overlapping requests and cannot become the request certificate store.
- `RepositoryMaterializer::begin` also permits one active session. Repository
  production, materialized namespaces, and external-repository requests are
  excluded from the first vertical rather than moving that lease outward.
- legacy `WorkspaceSnapshot`, `WorkspaceRawSnapshot`, and
  `WorkspaceDirectorySnapshot` adapters remain scaffolding outside the
  admitted family. They are not accepted source certificates.

The vendored DICE surface supports the bounded design:

- a `DiceTransaction` is fixed at creation, exposes exact-version
  `equivalent`/`equality_token`, and must never be stored by a computation or
  result;
- `existing_state` reads the current version, while `changed_to` batches
  typed injected values and `commit` publishes a successor against the newest
  state;
- DICE exposes no conditional-version or compare-and-swap commit and no public
  changed-key compatibility diff;
- A/B/A reuse and shared-work cancellation are supported, but do not provide a
  host-history owner; and
- projection keys cannot reobserve host state or commit a successor.

Therefore a DICE key cannot own final validation. A private
`WorkspaceRuntime` request-revision owner must linearize reobservation and
publication outside DICE.

## Selected first vertical

The implementation packet may add one private root-host file request family.
It is production Rust, but has no public command/server activation yet.

A request owns:

- the originating runtime identity and exact base DICE equality token;
- an immutable request overlay split into a semantic projection retained in
  root-key identity and presentation-only data retained outside DICE;
- one root-host `PathObservationDemand` using the existing typed
  `PathObservationKey`/`PathObservationEpochKey` producer chain;
- one provisional terminal/effect buffer; and
- one heap-independent source certificate containing the exact demand and
  observed `PathObservationResult`.

The root key structurally includes workspace, contained relative path, and the
relevant semantic projection. It computes the request-revision injected key and
the typed path observation. Presentation text, diagnostics, and test barriers
remain request-local and cannot alter root identity. The first vertical admits
exactly one Host-namespace file-bytes demand. Directory/glob unions,
materialization instances, repositories, root-module/loading migration, and
public output are successors.

The coordinator is service-owned and retains no accepted semantic snapshot. It
may retain only a monotonic Slug-native revision allocator and one asynchronous
publication mutex. All observations live in DICE values or ephemeral request
certificates.

## Attempt and publication algorithm

For every request:

1. under the short revision owner, obtain the current fixed transaction and
   base equality token; release the owner before computation;
2. compute the root provisionally with no runtime lock held;
3. on a typed path Need, observe that exact Host demand through the existing
   native path-observation kernel, enter the revision owner, confirm the base
   is still current, inject the complete path epoch plus successor revision in
   one `changed_to` batch, commit, release, discard the attempt, and retry;
4. on a provisional terminal, retain its exact one-demand certificate and
   buffered effects, then enter the revision owner;
5. read `existing_state`; if it is not equivalent to the provisional base,
   release, discard, and retry from the latest state;
6. reobserve the exact certified demand while holding only the publication
   owner. If unchanged, linearize acceptance and make the buffered terminal
   eligible for its private caller;
7. if changed, batch the new typed observation and successor revision, commit
   once, release, discard the stale terminal/effects, and retry; and
8. on cancellation, failure, or bounded nonprogress, publish nothing and drop
   the request's certificate, transaction, buffers, barriers, and interests.

The publication owner may span `existing_state`, exact host reobservation,
`changed_to`, and `commit`. It must never span `compute`, Starlark
evaluation, repository/materializer work, terminal selection, or event
formatting. The implementation must route all production commits capable of
racing this private family through the same owner; otherwise `REPLAN` because
DICE has no conditional commit.

Because the owner is held across `existing_state` and `commit().await`, it
must be an async `tokio::sync::Mutex`, never `std` or `parking_lot`.
Every held-path operation must be a leaf that cannot reacquire the owner;
the typed `changed_to` commit must perform no compute or callback re-entry.

Attempt outcomes are explicit:

- `Accepted { revision, terminal }`;
- `RetryVersionAdvanced { observed_current }`;
- `RetrySourceChanged { successor, changed_observation }`;
- `ComputeFailed`, `ObservationFailed`, `InjectionFailed`, or
  `PublicationFailed`;
- `Cancelled`; and
- `RetryNonProgress` or bounded exhaustion.

No failure or retry carries a publishable terminal. A transaction is attempt
scratch and never enters these retained values.

## Compatibility and memory

Exact within the admitted family: serial file present/bytes/absence/error
semantics plus the oracle-backed changed-source invalidation, compatible warm
reuse, and restoration relationships.

Slug-native: relevant/irrelevant overlay identity, overlapping-request
isolation, revision numbers, equality tokens exposed only to tests,
certificate representation, final reobservation, no-mixed-epoch publication,
provisional-output suppression, retry counters, and deterministic barriers.
Unavailable historical filesystem reads, materialized/repository sources,
directory/glob certificates, and public overlapping command behavior remain
unsupported/deferred.

Memory classes:

- service: revision owner, publication mutex, and monotonic allocator;
- DICE-retained semantic: injected revision/path epoch, root values, dependency
  edges, and equality-cutoff values;
- command: immutable overlay, attempt, certificate, cancellation interest, and
  provisional terminal/effects;
- scratch: reobservation buffers and deterministic test barriers.

There is no retained evaluator value, transaction, whole-workspace snapshot,
manual accepted semantic cache, spawned worker, repository root, or transfer in
this vertical. Retry, failure, cancellation, and request drop release all
command/scratch ownership; shutdown drops the service owner after DICE.

## Implementation allowlist and caps

Edit exactly:

- new `app/slug_core_v2/src/runtime/request_revision.rs`;
- `app/slug_core_v2/src/runtime/mod.rs`;
- `app/slug_core_v2/src/runtime/dice.rs`;
- canonical/current/Stage 2 ledgers for completion and successor selection.

No Cargo or BUILD edit is expected because the crate already globs Rust sources
and has the required dependencies. Caps are three Rust paths, one new
module, 560 net production lines, 700 in-module test lines, 260 ledger lines,
and 1,520 total net lines. One correction may adjust caps, but may not add
behavior or files.

Proof must include two genuinely overlapping requests with deterministic
post-demand barriers; relevant-overlay separation and irrelevant-overlay
shared reuse; V1 mutation after demand; stale V1 terminal discard; V2-only
acceptance; warm V2 and A/B/A; exact reobservation/commit/retry counters;
one-waiter and last-waiter cancellation; forced observation, injection,
publication, and nonprogress failures; no leaked task/interest/buffer; no
publish before validation; lock-state assertions at every compute barrier;
full `slug_core_v2` tests; `cargo clippy -p slug_core_v2 --all-targets -- -D
warnings`; targeted Bazel Rust tests if available; `git diff --check`;
archive status; line accounting; and independent ownership/cleanup review.

STOP on public command/server/Bzlmod changes, repository/materialization
sessions, a second DICE graph, a global command lease, reuse of
`AcceptedNativeDemandSnapshot`, a manual semantic side store, mutable global
options, command-side replay, source reads outside the typed observation owner,
watcher correctness, retained transactions/evaluator values, custom DICE
scheduler, spawned background worker, lock across compute/Starlark, unbounded
retry, out-of-allowlist files, or cap excess.

`REPLAN` if the implementation cannot close every competing production
commit site for this family, cannot demonstrate true overlap on one DICE,
requires repository-session concurrency, cannot suppress a stale terminal,
cannot cancel without retained request ownership, or needs historical host
reads.

## Immediate successor

Accept this private vertical only after its complete proof and independent
ownership/cleanup review. Then audit the smallest loading/public migration;
do not combine that successor with this implementation.
