# Stub Cleanup and Exec Group Resolution

> **Main Plan**: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)

## Overview

Replace remaining stubs in `context.rs` with proper implementations. This covers
two categories:

1. **Rename/cleanup** — stubs that are functionally correct but poorly named
   (BXL/dynamic_output fallbacks, empty CcInfo defaults, artifact roots)
2. **Real implementations** — `BuildConfigurationStub` (reads global state instead
   of per-target configuration) and `ExecGroupsDict` (no per-group toolchain
   resolution, always returns None)

The exec group work is the main effort: extending Plan 11's toolchain resolution
to run independently per exec group, producing per-group execution platforms and
toolchain maps, and exposing them through `ctx.exec_groups["name"].toolchains`.

## Current State Analysis

### Stubs to Rename (Functional, Wrong Names)

**Category A — BXL/dynamic_output fallbacks** (activated only when `attrs` is `None`):
- `CtxFilesStub` → returns empty list for any attribute
- `CtxFileStub` → returns None for any attribute
- `CtxExecutableStub` → returns None for any attribute
- `CtxSplitAttrStub` → returns `{"//conditions:default": None}` for any attribute

These have working counterparts (`CtxFiles`, `CtxFile`, `CtxExecutable`, `CtxSplitAttr`)
used in normal analysis. The stubs are correct behavior for no-attrs contexts.

**Category B — Empty-value defaults**:
- `CompilationContextStub` → empty depsets/lists for all CcCompilationContext fields
- `LinkingContextStub` → empty depset for `linker_inputs`
- `HeaderInfoStubSimple` → empty lists for all header fields
- `ArtifactRootStub` → wraps a path string for `artifact.root`

These represent "empty" instances, not missing implementations.

### Stubs Needing Real Implementations

**`BuildConfigurationStub`** (`context.rs:2928`):
- Always reads from global `BUILD_CONFIG` static (process-wide CLI flags)
- `short_id` is hardcoded `"{cpu}-{mode}"` instead of real config hash
- Missing: per-target configuration data from `ConfigurationData`
- The real `output_hash()` (Blake3 hash of platform constraints) already exists
  on `ConfigurationData` but isn't used here

**`ExecGroupsDict` / `ExecGroupInfo` / `ExecGroupToolchains`** (`context.rs:3043-3146`):
- `ctx.exec_groups["name"]` returns `ExecGroupInfo` for ANY key (never errors)
- `ExecGroupInfo.toolchains["type"]` always returns `None`
- `ctx.actions.run(exec_group="name")` silently discards the parameter
- `rule(exec_groups={...})` stores only dict keys, throws away `ExecGroupValue`
  (toolchains and exec_compatible_with are lost)
- No per-group toolchain resolution exists — Plan 11 resolves only for the
  default exec group (the rule-level `toolchains=[...]`)

### The Bazel 9 Exec Group Algorithm

In Bazel, toolchain resolution runs independently for every exec group (including
the default one). For each exec group:

1. Collect the group's required `toolchain_type`s and `exec_compatible_with` constraints
2. Filter registered execution platforms by the group's `exec_compatible_with`
3. For each (exec_platform, toolchain_type) pair, find first compatible registered
   toolchain (checking both `target_compatible_with` and `exec_compatible_with`)
4. Select first exec platform that satisfies ALL mandatory toolchain types
5. Produce `(exec_platform, {type → resolved_toolchain})` for the group

`ctx.toolchains` reads from the default exec group. `ctx.exec_groups["name"].toolchains`
reads from the named group's resolution. Named exec groups can select different
execution platforms than the default group.

### Bazel 9 Exec Group Changes (vs Bazel 7)

- `copy_from_rule` removed (was deprecated in Bazel 7 prerelease) — must error
- Test exec group constraints no longer propagate from default exec group
- New `exec_group_compatible_with` target attribute: dict mapping exec group names
  to additional constraint values
- `--incompatible_auto_exec_groups` still opt-in (not default)

## Desired End State

After implementation:
- All "Stub" suffixes removed from types that are functional fallbacks/defaults
- `ctx.configuration` reads per-target configuration data (real `short_id` from
  config hash, `is_tool_configuration()` from exec_cfg)
- `ctx.exec_groups["name"]` returns real resolved toolchains per exec group
- `ctx.exec_groups["nonexistent"]` errors with list of valid group names
- `ctx.actions.run(exec_group="name")` records the exec group on the action
- `rule(exec_groups={...})` stores full exec group definitions (toolchains +
  constraints) through freeze pipeline to analysis
- Exec group toolchain resolution runs independently per group via `resolve_toolchains()`

## What We're NOT Doing

1. **Automatic Execution Groups (AEGs)** — `--incompatible_auto_exec_groups` is
   still opt-in in Bazel 9. AEGs auto-create one exec group per toolchain type
   declared on a rule. This is a mechanical extension once manual exec groups work.
   **Follow-up plan recommended** for Bazel 9 parity — needed when users opt in
   or when Bazel 10 flips the default.
2. ~~**`exec_properties` per exec group** — remote execution routing metadata (e.g.,
   `"link.mem": "16g"`). No effect with local-only execution.~~
   → **Covered by [Plan 24](./24-exec-platform-resolution.md) Phase 4**, which
   wires the per-group selected platform's `exec_properties` into the action's
   RE `Platform.properties` message and adds Phase 2's per-target /
   per-action `exec_properties` overrides on top.
3. **`target_settings` on `toolchain()`** — config_setting-based toolchain filtering.
   Documented in Bazel 9 but has known bugs (issue #16671 closed without fix).
   Can be added later.
4. **`toolchains_aspects`** — Bazel 8 feature allowing aspects to propagate into
   toolchain deps. Orthogonal to exec group resolution.
5. **`--incompatible_use_default_test_toolchain`** — test action platform selection
   matching target platform constraints. Depends on AEG infrastructure.

## Phase 1: Rename Category A & B Stubs

### Overview
Rename all functional stubs to remove the misleading "Stub" suffix. These are
correct implementations (empty defaults or no-attrs fallbacks), just poorly named.

### Changes Required

#### 1. Rename BXL/dynamic_output fallbacks
**File**: `app/kuro_build_api/src/interpreter/rule_defs/context.rs`

| Old Name | New Name | Rationale |
|----------|----------|-----------|
| `CtxFilesStub` | `CtxFilesUnavailable` | Attrs not available in BXL/dynamic_output |
| `CtxFileStub` | `CtxFileUnavailable` | Same |
| `CtxExecutableStub` | `CtxExecutableUnavailable` | Same |
| `CtxSplitAttrStub` | `CtxSplitAttrUnavailable` | Same |

#### 2. Rename empty CcInfo defaults
**File**: `app/kuro_build_api/src/interpreter/rule_defs/context.rs`

| Old Name | New Name | Rationale |
|----------|----------|-----------|
| `CompilationContextStub` | `EmptyCompilationContext` | Represents CcInfo with no compilation context |
| `LinkingContextStub` | `EmptyLinkingContext` | Represents CcInfo with no linking context |
| `HeaderInfoStubSimple` | `EmptyHeaderInfo` | Empty header info |

#### 3. Rename ArtifactRootStub
**File**: `app/kuro_build_api/src/interpreter/rule_defs/artifact/methods.rs`

| Old Name | New Name | Rationale |
|----------|----------|-----------|
| `ArtifactRootStub` | `ArtifactRoot` | It's not a stub — returns correct path |

Also update the import in `context.rs` that references `ArtifactRootStub`.

### Success Criteria

#### Automated Verification:
- [x] `cargo check -p kuro` passes
- [x] No type named `*Stub` remains in context.rs except `BuildConfiguration*` (Phase 2)
- [x] All references to renamed types updated (imports, constructors, comments)

---

## Phase 2: Real BuildConfiguration

### Overview
Replace `BuildConfigurationStub` with `BuildConfiguration` that reads from the
target's actual `ConfigurationData` instead of only global process flags. The key
improvement is `short_id` using the real config hash.

### Changes Required

#### 1. Thread configuration data into BuildConfiguration
**File**: `app/kuro_build_api/src/interpreter/rule_defs/context.rs`

Replace:
```rust
pub struct BuildConfigurationStub {
    pub is_tool: bool,
}
```

With:
```rust
pub struct BuildConfiguration {
    pub is_tool: bool,
    /// The configuration hash from ConfigurationData (16-char hex, e.g. "6770d7f2ebfc0845")
    pub config_hash: String,
    /// The full configuration label (e.g. "@local_config_platform//:host#6770d7f2ebfc0845")
    pub config_label: String,
}
```

#### 2. Extract config data at construction site
**File**: `app/kuro_build_api/src/interpreter/rule_defs/context.rs`

At the `ctx.configuration` attribute (line ~629), the `AnalysisContext` has
`self.label` which is a `StarlarkConfiguredProvidersLabel`. From this:
- `label.inner().target().cfg()` → `Configuration`
- `cfg.output_hash()` → `ConfigurationHash` (the 16-char hex string)
- `cfg.full_name()` → the full configuration label string

Update `short_id` to return the hash-based string instead of `"{cpu}-{mode}"`:
```rust
"short_id" => {
    // Bazel's short_id encodes the full configuration.
    // We use the configuration hash which is a Blake3 hash of the platform constraints.
    Some(heap.alloc_str(&format!("{}-{}", host_target_cpu(), self.config_hash)).to_value())
}
```

Note: Keep `host_target_cpu()` as the CPU prefix for readability (Bazel's
short_id also includes the CPU), but use the real hash to distinguish configs.

#### 3. Rename type and update all references
Replace `BuildConfigurationStub` → `BuildConfiguration` in all type names,
starlark type strings, method names, and comments. The Starlark type string
remains `"configuration"` (unchanged for compatibility).

#### 4. Handle missing label (BXL/dynamic_output)
When `self.label` is `None`, fall back to empty hash and "unknown" label:
```rust
let (config_hash, config_label) = match &self.label {
    Some(label) => {
        let cfg = label.inner().target().cfg();
        (cfg.output_hash().as_str().to_owned(), cfg.full_name().to_owned())
    }
    None => (String::new(), "unknown".to_owned()),
};
```

### Success Criteria

#### Automated Verification:
- [x] `cargo check -p kuro` passes
- [x] `BuildConfigurationStub` type no longer exists
- [x] `ctx.configuration.short_id` includes real config hash (from ConfigurationData)
- [x] Existing `coverage_enabled`, `default_shell_env`, `test_env`, `stamp_binaries`,
      `host_path_separator` attributes unchanged in behavior
- [x] `is_tool_configuration()` unchanged in behavior

---

## Phase 3: Store Exec Group Definitions Properly

### Overview
Make `rule(exec_groups={...})` store the full exec group definitions (toolchain
types + exec_compatible_with constraints), not just the group names. This data
is needed for per-group toolchain resolution in Phase 4.

### Changes Required

#### 1. Define ExecGroupDef struct
**File**: `app/kuro_interpreter_for_build/src/rule.rs`

```rust
/// Stored definition of an exec group from rule(exec_groups={...}).
#[derive(Debug, Clone)]
pub struct ExecGroupDef {
    /// Toolchain type labels this group requires
    pub toolchain_types: Vec<String>,
    /// Exec-compatible-with constraint labels
    pub exec_compatible_with: Vec<String>,
}
```

#### 2. Extract full exec group info in rule()
**File**: `app/kuro_interpreter_for_build/src/rule.rs` (line ~998-1012)

Replace the current code that only extracts dict keys:
```rust
let exec_group_defs: Vec<(String, ExecGroupDef)> = if let Some(eg_val) = exec_groups {
    if let Some(dict) = DictRef::from_value(eg_val) {
        dict.iter().filter_map(|(k, v)| {
            let name = k.unpack_str()?.to_owned();
            // Extract toolchain types from ExecGroupValue
            let toolchains = v.get_attr("toolchains", heap)
                .and_then(|tc_val| {
                    // Parse toolchain requirements same as rule-level
                    extract_toolchain_type_labels(tc_val)
                })
                .unwrap_or_default();
            let exec_compat = v.get_attr("exec_compatible_with", heap)
                .and_then(|ec_val| {
                    extract_label_strings(ec_val)
                })
                .unwrap_or_default();
            Some((name, ExecGroupDef { toolchain_types: toolchains, exec_compatible_with: exec_compat }))
        }).collect()
    } else { Vec::new() }
} else { Vec::new() };
```

#### 3. Error on copy_from_rule
**File**: `app/kuro_interpreter_for_build/src/rule.rs` (exec_group function, line ~1260)

`copy_from_rule` was removed in Bazel 7. If someone passes it as `true`, error:
```rust
if copy_from_rule {
    return Err(starlark::Error::new_other(anyhow::anyhow!(
        "copy_from_rule is no longer supported (removed in Bazel 7). \
         Specify toolchains and exec_compatible_with explicitly."
    )));
}
```

#### 4. Store in StarlarkRuleCallable and FrozenStarlarkRuleCallable
**File**: `app/kuro_interpreter_for_build/src/rule.rs`

Replace `exec_group_names: Vec<String>` with `exec_group_defs: Vec<(String, ExecGroupDef)>`
in both `StarlarkRuleCallable` and `FrozenStarlarkRuleCallable`. Add a getter:
```rust
pub fn exec_group_defs(&self) -> &[(String, ExecGroupDef)] {
    &self.exec_group_defs
}
```

Also keep `exec_group_names()` as a convenience that returns just the names
(derived from `exec_group_defs`), since callers in `kuro_node` use it.

#### 5. Thread through RuleData and TargetNode
**Files**: `app/kuro_node/src/rule.rs`, `app/kuro_node/src/nodes/unconfigured.rs`,
`app/kuro_node/src/nodes/configured.rs`

Add `exec_group_defs: Vec<(String, ExecGroupDef)>` alongside or replacing
`exec_group_names`. Ensure `ConfiguredTargetNode::exec_group_defs()` accessor exists.

### Success Criteria

#### Automated Verification:
- [x] `cargo check -p kuro` passes
- [x] `exec_group()` with `copy_from_rule=True` produces an error
- [x] `rule(exec_groups={"link": exec_group(toolchains=[...])})` stores toolchain
      type labels in `ExecGroupDef`
- [x] `FrozenStarlarkRuleCallable::exec_group_defs()` returns full definitions

---

## Phase 4: Per-Group Toolchain Resolution

### Overview
Extend `resolve_toolchains()` to support per-exec-group resolution. Each exec
group gets independent resolution with its own toolchain requirements and exec
constraints, producing its own `(exec_platform, toolchain_map)` result.

### Changes Required

#### 1. Add ExecGroupResolutionRequest
**File**: `app/kuro_analysis/src/analysis/toolchain_resolution.rs`

```rust
/// A request to resolve toolchains for one exec group.
#[derive(Debug, Clone)]
pub struct ExecGroupResolutionRequest {
    /// Group name ("default" for the rule-level toolchains)
    pub group_name: String,
    /// Toolchain types this group requires
    pub required_types: Vec<RequiredToolchainType>,
    /// Additional exec constraints for this group
    pub exec_constraints: Vec<String>,
}

/// Result of resolving all exec groups for a target.
#[derive(Debug, Clone)]
pub struct MultiGroupResolutionResult {
    /// Per-group results keyed by group name
    pub groups: HashMap<String, ToolchainResolutionResult>,
}
```

#### 2. Implement resolve_toolchains_multi_group()
**File**: `app/kuro_analysis/src/analysis/toolchain_resolution.rs`

```rust
/// Resolve toolchains for multiple exec groups independently.
///
/// Each exec group gets its own call to resolve_toolchains() with its own
/// required types and exec constraints. Different groups may select different
/// execution platforms.
pub fn resolve_toolchains_multi_group(
    requests: &[ExecGroupResolutionRequest],
    target_platform: &PlatformConstraints,
    exec_platforms: &[PlatformConstraints],
) -> Result<MultiGroupResolutionResult, String> {
    let mut groups = HashMap::new();
    for req in requests {
        let result = resolve_toolchains(
            &req.required_types,
            target_platform,
            exec_platforms,
            &req.exec_constraints,
        )?;
        groups.insert(req.group_name.clone(), result);
    }
    Ok(MultiGroupResolutionResult { groups })
}
```

#### 3. Build exec group requests from rule + target data
**File**: `app/kuro_analysis/src/analysis/env.rs`

In `run_analysis_with_env_underlying()`, after extracting `toolchain_types` from
the rule (for the default group), also extract exec group defs and build requests:

```rust
// Default exec group: rule-level toolchains + target exec constraints
let mut requests = vec![ExecGroupResolutionRequest {
    group_name: "default".to_owned(),
    required_types: rule_level_types,
    exec_constraints: target_exec_constraints,
}];

// Named exec groups from rule definition
for (name, def) in rule.exec_group_defs() {
    requests.push(ExecGroupResolutionRequest {
        group_name: name.clone(),
        required_types: def.toolchain_types.iter().map(|t| RequiredToolchainType {
            type_label: t.clone(),
            mandatory: true, // Bazel default
        }).collect(),
        exec_constraints: def.exec_compatible_with.clone(),
    });
}

let multi_result = resolve_toolchains_multi_group(&requests, &target_platform, &exec_platforms)?;
```

#### 4. Handle exec_group_compatible_with target attribute
**File**: `app/kuro_analysis/src/analysis/env.rs`

Bazel 9 adds `exec_group_compatible_with` as an attribute on every target:
```python
exec_group_compatible_with = {"test": ["@platforms//os:linux"]}
```

When building exec group requests, merge any additional constraints from this
attribute into the corresponding group's `exec_constraints`.

This requires:
- Adding `exec_group_compatible_with` as an internal attribute on all rules
  (similar to `tags`, `testonly`, etc.)
- Reading it from the target node during analysis

### Success Criteria

#### Automated Verification:
- [x] `cargo check -p kuro` passes
- [x] `resolve_toolchains_multi_group()` correctly resolves multiple groups independently
- [x] Default exec group produces same results as current `resolve_toolchains()` (no regression)
- [x] Named groups with different exec constraints can select different exec platforms
- [x] Unit tests for multi-group resolution added (test_multi_group_resolution_empty)

---

## Phase 5: Wire Real Exec Groups into ctx

### Overview
Replace `ExecGroupsDict`/`ExecGroupInfo`/`ExecGroupToolchains` stubs with real
types backed by per-group resolution results. Wire `exec_group` parameter on
`ctx.actions.run()`.

### Changes Required

#### 1. Create ResolvedExecGroups type
**File**: `app/kuro_build_api/src/interpreter/rule_defs/context.rs`

```rust
/// Real exec group collection backed by per-group toolchain resolution.
/// Implements ctx.exec_groups indexing.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ResolvedExecGroups {
    /// Map of group name → per-group resolved toolchains
    /// Keys are the names from rule(exec_groups={...})
    groups: HashMap<String, ResolvedExecGroupContext>,
}
```

`StarlarkValue` impl:
- `at(index)`: look up group by name. Error if not found with message listing
  valid group names. Block access to "default" (Bazel behavior).
- `is_in()`: check if group name exists

#### 2. Create ResolvedExecGroupContext type
**File**: `app/kuro_build_api/src/interpreter/rule_defs/context.rs`

```rust
/// A single resolved exec group, returned from ctx.exec_groups["name"].
/// Exposes .toolchains attribute.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ResolvedExecGroupContext {
    /// The resolved toolchains for this exec group
    toolchains: ResolvedToolchains,
}
```

`StarlarkValue` impl:
- `get_attr("toolchains")`: returns the `ResolvedToolchains` for this group
  (same type already used by `ctx.toolchains`)

#### 3. Build ResolvedExecGroups during analysis
**File**: `app/kuro_analysis/src/analysis/env.rs`

After `resolve_toolchains_multi_group()` returns, for each named group (not "default"):
1. Analyze the resolved toolchain impl targets via DICE (same as current default group flow)
2. Build per-group `ResolvedToolchains` with the group's providers
3. Collect into `ResolvedExecGroups`

Set on `AnalysisContext` alongside the existing `resolved_toolchains` (which
continues to hold the default group's result for `ctx.toolchains`).

#### 4. Wire ctx.exec_groups to return ResolvedExecGroups
**File**: `app/kuro_build_api/src/interpreter/rule_defs/context.rs`

Replace the current `ctx.exec_groups` implementation (line ~802):
```rust
fn exec_groups<'v>(this: RefAnalysisContext<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
    Ok(this.0.resolved_exec_groups(heap))
}
```

When no exec groups were declared or resolution didn't run, return an empty
`ResolvedExecGroups` (indexing any key produces an error).

#### 5. Wire exec_group into ctx.actions.run()
**File**: `app/kuro_action_impl/src/context/run.rs`

Stop discarding the `exec_group` parameter. Store it on the action:
- Validate that the exec group name is one of the rule's declared groups
- Record the exec group name on the `RunActionValues` / action metadata
- At execution time, this could be used to select the correct execution
  platform (→ **[Plan 24](./24-exec-platform-resolution.md) Phase 4**
  wires the per-group selected platform into the action's RE
  `Platform.properties` message, retiring the local-only assumption)

#### 6. Delete old stub types
Remove `ExecGroupsDict`, `ExecGroupInfo`, `ExecGroupToolchains` from `context.rs`.
Also remove the two stub returns in `cc_common.rs` (lines ~899, ~993).

### Success Criteria

#### Automated Verification:
- [x] `cargo check -p kuro` passes
- [x] No `ExecGroupsDict`, `ExecGroupInfo`, `ExecGroupToolchains` types remain
- [x] `ctx.exec_groups["link"].toolchains` returns ResolvedToolchains (per-group)
- [x] `ctx.exec_groups` falls back to empty ResolvedExecGroups when no groups declared
- [ ] `ctx.actions.run(exec_group="link")` validates the group name (deferred — requires action-layer changes)
- [x] cc_test_example builds (hello_bin, hello_test_static, multi_package//app:calculator all pass)
- [x] All cargo tests pass (4/4 toolchain resolution tests)

#### Manual Verification:
- [ ] Rules with exec_groups (e.g., rules_cc link group) get real per-group toolchains
- [ ] `ctx.actions.run(exec_group="...")` records the group on the action

---

## Anti-Patterns to Avoid

### DO NOT support copy_from_rule
It was removed in Bazel 7. Error on it explicitly.

### DO NOT let ExecGroupsDict accept any key
Real Bazel errors when you access an undeclared exec group. The stub's "always
true" behavior hides bugs in rules.

### DO NOT mix default and named exec group results
`ctx.toolchains` is the default group. `ctx.exec_groups["name"].toolchains` is
a named group. These have independent resolution and may use different execution
platforms.

### DO NOT discard exec_group on ctx.actions.run()
Even though local execution ignores it, recording the exec group on the action
is needed for correctness validation and future remote execution support.

## References

- [Bazel Execution Groups](https://bazel.build/extending/exec-groups)
- [Bazel Automatic Execution Groups](https://bazel.build/extending/auto-exec-groups)
- [Bazel 9 Release Notes](https://github.com/bazelbuild/bazel/releases/tag/9.0.0)
- [Bazel ctx API](https://bazel.build/rules/lib/builtins/ctx)
- [Bazel configuration type](https://bazel.build/rules/lib/builtins/configuration.html)
- Plan 11: `thoughts/shared/plans/kuro-bazel-subplans/11-toolchain-resolution.md`
- Current stubs: `app/kuro_build_api/src/interpreter/rule_defs/context.rs`
- Toolchain resolution: `app/kuro_analysis/src/analysis/toolchain_resolution.rs`
- Rule definition: `app/kuro_interpreter_for_build/src/rule.rs`
- Analysis pipeline: `app/kuro_analysis/src/analysis/env.rs`
