# Current Slug V2 Work Packet

Packet: WP-7-97-m7a-selected-module-extension-host-factors-r1
Status: ACCEPTED 2026-09-24; M7A remains partial

## Outcome and compatibility

Admit declared OS and architecture evaluation factors for DICE-owned selected
module-extension owners. The source-authentic rules_rust 0.73
`rust_host_tools` extension must advance through its selected owner and the
accepted WP796 archive materializer; the generated BUILD
`rust_toolchain(iso_date=None)` Loading stop remains a separate next packet.
This advances the authentic M7A source closure, not root-build readiness.

Exact Bazel 9.2 behavior claimed here: declaration flags independently select
OS and architecture factor participation; unchanged extension ID retains the
same canonical generated-repository name; on this Linux x86_64 host, the pinned
rules_rust extension selects the host-tools repository rule and source-derived
Rust 1.97.1/nightly rustfmt attributes. Slug-native: Rust Host observations
and structural factor strings/identity, rather than Bazel's raw JVM
`os.arch`/HotSpot bytes or exact hidden-lockfile key spelling. Unsupported/
deferred: global pure-extension aggregation with factors, `facts_version`
or nonempty facts, exact visible/hidden lockfile factor replay and error-mode
reuse, cross-host Bazel factor-byte parity, unaudited generated-repository
effects, and any unrelated extension method or root-build claim. These
boundaries must fail closed; no Bazel/JVM semantic execution enters Slug.

## Source, owner and decision

Pinned Bazel 9.2 `RegularRunnableExtension.java:201-205` derives only the
declared factors from host OS and raw architecture.
`ModuleExtensionEvalFactors.java:32-97` keeps conditional components and
renders OS before architecture. `ModuleExtensionId.java:29-43`,
`BazelDepGraphFunction.java:170-215`, `SingleExtensionValue.java:68-74`,
and `SingleExtensionEvalFunction.java:139-303` separate extension ID,
generated repo name, factor-keyed evaluation and lockfile selection.
`BazelLockFileModule.java:157-310` puts reproducible entries in Bazel's
hidden output-base lockfile and retains/replaces other factor entries by
dependency shape. Pinned rules_rust
`rust/extensions.bzl:327-366` declares both flags, computes its host
triple through `module_ctx.os`, calls `rust_toolchain_tools_repository`,
and returns reproducible metadata; its MODULE self-use supplies the tag.
Accepted WP796 proved the six resulting Linux x86_64 archive effects.

The selected `HostSelectedExtensionOwnerPureObservationKey` is the semantic
producer. Its existing owner inputs, authenticated first/reacquired .bzl
projection, observed `RepositoryPlatformKey`, environment/download/path
frontiers, and invocation calls remain its dependencies. Add one compact
structural factor projection: OS only when `os_dependent`, architecture only
when `arch_dependent`; retain declaration shape and selected host values in
the pure result, post-host pure errors, and post-host outer observation errors
so DICE equality cannot cut off a factor change with identical repository
calls or identical error text.
The owner certificate inherits that value. Keep the factor distinct from
`module_ctx.os`'s displayed host values, toolchain triple, repository
mapping, and canonical generated name. No command-side source inference,
process-global cache, new DICE key, or lock held across a DICE computation.

The selected owner may admit the four flag combinations for existing
supported `RepositoryPlatform` values. The legacy aggregate
`HostPureModuleExtensionInvocationsKey` keeps its current factor rejection.
Existing host observation and authenticated re-acquisition precede evaluation;
missing or changed injected host input remains Need/typed terminal, not an
ambient OS read. Changed declaration, source, host platform, tags, environment,
mapping or owner must recompute or fail closed under existing
observation/promotion rules. A/B/A restoration must recover the original
factor and generated spec; output equality may cut off only when every
declared factor-relevant structural input matches. Bazel's
reproducibility/lockfile lifetime is not
silently simulated.

The retained factor is at most two compact strings plus declaration presence
in the DICE-owned result/error; no retained archive, evaluator value, temp root
or extra service cache. The Starlark invocation remains phase scratch, released
on Need/error/cancellation. The existing immutable materialization and
selected-download owners keep their publication and cleanup rules. The
large `module_extension.rs` file remains the cohesive selected-owner
implementation and colocated test owner; a short factor type/helper may live
there, but do not add a second broad extension driver.

## Evidence and validation

Reuse pinned Bazel sources and WP796's authentic
`@@rules_rust++rust_host_tools+rust_host_tools` mapping/RepoSpec evidence.
Bazel 9.2 `mod show_extension`, `dump_repo_mapping` and `show_repo`
on the genuine rules_rust 0.73 module pass without fetching archives.
`--host_cpu=aarch64` does not alter extension Host factors, so it is not
a cross-host oracle. Add a small Stage 1 self-contained nonreproducible
four-export fixture only for the demonstrated conditional-factor gap: both,
OS-only, architecture-only and general keys, stable generated names, and a
fresh-output-base missing-factor error-mode negative. Record provenance and
use message-shape/structured-semantic comparison for host-dependent text.
Do not claim a multi-OS oracle from this one host. The authentic rules_rust
lane is source-pinned and checked by the direct smoke; no copied upstream
archive or credential material belongs in the fixture.

Focused Rust tests must cover four factor selections, declaration change,
platform OS-only/arch-only/both A/B/A with identical repository calls,
post-host identical-error factor discrimination in both pure and outer
observation terminals, owner certificate/repository
mapping stability, and preserved arbitrary-owner `facts_version`/legacy
aggregate rejection. Include observed and legacy selected views, Need/error
and event nonpublication at the existing boundaries. Build with the pinned
Rust toolchain: JSON `--no-run`, exact-selector preflight, serial Cargo,
direct Loading/Bzlmod/Core compile dependents, and a fresh
`cargo build -p slug_cli_v2` before any `SLUG_V2_BIN` or CLI smoke. Clean
stale `slugd` before/after. The original `rust_host_tools` extension smoke
must reach the accepted archive behavior; a separately typed earlier source
prerequisite is diagnostic evidence for scope correction or REPLAN, not an
acceptance substitute. Do not pass an unsupported factor by fiat.
Run format, diff, archive and plan status checks; obtain independent DESIGN
review before Rust edits and FINAL review of the actual diff. No broad suite
or M7A close is selected.

## Scope and stops

Base clean main `1ecf53a8b`. Allowlist: this manifest, canonical Current
paragraph and Stage 5 owner contract only for accepted changes;
`app/slug_loading_v2/src/module_extension.rs` with colocated tests;
`app/slug_loading_v2/src/registration_diagnostic.rs` only to unwrap the
new structural selected-error carrier without changing displayed diagnostics;
`tests/v2_oracle/fixtures/module-extension-host-factors/` for the small
oracle fixture and its generated expectation. No Bzlmod lockfile parser or
writer, Core Host observer, global extension driver, repository materializer,
NativeLink/BuildBuddy configuration, credential file, target-pattern support,
generated BUILD attribute schema, or generic source acquisition change.
If factor retention requires altering a different DICE/public owner, source
proves a different factor domain, ordinary selected evaluation needs a
lockfile lifecycle to be correct, or authentic rules_rust requires an
unmodeled method before archive effects, stop for reviewed scope correction
or REPLAN. Do not weaken the prior arbitrary-factor rejection test without
its positive factor/identity replacements. Normal Bazel RC discovery may
pick up user auth when explicitly needed; never inspect or print
`~/.bazelrc` or netrc contents.

## Acceptance receipt

Independent DESIGN and FINAL reviews returned ACCEPT. A reviewed compile
correction added only the selected-error display unwrapping in
`registration_diagnostic.rs`; the structural factor remains in DICE equality.
Bazel 9.2's self-contained nonreproducible oracle passed generation and
independent replay in separate fresh roots: four visible factor shapes,
stable generated names, and a same-extension-ID missing-factor error after
an isolated declaration change and output-base expunge. The observed JVM
architecture factor was `amd64`; Slug's Rust Host projection was `x86_64`,
so no raw-factor-byte parity is claimed. Bazel's authentic offline
`//:probe` build passed for the dependency-owned rules_rust extension.

Pinned-toolchain JSON `--no-run` passed for Loading, Bzlmod and Core. Exact
selector preflight preceded 3/3 new tests, the existing global-factor guard,
and the existing selected Host-input test; all five passed. A fresh
`cargo build -p slug_cli_v2` passed. The original rules_rust `rust_host_tools`
extension smoke, in a new workspace with HOME/NETRC unset, passed the former
factor stop and WP796 materialization, then stopped at generated
`BUILD.bazel:131` because `rust_toolchain(iso_date=None)` is rejected by
Loading. No `slugd` process remained before or after. Format, diff, V1
archive and plan-status checks passed. No broad suite or root-build success
is claimed.

Two older `registration_diagnostic` tests were run and failed because their
unchanged owner fixture omits `RepositoryHostInputTransaction` and
`RepositoryPlatformKey`; the pre-WP797 selected owner already required those
inputs. Those broad diagnostic fixtures remain an independent validation gap,
not passing evidence for this packet. They were not silently repaired or
counted as passing tests.

## Immediate predecessor

WP796 at `1ecf53a8b` accepted the six source-selected host-tools tar.xz
effects and bounded materialization. An innate `use_repo_rule` route reaches
the generated BUILD, then stops at `iso_date=None` Loading; the original
module-extension route still stops at declared OS/architecture factors.
