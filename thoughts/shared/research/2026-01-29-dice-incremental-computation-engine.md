---
date: 2026-01-29T12:00:00-05:00
researcher: Claude
git_commit: de9665bc5ecc3112da78c9b975bd97a9ad1f8267
branch: main
repository: slug
topic: "DICE Incremental Computation Engine - Algorithm, Design, and Comparison to Bazel Skyframe"
tags: [research, codebase, dice, incremental-computation, skyframe, bazel, build-systems]
status: complete
last_updated: 2026-01-29
last_updated_by: Claude
last_updated_note: "Added follow-up research comparing DICE to Bazel 9.0.0 with SkyMeld"
---

# Research: DICE Incremental Computation Engine

**Date**: 2026-01-29T12:00:00-05:00
**Researcher**: Claude
**Git Commit**: de9665bc5ecc3112da78c9b975bd97a9ad1f8267
**Branch**: main
**Repository**: slug

## Research Question

Conduct an in-depth exploration of the DICE portion of the codebase. Explain the algorithm, design rationale, and comparison to equivalent engines in Bazel.

## Summary

DICE (Dynamic Incremental Computation Engine) is Slug's core incremental caching computation engine, providing the foundation for efficient incremental builds. It implements a sophisticated multi-version dependency tracking system that achieves **O(changed subset) recomputations** with only **O(invalidated subset) graph traversals**. DICE draws inspiration from systems like Adapton and Salsa but extends them with multi-version concurrency control (MVCC) for safe concurrent reads and writes.

Key differentiators from Bazel's Skyframe:
- **Rust async/await model** vs Java's restart-based concurrency
- **Multi-versioning** allows concurrent transactions at different versions
- **Series-parallel dependency graphs** enable optimized parallel validation
- **Value equality-based early cutoff** propagates through the graph

---

## Detailed Findings

### 1. Directory Structure and Organization

The DICE implementation spans ~7,000 lines of Rust code organized into multiple specialized crates:

```
dice/
├── dice/                    # Main DICE engine (7081 LOC)
│   ├── src/api/            # Public API (Key, DiceComputations, Transaction)
│   ├── src/impls/          # Core implementation
│   │   ├── core/           # Graph, versions, state management
│   │   ├── deps/           # Dependency tracking
│   │   ├── task/           # Async task execution
│   │   └── cache.rs        # Lock-free caching
│   ├── src/introspection/  # Graph serialization/debugging
│   └── docs/               # Comprehensive documentation
├── dice_error/             # Error handling types
├── dice_futures/           # Async utilities with cancellation support
├── dice_examples/          # Example computations
├── dice_tests/             # End-to-end tests
├── fuzzy_dice/             # Property-based testing
└── read_dump/              # Graph serialization reader
```

**Key Files**:
- `dice/dice/src/api/key.rs` - Core `Key` trait definition
- `dice/dice/src/impls/core/graph/storage.rs` - VersionedGraph with extensive algorithm documentation
- `dice/dice/src/impls/worker.rs` - Task execution flow
- `dice/dice/src/impls/cache.rs` - Lock-free concurrent caching

---

### 2. Core Algorithm

#### 2.1 Computation Model

DICE implements a **pull-based incremental computation** model where:

1. **Keys** define computations (what to compute)
2. **Values** are the results of computations
3. **Dependencies** are automatically tracked during computation
4. **Invalidation** propagates through reverse dependencies (rdeps)

```rust
#[async_trait]
pub trait Key: Allocative + Debug + Display + Clone + Eq + Hash + Send + Sync + 'static {
    type Value: Allocative + Dupe + Send + Sync + 'static;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        cancellations: &CancellationContext,
    ) -> Self::Value;

    // Critical: determines if cached value can be reused
    fn equality(x: &Self::Value, y: &Self::Value) -> bool;

    // Returns false for transient values that shouldn't be cached
    fn validity(_x: &Self::Value) -> bool { true }
}
```

#### 2.2 Version Tracking

DICE maintains a linear version history:

- **VersionNumber**: Simple incrementing counter (u64)
- **VersionRange**: Represents `[begin, end)` intervals of validity
- **VersionRanges**: Set of disjoint intervals tracking when values are valid
- **VersionTracker**: Global state managing active versions with reference counting

Each transaction commit increments the version. Computations see a consistent snapshot at their version.

#### 2.3 Dependency Tracking

During computation, DICE automatically records dependencies:

```rust
pub(crate) struct RecordingDepsTracker {
    deps: SeriesParallelDeps,           // Compact dependency graph encoding
    deps_validity: DiceValidity,        // Valid or Transient
    invalidation_paths: TrackedInvalidationPaths,
}
```

**SeriesParallelDeps**: A novel encoding that distinguishes:
- **Serial dependencies**: Must be checked sequentially
- **Parallel dependencies**: Can be validated concurrently

This enables the "checkDeps" algorithm to parallelize validation while respecting actual dependency structure.

#### 2.4 Recomputation Decision Algorithm

When a value is requested at version V:

```
1. CHECK CACHE
   - Look up (key, version) in SharedCache
   - If found and valid: return cached value

2. CHECK VERSIONED GRAPH
   - Get node's cell history
   - Find latest version VP where value was computed

3. DEPS CHECK (can value from VP be reused at V?)
   For each dependency D at version VP:
     - Get D's validity range
     - Check if range includes both VP and V
   If ALL deps pass: REUSE value (no recomputation needed)
   Otherwise: RECOMPUTE

4. RECOMPUTE
   - Execute Key::compute()
   - Record new dependencies
   - Store result with new cell history

5. EARLY CUTOFF
   If new_value == old_value && new_deps == old_deps:
     - Extend old cell history (value unchanged)
     - Skip invalidating dependents
   Otherwise:
     - Create new cell history [V, V+1)
```

#### 2.5 Invalidation Propagation

DICE uses **reverse dependency (rdeps) tracking** for efficient invalidation:

```
invalidate(node, version):
    if already_dirtied_at(node, version):
        return []

    mark_dirty(node, version)
    rdeps = drain_rdeps(node)  // Take ownership of rdeps

    for each rdep in rdeps:
        if not already_invalidated(rdep, version):
            mark_invalidated(rdep, version)
            queue.extend(rdep.rdeps)

    return queue  // Continue breadth-first propagation
```

Key optimizations:
- Rdeps stored only until invalidation (then drained)
- "Force-dirty" markers prevent reuse across invalidation boundaries
- Deferred dirty propagation for uncommitted versions

---

### 3. Design Rationale

#### 3.1 Why Multi-Versioning?

**Problem**: Single-version systems force all concurrent requests to either:
- Block on updates (poor concurrency)
- See inconsistent state (correctness issues)

**DICE Solution**: Each transaction gets an immutable version snapshot:
- Requests run in parallel without conflicts
- Natural isolation between transactions
- No global synchronization for reads

#### 3.2 Why Series-Parallel Dependency Graphs?

Earlier approaches had issues:
- **v1 (Eager parallel check)**: Panics on missing deps that wouldn't be accessed
- **v2 (Sequential check)**: Too slow, single-threaded

**DICE Solution**: Track dependency structure (serial vs parallel):
- When A calls `ctx.compute(B)` then `ctx.compute(C)`: serial deps
- When A calls `ctx.join_all([B, C])`: parallel deps
- Validation can safely parallelize the parallel portions

#### 3.3 Why Value Equality Checking?

**Observation**: Many changes have transitive effects that cancel out.

Example from DICE benchmarks (word count on 1M files):
| Scenario | Without Early Cutoff | With Early Cutoff |
|----------|---------------------|-------------------|
| Fix typo | 1101s | **100ms** |

If a recomputed value equals the previous value, dependents can reuse their cached results.

#### 3.4 Why Async/Await?

**Benefits**:
- Natural parallel composition with Tokio
- Automatic work deduplication via SharedCache
- Clean cancellation support
- No thread-per-computation overhead

---

### 4. Key Data Structures

| Structure | Purpose | Location |
|-----------|---------|----------|
| `Key` trait | Define computations | `api/key.rs` |
| `DiceComputations` | Context for running computations | `api/computations.rs` |
| `DiceTransaction` | Immutable version snapshot | `api/transaction.rs` |
| `VersionedGraph` | Multi-version cache storage | `impls/core/graph/storage.rs` |
| `SharedCache` | Lock-free concurrent cache | `impls/cache.rs` |
| `SeriesParallelDeps` | Dependency structure encoding | `impls/deps/graph.rs` |
| `VersionTracker` | Global version state | `impls/core/versions.rs` |
| `RecordingDepsTracker` | Records deps during compute | `impls/deps.rs` |

---

### 5. DICE Usage in Slug

DICE powers all incremental computation in Slug's build system:

#### Build Graph Layers

```
InterpreterResultsKey(package)    // Parses BUILD files
    ↓
TargetNode (unconfigured)         // Target definitions
    ↓
ConfiguredTargetLabel             // Target + configuration
    ↓
AnalysisKey                       // Rule implementation → providers + actions
    ↓
ActionKey                         // Action execution → outputs
```

#### Key Implementations

**InterpreterResultsKey** (`app/slug_interpreter_for_build/src/interpreter/calculation.rs`):
```rust
#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
pub struct InterpreterResultsKey(pub PackageLabel);

impl Key for InterpreterResultsKey {
    type Value = slug_error::Result<Arc<EvaluationResult>>;
    // Evaluates BUILD files → TargetNodes
}
```

**AnalysisKey** (`app/slug_analysis/src/analysis/calculation.rs`):
```rust
#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
pub struct AnalysisKey(pub ConfiguredTargetLabel);

impl Key for AnalysisKey {
    type Value = slug_error::Result<MaybeCompatible<AnalysisResult>>;
    // Runs rule implementation → providers + actions
}
```

#### Integration Patterns

1. **Extension Traits**: Add methods to `DiceComputations`
2. **Late-Binding**: Pluggable implementations via `LateBinding`
3. **InjectedKey**: External values (configs, inputs)
4. **ProjectionKey**: Efficient partial value access

---

### 6. Comparison to Bazel Skyframe

| Aspect | DICE (Slug) | Skyframe (Bazel) |
|--------|-------------|------------------|
| **Language** | Rust | Java |
| **Concurrency Model** | Async/await with Tokio | Restart-based (null return → reinvoke) |
| **Versioning** | Multi-version MVCC | Single version per build |
| **Dependency Encoding** | Series-parallel graphs | Flat dependency sets |
| **Caching** | Lock-free concurrent cache | Thread-safe map |
| **Invalidation** | Bottom-up with rdeps | Bottom-up with rdeps |
| **Early Cutoff** | Value equality checking | "Change pruning" |
| **Parallel Validation** | Structure-aware parallelism | Independent node parallelism |

#### Skyframe Architecture

Skyframe is Bazel's incremental evaluation framework with similar concepts:

- **SkyKey**: Immutable identifier for a computation (like DICE's Key)
- **SkyValue**: Result of a computation (like DICE's Value)
- **SkyFunction**: Computes SkyValue from SkyKey + dependencies

**Key Differences**:

1. **Concurrency Handling**:
   - Skyframe: When a dependency isn't ready, function returns `null` and is restarted later
   - DICE: Uses async/await; computation suspends and resumes automatically

2. **Version Management**:
   - Skyframe: Single version; all queries see latest state
   - DICE: Multi-version; transactions can query at different versions concurrently

3. **Dependency Structure**:
   - Skyframe: Flat set of dependencies
   - DICE: Series-parallel graph preserving execution structure

4. **Parallelism Strategy**:
   - Skyframe: Functions that don't depend on each other run in parallel
   - DICE: Additionally parallelizes dependency validation using series-parallel structure

#### Shared Design Principles

Both systems share core principles:
- **Hermeticity**: Functions only access data through declared dependencies
- **Determinism**: Same inputs → same outputs
- **Bottom-up invalidation**: Changes propagate through reverse dependencies
- **Change pruning/Early cutoff**: Unchanged values don't invalidate dependents

---

### 7. Performance Characteristics

From DICE documentation benchmarks (word count on 1M files):

| Scenario | Naive | Basic Cache | Eager Cutoff | SP Graph | Full DICE |
|----------|-------|-------------|--------------|----------|-----------|
| Cold build | 11100s | 12000s | 11100s | 11100s | 11100s |
| Add file | 11100s | 2000s | 1101s | 150ms | 150ms |
| Fix typo | 11100s | 2000s | 1101s | 150ms | **100ms** |

**Why the differences?**
- Naive: No caching at all
- Basic Cache: Caches but recomputes on any change
- Eager Cutoff: Stops propagation when values unchanged
- SP Graph: Parallel dependency checking
- Full DICE: Value equality enables maximum early cutoff

---

## Code References

### Core Implementation
- `dice/dice/src/api/key.rs:30-80` - Key trait definition
- `dice/dice/src/impls/core/graph/storage.rs:1-168` - Algorithm documentation
- `dice/dice/src/impls/worker.rs:1-200` - Task execution flow
- `dice/dice/src/impls/cache.rs:1-150` - Lock-free SharedCache
- `dice/dice/src/impls/deps.rs:27-192` - Dependency tracking
- `dice/dice/src/impls/value.rs:130-281` - Value storage and validity

### API Surface
- `dice/dice/src/api/computations.rs` - DiceComputations context
- `dice/dice/src/api/transaction.rs` - Transaction semantics
- `dice/dice/src/api/injected.rs` - InjectedKey for external values

### Slug Integration
- `app/slug_interpreter_for_build/src/interpreter/calculation.rs` - BUILD file parsing
- `app/slug_analysis/src/analysis/calculation.rs` - Target analysis
- `app/slug_build_api/src/actions/calculation.rs` - Action execution
- `app/slug_build_api/src/configure_dice.rs` - DICE initialization

### Documentation
- `dice/dice/docs/incrementality.md` - Incrementality overview
- `dice/dice/docs/api.md` - API usage guide
- `dice/dice/docs/parallelism.md` - Parallel computation behavior
- `dice/dice/docs/cancellations.md` - Cancellation mechanisms

---

## Architecture Insights

### Key Design Patterns

1. **Type Erasure with DiceKey**: Keys are assigned integer IDs for efficient graph operations while preserving type safety at API boundaries

2. **Arena-based Dependency Recording**: Parallel computations use arena allocation for dependency trackers, merged at completion

3. **Two-tier Caching**: Lock-free table for completed computations, DashMap for in-flight work

4. **Epoch-based Staleness Detection**: Version epochs prevent stale computations from updating state after version reuse

5. **Deferred Dirty Propagation**: Some invalidation deferred to recomputation time to reduce traversals

### Trade-offs Made

**Chosen**:
- Lazy invalidation (traverse during recomputation when you know what's needed)
- Multi-versioning (accept memory cost for concurrency)
- Async/await (leverage Tokio for parallelism)
- Type-erased values (generic computation engine)

**Avoided**:
- Graph coarsening (trust incremental checking)
- Mandatory single-version (no global sync for reads)
- Callback-based dependencies (type-safe async instead)

---

## Related Research

- Adapton: Original demand-driven incremental computation paper
- Salsa: Rust incremental computation (used by rust-analyzer)
- Skip: Facebook's incremental computation with MVCC
- Skyframe: Bazel's incremental evaluation framework

---

## Open Questions

1. **Memory Pressure**: How does DICE handle memory pressure with multi-version storage? Is there automatic version eviction?

2. **Distributed Execution**: How does DICE interact with remote execution? Are there plans for distributed DICE state?

3. **Debugging Tools**: Beyond `bazel dump --skyframe`, what tools exist for debugging DICE graph issues?

4. **Performance Tuning**: Are there configuration options for trading memory vs recomputation?

---

## Follow-up Research: DICE vs Bazel 9.0.0 with SkyMeld

**Date**: 2026-01-29
**Question**: What performance improvement might be expected if Bazel moved to a DICE computation engine?

### Executive Summary

**No published benchmarks exist** comparing DICE directly to Skyframe. Any performance estimates would be speculative. Meta's Buck2 (which uses DICE) claims **2x speedup over Buck1**, but this comparison is against their legacy system, not Bazel.

### Bazel 9.0.0 and SkyMeld Status

As of Bazel 9.0.0 (released January 2026):
- **SkyMeld is enabled by default** (since Bazel 7.0)
- SkyMeld merges analysis and execution phases, similar to DICE's unified graph
- WORKSPACE is completely removed; Bzlmod is mandatory

SkyMeld was Bazel's response to the same problem DICE solved architecturally: **removing the blocking boundary between analysis and execution phases**.

### Remaining Architectural Differences (Post-SkyMeld)

Even with SkyMeld, key differences remain:

| Aspect | DICE | Skyframe + SkyMeld |
|--------|------|-------------------|
| **Concurrency Model** | Rust async/await (suspend/resume) | Java restart model (null → reinvoke) |
| **Thread Efficiency** | Task suspends without thread | Function must restart from beginning |
| **Dependency Validation** | Series-parallel graph parallelization | Flat dependency groups |
| **Version Model** | Multi-version MVCC | Single version per build |
| **Language Runtime** | Rust (no GC pauses) | Java (GC overhead) |

### Theoretical Performance Impacts

#### 1. Restart Overhead (Skyframe-specific)

Skyframe's restart model has measurable overhead:

> "When a requested SkyValue is not yet ready... the requesting SkyFunction observes a null getValue response and should return null... Skyframe restarts the SkyFunctions when all previously requested SkyValues become available."
> — [Skyframe Documentation](https://bazel.build/reference/skyframe)

**Impact**: Functions with many sequential dependencies restart multiple times, re-executing earlier code. DICE's async/await suspends mid-computation without re-execution.

**Bazel attempted virtual threads** to address this but found them "orders of magnitude slower, leading to almost a 3x increase in end-to-end analysis latency" ([StateMachine Guide](https://bazel.build/contribute/statemachine-guide)).

#### 2. Dependency Validation Parallelism

DICE's series-parallel dependency encoding enables parallel validation of dependency groups while respecting execution order. Skyframe validates flat dependency sets but cannot parallelize within ordered dependency chains.

**Potential impact**: For builds with deep dependency chains, DICE can validate more dependencies in parallel during incremental builds.

#### 3. Known SkyMeld Issues

SkyMeld has known regressions in certain scenarios:

> "When upgrading from Bazel 6.5.0 to 7.0.2... users experienced 'extreme slowdown' during test execution... Adding `--experimental_merged_skyframe_analysis_execution=false` resolved the performance issue."
> — [GitHub Issue #22233](https://github.com/bazelbuild/bazel/issues/22233)

There's also a documented tradeoff:
> "Having both skymeld + bwob improves clean builds' performance, but comes with a performance penalty for incremental builds."
> — [Project Skymeld Issue](https://github.com/bazelbuild/bazel/issues/14057)

### Why No Direct Comparison Exists

1. **Different ecosystems**: Buck2/DICE primarily used at Meta; Bazel across diverse organizations
2. **Different baselines**: Meta compares Buck2 to Buck1 (2x faster); no equivalent Bazel baseline
3. **Workload variance**: Build system performance is highly workload-dependent
4. **No incentive**: Neither Google nor Meta benefits from publishing comparative benchmarks

### Honest Assessment

**Would Bazel benefit from DICE-like changes?**

The architectural advantages of DICE are real:
- **Async/await vs restart**: Eliminates redundant computation on dependency waits
- **Series-parallel deps**: Enables smarter parallel validation
- **Rust vs Java**: Eliminates GC pauses, lower memory overhead
- **Multi-versioning**: Better concurrent build support

**But the gains are difficult to quantify:**
- SkyMeld already addresses the biggest issue (phase separation)
- Remaining differences affect constant factors, not algorithmic complexity
- Both systems achieve O(changed) recomputation with O(invalidated) traversal
- Real-world performance depends heavily on build graph shape

**Estimated impact range** (speculative, no sources):
- **Cold builds**: Minimal difference (both limited by action execution)
- **Incremental builds with few changes**: 10-30% improvement possible from reduced restart overhead
- **Multi-target incremental builds**: Potentially larger gains from series-parallel validation
- **Highly parallel builds**: Rust's lower overhead could provide 5-15% improvement

These estimates are **unsubstantiated speculation** based on architectural analysis, not measurements.

### What Would Provide Real Data

1. **Porting DICE to Java** or Skyframe to Rust (isolate language vs algorithm)
2. **Running identical workloads** on Buck2 and Bazel with equivalent rule definitions
3. **Micro-benchmarks** of restart overhead vs async suspension
4. **Series-parallel validation** benchmarks on real dependency graphs

### Sources

- [Bazel Roadmap](https://bazel.build/about/roadmap)
- [Project Skymeld Issue #14057](https://github.com/bazelbuild/bazel/issues/14057)
- [Skymeld Regression #22233](https://github.com/bazelbuild/bazel/issues/22233)
- [How Bazel 7.0 Makes Your Builds Faster](https://www.buildbuddy.io/blog/how-bazel-7-0-makes-your-builds-faster/)
- [Why Buck2](https://buck2.build/docs/about/why/)
- [Buck2 Unboxing | BuildBuddy](https://www.buildbuddy.io/blog/buck2-review/)
- [Bazel StateMachine Guide](https://bazel.build/contribute/statemachine-guide)
- [Bazel 8.0 Release](https://blog.bazel.build/2024/12/09/bazel-8-release.html)
- [BazelCon 2025 Recap](https://blog.jetbrains.com/clion/2025/11/bazelcon-2025/)

---

## Sources

- [Skyframe | Bazel](https://bazel.build/reference/skyframe)
- [The Bazel codebase](https://bazel.build/contribute/codebase)
- [Bazel Skyframe source](https://github.com/bazelbuild/bazel/tree/master/src/main/java/com/google/devtools/build/skyframe)
