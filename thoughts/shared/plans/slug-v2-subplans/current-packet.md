# Current Slug V2 Packet

Packet: `WP-4-7A-post-utils-private-helper-loaded-frontier-audit`

Milestone: M7A command/ruleset bootstrap closure.

Result: authenticate the source-complete compile/freeze closures for all six
remaining exact `rust.bzl` imports from `utils.bzl`, then select exactly one
bounded proof successor. Implement no Rust or test proof.

## Accepted base and learned facts

Base is `cdd2f68f7` (`Prove exact utils crate root export`). Exact loading
proofs now cover all seven helper-free parent imports plus the two bounded
private-helper closures `expand_dict_value_locations` and `crate_root_src`.
All remain uninvoked.

The authenticated rules_rust 0.73.0 sources remain:

- `rust/private/utils.bzl`: 1,032 lines, SHA-256
  `8aa49b9312d4ae5c4aed033aba65392a039a681b3ee21ca83da0f05acac28ace`;
- `rust/private/rust.bzl`: 1,821 lines, SHA-256
  `a645bd5db6344bd3c0997dcf73600475c0af53fb4dd025890be24b8e1e2dbfd8`;
- exact parent load `rust.bzl:40-57`: SHA-256
  `1ad3406b7c58cc7d74e1e86991fdb6aeadbd836d32926fc54eee9583295ab500`.

Six parent imports remain:

| Export | Known local root | Known dependency class |
|---|---|---|
| `can_build_metadata` | lines 742-765 | accepted `can_use_metadata_for_pipelining` plus loaded `AlwaysEnableMetadataOutputGroupsInfo` |
| `compute_crate_name` | lines 410-445 | helpers at 374-408 and 573-740 plus accepted eager substitutions |
| `generate_output_diagnostics` | lines 967-991 | loaded `RustcOutputDiagnosticsInfo` |
| `transform_deps` | lines 536-554 | loaded `DepVariantInfo`, `CrateInfo`, `DepInfo`, `BuildInfo`, `CcInfo`, and `CrateGroupInfo` |
| `transform_link_deps` | lines 556-571 | loaded `DepVariantInfo` and `CcInfo` |
| `transform_sources` | lines 878-917 | loaded `paths` plus helper at 937-965 |

This inventory is not yet a source-complete authentication of loaded provider
declarations or the full crate-name helper/eager closure. Do not schedule an
implementation from names alone.

## Authorities and method

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, pinned
`ResolverTest.testBindingScopeAndIndex_functionBlock` and
`testBindingScopeAndIndex_loads`, and authenticated rules_rust/rules_cc/
bazel_skylib sources are sole exact authority.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only how to audit
recursively reachable defining-module and loaded bindings after evaluator
closure. Copy no Zig code, representation, owner pointer, traversal/order
algorithm, diagnostic, identity or behavior.

For each remaining export:

1. authenticate the exact public-function slice;
2. recursively enumerate every same-module helper, eager value and loaded
   binding needed to compile and freeze it without invocation;
3. authenticate every newly required source slice/file and producer identity;
4. mark each dependency as already accepted, proof-only admissible, missing, or
   too broad for a bounded packet;
5. estimate the smallest honest proof and choose exactly one successor.

Prefer a coherent family only when shared loaded bindings make it smaller and
clearer than separate proofs. Do not treat loaded-provider names as arbitrary
stubs when exact child declarations are required for the claimed closure.

## Compatibility

- **Exact:** authenticated source bytes/hashes, producer/load identities,
  dependency reachability facts, and the selected successor's Bazel binding
  surface.
- **Slug-native:** audit decomposition, packet sizing and proof-only
  concatenation strategy.
- **Unsupported/deferred:** all function invocation/results/diagnostics,
  configured behavior, whole-utils/parent freeze, and every implementation not
  explicitly selected by the successor packet.

## Allowlist and caps

Only these files may change:

| File | Base SHA-256 | Base lines |
|---|---|---:|
| `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md` | `59d2a7c08ff439d08ccdb32d4c80910e0aa9ebbaab1f6a616dea0e716511b13d` | 4,153 |
| `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md` | `d14c8af627fa3457f0750abcfe6a9f36184ec8129f2fa7288c1dcf2383db72a7` | 6,558 |
| `thoughts/shared/plans/slug-v2-subplans/current-packet.md` | `4016198f307ab4de785040c6d985988369677c797561b40dbc8b851f6b0c8ef6` | 138 |

Caps are 0 production, 0 proof and 240 planning additions; deletions do not buy
addition budget. The final canonical plan and manifest must name the same one
successor packet.

Required deliverable:

- complete six-export closure table with exact line/file hashes and producer/
  load edges;
- accepted-versus-missing classification for every dependency;
- one selected bounded successor with exact/Slug-native/deferred classes,
  allowlist, base hashes/lines, production/proof/total caps, required validation
  and STOP conditions;
- explicit Bazel authority and Zabel guidance-only record.

No Rust, fixture, oracle capture, Cargo command or function invocation is
authorized. Read-only hashing and source inspection are required.

## Validation and STOP

- verify the pinned Bazel, rules_rust, rules_cc, bazel_skylib and Zabel sources
  used by the audit are clean at their recorded commits/hashes;
- `git diff --check`;
- exact three-file scope and 240-line planning cap;
- `scripts/v2_archive_status.sh` with only its three known archive-only misses;
- independent review of the closure table and selected successor.

STOP and `REPLAN` for dirty authority; missing source bytes; an unbounded or
ambiguous dependency graph; more than one selected implementation successor;
Rust/test/fixture/oracle changes; function invocation; Java/JVM work; copied
Zabel content; or cap violation.

## Immediate predecessor

`cdd2f68f7` accepted exact `crate_root_src` plus its private helper with 231
unit, 24 invalidation and 31 BUILD-loading tests green. Independent review
verified both hashes, source order, closure completeness, private visibility,
public pointer identity and non-invocation.
