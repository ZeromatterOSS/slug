# Plan 19: Configuration transitions and exec-configuration semantics

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Prerequisites: Plan 11 (toolchain resolution, complete) and Plan 12
> (exec groups, complete) wired the *platform* side of exec-config.
> This plan closes the *build-settings* side: actually applying the
> transition's settings mutation, and letting Starlark rules consume
> the result.

## Scope

Implement real Bazel-compatible configuration transitions. The target
consumer is `cc_library` / `cc_binary` so that tool binaries (e.g.
llvm-tblgen) get compiled in the exec configuration with that
configuration's compile flags (opt mode → `-O2 -g0 -DNDEBUG -ffunction-
sections -fdata-sections`, plus cc_toolchain_config's always-on flags
like `-std=c++17 -fstack-protector -fno-omit-frame-pointer`).

Out of scope: rewriting cc_library's compile-action emission. That
work is already Starlark-shaped. The gap is that the Starlark can't
*see* the current configuration's compilation_mode or *select* among
cc_toolchain_config's flag sets. This plan makes both queryable via
the existing providers + select() machinery.

**Concrete goal.** After this plan lands, running
`kuro build @llvm-project//clang:clang` should produce an llvm-tblgen
binary compiled with the exec configuration's opt-mode flags, cutting
the td_generate critical-path entries from ~280 s to ~30 s each and
closing the ~296 s wall gap vs Bazel measured in
`benchmarks/post-plan-17-fixed-aggregator/FINDINGS.md`.

## Current State Analysis

### What's already wired

- **Attribute-level `cfg`.** `attr.label(cfg="exec")` parses and
  produces `AttrType::exec_dep` instead of `AttrType::dep`
  (`app/kuro_interpreter_for_build/src/attrs/attrs_global.rs:1026`).
  `attr.label(cfg=config.exec(...))` and user-defined transitions are
  parsed into `TransitionId::Target(...)` via
  `attrs_global.rs:423-447` (`transition_dep`).
- **Rule-level `cfg`.** `rule(cfg=transition)` is parsed and stored
  in `Rule.cfg` as `RuleIncomingTransition::Fixed(TransitionId)` or
  `::FromAttribute` (`app/kuro_interpreter_for_build/src/rule.rs:348`,
  `app/kuro_node/src/rule.rs:26`).
- **Transition provider scaffolding.** `transition()` Starlark
  function exists in `app/kuro_transition/src/transition/starlark.rs`.
  `StarlarkTransition` implements `TransitionValue::transition_id()`.
- **Per-target exec platform resolution.** Plan 11 lands real
  toolchain/exec-platform selection based on constraints
  (`app/kuro_configured/src/execution.rs`,
  `app/kuro_configured/src/nodes.rs:910-941`).
  `AttrConfigurationContextImpl` already carries a separate `exec_cfg`
  (`app/kuro_node/src/attrs/configuration_context.rs:45`).
- **Build setting rules.** `rule(build_setting=config.bool(flag=True))`
  stores `build_setting_type` on `Rule` and exposes a flag via
  `--//pkg:target=value` (`Rule.build_setting_is_flag`,
  `app/kuro_node/src/rule.rs:85-87`).

### What's missing

1. **Build settings aren't threaded through ConfigurationData.**
   `ConfigurationData` today is "platform + constraints". It has no
   key-value map of resolved build-setting labels → values. So even
   if you set `--//foo:bar=42` on the command line, rules can't read
   it at analysis time beyond per-rule `ctx.attr.*` for build-setting
   rules themselves.

2. **`--compilation_mode` is parsed-and-dropped.**
   `app/kuro_client_ctx/src/common.rs:261` declares the flag with a
   comment literally saying "Currently accepted but ignored."

3. **`transition()` impl functions don't run.** The Starlark API
   accepts `transition(impl=_my_impl, inputs=[...], outputs=[...])`
   and stores a `TransitionId`. But no code path calls the `impl`
   function to mutate settings. The `cfg` ends up as the incoming
   cfg, unchanged.

4. **No built-in exec transition.** Bazel's `cfg="exec"` corresponds
   to a specific transition that:
    - switches the platform to the chosen exec platform (kuro does),
    - sets `compilation_mode=opt` unless `--host_compilation_mode` says
      otherwise (kuro doesn't),
    - sets `--cpu` to the exec platform's CPU constraint value (kuro
      doesn't),
    - clears `--features` that don't apply in exec (kuro doesn't).

5. **cc_toolchain_config's flag sets aren't selected on compilation_mode.**
   The prelude (or `@rules_cc`'s Starlark) *does* model
   `compile_flags`, `fastbuild_compile_flags`, `opt_compile_flags`,
   `dbg_compile_flags`. Those get resolved via the features system
   based on which "feature" is active. The `opt` / `fastbuild` / `dbg`
   features are activated by the compilation_mode. Since compilation_mode
   isn't a configured value in kuro, none of the mode-specific flags
   get selected.

6. **`ctx.var["COMPILATION_MODE"]`, `ctx.fragments.cpp.compilation_mode`.**
   Bazel rules read compilation_mode through these accessors.
   `ctx.var` is populated with "bin_dir", "BINDIR" but nothing about
   configuration state (`app/kuro_build_api/.../context.rs:939-943`).
   `ctx.fragments.cpp.compilation_mode` would need a configured
   cpp fragment.

### The execution path for a cc_binary tool today

Trace of what happens when kuro builds `llvm-tblgen` (a `cc_binary`
used as a tool by `genrule`):

1. Target graph resolution sees `genrule` depends on `llvm-tblgen`
   via some `tools = [...]` attribute.
2. In Bazel's rules_cc, `tools` is `attr.label_list(cfg="exec")`.
3. Kuro's rules_cc (from the same source) also says `cfg="exec"`.
4. Kuro coerces that to `AttrType::exec_dep(...)`.
5. At dep gathering, `exec_dep` uses `exec_cfg` from the rule's
   exec-platform resolution — kuro replaces the target label's config
   with the exec platform's config.
6. `llvm-tblgen` gets analyzed with `cfg = exec_platform_cfg`.
7. Its dependencies (llvm:Support, llvm:TableGen, etc.) propagate
   this same config.
8. cc_library.impl for each of those produces compile actions.
9. The compile action reads `ctx.toolchains[CPP_TOOLCHAIN]` which
   returns the configured cc_toolchain_config.
10. But step 9 gives the same cc_toolchain_config whether we're in
    target or exec — because nothing about the cc_toolchain_config
    changes based on the cfg. And because compilation_mode isn't in
    the cfg, no mode-specific flag set is selected.
11. Final command passed to gcc: `-fPIC -D...` and nothing else.

### The execution path in Bazel (what we need to match)

1-8 same.
9. cc_library.impl reads `ctx.fragments.cpp.compilation_mode`.
10. For exec-config transitive deps (including the whole dep tree of
    llvm-tblgen), `compilation_mode = "opt"` because the exec
    transition set it.
11. cc_toolchain_config exposes features. The `opt` feature is
    activated when compilation_mode is opt. That feature adds
    `-g0 -O2 -D_FORTIFY_SOURCE=1 -DNDEBUG -ffunction-sections
    -fdata-sections` to the compile args.
12. Final command to gcc: always-on flags (`-fstack-protector -Wall
    -fno-omit-frame-pointer -fno-canonical-system-headers -std=c++17`)
    + opt flags + includes + defines.

Closing the gap is about making steps 9-11 work the same in kuro.

## Desired End State

- `ConfigurationData` carries `build_settings: SmallMap<BuildSettingLabel,
  BuildSettingValue>`. Two configs with the same platform but
  different compilation_mode are distinct.
- `--compilation_mode=opt` on the command line sets
  `@bazel_tools//tools/cpp:compilation_mode = "opt"` on the top-level
  configuration.
- `cfg="exec"` attribute transition sets `compilation_mode="opt"`
  on the exec-side config (matching Bazel's
  `HostTransitionFactory`).
- User-written `transition(impl=fn, inputs=["//:x"], outputs=["//:y"])`
  calls `fn(settings, attr)` during analysis; returned dict values
  replace settings in the outgoing config.
- `select({"@platforms//cpu:x86_64 + @.../cpp:opt": [...], "//conditions:default": [...]})`
  picks branches using build settings.
- cc_toolchain_config's `opt_compile_flags` / `fastbuild_compile_flags` /
  `dbg_compile_flags` / `compile_flags` end up on the compile command
  line correctly based on `--compilation_mode` and the exec transition.

### Key principle: no hardcoding

The *exec transition* defaults must live in rules_cc / cc_toolchain_config
Starlark (as in Bazel), not in kuro's Rust engine. Kuro's Rust layer
only knows:
- How to apply a TransitionId to a ConfigurationData.
- How to resolve exec platforms from constraints.
- How to hash ConfigurationData (so different settings → different
  analysis cache keys).
- How to make `ctx.fragments.cpp.compilation_mode` / `ctx.var` /
  `select()` read from ConfigurationData.build_settings.

The *choice* of "exec transition sets compilation_mode to opt" is a
Starlark default in the toolchain, overridable by user config — same
as Bazel. Never special-case llvm-tblgen or AMDGPU or anything else
in Rust.

## Phases

### 19.1 ConfigurationData carries build settings (OPEN, foundation)

Extend `ConfigurationData` (`app/kuro_core/src/configuration/data.rs`)
to hold `build_settings: Arc<SortedMap<BuildSettingLabel, BuildSettingValue>>`.
`BuildSettingValue` is an enum over the build-setting types kuro
already supports (`bool`, `string`, `int`, `string_list`, `string_set`).

Critical: these settings are part of the hash. Two configs with the
same platform but different settings must hash differently so
analysis cache keys don't collide.

Wire changes:
- `ConfigurationData::new_with_settings(platform, settings)` ctor.
- `.with_setting_override(label, value)` used by transition application.
- `.get_setting(label) -> Option<&BuildSettingValue>` for reads.
- `ConfigurationDataData` proto field for serialization (BEP / event
  log compat).
- `output_hash()` mixes settings into the digest.

Migration: existing `ConfigurationData::new(platform)` defaults to
empty settings. No existing callers break.

Test: two target configs differing only in a synthetic build setting
produce different `output_hash()`.

---

### 19.2 Transition function application (OPEN, core)

Make `transition(impl=...)` actually run. Create a DICE key
`TransitionKey` that takes `(TransitionId, ConfigurationData)` and
returns `ConfigurationData`:

```rust
#[async_trait]
impl Key for TransitionKey {
    type Value = ConfigurationData;
    async fn compute(&self, ctx: &mut DiceComputations) -> Self::Value {
        let transition = ctx.get_transition(&self.transition_id).await?;
        match transition {
            Transition::Target(label) => {
                // User-defined transition. Invoke `impl(settings, attr)`
                // via the Starlark evaluator. Validate outputs list
                // matches the returned dict's keys.
                let impl_fn = load_transition_impl(label);
                let settings_dict = build_settings_dict(&self.cfg, &transition.inputs);
                let attr_struct = ...;
                let result = eval.eval_function(impl_fn, [settings_dict, attr_struct])?;
                apply_result_to_cfg(&self.cfg, result)
            }
            Transition::Builtin(ExecTransition) => {
                apply_exec_transition(&self.cfg, ctx).await
            }
            Transition::Builtin(TargetTransition) => self.cfg.dupe(),
        }
    }
}
```

Wire into dep traversal:
- `AttrConfigurationContextImpl::configure_exec_target` and
  `configure_target` call through `TransitionKey` instead of directly
  using the current `cfg.exec_cfg`.
- `RuleIncomingTransition::Fixed(id)` in
  `kuro_configured/src/nodes.rs:1062-1064` already reads the
  transition id; this phase makes the subsequent configuration lookup
  actually apply the transition.

Note: inputs/outputs validation matters. If a transition declares
`outputs=["@foo//:bar"]` but the returned dict has a different key
set, that's a build error. Match Bazel's exact message.

Test: user-defined transition sets `//:my_flag = "baz"`. The
transitioned dep analyzed with that value in its cfg.

---

### 19.3 Built-in exec transition (OPEN)

Introduce an always-available `TransitionId::Builtin(BuiltinTransition::Exec)`.
Attribute spec `cfg="exec"` resolves to this.

Apply:
1. Determine exec platform via the existing Plan 11 resolution.
2. Set `ConfigurationData.platform = exec_platform_cfg.platform`.
3. Apply **transition defaults from the exec platform's
   `exec_properties`** — specifically the key
   `@bazel_tools//tools/cpp:compilation_mode` defaulting to `"opt"`.
   This is how rules_cc specifies the opt default without kuro
   hardcoding anything.
4. Clear configuration fragments that don't apply in exec (e.g.,
   test-specific fragments).

The "default to opt for exec" policy lives in rules_cc's
`local_config_cc/BUILD` as a `platform(exec_properties={
"@bazel_tools//tools/cpp:compilation_mode": "opt"})`. Kuro just
reads and applies.

Test: `cfg="exec"` attribute's dep has
`compilation_mode = "opt"` in its cfg; default (target) cfg has
`compilation_mode = "fastbuild"`.

---

### 19.4 `--compilation_mode` CLI flag actually populates the cfg (OPEN)

Change `app/kuro_client_ctx/src/common.rs:261` from accepted-and-ignored
to setting `@bazel_tools//tools/cpp:compilation_mode` in the top-level
config's build_settings. Similarly `--host_compilation_mode` controls
the exec transition default (overriding the `"opt"` fallback from
19.3).

Add `--@foo//:bar=value` style flags that directly populate build
settings for user-defined build setting rules (also accepted-and-
ignored today).

Test: `kuro build --compilation_mode=dbg //:foo` produces actions
compiled with dbg_compile_flags.

---

### 19.5 `ctx.var`, `ctx.fragments.cpp.compilation_mode`, `select()` (OPEN)

Expose configuration to Starlark:

- `ctx.var["COMPILATION_MODE"]` reads from
  `ctx.cfg.build_settings[@bazel_tools//tools/cpp:compilation_mode]`
  (existing dict replaced with a ConfigurationData-backed adapter).
- `ctx.fragments.cpp.compilation_mode` — a `CppFragmentInfo` provider
  populated from the configured cc_toolchain. Handle other fragment
  APIs minimally: copts, linkopts, force_pic. The current stub in
  `fragments/cpp.rs` returns hardcoded values.
- `select({"@foo//:flag": "val"})` — make select matching consult
  build_settings when the key is a build-setting-typed label. Today
  select handles config_setting()/constraint_value() matching; add
  build_setting matching to the same resolver.

Test: `ctx.var["COMPILATION_MODE"]` returns `"opt"` for exec-cfg
analysis, `"fastbuild"` for target-cfg analysis.

Test: `select({"@bazel_tools//tools/cpp:opt": "a", "//conditions:default": "b"})`
resolves to `"a"` when compilation_mode is opt.

---

### 19.6 cc_toolchain_config flag-set selection (OPEN)

The actual user-visible win. With 19.1-19.5 landed, rules_cc's
cc_library.impl can already read `ctx.fragments.cpp.compilation_mode`
and feed it into `cc_common.create_compile_variables` or equivalent.

The flag-selection code lives in rules_cc / prelude Starlark. This
phase is the integration test that confirms the chain works end-to-
end on a real build. Tasks:

1. Audit prelude's cc_toolchain_config emission: does it exit today
   with the right feature set (opt/fastbuild/dbg)? If not, port
   from `@rules_cc//cc/toolchains/impl/legacy_converter.bzl`.
2. Wire `cc_common.create_compile_variables(compilation_mode=...)` /
   `cc_common.get_memory_inefficient_command_line(features, variables)`
   — these are the Starlark entrypoints cc_library calls to get the
   final arg list.
3. Confirm the feature system: `features.features` activates "opt"
   when compilation_mode=opt. Active features append their flag sets.

Test: `kuro log what-ran` on a clang:clang cold build shows
`-O2 -g0 -DNDEBUG -ffunction-sections -fdata-sections -std=c++17
-fstack-protector -fno-omit-frame-pointer` on exec-config c_compile
commands.

---

### 19.7 User-written transitions (OPEN, exploratory)

Split-transitions, transitions with multiple output keys, transitions
attached via `attr.label(cfg=transition_obj)`, etc. Bazel has a rich
ecosystem of user-written transitions (e.g., platform-specific
compile modes, bazel_skylib's common_settings).

Needed for: rules_rust test transitions, mobile builds with arch
splits, anything using `config.exec()`/`config.target()` as expressions.

Defer until 19.1–19.6 are green and the harness numbers confirm the
exec-transition fix works.

## Dependencies and ordering

```
19.1 ConfigurationData with build_settings
    │
    ├─► 19.2 TransitionKey applies transitions
    │       │
    │       ├─► 19.3 Builtin exec transition
    │       │       │
    │       │       ├─► 19.4 --compilation_mode CLI flag
    │       │       │
    │       │       └─► 19.5 ctx.var / ctx.fragments / select() reads
    │       │               │
    │       │               └─► 19.6 cc_toolchain_config flag-set selection
    │       │
    │       └─► 19.7 User-written transitions (deferred)
    │
    └─► Incidental: BEP / event-log serialization of cfg settings
        (Plan 18 integration)
```

19.1 is foundational. 19.2-19.6 are the measured-win sequence.
19.7 is only if 19.6 doesn't completely close the Bazel gap.

## Open questions

- **Where do exec_properties defaults live?** Plan assumes rules_cc's
  `local_config_cc/BUILD` declares `platform(exec_properties=...)`.
  If Bazel's behavior is different (e.g., the opt-for-exec default is
  hardcoded in CcConfiguration.java), we need to confirm kuro's
  target-compat story. Checking against Bazel 9 source will settle.
- **Transition hash collisions.** If two transitions produce the
  same output settings, analysis should share a configuration.
  `ConfigurationData::from_settings(…)` must canonicalize-and-dedupe.
- **Select matching on build settings vs constraints.** Today
  config_setting exists; we need build_setting keys to also be
  matchable. This may require extending `ConfigurationInfo` to carry
  build-setting values, not just constraints.
- **Backward compat for empty cfg.** Unbound / unspecified
  configurations must still work (host-platform auto-detection etc.
  from Plan 17's memory notes). Preserve via empty build_settings
  default.

## Success criteria

- Fresh cold `@llvm-project//clang:clang` baseline after 19.1–19.6
  lands:
  - Cold wall within 5 % of Bazel (target: ≤ 1200 s vs Bazel 1131 s).
  - `kuro log critical-path` shows td_generate entries at ~30 s each
    (vs the 200-280 s pre-fix).
  - `kuro log what-ran | grep c_compile | head -1 | tr ' ' '\n' | grep ^-`
    includes `-std=c++17`, `-fstack-protector`, `-fno-omit-frame-pointer`,
    and compilation_mode-derived flags.
- No hardcoded tool-name or binary-path special cases in Rust.
  `cc_library.impl`, `cc_binary.impl`, and cc_toolchain_config
  Starlark are unchanged after the landings (only the kuro Rust
  plumbing that exposes cfg to Starlark changes).
- `kuro log diff summary` between baseline and post-landing shows
  negative delta on `total_wall_us`, `critical_path_wall_us`, and
  on td_generate's per-mnemonic numbers.
- Unit tests:
  - 19.1: cfg with differing settings hashes differently.
  - 19.2: user transition `impl` runs and mutates settings.
  - 19.3: exec transition sets compilation_mode to opt.
  - 19.4: `--compilation_mode=dbg` appears as `ctx.var["COMPILATION_MODE"]`.
  - 19.5: `select()` resolves on a build-setting key.
  - 19.6: prelude cc_library emits opt flags for exec-config deps.

## References

- Investigation that produced this plan:
  `thoughts/shared/research/td_generate-critical-path-investigation.md`
- Baseline showing the 296 s wall gap:
  `benchmarks/post-plan-17-fixed-aggregator/FINDINGS.md`
- Existing transition scaffolding: `app/kuro_transition/src/transition/starlark.rs`
- Existing RuleIncomingTransition enum: `app/kuro_node/src/rule.rs:26`
- Existing exec platform resolution: `app/kuro_configured/src/execution.rs`
- Existing attribute `cfg="exec"` parse site:
  `app/kuro_interpreter_for_build/src/attrs/attrs_global.rs:1026`
- Bazel's local_config_cc flag-set structure (reference):
  `/var/mnt/dev/llvm-project/utils/bazel/external/rules_cc+cc_configure_extension+local_config_cc/BUILD`
