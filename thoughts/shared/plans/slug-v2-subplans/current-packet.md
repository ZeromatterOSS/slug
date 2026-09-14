# Current Slug V2 Work Packet

Packet: WP-7A-authentic-configured-fixture
Status: ready

## Result and owner

Deliver one repository-owned authentic source fixture for the configured CLI
proof required by combined output-conflict R2. Stage 5 owns source provenance;
Stage 1 owns fixture preparation; Stage 6 retains R2's semantic acceptance.
Use [configured-cli-fixture.md](./configured-cli-fixture.md) for the selected
storage/acquisition policy, inputs, dependency ledger, and exact exit criteria.
The September 11 review-fix request authorizes this policy choice and bounded
fixture preparation without another design-only approval loop.

Exact: upstream MODULE/source/archive/patch bytes and hashes, plus the existing
named Bazel behavior claims. Slug-native: current configuration/path identity
and diagnostic infrastructure. Deferred: any new semantic source, execution-
group, action-family or configured ruleset support exposed by the fixture.

## Scope

Allowed changes:

- `tests/v2_oracle/fixtures/configured-cli-authentic/` for manifest, upstream
  notices and demanded authentic metadata/payloads;
- `tools/v2_oracle/configured_cli_fixture.py` and
  `tests/v2_oracle/test_configured_cli_fixture.py` for portable offline assembly
  and focused validation;
- `tools/v2_oracle/run_payload_demand_probe.sh` only to reuse/parameterize bounded
  supervision for the portable F3 gate; retain the historical evidence handles;
- this manifest, canonical status and the Stage 5/6 gate ledger at a genuine
  acceptance or blocker.

Start with these evidence handles; expand to the full R2 diff only for a semantic
question or R2 recovery:

- `git show 27e9e9c0c:app/slug_cli_v2/tests/cli.rs`, module
  `configured_action_conflicts`, especially `BUILD` and `workspace()`
  (lines 5246–5292): authentic test-owned root MODULE/BUILD/defs declarations.
- `tools/v2_oracle/run_payload_demand_probe.sh` lines 345–434: historical staging
  and invocation recipe; `app/slug_cli_v2/src/payload_demand_probe.rs` lines 20–67:
  retained `//:root` request and native publication boundary.
- [F3 invocation contract](./configured-cli-fixture.md#f3-invocation-contract):
  the portable proof to implement. The historical driver has personal-cache and
  vanished `/tmp` dependencies, deliberately missing abseil and an old timeout;
  do not run it unchanged for F3.

No production Rust, applying R2 to main, source-policy change, fake platforms
body or upstream local override belongs to fixture preparation. Keep authentic
custom root declarations; do not infer demand from the full registry catalog.

## Work and validation

1. Resolve pinned tools and the targeted evidence above. Recover root declarations
   from the durable Git object, not the obsolete temporary patch path. Preserve
   the root's behavior while replacing upstream stubs/overrides with authentic
   inputs under the fixture contract. Verify available content hashes.
2. Assemble authentic inputs with provenance and copied upstream notices under
   the selected repository-owned fixture directory; do not invent archive bytes.
   Acquire only exact missing inputs whose demand is established, under the
   ordinary execution/network permission mechanism.
3. Use bounded diagnostics to identify the first missing source or unsupported
   semantic owner. A diagnostic may use an explicitly supplied cache path;
   final fixture assembly/tests may not depend on that path or personal caches.
4. Prove offline assembly from a fresh temporary root, hash/patch mismatch and
   missing-object rejection, and complete configured source observation for the
   R2 root. Every named runtime test stays <=12 seconds/15 seconds absolute;
   compile/preparation stays separate and <=60 seconds per operation.
5. Record one terminal receipt: complete fixture, or exact missing input/owner
   and the smaller prerequisite required. Hash verification alone is not a
   complete configured-runtime proof. On success select combined R2 recovery;
   baseline attribution and remaining semantic gates are still required.

Invocation/compiler/test-selection corrections within this contract use the
orchestration skill. New source semantics, evidence contradicting the design,
unknown integrity/provenance, or inability to complete within user constraints
are genuine decisions for the root. Preserve successful evidence; no blind
repeat of the old spent probe. A fresh probe must name the new question and
use the accepted typed source diagnostic, not reinterpret old receipts.

## Immediate predecessor and durable candidate

Run registry policy accepted at `47163df7b`; earlier sandbox failures were
attributed to bind denial, and the named CLI/Server proofs passed. The September
11 review repairs workflow/status and daemon cleanup; it does not accept R2.

R2 snapshot: branch `review/output-conflict-r2`, commit `27e9e9c0c`,
base `97dffd5d4`, original patch SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`.
`git show 27e9e9c0c:review-evidence/validation.txt` retrieves the original receipt.
The snapshot must incorporate relevant landed prerequisites before new evidence;
its preservation metadata never enters production integration.
