# Current Slug V2 Packet

Packet: `WP-7-m6-filewrite-reapi-action-handoff-design`
Milestone: M6 design
Owner: `slug-v2-subplans/07-reapi-native-execution.md`
Result: freeze one bounded FileWrite Action IR-to-REAPI handoff, or record
`REPLAN`.

## Scope

Design only. Audit the accepted retained FileWrite action declaration,
configured owner/platform/constraint semantic view, Slug action identity, and
the existing REAPI protobuf/harness surface. Determine the smallest Rust-native
boundary that can produce one exact REAPI `Command`, input-root `Directory`,
and `Action` identity without a second executor-only action description.

Freeze where FileWrite contents, executable state, declared output, working
directory, environment, platform properties, timeout, and input-root emptiness
live. Keep semantic configuration identity, Slug display/action tokens, Bazel
ActionKey/checksum bytes, and REAPI/CAS digests separate. Any new retained field
must participate structurally in DICE equality and invalidation; missing inputs
fail closed.

Classify the bounded result as exact, Slug-native, or unsupported/deferred.
Exactness may cover FileWrite contents and REAPI protobuf/digest semantics for
Slug's actual graph. Slug action/configuration display bytes remain
Slug-native. Bazel ActionKey, exact Bazel configuration/output bytes, Spawn/
Run actions, paramfiles, tree artifacts, nonempty input roots, execution,
upload, cache lookup, and materialization remain deferred unless the design
proves a smaller prerequisite is inseparable.

## Evidence and review

Reuse the accepted FileWrite aquery fixture and retained NativeLink/REAPI
regressions. Inspect Bazel 9.2 action/remote source and the existing Rust action
and REAPI types only where they discriminate ownership or identity. Inspect the
Stage 9 extraction ledger and archived V1 sources only if the design selects a
concrete reuse candidate; do not import code in this packet.

Read `docs/developers/dice.md` before proposing any DICE key, retained-field
ownership change, or lock boundary. Read the Buck2 utility-reuse skill before
choosing any new retained representation, hashing, compact collection/string,
or memory-accounting implementation.

Require one independent Sol design review. The review must verify one semantic
action object feeds both aquery and execution, every admitted identity input is
structural, REAPI digests use serialized protobuf/CAS bytes, and no backend,
JVM, or Bazel semantic delegation enters Slug.

## Allowlist and caps

Edit only:

- `thoughts/shared/plans/slug-v2-subplans/07-reapi-native-execution.md`;
- `thoughts/shared/plans/slug-v2-subplans/09-v1-extraction-ledger.md` only if
  an extraction decision changes;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- the canonical V2 plan; and
- the orchestration routing log only for `REPLAN` or a reusable route change.

No production, test, fixture, Cargo, protocol, or generated-file edits. Cap
bookkeeping growth at 220 lines. One focused design correction is allowed; a
second material correction is `REPLAN`.

## Validation and stops

Check source anchors, existing representation ownership, exact allowlist,
archive boundary, credentials, and `git diff --check`. Stop if a bounded
single-object handoff requires action reconstruction, executor-only semantic
state, configuration-opaque identity, direct filesystem reads outside DICE,
Java/JVM artifacts, backend-specific semantics, or an unreviewed public wire.

At `ACCEPT`, schedule only the bounded implementation/oracle packet named by
the reviewed design. At `REPLAN`, record the concrete missing prerequisite
and schedule only its design packet.
