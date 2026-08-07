# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-prime-semantic-branch-discriminator-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one offline-reviewed fixed semantic branch discriminator.

## Goal and required evidence

Edit only these existing files:

- `tools/v2_oracle_lib/buildbuddy_build_cache_prime_stage_probe.py` (145 final)
- `tools/v2_oracle_lib/buildbuddy_build_cache_prime_output_semantics_probe.py`
  (140 final)
- `tests/v2_oracle/test_buildbuddy_build_cache_prime_output_semantics_probe.py`
  (250 final)

Stay within 535 final and +125 net lines. Add one ordered `_semantic_stage()`
helper to the shared prime-stage module, make `_ready()` delegate to it, and
have the output-semantics probe call the same helper. Preserve every earlier
process, anchor, output, descriptor, parser, shutdown, cleanup, and privacy
stage in its existing order.

Expand the current predicate in this exact first-failure order:

1. `PRIME_OUTCOME_REJECTED`
2. `PRIME_PROCESS_COUNTER_REJECTED`
3. `PRIME_BUILD_FINISHED_COUNTER_REJECTED`
4. `PRIME_TARGET_COUNTER_REJECTED`
5. `PRIME_OUTPUT_COUNTER_REJECTED`
6. `PRIME_PERSISTENT_CACHE_REJECTED`
7. `PRIME_ELIGIBLE_SET_REJECTED`
8. `PRIME_CACHE_EXPECTATION_REJECTED`
9. `PRIME_STATUS_EXPECTATION_REJECTED`
10. `PRIME_EXIT_EXPECTATION_REJECTED`
11. `PRIME_REMOTE_HIT_CLASS_REJECTED`
12. `PRIME_OTHER_RUNNER_CLASS_REJECTED`
13. `PRIME_RUNNER_PARTITION_REJECTED`
14. `PRIME_READY`

Emit only the fixed stage. Do not expose values, counts, paths, hashes, labels,
raw records, or runner spellings.

## Stops and budget

Test every semantic branch and ready result through the shared helper, exact
`_ready()` equivalence, earliest simultaneous failure, parser-driven reachable
branches, and mocked defensive branches. Preserve output-first short-circuit,
exact-one executable, private read attacks, anchors, shutdown/cleanup, schema,
privacy, exact argv, and fingerprint coverage. Remove the coarse semantic stage
from this probe and pass unchanged prime-stage/BEP/execution/lifecycle-guard/gate
regressions, `py_compile`, final/net caps, scope, and `git diff --check`.
Independent review is mandatory because this changes a fixed public stage enum.
No Bazel, network, home RC, live artifact, service, CLI, guard, parser, gate,
config, fixture, or unrelated edit. A later guarded live packet is separately
scheduled only after acceptance.
