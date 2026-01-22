---
oncalls: ['build_infra']
---

# Kuro Validation Rules

**ALWAYS** run this after changing files in `kuro/app/` or `fbcode/kuro/app/`:

```bash
arc rust-check fbcode//kuro/app/...
```
