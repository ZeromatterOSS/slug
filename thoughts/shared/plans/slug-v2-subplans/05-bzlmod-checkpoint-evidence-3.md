# Stage 5: Bzlmod Checkpoint Evidence, Part 3

This companion file continues detailed landed evidence for
[05-bzlmod-and-repository-graph.md](./05-bzlmod-and-repository-graph.md).

Use this file for new Stage 5 checkpoint entries after the accepted repository
materialization request/result design. Earlier evidence is in
[Part 1](./05-bzlmod-checkpoint-evidence.md) and
[Part 2](./05-bzlmod-checkpoint-evidence-2.md). Keep each evidence shard below
1000 lines.

## Checkpoint Evidence

### Stage 5 repository materialization request/result implementation

Status: Accepted in `5150dd8f`

Implementation: `slug_bzlmod_v2` now owns a complete-only structural
source-preparation carrier, exact normalized workspace/repository/`RepoSpec`
requests, an immutable per-workspace injected result epoch, and a real cached
per-request DICE projection. Missing or stale results return materialization
Need; Local success derives its logical Host root from the request; Immutable
success retains exact source identity, generation root, and observation
instance. Persistent spec errors are generation-independent, transient
transport/materialization failures are generation-tagged, and repository IO no
longer runs inside DICE.

Equality and evidence: Materialization equality is exact through immutable
root and instance, while source bytes, absence, and typed semantic errors remain
the pruning boundary. Tests prove lawful partial hashing with full request
equality, epoch construction failures, exact-spec isolation, unused and omitted
repository laziness, zero DICE IO, transient retry, logical Local symlink
retargeting, exact immutable-instance selection, cumulative materialization
then path Needs, byte pruning/change/restoration, and pure spec precedence.
One retained graph regression proves unrelated repository result additions and
changes do not invalidate the selected projection, while changing its exact
immutable root or instance does.

Validation: focused `source_preparation_dice` 26/26; full
`slug_bzlmod_v2` 226 tests plus zero doctests; downstream `slug_core_v2` 27
tests plus zero doctests; formatting, diff, exact three-file implementation
allowlist, forbidden repository-IO/obsolete-error/`RepoSpec`-surrogate scans,
and archive guards passed. Independent DICE and pinned Bazel 9.2 terminal
rereviews returned `ACCEPT`.

Residual risk: The runtime still does not produce cumulative result/path
epochs, validate Local roots before observation, retain final immutable
instances across retries, detect exact materialized-output dirtiness, preserve
captured archive bytes through extraction, or publish effects only for the
terminal attempt. Design only
`WP-5-m1-runtime-materialization-preflight-retry-design` next; do not activate
source preparation or edit Rust during that packet.
