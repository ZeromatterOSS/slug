# Plan 57: Build-Time MODULE.bazel.lock Read-Only Safety Policy

> Parent: [15-bazel-9-parity.md](15-bazel-9-parity.md)
>
> Per `AGENTS.md`, Slug targets Bazel 9 parity only. The Bazel lockfile format
> is a source-of-truth compatibility surface, not a Slug-private cache format.

## Status

Code-level implemented; external SHA smoke verification still pending.

## Original Blocker

A fresh `bazel mod deps` in `C:\dev\zeromatter-kuro` regenerated
`MODULE.bazel.lock` with SHA256:

```text
E2985FF577A3F7ED1B31B873D84A8B9A7CE452A80CCE85673F853A328F3507DB
```

Starting a Slug `//sdk:sdk` build previously mutated that same file within a few minutes:

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

This is a Plan 15 Bazel 9 lockfile compatibility issue, not an SDK target
workaround. The owning abstraction is `slug_bzlmod` lockfile/extension
execution policy.

At the time this plan was written, Slug treated `MODULE.bazel.lock` as both:

- a Bazel-compatible input used for startup-time extension repo pre-seeding and
  extension cache hits; and
- a Slug-private extension cache output written during ordinary builds.

Those roles conflict. Even when Slug can deserialize Bazel's file, writing it
back through Slug's model is not semantics-preserving: it expands the file with
Slug-observed extension results and can canonicalize fields differently from
Bazel.

This read-only policy is an interim Slug safety policy, not a direct Bazel
parity claim. Bazel itself may update `MODULE.bazel.lock` in update/refresh
lockfile modes (`RepositoryOptions.java`, `BazelLockFileModule.java`,
`SingleExtensionEvalFunction.java`). Slug ordinary build/query/audit paths stay
read-only until Slug has an explicit Bazel-parity lockfile update command.

2026-05-18 update: ordinary resolution/extension execution code now documents
and tests read-only behavior. Lockfile writer APIs still exist for tests and
future explicit `slug mod update`-style commands; they must not be called from
ordinary build/query/audit paths.

## Systemic Fix

Make ordinary build-time Slug lockfile access read-only until exact
mode-aware lockfile writes are implemented:

- Continue reading `MODULE.bazel.lock` for Bazel-authored extension caches,
  facts, registry hashes, and startup-time spoke pre-seeding.
- Do not create, rewrite, append to, or normalize `MODULE.bazel.lock` from
  `slug build` or lazy module extension evaluation.
- Keep any future Slug-specific extension-result persistence out of
  `MODULE.bazel.lock` unless a separate explicit command implements exact
  Bazel lockfile write parity.
- Plan 61 owns the mode-aware target shape: default/update, refresh, error, and
  off must be validated against pinned Bazel before Slug claims parity.

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
