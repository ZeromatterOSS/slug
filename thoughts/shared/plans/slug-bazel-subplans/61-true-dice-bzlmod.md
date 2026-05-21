# Plan 61: True DICE-Owned Bzlmod

> Parent: [Slug Bazel-Compatible Build Tool](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Created: 2026-05-18

## Status

In progress overall. Phase 61.1 guardrails started 2026-05-18 and now cover the
observable bzlmod replay/materialization bug shapes that blocked the current SDK
parity loop. The current guardrail file has 27 passing tests and no xfails. The
broader DICE-owned bzlmod plan is not complete until the acceptance criteria
below are satisfied or a real blocker is recorded.

Current SDK parity checkpoint 2026-05-20: complete under the user-approved
output-root exception. Slug builds `//sdk:sdk_contents`; directory/file
manifests and modes match Bazel 9 exactly; all non-ELF file hashes match; the
only remaining byte differences are the four ELF outputs `bin/zm`,
`bin/zerobuf`, `bin/zerosystem`, and `lib/libzeromatter_ffi.so`. Those four
differences are accepted for this checkpoint because the demonstrated remaining
class is output-root strings embedded in ELF/debug/build metadata
(`buck-out`/future `slug-out` versus Bazel's `bazel-out`). Exact-byte parity
remains a follow-up design item: add a Bazel-grounded optional output-root mode
that stores or exposes generated artifacts under `bazel-out` instead of
post-link string rewriting.

Current evidence:

- Slug full SDK build log:
  `/tmp/slug-plan61/plan61-sdk-contents-after-cc-all-files-20260520-150740.log`.
- Bazel/Slug mode manifests:
  `/tmp/slug-plan61/bazel-sdk-contents-modes-after-cc-all-files.txt` and
  `/tmp/slug-plan61/slug-sdk-contents-modes-after-cc-all-files.txt`.
- Bazel/Slug SHA manifests:
  `/tmp/slug-plan61/bazel-sdk-contents-sha-after-cc-all-files.txt` and
  `/tmp/slug-plan61/slug-sdk-contents-sha-after-cc-all-files.txt`.
- `/var/mnt/dev/zeromatter-kuro/execroot` was removed after validation;
  `/var/mnt/dev/zeromatter-kuro/buck-out` was retained as the current evidence
  tree and was about 21G at the last check.

SDK parity loop slice 2026-05-19 advanced the frontier from lockfile/repo
materialization failures to full execution:

- Restored ZeroMatter's dirty invalid `MODULE.bazel.lock` from HEAD after
  backing up the bad file to `/tmp/zeromatter-MODULE.bazel.lock.bad-20260519-135335`.
  Bazel 9.0.1 then passed `bazel build --nobuild //sdk:sdk_contents`
  (`/tmp/slug-plan61/bazel-sdk-contents-restored-lock-20260519-135345.log`).
- Fixed Slug bzlmod analysis/materialization issues discovered by the SDK
  smoke: scoped `use_repo` aliases now retain their declaring module,
  double-plus extension-owner alias lookup resolves, unique same-name provider
  lookup handles the Rust `StdLibInfo` identity split, extension repo execution
  writes spec-hash completion markers, `--unstable-no-execution` always uses
  the dry-run executor, `repository_ctx.patch` label paths anchor at the
  workspace root and fall back past GNU `patch` create-file mismatches, dry-run
  directory outputs materialize as directories, and host LLVM toolchain
  discovery recognizes Bazel 9 double-plus `llvm++http_archive+...` repos.
- Slug now passes the SDK analysis/materialization smoke:
  `/var/mnt/dev/slug/target/debug/slug --isolation-dir plan61-sdk-contents-noexec-drydir-20260519-145242 build --unstable-no-execution //sdk:sdk_contents`
  succeeded with 3,765 dry-run commands
  (`/tmp/slug-plan61/slug-sdk-contents-noexec-drydir-20260519-145242.log`).
- Full execution now reaches Rust execution and stops at a new hard blocker:
  `crates__const_format-0.2.35//:const_format` fails with
  `error[E0463]: can't find crate for const_format_proc_macros` while rustc is
  passed
  `--extern=const_format_proc_macros=.../libconst_format_proc_macros-1197439997.so`.
  The referenced `.so` exists, is executable, and exports
  `__rustc_proc_macro_decls_*` plus `rust_metadata_const_format_proc_macros_*`.
  Evidence log:
  `/tmp/slug-plan61/slug-sdk-contents-normal-clang2-20260519-150339.log`.

Blocker reflection 2026-05-19, Rust proc-macro execution semantics:

- Class boundary: this is not an SDK-specific missing file and not a C/C++
  toolchain frontier. It is a Rust action execution/modeling bug: rustc is
  asked to load a host proc-macro dynamic library via `--extern`, but the full
  Slug execution environment does not yet prove that every rustc-loadable
  artifact is a declared, materialized action input in the correct host/exec
  configuration and at a path valid from the action execroot.
- Bazel semantic: proc macros are execution-time compiler plugins. A consumer
  Rust action may target `x86_64-unknown-linux-musl`, but the proc macro must be
  built for and loaded by the host/exec rustc. The owning Slug abstractions are
  plugin propagation/configuration, `ctx.actions.run` input tracking, command
  line artifact rendering, and the local executor's per-action execroot.
- Other affected cases: any `rules_rust`/`rules_rs` crate graph with
  transitive proc macros, build-script produced Rust helpers loaded by rustc,
  cross-target Rust builds that mix target rlibs with host proc macros, and any
  action whose tool loads a dependency named only by a rendered argv path.
- Rejected workarounds: copying the observed `.so`, broadening the execroot to
  hide undeclared inputs, hardcoding `const_format_proc_macros`, hardcoding a
  `rules_rs++crate+...` path, or adding a path remap for the observed isolation
  directory/configuration hash. The systemic fix must make rustc-loadable
  artifacts first-class inputs and validate/rewrite their paths at the owning
  abstraction.

Blocker reflection 2026-05-19, git repository materialization integrity:

- Fresh post-cleanup SDK smoke
  `/tmp/slug-plan61/plan61-sdk-rust-repro-20260519-153221.log` failed before
  Rust execution while materializing
  `rules_rs++crate+crates__ts-rs-12.0.1`. The Starlark
  `crate_git_repository` implementation tried to fan out from the master
  `rules_rs++crate+https___github.com_Aleph-Alpha_ts-rs.git_a6bbbd...`
  clone and git reported `fatal: not a git repository: .../.git`. Bazel 9.0.1
  still passes `bazel build --nobuild //sdk:sdk_contents`
  (`/tmp/slug-plan61/bazel-sdk-after-git-clean-20260519-153414.log`).
- Class boundary: this is not a `ts-rs` target bug. It is a repository
  materialization integrity bug: Slug accepts a `.slug_repo_complete` marker
  without validating the layout invariants required by the repo rule result.
- Bazel semantic / Slug owner: repository fetch/materialization must reject or
  repair stale/corrupted output directories. For git repository rules, the
  materialized repository must still contain the `.git` state that downstream
  repository rules use for worktree fan-out. The owning Slug abstractions are
  `ExtensionRepoExecutionKey` marker-hit handling and the native
  `repository_executor` marker check.
- Other affected cases: any `git_repository` / `new_git_repository` repo whose
  working tree is partially deleted while its completion marker remains, and
  any downstream repo rule that uses the upstream clone as a git source.
- Rejected workarounds: manually restoring this one `.git` directory, deleting
  only the observed `ts-rs` repo, special-casing the Aleph-Alpha URL, or making
  `crate_git_repository` ignore git fan-out failures. The systemic fix is to
  validate git repo layout before treating a completion marker as a hit, then
  force a clean re-materialization on invalid layout.

Blocker reflection 2026-05-19, local repository source layout integrity:

- Patched SDK smoke
  `/tmp/slug-plan61/plan61-sdk-git-layout-fix-20260519-154850.log` advanced
  past the git repository failure and stopped in LLVM libcxx analysis:
  `llvm++llvm_source+libcxx//:src/filesystem/time_utils.h` was an unknown
  target. Inspection showed the materialized repo contained only
  `.slug_repo_complete`, `BUILD.bazel`, and `WORKSPACE.bazel`, while the source
  file existed under the source repository
  `llvm++llvm_source+llvm-raw/libcxx/src/filesystem/time_utils.h`.
  Deleting only the generated `llvm++llvm_source+libcxx` repo and rerunning a
  focused `targets` lookup caused current Slug to rematerialize the expected
  top-level symlinks and resolve the source label.
- Class boundary: this is another repository materialization integrity bug, not
  an LLVM target bug and not an implicit source-file target bug. Slug trusted a
  stale completion marker for a `new_local_repository`-style repo after the
  source-tree symlink layout had been lost.
- Bazel semantic / Slug owner: local repository rules expose the local source
  tree plus generated BUILD metadata as the repository contents. A completion
  marker is valid only if the materialized repo still reflects the local source
  directory entries that downstream package loading and `glob()` depend on. The
  owning Slug abstractions are native repository execution and
  `ExtensionRepoExecutionKey` marker-hit handling.
- Other affected cases: any `local_repository` or `new_local_repository` whose
  top-level source symlinks/files are partially deleted while its completion
  marker remains, especially large overlay repos such as LLVM where generated
  BUILD files reference source-tree paths.
- Rejected workarounds: deleting only the observed `libcxx` repo, special-casing
  LLVM/libcxx, synthesizing missing source targets, or teaching analysis to
  ignore absent source files. The systemic fix is to validate local repository
  source layout against the declared source path before accepting a marker hit.

Implementation update 2026-05-19:

- Added marker layout validation before trusting native repository execution,
  extension repo execution, extension repo file access, and eager extension
  materialization fast paths. The validator now rejects missing `.git` state for
  git repos, missing top-level source entries for local repos, and LLVM
  `_llvm_subproject_repository` outputs whose sibling `llvm-raw/<dir>` source
  tree is not reflected in the materialized repo.
- Focused tests pass:
  `git_repository_marker_requires_git_layout`,
  `new_local_repository_marker_requires_source_layout`, and
  `llvm_subproject_marker_requires_source_layout`.
- Focused stale-layout repro passed with patched Slug:
  `/tmp/slug-plan61/plan61-llvm-layout-fix2-20260519-160556.log`.
  The run started from a `llvm++llvm_source+libcxx` repo containing only
  `.slug_repo_complete`, `BUILD.bazel`, and `WORKSPACE.bazel`; Slug
  rematerialized the symlinked source tree and resolved
  `llvm++llvm_source+libcxx//:src/filesystem/time_utils.h`.

Implementation update 2026-05-20, module compatibility-level conflicts:

- Bazel ground truth: Bazel 9 `Selection.java` rejects selected modules with
  the same module name and different `compatibility_level` unless
  `multiple_version_override` explicitly permits multiple selected versions.
  Local source anchor:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/Selection.java:436`.
- Previous Slug behavior logged the conflict and selected the highest version,
  which could mask a Bazel `VERSION_RESOLUTION_ERROR`. Slug now fails MVS
  resolution for this class and only permits the split when a matching
  `multiple_version_override` is present.
- Validation:
  `cargo test -p slug_bzlmod compatibility_conflicts -- --nocapture`,
  `cargo check -p slug_bzlmod -p slug_common`, `cargo fmt --check`,
  `cargo build -p slug`, and
  `cargo test -p slug_bzlmod resolution -- --nocapture` all pass.

Blocker reflection 2026-05-20, `module(compatibility_level)` is no longer a
source of compatibility-level facts in Bazel 9:

- Bazel ground truth: Bazel 9 `ModuleFileGlobals.module()` accepts the
  deprecated `compatibility_level` attribute but stores compatibility level `0`
  unconditionally. Local source anchor:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileGlobals.java:188`.
- The prior compatibility-conflict fix remains useful for any resolver input
  that Bazel actually models with nonzero compatibility levels, but Slug must
  not synthesize those nonzero levels from a deprecated no-op `module()`
  attribute. Doing so would let Slug fail MODULE files that Bazel 9 only warns
  about.
- Slug now parses and accepts `module(compatibility_level = N)` while storing
  `0`, matching Bazel 9. Validation:
  `cargo test -p slug_bzlmod compatibility_level -- --nocapture`,
  `cargo test -p slug_bzlmod parser::tests -- --nocapture`,
  `cargo test -p slug_bzlmod resolution -- --nocapture`, and
  `cargo fmt --check` all pass.

Implementation update 2026-05-20, selected registry source failures are direct
resolution failures:

- Bazel ground truth: Bazel 9 computes `RepoSpecValue` for every selected
  registry module during `BazelModuleResolutionFunction`; `RepoSpecFunction`
  throws when it cannot fetch source/registry repo-spec data. Local anchors:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelModuleResolutionFunction.java:113`
  and
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/RepoSpecFunction.java:59`.
- Previous Slug behavior warned on `fetch_sources()` failure and continued to
  register a partially broken cell graph with empty source paths. That can mask
  a Bazel resolution failure until a later, less-informative cell/file access.
- Slug legacy bzlmod setup now propagates selected module source fetch failures
  with root-module context. This is still a transition toward
  `ModuleSourceKey` / `RepoSpecFunction`-shaped DICE ownership, but it removes
  the masking behavior at the current legacy boundary.
- Validation: `cargo check -p slug_common -p slug_bzlmod` and
  `cargo fmt --check` pass.

Implementation update 2026-05-20, registry file hash propagation:

- Bazel ground truth: `ModuleFileValue` carries hashes for registry files used
  to obtain module files, `ModuleFileFunction` collects those registry download
  events, `RepoSpecFunction` returns additional hashes for repo-spec/source
  files, and `BazelModuleResolutionFunction` merges them into the resolution
  value. Local anchors:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileValue.java:42`,
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileFunction.java:203`,
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/RepoSpecFunction.java:59`,
  and
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelModuleResolutionFunction.java:113`.
- Slug registry fetch APIs now have hash-returning variants for
  `MODULE.bazel` and `source.json` registry files. `ResolvedGraph` carries the
  collected URL-to-SRI-SHA256 map, and the legacy bzlmod session bridge carries
  those hashes forward while final `ModuleSourceKey` / `BzlmodResolutionKey`
  ownership is still pending.
- This does not write `MODULE.bazel.lock` on ordinary build/query paths. It
  makes the Bazel-shaped input facts observable at the current transition
  boundary so the future lockfile/update path can consume them without
  recomputing from side effects.
- Validation:
  `cargo test -p slug_bzlmod registry -- --nocapture`,
  `cargo test -p slug_bzlmod resolution -- --nocapture`,
  `cargo check -p slug_bzlmod -p slug_common`, `cargo fmt --check`, and
  `cargo build -p slug` all pass.

Implementation update 2026-05-20, non-registry override failures fail at
resolution:

- Bazel ground truth: non-registry override module files/sources are resolved
  through Bazel's module-file/repository-directory path; a missing or
  unreadable override source is a module resolution error, not a warning that
  falls back to the registry. This follows the same `ModuleFileFunction` /
  `RepoSpecFunction` failure boundary used for selected module sources.
- Previous Slug MVS discovery warned and skipped a failing local/git/archive
  override, allowing later registry resolution or a truncated graph to mask the
  real override failure. Slug now propagates the override resolution error with
  module context.
- Validation:
  `cargo test -p slug_bzlmod resolution -- --nocapture`,
  `cargo check -p slug_bzlmod -p slug_common`, and `cargo fmt --check` pass.

Validation checkpoint 2026-05-20:

- After the registry hash propagation and override-failure checkpoints, direct
  Plan 61 guardrails passed with
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx --tb=short`
  (`27 passed`).
- Slug's own test runner also passed
  `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails`
  (`27 passed` inside the test action). The generated `bash.runfiles` tree was
  removed afterward, `slugd` was killed, local `buck-out` was 3.6M, and
  `/var/mnt/dev/zeromatter-kuro/buck-out` was 3.3M.

SDK smoke checkpoint 2026-05-20:

- Fresh ZeroMatter no-exec SDK smoke passed after the registry hash and
  override-failure checkpoints:
  `/var/mnt/dev/slug/target/debug/slug --isolation-dir plan61-noexec-after-registry-hashes-20260520 build --unstable-no-execution //sdk:sdk_contents`.
- Evidence log:
  `/tmp/slug-plan61/plan61-noexec-after-registry-hashes-20260520.log`.
  The run reported 6,008 local commands, `load=39.4s`, `analyze=2m22s`,
  `execute=1m11s`, `total=3m42s`, and `BUILD SUCCEEDED`.
- The smoke initially revisited the known
  `rules_rust//ffi/rs:empty_allocator_libraries` wait, but the queued action
  count moved and the run advanced to Rust dry-run execution, so this was not a
  blocker. `slugd` was killed afterward. The retained evidence tree
  `/var/mnt/dev/zeromatter-kuro/buck-out/plan61-noexec-after-registry-hashes-20260520`
  brought ZeroMatter `buck-out` to about 213M; `execroot` was 8K.

Implementation update 2026-05-20, yanked-version command policy:

- Bazel ground truth: Bazel parses `BZLMOD_ALLOW_YANKED_VERSIONS` and all
  `--allow_yanked_versions` occurrences as comma-separated
  `<module name>@<version>` entries; any exact `all` entry disables
  yanked-version rejection. Local anchors:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/YankedVersionsUtil.java:37`
  and
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/repository/RepositoryOptions.java:134`.
- Bazel ground truth: selected registry modules are checked after discovery and
  selection; non-registry overrides are never yanked; if selected yanked
  metadata is unavailable because `metadata.json` cannot be read, Bazel warns
  and fails open. Local anchors:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelModuleResolutionFunction.java:176`,
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelModuleResolutionFunction.java:340`,
  and
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/YankedVersionsFunction.java:52`.
- Bazel ground truth: mutable yanked metadata is not always refreshed. Outside
  refresh mode, Bazel reuses visible lockfile `selectedYankedVersions`; if the
  selected module's `source.json` hash is already recorded and the module is
  not in `selectedYankedVersions`, Bazel treats it as not yanked without
  fetching `metadata.json`. Local anchor:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/IndexRegistry.java:613`.
- Slug now threads `--allow_yanked_versions` and the client
  `BZLMOD_ALLOW_YANKED_VERSIONS` environment into the bzlmod command policy,
  parses them with Bazel-shaped syntax, checks selected registry modules after
  MVS, fails on unallowed yanked selections, records allowed selected yanked
  versions in the transitional session data, and honors visible lockfile
  yanked/source-hash facts to avoid over-fetching mutable metadata.
- Validation:
  `cargo test -p slug_bzlmod yanked -- --nocapture`,
  `cargo test -p slug_bzlmod resolution -- --nocapture`, and
  `cargo check -p slug_client_ctx -p slug_common`, `cargo fmt --check`,
  `git diff --check`, and `cargo build -p slug` pass.

SDK smoke checkpoint 2026-05-21:

- Fresh ZeroMatter no-exec SDK smoke passed after the yanked-version command
  policy checkpoint:
  `/var/mnt/dev/slug/target/debug/slug --isolation-dir plan61-noexec-after-yanked-policy-20260521-000755 build --unstable-no-execution //sdk:sdk_contents`.
- Evidence log:
  `/tmp/slug-plan61/plan61-noexec-after-yanked-policy-20260521-000755.log`.
  The run reported 6,008 local commands, `load=29.5s`, `analyze=2m28s`,
  `execute=1m14s`, `total=3m52s`, and `BUILD SUCCEEDED`.
- The run again revisited the known
  `rules_rust//ffi/rs:empty_allocator_libraries` wait, but the queued action
  count moved and analysis advanced to Rust dry-run execution, so this was not
  a blocker. `slugd` was killed afterward. The generated ZeroMatter
  `buck-out` tree reached about 213M during the smoke and was removed after the
  log was preserved; `execroot` was also removed.

Implementation update 2026-05-21, latent repository-rule DICE stubs fail
directly:

- Bazel ground truth: `RepositoryFetchFunction` returns
  `RepositoryDirectoryValue.Success` only after an existing marker is validated,
  an override is set up, or the repository rule fetch has completed and its
  marker was written; missing/hidden repositories produce
  `RepositoryDirectoryValue.Failure`, and fetch/materialization failures
  propagate as `RepositoryFunctionException`. Local anchors:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/repository/RepositoryFetchFunction.java:147`,
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/repository/RepositoryFetchFunction.java:251`,
  and
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/repository/RepositoryFetchFunction.java:300`.
- Previous Slug `RepositoryRuleExecutionKey::compute()` still returned a
  placeholder successful `bazel-external/<name>` result if that DICE key was
  requested directly. The current common path uses the native repository
  executor and extension repo execution key, but the latent key still violated
  Plan 61's no-stub invariant.
- Slug now records `stub_fallback_attempt` and fails the direct key with
  `NoImplementation` instead of synthesizing a successful repository. The
  Plan 61 event enum/counter snapshot now includes the required
  `stub_fallback_attempt` counter so future audits can observe any remaining
  fallback attempts.
- Validation:
  `cargo test -p slug_bzlmod repository_execution -- --nocapture` and
  `cargo test -p slug_bzlmod dice_graph::tests::all_plan61_event_counters_are_observable_in_process -- --nocapture`
  pass.

Validation checkpoint 2026-05-21:

- After the yanked-version policy and latent repository-rule stub checkpoints,
  direct Plan 61 guardrails passed with
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx --tb=short`
  (`27 passed`).

Implementation update 2026-05-21, root `MODULE.bazel` parse failures are not
optional:

- Bazel ground truth: `ModuleFileFunction` computes
  `ModuleFileValue.KEY_FOR_ROOT_MODULE` from the root module file and wraps
  `CompiledModuleFile.parseAndCompile` failures in persistent
  `ModuleFileFunctionException` / `ExternalDepsException`, not in a fallback
  that disables bzlmod. Local anchors:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileFunction.java:170`
  and
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileFunction.java:311`.
- Previous Slug startup cell setup warned on root `MODULE.bazel` parse failure
  and returned `Ok(None)`, which could silently build as if bzlmod were absent.
  Slug now propagates the parse failure with root module-file path context.
- Validation:
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug pytest -q tests/core/bzlmod/test_module_parsing.py::test_module_bazel_syntax_error -rx --tb=short`
  and `cargo check -p slug_common` pass.

Implementation update 2026-05-21, extension replay lockfile reads route through
`LockfileContentKey`:

- Bazel ground truth: lockfile-backed extension replay is a Skyframe input to
  `SingleExtensionEvalFunction`; Bazel reads workspace and hidden lockfiles,
  compares recorded bzl/usages/recorded-input facts, and in error mode rejects
  changes rather than silently updating. Local anchors:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/SingleExtensionEvalFunction.java:147`,
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/SingleExtensionEvalFunction.java:315`,
  and
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelLockFileFunction.java:71`.
- Slug now computes workspace and hidden extension replay lockfile reads
  through the Plan 61 `LockfileContentKey` instead of calling
  `read_lockfile_with_mode` / `read_hidden_lockfile_path` directly inside
  `ModuleExtensionExecutionKey::compute()`. Hidden lockfile read/parse failures
  still fail open, while visible lockfile parse failures remain hard errors.
- Guardrail: `LockfileContentKey` is intentionally non-cacheable until its file
  reads are backed by tracked DICE filesystem inputs. This avoids introducing a
  stale lockfile cache while still moving the consumer onto a DICE-owned read
  boundary.
- Validation:
  `cargo test -p slug_bzlmod dice_graph::tests::lockfile_content_key_is_non_cacheable_until_file_deps_are_tracked -- --nocapture`,
  `cargo test -p slug_bzlmod extension_execution_dice -- --nocapture`,
  `cargo test -p slug_bzlmod lockfile -- --nocapture`, and
  `cargo check -p slug_bzlmod -p slug_common -p slug_external_cells`,
  `cargo fmt --check`, `git diff --check`, `cargo build -p slug`, and
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -k 'lockfile or replay or recorded' -rx --tb=short`
  (`17 passed, 10 deselected`) pass.

SDK smoke checkpoint 2026-05-21, lockfile DICE read path:

- Fresh ZeroMatter no-exec SDK smoke passed after routing extension replay
  lockfile reads through `LockfileContentKey`:
  `/var/mnt/dev/slug/target/debug/slug --isolation-dir plan61-noexec-after-lockfile-dice-20260521-002955 build --unstable-no-execution //sdk:sdk_contents`.
- Evidence log:
  `/tmp/slug-plan61/plan61-noexec-after-lockfile-dice-20260521-002955.log`.
  The run reported 6,008 local commands, `load=30.2s`, `analyze=2m25s`,
  `execute=1m12s`, `total=3m45s`, and `BUILD SUCCEEDED`.
- The known `rules_rust//ffi/rs:empty_allocator_libraries` wait recurred but
  action counts moved and analysis advanced to Rust dry-run execution, so it
  remains a non-blocking frontier pattern. `slugd` was killed afterward. The
  generated ZeroMatter `buck-out` tree reached about 213M during the smoke and
  was removed after the log was preserved; `execroot` was also removed.

Implementation update 2026-05-21, registry file hashes are enforced in
`--lockfile_mode=error`:

- Bazel ground truth: HTTP/HTTPS registries use
  `KnownFileHashesMode.ENFORCE` under `--lockfile_mode=error`; file registries
  ignore known hashes. Local anchor:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/RegistryFactoryImpl.java:59`.
- Bazel ground truth: when a registry file has no known checksum under
  `--lockfile_mode=error`, Bazel throws a missing-checksum error before
  downloading mutable registry content. Local anchors:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/IndexRegistry.java:169`
  and
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/IndexRegistry.java:203`.
- Slug now applies that policy to registry `MODULE.bazel` and `source.json`
  fetches during transitional MVS/source resolution. Error mode requires a
  known visible-lockfile hash for HTTP(S) registry files before fetch, file
  registries are exempt, and any known hash mismatch fails instead of recording
  a new hash opportunistically.
- Validation:
  `cargo test -p slug_bzlmod registry_checksum -- --nocapture`,
  `cargo test -p slug_bzlmod resolution -- --nocapture`,
  `cargo check -p slug_bzlmod -p slug_common`, `cargo fmt --check`,
  `git diff --check`, and `cargo build -p slug` pass.

Blocker reflection 2026-05-21, registry hash encoding:

- Fresh ZeroMatter no-exec SDK smoke after the first checksum-enforcement
  checkpoint failed during MVS with a false mismatch:
  `expected f46e8ddad60aef170ee92b2f3d00ef66c147ceafea68b6877cb45bd91737f5f8, got sha256-9G6N2tYK7xcO6SsvPQDvZsFHzq/qaLaHfLRb2Rc39fg=`
  for
  `https://bcr.bazel.build/modules/apple_support/1.24.1/MODULE.bazel`.
  Evidence log:
  `/tmp/slug-plan61/plan61-noexec-after-registry-guardrails-20260521-004500.log`.
- Class boundary: checksum enforcement was correct, but Slug generated the
  wrong lockfile hash encoding. Bazel `registryFileHashes` values are raw
  SHA-256 hex strings from `Checksum.toString()`, not SRI `sha256-<base64>`
  strings. Local anchors:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/RegistryFileDownloadEvent.java:45`
  and
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/repository/downloader/Checksum.java:132`.
- Rejected workaround: accepting both encodings would hide non-Bazel lockfile
  data. Slug now computes registry file hashes as Bazel-compatible
  hex-encoded SHA-256; SRI encoding remains separate for archive integrity
  uses.
- Validation:
  `cargo test -p slug_bzlmod registry -- --nocapture`,
  `cargo test -p slug_bzlmod resolution -- --nocapture`,
  `cargo check -p slug_bzlmod -p slug_common`, `cargo fmt --check`,
  `git diff --check`, and `cargo build -p slug` pass.

SDK smoke checkpoint 2026-05-21, registry hash encoding:

- Fresh ZeroMatter no-exec SDK smoke passed after switching registry file
  hashes to Bazel's hex encoding:
  `/var/mnt/dev/slug/target/debug/slug --isolation-dir plan61-noexec-after-hex-registry-hashes-20260521-005500 build --unstable-no-execution //sdk:sdk_contents`.
- Evidence log:
  `/tmp/slug-plan61/plan61-noexec-after-hex-registry-hashes-20260521-005500.log`.
  The run reported 6,008 local commands, `load=34.8s`, `analyze=2m24s`,
  `execute=1m07s`, `total=3m41s`, and `BUILD SUCCEEDED`.
- The known `rules_rust//ffi/rs:empty_allocator_libraries` wait recurred with
  moving action counts and then completed. `slugd` was killed afterward. The
  generated ZeroMatter `buck-out` tree reached about 213M during the smoke and
  was removed after the log was preserved; `execroot` was also removed.

Implementation update 2026-05-21, root `MODULE.bazel` parsing routes through
`RootModuleFileKey` on server commands:

- Bazel ground truth: root module-file parsing is a Skyframe
  `ModuleFileValue.KEY_FOR_ROOT_MODULE`, and `BazelModuleResolutionFunction`
  depends on that root value before discovery/selection. Local anchors:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileValue.java:33`,
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileValue.java:68`,
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileFunction.java:163`,
  and
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelModuleResolutionFunction.java:99`.
- Slug now implements the previously inert `RootModuleFileKey` as a DICE key
  that reads, digests, and parses the root `MODULE.bazel` file. Normal server
  command config loading computes this key before invoking the existing legacy
  cell/resolution bridge, so the direct root parse in `resolve_bzlmod_dependencies`
  is removed from the build/query/audit command path.
- Guardrail: `RootModuleFileKey` is intentionally non-cacheable until root
  module-file reads are backed by tracked DICE filesystem inputs. Bootstrap and
  completion paths without a DICE transaction still use the old direct fallback;
  MVS, transitive module files, and `BzlmodCellGraphKey` remain future slices.
- Validation:
  `cargo test -p slug_bzlmod root_module_file -- --nocapture`,
  `cargo check -p slug_bzlmod -p slug_common -p slug_server`,
  `cargo fmt --check`, `git diff --check`, `cargo build -p slug`, and
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx --tb=short`
  (`27 passed`) pass.

Validation checkpoint 2026-05-21, Plan 61 guardrails:

- Direct Python guardrails passed:
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx --tb=short`
  (`27 passed`).
- The Slug test target also passed:
  `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails`
  (`27 passed` inside pytest; Slug reported one passing test target).
- `slugd` was killed afterward. `/var/mnt/dev/slug/buck-out` was 3.6M after
  the run.

This plan supersedes the completion claims in Plans 02, 09, and 10 when
"DICE bzlmod" means replay-correct graph-owned semantics. The current bzlmod
implementation is useful scaffolding. It is not yet the authority for module
resolution, extension replay, repo mapping, or repo materialization.

Every structural claim in this plan must be grounded in one of:

- Bazel source/docs from a pinned Bazel 9 checkout.
- Current Slug source or an existing plan documenting the current bug shape.
- A named local experiment that compares pinned Bazel output and Slug output.

The local Bazel checkout currently available at `/var/mnt/dev/bazel` is a
Bazel-9-era checkout (`9e1683ffb649b354124fe3cbd7413ae12d05ed3d`). Before
implementation, refresh citations against the exact Bazel 9 release tag chosen
for parity, and do not cite moving `main`.

Recent official docs checked during this pass:

- `https://bazel.build/versions/9.0.0/external/lockfile`, last updated
  2026-05-07, for lockfile generation, modes, hidden lockfile, and extension
  lockfile fields.
- `https://bazel.build/versions/9.0.0/external/module` for module discovery,
  overrides, `use_repo_rule`, and module canonical names.
- `https://bazel.build/versions/9.0.0/external/extension` for extension
  repository visibility and `inject_repo` / `override_repo`.
- `https://bazel.build/versions/9.0.0/external/overview` for
  apparent-vs-canonical repository names.

## Problem Statement

Slug's current bzlmod path is functional but not truly DICE-owned.

The authoritative module graph is assembled in legacy cell setup before DICE,
then injected into DICE and supported by process globals, lockfile caches,
marker files, stubs, and repair heuristics. DICE keys exist for late extension
and repo work, but they do not own all semantic inputs. In daemon/watch mode,
that can reuse stale bzlmod facts or cross-contaminate workspaces.

Current anti-patterns:

| Anti-pattern | Current grounding | Replacement |
|---|---|---|
| Graph computed before DICE | `resolve_bzlmod_dependencies()` is called during legacy cell setup in `app/slug_common/src/legacy_configs/cells.rs`; `CellResolverKey` receives an injected resolver in `app/slug_common/src/dice/cells.rs`. | `BzlmodResolutionKey` and projections. |
| Process-global semantic facts | `MODULE_VERSIONS` / toolchain globals in `app/slug_bzlmod/src/lib.rs`, dynamic cell globals in `app/slug_core/src/cells.rs`, extension/spoke globals in `extension_execution_dice.rs` / `spoke_materialization.rs`. | Workspace-scoped DICE values. |
| Under-keyed DICE values | Current extension/repo keys carry `project_root` but exclude it from hash/equality in `extension_execution_dice.rs` and `repository_execution.rs`. | A first-class `WorkspaceId` participates in every bzlmod semantic key or parent value. |
| Approximate replay digests | `compute_bzl_transitive_digest_for_project` now hashes local workspace extension `.bzl` files reachable by literal `load()`s for the Plan 61 guardrail path, and lockfile replay validates absolute/main-workspace `FILE`, `DIRENTS`, `DIRTREE`, command-scoped `ENV`, module-scope `REPO_MAPPING`, and extension-generated source `REPO_MAPPING` recorded inputs. Full Bazel loaded-module graph ownership, typed DICE `RepoMappingKey` ownership, and complete `RepoRecordedInput` coverage are not implemented. | Bazel-shaped `bzlTransitiveDigest`, `usagesDigest`, and `RepoRecordedInput` validation. |
| Bare marker trust | `.slug_repo_complete` is trusted by repo/external-cell paths; Plan 38 documents marker-gated warm-build failures. | Bazel-shaped marker/recorded-input validation or a Slug DICE manifest with an explicit parity experiment. |
| Stub repos / empty specs on failure | Unknown repo rules and extension/repo-rule failures still create stubs in `repository_executor.rs` and `extension_repo.rs`. | Direct Bazel-shaped failure; no generated repo directory/marker on failure. |
| Canonical identity reconstruction | Current code accepts fallback spellings, suffix scans, and `bazel-external` discovery. | Typed module/extension/repo identity and scoped repo mappings. |
| Build-time lockfile mutation risk | `Lockfile::write` remains public and `--lockfile_mode` is not a complete policy boundary. | Explicit write capability for future `slug mod update`; ordinary paths cannot call the writer. |

## Non-Negotiables

- Bazel 9 parity only. No Bazel 8 compatibility, no WORKSPACE fallback, no
  compatibility shims for prototype behavior. Grounding: project `AGENTS.md`.
- DICE owns semantic facts. Process globals may exist temporarily as migration
  adapters, but each phase must reduce rather than expand them.
- Workspace identity is part of bzlmod identity. Two workspaces in one daemon
  cannot share extension aggregations, spokes, aliases, module versions,
  lockfile data, materialization roots, or project-root state.
- Extension/repo failures fail directly. They do not create stubs, silently
  repair outputs, or continue with empty generated repo sets.
- Apparent repo names are per-scope aliases, not identities. Canonical module
  repos, extension repos, innate `use_repo_rule` repos, bundled repos, and
  generated repos must have typed origin kinds.
- Lockfile policy is mode-aware. Bazel writes `MODULE.bazel.lock` in
  UPDATE/REFRESH modes (`RepositoryOptions.java`, `BazelLockFileModule.java`).
  Slug ordinary build/query/audit paths remain read-only as an interim safety
  policy until exact Bazel write parity exists; that safety policy is not a
  Bazel parity claim.

## Target DICE Graph

These names describe the target shape; exact Rust identifiers can differ. The
grounding column states whether the key follows Bazel structure or fixes a Slug
bug shape.

| Key / value | Owns | Grounding |
|---|---|---|
| `BzlmodWorkspaceKey { canonical_project_root, output_base } -> WorkspaceId` | Stable workspace identity used by all bzlmod semantic keys. | Current Slug under-keys by excluding `project_root`; Bazel scopes server/output-base state by workspace/output-base, and Slug daemon dirs include project root. Do not include command mode in `WorkspaceId`, or the same workspace becomes a different identity when flags change. |
| `BzlmodCommandPolicyKey { workspace_id, bazel_release_id, starlark_semantics_digest, bzlmod_flags, lockfile_mode, registry_config_digest, repository_cache_config, network_policy, repo_env_digest, nonstrict_repo_env_digest, ignore_dev_dependency, allow_yanked_versions, bazel_compatibility_policy, isolated_extension_usages_flag } -> BzlmodCommandPolicy` | Command-scoped policy and options that affect resolution, replay, and side effects. | Bazel 9 lockfile docs say lockfiles are specific to the Bazel version; Bazel `RepositoryOptions`, `ModuleFileFunction.IGNORE_DEV_DEPENDENCY`, `ModuleFileGlobals` dev-dependency/isolate handling, `RegularRunnableExtension` repo env inputs, and `SingleExtensionEvalFunction` Starlark semantics use. |
| `RootModuleFileKey { workspace_id } -> ParsedModuleFile` | Root `MODULE.bazel` parse and digest. | Bazel `ModuleFileValue` / `ModuleFileFunction`; current Slug parses in legacy `cells.rs`. |
| `ModuleSourceKey { workspace_id, command_policy_digest, module_key, registry_or_override_identity } -> ModuleSource` | Registry/local/archive source identity, registry order, patches/overlays, yanked state, auth/offline/cache policy. | Bazel `IndexRegistry`, `RepoSpecFunction`, module docs; current registry/cache code performs side effects. |
| `ModuleFileKey { workspace_id, source } -> ParsedModuleFile` | Transitive module parse from registry/local sources and included module files. | Bazel `ModuleFileFunction` root/non-root values and include handling. |
| `LocalOverrideSourceKey { workspace_id, declaring_module, override_literal, resolved_abs_path, module_file_digest }` | Local override identity and invalidation on create/edit/delete. | Bazel loads non-registry overrides through `RepositoryDirectoryValue`/`FileValue`; current Slug resolves local paths before DICE. |
| `LockfileContentKey { workspace_id, kind: workspace_or_hidden, path } -> { existence, digest, parse_result }` | Workspace lockfile and hidden output-base lockfile as filesystem inputs, including parse/version errors. In `off` mode, no semantic consumer should request this key. | Bazel 9 lockfile docs describe workspace and hidden lockfiles; `BazelLockFileValue.KEY` / `HIDDEN_KEY`; current `cached_lockfile` caches `None` process-wide. |
| `LockfileExtensionEntryKey { workspace_id, extension_instance_id, lockfile_digest, lockfile_mode, eval_factors }` | One extension's per-eval-factor lockfile replay entry. | Bazel `SingleExtensionEvalFunction` reads workspace then hidden lockfile, keyed by `ModuleExtensionEvalFactors`, unless mode is `off`. |
| `BzlmodResolutionKey { workspace_id, command_policy_digest } -> BzlmodResolutionValue` | Discovery, MVS selection, overrides, yanked versions, registry hashes, root/dev filtering. | Bazel `BazelModuleResolutionFunction`, `Selection`, `BazelModuleResolutionValue`, `RepositoryOptions`, and Bazel 9 module/lockfile docs. |
| `ResolvedModuleIdentity { workspace_id, module_key }` | Module canonical repo name, apparent repo name, `repo_name`, version metadata. | Bazel `ModuleKey` and external docs on canonical/apparent names. |
| `BzlmodCellGraphKey { workspace_id, resolution_digest } -> BzlmodCellGraph` | Slug root/external cells, bzlmod module cells, bundled cells, extension placeholder cells. | Slug-specific cell resolver shape; Bazel grounding is `BazelDepGraphValue` canonical repository graph. Include resolution identity explicitly to avoid hiding command-policy changes behind a workspace-only cell graph key. |
| `RepoMappingKey { workspace_id, resolution_digest, scope: RepoMappingScope } -> RepoMapping` | Scoped apparent-to-canonical mappings for module repos, extension implementation, generated repos, and innate repos. | Bazel repo mapping docs, `BazelDepGraphValue`, `ModuleExtensionRepoMappingEntriesFunction`; current Slug has global/scoped alias maps. |
| `RegisteredToolchainsKey { workspace_id, resolution_digest } -> Vec<RegisteredToolchain>` | Ordered `register_toolchains()` projection. | Bazel `ModuleFileGlobals`, `RegisteredToolchainsFunction`; current Slug global registry. |
| `RegisteredExecutionPlatformsKey { workspace_id, resolution_digest } -> Vec<CanonicalLabel>` | Ordered `register_execution_platforms()` projection. | Bazel `ModuleFileGlobals`, `RegisteredExecutionPlatformsFunction`; current Slug global registry. |
| `ModuleExtensionId { bzl_file_label: CanonicalLabel, extension_name, isolation_key }` | Extension identity. | Bazel `ModuleExtensionId`; current Slug string-normalizes extension ids. |
| `ExtensionUniqueName { workspace_id, extension_instance_id }` | Producer-owned generated repo namespace with Bazel's no-prefix-after-`+` uniqueness property. | Bazel `BazelDepGraphValue#getExtensionUniqueNames`, `BazelDepGraphFunction.calculateUniqueNameForUsedExtensionId`, and `SingleExtensionUsagesValue#getExtensionUniqueName`; generated repo canonical names are based on extension unique name plus internal repo name. |
| `ModuleExtensionAggregationKey { workspace_id, resolution_digest, extension_instance_id } -> ExtensionAggregationValue` | Aggregated usages/tags, eval factors, root module name, isolated usage separation. | Bazel `SingleExtensionUsagesFunction` / `SingleExtensionUsagesValue`; current Slug `EXTENSION_AGGREGATIONS` global. |
| `ModuleExtensionReplayInputKey { workspace_id, extension_instance_id, lockfile_entry_digest, bzl_transitive_digest, usages_digest, recorded_inputs }` | Normalized replay metadata while preserving individual `RepoRecordedInput.WithValue` entries for validation. Facts are provided to execution/replay values but are not normal replay invalidators. | Bazel 9 lockfile docs fields `bzlTransitiveDigest`, `usagesDigest`, `generatedRepoSpecs`, `moduleExtensionMetadata`; `RegularRunnableExtension`, `SingleExtensionEvalFunction`, `RepoRecordedInput`, facts docs. |
| `ModuleExtensionExecutionKey { workspace_id, extension_instance_id, command_policy_digest, replay_inputs_digest } -> ModuleExtensionResult` | Generated repo specs, metadata/facts, declared generated repo set. | Bazel `SingleExtensionValue` includes generated repos and facts; current Slug drops metadata from `ModuleExtensionResult`. Include command/Starlark semantics and repo env policy explicitly because extension evaluation can observe them. |
| `InnateExtensionKey { workspace_id, owner_module_key, bzl_label, rule_name, invocation_id }` | `use_repo_rule` as Bazel's innate extension shape, including dev-dependency and scoped visibility. | Bazel docs/source model `use_repo_rule` as an innate extension; current Slug stores flat `RepoRuleInvocation`. |
| `ExtensionSpokesKey { workspace_id, extension_instance_id } -> ExtensionSpokesValue` | Slug bridge for hub/spoke registration from lockfile replay or evaluated extension result. | Slug-specific Plan 36/38 bug shape; not Bazel terminology. |
| `ExtensionRepoExecutionKey { workspace_id, canonical_repo, repo_spec_digest, repo_rule_impl_digest, repo_replay_inputs_digest } -> RepositoryRuleResult` | Repo-rule execution from typed repo identity and canonical `RepoSpec`. | Bazel generated repo specs; current Slug `ExtensionRepoExecutionKey` is under-keyed. |
| `RepoMaterializationManifestKey { workspace_id, output_base, canonical_repo, repo_spec_digest } -> RepoLayoutManifest` | Slug DICE correctness mechanism for materialized layout validation. | This is not direct Bazel parity; Bazel uses marker files keyed by predeclared input/recorded input digests (`DigestWriter`, `RepositoryFetchFunction`). |
| `ExternalSymlinkLayoutKey { workspace_id, output_base, cell_graph_digest } -> ()` | Slug `external/`, `bazel-external/`, and buck-out layout side effects. | Slug-specific layout side effect; not a semantic dependency of resolution. |

## Values To Move Out Of Globals

Plan 61 retires these as semantic authorities:

- `MODULE_VERSIONS`
- `REGISTERED_TOOLCHAINS`
- `REGISTERED_EXECUTION_PLATFORMS`
- `LOCKFILE_CACHE`
- `EXTENSION_AGGREGATIONS`
- `SPOKE_REGISTRY`
- `SEEDED_EXTENSIONS`
- `ROOT_CELL_NAME`
- `EXTERNAL_CELL_NAMES`
- `DYNAMIC_EXTENSION_CELLS`
- `DYNAMIC_EXTENSION_CELL_SETUPS`
- `DYNAMIC_EXTENSION_CELL_ALIASES`
- `SCOPED_BZLMOD_REPO_ALIASES`
- `BZLMOD_APPARENT_ALIAS_CACHE`
- `DYNAMIC_PROJECT_ROOT`
- mutable `CellResolverInternals::dynamic_cells` insertion as a semantic path

Temporary adapters can call into DICE-backed values while call sites migrate, but
the final state must make these globals unnecessary.

## Desired Dependency Edges

- `CellResolverKey` depends on `BzlmodCellGraphKey { workspace_id,
  resolution_digest }`, not directly on pre-DICE resolution output. Grounding:
  current `CellResolverKey` consumes an injected resolver; legacy `cells.rs`
  assembles bzlmod state before DICE.
- BUILD and `.bzl` label resolution depends on `RepoMappingKey` for the owner
  mapping scope. Grounding: Bazel strict repo mapping docs and Slug's current
  `repo_mapping.rs` canonicalization paths.
- `native.module_name()`, `native.module_version()`, BUILD-file
  `module_name()`, and `module_version()` depend on `ResolvedModuleIdentity`.
  Grounding: Bazel BUILD/native globals docs and Slug `MODULE_VERSIONS` global.
- Toolchain and execution-platform registration depend on bzlmod graph
  projections. Grounding: Bazel registered toolchain/platform SkyFunctions.
- Extension repo access depends on `ExtensionSpokesKey`, then
  `ExtensionRepoExecutionKey`, then layout materialization. Grounding: current
  Slug `extension_repo.rs` access path and spoke-materialization bridge.
- Lockfile replay is an optimization edge under extension execution. A valid
  lockfile hit can provide generated repo specs/facts; stale or incomplete
  replay inputs force extension evaluation or error according to lockfile mode.
  Grounding: Bazel `SingleExtensionEvalFunction`; current Slug cache hit path
  only checks digests.

## Validation First

Plan 61 must add proof infrastructure before moving ownership:

- Baseline xfail/known-bad fixtures for root edit, local override edit, two
  workspaces with colliding module/repo names, lockfile SHA stability/mode behavior, extension
  `.bzl` edit, transitive `.bzl` edit, bad extension, unknown repo rule, stale
  marker, and no-stub failures.
- Structured events/counters: `bzlmod_resolution_compute`, `module_file_parse`,
  `extension_eval`, `extension_replay_hit`, `extension_replay_miss_reason`,
  `repo_materialization_hit`, `repo_materialization_miss_reason`,
  `lockfile_read`, `lockfile_write_attempt`, and `stub_fallback_attempt`.
- Every parity fixture records: pinned Bazel source/doc anchor, local Bazel
  output, local Slug output, and the decision for any source/doc/policy mismatch.

## Migration Phases

### 61.1 Inventory, Citations, And Guardrails

- Add typed bzlmod graph values without behavior changes.
- Document exact call sites for every global, marker trust path, path repair,
  lockfile writer, and stub fallback.
- Add the validation fixtures and counters listed above.
- Mark current tests that assert cross-workspace key equality as known-bad until
  replaced by workspace-scoped tests.

Exit criteria:

- New value types compile.
- A checked inventory links every current bzlmod semantic global to a DICE owner.
- No later phase can merge without using the structured events/counters.

Implementation slice 2026-05-18:

- Added inert typed graph identities and value-key shapes in
  `app/slug_bzlmod/src/dice_graph.rs`. These are not behavioral authority yet;
  they name the future DICE values while preserving today's legacy startup path.
- Added counters/tracing with the required event names:
  `bzlmod_resolution_compute`, `module_file_parse`, `extension_eval`,
  `extension_replay_hit`, `extension_replay_miss_reason`,
  `repo_materialization_hit`, `repo_materialization_miss_reason`,
  `lockfile_read`, `lockfile_write_attempt`, and `stub_fallback_attempt`.
- Wired those counters into current legacy locations so later phases can prove
  whether a path still depends on startup-built/global state.
- Added explicit skipped Python guardrail tests in
  `tests/core/bzlmod/test_plan61_guardrails.py` for the validation-first list:
  root `MODULE.bazel` edit, local override edit, two workspaces in one daemon,
  lockfile SHA/mode behavior, extension `.bzl` edit, transitive `.bzl` edit,
  bad extension, unknown repo rule, stale marker, and broader no-stub failures.
  These are not behavior claims; each skipped test carries a pinned Bazel
  source/doc anchor and the current Slug bug shape to convert into xfail/passing
  fixtures in later phases.
- Counter observability is validated in-process by
  `app/slug_bzlmod/src/dice_graph.rs::all_plan61_event_counters_are_observable_in_process`.
  Follow-up slice below exposes these counters through a daemon-side audit
  command for Python daemon tests.
- Manager rerun validated direct collection/execution of the skipped guardrail
  file with `python -m pytest --collect-only -q
  tests/core/bzlmod/test_plan61_guardrails.py` and `python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py` (10 collected, 10 skipped), plus
  `cargo test -p slug_bzlmod dice_graph::tests:: -- --nocapture` (3 passed).
  `./slug test tests/core/bzlmod:test_plan61_guardrails` is currently blocked
  before test execution by an existing Clap debug assertion: duplicate
  `test_summary` argument/group name.

Implementation slice 2026-05-18, phase 61.1/61.2 boundary:

- Exposed the Plan 61 counter snapshot through daemon-side
  `slug audit bzlmod-counters`, which prints a structured JSON object with the
  counter fields `bzlmod_resolution_compute`, `module_file_parse`,
  `extension_eval`, `extension_replay_hit`, `extension_replay_miss_reason`,
  `repo_materialization_hit`, `repo_materialization_miss_reason`,
  `lockfile_read`, `lockfile_write_attempt`, and `stub_fallback_attempt`.
  This is intentionally observability-only and does not change bzlmod
  resolution, replay, or materialization behavior.
- Converted the first two skipped guardrails into focused Python daemon
  fixtures in `tests/core/bzlmod/test_plan61_guardrails.py`: root
  `MODULE.bazel` edit and local override `MODULE.bazel` edit. Both assert that
  warm daemon commands observe the edited module input by checking structured
  counter deltas through `audit bzlmod-counters`.
- Converted the two-workspaces guardrail from a skip into a focused fixture
  with colliding root module names, extension names, and generated repo names,
  plus unique local-module sentinels for each workspace. The fixture now uses
  `audit bzlmod-counters` and `debug daemon-dir`; it xfails today because the
  daemon directory still includes `project_root`, so the two roots start
  separate daemons before same-daemon bzlmod state sharing can be tested.
- Seven guardrails remain skipped: lockfile SHA/mode behavior, extension `.bzl`
  edit, transitive extension `.bzl` edit, bad extension no-stub failure,
  unknown repo rule no-stub failure, stale repo marker, and broad no-stub
  failures.
- Validation used `cargo check -p slug_cmd_audit_client -p
  slug_cmd_audit_server -p slug_bzlmod`, `cargo test -p slug_bzlmod
  dice_graph::tests:: -- --nocapture`, `cargo build -p slug`, direct
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py`, and direct
  `./slug audit bzlmod-counters`.
  Manager rerun confirmed the same focused validation: direct pytest returned
  2 passed / 7 skipped / 1 xfailed after the two-workspace fixture conversion,
  `cargo build -p slug` succeeded, and
  `./slug audit bzlmod-counters` emitted the expected JSON counter object.
  A second manager rerun after reviewing the two-workspace fixture again
  confirmed `cargo build -p slug`, `cargo test -p slug_bzlmod
  dice_graph::tests:: -- --nocapture`, pytest collection, and direct
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py` (2 passed / 7 skipped /
  1 xfailed), with no `slugd[` processes left after cleanup.
  `./slug test tests/core/bzlmod:test_plan61_guardrails` now gets past the
  earlier Clap assertion after the manager-side test command fix, but this
  checkout then fails before test execution because `tests/core/bzlmod/BUILD.bazel`
  loads `@fbcode//buck2/tests:buck_e2e.bzl` and the local cell resolver has no
  `fbcode` alias.

Implementation slice 2026-05-18, lockfile guardrail conversion:

- Converted the lockfile SHA/mode skip into focused Python fixtures using
  `audit bzlmod-counters`.
- Passing coverage now proves that a visible workspace `MODULE.bazel.lock` read
  is observable through `lockfile_read`, ordinary `audit cell` leaves the file
  SHA stable, and `lockfile_write_attempt` stays zero on this ordinary path.
- Three strict xfail fixtures pin current missing policy boundaries:
  hidden output-base lockfile reads are not represented, `--lockfile_mode=off`
  is accepted but ignored instead of suppressing lockfile reads, and
  `--lockfile_mode=error` is accepted but ignored while `cached_lockfile`
  swallows invalid visible lockfiles as warnings.
- Six guardrails remain skipped: extension `.bzl` edit, transitive extension
  `.bzl` edit, bad extension no-stub failure, unknown repo rule no-stub
  failure, stale repo marker, and broad no-stub failures.
- Focused validation returned `python -m pytest --collect-only -q
  tests/core/bzlmod/test_plan61_guardrails.py` (13 collected) and
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py` (3 passed / 6 skipped /
  4 xfailed).
  Manager rerun confirmed the same direct pytest result, and fixed the
  in-process counter unit tests to use monotonic assertions because the counters
  are process-global atomics and Rust tests can run concurrently. `cargo test
  -p slug_bzlmod dice_graph::tests:: -- --nocapture` now passes (3 tests).

Implementation slice 2026-05-18, extension `.bzl` edit guardrail conversion:

- Converted the extension implementation `.bzl` edit guardrail from a skip into
  a focused strict xfail fixture using `audit bzlmod-counters`.
- The fixture first writes a minimal valid `MODULE.bazel.lock` extension entry
  whose `generatedRepoSpecs` produces an observable `extension_replay_hit`, then
  edits the extension implementation `.bzl` and asserts that replay is rejected
  through `extension_replay_miss_reason` without a second hit.
- Current Slug strict-xfails with the exact Plan 61 bug shape:
  `compute_bzl_transitive_digest` hashes only the extension id, so the edited
  implementation `.bzl` does not stale the lockfile replay entry. The expected
  Bazel behavior is pinned to `RegularRunnableExtension.java` lines 207-210
  (`BazelModuleContext` bzl transitive digest) and
  `SingleExtensionEvalFunction.java` lines 318-324 (changed implementation or
  transitive `.bzl` rejects replay).
- Five guardrails remain skipped: transitive extension `.bzl` edit, bad
  extension no-stub failure, unknown repo rule no-stub failure, stale repo
  marker, and broad no-stub failures.
- Focused validation returned `python -m pytest --collect-only -q
  tests/core/bzlmod/test_plan61_guardrails.py` (13 collected),
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py` (3 passed / 5 skipped /
  5 xfailed), focused new-test `-rx` (1 xfailed with the digest-only bug
  reason), and `cargo test -p slug_bzlmod dice_graph::tests:: -- --nocapture`
  (3 passed).
  Manager rerun confirmed the same collection/build/unit-test/focused-pytest
  state and left no `slugd[` processes after cleanup.

Implementation slice 2026-05-18, transitive extension `.bzl` edit guardrail
conversion:

- Converted the transitive extension helper `.bzl` edit guardrail from a skip
  into a focused strict xfail fixture using the same lockfile replay and
  `audit bzlmod-counters` pattern as the direct extension implementation edit
  fixture.
- The fixture writes a valid replay lockfile, proves an initial
  `extension_replay_hit` without `extension_eval`, edits `replay_helper.bzl`
  loaded by the extension implementation, and expects replay rejection through
  `extension_replay_miss_reason` with no additional replay hit.
- Current Slug strict-xfails with the same Plan 61 bug shape:
  `compute_bzl_transitive_digest` hashes only the extension id, so edits to
  transitive helper `.bzl` files are invisible to replay validation. Expected
  Bazel behavior is pinned to `RegularRunnableExtension.java` lines 207-210
  (`BazelModuleContext` bzl transitive digest) and
  `SingleExtensionEvalFunction.java` lines 318-324 (changed implementation or
  transitive `.bzl` rejects replay).
- Four guardrails remain skipped: bad extension no-stub failure, unknown repo
  rule no-stub failure, stale repo marker, and broad no-stub failures.
  Manager rerun confirmed `python -m pytest --collect-only -q
  tests/core/bzlmod/test_plan61_guardrails.py` (13 collected),
  `cargo test -p slug_bzlmod dice_graph::tests:: -- --nocapture`,
  `cargo build -p slug`, direct full pytest (3 passed / 4 skipped /
  6 xfailed), and focused transitive-helper `-rx` (1 xfailed with the
  digest-only bug reason), with no `slugd[` processes left after cleanup.

Implementation slice 2026-05-18, bad extension no-stub guardrail conversion:

- Converted the bad module-extension failure guardrail from a skip into a
  focused strict xfail fixture.
- The fixture writes a root extension whose implementation fails with
  `PLAN61_BAD_EXTENSION_EVAL`, imports `failed_repo`, and loads
  `@failed_repo//:defs.bzl` from the root BUILD file to force extension repo
  access. Expected behavior is a direct extension evaluation failure, no
  generated repo directory, no `.slug_repo_complete` marker, and no
  `stub_fallback_attempt` counter delta.
- Current Slug strict-xfails with the exact Plan 61 bug shape: the build
  unexpectedly succeeds, materializes
  `bazel-external/_main+bad_ext+failed_repo/.slug_repo_complete`, and leaves
  `stub_fallback_attempt` unchanged. Expected Bazel behavior is pinned to
  `SingleExtensionFunction.java` lines 45-72 (validated extension value and
  invalid import failure shape) and `SingleExtensionEvalFunction.java` lines
  262-291 (successful evaluation is required before lockfile info/value
  creation; failed evaluation is not converted into generated specs).
- Three guardrails remain skipped: unknown repo rule no-stub failure, stale
  repo marker, and broad no-stub failures.

Implementation slice 2026-05-18, unknown repo-rule no-stub guardrail
conversion:

- Converted the unknown repository-rule failure guardrail from a skip into a
  focused strict xfail fixture.
- The fixture writes a replay lockfile entry for a generated repo whose
  `repoRuleId` is a plain unsupported rule name, then loads that repo from the
  root BUILD file to force repository-rule materialization. Expected behavior is
  a direct repository rule resolution/evaluation failure, no generated
  repository directory, no `.slug_repo_complete` marker, and no
  `stub_fallback_attempt` counter delta.
- Current Slug strict-xfails with the exact Plan 61 bug shape: the generated
  repo spec reaches `repository_executor.rs`, which records
  `StubFallbackAttempt` for the unknown rule and materializes a synthetic repo
  before the command fails later while loading from that stub. Expected Bazel
  behavior is pinned to `RepoDefinitionFunction.java` and
  `RepositoryFetchFunction.java`: repository rule resolution/evaluation errors
  are repository fetch failures, not synthetic generated repositories.
- One guardrail remains skipped: broad no-stub failures.

Implementation slice 2026-05-18, stale repo marker guardrail conversion:

- Converted the stale repository marker guardrail from a skip into a focused
  strict xfail fixture.
- The fixture pre-seeds
  `bazel-external/+new_local_repository+marker_repo` with a stale BUILD file,
  a stale output, and a bare `.slug_repo_complete` marker, then declares a
  `use_repo_rule` `new_local_repository` whose current spec expects
  `fresh.txt`. Expected behavior is re-materialization or an explicit
  marker/spec failure; stale output must not be reused.
- Current Slug strict-xfails with the exact Plan 61 bug shape: repository
  access reuses the stale BUILD file behind the bare marker and fails with
  `Unknown target fresh.txt` while offering the stale target. The observed
  counters do not record a `repo_materialization_hit` or
  `repo_materialization_miss_reason` for this path, which shows that the stale
  file path is being trusted before a Bazel-shaped marker/recorded-input
  validation edge exists.
- Expected Bazel behavior is pinned to `DigestWriter.java`,
  `RepoRecordedInput.java`, and `RepositoryFetchFunction.java`: repository
  markers encode predeclared input hashes and recorded inputs, and outdated
  marker/input state makes the repository inconsistent instead of serving stale
  outputs.

Implementation slice 2026-05-18, broad no-stub failure guardrail conversion:

- Converted `test_no_stub_failures_cover_missing_generated_repo_and_repo_rule_failure`
  from the final skip into one focused strict xfail guardrail with two isolated
  sub-workspaces.
- The missing-generated-repo fixture uses a module extension that evaluates
  successfully but does not generate the repo imported with `use_repo`, then
  loads `@missing_repo//:defs.bzl` to force repository access. Expected Bazel 9
  behavior is a direct missing generated repository failure from the module
  extension path, with no generated repo directory, no `.slug_repo_complete`
  marker, and no synthetic `BUILD.bazel` or `defs.bzl`.
- The repo-rule-failure fixture uses `use_repo_rule` for
  `@@bazel_tools//tools/build_defs/repo:local.bzl%local_repository` but omits
  the required `path` attr, then loads `@broken_local_repo//:defs.bzl` to force
  materialization. Expected Bazel 9 behavior is a direct repository rule
  resolution/evaluation failure, with no generated repo directory, no marker,
  and no synthetic repo files.
- Current Slug strict-xfails with the exact Plan 61 bug shape: both builds
  unexpectedly succeed, `extension_repo.rs` materializes stub repositories at
  `bazel-external/_main+empty_ext+missing_repo` and
  `bazel-external/+local_repository+broken_local_repo`, both stubs contain
  `.slug_repo_complete`, `BUILD.bazel`, and `defs.bzl`, and both fallback
  paths leave `stub_fallback_attempt` unchanged (`delta=0`). This is distinct
  from the unknown repo-rule fallback in `repository_executor.rs`, which does
  increment `stub_fallback_attempt`.
- Expected Bazel behavior is pinned to `SingleExtensionFunction.java`,
  `RepoDefinitionFunction.java`, and `RepositoryFetchFunction.java`: missing
  generated repos and repository rule evaluation failures are direct bzlmod /
  repository fetch failures, not synthetic generated repositories.

Implementation slice 2026-05-18, no-stub failure enforcement:

- Removed normal bzlmod stub fallback creation from
  `app/slug_external_cells/src/extension_repo.rs`. Extension execution errors,
  DICE errors, successful extensions that do not generate an imported repo,
  `use_repo_rule` execution errors, and lazy generated repo-rule execution
  errors now return `ExtensionRepoError::MaterializationFailed` instead of
  writing synthetic `BUILD.bazel`, `defs.bzl`, `versions.bzl`, or
  `.slug_repo_complete` files.
- Removed unknown repository rule stub creation from
  `app/slug_bzlmod/src/repository_executor.rs`. Unsupported rule names now
  return `RepositoryExecutionError::NoImplementation` and do not increment the
  `stub_fallback_attempt` counter.
- Promoted three guardrails from strict xfail to passing expectations:
  bad extension no-stub failure, unknown generated repo rule no-stub failure,
  and the broad missing-generated-repo / repo-rule-failure no-stub matrix.
- Legacy `stub` marker detection remains only as cleanup compatibility for
  stale output trees written by older Slug builds; new normal bzlmod failure
  paths no longer create those markers.
- Validation: after freeing failed/generated linker artifacts in `target/`,
  `cargo build -p slug` succeeded, and
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py::test_bad_extension_fails_without_stub_repo
  tests/core/bzlmod/test_plan61_guardrails.py::test_unknown_repo_rule_fails_without_stub_repo
  tests/core/bzlmod/test_plan61_guardrails.py::test_no_stub_failures_cover_missing_generated_repo_and_repo_rule_failure
  --runxfail` passed (3 passed).

Implementation slice 2026-05-18, use_repo_rule stale marker enforcement:

- Populated `spec_hash` for `use_repo_rule` pending cells in
  `app/slug_bzlmod/src/pending_repo_cells.rs`, and for the legacy custom
  repo-rule extension-cell setup in `app/slug_common/src/legacy_configs/cells.rs`.
- This lets `app/slug_external_cells/src/extension_repo.rs` apply its existing
  spec-hashed completion marker validation to `use_repo_rule` repos. A bare
  `.slug_repo_complete` marker now mismatches the current spec hash, the stale
  repo directory is discarded, and `new_local_repository` materializes the
  current `BUILD.bazel`/output tree.
- Promoted the stale repo marker guardrail from strict xfail to a passing
  expectation.
- Validation: `cargo check -p slug_bzlmod -p slug_common -p
  slug_external_cells`, `cargo build -p slug`, and
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py::test_stale_repo_marker_does_not_mask_changed_repo_spec_or_outputs
  --runxfail` passed.

Implementation slice 2026-05-18, local extension `.bzl` transitive digest
enforcement:

- Added `compute_bzl_transitive_digest_for_project` in
  `app/slug_bzlmod/src/extension_execution_dice.rs` and re-exported it from
  `app/slug_bzlmod/src/lib.rs`. Lockfile replay pre-seeding and extension
  execution now use the project-root-aware digest when a workspace root is
  available.
- The new digest resolves the extension implementation label under the current
  workspace, parses Starlark with the local parser, follows literal
  `load()` labels to reachable workspace `.bzl` files, and hashes a stable
  versioned digest over relative paths plus file bytes. If the root extension
  label cannot be resolved locally, Slug falls back to the previous
  extension-id digest instead of inventing external repository state.
- Updated the Python guardrail lockfile writer to mirror the versioned local
  digest shape, then promoted both direct extension implementation edit and
  transitive helper `.bzl` edit guardrails from strict xfail to passing
  expectations.
- This is an incremental correctness fix, not the final 61.6 target: it covers
  the local workspace `.bzl` replay-staleness bug shape, while a fully
  DICE-owned Bazel-shaped loaded-module graph, external module digests,
  `usagesDigest`, and `RepoRecordedInput` validation remain future work.
- Validation: `cargo check -p slug_bzlmod`, `cargo build -p slug`,
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py::test_extension_bzl_edit_invalidates_or_rejects_replay
  tests/core/bzlmod/test_plan61_guardrails.py::test_transitive_extension_bzl_edit_invalidates_or_rejects_replay`
  (2 passed), and full
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py -rx` (9 passed / 4 xfailed)
  passed before the hidden-lockfile follow-up below.

Implementation slice 2026-05-18, hidden lockfile read observability:

- Routed the daemon output-base path into bzlmod config as
  `bzlmod.hidden_lockfile_path` from `ServerCommandContext`, and added
  mode-aware lockfile-cache readers in `app/slug_bzlmod/src/lockfile.rs` for
  both workspace and explicit hidden lockfile paths.
- Startup bzlmod cell registration in
  `app/slug_common/src/legacy_configs/cells.rs` now checks the hidden
  `MODULE.bazel.lock` path after the visible workspace lockfile path, so the
  Plan 61 hidden-lockfile guardrail observes a real `lockfile_read` event.
- Added config-backed lockfile-mode plumbing for `bzlmod.lockfile_mode`; this
  is enough for internal mode-aware reads, but the Bazel-shaped
  `audit cell --lockfile_mode=off/error` CLI form is still not reaching the
  cell-loading config path, so the two mode-policy guardrails remain strict
  xfail.
- Promoted `test_hidden_lockfile_read_is_observable_before_extension_replay`
  from strict xfail to a passing expectation.
- Validation: `cargo check -p slug_cmd_audit_client -p slug_client_ctx -p
  slug_common -p slug_bzlmod -p slug_server`, `cargo build -p slug`, and full
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py -rx` (10 passed / 3 xfailed)
  passed. The three remaining xfails are same-daemon workspace isolation
  precondition, `--lockfile_mode=off`, and `--lockfile_mode=error`.

Implementation slice 2026-05-18, lockfile mode policy promotion:

- Routed Bazel-shaped `--lockfile_mode` / `--lockfile-mode` CLI forms into
  bzlmod config before client/server bootstrap cell loading, and kept bootstrap
  bzlmod reads in `off` mode so invalid visible lockfiles do not warn or fail
  before command policy is available.
- Promoted `test_lockfile_mode_off_does_not_read_lockfiles` and
  `test_lockfile_mode_error_rejects_invalid_visible_lockfile` from strict
  xfail to passing expectations.
- Validation: `cargo check -p slug_client_ctx -p slug_server -p slug_common -p
  slug_bzlmod`, `cargo build -p slug`, focused
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py::test_lockfile_mode_off_does_not_read_lockfiles
  tests/core/bzlmod/test_plan61_guardrails.py::test_lockfile_mode_error_rejects_invalid_visible_lockfile
  --runxfail` (2 passed), and full
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py -rx` (12 passed / 1 xfailed)
  passed, with daemon processes cleaned up afterward.

Implementation slice 2026-05-18, workspace-scoped extension/repo execution keys:

- Removed the explicit under-keying where `ModuleExtensionExecutionKey` and
  `ExtensionRepoExecutionKey` carried `project_root` but excluded it from
  `Hash`/`Eq`. The keys now distinguish workspaces for extension lockfile/local
  `.bzl` replay inputs and repository materialization state.
- Added/updated unit assertions proving different project roots produce
  distinct extension and repo execution keys. Also fixed a stale test-only
  import for the renamed bzl transitive digest helper.
- Validation: `cargo test -p slug_bzlmod project_root -- --nocapture` (5
  passed), `cargo check -p slug_bzlmod -p slug_common -p slug_client_ctx -p
  slug_server`, `cargo build -p slug`, and full
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py -rx` (12 passed / 1 xfailed)
  passed, with daemon processes cleaned up afterward.

Implementation slice 2026-05-18, Slug test target guardrail enablement:

- Made `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails`
  runnable in this OSS checkout instead of relying only on direct pytest.
- Declared the root module's historical apparent repo name with
  `module(repo_name = "fbcode")`, and exposed the existing Starlark shim tree as
  Bazel-9-compatible innate `new_local_repository` repos for `fbcode_macros` and
  `fbsource`. This avoids a `.buckconfig`/cell-alias workaround while keeping
  the repository visibility in `MODULE.bazel`.
- Added a focused `@fbcode//buck2/tests:buck_e2e.bzl` OSS shim for
  `buck2_e2e_test` that returns `ExternalRunnerTestInfo` and runs the existing
  pytest files through Slug's test runner. The wrapper discovers the real
  checkout root from the execroot before setting `TEST_EXECUTABLE`, so the
  Plan 61 guardrails exercise the freshly built `target/debug/slug`.
- Added missing lightweight OSS shim macros for `python_pytest` and `ci_hint`,
  and registered the existing `oncall` package metadata function as a BUILD
  global.
- Validation: `cargo build -p slug` passed, and
  `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails` now passes
  with the same expected guardrail result as direct pytest: 12 passed and 1
  xfailed.

Implementation slice 2026-05-18, root MODULE.bazel include inputs:

- Added root `MODULE.bazel` include expansion for literal repo-relative labels
  such as `include("//:deps.MODULE.bazel")`, constrained to Bazel 9's root
  include shape: labels must start with `//`, name a target, and the included
  basename must end in `.MODULE.bazel` without starting with `.`.
- Included module segments are read as filesystem inputs before MODULE
  evaluation and recorded with bzlmod parse counters, so editing a root include
  segment invalidates the graph instead of silently reusing the prior parse.
- Added a Rust parser unit test and promoted a Plan 61 guardrail that edits an
  included `deps.MODULE.bazel` file to add a local override module, then asserts
  bzlmod parse/resolution counters advance and the new module appears.
- Validation: `cargo test -p slug_bzlmod
  test_parse_module_bazel_expands_root_include -- --nocapture`, `cargo build
  -p slug`, focused
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py::test_included_module_segment_edit_invalidates_bzlmod_graph
  -rx`, full direct pytest (13 passed / 1 xfailed), and
  `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails` (13
  passed / 1 xfailed inside pytest, Slug test pass) all passed, with daemon
  processes cleaned up afterward.

Implementation slice 2026-05-18, include variable-scope parity:

- Replaced the first include implementation's textual pre-expansion with an
  actual `include()` MODULE global. It records include labels during file
  evaluation, then the parser evaluates each included segment in a fresh
  Starlark module while accumulating directives into the same module parse
  context.
- This matches the Bazel 9 `ModuleFileGlobals.include` contract that included
  files behave like segment placement for directives, but variable bindings
  such as `use_extension` proxies are local to the file where they occur.
- Added Rust coverage proving an extension proxy defined in an included segment
  is not visible to `use_repo()` in the root module, and promoted the same
  behavior as a Plan 61 guardrail.
- Validation: `cargo test -p slug_bzlmod parse_module_bazel -- --nocapture`,
  `cargo build -p slug`, focused
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py::test_included_module_segment_variables_do_not_leak_to_root
  -rx`, full direct pytest (14 passed / 1 xfailed), and
  `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails` (14
  passed / 1 xfailed inside pytest, Slug test pass) all passed, with daemon
  processes cleaned up afterward.

Implementation slice 2026-05-18, include create/delete guardrail:

- Promoted a Plan 61 guardrail for create/delete of an included root
  `*.MODULE.bazel` segment. The fixture first observes the expected failure
  when the included file is absent, creates the segment with a local override
  module and verifies bzlmod resolution succeeds, then deletes the segment and
  verifies the daemon no longer reuses the successful graph.
- This covers the 61.2 input lifecycle requirement for included module files
  beyond ordinary content edits.
- Validation: focused
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py::test_included_module_segment_create_delete_invalidates_bzlmod_graph
  -rx`, full direct pytest (15 passed / 1 xfailed), and
  `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails` (15
  passed / 1 xfailed inside pytest, Slug test pass) all passed, with daemon
  processes cleaned up afterward.

Implementation slice 2026-05-18, extension tag usage digest guardrail:

- Added a focused Plan 61.6 guardrail for module-extension tag attr changes.
  The fixture writes a valid replay lockfile whose `usagesDigest` matches the
  initial tag kwargs, proves the entry replays without executing the extension,
  edits the tag attr in `MODULE.bazel`, and asserts that the stale lockfile
  entry is rejected through `extension_replay_miss_reason` without another
  replay hit.
- Added a Python mirror of Slug's current versioned tag-input hash shape for
  this fixture. This is validation-only; the final 61.6 target still needs
  pinned Bazel-shaped `SingleExtensionUsagesValue` serialization, eval factors,
  and recorded-input validation.
- Validation: focused
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py::test_extension_tag_attr_edit_invalidates_or_rejects_replay
  -rx` passed, full direct pytest passed (16 passed / 1 xfailed), and
  `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails` passed
  with the same 16 passed / 1 xfailed pytest result. The remaining xfail is the
  same-daemon workspace isolation precondition.

Implementation slice 2026-05-18, replayed generated repo materialization
guardrail:

- Added a focused Plan 61.6 guardrail proving a valid lockfile
  `generatedRepoSpecs` entry is usable for an actual build target. The fixture
  writes a replay lockfile for a generated `local_repository`, makes the
  extension implementation fail if executed, then builds a root filegroup that
  depends on `@replayed_repo//:data`.
- The passing result proves the replay hit registers and materializes the
  generated repo without executing the extension on that path. This strengthens
  the earlier `audit cell` replay checks, which only proved cell visibility.
- Validation: focused
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py::test_valid_lockfile_replay_materializes_generated_repo_without_extension_eval
  -rx` passed, full direct pytest passed (17 passed / 1 xfailed), and
  `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails` passed
  with the same 17 passed / 1 xfailed pytest result. The remaining xfail is the
  same-daemon workspace isolation precondition.

Manager process correction 2026-05-18:

- The loop manager must not stop after this scaffold. The next loop action is
  to dispatch an implementer worker for the remaining 61.1 validation-first
  work, then continue with 61.2/61.3 workers or the next SDK smoke failure.
- A final summary after a single slice is not a valid stop condition for this
  plan. Valid stop conditions are full `//sdk:sdk_contents` Slug-vs-Bazel 9
  output parity, an explicit user stop, or a blocker-grade resume prompt with
  exact commands/logs/state.

SDK parity frontier 2026-05-18:

- The bounded SDK smoke progressed through bzlmod analysis and failed during
  execution of `reactor//sdk:license_comment` because action inputs under
  `external/rules_python/python/private/...` resolved through
  `external/rules_python -> bazel-external/rules_foreign_cc++ext+rules_python`
  instead of the direct module repo
  `bazel-external/rules_python+1.9.0`.
- Missing semantic: `external/<apparent>` layout for a name that is both a
  direct module repo and an extension-generated repo must prefer the Bazel
  module-form repository for source-file action paths. Extension spokes still
  keep their canonical symlinks; the apparent symlink is not an extension
  identity.
- Owning subsystem: `slug_core::cells` external symlink layout, currently
  reached from legacy bzlmod cell setup in
  `slug_common::legacy_configs::cells`; the target Plan 61 owner is
  `ExternalSymlinkLayoutKey { workspace_id, output_base, cell_graph_digest }`.
- Other affected targets/features: any action input path under
  `external/<module>` when a module name collides with a generated repo apparent
  name, including `rules_python` templates and similar ruleset source files.
- One-off workaround rejected: manually relinking
  `zeromatter-kuro/external/rules_python` or adding an SDK-specific alias.

Implementation slice 2026-05-18, external apparent symlink collision:

- Added a layout regression in `app/slug_core/src/cells.rs` proving
  `external/rules_python` chooses the direct module-form repository
  `rules_python+1.9.0` when an extension spoke
  `rules_foreign_cc++ext+rules_python` also exists, while the canonical
  extension symlink still points at the extension repo.
- Updated `ensure_external_symlink` to compute the preferred apparent target
  before comparing or creating symlinks. Apparent names with a module-form
  repo now select that module target; canonical extension cell names are left
  untouched.
- Added a final `repair_external_symlink_targets` pass after legacy bzlmod cell
  and alias symlink setup so stale links from previous ordering are corrected
  once all module and extension repositories are visible.
- Validation: `cargo test -p slug_core external_symlink -- --nocapture`
  passed, `cargo fmt --check` passed, `cargo check -p slug_core -p
  slug_common` passed, and `cargo build -p slug` passed. The focused
  `zeromatter-kuro` build
  `/var/mnt/dev/slug/target/debug/slug --isolation-dir
  sdk-license-20260518-155252 build //sdk:license_comment` passed and repaired
  `external/rules_python` to the `rules_python+1.9.0` source tree.
- Full SDK smoke with `scripts/memory_smoke.sh` and isolation
  `sdk-parity-20260518-155728` no longer hit the previous `rules_python`
  template-read failure. It timed out after 900s while actively executing
  LLVM/libcxx/musl/compiler-rt actions, with about 2,560 actions remaining and
  peak sampled RSS above 6.8 GiB. This is progress evidence, not acceptance.

Previous implementer delegation prompt, completed by the stale-marker slice:

```text
Repo: /var/mnt/dev/slug. Active plan:
thoughts/shared/plans/slug-bazel-subplans/61-true-dice-bzlmod.md, phase 61.1/61.2 boundary.
Preserve existing dirty work unless it is yours to integrate.

Task: convert
`test_stale_repo_marker_does_not_mask_changed_repo_spec_or_outputs` into a
focused stale-marker fixture. The fixture should exercise repository
materialization reuse with a changed generated repo spec or changed expected
outputs, assert that a bare `.slug_repo_complete` marker is insufficient, and
pin expected Bazel behavior to Bazel 9 `DigestWriter`, `RepoRecordedInput`, and
`RepositoryFetchFunction` sources. If current Slug trusts the marker, convert
to strict xfail with the exact current bug shape. Do not start broad 61.3
graph-resolution work and do not convert the broad no-stub guardrail.

Current validation state: `cargo check -p slug_cmd_audit_client -p
slug_cmd_audit_server -p slug_bzlmod`, `cargo test -p slug_bzlmod
dice_graph::tests:: -- --nocapture`, `cargo build -p slug`,
`TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
tests/core/bzlmod/test_plan61_guardrails.py` (3 passed / 2 skipped / 8 xfailed), and
focused unknown-repo-rule `-rx` (1 xfailed with the unknown generated repo spec
stub fallback reason) all pass. `./slug test
tests/core/bzlmod:test_plan61_guardrails` is not a blocker for this slice; it
fails before test execution because the local cell resolver lacks the
`@fbcode` alias used by the repo's e2e test macro load.

Before broad Slug commands, clean slugd with the targeted cleanup from the loop
manager prompt. Run focused tests and report exact commands, statuses, changed
files, converted guardrail behavior, remaining skips, final daemon state, and
the next worker-sized task.
```

Class boundary for this slice:

- Missing Bazel semantic: DICE-owned bzlmod graph identity and observable
  invalidation/replay/materialization events.
- Owning Slug subsystem: `slug_bzlmod` graph/replay/materialization keys, with
  legacy startup integration in `slug_common::legacy_configs::cells`.
- Other affected targets/features: any MODULE.bazel build using remote modules,
  local overrides, module extensions, generated repos, repository rules,
  lockfile replay, or warm daemon reuse.
- One-off workaround rejected: adding SDK-specific labels, repo-name aliases,
  marker repairs, or repository stubs to advance `//sdk:sdk_contents`.

Checked inventory for current semantic authorities:

| Current authority / anti-pattern | Current call sites | Future owner |
|---|---|---|
| Pre-DICE root parse/resolution | `app/slug_common/src/legacy_configs/cells.rs::resolve_bzlmod_dependencies` calls `parse_module_bazel`, `resolve_local_modules`, `MvsResolver::resolve`, extension aggregation, repo mapping, and cell registration before injecting `CellResolverKey`. | `RootModuleFileKey`, `ModuleFileKey`, `BzlmodResolutionKey`, `BzlmodCellGraphKey`. |
| Injected cell resolver | `app/slug_common/src/dice/cells.rs::CellResolverKey` stores the already-built resolver. | `BzlmodCellGraphKey { workspace_id, resolution_digest }` consumed by `CellResolverKey`. |
| Module version adapter | Process-global `MODULE_VERSIONS` / `set_module_versions` / `get_module_version` were removed on 2026-05-20. Current startup resolution returns `BzlmodSessionData::module_versions`, injected through `BzlmodSessionDataKey` and read by interpreter cell info. This is command-scoped, but still produced by pre-DICE legacy resolution. | `ResolvedModuleIdentity` projection keyed by workspace and module key. |
| Toolchain/platform adapter | Process-global `REGISTERED_TOOLCHAINS` and `REGISTERED_EXECUTION_PLATFORMS` were removed on 2026-05-20. Current startup resolution stores them in `BzlmodSessionData`, and toolchain/execution-platform consumers compute `BzlmodSessionDataKey`. This is command-scoped, but still produced by pre-DICE legacy resolution. | `RegisteredToolchainsKey` and `RegisteredExecutionPlatformsKey` projections from `BzlmodResolutionKey`. |
| Extension aggregation adapter | Process-global `EXTENSION_AGGREGATIONS` and `set_extension_aggregations` were removed on 2026-05-20. Current startup resolution stores aggregations in `BzlmodSessionData`; lazy extension repo paths compute `BzlmodSessionDataKey` before constructing `ModuleExtensionExecutionKey`. | `ModuleExtensionAggregationKey`. |
| Partially scoped extension execution key | `ModuleExtensionExecutionKey` now includes `project_root` in `Hash`/`Eq`, but still lacks the final typed `workspace_id`, command policy digest, and replay input digest shape. | `ModuleExtensionExecutionIdentity { workspace_id, extension_instance_id, command_policy_digest, replay_inputs_digest }`. |
| Partially scoped repo execution key | `ExtensionRepoExecutionKey` now includes `project_root` in `Hash`/`Eq`, but still lacks the final typed `workspace_id`, repo rule implementation digest, and replay/recorded-input identity. | `ExtensionRepoExecutionIdentity` plus `RepoMaterializationManifestKey`. |
| Lockfile direct reader | Process-global `LOCKFILE_CACHE` and `cached_lockfile` were removed on 2026-05-20. Current readers are explicit `read_lockfile_with_mode` / `read_lockfile_path_with_mode`; they honor `LockfileMode::Off` and re-read disk, but are not yet DICE `LockfileContentKey` values. | `LockfileContentKey` and `LockfileExtensionEntryKey`. |
| Bare marker trust | `.slug_repo_complete` checks in `repository_executor.rs`, `repository_execution.rs`, `spoke_materialization.rs`, and `pending_repo_cells.rs` comments/bridges. | `RepoMaterializationManifestKey` or Bazel-shaped marker/recorded-input validation. |
| Stub fallback | Normal bzlmod stub fallback paths and the `stub_fallback_attempt` counter were removed on 2026-05-20. Guardrails now assert direct extension/repo-rule failures and no generated stub repo directories/markers. | Keep direct Bazel-shaped failure; no replacement compatibility path. |
| Dynamic cell globals | `DYNAMIC_PROJECT_ROOT`, `DYNAMIC_EXTENSION_CELLS`, `DYNAMIC_EXTENSION_CELL_SETUPS`, `DYNAMIC_EXTENSION_CELL_ALIASES`, `SCOPED_BZLMOD_REPO_ALIASES`, and `BZLMOD_APPARENT_ALIAS_CACHE` in `app/slug_core/src/cells.rs` remain temporary adapters. They are now cleared when the project root changes. `ROOT_CELL_NAME` is no longer write-once; `SPOKE_REGISTRY` / `SEEDED_EXTENSIONS` in `spoke_materialization.rs` are also root-scoped. | `BzlmodCellGraphKey`, `RepoMappingKey`, `ExternalSymlinkLayoutKey`, and `ExtensionSpokesKey`. |
| Stale local-path repair | `repository_execution.rs::repair_stale_local_path_attrs` and its suffix-search helper were removed on 2026-05-20; stale absolute paths now fail in the owning repo-rule execution path. | `LocalOverrideSourceKey` and repo-rule recorded input validation. |

### 61.2 Parsed Module Inputs As DICE Keys

- Move root/transitive `MODULE.bazel` parsing, include files, registry module
  files, local overrides, source JSON, patches/overlays, auth/offline/cache
  policy, and archive integrity behind DICE keys. Command-sensitive policy
  belongs in `BzlmodCommandPolicyKey`; stable workspace identity does not change
  just because `--lockfile_mode`, registry list, or dev-dependency flags change.
- Model create/edit/delete of root and local override `MODULE.bazel` files.
- Preserve current BCR/local override behavior while changing ownership.

Grounding: Bazel `ModuleFileValue`, `ModuleFileFunction`, `IndexRegistry`, and
`RepoSpecFunction`; current Slug source-kind registration in legacy `cells.rs`.

Exit criteria:

- Editing or creating/deleting root `MODULE.bazel`, included module files, local
  override module files, registry metadata, or `source.json` invalidates only the
  relevant parsed/resolution keys.
- Named experiment `plan61-module-input-dice-diff` compares pinned Bazel and
  Slug behavior.

### 61.3 Resolved Bzlmod Graph As A DICE Value

- Move discovery, MVS, overrides, selected/yanked versions, dev/root filtering,
  command policy, Bazel-version compatibility checks, and module metadata into
  `BzlmodResolutionKey`.
- Match pinned Bazel behavior for compatibility-level conflicts and multiple
  version override selection.
- Project toolchain/platform registrations from the graph.

Grounding: Bazel `BazelModuleResolutionFunction`, `Selection`,
`BazelDepGraphValue`, `RegisteredToolchainsFunction`, and
`RegisteredExecutionPlatformsFunction`.

Exit criteria:

- Compatibility-level conflicts fail like pinned Bazel.
- Two workspaces with the same module names but different versions/overrides in
  one daemon get distinct resolution values, module metadata, and registrations.
- Selected yanked-version behavior matches pinned Bazel or is explicitly
  classified as a remaining parity gap.

### 61.4 Shadow Cell Graph, Then Switch

- First add `BzlmodCellGraphKey` as a shadow projection and compare it against
  current startup-built cells.
- Switch `CellResolverKey` to the DICE-owned graph only after repo mapping,
  extension replay, and materialization tests cover the dynamic-cell cases.
- Do not delete dynamic/global adapters in this phase.

Grounding: Bazel does not have Slug cells; `BazelDepGraphValue` grounds
canonical repository graph semantics. Slug-specific removal is grounded in
Plan 38 and the current dynamic cell globals.

Exit criteria:

- Shadow and current cell graphs match for existing bzlmod fixtures.
- `@bazel_tools` remains an explicit bundled/well-known repo entry.
- No filesystem scan or mutable `CellResolver::get` insertion is the authority
  for canonical repo identity.

### 61.5 Canonical Identity And Repo Mapping

- Define `ResolvedModuleIdentity`, `ModuleExtensionId`, `ExtensionUniqueName`,
  `RepoOriginKind`, and `RepoMappingScope`.
- Implement module repo canonical names, multiple-version identity, `repo_name`,
  extension ids from canonical `.bzl` labels, isolated usages, generated repo
  names, `inject_repo`, `override_repo`, `use_repo`, and `use_repo_rule`.
- Use typed internal identities even where Bazel source currently performs
  prefix checks; externally preserve pinned Bazel canonical label behavior.

Grounding: Bazel external docs for canonical/apparent repo names, `ModuleKey`,
`ModuleExtensionId`, `BazelDepGraphFunction`, `ModuleExtensionUsage`,
`SingleExtensionValue`, `BazelDepGraphValue`, and
`ModuleExtensionRepoMappingEntriesFunction`.

Exit criteria:

- Tests cover single-version and multiple-version module canonical names.
- Extension-generated repos see host module visible repos plus same-extension
  generated repos with Bazel precedence. Include a conflict test where host
  visible `@foo` and generated `foo` both exist.
- `inject_repo` and `override_repo` resolve through root/module mappings,
  validate bad targets, handle positional/kwargs forms where Bazel does, and do
  not materialize overridden generated repos.
- `use_repo_rule` is modeled as an innate extension with invocation-level
  `dev_dependency` and current-module visibility.

### 61.6 DICE-Owned Extension Aggregation And Replay Inputs

- Replace `EXTENSION_AGGREGATIONS` with workspace-scoped DICE values.
- Compute real `.bzl` transitive digest from loaded extension modules. The
  current project-root-aware digest now catches local workspace direct and
  literal transitive `.bzl` edits, but the final value still needs Bazel-shaped
  loaded-module ownership across external/module repos and command semantics.
- Match pinned Bazel `usagesDigest` serialization, including module keys, eval
  factors, isolate status, and tag order where Bazel treats order as relevant.
- Include Bazel release id, Starlark semantics, repo env, non-strict repo env,
  timeout/process/remote-executor policy, and OS/architecture eval factors in
  the command/replay inputs according to pinned Bazel source. The lockfile docs
  state the visible lockfile is Bazel-version-specific; the implementation
  still needs source and experiment checks for which other policy changes affect
  replay versus execution.
- Validate recorded inputs using Bazel `RepoRecordedInput.WithValue` semantics:
  file, directory entries, directory tree, env var, and repo mapping entries.
- Facts are passed to execution/replay values and returned in
  `ModuleExtensionResult` or a sibling DICE value, but fact contents are not
  normal replay invalidators. In lockfile `ERROR` mode, match Bazel's
  post-execution facts validation behavior.

Grounding: Bazel `SingleExtensionUsagesFunction`,
`SingleExtensionUsagesValue`, `RegularRunnableExtension`,
`SingleExtensionEvalFunction`, `RepoRecordedInput`, `ModuleExtensionContext`,
and facts docs.

Exit criteria:

- Editing an extension `.bzl`, transitive loaded `.bzl`, tag attr, env input,
  file/dir/tree input, or recorded repo mapping rejects replay or re-executes as
  pinned Bazel does.
- Legitimate empty `generatedRepoSpecs` can replay; empty specs alone are not
  treated as stale historical stub data.
- Valid lockfile replay registers generated repos without executing the
  extension.
- Ordinary read-only Slug paths do not mutate the visible lockfile.

### 61.7 Repo Execution And Materialization Authority

- Replace Slug's bare `.slug_repo_complete` trust with Bazel-shaped
  marker/recorded-input validation or an equivalent Slug DICE manifest. A Slug
  output-tree manifest is a stronger internal correctness mechanism, not direct
  Bazel parity unless a Bazel experiment proves output-content validation.
- Repository rules do not declare outputs like normal actions. Correctness must
  come from recording `repository_ctx` side effects plus final tree digest, or
  from scanning an isolated repo output tree after execution.
- Track file digest, executable bit, directory entries, symlink target text,
  deleted/extra paths, repo-rule implementation `.bzl` transitive digest, and
  repo-rule recorded env/file/dir/repo-mapping/PATH inputs.
- Remove stale absolute-path repair, generated BUILD repair, empty-target label
  repair, and stub repo fallback from normal paths.

Grounding: Bazel `DigestWriter`, `RepositoryFetchFunction`,
`RepoRecordedInput`, `StarlarkBaseExternalContext`, and current Slug marker,
repair, and stub paths in `repository_execution.rs`, `repository_executor.rs`,
and `extension_repo.rs`.

Exit criteria:

- A valid marker with a deleted/corrupted file, changed mode, stale symlink, or
  stale recorded input rematerializes or fails directly.
- Unknown repo rule, repo-rule failure, extension failure, DICE error, missing
  generated repo, and lazy Label materialization failure create no stub repo, no
  `.slug_repo_complete` stub marker, and no downstream "No such file" masking.
- `rg "Creating stub|materialize_stub_repo|stub_marker"` has no normal bzlmod
  path remaining.
- `module_ctx.path(Label)` / `module_ctx.read(Label)` and
  `repository_ctx.path(Label)` / `repository_ctx.read(Label)` plus
  Label-accepting operations such as `symlink`, `template`, `patch`,
  `load_wasm`, and `execute_wasm` propagate materialization failure. Plain
  `execute()` has string args; tests should cover environment, `PATH`, and
  working-directory recording separately rather than claiming `execute(Label)`.
- Symlink tests cover relative symlink, absolute in-workspace symlink, absolute
  out-of-workspace symlink, broken symlink, Windows fallback, and non-symlink
  collision in `buck-out/v2/external_cells`.
- Missing absolute local paths fail; no suffix repair; local source content
  changes invalidate; cache corruption is detected or discarded deterministically.

### 61.8 Facts And Lockfile Policy

- Implement mode-aware lockfile semantics: `update`, `refresh`, `error`, and
  `off` are separate policy inputs. Until exact Bazel write parity exists,
  ordinary Slug build/query/audit paths use the interim read-only safety policy.
- Replace process-wide `cached_lockfile` with `LockfileContentKey` values keyed
  by workspace/hidden path, existence, digest, and parse result. The consuming
  replay/resolution keys are mode-aware; in `off` mode they must not read or
  update either lockfile, matching Bazel 9 docs.
- Read workspace/hidden lockfile facts where Bazel does; expose
  `module_ctx.facts`; accept and carry `extension_metadata(facts = ...)`.
- Persist newly returned facts/specs only through an explicit future
  lockfile-update path with a write capability such as
  `LockfileWritePurpose::ExplicitModUpdate`, or a Slug-owned cache documented as
  non-Bazel-visible. Ordinary paths cannot call `Lockfile::write`.

Grounding: Bazel `RepositoryOptions`, `BazelLockFileModule`,
`BazelLockFileFunction`, `SingleExtensionEvalFunction`,
`ModuleExtensionContext`, `ModuleExtensionMetadata`, `Facts`, and Slug Plan 57.

Exit criteria:

- Named experiment `plan61-lockfile-modes` records pinned Bazel and Slug SHA
  behavior for default/update, refresh, error, and off.
- Creating, deleting, and editing `MODULE.bazel.lock` after a prior daemon
  miss/hit invalidates the DICE lockfile value.
- `module_ctx.facts` and returned metadata survive through DICE values even when
  ordinary commands do not persist them.

### 61.9 Delete Transitional APIs

- Delete or quarantine bzlmod globals, marker trust paths, stub repo fallbacks,
  path repair heuristics, and manual cache invalidation hooks.
- Keep only compatibility readers needed for old test fixtures if they do not
  affect normal Bazel 9 behavior.

Exit criteria:

- `rg` for the global names above finds no semantic use in normal paths.
- Stub repo materialization is impossible for bzlmod extension/repo failures.

### 61.10 Real-World And Warm-Daemon Validation

- Add focused Rust unit tests for every new key/value.
- Add Python integration tests for daemon invalidation and lockfile policy.
- Add real-world smoke coverage for rules_cc, rules_python, rules_rs/rules_rust,
  bazel_features, and a bounded zeromatter target.
- Run named experiment `plan61-warm-daemon-noop`: instrument warm query/build to
  assert no unchanged module resolution, extension evaluation, or repo
  materialization reruns.

Implementation slice 2026-05-18, SDK performance frontier:

- After the apparent-repo symlink fix, `//sdk:sdk_contents` no longer fails in
  `rules_python` template expansion, but Slug analysis remains non-parity. The
  bounded smoke `sdk-parity-20260518-155728` entered execution only after
  minutes of analysis and the resumed run still spent roughly 8 minutes between
  daemon connection and first execution progress. Bazel completes analysis for
  this repository in under 10 seconds, so an excessively long Slug analysis run
  is now classified as a semantic SDK parity failure, not a timeout to wait out.
- The same smoke showed configuration multiplication: log aggregation found
  many distinct output hashes for the same logical platform, especially
  `//bazel/platforms:linux-gnu-host` and `//bazel/platforms:linux-musl`, and
  repeated analysis of LLVM/runtime labels such as
  `llvm++llvm_source+libcxx//:headers`,
  `llvm++llvm_source+compiler-rt//:builtins`, and rust toolchain labels across
  those hashes. The next implementation slice must focus on configuration
  canonicalization/transition ownership before any longer SDK smoke.
- Loop policy update: SDK smokes and focused repros must be bounded tightly
  enough to expose analysis stalls. A long run with ongoing compilation is still
  useful after analysis parity is restored, but a long pre-execution analysis
  phase is its own failure mode.

Implementation slice 2026-05-18, transitioned `flag_values` and cold-start
frontier:

- Fixed native `config_setting(flag_values = ...)` analysis so select matching
  analyzes the `config_setting` in the configuration being matched, not in
  Slug's unbound configuration-rule analysis cfg. This matches the existing
  alias/config-setting-group path and lets transitioned build settings affect
  `select()` keys. Regression coverage:
  `test_config_string_build_setting_after_transition` uses a Starlark
  transition to set `//:my_string_flag` and verifies the transitioned
  `config_setting` wins.
- Repaired the previous apparent external symlink fix's startup cost by
  indexing preferred `bazel-external/<module>+<version>` targets once per
  `external/` repair pass instead of rescanning `bazel-external` for each
  symlink. The focused Rust symlink tests cover the module-form preference and
  chain repair behavior.
- Validation for this slice: `cargo fmt --check`; `cargo test -p slug_core
  external_symlink -- --nocapture`; `cargo check -p slug_core -p
  slug_configured`; `cargo build -p slug`; and
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug timeout 120s python -m
  pytest -q tests/core/analysis/test_native_rules.py -k
  'config_string_build_setting'`.
- Fresh zeromatter probe `sdk-connect-probe-165019` still took 23 seconds
  between `Starting new slug daemon...` and `Connected to new slug daemon`
  (`/tmp/sdk-connect-probe-165019.log`) before returning
  `audit bzlmod-counters`. The counters show one bzlmod resolution and 428
  module parses. Top-level `external`/`bazel-external` max-depth walks alone
  were not enough to explain the delay: `find external -maxdepth 1 -type l`
  counted 12,827 symlinks in 0.010s and `find bazel-external -maxdepth 1`
  counted 7,425 entries in 0.013s.
- Follow-up timing probes split that startup delay into two costs. First,
  legacy bzlmod/cell startup was roughly 4.0s, dominated by pre-DICE extension
  cell registration and external symlink repair. Second, the open-source
  `notify` file watcher spent roughly 4.6s recursively walking generated
  workspace trees before the daemon became ready. The zeromatter workspace's
  `.bazelignore` did not ignore `buck-out`, `bazel-external`, or `external`;
  local counts found roughly 84,959 directories under `bazel-external` and more
  than 209,299 directories under the current `buck-out` tree.
- Fixed the immediate startup hang by applying the existing reserved-output
  component filter during `notify` watch installation, not only when processing
  events. The watcher no longer descends into `buck-out`, `bazel-*`, `external`,
  or `execroot` during daemon startup. Validation: `cargo fmt --check`; `cargo
  test -p slug_file_watcher notify -- --nocapture`; `cargo build -p slug`; and
  bounded zeromatter probe `sdk-connect-post-watchfix-171305`.
- After the watcher fix, `sdk-connect-post-watchfix-171305` completed
  `audit bzlmod-counters` in 8.8s end to end and connected the daemon in about
  4.35s (`/tmp/sdk-connect-post-watchfix-171305.log`). This removes the
  generated-tree startup hang and brings the read-only audit command under the
  10s analysis budget. Remaining known cost: about 4s of legacy bzlmod/cell
  setup still happens before daemon readiness. Plan 61's DICE-owned bzlmod graph
  should retire that startup work by moving semantic module/extension state
  behind workspace/command keys, but further Plan 61 implementation should use
  isolated unit/integration fixtures and keep real-world zeromatter smokes
  bounded until this remaining startup work is proven not to regress.

Implementation slice 2026-05-18, lockfile spoke preseed performance:

- Root caused the post-watchfix narrow-target stall to legacy lockfile spoke
  preseeding. With `SLUG_MEMORY_CHECKPOINTS=1`, the focused
  `@rules_rust//ffi/rs:empty_allocator_libraries --target-platforms=//bazel/platforms:linux-musl`
  repro showed startup parsing `MODULE.bazel.lock`, then
  `bzlmod_pre_compute_extension_repo_cells_from_lockfile` adding 2,350
  extension-internal repos to the static pre-DICE cell graph. The follow-on
  `legacy_cells_bzlmod_precomputed_repos` checkpoint built 2,730 precomputed
  cells and 3,181 aliases before target analysis. This was the dominant
  "Synchronizing slug internal state" cost and is a Plan 61 ownership bug:
  lockfile-generated spokes are lazy extension/repo materialization state, not
  startup cell-graph authority.
- Changed lockfile-only generated spokes to register in the dynamic
  extension-cell/spoke registries with their `ExtensionRepoCellSetup` and
  `RepoSpec`, without appending them to the static startup `CellResolver` or
  creating every `external/` symlink up front. Explicit `use_repo()` cells still
  remain in the static precomputed list. When an internal spoke is actually
  referenced, `CellResolver::get` promotes the dynamic entry and installs the
  symlink on demand.
- Validation: `cargo fmt --check`; `cargo check -p slug_core -p slug_common`;
  `cargo test -p slug_core dynamic_extension_cell -- --nocapture`; `cargo
  build -p slug`; focused replay guardrail
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py::test_valid_lockfile_replay_materializes_generated_repo_without_extension_eval
  -rx`; checkpointed audit `sdk-audit-lazy-lockfile-mem-172738`; and focused
  zeromatter repro `rules-rust-empty-lazy-lockfile-mem-172841`.
- Post-fix checkpoints show `legacy_cells_bzlmod_precomputed_repos` remains at
  380 precomputed cells / 401 precomputed aliases, and the final cell resolver
  is 458 cells / 831 aliases instead of 2,808 cells / 3,181 aliases. The
  focused Rust allocator target still succeeds and dropped from 43.4s to about
  22.1s end to end. Remaining non-parity cost is duplicated legacy bzlmod/cell
  setup around daemon bootstrap plus real target/toolchain analysis; longer SDK
  smokes must remain bounded and should not be treated as progress if analysis
  exceeds the <10s Bazel baseline.
- Bounded SDK smoke `sdk-build-lazy-lockfile-173003` still timed out at 45s
  after connecting in about 4.1s. It reached the same analysis frontier:
  `rules_rust//ffi/rs:empty_allocator_libraries`
  `(//bazel/platforms:linux-musl#f9d25665faba5414)` running in `evaluate_rule`
  with 37-38 other analyses. This confirms the static lockfile-cell fanout was
  one root cause of the hang, but not the whole performance discrepancy. The
  next performance slice should focus on the remaining SDK cfg/toolchain
  analysis multiplication, using the isolated allocator repro and memory
  checkpoints rather than long SDK waits.

Implementation slice 2026-05-18, optional C++ toolchain shim deferral:

- Root caused the remaining focused allocator stall to eager optional C++
  native-shim metadata materialization. `rules_rust//ffi/rs:empty_allocator_libraries`
  declares `@bazel_tools//tools/cpp:toolchain_type` with `mandatory = False`
  and its implementation only asks for the Rust toolchain, but Slug eagerly
  analyzed the optional C++ native-shim provider while constructing
  `ctx.toolchains`. Memory checkpoints from
  `rules-rust-empty-lazy-lockfile-mem-172841` showed the target stuck in
  `ctx_toolchain_provider_analysis_start` / `analysis_deps_start`, with fanout
  through LLVM, compiler-rt, libcxx, musl, and `rules_cc` link metadata before
  the rule implementation needed those providers.
- Added build-setting visibility to `audit configurations` and to `cfg_diff`.
  The direct allocator configuration `41d9087ec4cb7f34` and SDK-style
  `f9d25665faba5414` had the same musl constraints; the SDK-style hash only
  added `slug_settings//command_line_option:platforms =
  reactor//bazel/platforms:linux-musl`. A third configuration,
  `6992643dfb6c8fa2`, carried the LLVM/C++ build-setting envelope. This
  disproved the hidden-constraints theory and made the remaining discrepancy
  visible as build-setting/toolchain analysis fanout.
- Changed optional C++ native-shim toolchain entries to defer provider
  construction by storing `None` for that optional entry. Mandatory C++
  toolchains still use the eager metadata/runtime path. Existing
  `ResolvedToolchains::at` behavior returns the target-platform native shim on
  Starlark access, so optional users that actually query
  `ctx.toolchains["@bazel_tools//tools/cpp:toolchain_type"]` still receive the
  native shim; the new unit test
  `optional_cpp_toolchain_without_eager_provider_still_returns_shim` covers
  that fallback and checks the musl libc overlay.
- Validation: `cargo fmt --check`; `cargo check -p slug_analysis -p
  slug_build_api -p slug_cmd_audit_client -p slug_cmd_audit_server -p
  slug_core`; `cargo test -p slug_build_api
  optional_cpp_toolchain_without_eager_provider_still_returns_shim --
  --nocapture`; and `cargo build -p slug` passed. The focused SDK-style
  allocator repro
  `@rules_rust//ffi/rs:empty_allocator_libraries --target-platforms=//bazel/platforms:linux-musl --//command_line_option:platforms=reactor//bazel/platforms:linux-musl`
  passed in bounded isolation (`allocator-f9-defer-174441`, 14.5s end to end,
  down from 21.6s before this slice and 43.4s before lazy lockfile spokes).
  The follow-up memory run `allocator-f9-defer-mem-174513` showed the root
  allocator target's toolchain-provider phase lasting only same-second
  milliseconds and no LLVM/libcxx/compiler-rt fanout in the active snapshots.
- Bounded SDK smoke `sdk-build-defer-183959` still timed out at 45s, but the
  frontier moved from allocator analysis to `zstd//:zstd`
  `(//bazel/platforms:linux-musl#f9d25665faba5414)`. The focused zstd repro
  `zstd-f9-repro-184103` reached local C compilation
  (`Compiling external/zstd/lib/decompress/zstd_decompress.c`) and timed out
  with execution actions in flight, not analysis. This resolves the allocator
  analysis hang class. The next loop action should classify the remaining SDK
  delay as execution throughput versus any new analysis frontier; longer
  smokes remain bounded, and any future pre-execution analysis over the Bazel
  <10s baseline is still a Plan 61 performance failure.

Post-commit SDK frontier 2026-05-18:

- A longer bounded SDK smoke after `aad0f68b`
  (`sdk-build-post-defer-184554`) still timed out at 90s. It connected in
  about 4.2s, then repeatedly reported `zstd//:zstd`
  `(//bazel/platforms:linux-musl#f9d25665faba5414)` in `evaluate_rule` from
  18:46:35 through 18:47:18. This confirms the allocator fix did not finish
  SDK analysis parity and that long analysis remains an error, not a run to
  wait out.
- The focused checkpoint smoke `sdk-zstd-analysis-mem-184805` timed out at 45s
  after recording roughly 22k completed analyses and hundreds of active
  analyses. The final snapshots showed the oldest root waiting on broad SDK
  dependency chains (`sdk_contents`, `sdk_with_configs`,
  `sdk_with_data_configs`, `sdk/config_install`, `sdk/zeromatter_ffi`,
  `tools/zerobuf_cli`, `zm_cli`) rather than a single zstd local action.
- The hot configuration hashes in the checkpoint log were four host hashes
  plus the musl hash: `70eb5fc63ec6a4d3`, `8c57282663e321e1`,
  `097238d3cfd5694a`, `8973e623602a72ce`, and `f9d25665faba5414`. Many Rust
  crate targets and the default Rust toolchain appeared under multiple hashes,
  with Starlark samples dominated by `rules_rust` and `rules_cc`
  `cc_info.bzl` / `create_library_to_link.bzl`. The optional C++ deferral is
  active in this run (`optional_cc_toolchain_native_shim_deferred` appears for
  crate targets), so the next root cause is configuration multiplication and
  Rust/CC provider analysis cost, not the previous eager optional-shim path.
- Next action: use `audit configurations` / `cfg_diff` on the hot host hashes
  to identify which build settings are splitting otherwise equivalent host
  configurations, then create the narrowest focused repro around that
  transition/config-setting boundary. Do not proceed to broader 61.2/61.3
  ownership work until this SDK analysis multiplication is classified or
  clearly proven to be resolved by the DICE-owned bzlmod phase itself.

Transition/config canonicalization and SDK performance frontier
2026-05-18/2026-05-19:

- Empty `//command_line_option:platforms` transition inputs are now exposed to
  Bazel-style Starlark transitions as an empty list, and empty platform
  transition outputs are pruned from `ConfigurationData`. Non-empty platform
  outputs remain explicit build settings.
- Bazel-style transition output labels are now contextualized through the
  defining `.bzl` cell for magic-object transitions, so LLVM outputs such as
  `//config:ubsan` resolve to `llvm//config:ubsan` instead of the synthetic
  storage cell `slug_settings//config:ubsan`.
- Empty command-line-option values from process-global Starlark flags are
  pruned before entering configurations. Transition outputs equal to a
  build-setting rule's `build_setting_default` are also pruned. Default lookup
  is based on the output target's `build_setting_default` attr instead of the
  earlier rule-kind guard, because the SDK LLVM flags prove the attr is the
  semantic default source Slug can currently observe.
- Focused diagnostics proved sanitizer defaults are pruned at the transition
  frontier: direct `zstd//:zstd` analysis logged default hits/prunes for
  `llvm//config:{asan,ubsan,...}` and completed in about 21s wall clock with
  reported phases `load=7.8s analyze=5.5s execute=2.9s materialize=1.1s
  total=13.1s`. The full SDK timeout is therefore not an isolated zstd
  transition hang.
- SDK smoke/audit progression after the config fixes:
  `default-pruned-settings-191619` timed out at 45s with 271 interned
  configurations, 198 host / 73 musl, zero empty command-line settings, and
  zero `slug_settings//config:*` settings. Removing the default-attr
  `is_build_setting` guard left the same 271-config frontier. The earlier
  pre-fix audits were 448 then 297 configurations, so the current remaining
  failure is no longer the empty-platform or synthetic-LLVM-label class.
- The active timeout frontier remains
  `zstd//:zstd (//bazel/platforms:linux-musl#f9d25665faba5414) -- running
  analysis [evaluate_rule]`, but auditing that config shows no sanitizer
  defaults. It contains non-default LLVM settings
  `empty_sysroot=False` and `experimental_stub_libgcc_s=True`, rules_rust
  settings from `.bazelrc`, `compilation_mode=fastbuild`, and a non-empty
  `slug_settings//command_line_option:platforms` value.
- Memory checkpoints before and after the depset fast-path both classify the
  timeout as broad Starlark analysis pressure, not deadlock: about 18,160
  analyses completed, 352 analyses remained active, and the oldest root
  request was `reactor//sdk:sdk_contents` waiting through SDK/Rust dependency
  chains. Hot samples are dominated by `rules_rust+0.69.0` files
  `rust/private/rust.bzl`, `rust/private/utils.bzl`, and
  `rust/private/rustc.bzl`, with some `rules_cc` provider construction. The
  post-fast-path checkpoint still reached about 358k depset creations, max
  direct length 2359, max transitive length 3866, and max depth 20.
- Implemented one safe depset validation fast path: if a transitive child
  depset already carries `element_type`, Slug merges that recorded type
  directly instead of allocating a new visited set and recursively walking the
  child. This preserves the empty/unknown-type path and mutable direct-element
  validation, but it did not move the 45s SDK frontier enough to complete.
- Additional focused depset hot-path fixes were validated after the checkpoint:
  empty/single direct lists no longer allocate dedupe state, direct element
  hashability is not checked twice after creation-time dedupe already validated
  it, direct-only `to_list()` bypasses nested traversal and hash-set
  deduplication, and frozen direct-only depsets can reuse the existing flattened
  cache. These preserve depset immutability, hashability validation, order, and
  transitive traversal semantics.
- Added one more constructor shortcut for the `rules_cc` `_flat_depset` shape:
  `depset(transitive = [...])` with exactly one non-empty child of the same
  order now returns that child directly instead of wrapping it with many empty
  children. Validation still parses and checks every transitive child order.
- Found and fixed a second transition/default-retention bug after the depset
  work: `TransitionId::AnonymousBazel` bypassed the default-output pruning used
  by assigned Bazel-style transitions. This kept default LLVM settings such as
  sanitizer `false`, `source=prebuilt`, and `linkmode=dynamic` in many configs.
  Anonymous Bazel transitions now resolve relative outputs through the defining
  `.bzl` cell and prune outputs equal to build-setting defaults.
- Also pruned default `@bazel_tools//tools/cpp:compilation_mode=fastbuild` at
  CLI build-setting ingestion. Absence still reads as fastbuild via the existing
  fallback; explicit `opt`/`dbg` remain configuration-distinct.
- Validation for the depset hot-path series: `cargo fmt --check`,
  `cargo check -p slug_build_api`, `cargo build -p slug`,
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/analysis/test_depset_order.py`, and `git diff --check` passed.
  Repeated bounded SDK smokes still timed out at 45s:
  `depset-fastpath-sdk-193438`, `depset-direct-fastpath-sdk-193935`,
  `depset-tolist-fastpath-sdk-194229`, `depset-frozen-cache-sdk-194525`, and
  `depset-emptychild-sdk-195120`. The frozen-cache smoke audit dropped from
  271 to 265 interned configurations (198 host / 67 musl), but the latest fresh
  single-child-constructor smoke was back at 271 configurations (198 host /
  73 musl), zero `slug_settings//config:*`, 27 non-empty
  `slug_settings//command_line_option:platforms`, and still waited on the same
  `zstd//:zstd (//bazel/platforms:linux-musl#f9d25665faba5414)` analysis
  frontier.
- Post anonymous-default pruning smoke `anon-default-prune-sdk-195658` still
  timed out at 45s, but the audit dropped from 271 configs to 41 (16 host /
  25 musl) with only 4 non-empty `platforms` settings and no retained
  sanitizer/source/linkmode defaults. Post fastbuild-default pruning smoke
  `fastbuild-default-prune-sdk-200115` still timed out at 45s, but the audit
  dropped again to 39 configs (15 host / 24 musl), zero explicit
  `compilation_mode=fastbuild`, and the frontier stayed
  `zstd//:zstd (//bazel/platforms:linux-musl#145d4b2ad2508b2a) -- running
  analysis [evaluate_rule]`.
- Fresh memory-checkpoint run after anonymous-default pruning
  `anon-default-prune-mem-195830` showed the remaining timeout is not the fixed
  config-default explosion: about 18,080 analyses completed, 352 analyses were
  still active, and the oldest root remained `reactor//sdk:sdk_contents` waiting
  through SDK/Rust dependency chains. Hot samples are rules_rust dependency and
  action setup (`rust/private/rust.bzl`, `rustc.bzl`, `utils.bzl`) plus rules_cc
  compilation-context merging (`cc/private/cc_info.bzl` lines around
  `_merge_compilation_contexts` / `merge_cc_infos`). Depset counters remain
  high at about 360k creations and 131k `to_list()` calls, but the active graph
  shape is broad Rust/C++ analysis throughput under the reduced real config set,
  not a single zstd deadlock.
- Decision: Plan 61 true DICE-owned bzlmod is still required for graph
  correctness, workspace isolation, replay invalidation, and materialization
  semantics, but it will not by itself resolve the current SDK performance
  discrepancy. Keep the performance lane active before broader 61.2/61.3
  implementation. The fixed default-retention bugs explain the 271-config
  explosion, but not the remaining 45s SDK timeout; the next concrete
  investigation must target Slug's rules_rust/rules_cc Starlark analysis
  throughput and repeated depset/provider operations under the remaining
  39 real configurations.

Pause handoff 2026-05-19, SDK performance lane:

- Direct Bazel baseline was measured in `/var/mnt/dev/zeromatter-kuro` with
  the local Bazel 9 binary at `/usr/local/bin/bazel`. The working tree's
  visible `MODULE.bazel.lock` is currently modified and invalid for Bazel 9
  (`Illegal base64 character 2d`); it was temporarily moved or replaced and
  restored for these measurements. With the committed `HEAD:MODULE.bazel.lock`,
  `bazel shutdown && bazel build --nobuild //sdk:sdk_contents --profile=...`
  completed successfully in `ELAPSED_MS=10814` / Bazel elapsed `10.535s`,
  configuring `102,262` targets and `118` aspect applications. Profile:
  `/tmp/bazel-sdk-nobuild-headlock-200812.json.gz`; log:
  `/tmp/bazel-sdk-nobuild-headlock-200812.log`. A warm repeat with the visible
  lock moved aside and `--lockfile_mode=off` completed in `0.719s` elapsed with
  zero packages loaded and zero targets configured; profile:
  `/tmp/bazel-sdk-nobuild-nolock-warm-200752.json.gz`. A no-lock cold-ish run
  completed in `23.881s`, dominated by module-extension fetching, so the
  committed-lockfile run is the better analysis baseline.
- Parsed Bazel profile hotspots for the committed-lockfile baseline show the
  critical path is analysis, not execution: `skyframeExecutor.configureTargets`
  about `7.84s`, `runAnalysisPhase` about `8.29s`, and
  `ParallelEvaluator.eval` about `7.79s`. Top Starlark user-function samples
  include `_cc_library_impl` about `5.16s`, `_compile` / `compile` about
  `2.33s`, `_create_cc_compile_actions` about `1.96s`,
  `_rust_library_common` about `1.56s`, and `_rust_library_impl` about
  `1.35s`. This grounds the performance target: Slug should not spend 45s to
  finish only about 18k analyses when Bazel configures the whole 102k-target
  graph in about 10.5s.
- Bazel source inspection at `/var/mnt/dev/bazel` identified the high-payoff
  data-structure differences to keep targeting:
  `collect/nestedset/Depset.java`, `NestedSet.java`, and
  `NestedSetBuilder.java` use order-owned canonical empty depsets, compact
  single-object-or-array child storage, empty-child pruning, singleton child
  reuse, transitive child dedupe, weak cached flattening for transitive DAG
  nodes, and identity-array visited sets during flattening. `packages/
  StarlarkProvider.java`, `StarlarkInfoWithSchema.java`, and
  `StarlarkInfoNoSchema.java` store provider fields in compact sorted arrays;
  schemaful providers also unwrap matching depset fields to raw `NestedSet`
  values and reconstruct `Depset` wrappers on read.
- A follow-up depset patch is currently in the dirty worktree, validated by
  `cargo fmt --check`, `cargo check -p slug_build_api`, and
  `cargo test -p slug_build_api_tests interpreter::rule_defs::depset --
  --nocapture` (6 passed). It adds Bazel-shaped canonical empty depsets per
  order, construction-time empty-child pruning, and identity dedupe of
  non-topological transitive depset children, while preserving the earlier
  direct-only and frozen-cache fast paths. No post-patch SDK smoke was started
  because the user requested a pause at the next reasonable opportunity.
- Immediate next implementation steps for the next agent:
  1. Review the current dirty depset patch in
     `app/slug_build_api/src/interpreter/rule_defs/depset.rs`, especially
     `normalize_transitive_values`. Decide whether topological transitive child
     identity dedupe can also be enabled by first matching Bazel's LINK_ORDER /
     topological physical-order model; do not enable it without a focused
     Bazel-vs-Slug order probe.
  2. Run the remaining non-SDK validation for the dirty patch:
     `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
     tests/core/analysis/test_depset_order.py`, `cargo build -p slug`, and
     `git diff --check`.
  3. Clean `slugd` before and after a bounded SDK smoke, then run a 45s smoke
     with memory checkpoints using the same shape as
     `anon-default-prune-mem-195830`. Treat another 45s timeout as a
     performance failure to compare, not as a wait. Record completed/active
     analysis counts, depset create/to_list counters, and `audit
     configurations` counts. Expected question: did canonical empty/deduped
     children reduce the 360k depset creations or advance beyond the zstd
     frontier?
  4. If the smoke still times out with similar depset counters, implement the
     next Bazel-shaped data-structure fix rather than broad Plan 61 work:
     schemaful `UserProvider` depset-field unwrapping or another compact
     provider-field optimization modeled on Bazel's
     `StarlarkProvider.optimizeField` / `retrieveOptimizedField`. Start with a
     focused unit test for a provider that stores a depset field and repeatedly
     reads/flattens it, then rerun the SDK memory smoke.
  5. Separately, keep the invalid visible SDK `MODULE.bazel.lock` finding as a
     Plan 61 lockfile-parity item: Bazel 9 hard-fails that lockfile parse, while
     Slug's current ordinary paths have historically been more permissive. Do
     not "fix" this by masking Bazel's failure.

Continuation 2026-05-18/19 on Windows checkout:

- Rediscovered that the local zeromatter checkout at
  `C:\dev\zeromatter-kuro` (`383ac963cb`) no longer contains
  `//sdk:sdk_contents`; `sdk/BUILD.bazel` now defines the SDK tarball target as
  `//sdk:sdk`. A direct Slug run of the stale target failed correctly with
  `Unknown target sdk_contents` and listed `reactor//sdk:sdk` as the close
  target. Log: `C:\Users\WALTER~1\AppData\Local\Temp\sdk-parity-long-20260518-202811.log`.
- Post-depset-patch non-SDK validation on Windows: `cargo fmt --check`
  passed; `cargo check -p slug_build_api` passed with existing warnings;
  `cargo build -p slug` passed with existing warnings; `git diff --check`
  passed. `python -m pytest -q tests/core/analysis/test_depset_order.py`
  could not run because this Python environment has no `pytest` module.
- Bounded SDK smoke against the renamed target
  `target\debug\slug.exe --isolation-dir sdk-target-sdk-20260518-202942 build
  //sdk:sdk` timed out at 180s. Log:
  `C:\Users\WALTER~1\AppData\Local\Temp\sdk-target-sdk-20260518-202942.log`.
  The smoke reached analysis rather than a semantic failure: about 11,040
  analysis keys had completed with 448 still active near the final checkpoint,
  peak RSS was about 758 MiB, depset creation reached about 70,974, and
  depset `to_list()` reached about 58,042. Oldest active roots stayed under
  `reactor//sdk:sdk` and SDK/Rust tool dependencies. Hot Starlark samples again
  pointed at rules_rust `rust/private/rustc.bzl` and `rust/private/utils.bzl`.
- Class boundary for the next local patch: missing Bazel-shaped canonical empty
  depset reuse at provider accessor boundaries. The owner is Slug's depset
  facade plus Bazel provider shims, not the SDK target. A symptom-only patch
  would special-case `//sdk:sdk` or a particular `rules_rust` label; that is
  rejected. The intended patch is a systemic performance/parity fix: provider
  accessors that synthesize absent depset fields should return the canonical
  default-order empty depset instead of allocating fresh empty wrappers.
- Provider-accessor empty-depset slice implemented on 2026-05-18/19:
  `depset::empty_depset()` exposes the canonical default-order empty depset,
  and CcInfo/linking/debug, ctx empty linking context, JavaInfo defaults, and
  DefaultInfo/runfiles accessors now use it where they synthesize absent depset
  fields. Focused regression:
  `cargo test -p slug_build_api_tests
  interpreter::rule_defs::cc_common::cc_common_empty_provider_fields_are_depsets
  -- --nocapture` passed. `cargo fmt --check`, `cargo check -p slug_build_api`,
  `cargo build -p slug`, and `git diff --check` passed. The broader
  `cargo test -p slug_build_api_tests interpreter::rule_defs::cc_common --
  --nocapture` currently has four pre-existing Windows toolchain expectation
  failures (`/nologo` and `/OUT:out` vs Unix-style expected flags), unrelated to
  depset accessor reuse.
- Post-slice bounded SDK smoke:
  `target\debug\slug.exe --isolation-dir
  sdk-target-sdk-empty-depset-20260519-002 build //sdk:sdk` timed out at the
  185s harness cap. Log:
  `C:\Users\WALTER~1\AppData\Local\Temp\sdk-target-sdk-empty-depset-20260519-002.log`.
  It still did not complete, but it advanced farther than the previous 180s
  run: final checkpoint showed completed analysis keys about 11,248 and active
  keys 240, versus about 11,040 completed and 448 active before this slice.
  Peak RSS dropped from about 758 MiB to about 708 MiB. Depset `create_count`
  reached about 79,403 and `to_list` reached about 74,456, so the next
  bottleneck is not merely fresh empty-wrapper allocation. Hot samples continue
  to concentrate in rules_rust `rust/private/rustc.bzl` around
  `linker_inputs.to_list()` and adjacent provider plumbing.
- Next class boundary: repeated flattening/boxing of provider-stored depsets in
  schemaful and native provider fields. Bazel optimizes provider fields by
  storing raw `NestedSet` values for depset-typed fields and reconstructing
  `Depset` wrappers on read (`StarlarkProvider.optimizeField` /
  `retrieveOptimizedField`). Slug should first add a focused unit test around a
  provider/native provider depset field that is read and flattened repeatedly,
  then implement the smallest Bazel-shaped provider-field optimization or
  cached flattening improvement. Do not special-case `rules_rust`.
- Frozen transitive depset cache slice implemented next: frozen depsets with
  transitive children now cache their flattened frozen result after the first
  `to_list()` when all elements are frozen, matching the existing live-depset
  cache without changing traversal order. Validation:
  `cargo fmt --check`, `cargo test -p slug_build_api_tests
  interpreter::rule_defs::depset -- --nocapture` (6 passed),
  `cargo check -p slug_build_api`, and `cargo build -p slug` passed with
  existing warnings. Bounded SDK smoke
  `target\debug\slug.exe --isolation-dir
  sdk-target-sdk-frozen-cache-20260519-002 build //sdk:sdk` timed out at the
  185s harness cap. Log:
  `C:\Users\WALTER~1\AppData\Local\Temp\sdk-target-sdk-frozen-cache-20260519-002.log`.
  It advanced from 240 active analysis keys after the previous slice to 224,
  while peak RSS dropped from about 708 MiB to about 704 MiB. `to_list` reached
  about 70,196 versus 74,456 in the previous run, at a farther analysis point;
  `create_count` reached about 81,184. Hot samples still point at
  rules_rust `rust/private/rustc.bzl` line 295 (`linker_inputs.to_list()`) and
  neighboring link-provider code.
- Next class boundary after frozen-cache: reduce live depset construction and
  repeated list materialization in rules_rust link provider plumbing. Candidate
  fixes must stay Bazel-shaped: compact/native provider-field storage for
  depset-typed fields, or a depset/list bridge that avoids reconstructing
  large direct-only depsets from already-flattened linker input lists. Avoid
  label-specific handling.
- Rejected experiment: enabling exact identity dedupe for topological
  transitive children passed depset tests but regressed the SDK smoke
  (`sdk-target-sdk-topo-dedupe-20260519-001`) to about 751 MiB peak RSS and
  about 76,100 `to_list` calls while still stopping at 224 active analysis
  keys. The experiment was reverted before commit. Keep the earlier caution:
  topological/LINK_ORDER child normalization needs a deliberate physical-order
  parity change, not a quick identity-dedupe toggle.
- Checkpoint overhead cleanup: `depset.to_list()` memory checkpoints now read
  direct/transitive lengths from `DepsetSummary` instead of cloning direct and
  transitive vectors through `depset_direct_and_transitive()` only to compute
  lengths. Validation: `cargo fmt --check`, `cargo test -p
  slug_build_api_tests interpreter::rule_defs::depset -- --nocapture`,
  `cargo check -p slug_build_api`, and `cargo build -p slug` passed with
  existing warnings. Bounded SDK smoke
  `sdk-target-sdk-checkpoint-summary-20260519-001` timed out at 185s, again at
  224 active analysis keys, but peak RSS dropped to about 696 MiB. This is a
  checkpoint fidelity/perf fix rather than a semantic bzlmod fix; keep it
  because Plan 61 relies on memory-checkpoint smokes for performance decisions.
- No-checkpoint SDK smokes after the depset slices advance materially farther
  than the checkpointed runs but still do not finish: `sdk-target-sdk-no-
  checkpoints-20260519-001` timed out at 300s near the generated build-script
  tail, and `sdk-target-sdk-no-checkpoints-20260519-002` timed out at 600s with
  about 31 analysis actions still active, centered on generated build scripts
  such as `reactor//zerobuf_generated/playback:build_script`.
- Focused execution frontier: `target\debug\slug.exe --isolation-dir playback-
  build-script-checkpoints-20260519-001 build //zerobuf_generated/playback:
  build_script` reached execution and failed compiling
  `rules_rust++i+rules_rust_tinyjson//:tinyjson` with
  `bootstrap_process_wrapper: _spawnvp: No such file or directory`. The printed
  command runs the generated `bootstrap_process_wrapper.exe` successfully, then
  passes a relative `buck-out\...\rustc.exe` child path to the wrapper. Next
  class boundary: Windows action command-line path semantics for executable
  artifacts consumed by process wrappers. Own this in Slug's action command
  expansion / local execution path handling, not as a rules_rust or target-
  specific workaround.
- Process-wrapper path slice completed: local execution now rewrites the child
  `rustc.exe` path in Windows process-wrapper invocations to the shortest
  existing absolute path, including `GetShortPathNameW` retry through the
  long-path prefix. Validation: `cargo fmt --check`, `cargo test -p
  slug_execute_impl process_wrapper -- --nocapture`, `cargo check -p
  slug_execute_impl`, `cargo build -p slug`, and focused smoke
  `playback-build-script-process-wrapper-final-20260519-001`. The focused smoke
  advanced past the prior `_spawnvp` failure, ran 296 local commands, and failed
  later in `crates__windows_x86_64_msvc-0.42.2//:_bs_x86_64-pc-windows-msvc`
  when `cargo_build_script_runner` tried to link a generated script executable
  from an absolute short execroot source to a relative runfiles destination and
  Windows returned `Os { code: 87, kind: InvalidInput }`. Next class boundary:
  Windows runfiles link path semantics for cargo build-script runner actions.
  Own this in local action/runfiles materialization or argument path rewriting,
  not as a rules_rust crate-specific workaround.
- Cargo build-script runner Windows path slice completed: local execution now
  rewrites generated build-script runfile sources in the runner's
  `--cargo_manifest_args=@...` file to the short script alias, and rewrites
  existing `CARGO`, `RUSTC`, and `RUSTDOC` env tool paths to absolute short
  paths before the runner re-joins them against the action execroot. The
  intermediate `playback-build-script-runfiles-param-20260519-001` smoke proved
  that rewriting the runfiles output directory is wrong because the declared
  `_bs.cargo_runfiles` output must remain at its original path; that experiment
  was reverted before validation. Final validation: `cargo fmt --check`,
  focused `cargo_tool` and `cargo_runner` tests in `slug_execute_impl`,
  `cargo check -p slug_execute_impl`, `cargo build -p slug`, and focused smoke
  `playback-build-script-cargo-tool-env-20260519-002` all passed. The focused
  smoke built `//zerobuf_generated/playback:build_script` successfully in
  18m41s with 699 local commands and no daemon left running afterward. Next
  class boundary: return from focused build-script validation to the full
  `//sdk:sdk` SDK smoke and classify the next real-world frontier.
- Workspace `_main` cargo-manifest alias slice completed after the full SDK
  smoke reached a generated workspace build script:
  `sdk-target-sdk-after-cargo-paths-20260519-001` advanced for 68m12s and failed
  in `//zerobuf_generated/component_animation_types:build_script` because the
  short `CARGO_MANIFEST_DIR` alias had no source for `_main/...`, leaving the
  runner's working directory invalid. The fix maps `_main` runfiles manifests to
  the workspace package path, creates an empty alias directory if all source
  candidates are absent, and aliases any long generated executable runfile
  source in `--cargo_manifest_args` rather than only the exact `--script`
  executable. Validation: `cargo fmt --check`, focused
  `cargo_manifest_alias_sources` and `cargo_runner` tests, `cargo check -p
  slug_execute_impl`, `cargo build -p slug`, and focused smoke
  `component-animation-build-script-runfile-src-20260519-001` all passed. The
  focused smoke built `//zerobuf_generated/component_animation_types:
  build_script` successfully in 18m48s with 699 local commands and no daemon
  left running afterward. Next class boundary: rerun full `//sdk:sdk` and
  classify the next frontier beyond generated workspace build scripts.
- Full SDK rerun `sdk-target-sdk-after-cargo-paths-20260519-001-rerun2`
  advanced beyond generated build scripts and failed in
  `//sdk/zeromatter_ffi:zeromatter_ffi` during the Windows MSVC Rust link. The
  `rust-lld` invocation reports undefined imported UCRT symbols from
  `libaws_lc_sys-3256947194.rlib` (`strtol`, `getenv`, stdio/file APIs,
  `_setmode`) and notes that the found `libucrt.lib` cannot be used because it
  is not an import library. The printed link command includes
  `/defaultlib:msvcrt` plus MSVC UCRT/UM `-LIBPATH` entries. Next class
  boundary: Bazel 9/rules_rust parity for Windows MSVC CRT default libraries
  and link action construction for Rust targets that consume C/C++ static
  archives. Own this in Slug's Rust/C++ toolchain link-flag and environment
  modeling, not as a `zeromatter_ffi` or `aws-lc-sys` workaround.
- Focused rerun `zeromatter-ffi-link-crt-20260519-002` confirmed that adding
  CRT import libraries inside Slug's generic `cc_common` command-line expansion
  does not reach this direct `rules_rust`/`rust-lld` path: the failing command
  still contains only `/defaultlib:msvcrt` plus `LIBPATH` entries. Implementation
  boundary for the next slice: patch Slug's Windows Rust action argv preparation
  for process-wrapper `rustc` invocations that use `rust-lld` for MSVC targets,
  adding the missing dynamic CRT import libraries before the long Rust tail is
  parameterized. Do not keep a broad `cc_common` behavior change unless later
  evidence shows Bazel/rules_cc emits those libraries there too.
- Focused rerun `zeromatter-ffi-rust-lld-crt-20260519-003` showed the first
  Rust argv rewrite did not reach `zeromatter_ffi`: Slug had already moved the
  target's Rust arguments into the action-local generic `slug-params-0` file.
  The file contains `--target=x86_64-pc-windows-msvc`, `-Clinker=rust-lld.exe`,
  and MSVC `LIBPATH` link args, but no explicit CRT import-library link args.
  Extend the same Windows MSVC/rust-lld CRT insertion to generic Rust paramfile
  materialization, beside the existing Rust execroot-remap rewrite.
- Focused rerun `zeromatter-ffi-rust-param-crt-20260519-004` verified the
  paramfile insertion reached `rust-lld` and moved past the previous
  `zeromatter_ffi` UCRT unresolved symbols. The next failure is later, in
  `//sdk/sdk_builder:sdk_builder_bin`, with duplicate `__report_gsfailure`
  between explicit `vcruntime.lib` and the already-present `/defaultlib:msvcrt`.
  Narrow the insertion to the missing UCRT import library plus `oldnames.lib`;
  do not override the runtime flavor selected by the toolchain defaults.
- Focused rerun `zeromatter-ffi-rust-ucrt-crt-20260519-005` shows explicit
  `ucrt.lib` and `oldnames.lib` reach `rust-lld`, but Rust's default MSVC link
  still brings in static `libucrt.lib`, producing duplicate UCRT symbols
  (`_invalid_parameter_noinfo`, `_wctype`, `__pctype_func`). Pair the explicit
  UCRT import library with `/nodefaultlib:libucrt` so the dynamic import library
  replaces the static default rather than adding a second UCRT.
- Verified rerun `zeromatter-ffi-rust-ucrt-nodefault-20260519-006` succeeded
  for `//sdk/zeromatter_ffi:zeromatter_ffi` in 71m48s with 2247 local commands.
  The successful slice adds Windows Rust paramfile/argv rewriting for MSVC
  `rust-lld` links: if a Rust action targets `windows-msvc`, uses `rust-lld`,
  and has not already specified the dynamic UCRT fixup, Slug appends
  `/nodefaultlib:libucrt`, `ucrt.lib`, and `oldnames.lib` as rustc link args.
  This preserves the toolchain's existing `/defaultlib:msvcrt` runtime choice
  while making C archives that import UCRT symbols link under `rust-lld`. The
  smoke left one `slug.exe` daemon for the isolation dir; clean it before the
  next run. Next class boundary: commit this verified CRT slice, then run full
  `//sdk:sdk` again.
- Full SDK rerun `sdk-target-sdk-after-ucrt-nodefault-20260519-001` succeeded
  for `//sdk:sdk` in 80m32s with 2281 local commands. This completes the current
  SDK parity batch: after Windows process-wrapper path shortening, cargo build
  script path shortening, workspace cargo manifest alias handling, and the
  MSVC/rust-lld dynamic UCRT fix, the full SDK archive target builds under Slug
  on the Windows checkout. The run left one `slug.exe` daemon for the isolation
  dir; clean it before pausing or starting the next batch.

Implementation slice 2026-05-19, Bazel-shaped extension lockfile digests:

- Fresh Linux parity probing found the current zeromatter visible
  `MODULE.bazel.lock` malformed for Bazel 9.0.1: Bazel stops while computing
  the main repository mapping with `Illegal base64 character 2d`, because some
  extension `bzlTransitiveDigest` / `usagesDigest` entries use SRI-style
  `sha256-...` strings. Slug previously accepted the same file and continued
  into analysis, which would mask a Bazel 9 failure.
- Tightened `Lockfile::read` so every mode that reads lockfiles (`update`,
  `refresh`, and `error`) rejects unreadable, unparsable, or semantically
  invalid visible/hidden lockfiles. `off` remains the only mode that skips
  lockfile reads. The process cache now rechecks a prior negative miss if the
  file exists later.
- Changed Slug's extension replay digest writers to Bazel-shaped raw Base64
  SHA-256 bytes for `bzlTransitiveDigest` and `usagesDigest`, instead of
  SRI-style `sha256-...`. Updated the Python Plan 61 guardrail lockfile writer
  to mirror that shape so valid replay still materializes generated repos
  without extension evaluation.
- Added Rust coverage for malformed JSON, invalid SRI-prefixed extension
  digests, wrong decoded digest length, refresh-mode errors, and off-mode
  suppression. Added a Python guardrail proving default lockfile mode rejects a
  visible extension digest that Bazel rejects.
- Validation: `cargo fmt --check`, `cargo test -p slug_bzlmod lockfile --
  --nocapture` (32 passed), `cargo test -p slug_bzlmod
  test_compute_extension_input_hash -- --nocapture` (3 passed), `cargo test -p
  slug_bzlmod test_compute_bzl_transitive_digest -- --nocapture` (1 passed),
  `cargo build -p slug`, focused direct pytest for the new invalid-digest and
  replay-materialization guardrails (2 passed), full direct pytest
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py -rx` (18 passed / 1 xfailed), and
  `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails` (18 passed
  / 1 xfailed inside pytest, Slug test pass). Daemons were cleaned afterward.
- Ground-truth correction 2026-05-20, cross-workspace bzlmod isolation
  guardrail: Bazel source documents and implements one server per output base,
  and the default output base is derived from the workspace root
  (`BlazeServerStartupOptions.java` option help, `startup_options.h`, and
  `startup_options.cc::UpdateConfiguration` calling `GetHashedBaseDir` on the
  workspace; `blaze_util_posix.cc::GetHashedBaseDir` hashes the workspace
  string). Slug's daemon directory likewise includes the project root in
  `InvocationPaths::daemon_dir`. Therefore Plan 61 should not require a
  same-daemon two-workspace precondition as a Bazel parity claim. Updated
  `test_two_workspaces_do_not_share_bzlmod_state` to assert the observable
  requirement: workspaces with colliding module/repo names do not see each
  other's bzlmod state, and counters advance relative to the appropriate daemon
  baseline. Validation passed: focused direct pytest
  `python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py::test_two_workspaces_do_not_share_bzlmod_state -rx`,
  full direct pytest
  `python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx` (19
  passed), and
  `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails` (19 passed
  inside pytest, Slug test pass). Daemons were cleaned afterward.
- Manager process note 2026-05-20: subagent dispatch was attempted for the next
  Phase 61.8 lockfile-cache slice, but the runtime reported the agent thread
  limit was reached. Per the loop-manager prompt, continued locally and did not
  treat delegation unavailability as a blocker.
- Manager process note 2026-05-20: after the DICE session-data, extension
  aggregation, lockfile-cache, dynamic-root, stub-counter, and stale-path-repair
  slices, subagent dispatch was retried for remaining-work classification. The
  runtime again reported the agent thread limit was reached, so remaining
  classification and implementation continued locally.
- Implementation slice 2026-05-20, lockfile cache as content-fingerprint
  adapter: Bazel ground truth keeps workspace and hidden lockfiles as
  `BazelLockFileValue.KEY` and `BazelLockFileValue.HIDDEN_KEY` SkyValues
  (`BazelLockFileValue.java` lines 44-105), and
  `BazelLockFileFunction` reads lockfile contents into those values. Slug's
  temporary process-wide cache keyed existing parsed lockfiles by path alone, so
  an edited existing `MODULE.bazel.lock` could be hidden by the cached
  `Arc<Lockfile>` until explicit invalidation. Updated the cache to treat the
  file content digest as the reuse key: it still avoids reparsing unchanged
  JSON, but it re-reads and reparses when the file bytes change and re-reads a
  file created after a prior missing lookup. This does not finish
  `LockfileContentKey`, but it removes path-only cache authority and matches the
  Plan 61 direction. Validation passed: `cargo fmt --check`, focused
  `cargo test -p slug_bzlmod cached_lockfile -- --nocapture` (2 passed), and
  `cargo test -p slug_bzlmod lockfile -- --nocapture` (34 passed).
- Implementation slice 2026-05-20, extension replay lockfile mode propagation:
  after the content-fingerprint cache fix, extension execution still read
  `cached_lockfile(project_root)` with implicit default/update behavior. That
  meant a later lazy extension replay could bypass `--lockfile_mode=off` even
  though startup spoke preseed honored the explicit mode. Threaded
  `LockfileMode` through the temporary `EXTENSION_AGGREGATIONS` adapter into
  `ModuleExtensionExecutionKey`, included it in key hash/equality, and changed
  extension replay to call `cached_lockfile_with_mode`. This is still a
  transitional global-backed path, but the command policy now participates in
  the DICE key identity and lockfile reads are suppressed in `off` mode.
  Validation passed: `cargo fmt --check`,
  `cargo test -p slug_bzlmod lockfile -- --nocapture` (35 passed), and
  `python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx` (19
  passed).
- Implementation slice 2026-05-20, explicit lockfile write capability:
  `Lockfile::write` was still a public normal-build API even though current
  non-test paths do not call it. Converted the normal API to
  `write_for_purpose(path, LockfileWritePurpose::ExplicitModUpdate)` and left
  the old `write(path)` helper available only under `cfg(test)`. This does not
  implement future `slug mod update`, but it narrows the ordinary build/query
  surface so visible Bazel lockfile writes require an explicit capability
  instead of an incidental method call. Validation passed: `cargo fmt --check`,
  `cargo check -p slug_bzlmod`, and
  `cargo test -p slug_bzlmod lockfile -- --nocapture` (35 passed).
- Follow-up cleanup 2026-05-20: removed the legacy `cached_lockfile()` wrapper
  and export after extension execution switched to explicit
  `cached_lockfile_with_mode`. `rg "cached_lockfile\\(" app tests -g '*.rs'`
  now finds no mode-less lockfile readers. Validation passed:
  `cargo fmt --check` and
  `cargo test -p slug_bzlmod lockfile -- --nocapture` (35 passed).
- Implementation slice 2026-05-20, module extension failure is direct: removed
  the normal-path fallback that converted a failed Starlark module extension
  execution into empty generated specs, and removed the DICE late-binding
  fallback that returned an empty result when the concrete executor was not
  initialized. This matches the Plan 61/Bazel direction that extension failures
  fail at the owning extension boundary instead of producing downstream unknown
  repo/target failures. Validation passed:
  `cargo test -p slug_bzlmod extension_execution_dice -- --nocapture` (20
  passed) and focused no-stub guardrails
  `python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py::test_bad_extension_fails_without_stub_repo tests/core/bzlmod/test_plan61_guardrails.py::test_no_stub_failures_cover_missing_generated_repo_and_repo_rule_failure -rx`
  (2 passed).
- Implementation slice 2026-05-20, generic invalid marker handling: removed
  the remaining normal-path stub-marker interpretation in
  `app/slug_external_cells/src/extension_repo.rs`. Non-complete marker contents
  now mean only "this generated repository is not currently trusted"; they are
  not semantic authority for a stub repository shape. This keeps the
  materializer on the Plan 61 path: stale/invalid materialization state must
  rematerialize from the owning repo spec or fail directly, not create or bless
  a synthetic repo. Validation passed: `cargo fmt --check`,
  `cargo test -p slug_external_cells complete_marker -- --nocapture` (1
  passed), and
  `python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx` (19
  passed). A follow-up search for
  `Creating stub|materialize_stub_repo|stub_marker|stubbed out|falling back to empty specs|is_stub_marker|stale stub`
  finds no normal bzlmod path remaining; the only code-side stub fallback name
  left is the intentionally idle `stub_fallback_attempt` event counter used by
  guardrails to assert that removed paths stay removed.
- Implementation slice 2026-05-20, module/register globals moved to DICE
  session data: removed the process-global `MODULE_VERSIONS`,
  `REGISTERED_TOOLCHAINS`, and `REGISTERED_EXECUTION_PLATFORMS` registries from
  `app/slug_bzlmod/src/lib.rs`. Startup bzlmod resolution now returns
  `BzlmodSessionData`, the server injects it into DICE for the command, Starlark
  interpreter cell info receives the module-version map from that injected
  value, and toolchain/execution-platform resolution reads registrations from
  DICE rather than global state. This is still a transitional adapter around
  legacy cell parsing, but the facts now invalidate with the command graph
  instead of process lifetime. Validation passed: `cargo fmt --check`,
  `cargo check -p slug_bzlmod -p slug_common -p slug_interpreter_for_build -p slug_analysis -p slug_configured`,
  `cargo test -p slug_bzlmod lockfile -- --nocapture` (35 passed),
  `cargo test -p slug_bzlmod dice_graph -- --nocapture` (3 passed),
  `python -m pytest -q tests/core/analysis/test_build_globals.py -rx` (15
  passed), and
  `python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx` (19
  passed). A follow-up search for the removed global names and getter/setter
  APIs returns no matches.
- Implementation slice 2026-05-20, extension aggregation global moved to DICE
  session data: removed the process-wide `EXTENSION_AGGREGATIONS` mutex and
  `set_extension_aggregations` API. The legacy startup resolver now stores
  extension aggregations, root module name, project root, and lockfile mode in
  `BzlmodSessionData`; lazy extension repo paths compute the injected
  `BzlmodSessionDataKey` before constructing `ModuleExtensionExecutionKey`.
  This preserves the current lazy execution behavior while making extension
  replay input selection command-scoped rather than process-scoped. Validation
  passed: `cargo fmt --check`,
  `cargo check -p slug_bzlmod -p slug_common -p slug_external_cells`,
  `cargo test -p slug_bzlmod extension_execution_dice -- --nocapture` (20
  passed), `cargo test -p slug_external_cells complete_marker -- --nocapture`
  (1 passed), and
  `python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx` (19
  passed). A follow-up search for `EXTENSION_AGGREGATIONS` and
  `set_extension_aggregations` returns no matches.
- Implementation slice 2026-05-20, lockfile process cache removed: after the
  earlier content-fingerprint adapter proved the stale-read failure mode, the
  remaining process-wide `LOCKFILE_CACHE` was deleted rather than further
  elaborated. The public lockfile readers are now explicitly named
  `read_lockfile_with_mode` / `read_lockfile_path_with_mode`; they honor
  `LockfileMode::Off`, return `None` for absent files, and otherwise read the
  file bytes from disk as the current authority. This is a conservative
  transition toward `LockfileContentKey`: it trades a small parse-cache
  optimization for eliminating another daemon-lifetime semantic fact.
  Validation passed: `cargo fmt --check`,
  `cargo check -p slug_bzlmod -p slug_common`,
  `cargo test -p slug_bzlmod lockfile -- --nocapture` (35 passed), and
  `python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx` (19
  passed). A follow-up search for `cached_lockfile`,
  `invalidate_cached_lockfile`, `LOCKFILE_CACHE`, `lockfile_cache`, and
  `LockfileCacheEntry` returns no matches.
- Implementation slice 2026-05-20, dynamic bzlmod adapter root reset: the
  temporary dynamic extension-cell globals in `app/slug_core/src/cells.rs` are
  still not the final `BzlmodCellGraphKey`, but they no longer use a
  write-once project root. `set_dynamic_project_root` now updates the current
  root through a lock and clears dynamic extension cells, setups, aliases,
  scoped repo aliases, and apparent-alias cache when the root changes. The
  bzlmod resolver now sets this root before any lockfile or precomputed
  extension cells are registered, so the current workspace's registrations are
  not wiped after seeding. This prevents stale daemon-lifetime dynamic state
  from surviving a root transition while keeping the adapter behavior intact
  for the current workspace. Validation passed: `cargo fmt --check`,
  `cargo check -p slug_core -p slug_common`,
  `cargo test -p slug_core bzlmod -- --nocapture` (6 passed), and
  `python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx` (19
  passed).
- Implementation slice 2026-05-20, stub fallback counter deleted: production
  code no longer records stub fallback attempts, and the normal stub fallback
  paths have been removed, so the idle `StubFallbackAttempt` event kind and
  `stub_fallback_attempt` JSON counter were deleted from `dice_graph.rs`.
  Guardrails now assert the behavior directly: failed extension/repo-rule paths
  fail with the owning error and leave no repo dir, marker, BUILD file, or
  defs.bzl behind. Validation passed: `cargo fmt --check`,
  `cargo test -p slug_bzlmod dice_graph -- --nocapture` (3 passed), and
  `python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx` (19
  passed). A follow-up search for `StubFallbackAttempt`, `stub_fallback`, and
  `stub_fallback_attempt` returns no matches.
- Implementation slice 2026-05-20, stale absolute repo-path repair removed:
  deleted `repository_execution.rs::repair_stale_local_path_attrs` and its
  suffix-search helper. Extension repo execution now uses the `RepoSpec`
  attributes as recorded; if a repository rule contains a stale absolute
  `path`, the owning repository rule execution fails instead of Slug rewriting
  it to a guessed path under the current project. This matches the Plan 61
  policy that recorded inputs must be validated or re-executed, not silently
  repaired. Validation passed: `cargo fmt --check`,
  `cargo test -p slug_bzlmod repository_execution -- --nocapture` (16 passed),
  and `python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx` (19
  passed). A follow-up search for `repair_stale_local_path_attrs`,
  `existing_suffix_under_project_root`, and `Rewriting stale absolute` returns
  no matches.
- Validation/fix slice 2026-05-20, DICE session injection and root-cell
  adapter state: after moving bzlmod session facts into DICE, the Slug-level
  guardrail target exposed a real integration bug:
  `BzlmodSessionDataKey` was marked changed twice in the same transaction.
  The systemic fix was to keep default session data injection only in
  `setup_interpreter_basic` and let server command setup inject the real
  startup-produced session data once. Full `cargo build -p slug` then passed,
  and `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails`
  passed with 19 pytest cases. A follow-up slice removed write-once
  `ROOT_CELL_NAME` behavior in `app/slug_core/src/cells.rs`; the temporary
  root-cell adapter now tracks the current resolver through a mutable
  process-level value instead of preserving the first root cell built in the
  process. This is still not final `BzlmodCellGraphKey` ownership, but it
  removes another daemon-lifetime stale fact from the normal path. Validation
  passed: `cargo fmt --check`, `cargo check -p slug_core -p slug_common -p
  slug_interpreter_for_build -p slug_execute -p slug_build_api -p
  slug_analysis`, `cargo test -p slug_core cells -- --nocapture` (31 passed),
  `cargo build -p slug`, and
  `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails` (19
  passed inside pytest). Daemons were cleaned afterward; `/var/mnt/dev/slug/buck-out`
  was about 3.5M and the retained ZeroMatter evidence tree remained about 21G.
- Manager process note 2026-05-20: subagent dispatch was retried for the next
  remaining-work classification after the guardrail validation, but the runtime
  again reported the agent thread limit was reached. Per the loop-manager
  prompt, this was recorded and local implementation continued.
- Implementation slice 2026-05-20, spoke registry root scoping: the remaining
  `SPOKE_REGISTRY` / `SEEDED_EXTENSIONS` adapter in
  `app/slug_bzlmod/src/spoke_materialization.rs` is still temporary process
  state, but it now tracks the current project root and clears spoke
  registrations plus seeded-extension markers when bzlmod setup moves to a
  different workspace. `app/slug_common/src/legacy_configs/cells.rs` calls
  `set_spoke_materialization_project_root` immediately after
  `set_dynamic_project_root`, before lockfile/precomputed spokes are seeded for
  the current workspace. This keeps lazy spoke materialization behavior while
  preventing previous-workspace spoke specs from surviving as daemon-lifetime
  facts. Validation passed: `cargo fmt --check`,
  `cargo test -p slug_bzlmod spoke_materialization -- --nocapture` (3 passed),
  `cargo check -p slug_bzlmod -p slug_common -p slug_external_cells`,
  `cargo build -p slug`, and
  `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails` (19
  passed inside pytest).
- Implementation slice 2026-05-20, lockfile recorded file input replay
  validation: Bazel ground truth is `RepoRecordedInput.WithValue` parsing and
  `RepoRecordedInput.File.fileValueToMarkerValue` (`DIR`, `ENOENT`, or
  lowercase SHA-256 hex) plus `SingleExtensionEvalFunction` replay checking of
  recorded inputs before accepting cached extension results. Slug now validates
  absolute `FILE:<path> <value>` and main-workspace `FILE:@@//path <value>`
  recorded inputs before accepting `generatedRepoSpecs`; malformed entries,
  unsupported recorded input kinds, unsupported external-repo path spellings,
  stat failures, and changed file markers are conservative replay misses rather
  than cache hits. The
  guardrail exposed that same-root warm commands could also retain stale
  lockfile-seeded dynamic repo setups in the temporary adapter, so bzlmod
  startup now resets dynamic extension cells and spoke registrations for every
  fresh bzlmod resolution, not only when the project root changes. This remains
  a transition toward `LockfileContentKey` / `RepoRecordedInput` DICE values,
  but it removes another stale replay path without masking Bazel failures.
  Validation passed: `cargo fmt --check`, `cargo test -p slug_bzlmod recorded
  -- --nocapture` (4 passed), `cargo test -p slug_bzlmod lockfile --
  --nocapture` (39 passed), `cargo check -p slug_bzlmod -p slug_common`,
  `cargo build -p slug`, focused direct pytest
  `test_lockfile_replay_recorded_file_input_edit_rejects_cache`, full direct
  `python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx` (20
  passed), and
  `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails` (20
  passed inside pytest). Daemons were cleaned afterward; `/var/mnt/dev/slug/buck-out`
  was about 3.5M and the retained ZeroMatter evidence tree remained about 21G.
- Implementation slice 2026-05-20, lockfile recorded directory-entry replay
  validation: Bazel ground truth is `RepoRecordedInput.Dirents`, which records
  the sorted directory entry names and fingerprints them with
  `Fingerprint.addStrings` / SHA-256 before `SingleExtensionEvalFunction`
  accepts replay. Slug now validates absolute and main-workspace
  `DIRENTS:<path> <value>` recorded inputs with the same count/string framing
  before accepting cached `generatedRepoSpecs`; malformed values, unsupported
  external-repo path spellings, stat failures, non-UTF-8 entry names, and
  changed directory entry sets are conservative replay misses. This is another
  partial `RepoRecordedInput` slice: `DIRTREE`, `ENV`, and `REPO_MAPPING`
  remain unsupported and must miss until their Bazel semantics are implemented.
  Validation passed: `cargo fmt --check`, `cargo test -p slug_bzlmod recorded
  -- --nocapture` (6 passed), `cargo test -p slug_bzlmod lockfile --
  --nocapture` (41 passed), `cargo build -p slug`, focused direct pytest
  `test_lockfile_replay_recorded_dirents_input_edit_rejects_cache`, full direct
  `python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx` (21
  passed), and
  `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails` (21
  passed inside pytest). Daemons were cleaned afterward. Stale generated
  ZeroMatter output `buck-out/plan61-optional-cc-fix-20260520` was removed
  after preserving logs/manifests under `/tmp/slug-plan61`; ZeroMatter
  `buck-out` dropped from about 21G to about 3.3M.
- Implementation slice 2026-05-20, lockfile recorded directory-tree replay
  validation: Bazel ground truth is `RepoRecordedInput.DirTree` plus
  `DirectoryTreeDigestFunction`, which recursively fingerprints sorted entry
  names, sorted subdirectory digests, `FileStateType` ordinals, and raw file
  content digests with `Fingerprint` / SHA-256. Slug now validates absolute and
  main-workspace `DIRTREE:<path> <value>` recorded inputs before accepting
  cached `generatedRepoSpecs`; malformed values, unsupported external-repo path
  spellings, stat/digest failures, and changed nested files or entries are
  conservative replay misses. The implementation matches Bazel's ordinary
  regular-file/directory behavior and follows symlink targets via filesystem
  metadata; special files are treated as the Bazel `SPECIAL_FILE` ordinal for
  the replay marker. `ENV` and `REPO_MAPPING` remain unsupported by design
  because Bazel validates them through repository environment and repository
  mapping Skyframe values, not raw process env or ad hoc label resolution.
  Validation passed: `cargo fmt --check`, `cargo test -p slug_bzlmod recorded
  -- --nocapture` (8 passed), `cargo test -p slug_bzlmod lockfile --
  --nocapture` (43 passed), `cargo build -p slug`, focused direct pytest
  `test_lockfile_replay_recorded_dirtree_input_edit_rejects_cache`, full direct
  `python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx` (22
  passed), and
  `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails` (22
  passed inside pytest). Daemons were cleaned afterward; `/var/mnt/dev/slug/buck-out`
  was about 3.5M and ZeroMatter `buck-out` remained about 3.3M.
- Implementation slice 2026-05-20, lockfile recorded environment replay
  validation: Bazel ground truth is `RepoRecordedInput.EnvVar` and
  `RepoEnvironmentFunction`: module extensions observe the command-scoped
  repository environment, with `--repo_env` overlaying the effective client
  environment before `SingleExtensionEvalFunction` accepts or rejects replay.
  Slug now validates `ENV:<name> <value>` recorded inputs against the effective
  repository environment before accepting cached `generatedRepoSpecs`, including
  unset values represented by the Bazel lockfile `\0` marker. The guardrail
  initially exposed a systemic command-boundary bug: using the daemon process
  environment made the first command's environment persist into later commands,
  and `audit cell`'s hyphen-accepting alias positional shape did not reliably
  carry Bazel-style `--repo_env` through Clap. The fix moved repo-env transport
  to the shared client context by scanning raw Bazel-shaped argv
  (`--repo_env`, `--repo-env`, and `--flag=value` forms), merging that with
  parsed config options, and storing the resulting command-scoped repo env in
  `BzlmodSessionData` / `ModuleExtensionExecutionKey` / lockfile preseed
  replay. `module_ctx.getenv`, `repository_ctx.getenv`, and
  `repository_ctx.os.environ` now read the same command-scoped repo env. This is
  still a transition toward `BzlmodCommandPolicyKey`; strict repo env,
  action-env interaction, nonstrict repo env digests, and complete policy
  ownership are not finished. `REPO_MAPPING` recorded inputs remain unsupported
  by design until repo mapping is owned by typed DICE values.
  Validation passed: `cargo fmt --check`, `cargo check -p slug_client_ctx -p
  slug_cmd_audit_client`, `cargo check -p slug_cli_proto -p slug_client_ctx -p
  slug_server -p slug_interpreter_for_build -p slug_bzlmod`, `cargo test -p
  slug_bzlmod recorded -- --nocapture` (11 passed), `cargo test -p slug_bzlmod
  lockfile -- --nocapture` (46 passed), `cargo build -p slug`, focused direct
  pytest `test_lockfile_replay_recorded_env_input_change_rejects_cache`, full
  direct `pytest -q tests/core/bzlmod/test_plan61_guardrails.py --tb=short`
  (23 passed), and
  `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails` (23
  passed inside pytest).
- Implementation slice 2026-05-20, lockfile recorded repository-mapping replay
  validation: Bazel ground truth is `RepoRecordedInput.RecordedRepoMapping` in
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/rules/repository/RepoRecordedInput.java`,
  `StarlarkBaseExternalContext` recording of canonical repository visibility,
  `RepositoryMapping.get()`'s non-visible apparent-name fallback, and
  `BazelDepGraphValue.getFullRepoMapping` for module repository mappings.
  Bazel records lockfile entries as `REPO_MAPPING:<source_repo>,<apparent>
  <canonical_or_\0>` with repository names written without `@` prefixes; the
  main/root source repository name is the empty string. Slug now carries a
  command/session `RepoMappingSnapshot` into lockfile replay, validates
  module-scope recorded repo mappings before accepting cached
  `generatedRepoSpecs`, treats missing source scopes as conservative replay
  misses, and compares missing apparent names against Bazel's non-visible
  apparent-name value rather than accepting a stale null marker. The guardrail
  mutates a root `bazel_dep(..., repo_name = ...)` mapping and proves stale
  lockfile replay is rejected. This is still a transition toward typed
  `RepoMappingKey` / `ModuleExtensionRepoMappingEntriesFunction` ownership:
  extension-generated repo source scopes are intentionally conservative misses
  until their host-module, generated-repo, `inject_repo`, and `override_repo`
  mappings are produced by DICE-owned values.
  Validation passed: `cargo fmt --check`, `cargo test -p slug_bzlmod recorded
  -- --nocapture` (15 passed), `cargo test -p slug_bzlmod lockfile --
  --nocapture` (50 passed), `cargo check -p slug_bzlmod -p slug_common -p
  slug_server -p slug_client_ctx -p slug_cmd_audit_client`, `cargo build -p
  slug`, focused direct pytest
  `test_lockfile_replay_recorded_repo_mapping_change_rejects_cache`, full
  direct `pytest -q tests/core/bzlmod/test_plan61_guardrails.py --tb=short`
  (24 passed), and
  `./target/debug/slug test tests/core/bzlmod:test_plan61_guardrails` (24
  passed inside pytest). Daemons were cleaned afterward; `/var/mnt/dev/slug/buck-out`
  was about 3.5M and ZeroMatter `buck-out` remained about 3.3M.
- Implementation slice 2026-05-20, extension-generated repository mapping for
  recorded-input replay: Bazel ground truth is
  `ModuleExtensionRepoMappingEntriesFunction`, which computes the mapping
  visible from every repo generated by one extension as the host module's full
  mapping plus all repos generated by the same extension plus root
  `override_repo()` rows, with later entries winning. Slug now derives the
  same shape for the transitional `RepoMappingSnapshot`: module rows are
  extended with source rows for extension-generated repositories, lockfile
  replay can build candidate source rows from the cached `generatedRepoSpecs`
  before validating that same cache, and root override rows are threaded into
  `BzlmodSessionData` / `ModuleExtensionExecutionKey` so DICE lockfile replay
  uses the same mapping semantics. This removes the previous conservative miss
  for `REPO_MAPPING:<extension_generated_repo>,<apparent>` when the apparent
  repo is a same-extension sibling or overridden generated repo. This is still
  transitional: the mapping is assembled during legacy bzlmod cell setup and
  carried through DICE; final Plan 61 still needs a typed DICE `RepoMappingKey`
  / `BzlmodResolutionKey` owner rather than injected startup snapshots.
  Validation passed: `cargo fmt --check`, `cargo test -p slug_bzlmod
  repo_mapping -- --nocapture` (23 passed), `cargo test -p slug_bzlmod
  recorded_repo_mapping -- --nocapture` (6 passed), `cargo test -p
  slug_bzlmod recorded -- --nocapture` (17 passed), `cargo test -p
  slug_bzlmod lockfile -- --nocapture` (52 passed), `cargo check -p
  slug_bzlmod -p slug_common -p slug_server -p slug_client_ctx -p
  slug_cmd_audit_client`, `cargo build -p slug`, focused direct pytest
  `pytest -q tests/core/bzlmod/test_plan61_guardrails.py -k 'extension_repo_source or recorded_repo_mapping'
  --tb=short` (2 passed), full direct pytest
  `pytest -q tests/core/bzlmod/test_plan61_guardrails.py --tb=short` (25
  passed), and `./target/debug/slug test
  tests/core/bzlmod:test_plan61_guardrails` (25 passed inside pytest). Daemons
  were cleaned afterward; `/var/mnt/dev/slug/buck-out` was about 3.5M and
  ZeroMatter `buck-out` remained about 3.3M.
- Implementation slice 2026-05-20, module extension facts carried in DICE
  results: Bazel ground truth is `SingleExtensionValue` carrying `Facts`,
  `SingleExtensionEvalFunction` loading facts from the workspace and hidden
  lockfiles before extension execution, and Bazel's intentional replay behavior
  that facts are not part of the normal replay diff check. `Facts` are also
  excluded from `LockfileModuleExtensionMetadata` because Bazel stores them in
  the top-level lockfile field. Slug already exposed `module_ctx.facts` and
  accepted `module_ctx.extension_metadata(facts = ...)`; this slice preserves
  returned facts in `ModuleExtensionResult` and populates replay-hit DICE
  results from prior lockfile facts instead of dropping metadata at the DICE
  boundary. This is still partial Plan 61 work: ordinary Slug builds still do
  not persist returned facts to `MODULE.bazel.lock`, and Bazel's
  `--lockfile_mode=error` facts validation against the visible workspace
  lockfile remains the next owner-level behavior to implement.
  Validation passed: `cargo fmt --check`, `cargo test -p slug_bzlmod
  module_extension_result_carries_facts_metadata -- --nocapture`, `cargo test
  -p slug_bzlmod extension_execution_dice -- --nocapture` (21 passed), `cargo
  check -p slug_bzlmod -p slug_interpreter_for_build -p slug_external_cells`,
  and `cargo build -p slug`.
- Implementation slice 2026-05-20, lockfile ERROR-mode facts validation:
  Bazel ground truth is `SingleExtensionEvalFunction`: it reads visible
  workspace and hidden lockfiles when lockfile mode is not `OFF`, passes visible
  facts to extension execution when present and otherwise falls back to hidden
  facts, deliberately excludes facts from normal replay invalidation, and in
  `LockfileMode.ERROR` compares newly returned facts only against visible
  workspace lockfile facts before reporting the lockfile as outdated. Slug now
  carries the configured hidden lockfile path through `BzlmodSessionData` into
  `ModuleExtensionExecutionKey`, uses hidden facts/cache only as fallback input,
  validates returned `module_ctx.extension_metadata(facts = ...)` against the
  visible lockfile in ERROR mode, and emits the Bazel-shaped
  `bazel mod deps --lockfile_mode=update` remediation. While validating this,
  a systemic failure path was fixed: extension execution errors discovered
  during extension-spoke registration are now propagated instead of logged and
  ignored, so eager repository materialization side effects cannot mask a
  failed extension evaluation. This remains read-only: ordinary Slug builds do
  not write returned facts to `MODULE.bazel.lock`, and hidden-lockfile parse
  policy still needs a separate Bazel-grounded pass if it becomes observable.
  Validation passed: `python3 -m py_compile
  tests/core/bzlmod/test_plan61_guardrails.py`, `cargo fmt --check`, `cargo
  test -p slug_bzlmod error_mode_facts -- --nocapture` (3 passed), `cargo test
  -p slug_bzlmod extension_execution_dice -- --nocapture` (24 passed), `cargo
  check -p slug_bzlmod -p slug_common -p slug_external_cells -p
  slug_interpreter_for_build`, `cargo check -p slug_external_cells -p
  slug_bzlmod`, `cargo build -p slug`, focused direct pytest
  `pytest -q tests/core/bzlmod/test_plan61_guardrails.py -k
  'changed_extension_facts' --tb=short` (1 passed), full direct pytest
  `pytest -q tests/core/bzlmod/test_plan61_guardrails.py --tb=short` (26
  passed), and `./target/debug/slug test
  tests/core/bzlmod:test_plan61_guardrails` (26 passed inside pytest). The
  Slug daemon was killed afterward; `/var/mnt/dev/slug/buck-out` was about
  3.5M and ZeroMatter `buck-out` remained about 3.3M.
- Implementation slice 2026-05-20, hidden lockfile parse policy: Bazel ground
  truth is `BazelLockFileFunction`, which reads the hidden output-base
  `MODULE.bazel.lock` with `LockfileMode.UPDATE` and treats hidden
  read/parse/value failures as `EMPTY_LOCKFILE`, while visible workspace
  lockfile parse failures remain hard errors. Slug now has an explicit
  `read_hidden_lockfile_path` helper with that policy; legacy cell preseed and
  DICE module extension execution both use it for hidden lockfiles instead of
  applying the visible lockfile mode/error behavior. This is still not final
  `LockfileContentKey` ownership: hidden and visible lockfile bytes are read
  directly during the transitional startup/DICE path, but the externally
  observable failure policy now matches Bazel's hidden-vs-visible split.
  Validation passed: `python3 -m py_compile
  tests/core/bzlmod/test_plan61_guardrails.py`, `cargo fmt --check`, `cargo
  test -p slug_bzlmod hidden_lockfile -- --nocapture` (2 passed), `cargo check
  -p slug_bzlmod -p slug_common`, `cargo build -p slug`, focused direct pytest
  `pytest -q tests/core/bzlmod/test_plan61_guardrails.py -k
  'hidden_lockfile' --tb=short` (2 passed), `cargo test -p slug_bzlmod
  lockfile -- --nocapture` (54 passed), full direct pytest
  `pytest -q tests/core/bzlmod/test_plan61_guardrails.py --tb=short` (27
  passed), and `./target/debug/slug test
  tests/core/bzlmod:test_plan61_guardrails` (27 passed inside pytest). The
  Slug daemon was killed afterward; `/var/mnt/dev/slug/buck-out` was about
  3.5M and ZeroMatter `buck-out` remained about 3.3M.
- Historical note, superseded on 2026-05-20: real-world SDK output parity was
  still pending until the zeromatter checkout had a Bazel-valid lockfile again.
  Later validation restored the lockfile, reran Bazel and Slug on
  `//sdk:sdk_contents`, and accepted only the documented output-root ELF string
  differences for the SDK checkpoint.
- Blocker verified after the zeromatter cleanup: `bazel build --nobuild
  //sdk:sdk_contents` still fails before repository mapping with
  `Illegal base64 character 2d` from `MODULE.bazel.lock`, and patched Slug now
  fails before analysis on the same malformed visible lockfile with
  `Failed to parse lockfile ... invalid bzlTransitiveDigest ... Invalid byte
  45, offset 6`. The zeromatter checkout still has a user-modified
  `MODULE.bazel.lock`, so continuing SDK output/archive parity is blocked
  unless that lockfile is regenerated/restored or a separate clean checkout is
  used.

Implementation slice 2026-05-19, staged local execroot isolation:

- After zeromatter cleanup produced a Bazel-valid lockfile again, the broad
  Slug `//sdk:sdk_contents` smoke reached local Rust compilation and exposed an
  execution-isolation bug. `crates__binary-stream-9.1.0//:_bs_` linked against
  metadata-only sibling outputs because local execution exposed the shared
  project `buck-out` through the per-action execroot. Bazel's action input tree
  hides undeclared siblings. `--sandbox` was not usable in this environment
  because bind mount setup failed with `Operation not permitted`, so the
  systemic owner is Slug's local action execroot model, not SDK code or the
  Rust crate.
- Replaced top-level `buck-out` exposure in `slug_execute_impl` with an
  `ActionExecrootPlan`: declared non-output inputs are linked into a digest
  execroot, declared writable `buck-out` parents are created as staging
  directories, and declared outputs are moved back to the real project
  `buck-out` after successful execution. The plan hash includes top-level input
  prefixes, exact declared `buck-out` inputs, and staged writable output dirs,
  preserving the isolation-dir component of generated paths.
- Reflection: the missing Bazel semantic is that local actions see only their
  declared inputs plus declared output directories, even when the workspace has
  large stale generated trees. The Slug owner is
  `app/slug_execute_impl/src/executors/action_execroot.rs` and
  `local.rs`. The broader class includes all local actions whose correctness can
  change when undeclared `buck-out` siblings leak into the execution tree.
  Rejected symptom fixes: deleting the sibling metadata rlib only for this run,
  special-casing `binary-stream`, forcing sandbox mode in an environment where
  it cannot mount, or adding SDK-specific cleanup before one target.
- Validation: `cargo test -p slug_execute_impl action_execroot --
  --nocapture` passed with the new staged-execroot coverage, `cargo build -p
  slug_execute_impl` passed, `cargo build -p slug` passed, and the previous
  binary-stream failure did not recur in the next broad smoke.

Implementation slice 2026-05-19, process-wrapper execroot substitutions:

- The next broad Slug smoke
  `/tmp/slug-plan61/plan61-sdk-staged-execroot-20260519-164034.log` advanced to
  `reactor//lib/units:units` and failed reading a build-script generated
  `OUT_DIR` file from a doubled path:
  `/var/mnt/dev/zeromatter-kuro/execroot/<hash>/execroot/<hash>/buck-out/...`.
  The generated file existed in the real project `buck-out`, and the staged
  action execroot had the declared `build_script.out_dir` input linked
  correctly.
- Reflection: the missing Bazel semantic is that `rules_rust`'s
  `process_wrapper` substitutions for `${exec_root}` and `${output_base}` must
  resolve to the action's Bazel-shaped execroot/output-base values. Slug's
  staged execroot materializes `external/` as a real directory containing both
  project `external` and `bazel-external` entries; the wrapper derives
  `output_base` from `cwd/external`, so it inferred
  `<project>/execroot/<hash>` as output base and then appended
  `execroot/<hash>` again. The Slug owner is the local executor command-line
  preparation path. The broader class is every `rules_rust` action using
  wrapper-managed `${exec_root}` or `${output_base}` substitutions in env, args,
  arg files, or generated include paths. Rejected symptom fixes: copying
  `acceleration.rs`, changing `lib/units` sources, symlinking the doubled path,
  deleting only the stale `build_script.out_dir`, or special-casing the
  `reactor` target.
- Implemented a local-executor rewrite for `process_wrapper` `--subst` entries
  when an action execroot is active: `exec_root=${exec_root}` is replaced with
  the actual staged execroot, and `output_base=${output_base}` is replaced with
  the project output base before the wrapper runs. `pwd=${pwd}` is left for the
  wrapper to resolve from its actual cwd. This keeps SDK-independent
  `process_wrapper` templates intact while avoiding wrapper inference from
  Slug's synthetic `external/` layout.
- Validation so far: `cargo fmt --check`, `cargo test -p slug_execute_impl
  process_wrapper -- --nocapture` (3 passed), `cargo test -p slug_execute_impl
  action_execroot -- --nocapture` (12 passed), and `cargo build -p slug`
  passed. Before the next smoke, stale zeromatter `buck-out` and `execroot`
  generated trees were removed after cleaning the idle Slug daemon; `execroot`
  had grown to about 73G, so future broad smokes must report and prune both
  trees as part of the loop.

Implementation slice 2026-05-20, stable Rust remap substitutions:

- The focused rerun
  `/tmp/slug-plan61/plan61-units-subst-fix-20260520-100606.log` proved the
  `reactor//lib/units:units` `OUT_DIR` failure was fixed, then advanced to a
  Rust proc-macro failure in
  `crates__postgres-derive-0.4.7//:postgres-derive`: rustc reported it could
  not find `syn`, `proc_macro2`, and `quote` even though the declared `--extern`
  rlibs existed in the staged action execroot and in project `buck-out`.
- Reflection: the missing Bazel semantic is that Rust's logical execroot path
  is stable across actions. Slug's staged execroot digest is intentionally
  different per input/output view, but `rules_rust` passes
  `--remap-path-prefix=${pwd}=.`, `${exec_root}=.`, and `${output_base}=.` into
  rustc. Rust includes remap flags in crate metadata, so substituting each
  action's physical digest execroot made crates compiled in different staged
  views metadata-incompatible. The local executor owns this because it creates
  the staged cwd and rewrites wrapper substitutions. The broader class is every
  Rust compile action whose dependencies are built in different staged
  execroots. Rejected symptom fixes: rebuilding only `postgres-derive`, copying
  or relinking the dependency rlibs, removing the proc-macro target, or adding
  crate-specific `--extern` paths.
- Implemented stable Unix process-wrapper substitutions under a staged
  execroot: `pwd` and `exec_root` map to `/proc/self/cwd`, and `output_base`
  maps to `/proc/self/cwd/../..`. Runtime paths such as
  `OUT_DIR=${exec_root}/buck-out/...` still resolve inside the current action
  cwd, while remap flags have stable text across actions. The extra local
  rustc execroot remap now skips existing generic `${pwd}` / `${exec_root}`
  remaps and otherwise uses the stable `/proc/self/cwd` alias on Unix instead
  of the physical digest path.
- Validation: `cargo fmt --check`, `cargo test -p slug_execute_impl
  rustc_flags_execroot_remap -- --nocapture` (2 passed), `cargo test -p
  slug_execute_impl process_wrapper -- --nocapture` (3 passed), `cargo test -p
  slug_execute_impl action_execroot -- --nocapture` (12 passed), and `cargo
  build -p slug` passed. The previous failed smoke left about 651M in
  `buck-out` and 14G in staged `execroot`; remove both before the next fresh
  focused rerun so old rlibs compiled with physical digest remaps cannot
  contaminate validation.
- The fresh rerun
  `/tmp/slug-plan61/plan61-units-stable-remap-20260520-102047.log` still
  failed at a proc-macro (`crates__clap_derive-4.5.55`) with the same
  `syn`/`proc_macro2`/`quote` visibility shape. Manual rustc probes from the
  failed action execroot showed `E0460` version mismatches: downstream rustc
  saw direct dependency rlibs through the staged execroot path while dependency
  crate metadata referred to the same generated rlibs through their resolved
  shared project `buck-out` paths.
- Grounding correction after user challenge: treating generated file inputs as
  hardlinks/copies was only an implementation hypothesis, not a demonstrated
  Bazel 9 requirement. It was tested diagnostically, then rejected. The fresh
  rerun
  `/tmp/slug-plan61/plan61-units-hardlink-inputs-20260520-102847.log` failed
  earlier than the Rust proc-macro frontier in
  `llvm//runtimes/glibc:glibc_library_search_directory`: a generated archive
  input resolved through a dangling symlink under `buck-out/.../external/...`.
  That shows the hardlink/copy direction changes Slug's input-tree semantics
  without Bazel ground-truth support and can break symlinked generated
  artifacts. The hardlink/copy change was reverted; retain it only as evidence
  that the proc-macro blocker needs a Bazel-grounded investigation.
- Reflection before the next fix: the active blocker is still the
  `syn`/`proc_macro2`/`quote` proc-macro visibility/version shape from
  `/tmp/slug-plan61/plan61-units-stable-remap-20260520-102047.log`, but the
  implementation direction must first be verified against Bazel/rules_rust
  ground truth. Candidate areas are Bazel action input-tree materialization,
  rules_rust `process_wrapper` substitution semantics, and rules_rust Rust
  metadata/full-rlib provider selection for proc-macro compilation. Rejected
  symptom fixes remain: rewriting only `--extern` flags to absolute project
  paths, adding metadata rlibs to this one proc-macro action, disabling Rust
  metadata builds, special-casing `clap_derive`, or changing Slug input files
  to hardlinks/copies unless Bazel 9 evidence requires that behavior.
- Next action: inspect pinned Bazel source/local Bazel action output and the
  local `rules_rust` sources before any new code change. Record exact evidence
  here, then implement only the smallest systemic fix that preserves Bazel's
  observable action semantics.
- Ground-truth evidence 2026-05-20:
  - Bazel source `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/sandbox/SymlinkedSandboxedSpawn.java`
    lines 35-39 and 127-130 shows the normal sandbox action input tree creates
    input files as symlinks. `HardlinkedSandboxedSpawn.java` lines 35-39 and
    74-102 plus `SandboxOptions.java` lines 333-344 show hardlinks are only
    the opt-in `--experimental_use_hermetic_linux_sandbox` behavior, not a
    general Bazel requirement.
  - `rules_rust` process wrapper source
    `/var/mnt/dev/zeromatter-kuro/bazel-external/rules_rs++rules_rust+rules_rust/util/process_wrapper/options.rs`
    lines 128-154 derives `output_base` by canonicalizing `cwd/external` and
    derives `exec_root` from that real output base. `rust/private/rustc.bzl`
    lines 1013-1017 and 1104-1108 prove `rules_rust` relies on those
    substitutions for `--remap-path-prefix=${pwd}`, `${exec_root}`, and
    `${output_base}`.
  - Focused Bazel 9.0.1 probe:
    `bazel aquery --include_commandline --output=textproto 'mnemonic("Rustc", deps(//lib/units:units))'`
    saved to `/tmp/slug-plan61/bazel-aquery-lib-units-rustc.textproto`.
    The `clap_derive` action at lines 134532-134606 uses full `.rlib`
    `--extern` inputs for `proc_macro2`, `quote`, and `syn`, so switching
    proc-macro direct deps to metadata rlibs would not match Bazel.
- Systemic fix slice: keep Slug's symlinked declared-input execroot. Change
  only process-wrapper substitution rewriting so Unix `${pwd}` remains the
  stable action cwd alias `/proc/self/cwd`, while `${exec_root}` and
  `${output_base}` expand to Slug's real project/output base path. This matches
  the Bazel/rules_rust requirement that `${output_base}` be a canonical path
  capable of remapping symlink-resolved generated inputs back to stable
  workspace-relative metadata paths. The previous `/proc/self/cwd/../..`
  output-base value was lexically correct but did not match canonical
  `/var/.../zeromatter-kuro/buck-out/...` symlink targets recorded by rustc.
- The fresh rerun
  `/tmp/slug-plan61/plan61-units-output-base-remap-20260520-174649.log`
  passed the previous `clap_derive` frontier and failed later at
  `crates__zeroize_derive-1.4.3//:zeroize_derive`, where rustc reported it
  could not find `proc_macro2`, `quote`, and `syn`. A manual rustc probe from
  the failed action execroot (`/tmp/slug-plan61/zeroize_probe.log`) showed
  `E0460` crate hash mismatches rather than missing files.
- Ground-truth correction 2026-05-20:
  - Bazel 9.0.1 aquery
    `/tmp/slug-plan61/bazel-aquery-lib-units-rustc.textproto` shows
    proc-macro actions such as `zeroize_derive` and `clap_derive` use full
    rlibs for direct `proc_macro2`, `quote`, and `syn` deps, while normal Rust
    library actions use metadata rlibs under `_meta/`.
  - The same Bazel action graph uses the root override-selected
    `external/rules_rust+/.../util/process_wrapper/process_wrapper`, and the
    selected override source
    `/home/wgray/.cache/kuro/overrides/rules_rust/archive-349c75fc87e6a58/rust/private/rust.bzl`
    declares metadata outputs under `_meta/`.
  - Slug's failed action instead used
    `rules_rs++rules_rust+rules_rust//util/process_wrapper`, whose materialized
    source declared sibling `*_meta.rlib` outputs without the `_meta/`
    directory. That source shape disagrees with Bazel's selected override and
    explains the full-vs-metadata crate hash split.
- Reflection: the missing Bazel semantic is `override_repo()` visibility for
  repositories owned by a module extension. In ZeroMatter,
  `override_repo(rules_rust_ext, rules_rust = "rules_rust")` must make the
  extension-generated repo name resolve to the root module's selected
  `rules_rust` dependency, even when labels are evaluated from repos owned by
  `@rules_rs//rs/experimental:rules_rust.bzl`. The Slug owner is Bzlmod repo
  mapping/cell alias precomputation, specifically
  `pending_repo_cells::pre_compute_extension_repo_cells` and
  `legacy_configs::cells` alias registration. The broader class is every module
  extension that overrides a generated repo with a selected module dependency,
  not only `rules_rust`. Rejected symptom fixes: hardlinking generated inputs,
  switching this one proc-macro to metadata deps, patching the generated
  `rules_rs++rules_rust+rules_rust` repository contents, hardcoding
  `rules_rust`, or special-casing `zeroize_derive`/`clap_derive`.
- Implementation update: `pre_compute_extension_repo_cells` now emits both the
  owner-scoped apparent alias (`generated -> actual_dep`) and the generated
  canonical repo alias (`owner++ext+generated -> actual_dep`) for
  `override_repo()` entries, independent of whether `use_repo()` imports the
  same repo. `legacy_configs::cells` now resolves raw override targets such as
  `rules_rust` through the selected bzlmod graph before registering global or
  scoped aliases, so the alias points at the selected Slug cell
  (`rules_rust+<version>`) instead of a non-existent raw dep name.
- Validation so far: `cargo fmt`, `cargo test -p slug_bzlmod
  test_precompute_use_repo_honors_override_repo -- --nocapture`,
  `cargo check -p slug_common`, and `cargo build -p slug` pass. Before the next
  focused smoke, remove ZeroMatter's stale `buck-out` and staged `execroot`
  trees; the previous failed run left roughly 670M and 14G respectively.
- Follow-up smoke
  `/tmp/slug-plan61/plan61-units-override-repo-20260520-110302.log`
  confirms the override direction: the next failure path is under
  `gen/rules_rust/.../external/rules_rust/...`, and the materialized selected
  `rules_rust/0.69.0` source contains the Bazel-observed `_meta/` metadata
  output layout. The run stopped before the Rust frontier because the deferred
  materializer panicked inserting an already-declared path into SQLite:
  `.../external/rules_rust/util/process_wrapper/private/bootstrap_process_wrapper`.
- Reflection: the missing semantic is that materializer state updates must be
  replacement-safe for a declared artifact path. Bazel's observable build graph
  has one artifact path for the selected `rules_rust` output; Slug's aliasing
  and local execution may notify the materializer about that path more than
  once, but the materializer must keep one current state entry rather than crash
  on a duplicate persistence row. The Slug owner is the deferred materializer's
  SQLite state mirror, not Bzlmod selection or Rust action construction. The
  broader class includes any duplicate or replacement `declare_existing` /
  materialization update for the same artifact path, especially when aliases or
  repeated local output declarations converge on one project-relative output.
  Rejected symptom fixes: deleting this one SQLite row by hand, turning off the
  materializer database, special-casing `rules_rust`, or ignoring the corrected
  override alias.
- Implementation update: `MaterializerStateSqliteTable::insert` now replaces
  existing state for the artifact path inside the same transaction before
  inserting current metadata, deleting both the prior root row and any persisted
  full-directory member rows. Regression coverage
  `test_insert_replaces_existing_artifact_state` replaces a full directory
  artifact with a file artifact to catch duplicate primary-key rows and stale
  child rows. Validation: `cargo fmt`, `cargo test -p slug_execute_impl
  test_insert_replaces_existing_artifact_state -- --nocapture`,
  `cargo test -p slug_bzlmod test_precompute_use_repo_honors_override_repo --
  --nocapture`, and `cargo build -p slug` pass. The failed smoke left about
  205M in `buck-out` and 7.0G in staged `execroot`; clean those generated trees
  before the next rerun.
- Follow-up smoke
  `/tmp/slug-plan61/plan61-units-materializer-replace-20260520-110926.log`
  reaches the Rust frontier and fails compiling
  `crates__serde_derive-1.0.228//:serde_derive` with E0460 "possibly newer
  version" errors for `unicode_ident` / `proc_macro2`, then unresolved imports
  from `quote`. The materializer duplicate-row panic is gone. The Slug rustc
  command now uses the selected `rules_rust` process wrapper path under
  `gen/rules_rust/.../external/rules_rust/...`, but Slug's generated outputs
  for `proc_macro2` include `libproc_macro2-923445808.rlib` and
  `libproc_macro2-923445808_meta.rlib` in the same directory, with no
  `_meta/libproc_macro2-..._meta.rlib`.
- Bazel ground truth: Bazel 9.0.1 aquery for
  `mnemonic("Rustc", deps(//lib/units:units))` in ZeroMatter writes the
  metadata action output under
  `bazel-out/.../bin/external/rules_rs++crate+crates__proc-macro2-1.0.106/_meta/libproc_macro2-..._meta.rlib`.
  The selected override source
  `buck-out/.../external_cells/bzlmod/rules_rust/0.69.0/rust/private/rust.bzl`
  declares `_meta/` outputs, while the stale generated repository source under
  `bazel-external/rules_rs++rules_rust+rules_rust/rust/private/rust.bzl`
  declares sibling metadata without `_meta/`. Bazel's `serde_derive` proc-macro
  action does not set `RUSTC_BOOTSTRAP=1`; adding that env var is not a
  grounded fix.
- Reflection: the next systemic fix must determine whether Slug loses the
  `_meta/` subdirectory while implementing `ctx.actions.declare_file("_meta/...",
  sibling=...)`, or whether Slug still loads rule implementation files from the
  stale generated `rules_rs++rules_rust+rules_rust` repository despite the
  corrected override alias for action/tool labels. The owner is therefore
  either declare-file path joining or Bzlmod canonical repo resolution for
  extension-generated repos overridden to selected module repos. Rejected
  symptom fixes: hardlinking action inputs, injecting `_meta` into Rust command
  lines, switching only `serde_derive`/`proc_macro2` dependency paths,
  hand-patching the generated repository, or adding `RUSTC_BOOTSTRAP` to
  proc-macro actions.
- Bazel source grounding for the override direction: local Bazel 9 source
  `BazelDepGraphFunction.java` lines 220-247 builds extension repo overrides
  from root usages only and resolves override targets through the root module
  mapping. `ModuleExtensionRepoMappingEntriesFunction.java` lines 58-78 adds
  those override entries to the repository mapping visible from extension
  repos, and `SingleExtensionUsagesFunction.java` lines 53-65 carries the same
  override row with the collected extension usages. This means a root
  `override_repo()` must affect owner modules and extension-generated repo
  mappings even when the owner module itself does not declare that override.
- Implementation update: `pending_repo_cells::pre_compute_extension_repo_cells`
  now aggregates root-owned extension repo overrides and applies them when
  precomputing owner-module `use_repo()` aliases. `repo_mapping::for_module`
  also maps generated canonical override names such as
  `rules_rs++rules_rust+rules_rust` to the selected dependency, and apparent
  repo canonicalization consults repo mapping before treating `+`-containing
  names as already canonical. `legacy_configs::cells` registers dynamic
  generated-canonical aliases for these override entries even when an old
  generated extension cell with the same name exists.
- Validation for that slice: `cargo fmt`, `cargo test -p slug_bzlmod
  test_root_override_repo_applies_to_owner_module_use_repo -- --nocapture`,
  `cargo test -p slug_bzlmod test_precompute_use_repo_honors_override_repo --
  --nocapture`, `cargo test -p slug_bzlmod
  canonicalizes_override_generated_repo_name_to_selected_dep -- --nocapture`,
  `cargo test -p slug_bzlmod
  canonicalizes_keyword_use_repo_and_override_repo -- --nocapture`, `cargo test
  -p slug_core dynamic_alias_overrides_existing_generated_extension_cell --
  --nocapture`, `cargo check -p slug_common`, and `cargo build -p slug` pass.
- Follow-up smoke
  `/tmp/slug-plan61/plan61-units-root-override-aggregate-20260520-113738.log`
  confirms the stale generated `rules_rs++rules_rust+rules_rust` rule-source
  path is no longer the active frontier: waits now reference the selected
  `rules_rust//util/process_wrapper...` path, and generated crate outputs now
  include Bazel-shaped metadata directories such as
  `.../rules_rs++crate+crates__proc-macro2-1.0.106/_meta/libproc_macro2-923445808_meta.rlib`.
  The smoke still fails compiling
  `crates__thiserror-impl-2.0.18//:thiserror-impl` with E0463 for
  `proc_macro2`, `quote`, and `syn`, while the full `--extern` rlibs exist in
  project `buck-out` and contain valid archive entries. The `_meta/` topology
  blocker is therefore resolved, and the next blocker is the visibility or
  validity of full proc-macro dependency rlibs as action inputs.
- Reflection before the next fix: the missing semantic must be tied to Bazel's
  Rust action graph and sandbox/input behavior, not to hardlinking or metadata
  path reshaping. Candidate owners are Slug's action input declaration/staging,
  rustc command construction, or process-wrapper working-directory
  substitutions. Rejected symptom fixes remain: hardlinking action inputs,
  adding `_meta` paths to proc-macro `--extern` arguments, adding
  `RUSTC_BOOTSTRAP`, special-casing `thiserror-impl` or `serde_derive`, and
  changing generated repository contents without a Bazel source/probe anchor.
- Ground-truth correction 2026-05-20: manual rustc metadata probes classify the
  current `thiserror-impl` E0463 as a Rust crate hash split, not as a missing
  file. Slug's full `unicode_ident` rlib has hash
  `47513d36c8f4abbbc36b5fb2378c8b78`, while Slug's metadata rlib has hash
  `8d901924550e221a6bb1369b7f2dee51`; `proc_macro2` was compiled expecting
  the metadata hash. A focused Bazel 9.0.1 probe for the same crates shows
  Bazel's full `unicode_ident` rlib hash
  `3f9553724e0ff545bbb34e4c51cd80a9`, and Bazel's `proc_macro2` full rlib
  expects that same full hash. Bazel therefore keeps full and metadata Rust
  actions for the same crate SVH-compatible; Slug does not.
- `rules_rust` process-wrapper source
  `/var/mnt/dev/zeromatter-kuro/buck-out/plan61-units-root-override-aggregate-20260520-113738/external_cells/bzlmod/rules_rust/0.69.0/util/process_wrapper/options.rs`
  lines 137-178 explains the owning semantic: `${output_base}` may be
  sandbox-local for path remapping, but `${exec_root}` is resolved to the
  stable Bazel execroot so `CARGO_MANIFEST_DIR` / `OUT_DIR` values embedded in
  rustc metadata are identical across Rustc and RustcMetadata actions. The
  next systemic fix is to preserve Slug's narrowed action cwd while expanding
  `${exec_root}` to a stable shared Slug execroot path, not to the per-action
  digest execroot and not to the project root.
- Follow-up smoke
  `/tmp/slug-plan61/plan61-units-stable-execroot-20260520-120400.log` proves
  the first stable `${exec_root}` fix was incomplete: it still fails
  `crates__serde_derive-1.0.228//:serde_derive` with E0460. Manual metadata
  probes show Slug's full `unicode_ident` hash is
  `e2d027c3656756157431f7ba8ba51207`, the metadata hash is
  `3c75e5f1cd47aad079043321602d881e`, and `proc_macro2` expects the metadata
  hash. Re-reading the selected `rules_rust` process-wrapper source lines
  132-194 corrects the implementation direction: `${pwd}` is the actual action
  current directory, and `${output_base}` is the sandbox-local output base used
  by `--remap-path-prefix` to strip that directory from rustc metadata. Only
  `${exec_root}` should be the stable shared execroot. The next fix is
  therefore to expand `${pwd}` and `${output_base}` to Slug's per-action
  execroot while keeping `${exec_root}` stable.
- Implementation update: `local.rs` now rewrites `process_wrapper` substitutions
  to match the selected `rules_rust` behavior: Unix `${pwd}` and
  `${output_base}` expand to Slug's per-action execroot for
  `--remap-path-prefix`, while `${exec_root}` expands to the stable shared
  `<project>/execroot/<workspace>` path used by `CARGO_MANIFEST_DIR` and
  `OUT_DIR`. The auxiliary direct rustc execroot remap also uses the
  per-action execroot rather than `/proc/self/cwd`.
- Validation: `cargo fmt`, `cargo test -p slug_execute_impl process_wrapper --
  --nocapture`, `cargo test -p slug_execute_impl rustc_flags_execroot_remap --
  --nocapture`, and `cargo build -p slug` pass. Fresh focused smoke
  `/tmp/slug-plan61/plan61-units-wrapper-pwd-outputbase-20260520-121217.log`
  succeeds for `reactor//lib/units:units` after 684 local commands. This
  resolves the observed Rust proc-macro full-vs-metadata crate-hash blocker.
  The successful smoke left about 998M in ZeroMatter `buck-out` and 15G in
  staged `execroot`; clean those generated trees before the next broad
  `//sdk:sdk_contents` smoke.
- Reflection during full SDK smoke 2026-05-20 12:34 PDT: `execroot` growth is
  caused by repeated per-action `external/` fanout, not by hardlinks. Each
  narrowed execroot materialized a real `external/` directory with every
  apparent and canonical bzlmod repo symlink; a live sample showed roughly
  1,196 per-action execroot directories, many with about 22M of symlink entries
  under `external/`. Bazel ground truth from the local Bazel 9.0.1 sandbox
  stash
  `~/.cache/bazel/_bazel_wgray/.../sandbox/sandbox_stash/Rustc/335/execroot/_main/external`
  showed a sparse action external tree for that Rust action: 62 external
  entries, not the full bzlmod repo universe. Systemic direction: keep Slug's
  narrowed execroot cwd model, but track external repo names from declared
  paths and link only those repos into `external/`.
- Focused smoke
  `/tmp/slug-plan61/plan61-units-sparse-external-20260520-124720.log` failed
  quickly with `clang: error: no such file or directory:
  'external/llvm/runtimes/empty.c'` while disk stayed bounded (`execroot`
  about 2.1M). The failure is a refinement of the sparse-external fix:
  declared inputs may select a canonical module repo such as
  `external/llvm+0.7.0`, while action argv uses the apparent alias
  `external/llvm`. Ground truth from the local `external/` alias tree and
  Bazel sandbox behavior is to expose aliases for selected repos, not every
  repo. Next fix: when a selected external repo target is linked, also link
  any `external/<apparent>` entries that resolve to the same target.
- Focused smoke
  `/tmp/slug-plan61/plan61-units-sparse-external-alias-20260520-125041.log`
  passed after alias-refined sparse external materialization. It progressed
  through the prior `external/llvm/runtimes/empty.c` action, compiled LLVM
  runtime and Rust crates, and finished with `BUILD SUCCEEDED`, 684 local
  commands, and final disk use of about 998M `buck-out` plus 233M `execroot`.
  This keeps the per-action execroot model aligned with the observed Bazel
  9.0.1 sparse sandbox rather than relying on full `external/` fanout.
- Full SDK smoke
  `/tmp/slug-plan61/plan61-sdk-contents-sparse-external-20260520-125322.log`
  timed out at the 20 minute bound, but did not expose a semantic failure. The
  queue was still draining and reached 5 remaining actions at cutoff after
  compiling high-level `reactor//...` Rust targets; disk ended around 14G
  `buck-out`, 3.8G `execroot`, and 11,201 first-level/two-level action dirs.
  This is not the previous unbounded `external/` fanout: the growth is
  proportional to broad SDK action output/staging. Next action is to continue
  from the warmed outputs with a larger bounded smoke and only classify a
  performance blocker if progress stalls or the same small frontier repeats.
- Full SDK resume
  `/tmp/slug-plan61/plan61-sdk-contents-sparse-external-resume-20260520-1315.log`
  passed with `BUILD SUCCEEDED` for `//sdk:sdk_contents` under Slug. The run
  completed 3,759 local commands in 25m36s (`load=1m22s`,
  `analyze=6m27s`, `execute=18m57s`, `materialize=18m57s`), with top mnemonics
  `rustc=14m16s/1655`, `rustc_metadata=8m28s/1473`, and
  `c_compile=47.5s/376`. Final disk use was about 21G `buck-out` and 4.0G
  `execroot`; action directory count stayed bounded at roughly 11.2k rather
  than growing through full `external/` fanout. At this point the remaining SDK
  checkpoint criterion was output parity against Bazel 9 for the same target;
  that checkpoint is superseded by the later accepted-difference result below.
- Ground-truth correction 2026-05-20, optional C++ toolchain handling:
  Bazel/rules_rust evidence from
  `/tmp/slug-plan61/bazel-aquery-zeromatter-ffi-rustc.txt` shows the final
  `zeromatter_ffi` Rust link action receives LLVM C++ toolchain link flags from
  `cc_common.get_memory_inefficient_command_line`, including
  `-target x86_64-linux-gnu`, `-fuse-ld=lld`, `-resource-dir`, crt/glibc
  directories, and native C++ runtime libraries. Slug's previous optional C++
  native-shim deferral was therefore too broad: `mandatory=False` in
  `rules_rust` means the toolchain may be absent, not that a successfully
  resolved C++ toolchain should be replaced by a featureless fallback. The
  systemic fix is to build the metadata-backed C++ native shim whenever
  toolchain resolution returns a C++ toolchain, and reserve `None` for an actual
  optional miss. Rejected symptom fixes: appending link flags in Rust rules,
  target-specific `zeromatter_ffi` handling, or featureless fallback expansion.
  Validation passed: `cargo test -p slug_build_api
  optional_cpp_toolchain_without_resolved_provider_returns_none -- --nocapture`,
  `cargo test -p slug_build_api
  cc_toolchain_overlay_retains_provider_owner_for_late_attr_access --
  --nocapture`, `cargo test -p slug_build_api_tests
  cc_common_dynamic_library_uses_feature_args_instead_of_fallback_prefix --
  --nocapture`, `cargo test -p slug_build_api_tests
  cc_common_flag_sets_expand_action_configs_before_features -- --nocapture`,
  `cargo test -p slug_execute_impl action_execroot -- --nocapture`, and
  `cargo build -p slug`.
- Focused Slug smoke after that correction
  `/tmp/slug-plan61/plan61-zeromatter-ffi-optional-cc-fix-20260520-141458.log`
  advanced past the previous missing-link-flags blocker and exposed a new
  `CargoBuildScriptRun` failure in
  `crates__aws-lc-sys-0.38.0//:_bs`: clang/lld could not open
  `buck-out/.../external/llvm/runtimes/resource_directory/lib/x86_64-unknown-linux-gnu/libclang_rt.builtins.a`
  inside the per-action execroot. The same file exists in the shared Slug
  `buck-out` tree, while the per-action execroot was missing the entire
  `resource_directory` path. Current generated-tree size after the failed
  smoke was about 2.7G `buck-out` and 940M `execroot`.
- Bazel 9.0.1 ground truth for that new blocker:
  `/tmp/slug-plan61/bazel-aquery-aws-lc-sys-bs.txt` for
  `@@rules_rs++crate+crates__aws-lc-sys-0.38.0//:_bs` reports mnemonic
  `CargoBuildScriptRun`, the same C/C++/link flag shape, and explicitly lists
  `bazel-out/k8-fastbuild/bin/external/llvm+/runtimes/resource_directory` as an
  action input alongside the other LLVM runtime directories. That proves the
  correct direction is declared input propagation/materialization for generated
  directory artifacts, not changing `LDFLAGS`, removing `-resource-dir`, relying
  on hardlinks, or exposing broad `buck-out` siblings.
- Systemic fix 2026-05-20: Bazel/rules_rust source shows
  `cargo/private/cargo_build_script.bzl` appends `cc_toolchain.all_files` to
  build-script tools when a C++ toolchain is present, and the LLVM toolchain
  declares the `resource_directory` output as data on the link-action
  `resource_dir` arg. Slug's metadata-backed native C++ toolchain shim exposed
  only `compiler_files` through `all_files`, so downstream build scripts never
  received the Bazel-declared linker/runtime directory inputs. The fix is at
  the C++ toolchain provider boundary: `CcToolchainInfo.all_files` now merges
  compiler, linker, static-runtime, and dynamic-runtime toolchain files, while
  `_compiler_files` and `_linker_files` remain scoped to their specific
  providers. Rejected symptom fixes: hardlinking action inputs, broadening the
  per-action execroot to all `buck-out` siblings, editing `aws-lc-sys`
  `LDFLAGS`, or special-casing `resource_directory`.
- Validation for the `all_files` fix: `cargo test -p slug_build_api
  native_cc_toolchain_all_files_includes_linker_and_runtime_inputs --
  --nocapture`, `cargo test -p slug_build_api
  optional_cpp_toolchain_without_resolved_provider_returns_none -- --nocapture`,
  `cargo test -p slug_build_api
  cc_toolchain_overlay_retains_provider_owner_for_late_attr_access --
  --nocapture`, and `cargo build -p slug` all passed. Focused Slug smoke
  `/tmp/slug-plan61/plan61-zeromatter-ffi-cc-all-files-20260520-144226.log`
  then built `//sdk/zeromatter_ffi:zeromatter_ffi` successfully in 24m11s,
  including `llvm//runtimes:resource_directory`, `aws-lc-sys` build-script
  execution, and final `libzeromatter_ffi.so` linking. Generated-tree size
  after the successful focused run was about 11G `buck-out` and 2.9G staged
  `execroot`; keep warmed `buck-out` for full SDK parity, then clean staged
  execroots after validation.
- Full SDK validation after the `all_files` fix:
  `/tmp/slug-plan61/plan61-sdk-contents-after-cc-all-files-20260520-150740.log`
  built `//sdk:sdk_contents` successfully in 12m06s. Directory/file manifest
  and modes match Bazel 9 output exactly, and all non-ELF file hashes match.
  The only remaining differences are still the four ELF outputs:
  `bin/zm`, `bin/zerobuf`, `bin/zerosystem`, and
  `lib/libzeromatter_ffi.so`
  (`/tmp/slug-plan61/*-sdk-contents-*-after-cc-all-files.txt`).
- Accepted known byte difference 2026-05-20, Bazel logical derived-artifact
  exec paths:
  ELF strings show Bazel embeds generated-source paths like
  `./bazel-out/k8-fastbuild.../build_script.out_dir/...`, while Slug embeds
  `./buck-out/plan61-optional-cc-fix-20260520/gen/.../build_script.out_dir/...`.
  This is not a binary post-processing issue. Bazel source
  `ArtifactRoot.java` lines 72-82 defines derived artifact exec paths under
  `$EXEC_ROOT/bazel-out/<output-dir>/bin/...`; `OutputPathMnemonicComputer.java`
  lines 158-175 and 365-374 compute `<output-dir>` from CPU, compilation mode,
  platform suffix, fragment contributions, and ST transition hashes. The local
  Bazel aquery for `deps(//sdk:sdk_contents)` confirms the concrete values used
  here: GNU target actions use `bazel-out/k8-fastbuild/bin/...`, and musl
  target actions use `bazel-out/k8-fastbuild-ST-43014ebae176/bin/...`.
- User decision 2026-05-20: output-root string differences embedded in ELF
  debug/build metadata are acceptable for this Plan 61 SDK parity checkpoint
  when the SDK builds, tree shape/modes match, and only the known path-root
  strings explain byte differences in the four ELF outputs. Therefore this is
  no longer a Plan 61 hard blocker. It remains a follow-up parity/design item:
  Slug should support a Bazel-compatible output-root mode that stores or
  exposes generated artifacts under `bazel-out`, not `buck-out`, and `slug-out`
  is a more accurate non-Bazel-compatible root name if/when a non-Bazel mode is
  retained.
- rules_rust source explains why the follow-up matters for exact byte parity:
  `rust/private/rustc.bzl` lines 1031-1044 deliberately sets `OUT_DIR` to
  `${exec_root}/<Bazel exec path>` because `env!("OUT_DIR")` is baked into Rust
  crates and `--remap-path-prefix` does not normalize that raw env value; lines
  1118-1122 add the remap flags only as a separate debug-path mechanism. Slug
  currently has only Slug configuration hashes in artifact paths
  (`buck-out/<isolation>/gen/<cell>/<cfg_hash>/...`) and does not model Bazel's
  output-directory mnemonic/ST-hash computation. Implementing a correct fix
  requires a Bazel-grounded artifact exec-path layer that can expose
  `bazel-out/<output-dir>/bin/...` to actions and optionally store outputs
  there. Rejected symptom fixes: replacing strings after link, stripping debug
  info, mapping every Slug config hash to a hand-written `k8-fastbuild` value,
  or adding Rust-only remap flags that leave `env!("OUT_DIR")` wrong.

Implementation update 2026-05-21, root MODULE parsing DICE bridge:

- Bazel ground truth: `ModuleFileValue` is the Skyframe value for a parsed
  module file and `ModuleFileFunction` reads/expands module-file inputs before
  `BazelModuleResolutionFunction` consumes the root `ModuleFileValue`. Local
  anchors used for this slice:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileValue.java:33`,
  `:68`,
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileFunction.java:163`,
  and
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/BazelModuleResolutionFunction.java:99`.
- Slug now computes the root `MODULE.bazel` parse through
  `RootModuleFileKey` during server config loading and passes that parsed value
  into the legacy cell bridge. Direct filesystem parsing remains only for
  bootstrap/completion paths without a DICE transaction. This is deliberately a
  transitional bridge: MVS, local override module files, and the bzlmod cell
  graph are still computed inside legacy `cells.rs`.
- Validation before the first broad smoke:
  `cargo test -p slug_bzlmod root_module_file -- --nocapture`,
  `cargo check -p slug_bzlmod -p slug_common -p slug_server`,
  `cargo fmt --check`, `git diff --check`, `cargo build -p slug`, and
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py -rx --tb=short` all passed.
- Fresh ZeroMatter no-exec smoke
  `/tmp/slug-plan61/plan61-noexec-after-root-module-dice-20260521-011500.log`
  was stopped after about two hours with CPU still active and no client log
  progress beyond the already-known
  `rules_rust//ffi/rs:empty_allocator_libraries` analysis wait. The event log
  was not preserved before cleanup; preserved evidence is the text log copied
  to
  `/tmp/slug-plan61/plan61-noexec-after-root-module-dice-20260521-011500.hung.log`.
  Generated ZeroMatter output for that isolation was cleaned immediately
  afterward, reducing `/var/mnt/dev/zeromatter-kuro/buck-out` back to about
  3.3M and removing `execroot`.
- Blocker reflection: this is not evidence of a DICE dependency cycle; the
  root key does not request any other DICE keys. The systemic issue found in
  the slice was that the bridge was only root-digest based and reread the root
  module during parsing, while Bazel's `ModuleFileValue` dependency shape covers
  the actual module-file inputs. Slug now parses the root module from the bytes
  already read by `RootModuleFileKey` and computes the equality token over the
  root file plus every included `.MODULE.bazel` segment consumed by
  `include()`. The key remains non-cacheable (`validity=false`) until the root
  and include reads are backed by tracked DICE filesystem inputs. Rejected
  shortcuts: making the current direct-read key cacheable, ignoring includes in
  equality, or treating the broad smoke wait as a completed validation.

Exit criteria:

- Tests prove warm daemon reuse without stale cross-workspace state.
- Real-world smokes stay usable throughout the migration; cold read-only audit
  startup for zeromatter must stay below 10s, and any regression is a Plan 61
  performance failure rather than a smoke to wait out.

## Acceptance Criteria

| Claim | Grounding / validation |
|---|---|
| Editing root `MODULE.bazel` invalidates the resolved graph and cell graph. | Bazel `BazelModuleResolutionFunction` depends on root `ModuleFileValue`; current Slug parses before DICE in legacy `cells.rs`; experiment `plan61-module-input-dice-diff`. |
| Editing or creating/deleting a local override's `MODULE.bazel` invalidates only affected bzlmod graph nodes. | Bazel non-registry override loading in `ModuleFileFunction`; current Slug local override resolution in legacy `cells.rs`; experiment `plan61-local-override-dice-diff`. |
| Registry metadata/source JSON/integrity changes are modeled as DICE inputs. | Bazel lockfile docs and `IndexRegistry` / `RepoSpecFunction`; experiment mutates local registry metadata and `source.json`. |
| MVS compatibility conflicts fail like pinned Bazel. | Bazel `Selection`; experiment with conflicting compatibility levels and multiple-version override. |
| Extension `.bzl` changes invalidate or reject cached extension replay. | Bazel `RegularRunnableExtension` / `SingleExtensionEvalFunction`; experiment edits implementation and transitive helper `.bzl`. |
| Tag usage changes update `usagesDigest` and invalidate/reject replay. | Bazel `SingleExtensionUsagesValue`; experiment mutates attrs and tag order where pinned Bazel treats order as relevant. |
| Recorded env/file/dir/tree/repo-mapping input mismatches reject replay. | Bazel `RepoRecordedInput`, `StarlarkBaseExternalContext`, `SingleExtensionEvalFunction`; experiment mutates each input class. |
| Valid lockfile `generatedRepoSpecs` can register and materialize generated repos without executing the extension. | Bazel `SingleExtensionEvalFunction` lockfile hit path; Slug Plan 38 current lockfile preseed bridge. |
| Missing lockfile entries execute the extension once in DICE, then reuse DICE state inside the daemon. | Bazel lockfile miss execution path; Slug DICE counter `extension_eval` proves one execution on warm daemon. |
| Label-taking `module_ctx` / `repository_ctx` operations materialize needed extension repos or fail directly. | Bazel `ModuleExtensionContext` extends `StarlarkBaseExternalContext`; source shows `path`, `read`, `watch`, `symlink`, `template`, `patch`, `load_wasm`, and `execute_wasm` accept labels, while plain `execute()` records env/PATH/working-directory effects separately. Plan 36 bug shape; experiments split path/read/watch/write-like operations. |
| Unknown repo rule, extension failure, repo-rule failure, missing generated repo, and invalid override fail directly with no stub repo. | Bazel `SingleExtensionFunction`, `RepoDefinitionFunction`, `RepositoryFetchFunction`; current Slug stub paths; no-stub tests assert no repo directory/marker. |
| Repo materialization reuse is not based on bare `.slug_repo_complete`. | Bazel marker/recorded-input semantics or Slug manifest experiment; current Slug marker bug shape in Plan 38 and source. |
| Two workspaces with colliding module/repo names do not share bzlmod state. | Bazel has one server per output base and defaults output base from the workspace root; Slug daemon dirs include project root. Guardrail uses identical extension/repo names with different workspace roots and lockfiles, and validates the appropriate per-daemon or same-daemon counter baseline. |
| Ordinary Slug read-only paths do not mutate `MODULE.bazel.lock`; future write path matches Bazel modes. | Bazel `RepositoryOptions` / `BazelLockFileModule`; Slug policy is explicit interim safety, validated by `plan61-lockfile-modes`. |

## Test Matrix

| Area | Tests / grounding |
|---|---|
| Lockfile policy | Valid lockfile cold/warm repo, stale digest, stale recorded input, failed build, interrupted materialization, create/delete/edit after prior daemon miss/hit; grounded in Bazel lockfile docs/source and Plan 57. |
| DICE invalidation | Root module edit, include edit, local override edit, registry metadata edit, extension `.bzl` edit, tag attr edit, recorded input edit, facts availability on re-execution but not facts invalidation; grounded in Bazel `SingleExtensionEvalFunction` facts behavior. |
| Extension execution | Simple extension hub/spoke generation, valid replay hit, miss execution, no eager full-closure materialization unless pinned Bazel does the same; grounded in Bazel Skyframe extension execution and Plan 36 eager-materialization correction. |
| Label materialization | Separate tests for `path`, `read`, `watch`, `symlink`, `template`, `patch`, `load_wasm`, and `execute_wasm`; plain `execute()` tests cover env, `PATH`, and working-directory invalidation, not `execute(Label)`. Grounded in `ModuleExtensionContext`, `StarlarkBaseExternalContext`, and Plan 36. |
| Repo mapping | Apparent/canonical names, module repo identity, multiple-version identity, same-extension internal repos, `use_repo`, `inject_repo`, `override_repo`, `use_repo_rule`, isolated extensions; grounded in Bazel docs/source listed above. |
| Failure behavior | Bad extension, unknown repo rule, wrong symbol type, repo-rule failure, invalid repo spec, stale marker, missing generated target, invalid override; grounded in Bazel failure sources and current Slug stub fallbacks. |
| Materialization | Stale marker with changed `RepoSpec`, changed content with same marker, deleted file, corrupted file, changed mode, stale/broken symlink, local path missing, cache corruption; grounded in current Slug success-by-marker paths and Bazel marker docs. |
| Daemon isolation | Two workspaces with different module graphs, same extension id, same generated repo names, and different lockfiles/facts do not share bzlmod state; grounded in Bazel output-base scoping and current Slug global registry bug shape. |
| Real-world | rules_cc, rules_python, rules_rs/rules_rust, bazel_features, bounded zeromatter target; grounded in Plans 02, 36, and 57. |
| Performance guardrail | Named experiment `plan61-warm-daemon-noop` compares trace counters before/after a no-op warm invocation. |

## Out Of Scope

- Remote execution performance.
- Full sandbox strategy.
- Bazel 8 lockfile compatibility.
- WORKSPACE support.
- New language-rule parity outside failures directly exposed by bzlmod.
- Exact visible-lockfile writes from ordinary Slug builds before a
  mode-aware explicit lockfile write path exists.

## Source Of Truth And Validation Matrix

### Citation Index

Use these concrete anchors when implementing or updating line numbers. The local
Bazel checkout is a Bazel-9-era checkout; before implementation, pin the exact
Bazel 9 release tag and refresh line numbers.

| Topic | Bazel source/docs anchors | Current Slug / plan anchors |
|---|---|---|
| Current official Bazel 9 docs | `https://bazel.build/versions/9.0.0/external/lockfile` (checked 2026-05-18; page last updated 2026-05-07); `https://bazel.build/versions/9.0.0/external/module`; `https://bazel.build/versions/9.0.0/external/extension`; `https://bazel.build/versions/9.0.0/external/overview` | Use these only as versioned behavioral docs; still validate exact behavior with pinned source and local Bazel experiments. |
| Module parsing inputs | `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileValue.java`; `ModuleFileFunction.java`; `/var/mnt/dev/bazel/site/en/external/module.md` | `app/slug_common/src/legacy_configs/cells.rs`; `app/slug_bzlmod/src/parser.rs` |
| Resolution, MVS, yanked versions | `BazelModuleResolutionFunction.java`; `Selection.java`; `BazelDepGraphValue.java`; `/var/mnt/dev/bazel/site/en/external/module.md`; `/var/mnt/dev/bazel/site/en/external/lockfile.md` | `app/slug_bzlmod/src/resolution.rs`; legacy `cells.rs` |
| Canonical/apparent repo identity | `ModuleKey.java`; `BazelDepGraphFunction.java`; `ModuleExtensionId.java`; `ModuleExtensionUsage.java`; `ModuleExtensionRepoMappingEntriesFunction.java`; `/var/mnt/dev/bazel/site/en/external/overview.md`; `/var/mnt/dev/bazel/site/en/external/extension.md` | `app/slug_bzlmod/src/repo_mapping.rs`; `pending_repo_cells.rs`; `extension_execution_dice.rs` |
| Extension aggregation/replay | `SingleExtensionUsagesFunction.java`; `SingleExtensionUsagesValue.java`; `RegularRunnableExtension.java`; `SingleExtensionEvalFunction.java`; `SingleExtensionValue.java` | `app/slug_bzlmod/src/extensions.rs`; `extension_execution_dice.rs`; Plan 10 |
| Recorded inputs | `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/rules/repository/RepoRecordedInput.java`; `StarlarkBaseExternalContext.java` | `app/slug_bzlmod/src/lockfile.rs`; `app/slug_interpreter_for_build/src/module_ctx`; `repository_ctx.rs` |
| Repo materialization and markers | `DigestWriter.java`; `RepositoryFetchFunction.java`; `RepoDefinitionFunction.java`; `RepositoryDirectoryValue.java` | `app/slug_bzlmod/src/repository_execution.rs`; `repository_executor.rs`; `app/slug_external_cells/src/extension_repo.rs`; Plan 38 |
| Lockfile modes and writes | `RepositoryOptions.java`; `BazelLockFileModule.java`; `BazelLockFileFunction.java`; `SingleExtensionEvalFunction.java` | Plan 57 lockfile safety policy; `app/slug_bzlmod/src/lockfile.rs`; client `--lockfile_mode` parsing |
| Facts | `ModuleExtensionContext.java`; `ModuleExtensionMetadata.java`; `Facts.java`; `FactsAdapter.java`; `SingleExtensionEvalFunction.java`; `/var/mnt/dev/bazel/site/en/external/extension.md` | Plan 57 facts reuse; `module_ctx/methods.rs`; `module_extension_executor_impl.rs`; `extension_execution_dice.rs` |
| Toolchain/platform registration | `ModuleFileGlobals.java`; `RegisteredToolchainsFunction.java`; `RegisteredExecutionPlatformsFunction.java` | `app/slug_bzlmod/src/lib.rs`; legacy `cells.rs` |
| Slug-specific cell graph/layout | Bazel grounding is only canonical repository graph semantics, not Slug cells | `app/slug_core/src/cells.rs`; `app/slug_common/src/dice/cells.rs`; legacy `cells.rs`; Plans 36/38/61 |

| Structural claim | Bazel source/docs | Slug source/plans | Required experiment |
|---|---|---|---|
| Slug implements Bazel-9 bzlmod-only external dependency policy with no WORKSPACE fallback. | Project policy is `AGENTS.md`; pinned Bazel 9 docs/source and a local experiment must classify exact WORKSPACE behavior before claiming wording-level Bazel parity. | `AGENTS.md`. | Fixture with only `WORKSPACE`; record exact Bazel 9 and Slug behavior, including whether Bazel ignores, warns, or errors. |
| Module parsing and resolution are graph-owned. | `ModuleFileValue`, `ModuleFileFunction`, `BazelModuleResolutionFunction`, `Selection`. | Legacy `cells.rs` pre-DICE parse/resolution. | `plan61-module-input-dice-diff`. |
| Lockfile replay uses bzl digest, usages digest, recorded inputs, generated repo specs, metadata/facts, hidden/workspace lockfile lookup, and lockfile modes. | Bazel 9 lockfile docs; `BazelLockFileFunction`, `BazelLockFileValue.KEY` / `HIDDEN_KEY`, `SingleExtensionEvalFunction`, `LockFileModuleExtension`, `RegularRunnableExtension`, `RepoRecordedInput`. | Plan 57; current `lockfile.rs` digest-only validation. | `plan61-lockfile-modes` and recorded-input mutation fixtures. |
| Canonical identity and repo mapping are producer-owned. | External repo docs, `ModuleKey`, `ModuleExtensionId`, `BazelDepGraphFunction`, `ModuleExtensionUsage`, `ModuleExtensionRepoMappingEntriesFunction`. | Current Slug string/fallback alias paths. | Printed canonical label fixtures for module, extension, innate, isolated, inject/override cases. |
| Repo materialization is not bare marker trust. | Bazel `DigestWriter`, `RepositoryFetchFunction`, `RepoRecordedInput`; note Bazel marker semantics rather than output-manifest parity. | Plan 38 and current `.slug_repo_complete` / stub paths. | Stale marker and output mutation fixtures. |
| Facts are JSON-like metadata, not normal replay invalidators. | `ModuleExtensionContext`, `ModuleExtensionMetadata`, `Facts`, `SingleExtensionEvalFunction`. | Plan 57 and current metadata drop in `extension_execution_dice.rs`. | Facts reuse fixture plus `--lockfile_mode=error` facts validation fixture. |

A Plan 61 implementation claim is accepted only when the row has pinned Bazel
source/docs, local Bazel output, local Slug output, and an explicit decision for
any source/doc/policy mismatch. "Slug already works on this fixture" is not
evidence unless the matching Bazel 9 source or executable behavior is cited.
