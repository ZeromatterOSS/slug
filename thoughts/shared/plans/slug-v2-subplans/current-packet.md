# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-full-rbe-driver-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one offline-reviewed manifest-aware managed-RBE driver.

## Goal and required evidence

Add only `tools/v2_oracle_lib/buildbuddy_rbe.py` (300 lines),
`tools/v2_oracle/buildbuddy_rbe_gate.py` (20), and
`tests/v2_oracle/test_buildbuddy_rbe_gate.py` (360), at most 680 total lines.
Do not alter the manifest, full-cache gate, or accepted one-label files.

## Stops and budget

The command preserves the accepted one-label RBE order through publish-all-
actions, substitutes `test`, then appends once the three fixed test flags,
action/test env keys both `SLUG_BUILDBUDDY_RBE_GATE_NONCE`, BEP/execution paths,
and the fixed 44 labels. Keep cache reads/uploads disabled, top-level downloads,
managed bounds/no fallback, and forbid opposite/duplicate/profile reconstruction.

Emit closed mode `buildbuddy-rbe-only` and success `PROVED_RBE`, bound to the
manifest/version/counts, Bazel/host/managed platform, RC hash, and clean Git head.
Require process/output one; singleton successful BuildFinished/production; 43
singleton successful completions/PASSED summaries with run one/cache zero; zero
persistent hits; and every SpawnExec valid-digest, remotable true, cache-hit
false, empty status, exit zero, and runner remote. All other runners/errors are
zero. Use a distinct `slug-buildbuddy-full-rbe-*` private namespace, component-
anchored manifest, no-follow/replacement-aware evidence, executable output,
identity shutdown/removal, and pre/post clean suppression. Test exact argv/schema/
classes/counters, all runners/fields, BEP defaults/duplicates, manifest/artifact/
root replacements, cleanup, CLI privacy, and full-cache/one-label regressions.
Run offline tests, `py_compile`, scope/caps/diff, and independent review only.
