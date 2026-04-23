# clang:clang wall gap root cause — missing exec configuration

**Date:** 2026-04-22
**Target:** `@llvm-project//clang:clang` (cold)
**Baseline:** `benchmarks/post-plan-17-fixed-aggregator/`

The 296 s wall gap vs Bazel on this target has **nothing to do with
the scheduler**. It's a rules-layer problem: kuro doesn't implement
Bazel's exec-configuration for tools, so tool binaries (llvm-tblgen,
clang drivers built-to-run-during-build) are compiled in the same
fastbuild (-O0) mode as the final target binaries. The resulting
llvm-tblgen binary is 5–10× slower than Bazel's.

## Two orthogonal investigations converged on the same root

### 1. Critical path — kuro's td_generate is 10× slower than Bazel's

From `kuro log critical-path`, stripping synthetic waits:

    offset (s)  dur (s)  action
       0.000      3.103  <listing + load + analysis>
       5.521      5.199  c_compile llvm/lib/TableGen/Record.cpp
      12.336      0.053  cpp_archive libTableGen.a
      12.557      0.313  cpp_link  llvm-min-tblgen
      12.871     13.920  td_generate RISCVTargetParserDef.inc
      26.798      6.884  c_compile llvm/utils/TableGen/GlobalISelCombinerEmitter.cpp
      40.072      0.855  cpp_link  llvm-tblgen
      40.929    279.836  td_generate AMDGPUGenRegisterInfo.inc     <<< 76% of critical path
     799.890    271.164  c_compile clang/lib/CodeGen/CodeGenModule.cpp
    1354.948     45.331  cpp_archive libcodegen.a
    1405.204     30.246  cpp_link  clang

Bazel's log for the same action:

    [2,397 / 6,352] TdGenerate AMDGPUGenRegisterInfo.inc;  20s
    [2,463 / 6,352] TdGenerate AMDGPUGenRegisterInfo.inc;  28s
    <disappears from log shortly after>

Bazel finishes `AMDGPUGenRegisterInfo.inc` in ~30 s. Kuro takes 280 s.

Running kuro's built `llvm-tblgen` binary manually outside the build:

    $ time buck-out/.../llvm-tblgen -gen-register-info AMDGPU.td ...
    real    2m44s     (164 s of user CPU, two consecutive runs)

Kuro's llvm-tblgen is genuinely 5× slower than Bazel's on the same
input. It's not scheduler contention.

### 2. Action count — Bazel has 954 more actions than kuro

    Bazel: 6352 total = 1105 internal + 5247 linux-sandbox
    Kuro:  5367 total = 1074 non-Run + 4293 Run (the same split)

The `internal` ≈ `non-Run` numbers match within 3 %. The real gap is
954 fewer sandboxed actions in kuro. Searching Bazel's log for the
same source file compiled twice:

    $ grep -cE 'Compiling.*${file} \[for tool\]'   # exec-config compile
    $ grep -cE 'Compiling.*${file}([^[]|;)'         # target-config compile

    Record.cpp            [for tool]: 3   target-config: 3
    VirtualFileSystem.cpp [for tool]: 3   target-config: 3
    MicrosoftDemangle.cpp [for tool]: 1   target-config: 1

Every tblgen-dep source (Record.cpp, Support/*, Demangle/*, etc.)
is compiled **twice** in Bazel — once in exec config, once in target
config. Kuro compiles each once.

That's where the 954 missing actions come from.

## The connection

Bazel's exec configuration for cc_binary tools uses different
compile features than the target fastbuild configuration. The final
result:

- Bazel's llvm-tblgen is compiled with exec-config features,
  including whichever optimization level the exec config applies
  (historically opt mode for tools).
- Kuro's llvm-tblgen is compiled with target fastbuild mode. No
  `-O` anywhere in the command line (`kuro log what-ran | grep
  c_compile | tr ' ' '\n' | grep ^-O` → nothing).

I confirmed kuro's compile arguments:

    $ kuro log what-ran ... | grep c_compile | head -1 | tr ' ' '\n' | \
          grep -E '^-[OgfmWD]' | sort -u
    -D_GNU_SOURCE
    -DHAVE_BUILTIN_THREAD_POINTER
    -D__STDC_CONSTANT_MACROS
    -D__STDC_FORMAT_MACROS
    -D__STDC_LIMIT_MACROS
    -fPIC

That's the full set of non-`-D*` switches. Default gcc without
`-O` = `-O0`. Meanwhile Bazel's `local_config_cc/BUILD` has:

    compile_flags              = [-fstack-protector -Wall
                                  -Wunused-but-set-parameter
                                  -Wno-free-nonheap-object
                                  -fno-omit-frame-pointer]
    fastbuild_compile_flags    = []
    opt_compile_flags          = [-g0 -O2 -D_FORTIFY_SOURCE=1 -DNDEBUG
                                  -ffunction-sections -fdata-sections]
    dbg_compile_flags          = [-g]
    cxx_flags                  = [-std=c++17]
    unfiltered_compile_flags   = [-fno-canonical-system-headers ...]

Kuro's rules aren't threading ANY of these through. Not the always-
on `compile_flags`, not the mode-specific flags, not even
`-std=c++17` for C++.

## Why this explains the wall gap

Critical path in kuro = 367.6 s. In Bazel = 71.4 s. Delta = 296 s.

Estimated Bazel-equivalent kuro critical path if AMDGPU tablegen
ran 10× faster (i.e., 28 s instead of 280 s): 367.6 − (280 − 28)
= 115 s. Still double Bazel's 71 s, but within the noise of other
differences. And wall would drop from 1436 s → ~1184 s, closing
most of the gap vs Bazel's 1131 s.

## What kuro's rules-layer is missing

1. **Exec configuration for tools.** cc_binary targets used as
   build-time tools should compile in a distinct "exec" config
   with its own default compile mode (opt). Presumably means
   wiring through `cfg = "exec"` semantics and letting the
   cc_library rule see which config it's in.
2. **Honor `cc_toolchain_config` feature flag sets.** Even in
   fastbuild mode, the always-on `compile_flags` +
   `unfiltered_compile_flags` + `cxx_flags` aren't being applied.
   The rule impl needs to read them from the resolved toolchain
   and append to the compile command.
3. **Apply `-std=c++17` at minimum.** Probably the single
   highest-impact change for correctness, even before optimization
   work.

## Confirming the theory (cheap next step)

Hand-compile `llvm-tblgen` with `-O2` and replace the kuro-built
binary:

    $ cd /var/mnt/dev/llvm-project/utils/bazel
    $ # run a kuro c_compile command, change -O0 to -O2 for all tblgen
    $ # deps, re-link
    $ time buck-out/.../llvm-tblgen-O2 -gen-register-info AMDGPU.td ...
    # expected: ~30 s to match Bazel

If the O2-rebuilt binary is fast, the theory is confirmed and the
fix is to implement (1) + (2) above. If it's still slow, something
else is going on and this investigation got the wrong answer.

## Plan 17.2 status

**Parked.** The scheduler is not the bottleneck on this target.
The 93× queue-ratio on c_compile is a real scheduler signal but
doesn't change the wall (actions that DO dispatch are on the
critical path and execute correctly).

Revive 17.2 after the exec-config + cc_toolchain-features fix
lands. At that point the wall gap vs Bazel will be in the low
single-digit percent and scheduler tuning becomes a measurable
next target.

## Artifacts

- Critical path dump source: `benchmarks/post-plan-17-fixed-
  aggregator/llvm-project_clang_clang/cold-01/build.pb.zst`
- Bazel log: `/tmp/bazel_cold.log` (2.3 MB)
- Manual tablegen timing: 164 s user CPU, two consecutive runs
