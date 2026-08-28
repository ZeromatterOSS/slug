# Current Slug V2 Packet

Packet: `WP-4-5-7A-expanded-registration-consumer-cutover`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `58d0d0357`.

Result: replace configured analysis's duplicated raw root-registration adapter
with one prepared input over the accepted execution-platform and toolchain
expansion families. Admit repository-aware candidates only through the bounded
native platform/toolchain and marker-leaf shapes already represented; preserve
the exact bypass and keep every rule definition in BCR Starlark.

## Immediate predecessor

Commit `58d0d0357` accepts full `PackageIdentifier` identity in configured
package inputs and the retained package collection. Both legacy and observed
analysis now depend on the general `HostPackageInventory` carrier and borrow
its `LoadedPackage`; canonical configured targets and registration families
remain deliberately inactive. This completes steps 1-2 of the four-step
architecture frozen by `104291321`.

## Learned facts and research basis

Pinned Bazel 9.2 is behavioral authority:

- `RegisteredExecutionPlatformsFunction` and `RegisteredToolchainsFunction`
  produce the ordered MODULE registration inputs before configured validation;
- `SingleToolchainResolutionFunction`, `ToolchainResolutionFunction`,
  `ToolchainContextUtil`, `PlatformFunction` and `PlatformKeys` preserve
  candidate order and separate registered loading facts from configured
  platform/toolchain validation;
- `ToolchainResolutionFunctionTest#resolve_noToolchainType` and focused
  registered/resolution tests cover empty requirements, candidate order,
  invalid registered kinds and configured selection; and
- accepted fixtures `toolchain-resolution-first-platform` and
  `nonroot-module-consumers` already discriminate selection order, A/B/A
  invalidation and selected nonroot registrations. Reuse them; add no oracle
  unless implementation exposes a configured distinction those fixtures and
  pinned tests do not cover.

Clean Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer architectural
guidance only. Its configured-analysis registration graph keeps registration
sources explicit, and its loaded native-toolchain projection separates
repository-aware declarations from configuration/type-keyed compatibility.
The zero-toolchain fixture supplies a regression theme. Copy no Zig code,
arena/store ID, service graph, diagnostic or compatibility claim; Bazel 9.2
alone decides exact behavior.

## Decision and ownership

Add one private, request-compute-scratch prepared registration input in
`slug_analysis_v2::dice`. It retains the two existing immutable expansion
`Arc`s and borrows their ordered canonical-label slices. It is not a DICE key,
cache, semantic side store or final configured-result field.

Bypass both families only when
`!has_toolchain_requirement && local_declarations.is_empty()`. Otherwise demand
execution platforms first and toolchains second, using
`ModuleRegistrationExpansionKey` in legacy mode and the two observation keys
in observed mode. A local declaration with no requirement must demand both
families to decide whether it is registered. Preserve outer -> Need -> semantic
error polarity and stop before package closure when either family fails.

Pass the prepared input once through execution-platform preparation and
toolchain resolution. Remove `compute_root_anchor_input`,
`direct_root_registration(s)` and their raw MODULE imports only after every
consumer uses expanded labels. Loading expansion keys remain the sole owners
of warning events and source/package observations; analysis neither replays
their batches nor combines the two families into a workspace-global key.

The configured package closure may now load canonical labels through the
accepted carrier. Canonical configured admission is limited to the represented
native `constraint_setting`, `constraint_value`, `platform`, `toolchain_type`
and `toolchain` selection inputs plus the existing dependency-free marker-leaf
Starlark implementation. Reject other canonical configured shapes before
source resolution, child analysis or evaluator execution. Root configured
behavior remains unchanged.

Do not add a registration parser, repository route/source lookup, filesystem
read, mapping copy, second package representation, event owner, evaluator,
interner, process cache or global registry in analysis. BCR Starlark owns all
rule definitions and control flow including `cc_internal`; `cc_common` is only
a demanding client of the general evaluator and host ABI.

## Proof obligations

Prove all of the following in legacy and observed paths where applicable:

- zero requirement plus zero local declaration activates neither registration
  family even when registration state is broken;
- a local declaration without a requirement activates execution-platform then
  toolchain expansion and cannot bypass an error from either family;
- a required root toolchain preserves current candidate/selection order while
  depending on each expansion family exactly once and no MODULE anchor;
- selected nonroot exact/package/recursive registrations feed canonical native
  candidates and a bounded canonical marker leaf without path-only aliasing;
- same package paths in two canonical repositories remain distinct through
  closure, validation and selection;
- family warning/event ownership stays below analysis, Need/outer precedence is
  unchanged, cancellation publishes no configured result, warm equality prunes
  equal inputs and A/B/A restores the prior topology; and
- aliases, custom `PlatformInfo`, target settings, external sources and
  arbitrary external configured dependencies remain rejected at the declared
  boundary.

Reuse accepted expansion, carrier and analysis lifecycle evidence. New tests
are code-local proof and require no new `fixture.toml`; if a Bazel-visible gap
requires a fixture, STOP and `REPLAN` its provenance and comparison class.

## Compatibility classification

- **Exact:** accepted MODULE expansion order and warnings, exact bypass,
  represented native platform/constraint/toolchain validation, first compatible
  candidate selection, and bounded root/nonroot marker-leaf result under the
  cited Bazel 9.2 evidence.
- **Slug-native:** prepared-input Rust layout, DICE dependency transport,
  typed analysis error wording not covered by oracle, structural hashes,
  scratch collections and staged external-shape guard.
- **Unsupported/deferred:** aliases and custom platform providers, target
  settings, command-line extra-registration/signed precedence, optional/named
  exec groups, external toolchain-type lookup requiring contextual Starlark
  index spelling, general external configured graphs, broader toolchain
  implementations, new builtins/actions and exact configuration/output bytes.

## Request, revision and memory behavior

Workspace and request policy remain structural inputs of the existing loading
keys. No mutable host state is read by analysis. Overlapping requests retain
their own DICE transactions and observed epochs; final validation and event
publication remain with existing owners.

Expansion and package `Arc`s are DICE-retained semantic memory. The prepared
pair, label vectors, closure sets and selection buffers are compute scratch and
drop on completion or cancellation. Configured results retain only their
existing topology/action context. No lock may span `ctx.compute`; no retained
value may borrow evaluator or command scratch. Existing complete-only equality,
invalidation and graph-version release rules remain unchanged.

## Allowlist, complexity and caps

Production:

1. `app/slug_analysis_v2/src/dice.rs`

Proof:

2. `app/slug_analysis_v2/tests/root_analysis.rs`
3. `app/slug_analysis_v2/tests/starlark_rule.rs`

No loading, Bzlmod, identity, configuration, query, core, CLI, Cargo, BUILD,
fixture, oracle, Zabel or plan file is admitted after this scheduling commit.
Caps: 800 net production lines, 1,100 net proof lines, 1,900 total; no new or
materially rewritten function over 120 lines.

`dice.rs` exceeds the 2,000-line review trigger but remains the sole cohesive
configured-analysis DICE owner. This packet deletes duplicated registration
parsing and adds only private preparation/admission helpers; splitting key
publication from its lifecycle/error ordering would increase ownership seams.
STOP if a second responsibility or repeated family driver appears.

## Validation

Run serially:

1. focused bypass, dependency-order, canonical identity, selection and
   fail-closed tests;
2. complete `slug_analysis_v2` tests;
3. direct `slug_loading_v2`, `slug_bzlmod_v2` and `slug_query_v2` dependent
   suites needed to prove unchanged public owners;
4. accepted `toolchain-resolution-first-platform` and
   `nonroot-module-consumers` fixture verification only if the code-local matrix
   exposes an uncovered exact distinction;
5. `cargo fmt --all --check`, allowlist/cap/function checks,
   `git diff --check`, packet/canonical ID agreement and archive status against
   its recorded three-file baseline; and
6. independent terminal review before acceptance.

## Stops

STOP and `REPLAN` for a raw registration parser in analysis; any family bypass
when either conjunct is false; toolchain-before-platform dependency order;
analysis-side mapping, route, source or path discovery; flattening package
identity; a combined workspace registration DICE key; warning-event replay;
custom alias/provider/settings or option-precedence behavior; general external
configured graph admission; new rule/builtin/action semantics; a second
evaluator; Rust ownership of BCR rules or `cc_internal`; a C++ parser/rule
engine; Zabel treated as authority; a lock across DICE; files outside the
allowlist; or cap overflow.
