# Current Slug V2 Packet

Packet: `WP-4-5-7A-command-registration-overlay-consumer`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `f76576ccd`.

Result: consume the typed `extra_toolchains` and
`extra_execution_platforms` configuration facts accepted in the predecessor.
One loading-owned signed command expansion key must reuse the existing
contextual target-pattern/package walker; configured analysis must merge its
canonical labels before the unchanged positive-only MODULE expansions. This
packet does not widen configured eligibility, providers, or ruleset behavior.

## Accepted predecessor and bounded decision

Commit `f76576ccd` accepts the shared immutable command overlay, descriptor-
driven native normalization, contextual setting-preparation DICE owner, and
build/cquery one-shot plus daemon cutover. Commit `d5362b099` independently
freezes this successor's architecture: command registration rows are typed
configuration facts, never MODULE facts, and use one shared loading parser and
walker.

Pinned Bazel 9.2 is behavior authority. Clean Zabel `0795445f…` is peer
ownership/allocation guidance only. Buck2-derived Rust owns generic Starlark
syntax and evaluation; BCR-delivered Starlark owns every rule definition and
control path including `cc_internal`. `cc_common` is only a demanding future
client of the generic host ABI, not a C++ parser or Rust rule engine.

## Learned facts and source basis

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` remains exact authority.

- `RegisteredToolchainsFunction` reads the final structural configuration and
  root repository mapping, reverses the normalized command toolchain list,
  parses it as signed target patterns, then appends positive Bzlmod patterns.
- `RegisteredExecutionPlatformsFunction` performs the same sequence without
  reversing the configured execution-platform list.
- `TargetPatternUtil#parseAllSigned` and `SignedTargetPattern#parse` strip one
  leading `-`. `TargetPatternUtil#expandTargetPatterns` folds rows
  sequentially: positives append, negatives remove all matches, and the final
  immutable set preserves insertion order and deduplicates.
- `RegisteredToolchainsFunctionTest#testRegisteredToolchains_flagOverride`,
  `#testRegisteredToolchains_flagOverride_multiple`, and the corresponding
  `RegisteredExecutionPlatformsFunctionTest` cases prove command-before-
  MODULE and the family-specific order. Existing `TargetPatternUtilTest`
  plus Slug's complete contextual-pattern/package tests cover signed fold and
  root/canonical single, package, and recursive expansion.
- Loading already owns the sole contextual `CanonicalTargetPattern` parser,
  Root/Canonical package and subtree producers, family filters, ambiguity
  warnings, and two workspace-only positive MODULE expansion keys. Analysis
  owns the only configured registration consumer.
- The predecessor leaves both native lists solely in `SlugConfiguration` and
  exposes one generic borrowed string-list projection. No raw command overlay
  or joined text is needed here.
- `docs/developers/dice.md` requires structural keys, explicit observed input
  edges, no partial publication, and no lock across a DICE computation.

Zabel's separate final-option and MODULE registration ownership is concept
guidance. Reuse existing V2/Buck2-derived `Arc` slices, `Dupe`, `Allocative`,
compact strings and `SmallMap`/`SmallSet`; copy no Zabel layout, parser,
scheduler, checksum, diagnostic, or compatibility claim. Add no interner,
cache, standard retained map/set, or parallel registration store.

## Implementation contract

### One shared loading walker and signed command key

Keep `ModuleRegistrationExpansionKey` and its observed wrapper unchanged. Add
one command key family identified structurally by workspace, final
`SlugConfiguration`, and registration family. It reads only the generic typed
native string-list projection for the selected family. Empty lists complete
without demanding mapping or package producers.

For a nonempty command list, demand the final root mapping through the legacy
or observed Bzlmod owner, retain no copy of it, and parse each row in the root
canonical context. Toolchains walk the configuration's already normalized
keep-last list in reverse; execution platforms walk in stored order. Strip
and retain one leading sign before passing the remainder to the existing
`CanonicalTargetPattern` parser and the same package/subtree walker used by
MODULE rows.

Generalize only the walker's private row input and ordered-set operation.
Positive matches append unseen canonical labels. Negative matches remove every
matched label from membership and order. A later positive match reinserts at
its later position. Preserve exact single-target missing errors, family
filtering, recursive package postorder, wildcard ambiguity warnings, route and
package ownership. MODULE rows remain positive and retain each declaring
module's mapping context; command rows never enter
`HostSelectedRegistrationPatterns` or either MODULE key.

The retained command value reuses the existing immutable expansion shape:
canonical `Arc` label and ambiguity slices plus semantic error. The command key
retains only workspace/configuration/family. Mapping views, row signs,
worklists, package/route tables and ordered sets are compute scratch.
Observed results union the root-mapping and package/route frontiers. Outer
observation failure outranks Need; Need outranks semantic failure; Need is
invalid and complete results/errors use equality cutoff. Cancellation publishes
nothing and same-graph corrected input recovers.

### Command-first configured consumer

Replace analysis's `PreparedModuleRegistrations` scratch with one prepared
registration owner. When toolchain topology is needed, independently compute
command and MODULE expansions for execution platforms and toolchains before
choosing a terminal result. Across all four computations, the first outer
failure outranks the union of every Need, which outranks the first semantic
failure.

Merge canonical labels per family by appending the completed command slice and
then the positive MODULE slice through one compact ordered set. This is the
same sequential fold after command signs have been applied: command positives
win first position, command negatives affect preceding command matches, and a
later MODULE positive may insert a label absent from the command result.
Candidate platform/package loading consumes only these merged immutable
slices. Empty command lists preserve MODULE-only ordering and structural
results and demand no command expansion packages. Unrelated command settings
may key a distinct empty command computation but never invalidate the
workspace-only MODULE keys.

Do not add eligibility, alias resolution, target/exec constraint selection,
provider payloads, selected implementation analysis, or a ruleset-specific
consumer. Those remain category 4 and later packets.

## Compatibility classification

- **Exact:** admitted signed root/apparent-external command target patterns;
  positive/add, negative/remove and later reinsertion order; toolchain reverse
  after keep-last normalization; execution-platform stored order; command-
  before-MODULE precedence; family filtering; unchanged MODULE-only behavior.
- **Slug-native:** Rust retained layout, DICE key/result decomposition,
  structural configuration identity, valid-Unicode strings, observation
  carrier and unproved diagnostic wording.
- **Unsupported/deferred:** `.bazelrc`/`--config`, split-token native values,
  `--flag_alias`/MODULE aliases, configured registration aliases and
  eligibility, target/execution constraint truth, provider payloads,
  `ctx.toolchains`, exact Bazel configuration/output bytes, broader commands,
  and every Rust implementation of BCR rule control flow.

## Proof obligations

1. Command single/package/recursive rows expand in root and mapped-external
   contexts through the same parser/walker as MODULE rows.
2. Positive, negative, remove-all and later reinsertion preserve exact label
   order; exact missing targets still fail for either sign.
3. Toolchains reverse after normalized keep-last order while execution
   platforms do not.
4. Command results precede positive MODULE results and deduplicate at the
   earliest surviving position; a command-only negative does not suppress a
   later MODULE positive.
5. Empty command lists reproduce MODULE-only slices/results and activate no
   command mapping/package producer; unrelated configurations do not change
   MODULE-key equality.
6. Legacy and observed A/B/A tests prove configuration identity, mapping and
   package invalidation, warm equality, family isolation and exact frontier
   union.
7. Multi-input tests prove outer > union-of-Needs > semantic ordering, cold
   cancellation, no partial publication and same-graph recovery.
8. Ownership scans prove one parser/walker, no command row in MODULE owners,
   no raw overlay read, no retained standard collection/mapping copy/evaluator,
   and no ruleset discriminator.

## Allowlist, caps and validation

Writable files are limited to:

1. `app/slug_loading_v2/{Cargo.toml,src/lib.rs,src/registration_expansion.rs,src/registration_expansion_tests.rs}`;
2. `app/slug_analysis_v2/src/dice.rs` and
   `app/slug_analysis_v2/tests/{root_analysis.rs,starlark_rule.rs}`;
3. workspace `Cargo.lock` only for the loading-to-configuration dependency;
4. the canonical plan, Stage 6 owner, Stage 9 ledger and this manifest.

Caps: 700 net production Rust lines, 1,000 net test Rust lines, 260 net plan
lines, no new production module and no fixture/oracle file. Loading
`registration_expansion.rs`, analysis `dice.rs`, and analysis tests exceed
complexity triggers; they remain the cohesive existing expansion, prepared-
consumer and lifecycle owners. Do not split the shared walker into a second
module or add policy to large analysis DICE code.

Run formatting; full loading and focused/full analysis suites; one direct core
compile/test dependent; locked ownership/order scans; `scripts/v2_archive_status.sh`;
and `git diff --check`. No new fixture is needed: pinned Bazel source/tests and
the accepted loading fixtures discriminate the behavior more directly. Alias,
configured eligibility and provider tests are skipped because those phases are
explicitly deferred. Obtain independent ordering/DICE/lifecycle/retained-
identity review before acceptance.

There is no fallback or bridge. Residual risk is performance of repeated
negative removal in small command registration lists; correctness and compact
retained shape take precedence, and no hot-path claim is made without a later
measurement.

## Stops

STOP and `REPLAN` for a second target-pattern parser/walker; raw command input
or configuration inside a MODULE key; unsigned expansion; command rows after
MODULE rows; copied final mapping; parallel registration storage; package or
route discovery in analysis; eligibility/provider/platform-selection breadth;
retained evaluator state; a global cache/interner or lock across DICE; a
bootstrap-only/C++-specific path; Rust `cc_internal` or ruleset control flow;
Zabel as authority; caps/allowlist breach; or a material correction to the
accepted architecture.
