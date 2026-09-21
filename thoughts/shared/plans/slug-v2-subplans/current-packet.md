# Current Slug V2 Work Packet

Packet: WP-7-57-m7a-solib-consumption-observation-r1
Status: accepted

## Outcome and inherited scope

Resolve declared downstream input edges for the two allocator Solib outputs
before selecting native execution versus missing C++ analysis primitives.
Previous turn is progress: WP756 accepted at 0763cec2b implemented False/False
construction and structural sharing, with 15 focused tests and CLI compilation.
Reuse [solib-demand.json](./solib-demand.json) and
[solib-construction-inventory.md](./solib-construction-inventory.md).
Existing retained query dumps contain producer-only evidence, not consumers.

One bounded production aquery selects both the allocator's backing shared-library
inputs (positive controls selecting Solib producers) and their mangled outputs
(consumer edges). One dump permits exact artifact-ID joins; do not join consumers
by owner/configuration, since tools can cross configurations. A zero-consumer
result is valid only with the expected target and exec producer controls present,
fully validated graph references and matching original input/output paths.
Preserve unexpected rows/configurations explicitly rather than assuming two.
Do not infer source-vs-derived identity from analysis_v2 Artifact or path shape.

This adds no Slug runtime admission. Existing exact constructor mechanics and
Slug-native configuration/path/action identities remain unchanged; Solib execution,
full C++ compilation, aquery/ActionKey projection and M8 remain unsupported/unproved.
A declared edge is not proof of execution or selected CLI default-output reachability.
Absent declared edges under these flags is not global absence under other options.

## Pinned source and command

Bazel 9.2 source 8220c6198837d5c13d53fea211cf3282aa12408a:
query2/aquery/AqueryUtils.java:44–61,70–112 selects inputs and flattens the input
depset; AqueryActionFilter.java:46–55 uses full-string regex matching on exec paths.
AqueryProcessor.java:194–195 loses filters on a non-function query root, so no
union expression. analysis_v2.proto and KnownNestedSets own graph/schema limits.
Rules_rust 0.73 utils.bzl:229–254 and rustc.bzl:353–359,738–795 choose static/PIC
before interface/dynamic library inputs; authenticate relevant release source.

Exact query:
inputs("bazel-out/[^/]+/bin/(external/rules_rust[+]/ffi/cc/allocator_library/libcc_allocator_library[.]so|_solib_k8/libexternal_Srules_Urust[+]_Sffi_Scc_Sallocator_Ulibrary_Slibcc_Uallocator_Ulibrary[.]so)", deps(//app/slug_cli_v2:slug))

Use cached SHA256-verified Bazel executable
/run/media/system/Colossus/dev-home/.cache/bazelisk/downloads/sha256/7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694/bin/bazel
Verify executable SHA256 and --version=9.2.0 against WP755 before use.
Flags: --batch --ignore_all_rc_files --noblock_for_lock,
--output_base=/run/media/system/Colossus/dev-home/.cache/bazel/_bazel_wgray/343a846ed75d86315cf38594423501b5,
aquery <query> --output=jsonproto --include_artifacts --noinclude_commandline
--include_pruned_inputs --include_aspects --@rules_rust//rust/toolchain/channel=nightly
--lockfile_mode=error --nofetch --repository_disable_download --noshow_progress
--color=no --curses=no. Include-pruned selects original inputs plus scheduling
dependencies, a conservative declared-input view shared by filter and dump.
Include-aspects explicitly retains aspect actions in the scoped graph. No-edge
results do not prove absence from runfiles/output groups, tree membership or
root default-output selection; preserve that residual rather than broadening claims.
No build/test command, source helper execution, credentials/RC inspection, or Slug
binary invocation. No repin or lockfile mutation. Hash relevant input pins before/after.

## Ownership, scope and validation

Root owns runner/query/process cleanup, scheduling and integration. Scratch under
target/wp757 reuses WP755 runner/process-group logic. Twenty-second query cap,
separate from source preparation/version preflight; expected roughly ten seconds.
This is a packet operation cap, not a global user deadline. Inspect current Bazel/
Java/Cargo processes before invocation; do not interfere with another user's server.
Dedicated process group, at most5s TERM plus5s KILL cleanup. Preserve each receipt,
exact argv, hashes, exit/elapsed/timeout/group status; no overwrite of failed evidence.
Diagnose environment failures before a corrected invocation, never relax pins.

Extractor worker owns scratch target/wp757/extract.py after design acceptance:
validate duplicate IDs/JSON keys, path/depset cycles and complete action references;
retain matching artifact IDs/paths, ordered output IDs, owner/rule/configuration,
mnemonic and serialized execution platform/info. Record flattened membership only,
not an exact nested-set traversal-order claim. Same-dump producer output IDs bind
consumers. Negative extraction checks must reject a missing control, dangling
reference and fabricated/nonmatching input row. No persistent Slug state is changed.

Tracked allowlist: current manifest, canonical Live Status, bootstrap-readiness,
solib-construction-inventory and new solib-consumption.json. Prefer compact matched
edges/source witnesses/receipt plus raw hash over retaining thousands of irrelevant
input artifacts; retain the complete raw graph only if small. Raw observations are
evidence, never Slug runtime input. No implementation, fixture, dependency or lock edits.

Independent design review checks query semantics, positive controls and negative
claims; final review checks actual rows/source attribution and successor selection.
Source worker authenticates static/dynamic selection and runtime-file branches from
pinned archives while root runs the query. No Cargo/daemon suite is relevant.
Run extraction/negative checks, git diff --check, plan status and archive checks.
Commit accepted evidence with elapsed/review time and actual gate advanced.

REPLAN only if producer controls cannot identify the known outputs or evidence
requires a broader observation/semantic decision; root may select a bounded safe
successor within the ongoing plan. Native execution is selected only if current
evidence demonstrates demand; otherwise retain its explicit gate and advance the
missing production C++ analysis prerequisites. M7A remains partial and M8 unproved.

Independent design review ACCEPT. Bazel --version and binary SHA256 preflight
match 9.2.0/WP755. Previous turn was progress; current checkout starts clean at
0763cec2b and no live Bazel/Java/Cargo/slugd process was found before preparation.

## Observation and selected successor

The query succeeded exit0 in 2.452s, with zero executed build actions, no timeout,
clean process group, and all 11 input pins unchanged. The only two returned
actions are the expected target/exec Solib producers, each matched through its
backing library input. Neither output artifact ID occurs in any action input.
Raw SHA256 0a796b67c943164270c2d682cc6777b00adb1443d45f9a821a3aff3c403fe9ed
is byte-for-byte identical to WP755, so reuse its complete retained parsed graph.
[solib-consumption.json](./solib-consumption.json) owns the new invocation/pins,
same-dump output IDs and explicit zero consumer lists, without another graph copy.
The observed graph can reuse WP755's strict parser; no general consumer-graph
parser is needed for a result identical to the accepted two-producer graph.

Six loaded rules_rust/rules_cc witnesses match their exact release archive
members and hashes. Static/PIC preference explains why a retained dynamic field
need not become a Rust input; this is a conditional source explanation, not a
query observation of selected static archives. cc_library data_runfiles can still
retain a dynamic artifact, so do not generalize the negative to runfiles or other
options. Native Solib execution remains explicitly deferred.

Select the bounded C++ feature-model/configuration implementation contract next,
before compile/actions2ctx: authenticated cc_library.bzl:30–38 calls toolchain
lookup and configure_features before compile at :56. Provider construction needs
native cc_toolchain_features (CcStarlarkInternal.java:257–269), and feature
configuration uses default_features_and_action_configs plus configure_features
(CcToolchainFeatures.java:1230–1311). The successor must own complete toolchain feature inputs and the
required structural ctx/Cpp projections before activating selection; existing
Rustc tests with authored toolchains/feature_configuration=None do not prove this
surface. Compile/link primitives remain separately open. No new broad query/build
or native execution is implied by that selection.

Validation: python3 target/wp757/extract.py --self-test target/wp757/actions.json
passes exit0, two positive and seven negative controls plus two expected producer
controls and zero consumer edges. Root independently reused WP755's strict reader
on the new dump. Missing controls, dangling references, nonmatching rows, path/
depset cycles and duplicate IDs/JSON keys fail closed; synthetic membership is
only a validator control, not production evidence. Both archive hashes and six
loaded-file/member comparisons pass. No new fixture corpus or Rust changes;
git diff --check, plan status and archive checks pass. No process remains live.

Independent final review ACCEPT. Durable receipt comparison also passes exact
argv/raw graph, all 11 current pins, two controls, zero edges and six source
witnesses. Packet elapsed approximately 11 minutes (18:16–18:27 UTC), one query
2.452s, no Rust compilation/tests or build actions; independent review overlapped
source/extraction work. No environment repair or semantic REPLAN was needed.
Gate advanced: producer registration no longer supplies an assumed runtime demand;
the next implementation priority is the preceding C++ feature model/configuration.
