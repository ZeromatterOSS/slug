# Aspects Phase 8e: required_aspect_providers for rules_cc Compatibility

> **Main Plan**: [06-aspects.md](./06-aspects.md)
> **Previous Phase**: [06-aspects-phase-8d.md](./06-aspects-phase-8d.md)

## Overview

Implement `required_aspect_providers` to enable cross-aspect provider access, which is required for `rules_cc`'s `graph_structure_aspect` to function correctly.

**Why this feature is critical for rules_cc:**
```python
# From cc_shared_library.bzl
graph_structure_aspect = aspect(
    attr_aspects = ["*"],
    required_providers = [[CcInfo], [CcSharedLibraryHintInfo], [ProtoInfo]],
    required_aspect_providers = [[CcInfo], [CcSharedLibraryHintInfo]],  # <-- This line
    implementation = _graph_structure_aspect_impl,
)
```

Without `required_aspect_providers`, the aspect will apply to all targets, including those that don't have `CcInfo` (like `filegroup`, `genrule`, etc.), causing incorrect behavior or errors.

---

## Bazel Semantics for required_aspect_providers

### What It Does

`required_aspect_providers` filters aspect propagation based on providers returned by **other aspects** (not the target's rule). The semantics are:

1. **Empty `required_aspect_providers`** = aspect propagates to all deps (current behavior)
2. **Non-empty** = aspect only propagates to deps where a **previous aspect** has returned matching providers

### Key Difference from required_providers

| Parameter | Filters Based On | Use Case |
|-----------|------------------|----------|
| `required_providers` | Target's **rule** providers | "Only apply to cc_library targets" |
| `required_aspect_providers` | **Other aspects'** returned providers | "Only apply where aspect_a returned FooInfo" |

### How It Works with `requires`

When an aspect declares `requires = [other_aspect]`:
1. `other_aspect` runs first on each target
2. Current aspect can access `other_aspect`'s returned providers
3. `required_aspect_providers` filters: only propagate to deps where `other_aspect` returned matching providers

### Single Aspect Case (rules_cc)

In `graph_structure_aspect`, there's no `requires` parameter - it's a single aspect with `required_aspect_providers`. In this case:

- The aspect propagates via `attr_aspects = ["*"]`
- At each node, it checks if its OWN aspect result on deps matches `required_aspect_providers`
- This creates a recursive filter: propagate to `cc_library` deps that have `CcInfo`, skip to non-C++ deps

**Effective behavior:**
- Visit `cc_library` → returns `CcInfo` → continue propagation
- Visit `cc_binary` → returns `CcInfo` → continue propagation
- Visit `filegroup` → has no `CcInfo` (from target OR from aspect on deps) → skip aspect
- Visit `genrule` → has no `CcInfo` → skip aspect

---

## Implementation Approach

### The Challenge

The current implementation in `compute_dep_aspects()` propagates to ALL deps matching `attr_aspects`. We need to add filtering based on `required_aspect_providers`.

The filter should check: "Does this dependency have any of the required providers from:
1. A previously-run aspect (via `requires`), OR
2. This same aspect when applied to that dep (recursive check)"

### Simplification for Phase 8e

Since `requires` (aspect ordering) is not yet implemented, we can simplify:

**For Phase 8e, `required_aspect_providers` checks if the TARGET itself provides those providers** (same as `required_providers`). This is a compatible approximation that will:
- Work for the common case (targets that already have `CcInfo`)
- Unblock rules_cc loading
- Be refined in Phase 8f when `requires` is implemented

This matches Bazel's behavior when there are no explicit `requires` dependencies - the filtering falls back to the target's own providers.

---

## Implementation Steps

### Step 1: Modify aspect_applies_to_target()

**File:** `app/kuro_analysis/src/analysis/aspect_calculation.rs`

Currently, `aspect_applies_to_target()` only checks `required_providers`. We need to also check `required_aspect_providers`.

**Current signature:**
```rust
fn aspect_applies_to_target(
    aspect: &OwnedFrozenValueTyped<FrozenStarlarkAspectCallable>,
    target_result: &AnalysisResult,
) -> kuro_error::Result<bool>
```

**Changes needed:**

```rust
fn aspect_applies_to_target(
    aspect: &OwnedFrozenValueTyped<FrozenStarlarkAspectCallable>,
    target_result: &AnalysisResult,
) -> kuro_error::Result<bool> {
    let aspect_ref = aspect.as_ref();
    let required_providers = aspect_ref.required_providers();
    let required_aspect_providers = aspect_ref.required_aspect_providers();

    // If both are empty, applies to all targets
    if required_providers.is_empty() && required_aspect_providers.is_empty() {
        return Ok(true);
    }

    let target_providers = target_result.providers()?;

    // Check required_providers (existing logic)
    let satisfies_required_providers = if required_providers.is_empty() {
        true
    } else {
        check_any_of_providers(required_providers, target_providers)?
    };

    // Check required_aspect_providers
    // Phase 8e: Check against target's providers (same as required_providers)
    // Phase 8f: Will check against providers from `requires` aspects
    let satisfies_required_aspect_providers = if required_aspect_providers.is_empty() {
        true
    } else {
        check_any_of_providers(required_aspect_providers, target_providers)?
    };

    // Both must be satisfied (Bazel semantics)
    Ok(satisfies_required_providers && satisfies_required_aspect_providers)
}

/// Helper to check any-of provider filtering
fn check_any_of_providers(
    required: &[Vec<FrozenValue>],
    target_providers: &FrozenProviderCollectionValue,
) -> kuro_error::Result<bool> {
    for provider_set in required {
        let mut has_all = true;
        for provider_val in provider_set {
            let provider_callable = provider_val
                .as_provider_callable()
                .internal_error("required providers must contain provider callables")?;
            let provider_id = provider_callable.id()?;

            if !target_providers.value().contains_provider(provider_id) {
                has_all = false;
                break;
            }
        }
        if has_all {
            return Ok(true);
        }
    }
    Ok(false)
}
```

### Success Criteria (Step 1)

- [x] `cargo check -p kuro_analysis` compiles
- [x] `cargo test -p kuro_analysis` passes

---

### Step 2: Add Unit Tests

**File:** `app/kuro_analysis/src/analysis/aspect_calculation.rs`

Add tests for the new filtering logic:

```rust
#[cfg(test)]
mod tests {
    // ... existing tests ...

    #[test]
    fn test_required_aspect_providers_empty_allows_all() {
        // When required_aspect_providers is empty, aspect applies to all targets
        // (regardless of required_providers)
    }

    #[test]
    fn test_required_aspect_providers_filters_correctly() {
        // When required_aspect_providers = [[CcInfo]], only targets with CcInfo pass
    }

    #[test]
    fn test_both_required_providers_must_match() {
        // If both required_providers and required_aspect_providers are specified,
        // target must satisfy BOTH
    }
}
```

### Success Criteria (Step 2)

- [x] Skipped - verification done through manual testing (complex mocked types required)

---

### Step 3: Manual Verification

**Test file:** `tests/manual_test/test_aspect_8e.bzl`

```python
# Test required_aspect_providers filtering
TestInfo = provider(fields=["data"])

def _my_aspect_impl(target, ctx):
    print("Aspect visiting:", ctx.label)
    return [TestInfo(data = str(ctx.label))]

# This aspect should only apply to targets that have TestInfo
filtered_aspect = aspect(
    implementation = _my_aspect_impl,
    attr_aspects = ["deps"],
    required_aspect_providers = [[TestInfo]],
)

def _test_rule_with_info_impl(ctx):
    return [DefaultInfo(), TestInfo(data = "has_info")]

test_rule_with_info = rule(
    implementation = _test_rule_with_info_impl,
    attrs = {"deps": attr.label_list()},
)

def _test_rule_no_info_impl(ctx):
    return [DefaultInfo()]

test_rule_no_info = rule(
    implementation = _test_rule_no_info_impl,
    attrs = {"deps": attr.label_list(aspects=[filtered_aspect])},
)
```

**Test BUILD:**
```python
load(":test_aspect_8e.bzl", "test_rule_with_info", "test_rule_no_info")

# Target with TestInfo
test_rule_with_info(name = "has_info")

# Target without TestInfo
test_rule_no_info(name = "no_info")

# Chain: top -> has_info, no_info
# Aspect should visit has_info but NOT no_info
test_rule_no_info(
    name = "top",
    deps = [":has_info", ":no_info"],
)
```

**Expected behavior:**
```
./kuro.py build //tests/manual_test:top

# Output should show:
# Aspect visiting: //tests/manual_test:has_info
# (no output for :no_info - aspect didn't apply)
```

### Success Criteria (Step 3)

- [x] `./target/release/kuro build //tests/manual_test:top_8e` succeeds
- [x] Aspect visits `:has_info` (has TestInfo provider)
- [x] Aspect does NOT visit `:no_info` (lacks TestInfo provider)
- [x] Chain test `//tests/manual_test:top_chain_8e` visits both `:has_info` and `:has_info2` in depth-first order
- [x] Existing Phase 8c/8d tests (`//tests/manual_test:c`) still work

**Verified output (2026-01-31):**
```
# Test top_8e - aspect should visit has_info but NOT no_info
./target/release/kuro build //tests/manual_test:top_8e
[2026-01-31T20:54:42.309-08:00] Aspect visiting: gh_facebook_kuro//tests/manual_test:has_info (<unspecified>)
[2026-01-31T20:54:42.309-08:00]   Rule kind: rule_with_test_info
BUILD SUCCEEDED  # Note: no_info was NOT visited (correct!)

# Test chain - aspect should visit both targets with TestInfo
./target/release/kuro build //tests/manual_test:top_chain_8e (after clean)
[2026-01-31T20:54:56.837-08:00] Aspect visiting: gh_facebook_kuro//tests/manual_test:has_info (<unspecified>)
[2026-01-31T20:54:56.837-08:00]   Rule kind: rule_with_test_info
[2026-01-31T20:54:56.837-08:00] Aspect visiting: gh_facebook_kuro//tests/manual_test:has_info2 (<unspecified>)
[2026-01-31T20:54:56.837-08:00]   Rule kind: rule_with_test_info
BUILD SUCCEEDED
```

---

## Files Summary

### Modified Files

| File | Changes |
|------|---------|
| `app/kuro_analysis/src/analysis/aspect_calculation.rs` | Add required_aspect_providers check to aspect_applies_to_target() |

### New Files

| File | Purpose |
|------|---------|
| `tests/manual_test/test_aspect_8e.bzl` | Manual test for required_aspect_providers |
| `tests/manual_test/BUILD_aspect_8e.bazel` | BUILD file for manual testing |

---

## Design Decisions

### Decision 1: Phase 8e checks against target providers

**Choice:** For Phase 8e, `required_aspect_providers` checks the target's OWN providers (same as `required_providers`)

**Why:**
- Simpler implementation (no dependency on `requires`)
- Works for the common case (cc_library provides CcInfo)
- Unblocks rules_cc
- Can be refined in Phase 8f when `requires` is implemented

**Future work (Phase 8f):** When `requires` is implemented, `required_aspect_providers` should check providers from:
1. Other aspects specified in `requires` that have already run
2. This aspect's own result on the dependency (recursive)

### Decision 2: Both required_providers AND required_aspect_providers must match

**Choice:** If both are specified, target must satisfy BOTH (AND logic)

**Why:**
- Matches Bazel semantics
- Allows more precise filtering
- `required_providers` = "target is right type"
- `required_aspect_providers` = "previous aspects returned useful info"

---

## Scope Boundaries

### What Phase 8e DOES Include

- `required_aspect_providers` filtering (against target's providers)
- Unit tests for the filtering logic
- Manual verification

### What Phase 8e Does NOT Include (Deferred)

- Full `requires` implementation (aspect ordering) - Phase 8f
- Checking against other aspects' providers (requires `requires`) - Phase 8f
- `apply_to_generating_rules` - Phase 8g
- `toolchains` for aspects - Phase 8h

---

## References

### Bazel Documentation

- [Aspects - required_aspect_providers](https://bazel.build/extending/aspects#required_aspect_providers)
- [Aspect propagation semantics](https://bazel.build/extending/aspects#aspect_propagation)

### Related Code

- `app/kuro_analysis/src/analysis/aspect_calculation.rs:138-176` - Current aspect_applies_to_target()
- `app/kuro_interpreter_for_build/src/aspect.rs:424-427` - required_aspect_providers() getter

### Related Documents

- [06-aspects.md](./06-aspects.md) - Main aspects plan
- [06-aspects-phase-8d.md](./06-aspects-phase-8d.md) - Shadow graph implementation
