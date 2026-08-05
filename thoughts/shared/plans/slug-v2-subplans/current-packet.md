# Current Slug V2 Packet

Packet: `WP-5-m1-direct-local-external-build-source-target-evidence`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: evidence-only retained Bazel 9.2 source-target lifecycle discriminator
Evidence: accepted external build source-target activation design; checked-in
`module-root-dev-dependency-visibility` and `module-local-override` present
source successes at Bazel 9.2 commit `8220c619…`.

Do not edit Rust, tests, fixtures, generated oracle records, or harness code.
Run one isolated retained-server Bazel 9.2 probe for a direct
`local_path_override` repository whose BUILD declares
`exports_files(["target.txt"])`. Record commands, exit status, normalized
diagnostic shape, stdout, and relevant stderr for these serial states:

1. present source;
2. byte edit;
3. delete;
4. recreate with different bytes; and
5. a wrong-kind source path if Bazel distinguishes it from absence.

Use ordinary RC discovery without reading or copying `~/.bazelrc`. Pin Bazel
9.2.0/`8220c619…`, use an isolated temporary workspace/output base, and clean
its retained server afterward. Do not invoke a MODULE include cycle.

The evidence must decide whether source byte edits are semantically successful
no-ops, the exact missing/wrong-kind exit and diagnostic, whether recreation
succeeds in the same server, and whether stdout/manifest remain empty. Compare
the present result with the two checked-in accepted rows; do not regenerate
them.

At acceptance, append the exact evidence and provenance to the Stage 5 owner
plan, advance the manifest/canonical status to
`WP-5-m1-direct-local-external-build-source-target-activation-implementation`,
and preserve its accepted five-file allowlist and 280/850/1130 formatted net
caps. If the evidence requires configured analysis, actions, a new DICE key,
or a broader source/output contract, record `REPLAN` instead.

Stops: no implementation or repository asset change; no registry, contextual
mapping, dependency traversal, filegroup/rule activation, analysis, action,
execution, REAPI, command breadth, fixture, or oracle growth; no credential
inspection; and no cycle probe.
