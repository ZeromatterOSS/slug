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
| F1 authentic input inventory | partial with every payload demanded so far repository-owned, including rules_java 9.1.0 and all 183 demanded metadata objects | attribute the current bounded deadline before acquiring any further payload; catalog membership does not establish demand |
| F2 portable offline assembly | accepted through the rules_java checkpoint | 28 objects / 8,004,740 source bytes, inventory `4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`, and 177 bundled metadata entries verify and assemble in a fresh root; negative patch/input checks remain green |
| F3 configured source closure | corrected group runtime clears named/automatic invocation; supervised F3 selects one test and stops at rules_cc `_def_parser` computed-default target invocation with valid observer and cleanup | implement the reviewed narrow `attr.label` callback prerequisite on the same unaccepted R2/group stack, pass joint gates, then replay unchanged F3 |
| B1 baseline attribution | two failures reproduced on `97dffd5d4` | retain source/environment-specific evidence; characterize remaining reported failures without weakening assertions |
| R1 combined semantic gates | R2 plus corrected group runtime is preserved through `498ea2f49`, not accepted; focused owner suites pass, real CLI consumers select the same computed-default prerequisite | retain the stack, add only the reviewed package-owned callback prerequisite, then rerun affected selected-request, group, root-set conflict/sharing, CLI and direct-consumer gates for joint atomic acceptance |
| R2 production consumer gates | pending R1 and complete baseline attribution | one-shot/stable-daemon build/run/aquery conflict rejection before RPC/materialization, positive shared execution-view/REAPI proof and partitioned affected suites; cquery remains independent |
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

Recover combined R2 on an isolated worktree before shared group activation. Its
acceptance uses the complete non-F3 owner and consumer gates recorded in Stage 6;
F3 now follows the group runtime because its authentic closure reaches that
guard. A diagnostic naming the owner is useful demand evidence, but never F3
acceptance.

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

The protobuf checkpoint raised the explicit fixture cap to 16 MiB, verified 21
objects / 7,793,087 bytes at inventory SHA-256
`57a78210ed99a85f7461bef726e8153174ad10d62a64a0f65b263fb696f126fd`,
and passed all 3 focused fixture tests. With that archive present, the same F3
run advanced through protobuf's `REPO.bazel` and nested loads before naming
`@@bazel_features+//:features.bzl` in 8.43 seconds. Pinned bazel_features 1.42.1
metadata supplies archive SHA-256
`8189bac9a6bf9cc155a854c4cbebfebf58b9ca7a2d0a67645f7d0c1f83c523ac`
and patch SHA-256
`b69c27e64c4ac5043a3f254d88ef2d8383bbfaefd20881a24ed5b6eb13d4b818`.
That payload/patch is the only next acquisition established by this receipt.

The bazel_features checkpoint verified 24 objects / 7,822,792 bytes at inventory
SHA-256 `0ad0ac33e4275d51db0ad436cf639709170363b9a41bf82026d6c9e30f88b928`
and passed all 3 focused fixture tests, including the new BCR patch equivalence.
The same F3 run advanced to generated repository
`@@bazel_features++version_extension+bazel_features_globals` in 9.29 seconds,
but `ExternalBzlModuleError::Route` had flattened its typed load-route error into
a recursive string. The bounded renderer stopped at its output limit before the
cause. This selects typed route-error retention as the next prerequisite and
does not establish another payload demand.

Typed external route retention then passed the 11-test diagnostic slice and the
natural recursive route test; the observer harness compiled in 40.12 seconds.
The same F3 run traversed the generated bazel_features repository and named
`@@bazel_skylib+//lib:modules.bzl` in 9.36 seconds with valid one-test and cleanup
evidence. Pinned bazel_skylib 1.8.2 metadata supplies archive SHA-256
`6e78f0e57de26801f6f564fa7c4a48dc8b36873e416257a92bbb0937eeac8446`
and no patch. This accepts typed route identity and establishes only that next
payload acquisition.

The bazel_skylib checkpoint verifies 26 objects / 7,878,817 source bytes at
inventory SHA-256
`5800c9ed0df22c05229ddd908812304efa13e31609c5dd7377fd06d43d823818`;
all three focused fixture tests pass. The unchanged F3 proof selected one test
but reached its 12-second wall deadline in `RootCompute`, with valid observer
and complete cleanup evidence. Its latest sampled activity was
`PathObservationKey`, after 80,997 starts, 80,985 finishes, 78,500 dependency
checks, and 9,618 computes. A bounded scratch-only DICE trace independently
completed 92,500 outcomes while still advancing through rules_cc loads. Each of
184 registry-file keys, 157 discovered-module keys, and 156 module-source keys
was visited 115 times as the injected path epoch grew. That confirms the
previously recorded global epoch fanout and selects an exact-demand DICE
projection; it does not establish a new payload demand, semantic loop, or F3
acceptance.

A scratch exact-demand projection preserved completed path-key evaluations but left
the singleton epoch as one invalidation source: a post-change trace still
revisited all 184 registry-file, 157 discovered-module, and 156 module-source
keys 115 times. Native injection now derives 64 fixed DICE partitions from the
sole full `PathObservationEpoch`; every attempt updates all partitions, so
removal clears stale entries while only changed partitions invalidate. Direct
epoch callers retain the full-epoch lookup path. Focused tests cover
unrelated addition, exact change, removal, A/B/A restoration, and retained
provenance; the workspace suite passes 46/46, plus focused bzlmod source and Core
epoch-association tests. The observer harness compiled in 48.22 seconds after
bounded dependency compilation.

With partitioned injection, the unchanged F3 run selected and executed one test
and reached a typed terminal in 2.96 seconds, with valid observer and complete
cleanup evidence. It named `@@rules_java+//toolchains/REPO.bazel` with the
selected-registry archive capture failure. Pinned rules_java 9.1.0 metadata
supplies URL
`https://github.com/bazelbuild/rules_java/releases/download/9.1.0/rules_java-9.1.0.tar.gz`,
empty strip prefix, and archive SHA-256
`4e1a28a25c2efa53500c928d22ceffbc505dd95b335a2d025836a293b592212f`.
This accepts the bounded path-fanout correction and establishes only that next
payload acquisition; F3 remains incomplete.

The rules_java checkpoint verifies 28 objects / 8,004,740 source bytes at
inventory SHA-256
`4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`;
all three focused fixture tests pass, including fresh-root offline assembly and
negative input/patch checks. The pinned archive matches SHA-256
`4e1a28a25c2efa53500c928d22ceffbc505dd95b335a2d025836a293b592212f`
and its copied Apache license matches the manifest.

The one allowed unchanged F3 rerun selected one test and reached its 12-second
wall deadline with valid observer and complete cleanup evidence. The observer
recorded 53,789 starts, 53,774 finishes, 47,326 dependency checks, 47,316
dependency-check finishes, 7,553 computes, and 7,548 compute finishes. Latest
sampled activity was `HostCanonicalSelectedModuleDefinitionObservationKey` in
the root-compute phase. This names no later archive and is not evidence of a
semantic loop. A bounded causal fanout diagnostic is the immediate prerequisite;
acquire no further payload until the same proof names it.

Scratch observer counters then disproved the final sampled host-definition key
as the principal owner: it computed 13 times, while `PathObservationKey`
computed 1,820 times and `ExternalBzlModuleObservationKey` 1,726 times. A
bounded per-identity snapshot recorded 1,664 external-module computations over
135 identities; 129 identities repeated for 1,529 extra computations, led by
`@@rules_cc+//cc:cc_library.bzl` at 177. A source-syntax cache experiment left
the proof at the same 12-second deadline and was fully reverted.

The selected loading owner was the external-child loop, which returned the
first preparation need even though `SourcePreparationNeeds` already supports
unions. It now collects independent sibling needs into one frontier, retains
the first incompatible union as a typed terminal, and still stops immediately
at the first decisive semantic or observation error. The complete loading suite
passes 560/560 with one intentional ignored test in 1.17 seconds; the focused
external-Bzl slice passes 30/30, including sibling need collection and decisive
prefix behavior. The final observer harness compiled in 17.25 seconds.

The final unchanged F3 proof selected and executed one test and reached a typed
terminal in 9.87 seconds, with valid observer and complete cleanup evidence. It
recorded 45,130 starts, 39,240 dependency checks, and 7,434 computes. The error
is `toolchains registration row 7: CanonicalPackage: Attempt.Loading`; this is a
bounded diagnostic leaf, not F3 acceptance or authority for another payload.
The immediate prerequisite is the existing package-attempt loading error's
typed projection.

The bounded borrowed projection now renders the retained `LoadingError` under
`Attempt.Loading` without changing package evaluation or equality. Its focused
diagnostic test covers exact nested text, escaping, ownership and the 3,072-byte
output bound; the complete loading suite passes 561 active tests with one
intentional ignored test in 1.24 seconds. The observer harness completed its
final compile slice in 23.35 seconds.

The unchanged F3 proof then selected and executed exactly one test and reached
the same package in 9.98 seconds. Native publication exited 1, the probe failed
as expected, observer evidence was valid, and cleanup completed. The observer
recorded 47,625 starts and finishes, 41,735 dependency checks and finishes, and
7,432 computes and finishes. The exact terminal is rules_java
`@@rules_java+//toolchains:BUILD:138` calling
`@@rules_cc+//cc:cc_library.bzl:19`, where target invocation fails with
`named execution-group semantics is unsupported`. This confirms the existing
Stage 6 shared execution-group semantic owner and establishes no new archive
demand. The later reconciled R2 trial also showed its real CLI consumers enter
this builtin registration boundary. Independent review therefore selects
complete groups on top of unaccepted R2, followed by joint atomic acceptance
and F3. Neither owner may land alone.

Reconcile landed nodep/archive/file-capture/diagnostic/registry prerequisites
before validating; preservation metadata never ships. R1–R4 completion now
requires the combined R2/group stack and its joint gates.

The computed-default checkpoint `dab5cb5ea` then passed its focused and broad
library gates. A fresh portable fixture assembled the same 28 objects, 177
registry metadata files and 8,004,740 source bytes at inventory SHA-256
`4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`.
The supervised F3 selected and executed exactly one test in 9.82 seconds;
observer telemetry was valid and cleanup reported no children, open pipes or
live process group. Native publication exited 1 at rules_java
`@@rules_java+//toolchains:BUILD:205`: `alias.actual` contains `select()` but
Slug marks it nonconfigurable. This proves the callback boundary cleared and
selects only the native configurable-alias prerequisite. It establishes no new
payload demand and does not accept F3.

The configurable-alias checkpoint then passed its exact loading, configured
analysis and query proofs. The unchanged supervised F3 assembled the same 28
objects, 177 metadata files and 8,004,740 source bytes, selected and executed
one test in 10.26 seconds, and retained valid observer and complete cleanup
evidence. Native publication exited 1 at rules_java
`@@rules_java+//toolchains:BUILD:365` ->
`java/toolchains/java_toolchain.bzl:27`: `_java_toolchain(**attrs)` reaches
`target invocation for rule initializer is unsupported`. This establishes no
new payload demand and selects only the bounded initializer prerequisite.

The selected initializer is already in authenticated rules_java 9.1.0 bytes.
`java/common/rules/java_toolchain.bzl:253-265` receives `**kwargs` and checks
seven legacy names, but `toolchains/default_java_toolchain.bzl:149-178`
authentically supplies six of them as scalar Labels and omits `deps_checker`.
Those six pass through unchanged. Synthetic loading evidence owns singleton and
empty-list normalization through `_legacy_any_type_attrs`. The successor changes
no fixture object, mirror,
metadata or assembler path. Its portable gate remains the same ten exact CLI
selectors followed by unchanged supervised F3.
