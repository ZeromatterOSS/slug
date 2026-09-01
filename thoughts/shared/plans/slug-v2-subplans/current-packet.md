# Current Slug V2 Packet

Packet: `WP-6-7A-generic-aspect-declaration-architecture-r2`

Milestone: M7A generic Starlark/ruleset closure; Stage 4 frozen module
declarations and the future Stage 6 configured-aspect owner.

Status: R1 independent architecture pre-review returned `REVISE`: the proof
matrix was narrower than the generic private descriptor decision, `requires`
dedup was conflated with Bazel's separate attribute-attached-aspect traversal,
and the exact strictest-duplicate/subrule-union toolchain owner was unnamed.
Corrected R2 expands only those proofs and makes multi-aspect attachment
explicitly fail closed; the retained representation and other boundaries are
unchanged. Focused R2 rereview returns `ACCEPT`; commit this design before
materializing its implementation successor. Commit
`2799030dc` terminally accepts the complete generic rule predeclared-output
category. A rebuilt authentic replay clears it and stops while freezing
rules_rust `rust/private/unpretty.bzl:237`: Slug rejects the aspect's generic
implicit attribute dictionary because loading still admits only the old fixed
rustfmt and clippy schemas.

The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Observable result and compatibility classification

Freeze the architecture for Bazel 9.2's complete default-enabled Starlark
`aspect()` declaration family before implementation. This is one generic
loading abstraction, not the rules_rust dictionary that exposed it. The
implementation successor will replace the fixed rustfmt/clippy capture with a
typed, live/frozen/importable aspect declaration that preserves every admitted
declaration input needed by future configured-aspect evaluation.

Exact declaration-time behavior:

- `implementation` is positional-or-named and must be a Starlark function.
  All other parameters are named-only with Bazel 9.2 defaults;
- `attr_aspects` and `toolchains_aspects` each accept either a fixed sequence
  or a Starlark callback. Fixed entries deduplicate with first encounter order.
  `"*"` must be the sole fixed item; it is forbidden in a dynamic attribute
  result. A private attribute spelling semantically propagates over either
  Bazel computed-default or late-bound native storage. Fixed toolchain entries
  resolve strings through the defining module's repository mapping, while a
  dynamic toolchain callback must later return Label values;
- `attrs` accepts every already-owned descriptor kind. Every private kind
  requires an explicit literal default, while the existing late-bound Label
  kind requires its typed configuration-field default; explicit aspect
  parameters are limited to Boolean, integer, or string descriptors.
  Explicitly setting
  `configurable`, materializing/dormant types, and computed defaults fail at
  declaration time. Public defaults are validated against allowed values, and
  an absent/intrinsic default or `mandatory=True` marks that parameter
  required. Name order, typed defaults, file/provider/aspect policy,
  transitions and required parameter names survive freezing;
- `required_providers` and `required_aspect_providers` accept Bazel's direct
  conjunction or nested disjunction-of-conjunctions over builtin and exported
  user providers. Mixed nesting and nonproviders fail. Each conjunction and
  the alternative set are canonical set-semantic identities. An empty target
  predicate accepts any target, while an empty aspect-provider predicate
  accepts no aspect; the field domain, not a second representation, preserves
  that distinction. Any empty conjunction collapses to the corresponding
  default field meaning;
- `provides` retains the existing ordered, duplicate-free builtin/user
  provider identity;
- `requires` accepts any sequence of aspect values, collapses duplicates by
  declaration identity, retains every direct required aspect, and remains
  frozen/importable. This declaration-time set normalization is separate from
  attribute attachment and later required-aspect traversal;
- `propagation_predicate` is `None` or a Starlark function and is retained.
  `apply_to_generating_rules=True` is rejected with either a nonempty
  `required_providers` declaration or a propagation predicate;
- `fragments`, required `toolchains`, and `exec_compatible_with` retain their
  existing compact typed identities. Aspect toolchains use the existing
  `subrule_toolchain_requirements` parser: list/tuple inputs, repository
  mapping, first-label order, and duplicate-label convergence to the strictest
  mandatory requirement. Toolchains discovered transitively from attached
  subrules are unioned into that aspect requirement set with the same strictest
  rule. Bazel 9.2 accepts and type-checks `host_fragments` but its
  implementation does not add it to the aspect definition, so Slug validates
  and discards it as the same exact no-op;
- `subrules` reuses the accepted transitive `AttachedSubrules` and frozen
  callable ownership. Its discovered hidden attributes and toolchains remain
  aspect-owned declaration facts; and
- `doc` is a string or `None`. `exec_groups=None` and an empty dictionary
  are equivalent. Nonempty `exec_groups` fails closed until the complete
  `exec_group()` declaration category supplies a typed value.

Slug-native behavior:

- immutable `Arc` slices, `CompactString`, `CanonicalLabel`, shared
  `ProviderIdentity`, existing toolchain/subrule carriers, and frozen Starlark
  values form the retained representation. Slug does not claim Java object
  identity, Java serialization, or Bazel checksum bytes; and
- fixed propagation edges preserve source order for deterministic inspection
  while equality is set-semantic. Provider predicates use canonical sorted
  conjunctions/alternatives. Required aspects use first-encounter order for
  traversal but declaration identity for duplicate collapse. Slug retains one
  typed private-attribute edge rather than fabricating Bazel's two internal
  `$`/`:` names; the future consumer must match it against the base schema's
  retained default source; and
- normalized aspect documentation is discarded because Slug has no admitted
  documentation-extraction surface. Declaration-time type validation is exact;
  documentation retrieval remains unsupported/deferred.

Unsupported/deferred behavior:

- configured aspect selection, propagation callback/predicate invocation,
  aspect parameter extraction, required-aspect DAG construction, aspect
  implementation execution, provider publication, toolchain application,
  generating-rule redirection, aspect action ownership, query/cquery/aquery
  aspect projection and REAPI execution remain the Stage 6 configured-aspect
  category. The successor retains their complete declaration inputs but makes
  no execution-parity claim;
- attribute `aspects=[...]` attachment remains limited to the already admitted
  singleton form. Bazel's `AspectsList` recursively inserts required aspects
  before their parent, deduplicates shared transitive diamonds, and rejects
  direct duplicates or a direct aspect appearing after it was already reached
  as required. That complete attachment/traversal category remains fail-closed
  and must later consume the retained direct `requires` lists; this packet
  does not add a partial traversal or claim its ordering/errors;
- nonempty `exec_groups` remains unsupported until one packet admits Bazel
  9.2's `exec_group()` constructor, typed declaration, rule/aspect attachment
  and configured execution projection together. No raw dictionary or frozen
  evaluator value is retained as a bridge;
- experimental build-language-option gating for dynamic propagation and
  subrules is outside the currently admitted command-semantics surface. Their
  declaration shapes may be retained, but Slug does not claim option-toggle
  parity; and
- no parser grammar, `set`, rule body, native/C++ rule, rules_rust branch,
  `cc_common`, `cc_internal`, BCR resolver, DICE key, command, action or
  execution fallback is added. Bazel 9 BCR Starlark owns every rule/aspect
  body; `cc_common` is only a future consumer of the generic host API.

## Bazel 9.2 authority and accepted evidence

Bazel tag `9.2.0` commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
Pinned source SHA-256 values are:

- `StarlarkRuleFunctionsApi.java`:
  `be73dbda0b5a3e8285a05bb732a0a01441f99e8d20dc29b83759ef972c0392ea`;
- `StarlarkRuleClassFunctions.java`:
  `a1f706cfbbc67aa3cd2521df2091dd5ed9af96eb4568049f8eee966d06c622f7`;
- `AspectDefinition.java`:
  `a25f551417466121b56242e9e1b3313f306fa0eef4d7e489284722a41dd71d22`;
- `AspectPropagationEdgesSupplier.java`:
  `7dc4600caca01b928888e95b564790ede9c1b49252565f8d7745cc39e454d4f7`;
- `StarlarkDefinedAspect.java`:
  `5567543ec2ed455cc416aa0d4b612bfe582abdb1117cb29a3420d297c9ea1f6b`;
- `StarlarkRuleClassFunctionsTest.java`:
  `e09c93616e096d639ec69b6b0c6a397a8a36bc8a95fa21b986cb5fc7f8f010aa`;
- `StarlarkAspectsToolchainPropagationTest.java`:
  `fd7ee2aee61ea687377effa214f70631b5841f4e7661d6aaf0f07e8a410cc2a0`;
  and
- `StarlarkAspectsPropagationPredicateTest.java`:
  `d0cdcdbd43a2f6fa8bfb8fc55af49d943a185dd1f27b2fc63111bef7cc76a524`.

The API owns the full call signature and defaults. `StarlarkRuleClassFunctions`
owns attribute validation, both provider predicates, required aspect sets,
propagation suppliers/predicate, toolchains, constraints, subrules and the two
generating-rule conflicts. `StarlarkDefinedAspect` proves declaration
retention, parameter-name ownership and later definition construction;
`AspectDefinition` proves the final typed member domains. The three test
classes supply discriminating success/error themes for
generic private and public attrs, missing defaults, configurable rejection,
provider DNF, multiple required aspects, fixed/callback and wildcard
propagation, toolchain propagation, predicate typing and conflict validation.
No new permanent Bazel fixture is justified: loading-focused Slug regressions
adapt those cases, and the authentic BCR replay is the real imported consumer.

## Learned Slug facts and architecture decision

Slug already retains live/frozen implementation values, defining-module and
export identity, fixed attribute propagation, generic rule attribute schemas,
typed required toolchains/fragments, advertised providers and one required
aspect. It also owns a complete transitive `AttachedSubrules` carrier. The
current gaps are artificial fixed-schema validation, a user-provider-only
two-singleton predicate, single-aspect storage and absent adjacent API fields.

Replace those restrictions with:

- `AspectPropagationEdgesGen<V, T>`, with `Fixed(Arc<[T]>)` and `Callback(V)`,
  instantiated for
  `AspectAttributePropagationEdge::{All, Public(CompactString), Private(CompactString)}`
  and `AspectToolchainPropagationEdge::{All, Type(CanonicalLabel)}`; neither
  wildcard nor private-source expansion is stored as an inferred or magic user
  label;
- `required_aspects: Vec<V>` so the live value remains traceable and the
  frozen value owns every referenced aspect without an evaluator borrow;
- the shared canonical `Arc<[Arc<[ProviderIdentity]>]>` predicate carrier for
  both target and aspect-provider requirements;
- compact required-parameter names, predicate callback, generating-rule bit,
  execution constraints, `AttachedSubrules`, and frozen subrule callables on
  the existing aspect definition; and
- one generic aspect-schema validator over existing `AttributeDefinition` and
  `RuleAttributeSchema`, with no synthetic rule schema, special label table or
  consumer-specific default reconstruction.

Keep the retained aspect definition beside rule/attribute descriptor freezing
in `package.rs`: those private generic types and the defining-module resolver
are its cohesive owner. Extracting only this seam would widen private
interfaces while leaving declaration semantics split. Every new helper must
remain below 140 lines. A future configured-aspect packet may move the complete
type once there is a second production owner; this packet does not preemptively
create a cross-crate aspect crate.

Do not add an opaque exec-group map, second provider identity, aspect registry,
global interner, cache, DICE key, source-text callback, evaluator borrow,
ruleset allowlist or hard-coded rules_rust label.

## Lifetime, incremental ownership, and peer guidance

The frozen defining Bzl module owns implementation and callback code, required
aspect values, subrule callables, and the typed declaration. Fixed lists and
policies share immutable slices. Evaluation-time maps, duplicate sets and
label-conversion buffers are scratch dropped before module publication.
Existing frozen-module/package fingerprints own invalidation and A/B/A
restoration; no request overlay, filesystem observation, asynchronous task,
cache, eviction, cancellation or shutdown behavior changes.

The Buck2-utility decision is reuse: `CompactString`, immutable `Arc` slices,
`SmallSet` construction scratch and `Allocative` are already adopted. No new
Buck2/V1 code, hasher, interner or collection enters the packet.

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer
architecture and optimization guidance only. Its
`src/aspect/session_applied_aspect_node.zig` supports separating immutable
declaration identity from later per-configured-target application keys,
retaining required-aspect topology on the applied-aspect side, and keeping
provider classes typed. Slug adopts those ownership lessons, not Zabel's Zig
rows, allocator, dense IDs, configured-aspect implementation, scheduler,
limits, tests or behavior. Bazel 9.2 remains the oracle.

## Successor implementation boundary, caps, proofs, and stops

After architecture `ACCEPT`, materialize one implementation successor with:

Production allowlist:

- `app/slug_loading_v2/src/package.rs` for the retained type, generic
  validators, binding and freeze; and
- `app/slug_loading_v2/src/subrule.rs` only if the existing attached-subrule
  projection needs a read-only accessor rather than duplicated traversal.

Proof allowlist:

- `app/slug_loading_v2/src/host_package_load_tests.rs`; and
- existing loading integration/invalidation tests only if module publication
  or A/B/A needs a public-boundary proof unavailable in the host suite.

Plan/status allowlist is this manifest, the canonical plan, Stage 6 and the
Stage 9 utility ledger. Caps are 520 net / 680 gross production Rust lines,
520 net / 700 gross proof Rust lines and 1,380 total gross Rust lines. No new
function may exceed 140 lines.

Focused proofs cover:

1. the full positional/named ABI and defaults, wrong types, doc/no-op host
   fragments, empty exec groups and nonempty fail-closed behavior;
2. the complete default-capable private-kind matrix over every already-owned
   descriptor kind, the late-bound Label case, public bool/int/string
   parameters, missing/intrinsic/`mandatory` required-parameter cases,
   bad-allowed defaults, invalid public kinds, configurable/computed rejection,
   typed schema preservation and required parameter names;
3. direct and nested provider predicates over builtin/user identities,
   empty/direct/nested/mixed/error forms, canonical equality, and distinct
   target-versus-aspect fields;
4. zero/one/multiple/duplicate direct required aspects across local and
   imported frozen definitions, plus explicit fail-closed proofs for direct
   duplicate, reverse-required and multi-attachment forms until the complete
   `AspectsList` traversal category;
5. fixed/dynamic attribute and toolchain propagation, wildcard rules,
   repository-mapped labels, retained callbacks and freeze/import identity;
6. propagation predicate, generating-rule conflicts, fragments, list/tuple
   toolchains, optional/mandatory duplicate convergence, repository mapping,
   transitive subrule-toolchain union, constraints, subrules and all retained-
   field discrimination;
7. a same-module or same-DICE A/B/A proving changed declaration inputs
   invalidate and exact restoration cuts off; and
8. rebuilt authentic rules_rust replay clears generic `rust_unpretty_aspect`
   declaration and records the next independent frontier before any ruleset,
   `cc_common`, `cc_internal` or C++ special case.

Validation is serial: focused and complete loading tests, one direct analysis
compile/test dependent, `cargo fmt --check`, Cargo metadata,
`git diff --check`, archive status, pinned source hashes, clean Buck2/Zabel,
parked-file hash, `cargo build -p slug_cli_v2`, stale-`slugd` cleanup and the
authentic replay. Independent architecture pre-review and terminal retained-
representation review are required.

There is no fallback. `REPLAN` before Rust if a provider predicate needs a
second identity, if multiple required aspects require a global registry or
cycle side table, if callbacks/subrules would borrow an evaluator, if generic
aspect attributes cannot reuse the ordinary typed schema without lying about
public parameter policy, or if nonempty exec groups are required by the live
BCR frontier. `REPLAN` during implementation if any admitted field is dropped
across freeze/import, package fingerprinting cannot observe it, the production
cap is exceeded, or replay contradicts pinned Bazel 9.2 evidence.

## Immediate predecessor

Commit `2799030dc` accepts
`WP-6-7A-rule-predeclared-outputs-complete-r3`: generic static/callback rule
outputs produce package-owned generated targets and final compact key/label
facts, feed `ctx.outputs` and synthesized defaults, and pass complete loading,
analysis/query, terminal review and authentic replay gates.
