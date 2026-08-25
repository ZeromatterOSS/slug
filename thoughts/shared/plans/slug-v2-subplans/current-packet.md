# Current Slug V2 Packet

Packet: `WP-4-5-6-7A-selected-registry-extension-bzl-source-owner-implementation-r3`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: `04-starlark-loading-and-build-packages.md`,
`05-bzlmod-and-repository-graph.md`, and
`06-analysis-toolchains-and-actions.md`
Base: one-constructor authority correction after restored r2 preflight

Result: implement the first selected-registry module-extension definition
source owner and producer-view recursive loads without widening
repository-rule declarations or execution.

## Corrected accepted design

The r2 implementation preflight exposed one authority omission and then
restored every partial edit to the exact frozen hashes below.
`HostSelectedExtensionOwnerInputs` directly constructs and retains an
individual `HostSelectedExtensionDefinitionLoadRequest` in
`selected_extension_demand.rs` for pure reacquisition. Because the selected
source association is request-owned, that constructor must initialize the same
opaque association. Reconstructing it later from a container or global lookup
would split producer visibility from request identity and lifetime. No second
sibling or constructor is authorized.

The exact 33-package developer graph first leaves the accepted M7A surface at
root `use_extension("@rules_rust//rust:extensions.bzl", "rust")`.
`HostSelectedExtensionDefinitionLoadRequests` rejects that nonroot canonical
label before loading. The root-only loaded-definition and pure-invocation
consumers would then construct a root label, while the existing external-Bzl
loader has no selected-registry source association for same-repository
cross-package or mapped-repository loads.

Bzlmod is the semantic producer. Each admitted external request retains an
opaque `HostCanonicalSelectedModuleDefinition` association from already-owned
selected routes. The association owns canonical repository, selected
`RepoSpec`, local-path policy, self name and ordered repository mapping.
Loading owns one selected-source Bzl evaluator that consumes this typed fact,
uses the existing repository source owner for immutable bytes, evaluates
recursive loads and publishes the frozen module/manifest.

The accepted Bazel-only fixture
`selected-registry-extension-source-owner` is the exact behavioral anchor. A
root selects only `owner@1.0`; that selected owner loads one self cross-package
child and one child visible only through its own mapped `mapped_dep@1.0` view,
prints `SELECTED_REGISTRY_MARKER:local:mapped`, and cleanly exports a no-op
extension. The root has neither child view. Pinned Bazel 9.2 generation and two
fresh-root replays all pass. This proves the source/view law without evaluating
a repository rule, tag/schema, generated repository, toolchain or action.

Actual rules_rust remains a downstream negative boundary: it evaluates
unadmitted `repository_rule(doc = ...)` and later collection schemas before
export inspection. Neither this packet nor its proof may require successful
rules_rust export, pure reacquisition, or public command completion.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` supplies architecture guidance
only. Its natural layering keeps selected identity and repository views with
Bzlmod, physical realization with the repository owner, and ordinary `.bzl`
consumption with loading. Its typed source-fact pattern forbids path inference,
mutable visibility repair and consumer publication of an input bridge. Copy no
Zig code, representation, scheduler, digest, path, cache or root policy.

## Required implementation

Preserve root definition loading byte-for-byte. For only a root-owned,
non-isolated extension request whose definition repository resolves to one
selected registry module:

1. Validate a unique canonical selected-definition association and retain it
   opaquely with every request constructor, including owner-input retention for
   pure reacquisition. Unsupported owner/isolation, selected
   nonregistry/generated sources, missing or duplicate routes, invalid mapping
   and unsupported builtin definitions fail closed.
2. Project one structural selected-source route from the exact `RepoSpec`,
   local-path policy, canonical identity and self apparent name. This is a
   distinct polarity, never direct-local, generated or builtin in disguise.
3. Dispatch the definition label to one selected-source Bzl owner. Relative
   and `//pkg:x.bzl` loads preserve the current selected definition;
   `@apparent//pkg:x.bzl` resolves through its ordered mapping to another
   retained selected definition or the existing builtin route. Every mapped
   edge switches to the child producer's view. Never consult physical paths, a
   command registry or mutable repository table.
4. Loaded-definition projection and pure-invocation reacquisition share this
   owner and authenticate the same transitive manifest and module-extension
   projection. They retain no evaluator heap or callable beyond the existing
   frozen-module lifetime.

The implementation may factor root/selected dispatch locally, but adds no
second semantic producer, global cache/interner, lock, task, fallback scan or
command-side route repair.

## DICE, observation and event contract

Order is request association, definition source, recursive loads in declared
order, then named-export inspection. Observed mode merges the request epoch
left-first with source/load epochs before inspecting each child terminal. Stop
on the first `Need`, typed outer or semantic error; do not full-scan or union
Needs. Legacy is carrierless. Equality and validity are complete-only.

The association and selected routes are retained structural inputs. Frozen
modules, transitive manifests, projections, Result Arcs and the observed epoch
remain at existing owners; parse/evaluator/resolution scratch is compute-local.
Only the Bzl child owns evaluation events. Parents do not replay them, warm
reuse is silent, and cancellation publishes no partial state. A/B/A changes to
`RepoSpec`, mapping, source bytes or recursive loads restore exact A state and
observations without stale B state.

Semantic identity includes workspace, request owner/isolation, canonical
definition label/export, selected module/`RepoSpec`/source policy, ordered
repository view, package/target, source bytes and transitive manifest.
Apparent/display spelling and physical paths remain projections. Bazel
checksum/ActionKey and REAPI/CAS digests are untouched.

## Compatibility

- **Exact:** Bazel 9.2 root-owned, non-isolated selected-registry definition
  admission and producer-view self/mapped recursive source loads through clean
  module-extension export.
- **Slug-native:** typed Result/epoch carriers, Rust Starlark heap ownership,
  DICE key shape, event batches and immutable selected-source route shape.
- **Unsupported/deferred:** nonroot/isolated requests; selected nonregistry or
  generated definition sources; broader builtin content; repository-rule
  declaration/schema/invocation; actual rules_rust completion; repository
  effects, toolchains/providers/actions/input trees, crate_universe, M8/M7B and
  exact configuration/output bytes.

## Authority, budgets and proof

Read `docs/developers/dice.md` and the repo-local Buck2 utility-reuse skill
before editing. Seven-file Rust authority is exactly:

- `app/slug_bzlmod_v2/src/host_module.rs`
- `app/slug_bzlmod_v2/src/host_package.rs`
- `app/slug_bzlmod_v2/src/selected_repo_spec.rs`
- `app/slug_bzlmod_v2/src/selected_repo_spec/selected_extension_demand.rs`
- `app/slug_bzlmod_v2/src/lib.rs`
- `app/slug_loading_v2/src/bzl_module.rs`
- `app/slug_loading_v2/src/module_extension.rs`

Everything else, including fixtures/oracles, Cargo/BUILD and generated or
vendored content, is read-only. Caps remain <=850 production, <=1,050
colocated proof and <=1,900 aggregate additions; no helper/test exceeds 100
lines. Exact accepted dirty entry state is:

| Path | Lines | SHA-256 | Ceiling |
|---|---:|---|---:|
| `host_module.rs` | 4,872 | `56a7ffe34f8f26c3e70b02deed12268198599060cc455127f2edd3bddab22506` | 5,050 |
| `host_package.rs` | 5,009 | `1921abc6f0fedc0f7c0d14504168980f1063deec82bcfff9b64c2c3c6b8cc5b8` | 5,180 |
| `selected_repo_spec.rs` | 13,415 | `ad8c89c9d4613db408ae7429e51e3a59ae2e865a90f0daaf896dc2fe9624c333` | 13,900 |
| `selected_extension_demand.rs` | 1,128 | `ad47ae4e308d95ed3d55100cdd77caf2a0e067a10fb5322fb9f49338f4a0f508` | 1,210 |
| `slug_bzlmod_v2/src/lib.rs` | 469 | `0d33a0bacb5d8f6725cca664e9398abf8cecdf4274f18aa51c49d9fffe0641ee` | 525 |
| `bzl_module.rs` | 9,120 | `20737363c9048fa5b5f81e6b8d4cdeb139e413ae0f053abc0cdaa1cc85cb9a58` | 9,900 |
| `module_extension.rs` | 2,430 | `9d82768900f3459dda85cf7599a4b7f518717d986d9f570bcbbec634eef1a4c9` | 2,800 |

Treat all other dirty generated-repository work as accepted state; do not
format, revert or charge it. Preserve Buck2-derived `CompactString`,
`SmallMap`, immutable `Arc` slices, `Dupe` and `Allocative` conventions. Add no
`BTreeMap`, global interner or retained evaluator.

Proof covers root nonregression; selected association; self cross-package and
mapped selected recursive loads matching the accepted owner/mapped fixture;
builtin route selection without invented content; unsupported mappings; exact
epoch/event order; Need/outer stop; warm silence; structural inequality and
A/B/A for source/mapping/bytes/manifest; cancellation/recovery; and
loaded-definition/pure drift authentication. The actual rules_rust declaration
terminal must remain unchanged. Add no fixture or oracle.

Run formatting, focused tests, full `slug_bzlmod_v2` and `slug_loading_v2`, the
dependent `slug_core_v2` check, source-shape/absence checks,
`git diff --check`, exact isolated accounting and independent terminal review.
Rebuild `slug_cli_v2` only if an authorized change affects that path.

STOP repository-rule declaration/schema/invocation changes, new
`repository_ctx` APIs, `@bazel_tools` content, public command activation,
fixture/oracle edits, Java/JVM, unrelated cleanup, milestone closure, M8/M7B
and exact identity bytes. Do not add a second sibling/constructor or reconstruct
the request association through a container/global lookup. `REPLAN` before
widening files, caps, compatibility or admitting a second owner.
