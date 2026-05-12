# Aspects Phase 8b: Aspect Context and Basic Execution

> **Status: SUPERSEDED.** Use [06-aspects.md](./06-aspects.md) and the
> `06-aspects-phase-*` files for current aspect status.
>
> **Main Plan**: [08-aspects.md](./08-aspects.md)
> **Research**: [BXL vs Bazel Aspects Comparison](../../research/2026-01-30-bxl-vs-bazel-aspects-comparison.md)

## Overview

Implement AspectContext and basic aspect execution, enabling aspect implementation functions to be called with proper context. This phase builds on Phase 8a's stub implementation.

**Why this phase is critical:** Phase 8a allows `aspect()` calls to parse and unblocked rules_cc loading. However, aspects don't actually execute - their implementation functions are never called. Phase 8b enables basic aspect execution (without shadow graph propagation) as the foundation for Phase 8c.

---

## Key Insight: Aspects vs BXL

From [research document](../../research/2026-01-30-bxl-vs-bazel-aspects-comparison.md):

| Dimension | BXL | Bazel Aspects |
|-----------|-----|---------------|
| **Paradigm** | Imperative scripting | Declarative graph augmentation |
| **Execution** | Separate command (`slug bxl`) | During analysis phase |
| **Output** | Artifacts, stdout, JSON | Providers on shadow graph |
| **Integration** | External to build | Internal to build graph |

**Key Finding**: BXL cannot replace aspects because:
1. Aspects participate in analysis phase and return providers that rules consume
2. Shadow graph is computed implicitly based on `attr_aspects`
3. External rules (rules_cc, rules_python) use aspects internally

**Conclusion**: Aspects must be implemented natively as a first-class feature.

---

## Current State (Phase 8a Complete)

**What exists:**
- `StarlarkAspectCallable` stores all aspect parameters (implementation, attr_aspects, attrs, etc.)
- `FrozenStarlarkAspectCallable` is the frozen version (INCOMPLETE - missing fields)
- `aspect()` global function registered in .bzl files
- Aspects can be attached to `attr.label(aspects=[...])` and `attr.label_list(aspects=[...])`
- rules_cc loads successfully with stub aspects

**What's missing:**
- AspectContext object with `ctx.rule.kind`, `ctx.rule.attr`, `ctx.label`, etc.
- Target provider wrapper for `target[SomeInfo]` syntax
- Aspect implementation invocation
- Provider validation (no DefaultInfo allowed from aspects)

**Location:** `app/slug_interpreter_for_build/src/aspect.rs`

---

## Implementation Steps

### Step 1: Fix FrozenStarlarkAspectCallable (Bug Fix)

**File:** `app/slug_interpreter_for_build/src/aspect.rs`

The frozen version (lines 283-300) is missing critical fields:

| Field | Currently | Needed |
|-------|-----------|--------|
| `required_providers` | Missing | For Phase 8c filtering |
| `required_aspect_providers` | Missing | For Phase 8d cross-aspect |
| `provides` | Missing | For validation |
| `requires` | Missing | For Phase 8d ordering |
| `exec_compatible_with` | Missing | For execution platforms |
| `subrules` | Missing | For subrule support |

**Changes:**
1. Add missing fields to `FrozenStarlarkAspectCallable` struct
2. Update `Freeze` impl (lines 304-325) to freeze new fields
3. Add accessor methods for all new fields

---

### Step 2: Create AspectRuleInfo (ctx.rule object)

**New File:** `app/slug_build_api/src/interpreter/rule_defs/aspect/rule_info.rs`

Provides access to the underlying rule's information:

```rust
#[derive(Debug, ProvidesStaticType, Trace, NoSerialize, Allocative)]
pub struct AspectRuleInfo<'v> {
    /// Rule type name (e.g., "cc_library", "py_binary")
    kind: String,
    /// Rule's attributes as a Starlark struct
    attr: ValueOfUnchecked<'v, StructRef<'static>>,
}
```

**Members:**
- `ctx.rule.kind` → `String` - The kind of rule being visited
- `ctx.rule.attr` → `struct` - The rule's attributes (Phase 8b: plain attrs; Phase 8c: resolved to aspect results)

---

### Step 3: Create AspectContext

**New File:** `app/slug_build_api/src/interpreter/rule_defs/aspect/context.rs`

Model after `AnalysisContext` (context.rs:176-260):

```rust
#[derive(ProvidesStaticType, Debug, Trace, NoSerialize, Allocative)]
pub struct AspectContext<'v> {
    /// Aspect-specific attributes (from aspect's attrs={} parameter)
    attr: Option<ValueOfUnchecked<'v, StructRef<'static>>>,
    /// Actions registry (same as rule ctx)
    pub actions: ValueTyped<'v, AnalysisActions<'v>>,
    /// Target's label
    label: ValueTyped<'v, StarlarkConfiguredProvidersLabel>,
    /// Rule information (ctx.rule access)
    rule: ValueTyped<'v, AspectRuleInfo<'v>>,
    /// Configuration fragments
    fragments: ConfigurationFragments,
}
```

**Members (matching Bazel API):**

| Member | Type | Description |
|--------|------|-------------|
| `ctx.attr` | `struct` | Aspect-specific attributes |
| `ctx.actions` | `actions` | Action registration (same as rule ctx) |
| `ctx.label` | `Label` | Target's label |
| `ctx.rule` | `AspectRuleInfo` | Rule information (`ctx.rule.kind`, `ctx.rule.attr`) |
| `ctx.fragments` | `fragments` | Configuration fragments |

**Pattern:** Follow `RefAnalysisContext` wrapper pattern for unpacking.

---

### Step 4: Create AspectTargetProviders (target argument wrapper)

**New File:** `app/slug_build_api/src/interpreter/rule_defs/aspect/target_providers.rs`

The `target` argument passed to aspects must support:
- `target[SomeInfo]` - Get provider value
- `SomeInfo in target` - Check if provider exists

```rust
#[derive(Debug, ProvidesStaticType, Trace, NoSerialize, Allocative)]
pub struct AspectTargetProviders<'v> {
    /// The underlying provider collection
    providers: FrozenProviderCollectionValueRef<'v>,
    /// Target label (for error messages)
    label: ConfiguredTargetLabel,
}

#[starlark_value(type = "Target")]
impl<'v> StarlarkValue<'v> for AspectTargetProviders<'v> {
    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        // Delegate to provider collection for target[SomeInfo]
    }

    fn is_in(&self, other: Value<'v>) -> starlark::Result<bool> {
        // Check if provider exists for `SomeInfo in target`
    }
}
```

---

### Step 5: Add Aspect Module Structure

**New File:** `app/slug_build_api/src/interpreter/rule_defs/aspect/mod.rs`

```rust
pub mod context;
pub mod rule_info;
pub mod target_providers;

pub use context::AspectContext;
pub use rule_info::AspectRuleInfo;
pub use target_providers::AspectTargetProviders;
```

**Modify:** `app/slug_build_api/src/interpreter/rule_defs/mod.rs`
- Add `pub mod aspect;`

---

### Step 6: Add Aspect Provider Validation

**Modify:** `app/slug_build_api/src/interpreter/rule_defs/provider/collection.rs`

Aspects have different provider rules than rules:
- Aspects **cannot** return `DefaultInfo`
- Aspects don't **require** `DefaultInfo`

```rust
/// Error types for aspect provider validation
#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
pub enum AspectProviderError {
    #[error("Aspects cannot return DefaultInfo provider")]
    CannotReturnDefaultInfo,
}

impl<'v> ProviderCollection<'v> {
    /// Create provider collection from aspect return value.
    pub fn try_from_aspect_value(value: Value<'v>) -> slug_error::Result<Self> {
        let providers = Self::try_from_value_impl(value)?;

        // Aspects cannot return DefaultInfo
        if providers.contains_key(DefaultInfoCallable::provider_id()) {
            return Err(AspectProviderError::CannotReturnDefaultInfo.into());
        }

        Ok(ProviderCollection { providers })
    }
}
```

---

### Step 7: Create Basic Aspect Execution Function

**New File:** `app/slug_analysis/src/analysis/aspect_execution.rs`

For Phase 8b, create minimal execution path for testing. Full DICE integration comes in Phase 8c.

```rust
/// Run an aspect on a target (Phase 8b - basic execution, no propagation)
pub fn run_aspect_basic<'v>(
    heap: Heap<'v>,
    target_providers: FrozenProviderCollectionValueRef<'v>,
    target_label: &ConfiguredTargetLabel,
    target_node: ConfiguredTargetNodeRef<'_>,
    aspect: &FrozenStarlarkAspectCallable,
    eval: &mut Evaluator<'v, '_, '_>,
    registry: AnalysisRegistry<'v>,
    digest_config: DigestConfig,
) -> slug_error::Result<ProviderCollection<'v>> {
    // 1. Get rule kind from target node
    let rule_kind = target_node.rule_type().name().to_owned();

    // 2. Resolve rule attributes (basic - no aspect results yet)
    let rule_attr = node_to_attrs_struct(target_node, &resolution_ctx)?;

    // 3. Resolve aspect-specific attributes (from aspect.attrs())
    let aspect_attr = resolve_aspect_attrs(aspect, &resolution_ctx)?;

    // 4. Create AspectRuleInfo
    let rule_info = heap.alloc_typed(AspectRuleInfo::new(rule_kind, rule_attr));

    // 5. Create AspectContext
    let ctx = AspectContext::prepare(
        heap, aspect_attr, target_label, rule_info, registry, digest_config
    );

    // 6. Wrap target providers
    let target = heap.alloc(AspectTargetProviders::new(target_providers, target_label.dupe()));

    // 7. Invoke implementation: impl(target, ctx)
    let result = eval.eval_function(
        aspect.implementation().to_value(),
        &[target, ctx.to_value()],
        &[],
    )?;

    // 8. Validate and return providers
    ProviderCollection::try_from_aspect_value(result)
}
```

---

## Files Summary

### New Files

| File | Purpose |
|------|---------|
| `app/slug_build_api/src/interpreter/rule_defs/aspect/mod.rs` | Module organization |
| `app/slug_build_api/src/interpreter/rule_defs/aspect/context.rs` | AspectContext Starlark type |
| `app/slug_build_api/src/interpreter/rule_defs/aspect/rule_info.rs` | AspectRuleInfo (ctx.rule) |
| `app/slug_build_api/src/interpreter/rule_defs/aspect/target_providers.rs` | Target provider wrapper |
| `app/slug_analysis/src/analysis/aspect_execution.rs` | Basic execution function |

### Modified Files

| File | Changes |
|------|---------|
| `app/slug_interpreter_for_build/src/aspect.rs` | Add missing fields to FrozenStarlarkAspectCallable |
| `app/slug_build_api/src/interpreter/rule_defs/mod.rs` | Add `pub mod aspect;` |
| `app/slug_build_api/src/interpreter/rule_defs/provider/collection.rs` | Add `try_from_aspect_value()` |

---

## Success Criteria

### Automated Verification

- [x] `FrozenStarlarkAspectCallable` preserves all fields from unfrozen version
- [x] `AspectContext` type compiles and can be created
- [x] `AspectRuleInfo` provides `kind` and `attr` members
- [x] `AspectTargetProviders` supports `target[SomeInfo]` and `SomeInfo in target`
- [x] `try_from_aspect_value()` rejects DefaultInfo
- [x] All crates build: `cargo build -p slug_build_api -p slug_interpreter_for_build`

### Manual Verification

Create test in `tests/manual_test/`:

```python
# test_aspect_8b.bzl
def _test_aspect_impl(target, ctx):
    print("Phase 8b test: aspect invoked!")
    print("  ctx.label:", ctx.label)
    print("  ctx.rule.kind:", ctx.rule.kind)
    if DefaultInfo in target:
        print("  target has DefaultInfo")
    return []  # Return empty list (aspects cannot return DefaultInfo)

test_aspect = aspect(
    implementation = _test_aspect_impl,
    attr_aspects = ["deps"],
)
```

**Verification:**
- [ ] Aspect implementation function can be called
- [ ] `ctx.rule.kind` returns correct rule kind
- [ ] `ctx.rule.attr` provides access to rule attributes
- [ ] `ctx.label` returns target label
- [ ] Aspect can return providers (list)
- [ ] Simple aspect with no propagation works

---

## Phase 8b Scope Boundaries

### What Phase 8b DOES Include

- AspectContext with all basic members
- AspectRuleInfo for `ctx.rule.kind` and `ctx.rule.attr`
- AspectTargetProviders for `target[SomeInfo]` syntax
- Basic aspect execution on a single target
- Provider validation (no DefaultInfo from aspects)

### What Phase 8b Does NOT Include (Deferred to Phase 8c/8d)

- Shadow graph propagation via `attr_aspects`
- `ctx.rule.attr` with aspect-resolved dependencies
- Full DICE integration (`AspectKey` for caching)
- `required_providers` filtering
- Integration with rule analysis hooks
- `required_aspect_providers` (Phase 8d)
- `requires` aspect ordering (Phase 8d)
- Toolchain resolution for aspects (Phase 8d)

---

## References

### Implementation Patterns

- `app/slug_build_api/src/interpreter/rule_defs/context.rs` - AnalysisContext pattern to follow
- `app/slug_analysis/src/analysis/env.rs` - RuleSpec trait and run_analysis pattern
- `app/slug_build_api/src/interpreter/rule_defs/provider/collection.rs` - Provider collection handling

### Bazel Source (for reference)

- `StarlarkAspect.java` - Aspect definition
- `StarlarkAspectContext.java` - Context implementation
- `AspectFunction.java` - DICE computation (Phase 8c)

### Related Documents

- [08-aspects.md](./08-aspects.md) - Main aspects plan
- [BXL vs Aspects Research](../../research/2026-01-30-bxl-vs-bazel-aspects-comparison.md) - Why aspects must be native
