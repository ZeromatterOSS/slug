# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-zero-argument-depset-loading`

Milestone: M7A command/ruleset bootstrap closure.

Result: admit Bazel's zero-argument `depset()` call as one empty frozen loading
value while preserving the accepted one-list constructor. Do not add named,
transitive, order, flattening, configured or provider behavior.

## Accepted base and frontier audit

Base commit is `0699dffe7` (`Prove direct compatibility proxy providers`). The
two allowed files are clean:

- `app/slug_loading_v2/src/provider.rs`: 1,016 lines, SHA-256
  `f76a6eb7b0ff3774ba447df25fde13917ce5e99bf54e828dfb35a1a4bec64870`;
- `app/slug_loading_v2/src/host_package_load_tests.rs`: 9,622 lines, SHA-256
  `8fd410a7f5a8f23572421297245eae6b77ab5d774ee27a66ac5f66dada3276d7`.

Commit `0699dffe7` accepts the exact complete dependency-free
`CcSharedLibraryInfo` and `DebugPackageInfo` proxy children. The next smallest
eager child is exact `cc/private/objc_info.bzl`: 97 lines, SHA-256
`675fffb06e4731d2f0f4b7c9f2d38596fff042321dec5e581f73b5e44f8fde8a`.
It has no loads, but defining `_objcinfo_init` evaluates five `depset()` default
expressions at lines 18-22. Starlark evaluates defaults when the `def` is
created, so neither a lazy-body argument nor a provider-only source slice can
make that complete module freeze.

Slug's loading `depset` callable currently uses `Arguments::positional1`, then
retains the supplied list in `StarlarkDepset { direct }`. It already has a
natural empty representation and freeze path; only the zero-argument call
shape is absent. Keep the representation and every one-list behavior intact.

## Authorities and decision

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole exact authority. `Depset.DepsetLibrary.depset` lines 565-633 declares
`direct` with default `None`; the authenticated method/signature slice hashes to
`d904f82405d29c256e3cdaeb797df48cff73d0501933282a1723ed11a3d917e0`.
`DepsetTest.testEmptyGenericType` executes `depset()` and requires empty element
type; its authenticated lines 145-154 hash to
`ab1ec25d62fb54787b5625a5564f3bd8b74dfa4449557bf872fc4825e2b2e5f0`.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` guides architecture only. Its
`generic_depset.zig` centralizes empty sets without per-call retained topology
and tests `depset()` as empty. Slug should likewise reuse its current empty
`Vec`-backed frozen value. Copy no Zig code, layout, allocator strategy, order
mask, topology, caching, identity, diagnostic or behavior.

Accept zero or one positional argument in only the existing loading callable.
The zero-positional branch must first reject every named argument, then select
an empty direct vector. The one-positional branch continues through the
existing list validation and element retention without newly validating or
interpreting names. Preserve the existing arity failure for two or more
positionals.

## Compatibility

- **Exact:** `depset()` is accepted in BUILD and `.bzl` loading and produces an
  empty frozen value of type `depset`; a function definition may retain that
  value as a default without invoking its body.
- **Slug-native:** the existing direct-only `Vec<Value>`/`Vec<FrozenValue>`
  representation, display text, allocation and freeze mechanics.
- **Unsupported/deferred:** named `direct`, `transitive`, `order`; element-type
  identity, traversal/flattening, equality/hash/order semantics, configured
  consumers, provider construction, the exact ObjcInfo module/proxy exports,
  and the complete public CcInfo route.

This changes ordinary callable dispatch over an unchanged representation. It
does not trigger the Buck2 utility-reuse skill and involves no DICE, retained
identity, async, fixture, oracle or Java/JVM production work.

## Allowlist, caps and proof

Only these files may change:

- `app/slug_loading_v2/src/provider.rs`;
- `app/slug_loading_v2/src/host_package_load_tests.rs`.

Caps are 20 production, 50 proof and 70 total additions; deletions do not buy
budget. Final ceilings are 1,036 and 9,672 lines respectively. Keep the new
test at or below 45 lines. STOP if the current empty representation cannot
serve the admitted result without new retained state or utilities.

Required proof:

1. Evaluate and freeze a top-level `depset()` through both BUILD and `.bzl`
   globals; prove exact type and empty direct contents in both placements.
2. Freeze a function with five `depset()` defaults, matching the eager ObjcInfo
   prerequisite, without invoking the function body.
3. Preserve the accepted one-list constructor and its retained element order.
4. Preserve wrong-type and excess-positional failures, and reject
   zero-positional `direct`, `transitive`, `order` and unknown named arguments,
   without claiming exact diagnostic text.

No new Bazel run is required because the pinned signature and focused source
test discriminate zero-argument acceptance. No ObjcInfo source enters this
packet.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`: focused proof; loading lib; `bzl_invalidation`;
`build_file_loading`; locked analysis/core check; locked CLI build; format,
diff, exact scope and archive status (only the known three misses).

Independent review must verify zero/one arity, empty freeze semantics, unchanged
one-list behavior, negative boundaries, caps, compatibility classification and
Zabel's guidance-only use.

STOP and `REPLAN` for another depset call shape; transitive/order/flattening;
new representation, cache, interner or retained state; ObjcInfo/provider/proxy
work; configured behavior; DICE/identity/registry work; Java/JVM work; copied
Zabel content; dirty authority; or cap violation.

## Immediate predecessor and successor

Commit `0699dffe7` adds 158 proof lines and accepts the two exact direct-provider
children plus their pointer-identical narrowed proxy reexports. Focused proof,
all 236 loading-library tests, 24 invalidation tests, 31 BUILD-loading tests,
analysis/core checks and the CLI build pass. Independent review accepts hashes,
mapping, identity, nonconstruction, caps and compatibility boundaries.

The post-commit architecture audit rejects immediate ObjcInfo selection only
because exact full-source freeze eagerly calls `depset()` five times. After
this packet, schedule the exact full ObjcInfo and narrowed proxy-alias proof;
do not combine it here or jump to the broad `cc_common`, private CcInfo or
toolchain-config children.
