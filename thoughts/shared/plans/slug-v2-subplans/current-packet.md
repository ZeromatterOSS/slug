# Current Slug V2 Packet

Packet: `WP-4-5-7A-symbolic-macro-and-bzl-provider-key-implementation-r2`

Milestone: M7A bootstrap-critical generic Starlark/ruleset closure.

Base: independently accepted category architecture `368ef9296`, terminally
accepted repository-context implementation `c83e70f0f`, exact universe owner
`cb71a302d`, and the live loading provider/rule/package owners. All unrelated
dirty analysis, core, loading, and REAPI work is parked and read-only.

## Observable result

Implement the first bounded successor of the accepted Bazel 9.2 `.bzl` global
capability architecture:

- expose a real nonconstructible `PackageSpecificationInfo` provider key;
- expose and execute default non-finalizer symbolic `macro` definitions with
  exact declaration, defining-module export identity, attribute projection,
  fresh-evaluator nested package expansion, visibility, retained macro origin,
  and namespace-violation identity;
- correct Slug's `.bzl`/BUILD placement by removing `DefaultInfo` from BUILD
  globals;
- preserve the existing real starlark-rust `set`, `DefaultInfo`, and
  `RunEnvironmentInfo` value owners; and
- share the existing package-loading print/event sink with every fresh macro
  evaluator so macro prints retain source order in the same event batch; and
- rebuild the V2 CLI and run two fresh authenticated rules_rust replays through
  the current `bazel_features` `macro` boundary.

This packet retains a late macro namespace violation on the loaded target and
proves package-load behavior. It does not enforce that violation during
configured-target admission; the accepted next packet owns that one analysis
hunk. It does not implement `subrule`, finalizer macros, provider instances,
package-group `contains`, or any C++-specific parser/rule behavior.

## Authority and compatibility

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is sole semantic
authority. Accepted architecture commit `368ef9296` freezes the owner split and
proof matrix.

**Exact in this packet:** both `.bzl` environments contain the five currently
implemented/added names (`macro`, `PackageSpecificationInfo`,
`RunEnvironmentInfo`, `set`, and `DefaultInfo`) and explicitly lack deferred
`subrule`; BUILD globals contain only universe-owned `set`; nonconstructible
`PackageSpecificationInfo` key identity/type/repr/hash/freeze behavior; default
non-finalizer `macro` declaration/export/invocation; synchronous direct/nested
package expansion in a fresh evaluator; admitted attribute inheritance/default
semantics; macro visibility and target origin; retained namespace violations;
package/frozen-module DICE identity and restoration.

**Slug-native:** Rust types and compact collection choices; structural DICE
identity; evaluator representation; diagnostics except where the proof freezes
an error class or wording as discriminating.

**Unsupported/deferred:** configured-target enforcement of retained namespace
violations until the fixed next packet; macro finalizers/lazy expansion;
`PackageSpecificationInfo` instances and `contains`; `subrule`; subrule
toolchains/fragments/aspects; unadmitted provider fields; `.scl`; `_builtins`;
runtime-selectable Bazel versions; parser/set work; `cc_common`, `cc_internal`,
C++ rules/actions, or a ruleset shortcut. Every invoked deferred lane fails
before publishing new semantic state.

## Implementation contract

### Exact global composition

Keep `complete_loading_globals(bool_config, bzlmod_native)` as the only loading
composition owner. Install `macro`, `PackageSpecificationInfo`,
`RunEnvironmentInfo`, `DefaultInfo`, and later `subrule` only in the
`bool_config` `.bzl` branch. Do not install any of those five in BUILD globals,
`native`, or the process universe. Leave `slug_starlark_v2::populate_universe`
and `SetType` unchanged.

### Nonconstructible provider key

Add a loading-owned frozen/rematerializable `BuiltinProviderKey` in
`provider.rs`. It carries a static name, structural
`ProviderIdentity::builtin(name)`, exact `Provider` type and
`<function NAME>` repr, hash/equality, and no self-call or configured fields.
Use it for `PackageSpecificationInfo`.

Do not introduce a callability enum or generic builtin callable. Migrate a
nonconstructible TestingBootstrap token only if its focused observable matrix
is byte-for-byte unchanged; otherwise do not touch `testing_bootstrap.rs`.
Keep specialized `DefaultInfo`, `RunEnvironmentInfo`, and `OutputGroupInfo`
values distinct.

### Symbolic macro definition and export

Add transient/frozen symbolic macro values beside `RuleDefinitionGen` in
`package.rs`. The transient value owns implementation, defining
`BzlModuleIdentity`, declaration-order effective schema, `finalizer`, `doc`, and
one export-time name cell. Export binds defining label plus exported name once;
freeze/use rejects unexported definitions; imported aliases retain producer
identity.

Reject `finalizer = True` at the typed unsupported boundary in this packet.
Normalize direct and inherited public attributes once using existing
descriptors. Reserve automatic `name`/`visibility`; preserve order,
mandatory/default/configurable policy, explicit `None`, inheritance from rules,
exported macros and `"common"`, and `None` deletion. Reject private direct
attributes, computed/late-bound defaults, unknown inheritance sources, and
unsupported shapes before freezing a definition.

### Package-owned invocation

Invocation is keyword-only and requires the existing `PackageRecorder`.

1. Validate export/invocation/name/schema/defaults and new macro-instance name.
2. Append a compact package-owned record with stable id, parent, definition
   identity/package, name/depth, visibility, and call/generator metadata.
3. Push scratch state referencing that record and construct effective arguments
   in schema order.
4. Create a fresh Starlark evaluator/thread over the same package recorder,
   print/event owner, semantics, and computation-step owner; invoke the frozen
   implementation with named arguments.
5. Require `None` and restore the prior frame on every exit.

Nested macro calls repeat that sequence with child records. Reject recursive
macro-class identity and ordinary name collisions eagerly. Rule/native-rule
recording retains macro creator/origin and defining package, derives Bazel macro
default/actual visibility, and records namespace validity. A namespace
violation does not fail package loading. Do not promise rollback of targets
emitted before a later implementation error without pinned evidence.

Only the active frame and effective arguments are scratch. Frozen definitions,
compact instances, target origin/visibility, and namespace-violation state are
package semantics and participate structurally in equality/invalidation. Add
no macro cache, DICE key, lock, background task, or retained evaluator value.

### Existing event-owner handoff

The R1 preflight proved that starlark-rust exposes a print-handler setter but no
getter or evaluator-context inheritance API. The two BUILD evaluation owners in
`bzl_module.rs` create `LoadingPrintCapture`; a fresh macro evaluator otherwise
falls back to stderr and silently escapes the package event batch.

Move or wrap that existing concrete capture as one shared loading-owned value,
give the `PackageRecorder` an optional clone during the two existing BUILD
evaluator constructions, and install the same capture on each fresh macro
evaluator. Drain the unchanged sink only after the complete BUILD/macro call
tree returns. Preserve exact source order and the current capture-disabled
behavior. This is a carrier correction, not a second event owner: add no DICE
key, event type, buffering layer, public API, or starlark-rust change.

## Frozen proof matrix

Terminal implementation requires:

- six-row inventory over `loading_globals`, `bzlmod_loading_globals`, and
  `build_file_loading_globals`: both `.bzl` routes have the five names admitted
  by this packet and explicitly lack deferred `subrule`; BUILD has only `set`
  and rejects the other five;
- provider key: `type(...) == "Provider"`, exact
  `<function PackageSpecificationInfo>` repr, pinned noncallability/call error,
  hash/equality/provider identity, freeze/rematerialize, and no fabricated
  instance;
- declaration/export: `.bzl`-only binding, value class, unexported freeze/use
  rejection, defining/export identity, imported alias identity;
- schema: automatic name/visibility; every relevant already-admitted attr kind;
  mandatory/default/explicit `None`; private/unknown rejection; rule, exported
  macro, and `"common"` inheritance; deletion/order/configurable promotion;
- invocation: keyword-only, effective order, fresh evaluator, `None` return,
  direct nesting, recursion rejection, success/error frame restoration;
- visibility: top-level default plus call-site package, nested private default
  plus definition/call-site packages, explicit forwarding, and no BUILD package
  default leakage into nested macro declarations;
- ownership/naming: allowed separator rows, eager ordinary collisions, package-
  load success with retained namespace violations, compact parent/depth/source,
  call/generator metadata, and structural equality for every semantic field;
- forbidden operations/errors: package/environment/glob/subpackages/
  existing-rule access at pinned Bazel boundaries, no frame leakage, and pinned
  partial-mutation behavior;
- DICE A/B/A: source definition and BUILD invocation independently invalidate
  and restore through frozen-module/package owners, including origin,
  visibility, and namespace-violation changes; and
- event ownership: with capture enabled, one package attempt records
  BUILD-before, direct-macro, nested-macro, and BUILD-after prints in exact
  execution order in one `EventBatch`; a failing nested macro still drains the
  ordered prefix into the attempt error batch; with capture disabled, neither
  the BUILD nor fresh macro evaluators create or retain a capture sink;
- integration: focused tests, full loading and Bzlmod suites, rebuilt
  `slug_cli_v2`, stale-`slugd` cleanup, then two fresh authenticated rules_rust
  replays that clear `macro` and stop only at the next real boundary or succeed.

Do not weaken a proof row to fit the packet. Stop and `REPLAN` if one cannot be
implemented inside the exact file and cap envelope.

## File allowlist and caps

Only these files may change:

- `app/slug_loading_v2/src/package.rs`;
- `app/slug_loading_v2/src/bzl_module.rs` only for the concrete shared print
  capture and its two existing package-evaluator handoffs;
- `app/slug_loading_v2/src/provider.rs`;
- `app/slug_loading_v2/src/testing_bootstrap.rs` only for proven token
  convergence;
- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- `app/slug_loading_v2/src/bzl_invalidation_tests.rs`;
- this current packet, the canonical plan, Stage 4 plan, routing log/history for
  activation/terminal records only.

Caps are 2,250 production, 2,200 proof, and 4,450 aggregate net added/deleted
lines from this packet base. Moving code counts at the destination.

`package.rs` contains another workstream's exact unstaged baseline: 28
additions/zero deletions, HEAD blob
`39d35aa742d6db24989e6e5ce4a65963bf447d86`, worktree SHA-256
`623bcd93f7a8dde2fad8728ea157e9510b05dedd79a4f1cba5a4ba4a4275f047`.
It adds rule definition-source retention/resolution. Re-audit before every
stage, exclude those hunks byte-for-byte, and commit only packet-owned hunks.

`bzl_module.rs` is clean at R2 activation: HEAD blob
`d2c079a6b630c1abb0ef24aae39041a78282f852`, worktree SHA-256
`aba7f6623b5a38c6bdd5d5ded858cbc87c677ecfdcab9f0bdf381832b05f02ca`.
Only the existing concrete capture definition and the package evaluators near
the current host/root BUILD paths may change; all Bzl/DICE route logic is
read-only.

## Validation and terminal review

Run formatting, focused provider/global/macro/package/DICE tests, full
`cargo test -p slug_loading_v2 --lib --no-fail-fast`, Bzlmod/loading integration
tests, affected dependents, `cargo build -p slug_cli_v2`, both fresh replays,
`git diff --check`, cap accounting, archive status, and exact staged-hunk audit.
Never run shared-target Cargo commands concurrently. Clean stale `slugd` before
and after daemon-sensitive validation.

Independent terminal review must confirm the complete proof matrix, fresh-
evaluator ownership, retained package identity, BUILD placement correction,
provider nonconstructibility, dirty-hunk isolation, caps, and authentic replay
frontier before acceptance.

## Zabel peer guidance

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
concept/optimization guidance only: request-local macro call scope,
definition/export identity, compact retained origin, and reuse of ordinary
package owners. Copy no Zig code, layout, behavior, errors, allowlists, or
claims. Bazel 9.2 remains sole truth.

## Stop conditions

Stop and `REPLAN` for a new DICE/package owner, retained evaluator values,
parallel target/provider identity, transaction model, generic builtin-callable
erasure, starlark-rust modification, file outside the R2 allowlist, cap overflow,
or any need to touch the dirty analysis namespace-enforcement successor now.
Request human design input only if exact fresh-evaluator package expansion
cannot be represented with Slug's existing package/frozen-module owners.

Independent R1 activation review returned `ACCEPT`. The activation audit corrected
the architecture's ambiguous “six-name inventory” wording to an exact six-row
ledger: five names are present after this packet and `subrule` remains absent
until its scheduled declaration owner. No placeholder or compatibility claim
was added. Implementation preflight then returned `REPLAN` before Rust because
the fresh evaluator could not inherit the loading print sink within the
five-file envelope. R2 adds only clean `bzl_module.rs` capture/handoff sites and
150 production/aggregate lines. Initial correction review confirmed the owner
and two call sites but returned `REPLAN` because the frozen matrix omitted event
evidence; R2 now requires exact direct/nested/error/disabled capture rows before
implementation. Focused correction rereview returns `ACCEPT`; implement R2.
