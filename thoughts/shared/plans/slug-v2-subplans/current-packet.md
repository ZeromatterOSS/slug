# Current Slug V2 Packet

Packet: `WP-5-m1-external-repository-source-identity`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: implementation worker
Evidence: accepted independent retained-representation/DICE review of the
Host-only source result; the existing Host key is sole route-keyed,
complete-only owner, while legacy immutable source values retain bytes-only
equal-byte pruning.

Implement the accepted source-identity boundary only. Change
`HostRepositorySourceFileKey` to return public
`HostRepositorySourceFileValue::{Present { bytes: Arc<[u8]>, logical_path:
NormalizedAbsolutePath }, Absent}`. `logical_path` is the normalized requested
path submitted to `ResolvedPathKey` before resolution, may itself be a symlink,
and is not `real_path`, namespace, symlink provenance, source identity,
generation, or observation-instance state. Preserve it through the Host mapper
and use the existing byte Arc; legacy `RepositorySourceFileValue` remains
exactly `Present(bytes)` / `Absent`.

The shared resolver helper may use one private transient result to transport
the requested path: the Host mapper retains it and the legacy mapper strips it.
Do not add a second observation, DICE key, cache, lock, filesystem bypass, or
hashing/interner utility. Host currently supports direct local overrides only.
Immutable generation and observation-instance recomputation remains solely on
the legacy key: equal bytes retain the previous complete value, changed bytes
replace it. `Need` remains non-valid for both owners. Host value equality
requires equal bytes and logical path. A local-override root change is a
distinct `RootRepositoryRoute` key, not a same-key DICE replacement.

The production allowlist is exact:

- `app/slug_bzlmod_v2/src/source_preparation.rs`;
- `app/slug_bzlmod_v2/src/lib.rs`, only to re-export
  `HostRepositorySourceFileValue`; and
- `app/slug_loading_v2/src/bzl_module.rs`, only for the direct
  `HostRepositorySourceFileKey` import and `Present { bytes, .. }` / `Absent`
  migration at the existing repository package load boundary.

No loading test edits are authorized. In
`app/slug_bzlmod_v2/src/source_preparation.rs`, extend
`host_repository_source_requests_native_materialization_without_legacy_snapshot_keys`
and `immutable_materialization_equality_is_operationally_exact`; add exactly:

- `host_repository_source_value_retains_requested_logical_path_and_bytes`;
- `host_repository_source_value_equality_requires_equal_bytes_and_logical_path`;
- `host_repository_source_value_need_and_error_have_no_logical_path`;
- `legacy_immutable_repository_source_value_remains_bytes_only`; and
- `host_repository_source_local_override_root_change_is_distinct_key`.

Run serially, after cleaning stale `slugd` before and after any daemon-sensitive
smoke (none is expected here):

1. `cargo test -p slug_bzlmod_v2 source_preparation`
2. `cargo test -p slug_loading_v2 host_package_load`
3. `cargo check -p slug_loading_v2`
4. `cargo check -p slug_query_v2`
5. `cargo test -p slug_bzlmod_v2 --target x86_64-pc-windows-gnu --no-run`
6. `cargo test -p slug_loading_v2 --target x86_64-pc-windows-gnu --no-run`
7. `cargo fmt --check`
8. `scripts/v2_archive_status.sh`
9. `git diff --check`

Stop with `REPLAN` if implementation requires retaining `real_path`, namespace,
symlink route, physical materialization root, source identity, generation, or
observation instance as semantic state; a second source owner, observation,
cache/lock, filesystem bypass, public `BzlModuleIdentity` change, external Bzl
key/loader activation beyond the existing package boundary, query behavior, or
any oracle fixture/protocol/CLI/Cargo-metadata/cycle-detector/materialization-
owner change. Root-package source behavior, Starlark loads/rules, test-base
closure, visibility content evaluation, registry/discovery work,
configuration, analysis/actions/execution, repository rules/extensions,
JVM/Java bytecode, and Bazel delegation remain out of scope.
