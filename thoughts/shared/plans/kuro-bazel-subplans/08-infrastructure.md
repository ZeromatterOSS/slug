# Infrastructure Phases (15-17)

> **Parent Plan**: [Kuro Bazel-Compatible Build Tool](../2026-01-21-kuro-bazel-compatible-build-tool.md)

This sub-plan covers infrastructure improvements: sandboxing, platform support, and query commands.

---

## Phase 15: Local Build Isolation (Sandboxing)

### Overview

Implement local build sandboxing to ensure hermetic builds and catch undeclared dependencies.

### Bazel Source References

Bazel's sandboxing is well-documented in source. The linux-sandbox is particularly instructive:

| Feature                    | Bazel Source File                                                                      |
| -------------------------- | -------------------------------------------------------------------------------------- |
| Sandbox abstraction        | `src/main/java/com/google/devtools/build/lib/sandbox/SandboxedSpawn.java`              |
| Sandbox strategy base      | `src/main/java/com/google/devtools/build/lib/sandbox/AbstractSandboxSpawnRunner.java`  |
| **Linux sandbox**          | `src/main/java/com/google/devtools/build/lib/sandbox/LinuxSandboxedSpawnRunner.java`   |
| Linux sandbox C helper     | `src/main/tools/linux-sandbox/` (C code for namespace setup)                           |
| **macOS sandbox**          | `src/main/java/com/google/devtools/build/lib/sandbox/DarwinSandboxedSpawnRunner.java`  |
| macOS sandbox profile      | Look for `.sb` sandbox profile files                                                   |
| **Windows sandbox**        | `src/main/java/com/google/devtools/build/lib/sandbox/WindowsSandboxedSpawnRunner.java` |
| Symlink sandbox (fallback) | `src/main/java/com/google/devtools/build/lib/sandbox/SymlinkedSandboxedSpawn.java`     |
| Sandbox options            | `src/main/java/com/google/devtools/build/lib/sandbox/SandboxOptions.java`              |

**Critical implementation detail:** Study `src/main/tools/linux-sandbox/linux-sandbox.cc` - this is the actual C program that sets up Linux namespaces. You may want to write a similar helper in Rust.

**Key tests:**

- `src/test/java/com/google/devtools/build/lib/sandbox/` - Full sandbox test suite
- `src/test/shell/integration/sandboxing_test.sh` - Integration tests

### Changes Required:

#### 1. Sandbox Infrastructure

**File**: New module `kuro_sandbox/`

Create sandbox abstraction:

```rust
pub trait Sandbox {
    /// Execute an action in an isolated environment
    fn execute(&self, action: &Action, inputs: &[PathBuf], outputs: &[PathBuf]) -> Result<()>;
}
```

#### 2. Linux Sandbox Implementation

**File**: `kuro_sandbox/src/linux.rs`

Use Linux namespaces for isolation:

- Mount namespace: Create isolated filesystem view
- Symlink/bind mount declared inputs into sandbox
- Outputs written to sandbox, then copied out
- Network namespace (optional): Block network access

Similar to Bazel's `linux-sandbox`:

```rust
pub struct LinuxSandbox {
    // Sandbox root directory
    sandbox_root: PathBuf,
    // Whether to block network
    block_network: bool,
}
```

#### 3. macOS Sandbox Implementation

**File**: `kuro_sandbox/src/macos.rs`

Use `sandbox-exec` with custom profiles:

```rust
pub struct MacOsSandbox {
    profile: SandboxProfile,
}
```

Or use symlink-based sandbox (less secure but portable).

#### 4. Windows Sandbox Implementation

**File**: `kuro_sandbox/src/windows.rs`

Options:

- Symlink-based sandbox (most portable)
- Windows containers (heavier)
- Filesystem virtualization

Start with symlink-based approach:

```rust
pub struct WindowsSandbox {
    sandbox_root: PathBuf,
}
```

#### 5. Integration with Action Execution

**File**: Action execution code

```rust
fn execute_action(action: &Action) -> Result<()> {
    let sandbox = create_sandbox_for_platform()?;

    // Create sandbox with only declared inputs visible
    sandbox.execute(
        action,
        &action.inputs,
        &action.outputs,
    )?;

    // Verify outputs exist
    for output in &action.outputs {
        if !output.exists() {
            return Err(Error::MissingOutput(output.clone()));
        }
    }

    Ok(())
}
```

#### 6. Sandbox Configuration

**File**: CLI and configuration

```bash
# Enable/disable sandboxing
kuro build --sandbox=true //...   # Default on
kuro build --sandbox=false //...  # For debugging

# Sandbox strategy
kuro build --sandbox_strategy=linux-sandbox //...
kuro build --sandbox_strategy=symlink //...
```

### Success Criteria:

#### Automated Verification:

- [ ] Actions only see declared inputs
- [ ] Action fails if it reads undeclared file
- [x] Action fails if it writes outside declared outputs (undeclared /tmp writes are isolated via fresh tmpfs)
- [x] Sandbox works on Linux (namespace-based: CLONE_NEWUSER + CLONE_NEWNS, root read-only, output dirs writable)
- [ ] Sandbox works on macOS
- [ ] Sandbox works on Windows (symlink-based)
- [x] `--sandbox=false` (--nosandbox) disables sandboxing

#### Manual Verification:

- [ ] Deliberately omit an input dependency, verify build fails with sandbox
- [ ] Same build succeeds with `--sandbox=false` (proving sandbox caught it)
- [ ] Performance overhead is acceptable (< 10% slowdown)

#### Test Migration (Phase 15):

- [ ] ADD `tests/core/sandbox/test_input_isolation.py` for undeclared input detection
- [ ] ADD `tests/core/sandbox/test_output_isolation.py` for undeclared output detection
- [ ] ADD `tests/core/sandbox/test_sandbox_strategies.py` for strategy selection
- [ ] ADD `tests/core/sandbox/test_sandbox_disabled.py` for `--sandbox=false`
- [ ] Port tests from Bazel's `src/test/java/com/google/devtools/build/lib/sandbox/`
- [ ] Port shell tests from `sandboxing_test.sh`

**Implementation Note**: Start with symlink-based sandbox for all platforms, then optimize Linux with namespaces.

---

## Phase 16: Platform Support

### Overview

Ensure kuro works on Linux, Windows, and macOS.

### Changes Required:

#### 1. Linux Support (Primary)

- Test on Ubuntu, Fedora
- Linux namespace sandboxing

#### 2. Windows Support

- MSVC toolchain for rules_cc
- Handle .exe extensions
- Symlink-based sandboxing

#### 3. macOS Support

- Intel and Apple Silicon
- Xcode toolchain integration

### Success Criteria:

#### Automated Verification:

- [ ] CI passes on Linux, Windows, macOS

#### Manual Verification:

- [ ] Build same project on all three platforms

---

## Phase 17: Query Commands

### Overview

Implement Bazel-compatible query commands for build graph introspection.

### Bazel Source References

Bazel has three query engines. Study the query language carefully:

| Feature                   | Bazel Source File                                                                                 |
| ------------------------- | ------------------------------------------------------------------------------------------------- |
| **Query language parser** | `src/main/java/com/google/devtools/build/lib/query2/engine/QueryParser.java`                      |
| Query language grammar    | `src/main/java/com/google/devtools/build/lib/query2/engine/Lexer.java`                            |
| **Query functions**       | `src/main/java/com/google/devtools/build/lib/query2/engine/QueryFunctions.java`                   |
| `deps()` function         | `src/main/java/com/google/devtools/build/lib/query2/engine/DepsFunction.java`                     |
| `rdeps()` function        | `src/main/java/com/google/devtools/build/lib/query2/engine/RdepsFunction.java`                    |
| `kind()` function         | `src/main/java/com/google/devtools/build/lib/query2/engine/KindFunction.java`                     |
| Set operations            | `src/main/java/com/google/devtools/build/lib/query2/engine/BinaryOperatorExpression.java`         |
| **cquery (configured)**   | `src/main/java/com/google/devtools/build/lib/query2/cquery/ConfiguredTargetQueryEnvironment.java` |
| **aquery (action)**       | `src/main/java/com/google/devtools/build/lib/query2/aquery/ActionGraphQueryEnvironment.java`      |
| Output formatters         | `src/main/java/com/google/devtools/build/lib/query2/query/output/`                                |

**Query language specification:** https://bazel.build/query/language (official docs have the full grammar)

**Key tests:**

- `src/test/java/com/google/devtools/build/lib/query2/` - Comprehensive query tests
- `src/test/shell/integration/query_test.sh` - Integration tests

### Changes Required:

#### 1. Query Command (`kuro query`)

Query the unconfigured target graph:

```bash
kuro query "deps(//src:main)"
kuro query "rdeps(//..., //lib:foo)"
kuro query "//..." --output=label
kuro query "//..." --output=build
```

#### 2. Configured Query (`kuro cquery`)

Query with configurations applied:

```bash
kuro cquery "deps(//src:main)" --output=json
```

#### 3. Action Query (`kuro aquery`)

Query the action graph:

```bash
kuro aquery "//src:main" --output=jsonproto
```

#### 4. Query Language Compatibility

Support Bazel query syntax:

- `deps()`, `rdeps()`
- `allpaths()`, `somepath()`
- `kind()`, `attr()`
- `filter()`
- Set operations: `+`, `-`, `^`

### Success Criteria:

#### Automated Verification:

- [ ] `kuro query "deps(//...)"` returns dependencies
- [ ] `kuro cquery` shows configured targets
- [ ] `kuro aquery` shows actions
- [ ] Query output formats match Bazel

#### Manual Verification:

- [ ] IDE/tooling integration using query commands works

#### Test Migration (Phase 17):

- [ ] UPDATE `tests/core/query/test_buildfiles.py` for Bazel buildfiles() function
- [ ] ADD `tests/core/query/test_deps.py` for deps() function
- [ ] ADD `tests/core/query/test_rdeps.py` for rdeps() function
- [ ] ADD `tests/core/query/test_kind.py` for kind() function
- [ ] ADD `tests/core/query/test_attr.py` for attr() function
- [ ] ADD `tests/core/query/test_filter.py` for filter() function
- [ ] ADD `tests/core/query/test_allpaths.py` for allpaths() function
- [ ] ADD `tests/core/query/test_somepath.py` for somepath() function
- [ ] ADD `tests/core/query/test_set_operations.py` for +, -, ^ operators
- [ ] ADD `tests/core/query/test_output_formats.py` for --output=label|build|xml|json
- [ ] ADD `tests/core/query/test_cquery.py` for configured query
- [ ] ADD `tests/core/query/test_aquery.py` for action query
- [ ] Port comprehensive tests from Bazel's `bazel_query_test.sh` (50+ test cases)

---

## Testing Strategy

> **Detailed Mapping**: See [`2026-01-22-test-infrastructure-mapping.md`](../research/2026-01-22-test-infrastructure-mapping.md) for the complete test-by-test migration plan.

### Test Migration Summary

| Action      | Count | Description                         |
| ----------- | ----- | ----------------------------------- |
| KEEP+UPDATE | ~34   | Update Buck2 tests for Bazel syntax |
| DELETE      | ~32   | Remove Buck2-specific tests         |
| ADD         | ~123  | Create new Bazel-concept tests      |
| PRESERVE    | ~69   | Keep unchanged (shared concepts)    |

### Framework Preservation

We preserve the pytest-based test framework:

- **Location**: `tests/e2e_util/` (framework), `tests/core/` and `tests/e2e/` (tests)
- **Pattern**: `@buck_test()` decorator with async/await
- **Fixtures**: `test_*_data/` directories with `MODULE.bazel` and `BUILD.bazel`
- **Golden files**: `*.golden` with sanitization for non-determinism

### Unit Tests (Rust)

- `app/kuro_build_api_tests/src/attrs.rs` - Update for `attr.*` API
- `app/kuro_build_api_tests/src/actions.rs` - Update for `ctx.actions.*` API
- `app/kuro_build_api_tests/src/nodes.rs` - Preserve DICE node tests
- ADD new module: `app/kuro_bzlmod_tests/` for bzlmod resolution

### Integration Tests (Python)

- Full build tests with rules_cc, rules_rust, rules_python
- bzlmod resolution with real BCR
- Lockfile generation and caching
- Cross-platform sandbox tests

### Compatibility Tests

- Compare output with actual Bazel
- Test against real-world open source projects

### Performance Tests

- Benchmark against Bazel
- Measure sandbox overhead
- Profile memory usage

---

## Performance Considerations

### DICE Advantages to Preserve

- Incremental computation
- Parallel execution via Tokio
- Smart invalidation
- Deferred materialization

### Sandbox Performance

- Symlink-based sandbox is faster than copy-based
- Linux namespaces add minimal overhead
- Consider sandbox reuse between actions

### bzlmod Optimization

- Cache aggressively
- Parallel BCR downloads
- Use lockfile to skip resolution

---

## References

- Kuro repository: https://github.com/facebook/kuro
- Bazel documentation: https://bazel.build/
- Bazel Central Registry: https://registry.bazel.build/
- rules_cc: https://github.com/bazelbuild/rules_cc
- rules_rust: https://github.com/bazelbuild/rules_rust
- rules_python: https://github.com/bazelbuild/rules_python
- rules_oci: https://github.com/bazel-contrib/rules_oci
- Starlark specification: https://github.com/bazelbuild/starlark
- bzlmod documentation: https://bazel.build/external/module
- Costasiella kuroshimae (mascot): https://en.wikipedia.org/wiki/Costasiella_kuroshimae

### Bazel Source Code References

When implementing Bazel-compatible features, consult the Bazel source at https://github.com/bazelbuild/bazel for authoritative behavior and architectural patterns.

**Key directories:**

| Area                         | Bazel Source Path                                               |
| ---------------------------- | --------------------------------------------------------------- |
| **Starlark API definitions** | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/` |
| **Starlark builtins**        | `src/main/starlark/builtins_bzl/`                               |
| **bzlmod implementation**    | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/`     |
| **Sandboxing**               | `src/main/java/com/google/devtools/build/lib/sandbox/`          |
| **Query engine**             | `src/main/java/com/google/devtools/build/lib/query2/`           |
| **Actions**                  | `src/main/java/com/google/devtools/build/lib/actions/`          |
| **Rules (ctx, providers)**   | `src/main/java/com/google/devtools/build/lib/analysis/`         |
| **Package loading**          | `src/main/java/com/google/devtools/build/lib/packages/`         |
| **Skyframe (incremental)**   | `src/main/java/com/google/devtools/build/skyframe/`             |

**How to use these references:**

1. Clone Bazel source: `git clone https://github.com/bazelbuild/bazel`
2. Navigate to the relevant directory for the feature you're implementing
3. Study the interfaces, data structures, and algorithms
4. Pay attention to edge cases handled in tests: `src/test/java/...`
