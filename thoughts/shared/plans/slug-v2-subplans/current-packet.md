# Current Slug V2 Work Packet

Packet: WP-7-89-m7a-selected-extension-declared-host-environment-r1
Status: accepted; independent design and final ACCEPT

## Outcome and compatibility

Admit the real rules_rust 0.73.0 `crate` module extension's declared
`environ` factor in the selected-owner path. Before invoking its implementation,
observe every declared environment name (including absence) and Rust Host
platform through existing repository Host DICE cells; supply a bounded
`module_ctx.os` view for `name`, `arch`, and declared-name `environ` membership
and `.get`. The authentic root cquery must pass the factor guard, consume these
tracked facts and report its next honest terminal. It need not complete the
extension or configured root graph.

Exact: Bazel 9.2 declared-name environment change/restore re-evaluation,
absent versus present-empty distinction, and selected declared-name
`module_ctx.os.environ` membership/get results. Slug-native: Rust Host
`module_ctx.os` name/arch values and structural DICE invalidation; an undeclared
`os.environ` lookup fails closed rather than reproducing Bazel's visible but
untracked whole-process dictionary. Unsupported/deferred: other environment
map operations, dynamic `module_ctx.getenv`, declarations with
`os_dependent=True`, `arch_dependent=True` or nonzero `facts_version`,
repository path/watch/download/execute effects, lockfile facts lifecycle,
configured root actions and full M7A/M8 closure. Do not infer support for all
module extensions from this selected-owner admission.

## Evidence and semantic owner

WP788 at `66d29e2fe` rebuilt Slug and moved the real
`//app/slug_cli_v2:slug` cquery from the rules_rust `manifests` declaration
failure to `selected module extension has unsupported factors`. The root
`MODULE.bazel` uses `@rules_rust//crate_universe:extensions.bzl%crate`.
The BCR-resolved rules_rust 0.73.0 `extensions.bzl:1657-1684` declares only
`environ=CRATES_REPOSITORY_ENVIRON`; `private/generate_utils.bzl:16-26`
assembles the ordered eight names `CARGO_BAZEL_GENERATOR_URL`,
`CARGO_BAZEL_GENERATOR_SHA256`, `CARGO_BAZEL_REPIN`, `REPIN`,
`CARGO_BAZEL_REPIN_ONLY`, `CARGO_BAZEL_ISOLATED`, `CARGO_BAZEL_DEBUG`, and
`CARGO_BAZEL_TIMEOUT`. Its `_crate_impl` calls `get_host_triple(module_ctx)`
and `_get_generator`; the latter tests membership and `.get` for the two
declared generator names, and reads `module_ctx.os.name`. The host-triple
helper reads OS and architecture. No ambient environment read may be added.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`:
`RegularRunnableExtension.java:182-205` observes declared names and OS/arch
factors; `ModuleExtensionContext.java:91-94` records static environment
inputs; `StarlarkBaseExternalContext.java:194,1513-1543` supplies `os` and
dynamically records `getenv`; `StarlarkOS.java:31-79` exposes the complete
environment while warning that lookup alone creates no dependency. An
isolated Bazel 9.2 same-server oracle at `/tmp/slug-extension-factors.jzJPBe`
proved declared `--repo_env=SLUG_ORACLE_FACTOR` A→B→A re-evaluations and
proof bytes, absent versus present-empty, and no re-evaluation for an
unrelated changed name. That unrelated name was nevertheless visible through
`os.environ`; `getenv` dynamically tracked its name in a separate probe.
The isolated server was shut down. Slug must explicitly classify and reject
the untracked Bazel behavior rather than silently omit a visible input.

`slug_loading_v2::module_extension::HostSelectedExtensionOwnerPureKey` owns the
selected invocation and its DICE dependencies. Reuse the already injected
`RepositoryEnvironmentCellKey` per declared name,
`RepositoryPlatformKey`, `RepositoryHostInputTransaction`, and
`SourcePreparationNeeds::environment` frontier protocol; Core already
captures the command repository environment once and retries Needs. Check
frontier authorization before cell reads, then verify each observed cell
against the immutable request snapshot. Never read `std::env` or bypass the
cell. A missing authorization returns Need without invocation or publication;
a mismatched/unauthorized cell is an error without leaking its value. Stable
declared cells and platform become real DICE dependencies; unrelated names do
not. Declaration order/duplicates remain in the frozen projection identity;
deduplicate only the observation frontier. Preserve first/second Bzl manifest
and projection drift checks, event ownership, cancellation and error/Need
behavior. Existing `RepositoryEnvironmentCell` equality distinguishes absent,
empty and nonempty, so A→B→A restores the prior semantic result.

The Starlark `module_ctx.os` value is invocation-heap scratch, not a new
service or DICE cache. It owns only the verified platform and declared
name/value projection. Its `environ` view supports membership and `.get` for
declared names, returning false/`None` for declared-absent and true/empty
string for declared-present-empty; any undeclared name and unsupported map
operation must error, never appear absent by default. It must not expose the
full untracked transaction snapshot or print secret values in diagnostics.
Existing per-name DICE cells retain the values; only a bounded `Arc` projection
is held through this one invocation and released on completion/cancellation.
No new DICE key, global state, lock or hashing algorithm is authorized.

## Scope, stops and validation

Base clean `main` at `66d29e2fe`. Root owns integration, serial Cargo,
fresh CLI rebuild/smoke, reviews and commit. Production and test allowlist:
`app/slug_loading_v2/src/module_extension.rs` and its existing unit tests.
Scheduling allowlist: this manifest and the canonical Current packet paragraph.
No Core/CLI/Bzlmod production, repository-rule-context, root MODULE/lockfile,
fixture, REAPI, or transport edits. `module_extension.rs` is an existing
roughly 3.5k-line cohesive selected/legacy invocation owner; keep the new Host
observation and context value localized rather than adding a dispatcher or
duplicating repository-rule effect plumbing. Existing environment cell/Arc
representations and clone behavior are reused, not changed.

Obtain independent design review before Rust implementation and independent
final review of the diff. At least one focused dynamic test must compute
`HostSelectedExtensionOwnerPureObservationKey` over a real selected external
`.bzl` route, not merely assert source shape. Focused tests must prove
declared-present A→B→A,
absent→empty→absent, unrelated-variable non-invalidation, declaration-order
identity, platform change/restoration, Need with zero selected-owner invocation
events (earlier route/Bzl observations may already exist),
cell/request mismatch error with redacted diagnostics, declared membership/get,
and undeclared lookup rejection. Preserve rejection of OS/arch/facts factor
declarations. Reuse existing DICE test helpers and the isolated Bazel oracle;
no new repository fixture unless a demonstrated gap requires one. The fresh
root cquery must pass the factor guard and record the next terminal, not claim
root success. Stop and REPLAN if the selected source requires an undeclared
environment lookup, the Host cells/platform cannot own invalidation, the
Starlark proxy cannot reject unsupported access, or the implementation needs
a broader public/identity/lockfile owner. Diagnose ordinary test/compiler
failures within the accepted contract.

Resolve the pinned nightly, compile Loading tests offline/locked with Cargo
JSON `--no-run`, preflight exact nonignored selectors, run focused owner tests
plus one direct dependent. Rebuild `slug_cli_v2` before smoke and clean stale
`slugd` before/after. Run `cargo fmt --all -- --check`, `git diff --check`,
`scripts/v2_archive_status.sh`, `python3 scripts/v2_plan_status.py`, and
independent final review. A root smoke may use ordinary request environment;
never inspect or copy `~/.bazelrc` authentication. NativeLink/BuildBuddy are
not needed for this Loading-only packet.

## Immediate predecessor

WP788 at `66d29e2fe` admitted `tag_class` `allow_files` descriptors and
advanced the authentic root to this declared-environment factor stop.

## Validation receipt

Independent design and final reviews ACCEPTED the narrowed declared-name
proxy and Slug-native OS/architecture classification. The selected external-registry
owner test passed (1/1) through cold Need, declared A→B→A,
absent→empty→absent, unrelated-name reuse, platform change/restoration,
declaration-order identity and cell/request mismatch/unauthorized redaction.
A separate selected external-route test passed (1/1) for rejection of
`os_dependent=True`, `arch_dependent=True` and `facts_version=1` before
invocation. The direct `module_ctx.os` proxy test and one existing
Host-dependent test passed (1/1 each). Pinned-nightly offline/locked Loading
JSON no-run compile, exact-selector preflight for all four focused tests,
`cargo fmt --all -- --check`,
`git diff --check`, archive status and plan status passed. A fresh
`cargo build -p slug_cli_v2 --offline --locked -q` passed. The fresh
`target/debug/slug cquery '//app/slug_cli_v2:slug' --noimplicit_deps
--lockfile_mode=off --output=label` passed the old factor guard and stopped
at rules_rust 0.73.0 `extensions.bzl:851`: `module_ctx` has no `path`
attribute. No `slugd` remained before or after the smoke. This is a new
deferred repository-path effect, not configured-root success.
