# Current Slug V2 Packet

Packet: `WP-4-5-7A-bazel-universal-builtins-environment`

Milestone: M7A command/ruleset bootstrap closure, with shared Stage 4/5/core
Starlark-host policy.

Result: create one low-level Rust owner for Bazel 9.2's exact 30-name universal
environment, backed by process-stable frozen values, and compose every active
BUILD, `.bzl`, MODULE and REPO evaluator from it. Enable the vendored
starlark-rust `SetType` everywhere, remove REPO's stale always-disabled set
shim, exclude evaluator-only `chr`/`ord`, and retain context-specific overlays.
Do not retry the complete rules_cc helper in this packet.

## Learned facts and decision

Base commit is `1fb05138a` (`Select complete C++ compilation helper proof`).
The selected exact 666-line helper stops while resolving line 251 inside
`_module_map_struct_to_module_map_content`: `added_paths = set()` references
an absent predeclared value. No helper function was invoked and the rejected
+855 proof candidate was removed byte-for-byte.

That stop exposed category-wide drift rather than a two-call-site omission:

- loading builds `.bzl` from `[Print, StructType]` and BUILD from `[Print]`;
- root and nonroot/include MODULE evaluators independently use `[Print]`;
- REPO copies a handwritten 28-name list, adds `Print`, and replaces `set`
  with an always-failing flag-off shim;
- a DICE-reached core BUILD/MODULE evaluator uses bare
  `Globals::standard()`.

Pinned Bazel 9.2 instead owns one `Starlark.UNIVERSE` used by every file
context: `False`, `True`, `None` and 27 `MethodLibrary` functions. The
vendored evaluator supplies 28 of Bazel's 30 names through its standard globals
plus host-selected `Print` and `SetType`, but its standard environment also
contains non-Bazel `chr` and `ord`. Therefore neither
`GlobalsBuilder::standard()` nor ad hoc extension lists are a lawful complete
Bazel universe.

Run only `WP-4-5-7A-bazel-universal-builtins-environment`. Add a small
`slug_starlark_v2` crate that owns the exact name policy and process-stable
frozen registrations. Its builder filters the evaluator standard environment
through the authenticated 28-name Bazel list, then adds `Print` and `SetType`
once. Migrate all active loading, root/nonroot MODULE, REPO and core evaluator
routes. Each context appends only its existing overlay; `StructType` remains
`.bzl`-only. Cache finalized immutable context `Globals` only where every
binding is process-stable and request state continues to arrive through the
evaluator.

This packet closes the universal category, not every Bazel host global. Fixed
context utilities such as `struct`, `depset`, `select`, `json` and `proto`;
Build API declarations; and `native` members remain separate overlay
categories. After the universal packet and helper retry, select a complete
context-overlay inventory audit rather than adding future globals reactively.

## Authorities, ownership and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole exact compatibility authority. `Starlark.makeUniverse`,
`MethodLibrary`, `BuildLanguageOptions.experimental_enable_starlark_set`,
`Module.withPredeclared`, BUILD/Bzl/MODULE/REPO evaluation owners, `Param`
defaults and `testdata/set.star` authenticate names, placement and the bounded
set behavior. Vendored starlark-rust is implementation substrate, not behavior
authority.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is a peer implementation with
similar design goals. Its process-stable immutable universe, separation of
universal/predeclared/module values and no-per-evaluator universe allocation
inform this Rust design and optimization. Every adopted idea must remain
independently justified for Slug; copy no Zig code, representation, algorithm,
diagnostic or behavior.

- **Exact:** the complete 30-name Bazel 9.2 default universe as a set;
  explicit absence of `chr`/`ord`; placement in active BUILD, `.bzl`, root
  and nonroot/include MODULE, REPO and core BUILD/MODULE routes; no context
  overlay leakage; zero or one positional `set` iterable; named, over-arity,
  non-iterable and unhashable inputs reject; set type, first-insertion
  deduplication/order, membership, `add`, non-aliasing copy and module freeze.
- **Slug-native:** starlark-rust value representation, hashing and diagnostics;
  print/event plumbing; process-stable Rust storage, context caching and pointer
  sharing; callable/method behavior outside already accepted or packet-proved
  subsets.
- **Unsupported/deferred:** flag-off command plumbing and its exact diagnostic;
  exhaustive set algebra/error parity; a claim that all 27 callable ABIs are
  exact merely because their names are present; Vendor/builtins/cquery-only
  environments and incomplete context-overlay categories; the compilation
  helper and configured C++ behavior.

The central owner holds the source `Globals` for process lifetime and uses
`GlobalsStatic::populate` to allocate universal registrations once. Never copy
a `FrozenValue` from a temporary `Globals`: `AllocFrozenValue` does not add
a source-heap owner. Derived context globals may share the process-stable
universe but must contain no request, DICE, evaluator-scratch or module-retained
value. Context overlays remain their existing semantic owners.

## Allowlist, caps and proof

The scheduling selection changes only canonical/current, Stage 4, Stage 5 and
the orchestration routing log. Commit those five documents first; they must be
clean at the implementation base.

The subsequent implementation changes exactly these 16 paths:

- root `Cargo.toml` and mechanically derived `Cargo.lock`;
- new `app/slug_starlark_v2/Cargo.toml`,
  `app/slug_starlark_v2/BUILD.bazel` and
  `app/slug_starlark_v2/src/lib.rs`;
- `app/slug_bzlmod_v2/Cargo.toml` and
  `app/slug_bzlmod_v2/BUILD.bazel`;
- `app/slug_loading_v2/Cargo.toml` and
  `app/slug_loading_v2/BUILD.bazel`;
- `app/slug_core_v2/Cargo.toml` and `app/slug_core_v2/BUILD.bazel`;
- `app/slug_bzlmod_v2/src/module_eval.rs` and
  `app/slug_bzlmod_v2/src/repo_file.rs`;
- `app/slug_loading_v2/src/package.rs` and
  `app/slug_loading_v2/src/host_package_load_tests.rs`;
- `app/slug_core_v2/src/runtime/starlark.rs`.

Base authorities and physical ceilings:

| File | Base lines | Base SHA-256 | Ceiling |
|---|---:|---|---:|
| root `Cargo.toml` | 346 | `5780d166ae536f21b263cfa64115cb881771e099df80f12d21ea8ae08baa49d1` | 350 |
| `Cargo.lock` | 4,953 | `f2f76bbcfe089f7464c33cc1ff56719e3e8f9a0a11d9f897ec4a87cb5258858b` | 4,965 |
| `slug_bzlmod_v2/Cargo.toml` | 30 | `48eacfc09bf84921dc194989ff84a139f0909254a11129594fb68b8c8df22349` | 32 |
| `slug_bzlmod_v2/BUILD.bazel` | 181 | `9a3b2f95b15aa0e18ee165e76072db1d15cdd643d887d16ade0fafeaebcf6d9b` | 184 |
| `slug_loading_v2/Cargo.toml` | 26 | `a2267802f15eadbe97a5ab044b2768a3d8ebdc11f5ea62638fb35a60e067950c` | 28 |
| `slug_loading_v2/BUILD.bazel` | 102 | `7a3f1b77ed3c51209bc0744a8bb1a59d10b36fe26235c493efb0d91039fcfc2b` | 105 |
| `slug_core_v2/Cargo.toml` | 51 | `339f0c10a4abe53d688660f35c565e3a5a4e4098212d0159ba927198effd334e` | 53 |
| `slug_core_v2/BUILD.bazel` | 43 | `8f25021c7346ac366ec440f3fef0629ac65c82eb94ed8cb5d1321eddb8e13f4b` | 46 |
| `module_eval.rs` | 6,647 | `b618a135bbd954139a8f6535c59cb1256135091f923731ce7b5a842781e18449` | 6,687 |
| `repo_file.rs` | 3,567 | `e0bc8d4a89f69a254b4c52f5cad69bd640e75d9e31a46560afd40210330bad1e` | 3,607 |
| `package.rs` | 6,228 | `5af7fe599f60fef1e00a1162ee3922dcf43a32df98cdded963721fb81fa285c9` | 6,268 |
| `host_package_load_tests.rs` | 13,929 | `9922cb1b903f9a4494b5b489b1c3aeef4659b3d4f0e9cabe1a06713e2454310c` | 14,069 |
| `runtime/starlark.rs` | 105 | `c7c07a692fc1b0f10e011ab6428cfc8dd904bee38d8ff45a793647d6dce5f490` | 175 |
| new `slug_starlark_v2/Cargo.toml` | 0 | new | 20 |
| new `slug_starlark_v2/BUILD.bazel` | 0 | new | 25 |
| new `slug_starlark_v2/src/lib.rs` | 0 | new | 220 |

Each new or changed function remains at most 120 physical lines. Caps are 220
production, 300 proof and 520 total additions across exactly these 16
implementation paths; deletions, including removal of the REPO shim, do not buy
budget.

Required proof:

1. Central exact sorted 30-name set; `chr`, `ord`, `struct` and
   context-only symbols absent. Two independently built consumers resolve
   pointer-identical universal callable values from process-stable storage.
2. BUILD, `.bzl`, root/nonroot/include MODULE, REPO and core BUILD/MODULE
   consume the same universe. `struct` remains `.bzl`-only and representative
   overlay names neither disappear nor leak.
3. REPO uses the real default-enabled `SetType`; remove the always-failing shim
   and its flag-off expectations without changing REPO call-order/restriction
   ownership.
4. In representative loading contexts prove zero/one-positional ABI, invalid
   categories, type, insertion order, membership, `add`, hashability and
   exported freeze. For copy, build `copy = set(original)`, mutate one while
   both are mutable, and prove the other's contents/order do not change.
5. MODULE/REPO/core tests discriminate placement and the former missing/stale
   behavior. No test-only generic `Globals::standard()` helper is promoted to a
   production-context claim.

Run central crate tests; focused loading, MODULE, REPO and core tests; all
`slug_loading_v2` and `slug_bzlmod_v2` library tests; `bzl_invalidation`;
`build_file_loading`; locked analysis/core checks; locked CLI build;
formatting, diff, Cargo lock and archive hygiene. Rebuild `slug_cli_v2` before
any Slug-binary smoke. Measure caps/ceilings and obtain independent review of
the exact inventory, every active route, static source-heap lifetime, overlay
isolation, REPO correction, compatibility split, no vendored edit and Zabel's
peer-guidance role.

STOP and `REPLAN` for a vendored starlark-rust edit, temporary-source frozen
value, request/module state in a cache, giant all-context Globals, omitted active
route, overlay leakage, retained REPO shim, `chr`/`ord` exposure, flag
plumbing, helper-source restoration, helper invocation, DICE-key change, copied
Zabel content, dirty authority, allowlist escape or cap/function violation.
After acceptance, retry only the complete 666-line helper proof from a clean
base; stop again for any further missing global or evaluator shape.

## Immediate predecessor

Commit `1fb05138a` selected but did not accept the complete helper proof.
Commit `acca5cb68` remains the last accepted implementation slice and proves
only the dependency-free toolchain-config library freeze. The discarded helper
candidate exposed the universal-environment gap and left no Rust diff.
