# Plan 57: Build-Time MODULE.bazel.lock Read-Only Parity

> Parent: [15-bazel-9-parity.md](15-bazel-9-parity.md)
>
> Per `AGENTS.md`, Slug targets Bazel 9 parity only. The Bazel lockfile format
> is a source-of-truth compatibility surface, not a Slug-private cache format.

## Status

In progress.

## Current Blocker

A fresh `bazel mod deps` in `C:\dev\zeromatter-kuro` regenerated
`MODULE.bazel.lock` with SHA256:

```text
E2985FF577A3F7ED1B31B873D84A8B9A7CE452A80CCE85673F853A328F3507DB
```

Starting a Slug `//sdk:sdk` build mutated that same file within a few minutes:

```text
58273E0D359026FC94A98AC9E716F112252FAB104FFEEC1CC36B5C32320824EF
```

The file grew from `1,623,825` bytes to `7,441,941` bytes. A JSON summary of
the two files shows the systemic failure:

```text
Bazel lockfile: 12 moduleExtensions, 3 facts, 363 registryFileHashes
Slug-mutated:   36 moduleExtensions, 3 facts, 363 registryFileHashes
```

The largest Slug-added entries are broad extension caches, including
`@@rules_rs+//rs:extensions.bzl%crate` with 1367 generated repo specs and
`@@rules_rust+//rust:extensions.bzl%rust` with 302 generated repo specs. Slug
also rewrites some existing Bazel-authored `repoRuleId` and label spellings
when it serializes the lockfile back out.

## Classification

This is a Plan 15 Bazel 9 lockfile parity issue, not an SDK target workaround.
The owning abstraction is `slug_bzlmod` lockfile/extension execution policy.

Slug currently treats `MODULE.bazel.lock` as both:

- a Bazel-compatible input used for startup-time extension repo pre-seeding and
  extension cache hits; and
- a Slug-private extension cache output written during ordinary builds.

Those roles conflict. Even when Slug can deserialize Bazel's file, writing it
back through Slug's model is not semantics-preserving: it expands the file with
Slug-observed extension results and can canonicalize fields differently from
Bazel. Ordinary `slug build` must not mutate the Bazel-owned lockfile.

## Systemic Fix

Make ordinary build-time Slug lockfile access read-only:

- Continue reading `MODULE.bazel.lock` for Bazel-authored extension caches,
  facts, registry hashes, and startup-time spoke pre-seeding.
- Do not create, rewrite, append to, or normalize `MODULE.bazel.lock` from
  `slug build` or lazy module extension evaluation.
- Keep any future Slug-specific extension-result persistence out of
  `MODULE.bazel.lock` unless a separate explicit command implements exact
  Bazel lockfile write parity.

This rejects the one-off workaround of deleting `MODULE.bazel.lock` before Slug
or post-restoring it after Slug. The build engine itself must be read-only with
respect to this file.

## Verification

Required before continuing SDK parity:

- Focused unit coverage in `slug_bzlmod` proving module extension evaluation can
  read a lockfile project root without writing a lockfile update.
- `cargo test -p slug_bzlmod <focused-lockfile-test> -- --nocapture`
- `cargo build -p slug -j 1`
- Regenerate `MODULE.bazel.lock` with Bazel, record SHA256, run Slug smoke, then
  verify the SHA256 is unchanged after Slug exits or is interrupted.

