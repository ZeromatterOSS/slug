# Current Slug V2 Work Packet

Packet: WP-7A-output-conflict-r2-authentic-fixture-application-design-r1

Status: SELECTED, READ-ONLY DESIGN after terminal acceptance of normal `run`
registry parity. First commit/push that accepted milestone. Complete output-
conflict R2 remains preserved, unapplied and all-or-nothing.

## Accepted prerequisite

`RunRequest` now retains ordered `--registry=` values from the shared parser.
Text after `--` remains program arguments; workspace override normalization
preserves registry flags. One-shot run borrows the retained URLs and daemon run
uses the registry-aware Bzlmod wire constructor. The existing wire field remains
the sole daemon owner and omitted older requests still default to no registry.

The accepted change is6 production/147 proof/153 gross additions. Frozen source
SHA-256 values are:

- Commands run `3af8a7d6978f3cd4c1844ecfe3097071f38c66fd64593e740b4df34d73df7144`
- CLI run `eeafc8aa5d1b0fd209e90e2c7669b4ada131e9194682f1d4b82a794c922ef52f`
- CLI integration `92994cc9a6f862484c4f7e93f719664e6e75c336a5d03dda070ee692d791bc34`
- Server tests `a5e7bcbe204761b39706ff15bb3365b94380bb34f325fe8831535c866caabbab`

Previously compiled parser2 and workspace1 proofs passed in0.00s. The frozen CLI
integration passed1/1 outside the managed sandbox in0.06s/RSS18504KiB and covers
both one-shot and daemon invalid-local-registry handoff before remote execution.
The fully qualified Server wire proof passed1/1 inside in0.00s/RSS12520KiB. The
absolute nightly Cargo combined check passed in2.73s/RSS253172KiB. The earlier
bare `/snap/bin/cargo` status46 occurred before Cargo startup and is not evidence
or a repeated semantic check. Independent terminal review returned `ACCEPT`.
No network, replay, credential, fixture or R2 action occurred.

## Design goal

Design one atomic application of the complete preserved R2 candidate onto this
accepted registry baseline while replacing its known fake fixture inputs with an
authentic registry-fed source closure. The known fake sites are one MODULE
`local_path_override` and three synthetic platforms MODULE/host BUILD/constraints
body writes in the new CLI proof. The accepted `run --registry=` handoff is the
only newly admitted fixture input route; it must not enter action,
configuration, output-conflict or execution identity.

The design must preserve every non-fixture R2 hunk and its all-or-nothing19-file
ownership, exact output-conflict semantics, concurrent root-set conflict/
restoration, loaded raw/message A/B/A and context-pointer cutoff, CLI pipe
draining, and selected exec-properties correction. It must define the authentic
registry directory/payload inventory, one retained registry value shared by
direct Core and every CLI subprocess, cleanup/lifecycle ownership, source hashes,
and bounded named proof sequence before any application is authorized.

## Authorized work

- Reverify, but do not apply, `/tmp/slug-conflict-r2.XZJWwv/candidate.patch` at
  SHA-256 `90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`
  and read its adjacent `validation.txt`.
- Read only the preserved patch hunks, the accepted registry handoff, and exact
  fixture/source owners needed to replace the four fake sites. Keep added source
  excerpts below2MiB and record every file read.
- Produce docs only: a reviewed atomic merge/fixture/proof contract with explicit
  production/proof/per-file caps. Update canonical live status, Stage5 and
  `~/PROGRESS.md`; keep the progress ledger below500 lines.
- Obtain independent architecture review before authorizing application.

## Prohibited work and stops

Do not apply, reverse, copy, reconstruct or partially stage any R2 hunk; do not
edit Rust, fixtures or the preserved patch. Do not compile, test, invoke Slug or
Bazel, start a daemon, replay network, acquire sources, inspect credentials, or
raise any limit. A missing authentic payload, patch overlap that cannot be mapped
without semantic change, need for a second fixture route, or inability to keep
all19 files atomic is `REPLAN`.

Every future test remains <=12s and <=15s absolute. Preserve old probe SHA-256
`8eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2`.
