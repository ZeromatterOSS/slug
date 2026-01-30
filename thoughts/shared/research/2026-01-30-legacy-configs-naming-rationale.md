---
date: 2026-01-30T12:00:00-08:00
researcher: Claude
git_commit: fa5944048db114fd66629dc8c23ad1773609f407
branch: main
repository: kuro
topic: "Why is kuro_common/legacy_configs called 'legacy'? Is there a newer config system?"
tags: [research, codebase, legacy_configs, configuration, buck1, buck2, buckconfig]
status: complete
last_updated: 2026-01-30
last_updated_by: Claude
---

# Research: Why is legacy_configs Called "Legacy"?

**Date**: 2026-01-30T12:00:00-08:00
**Researcher**: Claude
**Git Commit**: fa5944048db114fd66629dc8c23ad1773609f407
**Branch**: main
**Repository**: kuro

## Research Question

Why is `kuro_common/legacy_configs` called "legacy"? Is there a newer configuration system that should be used preferentially?

## Summary

The `legacy_configs` module is called "legacy" because it handles **Buck v1 concepts** - specifically the `.buckconfig` INI file format. This is explicitly documented in the module header:

```rust
//! Contains utilities for dealing with buckv1 concepts (ex. buckv1's
//! .buckconfig files as configuration)
```

**Key finding**: There is NOT a direct replacement for `legacy_configs`. It's still actively used and required for all Kuro projects. However, there IS a different configuration system (`kuro_core/configuration`) that serves a completely different purpose:

| System | Purpose | Files | Status |
|--------|---------|-------|--------|
| `legacy_configs` | Project-level settings | `.buckconfig` INI files | Active, required |
| `kuro_core/configuration` | Build-level configuration | Starlark rules (`platform()`, `constraint_*`) | Active, modern |

These are **complementary systems**, not replacements for each other.

## Detailed Findings

### What legacy_configs Does

The `legacy_configs` module (`app/kuro_common/src/legacy_configs/`) handles:

1. **Cell definitions** - Mapping cell names to filesystem paths
2. **Project settings** - Compiler paths, tool configurations, feature flags
3. **Target aliases** - Shortcuts like `app = //apps/myapp:app`
4. **Config file parsing** - The `.buckconfig` INI format with extensions

**Key components:**
- `cells.rs` - `BuckConfigBasedCells` parses `[cells]` section
- `configs.rs` - `LegacyBuckConfig` main config type
- `parser.rs` - INI file parser with Buck extensions (includes, references)
- `dice.rs` - DICE integration for incremental config access

### What the "Modern" Configuration System Does

The `kuro_core/configuration` module handles **build-time configuration** - how targets are built for different platforms:

1. **Platforms** - Target platform definitions (linux-x86_64, macos-arm64, etc.)
2. **Constraints** - `constraint_setting` and `constraint_value` rules
3. **Transitions** - How configuration changes during dependency traversal
4. **select()** - Conditional attribute values based on configuration

This is the Bazel-compatible configuration model that Buck2/Kuro uses.

### Why "Legacy" is Misleading

The term "legacy" suggests deprecation, but:

1. **Still required** - Every Kuro project needs a `.buckconfig` file
2. **Actively maintained** - Recent commits add features (bzlmod integration)
3. **No replacement exists** - For project settings, there's no alternative

The "legacy" label refers to **Buck v1 heritage**, not deprecation status:
- Buck v1 (2013-2020): Java-based, rules in Java, .buckconfig for everything
- Buck v2/Kuro (2020+): Rust-based, rules in Starlark, but .buckconfig retained for project settings

### Future Direction: Removing .buckconfig Requirement

From `thoughts/shared/plans/kuro-bazel-subplans/02-bzlmod-phase-5b.md`:

> **Future Work: Remove `.buckconfig` Requirement**
>
> **Current state:** Pure bzlmod projects still require a `.buckconfig` file with:
> - Root cell definition
> - Cell aliases to prevent errors from external configs
> - `.buckroot` marker file
>
> **Goal:** Projects with `MODULE.bazel` should work without any Buck-specific configuration files.

This work is **not yet complete**. For now, `.buckconfig` remains required.

## Code References

- `app/kuro_common/src/legacy_configs.rs:11-12` - Module documentation with "buckv1" reference
- `app/kuro_common/src/legacy_configs/configs.rs:33-34` - `LegacyBuckConfig` main type definition
- `app/kuro_common/src/legacy_configs/cells.rs:252-434` - `BuckConfigBasedCells` with bzlmod stub
- `app/kuro_core/src/configuration.rs:11-19` - Modern configuration system documentation
- `docs/concepts/buckconfig.md` - User-facing .buckconfig documentation

## Architecture Insights

### Two Configuration Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                      User's Project                              │
├─────────────────────────────────────────────────────────────────┤
│  .buckconfig (legacy_configs)     │  BUCK files (configuration) │
│  ─────────────────────────────    │  ────────────────────────── │
│  • Cell definitions               │  • platform() rules         │
│  • Tool paths                     │  • constraint_setting()     │
│  • Feature flags                  │  • select() statements      │
│  • Aliases                        │  • Transitions              │
│                                   │                             │
│  "Where things are"               │  "How to build things"      │
└─────────────────────────────────────────────────────────────────┘
```

### Historical Context: Buck v1 vs Buck v2/Kuro

| Aspect | Buck v1 | Buck v2/Kuro |
|--------|---------|--------------|
| Core language | Java | Rust |
| Rule definitions | Java | Starlark |
| Project config | .buckconfig | .buckconfig (retained) |
| Build config | Limited | Full platform/constraint model |
| Incrementality | Partial | Full (DICE engine) |

The "legacy" in `legacy_configs` specifically refers to the Buck v1-era `.buckconfig` format being retained for backward compatibility.

## Related Research

- `thoughts/shared/plans/kuro-bazel-subplans/02-bzlmod-phase-5b.md` - Cell system integration
- `thoughts/shared/plans/kuro-bazel-subplans/02-bzlmod-phase-5e.md` - Module extension execution
- `thoughts/shared/research/2026-01-29-dice-incremental-computation-engine.md` - DICE engine context
- `docs/about/benefits/compared_to_buck1.md` - Buck v1 vs Kuro comparison

## Open Questions

1. **Timeline for .buckconfig removal** - When will MODULE.bazel-only projects be fully supported?
2. **Migration path** - How will existing projects transition away from .buckconfig when the time comes?
3. **Feature parity** - Will all .buckconfig features be available through alternative mechanisms?

## Conclusion

**Should you use legacy_configs?** Yes, it's the correct system for:
- Cell definitions
- Project-level settings
- Tool configuration
- Anything currently in .buckconfig

**Should you use kuro_core/configuration?** Yes, it's the correct system for:
- Platform definitions
- Constraint-based configuration
- select() conditionals
- Build-time configuration choices

The "legacy" name is historical, not a deprecation warning. Use both systems for their intended purposes.
