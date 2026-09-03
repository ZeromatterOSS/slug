# Current Slug V2 Packet

Packet: WP-4-5-7A-repository-context-which-implementation-r1

Milestone: M7A bootstrap-critical loading/repository execution closure. Add
the bounded generic Unix `repository_ctx.which` composition selected by the
accepted docs-only audit.

Status: ready for one bounded Rust implementation and independent terminal
review. The audit returns `ACCEPT`; only the allowlist and behavior below are
authorized.

## Accepted predecessor

`WP-5-7A-repository-context-which-audit` returns `ACCEPT` without a new DICE,
Host-input, path-observation or materialization owner. Pinned Bazel 9.2 source,
focused upstream tests and an isolated installed-Bazel 9.2 oracle agree on the
bounded Unix behavior. Authentic rules_shell 0.6.1 declares `PATH` and performs
the generic sequence `which("bash")`, then `which("sh")` when `BAZEL_SH` is
absent.

The predecessor template packet remains terminally accepted at 371 production
and 484 proof Rust additions. Do not revisit its source routing, byte
replacement or generated-file effect ownership.

## Audit evidence and compatibility decision

At Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a`,
`StarlarkBaseExternalContext.which` and `findCommandOnPath` establish that:

1. `program` is a string; empty input and either path separator are errors;
2. Unix lookup splits `PATH` on `:`, ignores non-absolute entries, preserves
   source order and appends the Java-trimmed program basename (leading and
   trailing code points at or below U+0020 only);
3. each candidate follows symlinks and must resolve to Bazel's Unix `isFile`
   category (regular or special file) with owner-executable `0100` set;
4. the first match returns a Starlark `path` preserving the candidate spelling,
   and stable exhaustion returns `None`; and
5. Windows additionally retries with Bazel's executable extension.

The isolated Bazel 9.2.0 oracle used two absolute directories plus one relative
entry. It discriminated first-match order; a non-executable first candidate and
executable second candidate; directory, missing and relative-only misses; a
symlink hit whose returned string remained the symlink path; surrounding
ASCII program whitespace; and the exact empty/slash errors. Pinned source
anchors are `StarlarkBaseExternalContext.which`/`findCommandOnPath`,
`StarlarkRepositoryContextTest.testWhich`, `UnixFileSystem.statNullable` and
`isExecutable`, `UnixFileStatus.isFile`, and `StarlarkPath.equals`/`hashCode`.
Authentic rules_shell is pinned at
`shell/private/repositories/sh_config.bzl`; its repository rule declares PATH
and its Unix helper tries `bash`, then `sh`.

The implementation admits **exact** Bazel 9.2 behavior for valid-Unicode,
lexically normalized absolute Unix `PATH` entries, a nonempty basename of at
most 255 UTF-8 bytes, at most 4 KiB of `PATH`, at most 64 components and at most
64 distinct candidate paths. Missing `PATH`, empty components, relative
components, wrong kinds, missing files, regular/special files, symlinks,
owner-executable mode, first-match order, candidate spelling, and `path`/`None`
are in the exact
stable fresh-evaluation slice. Slug's valid-Unicode environment/path
representation, fail-closed resource caps, and exact tracked invalidation for
PATH/candidate metadata and symlinks are **Slug-native** integrity behavior;
Bazel's `which` performs untracked host metadata checks. Whitespace-only-after-
Java-trim basenames, non-normalized or non-Unicode absolute entries, concurrent
metadata races/resolver observation failures, Windows separator/extension
behavior and wider inputs are **unsupported/deferred**. No Bazel-wide parity is
claimed outside this slice.

## Existing-owner composition

- `RepositoryEnvironmentCellKey`, `RepositoryPlatformKey` and
  `RepositoryHostInputTransaction` remain the sole Host environment/platform
  owners. `which` records `PATH` as a dynamic environment input. On a typed
  lookup demand the outer driver must first request a missing environment
  frontier or verify the authorized `PATH` cell; it must not observe a
  candidate against an unverified snapshot.
- `ResolvedPathKey` and `ResolvedPathObservationKey` remain the sole candidate
  resolution owners in `PathObservationNamespace::Host`. Their existing
  `PathObservationKey` demands and injected `PathObservationEpochKey` own
  missing-to-present, metadata, symlink and terminal-path invalidation. This is
  deliberately stronger Slug-native integrity than Bazel's untracked metadata
  checks. The requested logical path supplies the returned spelling; the
  resolved terminal `PathLstat` supplies regular-or-special kind and the exact
  Unix `0100` test.
- Add only invocation-local typed `WhichNeed`/prepared-candidate scratch. A
  demand escapes the synchronous invocation as an error, which drops the
  evaluator, heap, builder, captures and borrows before the outer effect key
  awaits either environment verification or path resolution and retries.
- Generalize `RepositoryStarlarkPath` provenance only enough to distinguish a
  canonical Label path from a `which` result. Visible equality, hashing,
  stringification and representation remain normalized physical path bytes.
  `repository_ctx.template` must continue to require canonical external Label
  provenance, so a `which` path cannot silently widen the accepted template
  source surface.
- Only the terminal invocation may publish prints, dynamic environment names
  or generated-file effects. Prepared candidates and observations are bounded
  attempt scratch and do not enter retained repository definition, call,
  certificate, manifest or effect-plan identity.

No new DICE key, injected state, retained cache, process environment read,
direct filesystem access, lock, source/materialization owner, rules_shell
branch, shell-name branch or toolchain branch is permitted.

## Frozen implementation

1. Add the `which(program)` repository-context method with exact string typing,
   empty/slash/backslash diagnostics and Java-compatible trimming of only
   leading/trailing code points at or below U+0020. Parse only the admitted
   `PATH` slice and preserve its order; do not use Rust Unicode `str::trim`.
2. Represent one unresolved logical candidate as a typed invocation error and
   one prepared outcome as executable hit or ordinary miss. Dangling symlinks,
   `PathResolutionError::Cycle` and `PathResolutionError::InfiniteExpansion`
   map to a miss and continue. Fail closed on resolver observation or
   inconsistent-state errors: the existing resolver cannot distinguish
   Bazel's nonthrowing metadata checks from the later throwable executable
   check during a concurrent race.
3. Before resolving the first candidate, demand and verify dynamic `PATH`
   through the existing transaction frontier. Resolve candidates one at a time
   through the legacy/observed `ResolvedPath` siblings after invocation state
   has been dropped; merge every observed frontier, including missing,
   wrong-kind, mode and symlink results.
4. Return the requested candidate path for an owner-executable regular or
   special terminal result, `None` for absence/exhaustion, and a generic Starlark
   `path` whose provenance cannot be consumed as an admitted template Label
   source.
5. Fail closed on any resource cap or unsupported platform/input shape before
   effect publication. Preserve the existing repository-rule Windows stop.

## Allowlist and size bounds

Production and adjacent proof edits are limited to:

- `app/slug_loading_v2/src/repository_rule_context.rs`
- `app/slug_loading_v2/src/module_extension_repository_file_effect.rs`

Gross additions are capped at 250 production Rust lines, 500 proof Rust lines
and 750 total Rust lines. Every new helper is at most 80 logical lines and at
most eight control-flow branches; existing helpers may grow by at most 40
logical lines. Both allowlisted files are already large, but the context method
and the repository-file-effect retry belong to their sole existing semantic
owners; splitting either would create a second invocation or retry owner.

## Required proof

- Context tests: exact value type and string/repr/hash/equality behavior; wrong
  type, empty, slash and backslash errors; Java U+0020/control trimming versus
  nonbreaking/Unicode whitespace; missing `PATH`; relative and empty
  components; normalized absolute order; all four resource caps; and Label-
  versus-which provenance isolation for template.
- Effect tests in legacy and observed modes: first and later hit, miss,
  non-executable first candidate, owner-versus-group/other execute bits,
  directory, executable special file, symlink-to-file, dangling/cyclic symlink,
  descendant symlink expansion, fail-closed observation/inconsistent-state
  errors, exact returned logical spelling and full observation-frontier merge.
- Retry tests: environment demand precedes every candidate observation,
  evaluator/heap/captures are dropped before DICE, multiple candidates make
  bounded progress, terminal success/failure alone publishes prints/effects,
  repeated candidates reuse scratch, and 64/65 boundaries fail closed.
- Incremental tests: same-daemon warm reuse plus A/B/A restoration for PATH
  order/value, missing-to-present, executable mode, kind and symlink target,
  with no stale `path`/`None` result.
- Run focused loading tests, `cargo test -p slug_loading_v2`,
  `cargo test -p slug_query_v2`, rebuild `slug_cli_v2`, then rerun the authentic
  rules_rust/rules_shell replay. The replay must clear both generic Unix
  `which` calls without a ruleset or shell special case.
- Run formatting, clippy where the workspace permits it, changed-test listing,
  diff/allowlist/cap checks, hygiene scans, archive-reference checks and an
  independent terminal review.

## Terminal stops

Return `REPLAN` if implementation needs a new Host/DICE/path owner, direct
filesystem or process-environment access, an evaluator/borrow/lock across
DICE, unverified `PATH` before candidate observation, a retained candidate
cache, widened template provenance, unresolved race/error equivalence, Windows
support, a rules_shell/toolchain special case, a third production file, a cap
violation, or more than one material correction cycle. Otherwise terminal
review may return `ACCEPT` and select the next authentic replay boundary.
