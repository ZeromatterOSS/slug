# Current Slug V2 Packet

Packet: `WP-4-5-6-generated-repository-file-effect-handoff-application-proof-accounting-correction-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: `04-starlark-loading-and-build-packages.md`,
`05-bzlmod-and-repository-graph.md` and
`06-analysis-toolchains-and-actions.md`
Base: accepted producer `b360be14`, accepted handoff design `d52336de` and the
exact retained Rust candidate below

Result: preserve the structurally accepted demand-only effect handoff, correct
two fail-closed materializer boundaries and complete its lawful direct/composed
proof without reconstructing unavailable dirty-baseline blobs.

## Retained candidate and write authority

The seven-file candidate is frozen at these exact current values:

| Path | Lines | SHA-256 | Retry authority | Physical ceiling |
|---|---:|---|---|---:|
| `app/slug_bzlmod_v2/src/host_module.rs` | 4,872 | `56a7ffe34f8f26c3e70b02deed12268198599060cc455127f2edd3bddab22506` | frozen | 4,872 |
| `app/slug_bzlmod_v2/src/source_preparation.rs` | 16,849 | `e17ffccdb402f4cad227d421b53b5d5952e39b8707a6980a8c2ef66b3cee9faf` | proof only | 17,050 |
| `app/slug_bzlmod_v2/src/host_package.rs` | 5,009 | `1921abc6f0fedc0f7c0d14504168980f1063deec82bcfff9b64c2c3c6b8cc5b8` | frozen | 5,009 |
| `app/slug_core_v2/src/runtime/generated_repository_definition.rs` | 4,083 | `a87b856c9bc8b279d134f01229cb3bc240f451f41c8741beefffb7aca7df3566` | frozen | 4,083 |
| `app/slug_core_v2/src/runtime/root_apparent_repository_definition.rs` | 1,735 | `033d545f96e571f1fcfb628ec7c9e90813f6662a5623a5e12ed5c6fa0fc256e1` | frozen | 1,735 |
| `app/slug_core_v2/src/runtime/generated_package_route.rs` | 675 | `073591147e0d406c609c96c630773fa05f7b4de42864b0a93a7f2e6746660975` | proof only | 1,150 |
| `app/slug_core_v2/src/runtime/repository_io.rs` | 5,900 | `648e1b840562596ceae2754d18c3e19f0eb27850e62a9ed6badf4deabd091390` | two production fixes plus proof | 6,500 |

All other Rust, tests, fixtures, oracles, Cargo/BUILD files and APIs are
retained and non-writable. Preserve every structural choice accepted in
`d52336de`: plan in route/capability/request kind; authenticated owner+ordinal;
mapping -> definition -> effect order; no second scan; no loading edit; private
temporary-root application; distinct structural, source-association and
physical identity domains.

## Exact production corrections

Change only `repository_io.rs` production:

1. `NativeGeneratedRepositoryFileEffectsIo::set_mode` must return a typed
   materialization failure under `cfg(not(unix))`. Non-POSIX executable mode is
   unsupported/deferred and must not silently succeed.
2. `validate_native_request` must reject
   `GeneratedFileEffects` paired with a recognized Bazel-tools local, HTTP or
   Git native rule as `RepositorySessionError::KindMismatch`. A custom
   generated repository-rule spec remains the only admitted pairing.

No other production change is authorized. In particular, do not change the
plan representation, route/request identity, source digest, I/O sequencing,
session selection, repository capability, observed carrier or public surface.

## Corrected accounting

Four frozen entry contents were retained dirty state and are not Git-reachable;
their recorded hashes and line counts cannot recover exact added/deleted-line
partitions. Do not fabricate the original 500/850 accounting. The conservative
mechanical comparison with `b360be14`, including the untracked 675-line route,
is `+1,451/-106` across the seven files. Charge all 1,451 additions as retained
production.

The retry may add at most 30 production, 1,250 proof and 1,280 aggregate lines
from the hashes above. Conservative cumulative ceilings are therefore 1,500
production, 1,250 proof and 2,750 aggregate. Add no `rustfmt::skip`; proof
helpers remain private and test-only.

## Required direct proof

Use lawful constructors and production functions to prove:

- route/capability Eq and manual Hash include the complete plan; full request
  Eq includes ordered path, content and executable polarity; the request gains
  no Hash implementation and result-key Hash remains ID-only;
- full-request and epoch comparison distinguish same-ID changed plans,
  reject conflicting concurrent requests and restore A/B/A;
- generated session stale-token discard, selection, warm reuse, changed-plan
  root replacement and A/B/A restoration;
- exact two-file bytes/order, `0755`/`0644` POSIX modes, preflight before root
  allocation, all six I/O failures and source-association framing;
- recognized local/http/git plus generated-kind mismatch and custom generated
  pairing success;
- real Legacy/Observed route Need, effect semantic terminal, child-only event
  ownership, non-Generated nonactivation and mapping -> definition -> effect
  order with the producer frontier present; and
- immutable instance/source consumption without a second materialization or
  rule execution.

An observed effect outer and cancellation cannot be independently constructed
at this route without violating lower-key invariants. Compose those two rows
from the accepted loading producer's direct outer/cancellation proof plus exact
route source-shape assertions showing carrierless propagation and no parent
event publication. Every source-shape assertion must inspect only the
production prefix before the terminal `#[cfg(test)] mod tests` marker;
test-only imports/helpers before that marker do not authorize whole-file
matching, and matching the assertion's own literal is not proof. Apply the same
bounded source-shape rule to prove the `cfg(not(unix))` branch returns a typed
failure and no longer falls through to success. Do not add an injection
surface. Do not require three distinct epoch rows: certificate and Host-Bzl
observations may lawfully dedupe; prove left-first association over the actual
nonempty rows.

## Validation and compatibility

Run formatting on the three writable files; focused Bzlmod/core proof; full
Bzlmod, loading and core suites serially; `cargo build -p slug_cli_v2`; clean
stale `slugd` before and after the retained generated-source fixture; run that
fixture unchanged; then run archive status, diff/scope/hash/accounting and
independent terminal review. The inherited mixed-horizon selected-graph test
that once failed in the full suite and immediately passed alone is tracked as
an order-sensitive baseline risk, not waived or attributed to this packet.

The fixture's ordered ASCII bytes and executable polarity on POSIX are
**exact Bazel 9.2**. Valid-Unicode paths, owner/ordinal, structural route and
request identity, source association, staging and immutable publication are
**Slug-native**. Other repository-context members, overwrite/delete/symlink/
download/execute, nonroot rule definitions, Label/StarlarkPath, non-POSIX mode
application, broader platforms, public query breadth and exact Bazel
configuration/output bytes remain **unsupported/deferred**.

Pinned Bazel 9.2 is behavioral authority. Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` remains concept-only guidance:
selected demand seeds choose one natural owner, generated effect materialization
owns one private candidate, and acceptance transfers only immutable output
state after validation. Copy no Zig code, representation, scheduler, digest,
manifest/root layout, output vector or cache policy.

STOP a fourth writable file, request field, plan bytes in RepoSpec attributes,
side table, physical-ID semantic widening, public capability variant,
certificate-internal export, core rule reload, parent event replay,
non-Generated effect call, new key/store/cache/lock/task, loading change,
Bzlmod -> loading reversal, retained evaluator/I/O handle, direct DICE write,
fixture edit, public behavior, Java/JVM, M7 closure, M8/M7B and identity-byte
work. `REPLAN` before widening or exceeding any cap.
