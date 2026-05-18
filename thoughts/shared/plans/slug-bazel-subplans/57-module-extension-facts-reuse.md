# Plan 57: Module extension facts reuse

## Goal

Reuse stored module extension facts so re-executed extensions can avoid repeating
expensive external metadata fetches, especially the sparse registry lookups used by
`rules_rs`.

This is a Bazel 9 parity feature, not a new URL-keyed repository cache. Bazel
allows checksum-less metadata downloads, and `rules_rs` relies on
`module_ctx.facts` plus `module_ctx.extension_metadata(facts = ...)` to make those
downloads reproducible across extension evaluations.

## Source of truth

- `../bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleExtensionContext.java`
  exposes `module_ctx.facts` as a Starlark struct field and accepts
  `facts` in `extension_metadata(...)`.
- `../bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/Facts.java`
  defines the supported JSON-like facts shape and validation.
- `../bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/SingleExtensionEvalFunction.java`
  reads facts from workspace/hidden lockfiles, writes facts in update/refresh
  modes, validates changed facts in error mode after execution, and deliberately
  does not use fact contents as a normal replay invalidation input.
- `zeromatter-slug/bazel-external/rules_rs+override/rs/private/downloader.bzl`
  skips sparse-index fetches when a key is present in `mctx.facts`.

## Current Slug Gap

2026-05-18 status: this plan is partly implemented. Slug can read top-level
lockfile facts, pass prior facts into extension execution, expose
`module_ctx.facts`, and accept/return `extension_metadata(facts = ...)`.

The remaining gaps are propagation and persistence policy. Returned metadata is
not written back to `MODULE.bazel.lock` during ordinary builds, which follows
Plan 57's Slug safety policy rather than Bazel's update/refresh write behavior.
Returned metadata also needs to remain available in a DICE value
(`ModuleExtensionResult` or a sibling value) so a future explicit lockfile
update command can persist it. Future persistence must happen through an
explicit Bazel-parity lockfile update command or a separate Slug-owned cache,
not lazy build-time mutation of the Bazel-owned lockfile.

## Implementation

1. Model module extension metadata

   Add a typed result for extension metadata, initially centered on `facts` but
   shaped so direct-dev-deps, direct-root-deps, reproducibility, and related
   Bazel metadata can be added without rewriting the execution API.

   Facts must be JSON-like values only. Validate and normalize at the Starlark
   boundary using Bazel `Facts.java` semantics before storing them in any DICE
   value or lockfile update.

2. Pass prior facts into module contexts

   Load `lockfile.facts[extension_id]` before executing an extension and pass it
   through the Bzlmod executor into module context construction.

   Expose the value as `mctx.facts`. For missing facts, expose an empty mapping
   compatible with Bazel/rules_rs usage rather than omitting the field.

3. Capture `extension_metadata(facts = ...)`

   Replace the no-op `extension_metadata` implementation with one that returns
   typed metadata or records it in extension execution state.

   The executor must capture the return value from the module extension
   implementation. If the implementation returns `None`, preserve current
   behavior and treat it as empty metadata.

4. Persist facts through an explicit update path

   After a successful extension execution in an explicit future lockfile update
   command, store returned facts at the top-level lockfile facts key for that
   extension id. Ordinary `slug build` must not mutate `MODULE.bazel.lock`.

   Keep the existing `moduleExtensions` repository-spec cache. Facts complement
   that cache; they do not replace it.

5. Reuse facts on stale extension-cache paths

   On a valid repo-spec cache hit, Slug can continue avoiding extension
   execution entirely.

   On cache miss or stale repo-spec cache, execute the extension with prior
   facts. This lets `rules_rs` skip repeated sparse registry fetches while still
   recomputing generated repositories when Bazel's normal invalidation inputs
   require it.

6. Add focused tests

   Add a lockfile roundtrip test proving `facts` survive load/save for the
   explicit lockfile-update path.

   Add a small Starlark extension test where the first execution returns facts
   and the second execution observes them via `mctx.facts`.

   Add a repository-fetch sentinel test shaped like `rules_rs`: call a download
   only when a fact key is absent, then assert the second execution does not call
   the download path.

   Validate against the zeromatter workspace in the distrobox by building twice and
   confirming the second run does not repeat `curl https://index.crates.io/...`
   sparse-index fetches for already-recorded facts.

## Risks

- Facts come from a previous extension execution and must not be treated as
  normal replay invalidation inputs. Bazel intentionally does not diff fact
  contents before reusing the lockfile value, but in `--lockfile_mode=error` it
  validates newly returned facts against the workspace lockfile after execution.
- Facts need Starlark-to-JSON conversion parity. Reject arbitrary providers,
  artifacts, functions, or other non-JSON values at `extension_metadata`.
- Lockfile merge/write behavior belongs to an explicit lockfile-update workflow.
  Ordinary build/test/query/audit commands should read facts but leave
  `MODULE.bazel.lock` byte-for-byte unchanged.

## Acceptance Criteria

- `hasattr(mctx, "facts")` is true for module extensions.
- `module_ctx.extension_metadata(facts = ...)` returns/captures facts without
  mutating `MODULE.bazel.lock` during ordinary builds.
- An explicit future lockfile-update command can persist facts into
  `MODULE.bazel.lock` with Bazel-shaped serialization.
- A stale extension repo-spec cache re-executes with prior facts available.
- The zeromatter `rules_rs` build no longer refetches the same sparse registry
  metadata on the second run.
- No ad hoc URL-keyed repository cache is introduced for this behavior.
