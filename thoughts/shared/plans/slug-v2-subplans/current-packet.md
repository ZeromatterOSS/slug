# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-execution-replacement-repair`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: a replacement-aware, descriptor-anchored cache gate.

## Goal and required design

Change only `tools/v2_oracle_lib/buildbuddy_build_cache.py` (150 net lines) and
`tests/v2_oracle/test_buildbuddy_build_cache_gate.py` (220 net): 370 maximum.
Do not change the CLI, closed schema, command vector, classification set, BEP
identity rule, configuration, targets, manifests, or docs.

Keep BEP's precreated exact-inode read. For execution only, open the final
direct child through the retained phase FD with no-follow/nonblocking read;
accept retained or replaced inode only when regular, mode 0600, and single-
link. Read through that descriptor, verify its dirent identity before/after,
then feed the existing strict JSON-sequence/spawn parser. Never expose raw
content or exact metadata.

Precreate/open each phase output directory with retained no-follow identity.
Require root/phase/output anchoring before and after parse, output inspection,
and shutdown. Cleanup removes both the original root inode and anything the
child placed at the reserved random root entry without following links.

Pinned Bazel 9.2 commit `8220c619…` requires this execution-only replacement:
`ExpandedSpawnLogContext` lines 106-130/291-316 delete the preexisting JSON
output then create the converted final file; `ExecutionOptions` lines 420-436
define its executed-spawn records. Empty remains source-valid and must reach
the existing parser's `EVIDENCE_INCOMPLETE`, not an ownership rejection.

## Stops and budget

Offline tests cover retained/replaced/empty execution, symlink/hardlink/mode/
directory rejection, BEP replacement rejection, root/phase/output swaps before
and during reads/shutdown, exact unchanged argv/schema/classes, cleanup, and
raw suppression. Run focused unittest, compilation, scope/caps/diff checks,
and independent privacy/lifecycle review only. No Bazel, home RC, network,
remote service, or raw live artifact. One focused correction is allowed.

Any need for a second material repair, schema/vector/class change, BEP
relaxation, links/mode relaxation, path-following, or raw data is `REPLAN`.
A separate packet owns one gate invocation. RBE/43-test/Stage 10 remain open.
