# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-compatibility-proxy-direct-provider-children-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze the exact complete `CcSharedLibraryInfo` and `DebugPackageInfo`
child modules and prove pointer-preserving reexports through a narrowed
Slug-native proxy harness. Invoke neither provider and claim no complete proxy
route.

## Accepted base and completed frontier audit

Base scheduling commit is `242325974` (`Select public CcInfo route frontier
audit`). The unchanged proof owner is 9,464 lines at SHA-256
`8c41414a329f70c7d39c0672ecb4ed14afd35ddedccb238afea1b4c28b031df6`.
Exact loading proofs now cover every utils import except `transform_deps` and
`transform_link_deps`. The accepted crate-name closure retains five new and
three eager slices in source order with exact visibility and identities; no
function is invoked.

Authenticated sources:

- `utils.bzl`: 1,032 lines, SHA-256 `8aa49b9312d4ae5c4aed033aba65392a039a681b3ee21ca83da0f05acac28ace`;
- `providers.bzl`: 238 lines, SHA-256 `57a59ec9a60b9709df197333c94bac464b572af63bc78f560ce32570b6d84ac6`;
- exact utils provider-load block, lines 21-30: SHA-256
  `70b9766134981b2468073457ace668c656b4b00adff6df0fe4a28b079ad9c68d`.

The completed six-export audit is:

| Export/root SHA-256 | Complete closure | Classification |
|---|---|---|
| `can_build_metadata` 742-765, `4d57fbeaa3abeee124920697c17f08cd785655f3de64723f9e071bd2b50cb8eb` | accepted `can_use_metadata_for_pipelining` 766-786, `00078da9862fec4e91d5e0e4453a5395dca29f12e4bc6dd44f280a58643b0b5a`; provider 109-118, `3c21b9e0c388512de065d30fe0910e8fc6db274e6643662fb1922ce47787db8b` | accepted by `cf76c0443` |
| `generate_output_diagnostics` 967-991, `8535acbf356edec97a667da93592f211b9c0f34f5a9b88de6e0a83ac453f5bec` | provider 120-128, `a066585ff0356b5baa65fb4ddcc3fe6d5644be4facd457bf83b5eb6886324086` | accepted by `53c4d7d78` |
| `compute_crate_name` 410-445, `8b79565b53edd586539f2f6697848038c598814b2706ed57fffe2c1229c0621f` | helpers 374-396 `8ef88e5e0c024de9214552db4ba8dc6e54018cf3fc52e6460d8ecd572c984c62`, 398-408 `da15bf3fe35c692ad74c76f1f80d234c0b0519697a6fee93335d3888ba745c81`, 573-595 `852e96f30111d5400489cf5512af8d27d8519f57194a3936e21600cd412b364e`, 652-662 `9347beaed27421b6f782c9f643014f8d2774dfbb9b7c83c0ed96143ac3698dc3`; accepted eager 601-650 `e0526a4d2bc5bc9d04544ecdbde305667c5a015b0c7f4597858891ae668f7b85`, 664-676 `b5ad15479c25ae84b1dba206ffc924d455003aaff98b5371773a3104f08d9027`, 692-740 `e5643897c866136bd788b242be0c983a2ae3aab511a1b7676c2d118be0200cd2` | accepted by `7d45bee02` |
| `transform_sources` 878-917, `1006a8daf526ca60d494f691067d417db5ca34ef350bd6fcf901b8f1d5fd14c7` | helper 937-965, `c5105f745ea0032b282f9de9825bac784ebd88ec55c80c2692017038357eaaaa`; accepted `@@bazel_skylib+//lib:paths.bzl`, 320 lines, `96cce43871d8228126a12ceff771351f9030b1e9d029f2185853aa6541766a83` | accepted by `4d037e48d` |
| `transform_link_deps` 556-571, `c6b644e8f5106089ce3d4ea1cde4b336e9d2f6d32251d8d71cd085bb0b73d564` | provider 94-107, `19fe3a0cdd81693acea508531452189dcdd9f1cc7f4ab116e79839f8cf60e7af`; exact public `CcInfo` route | deferred: CcInfo proxy/private closure not bounded here |
| `transform_deps` 536-554, `6983d42fd5e829722c88c303383fd53851e7b4972afbedc187f292ed3d507eea` | providers 17-56 `84dd9796019522c4b1bb0e0b04ad880c649fb70d1e92ce0033e775d6fdfd751a`, 58-72 `40cf12f9d8124bb1c16c1a97dd686c79041d111d4f73db24e851085fbd5ff803`, 74-79 `612afb9f408e48229b616feb8fa50462db59253e9a218653da256c6c3ec2fb9f`, 81-92 `6b4bf50624be50b86ee5b3fbfc6211919fe3344602f2ff47ca91eb1fbf17adbb`, 94-107 `19fe3a0cdd81693acea508531452189dcdd9f1cc7f4ab116e79839f8cf60e7af`; exact public `CcInfo` route | deferred on the same CcInfo boundary |

The exact CcInfo edge is utils line 20, SHA-256
`3f6e30c3620c98b4cee7f2d84d921be7fd194e6579369a918c73848b8b8fd074`,
to `@@rules_cc+//cc/common:cc_info.bzl` (18 lines,
`bac2bc3024fb0bacdfa2ca8d7ac3af946f447fe397c76b29fea959a35271f3da`). That module
reexports through generated compatibility `symbols.bzl` (15 lines,
`2adedeeaaad8c0e664dc35e9bf1480b1d6dc3d7840034f9efe3ee78476fc5902`)
to the 656-line private child
`4424bb876c3f8234d7cfce20652e7ab1a7b2fc34cc2c637b1cb4313590d9f1bc`;
its provider at 260-269
(`e3cd25c06dcd4132c02b5c9a9de0f54ce56c56973a6990400644d1537aa1918b`)
retains the initializer and eager contexts. Do not replace this source-complete
edge with a stub.

The completed route audit classifies the generated proxy:

| Proxy child | Full source | Status |
|---|---|---|
| `cc_common` | 788 lines, `5e6ab737945b487759c9f039c77a066dc65bbe15cf590b566fe86029cc610762` | source-shaped wrapper behavior accepted; complete module still broad |
| `CcInfo` | 656 lines, `4424bb876c3f8234d7cfce20652e7ab1a7b2fc34cc2c637b1cb4313590d9f1bc` | declaration/eager primitives accepted; complete module not proved |
| `CcSharedLibraryInfo` | 27 lines, `5b7dcd1f20611891bbe14d77c81fb47bf564f982e238d0ed2bc78d316efdb2f1` | selected complete dependency-free child |
| `DebugPackageInfo` | 26 lines, `b22666c62cafcb12b3e1cc01d5d3ecfcd48f530cf5b915fbdcfea4abcf8d19f8` | selected complete dependency-free child |
| `ObjcInfo` | 97 lines, `675fffb06e4731d2f0f4b7c9f2d38596fff042321dec5e581f73b5e44f8fde8a` | initialized-provider child; later bounded breadth |
| `CcToolchainConfigInfo` | 143 lines, `8c522773214e202b426ae43589f59a8bdbf3af19d2e595ba8ec7ac125fef5d39` | further legacy-feature loads; later breadth |

All six loads are eager in exact generated `symbols.bzl`. The selected pair is
the smallest coherent missing family; accepting it reduces the unavoidable
proxy closure without claiming that the full proxy freezes.

## Authorities and decision

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
`ResolverTest.testBindingScopeAndIndex_functionBlock` and `..._loads`, plus
the authenticated rules sources, are sole exact authority.

Clean `../zabel` `0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only
retaining recursively reachable defining-module and loaded bindings after
freeze, as illustrated by its defining-module value-graph tests. Copy no Zig
code, representation, owner pointer, traversal/order algorithm, diagnostic,
identity or behavior.

Embed each selected child's complete exact file, not only its declaration
slice. Evaluate both under their exact `@@rules_cc+//cc/private:...` producers.
Then evaluate only exact proxy load lines 4-5 and export lines 11 and 15 under
the generated compatibility-proxy producer with the actual `rules_cc` mapping.
This narrowed proxy harness is proof composition, not the exact full module.

The exact declaration slices are `cc_shared_library_info.bzl:16-27`, SHA-256
`74a2eea6f19b2ed262b2b6537b8aab209c27c52aefa6b895c2a87e1cb6a9840f`,
and `debug_package_info.bzl:16-26`, SHA-256
`bcab9fad2a29981dba4e635e9fdb8aa41143c9a43fc9c667f69b41a40a19123a`.
Proxy loads 4-5 hash to `1706f8413c5fff47df27ed55dab5c6d4b6a6d8afaf6abe8d3831ecaa5ac27007`;
exports 11 and 15 hash to
`f7d16f06aec82de1f61b38a05fbea7e0818d388bb382460011b0624bd44718ac`
and `18858e0f3e25b8ca1ff522ae5e4518124901635e09218f6353afc0e0772a52d7`.

## Compatibility

- **Exact:** both complete child files/hashes and producers; provider-callable
  types/definition identities; proxy slice bytes/load/export spellings; and
  pointer-preserving reexports.
- **Slug-native:** the narrowed four-line proxy composition and starlark-rust
  frozen representation.
- **Unsupported/deferred:** complete generated proxy/public CcInfo loading,
  omitted proxy children, provider construction/invocation, both remaining
  utils functions, configured C++ behavior, diagnostics, actions and analysis.

No production, DICE, identity, retained-memory, async, fixture, oracle, hot-path
or Buck2-derived utility change is involved.

## Allowlist, caps and proof

Only `app/slug_loading_v2/src/host_package_load_tests.rs` may change. Its base
is SHA-256 `8c41414a329f70c7d39c0672ecb4ed14afd35ddedccb238afea1b4c28b031df6`
at 9,464 lines; final ceiling is 9,624.

Caps are 0 production, 160 proof and 160 total additions; deletions do not buy
budget. Keep the test function at or below 100 lines; exact constants are
exempt. The large proof owner remains cohesive because this is the same exact
external-Bzl freeze/reexport family; no production complexity trigger applies.

Required proof:

1. Embed and hash-verify both complete exact child files and their declaration
   slices.
2. Freeze each child under its exact producer; prove exported values have
   `provider_callable` type without constructing them.
3. Evaluate the exact two-load/two-export proxy slices with actual load spelling
   and apparent-to-canonical rules_cc mapping; prove both pointer identities.
4. Preserve every accepted loading proof and assert no omitted proxy export.

No new oracle or fixture is needed; authenticated complete sources and accepted
provider declaration semantics discriminate the selected closure.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`: focused proof; loading lib; `bzl_invalidation`;
`build_file_loading`; locked analysis/core check; locked CLI build; format,
diff, exact scope and archive status (only the known three misses).

Independent review must verify full-file/range hashes, producers/mapping,
provider types, proxy pointer identities, nonconstruction, caps, preserved
proofs, exact/Slug-native/deferred boundaries and Zabel guidance-only use.

STOP and `REPLAN` for production change; provider construction/invocation;
another proxy child/export; complete-proxy or public-CcInfo claim; configured
behavior; identity/registry/DICE work; Java/JVM work; copied Zabel content;
dirty authority; or cap violation.

## Immediate predecessor

Audit `242325974` proves that complete public CcInfo loading requires all six
eager proxy children and selects this smallest coherent missing family.
Architecture review accepts exact children plus Slug-native narrowed harness.
