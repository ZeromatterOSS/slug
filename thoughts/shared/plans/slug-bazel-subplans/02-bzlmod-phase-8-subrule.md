# bzlmod Phase 8: Full subrule() Implementation (Future)

> **Main Plan**: [02-bzlmod.md](./02-bzlmod.md)

## Overview

Complete the `subrule()` implementation to match Bazel's semantics per the design document.

**Design Reference**: `thoughts/shared/research/bazel-subrule-design.md`
(Original: https://docs.google.com/document/d/1RbNC88QieKvBEwir7iV5zZU08AaMlOzxhVkPnmKDedQ)

---

## Current Status: STUB ONLY

The current implementation (`app/slug_interpreter_for_build/src/subrule.rs`) is a minimal stub.

**What's implemented:**
- `subrule()` Starlark global function
- `StarlarkSubruleCallable` - unfrozen callable with RefCell for name
- `FrozenStarlarkSubruleCallable` - frozen version
- Accepts parameters: `implementation`, `attrs`, `fragments`, `toolchains`, `subrules`, `doc`
- Validates called only in `.bzl` files
- Export_as() sets name when assigned to variable
- Basic documentation generation
- Accessors: `name()`, `attrs()`, `fragments()`, `toolchains()`, `implementation()`

**What's NOT implemented:**
- SubruleContext - currently just passes args through to implementation
- `subrules` parameter on `rule()` function
- `subrules` parameter on `aspect()` function
- Attribute lifting (subrule attrs -> parent rule attrs)
- Attribute name mangling for collision avoidance
- Implicit dep injection as keyword arguments
- Runtime validation that subrule is declared in parent's subrules list
- Toolchain resolution for subrules
- Exec group support for subrules
- Nested subrule composition

---

## Phase 8a: SubruleContext Implementation

**Goal**: Create a restricted context object for subrule implementations.

**SubruleContext members (per design doc):**

| Member           | Description                                                       |
| ---------------- | ----------------------------------------------------------------- |
| `ctx.actions`    | For creating actions (implicit toolchain/exec_group from subrule) |
| `ctx.toolchains` | Access to declared toolchains                                     |
| `ctx.label`      | Target label for naming artifacts                                 |
| `ctx.fragments`  | Configuration fragments (possibly - helps encapsulation)          |

**Members NOT provided (encapsulation):**

| Member                                                | Reason                                                              |
| ----------------------------------------------------- | ------------------------------------------------------------------- |
| `ctx.attr`, `ctx.file`, `ctx.files`, `ctx.executable` | Use function parameters instead                                     |
| `ctx.bin_dir`                                         | Use `file.root.path` on declared artifacts                          |
| `ctx.exec_groups`                                     | Only one exec group per subrule                                     |
| `ctx.rule`, `ctx.aspect_ids`                          | Breaks encapsulation                                                |
| `ctx.outputs`                                         | Outputs must be public attributes, subrules can't have public attrs |
| `ctx.split_attr`                                      | No split transitions on implicit deps                               |
| `ctx.features`, `ctx.disabled_features`               | Combine rule public attrs - subrule doesn't have public attrs       |
| `ctx.expand_location`                                 | Collects labels from public attrs - subrule has no public attrs     |
| `ctx.var`                                             | Collects from hardcoded public attrs - breaks encapsulation         |

**Implementation approach:**

```rust
#[derive(Debug, ProvidesStaticType, Trace, NoSerialize, Allocative)]
pub struct SubruleContext<'v> {
    parent_ctx: Value<'v>,
    toolchains: Vec<String>,
    exec_group: Option<String>,
    fragments: Vec<String>,
}
```

**Files to Create/Modify:**

- Create `app/slug_build_api/src/interpreter/rule_defs/subrule_ctx.rs`
- Modify `app/slug_build_api/src/interpreter/rule_defs/mod.rs` to register
- May need to modify `AnalysisActions` to support implicit exec_group

---

## Phase 8b: Attribute Lifting and rule() Integration

**Goal**: When a rule declares `subrules=[my_subrule]`, lift subrule's implicit deps.

**Requirements:**

- Subrule attrs MUST start with `_` (private/implicit)
- Subrule attrs MUST be `attr.label` or `attr.label_list` only
- No other attr types allowed (no strings, ints, etc.)
- Late-bound default labels ARE allowed
- Computed defaults are NOT allowed

**Attribute Name Mangling:**

```
# Format: {bzl_path}%{subrule_name}${attr_name}
@rules_java//java_common/compile.bzl%compile$java_toolchain
```

**Files to Modify:**

- `app/slug_interpreter_for_build/src/rule.rs` - Add `subrules` parameter to `rule()`
- `app/slug_interpreter_for_build/src/subrule.rs` - Add attr validation, generate mangled names
- `app/slug_node/src/rule.rs` - Store subrule references and lifted attrs
- `app/slug_node/src/attrs/spec.rs` - Support lifted attrs in AttributeSpec

---

## Phase 8c: Call Semantics

**Goal**: Implement proper subrule invocation from rule implementations.

**Required behavior:**

1. Get current analysis context from evaluator's extra data
2. Verify subrule is declared in parent rule's `subrules=[]` list
3. Create SubruleContext wrapping parent context
4. Resolve implicit deps from lifted attrs on the target
5. Call implementation with SubruleContext + implicit deps
6. Return implementation's return value

**Error cases:**

| Error | Message |
|-------|---------|
| Subrule not declared | "Subrule `{name}` was called but not declared in rule's `subrules` parameter" |
| Called outside rule impl | "Subrule can only be called during rule or aspect analysis" |
| Attr resolution failure | "Could not resolve implicit dependency `{attr}` for subrule `{name}`" |

**Files to Modify:**

- `app/slug_interpreter_for_build/src/subrule.rs` - Rewrite `FrozenStarlarkSubruleCallable::invoke()`
- `app/slug_build_api/src/interpreter/rule_defs/` - Expose analysis context to subrule

---

## Phase 8d: Toolchain and Exec Group Support

**Goal**: Subrules can declare their own toolchains and execution requirements.

**Exec group handling (per design doc):**

| Scenario | Behavior |
|----------|----------|
| `exec_compatible_with` set | Subrule auto-creates separate exec_group for its actions |
| `exec_group` set | Uses globally named exec_group (e.g., "cpp_link") |
| Neither set | Uses parent rule's default exec_group |

**MVP scope:**

1. Merge subrule toolchains into parent rule's toolchain requirements
2. Make `ctx.toolchains` on SubruleContext return subrule's declared toolchains
3. Defer exec_compatible_with and exec_group to later phase

---

## Phase 8e: Aspect Support

**Goal**: Aspects can also use subrules.

**Current status**: No `aspect()` function found in slug_interpreter_for_build. Bazel compatibility likely requires implementing aspects before this phase.

**When aspects are implemented:**

- Add `subrules` parameter to `aspect()` function (same as rule)
- Aspects should be able to call declared subrules during analysis
- SubruleContext works the same in aspect context

---

## Phase 8f: Nested Subrule Support

**Goal**: Subrules can declare other subrules they depend on.

**Behavior per design doc:**

- Subrule can have its own `subrules=[]` parameter
- Nested subrules' attrs are transitively lifted to the top-level rule
- Name mangling handles nested paths:
  ```
  @repo//pkg:file.bzl%outer_subrule%inner_subrule$_some_attr
  ```

**Files to Modify:**

- `app/slug_interpreter_for_build/src/subrule.rs` - Process nested subrules during freeze
- `app/slug_interpreter_for_build/src/rule.rs` - Recursively lift attrs from nested subrules

---

## Success Criteria

### Automated Verification

```bash
cargo test -p slug_interpreter_for_build
cargo test -p slug_build_api
```

**Unit tests to add:**

- [ ] `subrule.rs`: Validate attrs must start with `_`
- [ ] `subrule.rs`: Validate attrs must be `label` or `label_list`
- [ ] `subrule.rs`: Attr name mangling produces correct format
- [ ] `rule.rs`: `subrules` parameter accepted
- [ ] `rule.rs`: Lifted attrs added to rule's AttributeSpec
- [ ] `subrule_ctx.rs`: Only allowed members are accessible
- [ ] `subrule_ctx.rs`: Disallowed members raise appropriate errors
- [ ] Integration: Subrule not declared -> clear error message
- [ ] Integration: Subrule called outside analysis -> clear error message

### Manual Verification

**Test 1: Basic subrule invocation**

```python
def _my_subrule_impl(ctx, *, _helper):
    print("subrule invoked with helper:", _helper)
    return struct(value = 42)

my_subrule = subrule(
    implementation = _my_subrule_impl,
    attrs = {"_helper": attr.label(default = "//tools:helper")},
)

def _my_rule_impl(ctx):
    result = my_subrule()
    print("subrule returned:", result.value)
    return [DefaultInfo()]

my_rule = rule(
    implementation = _my_rule_impl,
    subrules = [my_subrule],
)
```

**Test 2: Undeclared subrule error**

```python
def _rule_impl(ctx):
    other_subrule()  # Should ERROR - not in subrules list
    return [DefaultInfo()]

my_rule = rule(
    implementation = _rule_impl,
    subrules = [],  # Empty - other_subrule not declared
)
```

---

## Implementation Priority

For rules_cc compatibility, the minimum viable implementation needs:

1. **Phase 8a** - Basic SubruleContext with `ctx.actions`, `ctx.label`
2. **Phase 8b** - Attribute lifting (implicit deps) + rule() subrules parameter
3. **Phase 8c** - Basic call semantics (inject ctx + implicit deps)

**Effort Estimates:**

| Phase | Scope | Files | Complexity |
|-------|-------|-------|------------|
| 8a | SubruleContext | 2 new | Medium |
| 8b | rule() integration | 3-4 modify | Medium-High |
| 8c | Call semantics | 2 modify | High |
| 8d | Toolchains | 2-3 modify | Medium |
| 8e | Aspects | Depends on aspect impl | Low |
| 8f | Nested subrules | 2 modify | Medium |

**Recommended Order:** 8b -> 8c -> 8a -> defer 8d/8e/8f

---

## References

**Design Documentation:**
- **Design Doc**: `thoughts/shared/research/bazel-subrule-design.md`
- **Original Google Doc**: https://docs.google.com/document/d/1RbNC88QieKvBEwir7iV5zZU08AaMlOzxhVkPnmKDedQ

**Bazel Source Files:**

| File | Purpose |
|------|---------|
| `StarlarkSubrule.java` | Main subrule implementation |
| `StarlarkSubruleContext.java` | SubruleContext implementation |
| `SubruleFactory.java` | Subrule factory |
| `SubruleTest.java` | Comprehensive tests |

**Real-world Usage Examples:**
- `rules_cc`: `cc/private/rules_impl/fdo/fdo_context.bzl`
- `rules_java`: `java_common/compile.bzl`

**Slug Implementation Files:**
- `app/slug_interpreter_for_build/src/subrule.rs` - Current stub implementation
- `app/slug_interpreter_for_build/src/rule.rs` - rule() function (needs subrules param)
- `app/slug_node/src/rule.rs` - Rule struct (needs subrules field)
- `app/slug_build_api/src/interpreter/rule_defs/` - Analysis context (needs SubruleContext)
