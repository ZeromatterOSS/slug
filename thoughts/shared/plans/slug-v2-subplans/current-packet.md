# Current Slug V2 Work Packet

Packet: WP-5-7A-source-observation-registration-diagnostic-design-r1

Status: SELECTED, DOCS-ONLY DESIGN. No implementation or execution is authorized.

## Accepted predecessor and audit result

The default-off observer is accepted in `64c7d2475`; its one execution contract
in `d6a9c41de` was consumed and independently accepted as
`bounded-observer-inconclusive` in `fded00c79`. No retry is available.

The source-boundary audit inspected nine direct source/test owners with excerpts
below2MiB and establishes the natural ownership chain. A canonical external Bzl
read computes `HostRepositorySourceObservationKey` or its epoch wrapper. The
result retains `HostRepositorySourceObservationError { input, relative_path,
kind }`; `finish_external_bzl_source` clones that full typed error and the exact
`CanonicalLabel` into `ExternalBzlModuleError::SourceObservation`, retaining the
observation epoch. Success is distinct: built-in or request-present continues,
and request-absent becomes `ExternalBzlModuleError::Absent`.

`HostRepositorySourceObservationErrorKind` exhaustively distinguishes
`BuiltinPath`, `Builtin`, `BuiltinCompute`, `Request(RepositorySourceFileError)`
and `RequestCompute`. The nested request enum has eleven typed variants covering
invalid paths, materialization/compute, observation/inconsistent state, wrong
kind, cycle/infinite expansion and resolution/file compute. The canonical source
input separately retains route and built-in/request disposition.

The terminal marker is intentional presentation, not lost semantic identity:
the bounded registration renderer explicitly maps `SourceObservation { .. }` to
`[diagnostic incomplete: SourceObservation]`, and natural tests freeze that
behavior while proving hidden `POISON` state is not formatted. Recursive `Child`
nodes publish `raw_load`, not their retained canonical label. Therefore the
receipt proves the exact outer `ExternalBzlModuleError::SourceObservation`
variant, but not the hidden error-kind discriminator, canonical leaf label,
materialization outcome, cache availability, payload demand or root cause. An
absent successful request would have rendered `Absent`, so this is also not an
absence result. Those remaining runtime facts are UNKNOWN.

The existing reusable pattern is a borrowed
`write_registration_diagnostic(&mut dyn fmt::Write)` method owned beside its
typed Bzlmod error. Loading supplies the fixed3072byte ASCII-escaping sink and
depth limit. The helper performs exhaustive causal matching, propagates writer
stop and never uses arbitrary `Debug`; retained equality remains unchanged.
Independent terminal review ACCEPTS the owner chain, counts, evidence limits and
docs-only successor scope.

## Goal

Freeze one bounded cross-crate presentation extension for the existing
`SourceObservation` owner. The design must make the exact retained leaf label,
source class/path and typed discriminator visible enough to distinguish source
failures without rendering retained route/materialization graphs or changing
semantic identity.

## Authorized work

- Read only the nine audited owners and at most four directly referenced utility
  or proof owners if an output field or proof obligation cannot otherwise be
  frozen. Keep combined excerpts below2MiB.
- Specify the exact output grammar for all five observation error kinds, all
  eleven `RepositorySourceFileError` variants and four built-in source variants.
- Reuse the existing borrowed writer pattern. The outer Loading buffer continues
  to own ASCII escaping, the3072byte output limit and writer-stop behavior.
- Freeze a file/line budget and focused proof matrix. Required proofs include
  exhaustive variant projection, natural legacy/observed Loading handoff, exact
  canonical leaf label, no retained request/route Debug leakage, output/depth
  limits, early writer stop, typed same-display inequality/A-B-A, unchanged Arc
  sharing/final release, unchanged Need/outer-error precedence and default build
  behavior.
- Update only this manifest, canonical Live Status, Stage5 summary and
  `~/PROGRESS.md`; outside this manifest add at most80 lines.
- Obtain independent terminal review before selecting implementation.

## Design invariants

- Add no retained field, key, cache, lock, allocation owner or DICE edge.
- Do not alter `Debug`, `Display`, equality, hashing, cloning, route selection,
  source admission, materialization, observation epochs, Need ordering or error
  propagation.
- The prospective Bzlmod helper must borrow the current error, match every
  private discriminator explicitly and stop immediately when its writer fails.
  It may stream bounded scalar strings/paths through the caller sink; it must not
  allocate a diagnostic String or format a retained request, route, graph or
  error through a fallback `Debug` implementation.
- The prospective Loading arm must print its retained canonical label before
  delegating to the helper; recursive child text remains explicitly raw-load
  presentation rather than canonical identity.
- This is diagnostic observability only. It cannot retroactively classify the
  consumed observer receipt, prove payload demand/unavailability, explain14GB or
  authorize acquisition/replay.

## Prohibited work and stops

Do not edit Rust/tests/fixtures, run Cargo, a compiler, test, probe, `strace`, CLI,
daemon, Bazel, network, cache scan or replay. Do not inspect/acquire sources,
increase any cap or restore any output-conflict R2 section. If the grammar cannot
be exhaustive without rendering retained graph/request state, changing identity
or introducing a new semantic owner, record UNKNOWN and REPLAN.

Preserve `/tmp/slug-conflict-r2.XZJWwv/candidate.patch` at SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`
and `/tmp/slug-sentinel-draft.Lln8y0/candidate.patch` at SHA-256
`8eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2`.
Never inspect/print/copy/commit `~/.bazelrc` or derived credentials.
