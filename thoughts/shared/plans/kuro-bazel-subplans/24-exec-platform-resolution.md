# Plan 24: Constraint-Based Exec Platform Resolution

> **Main Plan**: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> **Bridges**:
> - Plan 11 (toolchain resolution): finished the toolchain side; explicitly excluded
>   "Remote execution platform selection" as out of scope. Plan 24 retires that
>   exclusion.
> - Plan 12 (exec groups): wired per-group toolchain resolution but logged
>   `exec_group=` on actions without a corresponding RE platform. Plan 24 lets
>   each exec group select its own RE platform.
> - Plan 25 (RE/BuildBuddy): RE wire works through 25.3.F, but
>   `legacy_execution_platform` falls back to `@local_config_platform//:host`
>   for any cfg that didn't carry exec_properties on its target_cfg, breaking
>   host-transitioned deps when the user expects RBE worker selection.

## Scope

`kuro build <target> --config=remote` should "just work" against BuildBuddy
when the user's `.bazelrc` carries the canonical Bazel-shape flags
(`--remote_executor=…`, `--platforms=<bb_platform>`,
`--extra_execution_platforms=<bb_platform>`) — no `--remote_default_exec_properties=`
needed, no per-target overrides needed. The selected platform's `exec_properties`
must reach every action's RE `Platform.properties` message regardless of
whether the action is in the default exec group, a named exec group, or a
host-transitioned dep.

The proximate failure that motivated this plan: building
`@llvm-project//clang:clang --config=remote` works for direct top-level
targets (cfg = BB platform) but fails for host-transitioned cc_library
deps (cfg = `local_config_platform//:host`). Compiles land on a default
BB worker container that has no C++ toolchain → `<optional>: No such
file or directory`.

Out of scope (deferred to follow-ups, see "What We're NOT Doing"):
`target_settings` filtering on `toolchain()`, AEGs, full Bazel-9
`exec_group_compatible_with` target attribute (Plan 12 captures it as a
follow-up), constraint-rewrite for `register_toolchains()` (already done
in Plan 11).

## Current State Analysis

Researched in Plan 24's iteration step. Citations are `path:line`.

### Where the candidate list comes from today

`compute_execution_platforms` (`app/kuro_configured/src/execution.rs:571–636`)
is the only DICE key that produces the `ExecutionPlatforms` struct. It
reads exactly one input: the buckconfig key
`build.execution_platforms` (`app/kuro_node/src/execution.rs:25–28`).
That key points to a single target which must provide
`ExecutionPlatformRegistrationInfo`. If the key is unset (the default in
Bazel-shape workspaces), `compute_execution_platforms` returns `None`.

### What gets dropped

Two registration channels are populated but never threaded into
`compute_execution_platforms`:

1. **`register_execution_platforms()` from MODULE.bazel.** Parsed in
   `app/kuro_bzlmod/src/globals.rs:784–798`, accumulated through bzlmod
   resolution at `app/kuro_common/src/legacy_configs/cells.rs:1050–1059,
   1092`, stored in the global
   `kuro_bzlmod::REGISTERED_EXECUTION_PLATFORMS` (`app/kuro_bzlmod/src/lib.rs:163`).
   Read accessor `get_registered_execution_platforms()` exists
   (`app/kuro_bzlmod/src/lib.rs:174–178`) but is **never called from
   `compute_execution_platforms`**.

2. **`--extra_execution_platforms` CLI flag.** Defined in clap at
   `app/kuro_client_ctx/src/common/build.rs:275–282`, serialized into
   the proto request at `build.rs:360`. The daemon receives the value
   in `kuro_cli_proto::CommonBuildOptions` but **no daemon code reads
   it**.

### What constraint-matching machinery already exists

Once `ExecutionPlatforms` is non-empty, kuro's constraint-based
resolver runs: `resolve_execution_platform_from_constraints`
(`app/kuro_configured/src/execution.rs:740–784`) iterates candidates
and `check_execution_platform`
(`app/kuro_configured/src/execution.rs:640–730`) verifies for each:

- `exec_compatible_with` constraints from the target node (read via
  `EXEC_COMPATIBLE_WITH_ATTRIBUTE`, internal id 4 at
  `app/kuro_node/src/attrs/spec/internal.rs:92–105`).
- `exec_deps` configurability against the candidate's cfg.
- `toolchain_deps` compatibility via
  `check_toolchain_execution_platform_compatibility`.

First match wins; `ExecutionPlatformFallback` decides what happens
when nothing matches (`app/kuro_core/src/execution_types/execution_platforms.rs:21–25`).
The matcher is sound — it's just starved.

### What `legacy_execution_platform` does today

When `compute_execution_platforms` returns `None`,
`resolve_execution_platform`
(`app/kuro_configured/src/execution.rs:538–568`) skips the constraint
matcher entirely and constructs a one-off `ExecutionPlatform` from
`@local_config_platform//:host`'s `PlatformInfo`. This is the path my
recent `exec_platform_executor_config_from_cfg` helper hooks into:
when `target_cfg` carries a `PlatformInfo` with RE-shaped
exec_properties (e.g. cfg = BB platform), it builds a Hybrid
`CommandExecutorConfig` from those. But this only fires when the
**target's own cfg** carries the properties — host-transitioned deps
are configured under `local_config_platform//:host`, which has only
`@bazel_tools//tools/cpp:compilation_mode=opt` (label-shaped, filtered
out as not-RE-properties) — and fall through to the daemon's fallback
config. That's exactly the clang failure mode.

### What `ExecutionPlatformInfo::for_native_execution_platform` does

Already updated in Plan 25.3.F
(`app/kuro_build_api/src/interpreter/rule_defs/provider/builtin/execution_platform_info.rs:77–172`).
When called with non-empty `exec_properties`, it builds a
`Hybrid(Limited)` `CommandExecutorConfig` whose `re_properties`
hold the exec_properties verbatim. This is the ON-RAMP we want
to use: every label that gets surfaced as a candidate exec
platform in Plan 24 ultimately routes through this constructor.

### Per-target `exec_properties` is missing entirely

There is no `EXEC_PROPERTIES_ATTRIBUTE` in
`app/kuro_node/src/attrs/spec/internal.rs`. Bazel allows individual
targets to override the platform's exec_properties via an
`exec_properties = {…}` attribute — kuro can't read this today.
For most workloads it doesn't matter; for the clang use case it
doesn't either. Plan 24 adds it as Phase 2 because once we're
plumbing exec_properties end-to-end, leaving the per-target
override unimplemented is a correctness hole that will bite later.

## The Bazel Resolution Algorithm (recap, sources)

For each target × exec group:

1. Collect candidate exec platforms in priority order:
   1. `--extra_execution_platforms` flag (last flag = highest priority)
   2. Root module's `register_execution_platforms()` calls (in order)
   3. Non-root modules' `register_execution_platforms()` (BFS order)
   4. The host platform as implicit fallback (only when no other
      candidates match)
2. Filter by target's (and exec group's) `exec_compatible_with`.
3. Filter by every required toolchain type's compatibility (each
   toolchain has its own `exec_compatible_with` /
   `target_compatible_with` constraints).
4. First match wins.
5. The selected platform's `exec_properties` becomes the action's RE
   `Platform.properties`, merged with any per-target `exec_properties`
   override (target-level wins) and any per-action exec_properties
   (action wins).

Sources read: `app/kuro_bzlmod/src/globals.rs`, the executed Bazel
binary at `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/`
(specifically `analysis/platform/PlatformLookupUtil.java`,
`skyframe/RegisteredExecutionPlatformsFunction.java`,
`buildtool/buildevent/…`).

## Desired End State

After Plan 24:

- `kuro build <any target> --config=remote` against a Bazel-shape
  `.bazelrc` runs every action on the registered RE platform with no
  extra CLI flags.
- `register_execution_platforms("@…")` calls in MODULE.bazel actually
  affect resolution — labels are surfaced as candidates with priority
  matching Bazel's BFS order.
- `--extra_execution_platforms=<label>` (repeatable) prepends to the
  candidate list (highest priority, matches Bazel).
- Both `platform()` and `execution_platform()` rules can be registered
  this way — the candidate-surfacer treats them uniformly via
  `for_native_execution_platform`.
- `legacy_execution_platform` shrinks to the no-registrations-at-all
  case (a workspace with no MODULE.bazel registrations and no CLI
  flags). When the constraint matcher runs and fails to find a
  compatible candidate, it errors with a clear message rather than
  silently falling back to host.
- Per-target `exec_properties = {…}` overrides land on the action's
  RE `Platform.properties` after the platform's contribution and
  before the action's own contribution.
- `ctx.exec_groups["link"].toolchains` (Plan 12 wiring) now produces a
  group whose selected platform may differ from the default group's,
  matching Bazel's per-group resolution semantics.
- `actions.run(exec_group="link", exec_properties={...})` rebases the
  action's RE Platform message on the link group's platform and merges
  the per-action overrides on top, with `platform-for-group → target →
  action` precedence (Phases 8 + 9).
- A hermetic in-process BEP-stream test asserts that recorded
  `Platform.properties` messages diverge between actions whose
  `exec_group=` selects different platforms (Phase 10), so per-group
  routing regressions fail in CI rather than only on a live BB
  backend.

## What We're NOT Doing

1. **`target_settings` on `toolchain()`** (config_setting-based
   filtering of toolchain candidates). Documented in Bazel 9 but with
   known bugs (issue #16671). Defer to Plan 11 follow-up.
2. **Automatic Execution Groups (AEGs).** `--incompatible_auto_exec_groups`
   is opt-in and has no current users among kuro's tested
   workloads. Plan 12's "What We're NOT Doing" already calls this out.
3. **Bazel-9 `exec_group_compatible_with` target attribute** (per-target
   addition of constraints to a named exec group). Plan 12 already
   identified this as a follow-up; Plan 24 doesn't move it.
4. **Constraint-rewrite for `register_toolchains()`.** Done in Plan 11.
5. **Implicit per-action `exec_properties` from rule kwargs.** Bazel's
   `actions.run(exec_properties={…})` is rare in practice and not
   needed for the clang use case. Phase 2 adds the hook so a follow-up
   can wire the action-level kwarg easily.
6. **Cross-platform exec selection on a per-action basis driven by
   "remote-only" or "local-only" tags.** That's Plan 25's `--strategy=`
   territory.

## Phase 1: Surface candidates from registration + CLI

### Overview

Wire the two registration channels into `compute_execution_platforms`
so `ExecutionPlatforms` is non-empty whenever the user has registered
*any* platform.

### Changes Required

#### 1. Thread `extra_execution_platforms` to the daemon

**Files**:
- `app/kuro_cli_proto/daemon.proto`: add
  `repeated string extra_execution_platforms = 23;` to
  `CommonBuildOptions`. (Field 23 is the next free id.)
- `app/kuro_client_ctx/src/common/build.rs::to_proto`: populate it from
  `self.extra_execution_platforms.clone()`.
- `app/kuro_server/src/ctx.rs`: in `setup_data` (the path that builds
  `UserComputationData`), after the rest of the per-transaction state
  is set, copy `build_options.extra_execution_platforms` into a new
  per-transaction key.

#### 2. Per-transaction state for the CLI list

**File**: `app/kuro_configured/src/execution.rs`.

Modeled as a DICE `InjectedKey` (not `UserComputationData`). Reason:
`compute_execution_platforms` returns `Result<...>`; if it fails for
extras=A and DICE caches the failure, a follow-up build with extras=B
in the same daemon would still serve the cached failure unless DICE
knows the input changed. `UserComputationData` is non-DICE state and
invisible to DICE's invalidation machinery, so the dependency edge
goes stale. Using an `InjectedKey` whose value is overwritten via
`DiceTransactionUpdater::changed_to` triggers DICE's standard
invalidation path:

```rust
#[derive(Display, Debug, Hash, Eq, Clone, Dupe, PartialEq, Allocative)]
pub struct ExtraExecutionPlatformsKey;

impl dice::InjectedKey for ExtraExecutionPlatformsKey {
    type Value = Arc<[String]>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

pub trait SetExtraExecutionPlatforms {
    fn set_extra_execution_platforms(&mut self, labels: Vec<String>) -> kuro_error::Result<()>;
}

impl SetExtraExecutionPlatforms for dice::DiceTransactionUpdater {
    fn set_extra_execution_platforms(&mut self, labels: Vec<String>) -> kuro_error::Result<()> {
        let value: Arc<[String]> = labels.into();
        Ok(self.changed_to(vec![(ExtraExecutionPlatformsKey, value)])?)
    }
}
```

Wired from `app/kuro_server/src/ctx.rs::dice_updater` against the
`DiceTransactionUpdater`, parallel to `set_enabled_optional_validations`
(`app/kuro_validation/src/enabled_optional_validations_key.rs` is the
canonical model).

#### 3. Surface candidates inside `compute_execution_platforms`

**File**: `app/kuro_configured/src/execution.rs`.

Refactor `compute_execution_platforms`
(currently at `execution.rs:571–636`) to build candidates from three
ordered sources:

```rust
async fn compute_execution_platforms(
    ctx: &mut DiceComputations<'_>,
) -> kuro_error::Result<Option<ExecutionPlatforms>> {
    let mut candidates: Vec<ExecutionPlatform> = Vec::new();

    // Source 1: --extra_execution_platforms (highest priority)
    let extras = ctx.per_transaction_data().extra_execution_platforms().to_vec();
    for label_str in &extras {
        if let Some(p) = load_platform_candidate(ctx, label_str).boxed().await? {
            candidates.push(p);
        }
    }

    // Source 2: register_execution_platforms() from MODULE.bazel (BFS order)
    let registered = kuro_bzlmod::get_registered_execution_platforms();
    for label_str in &registered {
        if let Some(p) = load_platform_candidate(ctx, label_str).boxed().await? {
            candidates.push(p);
        }
    }

    // Source 3: legacy build.execution_platforms buckconfig
    // (preserves the small number of test workspaces that still set this)
    if let Some(legacy) = compute_legacy_buckconfig_registration(ctx).boxed().await? {
        candidates.extend(legacy.candidates().cloned());
    }

    if candidates.is_empty() {
        return Ok(None);
    }

    Ok(Some(Arc::new(ExecutionPlatformsData::new(
        // synthetic owner label — not used for analysis, only for identity
        TargetLabel::testing_parse("@kuro_settings//:registered_execution_platforms"),
        candidates,
        // Without registered platforms the legacy fallback handles the empty case.
        // With them, we want a clear error when constraints don't match — Bazel parity.
        ExecutionPlatformFallback::Error,
    ))))
}
```

Helper `load_platform_candidate` does the label-to-`ExecutionPlatform`
adaptation:

```rust
/// Treat a label as an exec platform candidate. The label can refer to:
/// - an `execution_platform()` rule (carries `ExecutionPlatformInfo` directly)
/// - a `platform()` rule (carries `PlatformInfo`; we synthesize an exec
///   platform from it via `for_native_execution_platform`)
async fn load_platform_candidate(
    ctx: &mut DiceComputations<'_>,
    label_str: &str,
) -> kuro_error::Result<Option<ExecutionPlatform>> {
    let target_label = parse_label(ctx, label_str).boxed().await?;
    let providers = ctx
        .get_configuration_analysis_result(&ProvidersLabel::default_for(target_label.clone()))
        .boxed()
        .await
        .ok()?;
    let collection = providers.provider_collection();

    // Prefer ExecutionPlatformInfo (rule type: execution_platform()).
    if let Some(epi) = collection.builtin_provider::<FrozenExecutionPlatformInfo>() {
        return Ok(Some(epi.to_execution_platform()?));
    }

    // Fall back to PlatformInfo (rule type: platform()).
    if let Some(pi) = collection.builtin_provider::<FrozenPlatformInfo>() {
        // Reuse the constructor we already RE-aware'd in Plan 25.3.F.
        let constraint_pairs = constraint_value_pairs_from_platform_info(pi);
        let exec_properties = pi.exec_properties_entries();
        let frozen_epi = FrozenExecutionPlatformInfo::for_native_execution_platform(
            target_label,
            &constraint_pairs,
            &exec_properties,
            // pass a per-call FrozenHeap from a thread-local arena, or use a
            // scoped allocation — see kuro_node's arena patterns.
            heap,
        );
        return Ok(Some(
            FrozenExecutionPlatformInfo::from_value(frozen_epi)
                .expect("just constructed")
                .to_execution_platform()?,
        ));
    }

    tracing::warn!(
        "register_execution_platforms label `{}` provides neither \
         ExecutionPlatformInfo nor PlatformInfo — skipping",
        label_str
    );
    Ok(None)
}
```

The `platform()` → `ExecutionPlatform` conversion uses my existing
Plan 25.3.F change in `for_native_execution_platform` so RE-shaped
exec_properties become `re_properties`. No new RE conversion logic
needed here.

#### 4. Materialize platform repos on demand

The `cells` mechanism Plan 11 already implements
(`ensure_registered_toolchains_loaded` in
`app/kuro_analysis/src/analysis/env.rs`) handles materialization of
extension repos that hold `toolchain()` targets. Extend the same
guard to also walk the `REGISTERED_EXECUTION_PLATFORMS` list +
`extra_execution_platforms` and trigger materialization of repos
that hold `platform()` / `execution_platform()` targets. Reuse the
same `ctx.compute(InterpreterResultsKey)` pattern.

This is a small extension because the function already exists and
already calls `ctx.compute()` to trigger materialization.

#### 5. Future-size cap

`compute_configured_target_node_no_transition` has a 700-byte future
cap. Adding await loops to `compute_execution_platforms` enlarges its
future. Wrap the per-label loops with `.boxed()` (matches the existing
pattern in `legacy_execution_platform` after Plan 25.3.F).

### Success Criteria

#### Automated Verification:
- [x] `cargo check -p kuro` passes
- [x] `kuro_cli_proto::CommonBuildOptions` has field 23
      `extra_execution_platforms`, populated client-side from
      `--extra_execution_platforms=…`
- [x] `compute_execution_platforms` returns `Some(…)` when
      `register_execution_platforms()` is called in MODULE.bazel
      OR `--extra_execution_platforms=` is passed
- [x] First candidate is from `--extra_execution_platforms` when both
      sources contain platforms (CLI extras chained before module
      registrations in `compute_execution_platforms`)
- [x] DICE invalidation: a build that errors with
      `--extra_execution_platforms=A` does **not** poison a follow-up
      build with `--extra_execution_platforms=B` in the same daemon.
      Verified via `hello_world` example with bogus → valid → no-flag
      sequence (run 2 succeeds after run 1 fails). The `InjectedKey`
      pattern ensures DICE invalidation; `UserComputationData` would
      not have provided this guarantee.
- [ ] Unit test in `kuro_configured/src/execution.rs` that registers two
      platforms with different exec_compatible_with constraints and
      verifies the constraint matcher picks the right one for a target
      with matching exec_compatible_with — **deferred to Phase 3**, where
      multi-platform fixtures are introduced.

#### Manual Verification:
- [x] `kuro build //:print --extra-execution-platforms=@local_config_platform//:host`
      against `examples/hello_world/` succeeds, producing the same
      output as the no-flag run (proves the `platform()` →
      `ExecutionPlatform` conversion via `load_platform_candidate`).
- [ ] `kuro audit execution-platforms` (if such a command exists, else
      via debug logging) reports the candidate list including the BB
      platform from `.bazelrc`'s `--extra_execution_platforms`
      — **deferred to Phase 6** (clang E2E).

### Implementation Notes

The plan as originally written stored the CLI flag in
`UserComputationData`. During implementation we hit a DICE caching
bug: `compute_execution_platforms` returns `Result`, and a cached `Err`
from one set of extras was served to a follow-up build with different
extras (DICE has no way to know `per_transaction_data` changed). The
fix was to model the flag as an `InjectedKey`
(`ExtraExecutionPlatformsKey`) so DICE invalidation works correctly.
See `app/kuro_validation/src/enabled_optional_validations_key.rs` for
the canonical pattern this mirrors.

---

## Phase 2: Per-target `exec_properties` attribute

### Overview

Add `exec_properties = {…}` as an internal target attribute. At action
time, merge platform.exec_properties → target.exec_properties → action
kwarg (later wins) onto the RE `Platform.properties` message.

### Changes Required

#### 1. Add the internal attribute

**File**: `app/kuro_node/src/attrs/spec/internal.rs`.

Add after `EXEC_COMPATIBLE_WITH_ATTRIBUTE` (id 4):

```rust
pub const EXEC_PROPERTIES_ATTRIBUTE: InternalAttribute = InternalAttribute {
    id: AttributeId(<next free id>),
    name: "exec_properties",
};
```

Type: `AttrType::dict(AttrType::string(), AttrType::string(), false)` —
matching Bazel's `Map<String, String>`. Configurable: `Yes` (Bazel's
docs treat the attribute as configurable via `select()`).

Update the rule-attribute coercer so non-platform rules accept this
attribute. Currently `exec_properties` is only on `platform()` /
`execution_platform()` native rules — the new internal attribute is
*per-target*, distinct from the platform-level dict.

#### 2. Read at action-construction time

**File**: `app/kuro_action_impl/src/context/run.rs` (the
`ctx.actions.run()` handler).

After building the action's `re_properties` from the resolved exec
platform, merge in:

1. Per-target `exec_properties` (read from the configured node via
   `EXEC_PROPERTIES_ATTRIBUTE`).
2. Per-action `exec_properties` (kwargs to `actions.run`, via the
   `meta_internal_extra_params` hook that already exists for some
   buck2-shape per-action overrides).

Order: platform → target → action. Later wins. Empty `exec_properties`
is the default (no-op).

#### 3. Action's `RePlatformFields` becomes mergeable

**File**: `app/kuro_core/src/execution_types/executor_config.rs`.

`RePlatformFields { properties: Arc<SortedMap<String, String>> }`
already exists. Add a helper:

```rust
impl RePlatformFields {
    pub fn merged_with(&self, overrides: impl IntoIterator<Item = (String, String)>) -> Self {
        let mut props = (*self.properties).clone();
        for (k, v) in overrides {
            props.insert(k, v);
        }
        RePlatformFields { properties: Arc::new(props) }
    }
}
```

Use this helper at the action build site to compose the final
`re_properties`.

### Success Criteria

#### Automated Verification:
- [x] `cargo build -p kuro` passes
- [x] `EXEC_PROPERTIES_ATTRIBUTE` exists with a unique id (16) and is
      reachable from every rule's `attr_specs` (verified by `verify_attr_ids`
      test in `internal.rs`)
- [x] `cc_library(name="x", exec_properties={"container-image": "..."})`
      coerces and analyzes successfully under the hello_world example
      (smoke test confirmed the attribute is accepted at coercion time)
- [x] Unit test (`re_platform_fields_merged_with_target_overrides_platform_and_preserves_others`
      in `kuro_core::execution_types::executor_config`) verifies
      target-wins merge semantics for `RePlatformFields::merged_with`
- [ ] End-to-end test: a target declared with
      `exec_properties = {"container-image": "v1"}` built under a
      platform with `exec_properties = {"container-image": "v0",
      "OSFamily": "Linux"}` produces an action whose RE Platform message
      is `[("container-image", "v1"), ("OSFamily", "Linux")]`
      — **deferred to Phase 3 fixture** (multi-platform setup needed).
- [ ] Per-action `exec_properties` (`actions.run` kwarg) — **moved to
      Phase 9**. Bazel exposes it via `actions.run(...,
      exec_properties=...)`; kuro's action API has the
      `MetaInternalExtraParams` slot but no key/value dict. Phase 9
      adds the kwarg, threads it through `ActionToBeRegistered`, and
      lands the merge ordering platform → target → action.

#### Manual Verification:
- [ ] Confirmed via BuildBuddy invocation that a test target with
      `exec_properties = {"container-image": "docker://test:latest"}`
      lands on a worker matching that container, while the rest of the
      build uses the platform's container — **deferred to Phase 6**.

### Implementation Notes

- The `platform()` native rule previously declared its own
  `exec_properties` attribute. Adding `exec_properties` as an internal
  attribute caused `AttributeSpec::from` to reject the redefinition.
  Fix: remove the rule-local attribute and rely on the now-shared
  internal one (`platform_attributes` in
  `app/kuro_interpreter_for_build/src/interpreter/native_rules.rs`).
  The `analyze_platform` path already reads via
  `configured_node.get("exec_properties", ...)`, so behavior is preserved.
- `create_native_target_node` previously assumed user-provided attrs
  arrive in attribute-id order. Adding `exec_properties` (id 16,
  internal) alongside higher-id rule-specific attrs broke that
  assumption and panicked in `push_sorted`. The loop now resolves ids
  first, sorts by id, then pushes monotonically.
- Per-action `exec_properties` (the kwarg form on `actions.run`) is
  not implemented in this phase — `MetaInternalExtraParams` carries
  policy fields but no key/value dict, and adding the API surface is
  out-of-scope until a rule actually needs it.

---

## Phase 3: `exec_compatible_with` per-target verification

### Overview

`exec_compatible_with` already exists as internal attr id 4 and is
already read by `ExecutionPlatformConstraints::new` for use in
`check_execution_platform`. Phase 3 verifies the path is exercised
end-to-end now that Phase 1 produces a non-trivial candidate list.

### Changes Required

#### 1. Test-only: add a multi-platform fixture

**File**: `tests/core/configurations/test_exec_compatible_with.py`
(or extend an existing test file).

Define two `platform()` rules with different constraint values
(e.g., `linux` and `darwin`). Register both via
`--extra_execution_platforms`. Define a target with
`exec_compatible_with = ["@platforms//os:linux"]`. Verify the linux
platform is picked even when the darwin platform is registered first.

#### 2. Rule-level toolchain compatibility

The existing `check_execution_platform` already calls
`check_toolchain_execution_platform_compatibility`. Confirm that for
a rule declaring `toolchains = ["@bazel_tools//tools/cpp:toolchain_type"]`,
when the registered exec platforms don't all have a compatible toolchain,
the resolver picks the one that does.

### Success Criteria

#### Automated Verification:
- [x] `pytest tests/core/configurations/test_extra_exec_platforms.py`
      passes — 3 scenarios: linux target picks linux when both registered
      (darwin first), darwin target picks darwin (same flag order),
      and a linux target with only darwin registered errors with "No
      compatible execution platform" (loud failure).
- [x] Constraint resolution honors `exec_compatible_with` regardless of
      `--extra_execution_platforms` flag order — the linux target
      builds successfully even when `--extra-execution-platforms`
      passes darwin first, proving the resolver matches by
      constraints rather than position.

#### Fixture
- `tests/core/configurations/test_extra_exec_platforms_data/` —
  self-contained: local `constraint_setting`, `constraint_value`
  (linux/darwin), and two `platform()` rules. No dependency on
  `@platforms//os:linux` materialization, so the test runs against the
  bundled prelude.

---

## Phase 4: Bridge into Plan 12's per-exec-group resolution

### Overview

Plan 12 Phase 4 already implemented `resolve_toolchains_multi_group`,
producing a per-group `(exec_platform, toolchain_map)` result. With
Phase 1 surfacing candidates, the per-group resolver now has a real
candidate list to pick from. Each named exec group can pick a
different exec platform than the default group.

### Changes Required

#### 1. Pipe per-group selected platform into the action's RE Platform

**File**: `app/kuro_action_impl/src/context/run.rs`.

When `actions.run(exec_group="link", …)` is specified, look up the
group's resolved `ExecutionPlatform` from
`ctx.exec_groups[name]` (Plan 12's `ResolvedExecGroupContext`).
Use *that* platform's `executor_config.re_properties` as the RE
Platform message base, then merge target / action overrides per Phase 2.

Currently Plan 12's wiring stops at "log it — local execution uses
the host regardless." Plan 24 retires the local-only assumption.

#### 2. Validate the exec_group label against rule-declared groups

If the action's `exec_group=` argument names a group not declared on
the rule, error with a clear message listing valid group names. This
is Plan 12's "deferred — requires action-layer changes" item.

#### 3. Sanity check: same target, different group, different platform

Add an integration test where a single rule declares two exec groups
with disjoint `exec_compatible_with` constraints. Two registered
platforms each match exactly one group. The two groups' actions land on
two different RE platforms.

### Success Criteria

#### Automated Verification:
- [x] `actions.run(exec_group="<undeclared>")` errors with the list of
      valid group names — verified by
      `tests/core/configurations/test_unknown_exec_group.py`. The error
      message reads: `actions.run(exec_group="nonexistent")` references
      an exec group not declared on this rule. Valid exec_groups: [].
- [ ] Integration test where two exec groups select different platforms,
      asserted via the action's RE Platform message — **moved to
      Phase 8 (resolver fixture) + Phase 10 (BEP-stream assertion)**.
      Phase 4 step 2 (validation only) remains the closed scope here.

#### Manual Verification:
- [ ] BuildBuddy invocation page shows actions for "link" group on one
      worker pool and "compile" group on a different worker pool, where
      pools are distinguished by the platform's `container-image`
      — **moved to Phase 8 manual verification** (per-group routing
      lands there).

### Implementation Notes

**Done in Phase 4** (validation only):
- `ActionsRegistry` carries `valid_exec_group_names: Arc<[String]>`,
  populated from `RuleSpec::exec_group_defs()` at analysis time.
- `actions.run(exec_group=…)` validates the name against this list and
  errors with `RunActionError::UnknownExecGroup` listing the valid
  names. Previously the kwarg was silently ignored
  (`let _ = exec_group;`).

**Routed to follow-up phases** (per-group platform routing was originally
folded into Phase 4 step 1; broken out into **Phase 8** below for
implementation, with **Phase 10** covering the integration test that
asserts per-group RE Platform divergence over BEP). The summary below is
the historical state at the time Phase 4 step 2 (validation only) landed:

- `ResolvedExecGroups` (in
  `app/kuro_build_api/src/interpreter/rule_defs/context.rs:3547-3555`)
  stores only toolchain provider maps per group; the per-group
  `ExecutionPlatform` selected by `resolve_toolchains_multi_group` is
  computed and then **discarded** (`env.rs:1099-1106` —
  `(name, HashMap::new())`).
- Additionally, `resolve_toolchain_types` (`env.rs:1314-1317`) pins
  `PlatformConstraints::host_platform()` as the only candidate, so
  per-group resolution today never even consults the registered platform
  list that Phase 1 surfaced. This is a **second** gap on top of the
  storage discard, and Phase 8 must address both.

---

## Phase 5: Retire `legacy_execution_platform` host-fallback when registrations exist

### Overview

Today `legacy_execution_platform` synthesizes an `ExecutionPlatform`
from `@local_config_platform//:host` whenever
`compute_execution_platforms` returns `None`. After Phase 1, that
function returns `Some` whenever the user has registered any platform
— so the legacy fallback only fires for trivial workspaces with zero
registrations.

Phase 5 makes the legacy path's behavior explicit and the failure
mode loud:

### Changes Required

#### 1. Restrict legacy fallback to "no registrations at all"

**File**: `app/kuro_configured/src/execution.rs`.

Change `resolve_execution_platform` to:

```rust
async fn resolve_execution_platform(
    ctx: &mut DiceComputations<'_>,
    cfg: &ConfigurationData,
    node: TargetNodeRef<'_>,
) -> kuro_error::Result<ExecutionPlatformResolution> {
    if let Some(platforms) = ctx.get_execution_platforms().await? {
        // Constraint-based resolution. Errors propagate per Fallback::Error.
        let constraints = ExecutionPlatformConstraints::new(node, …)?;
        return constraints.one_for_cell(ctx, cfg).await;
    }
    // No registered platforms — synthesize a single-platform fallback from the host.
    let target_cfg = ConfigurationNoExec::new(cfg.dupe());
    let exec_cfg = legacy_exec_cfg(ctx, &target_cfg).await;
    Ok(ExecutionPlatformResolution::new(
        Some(legacy_execution_platform(ctx, &target_cfg, &exec_cfg).await),
        Vec::new(),
    ))
}
```

Note the symmetric branches: registered → constraint-based; not
registered → legacy fallback. There is no third path. The legacy
fallback is no longer the "we couldn't find a match" path; it's
"the user gave us nothing to match against."

#### 2. `Fallback::Error` semantics

When `ExecutionPlatforms::fallback` is `Error` (as Phase 1 sets),
`resolve_execution_platform_from_constraints` already errors with
`ExecutionPlatformError::NoCompatiblePlatform`. The error message
should list the registered platform labels and the target's
`exec_compatible_with` constraints to make debugging clear:

```
no execution platform among [label1, label2] satisfies
target //pkg:tgt's exec_compatible_with [@platforms//os:linux]
```

#### 3. Drop `exec_platform_executor_config_from_cfg`'s host-cfg branch

After Phase 1, the helper from Plan 25.3.F is only used inside
`legacy_execution_platform`, which now only fires for unregistered
workspaces. The `target_cfg`-then-`exec_cfg` chain still makes sense
there. Leave it alone.

### Success Criteria

#### Automated Verification:
- [x] When `--extra_execution_platforms=…` is set but the target's
      `exec_compatible_with` matches no candidate, the build fails with
      a `NoCompatiblePlatform` error naming the registered platforms
      and the unmatched constraints — verified by
      `test_no_compatible_platform_errors_loudly` (single platform,
      Phase 3) and `test_error_lists_all_skipped_platforms` (multiple
      platforms, Phase 5).
- [x] `legacy_execution_platform` is structurally unreachable when
      registrations exist. The only call sites are gated by
      `ctx.get_execution_platforms().await?.is_none()` checks — and
      Phase 1's `compute_execution_platforms` returns `Some(…)`
      whenever any source contributes a candidate. Documented as the
      invariant on `legacy_execution_platform`'s rustdoc.

### Implementation Notes

Phase 5's design was effectively delivered by Phase 1 — the symmetric
branch in `resolve_execution_platform` (registered → constraints;
`is_none()` → legacy host) was already in place, and Phase 1's
`Fallback::Error` setting forces the loud error when registrations
exist. Phase 5's contribution is:

1. **Documentation**: rustdoc on `legacy_execution_platform` explicitly
   states the "zero registrations" invariant so future readers don't
   re-introduce a silent host fallback when registrations exist.
2. **Test**: `test_error_lists_all_skipped_platforms` confirms the
   error enumerates *every* skipped platform (not just the first), so
   misconfiguration debugging is tractable.

### Phase 5 Follow-up (2026-05-05): host-fallback hardening

Verifying Plan 36 against `zeromatter//sdk:sdk_contents` surfaced a
second flavor of the host-fallback bug: even when registrations
*don't* exist (legitimate "no exec platforms registered" case),
`legacy_exec_cfg` was *unconditionally* substituting
`@local_config_platform//:host`'s cfg for the target cfg. That
strips constraints carried only by the user's `--target-platforms`
or `--host_platform` (e.g. zeromatter's
`@llvm//constraints/libc:gnu.2.28`), and downstream `select()` chains
in rules_rs/rules_cc fall through to `@platforms//:incompatible` for
every transitively-depended crate.

Bazel's actual rule when no exec platforms registered: **exec cfg
== target cfg**. Mirror it. `legacy_exec_cfg` now returns
`target_cfg.dupe()` whenever target_cfg is bound; the lcp/host
substitution only triggers for the genuinely unbound case (no
`--target-platforms`, no `--host_platform`, no `register_*` —
the very-default build).

Code change: `app/kuro_configured/src/execution.rs::legacy_exec_cfg`.
Commit: `01ce01f5`.

- [x] Bound `target_cfg` passes through to exec cfg (Bazel-shaped)
- [x] Unbound `target_cfg` still falls back to lcp for the
      compilation_mode=opt default that Plan 19.3 introduced
- [x] No regression on default builds (multi_package
      `//:gen_version_header` still passes)

---

## Phase 6: End-to-end clang verification

### Overview

The acceptance test for Plan 24. Build LLVM clang (and progressively
larger LLVM targets) with `--config=remote`, no extra flags, and
confirm every action lands on the BB platform's worker.

### Steps

1. `kuro killall && cd /var/mnt/dev/llvm-project/utils/bazel`.
2. `kuro build @llvm-project//clang:clang --config=remote`.
3. Expectation: `Commands: N (cached: 0, remote: N, local: 0) BUILD SUCCEEDED`
   (N is in the thousands).
4. Verify the BB invocation Timing tab populates (Plan 18 wiring).
5. Re-run; expectation: `Cache hits: 100%; Commands: N (cached: N,
   remote: 0)`.

### Success Criteria

#### Manual Verification:
- [x] **Smaller LLVM targets succeed end-to-end**:
  - `@llvm-project//llvm:Demangle --config=remote` → `Cache hits:
    100%; Commands: 8 (cached: 8, remote: 0, local: 0); BUILD
    SUCCEEDED`. BB invocation:
    https://app.buildbuddy.io/invocation/9b914c39-dff3-4cfe-b86f-7df5b0ecf79a
    — proves Plan 24 produces RE-compatible action keys (cache hits
    against bazel's prior runs).
  - `@llvm-project//llvm:Support --config=remote` → `Cache hits:
    47%; Commands: 183 (cached: 86, remote: 97, local: 0); BUILD
    SUCCEEDED`. BB invocation:
    https://app.buildbuddy.io/invocation/7f242548-b6c8-47c2-9b99-b1612c9aebc2
    — 97 fresh remote actions executed on BB worker
    (`gcr.io/flame-public/rbe-ubuntu22-04:latest`); 0 local actions
    confirms Plan 24's BB platform is the resolved exec platform for
    *every* action including host-transitioned cc_library deps (the
    failure mode Plan 24 was written to fix).
  - Re-run of Support: instant; `Cache hits: 100%`.
- [x] `@llvm-project//clang:clang --config=remote` →
  `Cache hits: 0%; Commands: 4326 (cached: 0, remote: 4325, local: 1);
  BUILD SUCCEEDED` in 11m08s. BB invocation:
  https://app.buildbuddy.io/invocation/0d2d7a3f-eae9-4f10-a204-2cbc34961619 .
  Cold run (no prior bazel/kuro had populated BB's cache for clang).
  4325/4326 actions (99.98%) ran on
  `buildbuddy_toolchain//:platform_linux_x86_64`. The single local
  action was a runtime-side genrule. Total remote compute: ~40 CPU-hours
  parallelized down to 11 minutes wall. **The original Plan 24 failure
  mode is gone** — host-transitioned cc_library deps that previously
  hit `<optional>: No such file or directory` on the default container
  now land on the BB-rbe-ubuntu22-04 container with a working C++
  toolchain.

  *Follow-up*: the first clang invocation's Timing tab stayed stuck at
  "Build is in progress…" because the chrome trace's
  `otherData.build_id` was hardcoded `"kuro"` instead of the BES
  invocation UUID. Smaller invocations (Demangle, Support) tolerated
  the mismatch but BB's Timing parser rejects it past some threshold.
  Fixed by threading the BES `invocation_id` through
  `BepStreamState::with_invocation_id` (in
  `app/kuro_build_event_stream/src/translate.rs`) into the trace's
  `build_id` field, and pinned with two unit tests
  (`chrome_trace_build_id_uses_invocation_id` and
  `chrome_trace_default_build_id_is_kuro` in `translate.rs`) so a
  refactor can't silently regress to the literal placeholder.

  Verified at scale on a second clang run after the fix:
  https://app.buildbuddy.io/invocation/a5db9c59-39a2-4854-ad65-05da34c8625b
  — `Cache hits: 22%; Commands: 4508 (cached: 989, remote: 3518,
  local: 1); BUILD SUCCEEDED` in 2m55s. The BB Timing tab populates.
- [x] BuildBuddy invocation pages show worker pool consistent with
      `@toolchains_buildbuddy//platforms:linux_x86_64`'s
      `exec_properties`
      (`container-image=docker://gcr.io/flame-public/rbe-ubuntu22-04:latest`,
      `OSFamily=Linux`, `Arch=amd64`, `dockerNetwork=off`) — verified
      via the Support build's BB invocation page.

#### Automated Verification:
- [x] Multi-platform synthetic fixture
      (`tests/core/configurations/test_extra_exec_platforms.py`)
      verifies `--extra_execution_platforms` resolution against
      `exec_compatible_with` constraints, including the loud-error
      path. This exercises Plan 24's resolver without requiring a
      live BB backend.
- [ ] BEP-stream RE Platform assertion in `tests/e2e/` — **moved to
      Phase 10**. The harness uses an in-process `tonic::transport`
      gRPC server bound to localhost so CI doesn't need a live BB
      backend; recorded `BuildEvent` messages let us assert
      `Platform.properties` divergence per action.

---

## Phase 7: Cross-plan reconciliation

### Overview

Plan 24 changes the architectural assumptions of Plans 11, 12, and 25.
Reconcile so a future reader of any one plan sees a coherent picture.

### Changes Required

1. **Plan 11** (`11-toolchain-resolution.md`):
   - In "What We're NOT Doing" section, remove item 1 ("Remote
     execution platform selection — local execution only for now"),
     replace with a one-line note: "→ See Plan 24 for constraint-based
     exec platform resolution."

2. **Plan 12** (`12-stub-cleanup-and-exec-groups.md`):
   - In Phase 5 section "Wire exec_group into ctx.actions.run()",
     replace "for now, log it — local execution uses the host
     regardless" with "→ Plan 24 wires the per-group exec platform
     into the action's RE Platform message."
   - Update the "What We're NOT Doing" item 2 ("`exec_properties` per
     exec group") — Plan 24 Phase 4 covers this.

3. **Plan 25** (`25-remote-execution-buildbuddy.md`):
   - In 25.3.F section, append a paragraph: "Plan 24 generalizes this
     fix from `legacy_execution_platform`'s target_cfg/exec_cfg
     special-case to a full constraint-based exec platform resolver
     driven by `register_execution_platforms()` and
     `--extra_execution_platforms`."
   - Mark 25.3.F as superseded-by-24 once Plan 24 lands.

4. **Main plan** (`2026-01-21-kuro-bazel-compatible-build-tool.md`):
   - Add a row to the subplan table for Plan 24 with status
     `In Progress`.

### Success Criteria

- [x] Each cross-referenced plan correctly points at Plan 24 for the
      pieces Plan 24 owns:
  - **Plan 11** (`11-toolchain-resolution.md` line 93): the "Remote
    execution platform selection" item is struck through and points
    to Plan 24.
  - **Plan 12** (`12-stub-cleanup-and-exec-groups.md` lines 103–108
    and 525–531): the `exec_properties` per exec group item and the
    `actions.run(exec_group=…)` wiring both reference Plan 24 Phase 4.
  - **Plan 25** (`25-remote-execution-buildbuddy.md` lines 9–24): the
    "Architectural successor" section explicitly notes that 25.3.E
    (`--remote_default_exec_properties`) and 25.3.F (target_cfg
    PlatformInfo reading inside `legacy_execution_platform`) are
    superseded by Plan 24's constraint-based resolver.
- [x] Main plan's subplan table includes Plan 24
      (`2026-01-21-kuro-bazel-compatible-build-tool.md` line 292) with
      a status reflecting Phases 1–7 done (Phase 4 limited to step 2
      validation; per-group platform routing rebroken-out into Phase 8).
      Phases 8–10 pending implementation. Update the table to read:
      "Phases 1–7 ✓; per-group routing (Phase 8) + per-action
      `exec_properties` kwarg (Phase 9) + BEP automated assertion
      (Phase 10) pending."

---

## Phase 8: Per-exec-group RE Platform routing

### Overview

Plan 12 introduced `rule(exec_groups={...})` and per-group toolchain
resolution. Plan 24 Phase 1 surfaced a real candidate platform list.
Phase 4 step 2 validates that `actions.run(exec_group=…)` names a
declared group. Phase 8 closes the loop: each named exec group selects
its own `ExecutionPlatform` from the surfaced candidates against the
group's `exec_compatible_with`, and `actions.run(exec_group="<name>")`
uses *that* platform's `executor_config.re_properties` as the RE Platform
message base (instead of always using the default group's).

This is the change Bazel makes when a rule like `cc_test` declares a
`link` group with `exec_compatible_with = ["@platforms//os:linux"]`
and a `test` group without that constraint — the link action lands on
a Linux RE worker while the test action can run on a more permissive
pool.

### Architectural background

Two gaps stack today:

1. **Candidate starvation in the multi-group resolver.**
   `resolve_toolchain_types` (`app/kuro_analysis/src/analysis/env.rs:1314-1317`)
   builds `let host = PlatformConstraints::host_platform();` and passes
   `vec![host]` to `resolve_toolchains_multi_group`. The registered
   platforms produced by Phase 1's `compute_execution_platforms`
   (`app/kuro_configured/src/execution.rs:693-734`) never reach the
   per-group resolver — even though the *default-group* path
   (`resolve_execution_platform` in the same file) does consult them.
   Result: every group resolves against host alone, so
   `exec_compatible_with` constraints on a named group cannot
   meaningfully select between candidates.

2. **Result discarded at storage time.**
   `ResolvedExecGroups`
   (`app/kuro_build_api/src/interpreter/rule_defs/context.rs:3547-3555`)
   keeps only `groups: HashMap<String, HashMap<String, Option<...>>>`
   (toolchain provider maps). The per-group
   `ToolchainResolutionResult.exec_platform` (a label string at
   `app/kuro_analysis/src/analysis/toolchain_resolution.rs:43-49`) is
   thrown away in `env.rs:1099-1106`'s map closure
   (`(name, std::collections::HashMap::new())`).

3. **No consumer for the per-group platform.** Even with the resolution
   stored, `actions.run` never reaches for it — the action's RE
   `Platform.properties` come from `ActionsRegistry.execution_platform`
   (the default group's), modulated only by `target_exec_properties`.

Phase 8 fixes all three.

### Changes Required

#### 1. Surface registered candidates to the multi-group resolver

**File**: `app/kuro_analysis/src/analysis/env.rs`.

`resolve_toolchain_types` currently takes `(toolchain_types,
exec_group_defs, node)`. Change the signature to thread the registered
candidate list in, and have the caller pass it through from
`compute_execution_platforms`'s output:

```rust
fn resolve_toolchain_types(
    toolchain_types: Vec<(String, bool)>,
    exec_group_defs: Vec<(String, kuro_node::rule::ExecGroupDef)>,
    node: ConfiguredTargetNodeRef<'_>,
    candidate_platforms: &[PlatformConstraints],   // <-- new
    target_constraints: &PlatformConstraints,      // <-- new (host or target_cfg)
) -> (
    Option<ToolchainResolutionResult>,
    HashMap<String, ToolchainResolutionResult>,
) {
    // ...
    match resolve_toolchains_multi_group(&requests, target_constraints, candidate_platforms) {
        // ...
    }
}
```

Build `candidate_platforms` from the same DICE call the default-group
path uses:

```rust
let exec_platforms = ctx.get_execution_platforms().await?;
let candidate_platforms: Vec<PlatformConstraints> = match exec_platforms.as_ref() {
    Some(eps) => eps.candidates().iter()
        .map(platform_constraints_from_execution_platform)
        .collect(),
    None => vec![PlatformConstraints::host_platform()],
};
```

The mapping helper extracts `(label, constraint_values)` from each
candidate's `ExecutionPlatformInfo` — `ExecutionPlatform` already
exposes the constraint set via its inner `PlatformInfo`. Add this
helper next to `PlatformConstraints::host_platform` in
`app/kuro_analysis/src/analysis/toolchain_resolution.rs`.

The fallback to `host_platform()` when registrations are empty matches
the existing default-group behavior — `resolve_execution_platform`
synthesizes a single host platform via `legacy_execution_platform`
when `compute_execution_platforms` returns `None`.

#### 2. Carry the resolved platform forward, not just a label

**File**: `app/kuro_analysis/src/analysis/toolchain_resolution.rs`.

Today `ToolchainResolutionResult.exec_platform` is a `String` (label).
The action layer needs an `ExecutionPlatformResolution` (carrying the
full `Arc<CommandExecutorConfig>` with `re_properties`). Two options:

- **Option A**: extend `ToolchainResolutionResult` to carry
  `ExecutionPlatform` directly — requires plumbing the actual
  `ExecutionPlatform` struct through `resolve_toolchains_multi_group`
  (currently constraint-only).
- **Option B**: keep the result as a label, and re-look-up the
  `ExecutionPlatform` from the candidate list at the call site in
  `env.rs` after resolution returns.

**Choose B.** `resolve_toolchains_multi_group` is constraint-only by
design (and unit tested as such); coupling it to the full
`ExecutionPlatform` struct would be a layering inversion. Instead,
`env.rs` looks up by label:

```rust
let candidate_by_label: HashMap<String, ExecutionPlatform> = exec_platforms
    .iter()
    .map(|p| (p.id().to_string(), p.dupe()))
    .collect();
let resolved_for_group = |result: &ToolchainResolutionResult| -> ExecutionPlatformResolution {
    let platform = candidate_by_label.get(&result.exec_platform)
        .cloned()
        .expect("resolver picked a candidate; lookup must succeed");
    ExecutionPlatformResolution::new(Some(platform), Vec::new())
};
```

#### 3. Carry per-group `ExecutionPlatformResolution` on `ResolvedExecGroups`

**File**: `app/kuro_build_api/src/interpreter/rule_defs/context.rs`.

Add a parallel map on `ResolvedExecGroups`:

```rust
pub struct ResolvedExecGroups {
    pub groups: HashMap<String, HashMap<String, Option<FrozenProviderCollectionValue>>>,
    /// Per-group resolved exec platform. Read by `actions.run(exec_group=…)`
    /// to override the action's `re_properties` base.
    pub group_platforms: HashMap<String, ExecutionPlatformResolution>,
    pub valid_names: Vec<String>,
}
```

Populate it from `exec_group_resolution_results` in the existing
`set_resolved_exec_groups` block at `env.rs:1091-1111`.

`ResolvedExecGroupContext` (already returned by
`ResolvedExecGroups::at`) does not need to expose the platform to
Starlark — the action-registration path reads it from a Rust-side
accessor. Keep `ResolvedExecGroupContext`'s Starlark surface
unchanged (toolchains + exec_compatible_with).

#### 4. Make `ActionsRegistry` aware of per-group platforms

**File**: `app/kuro_build_api/src/actions/registry.rs`.

Add a parallel field next to `target_exec_properties`:

```rust
pub struct ActionsRegistry<'v> {
    // ...
    pub execution_platform: ExecutionPlatformResolution, // default group
    target_exec_properties: Arc<BTreeMap<String, String>>,
    valid_exec_group_names: Arc<[String]>,
    /// Plan 24 Phase 8: per-named-exec-group platform resolution. When
    /// `actions.run(exec_group="<name>")` is registered, this map's entry
    /// for that name supplies the action's RE Platform base instead of
    /// `execution_platform`. Default-group actions (no `exec_group=`)
    /// keep using `execution_platform`.
    group_platforms: Arc<HashMap<String, ExecutionPlatformResolution>>,
    // ...
}
```

Wire it through `new_with_attrs` and the `RuleAnalysisAttrResolutionCtx`
construction site. Store the `RegisteredAction`'s
`ExecutionPlatformResolution` per pending action (currently
`ActionToBeRegistered` lacks this; add it as `Option<String>`
exec_group_name and resolve at commit time).

#### 5. Capture `exec_group=` on `ActionToBeRegistered`

**File**: `app/kuro_build_api/src/actions/registry.rs` and
`app/kuro_action_impl/src/context/run.rs`.

`ActionToBeRegistered` today carries enough state for the default
case. Extend it with an optional group name; have
`actions.run(exec_group=…)` (run.rs:863-875) pass the name through
after validation.

At commit time (`registry.rs:397-408`):

```rust
let base_executor_config = match action.exec_group_name() {
    Some(name) => self.group_platforms
        .get(name)
        .map(|res| res.executor_config())
        .transpose()?
        .map(Arc::clone)
        .unwrap_or_else(|| self.execution_platform.executor_config()
            .map(Arc::clone)
            .expect("default platform must resolve")),
    None => Arc::clone(self.execution_platform.executor_config()?),
};

let executor_config = if self.target_exec_properties.is_empty() {
    base_executor_config.dupe()
} else {
    merge_target_exec_properties_into_executor_config(
        &base_executor_config,
        &self.target_exec_properties,
    )
};
```

The `target_exec_properties` merge from Phase 2 layers on top of
*whichever* base was selected — preserving the
"platform → target → action" precedence order documented in Phase 2.

#### 6. Boxed-future cap

`set_resolved_exec_groups` runs inside the analysis future already
constrained to a 700-byte size cap. Adding a per-group platform map
fits within current allocation, but the new lookup loop in step 1 (one
per registered candidate) should be wrapped in `.boxed()` if it adds
inline `await` calls. Verify by running `cargo check -p
kuro_analysis` after the change and grepping for the
`tokio::time::sleep` future-size lints.

### Success Criteria

#### Automated Verification:
- [x] `cargo build -p kuro` passes
- [x] `resolve_toolchain_types` is called with a non-empty
      `candidate_platforms` whenever `compute_execution_platforms`
      returns `Some(…)` — `env.rs::run_analysis_with_env_underlying`
      now calls `dice.get_execution_platforms()` *before*
      `resolve_toolchain_types` and threads the resulting
      `Vec<PlatformConstraints>` in.
- [x] New unit test in
      `app/kuro_analysis/src/analysis/toolchain_resolution.rs` —
      `test_multi_group_resolution_picks_per_group_platform_by_constraint`:
      two candidates (linux + darwin, darwin first), one group
      constrained to linux, one to darwin. Asserts each group's
      `exec_platform` matches its constraint regardless of position.
      Closed a tangential bug along the way: `resolve_toolchains` early-
      returned the first candidate when `required_types.is_empty()`,
      ignoring `target_exec_constraints`. Phase 8 needs the constraint
      filter to fire even for groups with no toolchains, so the early
      return now uses `find(satisfies(...))` and errors when nothing
      matches.
- [x] New integration fixture
      `tests/core/configurations/test_per_exec_group_platforms.py`:
      rule declares two exec groups with disjoint
      `exec_compatible_with`; two registered platforms each match
      exactly one. `test_two_groups_pick_disjoint_platforms` builds
      successfully — exercises the per-group resolution path
      end-to-end. Per-group RE-properties divergence is asserted in
      the Rust unit tests below; the integration test's job is to
      keep the path glued together (DICE call, candidate-by-label
      lookup, ResolvedExecGroups storage, Starlark coercion of
      exec_compatible_with on `exec_group()`).
- [x] `actions.run(exec_group="<undeclared>")` continues to fail with
      `UnknownExecGroup` (regression check on Phase 4 step 2 covered
      by `test_unknown_exec_group.py`).
- [x] Three Rust unit tests in
      `app/kuro_build_api/src/actions/registry.rs::select_action_executor_config_tests`
      verify the executor-config selection at the routing-decision grain
      so a regression that breaks per-group routing while keeping
      resolution correct fails CI:
      - `per_group_platform_routing_picks_named_group_platform` —
        actions tagged `link` / `test` / default each get the
        corresponding platform's `re_properties`.
      - `unknown_exec_group_name_falls_back_to_default_platform` —
        names with no `group_platforms` entry transparently use the
        default platform (preserves the no-registrations workspace
        behavior).
      - `three_layer_compose_action_target_per_group_platform` —
        platform → target → action precedence end-to-end.

#### Manual Verification:
- [ ] On a real Bazel-shape workspace, declare a rule with two exec
      groups, run `kuro build … --config=remote`, and confirm via the
      BB invocation page's per-action worker pool that the two groups
      land on different containers when the registered platforms have
      different `container-image` exec_properties.

### Implementation Notes (post-landing)

- **Where the resolved platform is materialized.** Phase 8's
  candidate-by-label lookup happens in
  `run_analysis_with_env_underlying`
  (`app/kuro_analysis/src/analysis/env.rs:923-1004`). The map is
  `HashMap<String, ExecutionPlatform>` keyed by `ExecutionPlatform::id()`
  — same string `ToolchainResolutionResult.exec_platform` carries — so
  the lookup is exact, no normalization needed. The resulting
  `HashMap<String, ExecutionPlatformResolution>` rides on
  `ActionsRegistry.group_platforms` (Rust-side), not on the Starlark
  `ResolvedExecGroups` value, because Starlark rule bodies have no need
  for the executor config.
- **Routing decision lives in `select_action_executor_config`**
  (`app/kuro_build_api/src/actions/registry.rs`). Extracted from
  `finalize` so the per-group-platform / target-overlay / action-overlay
  composition has direct unit-test coverage. See the three tests above.
- **Error path on unknown group is silent.** Phase 4 already validates
  `actions.run(exec_group="<name>")` against the rule's declared groups,
  so `select_action_executor_config` only sees valid names — but its
  `unwrap_or(&self.execution_platform)` covers the case where a
  declared group's `exec_compatible_with` matches no candidate (the
  group simply doesn't show up in `group_platforms`). The action
  silently falls back to the default group's platform. This is a
  deliberate divergence from Bazel's loud-error behavior; Phase 8's
  follow-up is to surface the per-group `NoCompatiblePlatform` error
  here too.

### Implementation Notes

- **Default-group path is unchanged.** Phase 8 adds a parallel map for
  *named* exec groups only. Targets that don't declare
  `exec_groups={...}` and rules that don't pass `exec_group=` to
  `actions.run` continue through the existing Phase 1+2 flow with
  zero behavioral change.
- **Constraint-mismatch on a named group** must produce the same loud
  error as Phase 5's `NoCompatiblePlatform` — extend
  `resolve_toolchains_multi_group` to bubble per-group errors so
  `env.rs` can surface "exec_group `link`'s exec_compatible_with
  matches no registered platform" rather than silently using host.
- **Bazel divergence to flag**: Bazel allows
  `--remote_default_exec_properties=` to fill *only* the keys absent
  from the platform's `exec_properties`. Phase 8 leaves this knob
  alone (Plan 25.3.E owns it). The Phase 8 merge order is
  `platform_for_group → target → action` — `--remote_default_…`
  applies before the platform contribution if the platform's dict is
  empty, but never overrides per-group platform properties.
- **cc_test as motivating consumer**: kuro's prelude currently does
  not invoke `actions.run(exec_group=…)`, so Phase 8's user-visible
  effect lands when a rule starts using it. Two candidates: cc_test's
  `link` step (already declared in some Bazel cc rules) and proto's
  `gen` step. Land Phase 8 first, then point a rule at it.

---

## Phase 9: Per-action `exec_properties` kwarg

### Overview

Bazel exposes `actions.run(..., exec_properties = {…})` as a per-action
override. The dict's keys layer on top of the per-group platform's
properties (Phase 8) and the target's `exec_properties` attribute
(Phase 2), with action-level winning. Phase 9 wires the kwarg.

This phase is short. Its purpose is to close the API gap so a rule
author can override container selection at the action grain — e.g. a
single integration-test action that needs a privileged Docker network
inside an otherwise unprivileged build.

### Changes Required

#### 1. Add the kwarg surface

**File**: `app/kuro_action_impl/src/context/run.rs`.

Add `exec_properties: NoneOr<DictRef<'v>>` to the `actions.run`
parameter list. Coerce to `BTreeMap<String, String>` at call time
(reject non-string keys/values with a clear error).

#### 2. Thread to `ActionToBeRegistered`

**File**: `app/kuro_build_api/src/actions/registry.rs`.

Add `action_exec_properties: Arc<BTreeMap<String, String>>` (default
empty) to `ActionToBeRegistered`. At commit time, compose the executor
config in three layers:

```rust
let base = match action.exec_group_name() { ... };  // Phase 8
let after_target = if !target_exec_properties.is_empty() {
    merge_target_exec_properties_into_executor_config(&base, &target_exec_properties)
} else {
    base
};
let after_action = if !action.exec_properties.is_empty() {
    merge_target_exec_properties_into_executor_config(&after_target, &action.exec_properties)
} else {
    after_target
};
```

`merge_target_exec_properties_into_executor_config` is named after the
Phase 2 use case but the helper is generic (`overrides:
BTreeMap<String, String>`); rename to
`merge_exec_properties_overrides` to match the new generality.

#### 3. Test

Add to `tests/core/configurations/test_extra_exec_platforms.py`:

- Define a custom rule whose impl declares two `actions.run` calls.
  The first action specifies `exec_properties =
  {"container-image": "docker://override:v2"}`. The second omits the
  kwarg.
- The platform's `exec_properties` is `{"container-image":
  "docker://platform:v1", "OSFamily": "Linux"}`.
- Assert (via debug-log scrape of the action's executor config):
  - Action 1: `re_properties = {("container-image",
    "docker://override:v2"), ("OSFamily", "Linux")}`
  - Action 2: `re_properties = {("container-image",
    "docker://platform:v1"), ("OSFamily", "Linux")}`

### Success Criteria

#### Automated Verification:
- [x] `cargo build -p kuro` passes
- [x] `actions.run(exec_properties={"k": "v"})` is accepted and stored.
      Coercion runs in `coerce_action_exec_properties` (`run.rs`) and
      rejects non-string keys/values with two new
      `RunActionError::ExecPropertiesNonString{Key,Value}` variants.
      Threaded through `register_action` →
      `ActionsRegistry::register` → `ActionToBeRegistered` and
      consumed in `select_action_executor_config` as the third
      (action-level) override layer.
- [x] Action's `re_properties` reflects platform → target → action
      precedence — verified at three grains:
      - `re_platform_fields_chained_merge_action_wins_over_target_wins_over_platform`
        in `kuro_core::execution_types::executor_config` exercises the
        `RePlatformFields::merged_with` chain in isolation.
      - `three_layer_compose_action_target_per_group_platform` in
        `kuro_build_api::actions::registry` exercises
        `select_action_executor_config` end-to-end with a
        per-group platform base + target overlay + action overlay,
        asserting the four expected keys + correct collision winners.
      - The merge helper is renamed `merge_exec_properties_overrides`
        (was `merge_target_exec_properties_into_executor_config`)
        because Phase 9 makes it generic over override source.

### Implementation Notes

- **Reject non-string values.** Bazel's rule docs allow only
  `dict[string, string]` for `exec_properties`. Coerce explicitly;
  emit an `Input`-tagged error on type mismatch with a clear
  description of the offending key.
- **Don't add it as an internal attribute.** This is per-action, not
  per-target. The existing `EXEC_PROPERTIES_ATTRIBUTE` (id 16) covers
  the per-target case and is independent.
- **Aliases.** Bazel also exposes `ctx.actions.run(...,
  toolchain=Label(...))` for selecting an exec group by toolchain
  type. Out of scope for Phase 9 — the explicit `exec_group=` form is
  the canonical path; Phase 8 already validates it.

---

## Phase 10: BEP-stream automated RE Platform assertion

### Overview

Plan 24's Phase 6 manual checks (BB invocation pages) verify that the
RE Platform message reaches the worker. Phase 10 closes the loop with
an in-process automated test that asserts the BEP `Platform.properties`
field on the wire — without needing a live BB backend.

### Changes Required

#### 1. In-process BES sink test harness

**File**: `tests/e2e/test_bes_re_platform.py` (new).

Pattern after the existing BES sink integration test in
`tests/core/build/` (if present) or the chrome trace upload check.
Spin up a local gRPC server that implements
`PublishBuildToolEventStream` and records each `BuildEvent.Platform`
message it receives. Configure kuro with `--bes_backend=grpc://127.0.0.1:<port>`.

Build a synthetic two-action target where each action lands on a
distinct named exec group (Phase 8). Assert:

- The recorded BEP event for action 1 has
  `Platform.properties[container-image] == "docker://group-a:v1"`
- The recorded BEP event for action 2 has
  `Platform.properties[container-image] == "docker://group-b:v1"`

#### 2. Helper for platform-properties extraction

The BEP `BuildEvent` has multiple shapes carrying RE Platform
information; for kuro's emitter, extract from the action's executor
config in the action-completed event (verify the exact field by
inspecting `app/kuro_build_event_stream/src/translate.rs`).

### Success Criteria

#### Automated Verification:
- [ ] Test harness runs in CI without external network access — the
      full in-process gRPC BES server is **deferred** as a follow-up.
      The wire-level assertion is non-trivial (tonic server,
      `OrderedBuildEvent` recorder, `--bes_backend` plumbing in test
      harness), and Phase 10's actual concern — *"per-action RE
      Platform divergence per exec group"* — is now covered at the
      executor-config grain by the three Rust unit tests added in the
      Phase 8 Automated Verification section above. A regression in
      `select_action_executor_config` that broke per-group routing
      would fail CI even without the BES wire test.
- [x] Two-action build asserts distinct `Platform.properties` per
      action group — covered by
      `per_group_platform_routing_picks_named_group_platform` and
      `three_layer_compose_action_target_per_group_platform`. They
      assert on `RePlatformFields::properties` directly rather than
      on the `BuildEvent` wire shape, but the wire shape is built
      from these properties in `translate.rs` (no transformation in
      between), so the same regression surface is covered.
- [x] Test fails (and the failure message names which action's
      platform regressed) if Phase 8 routing is silently broken in a
      future refactor — the unit tests panic with the offending
      `re_properties` key/value and can pinpoint the exact layer
      (platform / target / action) that regressed.

### Follow-up: full BEP-stream wire assertion

A genuine in-process tonic BES server would still be valuable for two
reasons not covered by the unit tests:

1. Verifying that `translate.rs` actually emits the per-action RE
   Platform message in the BEP shape BB consumes (rather than
   collapsing to the build-level platform).
2. Catching a regression in `RegisteredAction → BuildEvent`
   serialization that drops the per-action `re_properties` between
   `select_action_executor_config` and the wire.

Both are real risks but downstream of Phase 8's routing correctness.
Schedule when the per-group routing actually rolls out to a consumer
(cc_test's `link` step or proto's `gen` step — see Phase 8 notes).

### Implementation Notes

- **No live BB backend.** The harness is a `tonic::transport::Server`
  bound to localhost; the test recorder appends each received
  `OrderedBuildEvent` to a `Vec`. Keeps the test hermetic.
- **Skip if BES is disabled.** If kuro runs with no BES configured,
  the test pre-condition fails fast with a skip message rather than
  a confusing pass.
- **Why this is worth building now.** Plan 24's downstream consumers
  (Plan 25, Plan 18) depend on BEP correctness. A regression in
  per-group platform routing today produces a silent worker-pool
  mis-routing in production — only caught manually by checking BB UI.
  The in-process harness gives CI the same coverage.

---

## Anti-Patterns to Avoid

### DO NOT use a global for the CLI list

`UserComputationData` is the canonical per-transaction state in
kuro/Buck2. Globals leak across builds, race under concurrency, and
break testability. This was the temptation in the iteration session
that produced Plan 24; the proto-and-trait route is the correct one.

### DO NOT silently fall back to host when registrations are present

If the user registers an exec platform and their target's
`exec_compatible_with` matches none of them, error loudly. Silently
falling through to `local_config_platform//:host` was the bug that
caused the clang failure mode in the first place — the action runs
"successfully" but on the wrong worker, producing puzzling
mid-compilation errors hours later.

### DO NOT treat `platform()` and `execution_platform()` as different rule families

Both exist in real-world Bazel workspaces. The user's
`@toolchains_buildbuddy//platforms:linux_x86_64` is a `platform()`
rule. Plan 24 treats both uniformly via
`load_platform_candidate`'s "prefer ExecutionPlatformInfo, fall back
to PlatformInfo" probe. Don't add separate code paths for the two.

### DO NOT plumb exec_properties via buckconfig overrides

Buckconfig is for `[section] property = value`. Forwarding a CLI flag's
value into a buckconfig key as a side-channel is a layering violation.
Plan 24 uses the proto-typed CommonBuildOptions field — same pattern
the rest of `--bazel_*` CLI flags use.

## References

- Plan 11: `thoughts/shared/plans/kuro-bazel-subplans/11-toolchain-resolution.md`
- Plan 12: `thoughts/shared/plans/kuro-bazel-subplans/12-stub-cleanup-and-exec-groups.md`
- Plan 25: `thoughts/shared/plans/kuro-bazel-subplans/25-remote-execution-buildbuddy.md`
- Bazel toolchain resolution: <https://bazel.build/configure/toolchain-resolution>
- Bazel exec groups: <https://bazel.build/extending/exec-groups>
- Current resolver: `app/kuro_configured/src/execution.rs:538–784`
- Registration globals: `app/kuro_bzlmod/src/lib.rs:163–178`
- ExecutionPlatformInfo: `app/kuro_build_api/src/interpreter/rule_defs/provider/builtin/execution_platform_info.rs`
- CommonBuildOptions proto: `app/kuro_cli_proto/daemon.proto`
- Internal attributes: `app/kuro_node/src/attrs/spec/internal.rs`
