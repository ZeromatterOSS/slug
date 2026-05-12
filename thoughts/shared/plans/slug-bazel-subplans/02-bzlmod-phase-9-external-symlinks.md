# Phase 9: External Cell Symlinks for Bazel Compatibility

> **Parent Plan**: [bzlmod Implementation Plan](./02-bzlmod.md)
> **Blocks**: Full rules_cc functionality, IDE integration, external tooling

This sub-plan covers creating actual symlinks from `bazel-external/` to the module cache,
ensuring Bazel-compatible external repository access for tools outside of Slug.

---

## Problem Statement

### Current State

Slug downloads bzlmod modules to:
```
~/.cache/slug/registry/bcr.bazel.build/modules/{name}/{version}/source/
```

Cells are registered with virtual paths:
```
bazel-external/{module}/{version}
```

A `FileOpsDelegate` transparently maps reads from virtual paths to the cache location.

### The Issue

The `FileOpsDelegate` approach works for Slug's internal file operations, but fails when:

1. **Default attribute coercion** - `attr.label(default = "@bazel_tools//tools/cpp:...")` tries to
   resolve the path directly without using `FileOpsDelegate`
2. **External tools** - IDEs, language servers, and other tools expect actual files at `bazel-external/`
3. **Build actions** - Commands spawned by Slug can't use `FileOpsDelegate`

### Bazel's Approach

Bazel creates actual directories/symlinks that external tools can access:

```
$output_base/
├── external/
│   ├── bazel_skylib~1.5.0/      # Extracted module contents
│   ├── rules_cc~0.0.9/          # Actual files, not virtual
│   └── ...
└── execroot/__main__/
    └── external -> ../../external  # Symlink for build actions

workspace/
└── bazel-external -> $output_base/external  # Convenience symlink (optional)
```

**Key insight**: Bazel's `external/` directory contains real files/symlinks that any tool can access.

---

## Implementation Plan

### Phase 9a: Create External Symlinks During Resolution

**Goal**: After MVS resolution, create symlinks from `bazel-external/{module}/{version}` to the
cached source directory.

#### Files to Modify

**`app/slug_common/src/legacy_configs/cells.rs`**

After downloading and caching a module, create the symlink:

```rust
// Around line 640, after determining source_path
let external_path = format!("bazel-external/{}/{}", module_name, module_info.version);
let external_dir = project_root.resolve(ProjectRelativePath::new(&external_path)?);

// Create parent directories
if let Some(parent) = external_dir.parent() {
    std::fs::create_dir_all(parent)?;
}

// Create symlink to cache (if not already exists)
if !external_dir.exists() {
    #[cfg(unix)]
    std::os::unix::fs::symlink(&source_path, &external_dir)?;
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&source_path, &external_dir)?;
}
```

**`app/slug_bzlmod/src/synthetic_repos.rs`**

Synthetic repos (like `local_config_cc`, `bazel_features_version`) are generated in-memory.
These need to be materialized to `bazel-external/` as actual directories:

```rust
// In materialize_synthetic_repos(), use the project's bazel-external/ directory
pub fn materialize_synthetic_repos(
    repos: &[SyntheticRepo],
    project_root: &Path,  // Changed from arbitrary base_dir
) -> anyhow::Result<Vec<PathBuf>> {
    let base_dir = project_root.join("bazel-external");
    // ... rest of implementation
}
```

#### Success Criteria (Phase 9a)

- [x] `bazel-external/` directory created in project root
- [x] Each bzlmod module has a symlink: `bazel-external/{module}/{version}` → `~/.cache/slug/.../source/`
- [x] Synthetic repos materialized to `bazel-external/{repo_name}/`
- [x] Symlinks are idempotent (re-running doesn't fail)
- [x] `cargo build -p slug` succeeds
- [x] `slug build //:a` in manual_test succeeds without manual symlinks

---

### Phase 9b: Bazel-Compatible Naming Convention

**Goal**: Match Bazel's canonical repository naming for better tooling compatibility.

#### Bazel's Naming Convention (Bazel 7.x)

```
{module_name}~{version}           # Regular modules: bazel_skylib~1.5.0
{module_name}~                    # Single version: bazel_features~
{module}~{version}~{ext}~{repo}   # Extension repos: rules_cc~0.0.9~cc_configure~local_config_cc
```

#### Bazel 8.0+ Uses `+` Instead of `~`

```
{module_name}+{version}           # bazel_skylib+1.5.0
```

#### Implementation

For Bazel 9.0 compatibility, use `+` delimiter:

```rust
// In cells.rs, change external_path format
let external_path = if module_info.version.is_empty() {
    format!("bazel-external/{}+", module_name)
} else {
    format!("bazel-external/{}+{}", module_name, module_info.version)
};
```

#### Success Criteria (Phase 9b)

- [x] External paths use `{module}+{version}` format (2026-03-12)
- [ ] Extension repos use `{module}+{version}+{ext}+{repo}` format
- [x] Cell aliases map apparent names to canonical paths (existing cell alias system)
- [x] `@rules_cc` resolves to `bazel-external/rules_cc+0.2.16/` (2026-03-12)

---

### Phase 9c: Symlink Management and Cleanup

**Goal**: Handle symlink lifecycle - updates, stale symlinks, and cleanup.

#### Scenarios to Handle

1. **Module version update**: Old symlink should be removed or kept (for reproducibility)
2. **Stale symlinks**: Symlinks pointing to deleted cache entries
3. **Manual cleanup**: `slug clean` should optionally remove `bazel-external/`

#### Implementation

**Symlink Validation** (in cells.rs):
```rust
fn validate_external_symlink(symlink_path: &Path, expected_target: &Path) -> bool {
    match std::fs::read_link(symlink_path) {
        Ok(target) => target == expected_target,
        Err(_) => false,
    }
}

// Before creating symlink, check if it's correct
if symlink_path.exists() {
    if symlink_path.is_symlink() {
        if !validate_external_symlink(&symlink_path, &source_path) {
            std::fs::remove_file(&symlink_path)?;  // Remove stale symlink
        } else {
            continue;  // Symlink already correct
        }
    } else {
        // It's a real directory - leave it alone or warn
        tracing::warn!("bazel-external/{} is a directory, not a symlink", module_name);
        continue;
    }
}
```

**Clean Command Integration** (future):
```rust
// In slug clean implementation
if args.external {
    let external_dir = project_root.join("bazel-external");
    if external_dir.exists() {
        std::fs::remove_dir_all(&external_dir)?;
    }
}
```

#### Success Criteria (Phase 9c)

- [x] Stale symlinks are detected and removed (2026-03-12)
- [x] Version updates create new symlinks correctly (2026-03-12, ensure_symlink detects wrong target)
- [x] Real directories in `bazel-external/` are not deleted (2026-03-12, ensure_symlink skips non-symlinks)
- [ ] `slug clean --external` removes `bazel-external/` (optional flag)

---

### Phase 9d: Windows Support

**Goal**: Handle Windows-specific symlink requirements.

#### Windows Considerations

1. **Symlinks require admin or Developer Mode** - Fall back to junction points or directory copies
2. **Path separators** - Ensure consistent handling
3. **Long paths** - Enable long path support if needed

#### Implementation

```rust
#[cfg(windows)]
fn create_external_link(source: &Path, target: &Path) -> std::io::Result<()> {
    // Try symlink first (requires privileges)
    if let Err(_) = std::os::windows::fs::symlink_dir(target, source) {
        // Fall back to junction point
        junction::create(target, source)?;
    }
    Ok(())
}

#[cfg(unix)]
fn create_external_link(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, source)
}
```

#### Success Criteria (Phase 9d)

- [x] Symlinks work on Unix (Linux, macOS) (2026-03-12, #[cfg(unix)] std::os::unix::fs::symlink)
- [x] Junction points work on Windows without admin (2026-03-12, fallback to mklink /j)
- [ ] Fallback to directory copy if junctions fail
- [x] Path handling is consistent across platforms (2026-03-12, tested on Windows)

---

## Migration Path

### For Existing Projects

1. **First run after update**: Slug creates `bazel-external/` with symlinks
2. **Manual symlinks**: Can be removed after Slug creates them
3. **`.gitignore`**: Add `bazel-external/` (it's generated, like Bazel's)

### Recommended `.gitignore` Addition

```gitignore
# Slug/Bazel external repositories
bazel-external/
```

---

## Testing Strategy

### Manual Test Updates

Update `tests/manual_test/` to verify symlink creation:

```bash
# After running slug build
ls -la bazel-external/
# Should show symlinks to ~/.cache/slug/...

# Verify symlink targets
readlink bazel-external/rules_cc+0.2.16
# Should show ~/.cache/slug/registry/bcr.bazel.build/modules/rules_cc/0.2.16/source
```

### Automated Tests

Add to `tests/e2e/bzlmod/`:
- `test_external_symlinks.py` - Verify symlink creation
- `test_symlink_update.py` - Verify version change handling
- `test_synthetic_repo_materialization.py` - Verify synthetic repos exist

---

## Dependencies

### This Plan Depends On

- [x] Phase 4: Bzlmod resolution and caching
- [x] Phase 5: Module extensions and synthetic repos

### This Plan Unblocks

- [ ] Full rules_cc functionality (cc_library instantiation)
- [ ] IDE integration (IntelliJ, VS Code with Bazel plugin)
- [ ] External tooling (compilation database generators, etc.)
- [ ] Build actions that read external files directly

---

## Estimated Complexity

| Phase | Complexity | Files Changed | New Files |
|-------|------------|---------------|-----------|
| 9a | Medium | 2 | 0 |
| 9b | Low | 1 | 0 |
| 9c | Medium | 2 | 0 |
| 9d | Medium | 1 | 0 |

**Total Estimate**: Medium complexity, primarily in `cells.rs` and `synthetic_repos.rs`

---

## Open Questions

1. **Should we support both `~` and `+` delimiters?** Bazel 7.x uses `~`, Bazel 8.0+ uses `+`.
   Slug targets Bazel 9.0 compatibility, so `+` is recommended.

2. **Should `bazel-external/` be in the output directory instead of project root?**
   Bazel puts it in `$output_base/external/`. For simplicity, project root is easier but
   pollutes the workspace.

3. **How to handle `bazel-external/` in Buck2 compatibility mode?**
   Buck2 doesn't use this directory. Could be disabled via config.
