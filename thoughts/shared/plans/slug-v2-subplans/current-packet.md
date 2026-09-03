# Current Slug V2 Packet

Packet: WP-4-5-7A-repository-context-read-implementation-r1

Milestone: M7A bootstrap-critical loading/repository execution closure. Admit
the bounded Bazel 9.2 direct-external-Label `repository_ctx.read` shape reached
by the authentic apple_support replay through existing Label-path and routed
source owners.

Status: ready for bounded implementation after the docs-only audit returned
`ACCEPT`. Independent terminal review is required before acceptance.

## Immediate predecessor and replay boundary

Commit `3592fbfd1` accepts the exact one-file
`tools/osx/xcode_configure.bzl` catalog member at 6 production and 16 proof
gross Rust additions, 22 Rust total, plus the exact 329-line asset (351
aggregate additions). Source hash, mode, direct listings, physical manifest,
all three manifest-digest consumers, focused/full Bzlmod proof and the direct
loading consumer pass.

The rebuilt authenticated replay clears that catalog miss, loads and freezes
the exact source, invokes the selected apple_support extension and begins
materializing
`apple_support++apple_cc_configure_extension+local_config_apple_cc_toolchains`.
It stops at the next independent generic boundary:

`Object of type repository_ctx has no attribute read`

at
`@@apple_support+//crosstool:setup_internal.bzl:27`.

The apple_support 1.24.2 rule calls only:

`repository_ctx.read(Label("@build_bazel_apple_support//crosstool:BUILD.toolchains"))`

with the default `watch = "auto"`, then passes the returned string to the
already accepted `repository_ctx.file("BUILD", content = ...)`. The apparent
repository resolves through the already accepted repository mapping to the
selected canonical `apple_support+` source. This consumer is a discriminator,
never an activation branch.

## Audit evidence and decision

Pinned authority remains Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`:

- `src/main/java/com/google/devtools/build/lib/bazel/repository/RepositoryUtils.java`,
  `getRootedPathFromLabel` lines 54-72, first requests package lookup, rejects
  an absent package and only then derives the Label path;
- `src/main/java/com/google/devtools/build/lib/bazel/repository/starlark/StarlarkBaseExternalContext.java`,
  `getPath` lines 1570-1577, routes a Label through `getPathFromLabel`;
  `readFile` lines 1580-1624 freezes the direct
  String/Label/path signature, default `watch = "auto"`, read-event-before-watch
  ordering, directory rejection and ISO-8859-1 read;
- `getPathFromLabel`, lines 2354-2379, freezes package/materialization lookup and
  implicit Label-watch behavior, while `toRepoCacheFriendlyPath`/`maybeWatch`,
  lines 1626-1708, records external-repository file input for auto-watch; and
- `src/test/shell/bazel/starlark_repository_test.sh`, lines 2352-2399,
  distinguishes auto-watch from forced watch inside the generated working
  directory. The broader String, path, explicit-watch and arbitrary
  external-path tests cover deferred shapes and are not implementation gates.

Those three pinned files have SHA-256
`37af907998dd2b3fecb43254bfa6a0df51ec457a0c0b0d66bd4700e5077aca43`,
`69c1ed32510486148b5b84cb37000ddcabc6278df5cd9f5494a61cf4a2981d03`
and
`5d7f65a7f1dedc9509399a1d3dc41e152b376911649f5d539758ba14168e62a5`,
respectively.

Exact apple_support 1.24.2 tag evidence is:

- `crosstool/setup_internal.bzl`, SHA-256
  `620a2b434e2ad6d3882775fe46660f01aca45362b23a859058afb49077ea78e0`,
  3,139 bytes/73 lines, mode `0644`, one trailing LF; lines 25-30 are the live
  nested `file(read(Label(...)))` call; and
- `crosstool/BUILD.toolchains`, SHA-256
  `d634b84b9448ec60b60df1cece5107dc488be4800fd6133c6858496d765acf2e`,
  734 bytes/23 lines, mode `0644`, one trailing LF, entirely ASCII.

The cached BCR 1.24.2 source descriptor has SHA-256
`2c22c9827093250406c5568da6c54e6fdf0ef06238def3d99c71b12feb057a8d`
and archive integrity
`sha256-hiXMe3spUuOBAeF7Sp58BDs6efVdJuR3KBmJGZhyXhY=`. Its sole patch changes
the module-version declaration, not either crosstool file.

Audit result: `ACCEPT`. The smallest lawful category is one generic direct
canonical-external Label read with omitted/default auto-watch, a present ASCII
source and bounded size. Exact success is claimed for regular files. The
existing source owner also collapses an accepted special file into the same
byte-only `Present` value, so that source-kind success is an explicit
Slug-native behavior rather than a rejection claim. Bazel's required package
lookup and Slug's
already accepted source observation compose without a new key, byte owner,
filesystem read, lock or materialization path.

## Frozen compatibility boundary

Implement this common path, with **exactness limited to regular-file inputs**:

1. `repository_ctx.read(path)` accepts only a Starlark `Label` already resolved
   by the invocation's accepted repository mapping to a non-root, non-built-in
   canonical external repository. No `watch` argument is accepted, so omission
   selects Bazel's default `"auto"`.
2. Before requesting bytes, require the existing `LabelPathNeed`/prepared-path
   flow for that address. This preserves package lookup, selected repository
   route, materialization and failure ordering even though no path value is
   exposed to Starlark by `read`.
3. Then request the exact repository-relative file through the existing routed
   source need. Admit a `Present` source of at most 2 MiB whose bytes are all
   ASCII, including the empty file. Return the byte-identical Starlark string.
   ASCII regular files are the exact intersection of Bazel's internal
   ISO-8859-1 byte string and Slug's native valid-Unicode representation. The
   method cannot and must not reconstruct a regular/special kind distinction
   absent from the existing source value.
4. Repeated reads and template/read reuse of the same address use one prepared
   source value per invocation. At most 256 distinct Label/source addresses are
   admitted across path, template and read retry scratch.
5. When nested in `repository_ctx.file`, the terminal invocation appends the
   exact ASCII bytes to the existing generated-file plan. A need, missing
   package/file, directory, unreadable source, route/observation error, size or
   encoding rejection publishes no partial generated effect, print or dynamic
   environment from a speculative attempt.

Keep **Slug-native** the 2 MiB/256-address bounds, valid-Unicode representation,
diagnostic text, retry/sentinel transport, DICE equality cutoff and observation
carrier. This classification also includes successful reads when the existing
source producer accepted a special file, because `HostRepositorySourceFileValue`
retains the same bytes/logical-path `Present` variant for regular and special
files. Existing observation-backed invalidation may be stronger than Bazel's
untracked internal metadata checks, but external default-auto file edits,
deletion, kind and symlink changes must invalidate correctly.

Keep **unsupported/deferred**:

- String and `path` arguments, root/built-in labels, current generated-working-
  directory reads and arbitrary absolute or host paths;
- explicit `watch = "yes"`, `"no"` or `"auto"`, the `watch` method and exact
  workspace-rule read-event/log bytes;
- non-ASCII source bytes, files above 2 MiB, directory/absent/unreadable source
  success, exact Bazel diagnostics and Java raw-byte string behavior outside
  ASCII;
- write-after-read/self-generated behavior, Label forms not already accepted,
  package lookup widening, remote repository execution and mutable historical
  filesystem snapshots;
- any apple_support, Xcode, C++, toolchain, repository-name, OS or consumer
  special case; and
- every other repository method, generated BUILD evaluation, configured
  analysis/actions and the next authenticated replay boundary.

## Existing owners and implementation seam

Change only the synchronous repository context and its existing asynchronous
effect retry driver. Generalize the invocation-only prepared template-source
map/name into a bounded prepared canonical Label-source byte map shared by
`template` and `read`; do not add a second map. Add typed read argument,
source-need and size/ASCII failures to the invocation error algebra.

The `read` method must:

1. accept a direct `StarlarkLabel`, retain its typed
   `RepositoryLabelPathAddress`, and reject root before any source read;
2. issue the existing Label-path need until package/route/materialization state
   is prepared;
3. issue the shared source-byte need until exact routed bytes are prepared;
4. enforce the byte bound and ASCII subset, allocate a Starlark string, and
   return it without adding an effect.

After the evaluator, heap, `RefCell` borrows, builder and capture are dropped,
the outer loop resolves only the typed source need. Reuse and, if helpful,
generically rename the existing template source helpers. In legacy mode call
`HostRepositorySourceRoute::source_read_key`; in observed mode call
`source_read_observation_key` and merge the complete incoming epoch before
retry. Preserve `HostCanonicalRepositoryLoadRouteKey` and its observation
sibling, `HostRepositoryLabelPathKey` and its observation sibling,
`HostRepositorySourceObservationKey`/epoch key and
`HostRepositorySourceFileValue` as the sole semantic owners.

Do not reconstruct Labels from display strings, read the prepared physical
path, call `std::fs`, add a DICE key, change a route/materialization/source
value, or let invocation scratch enter retained effect identity. The existing
source key owns bytes and invalidation; the invocation map and returned string
are evaluator scratch; the terminal `GeneratedRepositoryFileEffectPlan` alone
owns any later destination/content/mode/order.

Overlapping requests continue to use immutable request projections and DICE
deduplication for the existing route/path/source keys. Observed requests merge
their own injected epochs; legacy and observed results must agree. No lock is
added or held across evaluation/compute, no task is detached, cancellation
remains with the current DICE computation, and all evaluator/prepared scratch
is released at completion or cancellation. There is no fallback or donor.

## Required proof

Add adjacent tests that discriminate:

- context reflection, direct-Label-only signature and string result;
- Label-path need before source need, exact repository-relative address,
  repeated-hit and template/read shared-source reuse without a second source
  observation;
- empty and representative ASCII bytes and byte-identical nested
  `file(read(...))` output/mode; the authenticated replay verifies the exact
  734-byte apple-support consumer source without adding a fixture;
- String, path, root, built-in, explicit-watch, non-ASCII and over-limit
  rejection without effects, plus explicit proof that an existing-owner
  special-file `Present` source follows the documented Slug-native success
  path rather than pretending to recover an unavailable kind bit;
- package absent/deleted/ignored before file access, then source absent,
  directory, unreadable and symlink/source changes;
- direct-local and immutable canonical routes, exact legacy/observed result
  parity, complete epoch merge, needs/cancellation, warm reuse and A/B/A
  restoration; and
- speculative print/effect/environment discard with only the terminal attempt
  published.

Reuse the existing repository-template route/observation harness and inline
test source bytes; add no fixture. The authentic apple_support replay is the
end-to-end provenance. Broader upstream read/watch shell tests are skipped
because they exercise explicitly deferred inputs and generated-working-
directory behavior. No benchmark is required: there is no new retained type,
key or demonstrated hot-path change, and the existing byte map remains bounded
invocation scratch.

## Allowlist, caps and complexity

Only these files may change in implementation:

- `app/slug_loading_v2/src/repository_rule_context.rs`; and
- `app/slug_loading_v2/src/module_extension_repository_file_effect.rs`.

Proof may change only adjacent `#[cfg(test)]` modules in those files. Do not
change Bzlmod/workspace owners, Cargo metadata, fixtures, source-route or
materialization types, repository definition/call/certificate shapes,
generated-plan representation, Label parsing or planning docs during the
implementation packet.

Gross additions are capped at 180 production Rust, 360 proof Rust and 540 Rust
total. Formatting and renames do not create headroom. No new helper may exceed
60 lines; the nested `read` method may not exceed 35 lines; no existing helper
other than the Starlark methods container may grow by more than 25 lines.

`module_extension_repository_file_effect.rs` is already above the 2,000-line
complexity trigger, and `repository_rule_context.rs` will approach/cross it.
They remain the bounded owners because splitting this one method would separate
the synchronous invocation error/prepared state from its evaluator or the
existing route/source retry from its effect key. Keep byte projection in one
small pure helper, reuse/generalize the existing mode-specific source helpers,
and add no third production responsibility.

## Validation and terminal stops

Run serially:

- focused repository-context read and repository-file-effect source tests;
- `cargo test -p slug_loading_v2 --lib -q` and every loading integration target
  touched by the private invocation signature;
- `cargo test -p slug_query_v2 --lib -q`;
- `cargo build -p slug_cli_v2 -q` before authentic replay;
- stale `slugd` cleanup before and after replay;
- `cargo fmt --check`, `git diff --check`, archive checker and exact
  allowlist/cap verification.

The authenticated replay must clear only this generic read call, create the
expected byte-identical generated `BUILD`, and stop at the next independently
owned boundary. Do not implement generated BUILD loading or that boundary.

Return `REPLAN` before or during Rust if package lookup cannot precede bytes;
the source requires a new key/direct filesystem access/new materialization
owner; legacy/observed epochs cannot be merged; a root/built-in/String/path or
explicit-watch input is needed; exact success requires non-ASCII raw-string
semantics; speculative state escapes; a retained route/source/effect type or
lock changes; the allowlist/caps/complexity bounds fail; or any consumer,
apple_support, Xcode, C++ or toolchain branch is required.

Architecture result: `ACCEPT`. Independent review should focus on the
two-stage package-then-source ordering, ASCII exactness boundary, shared
invocation scratch and the absence of a new semantic owner before implementation
begins.
