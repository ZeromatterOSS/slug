# Current Slug V2 Work Packet

Packet: WP-7A-output-conflict-r2-authentic-fixture-application-design-r1

Status: `REPLAN`; no R2 application is selected. Normal `run` registry parity is
accepted and pushed at `47163df7b`. Complete output-conflict R2 remains preserved,
unapplied and all-or-nothing.

## Read-only audit result

The complete preserved candidate still has SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`,
154761 bytes and exactly19 diff owners. Its adjacent `validation.txt` is5853
bytes. The sole R2 CLI module contains the four known fake-source sites: one root
MODULE `local_path_override` and writes of synthetic platforms `MODULE.bazel`,
`host/BUILD.bazel`, and `host/constraints.bzl`. Root platform/toolchain/BUILD/defs,
conflict semantics and the other18 owners are unrelated and remain unchanged.

The real platforms1.0.0 inputs are present and exact: BCR MODULE
`f05feb42b48f1b3c225e4ccf351f367be0371411a803198ec34a389fb22aa580`, source
descriptor `f4ff1fd412e0246fd38c82328eb209130ead81d62dcd5a9e40910f867f733d96`,
and7879-byte archive
`3384eb1c30762704fbe38e440204e114154086c8fc8a8c2e3e28441028c019a8`.
The real module loads `//host:extension.bzl`, generates `host_platform`, and
depends on rules_license0.0.7. Existing exact rules_license metadata includes
0.0.7 MODULE `088fbeb0b6a419005b89cf93fe62d9517c0a2b8bb56af3244af65ecfe37e7d5d`,
selected1.0.0 MODULE
`a7fda60eefdf3d8c827262ba499957e4df06f659330bbe6cdbdb975b768bb65c`, source
`a52c89e54cc311196e478f8382df91c15f7a2bfdf4c6cd0e2675cc2ff0b56efb`, and35903-byte archive
`26d4021f6898e23b82ef953078389dd49ac2b5618ac564ade4ef87cced147b38`.

These bytes are evidence, not a hermetic repository fixture. The previous exact
authentic staging recipe needed183 registry metadata files, a test-owned file
mirror, real platforms/rules_shell archives and the rules_shell patch. Its direct
Core request used the same R2 root MODULE/BUILD/defs minus the fake override and
did not publish success; retained diagnostics reached generated canonical route
`bazel_tools+winsdk_configure+local_config_winsdk`. The trace did not establish
abseil demand, while the exact abseil archive is absent at both known paths.
Therefore platform/rules_license bytes alone do not prove a complete configured
CLI source closure.

No current repo fixture owns the exact authenticated metadata/payload set. The
checked-in registry fixtures use synthetic `local_path` sources; substituting one
would recreate the rejected fake-source policy. Reading absolute Bazel-cache
paths from a normal integration test would be nonportable and would make default
tests depend on external user state. Vendoring a text-encoded third-party bundle
would add new owners/licensing/repository weight and still cannot include the
unresolved absent payload without a separately authorized acquisition. Neither
choice is implicit in the accepted19-file R2 patch.

## Decision boundary

Do not apply, reverse, copy, reconstruct or partially stage R2. Do not edit Rust
or fixtures, compile, test, invoke Slug/Bazel, replay, acquire sources, inspect
credentials, or raise limits. Before R2 can resume, choose and separately review
one of these materially different prerequisites:

1. Authorize a portable repo-owned authentic fixture bundle, including its
   third-party source/licensing policy and a demand-gated acquisition rule for
   any exact missing payload; or
2. Authorize an environment-bound diagnostic fixture that verifies and reads the
   existing Bazel CAS but is never a default/portable test.

A fresh runtime diagnostic to resolve the generated-repository/abseil boundary is
also separate authorization; the consumed observer attempt is not reopened by
this packet. Any future test remains <=12s and <=15s absolute. Preserve old probe
SHA-256 `8eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2`.

## Files inspected

Read-only excerpts stayed below2MiB: preserved R2 patch/validation, current CLI
test/run owners, payload-demand driver/test, Bzlmod registry/source owners,
Loading generated-route owners, canonical/Stage5 plans, exact platforms and
rules_license CAS metadata/archives, and existing registry fixture inventory.
No filesystem object was created, copied, extracted or executed during this audit.
