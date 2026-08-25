# Current Slug V2 Packet

Packet: `WP-4-5-6-7A-selected-registry-extension-bzl-source-owner-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: `04-starlark-loading-and-build-packages.md`,
`05-bzlmod-and-repository-graph.md`, and
`06-analysis-toolchains-and-actions.md`
Base: accepted frontier design at this manifest's parent commit

Result: admit the first selected-registry module-extension definition source
and its repository-view loads without widening repository-rule execution.

## Accepted design boundary

The exact 33-package developer graph first leaves the accepted M7A surface at
root `use_extension("@rules_rust//rust:extensions.bzl", "rust")`.
`HostSelectedExtensionDefinitionLoadRequests` rejects that nonroot canonical
label before loading. The root-only loaded-definition and pure-invocation
consumers would then incorrectly construct `HostRootBzlLabel`, while the
existing external-Bzl loader has no selected-registry source association and
rejects both cross-package `//rust:defs.bzl` and mapped
`@bazel_features//:features.bzl` loads.

Bzlmod is the natural semantic producer. Each admitted external request must
retain an opaque `HostCanonicalSelectedModuleDefinition` association from the
already-retained selected routes. That association owns canonical repository,
selected `RepoSpec`, local-path policy, self name and ordered repository
mapping. Loading owns one selected-source Bzl evaluator that consumes this
association, materializes/reads bytes through the existing repository source
owner, evaluates recursive loads and publishes the frozen module/manifest.

Pinned Bazel 9.2 `RegularRunnableExtension.load` supplies exact behavior: it
loads the extension's canonical label with the Bzlmod load key before export
inspection. The accepted `rules-rust-073-toolchain-owner` evidence already
discriminates successful loading of the pinned rules_rust extension closure.
No new oracle or invented `@bazel_tools` bytes precede this owner.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` supplies architecture guidance
only. Its repository-materialization plan assigns selected source identity and
repository views to Bzlmod, physical realization to the repository owner, and
normal `.bzl` consumption to loading; its typed source contract forbids a
consumer from reconstructing content identity or publishing an input bridge.
Copy no Zig code, representation, scheduler, digest, path, cache or root policy.

## Required implementation

Preserve root definition loading byte-for-byte. For only a root-owned,
non-isolated extension request whose definition repository resolves to one
selected registry module:

1. The request producer validates a unique canonical selected-definition
   association and retains it opaquely with the request. Unsupported owner,
   isolation, selected nonregistry/generated definition repository, missing or
   duplicate route, invalid mapping and builtin definition labels fail closed.
2. The selected definition projects one structural selected-source route from
   its exact `RepoSpec`, local-path policy, canonical identity and self apparent
   name. Add a distinct selected source polarity; do not disguise it as direct
   local, generated or builtin.
3. Loading dispatches the request label to the selected-source Bzl owner. A
   same-repository `:x.bzl` or `//pkg:x.bzl` load preserves the current selected
   definition; an `@apparent//pkg:x.bzl` load resolves through that definition's
   ordered mapping to another retained selected definition or the existing
   builtin `bazel_tools` route. Every child switches to its own producer view.
   Unknown, ambiguous, root/generated and unsupported nonregistry targets fail
   closed. Do not consult a path, command registry or mutable repo table.
4. Loaded-definition projection and pure-invocation reacquisition use the same
   selected-source owner and authenticate the same transitive manifest and
   module-extension projection. Do not retain a Starlark evaluator heap or
   callable beyond the existing frozen-module lifetime.

The implementation may factor shared root/selected dispatch locally, but may
not add a second semantic request producer, global cache/interner, lock, task,
fallback scan or command-side route repair.

## DICE, observation and event contract

Child order is request association first, then each definition source and its
recursive loads in declared load/request order, then named-export inspection.
Observed mode merges the request epoch left-first with source/recursive-load
epochs before inspecting that child's semantic terminal. Stop on the first
`Need`, typed outer or semantic error; do not full-scan or union Needs. Legacy
is carrierless. Complete-only equality and validity remain mandatory.

The selected request/definition association and every selected source route
are retained structural inputs. Frozen modules, transitive manifests,
definition projections, Result Arcs and the one observed epoch remain retained
at their existing owners; parse/evaluator/load-resolution scratch stays
compute-local. The Bzl child remains the sole owner of its evaluation event
batch. Parents neither replay nor republish child events. Warm reuse is silent.

Cancellation publishes no partial request, module, manifest, epoch or event
batch. Recovery recomputes through the same keys; A/B/A changes to `RepoSpec`,
canonical mapping, source bytes or recursive loads restore exact A semantics
and observations without stale B state.

Semantic DICE identity includes workspace, request owner/isolation, canonical
definition label and export, selected module identity and `RepoSpec`, source
policy, ordered apparent-to-canonical mapping, package/target, source bytes and
transitive load manifest. Display/apparent spelling and physical paths remain
separate projections. Bazel checksum/ActionKey and REAPI/CAS digests are not
created or consumed by this packet.

## Compatibility classification

- **Exact:** Bazel 9.2 root-owned, non-isolated selected-registry extension
  definition source admission; canonical/self/mapped label resolution for the
  pinned rules_rust source closure; source bytes, recursive-load ordering,
  export kind/projection and diagnostics already covered by the accepted Bzl
  evaluator semantics.
- **Slug-native:** typed Result/epoch carriers, Rust Starlark heap ownership,
  DICE key shapes, event-batch representation and immutable selected-source
  route representation.
- **Unsupported/deferred:** nonroot-owned and isolated extension requests;
  selected nonregistry or generated definition sources; broader builtin
  content; repository-rule `doc` and collection schemas/calls; `ctx.os`,
  download/extract, `repo_metadata`, toolchain/provider/action/input-tree,
  crate_universe, public ruleset breadth, M8/M7B and exact configuration/output
  identity bytes.

## Authority, budgets and validation

Rust authority is exactly:

- `app/slug_bzlmod_v2/src/host_module.rs`
- `app/slug_bzlmod_v2/src/host_package.rs`
- `app/slug_bzlmod_v2/src/selected_repo_spec.rs`
- `app/slug_bzlmod_v2/src/lib.rs`
- `app/slug_loading_v2/src/bzl_module.rs`
- `app/slug_loading_v2/src/module_extension.rs`

All other Rust, fixtures, oracle JSON/workspaces, Cargo/BUILD and generated or
vendored content are read-only. Caps are <=850 production, <=1,050 colocated
proof and <=1,900 aggregate additions across the six files; no helper/test may
exceed 100 lines. The exact accepted dirty entry state and physical ceilings
are:

| Path | Entry lines | Entry SHA-256 | Physical ceiling |
|---|---:|---|---:|
| `app/slug_bzlmod_v2/src/host_module.rs` | 4,872 | `56a7ffe34f8f26c3e70b02deed12268198599060cc455127f2edd3bddab22506` | 5,050 |
| `app/slug_bzlmod_v2/src/host_package.rs` | 5,009 | `1921abc6f0fedc0f7c0d14504168980f1063deec82bcfff9b64c2c3c6b8cc5b8` | 5,180 |
| `app/slug_bzlmod_v2/src/selected_repo_spec.rs` | 13,415 | `ad8c89c9d4613db408ae7429e51e3a59ae2e865a90f0daaf896dc2fe9624c333` | 13,900 |
| `app/slug_bzlmod_v2/src/lib.rs` | 469 | `0d33a0bacb5d8f6725cca664e9398abf8cecdf4274f18aa51c49d9fffe0641ee` | 525 |
| `app/slug_loading_v2/src/bzl_module.rs` | 9,120 | `20737363c9048fa5b5f81e6b8d4cdeb139e413ae0f053abc0cdaa1cc85cb9a58` | 9,900 |
| `app/slug_loading_v2/src/module_extension.rs` | 2,430 | `9d82768900f3459dda85cf7599a4b7f518717d986d9f570bcbbec634eef1a4c9` | 2,800 |

Existing dirty generated-repository work is accepted state and must not be
reformatted, reverted or charged to this packet. Validate these entry hashes
before editing and report the isolated packet diff against exact saved blobs.

The >2,000-line `selected_repo_spec.rs` and `bzl_module.rs` complexity triggers
are acknowledged. They remain cohesive here because the change extends their
existing selected-route/request and recursive-Bzl key families; a new central
or orchestration module would split private cycle/error/event machinery from
its owner. Keep each new helper below 100 lines and `REPLAN` if one selected
source driver cannot stay within the aggregate cap. `host_package.rs` may only
receive the exhaustive selected-source-polarity adjustment; this packet does
not activate selected package loading.

Reuse the accepted Buck2-derived `CompactString`, `SmallMap`, immutable `Arc`
slices, `Dupe` and `Allocative` conventions. Add no `BTreeMap`, global interner
or retained evaluator object. The applicable DICE authority is
`docs/developers/dice.md`: all semantic inputs are tracked key/value data, no
lock crosses a compute, and workspace identity plus structural inputs isolate
overlapping sessions. This is not a demonstrated performance hot path, so no
benchmark gate is authorized; retained-memory shape is a proof obligation.

Proof must cover root nonregression; selected registry request association;
self cross-package and mapped selected-registry recursive loads; builtin route
selection without inventing absent content; unknown/ambiguous/unsupported
mapping terminals; exact request -> source -> recursive epoch order; child-only
events; Need/outer stop; warm silence; key/value inequality for source,
mapping, bytes and manifest changes; A/B/A restoration; cancellation/recovery;
and loaded-definition/pure reacquisition drift authentication. Reuse the
accepted Bazel 9.2 source and `rules-rust-073-toolchain-owner` evidence; add no
oracle unless implementation proves a discriminating evidence gap.

Run formatting, focused tests, full `slug_bzlmod_v2` and `slug_loading_v2`, the
dependent `slug_core_v2` check, source-shape/absence checks, `git diff --check`
and independent terminal review. Rebuild `slug_cli_v2` only if a permitted
change affects its V2 binary path; no daemon smoke is required without public
activation.

STOP repository-rule declaration/schema/invocation changes, new
`repository_ctx` APIs, `@bazel_tools` content, public command activation,
fixture/oracle edits without `REPLAN`, Java/JVM, unrelated cleanup, milestone
closure, M8/M7B and exact identity bytes. `REPLAN` before widening files, caps,
compatibility or admitting a second owner.
