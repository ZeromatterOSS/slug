# Current Slug V2 Packet

Packet: `WP-4-5-7A-repository-rule-declaration-metadata-implementation-r2`

Milestone: M7A category 6 generated-repository prerequisite.

Base: catalog acceptance commit `87d332cf6`, retaining the dirty selected-
context R2 candidate unchanged. The exact Bazel 9.2 registered-toolchain
catalog packet is terminally `ACCEPT`.

R1's focused metadata proofs pass. Its required full loading suite reaches
411/412 and exposes one stale catalog-only expectation already present at this
packet's base: the built-in subtree test still names only the three packages
from before commit `87d332cf6`. R2 admits only that exact proof vector; the
metadata implementation, representation, behavior and Rust caps do not change.

## Observable result

`repository_rule()` accepts and retains its Bazel declaration metadata:
`local`, `configure`, and `environ`. The environment-name sequence is
deduplicated by first occurrence and remains available, with both booleans,
through freeze, exported-definition projection, module-extension call capture,
and generated-repository instantiation identity.

This packet records invalidation metadata only. It does not read an environment
value, observe Host OS, execute a repository rule, add a repository-context
capability, materialize an effect, expand registrations, or resume selected
configured analysis.

## Learned facts and authority

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole semantic authority.

- `StarlarkRepositoryModule.repositoryRule` passes `local` and `configure`
  directly to the repository rule and converts `environ` with
  `ImmutableSet.copyOf(Sequence.cast(...))`: names must be strings, duplicates
  collapse, and first-occurrence iteration order is retained.
- `RepositoryFetchFunction` and `DigestWriter` obtain the declared set from the
  repository-rule definition before asking `RepoEnvironmentFunction` for an
  environment view. Declaration capture therefore precedes and is independent
  of Host observation.
- The existing Slug loading path already owns the definition, frozen callable,
  exported projection, invocation record and instantiated repository. Rejecting
  these three arguments is the bounded gap; no new DICE key or side registry is
  needed.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is peer ownership guidance only: declaration metadata remains separate from
invocation capabilities and effects. No Zig code, layout, scheduler or
compatibility claim is copied.

The retained Buck2-derived utility review selects existing `CompactString`,
`SmallSet`, `Arc` and `Allocative`. Retain
`Arc<SmallSet<CompactString>>`: `SmallSet` preserves insertion order while its
equality is set-semantic, matching Bazel's `ImmutableSet`. No interner, cache,
global table or new hash identity is authorized.

## Compatibility and decisions

- **Exact:** boolean defaults are false; explicit true/false values survive;
  `environ` accepts only strings, removes duplicate names, and retains first
  occurrence iteration order while equality ignores unique-name order; the
  booleans and environment membership participate in every generic definition,
  call and instantiation projection that carries repository-rule identity.
- **Slug-native:** immutable Rust storage and structural DICE equality for the
  retained metadata.
- **Unsupported/deferred:** non-`None` `doc`, environment values, `--repo_env`
  precedence, Host OS, repository-context access, effects, Windows SDK
  discovery and selected-context closure proof.

The packet chooses producer-owned definition metadata. It does not make
`local` or `configure` execute policy, infer environment names from source,
read ambient process state, or place metadata in evaluator scratch. BCR
Starlark continues to own all rule/control flow including `cc_internal`;
`cc_common` remains a generic host/provider-ABI client.

## Natural ownership and lifetime

`package_globals::repository_rule` validates and captures the metadata on
`RepositoryRuleDefinitionGen`. Freeze moves it to
`FrozenRepositoryRuleDefinition`; `projection()` copies the shared immutable
metadata into `RepositoryRuleDefinitionProjection`; frozen invocation copies
that projection into `RepositoryRuleCallRecord`; instantiation retains the
record in `HostInstantiatedModuleExtensionRepository` and its existing DICE
values.

The strings and ordered set are DICE-retained semantic memory after freeze and
must not borrow evaluator or command scratch. Clones are Arc ref-count bumps.
Existing source/revision invalidation and complete-only equality remain the
publication boundary; changing either boolean or environment membership must
change the retained projection and downstream instantiation, while duplicate-
only and reordered-unique spelling changes normalize to the same semantic
metadata without losing the retained first-occurrence iteration order.
There is no async work, cache, eviction, cancellation or shutdown ownership in
this packet.

## Exact allowlist

Only these Rust files may change; tests stay module-private in them:

- `app/slug_loading_v2/src/package.rs`, current blob
  `a4dcb97585cc5463d820040930d6fae5fa3bdd45`;
- `app/slug_loading_v2/src/module_extension_repository_rule.rs`, current blob
  `c3e81f5a150911abf1dd945742175977153f6937`;
- `app/slug_loading_v2/src/module_extension_repository_instantiation.rs`,
  current blob `da82d97e7787136a621145a85998a5073b200b37`;
- `app/slug_loading_v2/src/module_extension.rs`, current blob
  `02a00cf7b97e9815d34f1f1333488b5f622b18c4`.

R2 proof-only correction:

- `app/slug_loading_v2/src/external_subtree_package_set_tests.rs`, current/base
  blob `85d1c8467018a903ead7dbd5a124726ca5f7c9cc`; only the two exact expected
  package vectors in
  `real_builtin_catalog_discovers_root_and_prefixed_package_sets` may change to
  include the packages imported by `87d332cf6`.

Completion documentation may update only this manifest, the canonical plan,
Stage 6 and routing log/history. No Cargo manifest, lockfile, fixture, other
test, DICE key, repository effect, analysis or command file is allowed.

## Implementation and proof

Add `local: bool`, `configure: bool`, and insertion-ordered/set-equal
`environment: Arc<SmallSet<CompactString>>` to the definition and projection
path. Construct it once from the declared sequence. Keep non-`None` `doc`
rejected exactly as today. Thread the complete projection through every
constructor and test helper; do not add defaults at a downstream consumer.

Module-private proof must establish:

- omitted and explicit-false arguments produce false/false/empty metadata;
- true `local` and `configure` survive capture, freeze and export;
- `environ = ["B", "A", "B"]` iterates exactly as `["B", "A"]`;
- a non-string element retains the Starlark argument-type failure;
- non-`None` `doc` remains rejected;
- two calls through the frozen exported definition receive the same complete
  metadata without sharing evaluator-lifetime state;
- instantiation retains the complete definition projection;
- changing each boolean or environment membership changes equality, while
  duplicate-only and reordered-unique inputs compare equal; and
- cold/warm/A-B-A source reload restores complete repository-call and
  instantiated-repository equality without reading Host state.

Run focused repository-rule definition, invocation and instantiation tests,
then full serial `cargo test -p slug_loading_v2`, one direct Bzlmod dependent if
the public loading boundary changes, `cargo fmt --all`, `git diff --check`,
file/blob/scope/cap audit and `scripts/v2_archive_status.sh`. Cargo commands are
serial.

## Caps, complexity and stops

Maximum additions: 420 production Rust lines, 650 proof Rust lines and 1,070
aggregate Rust lines. Deletions do not create budget. `package.rs` and
`module_extension.rs` already exceed the physical-size trigger, but remain the
cohesive existing Starlark-global and module-extension DICE owners; this packet
adds only fields, capture and colocated tests and must not add another owner.

`REPLAN` for any new DICE key, public schema, environment/OS read, command
overlay, effect execution, repository-context capability, selected-analysis
change, ruleset-specific special case, new file, representation outside the
compact immutable projection, or required change beyond the four-file
allowlist/caps.

After terminal acceptance, activate only
`WP-4-5-7A-effective-repository-host-input-architecture`, a zero-Rust design
packet. No generated-repository effect implementation is authorized yet.
