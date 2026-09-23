# Current Slug V2 Work Packet

Packet: WP-7-87-m7a-linux-module-tool-filegroups-r1
Status: accepted; independent final ACCEPT

## Outcome and compatibility

Admit the two Linux x86-64 Bazel 9.2 release-embedded module-tool binaries as
immutable `@bazel_tools` source Files, then prove that observed configured
analysis of the authentic child filegroups
`@@bazel_tools//tools/cpp/modules_tools:{aggregate-ddi-bin,generate-modmap-bin}`
selects those Files and forwards each singleton as its executable. This is the
direct configured child boundary, not the `tools/cpp` parent or a public
`cquery` command.

Exact: Bazel 9.2 Linux x86-64 release bytes, SHA-256, executable archive mode,
the two selected non-`.exe` source labels and singleton File/executable
forwarding demonstrated by Bazel 9.2 `cquery`. Slug-native: immutable catalog
manifest/route identity, DICE publication and artifact representation/path
tokens; no Bazel configuration checksum or output-path byte claim.
Unsupported/deferred: `.exe` sources and Windows selection, parent
`tools/cpp` package/rules_cc closure, public external-label `cquery`, runtime
catalog source acquisition, action construction/execution, REAPI/BuildBuddy,
ActionKeys and full M7A/M8 closure. A configured provider is not proof that
the embedded executable can be transferred to a remote worker.

## Evidence and provenance

Pinned Bazel 9.2 source commit `8220c6198837d5c13d53fea211cf3282aa12408a`:
`tools/cpp/modules_tools/BUILD.tools:3-22` defines the platform selector;
`tools/cpp/modules_tools/BUILD:58-87` builds the tools and includes them in
`embedded_tools`; `src/create_embedded_tools.py` maps the built outputs into
the self-extracting release archive and preserves executable mode;
`Filegroup.java:58-61,103-119` constructs the source File and singleton
executable providers. The installed `BUILD` matches the existing Slug catalog
SHA-256 `6f0727f09faaee782e8cf2219586243f767c7c4cb6dc3b9024f8c4c3aba9dad3`.

The official Bazel 9.2.0 Linux x86-64 release checksum asset
`https://github.com/bazelbuild/bazel/releases/download/9.2.0/bazel-9.2.0-linux-x86_64.sha256`
identifies the Bazelisk-cached release executable with SHA-256
`7668a95db1250f12c40407251e4e203b4ec8bf39bc495d2f485b2d8c99048694`.
Its embedded ZIP entries are mode 0755 and match the extracted install-base
files: `aggregate-ddi`, 104888 bytes,
`f12eb5bb2b0d8634745ff88deeed1c038f903e4d6058caf8c60db1de126c1f28`;
`generate-modmap`, 105576 bytes,
`222b381da1faf69abf68f5763ad0531aa7aef110d3b16917ddb27e7f57165f70`.
They are release-generated ELF64 x86-64 assets, not present as binaries in the
pinned source checkout. Import only these exact release bytes; do not build or
invent replacements. The pinned tool-specific C++ sources carry Apache-2.0
headers and depend only on their first-party module-tool libraries plus the
dynamic C++ runtime; the repository's `LICENSE-APACHE` covers source checkout
redistribution. A future standalone Slug bundle containing these bytes must
also ship that license text; do not infer MIT licensing from Slug's dual license.

An isolated copy of the existing `rules-cc-basic` fixture, ordinary Bazel RC
discovery and `--lockfile_mode=off` returned exit 0 for Linux `cquery` on both
child filegroups. `--output=build` selected `aggregate-ddi` and
`generate-modmap`, not `.exe`; a Starlark provider projection returned exactly
one source File per `DefaultInfo.files`, the same File for
`DefaultInfo.files_to_run.executable` and `FilesToRunProvider.executable`, and
the expected `DefaultInfo`, `FileProvider`, `FilesToRunProvider`,
`InstrumentedFilesInfo`, `OutputGroupInfo` provider names. The two short paths
were `../bazel_tools/tools/cpp/modules_tools/{aggregate-ddi,generate-modmap}`.
This extends WP786's accepted unconfigured Query child-edge evidence; no new
fixture is needed. The oracle's isolated Bazel server was shut down. Normal RC
discovery may use the user's authentication; never inspect, print or copy RC
contents or credential bytes.

## Owners and design

`app/slug_bzlmod_v2/src/builtin_repository.rs` owns immutable catalog bytes,
expected digests, executable bits, directory listing and the structural
manifest SHA-256 in `BuiltinBazelToolsRouteIdentity`. Gate the two catalog rows
at compile time to Linux x86-64; other host builds must retain the current
unsupported result for both non-`.exe` and `.exe` paths. This is a
host-release-content boundary, not target-configuration selection. The
manifest SHA-256 must differ on the Linux host while remaining the prior value
on other hosts, so no version-only identity collision occurs. Reuse its existing
`BuiltinBazelToolsSourceFileKey`, validation and deterministic sorted catalog
order. Do not add a host path, fallback read, second binary store, new hash
algorithm or DICE key. The two static include-bytes assets add about 211 KiB to
the Linux binary; catalog lookup also copies each requested byte slice into an
existing DICE-owned `Arc<[u8]>`, so at most one further copy per distinct
source-key result is retained per DICE instance. Existing Arc/Dupe/Allocative
value ownership and publication remain unchanged. Hashing remains exact
SHA-256 over the catalog's actual bytes and
metadata, separate from Slug configuration/action identity and REAPI digests.

Analysis already owns source classification through the canonical observed
repository route and catalog source key (`dice/source_file.rs:210-237`), and
native filegroup selected-source/provider projection (`dice/filegroup.rs`).
`HostPackageInventoryObservationKey` and
`ConfiguredNodeAnalysisObservationKey` retain the existing dependencies,
Need/error behavior and source/package observations. A built-in source has no
filesystem acquisition path: preserve that runtime refusal. The observed
configured test must request the real canonical `@@bazel_tools` child with the
actual built-in route, not a local override or authored provider envelope.
Use a Linux platform constraint fixture through existing test helpers and
assert selected label, File short path, `DefaultInfo.files`, executable,
files-to-run and zero actions for both children. An explicit Windows target
platform must select `.exe` and fail closed while that source remains absent;
this negative has only Slug-native diagnostic classification. Assert the
unselected `.exe` paths remain unsupported in the Linux catalog. On other host
builds both binary names remain absent from the catalog despite the Bazel
source-input filegroup glob seeing their checked-in paths. Include a separate
catalog SHA/mode/listing/host-conditional manifest identity regression and an observed-route negative
that fails closed for unsupported source or platform state; do not weaken an
existing catalog assertion without replacing it with the precise new boundary.

The Stage 9 compact-utility row is reuse-only: no retained representation,
interner, collection, clone-cost or hashing algorithm changes. The existing
catalog source file is under the 2,000-line production review trigger and
adds only two entries, not a dispatcher.

## Scope, stops and validation

Base `438135765` on clean main. Root owns source import, serial Cargo,
integration, documentation and commit. Production allowlist:
`app/slug_bzlmod_v2/src/builtin_repository.rs` and the two verbatim binary
assets under `app/slug_bzlmod_v2/builtin/bazel_tools/tools/cpp/modules_tools/`.
Test allowlist: `app/slug_bzlmod_v2/tests/builtin_bazel_tools.rs`, the existing
manifest-digest assertion in `app/slug_bzlmod_v2/src/host_module.rs`'s test
module, and a focused existing `app/slug_analysis_v2/tests/starlark_rule.rs`
submodule/test. The root may add a provenance attribution to repository
`NOTICE`; no catalog-local notice or unverified upstream content. No
Loading/Analysis production, Core/CLI, transport, root MODULE/lockfile or
oracle expected-byte edits. Independent design and final review gate the
release-content/public catalog admission.

Stop and REPLAN if the release executable cannot be independently authenticated,
the embedded entries differ from the installed bytes or declared modes, direct
child configured analysis requires parent `rules_cc` or a new semantic owner,
or Linux selection cannot be proved without widening public cquery or runtime
source acquisition. Diagnose ordinary fixture/invocation or implementation
failures within this packet first.

Resolve the pinned nightly before Rust validation. Compile offline/locked with
Cargo JSON `--no-run`, preflight exact nonignored selectors, then execute focused
Bzlmod catalog tests, the observed configured child test and one direct
Analysis dependent. Run `cargo fmt --all -- --check`, `git diff --check`,
`scripts/v2_archive_status.sh`, plan status and independent final review.
No NativeLink or BuildBuddy service is needed for this analysis-only packet.

## Immediate predecessor

WP786 at `438135765` independently accepted the authentic unconfigured
`@bazel_tools` cross-package module-tool filegroup Query edges and selected
traversal. It explicitly left configured providers and missing binaries open.
Independent design review ACCEPTED the host-guarded, release-backed packet;
independent final review ACCEPTED the implementation.

## Acceptance receipt

The two checked-in 0755 assets match the authenticated Bazel 9.2 release ZIP
entries byte-for-byte. The Linux catalog adds exactly their SHA-256-validated
rows, while non-Linux hosts omit them and retain their prior manifest digest.
The Linux manifest digest is
`77d037eb62f01ada4046310894bd00bd784480a0c8bebbcfeb46127e469711a7`.
The authentic observed configured test passed for both filegroups: selected
source File/short path, singleton executable and files-to-run, zero actions,
Windows `.exe` refusal, and Linux restoration on the same DICE instance.

Pinned-nightly offline/locked Cargo JSON `--no-run` compiled the Bzlmod library
and catalog integration test plus the Analysis `starlark_rule` test. Exact
nonignored preflight and execution passed for four focused Bzlmod library
tests, all four catalog integration tests, and one authentic Analysis test.
The first Analysis invocation failed because the scratch fixture omitted the
built-in MODULE's local dependency overrides; adding the existing host-platform
override pattern corrected that fixture, and the recompiled selector passed.
`cargo fmt --all -- --check`, `git diff --check`, archive status and plan status
passed. Review-requested cross-host metadata filtering was corrected and its
four-test integration executable reran 4/4; the strengthened Analysis test
reran 1/1. Packet elapsed about 15 minutes, with serial compilation and
subsecond focused executions; no architectural replan or remote service use.
