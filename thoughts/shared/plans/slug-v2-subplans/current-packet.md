# Current Slug V2 Packet

Packet: `WP-5-m1-query-noshow-progress-compatibility-implementation`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: implementation worker
Evidence: accepted Bazel 9.2 20-row `module-local-override` oracle, accepted
six-path external Bzl package/query activation core retained uncommitted at
`+793/-21`, and independent latest-diff **ACCEPT core** reviews. The exact
three macro-query outputs pass without `--noshow_progress`; the frozen replay
stops in loading-query flag validation before query evaluation.

Implement only one command-local compatibility exception: loading `query`
accepts exact bare `--noshow_progress` as a no-op when the parsed flag value is
`None`. It must not change output, order, graph, query policy, Bzlmod policy, or
evaluation, and it remains in the existing `QueryRequest.flags` vector.
Existing argument splitting makes flags position-independent and preserves
flag order, so cover the bare flag before and after the expression, ordered
with a supported flag, and repeated.

The exact edit allowlist is `app/slug_commands_v2/src/query.rs` and
`app/slug_commands_v2/tests/commands.rs`. Total parser-prerequisite growth is
at most `+40/-0`. Reject empty or valued forms including
`--noshow_progress=`, `--noshow_progress=true`, and
`--noshow_progress=false` with `Unexpected value after boolean option`.
Preserve rejection of `--show_progress`, `--color`, `--keep_going`, and every
other unsupported loading-query flag. Do not change `common.rs`, flag
classification, a public API, CLI/query evaluation, any fixture/tool, or the
retained activation core.

The six existing dirty activation paths are validation-only and must remain
byte-for-byte unchanged: `app/slug_loading_v2/src/bzl_module.rs`,
`app/slug_loading_v2/src/package.rs`,
`app/slug_loading_v2/src/host_package_load_tests.rs`,
`app/slug_query_v2/src/loading_environment.rs`,
`app/slug_query_v2/tests/loading_query.rs`, and
`app/slug_cli_v2/tests/cli.rs`. The four accepted fixture paths are also
byte-frozen.

Run serially:

- `cargo test -p slug_commands_v2 --test commands`
- `cargo check -p slug_commands_v2`
- `cargo test -p slug_commands_v2 --target x86_64-pc-windows-gnu --no-run`
- `cargo build -p slug_cli_v2`

Clean stale `slugd`, then run the exact three frozen macro commands with bare
`--noshow_progress`. Run the unchanged 20-row `module-local-override` fixture
against the absolute rebuilt `target/debug/slug` on a fresh run root. Require
all three macro rows and their exact accepted stdout to pass and disappear
from the failure list; only the pre-existing unrelated `build_dep_target` row
may remain failed. Remove the run root and clean `slugd` afterward. Finish with
`cargo fmt --all -- --check`, `scripts/v2_archive_status.sh`, exact two-file
parser scope and `+40/-0` cap checks, retained-core/fixture byte checks, and
`git diff --check`. Do not run Bazel or a workspace-wide Cargo suite.

Stop with **REPLAN** rather than widening if the exact exception needs any
third file; accepts a valued, positive, or other ignored-compatible flag;
changes classification or evaluation; touches the retained activation core or
frozen fixture; the macro rows still fail; or exceeds `+40/-0`.
