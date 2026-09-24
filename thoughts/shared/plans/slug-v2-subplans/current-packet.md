# Current Slug V2 Work Packet

Packet: WP-7-100-m7a-selected-module-getenv-r1
Status: ACCEPTED 2026-09-24; M7A remains partial

## Outcome and compatibility

Admit `module_ctx.getenv(name, default = None)` for a selected module extension, including names omitted from `module_extension(environ = ...)`. The authentic rules_python 1.7.0 extension must pass its first undeclared `RULES_PYTHON_REPO_DEBUG` observation stop under the existing normalized PATH; a later independently typed stop is acceptable. This is a selected environment-observation slice, not Python toolchain or root-build success.

Exact Bazel 9.2 observable slice: string name, absent `None`, absent string default, present empty string, present value and undeclared names; a raw absent observation is distinct from a present value equal to its default. `--repo_env` changes re-evaluate the selected extension, including A/B/A. Slug-native: typed structural DICE identity, bounded selected-owner replay/errors, and Rust Host snapshot observation rather than Bazel Skyframe or HotSpot bytes. Unsupported/deferred: nonselected extension execution, lockfile recorded-input replay, broad environment enumeration, arbitrary repository effects, exact diagnostic wording, Python toolchain analysis and root bootstrap. Existing declaration-restricted `module_ctx.os.environ` remains unchanged.

## Source, owner and decision

Pinned Bazel 9.2 commit `8220c619` has `StarlarkBaseExternalContext.java:1512-1542` recording a `RepoRecordedInput.EnvVar(name)` before applying a `None` or string default; `RepoRecordedInput.java:703-776` and `RepoEnvironmentFunction.java:49-61` distinguish absent, empty and present cells. `RegularRunnableExtension.java:182-186` and `ModuleExtensionContext.java:90-94` pre-record declared `environ` independently. The Bazel `bazel_lockfile_test.py:2599-2717` uses an undeclared lazy name; fresh Bazel 9.2 scratch oracles `/tmp/slug-wp800-getenv-a-MWm5ZF` and `/tmp/slug-wp800-getenv-b-Wk6LBM` proved returned absent/default, empty, one/two/one and the raw-host-lookup control in two output bases. Raw absent versus present-default-equal identity is proven by the pinned recorded-input path and a Slug structural test, not target output. Convert the value-behavior rows to a self-contained fixture and independently replay it. Authenticated rules_python 1.7.0 `python/private/python.bzl:1374-1386` declares only `RULES_PYTHON_BZLMOD_DEBUG`; `repo_utils.bzl:314-316` asks for `RULES_PYTHON_REPO_DEBUG` and verbosity through `getenv` if available.

`HostSelectedExtensionOwnerPureKey` in `module_extension.rs` owns selected evaluation. Its existing `observe_selected_host` preflights declared names through `RepositoryEnvironmentCellKey` and an immutable `RepositoryHostInputTransaction`; the selected invocation loop owns dynamic Need/replay. Add an attempt-local selected `getenv` scratch bridge: a first unprepared name records a pending name and aborts the Starlark pass; the async owner requests missing frontier names through existing `SourcePreparationNeeds::environment` or verifies a per-name DICE cell against the request snapshot, stores the raw optional value and replays. No evaluator code reads ambient process environment. Guards are dropped before any DICE compute. Invalid/unavailable cells fail closed, including authorized absence; Need/interrupted passes publish neither call records nor events. Preserve previous error/observation paths and revocation after request changes. `docs/developers/dice.md` governs dependency recording, equality cutoff and lock ownership; Core already injects/revokes these cells.

Retain a deterministic name/raw-value recorded-environment projection on Complete owner results and post-host error/observation terminals so equal generated repository calls or error strings cannot erase changed reads. It contains declared names and only dynamically *read* names, not all names prepared on aborted passes; absent and present-default-equal remain distinct. Its `Debug` and terminal formatting must redact raw values while preserving structural equality; test that no sentinel value appears in formatted results/errors. Keep this projection separate from OS/arch selection flags and REAPI/Bazel digest domains. Retained values use existing Arc/CompactString storage; attempt-local scratch uses existing small collections per Stage 9 row 96, with no global cache or new DICE key. Retained data has no evaluator-heap borrow and is released with its owner value. DICE equality/dependency revalidation owns invalidation and restoration; there is no command-side repair.

`module_ctx.getenv` is present in the shared Starlark method table but fails closed outside selected evaluation. `module_ctx.os.environ` remains declaration-restricted. `getenv` accepts a string name and optional string-or-None default, returning raw value when present and default otherwise. At most 16 distinct names requested through `getenv` are supported per selected evaluation (Slug-native bound), whether also declared or not; repeated reads of a name do not consume another slot. This covers both rules_python logger names and the source-selected case, without claiming unbounded Bazel parity.

## Evidence and validation

Make one small Stage 1 fixture with `fixture.toml` provenance and deterministic generated BUILD, for selected undeclared getenv returned absent/default, empty, one/two/one and Bazel's untracked `os.environ` control. Compare exact target names/values and message shape where appropriate; the control documents a deferred Slug boundary rather than an admitted negative behavior. Fresh Bazel 9.2 generation and independent replay must agree. The fixture does not claim to distinguish raw absent from present-default-equal, which has pinned-source and focused Rust structural evidence. Reuse accepted 9.1.1 lockfile fixture only as historical context; it is not Bazel 9.2 evidence for this exact slice. Keep fixture at most five small files and below the orchestration fixture-growth checkpoint; copy no upstream source, archive or credential. Launch oracle harness with synthetic probe variables unset. Bazel's upstream lockfile replay assertions are skipped because that lifecycle is deferred here.

Focused Rust tests must prove selected cold Need/replay; absent/empty/present and default type; undeclared dynamic versus declared-only `os.environ`; raw absent versus present-default-equal structural identity; A/B/A request revisions with unchanged repository calls and identical post-host error; no events/calls from interrupted passes; nonselected fail-closed; 16 distinct names accepted, 17th rejected, and repeated reads not consuming another slot or Need. Verify debug redaction. Reuse selected-owner declared Host, OS-environ, forbidden-ABI and factored-error regressions. Compile Loading/Bzlmod/Core direct dependents with pinned-toolchain JSON `--no-run`, run exact-selector preflight and focused tests serially, rebuild `slug_cli_v2` before authentic smoke, and clean stale `slugd` before and after. The smoke uses accepted normalized PATH with HOME/NETRC unset. Run format, diff, archive and plan-status checks; require independent DESIGN and FINAL review. No broad suite, remote transport gate or M7A close is selected. BuildBuddy is available through normal Bazel RC discovery if a later remote check needs it; never inspect or print `~/.bazelrc`, netrc or auth material.

## Scope and stops

Base clean main `f0bddcce3`. Allowlist: this manifest, canonical Current paragraph and Stage 5 owner contract only for accepted invariants; `app/slug_loading_v2/src/module_extension.rs` for selected bridge, identity and focused tests; `tests/v2_oracle/fixtures/module-extension-dynamic-getenv/` for the new small fixture. The large `module_extension.rs` remains the cohesive selected-owner/evaluator owner; add no second environment registry. A split is a review trigger only if implementation crosses another subsystem or mixes command/transport concerns. Do not change repository rule semantics, Core cell keys, lockfile, remote transport, NativeLink/BuildBuddy setup, credentials or generated rules_python source. REPLAN if dynamic reads cannot be tracked through existing per-name DICE cell/Need bridge, equality requires a new cross-crate owner, or the authentic next stop requires an unmodeled repository effect merely to pass this getenv gate.

## Immediate predecessor

WP799 at `f0bddcce3` admitted ordinary private repository-rule string/label defaults, passed an eight-row Bazel 9.2 fixture, six focused Rust tests and independent reviews, and moved authentic normalized-PATH smoke past `_rule_name` to undeclared `RULES_PYTHON_REPO_DEBUG` observation. M7A remains partial.

## Acceptance receipt

Independent DESIGN and FINAL reviews returned ACCEPT. The five-file Bazel
9.2 fixture passed seven rows during generation and independent fresh-root
replay (5.90 s and 5.92 s). Pinned source establishes raw recorded-input
identity where target-output rows cannot. The selected-owner bridge uses the
existing per-name DICE cell and Need/replay route, retains redacted structural
raw observations, and leaves `os.environ` declaration-restricted. A review
correction discarded repository calls from a pass interrupted by failed
dynamic Host verification; the focused test invokes a repository rule before
`getenv` to discriminate that case. Pinned-toolchain JSON `--no-run` compiled
Loading (latest 10.72 s), Bzlmod and Core (57.46 s before the local
interrupted-pass correction); exact preflight selected eight tests and all
eight passed after the correction. A fresh CLI build passed (5.69 s), and
the normalized-PATH authentic smoke passed the old debug-variable stop then
stopped at the separate `py_toolchain_suite.bzl` / generated
`pythons_hub//:BUILD.bazel` LoadLabel boundary (exit 2). No stale `slugd`
was present before or after. Format, diff, archive and plan-status checks
passed. The packet took about 24 minutes from draft through final review;
M7A and M8 remain open.
