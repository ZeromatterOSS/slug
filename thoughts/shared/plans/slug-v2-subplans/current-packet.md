# Current Slug V2 Packet

Packet: `WP-4-5-7A-contextual-command-setting-preparation`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `d5362b099`.

Result: implement category 3A from the accepted contextual-command-overlay
architecture. Build and cquery must carry one immutable typed command overlay
through one-shot and daemon paths, prepare the final structural configuration
after Bzlmod mapping through one DICE owner, and delete the singular
`@@//:setting` bridge. Native extra-registration values become typed
configuration facts in this packet but are not consumed until category 3B.

## Accepted predecessor and bounded decision

Commit `d5362b099` independently accepts the category-wide architecture. It
freezes one compact occurrence carrier, one post-Bzlmod configuration-
preparation owner, and one later shared signed registration consumer. Pinned
Bazel 9.2 is behavior authority; clean Zabel `0795445f…` is peer ownership and
allocation guidance only. Buck2-derived Rust owns generic Starlark parsing and
evaluation. BCR Starlark owns every rule/control path including `cc_internal`;
`cc_common` remains only a demanding future client of the generic host ABI.

This packet makes the accepted representation and first DICE boundary real.
It deliberately does not expand signed registration patterns, merge command
and MODULE registrations, select configured toolchains, evaluate providers,
or add any C++-specific parser or rule path.

## Learned facts and source basis

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` remains exact authority.

- `StarlarkOptionsParser#parseGivenArgs` and `#parseArg`, with
  `StarlarkOptionsParsingTest`, require every occurrence to load and convert;
  scalar settings keep the last value, allow-multiple strings and repeatable
  list/set settings accumulate in command order, nonrepeatable list/set text
  uses the declaration converter, Boolean no-value and `no` spellings are
  declaration checked, non-flags fail, and effective defaults are elided.
- `PlatformOptions` and `PlatformOptionsTest` establish comma conversion,
  last-occurrence replacement for `extra_execution_platforms`, concatenation
  for `extra_toolchains`, and normalization by deduplicating while keeping the
  last copy.
- Existing loading owns all five admitted direct build-setting declaration
  kinds and root/canonical package inventories. The selected final root mapping
  is owned by `HostRootRepositoryMappingKey` and its observed wrapper.
- Existing analysis owns the sole declaration-authenticated converter and
  default/scope resolution. Existing configuration owns the only native option
  vector, canonical Starlark option map, structural equality, canonical bytes,
  and Slug-native projection.
- Current command/build/cquery/server/core paths repeat `root_string_setting`
  and `explicit_starlark_option`, eagerly construct `@@//:setting`, and may
  claim output identity before contextual declaration resolution. These are
  the fallbacks deleted by this packet.
- Buck2 DICE ownership guidance in `docs/developers/dice.md` requires every
  semantic input in the key/dependency graph, publication only after all
  dependencies complete, and no lock across a DICE computation. Existing
  observed mapping/package wrappers provide the path-frontier union inputs.

Clean Zabel's immutable request occurrence projection and typed final option
owner are concept/test guidance. Retained Buck2-derived `Arc` slices, `Dupe`,
`Allocative`, compact strings and small deterministic maps are approved leaf
utilities. Copy no Zabel layout/parser/diagnostic/checksum and add no new
interner, cache, standard retained map/set, or Buck cell/label identity.

## Implementation contract

### Shared request projection and sole classifier

Add `CommandConfigurationOverlay` to `slug_configuration_v2`. It is an
immutable `Arc<[CommandConfigurationOccurrence]>`, cheap-clone, allocative,
structurally Eq/Hash, serde-compatible, and shared unchanged across command,
CLI, daemon wire, core root and preparation key. Empty overlays share the
standard empty allocation. Its only variants are:

1. direct root/apparent-external Starlark label text plus optional raw value
   and Boolean negation;
2. raw `extra_toolchains` joined value; and
3. raw `extra_execution_platforms` joined value.

`slug_commands_v2` remains the sole argv classifier. Build and cquery admit
direct `--//pkg:flag=value` / `--@repo//pkg:flag=value`, value-less positive
Boolean candidates and value-less `--no<label>` candidates. Native extras
require joined values. Preserve raw valid-Unicode bytes, missing versus empty,
negation and command order. Reject canonical `@@` spellings, relative labels,
split-token native values, `--config`, aliases and every unadmitted flag before
one-shot/daemon selection.

Replace build/cquery `root_string_setting` with the shared overlay. The daemon
wire serializes that type directly, with no raw argv or second classifier.
Delete fixed-label comments, JSON fields, test constructors and run-path
special cases associated with the old bridge.

### Batched structural configuration update

Add one `SlugConfiguration` batch boundary that accepts the complete final
`StarlarkOptions` plus the raw overlay and constructs canonical bytes once.
Convert every native occurrence through the existing descriptors and comma
converter. Replace execution-platform values with the last occurrence.
Concatenate toolchain values then deduplicate while keeping the last copy.
Expose one generic borrowed string-list projection over the existing native
option vector for category 3B; add no family-specific retained field.

If native values and final Starlark options equal the base, return the existing
configuration allocation. Otherwise retain one new configuration allocation.
All conversion vectors and dedupe sets are phase scratch. Native descriptor,
type or layout mismatches fail closed.

### One post-Bzlmod preparation DICE owner

Add `CommandConfigurationPreparationKey` and its observed wrapper in
`slug_analysis_v2`, keyed structurally by workspace, base structural target
configuration and the overlay. The retained result is only the final
`ConfigurationKey`; the observed value additionally owns the exact union of
mapping and distinct declaration-package observations.

The driver resolves main-repository labels directly and demands the final root
repository mapping before resolving any apparent-external label. This avoids
making a root-only option depend on an unrelated selected BCR graph while
retaining exact mapping ownership where canonical identity can change. It then
demands every distinct root/canonical package declaration even when a later
scalar occurrence wins. Unknown mapping, missing/non-rule/non-build-setting,
`flag = False`, project scope, invalid raw type, illegal no-value/negation and
unsupported route failures are semantic. All independently resolvable
declarations are demanded before semantic return.

Convert all occurrences before grouping final values. Group by mapping-free
canonical setting label, then apply:

- integer, Boolean, ordinary string and nonrepeatable list/set: last value;
- allow-multiple string and repeatable list: concatenate in command order;
- repeatable set: union then canonical sorted unique membership; and
- declaration-equal final values: remove that label from the base map.

Preserve unrelated base Starlark rows. Build the final `StarlarkOptions` and
native vector once through the batch boundary. Observed outer failure outranks
the union of Needs; Need outranks semantic failure. Complete errors/results are
equal-cutoff; Need is invalid. Deterministic cancellation publishes nothing and
same-graph corrected input recovers. No lock or evaluator value crosses the
compute boundary.

### Build/cquery no-shim cutover

Build roots retain base configuration plus overlay. Legacy, singleton,
observed multi-root and ordinary observed paths demand the matching preparation
key before configured-node preparation and pass the same final configuration
to every literal root and action-closure edge. Cquery computes one observed
prepared configuration before its root batch and uses it for every literal.

Move configured-output claiming after successful preparation/analysis. One-
shot and daemon build/cquery pass the same overlay type and demand the same
semantic owner. Delete `BuildCommandRootKey::new_with_starlark_option`, every
`explicit_starlark_option` field/argument, fixed `@@//:setting` construction,
and the old per-node explicit option repair. Keep the ordinary configured-node
preparation boundary for package/target admission with an already-final
configuration.

## Compatibility classification

- **Exact:** admitted direct root/apparent-external joined-value Starlark
  occurrences; Boolean value-less/no forms; all five loaded declaration kinds;
  declaration authentication; scalar last-wins; allow-multiple/repeatable
  accumulation; set normalization; default elision; native comma conversion,
  execution replacement and toolchain keep-last normalization; one-shot/
  daemon and build/cquery semantic identity.
- **Slug-native:** carrier/wire layout, DICE decomposition, Rust structural
  configuration identity, valid-Unicode strings, error wording and mapping-
  free canonical grouping representation.
- **Unsupported/deferred:** `.bazelrc`/`--config`, split-token native values,
  `--flag_alias` or MODULE `flag_alias`, build-setting aliases, canonical `@@`
  command spelling, label-valued/feature flags, project scope, signed command
  registration consumption, configured platform/toolchain eligibility,
  provider payloads, broader commands and exact Bazel checksum/output bytes.

## Proof obligations

1. Parser matrices distinguish ordered rows, missing/empty values, Unicode,
   Boolean negation, native joined values and rejected near misses for both
   build and cquery.
2. The same overlay round-trips daemon JSON without a second parser; old wire
   fields, root fields/types and every fixed `@@//:setting` construction are
   absent from production.
3. Configuration tests discriminate execution last-wins, toolchain
   concatenate/keep-last, explicit empty and embedded empty members, canonical
   bytes, borrowed native projection, equal-allocation reuse and Arc clone cost.
4. Root and apparent-external labels resolve through final mapping. Mapping,
   declaration kind/default/scope and raw row changes invalidate independently;
   semantically equal final configurations converge.
5. All five kinds cover scalar/repeated/default/malformed/non-flag behavior;
   malformed earlier scalar rows fail despite a later valid winner.
6. Multi-package tests prove outer > Need > semantic, cold cancellation,
   no partial publication and same-graph recovery for legacy and observed keys.
7. Build/cquery use one final configuration across every root/dependency;
   A/B/A one-shot and stable daemon requests restore structural results, and
   output claiming never precedes successful preparation.
8. Ownership scans prove one classifier/carrier/Starlark map/native vector,
   no retained standard collection or copied declaration/default, no raw row in
   MODULE keys, no evaluator retention and no ruleset discriminator.

## Allowlist, caps and validation

Writable production/test files are limited to:

1. `app/slug_configuration_v2/{Cargo.toml,src/lib.rs,src/command.rs,src/native/configuration.rs,src/native/mod.rs,src/native/tests.rs}`;
2. `app/slug_commands_v2/{Cargo.toml,src/common.rs,src/build.rs,src/cquery.rs,src/lib.rs,tests/commands.rs}`;
3. `app/slug_server_v2/{Cargo.toml,src/lib.rs,src/server.rs,src/tests.rs}`;
4. `app/slug_cli_v2/src/commands/{aquery.rs,build.rs,cquery.rs,run.rs}` and the
   mechanical compile-dependent call site in `app/slug_cli_v2/tests/cli.rs`;
5. `app/slug_analysis_v2/src/{lib.rs,build_setting.rs,dice.rs,command_configuration.rs}` and `app/slug_analysis_v2/tests/{root_analysis.rs,starlark_rule.rs}`;
6. `app/slug_core_v2/src/runtime/{mod.rs,dice.rs,tests/build_command_tests.rs,tests/cquery_command_tests.rs}` and `app/slug_core_v2/tests/runtime.rs`;
7. the mechanical compile-dependent call site in
   `app/slug_reapi_v2/tests/reapi.rs`;
8. workspace `Cargo.lock` only if dependency resolution changes; and
9. the canonical plan, Stage 6 owner, Stage 9 ledger and this manifest.

Caps: 1,900 net production Rust lines, 2,500 net test Rust lines, 550 net plan
lines, two new production modules and no fixture/oracle files. `runtime/dice.rs`,
analysis `dice.rs`, server tests and the owner plan exceed complexity triggers;
touch them only for their existing cohesive root/preparation/test/status roles.
New preparation semantics live in the bounded analysis module, not either
large DICE file. No performance benchmark is required: retained memory changes
are bounded by size/Arc identity and equal-allocation tests, not a demonstrated
runtime hot path.

Run formatting; focused configuration/commands/analysis/core/server/CLI tests;
one direct compile-dependent suite; `cargo build -p slug_cli_v2` before any
`SLUG_V2_BIN` smoke; stale-`slugd` cleanup around daemon tests; locked ownership
and forbidden-surface scans; `scripts/v2_archive_status.sh`; and
`git diff --check`. Obtain independent retained-identity/DICE/lifecycle review
before acceptance.

No new fixture is needed: accepted upstream source/tests discriminate this
internal command/configuration cutover, and owner-local unit/lifecycle tests are
stronger than copying a workspace. Alias/project/label-setting upstream tests
are skipped as explicitly deferred surfaces; Java implementation-detail maps
and diagnostic text are not ported. There is no fallback: the old bridge is
deleted in the same cutover.

## Stops

STOP and `REPLAN` for a second argv/configuration parser; parallel native or
Starlark option storage; raw argv on daemon wire or MODULE key; configuration
construction before final mapping; skipped earlier occurrence validation;
copied declarations/defaults; per-kind or per-command carrier fields; output
claim before preparation; retained evaluator state; global cache/interner or a
lock across DICE; signed-registration/provider/eligibility breadth; a C++-
specific path; Rust `cc_internal` control flow; Zabel as authority; caps/
allowlist breach; or a material correction to the accepted architecture.
