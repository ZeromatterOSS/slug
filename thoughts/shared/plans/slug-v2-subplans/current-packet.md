# Current Slug V2 Packet

Packet: `WP-4-7A-rules-cc-cc-info-public-route-frontier-audit`

Milestone: M7A command/ruleset bootstrap closure.

Result: authenticate the smallest honest exact public `CcInfo` load route for
the two remaining utils exports, reconcile it with accepted provider
primitives, and select one bounded successor or record `REPLAN`. Change no Rust
or proof.

## Accepted base and completed frontier audit

Base is `7d45bee02` (`Prove exact utils compute crate name export`). The proof
owner is 9,464 lines at SHA-256
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

The remaining pair shares exact `CcInfo`. Do not select either implementation
until this packet authenticates the complete public route or records why no
bounded source-complete route exists.

## Authorities and decision

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
`ResolverTest.testBindingScopeAndIndex_functionBlock` and `..._loads`, plus
the authenticated rules sources, are sole exact authority.

Clean `../zabel` `0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only
retaining recursively reachable defining-module and loaded bindings after
freeze, as illustrated by its defining-module value-graph tests. Copy no Zig
code, representation, owner pointer, traversal/order algorithm, diagnostic,
identity or behavior.

Accepted commits `9c51999f9fd4cf22bc8c86d4eda325082e4db316` and
`152caa6fec67a2f330bed446bf7938c896df4958` prove initialized-provider and
documented-map declaration behavior, including source-shaped `CcInfo`. They do
not prove that the complete public/proxy/private module route freezes, nor may
their narrowed source-shaped proof replace that route.

Audit in exact load order:

1. authenticate the full 18-line public module and its
   `@cc_compatibility_proxy//:symbols.bzl` edge;
2. authenticate all six generated proxy loads and seven exports, classifying
   every child as accepted, bounded missing, or unbounded;
3. audit the private child from its four loads through provider declarations,
   eager empty contexts, initializer 247-258 and `CcInfo` 260-269, recursively
   classifying every evaluated child/value required before publication;
4. distinguish exact full-module feasibility from a Slug-native narrowed proof;
5. select exactly one bounded prerequisite or public-route successor, or record
   `REPLAN` with the first irreducible boundary.

## Compatibility

- **Exact:** authenticated source bytes/hashes, complete load/export edges,
  recursive evaluated-value reachability, and accepted-versus-missing facts.
- **Slug-native:** audit decomposition, packet sizing and any future proof-only
  concatenation proposal; none is accepted by this audit alone.
- **Unsupported/deferred:** a narrowed proxy/private module as an exact route;
  either remaining utils function; all invocation, configured C++ behavior,
  diagnostics, actions and analysis.

No production, DICE, identity, retained-memory, async, fixture, oracle, hot-path
or Buck2-derived utility change is involved.

## Allowlist, caps and deliverable

Only these planning files may change:

| File | Base SHA-256 | Base lines |
|---|---|---:|
| `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md` | `70cbb5f10044ab19c1871241e9c813231eba317af50033d57a575c90cf79fd38` | 4,283 |
| `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md` | `5795ef5d4862ff3745b19ead318c20aa5b8bf44e0773b8121e5c1ae36f35b81a` | 6,661 |
| `thoughts/shared/plans/slug-v2-subplans/current-packet.md` | `10c70442a02150e366b08140b81deab4f24e8d69a35a0e457d62872a34778460` | 134 |

Caps are 0 production, 0 proof and 220 planning additions; deletions do not buy
addition budget. No Rust, fixture, oracle capture, Cargo command or function
invocation is authorized. Request/revision, retained-memory, async, DICE,
fallback, performance and production-file complexity concerns are inapplicable
because this is a read-only source/plan audit.

Required deliverable:

- a complete public -> proxy -> private closure table with exact full-file and
  newly discriminating range hashes;
- accepted/missing status for every proxy child and every private eager child;
- explicit proof whether the accepted provider abstraction is sufficient or
  insufficient for full-route exactness;
- exactly one bounded successor with compatibility classes, allowlist/base,
  caps, validation and STOP conditions, or a concrete `REPLAN` boundary.

## Validation and STOP

Verify the rules_cc archive, generated proxy, pinned Bazel and Zabel authorities;
run `git diff --check`, exact three-file scope, the 220-line planning cap and
`scripts/v2_archive_status.sh` with only its three known archive-only misses.
Independent review must verify closure completeness, accepted-evidence use,
the no-stub rule, selection/`REPLAN`, and Zabel guidance-only use.

STOP for dirty/missing authority; an unclassified proxy/private child; more
than one implementation successor; Rust/test/fixture/oracle change; function
invocation; a narrowed exact-route claim; Java/JVM work; copied Zabel content;
or cap violation.

## Immediate predecessor

`7d45bee02` accepted the compute-crate-name closure with 235 unit, 24
invalidation and 31 BUILD-loading tests green. Independent review verified
exact bytes/order, visibility, eager/public identities, nonexecution and caps.
