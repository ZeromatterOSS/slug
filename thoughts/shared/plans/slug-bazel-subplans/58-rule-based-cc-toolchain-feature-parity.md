# Plan 58: Rule-based C++ toolchain feature parity

## Goal

Make Slug evaluate Bazel/rules_cc C++ toolchain semantics from the declared
toolchain configuration instead of from native approximations or
target-specific LLVM/Rust workarounds.

The original visible target was `../zeromatter-slug` `//sdk:sdk_contents`.
The current active repro is narrower and better:
`../zeromatter-kuro` `@rules_rust//util/process_wrapper:process_wrapper`
with `--target-platforms=//bazel/platforms:linux-gnu-host`.

## Source Of Truth

- Bazel 9 C++ implementation:
  `src/main/java/com/google/devtools/build/lib/rules/cpp/`
- Bazel toolchain config APIs:
  `CcModule.java` and `CcToolchainFeatures.java`
- rules_cc toolchain conversion and linking logic:
  `@rules_cc//cc/toolchains:args.bzl`,
  `@rules_cc//cc/toolchains/impl:legacy_converter.bzl`,
  `@rules_cc//cc/toolchains/impl:toolchain_config_info.bzl`,
  `@rules_cc//cc/private/rules_impl/cc_shared_library.bzl`
- Zeromatter LLVM toolchain/runtime declarations:
  `@llvm//toolchain/args:BUILD.bazel`,
  `@llvm//toolchain/args/linux:BUILD.bazel`,
  `@llvm//runtimes/...`

## Current State

Commit `f40183ef` consolidated the latest completed slices. Slug now gets far
enough that the final Rust `process_wrapper` link includes Bazel-shaped
toolchain search-directory args and native static C++ runtime flags:
`-Lnative=.../libcxx.static_`, `-lstatic=libcxx.static`,
`-lstatic=libcxxabi.static`, and `-lstatic=libunwind.static`.

The active blocker is no longer missing `_linker_files`, missing runtime static
libs, or Rust link-arg ordering. Bazel uses the same broad Rust link-arg order.
The decisive mismatch is earlier: Bazel's glibc stub shared libraries are
non-empty, while Slug's corresponding actions produce shared libraries with
version metadata but no libc symbols.

Concrete mismatch:

- Bazel action for
  `@llvm//runtimes/glibc:libc_shared_library_/libc_shared_library` links
  `liblibc.a` under `-Wl,-whole-archive`.
- Slug's corresponding action emits approximately:
  `clang++ -shared -o libc.so.6 -Wl,--version-script=... -Wl,-soname,... -z notext`
  without the static libc archive input.

Likely boundary: rules_cc `cc_shared_library` `_filter_inputs` /
`GraphNodeInfo` / `CcInfo.linking_context.linker_inputs` provider propagation.
Do not special-case glibc, linker-script names, or process-wrapper.

## Next Implementer Pointers

Start from the smallest failing C++ runtime action, not the Rust final link:

```sh
cd /var/mnt/dev/zeromatter-kuro

bazel aquery '@llvm//runtimes/glibc:libc_shared_library_/libc_shared_library' \
  --platforms=//bazel/platforms:linux-gnu-host \
  --lockfile_mode=off --output=text \
  > /tmp/plan58-bazel-glibc-libc-aquery.txt

/var/mnt/dev/slug/target/debug/slug \
  --isolation-dir plan58-glibc-libc-next \
  build -M none @llvm//runtimes/glibc:libc_shared_library_/libc_shared_library \
  --target-platforms=//bazel/platforms:linux-gnu-host \
  > /tmp/plan58-glibc-libc-next.log 2>&1
```

If Bazel 9 still rejects the checked-in `MODULE.bazel.lock`, temporarily move it
aside under a shell trap or keep using `--lockfile_mode=off`; previous probes
needed that because Bazel 9.0.1 reported `Illegal base64 character 2d`.

Inspect these Slug boundaries first:

- `app/slug_build_api/src/interpreter/rule_defs/cc_common/providers.rs`:
  `LibraryToLinkGen`, `LinkerInputStubGen`, `CcInfoInstanceGen`, and any
  private attrs expected by rules_cc provider code.
- `app/slug_build_api/src/interpreter/rule_defs/cc_common/actions.rs`:
  `create_library_to_link`, `create_linker_input`,
  `create_linking_context`, `create_linking_context_from_compilation_outputs`,
  `merge_linking_contexts`, and `merge_cc_infos`.
- `app/slug_build_api/src/interpreter/rule_defs/context.rs`:
  `CcInfoNativeShim` and empty linking-context fallbacks.
- rules_cc `cc_shared_library.bzl`:
  `_filter_inputs` and `GraphNodeInfo` are the most likely Bazel-side path that
  decides whether `liblibc.a` is selected as a whole-archive input.

Useful temporary diagnostic, to remove before finalizing: print or expose
`linker_inputs.to_list()` immediately before rules_cc `cc_shared_library`
filtering and immediately inside Slug `cc_common.link`. Compare static library
artifact, pic/static variant, owner label, `alwayslink` /
`is_whole_archive`, and additional inputs.

Decision tree:

- If `cc_common.link` never receives `liblibc.a`, fix provider propagation or
  `GraphNodeInfo` / `CcInfo` shape.
- If it receives `liblibc.a` without whole-archive semantics, fix
  `LibraryToLink` field shape, attr names, hashing/equality, or merge behavior.
- If it receives the correct `LibraryToLink` and still omits argv, only then
  debug final C++ link argv conversion.

## Important Artifacts

- Kuro process-wrapper timeout and action trace:
  `/tmp/plan58-kuro-process-wrapper-aliasargs-20260513b.log`
  and `/tmp/plan58-kuro-process-wrapper-aliasargs-20260513b.whatran.txt`.
- Bazel process-wrapper dependency aquery:
  `/tmp/plan58-bazel-process-wrapper-deps-aquery.txt`.
  It proves Bazel also builds compiler-rt, libcxx/libcxxabi/libunwind,
  glibc stubs, `glibc_c_nonshared`, and `libc_nonshared.a` for this helper.
- Bazel glibc runtime aqueries:
  `/tmp/plan58-bazel-glibc-libld-aquery.txt`,
  `/tmp/plan58-bazel-glibc-dir-aquery.txt`.
  They prove Bazel splits target config and runtime-stage config for glibc
  shared libraries and the directory copy target.
- Direct Slug glibc directory success after fixes:
  `/tmp/plan58-glibc-dir-pathbasename-20260513-085546.log`.
- Latest useful Kuro narrow repro after runtime static dep fixes:
  `/tmp/plan58-kuro-process-wrapper-static-dep-cfg-20260513-113908.log`.

## Historical Breadcrumbs

These are not full reports; they are the old probes most likely to be useful if
a future implementer needs to verify that a path was already tried.

- `plan58-envsets-sdk-20260511-124517`: first durable feature-env slice;
  `//sdk:sdk_contents` built successfully.
- `plan58-outputmode30-sdk-20260511-133024`: output mode normalization made the
  SDK manifest mode/file-list comparison pass; remaining SDK hash drift was real
  link/content drift, not permissions.
- `plan58-modernflagsets-sdk-20260511-152514` and
  `plan58-modernprefer-sdk-20260511-160348`: preserving flag sets was not enough
  until public `cc_common.configure_features` used the selected toolchain's
  feature graph.
- `plan58-realcc-sdk-20260511-232124` and
  `plan58-shimconfig-sdk-20260512-044916`: analyzing the real `cc_toolchain` or
  full generated `cc_toolchain_config` target reopened rules_rust/runtime deps
  and stalled. This is the key evidence for metadata-only extraction plus the
  NativeShim cycle break.
- `plan58-modulemaptype-sdk-20260511-233921`,
  `plan58-formatliteral-sdk-20260511-234347`,
  `plan58-outputpathargs-sdk-20260511-234846`,
  `plan58-modulemapextract-sdk-20260511-232417`,
  `plan58-modulemapdepset-sdk-20260511-232534`,
  `plan58-mapexpander-sdk-20260511-233224`, and
  `plan58-requestedfeatures-sdk-20260511-233626`: module-map and
  feature-configuration shape failures. These justify the provider-shape,
  action-name normalization, two-argument `map_each`, and "do not re-enable
  unsupported defaults" fixes.
- `plan58-unconfiguredmetadata-sdk-20260512-055838`: unconfigured metadata
  extraction restored a healthy SDK build baseline after the broader analysis
  attempts stalled.
- `plan58-compositeselect-sdk-20260512-000317`: showed that composite
  select/concat metadata traversal was required for path-backed toolchain args.
- `plan58-linkdata-*`, `plan58-staticfile-*`, `plan58-allfilesonly-*`,
  `plan58-pathhash-*`, `plan58-carrier-*`, and
  `plan58-carrierdepset-*`: explored ways to expose toolchain data files.
  The useful lesson is to preserve provider/file shape generically; ad hoc
  frozen file/hash/carrier experiments either crashed or moved the failure to a
  more precise provider-shape boundary.
- `plan58-reporule-sdk-20260512-113546` and
  `plan58-scopedalias-sdk-20260512-115106`: bzlmod apparent/canonical repo
  alias handling. The durable fix was scoped `use_repo` alias registration and
  lookup, not a one-off `bazel_lib` alias.
- `plan58-kuro-process-wrapper-aliasargs-20260513b`: showed Slug was now
  building the same large runtime class Bazel builds for process-wrapper; do not
  "fix" this by trimming `_linker_files`.
- `plan58-cquery-trans-fix2-20260512` and
  `plan58-glibc-dir-fix-20260512-223120`: proved anonymous Bazel transitions
  must carry output build settings for `with_cfg` runtime-stage edges.
- `plan58-glibc-dir-archiveargs-20260512-224143`: after transition repair,
  exposed static archive argv duplication (`ar rcs <out> rcsD <out>`), which is
  now fixed by preferring feature-expanded archive mode/output.
- `plan58-glibc-dir-linkloc-*`, `plan58-glibc-dir-source-target-loc-*`,
  `plan58-glibc-dir-artifact-loc-*`, `plan58-glibc-dir-explicit-pool-*`, and
  `plan58-glibc-dir-debug-loc-*`: narrowed literal `$(location :all.map)` to
  `ctx.expand_location` explicit-target/provider shape. The durable fix is the
  single-output basename fallback in the Starlark facade.
- `plan58-glibc-dir-pathbasename-20260513-085546`: direct glibc search-directory
  target built successfully after location expansion fixes.
- `plan58-kuro-process-wrapper-pathbasename-20260513-085629`: process-wrapper no
  longer timed out, but final Rust link failed with undefined glibc symbols.
- `plan58-kuro-process-wrapper-overlay-runtime-20260513-092138` and
  `plan58-kuro-process-wrapper-static-dep-cfg-20260513-113908`: runtime-lib
  metadata work progressed from missing native static flags to real
  `libcxx`/`libcxxabi`/`libunwind` archive paths in Rust params.
- `plan58-kuro-process-wrapper-picobjs-20260513` and
  `plan58-kuro-process-wrapper-namedlibs-20260513`: final `cc_common.link`
  argv-conversion experiments did not change glibc shared-library actions and
  were reverted. This is the main evidence to inspect upstream linking providers
  before editing argv conversion again.

## Completed Work

These are considered durable Plan 58 progress, not active suspects unless a new
Bazel comparison contradicts them.

- Preserved feature `env_sets` and expanded action environment variables.
- Preserved and expanded rule-based legacy flag sets/action configs well enough
  for LLVM path-backed args to reach Rust/C++ actions.
- Changed public `cc_common.configure_features` to use the selected
  toolchain-owned feature graph instead of a name-only configuration.
- Kept the C++ toolchain NativeShim cycle break, but enriched it with metadata
  extracted from toolchain/config attrs rather than by analyzing full toolchain
  implementation targets.
- Implemented metadata traversal for unconfigured select/concat attrs and
  normalized Bzlmod canonical/apparent repo names for toolchain metadata.
- Added module-map and depset provider-shape fixes needed by rules_cc.
- Added `Args.add_all` / `add_joined` two-argument `map_each` support for
  rules_cc module-map generation.
- Normalized C++ action names for Slug action categories without changing Bazel
  feature action matching.
- Exposed C++ toolchain compiler/linker/runtime data through `_compiler_files`,
  `_linker_files`, static runtime, and dynamic runtime NativeShim fields.
- Expanded filegroups, aliases, scoped `use_repo` aliases, and generated static
  runtime archive outputs for toolchain data labels.
- Implemented anonymous Bazel transition output application for `with_cfg.bzl`
  runtime-stage rules, fixing the glibc directory/shared-library self-cycle.
- Fixed static archive argv duplication: use feature-expanded `rcsD <output>`
  when present instead of prefixing hardcoded `ar rcs <output>`.
- Fixed `ctx.expand_location(..., targets = ...)` for the rules_cc
  `additional_linker_inputs` shape where a select-file label is represented by
  a provider target with one output whose basename matches the queried label.
- Added bzlmod cache correctness fixes: incomplete source dirs are removed
  before refetch, incomplete repo rule working dirs are removed unless
  `.slug_repo_complete` matches the spec hash, and source fetching continues
  through later modules while remembering the first registry error.
- Normalized local action output modes to Bazel-like read-only executable modes
  for SDK materialization parity.

## Tried And Rejected

These attempts either failed directly or were shown by Bazel comparison to be
the wrong abstraction boundary.

- Do not trim `glibc`, libcxx/libunwind, compiler-rt, resource-dir, or CRT data
  from `_linker_files`/`all_files`. Bazel builds the same runtime class for the
  narrow process-wrapper helper.
- Do not special-case glibc, `libc_nonshared.a`, `all.map`, linker scripts, or
  process-wrapper.
- Do not analyze the full selected `cc_toolchain` implementation or the full
  generated `cc_toolchain_config` target just to read features. That traverses
  `DefaultInfo(files = toolchain_config.files.values())`, reopens runtime deps,
  and previously stalled on rules_rust toolchain cycles.
- Do not flatten every `select()` branch when extracting metadata. That made
  mutually exclusive LLVM feature branches unconditional and triggered
  unsupported `module_maps`.
- Do not re-enable all default-enabled features inside the rules_cc-facing
  `_toolchain_features.configure_features(requested_features=...)` path.
  rules_cc has already filtered unsupported defaults.
- Do not treat final Rust link ordering as the current blocker. Bazel's aquery
  showed similar link-arg ordering; the missing libc symbols come from empty
  glibc stub shared libraries.
- Do not keep patching final `cc_common.link` argv conversion for PIC object
  fallbacks, `alwayslink`, or named library providers until provider inputs are
  proven correct. The `plan58-kuro-process-wrapper-picobjs-20260513` and
  `plan58-kuro-process-wrapper-namedlibs-20260513` experiments did not change
  the glibc shared-library actions and were reverted.
- Do not assume `ctx.expand_location` failures are in `cc_common.link`; rules_cc
  often expands user flags in Starlark before calling link APIs.

## Methodology That Worked

- Prefer narrow runtime targets over full SDK builds while debugging provider
  shape. Direct `llvm//runtimes/glibc:glibc_library_search_directory` exposed
  the transition cycle and later confirmed it was fixed.
- Use Bazel aquery first, then patch Slug. Several plausible Slug-side theories
  were wrong until Bazel action shape was inspected.
- Keep every Slug run in a named `--isolation-dir` and save logs under
  `/tmp/plan58-...`.
- Use `slug --isolation-dir <dir> log what-ran` after a timeout to distinguish
  "not building enough data" from "building the correct but large runtime cone".
- Compare configured hashes with `cquery` when a cycle or duplicate runtime work
  is suspected. The key distinction for glibc was target configuration versus
  runtime-stage `ST` configuration.
- When a final action has the wrong argv, inspect upstream provider data before
  editing argv fallback logic. The current blocker is a provider/aspect
  propagation problem until proven otherwise.
- Keep temporary diagnostics narrowly gated and remove them before verification.
  Tracing at `debug`/`warn` is acceptable when it describes durable cache or
  execution behavior; ad hoc provider dumps are not.

## Common Pattern

Most successful fixes have followed the same pattern:

1. Use Bazel 9 to prove the semantic shape.
2. Identify the Slug subsystem that loses that shape.
3. Preserve the generic provider/configuration/metadata shape.
4. Reject target-label patches even when they make the immediate repro move.

Most failed attempts patched too late in the pipeline: final Rust flags, final
C++ argv conversion, or specific runtime labels. The durable fixes happened
earlier: feature graph extraction, configured transition application, provider
shape, `ctx.expand_location` semantics, and bzlmod/cache identity.

## Useful Commands

Focused Kuro process-wrapper repro:

```sh
/var/mnt/dev/slug/target/debug/slug \
  --isolation-dir plan58-kuro-process-wrapper-next \
  build -M none rules_rust//util/process_wrapper:process_wrapper \
  --target-platforms=//bazel/platforms:linux-gnu-host \
  > /tmp/plan58-kuro-process-wrapper-next.log 2>&1
```

After a timeout:

```sh
/var/mnt/dev/slug/target/debug/slug \
  --isolation-dir plan58-kuro-process-wrapper-next \
  log what-ran > /tmp/plan58-kuro-process-wrapper-next.whatran.txt
```

Configuration split check:

```sh
/var/mnt/dev/slug/target/debug/slug \
  --isolation-dir plan58-cquery-trans-next \
  cquery 'deps(llvm//runtimes/glibc:glibc_library_search_directory)' \
  --target-platforms=//bazel/platforms:linux-gnu-host --output label_kind |
  rg 'libld_shared_library|glibc_library_search_directory|libm_shared_library|libc_shared_library'
```

Bazel process-wrapper comparison:

```sh
cd /var/mnt/dev/zeromatter-kuro
bazel aquery 'deps(@rules_rust//util/process_wrapper:process_wrapper)' \
  --platforms=//bazel/platforms:linux-gnu-host \
  --lockfile_mode=off --output=text \
  > /tmp/plan58-bazel-process-wrapper-deps-aquery-next.txt
```

## Tests To Add Or Keep

- Synthetic rule-based C++ toolchain where `cc_args(data, format)` expands to a
  generated file/directory path and that generated target is an action input.
- Feature-condition coverage for `requires_*`, `requires_any_of`,
  `with_features`, and unsupported features.
- `iterate_over` coverage for list-valued link variables.
- Action-config tool-selection coverage proving configured tools and runfiles
  are used.
- Linking-context/provider-shape coverage for a `cc_shared_library`-like rule
  that selects a dependency static archive through `GraphNodeInfo` and passes it
  to `cc_common.link` as whole archive.
- Path-mapping coverage that command-line artifact strings are Bazel-shaped
  while materialized files remain in Slug's output tree.
- Materialization coverage for Bazel-compatible output executable/read-only
  modes.

## Acceptance Criteria

- `cc_common.get_memory_inefficient_command_line` is driven by configured
  toolchain features/action configs, not SDK-specific LLVM hardcoding.
- `cc_args(data = ...)` artifacts are built and included as inputs to actions
  whose command lines reference them.
- LLVM musl/glibc final Rust links include Bazel-equivalent `resource_dir`, CRT
  search, libc search, libc++/libc++abi/libunwind search, and runtime archive
  args.
- glibc stub shared-library actions receive the same static/whole-archive
  linker inputs Bazel selects.
- Slug command-line artifact strings use Bazel exec paths where Bazel embeds
  them in outputs.
- `../zeromatter-slug` `//sdk:sdk_contents` builds successfully with Slug and
  its output tree is byte-identical and mode-identical to Bazel.
- No SDK-, glibc-, or process-wrapper-specific link-flag/path-remap shims are
  introduced.

## Risks

- Feature expansion order is observable. Keep Bazel/rules_cc action-config and
  feature order; do not sort flag sets except where Bazel does.
- Starlark provider values can contain depsets, files, private providers, and
  nested structs. Prefer preserving typed data once near provider construction
  instead of repeatedly reflecting through Starlark during action registration.
- Logical path mapping must not change physical materialization locations or CAS
  keys. It should affect command-line strings, not artifact identity.
- Output mode normalization can disturb incremental cleanup if files become
  read-only too early. Apply it at action completion/materialization boundaries.
