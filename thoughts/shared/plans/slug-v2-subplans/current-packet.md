# Current Slug V2 Packet

Packet: `WP-4-5-7A-shared-registration-expander-architecture`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `79a36c580`.

Result: at zero Rust, freeze the shared contextual target-pattern, general
package-inventory and loading-owned expansion architecture used by both MODULE
registration families, then select its bounded implementation sequence.

## Immediate predecessor

Commit `79a36c580` accepts the Root/Canonical source-address implementation.
Canonical repository BUILD files, subtree traversal and recursive `.bzl`
loads now use one alias-free route carrier; built-in catalog bytes remain
zero-copy; mapped child loads observe the child route/effect before source;
and root behavior remains exact. The independently reviewed focused, complete
crate, dependent, locked CLI, formatting, cap and archive-baseline gates pass.

That closes the prerequisite named by the accepted registration design. This
packet must not revisit source routing or activate registrations.

## Research basis and learned facts

Pinned Bazel 9.2 is compatibility authority:

- `ModuleFileGlobals.checkAllAbsolutePatterns`, `registerToolchains` and
  `registerExecutionPlatforms` retain absolute raw strings after ignored-dev
  suppression. The default
  `experimental_single_package_toolchain_binding=false`, so recursive MODULE
  toolchain patterns remain admitted in Bazel 9.2.
- `TargetPattern.Parser` resolves `//` under the declaring canonical
  repository, resolves `@apparent` through that declaration's full mapping,
  accepts `@@canonical`, and retains the absolute `:all`, `:*` and
  `:all-targets` wildcard-conflict shape until package lookup.
- `TargetPattern.TargetsInPackage.getWildcardConflict` gives an existing
  same-named explicit target precedence and emits a warning.
- `RegisteredToolchainsFunction` and
  `RegisteredExecutionPlatformsFunction` parse every selected module in
  selected-graph order, expand each family independently and apply different
  loading filters. Explicit targets bypass those filters but must exist.
- `FilteringPolicies.ruleTypeExplicit("toolchain")` retains wildcard targets
  whose associated rule class is `toolchain`.
  `RegisteredExecutionPlatformsFunction.HAS_PLATFORM_INFO` retains explicit
  targets and wildcard platform candidates; aliases can survive for later
  configured resolution.
- `TargetPatternUtil.expandTargetPatterns` folds patterns in input order and
  preserves the first occurrence after duplicate suppression. MODULE rows are
  all positive; signed CLI-option folding is a later input layer.
- `RecursivePkgFunction` contributes child transitive sets before the direct
  package in `STABLE_ORDER`.
  `RegisteredToolchainsFunctionTest.testRegisteredToolchains_targetPattern_order`
  proves lexical sibling traversal, child-before-parent package order and
  lexical target-name order inside each package. This priority-sensitive order
  is exact, not the Slug-native lexical identity of the existing subtree set.

Applicable upstream tests are
`ModuleFileFunctionTest.testRegisterToolchains_singlePackageRestriction_underDir`,
`TargetPatternTest.validPatterns_*`, the expansion matrix in
`TargetPatternUtilTest`,
`RegisteredToolchainsFunctionTest.testRegisteredToolchains_targetPattern_order`,
`testRegisteredToolchains_wildcard_fakeToolchain`, both registered-family
`*_bzlmod` tests, and execution-platform wildcard/alias tests. Existing Slug
oracle evidence already distinguishes wildcard spelling and ambiguity; add an
oracle only for a remaining order, duplicate or family-filter gap.

The live checkout shows two ownership facts the implementation must respect:

- `HostSelectedRegistrationPatterns` already retains raw rows through compact
  route/pattern ordinals over the complete final mapping. It must remain the
  sole declaration/mapping owner.
- `RepositoryPackageLoadKey` currently mixes general external BUILD evaluation
  with restrictions from an earlier configured consumer. A registration
  wildcard needs the complete loaded target inventory, including aliases and
  Starlark rule-class metadata. It may not duplicate BUILD evaluation or
  weaken the old consumer silently.

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer
guidance only. Its selected-row owner, canonical text resolution, compact
package facts, category-specific label builders and stable-postorder recursive
flattening support the same producer/consumer separation. Copy no Zig code,
allocation layout or compatibility claim; Bazel 9.2 owns behavior.

## Architecture to freeze

### Shared contextual syntax

Keep the existing command-facing `TargetPattern` representation and behavior.
Factor its grammar into one identity-owned parser path and add a pure
contextual projection returning canonical pattern parts:

- exact canonical target;
- package wildcard with canonical package, exact wildcard kind and optional
  same-named explicit-target candidate; or
- recursive canonical package with rules-only/all-target classification.

The projection accepts the declaring canonical repository and a borrowed point
lookup over its complete mapping. `//` selects the declaring repository,
`@name` uses that mapping, and `@@name` is already canonical. It performs no
DICE lookup, package read, interning or filesystem access. Parsed values are
compute scratch and are never retained beside the raw selected owner.

Add only a borrowed point-resolution method to
`HostSelectedRegistrationPatternView`; do not copy or republish mappings.

### General package inventory

Split the current external repository package evaluation into one general
Root/Canonical inventory producer plus the existing policy adapter:

- the inventory key owns source, recursive `.bzl`, BUILD evaluation, loaded
  targets, evaluation events and observed epochs exactly once;
- the existing `RepositoryPackageLoadKey` becomes a projection that preserves
  every accepted old restriction/error for current consumers; and
- the registration expander consumes the inventory key directly.

Root packages continue through the accepted root package-load owner. This
split is not a fallback or a second evaluator: both external consumers depend
on the same inventory node. The old policy adapter is deleted only after every
direct command, query and configured-analysis consumer has migrated its policy
to its natural owner and its accepted restriction/error regressions still
pass. Until then its violated general-inventory invariant, deletion condition,
owning consumer migrations and permanence-prevention regressions are explicit.

### Family-independent DICE shape

Add one loading key type parameterized by a semantic
`RegistrationFamily::{Toolchain,ExecutionPlatform}` and keyed by normalized
workspace plus family. Separate family keys prevent a toolchain request from
activating platform packages or errors and vice versa. Each key depends on the
matching iterator of the observed or legacy selected-registration owner.

For each row in selected-module/declaration order, the driver:

1. contextually parses to canonical scratch;
2. obtains the root route for canonical root or the accepted canonical
   load-route for an external canonical repository;
3. obtains the package inventory for exact/package patterns, or the accepted
   subtree set followed by each package inventory for recursive patterns;
4. resolves absolute wildcard-name ambiguity after package load;
5. applies the family filter only to a true wildcard expansion;
6. orders targets lexically by target name; and
7. appends through one first-seen ordered set.

The existing subtree value retains its accepted Slug-native lexical identity.
For registration scratch only, sort its complete package set with a component
comparator that orders lexical sibling subtrees and a descendant before its
prefix ancestor. Prove this is equivalent to Bazel stable postorder for deep,
missing-intermediate-package and root-package cases. Do not add a second
traversal or mutate the retained subtree value.

Cache resolved routes, exact subtree requests and loaded packages only in
compute scratch so repeated/overlapping patterns merge each observation epoch
once. DICE remains the cross-request cache. No manual lock may cross a compute.

The retained semantic value contains one immutable ordered
`Arc<[CanonicalLabel]>` result or typed family/row/lower error plus one ordered
immutable slice of ambiguity-warning facts encountered before that terminal.
Warning facts contain every field needed to render the admitted diagnostic and
retain no evaluator heap or mapping. Complete-only equality includes both the
terminal and warnings; label equality alone cannot cut off a warning change.
Operational event capture derives its local diagnostic batch from those facts.
Cancellation publishes neither value nor batch.

Observed precedence is selected-owner outer failure/Need/semantic error, then
row-order parse, route/effect, subtree and package terminals. Within a row,
route observations precede subtree/package source observations. An ambiguity
warning is a local `EvaluationEvent::Diagnostic(Warning)` stored only with a
complete semantic terminal and replays in row order.

### Family policies and downstream boundary

For both families an exact target must exist, bypasses the wildcard filter and
is retained for configured validation. A wildcard toolchain result retains an
associated rule class exactly named `toolchain`. A wildcard execution-platform
result retains native platform and alias candidates admitted by the loaded
inventory; exact custom advertised-`PlatformInfo` behavior remains deferred
until that provider and `uses_toolchain_resolution` metadata are represented.
Empty family-filter results are allowed when Bazel allows an existing package
to expand to no candidates; a recursive request with no packages preserves the
pinned Bazel error shape.

Configured provider checks, alias resolution, target settings, toolchain
selection, extra command-line registrations and registration activation remain
downstream. CLI extra-toolchain reversal, execution-platform option order and
signed negative folding may reuse the canonical leaf later but are not inputs
to this MODULE key.

This is generic Starlark loading/toolchain-service architecture. Bazel 9 BCR
Starlark owns every rule definition and control-flow layer, including
`cc_internal`. `cc_common` is a demanding client of the reusable evaluator and
host ABI, never a Rust C++ parser or rule implementation. Future builtins stay
grouped by reusable values, declarations, collections/depsets,
labels/patterns, actions/artifacts, configuration/toolchains and
repository/loading services.

## Compatibility classification

- **Exact:** admitted Bazel 9.2 MODULE `//`, `@apparent` and `@@canonical`
  exact/package/recursive patterns; declaration and selected-module order;
  wildcard conflict polarity and warning presence; deep stable-postorder
  package order; lexical target order; family filtering for represented native
  and Starlark `toolchain` classes, native platforms and aliases; first-seen
  duplicate suppression; exact-target existence; and predecessor/route/source
  Need and error precedence.
- **Slug-native:** Rust key/value layout, canonical parser result types,
  Root/Canonical route carrier, scratch caches/comparator, structural hashes,
  observation transport and retained-memory accounting.
- **Unsupported/deferred:** configured providers/settings/alias resolution and
  selection; custom Starlark `PlatformInfo` candidates until their metadata is
  represented; CLI option registrations and signed folding; exact diagnostic
  text outside accepted fixtures; symlink traversal beyond the accepted
  subtree boundary; registration activation; additional host builtin
  categories including `cc_common`; rules and actions.

## Implementation sequence selected by this design

1. `WP-4-5-7A-registration-expansion-prerequisite-owners`: add the contextual
   canonical syntax projection and selected-view point lookup; extract the
   general package-inventory key under the old policy adapter; preserve all
   current command/query/analysis behavior. No registration key yet.
2. `WP-4-5-7A-shared-module-registration-expander`: add both family keys and
   the one shared driver, exact ordering/filter/ambiguity/dedup behavior,
   lifecycle proof and a read-only analysis compile dependent. Do not activate
   configured consumers.
3. Later configured packets consume only expanded canonical labels and own
   provider/settings/alias/selection semantics before the ordinary Stage 10.3
   graph is retried.

## Docs-only allowlist, validation and review

Change only:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
2. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
3. `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

No Rust, Cargo, BUILD, fixture, oracle, lockfile or other plan file is admitted.
Run targeted source/structure checks, `git diff --check`, packet/canonical ID
agreement and the archive-status baseline. Require independent review of DICE
ownership, package-producer split, exact recursive order, family isolation,
warning-bearing equality, retained memory and the generic Starlark/host-ABI
boundary before selecting step 1. Require A/B/A proof where `:all` conflict and
`:*` wildcard forms produce the same ordered labels but only the conflict form
produces the warning; the restored state must restore the warning, while Need
and cancellation publish no value or batch.

## Stops

STOP and `REPLAN` for a second raw declaration or mapping owner; retained
parsed rows; a second BUILD evaluator or recursive traversal; a family-combined
key; root mapping used for a nonroot declaration; apparent spelling in
canonical route identity; direct filesystem IO; lexical recursive priority; a
package restriction left inside the general inventory owner; configured
provider/selection work; a manual lock across DICE; unbounded plan scope; Rust
ownership of BCR rules or `cc_internal`; a C++ parser/rule engine; or treating
Zabel as compatibility authority.
