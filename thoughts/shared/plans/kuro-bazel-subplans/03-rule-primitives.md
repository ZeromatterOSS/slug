# Rule Primitives Phase (6)

> **Parent Plan**: [Kuro Bazel-Compatible Build Tool](../2026-01-21-kuro-bazel-compatible-build-tool.md)

This sub-plan covers ensuring kuro's rule execution API matches Bazel's ctx, actions, and provider interfaces.

---

## Phase 6: Rule Primitives and Provider Compatibility

### Overview

Ensure kuro's rule execution API matches Bazel's ctx, actions, and provider interfaces. Kuro already has substantial infrastructure that needs Bazel API alignment.

### Kuro Existing Implementation

Kuro already has most of this infrastructure. The work is primarily **API alignment**, not building from scratch:

| Feature                        | Kuro Location                                                                       | Status                                   |
| ------------------------------ | ----------------------------------------------------------------------------------- | ---------------------------------------- |
| `AnalysisContext` (ctx)        | `app/kuro_build_api/src/interpreter/rule_defs/context.rs:176`                       | Exists, needs API tweaks                 |
| `ctx.attr`                     | `app/kuro_build_api/src/interpreter/rule_defs/context.rs`                           | **Kuro uses `ctx.attrs`**, needs alias   |
| `ctx.actions`                  | `app/kuro_build_api/src/interpreter/rule_defs/context.rs:60`                        | Exists via `AnalysisActions`             |
| `ctx.actions.run()`            | `app/kuro_action_impl/src/context/run.rs:121`                                       | Exists, verify parameter names           |
| `ctx.actions.write()`          | `app/kuro_action_impl/src/context/write.rs:110`                                     | **Kuro uses positional args**, Bazel uses named |
| `ctx.actions.declare_output()` | `app/kuro_action_impl/src/context/unsorted.rs:50`                                   | **Needs alias to `declare_file()`**      |
| `DefaultInfo`                  | `app/kuro_build_api/src/interpreter/rule_defs/provider/builtin/default_info.rs:136` | Exists                                   |
| `RunInfo`                      | `app/kuro_build_api/src/interpreter/rule_defs/provider/builtin/run_info.rs`         | Exists                                   |
| `TransitiveSet`                | `app/kuro_build_api/src/interpreter/rule_defs/transitive_set/transitive_set.rs:112` | Exists, needs `depset` alias             |
| Action execution               | `app/kuro_build_api/src/actions/execute/action_executor.rs:312`                     | Exists                                   |

**Critical API Discrepancies Found During Testing:**

These discrepancies were discovered while testing `examples/bzlmod_local_test/`:

1. **`ctx.attr` vs `ctx.attrs`**: Bazel uses `ctx.attr.foo`, Kuro uses `ctx.attrs.foo`. Need to add `attr` as an alias.
2. **`declare_file()` vs `declare_output()`**: Bazel uses `ctx.actions.declare_file()`, Kuro uses `ctx.actions.declare_output()`. Need to add alias.
3. **`write()` parameter style**: Bazel accepts `write(output=out, content=text)`, Kuro requires positional `write(out, text)`. Need to accept both styles.

### Bazel Source References

This is a critical phase - the rule API must match Bazel exactly. Study these thoroughly:

| Feature                 | Bazel Source File                                                                            |
| ----------------------- | -------------------------------------------------------------------------------------------- |
| **ctx object**          | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/StarlarkRuleContextApi.java`   |
| ctx implementation      | `src/main/java/com/google/devtools/build/lib/analysis/starlark/StarlarkRuleContext.java`     |
| **actions API**         | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/StarlarkActionFactoryApi.java` |
| actions implementation  | `src/main/java/com/google/devtools/build/lib/analysis/starlark/StarlarkActionFactory.java`   |
| **Args builder**        | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/CommandLineArgsApi.java`       |
| Args implementation     | `src/main/java/com/google/devtools/build/lib/analysis/starlark/Args.java`                    |
| **DefaultInfo**         | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/DefaultInfoApi.java`           |
| **RunInfo**             | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/RunEnvironmentInfoApi.java`    |
| **OutputGroupInfo**     | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/OutputGroupInfoApi.java`       |
| **depset**              | `src/main/java/com/google/devtools/build/lib/collect/nestedset/Depset.java`                  |
| depset ordering         | `src/main/java/com/google/devtools/build/lib/collect/nestedset/Order.java`                   |
| **Runfiles**            | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/RunfilesApi.java`              |
| Runfiles implementation | `src/main/java/com/google/devtools/build/lib/analysis/Runfiles.java`                         |
| **Provider definition** | `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/ProviderApi.java`              |

**Starlark builtins (important!):**

- `src/main/starlark/builtins_bzl/common/` - Built-in rule implementations in Starlark
- These show how Bazel's own rules use the ctx/actions API

**Key tests:**

- `src/test/java/com/google/devtools/build/lib/analysis/starlark/StarlarkRuleContextTest.java`
- `src/test/java/com/google/devtools/build/lib/analysis/RunfilesTest.java`

### Changes Required:

#### 1. AnalysisContext (ctx) API Alignment

**File**: `app/kuro_build_api/src/interpreter/rule_defs/context.rs`

Current Kuro `AnalysisContext` (line 176) needs these additional/renamed attributes:

```python
# Bazel ctx attributes (ensure all available):
ctx.label              # ✓ Already exists
ctx.attr               # ✓ Already exists (line 295)
ctx.file               # ✓ Implemented (2026-02-02) - CtxFile type in context.rs
ctx.files              # ✓ Implemented (2026-02-02) - CtxFiles type in context.rs
ctx.executable         # ✓ Implemented (2026-02-02) - CtxExecutable type in context.rs
ctx.outputs            # ✓ Exists as declare_output, needs direct access
ctx.actions            # ✓ Already exists (line 178)
ctx.build_file_path    # ✓ Implemented (2026-02-02) - returns BUILD.bazel path
ctx.workspace_name     # ✓ Implemented (2026-02-02) - returns module name
ctx.bin_dir            # ✓ Implemented (2026-02-02) - CtxDirRoot type
ctx.genfiles_dir       # ✓ Implemented (2026-02-02) - CtxDirRoot type
ctx.var                # ✓ Implemented (2026-02-02) - CtxVarDict type
ctx.configuration      # NEED: Build configuration access
ctx.fragments          # NEED: Configuration fragments
```

Add to `analysis_context_methods()` (line 295):

```rust
#[starlark_module]
fn analysis_context_methods(builder: &mut MethodsBuilder) {
    // Existing methods...

    #[starlark(attribute)]
    fn file<'v>(this: &AnalysisContext<'v>) -> anyhow::Result<...> {
        // Return struct with single-file attributes
    }

    #[starlark(attribute)]
    fn files<'v>(this: &AnalysisContext<'v>) -> anyhow::Result<...> {
        // Return struct with file-list attributes
    }

    #[starlark(attribute)]
    fn executable<'v>(this: &AnalysisContext<'v>) -> anyhow::Result<...> {
        // Return struct with executable attributes
    }

    #[starlark(attribute)]
    fn build_file_path<'v>(this: &AnalysisContext<'v>) -> anyhow::Result<&str> {
        // Return path to BUILD.bazel file
    }

    #[starlark(attribute)]
    fn workspace_name<'v>(this: &AnalysisContext<'v>) -> anyhow::Result<&str> {
        // Return module name from MODULE.bazel
    }
}
```

#### 2. Actions API Alignment

**Files**: `app/kuro_action_impl/src/context/*.rs`

| Bazel Method          | Kuro File        | Status                          |
| --------------------- | ---------------- | ------------------------------- |
| `run()`               | `run.rs:121`     | ✓ Verify parameters match Bazel |
| `run_shell()`         | `unsorted.rs:313`| ✓ Implemented (stub, 2026-02-02)|
| `write()`             | `write.rs:110`   | ✓ Exists                        |
| `declare_file()`      | `unsorted.rs:98` | ✓ Implemented (2026-02-02)      |
| `declare_directory()` | `unsorted.rs:127`| ✓ Implemented (2026-02-02)      |
| `args()`              | `unsorted.rs:282`| ✓ Implemented (2026-02-02)      |
| `symlink()`           | `copy.rs`        | CHECK: May exist                |
| `expand_template()`   | TBD              | NEED: Template expansion        |

**Key addition - `ctx.actions.args()` builder:**

```rust
// New file: app/kuro_action_impl/src/context/args.rs

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ArgsBuilder {
    items: Vec<ArgItem>,
    param_file: Option<ParamFileSpec>,
}

impl<'v> StarlarkValue<'v> for ArgsBuilder {
    // Methods: add(), add_all(), add_joined(), use_param_file(), set_param_file_format()
}
```

#### 3. depset() Global Function

**File**: `app/kuro_build_api/src/interpreter/rule_defs/transitive_set/globals.rs`

Kuro uses `transitive_set()` but Bazel uses `depset()`. Add alias:

```rust
#[starlark_module]
pub fn register_depset(builder: &mut GlobalsBuilder) {
    /// Bazel-compatible depset (alias for transitive_set)
    fn depset<'v>(
        direct: Option<&List<'v>>,
        transitive: Option<&List<'v>>,
        order: Option<&str>,  // "default", "postorder", "preorder", "topological"
    ) -> anyhow::Result<TransitiveSet<'v>> {
        // Map to transitive_set implementation
    }
}
```

**Order mapping:**
| Bazel Order | Kuro Equivalent |
|-------------|-----------------|
| `"default"` | BFS traversal |
| `"postorder"` | `PostorderTransitiveSetIterator` (line 110) |
| `"preorder"` | `PreorderTransitiveSetIterator` (line 53) |
| `"topological"` | `TopologicalTransitiveSetIterator` (line 189) |

#### 3b. Depset ↔ TransitiveSet Bridge

**Goal**: Provide explicit conversion helpers for Kuro‑specific rules that want to use `transitive_set` internally while exposing Bazel `depset` at API boundaries.

**Proposed APIs** (native helpers, names subject to bikeshed):
```python
native.transitive_set_from_depset(d, order = "default")
native.depset_from_transitive_set(t, order = "default")
```

**Implementation sketch**:

- **Location**: `app/kuro_build_api/src/interpreter/rule_defs/depset.rs` (bridge helpers)
- **Supporting types**:
  - Add a built‑in `TransitiveSetDefinition` (e.g. `BazelDepsetTset`) with a single `items` field.
  - Cache conversion results on frozen values:
    - `FrozenTransitiveSet` → cached `depset` per order.
    - `depset` → cached `transitive_set` per order.
- **Semantics**:
  - `depset -> transitive_set`: preserve structure by wiring depset children into tset children where possible; keep `order` metadata for traversal hints.
  - `transitive_set -> depset`: materialize via traversal (default preorder) and build a depset; projections/reductions are dropped.
  - Explicit conversion only (no implicit coercion in attr coercers).

**Performance considerations**:
- Avoid repeated materialization: add per‑order caches to prevent hot‑path regressions.
- Conversion should be applied only at API boundaries (providers, native functions).

**Tests**:
- Round‑trip conversion preserves set membership.
- Ordering behavior tested for `"default"`, `"preorder"`, `"postorder"`, `"topological"`.
- Frozen/live conversion works in both directions.

#### 4. Built-in Providers

**Directory**: `app/kuro_build_api/src/interpreter/rule_defs/provider/builtin/`

**DefaultInfo** (line 136 in `default_info.rs`):

- ✓ Already exists with `files`, `runfiles`, `executable`
- ✓ Bazel-style `files` parameter now properly converts depset to default_outputs (fixed 2026-02-03)
  - Added `extract_depset_elements()` helper to handle both frozen and live depsets
  - Supports nested depsets with transitive children
  - Test: `tests/simple_files_test:bazel_files_test` verifies this works

**NEED: OutputGroupInfo**

```rust
// New file: output_group_info.rs
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct OutputGroupInfo {
    groups: HashMap<String, TransitiveSet>,
}
```

**NEED: CcInfo, PyInfo** (for rules_cc, rules_python integration):

```rust
// New file: cc_info.rs - Critical for Phase 7
pub struct CcInfo {
    compilation_context: CompilationContext,
    linking_context: LinkingContext,
}

pub struct CompilationContext {
    headers: TransitiveSet,
    includes: TransitiveSet,
    defines: TransitiveSet,
    // ...
}
```

#### 5. Provider Access Semantics (Completed 2026-02-02)

**Critical for rules_cc analysis phase** - The `in` operator must work for checking providers on targets and artifacts.

| Operation | Bazel Behavior | Kuro Status | Blocker For |
|-----------|---------------|-------------|-------------|
| `Provider in target` | Returns `True` if target provides Provider | ✓ Implemented (Dependency type) | rules_cc analysis |
| `Provider in artifact` | Returns `True` (source artifacts have DefaultInfo) | ✓ Implemented | rules_cc analysis |
| `target[Provider]` | Returns provider instance or error | ✓ Implemented | rules_cc analysis |
| `artifact[Provider]` | Returns DefaultInfo with artifact as default_output | ✓ Implemented | rules_cc analysis |

**Example from rules_cc** (`cc_helper.bzl:369`):
```python
# This pattern is used extensively in rules_cc
if DefaultInfo in attr_value:
    files = attr_value[DefaultInfo].files.to_list()
    # ... process files
```

**Current Error:**
```
error: Operation `in` not supported for types `default_info_callable` and `Artifact`
```

**Files to Modify:**

1. **Artifact type** - `app/kuro_build_api/src/artifact/artifact_type.rs`
   - Implement `StarlarkValue::is_in()` for Artifact to support `Provider in artifact`

2. **Target/dependency type** - Location TBD
   - Implement `StarlarkValue::is_in()` for configured targets

3. **Provider callable** - `app/kuro_build_api/src/interpreter/rule_defs/provider/`
   - Ensure `DefaultInfo` and other providers work as dict keys for `in` checks

**Implementation Notes:**

In Bazel, source files (artifacts) implicitly have `DefaultInfo` with the file in `files`. The `in` operator checks if a provider type is available on a target/artifact.

```python
# Bazel behavior:
src_file = ctx.file.src  # An artifact
DefaultInfo in src_file  # Returns True
src_file[DefaultInfo]    # Returns DefaultInfo(files=depset([src_file]))
```

#### 6. Runfiles

**File**: Likely in `app/kuro_build_api/src/interpreter/rule_defs/`

```python
ctx.runfiles(
    files = [...],
    transitive_files = depset(...),
    symlinks = {...},
    root_symlinks = {...},
    collect_data = True,
    collect_default = True,
)
```

Check Kuro's existing runfiles implementation and align API.

### Success Criteria:

#### Automated Verification:

- [ ] ctx.actions.run() executes actions correctly
- [x] ctx.actions.args() builds command lines (implemented 2026-02-02)
- [ ] depset operations are efficient
- [x] DefaultInfo provider works (including Bazel-style `files` parameter - fixed 2026-02-03)
- [x] `Provider in target` operator works (e.g., `DefaultInfo in dep`)
- [x] `Provider in artifact` operator works (e.g., `DefaultInfo in src_file`)
- [x] `target[Provider]` indexing works
- [x] `artifact[Provider]` indexing works (returns synthetic DefaultInfo)
- [x] Runfiles are collected correctly
- [x] All documented ctx methods available (implemented 2026-02-02: ctx.file, ctx.files, ctx.executable, ctx.bin_dir, ctx.genfiles_dir, ctx.workspace_name, ctx.build_file_path, ctx.var)
- [x] ctx.files.<attr> extracts File objects from dependencies (fixed 2026-02-05: CtxFiles/CtxFile now extract DefaultInfo.default_outputs from Dependency objects)
- [x] Label() in .bzl files resolves relative to .bzl file's repository (fixed 2026-02-05: uses starlark_path().cell() instead of cell_info().name())

#### Manual Verification:

- [x] Simple rule that compiles a C file works (verified 2026-02-02: cc_library from rules_cc builds successfully)
- [x] Rule with transitive dependencies collects all inputs (verified 2026-02-02: diamond dependency pattern with aspects)

#### Test Migration (Phase 6):

- [x] UPDATE `tests/core/analysis/test_cmd_args.py` for `ctx.actions.args()` API
- [ ] UPDATE `tests/core/transitive_sets/test_transitive_sets.py` → rename to `test_depset.py`
- [ ] ADD `tests/core/analysis/test_ctx_attr.py` for ctx.attr access
- [ ] ADD `tests/core/analysis/test_ctx_file.py` for ctx.file/ctx.files
- [ ] ADD `tests/core/analysis/test_ctx_actions_run.py` for ctx.actions.run()
- [ ] ADD `tests/core/analysis/test_ctx_actions_write.py` for ctx.actions.write()
- [ ] ADD `tests/core/analysis/test_ctx_actions_declare.py` for declare_file/directory
- [ ] ADD `tests/core/analysis/test_default_info.py` for DefaultInfo provider
- [ ] ADD `tests/core/analysis/test_provider_in_operator.py` for `Provider in target/artifact`
- [x] ADD `tests/core/analysis/test_runfiles.py` for runfiles collection
- [ ] ADD `tests/core/analysis/test_depset_ordering.py` for depset order parameter
- [ ] ADD `tests/core/analysis/test_depset_transitive_set_bridge.py` for explicit conversion helpers
- [ ] ADD depset ordering coverage: `default`, `preorder`, `postorder`, `topological`
- [ ] ADD depset merge-rule coverage (only same order or `default` allowed)
- [ ] ADD `tests/core/analysis/test_provider_definition.py` for custom providers

---
