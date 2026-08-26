# Current Slug V2 Packet

Packet: `WP-4-7A-rustfmt-second-aspect-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: docs-only loading/aspect architecture audit
Base: `d4d4d6dc`

Result: authenticate the declaration-time semantics needed by accepted
rules_rust 0.73.0 `rust/private/rustfmt.bzl:152-192`, identify the exact first
unsupported expression after the accepted first aspect, and select one bounded
implementation packet or `REPLAN`. Make no Rust, fixture, oracle or behavior
change.

## Accepted starting point and source order

Commit `d4d4d6dc` freezes `rustfmt_srcs_aspect` with exactly two singleton
provider alternatives and fixed `cpp`, retaining the provider identities from
`rust/private/providers.bzl` through `common.bzl` and the consuming rustfmt
module. Its implementation remains lazy.

Source order then skips the lazy `_rustfmt_aspect_impl` body at lines 129-150
and reaches `rustfmt_aspect = aspect(...)` at lines 152-192. The existing aspect
global accepts `implementation` and `doc`; `attrs` at lines 170-182 is the
first unknown argument. The same fixed call later reuses the now-accepted
provider predicate and fragment, then supplies the first required-aspect edge
at line 187. Its single toolchain expression uses an already-accepted
canonical Label/string handoff. This audit must verify that sequence against
the live evaluator rather than assuming all later arguments are otherwise
complete.

## Required authorities

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
The accepted rules_rust archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Use only those pinned objects and archive.

Inspect at minimum:

- `StarlarkRuleFunctionsApi.aspect` parameter contracts for `attrs`,
  `requires`, provider predicates, fragments and toolchains;
- `StarlarkRuleClassFunctions.aspect` descriptor construction, aspect-specific
  attribute restrictions, required-aspect validation and declaration creation;
- `StarlarkDefinedAspect` export identity, retained attributes, required-aspect
  class ownership, equality/hash and definition construction;
- focused `StarlarkRuleClassFunctionsTest` and
  `StarlarkDefinedAspectsTest` cases for implicit aspect attributes, missing
  defaults, invalid attribute kinds, imported/required aspect identity, cycles
  and ordering; and
- Slug's existing `AttributeDefinitionGen`, `RuleAttributeSchemaGen`,
  `AspectDefinitionGen`, `FrozenAspectDefinition`, caller-aware label-default
  conversion and fail-closed configured-analysis boundary.

Pinned Zabel commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Inspect `build_rule_declaration.zig`'s complete `AspectDefinition`, named
attribute retention and distinct `AspectExportIdentity`, plus only directly
relevant required-aspect consumers. Determine whether Slug can reuse its
existing frozen attribute schema and producer-owned aspect identity without a
side registry or importer rebinding. Do not copy Zig code, representation,
runtime behavior, cache, analysis algorithm or compatibility conclusions;
Bazel remains sole behavior authority.

## Audit questions and compatibility classification

Answer all of these before selecting implementation:

1. Which exact fields from the two fixed private label descriptors survive
   aspect declaration and freeze, and which checks differ from `rule(attrs)`?
2. Does each constructed Label default remain owned by the rustfmt defining
   module, including package and exec-configuration identity?
3. Does `requires = [rustfmt_srcs_aspect]` retain the required aspect object,
   its first-export producer identity, or a derived class key, and when are
   cycles rejected?
4. Can the existing frozen aspect owner retain both fixed attributes and one
   required-aspect edge without executing either implementation or wiring
   configured propagation?
5. What is the first unsupported expression after those fields, if they are
   admitted, and does completing this call require any public/cross-crate
   consumer?

Classify every selected behavior as **exact**, **Slug-native**, or
**unsupported/deferred**. At minimum, aspect application, provider matching,
required-aspect propagation, configured dependencies, `ctx.fragments`,
toolchain selection, actions, later rustfmt declarations, M8/M7B and exact
Bazel configuration/output identity remain deferred unless the audit proves a
smaller declaration-only boundary.

## Ownership, utility and scope gates

Prefer the existing attribute descriptor/schema owner, provider IDs,
`AspectDefinitionGen`/`FrozenAspectDefinition`, canonical labels, compact
strings and Arc slices. Required-aspect identity must remain producer-owned;
an importer, consuming rule, BUILD package or configured-analysis command must
not reconstruct it. Retained values must freeze with the recursive Bzl module
and borrow no evaluator heap or request scratch.

Read the Buck2 utility ledger only if the selected design changes a retained
collection, identity, hashing, clone cost or memory accounting. Record the
reuse decision in the implementation packet. No DICE key, mapping, source
observer, cache, I/O, interner, global registry, lock or async owner is
authorized by this audit.

## Docs-only allowlist and validation

Only these files may change from base `d4d4d6dc`:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

Validate pinned object existence, accepted archive provenance, source line
order, canonical/manifest packet-ID agreement, `git diff --check`, document
structure and file allowlist. A selected implementation packet must include
exact proof, file hashes/line caps, production/test addition caps, function
caps, serial validation, residual risk and STOP conditions. Independent review
is mandatory before commit.

## STOP / `REPLAN`

STOP and `REPLAN` if a bounded declaration-only design cannot preserve
attribute defaults and required-aspect producer identity in the existing
frozen aspect lifetime; if the fixed call requires aspect application,
propagation, configured analysis or actions; if it requires a new DICE key,
registry, cache, I/O path, mapping, interner, hash or lifetime owner; if proof
requires a new oracle/fixture/network request; if behavior depends on Zabel;
or if any Java/JVM component would enter Slug.
