# Bzlmod Dependency Resolution Algorithm

**Referenced from:** `2026-01-21-slug-bazel-compatible-build-tool.md` (Phase 4d)

This document provides an in-depth analysis of Bazel's bzlmod dependency resolution algorithm for implementing compatible behavior in Slug.

## Overview

Bzlmod uses **Minimal Version Selection (MVS)**, an algorithm introduced by Go modules. The key principle is:

> Select the **highest version** that any dependent requests, but no higher.

This is "minimal" in the sense that it picks the minimum version that satisfies all constraints - not the latest available version.

## Why MVS?

Traditional dependency resolution (npm, Maven, pip) uses SAT solvers to find compatible versions, which can be:
- Non-deterministic
- Computationally expensive
- Prone to "dependency hell"

MVS is:
- **Deterministic**: Same inputs always produce same outputs
- **Fast**: O(n) graph traversal, no backtracking
- **Reproducible**: No solver randomness
- **Predictable**: Upgrading a dependency only affects that dependency's subtree

## The Core Algorithm

### Step 1: Build the Dependency Graph

Starting from the root `MODULE.bazel`, recursively collect all `bazel_dep()` declarations:

```
Root requires: A@1.0, B@2.0
A@1.0 requires: C@1.0, D@1.5
B@2.0 requires: C@1.2, D@1.0
C@1.0 requires: (none)
C@1.2 requires: (none)
D@1.0 requires: (none)
D@1.5 requires: (none)
```

### Step 2: Group by Selection Key

Each module version is assigned to a **SelectionGroup** based on:
- `moduleName`
- `compatibilityLevel`
- `targetAllowedVersion` (for multiple_version_override)

```rust
struct SelectionGroup {
    module_name: String,
    compatibility_level: u32,
    target_allowed_version: Option<Version>,
}
```

### Step 3: Select Maximum Version per Group

For each SelectionGroup, keep only the **maximum version**:

```
Before:  C@1.0, C@1.2 (both compatibility_level=1)
After:   C@1.2 selected

Before:  D@1.0, D@1.5 (both compatibility_level=1)
After:   D@1.5 selected
```

### Step 4: Rewrite Dependencies

Update all dependency edges to point to the selected versions:

```
A@1.0 requires: C@1.2 (rewritten from C@1.0), D@1.5
B@2.0 requires: C@1.2, D@1.5 (rewritten from D@1.0)
```

### Step 5: Prune Unreachable Modules

Remove any module versions that are no longer reachable from the root after rewriting.

## Compatibility Level

### Purpose

Bazel cannot encode breaking changes in package paths like Go does (`v1` vs `v2` in import paths). Instead, modules declare a `compatibility_level`:

```python
module(
    name = "my_module",
    version = "2.0.0",
    compatibility_level = 2,  # Incompatible with compatibility_level = 1
)
```

### Conflict Detection

If the dependency graph contains two versions of the same module with **different compatibility levels** and no `multiple_version_override`, Bazel throws an error:

```
ERROR: my_module@1.5.0 (compatibility_level=1) and
       my_module@2.0.0 (compatibility_level=2)
       cannot coexist in the dependency graph.
```

### When to Increment

Increment `compatibility_level` when making a breaking change that:
- Affects most users
- Cannot be easily migrated or worked around
- Fundamentally changes the module's contract

Do NOT increment for every semver major version - only for truly incompatible changes.

## Version Format and Comparison

### Format

Bazel uses a **relaxed SemVer** format:

```
RELEASE[-PRERELEASE][+BUILD]

Examples:
  1.0.0
  2.3.1-alpha
  20210324.2        (date-based, like Abseil)
  1.0               (fewer than 3 segments OK)
  1.0.0-rc1+build5  (build metadata ignored)
```

### Comparison Rules

Versions are compared as follows:

1. **Empty version** (used for non-registry overrides) compares **higher than everything**
2. **Release segments** compared left-to-right:
   - Numeric identifiers compared as numbers
   - Non-numeric compared lexicographically
   - Numeric < non-numeric
3. **Prerelease**: A version with prerelease is **lower** than the same release without
4. **Prerelease segments** compared like release segments

```
1.0.0-alpha < 1.0.0-beta < 1.0.0-rc1 < 1.0.0 < 1.0.1
```

### Implementation Note

```rust
pub struct Version {
    release: Vec<Identifier>,
    prerelease: Vec<Identifier>,
    // build metadata is parsed but NOT stored
}

pub enum Identifier {
    Numeric(u64),
    String(String),
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        // 1. Empty versions compare highest
        // 2. Compare release segments
        // 3. Non-prerelease > prerelease
        // 4. Compare prerelease segments
    }
}
```

## Override Types

### single_version_override

Forces a specific version for a module, ignoring what the graph requests:

```python
single_version_override(
    module_name = "protobuf",
    version = "3.19.2",
    registry = "https://bcr.bazel.build",
    patches = ["//patches:protobuf.patch"],
    patch_strip = 1,
)
```

**Use cases:**
- Pin to a known-good version
- Apply security patches
- Use a different registry

### multiple_version_override

Allows multiple versions of the same module to coexist:

```python
multiple_version_override(
    module_name = "protobuf",
    versions = ["3.18.0", "3.19.0"],
)
```

**Behavior:**
1. Only listed versions can exist in final graph
2. Unlisted versions are upgraded to nearest higher allowed version at same compatibility level
3. If no higher allowed version exists at same compatibility level, error

**Use case:** Gradual migration when dependencies require different major versions.

### Non-Registry Overrides

These completely bypass version resolution:

```python
# Local filesystem
local_path_override(
    module_name = "my_local_module",
    path = "../my-local-module",
)

# Archive download
archive_override(
    module_name = "rules_cc",
    urls = ["https://example.com/rules_cc.tar.gz"],
    integrity = "sha256-...",
    strip_prefix = "rules_cc-main",
)

# Git repository
git_override(
    module_name = "rules_rust",
    remote = "https://github.com/example/rules_rust.git",
    commit = "abc123...",
)
```

**Important:** Non-registry overrides get `Version::EMPTY`, which compares higher than all other versions, ensuring they're always selected.

## Yanked Versions

Registries can mark versions as yanked (e.g., security vulnerabilities):

```json
// In BCR: modules/foo/metadata.json
{
  "versions": ["1.0.0", "1.0.1", "1.1.0"],
  "yanked_versions": {
    "1.0.0": "Security vulnerability CVE-2024-XXXX"
  }
}
```

**Behavior:**
- If resolution selects a yanked version, Bazel throws an error
- User can override with `--allow_yanked_versions=foo@1.0.0`

## Repository Naming

### Apparent Name (User-Facing)

What you use in `@repo//pkg:target`:

```python
bazel_dep(name = "rules_cc", version = "0.0.9")
# Apparent name: rules_cc
# Usage: @rules_cc//cc:defs.bzl

bazel_dep(name = "rules_cc", version = "0.0.9", repo_name = "cc_rules")
# Apparent name: cc_rules
# Usage: @cc_rules//cc:defs.bzl
```

### Canonical Name (Internal)

Internal identifier, format is unstable:
- Single version: `rules_cc+`
- Multiple versions: `rules_cc+0.0.9`

**Do not depend on canonical name format** - use `Label.repo_name` instead.

## Algorithm Pseudocode

```rust
fn resolve_mvs(root: &Module, registry: &Registry) -> Result<ResolvedGraph> {
    // Phase 1: Discover all modules
    let mut all_deps: HashMap<ModuleKey, Module> = HashMap::new();
    let mut queue = VecDeque::from([root.key()]);

    while let Some(key) = queue.pop_front() {
        if all_deps.contains_key(&key) {
            continue;
        }
        let module = registry.fetch_module(&key)?;
        for dep in &module.bazel_deps {
            queue.push_back(dep.to_module_key());
        }
        all_deps.insert(key, module);
    }

    // Phase 2: Apply overrides
    apply_overrides(&mut all_deps, &root.overrides)?;

    // Phase 3: Group by selection key
    let mut selection_groups: HashMap<SelectionGroup, Vec<Version>> = HashMap::new();
    for (key, module) in &all_deps {
        let group = SelectionGroup {
            module_name: key.name.clone(),
            compatibility_level: module.compatibility_level,
            target_allowed_version: compute_target_allowed(&root.overrides, &key),
        };
        selection_groups.entry(group).or_default().push(key.version.clone());
    }

    // Phase 4: Select max version per group
    let selected: HashMap<SelectionGroup, Version> = selection_groups
        .into_iter()
        .map(|(group, versions)| (group, versions.into_iter().max().unwrap()))
        .collect();

    // Phase 5: Check for compatibility conflicts
    check_compatibility_conflicts(&selected)?;

    // Phase 6: Build final graph with rewritten deps
    let resolved = build_resolved_graph(root, &all_deps, &selected)?;

    // Phase 7: Check for yanked versions
    check_yanked_versions(&resolved, registry)?;

    Ok(resolved)
}
```

## Edge Cases to Handle

### 1. Diamond Dependencies

```
Root → A@1.0 → C@1.0
Root → B@1.0 → C@2.0
```

**Resolution:** Select C@2.0 (max version). Both A and B use C@2.0.

### 2. Compatibility Level Conflict

```
Root → A@1.0 → C@1.0 (compatibility_level=1)
Root → B@1.0 → C@2.0 (compatibility_level=2)
```

**Resolution:** Error unless `multiple_version_override` allows both.

### 3. Circular Dependencies

```
A@1.0 → B@1.0 → A@1.0
```

**Resolution:** Allowed (detected during graph walk, just don't revisit).

### 4. Version Downgrade Request

```
Root → A@2.0
A@2.0 → B@1.0
Root → B@2.0  (higher than what A wants)
```

**Resolution:** Select B@2.0. A gets B@2.0 even though it asked for B@1.0.

### 5. Missing Transitive Dependency

```
Root → A@1.0
A@1.0 → B@1.0 (but B not in registry)
```

**Resolution:** Error with clear message about missing module.

### 6. Multiple Version Override Upgrade

```
multiple_version_override(name = "foo", versions = ["1.0", "2.0"])
Graph contains: foo@1.5
```

**Resolution:** foo@1.5 upgraded to foo@2.0 (nearest higher allowed at same compat level).

## Lockfile Integration

After resolution, results are cached in `MODULE.bazel.lock`:

```json
{
  "lockFileVersion": 24,
  "registryFileHashes": {
    "https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel": "sha256-..."
  },
  "selectedYankedVersions": {},
  "moduleDepGraph": {
    "rules_cc@0.0.9": {
      "name": "rules_cc",
      "version": "0.0.9",
      "compatibilityLevel": 0,
      "dependencies": {...}
    }
  }
}
```

**Lockfile behavior:**
- If `MODULE.bazel` unchanged and lockfile exists, skip resolution
- If `MODULE.bazel` changed, re-resolve and update lockfile
- `--lockfile_mode=error` fails if lockfile would change (CI use)

## Bazel Source Code References

| Component | Source File |
|-----------|-------------|
| Main algorithm | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/Selection.java` |
| SelectionGroup | Same file |
| Version parsing | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/Version.java` |
| Compatibility check | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/Module.java` |
| Lockfile format | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelLockFileValue.java` |
| Override handling | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileGlobals.java` |

**Key tests:**
- `src/test/java/com/google/devtools/build/lib/bazel/bzlmod/SelectionTest.java`
- `src/test/java/com/google/devtools/build/lib/bazel/bzlmod/VersionTest.java`

## Implementation Recommendations for Slug

1. **Start simple**: Implement basic MVS without multiple_version_override first
2. **Use ordered collections**: Determinism requires consistent iteration order
3. **Cache aggressively**: Registry fetches are slow; cache MODULE.bazel content
4. **Clear error messages**: Version conflicts should explain the dependency chain
5. **Match Bazel's version comparison exactly**: Test against Bazel's VersionTest cases
6. **Lockfile compatibility**: Consider using same JSON format for interop

## References

- [Bzlmod User Guide](https://bazel.build/external/module)
- [Bazel Modules Documentation](https://bazel.build/external/module)
- [Go MVS Algorithm](https://research.swtch.com/vgo-mvs) (original inspiration)
- [Bazel Source: Selection.java](https://github.com/bazelbuild/bazel/blob/master/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/Selection.java)
- [Bazel Source: Version.java](https://github.com/bazelbuild/bazel/blob/master/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/Version.java)
- [DeepWiki: Bzlmod Module System](https://deepwiki.com/bazelbuild/bazel/5.1-bzlmod-module-system)
