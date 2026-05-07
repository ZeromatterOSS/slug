# Aspects Phase 8c: Shadow Graph Propagation and DICE Integration

> **Status: SUPERSEDED.** Use [06-aspects.md](./06-aspects.md) and the
> `06-aspects-phase-*` files for current aspect status.
>
> **Main Plan**: [08-aspects.md](./08-aspects.md)
> **Previous Phase**: [08-aspects-phase-8b.md](./08-aspects-phase-8b.md)

## Overview

Implement recursive aspect propagation through dependency graphs with DICE integration for incremental caching. This enables aspects to actually execute during builds and produce the "shadow graph" that Bazel semantics require.

**Why this phase is critical:** Phase 8b created all the types (AspectContext, AspectRuleInfo, AspectTargetProviders, run_aspect_basic) but aspects don't execute during builds. Phase 8c wires everything together so aspects run automatically when rules with aspect-attached attributes are analyzed.

---

## Research Summary (2026-01-30)

**Problem discovered:** The original plan stored only aspect names in attributes, but
`AspectKey::compute()` needs the module path to load the aspect callable via DICE.

**Research conducted:**
1. **Module loading in Kuro** - No global registry; rules use `StarlarkRuleType = (path, name)`
2. **DICE constraints** - Keys must be Hash+Eq (no FrozenValue); Values can contain FrozenModule
3. **Bazel architecture** - Uses `AspectDescriptor = (AspectClass, AspectParameters)` with `bzl_file%aspect_name`

**Solution:** Store `StarlarkAspectType = (BzlOrBxlPath, name)` instead of just names.
This matches Bazel's architecture and follows the existing rule loading pattern in Kuro.

**Files affected by the update:**
- `app/kuro_node/src/aspect_type.rs` (NEW)
- `app/kuro_interpreter_for_build/src/aspect.rs` (add aspect_path field)
- `app/kuro_node/src/attrs/attr.rs` (change aspects field type)
- `app/kuro_interpreter_for_build/src/attrs/attrs_global.rs` (extract full type)
- `app/kuro_analysis/src/analysis/aspect_key.rs` (use StarlarkAspectType)
- `app/kuro_analysis/src/analysis/aspect_calculation.rs` (implement module loading)
- `app/kuro_configured/src/nodes.rs` (update aspect key creation)

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

### Step 1: Add Unit Tests for run_aspect_basic() (Phase 8b Completion) ✅

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

### Step 2: Extend Attribute Struct to Store Aspects ✅

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

### Step 3: Store Aspects in attr.label() and attr.label_list() ✅

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

### Step 4: Create AspectKey for DICE Caching ✅

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

### Step 5: Implement DICE Key Computation for Aspects ✅ (NEEDS UPDATE)

**UPDATED (2026-01-30):** The original stub implementation is in place, but needs to be
updated to use proper module-based aspect loading. See Step 5a for the required changes.

**Current File:** `app/kuro_analysis/src/analysis/aspect_calculation.rs`

The current implementation is a stub that returns the target's providers without executing
the aspect. The following changes are needed to complete aspect execution:

### Step 5a: Add AspectType for Module-Based Aspect Identity (NEW)

**New Type:** `app/kuro_node/src/aspect_type.rs`

Similar to `StarlarkRuleType` for rules, create `StarlarkAspectType` for aspects:

```rust
use allocative::Allocative;
use derive_more::Display;
use dupe::Dupe;
use kuro_core::bzl_or_bxl_path::BzlOrBxlPath;

/// Identifies an aspect by its defining module and exported name.
/// Analogous to StarlarkRuleType for rules.
#[derive(Clone, Dupe, Debug, Display, Eq, Hash, PartialEq, Allocative)]
#[display("{path}:{name}")]
pub struct StarlarkAspectType {
    /// The .bzl file that defines this aspect
    pub path: BzlOrBxlPath,
    /// The exported symbol name (e.g., "my_aspect")
    pub name: String,
}

impl StarlarkAspectType {
    pub fn new(path: BzlOrBxlPath, name: String) -> Self {
        Self { path, name }
    }
}
```

### Step 5b: Update StarlarkAspectCallable to Store Path (NEW)

**File:** `app/kuro_interpreter_for_build/src/aspect.rs`

Add `aspect_path` field (following the pattern from `rule_path` in StarlarkRuleCallable):

```rust
pub struct StarlarkAspectCallable<'v> {
    /// The import path that contains the aspect() call
    aspect_path: BzlOrBxlPath,  // NEW FIELD
    /// The name of this aspect (set when exported/assigned to a variable)
    name: RefCell<Option<String>>,
    // ... existing fields ...
}
```

In the `aspect()` function, capture the path from BuildContext:

```rust
fn aspect<'v>(..., eval: &mut Evaluator<'v, '_, '_>) -> starlark::Result<StarlarkAspectCallable<'v>> {
    let build_context = BuildContext::from_context(eval)?;
    let aspect_path = match &build_context.additional {
        PerFileTypeContext::Bzl(bzl_path) => BzlOrBxlPath::Bzl(bzl_path.bzl_path.clone()),
        _ => return Err(AspectError::AspectNotInBzl.into()),
    };

    Ok(StarlarkAspectCallable {
        aspect_path,  // Store the path
        name: RefCell::new(None),
        // ... other fields ...
    })
}
```

Add getter to `FrozenStarlarkAspectCallable`:

```rust
impl FrozenStarlarkAspectCallable {
    pub fn aspect_type(&self) -> StarlarkAspectType {
        StarlarkAspectType::new(self.aspect_path.clone(), self.name.clone())
    }
}
```

### Step 5c: Update Attribute to Store AspectType (MODIFY Step 2)

**File:** `app/kuro_node/src/attrs/attr.rs`

Change the aspects field from names to full types:

```rust
pub struct Attribute {
    default: AttributeDefault,
    doc: String,
    coercer: AttrType,
    /// Aspects to apply to dependencies of this attribute (Phase 8c)
    /// Uses StarlarkAspectType to enable DICE-based module loading
    aspects: Vec<StarlarkAspectType>,  // CHANGED from Vec<Arc<String>>
}

impl Attribute {
    pub fn with_aspects(mut self, aspects: Vec<StarlarkAspectType>) -> Self {
        self.aspects = aspects;
        self
    }

    pub fn aspects(&self) -> &[StarlarkAspectType] {
        &self.aspects
    }
}
```

### Step 5d: Update attrs_global.rs to Extract Full AspectType (MODIFY Step 3)

**File:** `app/kuro_interpreter_for_build/src/attrs/attrs_global.rs`

Change aspect extraction to capture full type:

```rust
// Extract aspect types from the aspects parameter (Phase 8c - UPDATED)
use crate::aspect::FrozenStarlarkAspectCallable;
let mut aspect_types = Vec::new();
for aspect_val in aspects.items {
    if let Some(frozen) = aspect_val.unpack_frozen() {
        if let Some(aspect) = frozen.downcast_ref::<FrozenStarlarkAspectCallable>() {
            aspect_types.push(aspect.aspect_type());  // Full type, not just name
        } else {
            return Err(ValueError::IncorrectParameterTypeNamed("aspects".to_owned()).into());
        }
    }
}

// Create attribute with aspects attached
let base_attr = Attribute::attr(eval, default, doc, coercer)?;
Ok(if aspect_types.is_empty() {
    base_attr
} else {
    StarlarkAttribute::new(base_attr.clone_attribute().with_aspects(aspect_types))
})
```

### Step 5e: Update AspectKey to Use Module Path (MODIFY Step 4)

**File:** `app/kuro_analysis/src/analysis/aspect_key.rs`

```rust
use kuro_node::aspect_type::StarlarkAspectType;

/// DICE key for caching aspect computation results.
/// Key = (target, aspect_type) → Value = AspectValue (providers)
#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display("AspectKey({}, {})", target, aspect_type)]
pub struct AspectKey {
    pub target: ConfiguredTargetLabel,
    pub aspect_type: StarlarkAspectType,  // CHANGED: full type instead of just name
}

impl AspectKey {
    pub fn new(target: ConfiguredTargetLabel, aspect_type: StarlarkAspectType) -> Self {
        Self { target, aspect_type }
    }
}
```

### Step 5f: Implement Proper AspectKey::compute() (MODIFY Step 5)

**File:** `app/kuro_analysis/src/analysis/aspect_calculation.rs`

```rust
#[async_trait]
impl Key for AspectKey {
    type Value = kuro_error::Result<AspectValue>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        cancellations: &CancellationContext,
    ) -> Self::Value {
        // 1. Get target's analysis result (ensures target is analyzed first)
        let target_result = ctx
            .compute(&AnalysisKey(self.target.dupe()))
            .await?
            .buck_error_context("Failed to get target analysis result for aspect")?
            .require_compatible()?;

        // 2. Load aspect callable from module (follows rule loading pattern)
        let module = load_aspect_module(ctx, &self.aspect_type).await?;
        let aspect = get_aspect_from_module(&module, &self.aspect_type.name)?;

        // 3. Check required_providers filter
        if !aspect_applies_to_target(&aspect, &target_result)? {
            return Ok(AspectValue::empty());
        }

        // 4. Execute aspect (similar to rule analysis pattern)
        // This requires setting up Starlark evaluation context
        let providers = execute_aspect(
            ctx,
            &self.target,
            &aspect,
            &target_result,
            cancellations,
        ).await?;

        Ok(AspectValue { providers })
    }
}

/// Load the module containing the aspect definition.
/// Follows the same pattern as get_loaded_module() for rules.
async fn load_aspect_module(
    ctx: &mut DiceComputations<'_>,
    aspect_type: &StarlarkAspectType,
) -> kuro_error::Result<LoadedModule> {
    match &aspect_type.path {
        BzlOrBxlPath::Bzl(import_path) => {
            ctx.get_loaded_module_from_import_path(import_path).await
        }
        BzlOrBxlPath::Bxl(bxl_path) => {
            ctx.get_loaded_module(StarlarkModulePath::BxlFile(bxl_path)).await
        }
    }
}

/// Extract the frozen aspect callable from a loaded module by name.
/// Follows the same pattern as get_rule_callable() for rules.
fn get_aspect_from_module(
    module: &LoadedModule,
    name: &str,
) -> kuro_error::Result<&FrozenStarlarkAspectCallable> {
    let aspect_value = module
        .env()
        .get_any_visibility(name)
        .map_err(|e| from_any_with_tag(e, kuro_error::ErrorTag::Tier0))
        .with_buck_error_context(|| format!("Couldn't find aspect `{name}`"))?
        .0;

    aspect_value
        .downcast_ref::<FrozenStarlarkAspectCallable>()
        .internal_error("Expected aspect callable")
}
```

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

**Location:** Lines 439-557 (gather_deps function), Lines 431-437 (GatheredDeps struct)

#### 6.1: Modify GatheredDeps Struct (Lines 431-437)

Add aspect_results field to the struct:

```rust
#[derive(Default)]
pub(crate) struct GatheredDeps {
    pub(crate) deps: Vec<ConfiguredTargetNode>,
    pub(crate) exec_deps: SmallMap<ConfiguredProvidersLabel, CheckVisibility>,
    pub(crate) toolchain_deps: SmallSet<TargetConfiguredTargetLabel>,
    pub(crate) plugin_lists: PluginLists,
    /// Aspect results for dependencies with aspects attached (Phase 8c)
    pub(crate) aspect_results: HashMap<(ConfiguredTargetLabel, ArcStr), AspectValue>,
}
```

**Import additions needed at top of file:**
```rust
use std::collections::HashMap;
use kuro_util::arc_str::ArcStr;
use crate::analysis::aspect_key::{AspectKey, AspectValue};
```

#### 6.2: Collect Aspect Keys During Attribute Traversal

**Insert after line 495** (after `configured_attr.traverse()` completes):

Add a second traversal pass to collect aspect requirements:

```rust
// Phase 8c: Collect aspects that need to be applied to dependencies
let mut aspect_keys = Vec::new();

for a in target_node.attrs(AttrInspectOptions::All) {
    let attr = a.attribute();

    // Check if this attribute has aspects attached
    if !attr.aspects().is_empty() {
        let configured_attr = a.configure(attr_cfg_ctx)?;

        // Extract dependency labels from this configured attribute
        // Using the same ConfiguredAttrTraversal pattern
        struct AspectDepsCollector {
            deps: Vec<ConfiguredTargetLabel>,
        }

        impl ConfiguredAttrTraversal for AspectDepsCollector {
            fn dep(&mut self, dep: &ConfiguredProvidersLabel) -> kuro_error::Result<()> {
                self.deps.push(dep.target().dupe());
                Ok(())
            }

            fn dep_with_plugins(
                &mut self,
                dep: &ConfiguredProvidersLabel,
                _plugin_kinds: &PluginKindSet,
            ) -> kuro_error::Result<()> {
                self.deps.push(dep.target().dupe());
                Ok(())
            }

            // Exec deps and toolchain deps don't propagate aspects in Phase 8c
            fn exec_dep(&mut self, _dep: &ConfiguredProvidersLabel) -> kuro_error::Result<()> {
                Ok(())
            }

            fn toolchain_dep(&mut self, _dep: &ConfiguredProvidersLabel) -> kuro_error::Result<()> {
                Ok(())
            }

            fn plugin_dep(&mut self, _dep: &TargetLabel, _kind: &PluginKind) -> kuro_error::Result<()> {
                Ok(())
            }
        }

        let mut collector = AspectDepsCollector { deps: Vec::new() };
        configured_attr.traverse(target_node.label().pkg(), &mut collector)?;

        // Schedule aspect computation for each dep
        for dep_label in collector.deps {
            for aspect_name in attr.aspects() {
                aspect_keys.push(AspectKey::new(
                    dep_label.dupe(),
                    aspect_name.dupe(),
                ));
            }
        }
    }
}
```

#### 6.3: Compute Aspects in Parallel via DICE

**Insert after aspect key collection** (continuing from above):

```rust
// Compute all aspects in parallel via DICE (following pattern from lines 497-501)
let aspect_results = if !aspect_keys.is_empty() {
    ctx.compute_join(aspect_keys.iter(), |ctx, key| {
        async move {
            // Returns Result<AspectValue>
            ctx.compute(key).await
        }.boxed()
    })
    .await
} else {
    Vec::new()
};
```

#### 6.4: Process Aspect Results

**Insert after DICE computation** (continuing from above):

```rust
// Process aspect results and handle errors
// Following the pattern from lines 506-535 (processing dep_results)
let mut aspect_results_map = HashMap::new();

for (key, result) in aspect_keys.iter().zip(aspect_results) {
    match result {
        Ok(aspect_value) => {
            aspect_results_map.insert(
                (key.target.dupe(), key.aspect_name.dupe()),
                aspect_value,
            );
        }
        Err(e) => {
            // Add to errors_and_incompats following existing error handling pattern
            errors_and_incompats.errs.push(e);
        }
    }
}

// Store in gathered_deps for later use during rule analysis
gathered_deps.aspect_results = aspect_results_map;
```

#### 6.5: Update Function Return

The `aspect_results` field is now populated in `GatheredDeps`, no changes needed to the return statement at lines 548-556.

**Note:** The integration point is **before** the execution platform resolution (lines 757-788) so that aspect computation happens early in the dependency gathering phase, similar to regular dependencies.

---

### Step 7: Add Aspect Modules to kuro_analysis ✅

**File:** `app/kuro_analysis/src/analysis/mod.rs`

Add:
```rust
pub mod aspect_calculation;
pub mod aspect_key;
```

---

### Step 8: Run Automated Tests ✅

**Commands to run:**

```bash
# Check all modified crates compile
cargo check -p kuro_build_api -p kuro_node -p kuro_interpreter_for_build -p kuro_analysis

# Run unit tests for kuro_build_api
cargo test -p kuro_build_api

# Full build to verify integration
cargo build
```

**Verify:**
- All crates compile without errors
- Unit tests for `run_aspect_basic()` pass
- No new warnings introduced

---

### Step 9: Create Manual Test Files

**Objective:** Create test files to manually verify aspect propagation works end-to-end.

#### 9.1: Create Test Aspect Definition

**New file:** `tests/manual_test/test_aspect_8c.bzl`

```python
# Provider to collect names from dependency chain
CollectNamesInfo = provider(fields=["names"])

def _collect_aspect_impl(target, ctx):
    """Aspect that collects target names through the dependency graph."""
    print("Aspect visiting:", ctx.label)
    print("  Rule kind:", ctx.rule.kind)

    # Start with current target's name
    names = [str(ctx.label)]

    # Collect names from dependencies if they have the aspect's provider
    if hasattr(ctx.rule.attr, "deps"):
        for dep in ctx.rule.attr.deps:
            if CollectNamesInfo in dep:
                names.extend(dep[CollectNamesInfo].names)

    return [CollectNamesInfo(names=names)]

collect_aspect = aspect(
    implementation = _collect_aspect_impl,
    attr_aspects = ["deps"],  # Propagate through deps attribute
)

def _test_rule_impl(ctx):
    """Simple rule that just returns DefaultInfo."""
    return [DefaultInfo()]

test_rule = rule(
    implementation = _test_rule_impl,
    attrs = {
        "deps": attr.label_list(aspects=[collect_aspect]),
    },
)
```

#### 9.2: Create Test BUILD File

**New file:** `tests/manual_test/BUILD.bazel`

```python
load(":test_aspect_8c.bzl", "test_rule")

# Create a dependency chain: c -> b -> a
test_rule(name = "a")
test_rule(name = "b", deps = [":a"])
test_rule(name = "c", deps = [":b"])

# Create a wider dependency graph: d -> [b, c]
test_rule(name = "d", deps = [":b", ":c"])
```

#### 9.3: Manual Verification Steps

Run the following commands and verify output:

```bash
# Test 1: Simple linear chain
kuro build //tests/manual_test:c

# Expected output should show aspect visiting all three targets:
# Aspect visiting: //tests/manual_test:a
#   Rule kind: test_rule
# Aspect visiting: //tests/manual_test:b
#   Rule kind: test_rule
# Aspect visiting: //tests/manual_test:c
#   Rule kind: test_rule
```

**Verify:**
- Aspect executes on all targets in dependency chain
- Aspect visits dependencies before dependents (depth-first)
- `ctx.rule.kind` returns "test_rule"
- `ctx.label` returns correct target label
- `ctx.rule.attr.deps` contains aspect results (shadow graph)

```bash
# Test 2: Wider dependency graph
kuro build //tests/manual_test:d

# Expected: Aspect visits a, b, c, and d
# Order should respect dependency structure (a before b, b before d)
```

#### 9.4: Create Integration Test (Optional)

**New file:** `tests/core/aspects/test_propagation.py`

```python
def test_simple_aspect_propagation(tmp_path):
    """Test aspect propagates through deps attribute."""

    # Create test files (bzl and BUILD.bazel from above)
    bzl_content = """
CollectNamesInfo = provider(fields=["names"])

def _collect_aspect_impl(target, ctx):
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
    attrs={"deps": attr.label_list(aspects=[collect_aspect])},
)
"""

    build_content = """
load(":test.bzl", "test_rule")
test_rule(name="a")
test_rule(name="b", deps=[":a"])
test_rule(name="c", deps=[":b"])
"""

    (tmp_path / "test.bzl").write_text(bzl_content)
    (tmp_path / "BUILD.bazel").write_text(build_content)

    # Run kuro build
    result = subprocess.run(
        ["kuro", "build", f"//{tmp_path}:c"],
        capture_output=True,
        text=True
    )

    # Verify aspect executed on all targets
    assert "Aspect visiting:" in result.stdout
    assert "test:a" in result.stdout
    assert "test:b" in result.stdout
    assert "test:c" in result.stdout

    # Verify build succeeded
    assert result.returncode == 0
```

---

### Step 10: Unit Tests for Phase 8c Infrastructure

**Objective:** Add Rust unit tests to verify Phase 8c infrastructure without requiring full builds.
This bypasses build environment issues that block manual testing.

#### 10.1: Unit Tests for StarlarkAspectType

**File:** `app/kuro_node/src/aspect_type.rs` (already has basic test)

Add comprehensive tests following the `rule_type.rs` pattern:

```rust
#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::sync::Arc;

    use kuro_core::bzl::ImportPath;

    use crate::aspect_type::StarlarkAspectType;
    use crate::bzl_or_bxl_path::BzlOrBxlPath;

    #[test]
    fn aspect_type_has_useful_string() {
        let import_path = ImportPath::testing_new("root//some/subdir:aspects.bzl");
        let name = "my_aspect".to_owned();

        assert_eq!(
            "root//some/subdir/aspects.bzl:my_aspect",
            &StarlarkAspectType {
                path: BzlOrBxlPath::Bzl(import_path),
                name
            }
            .to_string()
        );
    }

    #[test]
    fn aspect_type_equality() {
        let path1 = ImportPath::testing_new("root//pkg:aspects.bzl");
        let path2 = ImportPath::testing_new("root//pkg:aspects.bzl");
        let path3 = ImportPath::testing_new("root//other:aspects.bzl");

        let type1 = StarlarkAspectType::new(BzlOrBxlPath::Bzl(path1), "my_aspect".to_owned());
        let type2 = StarlarkAspectType::new(BzlOrBxlPath::Bzl(path2), "my_aspect".to_owned());
        let type3 = StarlarkAspectType::new(BzlOrBxlPath::Bzl(path3), "my_aspect".to_owned());

        assert_eq!(type1, type2);  // Same path and name
        assert_ne!(type1, type3);  // Different path
    }

    #[test]
    fn aspect_type_hash_consistency() {
        let path = ImportPath::testing_new("root//pkg:aspects.bzl");
        let type1 = StarlarkAspectType::new(BzlOrBxlPath::Bzl(path.clone()), "my_aspect".to_owned());
        let type2 = StarlarkAspectType::new(BzlOrBxlPath::Bzl(path), "my_aspect".to_owned());

        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        type1.hash(&mut hasher1);
        type2.hash(&mut hasher2);

        assert_eq!(hasher1.finish(), hasher2.finish());  // Equal types have equal hashes
    }

    #[test]
    fn aspect_type_arc_wrapped() {
        // Verify Arc wrapping works correctly for DICE key usage
        let path = ImportPath::testing_new("root//pkg:aspects.bzl");
        let aspect_type = Arc::new(StarlarkAspectType::new(
            BzlOrBxlPath::Bzl(path),
            "my_aspect".to_owned(),
        ));

        let cloned = aspect_type.clone();
        assert_eq!(aspect_type.name, cloned.name);
        assert_eq!(Arc::strong_count(&aspect_type), 2);
    }
}
```

#### 10.2: Unit Tests for AspectKey

**File:** `app/kuro_analysis/src/analysis/aspect_key.rs`

Add tests for AspectKey DICE key functionality:

```rust
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kuro_core::bzl::ImportPath;
    use kuro_core::configuration::data::ConfigurationData;
    use kuro_core::target::label::label::TargetLabel;
    use kuro_node::aspect_type::StarlarkAspectType;
    use kuro_node::bzl_or_bxl_path::BzlOrBxlPath;

    use super::AspectKey;

    fn make_aspect_type(path: &str, name: &str) -> Arc<StarlarkAspectType> {
        Arc::new(StarlarkAspectType::new(
            BzlOrBxlPath::Bzl(ImportPath::testing_new(path)),
            name.to_owned(),
        ))
    }

    fn make_configured_label(label: &str) -> ConfiguredTargetLabel {
        TargetLabel::testing_parse(label).configure(ConfigurationData::testing_new())
    }

    #[test]
    fn aspect_key_display() {
        let target = make_configured_label("root//pkg:target");
        let aspect_type = make_aspect_type("root//aspects:defs.bzl", "my_aspect");
        let key = AspectKey::new(target, aspect_type);

        // Verify Display includes both target and aspect
        let display = key.to_string();
        assert!(display.contains("root//pkg:target"));
        assert!(display.contains("my_aspect"));
    }

    #[test]
    fn aspect_key_equality() {
        let target1 = make_configured_label("root//pkg:t1");
        let target2 = make_configured_label("root//pkg:t1");
        let target3 = make_configured_label("root//pkg:t2");

        let aspect1 = make_aspect_type("root//a:a.bzl", "asp");
        let aspect2 = make_aspect_type("root//a:a.bzl", "asp");

        let key1 = AspectKey::new(target1, aspect1.clone());
        let key2 = AspectKey::new(target2, aspect2);
        let key3 = AspectKey::new(target3, aspect1);

        assert_eq!(key1, key2);  // Same target and aspect
        assert_ne!(key1, key3);  // Different target
    }

    #[test]
    fn aspect_key_dupe() {
        use dupe::Dupe;

        let target = make_configured_label("root//pkg:target");
        let aspect_type = make_aspect_type("root//aspects:defs.bzl", "my_aspect");
        let key = AspectKey::new(target, aspect_type);

        let duped = key.dupe();
        assert_eq!(key, duped);
        // Verify Arc sharing (cheap clone)
        assert!(Arc::ptr_eq(&key.aspect_type, &duped.aspect_type));
    }
}
```

#### 10.3: DICE Integration Tests for Module Loading

**New File:** `app/kuro_analysis/src/analysis/aspect_calculation_tests.rs`

Add async DICE tests following `kuro_build_api_tests` patterns:

```rust
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use dice::{DetectCycles, Dice, UserComputationData};
    use indoc::indoc;
    use kuro_core::bzl::ImportPath;
    use kuro_core::cells::name::CellName;
    use kuro_core::cells::paths::CellRootPathBuf;
    use kuro_core::cells::CellResolver;
    use kuro_interpreter::file_loader::LoadedModules;
    use kuro_interpreter::load_module::InterpreterCalculation;
    use kuro_interpreter::paths::module::OwnedStarlarkModulePath;
    use kuro_interpreter_for_build::interpreter::testing::Tester;
    use kuro_node::aspect_type::StarlarkAspectType;
    use kuro_node::bzl_or_bxl_path::BzlOrBxlPath;
    use kuro_util::arc_str::ArcStr;

    use crate::analysis::aspect_calculation::load_aspect_module;

    /// Helper to create a test DICE context with filesystem
    async fn test_dice_ctx() -> (ProjectRootTemp, DiceTransaction) {
        let fs = ProjectRootTemp::new().unwrap();
        // ... standard DICE setup following kuro_interpreter_for_build_tests pattern
        // Return (fs, ctx)
    }

    #[tokio::test]
    async fn test_load_aspect_module_bzl() {
        let (fs, mut ctx) = test_dice_ctx().await;

        // Create test aspect file
        fs.write_file(
            "pkg/aspects.bzl",
            indoc!(r#"
                def _impl(target, ctx):
                    return []
                my_aspect = aspect(implementation = _impl)
            "#),
        );

        let aspect_type = Arc::new(StarlarkAspectType::new(
            BzlOrBxlPath::Bzl(ImportPath::testing_new("root//pkg:aspects.bzl")),
            "my_aspect".to_owned(),
        ));

        // Test module loading
        let result = load_aspect_module(&mut ctx, &aspect_type).await;
        assert!(result.is_ok(), "Module should load successfully");

        let module = result.unwrap();
        assert!(module.env().get("my_aspect").is_some(), "Aspect should be exported");
    }

    #[tokio::test]
    async fn test_load_aspect_module_not_found() {
        let (_fs, mut ctx) = test_dice_ctx().await;

        let aspect_type = Arc::new(StarlarkAspectType::new(
            BzlOrBxlPath::Bzl(ImportPath::testing_new("root//nonexistent:aspects.bzl")),
            "my_aspect".to_owned(),
        ));

        let result = load_aspect_module(&mut ctx, &aspect_type).await;
        assert!(result.is_err(), "Should fail for nonexistent module");
    }

    #[tokio::test]
    async fn test_aspect_callable_has_aspect_type() {
        let (fs, mut ctx) = test_dice_ctx().await;

        fs.write_file(
            "pkg/aspects.bzl",
            indoc!(r#"
                def _impl(target, ctx):
                    print("Aspect running")
                    return []

                my_aspect = aspect(
                    implementation = _impl,
                    attr_aspects = ["deps"],
                )
            "#),
        );

        // Load module and verify aspect callable
        let module = ctx
            .get_loaded_module_from_import_path(&ImportPath::testing_new("root//pkg:aspects.bzl"))
            .await
            .unwrap();

        let aspect_val = module.env().get("my_aspect").unwrap();
        // Verify it's a frozen aspect callable
        assert!(aspect_val.to_repr().contains("aspect"));
    }
}
```

#### 10.4: Test Attribute Aspects Storage

**File:** `app/kuro_node/src/attrs/attr.rs`

Add tests for aspects field:

```rust
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kuro_core::bzl::ImportPath;
    use kuro_node::aspect_type::StarlarkAspectType;
    use kuro_node::bzl_or_bxl_path::BzlOrBxlPath;

    use super::*;

    fn make_aspect_type(name: &str) -> Arc<StarlarkAspectType> {
        Arc::new(StarlarkAspectType::new(
            BzlOrBxlPath::Bzl(ImportPath::testing_new("root//pkg:aspects.bzl")),
            name.to_owned(),
        ))
    }

    #[test]
    fn attribute_with_aspects() {
        let attr = Attribute::new(None, "test attr", AttrType::string())
            .with_aspects(vec![
                make_aspect_type("aspect1"),
                make_aspect_type("aspect2"),
            ]);

        assert_eq!(attr.aspects().len(), 2);
        assert_eq!(attr.aspects()[0].name, "aspect1");
        assert_eq!(attr.aspects()[1].name, "aspect2");
    }

    #[test]
    fn attribute_without_aspects() {
        let attr = Attribute::new(None, "test attr", AttrType::string());
        assert!(attr.aspects().is_empty());
    }

    #[test]
    fn attribute_aspects_preserves_module_path() {
        let aspect = make_aspect_type("my_aspect");
        let attr = Attribute::new(None, "test attr", AttrType::string())
            .with_aspects(vec![aspect.clone()]);

        let stored = &attr.aspects()[0];
        assert_eq!(stored.name, "my_aspect");
        assert!(stored.path.to_string().contains("root//pkg:aspects.bzl"));
    }
}
```

#### 10.5: Run Unit Tests

**Commands:**

```bash
# Run all unit tests for Phase 8c infrastructure
cargo test -p kuro_node -- aspect_type
cargo test -p kuro_analysis -- aspect
cargo test -p kuro_node -- attr::tests

# Run with verbose output
cargo test -p kuro_node -p kuro_analysis -- --nocapture

# Full test suite to ensure no regressions
cargo test -p kuro_node -p kuro_analysis -p kuro_interpreter_for_build
```

---

## Files Summary

### New Files

| File | Purpose |
|------|---------|
| `app/kuro_node/src/aspect_type.rs` | StarlarkAspectType definition + unit tests |
| `app/kuro_analysis/src/analysis/aspect_key.rs` | AspectKey, AspectValue DICE types + unit tests |
| `app/kuro_analysis/src/analysis/aspect_calculation.rs` | DICE Key implementation |
| `app/kuro_analysis/src/analysis/aspect_calculation_tests.rs` | DICE integration tests (Step 10) |

### Modified Files

| File | Changes |
|------|---------|
| `app/kuro_node/src/attrs/attr.rs` | Add `aspects: Vec<Arc<StarlarkAspectType>>` field + unit tests |
| `app/kuro_interpreter_for_build/src/aspect.rs` | Add `aspect_path` field, `aspect_type()` getter |
| `app/kuro_interpreter_for_build/src/attrs/attrs_global.rs` | Store aspects in label()/label_list() |
| `app/kuro_configured/src/nodes.rs` | Wire aspect computation into gather_deps() |
| `app/kuro_analysis/src/analysis/mod.rs` | Add aspect modules |
| `app/kuro_build_api/src/interpreter/rule_defs/aspect/execution.rs` | Add unit tests |

---

## Success Criteria

### Automated Verification

**Phase 8c Infrastructure (COMPLETE):**
- [x] Unit tests pass for `run_aspect_basic()` (Phase 8b completion)
- [x] Attribute struct stores aspects field
- [x] attr.label(aspects=[...]) extracts and stores aspect names
- [x] AspectKey DICE computation skeleton works
- [x] `cargo build` succeeds for all crates
- [x] `cargo test -p kuro_build_api` passes
- [x] Step 6 integration compiles (after implementation)
- [x] gather_deps() collects aspect keys correctly

**Phase 8c Execution (COMPLETE - Steps 5a-5f):**
- [x] StarlarkAspectType created in kuro_node
- [x] StarlarkAspectCallable stores aspect_path
- [x] FrozenStarlarkAspectCallable exposes aspect_type()
- [x] Attribute.aspects stores Vec<Arc<StarlarkAspectType>>
- [x] attrs_global.rs extracts full AspectType
- [x] AspectKey uses Arc<StarlarkAspectType>
- [x] load_aspect_module() implemented (follows rule loading pattern)
- [x] get_aspect_from_module() stub (TODO: full implementation needs Starlark heap access)
- [ ] Aspects execute and print output during builds (requires Steps 6-9)

### Manual Verification

**Test files created in Step 9:**

- [x] `tests/manual_test/test_aspect_8c.bzl` created
- [x] `tests/manual_test/BUILD.bazel` updated with Phase 8c tests
- [ ] Run `kuro build //tests/manual_test:c`
- [ ] Output shows "Aspect visiting: //tests/manual_test:a"
- [ ] Output shows "Aspect visiting: //tests/manual_test:b"
- [ ] Output shows "Aspect visiting: //tests/manual_test:c"
- [ ] Aspect propagates through dependency chain (depth-first order)
- [ ] `ctx.rule.kind` returns "test_rule"
- [ ] `ctx.rule.attr.deps` contains aspect results (shadow graph)
- [ ] Run `kuro build //tests/manual_test:d` (wider graph test)

### Integration Test (Optional)

- [ ] `tests/core/aspects/test_propagation.py` created
- [ ] Integration test passes with `cargo test -p kuro_core_tests`

### Unit Tests (Step 10)

Unit tests allow verifying Phase 8c infrastructure without full build environment:

- [ ] `cargo test -p kuro_node` passes (aspect_type tests)
- [ ] `cargo test -p kuro_analysis` passes (AspectKey tests)
- [ ] `cargo test -p kuro_interpreter_for_build` passes (aspect.rs tests)
- [ ] All unit tests verify DICE-based module loading works correctly

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

### Decision 1: Store aspect identity in Attribute

**Choice:** Store `AspectId = (module_path, aspect_name)` in Attribute

**UPDATED based on research (2026-01-30):**

The original plan stored only aspect names (`Vec<Arc<String>>`), but this is insufficient
because `AspectKey::compute()` needs to load the aspect callable from its defining module.

**Research findings:**
- Kuro has no global module registry
- Rules use `StarlarkRuleType = (BzlOrBxlPath, name)` for identification
- Bazel uses `AspectDescriptor = (AspectClass, AspectParameters)` where AspectClass contains `bzl_file%aspect_name`
- DICE keys cannot contain FrozenValue/FrozenModule (no Hash/Eq), but CAN use strings/paths
- DICE values CAN contain FrozenModule (via Arc, implements Dupe)

**Solution - follows existing rule pattern:**
1. Add `aspect_path: BzlOrBxlPath` field to `StarlarkAspectCallable` (like `rule_path` in rules)
2. Create `StarlarkAspectType = (BzlOrBxlPath, String)` similar to `StarlarkRuleType`
3. Store `StarlarkAspectType` in `FrozenStarlarkAspectCallable` after export/freeze
4. In `attrs_global.rs`, extract full `StarlarkAspectType` (not just name) from frozen aspect
5. Change `Attribute.aspects` from `Vec<Arc<String>>` to `Vec<StarlarkAspectType>`
6. In `AspectKey`, use module path to load module via DICE, then extract aspect by name

**Why this matches Buck2/Bazel architecture:**
- Reuses existing DICE module loading infrastructure (EvalImportKey)
- No new global registry needed
- Follows same pattern as rule analysis
- Module caching handled automatically by DICE

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
