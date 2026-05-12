# Prelude Architecture Phase (6b)

> **Status: SUPERSEDED.** Use
> [04-prelude-architecture.md](./04-prelude-architecture.md) as the
> authoritative prelude plan. This duplicate preserves older history and may
> contain stale `.buckconfig`/`BUCK` references.
>
> **Parent Plan**: [Slug Bazel-Compatible Build Tool](../2026-01-21-slug-bazel-compatible-build-tool.md)

This sub-plan covers the prelude architecture: preserving Buck2's prelude loading mechanism, migrating Bazel shims from native Rust to Starlark, and removing unused Buck2-specific prelude code.

---

## Overview

Slug inherits Buck2's prelude architecture which provides a powerful mechanism for injecting symbols into BUILD files. This architecture should be **preserved** while being **adapted** for Bazel compatibility.

### Current Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           SYMBOL INJECTION FLOW                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Native Rust Globals                 Prelude (Starlark)                     │
│  ────────────────────                ──────────────────                     │
│  register_build_api_globals()        prelude/prelude.bzl                    │
│    ├── cc_common (CcCommonModule)      └── load("native.bzl")               │
│    ├── proto_common_do_not_use              │                               │
│    ├── apple_common                   prelude/native.bzl                    │
│    ├── config_common                    ├── __shimmed_native__ = dict()     │
│    ├── CcInfo = None                    ├── .update(__slug_builtins__)      │
│    ├── DebugPackageInfo = None          ├── .update(__rules__)              │
│    ├── CcSharedLibraryInfo = None       ├── .update(_user_rules)            │
│    ├── ProtoInfo = None                 └── native = struct(**dict)         │
│    └── CcToolchainInfo                                                      │
│            │                                     │                          │
│            ▼                                     ▼                          │
│  ┌──────────────────────────────────────────────────────────────┐           │
│  │                    __slug_builtins__                          │           │
│  │   (namespace in base_globals containing native functions)     │           │
│  └──────────────────────────────────────────────────────────────┘           │
│                               │                                             │
│                               ▼                                             │
│  ┌──────────────────────────────────────────────────────────────┐           │
│  │                   BUILD File Globals                          │           │
│  │   Extracted from prelude's `native` struct via file_loader    │           │
│  └──────────────────────────────────────────────────────────────┘           │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### CRITICAL: Symbol Availability by Context (2026-01-28 Discovery)

| Context                     | Base Globals | Prelude Symbols | native.* Extraction |
|-----------------------------|--------------|-----------------|---------------------|
| Root cell BUILD file        | ✓            | ✓               | ✓                   |
| Root cell .bzl file         | ✓            | ✓               | ✗                   |
| **External cell .bzl file** | **✓**        | **✗**           | **✗**               |

**This is why Bazel modules (apple_common, config_common, etc.) MUST remain native:**

External cells (rules_cc, bazel_skylib, protobuf, etc.) access these at module level:
```python
# rules_cc//cc/private/rules_impl/objc_common.bzl:22
_apple_toolchain = apple_common.apple_toolchain()

# rules_cc//cc/find_cc_toolchain.bzl:131
return [config_common.toolchain_type(CC_TOOLCHAIN_TYPE, mandatory = mandatory)]
```

External .bzl files **do not receive prelude injection** - they only see base globals
from `REGISTER_BUCK2_BUILD_API_GLOBALS`. See `interpreter_for_dir.rs:create_env()`.

### Key Files

| File | Purpose |
|------|---------|
| `prelude/prelude.bzl` | Entry point, loads native.bzl |
| `prelude/native.bzl` | Constructs `native` struct from builtins + rules |
| `prelude/rules.bzl` | Rule definitions (filegroup, genrule, etc.) |
| `app/slug_interpreter/src/file_loader.rs` | Extracts `native` struct → BUILD globals |
| `app/slug_interpreter_for_build/src/interpreter/globals.rs` | Creates `__slug_builtins__` namespace |
| `app/slug_build_api/src/interpreter/more.rs` | Registers Bazel shim modules |

---

## Goals

1. **Preserve Buck2's prelude loading mechanism** - The mechanism is well-designed and works
2. **Move Bazel shims from native Rust to Starlark** - Reduce native code complexity
3. **Remove unused Buck2 prelude code** - Reduce maintenance burden
4. **Create minimal Bazel-compatible prelude** - Only what's needed for Bazel rules

---

## Phase 6b.1: Preserve Prelude Loading Mechanism

### What to Keep

The following components should be preserved exactly as-is:

#### 1. Prelude Path Resolution
**File**: `app/slug_interpreter/src/prelude_path.rs`

```rust
// This resolves @prelude//:prelude.bzl from the cell alias
pub fn get_prelude_path(root_resolver: &CellResolver) -> PreludePath {
    // 1. Look up "prelude" cell alias in root cell
    // 2. Construct ImportPath to prelude.bzl
    // 3. Return PreludePath wrapper
}
```

#### 2. Native Symbol Extraction
**File**: `app/slug_interpreter/src/file_loader.rs:116-134`

```rust
// This extracts the `native` struct and injects members as BUILD globals
pub fn get_native_symbols_from_prelude(prelude_env: &FrozenModule) -> Vec<(String, Value)> {
    // 1. Get `native` value from prelude
    // 2. Validate it's a struct
    // 3. Iterate members and return as (name, value) pairs
}
```

#### 3. Base Globals with __slug_builtins__
**File**: `app/slug_interpreter_for_build/src/interpreter/globals.rs:154-162`

```rust
pub fn base_globals() -> GlobalsBuilder {
    let mut builder = GlobalsBuilder::standard().with(register_all_natives);
    // __slug_builtins__ namespace makes native functions available to prelude
    builder.namespace("__slug_builtins__", register_all_natives);
    builder
}
```

### Why This Architecture Works

1. **Separation of concerns**: Native code provides primitives, Starlark provides composition
2. **Extensibility**: Users can add custom rules via `prelude/user/`
3. **Introspection**: Rules are visible via `native.*` in .bzl files
4. **Performance**: Native primitives are fast, Starlark is flexible

---

## Phase 6b.2: Bazel Shim Architecture (REVISED 2026-01-28)

### Current State

Native Rust modules in `app/slug_build_api/src/interpreter/rule_defs/`:

| Module | Status | Can Move to Starlark? |
|--------|--------|----------------------|
| `cc_common.rs` | Stub methods + action primitives | **No** - External cells need native |
| `proto_common.rs` | Stub methods + compile() | **No** - External cells need native |
| `apple_common.rs` | Simple stubs | **No** - External cells need native |
| `config_common.rs` | Simple stubs | **No** - External cells need native |
| `coverage_common.rs` | Simple stubs | **No** - External cells need native |
| `platform_common.rs` | Provider stubs | **No** - External cells need native |

### Why Starlark Migration is Not Possible

Per the architectural constraint discovered on 2026-01-28:

1. External cell .bzl files only see **base globals** (from `REGISTER_BUCK2_BUILD_API_GLOBALS`)
2. Prelude injection only affects BUILD files and root cell .bzl files
3. rules_cc, bazel_skylib, protobuf, etc. access these modules at **module level** in their .bzl files
4. There is no mechanism to inject Starlark-defined values into external cell .bzl file globals

**Conclusion**: These modules must remain as native Rust registrations in
`app/slug_build_api/src/interpreter/rule_defs/`.

### Migration Strategy

#### Step 1: Keep Action Primitives in Rust

These methods **must remain native** because they create actual build actions:

**cc_common (keep in Rust):**
- `create_cc_compile_action()` - Creates compile actions
- `create_cc_compile_action_template()` - Tree artifact templates
- `wrap_link_actions()` - Link action wrapping
- `declare_compile_output_file()` - Output file declaration
- `declare_other_output_file()` - Auxiliary outputs
- `actions2ctx_cheat()` - Context access from actions

**proto_common (keep in Rust):**
- `compile()` - Creates proto compilation actions

**Core types (keep in Rust):**
- `CcToolchainVariables` - Needs native storage/lookup
- `ToolchainTypeRequirement` - Needs native resolution
- Provider types - Need native registration mechanism

#### Step 2: Move Stub Methods to Starlark

These methods just return hardcoded values and can be pure Starlark:

**cc_common stubs → `bazel_tools/tools/cpp/cc_common.bzl`:**
- `get_tool_for_action()` - Returns tool paths
- `get_execution_requirements()` - Returns empty dict
- `action_is_enabled()` - Returns True
- `get_memory_inefficient_command_line()` - Returns empty list
- `get_environment_variables()` - Returns empty dict
- `empty_variables()` - Creates empty struct
- `legacy_cc_flags_make_variable_do_not_use()` - Returns empty string
- All `check_*` and `incompatible_*` methods - Return booleans

**cc_common.internal_DO_NOT_USE stubs → same file:**
- `get_artifact_name_for_category()` - String manipulation
- `combine_cc_toolchain_variables()` - Struct merging
- `cc_toolchain_variables()` - Struct creation
- `freeze()` - Value pass-through
- `get_link_args()` - Returns empty list
- `compute_output_name_prefix_dir()` - String manipulation
- All `create_header_info*` methods - Dict creation

**proto_common stubs → `bazel_tools/tools/build_defs/proto/proto_common.bzl`:**
- `proto_path_flag()` - Returns "--proto_path="
- `descriptor_set_flag()` - Returns "--descriptor_set_out="
- `experimental_use_proto_source_order()` - Returns False
- `get_tool_path()` - Returns "/usr/bin/protoc"
- `has_plugin()` - Returns False

**apple_common → `bazel_tools/tools/apple/apple_common.bzl`:**
- `apple_toolchain()` - Returns struct
- `Objc` - Provider stub
- `platform_type` - Struct with platform strings
- `XcodeVersionConfig` - Provider stub
- `AppleDynamicFramework` - Provider stub

**config_common → `bazel_tools/tools/build_defs/config_common.bzl`:**
- `toolchain_type()` - Creates struct
- `feature_flag_info()` - Returns None

#### Step 3: Bazel Globals Injection

Create a mechanism to inject Bazel-specific globals from `@bazel_tools`:

**Option A: Prelude loads bazel_tools (Recommended)**

Add to `prelude/native.bzl`:
```starlark
# Load Bazel compatibility modules
load("@bazel_tools//tools/build_defs:globals.bzl", "BAZEL_TOOLS_GLOBALS")

# Inject Bazel globals into native
__shimmed_native__.update(BAZEL_TOOLS_GLOBALS)
```

**Option B: Native registration of Starlark modules**

Keep minimal native types but have them delegate to loaded Starlark:
```rust
// In cc_common.rs
const cc_common: CcCommonModule = CcCommonModule;
// CcCommonModule.get_methods() delegates most calls to loaded Starlark
```

#### Step 4: None Placeholders Stay Native

The None placeholders for deprecated providers must stay in native Rust because they need to be available before any Starlark loads:

```rust
// In cc_common.rs - KEEP these
const CcInfo: NoneType = NoneType;
const DebugPackageInfo: NoneType = NoneType;
const CcSharedLibraryInfo: NoneType = NoneType;

// In proto_common.rs - KEEP this
const ProtoInfo: NoneType = NoneType;
```

**Why**: Code checks `if CcInfo == None` during early loading, before prelude or bazel_tools are available.

---

## Phase 6b.3: Remove Unused Buck2 Prelude Code

### Buck2-Specific Code to Remove

The following prelude directories contain Buck2-specific rules that are not used for Bazel compatibility:

| Directory | Contents | Action |
|-----------|----------|--------|
| `prelude/android/` | Android build rules | **REMOVE** - Use rules_android |
| `prelude/apple/` | Apple/iOS build rules | **REMOVE** - Use rules_apple |
| `prelude/erlang/` | Erlang build rules | **REMOVE** |
| `prelude/haskell/` | Haskell build rules | **REMOVE** |
| `prelude/csharp/` | C# build rules | **REMOVE** |
| `prelude/cxx/` | Buck2 C++ rules | **REMOVE** - Use rules_cc |
| `prelude/go_bootstrap/` | Go bootstrap rules | **REMOVE** - Use rules_go |
| `prelude/python/` | Buck2 Python rules | **REMOVE** - Use rules_python |
| `prelude/rust/` | Buck2 Rust rules | **REMOVE** - Use rules_rust |
| `prelude/java/` | Java build rules | **REMOVE** - Use rules_java |
| `prelude/kotlin/` | Kotlin build rules | **REMOVE** |
| `prelude/ocaml/` | OCaml build rules | **REMOVE** |
| `prelude/julia/` | Julia build rules | **REMOVE** |
| `prelude/js/` | JavaScript build rules | **REMOVE** |
| `prelude/lua/` | Lua build rules | **REMOVE** |
| `prelude/third-party/` | Third-party rule helpers | **REVIEW** |

### Files to Keep (Core Infrastructure)

| File/Directory | Purpose | Action |
|----------------|---------|--------|
| `prelude/prelude.bzl` | Entry point | **KEEP** - Simplify |
| `prelude/native.bzl` | Native struct construction | **KEEP** - Simplify |
| `prelude/rules.bzl` | Core rule declarations | **KEEP** - Reduce to essentials |
| `prelude/rules_impl.bzl` | Core rule implementations | **KEEP** - Reduce to essentials |
| `prelude/alias.bzl` | Alias rule | **KEEP** |
| `prelude/filegroup.bzl` | Filegroup rule | **KEEP** |
| `prelude/genrule.bzl` | Genrule implementation | **KEEP** |
| `prelude/sh_binary.bzl` | Shell binary rule | **KEEP** |
| `prelude/sh_test.bzl` | Shell test rule | **KEEP** |
| `prelude/test_suite.bzl` | Test suite rule | **KEEP** |
| `prelude/export_file.bzl` | Export file rule | **KEEP** |
| `prelude/paths.bzl` | Path utilities | **KEEP** |
| `prelude/utils/` | General utilities | **KEEP** |
| `prelude/artifacts.bzl` | Artifact utilities | **KEEP** |
| `prelude/user/` | User customization point | **KEEP** |
| `prelude/decls/` | Rule attribute declarations | **REVIEW** |
| `prelude/bxl/` | BXL support files | **KEEP** - For developer tooling |
| `prelude/toolchains/` | Toolchain definitions | **REVIEW** - May need adaptation |

### Minimal Prelude Structure

After cleanup:

```
prelude/
├── prelude.bzl          # Entry point (simplified)
├── native.bzl           # Native struct construction (simplified)
├── rules.bzl            # Core rules: alias, filegroup, genrule, sh_*, export_file
├── rules_impl.bzl       # Core rule implementations
├── alias.bzl
├── filegroup.bzl
├── genrule.bzl
├── sh_binary.bzl
├── sh_test.bzl
├── test_suite.bzl
├── export_file.bzl
├── paths.bzl
├── artifacts.bzl
├── artifact_tset.bzl
├── utils/
│   └── *.bzl
├── user/
│   └── all.bzl          # User customization point
├── bxl/
│   └── *.bzl            # BXL support
├── decls/
│   └── common.bzl       # Common declarations
├── .buckconfig          # Prelude cell config
└── BUCK                 # Prelude package
```

---

## Phase 6b.4: Simplify Native Module Registration

### Current Registration (more.rs)

```rust
pub fn register_build_api_globals(globals: &mut GlobalsBuilder) {
    register_apple_common(globals);   // 260 lines of Rust
    register_cc_common(globals);      // 640 lines of Rust
    register_config_common(globals);  // 170 lines of Rust
    register_proto_common(globals);   // 290 lines of Rust
    // ... many more registrations
}
```

### Target Registration (after migration)

```rust
pub fn register_build_api_globals(globals: &mut GlobalsBuilder) {
    // Minimal Bazel shims - only action primitives and None placeholders
    register_cc_common_minimal(globals);    // ~100 lines - action primitives only
    register_proto_common_minimal(globals); // ~50 lines - compile() only

    // None placeholders (cannot move to Starlark)
    register_bazel_provider_stubs(globals); // CcInfo, ProtoInfo, etc. = None

    // Keep existing Buck2 infrastructure
    register_rule_defs(globals);
    register_provider_defs(globals);
    // ...
}
```

### New Minimal Modules

**cc_common_minimal.rs** (~100 lines):
```rust
#[starlark_module]
pub fn register_cc_common_minimal(globals: &mut GlobalsBuilder) {
    const cc_common: CcCommonMinimalModule = CcCommonMinimalModule;
    const CcInfo: NoneType = NoneType;
    const DebugPackageInfo: NoneType = NoneType;
    const CcSharedLibraryInfo: NoneType = NoneType;
    const CcToolchainInfo: CcToolchainInfoProvider = CcToolchainInfoProvider;
}

// CcCommonMinimalModule only has action primitives
// All stub methods removed - they live in bazel_tools Starlark
```

---

## Implementation Order

### Step 1: Verify Starlark Implementations Work
- [ ] Ensure `bazel_tools/tools/cpp/cc_common.bzl` has all needed methods
- [ ] Ensure `bazel_tools/tools/build_defs/proto/proto_common.bzl` is complete
- [ ] Create `bazel_tools/tools/apple/apple_common.bzl`
- [ ] Create `bazel_tools/tools/build_defs/config_common.bzl`
- [ ] Test that rules_cc can load using Starlark-only modules

### Step 2: Create Minimal Native Modules
- [ ] Create `cc_common_minimal.rs` with action primitives only
- [ ] Create `proto_common_minimal.rs` with compile() only
- [ ] Create `bazel_provider_stubs.rs` for None placeholders
- [ ] Update `more.rs` to use minimal modules

### Step 3: Clean Up Prelude
- [ ] Remove unused language-specific directories
- [ ] Simplify `native.bzl` to only load essentials
- [ ] Update `rules.bzl` to only include core rules
- [ ] Test that existing functionality still works

### Step 4: Integrate Bazel Globals
- [ ] Add bazel_tools loading to prelude (Option A) or native delegation (Option B)
- [ ] Test that BUILD files see all expected Bazel globals
- [ ] Verify rules_cc/rules_proto can load and execute

---

## Success Criteria

### Functional Requirements
- [ ] All existing tests pass
- [ ] rules_cc 0.2.16 loads successfully
- [ ] Bazel-style BUILD files work unchanged
- [ ] BXL functionality preserved

### Code Quality Requirements
- [ ] Native Bazel shim code reduced by >60% (from ~1400 to <500 lines)
- [ ] Prelude directory reduced by >70% (remove 15+ unused language dirs)
- [ ] Clear separation: action primitives (Rust) vs stubs (Starlark)

### Documentation Requirements
- [ ] Architecture diagram updated
- [ ] CLAUDE.md updated with prelude structure explanation
- [ ] Comments in code explain why things are where they are

---

## Risks and Mitigations

### Risk: Breaking existing Buck2 functionality
**Mitigation**:
- Keep all Buck2 infrastructure intact initially
- Only remove language-specific prelude code
- Comprehensive testing before each removal

### Risk: Circular dependency between prelude and bazel_tools
**Mitigation**:
- None placeholders stay native (available before any loads)
- bazel_tools provides methods, not required for basic loading
- Prelude loads bazel_tools only for BUILD file globals

### Risk: Performance regression from Starlark vs native
**Mitigation**:
- Action primitives stay native (critical path)
- Only stub methods (rarely called) move to Starlark
- Profile if any slowdown detected

---

## Related Documents

- [rules_cc Native Requirements](../research/2026-01-26-rules-cc-native-requirements.md)
- [03-rule-primitives.md](./03-rule-primitives.md) - Context API alignment
- [02-bzlmod.md](./02-bzlmod.md) - Module system integration
