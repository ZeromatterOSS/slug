# Current Slug V2 Packet

Packet: `WP-2A-m1-next-source-certificate-consumer-audit`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: audit the next smallest single source-certificate consumer after the
accepted native root exported-source bridge and select one bounded design
successor or an explicit prerequisite `REPLAN`. This packet is
documentation-only.

## Fixed predecessor

Commit `f0849151` accepts the sole-root exported-source bridge over the
private request-revision family in `207fe438`, its design in `94324880`,
and deterministic source-ordering evidence in `2ffad088`. An exactly
one-target root build initializes revision with its already-full native path
epoch. Only a later `PackageTargetKind::ExportedFile` success or completed
root-source error retains the exact FileBytes certificate. Finalization
current-checks, exact-reobserves, publishes revision plus the full selected or
one-entry-replaced epoch atomically, and retries through a reversible terminal
token without exposing provisional events.

The implementation is private to `runtime/request_revision.rs`,
`runtime/dice.rs`, and `runtime/events.rs`. Public root-source bytes remain
a regression/non-widening invariant; certificate identity, revision,
final-reobservation, retry/reset, and stale-effect suppression are
Slug-native. Public commands remain lease-serialized.

Focused proof passes 11 revision tests, the sole-root bridge integration, the
multi-target isolation regression, and five terminal-token lifecycle tests.
The bounded full crate passes 220 library and 12 integration tests with two
independently reproduced inherited failures skipped. Strict Clippy stops first
in unchanged `allocative_derive`; targeted Bazel Rust reaches analysis and
then stops on six unchanged missing `slug_bzlmod_v2` `include_bytes!`
inputs. Formatting, diff hygiene, artifact checks, and independent
DICE/event/cleanup review pass. Conservative accounting is 555/600 net
production, 383/750 test, and 938/1,350 total Rust lines.

## Audit question

What is the next uniquely smallest terminal whose complete mutable Host source
frontier can be retained as an exact private certificate and final-validated
through the accepted native publication owner without widening into a second
graph, repository/materialization ownership, public overlap, or a partial
certificate?

Inspect the live checkout and compare only enough of these candidate families
to select one bounded successor or prove the prerequisite:

- root `MODULE.bazel` discovery/evaluation;
- selected root package BUILD discovery/load;
- one loaded root `.bzl` module; and
- the already admitted direct-local external exported-source route.

Map the public/daemon entry, DICE root and dependency chain, source discovery
and byte-read keys, recursive expansion, Need/error precedence, provisional
terminal/effect owner, and every updater/commit boundary. A selected consumer
must cover its complete mutable source-selection frontier. One selected file's
bytes are insufficient if directory precedence, package roots, includes,
recursive loads, repository routes, or materialization can change the same
terminal.

For every candidate inspected, record:

- exact live symbols and ownership direction between core, loading, and Bzlmod;
- the observations required to certify both source selection and contents;
- whether the frontier is fixed and bounded before evaluation or expands
  dynamically through includes or Starlark loads;
- success, absence, wrong-kind, read/evaluation error, Need, event, and
  cancellation ordering;
- whether an existing private carrier crosses into core without a reverse
  dependency or public ABI;
- whether final validation can avoid DICE compute, Starlark, repository work,
  materialization, event callbacks, and lock reentry under the owner;
- the accepted evidence and exact/Slug-native/deferred classification; and
- one smallest design successor with file/cap/proof boundaries, or a precise
  `REPLAN` prerequisite.

## Compatibility and authority

Preserve every already accepted serial root module, BUILD, `.bzl`, and
direct-local public behavior. Do not claim new Bazel parity from the private
bridge. Exact file observations and existing admitted serial outputs stay
exact only where separately evidenced. Certificate aggregation, revision,
final validation, retry/reset, no-mixed publication, and future overlap remain
Slug-native. Directory/glob unions, repository/materialized source
certificates, historical Host reads, and public overlapping commands remain
unsupported/deferred until separately admitted.

The current manifest is scheduling authority. The compact predecessor plus Git
preserves accepted implementation, proof, compatibility, and cleanup evidence.

## Allowlist and caps

Edit exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`; and
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Caps are 40 canonical, 220 current-packet, 220 Stage 2, and 480 total net
ledger lines. No Rust, Cargo/BUILD, oracle fixture, generated evidence, or
other ledger file is authorized. Read-only source inspection and existing-test
discovery are permitted; do not run or update an oracle.

STOP on any code write, public API/output/overlap or lease change, new DICE
key/store/graph, snapshot replacement, partial one-file certificate for a
multi-observation terminal, repository/materialization activation, source
observation, watcher or historical Host claim, JVM work, evidence generation,
or cap excess.

`REPLAN` if every remaining terminal has a multi-observation or dynamically
expanding source frontier, if a certificate needs a reverse loading/Bzlmod-to-
core dependency or public ABI, if final validation requires compute or Starlark
under the owner, or if accepted evidence cannot bound the selected behavior.

## Acceptance and immediate successor

Accept only after independent source/ownership review confirms either one
uniquely smallest complete certificate frontier or the exact prerequisite that
must be designed first. The successor remains documentation-only when a new
shared representation or owner boundary is required. Do not combine root
module, BUILD, `.bzl`, external repository, lease removal, or public overlap
with implementation.
