---
date: 2026-01-30T16:45:00-08:00
researcher: Claude
git_commit: 2ef2a593765ab0c603f7479c1b8426d62dd1ebd2
branch: main
repository: slug
topic: "BXL vs Bazel Aspects: Feature Comparison and Implementation Strategy"
tags: [research, bxl, aspects, bazel-compatibility, build-system, dependency-traversal]
status: complete
last_updated: 2026-01-30
last_updated_by: Claude
---

# Research: BXL vs Bazel Aspects - Feature Comparison and Implementation Strategy

**Date**: 2026-01-30T16:45:00-08:00
**Researcher**: Claude
**Git Commit**: 2ef2a593765ab0c603f7479c1b8426d62dd1ebd2
**Branch**: main
**Repository**: slug

## Research Question

Buck2 and Slug have support for BXL (Buck Extension Language). Bazel has aspects. Since Slug is working on adding support for Bazel compatibility, this research explores:

1. What are the overlaps between BXL and Bazel aspects?
2. Could aspect support be implemented in terms of BXL?
3. Could the BXL codebase be extended to support aspects?

## Summary

**BXL and Bazel aspects solve overlapping problems but use fundamentally different paradigms:**

| Dimension | BXL | Bazel Aspects |
|-----------|-----|---------------|
| **Paradigm** | Imperative scripting | Declarative graph augmentation |
| **Execution** | Separate command (`slug bxl`) | During analysis phase |
| **Graph Traversal** | Explicit via queries | Automatic via propagation rules |
| **Output** | Artifacts, stdout, JSON | Providers on shadow graph |
| **Integration** | External to build | Internal to build graph |
| **Incrementality** | DICE-cached operations | DICE-cached aspect computations |

**Key Finding**: While BXL can achieve similar end results as aspects (e.g., generating compilation databases), it cannot replace aspects because:

1. **Aspects are internal** - They participate in the analysis phase and return providers that rules can consume
2. **Aspects propagate automatically** - The shadow graph is computed implicitly based on `attr_aspects`
3. **Aspects integrate with toolchains** - They can resolve toolchains and execution platforms
4. **Rule authors depend on aspects** - External rules (rules_cc, rules_python) use aspects internally

**Recommendation**: Implement Bazel aspects natively (as planned in Phase 8), while preserving BXL as a complementary tool for user-facing automation and IDE integration.

---

## Detailed Findings

### 1. BXL Architecture and Capabilities

BXL (Buck Extension Language) is a Starlark-based scripting system that provides full access to Slug's build graph, query capabilities, and action execution.

#### Core Purpose

From the BXL RFC (`docs/rfcs/implemented/bxl.md:6-8`):
> "BXL allows integrators to interact with buck commands like build and query within Starlark, creating sequences of operations that introspect, build, and extend the build graph."

#### Key Capabilities

| Capability | API | Description |
|------------|-----|-------------|
| **Query Operations** | `ctx.uquery()`, `ctx.cquery()`, `ctx.aquery()` | Unconfigured, configured, and action queries |
| **Target Access** | `ctx.configured_targets()`, `ctx.unconfigured_targets()` | Direct access to target nodes |
| **Analysis** | `ctx.analysis()` | Run analysis and access providers |
| **Build** | `ctx.build()` | Trigger builds and materialize artifacts |
| **Actions** | `ctx.bxl_actions().actions` | Create actions (write, run, copy) |
| **Output** | `ctx.output.print()`, `ctx.output.ensure()` | Emit results and materialize artifacts |
| **Dependency Traversal** | `ctx.cquery().deps()`, `ctx.uquery().deps()` | Traverse dependency graphs |

#### Entry Point Structure

```python
def _main(ctx: bxl.Context):
    # Access CLI arguments
    target = ctx.cli_args.target

    # Query the build graph
    deps = ctx.cquery().deps(target, -1)  # Unbounded traversal

    # Analyze targets
    analysis = ctx.analysis(deps)

    # Collect information
    for label, result in analysis.items():
        info = result.providers().get(SomeProvider)
        # Process...

    # Write output
    ctx.output.print_json(collected_data)

main = bxl_main(
    impl = _main,
    cli_args = {"target": cli_args.target_label()},
)
```

Reference: `app/slug_bxl/src/bxl/starlark_defs/context/methods.rs:88-822`

#### DICE Integration

BXL executes as DICE nodes keyed by `BxlKey`:

```rust
struct BxlKeyData {
    spec: BxlFunctionLabel,              // Which BXL function
    bxl_args: Arc<OrderedMap<String, CliArgValue>>, // CLI arguments
    global_cfg_options: GlobalCfgOptions, // Configuration
}
```

Reference: `app/slug_bxl/src/bxl/key.rs:42-55`

#### Aspect-Like Patterns in BXL

BXL achieves aspect-like behavior through explicit traversal:

**Example: Rust Analyzer Dependency Collection** (`prelude/rust/rust-analyzer/resolve_deps.bxl:205-244`)

```python
def gather_deps(ctx: bxl.Context, target_analysis, workspaces):
    targets = set()
    for _target, analysis in target_analysis.items():
        info = analysis.providers().get(RustAnalyzerInfo)
        if info:
            for target_set in info.transitive_target_set:
                targets.add(target_set)

    outputs = ctx.target_universe(list(targets)).target_set()
    analysis = ctx.analysis(outputs)  # Bulk analysis

    # Process each target...
```

**Example: Compilation Database** (`prelude/cxx/tools/compilation_database.bxl:34-74`)

```python
def _impl(ctx: bxl.Context):
    targets = ctx.configured_targets(ctx.cli_args.targets)
    if ctx.cli_args.recursive:
        targets = ctx.cquery().deps(targets)  # Manual propagation

    db = []
    for name, target in ctx.analysis(targets).items():
        comp_db_info = target.providers().get(CxxCompilationDbInfo)
        if comp_db_info:
            for cc in comp_db_info.info.values():
                db.append(_make_entry(ctx, name, cc))

    # Write compile_commands.json
    actions.write_json(db_file, db)
```

---

### 2. Bazel Aspects Architecture

Aspects are a Bazel feature for augmenting the dependency graph with additional computation and providers.

#### Core Concept

From Bazel documentation:
> "Aspects allow augmenting build dependency graphs with additional information and actions. When applied to a target X, an aspect yields a 'shadow graph' of the original dependency graph."

#### Key Properties

| Property | Description |
|----------|-------------|
| **Shadow Graph** | Parallel graph where each node is aspect(target) |
| **Automatic Propagation** | Follows `attr_aspects` automatically |
| **Provider Return** | Returns list of providers (NOT DefaultInfo) |
| **Analysis Integration** | Runs during analysis phase |
| **Rule Integration** | Aspects attached to attributes via `aspects=[...]` |

#### Function Signature

```python
aspect(
    implementation,           # function(target, ctx) -> list[Provider]
    attr_aspects = [],        # Attributes to propagate through
    attrs = {},               # Aspect-specific attributes
    required_providers = [],  # Filter by providers
    required_aspect_providers = [],  # Access other aspects
    provides = [],            # Providers this aspect returns
    requires = [],            # Aspect dependencies
    fragments = [],           # Configuration fragments
    toolchains = [],          # Required toolchains
)
```

#### Implementation Function

```python
def _my_aspect_impl(target, ctx):
    # Access target's providers
    if CcInfo in target:
        cc_info = target[CcInfo]

    # Access rule's attributes (with aspect results!)
    rule_kind = ctx.rule.kind
    deps = ctx.rule.attr.deps  # Contains A(Y), A(Z), not Y, Z

    # Process aspect results from dependencies
    for dep in deps:
        if MyAspectInfo in dep:
            # Aggregate information
            pass

    # Create actions if needed
    ctx.actions.write(...)

    # Return providers
    return [MyAspectInfo(collected_data=...)]
```

#### Propagation Mechanism

When `attr_aspects = ["deps"]`:
1. Aspect A applied to target X
2. X has deps = [Y, Z]
3. System computes: A(Y), A(Z) first
4. In A(X) implementation, `ctx.rule.attr.deps` contains [A(Y), A(Z)]

This is **implicit** - the Bazel runtime handles traversal automatically.

#### Usage in rules_cc

**graph_structure_aspect** (`@rules_cc//cc:cc_shared_library.bzl:828`):

```python
graph_structure_aspect = aspect(
    attr_aspects = ["*"],
    required_providers = [[CcInfo], [CcSharedLibraryHintInfo], [ProtoInfo]],
    required_aspect_providers = [[CcInfo], [CcSharedLibraryHintInfo]],
    implementation = _graph_structure_aspect_impl,
)
```

This aspect traverses the entire dependency graph to collect linking information for shared libraries.

---

### 3. Feature Comparison

#### Traversal Mechanism

| Feature | BXL | Bazel Aspects |
|---------|-----|---------------|
| **Initiation** | Explicit: `ctx.cquery().deps(target)` | Implicit: `attr_aspects = ["deps"]` |
| **Depth Control** | Parameter: `deps(target, depth=-1)` | Implicit: follows all edges |
| **Filtering** | Post-hoc: `if provider in target` | Declarative: `required_providers` |
| **Direction** | Any: deps, rdeps, allpaths | Down only: follows attributes |
| **Explicit vs Implicit** | Explicit control flow | Implicit propagation rules |

#### Integration Points

| Feature | BXL | Bazel Aspects |
|---------|-----|---------------|
| **Execution Timing** | Separate command | During analysis |
| **Provider Access** | Read-only via `ctx.analysis()` | Returns providers to graph |
| **Rule Visibility** | None (external) | Via attribute `aspects=[...]` |
| **Action Registration** | Via `ctx.bxl_actions()` | Via `ctx.actions` |
| **Toolchain Access** | Via `exec_deps`, `toolchains` | Via `toolchains` parameter |

#### Output Mechanism

| Feature | BXL | Bazel Aspects |
|---------|-----|---------------|
| **Primary Output** | Artifacts, stdout, JSON | Providers on shadow graph |
| **Artifact Creation** | `actions.write()`, `actions.run()` | Same |
| **Materialization** | `ctx.output.ensure()` | Part of build |
| **Caching** | DICE (full script) | DICE (per-target aspect) |

#### Use Cases

| Use Case | BXL | Bazel Aspects |
|----------|-----|---------------|
| **Compilation Database** | Primary tool | Also works |
| **IDE Integration** | Primary tool | Primary tool |
| **License Compliance** | Works | Primary tool |
| **Custom Analysis** | Primary tool | Works |
| **Cross-cutting Providers** | Cannot | Primary use case |
| **Build Graph Augmentation** | Cannot | Primary use case |

---

### 4. Can Aspects Be Implemented Using BXL?

#### What BXL CAN Do

1. **Traverse dependencies**: `ctx.cquery().deps()` provides explicit traversal
2. **Access providers**: `ctx.analysis().providers()` reads target providers
3. **Create artifacts**: Same action APIs available
4. **Similar end results**: Generate compilation databases, collect metadata

#### What BXL CANNOT Do

1. **Return providers to rules**: BXL output is external to the build graph
2. **Automatic propagation**: Must explicitly query and iterate
3. **Integration with rule attributes**: Cannot attach to `attr.label(aspects=[...])`
4. **Run during analysis**: BXL runs as separate command
5. **Shadow graph semantics**: Dependencies don't automatically become aspect results

#### Fundamental Limitation

The core issue is **integration point**:

```
Bazel Aspects:
  rule analysis → aspect invoked → providers returned → visible to dependents

BXL:
  build completes → bxl invoked → artifacts/stdout → external
```

Aspects participate in the build graph; BXL observes it from outside.

#### Conclusion

**BXL cannot replace Bazel aspects** because:
- External rules (rules_cc, rules_python, etc.) use aspects internally
- Aspects provide providers that rules consume during analysis
- The declarative propagation model cannot be replicated with imperative queries

---

### 5. Can BXL Be Extended to Support Aspects?

#### Theoretical Possibility

BXL could theoretically be extended with:
1. **Provider return mechanism**: Allow BXL to contribute providers to targets
2. **Automatic propagation**: Add `bxl_aspect()` with `attr_aspects`
3. **Analysis integration**: Hook BXL into the analysis phase
4. **Shadow graph computation**: Compute aspect results before target analysis

#### Why This Isn't the Right Approach

1. **Would essentially implement aspects anyway**: The extension would need all aspect machinery
2. **Different execution model**: BXL runs as single script; aspects run per-target
3. **DICE integration differs**: Aspects need per-target caching; BXL caches whole script
4. **Semantic mismatch**: BXL is imperative; aspects are declarative

#### Better Approach

Keep BXL and aspects as complementary:

| System | Role |
|--------|------|
| **BXL** | User-facing automation, IDE integration, custom analysis workflows |
| **Aspects** | Rule-internal computation, provider propagation, cross-cutting build concerns |

---

### 6. Slug's Current Implementation Status

#### Aspects (Phase 8)

**Phase 8a: COMPLETE**
- `aspect()` built-in function parses
- Aspects can be attached to attributes
- Stub returns placeholder, doesn't execute
- Unblocked rules_cc loading

**Phases 8b-8d: NOT STARTED**
- 8b: AspectContext and basic execution
- 8c: Shadow graph propagation with DICE
- 8d: Advanced features (required_aspect_providers, toolchains)

Reference: `app/slug_interpreter_for_build/src/aspect.rs`

#### BXL

**Fully Functional**
- Complete implementation inherited from Buck2
- Query operations (uquery, cquery, aquery)
- Analysis and build integration
- Action creation and materialization
- DICE-based incrementality

Reference: `app/slug_bxl/src/`

---

## Architecture Insights

### Why Buck2 Didn't Have Aspects

Buck2 used alternative mechanisms:
1. **Anonymous targets**: Create derived targets during analysis
2. **Subtargets**: Access sub-providers via `target[name]`
3. **BXL**: External analysis for IDE integration

These provide similar capabilities but with different semantics.

### Bazel's AXL (Aspect Extension Language)

Bazel is developing AXL as their equivalent to BXL, suggesting convergence:
> "AXL is the Bazel equivalent of Buck2's BXL"

This indicates both systems recognize the need for:
- **Declarative aspects**: For rule-internal computation
- **Imperative scripting**: For user-facing automation

### Recommended Architecture for Slug

```
┌─────────────────────────────────────────────────────┐
│                   Slug Build System                  │
├─────────────────────────────────────────────────────┤
│                                                     │
│   ┌─────────────┐         ┌─────────────────────┐  │
│   │   Aspects   │         │        BXL          │  │
│   │  (Phase 8)  │         │  (Inherited)        │  │
│   ├─────────────┤         ├─────────────────────┤  │
│   │ Declarative │         │ Imperative          │  │
│   │ Rule-internal│        │ User-facing         │  │
│   │ Provider flow│        │ External analysis   │  │
│   │ Shadow graph │        │ Query & build       │  │
│   └──────┬──────┘         └─────────┬───────────┘  │
│          │                          │               │
│          └──────────┬───────────────┘               │
│                     │                               │
│              ┌──────▼──────┐                        │
│              │    DICE     │                        │
│              │ Incremental │                        │
│              │ Computation │                        │
│              └─────────────┘                        │
└─────────────────────────────────────────────────────┘
```

---

## Code References

### BXL Implementation
- `app/slug_bxl/src/bxl/starlark_defs/context/methods.rs:88-822` - BxlContext API
- `app/slug_bxl/src/bxl/key.rs:42-55` - BxlKey for DICE caching
- `app/slug_bxl/src/command.rs:77-83` - Command entry point
- `prelude/rust/rust-analyzer/resolve_deps.bxl:205-244` - Aspect-like pattern

### Aspects Implementation
- `app/slug_interpreter_for_build/src/aspect.rs` - Phase 8a stub
- `app/slug_interpreter_for_build/src/attrs/attrs_global.rs:763-856` - Attribute integration
- `thoughts/shared/plans/slug-bazel-subplans/06-aspects.md` - Implementation plan

### Usage Examples
- `bazel_tools/tools/compliance/gather_packages.bzl:38-219` - Aspect example
- `prelude/cxx/tools/compilation_database.bxl:34-74` - BXL compilation database
- `tests/manual_test/test_aspect.bzl` - Aspect tests

---

## Recommendations

### 1. Implement Aspects Natively (Continue Phase 8)

Aspects must be implemented as a first-class feature because:
- External rules (rules_cc, rules_python) depend on aspects
- Provider propagation requires analysis-phase integration
- Shadow graph semantics are fundamentally different from BXL queries

### 2. Preserve BXL for Complementary Use Cases

BXL remains valuable for:
- User-facing automation scripts
- IDE integration (compile_commands.json)
- Custom analysis workflows
- One-off introspection tasks

### 3. Document the Distinction

Clarify when to use each:

| Use Case | Recommended Tool |
|----------|------------------|
| Rule author needs provider propagation | Aspects |
| User wants compilation database | BXL |
| Cross-cutting build concern | Aspects |
| Custom analysis script | BXL |
| Internal rule implementation detail | Aspects |
| External tooling integration | BXL |

### 4. Consider Future Convergence

Monitor Bazel's AXL development for potential patterns to adopt in BXL, and consider whether BXL could gain aspect-like conveniences for common patterns (while keeping aspects for rule-internal use).

---

## Open Questions

1. **Performance comparison**: How does BXL's query-based traversal compare to aspect propagation at scale?
2. **AXL evolution**: What features will Bazel's AXL provide that BXL doesn't have?
3. **Hybrid patterns**: Could BXL invoke aspects, or aspects invoke BXL-like queries?
4. **Migration path**: For users coming from Buck2, how do we explain when to use BXL vs aspects?

---

## Related Research

- `thoughts/shared/plans/slug-bazel-subplans/06-aspects.md` - Aspects implementation plan
- `thoughts/shared/plans/2026-01-21-slug-bazel-compatible-build-tool.md` - Master plan
- `thoughts/shared/research/bxl-vs-axl-comparison.md` - BXL vs AXL (incomplete)
- `docs/rfcs/implemented/bxl.md` - BXL RFC
