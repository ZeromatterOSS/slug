# Current Slug V2 Work Packet

Packet: WP-7-55-m7a-solib-demand-observation-r1
Status: accepted

## Outcome and inherited boundary

Identify the actual native Solib constructors demanded by the configured production
root before implementing them. WP754 is accepted at 40e3c599e; its
[inventory](./solib-construction-inventory.md) owns pinned semantic/callsite sources,
repository-placement and sharing gaps. The previous turn made progress by completing
that source inventory. No live query/process is inherited.

One bounded action observation selects SolibSymlink from deps(//app/slug_cli_v2:slug).
Record every returned owner's label/rule class, exact configuration/checksum/tool bit,
ordered input-depset traversal, one input/output artifact ID/path/tree flag, and observed
execution_platform/execution_info fields. ActionGraphDump.java:237–250 serializes the
owner's platform and action execution info, not the Solib action's empty platform/exec
property overrides used by key semantics. Preserve those distinct source facts; do not
assert the serialized platform is empty. Do not assume the historical count is still two.
Attribute each native route using authenticated owner BUILD/ruleset source and explicit
arguments; do not infer source-vs-derived identity or constructor from path shape alone.
The analysis_v2.proto Artifact schema has no source/derived boolean. If evidence cannot
resolve classification, preserve that unknown rather than expanding the query silently.

This observation adds no Slug runtime admission. Existing exact slices and Slug-native
configuration/output/action identity remain unchanged; native Solib execution, aquery
projection and exact ActionKey remain unsupported/deferred. No Slug/Cargo invocation,
build action, full production build or F3 replay.

## Frozen invocation and environment

Root source baseline 40e3c599e, clean before packet docs. Hash MODULE.bazel,
MODULE.bazel.lock, Cargo.toml, Cargo.lock, Cargo.Bazel.lock, .bazelversion and
app/slug_cli_v2/BUILD.bazel before/after; reject a changed tracked implementation tree.

Use the cached executable
/run/media/system/Colossus/dev-home/.cache/bazelisk/downloads/sha256/7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694/bin/bazel,
verified SHA256 7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694.
A bounded --version preflight must say 9.2.0.

Use --batch --ignore_all_rc_files --noblock_for_lock and the existing workspace
output base /run/media/system/Colossus/dev-home/.cache/bazel/_bazel_wgray/343a846ed75d86315cf38594423501b5.
Its external tree already contains rules_rust, rules_cc, crate and nightly toolchain
repositories. No live Bazel/java process was observed in preflight. Do not delete its
state or restart another user's server. RC omission makes this a current observation
under the named flags, not an exact replay of WP714's ambient options.

Command: aquery 'mnemonic("SolibSymlink", deps(//app/slug_cli_v2:slug))'
--output=jsonproto --include_artifacts --noinclude_commandline
--@rules_rust//rust/toolchain/channel=nightly --lockfile_mode=error --nofetch
--repository_disable_download --noshow_progress --color=no --curses=no.
No build/test command is authorized. Repository-cache staleness warnings limit source
freshness claims; authenticate relevant loaded ruleset/owner bytes separately.
This is cached metadata observation, not a hermetic network-isolation claim.

20-second query cap, separate from --version and source preparation; expected a few
seconds from historical queries. This is a packet operation cap, not a user-imposed
global deadline. Supervise one process group, with at most 5s TERM then 5s KILL cleanup
using the existing oracle runner's _stop_group. Receipt includes exit/timeout/elapsed,
PID/group-cleanup result, binary/input/output hashes and exact argv. Never print/read
user RC files or credentials. Invocation/environment corrections stay in this packet
when safe and supported by a new diagnosis; a resource stop preserves evidence and
requires selecting a smaller viable check, not an identical automatic retry.

## Scope and gates

Root owns manifest/canonical scheduling and compact bootstrap/inventory result notes.
Scratch runner and raw evidence live under target/wp755; preserve a compact extracted
result with provenance under thoughts/shared/plans/slug-v2-subplans/solib-demand.json
so missing temporary storage cannot erase the resolved demand. Raw query is evidence,
never a Slug runtime input. No production, lockfile or fixture edits.

Independent design review checks the exact filter, process cap, artifact-schema limits
and source attribution before query. Independent final review checks actual rows and
source attribution; use other workers for distinct owner-source checks as results arrive.
Validate nonempty rows, complete references/acyclic path and depset graphs, exact
SolibSymlink mnemonic and one regular input/output per action; classify unfamiliar
rows explicitly. Zero rows/failure supplies no positive demand evidence and cannot close the gate.
Run git diff --check, plan status and archive checks; commit accepted evidence.
No new parity test or fixture is needed for an observation-only packet.

## Diagnosed cache recovery

Independent design review ACCEPT after separating aquery's owner platform/execution
info from native key semantics. The first invocation exited 1 in 1.494s before analysis:
the slug_crates symlink points to an absent repository-cache generation. Zero processes,
no timeout, clean group and unchanged pins; preserve target/wp755/receipt.json.
No reusable generation exists in the bounded inspected output-base/cache locations.

Allow one corrected observation with --fetch in place of --nofetch, keeping every other
flag and the 20s cap. Remove CARGO_BAZEL_REPIN and REPIN from the child environment.
Permit normal cache-backed repository metadata materialization, never repinning or
building/bootstrap-compiling a generator; new cache misses need diagnosis. Independent
source review confirmed pinned DownloadManager.java:240–325 permits download-cache and
distdir hits before enforcing download-disable; the current hub repository implementation
only writes the supplied contents (rules_rust private/crates_vendor.bzl:742–749).
Repository execution and Bzlmod registry lookup are not network-isolated by this flag.
Save the corrected receipt under target/wp755/cached-fetch without replacing the first.

That correction restored the hub and then exited 1 in 1.548s on a blake3-1.8.5
download-cache miss, again zero processes, clean group and unchanged pins. Its local
Cargo archive matches the exact lockfile SHA256. Permit one source-verified distdir
recovery: stage only current Cargo.Bazel.lock HTTP crate archives found in Cargo's
cache and matching their expected hashes, in separate scratch directories named by
digest, with URL basename download. Record URL/source/digest/staged path; never
overwrite differing bytes. Bazel owns persistent download-cache insertion, including
canonical URL IDs (DownloadManager.java:282–317). Retain download-disable, --fetch,
all other flags and the 20s cap. This is diagnosed environment preparation, not a
semantic retry. Preparation cap60s; receipt and query under target/wp755. Missing
archives remain explicit; do not relax pins or repin to avoid cache failures.

The distdir query imported verified local crates and reached the nightly toolchain,
then exited 1 in 6.866s because rustfmt's pinned archive is absent from both Bazel
and Rustup caches (zero processes, clean group, unchanged pins). Bounded acquisition
of that exact MODULE.bazel archive is normal environment repair: HTTPS URL
https://static.rust-lang.org/dist/2025-09-14/rustfmt-nightly-x86_64-unknown-linux-gnu.tar.xz,
SHA256 d9206e55ee07ae3ba9de7190142a8e422a72f300726fa8e094a6300c7128aa82.
Download to packet scratch with a 30s cap, verify before reuse via distdir, and keep
the query's download-disable policy. Inspect the other five named archive cache
entries before another query; any further absence requires its own precise diagnosis.
No compiler/build action is requested; repository setup may inspect tool metadata.
No pins change.

## Acceptance receipt and next boundary

The corrected invocation succeeded in 10.518s, exit0, with two SolibSymlink actions,
zero executed build actions, no timeout, clean process-group exit and unchanged pins.
Raw SHA256 0a796b67c943164270c2d682cc6777b00adb1443d45f9a821a3aff3c403fe9ed;
full receipt SHA256 1039959c40b26f8500d94975d42ae926766a8e35a08d2ccbc794fc0c5d6b5eed.
[solib-demand.json](./solib-demand.json) retains the complete parsed raw graph,
exact non-distdir argv, explicit distdir elision/provenance, all seven input hashes,
failed-attempt receipt hashes and five archive-authenticated source witnesses.

Both rows belong to @@rules_rust+//ffi/cc/allocator_library:cc_allocator_library:
k8-fastbuild target and k8-opt-exec tool configurations. The real cc_library route
declares the shared-library input and calls dynamic_library_symlink with preserve_name
false and prefix_consumer false. This is a fourth callsite route absent from WP754's
initial candidates. Pinned OutputDirectories.java:147–148 also corrects that inventory:
default nonsibling layout shares the configuration bin root; repository information
enters the mangled input root-relative path. Native action key platform semantics and
serialized owner platform remain distinct. The filtered graph cannot prove downstream
consumption/execution of these declared actions or a complete C++ action inventory.

Select the bounded False/False native Solib construction and structural action boundary
next. Its design must resolve input root-relative/repository projection, bin-relative
output declaration, same-owner/cross-owner sharing and explicit runtime gating before
implementation. Full cc_library feature/compile/link/toolchain primitives remain open;
adding this constructor alone cannot establish full allocator analysis or execution.

Independent final review ACCEPT. The source/receipt verifier passed two rows, exact
observed mangling, five archive-backed witnesses and seven unchanged pins in 0.69s.
Extractor validates all graph references and cycles; the two input depsets each have
one direct member, so absent serialized traversal order does not affect this result.
git diff --check, plan status and archive checks pass. No Slug tests or builds ran.
Recorded preparation-to-acceptance window approximately 15 minutes (runner created
17:37 UTC, final checks 17:52 UTC on 2026-09-21); earlier inspection was not timed.
Query attempts total 20.426s; no Rust compilation or build actions. Final review about
two minutes. Environment repair recovered authoritative demand; no semantic REPLAN.
M7A remains partial and M8 unproved.
