# BXL vs AXL Comparison

## Overview

This document compares Buck2's BXL (Build Extension Language) with Aspect's AXL (Aspect Extension Language) for build graph introspection and developer tooling.

**Goal**: Understand how AXL approaches similar problems to BXL and identify features/patterns that could improve Slug's developer experience.

## Resources

- **AXL**: https://www.aspect.build/axl
- **BXL Documentation**: https://buck2.build/docs/bxl/
- **Slug BXL**: `app/slug_bxl/` in Slug codebase

## Status

- [ ] Research AXL capabilities and API
- [ ] Document BXL current capabilities in Slug
- [ ] Compare feature sets
- [ ] Identify gaps and opportunities
- [ ] Recommend improvements for Slug's BXL

---

## AXL Overview

*TODO: Research and document*

### Key Features

-

### Use Cases

-

### API Surface

-

---

## BXL Overview (Slug/Buck2)

### Current Capabilities

BXL allows self-introspection of the build graph for:
- Generating compilation databases (compile_commands.json)
- IDE integration
- Custom analysis and reporting
- Automation tooling

### Key Features

- Starlark-based scripting
- Access to build graph nodes and actions
- Can run actions and inspect results
- Integrated with DICE for incrementality

### API Surface

- `bxl()` function to define BXL scripts
- `ctx.analysis()` for target analysis
- `ctx.audit()` for build auditing
- Action inspection and execution

---

## Feature Comparison

| Feature | BXL (Slug) | AXL (Aspect) | Notes |
|---------|------------|--------------|-------|
| Language | Starlark | *TBD* | |
| Build graph access | Yes | *TBD* | |
| Action execution | Yes | *TBD* | |
| Incrementality | DICE-based | *TBD* | |
| IDE integration | compile_commands.json | *TBD* | |
| Custom queries | Yes | *TBD* | |

---

## Recommendations

*TODO: After research*

---

## References

- Aspect Build AXL announcement/docs
- Buck2 BXL documentation
- Bazel Aspects (related but different concept)
