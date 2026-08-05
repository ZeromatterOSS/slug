# Current Slug V2 Packet

Packet: `WP-6-m2-pure-native-value-default-and-rendering-kernel-retry-7`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: implement the closed context-free native value/default/cache kernel for
the accepted 287 pure descriptors plus the default-materializer-only Runs seed.

Predecessors are authoritative: the committed 341 registry, accepted
`287 + 8 + 5 + 41` cohort, renderer and forced-identity evidence, retry-7
family contracts, and 287-row attachment ledger. Reuse them; add no source or
oracle evidence.

Retry 2 was discarded cleanly after its permitted correction exposed further
material misses. Before writing breadth, freeze these implementation seams in
types and focused tests: occurrence conversion represents Void `E("null")` as
absence rather than a sentinel value; timeout implements the accepted decimal
`.limit(6)` split/rejection discriminators; Dotted validates the full accepted
component/descriptive grammar, signed-i32 capture bounds, underscore/early-stop
behavior, and full-original-input identity; ordered maps use exactly
`Arc<[(NativeValue, NativeValue)]>`; Bool annotation `null` materializes
`false`, Tri annotation `null` materializes `AUTO`, and reference/repeat
special-null remains distinct; Runs uses a positive typed private seed rather
than pre-rendered text. These are frozen predecessor facts, not new evidence.

Retry 3 was also discarded cleanly: its passing tests omitted frozen empty
comma/set, Env, shard, Fission, privacy, and positive-Runs discriminators, and
production violated each omitted route. Retry 4 therefore has a mandatory
serial gate. First add a test-only matrix, mechanically checked against all 17
family rows, 16 enum rows, 287 attachments, exact 8/5/41 exclusions, and every
retry-2/3 discriminator; obtain independent source review of that matrix before
adding production code. Then implement only the reviewed tests. Tests may use
parent-module access to child-private items, but no new kernel symbol may be
publicly re-exported.

Retry 4 was discarded before production because its Phase-1 test draft marked
accepted-but-deferred Runs `U("+2")` as rejected and substituted checklist
strings/undefined helper shims for direct discriminators. Retry 5 reached an
independently accepted direct matrix, but Phase 2 exposed a material default
association error: the test used empty-default
`PlatformOptions#extra_execution_platforms` while asserting the nonempty
`[-O0, -DDEBUG=1]` default owned by
`ObjcCommandLineOptions#experimental_objc_fastbuild_options`. All retry-5 Rust
was discarded; no production was retained.

Retry 6 keeps root-owned mechanical transcription and the serial review gate.
Before behavioral acceptance, add one active literal binding row for every one
of the 287 attachments, keyed by exact attachment ordinal, FQCN, and canonical
name. Each row states the exact registry field type/raw default/converter/repeat
bit, accepted family/route, and expected materialized-default outcome/cache;
the test first resolves that descriptor by FQCN/name and asserts the complete
registry tuple before materializing it. No constructed descriptor,
family/converter-only lookup, or unbound family expected value may substitute.
Review duplicate-family collision rows explicitly. In particular,
`extra_execution_platforms` owns `D("")`/empty-list cache bytes, while
`experimental_objc_fastbuild_options` owns
`D("-O0,-DDEBUG=1")`/`[-O0, -DDEBUG=1]`. Keep the Runs default-only seed outside
the 287 table and the 8/5/41 exclusions assertion-only. The independent
reviewer must compare every binding with the accepted attachment, registry,
and family ledgers before Phase 2. The matrix must remain direct enough that
Phase 2 only supplies private implementations and removes the temporary
compile gate—no invented helper semantics or production before acceptance.

Retry 6 was discarded after terminal production review. Its independently
accepted 1,155-line matrix and 677-line private implementation passed 13 tests,
crate/check/format/GNU-Windows/archive gates, but the latest-diff audit exposed
five new material contradictions: list-valued occurrences ignored
`allow_multiple`; Dotted descriptions were lowercase-only; timeout used one
literal rejection instead of the accepted `.limit(6)` grammar and treated
malformed values as fallback; total-`u64` nanoseconds did not preserve Java
signed-long duration range; and Fission case-folded the exact `yes`/`no`
specials. The whole seven-file diff was discarded.

Retry 7 restores the same root-owned test-only Phase 1 and Terra review-only
gate, reusing every retry-6 binding and behavioral discriminator. Before
acceptance, add direct tests that nonrepeat AllowComma/StringSet/Fission/
EmptyList return scalar `NativeValue::List` while only repeat comma occurrences
return `NativeOccurrence::List`; uppercase `A` and `A_internal` are valid
Dotted descriptive early stops; timeout implements the general `.limit(6)`
split/arity/decimal-validation path with fallback only for successfully parsed
nonpositive entries; duration uses Java signed-long input bounds and a
seconds-plus-nanos representation that admits large valid values without total
nanosecond overflow; and only exact lowercase Fission `yes`/`no` are special,
so `YES`/`No` reject through compilation-mode conversion. Independent review
must accept those tests before any production edit.

Implement one closed `NativeValue` algebra, source-default materializer,
per-occurrence converter, and exact Java cache projection inside
`slug_configuration_v2`. Repeat merging remains command-owned: the kernel must
return the accepted scalar-versus-list occurrence shape but expose no argv,
priority, or accumulation API. Routing must be a bounded match over the static
descriptor/type/converter metadata, not a runtime registry map, generated table,
global interner, cache, hash, or hidden mutable state.

Representation is frozen by the accepted Stage 6 design and Buck2 utility
boundary:

- dynamic valid-Unicode text uses `CompactString`;
- immutable lists and ordered maps use `Arc<[NativeValue]>` and
  `Arc<[(NativeValue, NativeValue)]>`;
- retained value/container types derive `Allocative`;
- use `Dupe` only for an aggregate newtype whose clone is pointer-cheap; and
- keep descriptor names borrowed `&'static str`; add no interner or unordered
  value map. Java UTF-16 comparison is an explicit helper, never Rust byte/code
  point ordering.

Implement exactly the accepted default and per-occurrence routes: special null
versus explicit `null`; `D/E/N/Ø`; Bool/Int/Text/Tri/Void; all 16 enum routes and overrides;
comma-list flattening, UTF-16 set sorting/dedup, Entry, Env records, Dotted
full-input identity, timeout/duration fallback, structural forced sharding,
Fission `no`/`yes`/comma order, Platform ASCII lowering, EmptyList, outer
`NULL`/`EMPTY`/escaping, and the private `RunsPerTestSeed D("1")` exact
cache text. `U("+2")` and every general runs/regex occurrence remain
unsupported.

Tests must independently cover:

- all 287 attachments and the exact excluded 8/5/41 partition;
- every annotation-default family, special-null versus explicit-null, and
  repeat empty default plus scalar-versus-list per-occurrence shapes needed by
  the frozen later `A` routes;
- every enum member/alias/rendering override and all retry-7 discriminators;
- exact list/entry/env/map/duration/Dotted/shard/Fission/Platform/cache bytes;
- Java UTF-16 reverse U+E000/U+10000 ordering and duplicate removal;
- structural equality/order, cheap aggregate clone, and retained allocation
  shape; and
- explicit refusal of Java-regex, Host, repository/loading, command/argv,
  normalization, and checksum/wire; plus a type-level valid-Unicode boundary
  with no lossy surrogate replacement API.

Allowlist:

- `app/slug_configuration_v2/Cargo.toml`
- `app/slug_configuration_v2/src/native/mod.rs`
- `app/slug_configuration_v2/src/native/cache_grammar.rs`
- `app/slug_configuration_v2/src/native/value.rs`
- `app/slug_configuration_v2/src/native/defaults.rs`
- `app/slug_configuration_v2/src/native/convert.rs`
- `app/slug_configuration_v2/src/native/tests.rs`
- terminal scheduling only:
  `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`,
  `thoughts/shared/plans/slug-v2-subplans/current-packet.md`, and
  `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- routing only on `REPLAN`:
  `.codex/skills/slug-agent-orchestration/references/routing-log.md` and
  `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`.

Caps: 1,550 formatted production, 1,250 test, and 2,800 total net lines across
the seven crate files. Add only existing workspace `compact_str`,
`allocative`, and `dupe` dependencies required by the frozen
representation. No registry edit, root/workspace dependency edit, lockfile
change, fixture, Java/JVM, generated source, oracle, or downstream edit.

Validation: serial `cargo test -p slug_configuration_v2`, crate check,
formatting, applicable GNU-Windows no-run check, `scripts/v2_archive_status.sh`,
`git diff --check`, scope/cap checks, then independent latest-diff review.

Stop and `REPLAN` on a route/count/byte disagreement, Java-regex need, lone
surrogate or lossy conversion, Host/repository/loading context, command-layer
behavior including repeat merging, whole P/C/T normalization, runtime registry/interner/hash, new
identity issue, dependency/lockfile expansion, cap breach, a production edit
before test-matrix review, or any new material correction beyond the frozen
retry-2/3/4/5/6 set above. Defer normalization, checksum/wire integration, DICE, downstream
activation, and user-approved later configured-target dependency cycles.
