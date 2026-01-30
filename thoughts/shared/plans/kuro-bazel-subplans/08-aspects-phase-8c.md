# Aspects Phase 8c: Shadow Graph Propagation and DICE Integration

> **Main Plan**: [08-aspects.md](./08-aspects.md)
> **Previous Phase**: [08-aspects-phase-8b.md](./08-aspects-phase-8b.md)

## Overview

Implement recursive aspect propagation through dependency graphs with DICE integration for incremental caching. This enables aspects to actually execute during builds and produce the "shadow graph" that Bazel semantics require.

**Why this phase is critical:** Phase 8b created all the types (AspectContext, AspectRuleInfo, AspectTargetProviders, run_aspect_basic) but aspects don't execute during builds. Phase 8c wires everything together so aspects run automatically when rules with aspect-attached attributes are analyzed.

---

## Current State (Phase 8b Complete)

**What exists:**
- `AspectContext` with ctx.attr, ctx.actions, ctx.label, ctx.rule, ctx.fragments
- `AspectRuleInfo` providing ctx.rule.kind and ctx.rule.attr
- `AspectTargetProviders` supporting target[SomeInfo] and SomeInfo in target
- `try_from_aspect_value()` rejecting DefaultInfo from aspects
- `run_aspect_basic()` function that can execute an aspect on a single target

**What's missing:**
- Aspects stored in Attribute metadata (currently ignored in `_unused`)
- AspectKey DICE integration for caching
- Recursive propagation through attr_aspects
- Shadow graph construction (ctx.rule.attr.deps contains aspect results)
- Integration with rule analysis/gather_deps

**Key insight:** The function `run_aspect_basic()` exists but nothing calls it during builds.

---

## Implementation Steps

### Step 1: Add Unit Tests for run_aspect_basic() (Phase 8b Completion)

**File:** `app/kuro_build_api/src/interpreter/rule_defs/aspect/execution.rs`

Before wiring Phase 8c, verify Phase 8b infrastructure works:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aspect_executes_and_receives_context() {
        // Create Starlark module with test aspect
        // Execute aspect with run_aspect_basic()
        // Verify ctx members are accessible
    }

    #[test]
    fn test_aspect_rejects_default_info() {
        // Aspect that returns DefaultInfo should fail
    }

    #[test]
    fn test_target_provider_access() {
        // Verify target[SomeInfo] and SomeInfo in target work
    }
}
```

**Test coverage needed:**
- Aspect implementation is called
- ctx.rule.kind returns correct value
- ctx.rule.attr is accessible
- ctx.label returns target label
- Empty provider list is valid
- DefaultInfo is rejected

---

### Step 2: Extend Attribute Struct to Store Aspects

**File:** `app/kuro_node/src/attrs/attr.rs`

Current struct (lines 31-39):
```rust
pub struct Attribute {
    default: AttributeDefault,
    doc: String,
    coercer: AttrType,
}
```

Add aspects field:
```rust
pub struct Attribute {
    default: AttributeDefault,
    doc: String,
    coercer: AttrType,
    /// Aspects to apply to dependencies of this attribute (Phase 8c)
    aspects: Vec<Arc<String>>,  // Store aspect names for minimal implementation
}
```

Add methods:
```rust
impl Attribute {
    pub fn with_aspects(mut self, aspects: Vec<Arc<String>>) -> Self {
        self.aspects = aspects;
        self
    }

    pub fn aspects(&self) -> &[Arc<String>] {
        &self.aspects
    }
}
```

Update `new()` constructor to initialize `aspects: Vec::new()`.

---

### Step 3: Store Aspects in attr.label() and attr.label_list()

**File:** `app/kuro_interpreter_for_build/src/attrs/attrs_global.rs`

In `label()` function (line 798), replace:
```rust
let _unused = (mandatory, executable, allow_files_bool, allow_single_file_bool, allow_rules, flags, aspects);
```

With:
```rust
// Extract aspect names from the aspects parameter
let mut aspect_names = Vec::new();
for aspect_val in aspects.items {
    if let Some(frozen) = aspect_val.unpack_frozen() {
        if let Some(aspect) = frozen.downcast_ref::<FrozenStarlarkAspectCallable>() {
            aspect_names.push(Arc::new(aspect.name().to_owned()));
        } else {
            return Err(starlark::Error::new_other(
                "aspects parameter must contain aspect objects"
            ));
        }
    }
}
let _unused = (mandatory, executable, allow_files_bool, allow_single_file_bool, allow_rules, flags);

// Create attribute with aspects attached
let base_attr = Attribute::attr(eval, default, doc, coercer)?;
Ok(if aspect_names.is_empty() {
    base_attr
} else {
    StarlarkAttribute::new(base_attr.inner().with_aspects(aspect_names))
})
```

**Apply same changes to `label_list()` function (line 856).**

---

### Step 4: Create AspectKey for DICE Caching

**New File:** `app/kuro_analysis/src/analysis/aspect_key.rs`

```rust
use allocative::Allocative;
use derive_more::Display;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use dupe::Dupe;

/// DICE key for caching aspect computation results.
///
/// Key = (target, aspect_name) → Value = AspectValue (providers)
#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display("AspectKey({}, {})", target, aspect_name)]
pub struct AspectKey {
    pub target: ConfiguredTargetLabel,
    pub aspect_name: String,
}

impl AspectKey {
    pub fn new(target: ConfiguredTargetLabel, aspect_name: String) -> Self {
        Self { target, aspect_name }
    }
}

/// Result of aspect computation (cached in DICE).
#[derive(Clone, Debug, Allocative)]
pub struct AspectValue {
    pub providers: Arc<FrozenProviderCollection>,
}

impl AspectValue {
    pub fn empty() -> Self {
        Self {
            providers: Arc::new(FrozenProviderCollection::default()),
        }
    }
}
```

---

### Step 5: Implement DICE Key Computation for Aspects

**New File:** `app/kuro_analysis/src/analysis/aspect_calculation.rs`

```rust
use dice::{CancellationContext, DiceComputations, Key};
use async_trait::async_trait;
use std::sync::Arc;
use std::collections::HashMap;

use super::aspect_key::{AspectKey, AspectValue};
use super::calculation::AnalysisKey;
use kuro_build_api::interpreter::rule_defs::aspect::run_aspect_basic;

#[async_trait]
impl Key for AspectKey {
    type Value = kuro_error::Result<AspectValue>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        // 1. Get target's analysis result (ensures target is analyzed first)
        let target_result = ctx.compute(&AnalysisKey(self.target.dupe())).await??;

        // 2. Load aspect callable from module registry
        let aspect = load_aspect_by_name(ctx, &self.aspect_name).await?;

        // 3. Check required_providers filter
        if !aspect_applies_to_target(&aspect, &target_result)? {
            return Ok(AspectValue::empty());
        }

        // 4. Recursively compute aspects on dependencies (depth-first)
        let dep_aspect_results = compute_dep_aspects(
            ctx,
            &self.target,
            &aspect,
        ).await?;

        // 5. Build shadow graph (replace deps with aspect results)
        let shadow_attrs = build_shadow_attrs(&target_result, &dep_aspect_results)?;

        // 6. Execute aspect using run_aspect_basic()
        let providers = execute_aspect_impl(
            ctx,
            &self.target,
            &aspect,
            &target_result,
            shadow_attrs,
        ).await?;

        Ok(AspectValue {
            providers: Arc::new(providers.freeze()?),
        })
    }
}

async fn compute_dep_aspects(
    ctx: &mut DiceComputations,
    target: &ConfiguredTargetLabel,
    aspect: &FrozenStarlarkAspectCallable,
) -> kuro_error::Result<HashMap<ConfiguredTargetLabel, AspectValue>> {
    let node = ctx.get_configured_target_node(target).await?;

    let attr_aspects = aspect.attr_aspects(); // e.g., ["deps"] or ["*"]
    let propagate_all = attr_aspects.iter().any(|a| a == "*");

    let mut futures = Vec::new();

    // For each attribute that matches attr_aspects
    for attr in node.attrs() {
        if !propagate_all && !attr_aspects.iter().any(|a| a == attr.name()) {
            continue;
        }

        // Extract dep labels from attribute value
        for dep in extract_dep_labels(&attr.value())? {
            let key = AspectKey::new(dep.dupe(), aspect.name().to_owned());
            futures.push(ctx.compute(&key));
        }
    }

    // Execute all in parallel via DICE
    let results = futures::future::try_join_all(futures).await?;

    // Collect into map
    Ok(results
        .into_iter()
        .filter_map(|r| r.ok())
        .map(|v| (v.target.dupe(), v))
        .collect())
}

fn aspect_applies_to_target(
    aspect: &FrozenStarlarkAspectCallable,
    target_result: &AnalysisResult,
) -> kuro_error::Result<bool> {
    let required_providers = aspect.required_providers();

    // Empty required_providers = applies to all targets
    if required_providers.is_empty() {
        return Ok(true);
    }

    // Check any-of logic: [[A], [B, C]] means A OR (B AND C)
    for provider_set in required_providers {
        let has_all = provider_set.iter().all(|provider_id| {
            target_result.providers().contains_provider(provider_id)
        });
        if has_all {
            return Ok(true);
        }
    }

    Ok(false)
}
```

---

### Step 6: Wire Aspect Computation into gather_deps()

**File:** `app/kuro_configured/src/nodes.rs`

After dependency collection in `gather_deps()` (around line 750), add:

```rust
// Phase 8c: Collect aspects that need to be applied to dependencies
let mut aspect_keys = Vec::new();

for a in target_node.attrs(AttrInspectOptions::All) {
    let attr = a.attribute();

    // Check if this attribute has aspects attached
    if !attr.aspects().is_empty() {
        let configured_attr = a.configure(&attr_cfg_ctx)?;

        // Extract dependencies from this attribute
        let deps = extract_deps_from_attr(&configured_attr)?;

        // Schedule aspect computation for each dep
        for dep_label in deps {
            for aspect_name in attr.aspects() {
                aspect_keys.push(AspectKey::new(
                    dep_label.dupe(),
                    aspect_name.as_ref().clone(),
                ));
            }
        }
    }
}

// Compute all aspects in parallel via DICE
if !aspect_keys.is_empty() {
    let aspect_results = ctx.compute_join(aspect_keys.iter(), |ctx, key| {
        async move { ctx.compute(key).await }.boxed()
    }).await;

    // Store in gathered_deps for later use during rule analysis
    gathered_deps.aspect_results = process_aspect_results(aspect_results)?;
}
```

Also modify `GatheredDeps` struct to include:
```rust
pub struct GatheredDeps {
    // ... existing fields ...
    /// Aspect results for dependencies with aspects attached (Phase 8c)
    pub aspect_results: HashMap<(ConfiguredTargetLabel, String), AspectValue>,
}
```

---

### Step 7: Add Aspect Modules to kuro_analysis

**File:** `app/kuro_analysis/src/analysis/mod.rs`

Add:
```rust
pub mod aspect_calculation;
pub mod aspect_key;
```

---

## Files Summary

### New Files

| File | Purpose |
|------|---------|
| `app/kuro_analysis/src/analysis/aspect_key.rs` | AspectKey, AspectValue DICE types |
| `app/kuro_analysis/src/analysis/aspect_calculation.rs` | DICE Key implementation |

### Modified Files

| File | Changes |
|------|---------|
| `app/kuro_node/src/attrs/attr.rs` | Add `aspects: Vec<Arc<String>>` field |
| `app/kuro_interpreter_for_build/src/attrs/attrs_global.rs` | Store aspects in label()/label_list() |
| `app/kuro_configured/src/nodes.rs` | Wire aspect computation into gather_deps() |
| `app/kuro_analysis/src/analysis/mod.rs` | Add aspect modules |
| `app/kuro_build_api/src/interpreter/rule_defs/aspect/execution.rs` | Add unit tests |

---

## Success Criteria

### Automated Verification

- [ ] Unit tests pass for `run_aspect_basic()` (Phase 8b completion)
- [ ] Attribute struct stores aspects field
- [ ] attr.label(aspects=[...]) extracts and stores aspect names
- [ ] AspectKey DICE computation works
- [ ] `cargo build` succeeds for all crates
- [ ] `cargo test -p kuro_build_api` passes

### Manual Verification

**Test file:** `tests/manual_test/test_aspect_8c.bzl`

```python
CollectNamesInfo = provider(fields=["names"])

def _collect_aspect_impl(target, ctx):
    print("Aspect visiting:", ctx.label)
    print("  Rule kind:", ctx.rule.kind)
    names = [str(ctx.label)]
    if hasattr(ctx.rule.attr, "deps"):
        for dep in ctx.rule.attr.deps:
            if CollectNamesInfo in dep:
                names.extend(dep[CollectNamesInfo].names)
    return [CollectNamesInfo(names=names)]

collect_aspect = aspect(
    implementation=_collect_aspect_impl,
    attr_aspects=["deps"],
)

def _test_rule_impl(ctx):
    return [DefaultInfo()]

test_rule = rule(
    implementation=_test_rule_impl,
    attrs={
        "deps": attr.label_list(aspects=[collect_aspect]),
    },
)
```

**BUILD.bazel:**
```python
test_rule(name="a")
test_rule(name="b", deps=[":a"])
test_rule(name="c", deps=[":b"])
```

**Verification:**
- [ ] Run `kuro build //tests/manual_test:c`
- [ ] Output shows "Aspect visiting: //tests/manual_test:a"
- [ ] Output shows "Aspect visiting: //tests/manual_test:b"
- [ ] Output shows "Aspect visiting: //tests/manual_test:c"
- [ ] Aspect propagates through dependency chain
- [ ] `ctx.rule.attr.deps` contains aspect results (shadow graph)

### Integration Test

**New file:** `tests/core/aspects/test_propagation.py`

```python
def test_simple_aspect_propagation(kuro):
    """Test aspect propagates through deps attribute."""
    # Create test files
    # Run kuro build
    # Verify aspect executed on all targets in dependency chain
```

---

## Phase 8c Scope Boundaries

### What Phase 8c DOES Include

- Store aspects in Attribute metadata
- AspectKey DICE integration for caching
- Recursive propagation via attr_aspects
- Basic shadow graph (ctx.rule.attr.deps contains aspect results)
- required_providers filtering
- Integration with gather_deps()

### What Phase 8c Does NOT Include (Deferred to Phase 8d)

- `required_aspect_providers` - Cross-aspect dependencies
- `requires` - Explicit aspect ordering
- Toolchain resolution for aspects
- `exec_groups` for aspects
- `apply_to_generating_rules`
- Multiple aspects on same attribute with ordering guarantees

---

## Design Decisions

### Decision 1: Store aspect names vs full callable?

**Choice:** Store aspect names (`Vec<Arc<String>>`) in Attribute

**Why:**
- Avoids circular dependencies between crates
- Simpler serialization/hashing for DICE
- Load callable on-demand from module registry

### Decision 2: Propagation order?

**Choice:** Depth-first (deps computed before parent)

**Why:**
- Natural DICE recursion pattern
- Matches Bazel semantics
- Each level waits for deps to complete

### Decision 3: Shadow graph representation?

**Choice:** Replace deps in ctx.rule.attr with AspectTargetProviders

**Why:**
- Transparent to aspect implementation
- Matches Bazel semantics exactly
- AspectTargetProviders already supports target[Provider] syntax

---

## References

### Implementation Patterns

- `app/kuro_analysis/src/analysis/calculation.rs` - AnalysisKey DICE pattern to follow
- `app/kuro_analysis/src/analysis/env.rs` - RuleSpec and run_analysis pattern
- `app/kuro_configured/src/nodes.rs` - gather_deps() integration point

### Bazel Source (for reference)

- `AspectFunction.java` - DICE computation for aspects
- `AspectValue.java` - Aspect result storage
- `ConfiguredTargetFunction.java` - Dependency resolution pattern

### Related Documents

- [08-aspects.md](./08-aspects.md) - Main aspects plan
- [08-aspects-phase-8b.md](./08-aspects-phase-8b.md) - Phase 8b types (prerequisite)
