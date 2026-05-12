# Slug Mascot Icon Set — Design Specification

## Purpose
Create an ultra-minimal mascot icon set for the **Slug build tool** that remains highly legible at **32×32 px** (favicon scale) while also working at **64×64** and **128×128**.

The mascot is a **leaf sheep sea slug** (*Costasiella kuroshimae*) chewing on **three green cubes** arranged exactly like the **Bazel logo cube stack** (3 joined cubes in a V shape).  
**Cubes must be unbranded**: no letters, no “B”, no text.

---

## Deliverables
### Primary outputs
1. **4 mascot logo variants** (vector):
   - Front-facing stamp icon (symmetric, favicon-ready)
   - 3/4 angled icon (slight rotation, still readable at 32px)
   - Super-minimal favicon mode (head + cubes + 3 leaves)
   - Monochrome outline version (single-color line icon)

2. **Export sizes** (for each variant):
   - 128×128 PNG
   - 64×64 PNG
   - 32×32 PNG

3. Source vector files:
   - SVG (preferred)
   - Optional: AI/Figma source

---

## Style Requirements
- **Extremely simple flat vector shapes**
- **Bold clean outline**, consistent stroke weight
- Minimal interior detail
- No gradients
- No texture
- No background (transparent background for final exports)
- Few shapes total (icon-friendly)
- Cute, modern open-source energy
- Prioritize silhouette and readability over detail

---

## Mascot Requirements (must read at 32px)
### Leaf sheep (Costasiella kuroshimae)
- White rounded body “blob”
- Two black oval eyes (simple, friendly)
- Two upright rhinophores (“bunny ears”)
  - IMPORTANT: **make ears slightly thinner** than typical cartoon bunny ears
- Leaf fan on back:
  - Only **3–5 leaves max**
  - Simple leaf shapes, no veins

### Chewing action
- Mouth must be **visibly touching** the cubes
- Include a **tiny bite notch** on the top cube edge (simple cutout)
- Optional: **1–2 tiny square crumbs**, only if it stays clean at 32px

---

## Cube Stack Requirements (Bazel arrangement)
- Exactly **three cubes** in the **Bazel V arrangement**
  - Two cubes on the bottom, one cube centered on top
  - Joined / stacked so they read as one cluster
- Plain green cubes
- Optional: very simple 2-tone faces (subtle light/dark) to suggest volume
- No text, no logos, no markings
- Strong silhouette

---

## Composition Rules
- Centered, compact, minimal wasted space
- Balanced icon footprint (avoid wide empty margins)
- The mascot should “hug” the cube cluster visually
- Keep outlines clean and uniform
- Avoid extra decorations:
  - No gears
  - No circuit patterns
  - No complex shading or highlights

---

## Color Guidance
### Full-color variants
- Body: white fill + black outline
- Eyes: black fill
- Leaves: green fill + black outline
- Cubes: green fill
  - Optional: 2-tone cube faces (same hue, different value)

### Monochrome variant
- Single color line icon (no fills required)
- Must remain readable at 32×32

---

## Quality Checklist (32×32 validation)
At 32×32 px, confirm:
- Eyes are clearly visible as two ovals
- Ears read as two upright antennae (not blobs)
- Leaf fan reads as 3–5 distinct leaves
- Cube stack reads as 3 cubes in V stack
- Mouth contact with cubes is obvious
- Bite notch is visible as a small cutout

---

## Reference Output
See: `assets/mascot/slug-mascot-variants.png`

