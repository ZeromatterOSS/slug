# Current Slug V2 Packet

Packet: `WP-4-7A-rules-rust-utils-compute-crate-name-export-loading-proof`

Milestone: M7A command/ruleset bootstrap closure.

Result: freeze exact `compute_crate_name` and its four new dependency helpers with
the accepted exact eager encoding closure, prove private/public/parent binding
identity, invoke none, and stop before the two CcInfo-dependent exports.

## Accepted base and completed frontier audit

Base is `4d037e48d` (`Prove exact utils transform sources export`). The proof
owner is 9,234 lines at SHA-256
`aa87daf38be1aa6414977c43ea07622d8ddb10acf90206a0283a8fb4b6f8d970`.
Exact loading proofs now also cover `transform_sources`, its private helper,
the actual mapped Skylib load, loaded paths struct and public parent identities.
Neither function is invoked.

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
| `compute_crate_name` 410-445, `8b79565b53edd586539f2f6697848038c598814b2706ed57fffe2c1229c0621f` | helpers 374-396 `8ef88e5e0c024de9214552db4ba8dc6e54018cf3fc52e6460d8ecd572c984c62`, 398-408 `da15bf3fe35c692ad74c76f1f80d234c0b0519697a6fee93335d3888ba745c81`, 573-595 `852e96f30111d5400489cf5512af8d27d8519f57194a3936e21600cd412b364e`, 652-662 `9347beaed27421b6f782c9f643014f8d2774dfbb9b7c83c0ed96143ac3698dc3`; accepted eager 601-650 `e0526a4d2bc5bc9d04544ecdbde305667c5a015b0c7f4597858891ae668f7b85`, 664-676 `b5ad15479c25ae84b1dba206ffc924d455003aaff98b5371773a3104f08d9027`, 692-740 `e5643897c866136bd788b242be0c983a2ae3aab511a1b7676c2d118be0200cd2` | selected 104 new exact local lines plus accepted eager closure |
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

After accepting `transform_sources`, `compute_crate_name` is the sole bounded
source-complete residual closure not blocked on exact CcInfo.

## Authorities and decision

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
`ResolverTest.testBindingScopeAndIndex_functionBlock` and `..._loads`, plus
the authenticated rules sources, are sole exact authority.

Clean `../zabel` `0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only
retaining recursively reachable defining-module and loaded bindings after
freeze, as illustrated by its defining-module value-graph tests. Copy no Zig
code, representation, owner pointer, traversal/order algorithm, diagnostic,
identity or behavior.

Concatenate the exact root and four new dependency-helper slices with accepted
exact `_substitutions`, `_encode_raw_string` and `_replace_all` slices in their
authenticated utils source order. Use a proof-only parent with actual
`:utils.bzl` spelling.

## Compatibility

- **Exact:** selected source bytes and hashes; utils and parent producers;
  symbol/load spelling; frozen function types, private visibility, accepted
  eager-value/function retention and public pointer identity.
- **Slug-native:** narrowed proof-only utils/parent modules, concatenation
  separators and starlark-rust frozen representation.
- **Unsupported/deferred:** invoking any selected or accepted helper; crate-name
  values, diagnostics and configured behavior; complete utils/parent loads; and
  the two CcInfo-dependent exports.

No production, DICE, identity, retained-memory, async, fixture, oracle, hot-path
or Buck2-derived utility change is involved.

## Allowlist, caps and proof

Only `app/slug_loading_v2/src/host_package_load_tests.rs` may change. Its base
is SHA-256 `aa87daf38be1aa6414977c43ea07622d8ddb10acf90206a0283a8fb4b6f8d970`
at 9,234 lines; final ceiling is 9,474.

Caps are 0 production, 240 proof and 240 total additions; deletions do not buy
budget. Keep the test function at or below 120 lines; exact constants are
exempt.

Required proof:

1. Embed exact unabridged utils slices 374-396, 398-408, 410-445, 573-595 and
   652-662; verify their five full audit hashes.
2. Reuse and hash-reverify accepted exact slices 601-650, 664-676 and 692-740
   without duplicating their existing constants.
3. Evaluate only the eight slices in authenticated source order under
   `@@rules_rust+//rust/private:utils.bzl`. Prove all function types; private
   `_invalid_chars_in_crate_name`; public `name_to_crate_name`,
   `should_encode_label_in_crate_name`, `encode_label_as_crate_name` and
   `compute_crate_name`; and retained accepted eager aliases; invoke none.
4. Import only `compute_crate_name` through actual `:utils.bzl` spelling
   under `@@rules_rust+//rust/private:rust.bzl` and prove pointer identity.
5. Preserve every accepted loading proof.

No new oracle is needed: authenticated source and pinned resolver tests
discriminate the selected binding closure. The large test file remains the
cohesive exact external-Bzl proof owner.

## Serial validation and STOP

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`: focused proof; loading lib; `bzl_invalidation`;
`build_file_loading`; locked analysis/core check; locked CLI build; format,
diff, exact scope and archive status (only the known three misses).

Independent review must verify hashes, source order, closure/producers, helper
visibility, accepted eager retention, pointer identities, nonexecution, caps,
preserved proofs, Zabel guidance-only use and validation.

STOP and `REPLAN` for production change; any function invocation; another helper
or export; complete utils/parent loading; crate-name/configured behavior;
identity/registry/DICE work; Java/JVM work; copied Zabel content; dirty
authority; or cap violation.

## Immediate predecessor

`4d037e48d` accepted the transform-sources closure with 234 unit, 24 invalidation
and 31 BUILD-loading tests green. Independent review verified exact bytes,
mapping, private/loaded/public identities, nonexecution and caps.
