# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-license-default-source-evidence`
Milestone: M3 query / Stage 4 loading-default evidence
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: determine exactly whether Bazel 9.2 can render package-derived
`licenses=[notice]` for the required rule families/layout, and freeze the
smallest evidence-backed correction before source-template work resumes.

## Background and boundary

The current semantic manifest has 165 rows with vector
`13/7/5/4/3/3/3/6/11/12/16/3/23/5/10/16/15/10` and SHA-256
`99b772e6a8a19540ad379792fe5db7c8683d50d6e8af282ba55766585242300d`.
The source-template retry stopped before plan edits: a minimal Bazel 9.2 package
with required `default_package_metadata`, `licenses(["notice"])`, and a
`filegroup` loads successfully, but
`attr("licenses","^\\[notice\\]$",//attr:x)` returns empty. The package-derived
license obligation therefore has no validated source construction. Explicit
target licenses produce the value but would change the frozen provenance.

## Required evidence

- Read pinned Bazel 9.2 license declaration/default/injection and query-renderer
  sources at commit `8220c6198837d5c13d53fea211cf3282aa12408a`; distinguish
  package metadata, package license declarations, explicit rule licenses, and
  any incompatible/disabled legacy path.
- In disposable roots, run a minimal matrix with and without
  `default_package_metadata`, package `licenses(["notice"])`, and explicit
  licenses across Starlark normal, native filegroup, and config_setting targets.
  Freeze exact `attr("licenses",...)` outputs and relevant diagnostics under
  ordinary RC discovery; remove every temp root afterward.
- Decide one bounded outcome: an exact package-derived source construction that
  preserves all six manifest atoms, or a manifest correction changing those
  operands to their actually observable explicit/default semantics. List every
  affected stable ID and the resulting checksum work; do not apply it here.
- Preserve the other accepted source-template obligations: pair-specific
  lane-1 supports, computed medium/small timeout, lane-13 `legacy_macro`
  provenance, and suite/manual tag closure.
- Obtain independent review of the source ranges, oracle matrix, and chosen
  outcome before scheduling any manifest or template edit.

## Boundary and review

Edit only Stage 4 and Stage 8. Temporary sources stay outside the checkout and
are removed. Add no fixture, payload, expected record, source template, Python,
Rust, Cargo/lockfile, BUILD, graph/DICE/regex state, configured analysis,
toolchain resolution, JVM, Java source/bytecode/helper, or production Bazel
delegation. Use ordinary RC discovery without reading/copying the private RC.

## Stops

Stop and `REPLAN` if pinned source and the disposable matrix disagree, the
behavior depends on configured analysis/toolchain resolution or unbounded
flags, the affected manifest family is not finite, or fixture/code/JVM/
production-delegation work is needed.
