# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-compatibility-proxy-objc-info-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze exact complete `cc/private/objc_info.bzl` and prove its two
generated-proxy aliases preserve the public initialized-provider callable,
never its private raw constructor. Invoke neither constructor nor init body and
claim no complete proxy route.

## Accepted base and completed prerequisite

Base commit is `498e5efc7` (`Admit zero argument depset loading`). The sole
allowed proof owner is 9,669 lines at SHA-256
`6d551a7bba8799f5d3c0badecc57c0ce1416e2fc5907f2a05f860e13b353cf82`.

Commit `0699dffe7` accepts exact complete `CcSharedLibraryInfo` and
`DebugPackageInfo` proxy children. Commit `498e5efc7` then admits exact empty
`depset()` in BUILD and `.bzl` globals, which satisfies all five eagerly
evaluated defaults in `_objcinfo_init`. The next child now has no missing loads
or eager builtins.

Authenticated rules_cc 0.2.17 sources:

- complete `cc/private/objc_info.bzl`: 97 lines, SHA-256
  `675fffb06e4731d2f0f4b7c9f2d38596fff042321dec5e581f73b5e44f8fde8a`;
- initializer definition lines 17-63: SHA-256
  `949ae2397a00597af07e8bf51f02c0404fed065d37896fb11eb978e979e0aa66`;
- initialized-provider declaration lines 65-96: SHA-256
  `f75ec37fe395db4db5bb3d40aaf9c36be62e4064850b2f4246cad6b3028a6711`;
- generated proxy load line 6: SHA-256
  `5fdd7f63aac8d3614d498be79508aeffe0e26961979ae6082e7a594b79c276cd`;
- proxy export lines 13-14: SHA-256
  `9ddc47bdb453d8c0d6feb4ca0749eb01c4556397c475a90bc785f00164a4d744`.

The child assigns `ObjcInfo, _new_objcinfo = provider(..., init =
_objcinfo_init)`. Public `ObjcInfo` has `provider_callable` type; private raw
`_new_objcinfo` is a distinct callable of type `function` that retains the
public provider definition and bypasses its initializer when invoked. Generated
proxy `ObjcInfo = _ObjcInfo` and the
historically named `new_objc_provider = _ObjcInfo` both alias the public value;
the proxy does not load or expose `_new_objcinfo`.

## Authorities and architecture

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
provider initializer/raw-constructor tests plus authenticated rules sources are
sole exact authority. Accepted Slug initialized-provider proofs already cover
two-value declaration, shared provider identity and distinct public/raw call
paths; this packet exercises no call path.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only defining-module
ownership: its provider definition co-owns public/raw values, and reexporting a
callable preserves definition identity. Copy no Zig code, representation,
pointer, allocator, schema storage, invocation logic, diagnostic or behavior.

Embed and evaluate the complete exact child under
`@@rules_cc+//cc/private:objc_info.bzl`. Then evaluate only exact proxy load
line 6 and export lines 13-14 under the generated compatibility-proxy producer
with apparent `rules_cc` mapped to canonical `rules_cc+`. This narrowed proxy
composition is proof-only Slug-native harnessing, not the complete module.

## Compatibility

- **Exact:** complete child bytes/producer; initializer and provider-declaration
  slices; public provider-callable/private raw-function types and distinct
  values; exact proxy slice bytes/spellings; both pointer-preserving public
  aliases and their nonidentity with the raw callable.
- **Slug-native:** narrowed three-line proxy composition and starlark-rust
  frozen representation.
- **Unsupported/deferred:** initializer or constructor invocation; field/depset
  behavior; omitted proxy children; complete proxy/public CcInfo loading;
  configured ObjC/C++ behavior, diagnostics, actions and analysis.

No production, DICE, retained representation, utility, async, fixture, oracle,
registry or Java/JVM work is involved.

## Allowlist, caps and proof

Only `app/slug_loading_v2/src/host_package_load_tests.rs` may change. Caps are
0 production, 220 proof and 220 total additions; deletions do not buy budget.
Final ceiling is 9,889 lines. Keep the test at or below 100 lines; exact source
constants are exempt.

Required proof:

1. Embed and hash the complete child, initializer/declaration slices and exact
   proxy load/export slices.
2. Freeze the child at its exact producer; prove the private initializer and
   raw constructor are functions, public ObjcInfo is a distinct provider
   callable, and invoke none of them.
3. Freeze the narrowed proxy with its actual load spelling/repository mapping;
   prove `ObjcInfo` and `new_objc_provider` both pointer-equal public ObjcInfo
   and neither pointer-equals the raw callable.
4. Assert every other proxy export remains absent.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`: focused proof; loading lib; `bzl_invalidation`;
`build_file_loading`; locked analysis/core check; locked CLI build; format,
diff, exact scope and archive status (only the known three misses).

Independent review must verify hashes, producer/mapping, public/raw types and
distinctness, both proxy aliases, noninvocation, negative boundary, caps,
compatibility classification and Zabel's guidance-only use.

STOP and `REPLAN` for production change; any constructor/init invocation;
another proxy child/export; complete-proxy/public-CcInfo claim; configured
behavior; representation/utility/DICE/registry work; Java/JVM work; copied
Zabel content; dirty authority; or cap violation.

## Immediate predecessor and successor

Commit `498e5efc7` adds 9 production and 50 proof lines. Focused proof, all 237
loading-library tests, 24 invalidation tests, 31 BUILD-loading tests,
analysis/core checks and the CLI build pass; two reviewers accept the exact
zero/no-name boundary, unchanged one-list path, caps and guidance-only Zabel
use.

After this packet, re-audit the remaining `cc_common`, private CcInfo and
toolchain-config children. Do not infer that exact public CcInfo freezes merely
because four of six proxy children are accepted.
