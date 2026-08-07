# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-prime-stderr-sanitizer-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: a fail-closed token-safe normal-RC prime diagnostic.

## Goal and required design

Add only `tools/v2_oracle_lib/buildbuddy_prime_diagnostic.py` (240 lines),
`tools/v2_oracle/buildbuddy_prime_diagnostic.py` (40), and
`tests/v2_oracle/test_buildbuddy_prime_diagnostic.py` (320): 600 maximum.

The stdlib diagnostic reuses `buildbuddy_cache.command("prime", ...)` with only
`//app/slug_cli_v2:slug`, ordinary RC discovery, one mode-0700 root, mode-0600
terminal/BEP/execution files, RC-disabled shutdown, exact-root cleanup, and
clean Git/no-`slugd`. It never inspects/expands home RC; stdout, BEP, and
execution data remain unread.

Only on Bazel exit two may a pure sanitizer read at most 65,536 strict-UTF-8
stderr bytes. Pinned Bazel 9.2 `8220c619…` renders invalid argument, ` :: `, and
`Unrecognized option: <argument>`. The bare frozen-vector intersection is
exactly `--noremote_local_fallback`, `--build_event_publish_all_actions`,
`--noremote_accept_cached`, `--remote_upload_local_results`, and
`--noremote_cache_async`.

The entire payload, except fixed terminal whitespace, must equal
`ERROR: <flag> :: Unrecognized option: <same-flag>`. Only those five map to a
fixed `CHECKED_IN_OPTION_*` ID. Every other prime flag or any extra byte/line,
zero/multiple/unknown flag, malformed/oversized input, path, URL, header,
bearer/token/nonce text, value, or unexpected shape emits only
`NORMAL_RC_PRIME_UNEXPLAINED` or `SANITIZER_REJECTED`. No source text/hash,
exception, path, endpoint, option value, or raw enum enters closed JSON, CLI
stdout/stderr, persistence, or Git. Exit two plus one ID is
`NORMAL_RC_PRIME_DIAGNOSED`; all other outcomes are opaque; cleanup fails closed.

## Stops and budget

Synthetic/mocked tests cover all five mappings, every other prime flag as
rejection, near misses/extra data, malicious secret/path/value inputs,
oversize/non-UTF-8, frozen argv, modes/outcomes, shutdown/deletion, empty CLI
stderr, and closed schema. Run only focused offline tests, Python compilation,
caps/diff checks, and independent privacy review. Do not run Bazel, use normal/
home RC, contact BuildBuddy, change existing cache code/config/targets, or make
a live attempt; a later reviewed packet owns one invocation.
