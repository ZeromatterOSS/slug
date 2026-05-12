# AXL vs BXL Compatibility Research

## Executive Summary

AXL (Aspect Extension Language) is explicitly designed as "the Bazel equivalent of Buck2's BXL." Both languages share the same fundamental goals and programming model, making them highly conceptually compatible. However, there are significant implementation differences due to the underlying build systems (Bazel vs Buck2/Slug).

**Key Finding**: AXL and BXL are architecturally similar enough that supporting AXL in Slug would be feasible, but would require adaptation layers for Bazel-specific concepts.

## Overview Comparison

| Aspect | BXL (Buck2/Slug) | AXL (Aspect/Bazel) |
|--------|------------------|-------------------|
| Language | Starlark | Starlark |
| File Extension | `.bxl` | `.axl` |
| Entry Point | `bxl_main()` / `bxl.main()` | Task function with `ctx` |
| Context Object | `ctx` | `ctx` |
| Query Support | uquery, cquery, aquery | query, cquery, aquery (via Bazel) |
| Build Execution | `ctx.build()` | `ctx.bazel.build` |
| Action Creation | `ctx.bxl_actions()` | Custom actions via WASM |
| Output | `ctx.output.print()` | Streaming output |
| CLI Args | `cli_args.*` module | Task argument definitions |

## Detailed API Comparison

### Context Object Structure

**BXL (Slug)**
```starlark
def _impl(ctx):
    # Query operations
    ctx.uquery()              # Unconfigured query
    ctx.cquery()              # Configured query
    ctx.aquery()              # Action query

    # Target operations
    ctx.configured_targets()   # Resolve targets
    ctx.unconfigured_targets() # Unconfigured resolution
    ctx.target_universe()      # Create query universe

    # Build operations
    ctx.build()               # Build and materialize
    ctx.analysis()            # Run analysis, get providers

    # Action creation
    ctx.bxl_actions()         # Get action factory

    # Filesystem
    ctx.fs.exists()
    ctx.fs.list()
    ctx.fs.is_dir()

    # Output
    ctx.output.print()
    ctx.output.ensure()       # Materialize artifact

    # Lazy operations
    ctx.lazy.analysis()
    ctx.lazy.join()
```

**AXL (Aspect)**
```starlark
def _task(ctx):
    # Build operations
    ctx.bazel.build           # Execute builds
    ctx.bazel.query           # Query operations (presumed)

    # BEP Integration
    # Can iterate over Build Event Protocol events

    # Note: Full API not publicly documented yet
```

### Entry Point Definition

**BXL**
```starlark
my_script = bxl_main(
    impl = _impl,
    cli_args = {
        "target": cli_args.target_label(),
        "verbose": cli_args.bool(default=False),
    },
    doc = "My BXL script"
)
```

**AXL**
```starlark
# Dependency declaration in config.axl
axl_archive_dep(
    name = "my_extension",
    urls = ["..."],
    integrity = "sha256-...",
    auto_use_tasks = True,
)

# Task definition (exact syntax TBD - not fully documented)
```

### Query Capabilities

**BXL Query Operations** (fully documented)
- `ctx.uquery()` - Unconfigured graph queries
  - `allpaths(from, to)`, `somepath(from, to)`
  - `deps(universe, depth)`, `rdeps(universe, targets, depth)`
  - `attrfilter(attr, value, targets)`
  - `kind(pattern, targets)`, `owner(files)`, `testsof(targets)`

- `ctx.cquery()` - Configured graph queries
  - Same operations as uquery but on configured targets

- `ctx.aquery()` - Action graph queries
  - `deps(universe, depth)`

**AXL Query Operations**
- Access to Bazel's native query/cquery/aquery
- Exact API surface not publicly documented
- Presumably wraps Bazel's query commands

### Build Event Protocol Integration

**BXL**
- No direct BEP integration in the API
- Events are emitted but not directly accessible to scripts
- `ctx.instant_event(id, metadata)` for custom events

**AXL**
- First-class BEP integration
- Can iterate over build events in real-time
- Callback-based event subscription
- This is a key differentiator

## Architectural Differences

### 1. Build System Integration

**BXL in Slug**
- Deep DICE integration for caching
- Native Rust implementation
- Action graph is directly accessible
- Providers are first-class citizens
- Execution context is hermetic

**AXL in Aspect CLI**
- Go-based CLI wrapping Bazel
- BEP-based integration with Bazel
- Relies on Bazel's caching
- Plugin system for extensibility
- Less hermetic (can have side effects in Go plugins)

### 2. Execution Model

**BXL**
- Scripts execute within Slug's Starlark evaluator
- Full access to build graph internals
- Can create actions that run during the build
- Three context types: Root, Dynamic, AnonTarget
- Lazy operations for batching

**AXL**
- Scripts execute in Aspect CLI's Starlark interpreter
- Interacts with Bazel via CLI/BEP
- Tasks run as orchestration layer
- WASM support for platform-agnostic tools

### 3. Filesystem Access

**BXL**
```starlark
ctx.fs.exists("cell//path")
ctx.fs.list("cell//path")
ctx.fs.is_dir("cell//path")
```

**AXL**
- Filesystem access not documented
- Likely relies on Bazel's sandboxing model

### 4. Action Creation

**BXL**
```starlark
actions = ctx.bxl_actions().actions
output = actions.write("file", "content")
actions.run(["cmd"], outputs=[output])
ctx.output.ensure(output)
```

**AXL**
- WASM-based action execution (for buildozer, etc.)
- Custom actions via Go plugins (legacy)
- Starlark-based actions (emerging)

## Compatibility Assessment

### High Compatibility Areas

1. **Core Programming Model**
   - Both use Starlark with context object
   - Similar function-based entry points
   - Both can query build graphs

2. **Query Operations**
   - Both support query/cquery/aquery concepts
   - Query language is similar (derived from Bazel)

3. **Build Execution**
   - Both can trigger builds
   - Both return build results

### Medium Compatibility Areas

1. **CLI Arguments**
   - BXL has rich `cli_args` module
   - AXL uses task-level argument definitions
   - Translation possible but not 1:1

2. **Output Handling**
   - BXL has `ctx.output.print()` and streaming
   - AXL output model not fully documented

### Low Compatibility Areas

1. **Build Event Protocol**
   - AXL has first-class BEP integration
   - BXL does not expose BEP directly
   - Would require significant work to add

2. **Provider Access**
   - BXL has deep provider integration
   - AXL relies on Bazel's provider model
   - Different provider systems

3. **Cell/Repository Model**
   - BXL uses Buck2's cell model
   - AXL uses Bazel's repository/workspace model
   - Namespace differences

4. **Action Model**
   - BXL creates actions in the build graph
   - AXL orchestrates external tools
   - Fundamentally different approaches

## Recommendations for Slug

### Option 1: AXL Compatibility Layer

Create an AXL compatibility shim that translates AXL scripts to BXL:

**Pros:**
- Allows Bazel users to migrate scripts
- Minimal changes to BXL core

**Cons:**
- Imperfect compatibility
- Maintenance burden
- BEP integration would be difficult

### Option 2: Native AXL Support

Implement AXL as a separate extension language:

**Pros:**
- Full AXL compatibility
- Clear separation from BXL

**Cons:**
- Duplicate functionality
- More code to maintain
- Confusing for users

### Option 3: BXL Extensions for AXL Concepts

Add AXL-inspired features to BXL:

**Features to Consider:**
1. **BEP Access** - Add `ctx.bep_events()` or similar
2. **Task Runner Mode** - Allow BXL scripts to run as tasks
3. **WASM Actions** - Support WASM-based tool execution

**Pros:**
- Single extension language
- Best features from both systems
- Consistent user experience

**Cons:**
- BXL API surface grows
- May not satisfy AXL users completely

### Recommended Approach: Option 3

Given that:
1. AXL is very new (announced Nov 2025)
2. AXL documentation is incomplete
3. BXL already has rich functionality
4. Slug aims to be Bazel-compatible

The most pragmatic approach is:
1. Monitor AXL's development and API stabilization
2. Add BEP access to BXL (the main missing feature)
3. Consider task-runner mode for BXL scripts
4. Document migration paths for common AXL patterns

## Open Questions

1. **AXL API Stability** - AXL is brand new; API may change significantly
2. **Bazel-specific Features** - Some AXL features may depend on Bazel internals
3. **Go Plugin System** - AXL's Go plugins cannot be supported in Slug
4. **User Demand** - Is there actual demand for AXL compatibility?

## Sources

- [Aspect Build AXL Landing Page](https://www.aspect.build/axl)
- [BazelCon 2025 Blog Post](https://blog.aspect.build/bazelcon-2025)
- [Aspect CLI GitHub](https://github.com/aspect-build/aspect-cli)
- [Aspect Extensions GitHub](https://github.com/aspect-extensions)
- Slug BXL Implementation: `app/slug_bxl/`
- Buck2 BXL Documentation: https://buck2.build/docs/bxl/

## Appendix: BXL API Reference Summary

### Context Methods

| Method | Description |
|--------|-------------|
| `ctx.configured_targets()` | Resolve targets to configured nodes |
| `ctx.unconfigured_targets()` | Resolve to unconfigured nodes |
| `ctx.target_universe()` | Create query universe |
| `ctx.uquery()` | Unconfigured query context |
| `ctx.cquery()` | Configured query context |
| `ctx.aquery()` | Action query context |
| `ctx.analysis()` | Run analysis, get providers |
| `ctx.build()` | Build and materialize |
| `ctx.bxl_actions()` | Get action factory |
| `ctx.output` | Output stream |
| `ctx.fs` | Filesystem operations |
| `ctx.lazy` | Lazy/batch operations |
| `ctx.audit()` | Audit context |
| `ctx.root()` | Repository root path |
| `ctx.cli_args` | Parsed CLI arguments |

### CLI Argument Types

| Type | Description |
|------|-------------|
| `cli_args.string()` | String argument |
| `cli_args.int()` | Integer argument |
| `cli_args.bool()` | Boolean argument |
| `cli_args.list()` | List argument |
| `cli_args.target_label()` | Target label |
| `cli_args.target_expr()` | Target pattern |
| `cli_args.json()` | JSON object |
