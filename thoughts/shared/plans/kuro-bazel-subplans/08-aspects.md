# Phase 8: Bazel Aspects Implementation

> **Parent Plan**: [Kuro Bazel-Compatible Build Tool](../2026-01-21-kuro-bazel-compatible-build-tool.md)
> **Blocks**: [02-bzlmod.md](./02-bzlmod.md) - rules_cc loading requires `aspect()` built-in

This sub-plan covers implementing Bazel's `aspect()` built-in function, which is required for loading `rules_cc` from the BCR.

---

## Overview

### What Are Aspects?

Aspects are a Bazel feature that allows additional computation to run over a target's dependency graph. When an aspect is attached to a dependency attribute, it automatically propagates through the graph, creating a "shadow graph" where each node runs the aspect's implementation function.

**Key use case in rules_cc**: The `graph_structure_aspect` in `cc_shared_library.bzl` traverses the dependency graph to collect linking information:

```python
graph_structure_aspect = aspect(
    attr_aspects = ["*"],
    required_providers = [[CcInfo], [CcSharedLibraryHintInfo], [ProtoInfo]],
    required_aspect_providers = [[CcInfo], [CcSharedLibraryHintInfo]],
    implementation = _graph_structure_aspect_impl,
)
```

### Why This Is Needed

- **Blocker**: `@rules_cc//cc:defs.bzl` cannot load because `cc_shared_library.bzl:828` calls `aspect()`
- **No Buck2 equivalent**: Buck2 uses different mechanisms (anon targets, subtargets) - aspects must be implemented from scratch
- **Wide usage**: Many Bazel rules use aspects for IDE integration, linting, code coverage, license compliance, etc.

---

## Bazel Aspect API Reference

### `aspect()` Function Signature

```python
aspect(
    implementation,           # function(target, ctx) -> list[Provider]
    attr_aspects = [],        # list[str] - attributes to propagate through
    attrs = {},               # dict[str, Attribute] - aspect-specific attributes
    required_providers = [],  # list[list[Provider]] - filter by providers
    required_aspect_providers = [],  # list[list[Provider]] - access other aspects
    provides = [],            # list[Provider] - providers this aspect returns
    requires = [],            # list[Aspect] - aspects that must run first
    fragments = [],           # list[str] - configuration fragments
    toolchains = [],          # list[str|Label] - required toolchains
    doc = None,               # str - documentation
    apply_to_generating_rules = False,  # bool - apply to generating rule of output files
    exec_compatible_with = [],  # list[str] - execution platform constraints
    exec_groups = None,       # dict - execution groups
    subrules = [],            # list[Subrule] - subrules used by aspect
)
```

### Implementation Function

The aspect implementation receives two arguments:
- `target`: The target the aspect is being applied to
- `ctx`: An aspect context object

```python
def _my_aspect_impl(target, ctx):
    # Access the target's providers
    if CcInfo in target:
        cc_info = target[CcInfo]

    # Access the rule's attributes
    rule_kind = ctx.rule.kind
    deps = ctx.rule.attr.deps

    # Access aspect-applied dependencies (shadow graph)
    # deps now contains aspect results, not original targets
    for dep in deps:
        if MyAspectInfo in dep:
            # Process aspect info from dependency
            pass

    # Create actions if needed
    ctx.actions.write(...)

    # Return providers (NOT DefaultInfo)
    return [MyAspectInfo(...)]
```

### Aspect Context (`ctx`)

| Attribute | Type | Description |
|-----------|------|-------------|
| `ctx.rule.kind` | `str` | The kind of rule being visited |
| `ctx.rule.attr` | `struct` | The rule's attributes (resolved to aspect results) |
| `ctx.label` | `Label` | The target's label |
| `ctx.actions` | `actions` | Action registration (same as rule ctx) |
| `ctx.fragments` | `fragments` | Configuration fragments |
| `ctx.attr` | `struct` | Aspect-specific attributes |
| `ctx.toolchains` | `dict` | Resolved toolchains |

### Provider Rules

- Aspects return a **list of providers** (not a single provider)
- Aspects **cannot return `DefaultInfo`**
- If rule and aspect return same provider type → error
- **Exceptions**:
  - `OutputGroupInfo`: merged if different groups
  - `InstrumentedFilesInfo`: taken from aspect

### Propagation Logic

When `attr_aspects = ["deps"]`:
1. Aspect A applied to target X
2. X has deps = [Y, Z]
3. Aspect propagates: A(Y), A(Z) computed first
4. In A(X) implementation, `ctx.rule.attr.deps` contains [A(Y), A(Z)] results

When `attr_aspects = ["*"]`:
- Propagates through ALL label/label_list attributes

### Required Providers Filtering

`required_providers = [[FooInfo], [BarInfo], [BazInfo, QuxInfo]]` means:
- Aspect only applies to targets that provide:
  - FooInfo, OR
  - BarInfo, OR
  - (BazInfo AND QuxInfo)

---

## Implementation Phases

### Phase 8a: Stub `aspect()` Function

**Goal**: Allow `aspect()` calls to parse without error. Return a placeholder that can be attached to attributes but doesn't execute.

#### Files to Create

**`app/kuro_interpreter_for_build/src/aspect.rs`**

```rust
// Core types
pub struct StarlarkAspectCallable<'v> {
    name: RefCell<Option<String>>,
    implementation: Value<'v>,
    attr_aspects: Vec<String>,
    attrs: Vec<(String, StarlarkAttribute)>,
    required_providers: Vec<Vec<Value<'v>>>,
    required_aspect_providers: Vec<Vec<Value<'v>>>,
    provides: Vec<Value<'v>>,
    requires: Vec<Value<'v>>,
    fragments: Vec<String>,
    toolchains: Vec<String>,
    doc: Option<String>,
    apply_to_generating_rules: bool,
}

pub struct FrozenStarlarkAspectCallable {
    // Frozen version
}

#[starlark_module]
pub fn register_aspect_function(builder: &mut GlobalsBuilder) {
    fn aspect<'v>(...) -> StarlarkAspectCallable<'v> { ... }
}
```

#### Files to Modify

**`app/kuro_interpreter_for_build/src/lib.rs`**
- Add `pub mod aspect;`

**`app/kuro_interpreter_for_build/src/interpreter/globals.rs`**
- Import and call `register_aspect_function(builder);`

**`app/kuro_interpreter_for_build/src/rule.rs`**
- Add `aspects` parameter to `rule()` function
- Store aspect list in rule callable

**`app/kuro_interpreter_for_build/src/attrs/starlark_attribute.rs`**
- Add `aspects` parameter to `attr.label()` and `attr.label_list()`

#### Success Criteria (Phase 8a)

- [x] `aspect()` function available in .bzl files
- [x] Aspect can be assigned to variable: `my_aspect = aspect(...)`
- [x] Aspect can be passed to `attr.label(aspects=[my_aspect])`
- [x] Aspect can be passed to `rule(attrs={"deps": attr.label_list(aspects=[...])})`
- [x] `@rules_cc//cc:defs.bzl` loads without "Variable `aspect` not found" error
      (Now blocked on `allow_empty` param for `attr.label_list` - separate issue)
- [x] Aspect implementation function is NOT called (stub only)

---

### Phase 8b: Aspect Context and Basic Execution

**Goal**: Implement aspect context object and basic execution (without full graph propagation).

#### Files to Create

**`app/kuro_build_api/src/interpreter/rule_defs/aspect_ctx.rs`**

Aspect context providing:
- `ctx.rule.kind` - string
- `ctx.rule.attr` - struct with rule's attributes
- `ctx.label` - target label
- `ctx.attr` - aspect-specific attributes
- `ctx.actions` - action registration

**`app/kuro_node/src/aspect.rs`**

```rust
pub struct AspectId {
    pub name: String,
    pub bzl_path: ImportPath,
}

pub struct Aspect {
    pub id: AspectId,
    pub attr_aspects: Vec<String>,
    pub required_providers: Vec<Vec<ProviderId>>,
    pub attrs: AttributeSpec,
}
```

#### Files to Modify

**`app/kuro_node/src/rule.rs`**
- Add `aspects: Vec<AspectId>` to attribute specs

**`app/kuro_build_api/src/analysis/calculation.rs`**
- Hook for aspect invocation during analysis

#### Success Criteria (Phase 8b)

**Completed (Types and Structures):**
- [x] `FrozenStarlarkAspectCallable` preserves all fields from unfrozen version
- [x] `AspectContext` type compiles with `ctx.attr`, `ctx.actions`, `ctx.label`, `ctx.rule`
- [x] `AspectRuleInfo` provides `ctx.rule.kind` and `ctx.rule.attr` members
- [x] `AspectTargetProviders` supports `target[SomeInfo]` and `SomeInfo in target`
- [x] `try_from_aspect_value()` rejects DefaultInfo
- [x] All crates build: `cargo build -p kuro_build_api -p kuro_interpreter_for_build`

**Deferred to Phase 8c (Requires DICE Integration):**
- [ ] Aspect implementation function can be called
- [ ] `ctx.rule.kind` returns correct rule kind
- [ ] `ctx.rule.attr` provides access to rule attributes
- [ ] `ctx.label` returns target label
- [ ] Aspect can return providers
- [ ] Simple aspect with no propagation works

---

### Phase 8c: Shadow Graph Propagation

**Goal**: Implement recursive aspect propagation through dependency graph.

#### Key Concepts

1. **Shadow Graph**: When aspect A is applied to target X:
   - First apply A to all dependencies (Y, Z) reachable via `attr_aspects`
   - In A(X), replace deps with aspect results [A(Y), A(Z)]

2. **DICE Integration**:
   - `AspectKey`: (target_label, aspect_id, configuration)
   - `AspectValue`: Computed aspect result (providers)
   - Incremental: Only recompute when dependencies change

3. **Propagation Filtering**:
   - `required_providers` filters which targets get aspect applied
   - Targets not matching get passed through unchanged

#### Files to Create

**`app/kuro_build_api/src/analysis/aspect_calculation.rs`**

```rust
pub struct AspectCalculation {
    // DICE computation for aspects
}

impl AspectCalculation {
    pub async fn compute_aspect(
        &self,
        target: ConfiguredTargetLabel,
        aspect: AspectId,
        ctx: &DiceComputations,
    ) -> Result<AspectResult> {
        // 1. Check required_providers filter
        // 2. Recursively compute aspect on dependencies
        // 3. Build aspect context with shadow graph
        // 4. Invoke aspect implementation
        // 5. Collect and return providers
    }
}
```

**`app/kuro_build_api/src/analysis/aspect_key.rs`**

```rust
#[derive(Clone, Debug, Hash, Eq, PartialEq, Allocative)]
pub struct AspectKey {
    pub target: ConfiguredTargetLabel,
    pub aspect: AspectId,
}

impl Key for AspectKey {
    type Value = AspectValue;
}
```

#### Files to Modify

**`app/kuro_build_api/src/analysis/mod.rs`**
- Add aspect modules

**`app/kuro_build_api/src/analysis/registry.rs`**
- Add aspect storage

#### Success Criteria (Phase 8c)

- [ ] Aspect propagates through `attr_aspects` attributes
- [ ] `ctx.rule.attr.deps` contains aspect results (shadow graph)
- [ ] `required_providers` filtering works
- [ ] Aspect results are cached via DICE
- [ ] Incremental recomputation works correctly
- [ ] `graph_structure_aspect` from rules_cc executes successfully

---

### Phase 8d: Advanced Features

**Goal**: Complete aspect feature set for full Bazel compatibility.

#### Features

1. **required_aspect_providers**: Access providers from other aspects
2. **requires**: Declare aspect dependencies (run other aspects first)
3. **toolchains**: Toolchain resolution for aspects
4. **exec_groups**: Execution groups for aspects
5. **apply_to_generating_rules**: Apply to generating rule of output files
6. **subrules**: Subrules used by aspects

#### Success Criteria (Phase 8d)

- [x] Shadow graph propagation via `compute_dep_aspects()` (**Completed 2026-01-31**)
- [x] `ctx.rule.attr.deps` contains aspect results
- [x] Depth-first propagation through dependency chains

#### Phase 8e: required_aspect_providers (Completed 2026-01-31)

- [x] `required_aspect_providers` filtering works (checks target's own providers)
- [x] Both `required_providers` AND `required_aspect_providers` must match

#### Phase 8f: rules_cc Loading (Completed 2026-01-31)

**Discovered Issues and Fixes:**

1. **`bazel_tools` bundled cell disabled** - Was removed from `get_bundled_data()` in
   `kuro_external_cells_bundled/src/lib.rs`. Fixed by re-adding `BAZEL_TOOLS` to the returned array.

2. **`bazel_tools` cell auto-registration disabled** - Was commented out in
   `kuro_common/src/legacy_configs/cells.rs:448-458`. Fixed by uncommenting.

3. **External cell symlinks not created** - The MVS resolver downloads sources to
   `~/.cache/kuro/registry/bcr.bazel.build/modules/{module}/{version}/source` but cells
   are registered with paths like `bazel-external/{module}/{version}`. These directories
   don't exist, causing file operation failures.

   **Workaround**: Manually create symlinks:
   ```bash
   ln -sf ~/.cache/kuro/.../source bazel-external/{module}/{version}
   ```

4. **Synthetic repos use Bazel-specific `package()` calls** - Synthetic repos generated in
   `kuro_bzlmod/src/synthetic_repos.rs` contained `package(default_visibility=["//visibility:public"])`
   which only works in Bazel BUILD files, not in Kuro's Buck2-based model where `package()` is
   only valid in PACKAGE files. Fixed by removing `package()` calls and adding explicit
   `visibility = ["//visibility:public"]` to each rule.

**Success Criteria (Phase 8f):**
- [x] `bazel_tools` bundled cell re-enabled
- [x] `bazel_tools` cell auto-registration re-enabled
- [x] Synthetic repos don't use Bazel-specific `package()` calls
- [x] `@rules_cc//cc:defs.bzl` loads successfully (with manual symlinks)
- [ ] Automatic symlink creation during MVS resolution (deferred to Phase 8g)

**Known Limitations:**
- `cc_library()` instantiation fails because rules_cc expects internal attributes like
  `_def_parser` that Kuro doesn't provide. The rule *definition* loads correctly, but
  *using* the rule requires additional work to provide expected implicit attributes.

#### Remaining (Phase 8g+)

- [ ] Automatic symlink/copy creation for external cells during MVS resolution
- [ ] Implicit rule attributes like `_def_parser` for rules_cc compatibility
- [ ] `requires` ensures aspect ordering
- [ ] Aspect toolchain resolution works
- [ ] `apply_to_generating_rules` works
- [ ] All rules_cc aspects function correctly
- [ ] Complex aspect chains (aspect-on-aspect) work

---

## Testing Strategy

### Unit Tests

**`tests/core/aspects/`**
- `test_aspect_definition.py` - aspect() function parsing
- `test_aspect_context.py` - ctx.rule.kind, ctx.rule.attr, etc.
- `test_aspect_propagation.py` - shadow graph, attr_aspects
- `test_aspect_providers.py` - provider return, merging rules
- `test_required_providers.py` - filtering logic

### Integration Tests

**`tests/e2e/aspects/`**
- `test_simple_aspect.py` - basic aspect execution
- `test_propagating_aspect.py` - recursive propagation
- `test_rules_cc_aspects.py` - graph_structure_aspect works

### Manual Test Addition

Add to `tests/manual_test/BUILD.bazel`:
```python
# Test: Simple aspect definition and application
load(":test_aspect.bzl", "my_aspect", "aspect_rule")

aspect_rule(
    name = "aspect_test",
    deps = [":dep1", ":dep2"],
)
```

---

## Bazel Source References

| Component | Bazel Source File |
|-----------|------------------|
| AspectClass interface | `src/main/java/.../packages/AspectClass.java` |
| Aspect instance | `src/main/java/.../packages/Aspect.java` |
| AspectDefinition | `src/main/java/.../packages/AspectDefinition.java` |
| AspectFunction (Skyframe) | `src/main/java/.../skyframe/AspectFunction.java` |
| AspectValue | `src/main/java/.../skyframe/AspectValue.java` |
| RuleContext (ctx) | `src/main/java/.../analysis/RuleContext.java` |
| StarlarkAspect | `src/main/java/.../starlark/StarlarkAspect.java` |

---

## Dependencies and Blockers

### This Plan Depends On

- [x] `subrule()` implementation (complete) - similar pattern to follow
- [x] Provider system working
- [x] DICE computation infrastructure
- [x] Rule context implementation

### This Plan Unblocks

- [x] `@rules_cc//cc:defs.bzl` loading (Phase 8f complete, requires symlinks)
- [ ] Full rules_cc functionality (needs automatic symlinks, Phase 8g)
- [ ] IDE integration aspects
- [ ] Coverage/linting aspects
- [ ] License compliance aspects

---

## Estimated Complexity

| Phase | Complexity | Files Changed | New Files |
|-------|------------|---------------|-----------|
| 8a | Medium | 4 | 1 |
| 8b | Medium-High | 5 | 2 |
| 8c | High | 6 | 3 |
| 8d | Medium | 4 | 0 |

**Note**: Phase 8a alone is sufficient to unblock rules_cc loading. Phases 8b-8d are needed for rules_cc to actually work at runtime.
