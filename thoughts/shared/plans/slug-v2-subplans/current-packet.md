# Current Slug V2 Packet

Packet: `WP-1-m1-loading-inflight-source-lock-oracle`
Milestone: M1 one semantic spine
Owners: `slug-v2-subplans/01-compliance-oracle-harness.md` and
`slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: generate and independently replay one pinned Bazel 9.2 package-loading
source-mutation/output-base-lock oracle before request-revision Rust.

The source-consumer cutover `53152727` and post-cutover audit `47090561`
are fixed. The accepted design selects one Bazel-only fixture and one narrow
coordination table. This packet implements that evidence only; it cannot edit
Rust or infer Slug concurrency from Bazel's serialized clients.

## Exact fixture contract

Add `tests/v2_oracle/fixtures/loading-inflight-source-lock/` with exactly
seven authored files plus generated `expected/oracle.json`:

- `fixture.toml`;
- `workspace/MODULE.bazel`;
- `workspace/a/BUILD.bazel` and `workspace/a/defs.bzl`;
- `workspace/a/before.txt` and `workspace/a/after.txt`; and
- `workspace/b/gate.txt`.

There is no tracked `b/BUILD.bazel`. V1 `defs.bzl` prints one V1 marker and
exports the complete `srcs` list `["before.txt", "//b:b"]`; V2 changes
only that marker and first label to `after.txt`. `a/BUILD.bazel` loads the
list and declares `//a:root`. The runtime gate body declares public
`//b:b` over `gate.txt`.

The fixture is `required_host_os = "posix"`, `daemon = true`, and
`observe_server_epochs = true`. Its first two ordinary commands are the
adjacent group-owned rows:

1. `inflight_v1_loading`: `query deps(//a:root)`;
2. `same_output_base_noblock`: startup option
   `--noblock_for_lock`, command `info`, expected exit 9.

The remaining serial rows are `post_mutation_v2`,
`warm_v2_no_replay`, and `restored_v1`; the last uses ordinary inverse
text mutations. All five capture the same server epoch. Anchored
message-shape expressions require an unmixed V1 primary result, the public
PID-shaped lock diagnostic, unmixed V2 post-mutation and warm outputs with no
warm marker replay, and unmixed restored V1. Command order and literal exits
remain fixture/schema assertions; generated coordination evidence is retained
for provenance.

Permit exactly this optional table shape:

```toml
[concurrent_command_group]
primary = "inflight_v1_loading"
contender = "same_output_base_noblock"
gate_path = "b/BUILD.bazel"
gate_content = "filegroup(name = \"b\", srcs = [\"gate.txt\"], visibility = [\"//visibility:public\"])\n"
mutations = [{ path = "a/defs.bzl", find = "V1_SENTINEL", replace = "V2_SENTINEL" }]
```

`primary` and `contender` must name distinct commands at indexes zero and
one. Parse `mutations` through the existing `Mutation` model, but require
exactly one ordinary text replacement. Reject extra table keys, duplicate
command names, nonadjacency, command-local mutations on either owned row,
non-Bazel execution, non-POSIX fixtures, absent/symlink parents, an existing
gate, escaping paths, a gate at/under any manifest root, an empty release body,
or a second group/schema. This is not a reusable asynchronous scheduler.

## Deterministic runner lifecycle

For the selected group only, `runner.py` must:

1. validate the contained absent gate and create it as a mode-0600 FIFO;
2. create the writer thread and start primary A with `Popen`, pipes, and its
   own process group;
3. treat successful blocking writer `open(O_WRONLY)` as the only readiness
   acknowledgement; reaching `b/BUILD.bazel` is causally after V1
   `defs.bzl` supplied the `//b:b` edge;
4. apply the one V1-to-V2 mutation, then start and collect contender B against
   the same output base within the remaining deadline;
5. require B exit 9, signal the writer to write the fixed BUILD body, collect
   A, close descriptors, unlink the FIFO, and atomically create the identical
   regular `b/BUILD.bazel` for later rows; and
6. build normal records for A then B in declaration order before returning to
   the existing serial loop.

One monotonic absolute deadline bounds gate readiness, B, release, and A
collection. The writer propagates open/write/close failures. The `finally`
path must signal release/cancellation, use a nonblocking cleanup reader to
unblock a writer still in `open`, close every owned descriptor, terminate
then kill and wait for uncollected process groups, join the writer, remove the
FIFO, and preserve the primary failure with cleanup chained. Fail early A
exit, gate timeout, B timeout/success/wrong exit, mutation or release failure,
missing collection, surviving process/descendant, writer failure, or
regular-file replacement failure. No sleep or polling establishes readiness.

## Source authority and compatibility

Use only pinned local Bazel tag `9.2.0` at
`8220c6198837d5c13d53fea211cf3282aa12408a`:

- `src/test/shell/integration/client_test.sh:465-495`,
  `test_noblock_for_lock_reuse_server`;
- `client_test.sh:286-393`, same-output-base serialization;
- `src/main/cpp/blaze.cc:96-128,286-323` and
  `src/main/cpp/startup_options.cc:73,122`;
- `src/test/java/com/google/devtools/build/lib/skyframe/PackageFunctionTest.java:896-938`;
  and
- `src/test/java/com/google/devtools/build/lib/skyframe/LocalDiffAwarenessIntegrationTest.java:104-113,270-291`.

The FIFO proves ordering only. Upstream supplies no final-reobservation
guarantee for an already demanded loading source. Accept the first V1 result
only as stable pinned-version Bazel evidence after generation and two
fresh-root replays; it is not a Slug parity rule. Same-output-base serialization,
exit 9, the named diagnostic, and serial V1/V2/warm/V1 relationships are exact
within this fixture. Slug overlapping requests, source certificates, revision
identity, final validation/retry, barriers/counters, and no-mixed-epoch
publication remain Slug-native and create no Bazel record. Bazel client
serialization does not authorize the production global command lease.

Deliberately exclude module-extension `ctx.read`, repository materialization,
lockfile behavior, `EditDuringBuildTest` execution-only undefined results,
watcher correctness, historical host reads, and any public Slug behavior.

## Allowlist, caps, proof, and stops

Edit exactly:

- `tools/v2_oracle_lib/fixture.py`;
- `tools/v2_oracle_lib/runner.py`;
- `tests/v2_oracle/test_v2_oracle.py`;
- the eight selected fixture paths above; and
- canonical/current/Stage 1/Stage 2 ledgers only for completion and successor
  selection.

Caps: three harness files, one fixture, seven authored fixture files plus one
generated oracle, five records, 430 net production-harness lines, 380 net
harness-test lines, 150 authored fixture lines, 500 generated-oracle lines,
260 net ledger lines, and 1,850 total net lines. The single allowed correction
is consumed by this cap-only increase after the strict parser plus complete
owned-process cleanup proved the original 260/280 budgets underfit.

Proof must include parser invariants; FIFO containment/mode and causal
handshake; success record order; early-exit, timeout, writer, contender, and
cleanup failure injection; exact process reap/join assertions; the focused
fixture generation; two independent fresh-root replays with identical expected
JSON; the full harness test module; local pinned-source-anchor verification;
daemon cleanup; `git diff --check`; archive status; cap accounting; and an
independent evidence/cleanup review. Never inspect or copy Bazel RC contents.

STOP on Rust, Cargo/BUILD, Slug command/server or DICE changes, network access,
JVM/Java artifacts, a second fixture/group/schema, arbitrary fixture
executable, polling/sleep race, module-extension/repository execution,
generated custom repository, generalized scheduler, unbounded process tree,
cap excess, or an out-of-allowlist file. `REPLAN` if Bazel rejects the FIFO,
A does not stably produce unmixed V1 across generation and both replays, the
contender does not deterministically exit 9, serial V2/warm/V1 is unstable, or
both process groups cannot be terminated and reaped deterministically.

## Immediate predecessor record

The accepted docs-only design audited the live harness and pinned Bazel 9.2
source without executing Bazel. It rejected the broader module-extension gate,
classified the client boundary as serialization rather than overlapping
server requests, and froze the fixture/schema/lifecycle above. The compact
post-cutover audit in `47090561` remains the authority for current input
ownership, DICE transactions, the external request-coordinator requirement,
and the later Slug-native two-request/final-reobservation proof.
