# Current Slug V2 Work Packet

Packet: WP-5-7A-source-observation-registration-diagnostic-implementation-r1

Status: SELECTED IMPLEMENTATION, AUTHORIZED ONLY AFTER THIS ACCEPTED DESIGN IS
PUSHED. No runtime scenario or replay is authorized.

## Accepted predecessors

The default-off observer is accepted in `64c7d2475`; its sole execution contract
in `d6a9c41de` was consumed and accepted as `bounded-observer-inconclusive` in
`fded00c79`. The source-boundary audit is independently accepted in `2a86d108f`.
It proves that full `HostRepositorySourceObservationError` identity and the exact
canonical label survive in `ExternalBzlModuleError::SourceObservation`, while the
bounded registration renderer intentionally publishes only its incomplete marker.
The retained inner runtime discriminator and cause remain UNKNOWN; no retry exists.

## Goal

Replace only that marker with a bounded borrowed causal projection owned beside
the existing Bzlmod error. Make the canonical source label, source class and typed
failure discriminator visible without rendering retained route/materialization
graphs or changing identity, DICE behavior, source admission or semantic order.

## Exact presentation grammar

Loading changes only the existing `SourceObservation` arm to emit:

`SourceObservation <canonical-label>: <source-class>: <cause>`

`<canonical-label>` uses the existing fixed-buffer label writer. `<source-class>`
is exactly one of `RootBuiltin`, `RootRequest`, `CanonicalBuiltin` or
`CanonicalRequest`, selected by exhaustive input/disposition matching without
formatting the retained input. `<path>` is `hex:` followed by two lowercase hex
digits for every byte from `OsStr::as_encoded_bytes`; it is byte-exact on the
current platform, ASCII and allocation-free. `<text>` is streamed directly to
the caller's existing ASCII-escaping sink. All enum tokens below are literal.

The five top-level causes are:

- `BuiltinPath path=<path>`
- `Builtin.<builtin-cause>`
- `BuiltinCompute path=<path> message=<text>`
- `Request.<request-cause>`
- `RequestCompute path=<path> message=<text>`

The four `<builtin-cause>` forms are:

- `InvalidPath path=<text>`
- `WrongKind path=<text> actual=<builtin-source-kind>`
- `UnsupportedCatalog path=<text>`
- `Integrity path=<text> actual_sha256=<text> expected_sha256=<text>`

The eleven `<request-cause>` forms are:

- `InvalidRepoRelativePath path=<path>`
- `MaterializationCompute path=<path> message=<text>`
- `Materialization path=<path> kind=<materialization-kind> message=<text>`
- `InvalidMaterializedPath path=<path>`
- `Observation path=<path> operation=<operation> error=<observation-error>`
- `InconsistentState path=<path> operation=<operation> before=<state> after=<state>`
- `WrongKind path=<path> actual=<node-kind>`
- `Cycle path=<path>`
- `InfiniteExpansion path=<path>`
- `ResolutionCompute path=<path> message=<text>`
- `FileCompute path=<path> message=<text>`

`<materialization-kind>` is the exact declared identifier from an exhaustive
match over `RootModuleFiles`, `MissingOverride`, `UnsupportedOverride`,
`InvalidCanonicalRepository`, `InvalidWorkspace`, `ResultCompute`,
`MissingGeneration`, `Spec`, `Transport` and `Materialization`; `<message>` is its
retained compact string. `<operation>` exhaustively maps the six declared
`PathObservationOperation` identifiers. `<node-kind>` exhaustively maps
`RegularFile`, `Directory`, `Symlink` and `SpecialFile`.
`<builtin-source-kind>` separately maps exactly `File` and `Directory` from
`BuiltinBazelToolsSourceKind`; it must not be conflated with `<node-kind>`.

`<observation-error>` is `Io kind=<io-kind> raw_os_error=<integer|none>`,
`NotALink`, `WrongKind expected=<node-kind> actual=<node-kind>`, or
`InconsistentState before=<state> after=<state>`. `<io-kind>` is the exact
declared identifier from an exhaustive match over all39 `PathIoErrorKind`
variants; no `Debug` formatting is permitted. `<state>` is `none` or only the
retained lstat `<node-kind>`; timestamps, ids, size and permission state stay out
of presentation. The same bounded projection may intentionally render distinct
full errors identically.

## Authorized implementation

Change at most these five Rust owners:

1. `source_preparation/repository_source_observation.rs`: declare the private
   diagnostic child module only.
2. New `source_preparation/repository_source_observation/registration_diagnostic.rs`:
   implement the borrowed public/doc-hidden writer and private scalar helpers.
3. New sibling diagnostic test file: exhaustively prove its grammar and stops.
4. `slug_loading_v2/src/registration_diagnostic.rs`: replace only the marker arm
   with canonical-label prefix plus helper delegation.
5. `slug_loading_v2/src/registration_diagnostic_tests.rs`: correct/add focused
   synthetic and natural expectations.

No Cargo/feature/dependency/fixture/driver change is allowed. Net additions are
capped at400 production,700 proof and1100 gross. The Bzlmod helper file is capped
at320 production lines and its test file at600; Loading production/test growth is
capped at40/100. Exceeding a file or total cap is REPLAN, not permission to widen.

## Semantic and resource invariants

- Add no retained field, key, cache, lock, allocation owner or DICE edge. Preserve
  `Debug`, `Display`, equality, hashing, cloning, route selection, source admission,
  materialization, observation epochs, Need/outer-error precedence and release.
- The helper borrows the current error, explicitly matches every admitted enum,
  streams only literals/scalars and immediately propagates writer failure. No
  diagnostic String, arbitrary `Debug`, request/route/graph formatting, I/O, lock,
  wait, environment read or unsafe code is allowed.
- Loading's current stack `[u8;3072]`, ASCII escaping, output-stop reserve and
  depth limit remain the sole budget/presentation owner. Recursive Child output
  remains raw-load text; only the leaf now prints its retained canonical label.
- This cannot reclassify the consumed observer receipt, establish payload demand
  or unavailability, explain14GB, or authorize acquisition/replay.

## Proof and command contract

- Before changing production, add one focused red expectation for the natural
  SourceObservation branch. Compilation preparation may run once per affected
  crate with `--lib --no-run`, timeout60; it is not a test execution.
- Bzlmod proofs cover four source classes, five top-level kinds, four built-in and
  eleven request variants, both built-in source kinds, and all nested
  materialization/operation/node/observation/I/O tokens, byte-exact non-UTF8 path
  output, long-text outer stop and immediate failing-writer return before later
  fields.
- Loading proofs cover exact canonical leaf label, recursive raw-load prefix,
  natural legacy and observed handoffs, output/depth caps, hidden route/request
  poison, typed same-display inequality/A-B-A, Arc sharing/final release, and
  unchanged Need/outer-error precedence.
- Every test invocation has an absolute15s timeout. Run only named focused tests;
  no whole crate/workspace suite. Default affected-crate checks may each run once
  with timeout30. Formatting, diff/cap accounting and credential/archive hygiene
  are non-runtime gates. A timeout/failure is investigated, never automatically
  retried or granted a longer limit.
- Obtain independent implementation/proof review before commit. Commit and push
  the accepted milestone to `main` before any later work.

## Prohibited work and stops

Do not run the probe, `strace`, CLI scenario, daemon, Bazel, network, cache scan,
source acquisition or replay. Do not inspect credentials, change any observer
limit, infer a new root cause, edit fixtures, or restore any output-conflict R2
section. Any semantic-owner change, unbounded formatter, fallback Debug, extra
file/dependency, command over its cap or test over15s requires REPLAN.

Preserve `/tmp/slug-conflict-r2.XZJWwv/candidate.patch` at SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`
and `/tmp/slug-sentinel-draft.Lln8y0/candidate.patch` at SHA-256
`8eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2`.
Never inspect/print/copy/commit `~/.bazelrc` or derived credentials.

Independent design review first returned REVISE because built-in source kind had
been conflated with path node kind. The corrected distinct two-/four-way grammar
and proof obligation received terminal ACCEPT; all other scope, caps and
invariants remained accepted.
