# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-two-package-observable-candidate-oracle-generation`
Milestone: M3 query / Stage 4 loading evidence
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: generate and independently replay the corrected isolated Bazel 9.2
ordinary-`attr()` observable-candidate fixture.

## Background and boundary

The reviewed fixture remains absent from Rust projections and all Slug semantic
consumers. Its five files span the positive-default `//attr` package and the
baseline `@@ext+//leaf` package. Exactly 18 rows cover 165 globally unique pairs
with vector `13/7/5/4/3/3/3/6/11/12/16/3/23/5/10/16/15/10`; null/nonrule
controls are negative-only and no positive is reused. This is Bazel-only
loading evidence, never Slug activation or JVM/Java/Bazel delegation.

## Required generation

- Add only `MODULE.bazel`, `attr/defs.bzl`, `attr/BUILD.bazel`,
  `modules/ext/MODULE.bazel`, and `modules/ext/leaf/BUILD.bazel` in the new
  `query-attr-observable-candidates` payload workspace.
- Bind every accepted Stage 4 atom to a unique `lNN_aMMM_yes/_no` pair and an
  anchored whole-value regex. Exact stdout contains every positive once and no
  negative. Preserve lane 9 direct/macro provenance and lane 12's transition
  allowlist with real `//attr:base_setting` output.
- In the external leaf BUILD, preserve `filegroup(name="label")`, canonically
  load the public main schema with `@@//attr:defs.bzl`, and instantiate only
  lane 2's same-schema null-deprecation `l02_a007_no`. Its positive remains
  `//attr:l02_a007_yes`; all other pairs remain in their accepted packages.
- Independently freeze `@@ext+//leaf:label`; stop rather than weaken its regex.
  Add no sixth source, registry, lockfile, mutation, action, copied/generated
  tools content, configured analysis, or toolchain resolution.

## Files and growth

Change only the new fixture TOML/expected JSON, the canonical payload, Python
derived global/body/SHA plus projection integrity, and Rust global SHA plus
275-to-285 count. Do not add a Rust projection, CLI/server case, runner/BUILD/
Cargo/plan/production change, or semantic consumer. Existing fourteen
projection hashes remain byte-exact.

The payload grows by five files/five directories/ten entries to `(285, 117)`;
generate body bytes and hashes. From `51540963`, cap the expanded corpus at +7
regular files, +5 directories, zero links, 18 rows, and 2,400 lines, with
absolute ceilings 1,368 files/24 links/44,920 lines/882 rows.

## Validation and stops

Use ordinary Bazel RC discovery without inspecting/printing/copying the private
home RC. Run one update and one clean distinct-root replay of all 18 rows, then
Python payload inventory/projection/metadata, Rust global conformance without a
new projection, protected 29-row CLI and two generated-kind CLI/server cases,
lane 9/lane 12 positives, and `git diff --check`, serializing Cargo. Obtain
independent evidence review.

Stop and `REPLAN` on a sixth source, changed 165-atom contract, incomplete or
reused pair, restricted canonical-main load, mapping/registry/lockfile need,
nonexact external token, Rust projection/consumer, configured analysis, cap or
protected drift, production Rust/graph/DICE/regex, JVM/Java, or Bazel semantic
delegation.
