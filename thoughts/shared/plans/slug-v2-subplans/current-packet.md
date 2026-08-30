# Current Slug V2 Packet

Packet: `WP-4-5-7A-subrule-declaration-and-analysis-architecture-r2`

Milestone: M7A bootstrap-critical generic Starlark/ruleset closure.

Base: accepted six-name category architecture `368ef9296`, symbolic-macro and
provider loading `e34cfdc7a`, configured macro namespace enforcement
`541fcfaf2`, and the live loading/configured-analysis owners. All unrelated
dirty Rust work remains parked and read-only.

## Observable result

Correct and freeze one Rust-native architecture plus bounded implementation
series for Bazel 9.2 rule-side `subrule`: declaration/export, lifted hidden
dependencies, typed `configuration_field` defaults, rule attachment,
configured resolution, callable context lifetime, nesting, fragments, and the
zero/one-toolchain contract.

This is a docs/source audit only and authorizes no Rust. Revision one was
`REPLAN`: it gave `configuration_field` the wrong identity, conflated ordered
lifting with set-semantic authorization, did not freeze its first successor,
and implied a larger action/FDO invocation surface than Slug currently owns.
Revision two must close exactly those four blockers and select only the first
loading successor.

## Learned facts and authority

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is sole semantic
authority:

- `StarlarkRuleFunctionsApi.java:1090-1162` and
  `StarlarkRuleClassFunctions.java:2159-2252` define the declaration ABI,
  descriptor validation, set conversion, at-most-one toolchain check, and
  label-deduplicated strictest toolchain requirements;
- `StarlarkLateBoundDefault.java:50-243`, `BazelBuildApiGlobals.java:97-107`,
  and `BzlInitThreadContext.java:30-142` show that `configuration_field`
  resolves a typed fragment class and is cached by fragment class plus tools
  repository, then selects the annotated field name. The calling/defining
  `.bzl` module is not part of identity;
- `StarlarkSubrule.java:74-590` defines export identity, descriptor-order
  hidden lifting, set-semantic transitive discovery/authentication, hidden-name
  source prefixes, context locking/restoration/invalidation, and the exact
  `subrule_ctx` surface;
- `RuleClass.java:715,1364,1892,2420` retains declaration-order input while the
  final attached rule-class subrule collection is set-semantic; and
- `StarlarkSubruleTest` plus `src/test/shell/integration/subrules_test.sh`
  define public declaration, dependency, context, nesting, fragment,
  toolchain, query XML, and invisibility observations.

The first authenticated BCR consumer is rules_cc
`cc/private/rules_impl/fdo/fdo_context.bzl` (SHA-256
`91b7b46c515b4773d5a241e699027212f679ab93160cc79218bd687eac51d5b7`).
It declares `create_fdo_context` with fragment `cpp` and eight private
`attr.label` defaults from `configuration_field`; `cc_toolchain.bzl` (SHA-256
`6a460affdf52e39bcc2ab1d4f7f5f6c135eaae24912a1d1a92f2b5b285321168`)
attaches it through `rule(subrules = [create_fdo_context])`. The implementation
also uses `ctx.label`, `ctx.fragments.cpp`,
`ctx.actions.declare_file/args/run/symlink`, and
`cc_common.absolute_symlink`.

Therefore a token-only global does not cross real loading, while successful
declaration/loading still does not imply complete FDO invocation. The first
successor stops before configured hidden-dependency resolution or any subrule
call.

## Compatibility decision

**Exact:** `.bzl`-only declaration; Starlark-function implementation; export,
repr, and exported equality identity; private label/label-list descriptors;
target/exec cfg validation; required defaults; descriptor-order lifting;
definition-relative literal defaults; `$` ordinary and `:` late-bound hidden
names; typed admitted `configuration_field` producer identity; direct and
transitive rule-side authorization; configured dependency arguments; exact
`subrule_ctx` lifetime and admitted members; declared fragment views; zero/one
toolchain with automatic exec groups; and enclosing-target action ownership.

**Slug-native:** structural Rust identities and equality, evaluator/context
wrappers, valid-Unicode strings, diagnostics not frozen by a discriminating
test, and compact collection/layout choices.

**Unsupported/deferred:** aspect-owned/configured-aspect subrules; rule
inheritance and `ctx.super()` rows; native fragment/field producers other than
explicitly admitted typed rows; finalizer macros; multiple toolchains (invalid
in Bazel 9.2); and complete rules_cc FDO invocation until its action and
`cc_common.absolute_symlink` families are independently admitted. No C++ rule,
parser, or evaluator shortcut is authorized.

## Natural owners and retained representation

### Loading definition and attachment

Add a focused `subrule.rs` loading leaf beside `package.rs`. Its transient and
frozen `SubruleDefinition` owns the implementation function, definition/export
identity, ordered private descriptors, requested fragment set, direct nested
subrule set, and zero/one toolchain requirement. Before export, equality is
evaluator object identity; after export it is canonical extension label plus
exported name. Frozen module lifetime owns executable code.

The existing `FrozenRuleDefinition` retains an ordered direct attachment input
only long enough to produce two distinct facts:

1. a set-semantic direct/transitive subrule identity collection for
   authorization, deduplication, equality, fragments, nested subrules, and
   toolchain publication; and
2. a deterministic ordered sparse suffix of hidden attribute descriptors plus
   per-subrule spans for lifting/query presentation.

Descriptor order follows each definition's `attrs` insertion order. Transitive
discovery is deterministic first encounter over direct declaration order, but
duplicates do not change semantic authorization or publish duplicate hidden
rows. Reordering direct/nested inputs changes deterministic lifted-row
presentation when first-encounter order changes, but it must not change the
authorization/publication set. Ordinary rule attributes retain their current
representation. Hidden rows reuse current descriptor/default/coercion
machinery, remain absent from `ctx.attr`, macro inheritance, and
`native.existing_rules`, and retain `$` versus `:` value-source identity.

### Typed late-bound producer

`configuration_field(fragment, name)` returns a frozen loading value with:

- an admitted typed fragment producer ID corresponding to Bazel's fragment
  class, not a stringly/global lookup;
- the annotated field name; and
- canonical tools-repository identity, because annotation default labels may
  be tools-repository relative.

The defining or calling `.bzl` module is deliberately absent. Two modules that
request the same typed producer, field, and tools repository compare equal;
changing the tools repository or field does not. The first successor admits
only the eight pinned `cpp` FDO producer rows and fails closed for unknown
fragment/field pairs. Later producer additions extend the same finite typed
table with Bazel-source/oracle proof; they do not add a registry or change the
carrier.

Configured structural configuration owns the eventual optional canonical
label and invalidation. A producer value edit leaves package/loading identity
unchanged and must produce A/B/A configured-dependency/result restoration.
The loading carrier is not a label and is never canonicalized/stringified
through the literal-label path.

### Configured invocation and exact member ledger

The existing configured rule analysis key resolves hidden rows through the
ordinary selector/transition/dependency pipeline and publishes them only as
arguments to the owning subrule. `SubruleContext` is phase scratch borrowing
the enclosing rule context, action accumulator, configured owner, and target
label. It is never frozen, DICE-retained, cached, or shared. Entry locks the
enclosing rule context; nested entry saves the caller; every success/error exit
invalidates the callee and restores the caller.

The exact Bazel `subrule_ctx` member ledger is only `label`, `actions`,
`fragments`, and `toolchains`, plus its repr. It does not expose `attr`,
`outputs`, `build_setting_value`, or the enclosing rule's undeclared fragment/
toolchain views. Action ownership remains the enclosing configured target.

The first call packet admits only action members Slug already owns exactly:
`declare_file` and `write`; `run_shell` remains available to ordinary rule
contexts but is not silently claimed for a subrule test that asks for Bazel
`run`. Bazel `args`, `run`, and `symlink`, the subrule-specific rejection of
explicit `toolchain`/`exec_group`, and `cc_common.absolute_symlink` require
their named action-family packets. A subrule FDO replay must stop before the
first unavailable member and may not be reported as complete invocation.

## Ordered implementation series and upstream ledger

1. **Loading declaration and late-bound carrier.** Global visibility,
   instantiation/export/repr/equality, declaration validation, typed FDO
   `configuration_field` values, rule attachment, hidden rows/spans, and
   loading equality. Covers `testSubruleFunctionSymbol_*`,
   `testSubruleInstantiation_*`, attr-length tests, the six declaration
   validation tests at lines 725-800, invalid-fragment declaration,
   at-most-one-toolchain declaration, rule-private-API gating, and exact
   hidden names. No subrule call executes.
2. **Configured hidden dependencies/query.** Literal and admitted late-bound
   label/list defaults, target/exec cfg, providers, single-file/executable
   materialization, override rejection, invisibility from `ctx.attr`, and the
   three rule-side shell query observations. Covers the rule variants at lines
   801-1219; aspect variants remain deferred. Prove configured producer-value
   A/B/A in one DICE graph independently of the loading carrier's stable
   fragment/field/tools-repository identity.
3. **Direct call and base context.** Rule declaration authentication, context
   positional argument, label, `declare_file`/`write`, return values, lock,
   invalidation, and error restoration. Covers rule-side tests at lines
   90-133, 423-593, 658-724. It explicitly excludes Bazel `run` rows.
4. **Nested calls and fragments.** Set-semantic direct/transitive auth,
   duplicate/shared-diamond discovery, arbitrary chains, caller locking/
   restoration, and declared fragment visibility. Covers rule-side tests at
   lines 1452-1899, excluding inheritance rows 199-341.
5. **Automatic-exec-group toolchain view.** Label-deduplicated strictest
   requirements and rule-side zero/one toolchain resolution/action defaulting.
   Covers rule variants at lines 1225-1451 only after automatic exec groups
   have an exact owner. Aspect rows stay deferred.
6. **Action-family integration.** Admit `args`, `run`, `symlink`, explicit
   `toolchain`/`exec_group` rejection, and `cc_common.absolute_symlink` in their
   natural shared action/cc-common packets. Only then run and claim the full
   rules_cc FDO invocation terminal.

Reorder, duplicate-direct, duplicate-nested, and shared-diamond proofs must
separately assert ordered hidden-row presentation and set-semantic
authorization/publication. A new side registry or duplicated dependency/action
pipeline is `REPLAN`.

## First successor frozen contract

After architecture acceptance, activate only
`WP-4-5-7A-subrule-loading-declaration-and-late-bound-carrier`.

Allowed files and frozen baselines at `541fcfaf2`:

- new `app/slug_loading_v2/src/subrule.rs` (absent at base);
- `app/slug_loading_v2/src/lib.rs`, HEAD/worktree blob
  `c124cef749503f362c8e38a6d7df8c09dab7d0e6`;
- `app/slug_loading_v2/src/package.rs`, HEAD blob
  `22a2d01bdb317a2bcc5dd9f6c7ba66a451aa457a`, with the pre-existing
  28-line worktree blob `13bfe7cbc70427baf0dadfa53471c68098eb1599`
  excluded from the packet and retained unstaged;
- new `app/slug_loading_v2/tests/subrule_loading.rs` (absent at base); and
- `app/slug_loading_v2/tests/bzl_invalidation.rs`, HEAD/worktree blob
  `6874ee9341a1b945be9ac6d7c4a7b9c2ee31bf19`.

All other paths are read-only, especially the dirty analysis work,
`registration_expansion_tests.rs`, and `tests/build_file_loading.rs`. Stage
exact hunks; validate an index-isolated candidate before acceptance. Caps are
1,250 production additions, 1,100 proof additions, 2,350 aggregate additions,
and no touched production function above 150 added lines. `package.rs` is over
the 2,000-line trigger, so reusable definition/value/validation/lifting logic
belongs in `subrule.rs`; `package.rs` may contain only global/rule-schema and
evaluation-owner integration.

The successor proves: BUILD exclusion and `.bzl` visibility; export/repr/
cross-module identity; all admitted declaration failures; `$`/`:` names;
descriptor order versus duplicate/reorder/shared-diamond set identity; same
typed field equality across defining modules; tools-repository/field
discrimination; package semantic A/B/A for descriptor, attachment, and typed
producer edits; and exact freeze of the authenticated eight-field FDO
declaration plus `cc_toolchain` attachment. Reuse the authenticated BCR bytes
for a manual source replay; persist only compact synthesized regressions, so no
new oracle fixture or `fixture.toml` is needed.

The first deterministic replay stop is configured analysis of a rule carrying
the attached FDO subrule: it must fail closed at unsupported hidden
late-bound-dependency resolution before invoking `create_fdo_context` or
publishing actions. Any earlier missing-`subrule`, missing-
`configuration_field`, rule-attachment, hidden-name, or package-loading failure
rejects the packet.

Validation: focused new integration test, `bzl_invalidation`, full
`slug_loading_v2 --tests`, formatting, `git diff --check`, isolated staged-tree
test, the authenticated FDO/attachment replay, and one rebuilt-CLI ruleset
replay if the loading binary path changes. No parallel Cargo commands; clean
stale `slugd` around CLI smokes.

## DICE, request, lifetime, and performance

No new DICE key, request overlay, asynchronous task, service cache, or fallback
is authorized. Frozen definitions live with their modules; attached identities
and hidden rows live with package/rule equality; configured labels live with
existing analysis; call contexts are evaluator scratch. Existing package/Bzl
source observations drive invalidation, and no lock crosses a DICE compute.

This changes retained rule/package structures, so every implementation packet
must use the Buck2-utility reuse skill, measure retained per-rule/per-package
rows, share definitions/identities, and retain sparse spans. Correctness and
exact output precede optimization; no benchmark is required without a measured
hot-path regression.

## Zabel peer guidance

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is concept/test
and optimization guidance only. Useful ideas are sparse hidden suffixes with
per-subrule spans, definition-relative canonicalization, typed late-bound
producers, and a subrule context borrowing its enclosing action owner. Copy no
Zig code, layout, allowlist, diagnostics, or parity claims. Bazel 9.2 remains
the only authority.

## Review, docs validation, and stops

Independent correction review must check only: typed producer identity and
cross-module/value-change proof; ordered-versus-set representation and four
graph-shape proofs; the frozen first-successor contract; and the exact context/
action/FDO stop ledger. Run targeted scheduling consistency,
`git diff --check`, and the archive checker. No Cargo is required for this
docs-only packet.

Only this manifest, the canonical plan, Stage 4 plan, and routing history may
change. Stop and `REPLAN` for an untyped/global configuration lookup, defining-
module late-bound identity, a side registry, duplicate rule/dependency/action
pipeline, evaluator values retained outside frozen module ownership, context
escape, Java/Bazel runtime dependency, or any claim of complete FDO/C++
semantics before its named action families are accepted.

## Immediate predecessor

Commit `541fcfaf2` consumes retained symbolic-macro namespace violations at
configured-target admission. Its isolated staged candidate, full analysis
suite, named cquery/build dependents, and independent terminal review pass.
The BCR frontier remains missing `subrule`; `set` stays starlark-rust, while
`cc_common`/`cc_internal` are generic downstream BCR Starlark consumers.
