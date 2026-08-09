# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-total-ruleclass-schema-source-ledger-design`
Milestone: M3 query / Stage 4 loading prerequisite
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: close the complete Bazel 9.2 attr-visible `RuleClass` schema and
loading-value source before designing the observable-candidate oracle.

## Background and boundary

The prior oracle design reached `REPLAN`: `attr()` asks each rule for its full
`RuleClass` definition, not only fields accepted or retained by V2. Inherited,
hidden, computed, late-bound, automatically populated, and overridden values
are observable loading-query inputs. Candidate position and equal-candidate
multiplicity remain unobservable. This packet is a pinned-source ledger only;
it does not design or generate a fixture, change representation or graph
breadth, or activate `attr`.

## Required oracle design

- Enumerate every attr-visible attribute for current Starlark normal,
  executable, test, and root string-build-setting rule classes and for native
  `filegroup`, `alias`, `config_setting`, `test_suite`, `constraint_setting`,
  `constraint_value`, `platform`, `toolchain_type`, and `toolchain`.
- For each attribute, record the exact query spelling, Bazel type, defining and
  overriding/removing rule class, configurability/order-independent flags,
  loading value source, intrinsic/package/computed/late-bound/automatic default,
  null suppression, and whole-value renderer. Include BOOLEAN `0`/`1`, integer,
  license, every list/dict orientation, hidden labels, macro `generator_*`
  provenance, and canonical main/external/`@bazel_tools` labels.
- Mechanically prove the final per-class attribute sets after inheritance,
  removals, and overrides. Separate fixed loading values from inputs V2 must
  capture and from genuinely configuration-resolved values that ordinary query
  does not observe.
- Partition the complete ledger into typed/default equivalence classes and
  class-specific exceptions. Specify one later positive/negative discriminator
  family for every class without selecting a fixture or duplicating equivalent
  empty/default rows.
- Map every ledger row to the current V2 source owner and loss point, including
  Starlark raw coercion, native call recording, macro provenance, test built-in
  defaults, universal name, query graph projection, and the native-toolchain
  graph rejection. Freeze no representation yet.

## Files

Edit only the Stage 4 and Stage 8 owner plans. Read pinned Bazel source and
current loading/query representations without edits. Add no fixture, oracle
record, Rust, Cargo/lockfile, BUILD, canonical-plan, manifest, routing-log, or
`@bazel_tools` content change during this design packet. Obtain independent
source-ledger review before returning to oracle design.

## Stops

Stop and `REPLAN` if the final schemas cannot be mechanically closed from
pinned source, if an ordinary-query value requires configured analysis or an
unbounded runtime registry, or if the work would add a fixture, Java helper or
artifact, JVM integration, bytecode, production Bazel delegation, query-time
filesystem/Starlark reads, a DICE key, regex redesign, or cquery/aquery breadth.
Bazel/Java source remains read-only oracle evidence only.
