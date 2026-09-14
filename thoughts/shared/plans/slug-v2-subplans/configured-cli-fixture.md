# Authentic configured CLI fixture and R2 gate ledger

## Selected policy

The September 11 review-fix instruction selects a portable repository-owned
fixture, including its demanded verbatim upstream metadata, archive/patch
bytes and upstream notices. Use ordinary archive files, not text/base64 bundles.
Inventory actual total size and licenses in `fixture.toml` and `NOTICE`; do not
fetch or vendor registry content merely because it appears in a catalog. The
September 14 typed trace requested all 183 metadata objects in the pinned
catalog before repository loading. Preserve those exact demanded bytes in one
deterministic tar plus a per-entry order/hash/size/URL inventory; this crosses
the fixture-growth checkpoint without adding 177 loose files. Catalog membership
still cannot establish payload-archive demand. Exact missing objects may be
acquired from their pinned source URL after demand is established, using normal
execution/network permissions. Never read credentials or private user RC files.

An explicitly supplied personal cache may seed preparation or diagnose demand;
its location is not committed and is never read by default acceptance tests.
The final fixture owns every required byte and assembles offline in a fresh
temporary directory. A missing/mismatched object fails before Slug starts;
no implicit network fallback, fake upstream module, or success-by-skip.
This policy resolves the former user-level choice in the old manifest. Its F2
assembly is implemented and accepted below; it does not claim that the partial
F1 payload closure or F3 configured-source proof has succeeded.

## Known inputs and question

The original candidate is `27e9e9c0c` on `review/output-conflict-r2`, based on
`97dffd5d4`; `review-evidence/candidate.patch` contains all 19 owners and the
original receipt. Its CLI fixture has four fake-source sites: a root
`local_path_override` for platforms and synthetic platforms MODULE,
`host/BUILD.bazel`, and `host/constraints.bzl` writes. Replace those with the
real sources. Preserve the custom test root's platform/toolchain/BUILD/defs and
conflict scenarios; unrelated upstream modules must not be stubbed to force it.

Known exact authentic content:

| Object | SHA-256 / size where recorded |
|---|---|
| platforms 1.0.0 MODULE | `f05feb42b48f1b3c225e4ccf351f367be0371411a803198ec34a389fb22aa580` |
| platforms source.json | `f4ff1fd412e0246fd38c82328eb209130ead81d62dcd5a9e40910f867f733d96` |
| platforms archive | `3384eb1c30762704fbe38e440204e114154086c8fc8a8c2e3e28441028c019a8`, 7,879 bytes |
| rules_license 0.0.7 MODULE | `088fbeb0b6a419005b89cf93fe62d9517c0a2b8bb56af3244af65ecfe37e7d5d` |
| rules_license 1.0.0 MODULE | `a7fda60eefdf3d8c827262ba499957e4df06f659330bbe6cdbdb975b768bb65c` |
| rules_license 1.0.0 source.json | `a52c89e54cc311196e478f8382df91c15f7a2bfdf4c6cd0e2675cc2ff0b56efb` |
| rules_license 1.0.0 archive | `26d4021f6898e23b82ef953078389dd49ac2b5618ac564ade4ef87cced147b38`, 35,903 bytes |

The real platforms module loads `//host:extension.bzl`, generates host_platform,
and depends on rules_license. The catalog audit verified 184 registry rows: one
registry policy and 183 module/source metadata objects. The bounded portable
trace subsequently requested every metadata object, then reached
`bazel_tools+winsdk_configure+local_config_winsdk` and named the selected
`rules_cc@0.2.17` archive as the first missing payload. The previous staged
runtime recipe used platforms and rules_shell archives plus the rules_shell
patch. Abseil metadata is demanded, but its archive demand remains unknown; its
archive was absent at two checked paths. Catalog membership is not payload
demand.
Use the accepted typed source-observation diagnostic to identify the precise
missing requested source or unsupported producer. Do not infer causality from
a previous truncated display or acquire all 47 declared payloads.

## Provenance and semantic owners

Use pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`,
`IndexRegistry`/`RepoSpecFunction` and the existing Stage 1 fixture provenance
contract. Record source URL, module/version, exact content digest, applied patch
order, strip prefix, source test adaptation, upstream notices and comparison
class. Test root declarations are original fixture code; upstream bodies stay
verbatim. Fixture preparation is test infrastructure, not a Slug semantic owner.

Production keeps the accepted Bzlmod source/session observation, immutable
capture and publication boundaries. Missing source semantics require a separate
producer correction; no command-side filesystem bypass or fresh DICE graph.
The probe supervisor remains bounded and releases child/pipe/temp resources.
Preserve the old probe SHA-256
`8eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2` as historical
evidence. The new bounded question does not turn old inconclusive output into
proof or permit an unchanged blind retry.

## F3 invocation contract

Implement this proposed entry point in the fixture packet:
`python3 tools/v2_oracle/configured_cli_fixture.py prove --harness <lib-test-executable>`.
Compile separately using pinned tools and
`cargo test -p slug_cli_v2 --features native-probe-observer --lib --no-run --message-format=json`.
Select the library test executable from Cargo JSON; the observer feature is
intentionally unavailable to a production binary.

The entry point assembles workspace/registry/mirror in a fresh temporary root
from repository-owned inputs, then reuses the historical driver's bounded
supervision via an explicit portable mode. Reuse its observer-FD setup, process
isolation and cleanup; do not duplicate a second supervisor or inherit personal
cache paths, the obsolete patch path, deliberate missing inputs or diagnostic
success classification. Runtime has a 12-second deadline/15-second absolute
ceiling; preparation is separate and <=60 seconds per operation.

The supervisor supplies `SLUG_SENTINEL_SCRATCH` and a valid inherited
`SLUG_SENTINEL_OBSERVER_FD`, verifies exactly one ignored test via
`<harness> --list --ignored --exact payload_demand_probe::authentic_sentinel_demand`,
and invokes `<harness> --ignored --exact payload_demand_probe::authentic_sentinel_demand --nocapture`.
This opt-in probe uses its explicit ignored-selector check; the ordinary
nonignored preflight helper must remain strict. Reuse selector evidence only
while this executable is unchanged.

F3 requires one selected/executed/passed test, process exit 0, native publication
exit 0 and `SLUG_SENTINEL_NATIVE_SUCCESS_PUBLISHED_0`, with valid observer evidence
and no timeout, output overflow or cleanup failure. This proves the retained
configured source boundary on the recorded current source revision; R2's later
conflict-rejection proof is separate. Missing/unsupported source diagnostics,
sentinel non-demand or hashes alone cannot pass F3. Record source/features,
fixture hashes, command, counts, status and supervisor receipt.

## Acceptance dependencies

| Gate | Current state | Required evidence / successor |
|---|---|---|
| F1 authentic input inventory | partial at the next named payload; all 183 demanded metadata objects and the selected platforms, rules_shell, and `rules_cc@0.2.17` payloads/patches/notices are repository-owned and verified | add exact protobuf 33.4 archive `687e98a471973b5c5fd711750c40b8b82c0ade33f649db65e00b290f29345a2b` from its pinned BCR URL plus notice, then resume the unchanged demand proof |
| F2 portable offline assembly | accepted by focused checkpoint evidence | 19 objects / 901,651 source bytes and 177 bundled metadata entries verify and assemble in a fresh root; missing, corrupt, and semantically mismatched patch inputs fail closed |
| F3 configured source closure | blocked by missing protobuf 33.4 archive while reading `REPO.bazel` for `@@protobuf+//bazel/private/toolchains/prebuilt` | add only that demand-established authentic payload, then rerun once to name the next payload or semantic owner |
| B1 baseline attribution | two failures reproduced on `97dffd5d4` | retain source/environment-specific evidence; characterize remaining reported failures without weakening assertions |
| R1 combined semantic gates | R2 preserved, not accepted | focused selected-request, root-set conflict/sharing, raw-platform identity, concurrency and A/B/A on integrated candidate |
| R2 production consumer gates | pending F2/F3/R1 | one-shot/stable-daemon build/run/aquery conflict rejection before RPC/materialization; cquery remains independent |
| R3 positive sharing and closure | pending R1 | common execution-view/REAPI emits one representative; aquery retains all owners |
| R4 owner/dependent regression and review | incomplete | partitioned full required owner/dependent suites, all failures attributed, independent invariant-to-evidence review |

B1 can be investigated independently of fixture construction. The two proven
baseline failures are
`build_command_root_selects_each_terminal_producer_once_for_duplicate_targets`
(event order) and `resolved_run_view_reuses_exact_executable_filewrite_relation`
(missing retained Host action-environment facts). Other reported terminal event,
exported-source, external query, cquery retry and build/public path failures were
not attributed. Do not call full Core green or waive them based on two controls.

Use the orchestration preflight and classify invocation/environment failures
separately. Compile <=60 seconds apart from tests <=12/15 seconds; partition
suite execution with exact test accounting, rather than raise the user's limit.
Required acceptance is unchanged when a suite is partitioned. Record base and
candidate revisions/features/environment, exact selected count, command and exit.

Only successful F1–F3 acceptance selects recovery of combined R2 on an isolated
worktree. A diagnostic naming a missing/unsupported producer is useful evidence,
but never F3 acceptance.

The September 14 fresh-root checkpoint verified fixture inventory SHA-256
`f4d6a54ff985fe03a5f489613a0f782422bb69fef5b12b1cc13640c96f925a2d`,
then selected and executed exactly one probe test in 6.38 seconds. Native
publication exited 1, the test failed, the observer receipt was valid, and
cleanup completed. The source closure had already consumed the full typed
metadata inventory and selected the verified `rules_cc@0.2.17` payload; its
terminal message was
`toolchains registration row 5: [diagnostic incomplete: CanonicalPackage]`.
This is F3 blocker evidence, not configured-source acceptance. The immediate
prerequisite is a bounded causal diagnostic for that private loader error; it
must not change loading, toolchain, DICE, or source-selection semantics.

The bounded canonical-package projection then passed its 11-test diagnostic
slice and changed the same fresh-root terminal message to
`toolchains registration row 5: CanonicalPackage: Source` in 5.88 seconds, again
with one selected/executed test, valid observer evidence, and complete cleanup.
That accepts the package-load projection and selects the smaller opaque
`RepositoryPackageSourceError` diagnostic prerequisite; it still does not name
an input or accept F3.

The bounded source projection then passed 2 focused package-source tests, 7
request/observation grammar tests, and the 11-test dependent diagnostic slice.
The same F3 run reached `CanonicalPackage: Lookup
package=@@protobuf+//bazel/private/toolchains/prebuilt error=RepositoryIgnore` in
5.89 seconds with the same one-test and cleanup guarantees. This identifies the
package and typed lookup owner, but not yet the repository-ignore variant or
input; the immediate prerequisite is that final borrowed projection.

The repository-ignore projection passed its 2 focused tests and preserved the
package-source handoff tests. The same F3 run reached
`RepositoryIgnore.RouteRepoFile` in 5.65 seconds with the same one-test and
cleanup guarantees. This rules out `.bazelignore` handling and selects the
smaller routed `REPO.bazel` diagnostic prerequisite; it still does not identify
the source/evaluation leaf or accept F3.

The routed `REPO.bazel` projection then passed its 2 focused tests and preserved
the repository-ignore handoff tests. The same F3 run reached
`RouteRepoFile.SourceObservation.CanonicalRequest:
Request.Materialization path=hex:5245504f2e62617a656c kind=Transport` in 5.61
seconds; the bounded message named the failed selected-registry archive capture.
Pinned protobuf 33.4 `source.json` supplies URL
`https://github.com/protocolbuffers/protobuf/releases/download/v33.4/protobuf-33.4.bazel.tar.gz`,
strip prefix `protobuf-33.4`, and SHA-256
`687e98a471973b5c5fd711750c40b8b82c0ade33f649db65e00b290f29345a2b`.
This establishes the next authentic payload demand and accepts the diagnostic
chain; it remains F3 blocker evidence until the archive is repository-owned.
Reconcile landed nodep/archive/file-capture/diagnostic/registry prerequisites
before validating; preservation metadata never ships. R1–R4 completion permits
atomic integration, followed by the demand-scoped Stage 6 execution-group
successor in canonical Live Status.
