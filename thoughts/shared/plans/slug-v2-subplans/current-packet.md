# Current Slug V2 Packet

Packet: `WP-2A-m1-cquery-parallel-workspace-isolation-correction-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Rust base: `941db0d0`
Frozen design: `895996d5`
Scheduling base: `7280b9c2`
Result: authorize exactly one stable-parent test-harness exception before
resuming the retained observed cquery implementation candidate.

## Authority and caps

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`; and
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Documentation caps against `7280b9c2` are 40 canonical, 140 manifest, 100
Stage 2, 30 routing and 310 aggregate net lines. The unaccepted Rust candidate
remains in place but is not writable under this packet.

## Frozen correction

Record the implementation `REPLAN`: the frozen relocated range remains
byte-identical, the default-parallel 16-test cquery batch fails only in
`cquery_restores_structural_configuration_and_display_projection`, and that
test passes in isolation. Concurrent `tempfile::tempdir()` siblings mutate the
shared observed `/tmp` ancestor after runtime construction, producing a real
Host observation replay rather than a production terminal/event defect.

Freeze exactly one exception in that relocated test. Replace only its first
workspace-construction line with:

```rust
let stable_parent = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../target/slug-cquery-restores-structural-configuration");
fs::create_dir_all(&stable_parent).unwrap();
let workspace = tempfile::tempdir_in(stable_parent).unwrap();
```

This test-exclusive parent exists before runtime construction and no other
cquery test writes beneath it. Preserve every assertion and every other
relocated body byte-identically. Existing parent-module `Path` and `fs`
imports remain sufficient through `use super::*;`; add no import or fixture.

Count the replacement as +3 test semantic lines. The retained candidate is
currently 11,513 physical DICE lines and 835 physical child-test lines; the
replacement yields 838 child-test and 12,351 combined lines. Preserve the
existing 160 production, 300 test, 460 aggregate semantic caps and
12,435/1,200/13,635 physical caps against `941db0d0`; no increase is
authorized. The existing cfg(test) activation audit may remain only if its
new lines are charged to tests and the nonrelocated root-count test body stays
unchanged.

The immediate successor resumes the same two-file implementation candidate and
the complete production/terminal/event/retention contract in `895996d5`.
Require the isolated lifecycle test and the 16-test default-parallel cquery
batch, followed by the frozen focused and broader validation.

## Compatibility

Production cquery output/error/event behavior remains exact. The stable
test-workspace parent and observed-family/typed-outer/selected-epoch mechanics
are Slug-native. All unsupported/deferred boundaries from `895996d5` remain.

## STOP / REPLAN

STOP on Rust, Cargo/BUILD, fixtures/oracles/generated files, any broader test
body exception, changed production design, assertion weakening, another file,
cap increase/excess or premature M1 close. `REPLAN` if the stable-parent fix
does not pass both isolated and default-parallel validation or if another
relocated body must change. Acceptance schedules only the same implementation
retry.

## Immediate predecessor

`7280b9c2` scheduled the complete two-file design frozen in `895996d5`.
Implementation exposed only the parallel `/tmp` ancestor harness artifact;
independent review required this formal correction before changing the frozen
relocated lifecycle body.
