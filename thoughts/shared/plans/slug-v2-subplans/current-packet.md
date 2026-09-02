# Current Slug V2 Packet

Packet: WP-5-7A-repository-context-path-audit

Milestone: M7A bootstrap-critical loading/repository execution closure. Audit
the complete Bazel 9.2 `repository_ctx.path` and path-value category exposed by
the accepted repository-rule `Label()` context, without treating a filesystem
path as a label-formatting detail.

Status: docs-only audit terminally `REPLAN`. A dedicated cross-stage label-path
owner design is required before Rust. No production, proof, fixture or oracle
file changed in this packet.

Immediate predecessor
`WP-4-5-7A-repository-rule-label-constructor-context-implementation-r1` is
terminally accepted in `77cfe94ce`. Its authenticated rules_rust 0.73.0 replay
clears `Label()` and stops at
`repository_ctx.path(Label(label))` in
`@@rules_cc+//cc/private/toolchain:lib_cc_configure.bzl:38`.

## Learned facts and research basis

Pinned Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` owns the category:

- `StarlarkBaseExternalContext.getPath` accepts a string, Label or existing
  `StarlarkPath`. A string is resolved against the repository working
  directory, a Label goes through `getPathFromLabel`, and a path is returned
  unchanged.
- `RepositoryUtils.getRootedPathFromLabel` first requests the Label package's
  `PackageLookupValue`. A missing package or package without BUILD metadata is
  an evaluation error. A successful lookup returns the package root joined
  with the Label path fragment; it does **not** inspect whether the named
  target path exists and does not resolve its symlinks.
- `getPathFromLabel` materializes a remote external repository when necessary.
  Bazel 9.2 defaults `--incompatible_no_implicit_watch_label=true`, so merely
  constructing the path does not watch the target. Package lookup and source
  materialization still remain semantic prerequisites.
- `StarlarkPath` is immutable and hashable. Equality and hashing use the
  underlying physical `Path`; `str` emits that path and `repr` quotes it.
  `basename`, `dirname` and `get_child` are lexical. `exists` and `is_dir`
  perform unwatched filesystem reads. `realpath` resolves symlinks, while
  `readdir` can add a watched directory-entry input according to its `watch`
  argument.
- `StarlarkRepositoryContext.symlink` converts both operands through the same
  path constructor, checks that the link is under the generated repository
  directory, creates it immediately, and on filesystems without native
  symlinks may watch the target. `template` similarly converts its output and
  source, reads source bytes, applies substitutions and writes immediately.
  These are effects, not path-constructor behavior.

The authenticated rules_cc 0.2.18 source confirms the first consumer shape.
`resolve_labels` constructs a dictionary of Label-backed paths specifically to
front-load Skyframe restarts. On Unix the caller requests nine `@rules_cc`
files, then immediately supplies two of those path values to
`repository_ctx.symlink`; later code stringifies host-tool paths and compares
`dirname` values. Windows additionally exercises `exists`, `get_child`,
`dirname`, `basename` and `readdir`. This is generic repository API use; no
rules_cc or toolchain branch is admissible.

Slug already owns most prerequisites, but not their required projection:

- `HostCanonicalRepositoryLoadRoute{,Observation}Key` and
  `HostRootRepositoryLoadRoute{,Observation}Key` select authenticated source
  routes. `RepositoryMaterializationResultKey` owns local or immutable source
  roots and immutable observation-instance identity.
- `ExternalRepositoryPackageLookup{,Observation}Key` and the root package
  lookup owners already implement BUILD/BUILD.bazel package existence,
  deleted-package and repository-ignore policy.
- `HostRepositoryPathKey` is **not** the Bazel Label-path operation. It asks the
  exact path resolver to inspect existence, expand symlinks and retain the
  resolved route. Reusing it would add target observations that Bazel 9.2 does
  not make and would return the real path rather than the lexical rooted path.
- Built-in `@bazel_tools` content is an immutable in-memory catalog with no
  physical materialization root. Root-workspace paths use a different package
  lookup owner. Neither case can be inferred from a loaded `.bzl` filename.
- Generated repository effects are currently planned before the materializer
  chooses a temporary root. Therefore string-relative path values cannot be
  assigned honest physical bytes during invocation, and the current file-only
  effect plan cannot encode symlinks or templates.

## Decision and compatibility classification

The audit returns `REPLAN`; it does not authorize Rust.

A later bounded slice may classify as **exact**:

1. `repository_ctx.path(Label)` routes the canonical Label to its selected
   source repository, requires the Label's package to exist, and returns the
   lexical package-root-plus-label-fragment path without testing or resolving
   the target.
2. With Bazel 9.2 default semantics, construction adds no observation of the
   target path. Source-route, materialization and package-lookup dependencies
   remain owned and invalidating.
3. The resulting repository path is immutable and hashable. Equality compares
   concrete path identity, not Label spelling, mapping provenance or the
   target's contents.
4. An unresolved route, package or materialization restarts through the normal
   DICE/need path; malformed or absent packages fail before an effect is
   published.

Keep **Slug-native** physical generated/materialization directory bytes,
native-Unicode path representation, diagnostics, retry count, evaluator error
transport, observation carrier representation and DICE cutoff mechanics.
Exact Bazel output-base or Java VFS path bytes are not claimed.

Keep **unsupported/deferred** until separately designed and admitted:

- `repository_ctx.path` string and existing-path inputs, generated-repository
  working-directory identity, absolute/relative normalization and cross-origin
  path equality;
- built-in `@bazel_tools` Label paths before an immutable physical catalog
  materialization owner exists;
- `basename`, `dirname`, `get_child`, `exists`, `is_dir`, `realpath`, `readdir`
  and their host-observation/watch/error semantics;
- `symlink`, `template`, `read`, `watch`, `watch_tree`, `execute`, `which`,
  download/extract/patch/delete/rename and every other repository effect;
- non-Starlark/native repository rules, module-extension path values, remote
  repository execution, exact generated-repository layout, lockfile changes
  and configured analysis/actions.

This boundary is intentionally narrower than the complete Bazel path type. It
does not imply that returning an opaque Label wrapper or a formatted Label
would be compatible: an admitted value must be backed by the selected package
root and materialization identity.

## Required successor architecture

Select docs-only
`WP-2-4-5-7A-repository-label-path-owner-design-r1`. It must converge all of
the following before implementation:

1. Add one public, narrow Bzlmod projection for a source route plus canonical
   package/target to a lexical materialized path. It must reuse the existing
   materialization and package-lookup owners and must not call
   `HostRepositoryPathKey`, inspect the target, resolve a symlink or perform
   direct filesystem I/O.
2. Give root-workspace labels an equivalent package-lookup projection without
   loading/evaluating the BUILD file. Decide built-in catalog disposition
   explicitly; fail closed rather than inventing a root.
3. Let `HostSelectedRepositoryFileEffectKey` drive synchronous Starlark
   evaluation in bounded retries. An unresolved Label path may be captured as
   an invocation demand; the evaluator and all invocation `RefCell` borrows
   must be dropped before the outer key awaits route/package/materialization
   DICE computations. Prepared paths are invocation-local scratch only.
4. Retain the path's normalized physical bytes and observation namespace in a
   Starlark path value, but compare path equality by physical path only. Do not
   retain caller mapping provenance, source bytes or a second Label identity.
5. Prove A/B/A route, package-marker and materialization-root restoration,
   need/retry/cancellation behavior, missing-package versus missing-target
   discrimination, no target observation, and exact DICE equality/cutoff.

The successor must decide whether a single-demand retry is sufficient or a
compact prepared-path map is justified. The Buck2 utility-reuse audit prefers
the existing `SmallMap`/`SmallSet`, `Arc`, `CompactString`, `Dupe`,
`Allocative`, `NormalizedAbsolutePath`, `PathObservationNamespace`,
`HostRepositorySourceRoute` and canonical Label/package types. No new
interner, global cache, registry, path parser or retained unbounded collection
is justified. Any new retained key/value shape requires an explicit Stage 9
decision, even when the answer is reuse with no extraction row.

The DICE ownership audit rejects a lock, evaluator borrow or invocation-state
borrow across any compute. It also rejects direct host reads from a Starlark
method and reconstruction of a materialization root from
`BzlModuleIdentity.workspace_path`.

## Evidence and proof contract for the successor

Reuse the pinned Bazel sources and tests:

- `StarlarkBaseExternalContext.getPath` and `getPathFromLabel`;
- `RepositoryUtils.getRootedPathFromLabel`;
- `StarlarkPath` and `StarlarkPathTest`;
- `StarlarkRepositoryContextTest`'s implicit-watch flag cases; and
- the rules_cc `resolve_labels` call sites above.

The design must require focused Rust proof for external immutable, direct
local and root-workspace routes; package present/absent; target present/absent
with identical construction; lexical symlink non-resolution; path
hash/equality/stringification; route/materialization revisions; and observed
versus legacy parity. An authentic replay is a discriminator only after all
focused and owning-crate suites pass.

No new checked-in Bazel fixture is selected by this audit. If source evidence
cannot discriminate a proposed edge, the successor must first add an isolated
pinned Bazel 9.2 oracle decision and then freeze its own allowlist/caps.

## Audit allowlist, validation and stops

This docs-only packet may change only:

- the canonical plan;
- Stage 4;
- Stage 5; and
- this current-packet manifest.

Validate `git diff --check`, current-link consistency, archive-baseline
integrity and a clean status after commit. Cargo, CLI rebuild and authentic
replay are unnecessary because no Rust, fixture or executable input changes.

Return `REPLAN` from the successor if it:

- reuses `HostRepositoryPathKey` or otherwise observes/resolves the target;
- infers a repository root from a `.bzl` filename, output-base convention or
  generated repository name;
- loads a BUILD file merely to prove package existence;
- holds a lock, evaluator, heap or `RefCell` borrow across DICE computation;
- stores retry demands in a process global, injected frontier or published
  repository effect;
- admits string/generated-root paths, built-in paths, filesystem path methods
  or repository effects without their full owner and observation semantics;
- changes the existing Label grammar/mapping owner, repository-rule
  definition/call/certificate identity or file-effect plan; or
- adds a rules_cc, rules_rust, toolchain, repository-name or platform special
  case.

Audit result: `REPLAN`. The Bazel category is implementable only after a new
narrow lexical label-path projection and a lock-safe evaluator/DICE retry
contract are independently frozen. The current file-only effect model and
existing resolved-path key are not compatible substitutes.
