# Current Slug V2 Packet

Packet: `WP-5-m1-external-bzl-module-owner`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: implementation worker
Evidence: accepted Host repository source identity `980373f9`, accepted
external query package identity `845e89b7`, pinned Bazel 9.2 external
same-package load/missing/cycle evidence, and the independently accepted
dormant owner design in the Stage 5 owner plan. The 17-row, 598-line fixture is
frozen.

Implement only the private dormant external Bzl-module owner and its isolated
cycle family. Read `AGENTS.md`, the orchestration skill implementation-worker
reference, `docs/developers/dice.md`, the Buck utility skill, and the accepted
owner appendix before editing. Inspect the live checkout and dirty diff first.

The exact changed-file allowlist and addition caps are:

- `app/slug_loading_v2/src/bzl_module.rs`: at most +950, including inline
  private tests and the stale `BzlModuleIdentity` documentation correction;
- `app/slug_loading_v2/src/cycle_detector.rs`: at most +300, including inline
  private cycle tests; and
- `app/slug_loading_v2/src/host_package_load_tests.rs`: at most +800 for the
  direct same-module evaluator/cycle/lifecycle seam.

The total cap is +2050 additions with no other diff. Do not edit Cargo,
bzlmod, query, CLI/server, fixtures/oracles, protocol, schedules, or any other
test/production file.

Add private `RepositoryBzlLabel { package: PackagePath, target:
RootPackageBzlTarget }` and `ExternalBzlModuleEvalKey { route:
RootRepositoryRoute, label: RepositoryBzlLabel }`. The canonical label is
derived only from `route.canonical_repo()` plus the typed package/target; never
accept a separate canonical repository. Route remains whole key identity,
including its apparent/module/spec source-routing state. Manifest/value
identity is canonical and contains no apparent rendering.

Use a separate resolver. Parse and validate every direct AST load before
computing any child. Accept only `:target.bzl` and exact same-package
`//package:target.bzl` (including `//:target.bzl` for the root package).
Reject malformed values, every `@`/`@@` spelling, every different package,
and target raw bytes containing `/` before child source lookup. Construct the
repository-relative source path from the validated package and target raw
bytes: Unix uses `OsStringExt::from_vec`; non-Unix maps each Latin-1 byte to
its corresponding Unicode scalar. No lossy round trip, mapping/discovery,
subpackage traversal, or root fallback.

The key reads only `HostRepositorySourceFileKey`. Its present bytes and exact
requested logical path form parsing input and `BzlModuleIdentity`. Add private
`ExternalBzlModuleError` with exact typed, equality-bearing source-compute,
source, absent-label, encoding, parse, normalized-load, child, external-cycle,
evaluation, and freeze variants. Retain no operational physical path,
evaluator/frozen pointer, or catch-all error string in semantic equality.

Normalize all children, compute them sequentially under the new typed guard,
reuse `LocalBzlLoader`, `BzlLoadManifest::new`, and the existing flattened
frozen lifetime closure, evaluate, freeze, and capture exactly this key's local
complete event batch. The value is
`SourcePreparationOutcome<Arc<Result<FrozenBzlModule,
ExternalBzlModuleError>>>`; equality is `complete_eq`, validity complete-only,
and Need invalid/unequal. Cached reuse does not recapture; retained rich
activation metadata remains available and command selection still ignores
`Reused`.

Add an isolated third external node/cycle/guard family to the existing
request-scoped detector. Record only external-key edges and never mix legacy
or root Host families. Use the existing poison key to release recursive waits.
The only allowed lock is the per-guard async receiver mutex held while one
sequential child future is polled; prove the guard cannot re-enter. Recovery
uses the same DICE in a new transaction with a fresh detector; never reuse a
detector after retained `CycleDetected` state.

Keep `RepositoryPackageLoadKey::LoadsUnsupported` byte-for-byte unchanged in
behavior and do not add a production caller for the private key. This packet
must not evaluate external BUILD loads, expose macro-produced native targets,
populate external package manifests, change query graph acceptance, or
activate Bzl/fake companion output. BUILD integration and query projection are
a separate reviewed packet.

Focused tests must cover byte-level relative/absolute normalization, all
pre-source rejections, canonical manifest labels and logical paths, direct
dedup/diamond order, retained frozen lifetime, typed success/error equality,
Need invalidity, missing/parse/evaluation/freeze/child failures, private
self/ordered cycle release and fresh-detector recovery, evaluated versus reused
activation, unchanged retained metadata, empty replacement, and no recapture.
Do not claim BUILD origin, package lifecycle, aggregate print order, or command
output.

Run serially:

- `cargo test -p slug_loading_v2 host_package_load`
- `cargo test -p slug_loading_v2 external_bzl_module_`
- `cargo test -p slug_loading_v2`
- `cargo test -p slug_query_v2 --test loading_query external_owner_dispatches_siblings_rdeps_and_loading_files_without_root_fallback`
- `cargo check -p slug_loading_v2`
- `cargo check -p slug_query_v2`
- `cargo test -p slug_loading_v2 --target x86_64-pc-windows-gnu --no-run`
- `cargo test -p slug_query_v2 --target x86_64-pc-windows-gnu --no-run`
- `cargo fmt --all -- --check`
- `scripts/v2_archive_status.sh`
- `git diff --check`

Obtain one independent latest-diff DICE/lifetime/cycle review before
acceptance. Stop with **REPLAN** rather than widening scope if implementation
needs a third production file, public identity/API, root-key/cycle reuse,
another source/observation owner, direct filesystem access, another lock,
package/query activation, cross-package/repository loads, mapping/discovery,
non-local overrides, globs, visibility content, test/executable rules, suites,
implicit/user dependencies, generated outputs, `@bazel_tools`, configuration,
analysis/actions/execution, repository rules/extensions, JVM, Java bytecode,
or Bazel delegation.
