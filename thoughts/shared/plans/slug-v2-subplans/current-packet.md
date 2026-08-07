# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-complete-command-diagnostic-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: a reviewed complete Bazel 9.2 exit-2 structured diagnostic.

## Goal and required design

Add the source-ordered table of all 131 Bazel 9.2 exit-2 category/code pairs,
whose exact canonical serialization is frozen in the owner plan with SHA-256
`cbc5777c…6d57`. Retain the existing 33 semantic classes and assign each of
the other 98 pairs a unique fixed `B92_EXIT2_CLASS_NNN` source ordinal. Add
fixed missing/malformed/unsupported-general/unrecognized structural classes
using the exhaustive 64-key oneof set. Never output any raw category, code,
enum, message, option, path, credential, nonce, header, or stderr data.

## Stops and budget

Change only `tools/v2_oracle_lib/buildbuddy_cache.py` (150 changed lines) and
`tests/v2_oracle/test_buildbuddy_cache_gate.py` (180), at most 330 total. Tests
must pin all 131 pairs, exact canonical hash, 98 unique opaque IDs, current 33
classes, structural cases, raw/private suppression, no stderr read, and prior
behavior. Run only offline Python tests/compilation and diff/cap checks; obtain
independent privacy/schema review. Do not run Bazel, discover/inspect home RC,
read raw artifacts, contact BuildBuddy, invoke RBE, change config/CLI/CI/BUILD/
MODULE/locks, or make a live attempt.
