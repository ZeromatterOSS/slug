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
- `../bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/SingleExtensionEvalFunction.java`
  reads facts from extension metadata, writes them to the lockfile value, and
  deliberately does not use fact contents as an invalidation input.
- `zeromatter-kuro/bazel-external/rules_rs+override/rs/private/downloader.bzl`
  skips sparse-index fetches when a key is present in `mctx.facts`.

## Current Kuro Gap

- `app/kuro_bzlmod/src/lockfile.rs` already has a top-level `facts` field, but
  the extension execution path does not populate or consume it.
- `app/kuro_interpreter_for_build/src/module_ctx/context.rs` does not expose a
  `facts` field on `module_ctx`.
- `app/kuro_interpreter_for_build/src/module_ctx/methods.rs` has an
  `extension_metadata` stub that accepts kwargs but discards them.
- `app/kuro_bzlmod/src/extension_execution_dice.rs` caches generated repository
  specs, but does not pass prior facts into extension execution or persist newly
  returned facts.

## Implementation

1. Model module extension metadata

   Add a typed result for extension metadata, initially centered on `facts` but
   shaped so direct-dev-deps, direct-root-deps, reproducibility, and related
   Bazel metadata can be added without rewriting the execution API.

   Facts must be JSON-like values only. Validate and normalize at the Starlark
   boundary before storing them in the lockfile.

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

4. Persist facts

   After a successful extension execution, store returned facts at the top-level
   lockfile facts key for that extension id.

   Keep the existing `moduleExtensions` repository-spec cache. Facts complement
   that cache; they do not replace it.

5. Reuse facts on stale extension-cache paths

   On a valid repo-spec cache hit, Kuro can continue avoiding extension
   execution entirely.

   On cache miss or stale repo-spec cache, execute the extension with prior
   facts. This lets `rules_rs` skip repeated sparse registry fetches while still
   recomputing generated repositories when Bazel's normal invalidation inputs
   require it.

6. Add focused tests

   Add a lockfile roundtrip test proving `facts` survive load/save.

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
  invalidation inputs. Bazel intentionally does not diff fact contents before
  reusing the lockfile value.
- Facts need Starlark-to-JSON conversion parity. Reject arbitrary providers,
  artifacts, functions, or other non-JSON values at `extension_metadata`.
- Lockfile merge behavior can be deferred unless Kuro already has a lockfile
  merge workflow. The first implementation should store the exact returned facts
  for each extension id.

## Acceptance Criteria

- `hasattr(mctx, "facts")` is true for module extensions.
- `module_ctx.extension_metadata(facts = ...)` persists facts into
  `MODULE.bazel.lock`.
- A stale extension repo-spec cache re-executes with prior facts available.
- The zeromatter `rules_rs` build no longer refetches the same sparse registry
  metadata on the second run.
- No ad hoc URL-keyed repository cache is introduced for this behavior.
