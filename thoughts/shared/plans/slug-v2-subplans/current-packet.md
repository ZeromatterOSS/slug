# Current Slug V2 Work Packet

Packet: WP-7-12-m7a-rustc-callback-proof-entrypoint-r1
Status: result REPLAN; authentic local inputs and same-DICE harness required

## Outcome and classification

Determine the smallest **real loaded-source** command path that could prove
the pinned `rules_rust` Rustc `Args.add_all` callsite and later same-DICE
source A/B/A for the unaccepted regular crate-root adapter. This packet
admits no behavior. The candidate remains local at review branch
`review/wp-7-11-rustc-crate-root-map-each`, commit `ea5fcc6fd`, based on
accepted main `77ea40e89`; do not merge or push it as an accepted slice.
The candidate class is finite pinned-output compatibility with Slug-native
error timing, not generic Bazel callback support.

The accepted source audit `77ea40e89` identified callback admission as the
first known configured CLI Spawn blocker. Candidate compilation and focused
tests establish only the structural recipe and existing callback rejection.
Independent final-delta review returned `REPLAN` because no real loaded
`rustc.bzl:1169` positive or same-DICE source A/B/A proof exists. A focused
attempt with checked-in fixture
`tests/v2_oracle/fixtures/rules-rust-073-toolchain-owner/workspace`
selected its single `//pkg:support` rust_library. The existing analysis test
helper stopped before rules_rust loading: first on a noncanonical test path,
then, after canonicalization, on missing `RootModuleRegistryUrlsKey`.
Both attempts took under 0.03 seconds, were removed from the candidate, and
do not count as callback evidence. Source review confirms the selected
source-file route reaches `ConfiguredNodeKind::SourceFile` only after a DICE
`PathNodeKind::RegularFile` observation (`dice.rs:5318-5350`); the generic
`AnalysisArtifact::Source` type does not encode that fact.

## Read-only decision

The normal one-shot `aquery` route enters
`evaluate_workspace_build_command_with_repository_environment` and injects
the root module policy, registry URLs/request generation, and repository
materialization generation (`app/slug_core_v2/src/runtime/dice.rs:6333-6367`).
It could reach the real callback after source preparation, but the public
wrapper constructs a fresh `WorkspaceRuntime` per call
(`runtime/mod.rs:150-173`). Repeating that command cannot prove same-DICE
source A/B/A. A daemon or a focused test retaining one `WorkspaceRuntime`
would be needed for the invalidation gate.

The checked-in tiny workspace depends on `rules_rust` 0.73.0 and `platforms`
1.1.0 and registers a generated Rust toolchain. It contains neither a local
registry for those versions nor their source trees/archives or a lockfile.
The command defaults to `https://bcr.bazel.build/` when no `--registry` is
provided (`registry.rs:69-80`); Bazel's external cache is not a Slug
repository materialization. There is no bounded, fully local command proof
from this fixture as checked in. The missing prerequisite is a pinned,
locally materializable registry/repository fixture with exact source hashes,
including the candidate's authenticated `@@rules_rust+//rust/private:rustc.bzl`
and `rustc.bzl:1169`, plus a harness that keeps the command runtime/DICE
instance while changing an observed regular source file A/B/A. That fixture
must also prove toolchain registration and root command policy through the
ordinary source path. Do not claim the preserved callback candidate from
the two pre-loading diagnostics or from a copied Bazel cache tree.

This is `REPLAN` for the callback candidate's missing proof prerequisite, not
a semantic failure of its passing structural test. The candidate remains on
`review/wp-7-11-rustc-crate-root-map-each` (`ea5fcc6fd`) and is not merged.
No M7A readiness row advances. The next independent demanded implementation
packet is `WP-7-13-m7a-reapi-cache-core-leaf-r1`: establish a graph-independent
protocol/digest/CAS/AC leaf from the existing `slug_reapi_v2` code and route
the admitted FileWrite consumer through it, preserving exact REAPI/CAS bytes.
Stage 11's bootstrap cache-core contract and Stage 7's accepted FileWrite
handoff govern that packet. Its design must first pin the existing and
upstream protocol inputs, name the actual leaf API and Cargo/Bazel dependency
edges, and get independent review of the shared public boundary before code.
It must not activate Spawn, virtual param files, or the parked callback.

## Inspection scope and validation

Inspection covered the existing command-runtime registry/repository injection,
the checked-in tiny rules_rust fixture, the one-shot aquery wrapper, and the
reusable cache-core owner contract. It found no exact runnable proof route
without new local external inputs and a retained-runtime harness.

Use local source inspection only; do not run another Cargo compile, test,
Bazel/Slug query, network fetch, build action, or daemon in this packet. The
two failed diagnostics already identify the missing inputs, so repeating them
would add no evidence. No fixture, source copy, DICE key, helper, callback
adapter, ActionSpec, or REAPI behavior changes here. The natural owners remain
the command runtime's injected request, existing Bzl source observations,
configured action recipe, and final acceptance boundary. No new retained
memory, cache, utility extraction, or lifecycle owner is selected.

Allowlist: this manifest, canonical Live Status, and Stage 7/bootstrap
readiness only if the handoff contract changes. Validate with targeted source
inspection, `python3 scripts/v2_plan_status.py`, and `git diff --check`.
No runtime result is claimed from this read-only packet. Subsequent
tests should be as small as possible, avoid repeating runs over a few
seconds, and scrutinize any test over roughly 30 seconds before selection.
