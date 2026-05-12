---
oncalls: ['build_infra']
---

# Slug Validation Rules

**ALWAYS** run this after changing files in `slug/app/` or `fbcode/slug/app/`:

```bash
arc rust-check fbcode//slug/app/...
```
