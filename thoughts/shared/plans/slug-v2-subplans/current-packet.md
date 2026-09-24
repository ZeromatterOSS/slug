# Current Slug V2 Work Packet

Packet: WP-7-138-m7a-repository-which-path-normalization-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews accepted; M7A remains partial

## Outcome and compatibility

Admit lexical normalization of an absolute Unix `PATH` component before
`repository_ctx.which` probes a program. The selected component, first-hit
ordering, returned path spelling and repository result are **exact** for this
Bazel 9.2 slice. Existing fixed program/PATH/component/candidate caps, tracked
Host observations, Rust valid-Unicode path representation and diagnostics are
**Slug-native**. Relative and empty entries remain ignored. Windows/PATHEXT,
non-UTF-8 environment and program names, process-race snapshots, uncapped
arbitrary PATHs and `repository_ctx.execute` are **unsupported/deferred**. No
existing accepted exact slice is widened silently, and no JVM enters Slug.

## Pinned evidence and authentic demand

Pinned Bazel 9.2 `StarlarkBaseExternalContext.java:2334-2352` runs
`PathFragment.create(p)` for each `PATH` entry before absolute filtering,
candidate construction and executable probing. `PathFragmentTest.java:692-704`
and `:802-807` prove lexical `.`/`..` normalization; the selected
`StarlarkRepositoryContextTest.java:236-256` checks `which` ordering and
relative-entry filtering, but does not directly test `..` entries. A fresh
ignored-RC, download-disabled Bazel 9.2 query in
`/tmp/slug-wp838-which-dotdot-nLDbcM` returned `@selected//:found` (exit 0)
when a repository rule called `which("sh")` under `PATH=/usr/../bin`. The
selected rules_shell 0.6.1 `sh_config.bzl:121-138` calls `which("bash")`, then
`which("sh")` on miss. Slug's ambient `PATH` is 657 bytes/25 entries, below
all caps, but contains an absolute `..` component before the first executable
shell; the current preflight rejects it as `WhichLimit`. With explicit bounded
`PATH=/bin:/usr/bin:/usr/local/bin`, the authentic Rust-toolchain `//:probe`
clears rules_shell and next stops at selected rules_cc 0.2.17
`unix_cc_configure.bzl:273` calling unsupported `repository_ctx.execute`.

## Owner and decision

Keep the existing synchronous repository-rule context and selected file-effect
replay owner. `which_candidates` filters nonabsolute entries and maps each
absolute entry through `NormalizedAbsolutePath::new`, which lexically clamps
parents at the filesystem root; build the candidate from that normalized path
instead of requiring the raw spelling to equal it. Preserve fixed caps and
deduplication, program Java-trim behavior and the current ordered prepared
candidate Need protocol. The existing environment snapshot/selected-owner
certificate owns `PATH`; replay verifies the declared cell before Host
resolved-path observations and publishes only the terminal invocation. No
new DICE key, direct filesystem read, cache, retained data shape or async task.
Normalizing different raw PATH spellings to one candidate never erases the raw
tracked environment input: A→B→A must reselect/restabilize correctly.

The 3,000-line repository context remains cohesive for this bounded helper and
its colocated tests; extraction would add an owner seam for a one-line semantic
change. Do not change general path normalization or repository execution.

## Proof and validation

Add a small offline Stage 1 fixture `repository-rule-which-normalized-path`
with fixture-level `PATH=/usr/../bin`, a local generated repository that
requires `which("sh")` to return `/bin/sh`, and one anchored query row. Its
provenance cites the pinned source/tests and isolated probe above; fresh
ignored-RC Bazel 9.2 generation, independent fresh verification and rebuilt
Slug replay must pass. Expected fixture growth is five files/one row. No
credential-bearing RC, upstream archive or network dependency is copied.

Focused Rust tests prove normalized `..`/`.`/repeated-slash components,
relative ignoring, candidate deduplication, normalized first Need, existing
limits, and returned path. Extend the selected-owner observed test with a
same-candidate raw-spelling A→B→A (`/which/first` versus
`/which/../which/first`): assert B's raw environment is retained, the Host
candidate remains normalized, and A restores. Protect ordered Host frontier
and mode/symlink checks. Run exact-selector preflight, focused/protected
Loading tests, JSON no-run compile, format/diff/plan/archive/fixture hygiene,
and `cargo build -p slug_cli_v2` before `SLUG_V2_BIN` replay. Serialize Cargo.
Clean `slugd` before and after the authentic ambient-PATH `//:probe` smoke;
report only its first new stop, not full C++ or root Build closure.

## Scope and stops

Base clean main `95fc985e8`. Allowlist: this manifest and canonical Current;
`app/slug_loading_v2/src/repository_rule_context.rs` and, only if needed for
raw-environment A→B→A proof, existing focused tests in
`app/slug_loading_v2/src/module_extension_repository_file_effect.rs`;
`tests/v2_oracle/fixtures/repository-rule-which-normalized-path/**`; compact
Stage 4 owner and Stage 9 disposition notes after acceptance. No changes to
the general `NormalizedAbsolutePath` constructor, DICE key/observation owner,
candidate caps, Windows behavior, `execute`, rules_cc special cases, remote
transport or configuration identity. Expected production delta under 15 lines
and proof under 100 Rust lines plus one fixture; counts are review triggers,
not caps. Independent design and final review required. `REPLAN` if Bazel
does not return normalized `/bin/sh` in the checked-in oracle, if raw PATH
identity is lost in selected-owner replay, or if the correction needs a new
filesystem/async owner.

## Immediate predecessor

WP837 at `95fc985e8` accepted the bounded generated repository symlink
surface. M7A remains partial. Its authentic ambient-PATH smoke exposed this
`which` rejection; the bounded-PATH replay establishes later
`repository_ctx.execute` as a separate first stop after this repair.

## Validation evidence (2026-09-24)

The one-line semantic correction builds each candidate from the normalized
absolute component. Two repository-context tests prove normalized first Need,
returned path and dot/dot-dot/repeated-slash deduplication. The selected
file-effect test now changes only raw PATH spelling across A→B→A, retaining
B's raw environment while observing the same normalized Host candidate and
restoring A. JSON no-run compilation and exact-selector preflight pass;
repository-context tests pass 24/24 and selected file-effect tests 25/25.
`cargo fmt --all -- --check`, diff, plan-status and archive checks pass.

The five-file/one-row fixture passed fresh ignored-RC Bazel 9.2 generation,
independent fresh Bazel verification and rebuilt Slug replay. `slug_cli_v2`
rebuilt before the fixture and authentic smoke. No stale `slugd` existed
before or after the smoke. The authentic ambient-PATH `//:probe` cquery no
longer reports the rules_shell `which` limit; its first stop is selected
rules_cc 0.2.17 missing `repository_ctx.execute` (exit 2), not full toolchain
or Build closure. Fixture discovery passes. The broad Python oracle unit
suite reports 123 passed and three pre-existing assertions against unrelated
`load-invalidation`/`simple-rule-action` fixtures; it is not a WP838 fixture
failure. Independent FINAL review accepted the bounded Unix classification,
same-candidate environment proof and exact fixture; `repository_ctx.execute`
remains a separate packet.
